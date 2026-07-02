# PASS 2 — Qualité du code

> Outillage invoqué : principes `rust-best-practices` (idiomatique Rust,
> ownership, `Result`), `error-handling-patterns` (fail-fast vs fallback
> silencieux), `clean-code` (SRP, naming, fonctions courtes),
> `pragmatic-programmer` (DRY, une seule source de vérité). Audit seul —
> aucun fichier source modifié.

## 1. Gestion d'erreurs Rust — `unwrap()`/`expect()` en production

**Comptage exact** (grep `\.unwrap\(\)` et `\.expect\(` sur tout
`src-tauri/src/**/*.rs`) : **454 occurrences brutes** réparties sur 21
fichiers.

Après lecture ligne par ligne de chaque occurrence hors `#[cfg(test)]` :

- `filing.rs` : toutes les occurrences sont dans `mod tests`
  (`#[cfg(test)]` en tête de module) — **0 survivant**.
- `actions.rs:194` — seule occurrence hors test :
  ```rust
  188    if rows.is_empty() {
  189        return Err(RevertError::Blocked(format!("no live actions for batch {batch_id}")));
  190    }
  191
  192    let max_id = rows.iter().map(|r| r.0).max().unwrap();
  ```
  Non dangereux : `rows.is_empty()` est déjà exclu par le retour anticipé
  ligne 189-191, donc `.max()` sur un itérateur non-vide ne peut jamais
  retourner `None`. Prouvé sûr par le flot de contrôle local.
- 19 autres fichiers de production (`ipc.rs`, `ipc_filing.rs`,
  `ipc_identify.rs`, `ipc_library.rs`, `encode.rs`, `tagging.rs`,
  `scanner.rs`, `db.rs`, `sources.rs`, `settings.rs`, `genres.rs`,
  `queue.rs`, `dedup.rs`, `fingerprint.rs`, `ecartes.rs`,
  `analysis/mod.rs`, `analysis/decode.rs`, `metadata/discogs.rs`,
  `metadata/mod.rs`, `metadata/cover.rs`, `naming.rs`) : **aucune**
  occurrence hors `#[cfg(test)]`, confirmé fichier par fichier.
- `lib.rs` (bootstrap `setup()`) : 4 `.expect(...)`, acceptables (panic au
  démarrage documenté comme tolérable) :
  ```rust
  52    let dir = app.path().app_data_dir().expect("no app data dir");
  54    let conn = db::open(&dir.join("sift.db")).expect("db open failed");
  64    settings::set(&conn, settings::CURRENT_SESSION_ID, &session_id).expect("session_id write failed");
  119   .expect("error while running tauri application");
  ```
- **Aucun `.lock().unwrap()` / `.lock().expect(...)`** trouvé dans tout
  `src-tauri/src` — tout le code passe par
  `.lock().map_err(|e| e.to_string())?`. Pas de risque de panic par mutex
  poisoning.

**Bilan** : 454 occurrences brutes → 450 dans les tests, 4 dans le
bootstrap (acceptables), **0 survivant dangereux** atteignable par une
entrée utilisateur (fichier malformé, réponse Discogs, DB corrompue,
chemin exotique). Verdict : discipline exceptionnelle sur ce point précis,
rien à corriger.

## 2. Fallbacks silencieux (violation fail-fast)

- **Priorité : High**
- **Fichier** : `src-tauri/src/dedup.rs:72-78`
- **Composant** : `find_duplicate`
- **Extrait** :
  ```rust
  72  pub fn find_duplicate(conn: &Connection, track_id: i64) -> rusqlite::Result<Option<DupMatch>> {
  73      let path: String = match conn
  74          .query_row("SELECT path FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
  75      {
  76          Ok(p) => p,
  77          Err(_) => return Ok(None),   // avale TOUTE erreur SQL, pas seulement "no rows"
  78      };
  ```
- **Description** : une vraie erreur DB (connexion cassée, table
  verrouillée) est confondue avec « track introuvable ».
- **Impact concret** : le front reçoit `None` (« pas de doublon ») au lieu
  d'un signal d'erreur distinct — un dédoublonnage peut silencieusement ne
  jamais se déclencher sur une erreur DB transitoire.
