# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> Repo à worktree unique : **`C:\dev\sift`**. Le dev se fait **directement sur `main`**
> (app non publiée, pas de branche de release à protéger) — pas de branche de chantier
> par défaut. L'écart de branche **ne se note pas, il se mesure** — un nombre écrit ici
> est faux dès le commit suivant : `git rev-list --left-right --count main...<branche>`.
> Une session en worktree livre sur `main` aussi : gates vertes → `git push origin
> HEAD:main` (fast-forward). Non-ff = une session parallèle a avancé `main` : re-vérifier
> que le travail n'est pas déjà fait, `git merge origin/main` (jamais de rebase d'une
> branche déjà poussée), re-gater, re-pousser. Jamais de tag.

## Quoi

App desktop **Tauri v2** (Windows + macOS), gratuite, de prépa de musique pour DJ :
analyse (détection de faux lossless au spectrogramme), dédoublonnage, identification
Discogs, rangement, export Rekordbox (XML + écriture directe `master.db`), formatage
clé USB. Principe : « déplacer = encoder + ranger ».

État des jalons (M0→M8, tous livrés) : `README.md`.
Glossaire de domaine : `CONTEXT.md` — le lire avant tout travail qui manipule le
vocabulaire métier.

## Stack

Tauri v2 (Rust, crate lib = **`sift_lib`**) · frontend **Vite + TypeScript vanilla,
sans framework** (tests **Vitest** en env Node + **ESLint** flat config, ajoutés le
2026-08-05) · **Symphonia** (décodage analyse, in-process) + **FFmpeg sidecar**
bundlé (encodage) · **SQLite** (`rusqlite`, bundled) · `rustfft` · `lofty` ·
`rusty-chromaprint` · `ureq` · `wavesurfer.js` v7 (lecture/waveform) · `fatfs`
(écriture FAT32 au-delà du plafond de 32 Go de Windows — **MIT**, choisi contre
`fat32format` qui est GPL, incompatible avec une distribution commerciale).

Les patterns React (hooks/stores/providers) **ne s'appliquent pas ici**, et une
migration de framework est explicitement écartée.

**Toolchain Rust : `rust-toolchain.toml` à la RACINE** (pas dans `src-tauri/`) épingle
le canal `1.96.0`. `src-tauri/Cargo.toml:9` déclare encore `rust-version = "1.77.2"` —
ce chiffre n'est **plus vérifié par rien** (ni CI, ni job dédié) depuis l'épinglage.
Ne pas le traiter comme une contrainte tant qu'un build réel ne l'a pas rétabli.

## Commandes

Windows : npm passe par `cmd /c "npm …"` si le shell direct pose problème.

