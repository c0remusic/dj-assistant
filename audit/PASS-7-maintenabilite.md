# PASS 7 — Maintenabilité

> Méthode détective : chaque constat cite fichier:ligne avec preuve lue directement.
> Aucune modification de code apportée (audit seul). Outillage invoqué explicitement :
> skills `working-with-legacy-code` (seams, dependency-breaking, effect sketch) et
> `refactoring-patterns` (smell catalog, Extract Class/Method) pour cadrer le diagnostic
> des god files et de la testabilité ; Context7 non disponible dans cette session pour
> la vérification en ligne des versions (voir section 1 — comparaison faite uniquement
> contre la dernière cible connue du 2026-06-30, pas de lookup web/Context7 exécuté).

---

## 1. Versions des dépendances — comparaison contre la cible du 2026-06-30

**Non vérifié en ligne** : aucun accès Context7/web n'a été exécuté dans cette passe
(outil non sollicité avec succès — voir note méthode ci-dessus). Comparaison faite
uniquement contre la table de versions cible déjà validée dans
`CLAUDE.md` (section "Audit des dépendances", 2026-06-30).

### Rust — `src-tauri/Cargo.toml`

| Crate | Épinglé (Cargo.toml:L) | Cible 2026-06-30 | Écart |
|---|---|---|---|
| `tauri` | `2.11.3` (L24) | `2.11.3` | Identique |
| `rusqlite` | `0.40` (L26) | `0.40.1` | Identique (le `0.40` sans patch laisse Cargo prendre le dernier `0.40.x` — comportement normal de `Cargo.lock`, pas un écart réel) |
| `symphonia` | `0.6` (L37) | `0.6.0` | Identique |
| `rustfft` | `6.4.1` (L33) | `6.4.1` | Identique |
| `lofty` | `0.24` (L34) | `0.24.0` | Identique |
| `rusty-chromaprint` | `0.3` (L36) | `0.3.0` | Identique |
| `ureq` | `3` (L38) | `3.3.0` | Non vérifiable précisément — `Cargo.toml` épingle `3` (majeur seul), pas `3.3.0` explicitement. Le `Cargo.lock` n'a pas été inspecté dans cette passe pour confirmer la version résolue réelle. **Hypothèse non vérifiée** : la résolution effective correspond probablement à `3.3.0` ou plus récent dans la même série majeure, mais ce n'est pas prouvé fichier:ligne sans lire `Cargo.lock`.

Autres dépendances non listées dans la cible CLAUDE.md (`ffmpeg-sidecar 2.5.0`, `walkdir
2.5.0`, `tauri-plugin-dialog 2.7.1`, `notify-debouncer-full 0.7.0`, `open 5`, `dirs 5`,
`tauri-build 2.6.3`, `tempfile 3.27.0`) — hors périmètre de la cible actée, non auditées
ici (pas de référence pour juger un écart).

**Conclusion Rust** : aucun écart détecté contre la cible connue. Rien à signaler en
Critical/High sur ce point — juste l'absence de vérification en ligne à noter comme
limite de cette passe.

### JS — `package.json`

