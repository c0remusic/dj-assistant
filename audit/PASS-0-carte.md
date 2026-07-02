# PASS 0 — Carte du projet

> Cartographie factuelle avant les 9 passes d'audit. Aucune modification de
> code. Chiffres obtenus par lecture directe (`wc -l`, `grep`) le 2026-07-02
> sur le worktree `dj-assistant-m6a` (branche `m6a-discogs`).

## Arborescence — Rust (`src-tauri/src/`, 8639 lignes, 32 fichiers)

Fichiers plats + 2 sous-dossiers (`analysis/`, `metadata/`), triés par taille :

| Fichier | Lignes | Rôle |
|---|---|---|
| `filing.rs` | 916 | Logique de rangement (déplacement, conversion, nommage) |
| `actions.rs` | 801 | Journal d'actions (file/reject/trash/revert) |
| `ipc_filing.rs` | 588 | 23 commandes Tauri — filing, journal, bins, settings |
| `metadata/discogs.rs` | 525 | Client API Discogs (search/master/release) |
| `library.rs` | 488 | Écran Bibliothèque (liste, filtres, facettes) |
| `naming.rs` | 379 | DSL de masks pour nommage de fichiers |
| `ipc.rs` | 359 | 15 commandes Tauri — sources, queue, scan, analyse |
| `metadata/mod.rs` | 350 | Orchestration metadata (tags + cover) |
| `tagging.rs` | 330 | Écriture tags ID3/Vorbis via `lofty` |
| `worker.rs` | 324 | Worker d'analyse en arrière-plan |
| `db.rs` | 286 | Schéma SQLite + 8 migrations versionnées |
| `ecartes.rs` | 293 | Écran Écartés (rejets) |
| `scanner.rs` | 270 | Scan disque + identité fichier (taille+mtime) |
| `encode.rs` | 258 | Encodage FFmpeg (conversion CDJ) |
| `analysis/spectrum.rs` | 254 | FFT / spectrogramme |
| `dedup.rs` | 252 | Dédoublonnage (Chromaprint) |
| `analysis/mod.rs` | 210 | Orchestration pipeline d'analyse |
| `analysis/decode.rs` | 183 | Décodage Symphonia → PCM f32 |
| `watcher.rs` | 163 | File-watcher (notify-debouncer-full) |
| `sources.rs` | 145 | CRUD sources (dossiers surveillés) |
| `fingerprint.rs` | 130 | Génération empreinte Chromaprint |
| `lib.rs` | 120 | Bootstrap Tauri, state, invoke_handler (43 commandes) |
| `analysis/structure.rs` | 118 | Détection silence tête/queue, troncature |
| `analysis/dynamics.rs` | 116 | True peak, DC offset, phase |
| `analysis/verdict.rs` | 99 | Calcul du verdict ok/fake/grey |
| `analysis/tags.rs` | 93 | Lecture tags (ID3 version, cover présente) |
| `settings.rs` | 87 | Clé/valeur settings + session_id courant |
| `queue.rs` | 77 | File d'attente d'analyse |
| `analysis/phase.rs` | 70 | Corrélation de phase |
| `ipc_identify.rs` | 65 | 2 commandes Tauri — identify, apply_identity |
| `metadata/cover.rs` | 60 | Extraction/écriture pochette |
| `ipc_library.rs` | 60 | 3 commandes Tauri — bibliothèque |
| `analysis/peaks.rs` | 53 | Génération peaks (waveform) |
| `ffmpeg.rs` | 50 | Résolution du sidecar FFmpeg |
| `main.rs` | 6 | Point d'entrée, appelle `sift_lib::run()` |

## Arborescence — Frontend (`frontend/`, 6154 lignes TS/JS, 16 fichiers)