- **Cause probable** : `match ... Err(_) => return Ok(None)` au lieu de
  distinguer `QueryReturnedNoRows` des autres erreurs.
- **Correction proposée** : ne matcher `Ok(None)` que sur
  `rusqlite::Error::QueryReturnedNoRows`, propager les autres erreurs.
- **Effort** : Faible.
- **Bénéfice** : erreurs DB visibles au lieu d'un faux-négatif silencieux.

---

- **Priorité : High**
- **Fichier** : `src-tauri/src/dedup.rs:134-154`
- **Composant** : `get_or_compute_fp`
- **Extrait** :
  ```rust
  134 fn get_or_compute_fp(conn: &Connection, track_id: i64, path: &str) -> Option<Vec<u32>> {
  135     let cached: Option<String> = conn
  136         .query_row("SELECT fingerprint FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
  137         .ok().flatten();
  ...
  144     match fingerprint::compute_for_path(path) {
  145         Ok(fp) => {
  146             let _ = conn.execute(   // erreur d'écriture du cache avalée silencieusement
  147                 "UPDATE tracks SET fingerprint=?2 WHERE id=?1",
  148                 params![track_id, fingerprint::encode(&fp)],
  149             );
  150             Some(fp)
  151         }
  152         Err(_) => None,   // fichier illisible/corrompu → traité comme "pas de fingerprint"
  153     }
  154 }
  ```
- **Description** : un fichier audio corrompu, ou une erreur d'écriture DB
  lors du cache du fingerprint, ne génère aucun log ni signal.
- **Impact concret** : dédoublonnage dégradé silencieusement — le code de
  scoring retombe sur `("name", 1.0)` (match par nom, score arbitraire)
  sans que rien n'indique que le fingerprinting a échoué.
- **Cause probable** : `Err(_) => None` sans log, `let _ = conn.execute(...)`
  sans log d'échec d'écriture.
- **Correction proposée** : logger (`log::warn!`) les deux cas d'échec
  avant de dégrader.
- **Effort** : Faible.
- **Bénéfice** : diagnosticabilité d'un dédoublonnage dégradé.

---

- **Priorité : Medium**
- **Fichier** : `src-tauri/src/filing.rs:219-232`
- **Composant** : `load_tag_extras`
- **Extrait** :
  ```rust
  219 pub fn load_tag_extras(conn: &Connection, track_id: i64) -> TagExtras {
  220     TagExtras {
  221         label: conn
  222             .query_row("SELECT label FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<String>>(0))
  223             .ok().flatten(),
  ```
- **Description** : une vraie erreur DB (pas juste « pas de ligne ») produit
  silencieusement un `TagExtras` vide.
- **Impact concret** : le fichier sera filé/taggé sans label/année/genres,
  sans que rien ne le signale à l'utilisateur ni au log.
- **Correction proposée** : distinguer absence de ligne (attendu) vs erreur
  DB réelle (à propager/logger).
- **Effort** : Faible.
- **Bénéfice** : évite un filing silencieusement dégradé.

---

- **Priorité : Medium**
- **Fichier** : `src-tauri/src/metadata/discogs.rs:330-336`
- **Composant** : gestion d'erreur de `fetch_tracklist`
- **Extrait** :
  ```rust
  330  // Best-effort: a rate-limited/failed detail call just leaves this candidate
  331  // unscored (ranking falls back to format relevance). Log the rate-limit so a
  332  // sluggish search is diagnosable rather than looking like a normal result.
  333  Err(ProviderError::RateLimited { .. }) => {
  334      log::warn!("Discogs tracklist rate-limited; ranking falls back to format relevance");
  335  }
  336  Err(_) => {}   // toute AUTRE erreur (parse JSON, réseau) avalée sans log
  ```
- **Description** : le cas rate-limit est loggé (bon pattern, assumé et
  documenté), mais toute autre erreur (parse JSON malformé, timeout réseau,
  404) est avalée sans aucun log.
- **Impact concret** : un classement Discogs dégradé sans cause
  diagnosticable en dehors du cas rate-limit.
