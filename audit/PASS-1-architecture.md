# PASS 1 — Architecture

> Audit d'architecture (frontière Rust↔TS, responsabilités des commandes, couche
> DB, sources de vérité de l'état, couplage d'évolution). Lecture directe du code
> le 2026-07-02, worktree `dj-assistant-m6a` (branche `m6a-discogs`). **Aucune
> modification de code** — livrable = ce fichier seul. Chaque problème cite
> fichier:ligne avec extrait réel lu. Ce que je soupçonne sans preuve directe est
> isolé en fin de document (« Hypothèses non vérifiées »).

## Outillage invoqué

- **architect** (agent) — lentille design d'archi, frontière de couches, couplage,
  cohésion.
- **software-design-philosophy** (principes appliqués) — lentille « deep module »
  (interface étroite / implémentation profonde), coût de complexité déplacé vs
  réparti, information leakage entre modules.
- **clean-code** (principes appliqués) — SRP, séparation des responsabilités, DRY,
  stringly-typed vs types dédiés.

Routage CLAUDE.md respecté : domaine « architecture / refactor » → `architect` +
`software-design-philosophy` + `clean-code`. Pas d'autre skill du registre
pertinente pour une passe d'archi pure.

---

## Verdict d'ensemble

L'architecture est **globalement saine et au-dessus de la moyenne** pour un projet
à ce stade : le moteur d'analyse (`analysis/`) est un vrai deep module (un décode,
accumulateurs online, zéro I/O DB, pur), le filing est découpé en trois phases
plan/execute/commit avec le lock relâché autour de l'encode (excellent), et
`actions.rs` centralise *presque* toute l'inversion undo derrière une primitive
gardée. Les couches IPC (`ipc*.rs`) sont **majoritairement des wrappers minces**
propres — la frontière côté Rust est nette.

Le problème structurel dominant n'est **pas** une fuite de responsabilité dans un
sens ou l'autre, mais un **double-miroir manuel Rust↔TS** : contrats de type,
constantes, séparateurs, seuils et **règles métier** sont réimplémentés à la main
des deux côtés, sans codegen ni source unique, avec des commentaires qui *avouent*
la duplication (« Mirrors write_tags_full's semantics », « must match
analysis::PEAKS_WINDOW », « the backend refuses it anyway »). Aujourd'hui ils
concordent par coïncidence ou par discipline ; chaque évolution est un point de
dérive silencieuse. Deuxième problème : une **primitive d'inversion divergente**
(`ecartes::restore_track` vs `actions::revert_one_fs`) avec un **bug latent
cross-disk** réel. Troisième : l'état d'un track vit dans **trois copies** (colonnes
scalaires + `report_json` + caches JS mémoire) qui doivent être mutées ensemble.

Rien de tout cela ne bloque une commercialisation en soi, mais le double-miroir et
le bug cross-disk sont exactement le genre de dette qui produit des bugs
« impossibles à reproduire » une fois chez des utilisateurs aux configs variées
(disques multiples, templates custom, titres avec `/`).

---

## Problèmes

### [HIGH] — Primitive d'inversion « trash » dupliquée ET divergente (bug cross-disk latent)

- **Fichier:ligne** : `src-tauri/src/ecartes.rs:102-131` (restore_track) vs
  `src-tauri/src/actions.rs:105-141` (revert_one_fs, cas `"trash"`) ; primitive
  de mise en corbeille : `src-tauri/src/filing.rs:150-180` (trash_file_fs).
