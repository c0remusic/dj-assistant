# Sift

> **Sift** (nom de travail) — le poste de prépa entre Soulseek et les platines.
> App desktop (Windows + macOS) qui **écoute, vérifie et range** tes téléchargements :
> repère les **faux fichiers** (MP3 transcodés vendus pour du lossless) au spectrogramme,
> évite les **doublons** et ce qui est **déjà dans ta biblio**, **convertit au format CDJ**
> au moment du rangement, **renomme** depuis Discogs, et pousse tes dossiers en **playlists
> Rekordbox**. Un seul geste par morceau : écouter → ranger ou écarter.

## État du projet

| Jalon | Statut |
|---|---|
| **M0 — Scaffolding** | ✅ **fait** — Tauri v2 boote, FFmpeg sidecar bundlé (`ffmpeg-sidecar`), SQLite + migrations, IPC typé, CI Win+Mac |
| **M1 — Watcher + file « à traiter »** | ✅ **fait** — multi-dossiers, scan complet + diff, watcher live (`notify`), file = `tracks pending`, UI Accueil + Revue câblées |
| **M2 — Analyseur (waveform/spectro/verdict)** ⭐ | ✅ **fait** — décodage **Symphonia** (pur Rust), `rustfft`, verdict fake/grey, clipping/troncature/silence/DC/phase, cache DB (`analysis/`, `worker.rs`) |
| **M3 — Player + tempo** | ✅ **fait** — WaveSurfer v7 (lecture native AIFF/WAV/FLAC/MP3), key-lock `preservesPitch`, fader tempo (`report-view.ts`) |
| **M4 — Encodeur + « déplacer = encoder + ranger »** ⭐ | ✅ **fait** — 2 rails, anti-upscale, tags+nommage, bacs, undo/corbeille (`encode/naming/tagging/filing/actions.rs`, `filing.ts`) |
| **M4b — Écartés** | ✅ **fait** — re-sourcer/corbeille, liens d'achat, copie Soulseek (`ecartes.rs`) |
| **M5 — Dédup par empreinte** | ✅ **fait** (flux entrant) — `name_key` + `rusty-chromaprint` à la demande (`dedup.rs`, `fingerprint.rs`) |
| **M6a — Identification Discogs** | ✅ **fait** — trait `MetadataProvider`, cascade, pochette + genres + `release_id` (`metadata/discogs.rs`, `ipc_identify.rs`) |
| **M6b — Bibliothèque** | 🚧 **en cours** — parcourir/éditer/re-ranger (`library.rs`, `library-detail.ts`) |
| M7 (Rekordbox XML + batch + clé USB) · M8 (écriture Rekordbox, **gelée**) | à venir — voir [`docs/plan-implementation.md`](docs/plan-implementation.md) |

La maquette UI/UX d'origine vit dans `index.html` + `frontend/` (migrée comme shell frontend
de l'app). Le découpage complet et les décisions de cadrage : [`docs/plan-implementation.md`](docs/plan-implementation.md).
Le plan détaillé de M0 : [`docs/plans/2026-06-12-m0-scaffolding.md`](docs/plans/2026-06-12-m0-scaffolding.md).

## Pile technique

| Brique | Choix |
|---|---|
| Shell desktop | **Tauri v2** (Rust + WebView), frontend **Vite** vanilla |
| Décodage analyse | **Symphonia** (pur Rust, in-process) → `rustfft` — pas de spawn par fichier |
| Conversion / encodage | **FFmpeg** via le crate **`ffmpeg-sidecar`**, binaire bundlé (Tauri `externalBin`) |
| Waveform/lecture | **wavesurfer.js** v7 (lecture native, key-lock `preservesPitch` pour le nudge tempo) |
| Empreinte | **`rusty-chromaprint`** (dédup local) — AcoustID en ligne = piste future |
| État | **SQLite** (rusqlite, bundled) — migrations via `PRAGMA user_version` |

## Prérequis dev

- **Node** ≥ 20 (testé sur 24) + npm
- **Rust** (stable, toolchain MSVC sur Windows) — https://rustup.rs
- Tauri v2 (CLI fournie en devDependency)

## Lancer l'app (dev)

```bash
npm install
npm run fetch-ffmpeg     # télécharge le binaire FFmpeg dans src-tauri/binaries/ (par OS)
npm run tauri dev        # compile le backend Rust + ouvre la fenêtre native
```

- Tests Rust : `cargo test --manifest-path src-tauri/Cargo.toml`
- Type-check frontend : `npx tsc --noEmit`
- Build installeurs (non signés) : `npm run tauri build` → `src-tauri/target/release/bundle/`

## Lancer juste le frontend web (sans Tauri)

```bash
npm run dev              # Vite sur http://localhost:5173
```

> Le frontend rend la même UI que l'app native (les appels IPC Tauri échouent silencieusement
> hors de l'app — c'est attendu). Utile pour itérer vite sur l'UI/UX dans un navigateur.

## Structure

```
sift/
├── index.html                  # entrée Vite (markup + shell nav de l'app)
├── frontend/                   # UI : main.ts (boot) · app.js (maquette navigateur)
│   ├── sift-live.ts            #   wiring live (Tauri only) : remplace la maquette par du réel
│   ├── filing.ts · report-view.ts   # écran Revue (rail de classement · détail son-d'abord)
│   ├── library-detail.ts · identify-shared.ts · selftest.ts
│   ├── ipc.ts · styles.css
├── shared/contracts.ts         # types IPC partagés (miroir manuel des structs Rust)
├── scripts/fetch-ffmpeg.mjs    # télécharge le binaire FFmpeg par OS
├── src-tauri/src/              # backend Rust (lib = sift_lib)
│   ├── analysis/               #   décodage Symphonia + DSP (verdict, peaks, spectre, phase…)
│   ├── metadata/               #   Discogs + cover + apply_identity
│   ├── filing/queue/dedup/encode/worker/scanner/watcher + ipc_*.rs
│   ├── binaries/               #   ffmpeg-<triple> (gitignored, fetché)
│   └── tauri.conf.json
├── docs/                       # plan d'implémentation + plans/specs/reviews par jalon
├── audit/                      # audit de direction (lecture seule, 2026-06)
└── .github/workflows/build.yml # CI : .msi (Win) + .dmg (Mac)
```

## CI

Chaque push sur `main` build des installeurs **non signés** pour Windows (`.msi`/`.exe`) et
macOS (`.dmg`), uploadés en artefacts. Le code-signing / notarization + auto-update sont prévus
en V1 (app diffusée gratuitement).

## Démo web (Vercel)

> ⚠️ Le déploiement statique d'origine ne fonctionne plus tel quel : depuis la migration Vite,
> `index.html` importe un module TypeScript qui doit être **buildé**. Pour une démo web,
> configurer Vercel avec build `npm run build` et output `dist/` (la même UI s'affiche ; les
> appels IPC échouent silencieusement hors app native).