- **Correction proposée** : ajouter un `Err(e) => log::warn!("tracklist fetch failed: {e}")` générique avant/à la place de `Err(_) => {}`.
- **Effort** : Faible.
- **Bénéfice** : parité de diagnosticabilité avec le cas rate-limit déjà traité.

---

- **Priorité : Medium**
- **Fichier** : `src-tauri/src/ipc_filing.rs:348-367`
- **Composant** : `run_file_batch`
- **Extrait** :
  ```rust
  348  Ok(p) => p,
  349  Err(_) => {
  350      needs_validation.push(id);
  351      continue;   // la FilingError originale (Upscale/Encode/Tag/Io/Db) n'est jamais loggée
  352  }
  ...
  361  let log = match filing::execute_file(&plan) {
  362      Ok(l) => l,
  363      Err(_) => {
  364          needs_validation.push(id);
  365          continue;   // idem — erreur d'encode FFmpeg perdue
  366      }
  367  };
  ```
- **Description** : le routage vers `needs_validation` est un comportement
  voulu et documenté, mais l'erreur réelle (pourquoi le plan/encode/commit a
  échoué) est jetée sans log. Contraste avec `worker.rs::persist_result`
  (loggue explicitement chaque échec) — pattern non appliqué uniformément.
- **Impact concret** : un échec de batch de filing sur N tracks ne laisse
  aucune trace de la cause exacte (upscale/encode/tag/IO/DB) dans les logs.
- **Correction proposée** : logger l'erreur avant `continue`, garder le
  routage `needs_validation` inchangé.
- **Effort** : Faible.
- **Bénéfice** : diagnosticabilité des échecs de batch.

## 3. Duplication de mécanisme

- **Priorité : Low**
- **Fichier** : `src-tauri/src/tagging.rs:27,83,125,155` +
  `src-tauri/src/analysis/tags.rs:39`
- **Description** : l'appel `Probe::open(path).and_then(|p| p.read())`
  apparaît **5 fois** dans le code de production (4× dans `tagging.rs`,
  1× dans `analysis/tags.rs`), jamais factorisé en une fonction commune.
  Chaque site reformule le même message d'erreur (`"read tags: {e}"`
  répété 4 fois identiquement).
- **Impact concret** : mineur — maintenance dupliquée si le comportement de
  lecture de tags doit changer (ex: ajouter un fallback ou un timeout).
- **Correction proposée** : extraire `fn open_tagged(path) -> Result<TaggedFile, String>`.
- **Effort** : Faible.
- **Bénéfice** : un seul point de vérité pour l'ouverture lofty.

Le décodage audio (Symphonia), en revanche, **n'est pas dupliqué en
mécanisme** : `analysis::analyze()` et `fingerprint::compute_for_path()`
appellent tous les deux `analysis::decode::decode_pcm()` — le décodeur est
bien partagé. Il reste néanmoins un doublon d'**I/O disque** (le fichier est
redécodé intégralement pour le fingerprint après l'avoir déjà été pour
l'analyse, sans réutilisation du buffer PCM) — cf. `audit/PASS-4-perfs.md`
pour le traitement de cet aspect sous l'angle performance.

## 4. Duplication de seuils Rust ↔ TS

- **Priorité : High**
- **Fichier** : `src-tauri/src/analysis/verdict.rs:15-16,22-31` vs
  `frontend/report-view.ts:59-62`
- **Extrait Rust** :
  ```rust
  pub fn min_cutoff_hz_for_bitrate(kbps: u32) -> f32 {
      match kbps {
          b if b >= 320 => 19000.0,
          b if b >= 256 => 18000.0,
          b if b >= 192 => 16500.0,
          b if b >= 160 => 15500.0,
          b if b >= 128 => 14500.0,
          _ => 12000.0,
      }
  }
  ```
  (plus `LOSSLESS_OK_HZ = 20000.0` / `LOSSY_CLIFF_HZ = 19500.0`, lignes 15-16)
- **Extrait TS** :
  ```ts
  // measured low-pass cutoff (LAME-style: 16k≈128, 17k≈160, 19k≈192, 20k≈256, 20.5k≈320).
  const estKbps = (hz: number) =>
    hz >= 20000 ? 320 : hz >= 19000 ? 256 : hz >= 18000 ? 192 : hz >= 16500 ? 160 : 128;
  ```