```bash
npm ci && npm run fetch-ffmpeg   # bootstrap (ffmpeg → src-tauri/binaries/, gitignoré)
                                 # macOS : COMPILE ffmpeg depuis les sources (LGPL), plusieurs
                                 # minutes — aucun build LGPL statique n'est publié
                                 # Windows : lancer via PowerShell, pas Git Bash — le tar MSYS
                                 # lit « C:\ » comme un hôte réseau (« Cannot connect to C: »)
npm run tauri dev                # dev : Vite 5173 + backend Rust
npm run dev                      # frontend seul (navigateur) — voir la mise en garde §Vérification UI
npm run build                    # → dist/
npm run tauri build              # installeurs → src-tauri/target/release/bundle/
npm run test                     # Vitest, un seul projet `unit` en env Node
npm run test:watch               # idem en watch
npx vitest run test/b85.test.ts  # un seul fichier · par nom : npx vitest run -t "vecteur gelé"
npm run lint                     # ESLint (binaire local, pas `npx eslint`)
npm run lint:tokens              # couleurs/z-index/spacing en dur qui contournent un token —
                                 # ratchet à baseline versionnée (scripts/lint-tokens-baseline.json) :
                                 # un +N délibéré se grave par `node scripts/lint-tokens.mjs --write-baseline`
npm run lint:accents             # blocs de commentaire FR écrits sans accent (bug #43) —
                                 # ratchet à baseline (scripts/lint-accents-baseline.json)
npm run lint:css-comments        # `*/` orphelin dans styles.css : texte hors /* */ = sélecteur
                                 # invalide, la règle suivante meurt en silence (payé 2x, 09-06/07)
npm run check:security           # scope asset et CSP — refuse le retour du wildcard (aussi en CI)
npm run storybook                # doc visuelle des états UI (port 6006), stories = frontend/*.stories.ts

npx tsc --noEmit                                                            # type-check front
cargo test --manifest-path src-tauri/Cargo.toml                             # tests Rust
cargo test --manifest-path src-tauri/Cargo.toml <nom_du_test>               # un seul test
cargo test --manifest-path src-tauri/Cargo.toml --release -- --ignored --nocapture   # benchmarks volume
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

⚠️ **`src-tauri/fixtures/*` et `src-tauri/binaries/` sont gitignorés.** Un checkout
frais (clone ou worktree) n'a ni l'un ni l'autre, et chaque absence casse autrement :
sans le sidecar, `cargo check` lui-même sort en 101 (``resource path
`binaries\ffmpeg-…` doesn't exist``) ; sans fixtures, les tests `analysis::decode`
échouent en *file not found*. Ni l'un ni l'autre n'est un vrai bug. Bootstrap dans
l'ordre : `npm run fetch-ffmpeg` PUIS `node scripts/make-fixtures.mjs` (qui plante en
`ENOENT` si `binaries/` manque) — ou copier binaries + fixtures depuis le checkout
principal. Vécu le 2026-08-20 sur deux worktrees. ⚠️ En copiant : `fixtures/README.md`
est SUIVI par git, donc le dossier `fixtures/` existe déjà dans un worktree frais, et
`Copy-Item -Recurse` y IMBRIQUE alors la copie (`src-tauri/fixtures/fixtures/`) — même
symptôme *file not found* que l'absence. Copier le CONTENU (`fixtures\*`), pas le
dossier. Vécu le 2026-08-20 (agent #38). Les deux anchors authentiques facultatives
(`src-tauri/fixtures/README.md`) restent manuelles ; les tests de caractérisation les
sautent quand elles sont absentes.

⚠️ **Ne pas lancer `cargo test`/`clippy` pendant qu'un `tauri dev` compile** : ils se
disputent le lock du `target/` (attente, ou corruption du cache incrémental).
`scripts/cargo-isolated.sh` lance `cargo` avec un `CARGO_TARGET_DIR` isolé.

Storybook est le **miroir vivant** de `docs/design-system-states.md` : documenter un
nouvel état veut dire ajouter sa story, pas seulement étendre le markdown.

Scripts hors binaire de prod : `.claude/scripts/cdp.cjs` (§Vérification UI) ·
`scripts/rekordbox-spike-helper.ps1` (§Méthode) · `scripts/decrypt-masterdb-debug.py`
(décrypte une copie de `master.db` en SQLite clair pour inspection ad-hoc — port Python
de `rekordbox_masterdb.rs`, dépend de PyCryptodome).

## Définition de fini

Deux gardiens, à connaître avant de dire « terminé » :

- **`.claude/verify.sh`** — `tsc --noEmit` + `lint:tokens` + `lint:accents` +
  `cargo fmt --check` + `cargo check`. Seul `cargo check` est borné, à 25 s (le
  retrouver par `grep -n "timeout 25" .claude/verify.sh`, abandonné **sans échec** au-delà :
  une gate de fin de tour doit être rapide ou muette). **Déclenchée automatiquement**
  par un hook `Stop` déclaré dans `.claude/settings.json`, versionné, timeout 90 s.
  Elle avait cessé de l'être entre le reset vanilla de `~/.claude` du 2026-07-31
  (commit `6f6c132`, sauvegarde au tag `pre-reset-vanilla`) et son rebranchement par
  `f9fa086` le 2026-08-11 — ne pas rejouer ce lancement à la main. **Deux** hooks sont
  actifs sur ce dépôt : celui-ci, et celui d'`impeccable` en `PostToolUse`
  (`.claude/settings.local.json`, non versionné).
- **`.github/workflows/test.yml`** — sur **toute** branche et toute PR (Windows) :
  `tsc --noEmit` → `npm run test` → `npm run lint` → `cargo fmt --check` →
  `clippy -D warnings` → `cargo test`. Ordre délibéré, du moins cher au plus cher : les
  trois gates frontend ne compilent rien. Le job régénère fixtures + ffmpeg d'abord.
  ⚠️ Restent hors `.claude/verify.sh` : `npm run test`, `npm run lint` (ESLint) et
  `clippy` — une fin de tour verte ne dit rien d'eux, ils ne tombent qu'en CI.
  (`cargo fmt --check` y est depuis le 2026-08-26 — quatre commits de CI rouge
  invisible en local, `7b4089b`.)

`build.yml` (installeurs non signés Win+Mac) et `release.yml` ne se déclenchent que sur
`main` / tags — ils ne sont pas un filet de branche.

### Un raté attrapé se convertit en gate, il ne se réécrit pas en prose

Quand une erreur est attrapée — par Antoine, par une relecture, par une mesure —, la corriger
ne suffit pas : **la gate qui aurait dû l'attraper se durcit dans le même geste**, avant de
passer à la suite.

Barème, du plus fort au plus faible. Ne descendre d'un cran que si celui du dessus est
réellement impossible, et écrire pourquoi :

1. **Test** — Vitest ou `cargo test`. Un raté qui porte sur une valeur ou une forme se fige en
   vecteur. Il ne compte comme gate qu'une fois **mesuré en mutant le code qu'il garde**
   (§ Méthode) : vert ne veut pas dire tenant.
2. **Lint** — `lint:tokens`, ESLint, clippy. Un raté de MOTIF (un littéral qui contourne un
   token, un `innerHTML` sans `esc()`) se compte, avec baseline à ratchet — jamais un
   avertissement qu'on lit et qu'on oublie.
3. **Hook** — `.claude/settings.json`. Un raté de SÉQUENCE — étape sautée, vérification omise,
   source consultée dans le mauvais ordre — s'intercepte là, pas ailleurs.
4. **Prose ici** — dernier recours, et il faut alors dire pourquoi les trois crans du dessus
   échouent.

⚠️ **Le test qui décide du cran.** Si la règle était **déjà écrite** quand le raté est survenu,
la réécrire ne sert à rien : ce n'est pas la règle qui manquait, c'est le cran 1, 2 ou 3.
Mesuré le 2026-08-26 sur une session de design — six ratés d'affilée, dont cinq contredisaient
une règle déjà présente (§ Front « CTA à label descriptif : texte seul », `patterns.md:54`
« une surface de contenu ne peint rien », `revue.md` § Zone A pour le compte de la file). Ajouter
un paragraphe à chacun aurait allongé ce fichier sans rien empêcher.

Corollaire : **ce fichier ne grossit pas d'un raté**. Il grossit d'une décision d'architecture,
d'un piège d'environnement non détectable, ou d'un fait que rien d'exécutable ne peut porter.

## Architecture — ce qui ne se voit pas dans un seul fichier

### La frontière IPC est un miroir manuel, pinné par des tests

`shared/contracts.ts` reflète à la main les structs serde de `src-tauri/src/ipc*.rs`.
Ce n'est pas une génération : les deux côtés se maintiennent dans le même geste.

Des **tests Rust tiennent le contrat** — ils destructurent exhaustivement (sans `..`),
donc l'ajout d'un champ Rust casse la compilation du test tant que `contracts.ts` n'a
pas suivi (`analysis/mod.rs::spectrogram_shape_matches_contracts_ts`,
`analysis_report_shape_matches_contracts_ts`). Ils **ne détectent pas** un désaccord de
*type* avec le miroir TS, seulement un champ manquant.

Des **sentinelles littérales** doivent rester identiques des deux côtés (voir les
commentaires de `shared/contracts.ts`, qui nomment le test Rust qui les épingle) :
`FILE_IN_PLACE` · `EXTERNAL_DEST_PREFIX` · `FILE_GONE` (sa rupture **supprime une ligne
en base**) · `DRIVE_VANISHED` / `IDENTITY_MISMATCH` / `ELEVATION_DECLINED` /
`EJECT_BUSY` (formatage et éjection USB) ·
`DEFAULT_FILENAME_TEMPLATE`. Ne jamais réimplémenter le rendu de nom de fichier en TS :
appeler `previewFilename`.

Le spectrogramme voyage en **base85** (`frontend/b85.ts` ↔ `src-tauri/src/b85_bytes.rs`,
miroir exact du crate `base85` 2.0.0) pour éviter les ~3,7 caractères/octet de
`serde_json` sur un `Vec<u8>` de ~360 ko par piste.
Depuis le 2026-08-03 la grille ne voyage **plus au repos** : elle n'est plus stockée dans
`tracks.report_json` et se recalcule à l'ouverture du collapse Diagnostic (631 ms mesurées).
Le rapport en cache est passé de 829 ko à 39 ko, la base de 4,11 Go à 119 Mo.

### Le frontend a deux vies : `app.js` et les modules live

`frontend/main.ts` importe **`app.js` inconditionnellement** (maquette d'origine, qui
tourne donc réellement en prod — c'est elle qui route les clics nav via
`e.target.closest('[data-view]')`), puis n'installe le wiring live **que si Tauri est
présent** (`"__TAURI_INTERNALS__" in window` → `installLiveWiring()`).

Conséquence à ne jamais oublier : `sift-live.ts`, `filing*.ts`, `report-view.ts`,
`*-view.ts` — tout ce qui touche l'IPC — **ne s'exécutent jamais dans un navigateur
classique**. Une capture issue de `npm run dev` ne prouve rien sur ces fichiers.

C'est aussi pourquoi la suite Vitest tourne en **env Node, sans jsdom** : un jsdom
fournirait un `window` SANS Tauri, donc n'exécuterait justement pas le wiring live — tout
en donnant l'illusion de le couvrir. Périmètre de `test/` : la **logique pure** dont une
erreur est silencieuse (codecs, échappement, mappings, calculs). Le DOM réel se vérifie
par la vraie fenêtre (skill `run-sift`, CDP), les états visuels par Storybook.

`dev-inspector.ts` / `selftest.ts` sont chargés dynamiquement sous
`import.meta.env.DEV` (éliminés du build de prod). Côté Rust, `dev_locate.rs` /
`dev_annotate.rs` sont gardés par `cfg!(debug_assertions)`.

### Modules frontend, par écran

`main.ts` (boot) · `router.ts` (routage réel — `app.js` ne tourne plus sous Tauri) ·
`view-epoch.ts` (feuille sans dépendance : époque de vue qui invalide un rendu async en
retard — correctif #42 de la course de rendu ; DOM-free, testable env Node) ·
`chrome.ts` (shell, barre unifiée, fenêtre) · `toolbar.ts` (recherche, actions et segmenté
de la barre) · `rail-sources.ts` (sources du rail, ex-Accueil — `home-sources.ts` supprimé
par `6d1cc85`) · `shortcuts.ts` (clavier couches 1-2) · `report-view.ts` (Revue :
son-d'abord, waveform, verdict) · `bibliotheque-view.ts` + `library-columns.ts` /
`library-views.ts` (table : colonnes persistées, rendus, tri) · `ecartes-view.ts` ·
`rekordbox-view.ts` · `reglages-view.ts` · `usb-view.ts` (Clé USB) · `library-detail.ts` ·
`context-menu.ts` (menu contextuel partagé).

`sift-live.ts` reste le **point d'entrée du wiring et le dispatch de clic centralisé**
pour la plupart des écrans. Un seul écart volontaire : `rekordbox-view.ts` porte son
propre `handleRekordboxAction`.

`filing.ts` n'est plus que l'orchestration résiduelle ; le reste vit dans
`filing-state.ts` (état `RevueState`), `filing-bins.ts` (arbre de destination),
`filing-identify.ts` (Discogs + éditeur + apply-tags), `filing-actions.ts`,
`filing-preview.ts`, `filing-toast.ts`. Les splits utilisent l'**injection de
dépendance** (`register*`/callback, ex. `registerOpenTrackPathGetter`,
`registerDestChangeHook`) pour casser les cycles d'import — jamais un import statique
retour.

Transverses : `ipc.ts` (wrappers IPC typés) · `errors.ts` (garantit le `console.error`
de la chaîne brute — pas de table code→message, délibérément) · `dom.ts` ·
`confirm-modal.ts` · `list-virtual.ts` · `queue-panel.ts` / `batch-panel.ts` /
`batch-tracklist.ts` (file Revue, mode Lot) · `journal.ts` (journal d'actions + revert) ·
`progress-zone.ts` · `theme.ts` · `updater.ts` · `usb-format-modal.ts` ·
`usb-row.ts` · `usage-chart.ts` (graphique d'occupation, Clé USB + Bibliothèque) ·
`empty-state.ts` · `library-views.ts` · `identify-shared.ts` · `genre-families.ts` ·
`popover-position.ts` (géométrie d'ancrage d'un popover `position:fixed`, **sans DOM** —
séparée de `filing-bins.ts` pour être testable en env Node, qui ne peut pas charger un
module important `./ipc`) · `source-color.ts` (teinte d'identité des sources du rail :
override manuel sinon cycle par ordre d'ajout — même motif sans-DOM/testable env Node,
importée par `rail-sources.ts`) · `rail-source-entry.ts` (markup d'une entrée de source du
rail : teintes, `--error`, suspendue — même motif sans-DOM/testable, story et Vitest exécutent
le vrai rendu, importé par `rail-sources.ts`) · `rails.ts` (miroir frontend unique de
`tags::rail_from_ext`) · `seg-thumb.ts` (pouce de contrôle segmenté partagé, ex-six copies) ·
`styles.css`.

`dev-inspector.ts` + `dev-annotate.ts` forment l'outil d'annotation **Alt+Clic**
(dev-only) : cadre de sélection, localisation du source via `locate_source`, note libre
envoyée par `save_annotation` qui append `docs/annotations.jsonl`.

### Backend : synchrone, un Mutex, des migrations append-only

- **Aucun runtime async.** Ni `tokio` ni `async-std` dans l'arbre. La concurrence est un
  pool `std::thread::spawn` dimensionné sur `available_parallelism()`, coordonné par
  `Arc<(Mutex<Queue>, Condvar)>` (`worker.rs`). Ne pas proposer de patterns async.
- **SQLite = `Mutex<Connection>` derrière un Tauri `State`.** Utiliser
  `db::lock_conn(&conn)`, jamais `.lock().map_err(...)` à la main.
- **`db.rs::MIGRATIONS`** : index + 1 == version de schéma (`PRAGMA user_version`).
  **Ne jamais réordonner ni éditer une entrée livrée — uniquement ajouter.** Une
  colonne morte se commente, elle ne se `DROP` pas (une migration sur les bases
  utilisateurs pour zéro octet récupéré).
- **Erreurs : enums à la main** (`Display` manuel + `impl Error`), `Result<T, String>` à
  la frontière IPC. Le projet n'utilise **pas** `thiserror`/`anyhow` — c'est un choix,
  pas un défaut ; ne pas les introduire sans le signaler.
- **`unwrap()`/`expect()` hors `#[cfg(test)]` = interdit dur.**
- **5 blocs `unsafe`**, tous avec leur `// SAFETY:` (recomptés le 2026-08-29) : deux dans
  `lib.rs` (`DwmExtendFrameIntoClientArea`, titlebar Win32 ; `DwmSetWindowAttribute`,
  coins arrondis #41) et **trois dans `usb_format/raw_volume.rs`** (handles Win32 sur
  volume brut). Un bloc touché garde ou met à jour son commentaire, jamais le supprimer.

Détails et overrides d'audit Rust : **`.claude/rules/rust.md`** (chargé pour
`src-tauri/**`).

### Modules Rust, par domaine

`analysis/` (decode Symphonia · verdict · spectrum · peaks · phase · dynamics ·
structure · tags) · `metadata/` (discogs · cover) ·
`usb_format/` (windows · macos · fat32 · raw_volume · sector_io · privileged — `fat32` et
`sector_io` sont gatés `cfg(any(windows, test))` : macOS formate par `diskutil eraseDisk`,
qui ignore le plafond de 32 Go, donc le binaire macOS ne lie plus `fatfs` ; le bras `test`
garde leurs tests exécutables sur n'importe quelle machine) ·
`volume_usage.rs` (occupation par format) ·
`scanner.rs` / `watcher.rs` / `sources.rs` / `worker.rs` / `queue.rs` (ingestion) ·
`reverdict.rs` (passe de re-verdict au démarrage : rejoue le verdict sur les mesures déjà stockées,
sans ré-analyse, pour toute ligne à `verdict_ver` périmée) ·
`filing.rs` / `actions.rs` / `encode.rs` / `naming.rs` / `tagging.rs` (rangement) ·
`dedup.rs` / `fingerprint.rs` · `library.rs` / `ecartes.rs` / `genres.rs` ·
`rekordbox_xml.rs` / `rekordbox_masterdb.rs` / `rekordbox_repairs.rs` (M8 Tier 1/2/3) ·
`ipc.rs` + `ipc_filing.rs` / `ipc_identify.rs` / `ipc_library.rs` / `ipc_usb.rs` /
`ipc_usage.rs` ·
`db.rs` / `settings.rs` / `ffmpeg.rs`.

Test-only : `bench_dedup.rs` (coût unitaire de `fingerprint::similarity`, taux de survie du
pré-filtre de durée, `group_duplicates` bout à bout, empreinte RAM) ·
`bench_volume.rs` (mesure `list_filed`/`list_pending` à 15k/100k lignes,
`EXPLAIN QUERY PLAN`) · `bench_sqlite.rs` (Phase 5 : attente du verrou et `SQLITE_BUSY`
sous charge, coût d'une analyse sur de vrais fichiers via `SIFT_BENCH_TRACKS_DIR`)
· `bench_cpu_budget.rs` (budget CPU partagé entre le pool d'analyse et le pool
d'encodage, mesuré en **débits** — fichiers/seconde sur une fenêtre de durée fixe — et
non en durées : la forme en durées a été réfutée à sa première exécution, l'analyse
finissant dans les 5 % premiers de la fenêtre commune)
— `--ignored`, jamais dans la suite normale.
`search_corpus.rs` / `search_terms.rs` : corpus de noms de fichiers **sales** tirés
d'une vraie bibliothèque, étalonné à la main. Chaque entrée est un motif verrouillé —
en ajouter un quand on rencontre un motif nouveau, **jamais en retirer un pour faire
passer un test**.

## Méthode

Détective (théorie → preuve → correctif), **fail fast**, **pas de fallback silencieux**,
changements chirurgicaux. Vérifier avant d'agir.

Garde-fous issus d'incidents réels :

- **Actions destructives/coûteuses** : jamais `window.confirm()`/`alert()`/`prompt()` —
  un clic synthétique en a déjà traversé un sans bloquer. Utiliser la confirmation
  in-app, armée et horodatée contre les doubles clics (`confirm-modal.ts`,
  `BATCH_CONFIRM_THRESHOLD` / `batchConfirmArmed`).
- **Systèmes live** : ne jamais écrire dans un vrai `master.db` Rekordbox sur la seule
  foi d'un rapport d'agent. Relire l'état indépendamment et vérifier le backup contre
  une référence propre juste avant l'écriture. Filet :
  `scripts/rekordbox-spike-helper.ps1` (`-Action backup|swap|restore|status`, chaque
  étape vérifiée par SHA256, refuse d'agir si Rekordbox tourne).
- **Tests réalistes** : un test qui seed un setting ou fichier de config doit passer par
  les mêmes validations et dispositions de fichiers que la production.
- **Un test de non-régression se mesure en mutant le code qu'il garde.** Vert ne veut pas
  dire tenant : casser volontairement la ligne visée et vérifier qu'il tombe, avant de le
  déclarer couvrant. Mesuré le 2026-08-05 sur `frontend/b85.ts` — les 24 vecteurs de
  round-trip produits par le crate Rust décodaient à l'identique avec la constante de
  padding 126, 84, 85 ou 127 ; la couverture du chemin de reste n'aurait tenu aucune des
  valeurs que son propre commentaire interdit. La tenir a demandé des entrées hors image
  de l'encodeur (`test/b85.test.ts`, `RUST_PADDING_PROBES`).
- **Prose française : par FICHIER, jamais par argument de ligne de commande.**
  `git commit -F -` et `gh issue create --body-file`, pas `-m "…"` ni `--title "…"` pour
  du texte long. Cause du strip d'accents **mesurée le 2026-08-26** (issue #43) : ce n'est
  ni une session ni un bug d'encodage, mais le texte destiné à un argument, rédigé en ASCII
  par prudence de quoting — l'évitement déborde sur la prose. Discriminant : sur 47 corps
  d'issue (canal `--body-file`), **zéro** désaccentué, alors que des titres (canal
  `--title`) l'étaient ; côté git, 147 messages intacts en `-F` contre 43 tombés en `-m`.
  Le canal transporte pourtant l'UTF-8 sans perte — vérifié en repassant douze titres
  accentués par `--title`. Deux gates tiennent la place : `npm run lint:accents` (blocs de
  commentaire, dans `verify.sh` et en CI) et `scripts/lint-commit-msg.mjs` (contrat d'un
  hook `commit-msg`). En corrigeant, ne toucher ni l'anglais ni les citations verbatim
  (annotations d'Antoine) — l'excès inverse est documenté (learning-log, 2026-08-19).
- **Debug UI** : après deux correctifs visuels infructueux, mesurer la vraie fenêtre
  `tauri dev` (CDP, ci-dessous) avant un troisième essai.
- **Pas d'affirmation d'implémentation invérifiable.** Toute phrase de rapport qui dit
  qu'une chose est faite se rattache à une preuve citable — `fichier:ligne`, sortie de
  commande, capture. Ce qui n'a pas été vérifié se dit « non vérifié », jamais au passé
  composé. Le risque n'est pas le mensonge, c'est de décrire l'intention comme si
  c'était l'artefact — et l'écart se concentre sur ce qu'une capture ne montre pas :
  état, focus clavier, chemin d'erreur. Mesure et source :
  `docs/skills/sift-ui-design-governance.md` § Design Theater.

## Front — CSS et événements

**Tout travail UI/UX/design passe par la skill `sift-macos-ui`
(`.claude/skills/sift-macos-ui/`) — la consulter avant toute recommandation visuelle.**

Sources de vérité design : **`frontend/styles.css`** (`:root`, canonique unique des
tokens) + `docs/design-system-states.md` (états réels des composants) +
**`docs/design-system/`** — 6 fichiers versionnés : `foundations.md` (personnalité
visuelle, anti-références), `components.md`, `tokens.md`, `patterns.md`, `governance.md`,
`content.md`. Ils portent l'historique **daté** des décisions de surface : les lire avant
de qualifier un écart visuel de « dérive » — la collision de surfaces relevée par l'issue
#23 était l'intersection de deux décisions volontaires, pas un accident. **Résolu le
2026-08-14** : les surfaces de contenu ont été retirées (avoir une surface devient la marque
de la charpente), et la mesure dans la vraie fenêtre a montré que le mécanisme invoqué par le
ticket — `.sift-ui-card` — n'existait **pas** dans le DOM de Revue. La leçon tient, sa cause
supposée non : lire l'historique daté ne dispense pas de mesurer.
⚠️ **`DESIGN.md` (racine) n'est PAS une source de décision.** Couche systémique posée le
2026-08-19, il a divergé du tracker le jour même (mouvement 100/150/250 en § 6/D-3 contre
la racine 75/150/300 de l'issue #10, recalé le 2026-08-20). Exécuter une décision de
design = lire le ticket de la map ; en cas d'écart, le tracker prime et l'écart se nomme
dans les deux fichiers.
Ne jamais créer de fichier de thème parallèle,
ni extraire une valeur d'une capture d'écran. ⚠️ `.interface-design/system.md` existe
encore sur le disque avec une **palette et une typo périmées** : ne jamais y puiser une
valeur. ⚠️ `docs/wireframes/<feature>.html`, cité ici jusqu'au 2026-08-05 comme lieu de
vie des wireframes de feature, **n'existe pas** — aucun `.html` nulle part sous `docs/`.
La seule maquette réelle est `frontend/app.js`, chargée inconditionnellement par
`frontend/main.ts` (§ Architecture) : elle tourne en prod, et ne fait pas autorité.

**Jamais de style ou de comportement UI sorti de la mémoire d'entraînement.** Avant tout
élément neuf sans exemple fourni, consulter une référence réelle et **citer laquelle** a
guidé la structure ou le comportement : micro-composants → MCP `shadcn`, MCP `ui-thing`,
puis 21st.dev ; décisions desktop → **Apple HIG § Foundations** (layout, matériaux,
typographie, accessibilité, densité) et **Apple Design Resources** (guides couleur
officiels — source amont de la palette de `styles.css`) ; référence donnée par Antoine →
la lire telle quelle, sans extrapoler. Ces sources servent à étudier structure, variantes
et états : ne jamais les installer dans `package.json`, ni copier leur palette, ni
déplacer les tokens de `styles.css`.

**Apple donne une chaîne de dérivation, presque jamais une valeur.** Avant de proposer un
arbitrage d'interface, la question n'est pas « quelle valeur Apple donne » mais **« Apple
laisse-t-elle cette question exister »**. Séparation ← le conteneur · poids d'icône ← le
texte voisin · rayon imbriqué ← la barre · tracking ← la taille · encre ← le matériau ·
densité ← un cran. Si la doc répond par une règle, ne pas transformer ses issues en menu
d'options : écrire la règle, et ne trancher que la **racine**, seule chose qu'Apple ne
publie jamais. Nommer la racine depuis une mesure du dépôt quand elle existe.

- **Les pages HIG ne se lisent pas avec `WebFetch`** — SPA, la réponse est « I don't have
  access to browse web pages ». Passer par le Browser pane (`get_page_text`), vérifié le
  2026-08-05 sur `designing-for-macos`. `developer.apple.com/design/` n'est qu'un hall
  d'entrée : il ne contient aucune page « principles », ceux-ci vivent dans les HIG.
- **HIG § « Designing for <plateforme> » : garder l'intention, jeter le mécanisme.** Sift
  cible Windows **et** macOS avec `"decorations": false`
  (`src-tauri/tauri.conf.json:22`) — donc aucune barre de menus native, et des boutons de
  fenêtre dessinés par l'app. Test à appliquer phrase par phrase : si elle nomme un
  **organe du système** (menu bar globale, Dock, Space, plein écran comme Space, position
  des traffic lights), elle ne vaut que pour la cible macOS ; si elle nomme un **fait
  humain ou matériel** (densité d'information confortable, distance de vue 0,3–0,9 m,
  raccourcis clavier comme accélérateurs, pointage de précision, personnalisation des
  vues), elle vaut pour les deux.

- Toute édition de token doit rester cohérente dans `:root`, le bloc sombre système
  (`prefers-color-scheme`) **et** `:root[data-theme="dark"]` — et comparer les valeurs
  **résolues** des deux tokens dans les deux thèmes, pas seulement leurs noms.
  Un token peut aussi avoir un **miroir JS déclaré** (`TOAST_EXIT_MS` ↔ `--duration-fast`
  dans `filing-toast.ts`, `QCOL_DEFAULT` ↔ `--pane-w` dans `router.ts`) : avant de changer
  une valeur, `grep -rn "stay in step|miroir de" frontend/` et faire suivre le miroir dans
  le même geste.
- **Concept avant chiffres.** Une demande impressionniste (« ça fait tassé », « pas
  assez sérieux ») ne se répond pas par une édition de token. La reformuler d'abord en
  vocabulaire de surface — quel écran, quelle décision utilisateur, deux directions
  candidates — puis descendre au px/token sur celle retenue. Deux correctifs visuels
  ratés d'affilée sur la même surface = l'accord de surface n'a jamais eu lieu :
  remonter, pas préciser. Détail et rationale :
  `docs/skills/sift-ui-design-governance.md` § Lexical Granularity.
- Renderer déclenché par un événement en rafale (progress, watcher, scroll, resize) :
  **créer les nœuds une fois, muter ensuite**. Jamais d'`innerHTML =` dans un handler
  appelé en boucle. Modèle : `progress-zone.ts`. En écrivant un handler, **nommer la
  fréquence supposée** de l'événement pour que le risque soit visible à la revue.
- Un `render*()` qui ajoute plusieurs blocs siblings à `#content` doit les envelopper
  dans **un seul wrapper** retiré/recréé en un point unique (modèle :
  `renderReglagesLive()`, wrapper `#sift-reglages-live`).
- Pas de `border-left`/`border-right` coloré comme accent : fond teinté existant.
- Un état confirmé **permanent** reste neutre ; ne colorer que sa transition, brièvement.
- Animer `transform`/`opacity`. Une transition n'anime rien si le render rebuild via
  `innerHTML` à chaque clic — vérifier le chemin de render d'abord.
- CTA à label descriptif : **texte seul**. Icône réservée à ce qui n'a pas d'équivalent
  textuel (spinner). `.lk-icon` est réservé aux boutons icône-seule 22×22.
- Un bouton qui redéfinit `background` doit le réaffirmer dans son `:hover` (sinon il
  perd face au `button:hover` générique).
- Toute règle auteur qui pose `display` bat `[hidden]` : garder `:not([hidden])` sur
  tout élément togglé par `hidden`.
- Un placement qui lit `offsetWidth`/`offsetLeft`/`getBoundingClientRect()` (pouce
  glissant, ancrage de popover, centrage mesuré) rend **0** tant que son conteneur est
  `display:none`. Ne le lancer que lorsque le conteneur est dans le flux, et le rejouer
  au point unique qui le réaffiche (handler d'onglet/catégorie), pas seulement au render
  initial ; mettre la fonction de placement au niveau module, pas en closure du render,
  pour que ce point de réaffichage puisse l'appeler. Mesuré 2026-08-20 : le pouce du
  segmenté Thème de Réglages, placé au render pendant que sa carte était cachée par la
  colonne de catégories, restait à 0px jusqu'au premier clic sur un bouton de thème.
- Avant qu'un `querySelector` dépende d'une classe, vérifier sa présence dans le markup
  réellement rendu — et **lire la règle CSS** avant de réutiliser un nom de classe
  générique.
- Tout élément rendu via `innerHTML` avec des données non fiables passe par `esc()`
  (`dom.ts`) — vérifier systématiquement à la création d'un nouveau fichier frontend :
  un XSS stocké réel a été livré par le seul fichier qui l'avait oublié.
  **Son contrat s'arrête au texte et aux valeurs d'attribut ENTRE GUILLEMETS**, et c'est
  suffisant parce qu'une URL ne devient jamais un attribut ici : elle part par `openUrl()`
  vers `ipc.rs::open_url`, qui refuse tout schéma autre que `http(s)://`. Gelé par
  `test/dom.test.ts`. Le premier `href="${…}"`, attribut non quoté ou donnée dans un
  `<script>` casse ce raisonnement — il demande alors une SECONDE fonction (`safeUrl` /
  `escAttr`), jamais un `esc()` élargi qui alourdirait les dizaines de sites corrects.

Jargon anglais volontairement conservé dans l'UI (ne pas « corriger ») : LOSSLESS,
DUPLICATE, MATCH, CHECK MATCH, FAKE, kbps, kHz, MP3, AIFF, WAV.

## Vérification UI — l'app réelle, pas la maquette

**Défaut : Antoine regarde lui-même la fenêtre `tauri dev`** (HMR, retour instantané) —
ne pas driver l'app à sa place. `computer-use` est écarté par défaut.

Pour une inspection ponctuelle du code `inTauri` réel :
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` **au lancement de
la commande dev** expose un endpoint CDP standard sur la vraie fenêtre WebView2
(`.claude/scripts/cdp.cjs`). Trois règles :

- **Ne JAMAIS** poser ce port via `additionalBrowserArgs` dans `tauri.conf.json` : il
  s'appliquerait aux builds de prod distribués et écrase les arguments par défaut de wry.
- Le port est squattable par un projet Tauri voisin : vérifier que `document.title`
  (ou `curl http://127.0.0.1:<port>/json`) correspond bien à Sift avant de faire
  confiance à une session. Si non, changer de port — ne jamais tuer le process d'un
  autre projet. **Constaté le 2026-08-05 : 9222 ET 9223 étaient tous deux tenus par un
  autre projet Tauri, dont le CDP répond normalement.** Mesurer sans vérifier l'identité
  produit un résultat faux et crédible.
- **`:hover` et `:focus-visible` ne se déclenchent PAS par le DOM** (`dispatchEvent`/`focus()`
  seuls) : seuls de vrais événements CDP les posent — `Input.dispatchMouseEvent` (mouseMoved aux
  coordonnées du nœud), `Input.dispatchKeyEvent` (Tab réel) puis `focus()`. `driver.mjs hover|focus`
  encapsule la recette ; détail : mémoire `cdp-real-pseudo-states-for-verification`.
- **Un override de token injecté doit s'écrire `:root:root:root`.** Le bloc sombre du dépôt
  est `:root:not([data-theme="light"])` (`styles.css:339` au 2026-08-20 — le numéro dérive,
  le retrouver par `grep -n ':root:not'`), de spécificité (0,2,0) : une
  feuille injectée en `:root` perd, quelle que soit sa position. Constaté le 2026-08-19 —
  quatre variantes d'encre capturées **octet pour octet identiques**, ce qui se lit comme
  « re-teinter ne change rien ». Relire la valeur calculée après injection ET après retrait,
  avant de faire confiance à une capture.

**Ne pas rejouer cette plomberie à la main** : le skill `run-sift`
(`.claude/skills/run-sift/`, invoquable par `/run-sift`) enveloppe le tout — choix d'un
port libre *et* vérifié Sift, attente du build, ouverture d'une piste jusqu'à ce que
`#mid` soit réellement peint, capture, arrêt des trois processus. Son `SKILL.md` porte les
douze pièges rencontrés ; son `driver.mjs` est le seul chemin agent recommandé.

**Deuxième instance dev (worktree, session concurrente)** : `tauri-plugin-single-instance`
avale tout second `sift.exe` en exit 0 silencieux, et `driver.mjs stop` tue les listeners
de 5173 + du port CDP mémorisé — la fenêtre d'une AUTRE session. Coexistence vérifiée
(2026-08-20) : `npm run tauri dev -- --config <json>` avec `identifier` sandbox (mutex +
appdata séparés, vraie DB intouchée) + `devUrl`/`beforeDevCommand` sur port Vite dédié,
CDP via `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`. Arrêt par PID de SA chaîne uniquement.

## Dépendances et docs externes

Avant d'écrire du code touchant une librairie externe, récupérer sa doc à jour via
**Context7** — ne jamais se fier à la mémoire d'entraînement pour une signature, un nom
de feature ou une config de version. Si le lookup échoue, le dire (fail-fast) plutôt que
deviner.

Audit de versions : `cargo outdated` → classer chaque écart (patch/minor sûr ; **bump
majeur = STOP**, signaler sans toucher) → changelog réel pour les breaking changes qui
touchent nos call sites → `cargo update` **crate par crate**, build + test entre chaque.
Jamais un update global, jamais un pin de contournement.

Ajouter une dépendance frontend demande une validation humaine explicite (voir
l'en-tête de `frontend/b85.ts`).

## `docs/` — liste blanche depuis le 2026-07-31 (issue #5)

`.gitignore` ignore le **contenu** de `docs/` et ne ré-autorise que ce qui fait
autorité : `install-non-signe.md`, `manuel.md`, `design-system-states.md`,
`ressources-externes.md`, `cdj-metadata-formats.md`, `design-system/`, **`ui-specs/`**,
**`design-refs/`**, `skills/`, `agents/`, et **chaque dossier de chantier** de
`superpowers/changes/` pris un par un. ⚠️ Les **specs d'écran sont SUIVIES**
(`docs/ui-specs/`, 7 fichiers) : une décision de design s'y consigne et se pousse. Restent
hors suivi : plans de jalons, revues et comptes rendus de chantier.

- Un nouveau document sous `docs/` est ignoré **par défaut** ; le publier demande
  d'ajouter sa négation au `.gitignore` dans le même geste. C'est voulu — la liste noire
  précédente laissait repasser tout fichier neuf. ⚠️ Vaut AUSSI pour un nouveau dossier
  sous `superpowers/changes/` : `.gitignore` ré-autorise le dossier `changes/` puis
  ré-ignore son contenu (`docs/superpowers/changes/*`), donc il faut une ligne
  `!docs/superpowers/changes/<date>-<slug>/` par chantier. Sans elle le dossier n'existe
  pas pour git, en silence — constaté le 2026-08-02.
- Les chemins `docs/superpowers/...` cités ici ou dans le code sont réels **localement**
  et introuvables dans un clone frais : ne pas les « réparer » en les supprimant, ne pas
  les citer à un lecteur externe.
- L'historique n'a pas été réécrit : tout reste récupérable par `git log --`.
- Un nouveau chantier ouvre **un seul dossier** `docs/superpowers/changes/<date>-<slug>/`
  contenant `design.md` / `plan.md` / `review.md` — pas `specs/` + `plans/` + `reviews/`
  à plat. Les fichiers antérieurs à la convention n'ont pas été migrés.

Deux références lourdes, **à lire à la demande via leur sommaire en tête de fichier**
(`Read offset=<L>`), jamais en entier : `docs/ressources-externes.md` (décisions
techniques ; section « Écarté » = tranché, « Différé » = pas assez de preuve, avec
trigger de réouverture — ne pas confondre) et `docs/design-system-states.md` (états
réels des composants, miroir des stories Storybook).

⚠️ **Ces sommaires citent des numéros de ligne, donc une section qui grandit les périme
en bloc.** Agrandir une section décale tout ce qui la suit, et le sommaire devient faux
sur toutes ses entrées suivantes — silencieusement, puisqu'un `Read offset=<L>` tombe
alors à côté sans erreur. Renuméroter dans le MÊME geste. La vérification ne se fait pas
à l'œil : comparer la distribution des écarts sommaire → titre avec celle de
`git show HEAD:<fichier>` ; identiques = juste. C'est ce qui a attrapé un off-by-one le
2026-08-13, invisible autrement.

⚠️ `docs/INDEX.json` **n'existe plus** — vérifié absent le 2026-08-05. Ce catalogue local
listait chaque document ; sa règle de maintien (« créer un document sous `docs/` veut
dire y ajouter son entrée dans le même geste ») est donc **caduque** : ne pas la suivre,
et ne pas recréer le fichier sans décision explicite. Il n'a jamais été versionné, aucun
`git log` ne le ramène. Pour trouver le statut d'un chantier, lire directement le dossier
`docs/superpowers/changes/<date>-<slug>/` correspondant.

## Release

**Écrire la section `## vX.Y.Z` de `CHANGELOG.md` d'abord.** `release.yml` l'extrait via
`scripts/changelog-section.mjs` et la passe en `releaseBody` ; le script sort en code 1 si
elle manque, donc la release **échoue** au lieu de publier des notes vides. Ce texte ne sert
pas qu'à la page GitHub : `tauri-action` le recopie dans le champ `notes` de `latest.json`,
que **chaque installation existante télécharge**. Éditer le corps d'une release après coup
change la page GitHub mais PAS `latest.json`, généré au build.

Synchroniser les versions de `package.json`, `src-tauri/Cargo.toml` et
`src-tauri/tauri.conf.json`, depuis `main`. Après `git tag vX.Y.Z && git push --tags`,
`release.yml` publie un **brouillon** (`releaseDraft: true`) : sans publication manuelle
sur GitHub, `/releases/latest/` ne résout jamais le brouillon et **l'auto-update ne
trouve rien, silencieusement, pour toujours**.

## Pointeurs

- `.claude/rules/context-packs.md` — packs de contexte par type de tâche.
- `.claude/rules/rust.md` — override projet pour tout travail sur `src-tauri/**`.
- `.claude/skills/run-sift/` — lancer et **piloter** la vraie fenêtre (§ Vérification UI).
  Versionné par une négation ciblée dans `.gitignore`, contrairement au reste de
  `.claude/skills/`.
- `docs/skills/sift-ui-design-governance.md` — gouvernance des décisions UI (versionné).
- `docs/superpowers/changes/2026-08-05-hig/` — repliage des Apple HIG : inventaire des
  écarts mesurés (`design.md`), plan (`plan.md`), protocole et résultats de vérification
  (`review.md`).
- `.claude/learning-log.md` — incidents et leçons machine-locales.
- `AGENTS.md` — simple pointeur vers ce fichier (ne pas y dupliquer de contenu).

## Agent skills

### Issue tracker

Les issues vivent dans les GitHub Issues de `c0remusic/sift`, pilotées via la CLI `gh`. Voir `docs/agents/issue-tracker.md`.

### Triage labels

Les cinq rôles canoniques, chaînes de label inchangées (`needs-triage`, `needs-info`,
`ready-for-agent`, `ready-for-human`, `wontfix`) — les cinq existent réellement sur le
dépôt, vérifiés par `gh label list` le 2026-08-11, aux côtés des six `wayfinder:*`.
Voir `docs/agents/triage-labels.md`.

### Domain docs

Dépôt single-context : le glossaire est `CONTEXT.md` à la racine. **`docs/adr/` n'existe
pas** (vérifié absent le 2026-08-11) — c'est l'emplacement prévu si un ADR est un jour
écrit, pas un dossier à lire. `docs/agents/domain.md` demande d'ailleurs de passer en
silence sur son absence, et non de le créer d'avance. Voir `docs/agents/domain.md`.

### Wayfinder

Chantier trop gros pour une session : `/wayfinder` charte la carte sur le tracker ci-dessus. Labels `wayfinder:map` et `wayfinder:{research,prototype,grilling,task}` créés. Sous-issues et blocage natif GitHub disponibles — pas de repli par convention de corps.