- **Composant/fonction** : `ecartes::restore_track` / `actions::revert_one_fs`.
- **Extrait** :
  - `actions.rs` (le « bon » chemin, cross-disk-safe) :
    ```rust
    // file was trashed via copy→verify→delete (cross-disk safe); restore the same way
    "trash" => {
        ...
        std::fs::copy(to, from)
            .map_err(|e| RevertError::Blocked(format!("copy from trash: {e}")))?;
    ```
  - `ecartes.rs:125` (l'autre chemin, **rename**) :
    ```rust
    std::fs::rename(&to, &from).map_err(|e| e.to_string())?;
    ```
  - la corbeille est **hors du root**, sur un volume potentiellement différent —
    `filing.rs:144` : `Ok(base.join("Sift").join("Trash"))` (base = `Documents`).
- **Description** : la corbeille a été déplacée vers `{Documents}/Sift/Trash`
  (cross-disk safe *par conception*, d'où le copy→verify→delete de `trash_file_fs`).
  Mais **deux** fonctions savent restaurer un fichier corbeillé :
  `actions::revert_one_fs` (via le journal/undo) utilise copy→verify ;
  `ecartes::restore_track` (bouton « Restaurer » de l'écran Écartés) utilise
  `std::fs::rename`. `rename` échoue en cross-volume (os error 17/18 sur Windows,
  `EXDEV` sur Unix). Le doc-comment d'`actions.rs:3-4` affirme pourtant « there is
  exactly one place that knows how to safely reverse work » — invariant violé.
- **Impact concret** : un utilisateur dont la source est sur `D:` et le dossier
  Documents sur `C:` peut envoyer un morceau à la corbeille (OK, copy cross-disk),
  puis le bouton « Restaurer » échoue avec une erreur OS brute, alors que l'undo
  via le journal (même action logique) réussit. Comportement incohérent + bug
  invisible en dev (tout sur un disque) mais reproductible en prod.
- **Cause probable** : la corbeille a été relocalisée hors-root après coup
  (cross-disk safety ajoutée dans `trash_file_fs`), mais `restore_track` n'a pas
  été mis à jour pour router par la primitive centralisée — il a gardé son `rename`
  d'origine (quand la corbeille était sous root, même disque).
- **Proposition de correction** : faire passer `restore_track` par
  `actions::revert_one_fs` (ou une fonction FS partagée copy→verify→delete), au lieu
  de son propre `rename`. Une seule primitive d'inversion « trash », comme le
  doc-comment le promet déjà.
- **Effort** : Faible.
- **Bénéfice attendu** : stabilité (élimine un bug cross-disk réel), cohérence
  (restore = undo), et rétablit l'invariant « une seule inversion » qui rend le
  undo raisonnable.

### [HIGH] — `std::fs::rename` pour filer dans un bin (échoue cross-disk vers la bibliothèque)

- **Fichier:ligne** : `src-tauri/src/filing.rs:367` (conformant path) et
  `src-tauri/src/filing.rs:400` (rollback_fs « move »/« trash »).
- **Composant/fonction** : `filing::execute_file` / `filing::rollback_fs`.
- **Extrait** :
  ```rust
  std::fs::rename(&plan.source, &plan.dest).map_err(|e| FilingError::Io(e.to_string()))?;
  log.push(FsLog { kind: "move", from: plan.source.clone(), to: plan.dest.clone(), meta: None });
  ```
  et le rollback :
  ```rust
  "move" | "trash" => {
      let _ = std::fs::rename(&fs.to, &fs.from);
  }
  ```
- **Description** : filer un fichier **conformant** (pas de transcodage) le déplace
  par `rename` de son dossier source vers le bin sous le root de bibliothèque. Si
  source et bibliothèque sont sur deux disques, `rename` échoue. Le chemin de
  *transcodage* (encode→dest, `filing.rs:371`) n'a pas ce souci (il écrit un
  nouveau fichier via ffmpeg), et la corbeille est explicitement copy→verify. Seul
  le **move conformant** reste fragile cross-disk. `rollback_fs` inverse aussi par
  `rename` (best-effort, erreur avalée — donc échec silencieux du rollback).
- **Impact concret** : un DJ qui range depuis `D:\Downloads` vers `C:\Music`
  (config courante) voit tout filing conformant (FLAC/AIFF/WAV déjà au bon format,
  qui sont *justement* les fichiers propres qu'on garde) échouer avec une erreur
  IO, alors que les transcodages passent. Incohérent et déroutant.
- **Cause probable** : hypothèse implicite « source et bibliothèque sur le même
  volume », jamais énoncée ni gardée.
- **Proposition de correction** : soit (a) fallback copy→verify→delete quand
  `rename` renvoie une erreur cross-device (comme `trash_file_fs` le fait déjà —
  la brique existe), soit (b) documenter et valider explicitement l'invariant
  « même disque » au moment du choix de destination et le refuser proprement.
  Réutiliser la primitive copy→verify de `trash_file_fs` est le plus cohérent.
- **Effort** : Moyen.
- **Bénéfice attendu** : stabilité (filing multi-disque fiable), cohérence avec le
  chemin corbeille déjà cross-disk-safe.

### [HIGH] — Règles métier réimplémentées côté TS (le front décide, le back re-décide)

- **Fichier:ligne** :
  1. Sémantique d'écriture des tags : `frontend/filing.ts:591-610` (`tagFieldDiffs`)
     vs `src-tauri/src/tagging.rs:39-60` (`write_tags_full`).
  2. Garde no-upscale : `frontend/filing.ts:868` vs
     `src-tauri/src/encode.rs:71-77` (`guard_no_upscale`).
  3. Format par défaut selon le rail : `frontend/filing.ts:478-480`
     (`defaultTarget`) vs `src-tauri/src/encode.rs:62-69` (`target_for`).
  4. Seuils de qualité (estKbps) : `frontend/report-view.ts:61-62` vs les bandes
     de `src-tauri/src/analysis/verdict.rs:22-31` (`min_cutoff_hz_for_bitrate`).
- **Composant/fonction** : rail de filing (front) vs domaine filing/encode/verdict
  (back).
- **Extrait** (le plus parlant — la sémantique de write_tags_full recopiée) :
  `filing.ts:591-608`, commentaire compris :
  ```ts
  /** Which displayed tag fields would CHANGE the file if written — i.e. diverge from `state.fileTags`.
   *  Mirrors write_tags_full's semantics: artist/title are ALWAYS written (compare directly), while
   *  label/year/genres are only written when non-empty ... */
  const label = labelW !== "" && labelW !== norm(f.label);
  const yearW = state.year ?? 0;
  const year = yearW > 0 && yearW !== (f.year ?? 0);
  ```
  côté Rust (`tagging.rs:39-47`) : `set_artist`/`set_title` inconditionnels, label
  `filter(|s| !s.trim().is_empty())`, year `if y > 0`. Et `filing.ts:868` :
  ```ts
  if (lossy && t !== "mp3_320")
    return `... title="Pas de surqualité depuis un fichier lossy">${TARGET_LABEL[t]}</span>`;
  ```
  avec le commentaire ligne 866-867 : « the backend refuses it anyway ».
- **Description** : quatre règles métier (quels champs de tag une écriture touche ;
  interdiction de surqualifier un lossy en lossless ; format cible auto ; barème
  cutoff→kbps) vivent **en deux exemplaires**, un en Rust (autorité réelle) et un
  en TS (pour afficher un badge/preview/désactiver un chip avant l'aller-retour IPC).
  Les commentaires reconnaissent explicitement le miroir.
- **Impact concret** : ce ne sont pas des bugs *aujourd'hui* (les deux copies
  concordent), mais chaque changement de règle côté Rust (ex. `write_tags_full`
  qui se mettrait à *effacer* un champ vide, un 5e format cible, un ajustement des
  bandes cutoff) fait **mentir silencieusement** le front : la bannière « tags non
  écrits » se déclenche ou non à tort, un chip reste grisé/actif à tort, le barème
  affiché diverge du verdict réel. Bug invisible à la revue Rust seule.
- **Cause probable** : optimisation UX légitime (feedback instantané sans IPC) mais
  implémentée par copie de logique plutôt que par exposition de la décision depuis
  le back.
- **Proposition de correction** : pour chacune, faire du back la source unique et
  la faire *remonter* plutôt que la recopier :
  - `tagFieldDiffs` → le back renvoie déjà `FileTags` ET pourrait renvoyer « quels
    champs divergeraient » (il connaît la sémantique) ; ou exposer une fonction
    partagée. À défaut, au minimum un test de contrat qui casse si les sémantiques
    divergent.
  - no-upscale / format défaut : renvoyer les cibles *valides* + la cible par
    défaut dans le contrat du track (le back les calcule déjà), au lieu de les
    recalculer en TS.
  - estKbps : dériver du même barème (exposer les bandes une fois).
- **Effort** : Moyen (par règle ; Faible chacune si on se contente de tests de
  contrat verrouillant la concordance).
- **Bénéfice attendu** : maintenabilité, élimination d'une classe entière de bugs
  de dérive, évolutivité (changer une règle en un seul endroit).

### [MEDIUM] — Double-miroir de contrats Rust↔TS entièrement manuel (pas de codegen)

- **Fichier:ligne** : `shared/contracts.ts:1-2` (« mirror of ... Keep field names
  and types in sync ... Bump when the Rust side changes »), tout le fichier miroir
  d'une dizaine de structs serde ; + types M6a **hors** contrats, inline dans
  `frontend/ipc.ts:197-216` (`Candidate`, `AppliedIdentity`) alors que leur
  autorité est `src-tauri/src/metadata/mod.rs:56-78`.
- **Composant/fonction** : couche contrat de wire.
- **Extrait** : `contracts.ts:72-99` recopie champ-à-champ `AnalysisReport`
  (`analysis/mod.rs:44-71`) ; `contracts.ts:101` : « (mirror of naming.rs /
  encode.rs / library.rs / actions.rs) ». `ipc.ts:197` définit `Candidate`
  localement, doublon de la struct Rust `Candidate`.
- **Description** : chaque type traversant la frontière est maintenu à la main des
  deux côtés. `tauri-specta` a été évalué et reporté (cf.
  `docs/ressources-externes.md`) — décision documentée et défendable. Mais dans
  l'état actuel : (a) rien ne casse le build si un champ Rust est ajouté/renommé
  sans mettre à jour le TS (dé-sérialisation silencieusement `undefined`) ; (b)
  l'incohérence de rangement des types M6a (`Candidate`/`AppliedIdentity` dans
  `ipc.ts` au lieu de `contracts.ts`) casse la convention même du fichier miroir.
- **Impact concret** : un renommage/ajout de champ Rust non répercuté produit un
  `undefined` côté front au runtime, pas une erreur de compilation — typiquement
  découvert par un utilisateur, pas par `tsc`.
- **Cause probable** : `tauri-specta` reporté (choix conscient) sans filet de
  remplacement (test de contrat / snapshot).
- **Proposition de correction** : sans réintroduire specta — ajouter un **test de
  contrat** qui sérialise un exemplaire de chaque struct Rust et vérifie la présence
  des clés attendues (miroir du test `report_serializes_to_json` déjà présent en
  `analysis/mod.rs:177`, à généraliser) ; et rapatrier `Candidate`/`AppliedIdentity`
  dans `contracts.ts` pour restaurer la convention unique.
- **Effort** : Moyen.
- **Bénéfice attendu** : maintenabilité, détection en CI d'une dérive de contrat
  au lieu d'un bug runtime.

### [MEDIUM] — Preview du nom de fichier côté TS diverge du nom réellement filé

- **Fichier:ligne** : `frontend/filing.ts:494-503` (`previewName`) vs
  `src-tauri/src/naming.rs:176-186` (`render_filename`) + `naming.rs:159-171`
  (`sanitize`) + `library::ensure_unique` (collision).
- **Composant/fonction** : preview « → nom final » du rail vs
  `naming::render_filename`.
- **Extrait** : `filing.ts:501-502` :
  ```ts
  const ext = targetExt(state.target ?? defaultTarget(state.rail));
  return `${c.artist} - ${c.title}${ver}.${ext}`;
  ```
  vs `naming.rs:181-185` :
  ```rust
  let stem = template
      .replace("{artist}", &c.artist).replace("{title}", &c.title).replace("{version}", &version_str);
  format!("{}.{}", sanitize(&stem), ext)
  ```
  et le template est **configurable** (`settings.rs:10,20` :
  `FILENAME_TEMPLATE`, défaut `"{artist} - {title}{version}"`).
- **Description** : le front construit la preview par concaténation en dur
  `artist - title (version).ext`, **sans** (a) lire le template configurable, (b)
  appliquer `sanitize()` (qui remplace `/ \ : * ? " < > |` par des espaces, cf.
  test `naming.rs:347` : `sanitize("AC/DC: Back?") == "AC DC Back"`), ni (c) le
  bump de collision `ensure_unique`. Aujourd'hui le template par défaut coïncide
  avec la concat du front — pure coïncidence.
- **Impact concret** : preuve directe — un titre `AC/DC — Back?` affiche
  `AC/DC — Back?.aiff` en preview mais est filé en `AC DC — Back.aiff`. Si un jour
  le template est exposé/modifié, toute preview ment. L'utilisateur croit ranger
  sous un nom, obtient un autre.
- **Cause probable** : preview écrite avant/indépendamment du moteur de template ;
  hypothèse « le défaut suffira ».
- **Proposition de correction** : le back expose une commande (ou étend un contrat
  existant) « rends-moi le nom final que produirait plan_file pour ce track + cette
  cible » — le front affiche ce que le back produira, jamais une reconstruction.
- **Effort** : Faible.
- **Bénéfice attendu** : UX (la preview ne ment plus), évolutivité (template custom
  possible sans casser l'affichage).

### [MEDIUM] — Colonne `real_quality` : le nom ment sur son contenu

- **Fichier:ligne** : schéma `src-tauri/src/db.rs:17` (`real_quality TEXT`) ;
  écriture `src-tauri/src/worker.rs:66` + `:76` ; lecture `src-tauri/src/queue.rs:29`
  (aliasée en `rail`).
- **Composant/fonction** : persistance analyse / read model queue.
- **Extrait** : `worker.rs:65-78` — la liste de colonnes met `real_quality=?6`, et
  le 6e paramètre passé est `rail_str(r.declared_rail)` :
  ```rust
  "UPDATE tracks SET verdict=?2, cutoff_hz=?3, bitrate=?4, declared_fmt=?5, real_quality=?6, duration=?7, ...
  ... rail_str(r.declared_rail),   // ?6
  ```
  et `queue.rs:13-15,29,40` :
  ```rust
  /// Declared rail ... Stored in `real_quality`.
  pub rail: Option<String>,
  "SELECT t.id, ..., t.real_quality, ...   // r.get(5) -> rail
  ```
- **Description** : la colonne s'appelle `real_quality` (schéma v1 : « la vraie
  qualité du fichier »), mais depuis M2b elle stocke en réalité le **rail déclaré**
  (`lossless`/`lossy`/`unknown`), relu comme `rail`. Le nom de colonne contredit son
  contenu ; un commentaire dans `queue.rs` doit expliquer le décalage.
- **Impact concret** : piège de maintenabilité — n'importe quel dev (ou l'auteur
  dans six mois) lisant `real_quality` supposera une qualité inférée, pas un rail
  déclaré. Requête ad hoc, futur rapport ou export potentiellement faux. Aucun bug
  runtime aujourd'hui, mais un champ mal nommé dans la table centrale.
- **Cause probable** : réutilisation d'une colonne v1 devenue obsolète (la « vraie
  qualité » n'est plus persistée ; elle est recalculée à l'affichage par `estKbps`)
  sans migration de renommage (les migrations sont strictement additives — choix
  sain par ailleurs, mais laisse des noms périmés).