- **Description** : c'est la fonction miroir inverse (Hz → kbps estimé côté
  TS, kbps → Hz minimum côté Rust), mais avec des seuils **différents**, pas
  juste réexprimés : 320↔20000 (TS) vs 320↔19000 (Rust) ; 256↔19000 (TS) vs
  256↔18000 (Rust) ; 192↔18000 (TS) vs 192↔16500 (Rust) ; 160↔16500 (TS) vs
  160↔15500 (Rust). Aucun de ces seuils n'est transmis par IPC — le front
  les recalcule en dur, indépendamment de la source Rust.
- **Impact concret** : l'estimation « MP3 ≈ X kbps » affichée en Revue peut
  être incohérente avec le verdict Rust qui a produit `cutoff_hz` — un
  même fichier peut afficher un kbps estimé qui ne correspond pas au seuil
  qui a réellement déterminé son verdict.
- **Cause probable** : deux tables de seuils maintenues indépendamment sans
  référence croisée ni transmission IPC de la table source.
- **Correction proposée** : exposer la table de seuils (ou directement le
  kbps estimé) depuis Rust via IPC plutôt que de la dupliquer en TS ; à
  défaut, un commentaire croisé nommant l'autre fichier comme source de
  vérité.
- **Effort** : Faible à Moyen (selon si on expose via IPC ou juste
  aligne les constantes).
- **Bénéfice** : cohérence garantie entre le verdict et son explication
  affichée à l'utilisateur.

Autres seuils vérifiés **non dupliqués** côté TS (recherchés explicitement,
absents du frontend) : rate-limit Discogs (60/25 req/min, uniquement
documenté, pas en dur dans le code), `HTTP_TIMEOUT = 15s`
(`discogs.rs:13`), `SEGMENT_SCORE_MAX = 8.0` / `MATCH_THRESHOLD = 0.6`
(`fingerprint.rs:10,12`), `FFT_SIZE`/`PEAKS_WINDOW`/`CLIP_THRESHOLD`/
`SILENCE_THRESHOLD` (`analysis/mod.rs:79-83`).

## 5. Code mort

**43 commandes Tauri** enregistrées dans `lib.rs::invoke_handler`
(lignes 73-117) — vérification exhaustive nom par nom contre
`frontend/ipc.ts` et les appelants directs (`report_smoke` appelé hors
`ipc.ts` depuis `main.ts`, `report-view.ts`, `selftest.ts`) : **toutes les
43 commandes ont un point d'appel réel côté front. 0 commande Tauri
morte.**

- **Priorité : Low**
- **Fichier** : `src-tauri/src/db.rs:11` (`tracks.hash`)
- **Description** : colonne créée en migration v1, **aucune** référence
  trouvée ailleurs dans `src-tauri/src` (le seul autre « hash » du repo est
  `std::hash::Hash` dans `ipc.rs:289`, sans rapport).
- **Impact concret** : colonne totalement inerte, occupe de l'espace et de
  la confusion pour un futur lecteur du schéma.
- **Correction proposée** : à documenter comme morte ; suppression
  seulement via une migration dédiée si confirmé qu'aucune build
  antérieure n'en dépend en lecture.
- **Effort** : Faible (documentation) / Moyen (migration de suppression).
- **Bénéfice** : schéma plus honnête.

---

- **Priorité : Low**
- **Fichier** : `src-tauri/src/db.rs:39-43` (table `custom_tags`)
- **Description** : table entière créée en v1, **aucune** référence
  trouvée nulle part (ni INSERT, ni SELECT, ni struct correspondante) en
  dehors de sa définition de schéma.
- **Impact concret** : table fantôme, coût de maintenance nul mais
  confusion pour la compréhension du schéma réel.
- **Correction proposée** : documenter comme morte ou supprimer via
  migration dédiée.
- **Effort** : Faible / Moyen.
- **Bénéfice** : schéma plus lisible.

---