| Fichier | Lignes | Rôle |
|---|---|---|
| `sift-live.rs`… `sift-live.ts` | 1412 | Point d'entrée wiring live (Tauri only), délègue |
| `filing.ts` | 1653 | Rail de classement (destination, format, actions) |
| `report-view.ts` | 826 | Écran Revue (waveform, verdict, son-d'abord) |
| `journal.ts` | 358 | Journal d'actions post-batch (toasts, revert) |
| `app.js` | 352 | Maquette navigateur (source de vérité UI initiale) |
| `library-detail.ts` | 337 | Écran Bibliothèque (détail M6b) |
| `ipc.ts` | 240 | Wrappers IPC Tauri typés |
| `progress-zone.ts` | 204 | Zone de progression encodage |
| `chrome.ts` | 158 | Shell global (nav rail, routing écrans) |
| `ecartes-view.ts` | 113 | Écran Écartés |
| `home-sources.ts` | 113 | Écran Accueil (sources, watcher) |
| `selftest.ts` | 98 | Smoke tests IPC au démarrage |
| `batch-tracklist.ts` | 95 | Tracklist batch (multi-sélection, progression) |
| `identify-shared.ts` | 47 | UI partagée identification Discogs |
| `empty-state.ts` | 46 | États vides communs |
| `main.ts` | 41 | Boot |
| `theme.ts` | 37 | Bascule thème |
| `dom.ts` | 24 | Helpers DOM partagés |

Fichiers les plus volumineux du repo, tous langages confondus : **`filing.ts`
(1653)**, **`sift-live.ts` (1412)**, **`filing.rs`** (916, Rust), `report-view.ts`
(826), `actions.rs` (801, Rust).

## Frontière Rust ↔ Front

- **43 commandes Tauri** (`#[tauri::command]`), toutes exposées via un seul
  `invoke_handler![...]` dans [lib.rs:73-117](../src-tauri/src/lib.rs#L73-L117).
  Répartition : `ipc_filing.rs` 23, `ipc.rs` 15, `ipc_library.rs` 3,
  `ipc_identify.rs` 2.
- **Events Tauri** (Rust → front) : à vérifier en Pass 1/3 — `analysis_progress`
  est une commande (pull), pas confirmé si des `app.emit(...)` existent en plus
  pour du push (watcher, worker, filing).
- **State partagé** : `Mutex<Connection>` (SQLite, une connexion unique
  partagée par tout le process) + `ipc_filing::FilingCancel` (state d'annulation).
  Un seul point d'entrée DB — à vérifier en Pass 1 si tout le monde y passe ou
  si des fichiers ouvrent leur propre connexion.

## Schéma SQLite (8 migrations, `db.rs`)

Tables (7 au total après migration v8) : `tracks`, `metadata`, `custom_tags`,
`actions`, `sources`, `settings` (v4), `track_genres` (v6).

- `tracks` est la table centrale : identité fichier (path/hash/fingerprint),
  résultats d'analyse (verdict, cutoff_hz, true_peak_dbtp, dc_offset,
  phase_correlation, clip_runs...), état de pipeline (`status`,
  `target_format`, `confidence`, `analyzed_at`), cache JSON du rapport
  complet (`report_json`, v5 — évite un re-décodage à la réouverture).
- `actions` = journal d'audit/undo : type, from/to path, `batch_id` (groupe un
  filing), `undone` (0/1), `meta` (JSON, v7 — snapshot pré-édition pour revert
  des tags), `session_id` (v8 — actions groupées par session app).
- Index : `idx_tracks_source`, `idx_tracks_status`, `idx_tracks_analyzed`,
  `idx_track_genres_track`. Pas d'index sur `actions.batch_id` ni
  `actions.session_id` malgré des filtres probables dessus côté Journal — à
  vérifier en Pass 4 (perf) si ces requêtes sont sur le chemin chaud.
- Migrations strictement additives (`ALTER TABLE ADD COLUMN`, jamais de
  `DROP`/rename), versionnées par `PRAGMA user_version`, testées (8 tests dans
  `db.rs`, y compris idempotence). Pattern sain.
- `conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;
  PRAGMA busy_timeout = 5000;")` — WAL + busy_timeout déjà en place, MAIS le
  commentaire dit explicitement *"prep for moving off the single-connection
  model"* : aujourd'hui une seule connexion `Mutex`-partagée, donc ces
  réglages de concurrence ne servent à rien tant que ça reste vrai (à vérifier
  s'il existe un 2e point d'accès DB ailleurs, ex. worker sur son propre
  thread avec sa propre connexion — Pass 1/3).

## Intégrations

- **FFmpeg** : sidecar bundlé (`ffmpeg.rs`, `ffmpeg-sidecar` a priori), résolu
  au `setup()` (`ffmpeg::init_ffmpeg_path()`). Utilisé pour l'**encodage**
  (`encode.rs`, conversion CDJ) — decode-only côté analyse est délégué à
  Symphonia (décision actée dans `docs/ressources-externes.md`).
- **Symphonia** : `analysis/decode.rs`, décodage pur Rust → PCM `f32`,
  alimente `rustfft` (`analysis/spectrum.rs`, `analysis/peaks.rs`).
- **lofty** : `tagging.rs` (écriture ID3/Vorbis), `metadata/cover.rs`
  (pochette), `analysis/tags.rs` (lecture ID3 version).
- **rusty-chromaprint** : `fingerprint.rs` (génération), `dedup.rs`
  (comparaison Hamming pour dédoublonnage local).
- **Discogs (ureq)** : `metadata/discogs.rs`, `ipc_identify.rs` (commandes
  `identify` / `apply_identity_cmd`).
- **Spectrogramme** : généré côté Rust (`analysis/spectrum.rs`), consommé côté
  front dans `report-view.ts` (waveform SoundCloud-style, cf. commits récents
  sur le player).

## Fichiers concentrant le plus de logique (candidats prioritaires Pass 1/2/7)

1. `frontend/filing.ts` (1653 l.) — rail de classement, le plus gros fichier
   du repo tous langages confondus.
2. `frontend/sift-live.ts` (1412 l.) — point d'entrée wiring, déjà signalé
   comme "god file" dans le CLAUDE.md (D3, split prévu mais pas fait).
3. `src-tauri/src/filing.rs` (916 l.) — pendant Rust du rail de classement.
4. `frontend/report-view.ts` (826 l.) — écran Revue, complexité UI + player.
5. `src-tauri/src/actions.rs` (801 l.) — journal, undo/revert, logique batch.

## Plan de lecture pour les passes suivantes

- **Pass 1 (architecture)** : `lib.rs`, `ipc.rs`, `ipc_filing.rs`, `db.rs`,
  `sift-live.ts`, `chrome.ts`, `ipc.ts` (front) — frontière et couplage.
- **Pass 2 (qualité)** : recherche ciblée `unwrap()`/`expect()` sur tout
  `src-tauri/src/**/*.rs` (hors `#[cfg(test)]`), duplication front/back des
  seuils de verdict.
- **Pass 3 (bugs)** : `worker.rs`, `watcher.rs`, `filing.rs`, `actions.rs`,
  `ipc_filing.rs` (races, sentinel `__SOURCE__`, DB concurrente).
- **Pass 4 (perfs)** : `encode.rs`, `analysis/decode.rs`, `analysis/spectrum.rs`,
  `analysis/peaks.rs`, `sift-live.ts` (event handlers).
- **Pass 5/6 (UI/UX, produit)** : `report-view.ts`, `filing.ts`, `journal.ts`,
  `library-detail.ts`, `.interface-design/system.md`.
- **Pass 7 (maintenabilité)** : `Cargo.toml`, `package.json` vs versions cible
  actées (2026-06-30), couplage `sift-live.ts`/`filing.ts`.
- **Pass 9 (benchmark)** : nécessite recherche web — à vérifier en démarrant
  la passe si l'outil est disponible dans cette session.