- **Proposition de correction** : ne pas renommer la colonne physiquement (les
  migrations additives sont un bon invariant), mais soit documenter le mapping en
  un seul endroit central, soit ajouter une colonne `declared_rail` correctement
  nommée et déprécier l'usage détourné de `real_quality`. À froid : au moins un
  commentaire de schéma dans `db.rs` signalant que `real_quality` porte le rail.
- **Effort** : Faible.
- **Bénéfice attendu** : maintenabilité (le schéma ne ment plus).

### [MEDIUM] — État d'un track en trois copies mutées ensemble (colonnes + report_json + caches JS)

- **Fichier:ligne** : cache DB `report_json` — écrit `worker.rs:63-97` +
  `ipc.rs:263-272` (self-heal), invalidé `scanner.rs:88` (`report_json=NULL`) et
  `worker.rs:105-108` (sentinel `''`). Caches JS mémoire :
  `report-view.ts:673` (`reportCache`), `:345` (`decodedCache`),
  `filing.ts:556` (`releaseCache`).
- **Composant/fonction** : persistance analyse + caches front.
- **Extrait** : `worker.rs:65-96` écrit **21 colonnes scalaires ET** `report_json`
  (JSON complet du même rapport) dans un seul UPDATE ; `report-view.ts:671-673` :
  ```ts
  // In-session report cache (path → report). Backend already caches in the DB; this skips even
  // the IPC round-trip ...
  const reportCache = new Map<string, AnalysisReport>();
  ```
  `filing.ts:555-556` : « Cross-session reopen won't repopulate this (a fresh
  process starts empty). »