- **Priorité : Low**
- **Fichier** : `src-tauri/src/db.rs:36` (`metadata.genre`)
- **Description** : le commentaire de la migration v6 (`db.rs:94`) dit
  explicitement *« metadata.genre stays for back-compat but track_genres is
  the source »* — colonne délibérément conservée, mais son usage actif
  (lecture/écriture) n'apparaît dans aucun des fichiers consommateurs
  (`library.rs`, `metadata/mod.rs`) : seul `track_genres` est lu/écrit
  activement.
- **Impact concret** : nul aujourd'hui (retro-compat assumée), mais à
  garder en tête pour une future passe de nettoyage de schéma.
- **Correction proposée** : rien à faire maintenant, l'intention est déjà
  documentée dans le code.
- **Effort** : n/a.
- **Bénéfice** : n/a — inclus pour complétude de l'audit.

## 6. Fonctions trop longues / complexité

**Backend Rust** (`filing.rs`, `actions.rs`) : aucune fonction ne dépasse
franchement 80 lignes. `plan_file` (`filing.rs:272-346`, ~75 lignes) est la
plus proche de la limite — mêle déjà résolution de rail/target, garde
anti-upscale, résolution de 3 chemins de destination (sentinel
`FILE_IN_PLACE`, préfixe externe, bin normal), et calcul du nom final (4
responsabilités denses mais sous le seuil). `execute_file` (44 lignes) et
`commit_file` restent courtes. **Le backend Rust respecte une discipline de
petites fonctions à responsabilité unique.**

**Frontend** — plusieurs fonctions massivement au-delà de 80 lignes :

- **Priorité : High**
- **Fichier** : `frontend/sift-live.ts:1143-1401`
- **Composant** : `installLiveWiring` (**259 lignes**)
- **Description** : un seul délégué de clic sur `#pa` (lignes 1181-1342,
  ~160 lignes à lui seul) mélange sélection de ligne de queue (avec
  debounce), actions Écartés (5 branches : slsk/trash/restore/requeue/
  purge/store), actions Bibliothèque (5 branches : qual/facet/pick/link/
  play), et 8 branches de gestion de batch — plus l'installation d'un
  listener `change` séparé, 2 abonnements IPC (`onQueueChanged`,
  `onFileDone`, `onFileProgress`), et une fonction imbriquée
  `scheduleAnalyzeRender` avec sa propre logique de throttle RAF.
- **Impact concret** : routing, mutation d'état, dispatch IPC et rendu tous
  emmêlés dans un seul handler géant — modifier une branche de batch risque
  de casser une branche Écartés sans lien logique, juste par proximité.
- **Cause probable** : croissance organique du point d'entrée de wiring
  sans extraction au fur et à mesure (fichier déjà signalé comme god file
  dans CLAUDE.md, a grossi depuis).
- **Correction proposée** : extraire chaque groupe de branches (queue,
  écartés, bibliothèque, batch) en handlers nommés séparés, dispatchés
  depuis un routeur fin.
- **Effort** : Élevé (fichier central, risque de régression au découpage).
- **Bénéfice** : testabilité et lisibilité, réduit le risque de régression
  croisée entre écrans.

---

- **Priorité : Medium**
- **Fichier** : `frontend/report-view.ts:405-609`
- **Composant** : `mountPlayer` (205 lignes)
- **Description** : construction WaveSurfer, wiring de 3 boutons
  (play/key-lock), sliders drag-and-drop manuels (volume + tempo,
  réutilisant une closure `dragSlider`), calcul de temps écoulé/restant, et
  manipulation de pixels canvas pour le masque de hover (lecture/écriture
  `ImageData`, `toDataURL`, calcul de position relative) — au moins 5
  responsabilités indépendantes dans une seule fonction.
- **Impact concret** : un bug dans le calcul de hover peut nécessiter de
  lire tout le setup du lecteur pour le localiser.
- **Correction proposée** : extraire le masque de hover canvas et les
  sliders en fonctions/modules séparés.
- **Effort** : Moyen.
- **Bénéfice** : isole la logique canvas (déjà source de bugs récents —
  cf. historique des commits player) du reste du lecteur.

---