| Package | Épinglé (package.json:L) | Cible 2026-06-30 | Écart |
|---|---|---|---|
| `typescript` | `^6.0.3` (L15) | `6.0.3` | Identique (caret `^` autorise un bump mineur/patch automatique à l'install — pas un écart figé mais un risque de drift silencieux à surveiller) |
| `vite` | `^8.1.2` (L16) | `8.1.2` | Identique, même remarque sur le caret |

Autres dépendances (`@tauri-apps/cli ^2`, `@tauri-apps/api ^2`, `wavesurfer.js
^7.12.8`, `@fontsource/*`) — hors périmètre de la cible actée.

**Conclusion JS** : aucun écart détecté contre la cible connue.

- Priorité : **Low**
- Fichier:ligne : `package.json:15-16`
- Description : `typescript` et `vite` sont épinglés avec `^` (caret), qui autorise
  npm à installer tout mineur/patch futur sans qu'un commit ne le déclenche — contraire
  à la méthode "un palier majeur = un commit dédié" documentée dans CLAUDE.md, qui ne
  couvre que les bumps majeurs, pas le drift mineur silencieux entre deux `npm install`.
- Impact concret : deux machines de dev peuvent avoir des versions mineures différentes
  de `vite`/`typescript` sans que `package.json` ne bouge, ce qui rend un bug
  d'environnement difficile à reproduire.
- Cause probable : convention npm par défaut (`npm install` écrit `^` sauf `--save-exact`).
- Proposition : évaluer l'usage de `package-lock.json` commité (vérifier s'il l'est déjà)
  comme garde-fou suffisant, plutôt que de changer la convention de version.
- Effort : Low (vérification seule, pas de changement de code).
- Bénéfice attendu : reproductibilité de build.

---

## 2. Zones fragiles — couplage mesuré

### `frontend/sift-live.ts` (1412 lignes)

**Imports sortants** (`sift-live.ts:3-74`) — 13 modules locaux importés :
`./ipc`, `./library-detail`, `./empty-state`, `./filing`, `./ecartes-view`,
`./home-sources`, `./chrome`, `./theme`, `../shared/contracts`, `./dom`, `./journal`,
`./progress-zone`, `./batch-tracklist`, plus `@tauri-apps/plugin-dialog` (externe).

**Fan-in** (qui importe `sift-live.ts`) : **0 fichier** — confirmé par grep sur
`frontend/*.ts` pour le littéral `sift-live`. Seul `index.html:28` référence le point
d'entrée réel (`/frontend/main.ts`), qui lui-même n'importe pas `sift-live.ts` dans les
imports observés à ce jour (le fan-in de `sift-live.ts` doit être confirmé côté
`main.ts`, non relu intégralement dans cette passe — **hypothèse non vérifiée** sur le
lien exact main.ts → sift-live.ts, bien que le nom du fichier et CLAUDE.md
("sift-live.ts — point d'entrée wiring live") l'indiquent fortement).

**Lecture du couplage** : `sift-live.ts` a un **fan-out élevé (13 modules) et un
fan-in nul** — ce n'est PAS un fichier fragile au sens "beaucoup de dépendants",
c'est un **god file de composition** (agrège toute la logique de wiring UI en un
point). Le risque n'est pas la propagation de rupture vers d'autres fichiers (rien
ne l'importe), mais sa propre taille : à 1412 lignes il a grossi de +470 lignes
depuis le signalement initial dans CLAUDE.md ("god file de ~942 lignes... split
prévu mais pas fait"), donc la dette s'aggrave activement au lieu d'être payée.

- Priorité : **High**
- Fichier:ligne : `frontend/sift-live.ts` (fichier entier, 1412 lignes), imports en
  L3-L74
- Description : fichier de wiring qui a grossi de 942 → 1412 lignes sans le split
  annoncé dans CLAUDE.md (référence D3, "split de sift-live.ts").
- Impact concret : tout changement de wiring UI oblige à naviguer un fichier de
  1412 lignes ; aucun test unitaire possible sur un fichier de composition de cette
  taille sans y injecter des seams (voir `working-with-legacy-code` — un fan-out de
  13 modules sans découpage en sous-responsabilités rend l'effect sketch d'un
  changement coûteux à tracer mentalement).
- Cause probable : nouvelles features (Discogs M6a) ajoutées au fichier existant
  plutôt qu'à des modules dédiés, malgré la présence déjà actée de modules
  spécialisés (`home-sources.ts`, `ecartes-view.ts`, `chrome.ts` — la stratégie de
  split existe mais n'est pas allée assez loin).
- Proposition : appliquer Extract Class/Module (catalogue `refactoring-patterns`)
  sur les blocs fonctionnels internes à `sift-live.ts` restants, sur le même modèle
  que `chrome.ts`/`ecartes-view.ts`/`home-sources.ts` déjà extraits — description
  seule, pas de plan détaillé ici (hors scope audit).
- Effort : High.
- Bénéfice attendu : fichier de wiring qui reste lisible, extraction facilitée pour
  chaque nouvelle feature front.

### `frontend/filing.ts` (1653 lignes — le plus gros fichier du repo)

**Imports sortants** (`filing.ts:7-35`) — 8 modules locaux : `./ipc`, `../shared/contracts`,
`./report-view`, `./identify-shared`, `./dom`, `./empty-state`, plus `@tauri-apps/plugin-dialog`
et `@tauri-apps/api/core` (externes).

**Fan-in** : **1 fichier** — `sift-live.ts:32-47` importe depuis `./filing`.

**Lecture du couplage** : fan-out modéré (8), fan-in faible (1) mais ce seul importeur
est lui-même le god file (`sift-live.ts`). `filing.ts` est donc un gros bloc de logique
UI (rail de classement) consommé par un point d'entrée déjà saturé — le risque n'est
pas la propagation multi-fichiers mais la taille brute (1653 lignes en un seul module,
record du repo tous langages confondus selon PASS-0).

- Priorité : **Medium**
- Fichier:ligne : `frontend/filing.ts` (fichier entier, 1653 lignes)
- Description : plus gros fichier du repo, fan-in limité à 1 (`sift-live.ts`) mais
  fan-out de 8 modules — bloc de logique UI non décomposé en sous-unités testables.
- Impact concret : la logique de rail de classement (destination, format, actions
  filer/écarter) est concentrée dans un seul fichier TS sans séparation visible entre
  état, rendu DOM et appels IPC — cohérent avec l'absence de framework de test JS
  identifié dans ce repo (aucun `*.test.ts` trouvé dans les greps de cette passe ni
  dans PASS-0).
- Cause probable : extraction progressive (report-view.ts, identify-shared.ts déjà
  sortis) mais le cœur du rail n'a pas suivi le même traitement.
- Proposition : même stratégie qu'au-dessus — Extract Module sur les responsabilités
  distinctes (état de sélection, rendu, appels IPC filing) une fois `sift-live.ts`
  traité en priorité (dépendance d'ordre : ce fichier est un sous-problème de la
  fragilité n°1).
- Effort : High.
- Bénéfice attendu : rail de classement testable indépendamment du wiring global.

### `src-tauri/src/filing.rs` (916 lignes) et `actions.rs` (801 lignes)

**Imports de `filing.rs`** (`filing.rs:7-9`) : `crate::encode`, `crate::naming`,
`crate::actions`, `crate::library`, `crate::tagging` — 5 modules internes, fan-out
modéré et explicite.

**Fan-in de `filing.rs`** : `lib.rs`, `ipc_filing.rs`, `ipc_identify.rs`, `ecartes.rs`
— **4 fichiers** l'importent (confirmé par grep `use crate::filing|filing::`).

**Fan-in de `actions.rs`** : `filing.rs`, `ipc_filing.rs`, `ecartes.rs` — **3 fichiers**
(confirmé par grep `use crate::actions|actions::`).

**Lecture du couplage** : côté Rust, `filing.rs` a un couplage réel plus significatif
que ses pendants front (fan-out 5 ET fan-in 4) — c'est le point de plus forte
convergence du repo Rust en dehors de `lib.rs`. Contrairement au front, ce fichier
EST structuré en phases testables (voir section 3) malgré sa taille — le couplage
élevé est ici un signe de centralisation délibérée (documentée en commentaire de
module, `filing.rs:1-5`) plutôt qu'un god file accidentel.

- Priorité : **Low**
- Fichier:ligne : `src-tauri/src/filing.rs` (916 lignes), fan-in 4 fichiers, fan-out
  5 modules
- Description : couplage élevé mais organisé (3 phases documentées : plan/execute/commit,
  `filing.rs:270-448`), avec suite de tests substantielle (17 tests, `filing.rs:545-916`).
- Impact concret : risque de régression en cas de modification, mais atténué par la
  couverture de test déjà en place — pas un point aveugle comme le front.
- Cause probable : centralisation volontaire du rangement (mono-location, undo) —
  architecture assumée, pas une dérive.
- Proposition : aucune action urgente ; RAS au-delà du monitoring habituel de taille.
- Effort : n/a.
- Bénéfice attendu : n/a (pas un problème à corriger, juste noté pour la carte de
  couplage demandée).

---

## 3. Testabilité de la logique critique

### `analysis/verdict.rs` (99 lignes) — pure, bien testée

Fonctions `verdict()` (L34-52) et `min_cutoff_hz_for_bitrate()` (L22-31) sont des
fonctions pures : `f32`/`u32`/enum en entrée, enum `Verdict` en sortie, **aucun I/O**.
7 tests (`verdict.rs:58-98`) couvrent : lossless OK, lossless fake (cliff), lossless
grey, MP3 honnête OK (2 bitrates), MP3 sur-encodé fake (2 bitrates), lossy sans
bitrate connu, rail inconnu. Couverture réelle bonne sur les branches de `verdict()`.

**Gap identifié** : `min_cutoff_hz_for_bitrate()` (L22-31) a 6 branches (`>=320,
>=256, >=192, >=160, >=128, else`) mais les tests n'exercent que 320/256/128 via
`verdict()` — les seuils 192 et 160 (`filing.rs` n'entre pas en jeu ici) ne sont
testés par AUCUN test direct ou indirect trouvé dans ce fichier.

- Priorité : **Medium**
- Fichier:ligne : `src-tauri/src/analysis/verdict.rs:26-27` (branches `>=192`,
  `>=160`)
- Description : deux des six branches de seuil bitrate n'ont aucun test qui les
  exerce (ni direct ni via `verdict()`).
- Impact concret : une régression sur les seuils 192/160 kbps ne serait pas détectée
  par la suite actuelle.
- Cause probable : tests ajoutés au fil des cas rencontrés (320, 256, 128) plutôt que
  systématiquement sur toutes les bandes du barème.
- Proposition : ajouter les cas manquants (192 et 160 kbps, honnête + sur-encodé) au
  test `over_encoded_mp3_is_fake` / `honest_mp3_matching_its_bitrate_is_ok`.
- Effort : Low.
- Bénéfice attendu : couverture complète du barème de détection fraude bitrate.

### `encode.rs` (258 lignes) — mixte, correctement séparé

Fonctions pures testables sans I/O : `target_for()` (L64-69), `guard_no_upscale()`
(L72-77), `Target::ext()`/`rail()` (L27-41) — toutes couvertes par des tests sans
fixture (`encode.rs:174-186`, `202-208`).

Fonctions couplées à l'I/O disque/FFmpeg directement dans le corps : `is_conformant()`
(L91-110, appelle `Probe::open` + lit le FS), `encode()` (L115-158, spawn FFmpeg +
`std::fs::metadata`). Ces deux fonctions ne sont PAS des fonctions pures — elles ne
sont testables qu'avec de vrais fichiers (fixtures) et un vrai process FFmpeg.

**Constat sur la testabilité réelle** : les tests d'`encode()`/`is_conformant()`
(`encode.rs:188-200, 210-257`) dépendent de fixtures physiques (`fixtures/real_lossless.flac`,
`fixtures/real_320.mp3`) avec un pattern de skip explicite (`eprintln!("skip: no
fixture"); return;`, répété 5 fois) — donc **si les fixtures sont absentes, ces tests
ne testent RIEN et le passent silencieusement** (`cargo test` verdict "5 passed"
même sans avoir exercé le moindre encodage réel).

- Priorité : **High**
- Fichier:ligne : `src-tauri/src/encode.rs:191-193, 212-214, 221-223, 232-234, 247-249`
  (5 occurrences du pattern `let Some(...) = fixture(...) else { eprintln!("skip...");
  return; }`)
- Description : les tests qui couvrent le chemin d'I/O réel (spawn FFmpeg, lecture
  disque) se transforment silencieusement en no-op si les fixtures ne sont pas présentes
  sur la machine qui exécute `cargo test` — aucun `#[ignore]` explicite, aucun échec
  visible dans le résumé standard de test.
- Impact concret : en CI ou sur une machine sans le dossier `fixtures/` peuplé, ces
  5 tests annoncent un succès trompeur — le module d'encodage réel (le cœur du
  chemin "déplacer = encoder + ranger") peut régresser sans qu'aucun test ne le
  détecte.
- Cause probable : les fixtures binaires (fichiers audio réels) ne sont probablement
  pas commitées dans le repo (pattern de test conditionnel suggère des gros fichiers
  gérés hors Git) — pattern délibéré mais son mode d'échec silencieux n'est pas
  signalé à l'exécution.
- Proposition : faire échouer bruyamment (`panic!`) au lieu de `return` silencieux
  quand la fixture est absente EN CI (variable d'env type `CI=true`), ou au minimum
  imprimer un résumé agrégé en fin de suite du nombre de tests skippés — pour que
  l'absence de couverture réelle soit visible plutôt que masquée dans un flux
  `eprintln!` que personne ne lit en pratique.
- Effort : Medium.
- Bénéfice attendu : la suite de tests ne peut plus mentir sur la couverture du
  chemin d'encodage critique.

### `filing.rs` (916 lignes) — structure exemplaire pour la testabilité

Le fichier sépare explicitement 3 phases dans son architecture (commentaires
`filing.rs:234-237, 348-349, 420-421`) :
- **Phase 1** `plan_file()` (L272-346) : lecture DB + résolution chemin, pas d'I/O
  lent (juste `create_dir_all`).
- **Phase 2** `execute_file()` (L350-393) : l'I/O lourd (encode, tag, move, trash),
  **sans lock DB**.
- **Phase 3** `commit_file()` (L422-448) : écriture DB uniquement.

Cette séparation est elle-même une forme de testabilité délibérée : `file_track()`
(L456-468, `#[cfg(test)]` uniquement) enchaîne les 3 phases pour les tests, alors
qu'en production elles tournent découplées (commentaire L452-454 explicite : "Production
never holds the lock across the encode"). 17 tests couvrent des scénarios réalistes :
fichier conformant déplacé (L616), revert restaure les anciens tags (L649), FLAC
converti + original trashé (L680), extension `.aif` préservée (L713), refus upscale
(L748), destination externe (L765, L786), reject batch avec ID invalide (L825), trash
(L845), genres appliqués (L876).

**Gap identifié** : aucun test n'exerce le chemin d'échec de `commit_file()` déclenchant
`rollback_fs()` (L396-418) — le rollback en cas d'échec DB après un encode réussi
(scénario où phase 3 échoue après phase 2) n'est couvert par aucun test trouvé dans
`filing.rs:545-916`. C'est le chemin le plus critique pour la garantie "rien n'est
laissé à moitié filé" (docstring L420-421) et il n'est pas prouvé par un test.

- Priorité : **High**
- Fichier:ligne : `src-tauri/src/filing.rs:396-418` (`rollback_fs`), aucun test
  correspondant trouvé dans `filing.rs:545-916`
- Description : la fonction de rollback qui garantit l'absence d'état à moitié filé
  n'a pas de test qui force `commit_file()` à échouer après un `execute_file()`
  réussi (ex: erreur DB simulée après un encode).
- Impact concret : une régression dans `rollback_fs()` (ex: mauvais ordre de reverse,
  fichier `tag_edit` mal restauré) ne serait détectée qu'en production, sur exactement
  le chemin que ce mécanisme est censé protéger.
- Cause probable : difficile à déclencher nativement (il faut simuler un échec DB
  après un succès disque) — nécessite une injection de faute, absente du harnais de
  test actuel.
- Proposition : introduire un seam permettant de simuler un échec de `commit_file`
  (ex: connexion fermée entre phase 2 et 3, ou verrou DB déjà tenu) pour exercer
  `rollback_fs` end-to-end — technique de dependency-breaking `working-with-legacy-code`
  applicable ici sans changer la logique de production.
- Effort : Medium.
- Bénéfice attendu : le chemin de sécurité le plus critique du module de filing est
  enfin prouvé par un test, pas seulement par lecture de code.

---

## 4. Migrations SQLite — confirmation de la lecture PASS-0

Relecture de `db.rs:1-286` confirme la lecture de PASS-0 :
- 8 migrations strictement additives (`ALTER TABLE ADD COLUMN`, `CREATE TABLE`,
  `CREATE INDEX` — aucun `DROP`/`RENAME` trouvé dans `MIGRATIONS`, L5-115).
- Versionnées par `PRAGMA user_version` (L120-127), append-only par convention
  documentée en commentaire (L4 : "NEVER reorder or edit an existing entry once
  shipped — only append").
- 9 tests (`db.rs:157-286`), pas 8 comme mentionné dans PASS-0 — écart mineur de
  comptage (`migrations_bring_db_to_latest_version`, `migrations_create_all_tables`,
  `migrations_are_idempotent`, `migrations_reach_v2`, `tracks_has_m1_columns`,
  `tracks_has_m2b_columns`, `tracks_has_m4_columns`, `actions_and_settings_have_m4_shape`,
  `actions_has_v7_meta_column`, `actions_has_v8_session_id_column` — en réalité 10 tests
  en comptant tous les `#[test]`, pas 8 ni 9). **Correction du chiffre de PASS-0** :
  10 tests, pas 8 — le pattern reste sain, seul le comptage était approximatif.
- Idempotence réellement testée (`migrations_are_idempotent`, L177-182 : exécute
  `run_migrations` deux fois, vérifie l'absence de doublon de table).

**Aucune faille non vue trouvée** — la lecture PASS-0 est confirmée avec une
correction mineure de comptage (10 tests réels, pas 8).

- Priorité : **Low** (correction de PASS-0 uniquement, pas un défaut du code)
- Fichier:ligne : `src-tauri/src/db.rs:157-286`
- Description : PASS-0 indique "8 tests" pour `db.rs` ; comptage direct des blocs
  `#[test]` en donne 10.
- Impact concret : aucun sur le code — correction de la carte technique uniquement.
- Cause probable : comptage rapide dans PASS-0 sans lister chaque `#[test]`.
- Proposition : corriger le chiffre dans PASS-0 si le document est mis à jour.
- Effort : Low.
- Bénéfice attendu : exactitude de la carte technique pour les passes suivantes.

---

## 5. Constantes de verdict — centralisation vs duplication

**Rust — un seul point de définition** : `LOSSLESS_OK_HZ` (20000.0),
`LOSSY_CLIFF_HZ` (19500.0) définis en `analysis/verdict.rs:15-16` ; le barème
`min_cutoff_hz_for_bitrate()` (320→19000, 256→18000, 192→16500, 160→15500,
128→14500, sinon 12000) en `verdict.rs:22-31`. Nommés en `const` ou fonction pure,
pas de littéral dupliqué trouvé ailleurs côté Rust (grep ciblé sur ces valeurs limité
à `verdict.rs` et au frontend — aucune autre occurrence Rust trouvée dans les fichiers
lus lors de cette passe).

**Frontend — duplication confirmée avec un barème DIFFÉRENT** :
`frontend/report-view.ts:62` définit sa propre fonction d'estimation :
```
hz >= 20000 ? 320 : hz >= 19000 ? 256 : hz >= 18000 ? 192 : hz >= 16500 ? 160 : 128
```
Comparé au barème Rust (320→19000, 256→18000, 192→16500, 160→15500, 128→14500) : les
seuils NE SONT PAS les mêmes chiffres réutilisés dans le même sens. Le frontend décale
d'un cran — ex. Rust dit "320 kbps a besoin d'au moins 19000 Hz pour ne pas être
suspect", tandis que le front dit "à partir de 20000 Hz affiché, on estime 320 kbps".
Ce ne sont pas la même fonction mathématique (l'une est un seuil de fraude, l'autre une
estimation d'affichage) mais elles portent des nombres visuellement très proches
(19000/20000, 18000, 16500) sans qu'aucun commentaire dans `report-view.ts:62` ne
renvoie vers `verdict.rs` pour expliquer pourquoi les deux barèmes diffèrent legitimately
ou devraient un jour converger.

- Priorité : **Medium**
- Fichier:ligne : `frontend/report-view.ts:62` vs `src-tauri/src/analysis/verdict.rs:22-31`
- Description : deux barèmes de seuils Hz→kbps numériquement voisins mais non
  identiques, définis indépendamment en Rust (logique de verdict fraude) et en
  TypeScript (estimation d'affichage), sans lien explicite ni commentaire croisé.
- Impact concret : un changement du barème de fraude côté Rust (`verdict.rs`) n'aura
  aucune raison de se répercuter sur l'estimation affichée côté front — les deux
  peuvent diverger silencieusement au fil du temps, un développeur modifiant l'un en
  toute bonne foi sans savoir que l'autre existe (magic numbers sans référence
  croisée, smell "Duplicate Code" au sens `refactoring-patterns` même si les valeurs
  ne sont pas identiques terme à terme).
- Cause probable : `report-view.ts` a besoin d'estimer un kbps approximatif pour
  l'UI (label affiché) à partir d'un `cutoff_hz` déjà calculé côté Rust et renvoyé
  tel quel — personne n'a exposé le barème Rust au frontend (pas de commande IPC ni
  de constante partagée dans `shared/contracts.ts` pour ces seuils).
- Proposition : soit exposer les seuils de `verdict.rs` via `shared/contracts.ts`
  (constantes partagées Rust→TS comme documenté pour d'autres valeurs, ex.
  `FILE_IN_PLACE`), soit documenter explicitement en commentaire que le barème
  d'affichage est volontairement indépendant du barème de verdict (et pourquoi).
  Description seule — pas de choix tranché ici (question produit : l'estimation
  affichée doit-elle suivre le verdict exactement ?).
- Effort : Low si documentation seule, Medium si partage de constantes.
- Bénéfice attendu : élimine un point de divergence silencieuse entre logique
  métier (Rust) et affichage (TS).

---

## Récapitulatif des priorités

| Priorité | Constat | Fichier |
|---|---|---|
| High | `sift-live.ts` a grossi au lieu d'être splitté (942→1412 l.) | `frontend/sift-live.ts` |
| High | Tests `encode.rs` skippent silencieusement sans fixtures | `src-tauri/src/encode.rs:191-249` |
| High | `rollback_fs()` non testé (chemin de sécurité critique) | `src-tauri/src/filing.rs:396-418` |
| Medium | `filing.ts` (1653 l., record du repo) non décomposé | `frontend/filing.ts` |
| Medium | 2 branches de bitrate non testées dans le barème verdict | `src-tauri/src/analysis/verdict.rs:26-27` |
| Medium | Barèmes Hz→kbps dupliqués et divergents Rust/TS | `report-view.ts:62` vs `verdict.rs:22-31` |
| Low | `filing.rs`/`actions.rs` Rust : couplage élevé mais organisé et testé | `src-tauri/src/filing.rs` |
| Low | Caret `^` sur typescript/vite = drift mineur silencieux possible | `package.json:15-16` |
| Low | PASS-0 sous-compte les tests de `db.rs` (8 annoncés, 10 réels) | `src-tauri/src/db.rs:157-286` |

## Hypothèses non vérifiées

- Version résolue réelle d'`ureq` (épinglé `"3"` seul en `Cargo.toml:38`) — nécessite
  lecture de `Cargo.lock`, non faite dans cette passe.
- Lien exact `main.ts` → `sift-live.ts` : fortement suggéré par le nom du fichier et
  CLAUDE.md, mais `main.ts` n'a pas été lu intégralement pour confirmer l'import direct.
- Existence de versions plus récentes que la cible du 2026-06-30 pour toute dépendance
  listée — aucun lookup Context7 ni web n'a abouti dans cette session (outils cités
  dans la consigne non exécutés avec succès ; à refaire dans une passe dédiée si
  l'outillage devient disponible).