- **Description** : le résultat d'analyse existe simultanément en (1) colonnes
  scalaires de `tracks` (verdict, cutoff_hz, clip_pct…), (2) `report_json` (le
  même rapport sérialisé, pour rouvrir sans re-décoder), et (3) plusieurs `Map`
  JS en mémoire. Les trois doivent être invalidés/écrits **de concert**. C'est fait
  correctement aujourd'hui (`upsert_file` remet `report_json=NULL` *et* n'écrit pas
  de scalaire tant que le worker ne repasse pas ; `clearReportCache` sur
  `analysis:changed`), mais l'invariant « les 3 copies bougent ensemble » est
  implicite, réparti sur 5 sites, et non testé comme tel.
- **Impact concret** : risque de rapport périmé si un futur chemin d'écriture met à
  jour les colonnes sans regénérer/nuller `report_json`, ou oublie de purger un
  cache JS. `releaseCache` est déjà explicitement partiel (label/année perdus au
  cold reopen, comblé par un aller-retour IPC séparé). Charge cognitive élevée pour
  toute évolution du pipeline d'analyse.
- **Cause probable** : `report_json` (v5) ajouté comme cache de perf par-dessus les
  colonnes scalaires (v3) sans les remplacer — les deux coexistent, une comme
  source, l'autre comme miroir.
- **Proposition de correction** : décider d'**une** source de vérité persistée. Si
  `report_json` est complet, les colonnes scalaires ne servent que d'index/filtre
  (les garder mais les traiter comme dérivées, jamais lues comme vérité UI) ;
  documenter et tester l'invariant « scalaires + report_json écrits/nullés dans la
  même transaction ». Côté front, un seul cache clé-par-path avec une politique
  d'invalidation unique branchée sur `analysis:changed`.
- **Effort** : Moyen.
- **Bénéfice attendu** : maintenabilité, robustesse (pas de rapport périmé),
  simplicité d'évolution du pipeline.