- **Priorité : Medium**
- **Fichier** : `frontend/sift-live.ts:858-1011`
- **Composant** : `renderReglagesLive` (154 lignes)
- **Description** : nettoyage DOM, masquage de maquette statique, 3 appels
  `await getSetting(...)` séquentiels chacun avec son propre try/catch
  dupliqué, construction de 3 blocs HTML indépendants (Discogs/
  Bibliothèque/Apparence) par concaténation de chaînes, wiring d'au moins 6
  listeners (dont un debounce de sauvegarde token à 600ms).
- **Impact concret** : violation nette de la séparation lecture-état /
  construction-DOM / wiring-événements — modifier un des 3 blocs de
  réglages risque d'affecter les 2 autres par erreur de portée de variable.
- **Correction proposée** : séparer en 3 fonctions de rendu (une par bloc
  de réglages), fonction de lecture d'état commune.
- **Effort** : Moyen.
- **Bénéfice** : réglages ajoutables/modifiables indépendamment.

---

- **Priorité : Medium**
- **Fichier** : `frontend/filing.ts:297-461`
- **Composant** : `renderBins` (165 lignes)
- **Description** : rendu conditionnel « pas de racine », filtrage de
  dossiers (mode plat vs arbre), construction de 5 blocs HTML distincts,
  logique de griséification du wrapper d'arbre, restauration de
  focus/caret, wiring de 7 listeners différents.
- **Impact concret** : calcul de filtre, génération de template et
  event-wiring du popover de destination dans la même fonction — risque
  élevé de régression au moindre changement du popover (déjà signalé
  fragile dans les mémoires de session : repositionnement/cascade CSS).
- **Correction proposée** : séparer calcul de filtre (pur), génération
  HTML (pure), wiring (effets de bord) en 3 fonctions.
- **Effort** : Moyen.
- **Bénéfice** : réduit la fragilité déjà documentée du popover de
  destination.

---

- **Priorité : Low**
- **Fichier** : `frontend/sift-live.ts:393-538`
- **Composant** : `renderBatch` (146 lignes)
- **Description** : filtre 3 sous-listes (ready/fakes/pending), gère
  l'initialisation de sélection par défaut via un flag mutable, définit 5
  fonctions de rendu HTML imbriquées avant de composer le HTML final —
  logique métier et présentation au même niveau.
- **Correction proposée** : extraire le filtrage en fonction pure testable
  séparément des templates.
- **Effort** : Faible à Moyen.
- **Bénéfice** : logique de sélection par défaut testable sans DOM.

---

- **Priorité : Low**
- **Fichier** : `frontend/filing.ts:990-1088`
- **Composant** : `renderEditor` (99 lignes)
- **Description** : construit tout le panneau d'identification en un bloc
  `innerHTML` unique, définit une closure `upd` de synchronisation d'état,
  puis wire 6 listeners distincts — templating + mutation d'état + wiring
  dans la même fonction.
- **Effort** : Faible à Moyen.
- **Bénéfice** : cohérent avec le reste des recommandations de découpage.

**Constat global** : le backend Rust respecte une discipline stricte de
petites fonctions à responsabilité unique. Le frontend concentre à
l'inverse sa complexité dans un petit nombre de fonctions « orchestrateur »
géantes qui mélangent systématiquement templating HTML par concaténation
de chaînes, mutation d'état module-level, et wiring d'événements — trois
responsabilités séparables dans chaque cas cité.

## Récapitulatif

| Priorité | Nombre |
|---|---|
| Critical | 0 |
| High | 3 (dedup fallback ×2, duplication seuils cutoff/kbps) |
| Medium | 5 (load_tag_extras, discogs fetch, run_file_batch, mountPlayer, renderReglagesLive, renderBins — note : 6 items classés Medium, regroupés ici) |
| Low | 6 (duplication Probe::open, 3× code mort DB, renderBatch, renderEditor) |

Aucun `unwrap()`/`expect()` dangereux survivant (0/454, voir §1) —
constat le plus rassurant de cette passe. Le point le plus préoccupant
n'est pas un crash potentiel mais un **risque de cohérence produit** : la
table de seuils cutoff/kbps dupliquée avec des valeurs différentes entre
Rust et TS (§4) touche directement la crédibilité du verdict, qui est la
feature signature de Sift.