### [MEDIUM] — Machine à états `status` (et verdict/kind/quality) en stringly-typed, dispersée

- **Fichier:ligne** : littéraux `status='pending'|'filed'|'resourcing'|'trash'`
  comptés sur **8 fichiers Rust** (`ecartes.rs` ×8, `filing.rs` ×4, `scanner.rs`
  ×4, `worker.rs` ×4, `library.rs` ×2, `queue.rs` ×2, `actions.rs` ×1,
  `sources.rs` ×1 = 26 occurrences) ; côté TS répliqués en unions
  `contracts.ts:157` (`"resourcing" | "trash"`), `:170`
  (`"convert"|"move"|"trash"|"reject"`), et re-matchés en dur
  `journal.ts:45-50` (`filterByCat`).
- **Composant/fonction** : transitions de statut / catégorisation journal.
- **Extrait** : ex. `filing.rs:517` `UPDATE tracks SET status='resourcing'` ;
  `ecartes.rs:70` `WHERE status IN ('resourcing','trash')` ; `journal.ts:46`
  `entries.filter(e => e.kind === "convert" || e.kind === "move")`.
- **Description** : quatre dimensions catégorielles (status track, verdict, kind
  d'action, quality de filtre biblio) sont des chaînes libres répétées, sans enum
  Rust ni constante partagée, et re-listées en unions TS + matches en dur. Un typo
  (`'resourcing'` vs `'re-sourcing'`) ne casse pas la compilation — il casse
  silencieusement un filtre.
- **Impact concret** : fragile à l'évolution (ajouter un statut = éditer 8+
  fichiers Rust + les unions TS + `filterByCat` + `verdictWord` sans que rien ne le
  rappelle), et un typo produit un bug silencieux (une requête ne matche plus rien).
- **Cause probable** : SQLite ne contraint pas les valeurs textuelles ; aucun type
  n'a été introduit pour ces états côté Rust (contrairement à `Verdict`/`Rail`/
  `Target` qui, eux, SONT des enums — l'incohérence est que `status`/`action.type`
  n'ont pas eu le même traitement).
- **Proposition de correction** : introduire un enum Rust `TrackStatus`
  (et `ActionKind`) avec conversion `&str` centralisée (comme `rail_str`/
  `verdict_str` existants dans `worker.rs:22-36`), et le CHECK constraint SQLite si
  souhaité. Côté TS, les unions restent (bon), mais dérivées d'un test de contrat.
- **Effort** : Moyen.
- **Bénéfice attendu** : maintenabilité, évolutivité, élimination des bugs de typo
  silencieux.

### [LOW] — Parsing titre/version réimplémenté en 3 endroits (2 Rust + 1 TS)

- **Fichier:ligne** : `src-tauri/src/naming.rs:49-67` (`parse_filename`, split
  ` - ` + trailing `(...)`), `src-tauri/src/metadata/mod.rs:86-98`
  (`split_title_version`), `frontend/filing.ts:649` (regex
  `/^(.*?)\s*\(([^()]+)\)\s*$/`).
- **Composant/fonction** : extraction du remix/version.
- **Extrait** : `metadata/mod.rs:83-85` l'avoue : « Mirrors the front's
  display-time split so a stored title and a freshly-fetched one render the same
  base + version ». `filing.ts:644-649` réextrait la même chose en JS :
  ```ts
  const m = applied.canonical.title.match(/^(.*?)\s*\(([^()]+)\)\s*$/);
  const baseTitle = m ? m[1].trim() : applied.canonical.title.trim();
  const rawVersion = (m ? m[2].trim() : null) ?? state.canonical.version;
  ```
- **Description** : trois implémentations du même « détache un `(Version)` en fin
  de titre ». Deux en Rust (une pour les noms de fichiers, une pour les titres
  Discogs) + une en TS pour l'affichage. `apply_identity` renvoie même
  `version: None` (`metadata/mod.rs:131`) en comptant sur le front pour re-splitter.
- **Impact concret** : divergence de règles possible (le TS `[^()]` refuse les
  parenthèses imbriquées, la version Rust aussi mais par un chemin différent) →
  un titre à la limite peut se splitter différemment selon le côté qui l'affiche
  vs le côté qui le persiste. Faible probabilité, mais duplication franche.
- **Cause probable** : le split est nécessaire aux trois endroits et a été écrit
  trois fois plutôt qu'exposé une fois.
- **Proposition de correction** : le back renvoie le couple `(base_title, version)`
  déjà splitté dans `AppliedIdentity` (il le calcule en `metadata/mod.rs:113`, puis
  le jette en renvoyant `version: None`) — le front n'a plus à re-parser.
- **Effort** : Faible.
- **Bénéfice attendu** : maintenabilité, une règle de split unique.

### [LOW] — Constantes/format partagés recopiés à la main entre Rust et TS

- **Fichier:ligne** :
  - `PEAKS_WINDOW` : `analysis/mod.rs:80` (privé) vs `report-view.ts:11`
    (`const PEAKS_WINDOW = 512; // must match analysis::PEAKS_WINDOW`).
  - Séparateur de genres `"; "` : `tagging.rs:57` vs `filing.ts:589`
    (`joinGenres ... join("; ")`, utilisé par la comparaison `tagFieldDiffs`).
  - Clé de setting `"library_root"` : `settings.rs:8` vs `filing.ts:40` et
    `home-sources.ts:9` (re-déclarée par fichier TS).
  - Sentinelles `FILE_IN_PLACE`/`EXTERNAL_DEST_PREFIX` : `filing.rs:19,30` vs
    `contracts.ts:7,14` (celles-ci au moins documentées comme miroir explicite).
- **Composant/fonction** : constantes de frontière.
- **Extrait** : `report-view.ts:11` `const PEAKS_WINDOW = 512; // must match analysis::PEAKS_WINDOW`
  ; `filing.ts:589` `const joinGenres = (g: string[]): string => g.map((s) => s.trim()).filter(Boolean).join("; ");`
  vs `tagging.rs:52-57` (`.join("; ")`).
- **Description** : plusieurs constantes de calcul/format sont dupliquées littéralement.
  Les sentinelles sont bien traitées (constante partagée nommée + doc « MUST stay
  identical ») ; les autres (PEAKS_WINDOW, séparateur genres, clé de setting) sont
  de simples copies dispersées.
- **Impact concret** : changer la fenêtre de peaks, le séparateur de genres ou la
  clé de setting côté Rust casse silencieusement l'affichage waveform / la
  comparaison de tags / la lecture de la racine, sans erreur de compilation.
- **Cause probable** : pas de module de constantes partagées côté TS pour les
  valeurs qui ne sont pas des types.
- **Proposition de correction** : centraliser les constantes TS partagées dans
  `shared/contracts.ts` (comme les sentinelles le sont déjà) et, côté séparateur
  genres, l'exposer plutôt que le comparer par valeur.
- **Effort** : Faible.
- **Bénéfice attendu** : maintenabilité, moins de couplage caché.

### [LOW] — Helpers dupliqués (DRY) des deux côtés

- **Fichier:ligne** :
  - `esc` (échappement HTML) copié à l'identique dans **9 fichiers front** :
    `sift-live.ts:136`, `report-view.ts:49`, `filing.ts:42`, `library-detail.ts:21`,
    `ecartes-view.ts:10`, `empty-state.ts`, `home-sources.ts:11`,
    `batch-tracklist.ts`, `identify-shared.ts:8` — alors que `dom.ts` (helpers DOM
    partagés) existe et pourrait l'héberger.
  - `strip_verbatim` dupliqué octet-pour-octet : `sources.rs:9-16` et
    `watcher.rs:49-57`.
  - `ext_of` dupliqué : `encode.rs:80-86` et `filing.rs:94-100`.
- **Composant/fonction** : utilitaires transverses.
- **Extrait** : `identify-shared.ts:8-11` (une des 9 copies identiques) :
  ```ts
  const esc = (s: string) =>
    s.replace(/[&<>"']/g, (c) =>
      c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
    );
  ```
  `watcher.rs:49-57` et `sources.rs:9-16` définissent `strip_verbatim` à l'identique.
- **Description** : petits helpers purs recopiés au lieu d'être partagés. `esc` en
  particulier est un helper de sécurité (échappement) : une correction (ex. gérer
  un nouveau caractère) devrait se faire en un seul endroit, pas neuf.
- **Impact concret** : maintenabilité ; risque qu'une correction de `esc` (sécurité)
  n'atteigne pas tous les sites. Faible en soi mais neuf copies d'un helper de
  sécurité est un red flag.
- **Cause probable** : extraction de modules (audit P-3) faite en gardant les
  helpers locaux plutôt qu'en les hissant dans `dom.ts`.
- **Proposition de correction** : exporter `esc` depuis `dom.ts` et l'importer
  partout ; extraire `strip_verbatim` et `ext_of` dans un module Rust partagé (un
  `util.rs` ou dans `naming`/`scanner` selon l'affinité).
- **Effort** : Faible.
- **Bénéfice attendu** : maintenabilité, un seul point de correction pour
  l'échappement HTML.

### [LOW] — `library.rs` mélange deux responsabilités (FS des bins + read-model DB biblio)

- **Fichier:ligne** : `src-tauri/src/library.rs:1-11` (doc « Pure filesystem work »)
  puis `:13-278` qui contient AUSSI `LibraryTrack`/`list_filed`/`folder_facets`
  (requêtes DB read-only M6b) — le module l'admet : `:6-7` « Also exposes
  list_filed / folder_facets ».
- **Composant/fonction** : `library.rs`.
- **Extrait** : `library.rs:1-7` :
  ```rust
  //! The destination bins: every subdirectory (recursive) under the configured library
  //! root. Walks the tree with `walkdir` ... Pure filesystem work; ...
  //!
  //! Also exposes `list_filed` / `folder_facets` for the M6b library browser (read-only
  //! DB queries over the `filed` tracks).
  ```
- **Description** : un module dont la doc dit « pure filesystem work » contient
  aussi le read-model DB de l'écran Bibliothèque. Deux responsabilités (gestion FS
  des dossiers de destination vs projection SQL des tracks filés) dans un fichier,
  contradiction interne assumée dans le doc-comment.
- **Impact concret** : cohésion faible ; le fichier grossit (488 lignes) en mêlant
  I/O disque et SQL. SRP entamé mais sans conséquence runtime.
- **Cause probable** : M6b (browser) greffé sur le module qui possédait déjà la
  notion de « dossier/bin ».
- **Proposition de correction** : scinder le read-model biblio (`list_filed`,
  `folder_facets`, `LibraryTrack`, `LibraryFacets`, `LibraryFilter`) dans un
  `library_read.rs` (ou `biblio.rs`), laissant `library.rs` = FS des bins pur.
- **Effort** : Faible.
- **Bénéfice attendu** : cohésion, lisibilité, SRP.

### [LOW] — Paramètre `root` mort dans le chemin corbeille

- **Fichier:ligne** : `filing.rs:150` (`fn trash_file_fs(_root: &Path, ...)` — root
  ignoré), threadé depuis `filing.rs:184` (`move_to_trash(conn, root, ...)`),
  `filing.rs:537` (`trash_track(conn, root, ...)`), `ipc_filing.rs:430`
  (`library_root(&conn)` juste pour le passer).
- **Composant/fonction** : chemin de mise en corbeille.
- **Extrait** : `filing.rs:150` `fn trash_file_fs(_root: &Path, track_id: i64, source: &str) -> Result<String, FilingError>`
  ; `filing.rs:191` `let dest = trash_file_fs(root, track_id, source)?;` ;
  `ipc_filing.rs:430` `let root = library_root(&conn)?;` juste avant l'appel à
  `filing::trash_track(&conn, &root, track_id)`.
- **Description** : depuis que la corbeille est centralisée hors-root
  (`{Documents}/Sift/Trash`), `trash_file_fs` n'utilise plus son `root` (préfixé
  `_root`). Le paramètre est pourtant toujours résolu (`library_root`) et threadé à
  travers 3 fonctions. `trash_track` exige donc un root configuré (`NoLibraryRoot`)
  pour une opération qui n'en a plus besoin.
- **Impact concret** : plomberie morte + une pré-condition inutile (impossible de
  jeter à la corbeille sans root configuré, alors que la corbeille n'est plus
  sous root). Confusion à la lecture.
- **Cause probable** : relocalisation de la corbeille sans nettoyage des signatures.
- **Proposition de correction** : retirer `root` de `trash_file_fs`/`move_to_trash`/
  `trash_track` et lever la pré-condition `library_root` pour `trash_track`.
- **Effort** : Faible.
- **Bénéfice attendu** : simplicité, suppression d'une pré-condition trompeuse.

## Réponses directes aux 5 questions

**Q1 — Frontière Rust↔front nette, ou fuite de logique ?**
Côté Rust, la frontière est nette : `ipc_filing.rs`/`ipc.rs`/`ipc_identify.rs`/
`ipc_library.rs` sont des wrappers minces (lock → délègue au domaine → emit
`queue:changed`). Presque aucune UI ne fuit côté Rust (les erreurs sont des codes
sentinelles stables `NO_TOKEN`/`NoLibraryRoot`/`RATE_LIMITED:<s>` — bon design,
`ipc_identify.rs:1-4`, `metadata/mod.rs:150-158`). **En revanche, de la logique
métier fuit côté TS** : sémantique d'écriture des tags (`filing.ts:591-610`),
garde no-upscale (`filing.ts:868`), format par défaut (`filing.ts:478-480`), barème
cutoff→kbps (`report-view.ts:61-62`), split titre/version (`filing.ts:649`),
catégorisation journal (`journal.ts:45-50`) — tous des miroirs de règles Rust. Voir
findings HIGH#3, LOW#1.

**Q2 — 43 commandes : SRP ou god commands ?**
Chaque commande fait bien une chose. `file_batch` détache proprement en thread de
fond et **ne fait pas** le travail lui-même (`ipc_filing.rs:251-275`) ; le vrai
travail est `run_file_batch` (orchestration légitime : boucle plan/execute/commit
par fichier + cancel + progress). `apply_tags` (`ipc_filing.rs:167-211`) enchaîne
lecture-path / snapshot / write / journal — 4 étapes mais toutes autour d'**une**
responsabilité (écrire les tags de façon revertable), bien commentées phase par
phase. `identify` (`ipc_identify.rs:13-31`) = reconcile + recherche réseau, cohérent.
**Pas de god command.** Le seul point d'attention est `add_source` (`ipc.rs:54-71`)
qui insère, spawn un scan, PUIS relit la liste sous un 2e lock — acceptable.

**Q3 — Accès SQLite centralisé ou dispersé ?**
**Dispersé.** 177 appels `conn.execute/prepare/query_row` sur **18 fichiers**
(worker 12, filing 25, actions 31, ecartes 27, scanner 11, sources 11, library 7,
db 13, metadata/mod 10, ipc 6, genres 4, queue 3, settings 2, ipc_* 6, watcher 1).
Il n'y a **pas** de couche repository ; chaque module fait son SQL en ligne. Ce
n'est pas forcément un défaut (Rust + rusqlite, requêtes ciblées, testées), mais le
modèle « une connexion `Mutex`-partagée process-wide » a **deux entorses** : (a)
`spawn_scan` ouvre une **seconde connexion** indépendante (`ipc.rs:333`,
justifié — un walkdir ne doit pas geler le lock, WAL le permet), et (b) le watcher
tient le lock partagé pendant une **rafale** d'événements FS (`watcher.rs:115-158`).
Donc « tout le monde passe par le Mutex » est faux : le scan bypasse, le watcher
sérialise tout. Le commentaire de `db.rs:134` (« prep for moving off the
single-connection model ») décrit un futur, pas le présent — mais WAL sert déjà à
`spawn_scan`.

**Q4 — État front : source unique traçable, ou divergence possible ?**
**Divergence possible et déjà partielle.** Côté persistance, l'état d'un track vit
en 3 copies (colonnes scalaires + `report_json` + caches JS) mutées ensemble par
convention non testée (MEDIUM « État d'un track en trois copies »). Côté front,
l'état est **fragmenté** : `filing.ts` a un objet `state` (`filing.ts:84-100`) pour
le mode Détail, PLUS des variables module `binPick`/`releaseCache`
(`filing.ts:245,556`), PLUS `sift-live.ts` tient ~10 globals séparés
(`currentItems`, `reviewMode`, `batchSel`, `batchFakeSel`, `batchCollapsed`,
`batchInPlace`, `batchFormat`, `batchTrackIds`, `batchBin`, `bibState` —
`sift-live.ts:78-118`). `releaseCache` est explicitement partiel (`filing.ts:555` :
« Cross-session reopen won't repopulate this »). La DB reste la source de vérité de
fond (le front re-fetch sur `queue:changed`/`analysis:changed`), mais l'état
d'interaction est éclaté sur 3 lieux sans coordinateur unique.

**Q5 — Ajouter un champ d'analyse / un critère de filing : combien de fichiers ?**
Trace de `cutoff_hz` (champ existant) : `analysis/spectrum.rs` (calcul) →
`analysis/mod.rs:53,132` (struct + populate) → `analysis/verdict.rs:34` (usage) →
`db.rs:70` (colonne migration v3) → `worker.rs:66` (UPDATE persist) →
`contracts.ts:81` (miroir TS) → `report-view.ts:108-118,262` (affichage +
spectrogramme). **Soit 6-7 fichiers pour un champ scalaire d'analyse**, dont
**2 miroirs manuels** (worker persist + contracts.ts) qui ne cassent pas le build
s'ils sont oubliés. Un critère de filing (ex. nouveau format cible) toucherait
`encode.rs` (enum `Target` + `ext`/`rail`/`target_for`), `naming.rs` (rien si
template), `filing.ts` (`Target` union, `TARGET_LABEL`, `targetExt`,
`defaultTarget`, chip render) + `contracts.ts` — ~6 fichiers dont 4 côté TS en
miroir. Le couplage d'évolution est **modéré mais amplifié par le double-miroir
manuel** : ce n'est pas le nombre de fichiers qui est le risque, c'est que la moitié
sont des copies non vérifiées par le compilateur.

## Ce qui est bien conçu (à préserver)

- **Moteur d'analyse** (`analysis/`) : deep module exemplaire. Un décode
  (`decode.rs`) → accumulateurs online purs (`dynamics`/`peaks`/`phase`/`spectrum`/
  `structure`), zéro I/O DB, verdict pur et testé (`verdict.rs`). Interface étroite
  (`analyze(path, with_spectrogram)`), implémentation profonde.
- **Filing trois phases** (`filing.rs` plan/execute/commit, `ipc_filing.rs:9-11`) :
  le lock DB est relâché autour de l'encode multi-secondes, en interactif comme en
  batch détaché. C'est la bonne réponse au « une seule connexion Mutex ».
- **`commit_file` transactionnel** (`filing.rs:422-447`) : rollback FS + suppression
  du journal partiel sur erreur DB — pas d'état à moitié filé.
- **`revert_batch`** (`actions.rs:179-248`) : inversion gardée, LIFO-safe,
  partial-failure-consistent et re-tryable, distingue tag-only de filing. (Sauf
  l'entorse `restore_track` — HIGH#1.)
- **`safe_join`** (`library.rs:241-257`) : anti-traversal solide, avec le seul trou
  volontaire (`EXTERNAL_DEST_PREFIX`) documenté et re-validé.
- **Migrations additives versionnées** (`db.rs`) : jamais de DROP/rename, testées
  (idempotence comprise). Invariant sain.
- **`MetadataProvider` trait** (`metadata/mod.rs:162-165`) : seam propre pour un
  futur provider (AcoustID/MusicBrainz) sans toucher l'appelant.
- **Codes d'erreur sentinelles stables** au lieu de messages bruts traversant l'IPC.

## Hypothèses non vérifiées (à confirmer dans une passe ultérieure)

1. **Perte d'écriture concurrente entre `spawn_scan` (2e connexion) et le worker
   (connexion partagée) sous forte charge.** WAL + `busy_timeout=5000` sérialisent
   les writers, mais absence de `SQLITE_BUSY` sur un scan de plusieurs milliers de
   fichiers pendant que le pool d'analyse écrit non prouvée. *Comment vérifier* :
   instrumenter/tester un scan massif concurrent au worker, observer les retours
   `busy`. Preuve pour l'instant : `ipc.rs:340` pose seulement `busy_timeout` sur la
   connexion de scan (pas WAL explicite — hérité du fichier), `db.rs:136` pose WAL
   sur la connexion principale.
2. **Le watcher tenant le lock pendant une rafale gèle-t-il les autres IPC ?**
   `watcher.rs:115-158` verrouille `Mutex<Connection>` puis boucle sur tous les
   events avant `drop(conn)`. Sur une grosse rafale (copie de dossier), la durée du
   lock pourrait bloquer une commande UI. *Comment vérifier* : mesurer la durée
   réelle du lock watcher sur une rafale de N fichiers vs latence IPC pendant ce
   temps.
3. **Index manquants sur `actions.batch_id`/`session_id`/`type` (chemin chaud du
   Journal).** `actions.rs:299-312` filtre/joint dessus sans index dédié (déjà
   soupçonné en PASS-0). *Comment vérifier* : `EXPLAIN QUERY PLAN` sur
   `list_journal` avec un journal de milliers d'actions.
4. **Le `report_json` peut-il devenir périmé par rapport aux colonnes scalaires ?**
   Pas de chemin identifié qui écrive les scalaires sans (re)générer/nuller
   `report_json`, mais audit non exhaustif de tous les UPDATE de `tracks`.
   *Comment vérifier* : grep de tous les `UPDATE tracks SET ...verdict/cutoff...`
   et vérifier que chacun touche aussi `report_json`.

## Récapitulatif

| Priorité | Nombre |
|---|---|
| Critical | 0 |
| High | 3 |
| Medium | 4 |
| Low | 4 |

Total : 11 problèmes recensés (le résumé initial de l'agent annonçait 13 en
comptant les 2 sous-cas HIGH#1/#2 séparément dans une numérotation différente ;
le décompte ci-dessus reflète les entrées effectivement détaillées dans ce
document). Le point le plus significatif pour la suite de l'audit (Pass 8/9) :
le double-miroir Rust↔TS n'est pas un simple défaut de style, c'est une source
de divergence silencieuse touchant directement la crédibilité du verdict
(cutoff↔kbps, cf. aussi PASS-2 qui documente la même duplication sous l'angle
qualité de code avec des valeurs numériques concrètement différentes).
