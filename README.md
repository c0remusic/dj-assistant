# Sift

> **Sift** (nom de travail) — le poste de prépa entre Soulseek et les platines.
> App desktop (Windows + macOS) qui **écoute, vérifie et range** tes téléchargements :
> repère les **faux fichiers** (MP3 transcodés vendus pour du lossless) au spectrogramme,
> évite les **doublons** et ce qui est **déjà dans ta biblio**, **convertit au format CDJ**
> au moment du rangement, **renomme** depuis Discogs, et pousse tes dossiers en **playlists
> Rekordbox**. Un seul geste par morceau : écouter → ranger ou écarter.

## Se servir de Sift

| | |
|---|---|
| **Télécharger** | [dernière version](https://github.com/c0remusic/sift/releases/latest) — Windows (`x64-setup.exe`), Mac Apple Silicon (`aarch64.dmg`), Mac Intel (`x64.dmg`) |
| **Installer** | [`docs/install-non-signe.md`](docs/install-non-signe.md) — quel fichier prendre, et quoi faire de l'avertissement du système |
| **Utiliser** | [le manuel en ligne](https://dj-assistant-navy.vercel.app/manuel.html) · [en PDF](https://dj-assistant-navy.vercel.app/manuel.pdf) · [`docs/manuel.md`](docs/manuel.md) — le vocabulaire, les huit écrans, le clavier, et ce que la détection laisse passer |

Ces deux liens passent avant tout ce qui suit, et ce n'est pas une question de mise en page :
quelqu'un à qui on envoie ce dépôt cherche une app, pas une fiche de projet. Tout ce qui vient
ensuite s'adresse à qui veut la **construire**.

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
| **M6b — Bibliothèque** | ✅ **fait** — parcourir/éditer/re-ranger, doublons internes (empreinte), dashboard de stats cliquable (`library.rs`, `dedup.rs`, `ipc_library.rs`, `library-detail.ts`, `sift-live.ts`) |
| **M7 — Export Rekordbox + clé USB** | ✅ **fait** — export/suivi XML Rekordbox (playlists protégées d'un renommage/déplacement/reformat via le `TrackID`, jamais le chemin), formatage clé USB FAT32/exFAT Windows+macOS (`rekordbox_xml.rs`, `usb_format/`, `ipc_library.rs`, `ipc_usb.rs`) |
| **M8 — Écriture directe `master.db` Rekordbox** | ✅ **fait** — Tier 1 (réparation de chemins), Tier 2 (dédoublonnage playlists), Tier 3 (synchro métadonnées + pochette), chaîne de sûreté backup/vérif round-trip/rollback, vérifié contre une vraie bibliothèque (2828 pistes, 2026-07-12) (`rekordbox_masterdb.rs`, `rekordbox_repairs.rs`, `ipc_library.rs`) |

Scope V1 restant (décidé au brainstorm) : diffusion —
**auto-update Tauri ✅ fait** (2026-07-24, gratuit, sans certificat OS, vérifié en
conditions réelles sur `v0.0.1`/`v0.0.2`) ; code-signing Windows +
notarization macOS + site **différés** (budget) — `.github/workflows/build.yml`
build encore des installeurs non signés, contournement documenté dans
[`docs/install-non-signe.md`](docs/install-non-signe.md).

La maquette UI/UX d'origine vit dans `index.html` + `frontend/` (migrée comme shell frontend
de l'app).

Les documents de planification et de session (plans de jalons, specs, revues, comptes rendus)
ne sont plus suivis par git depuis le 2026-07-31 : ils décrivaient un état que le code a
dépassé, et un contributeur ne pouvait pas savoir lesquels faisaient encore autorité. Ils
restent lisibles dans l'historique du dépôt, qui n'a pas été réécrit. Ce qui fait autorité
aujourd'hui : ce `README.md` pour l'état, `CLAUDE.md` pour les conventions, et les sources
de vérité vivantes listées dans [`docs/`](docs/).

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

## Installer (utilisateur final)

Voir § [Se servir de Sift](#se-servir-de-sift) en tête de ce fichier — les deux liens y sont, et
n'étaient ici qu'à la ligne 72, sous la pile technique et les prérequis de développement.

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
├── frontend/                   # UI
│   ├── main.ts · app.js        #   boot + maquette navigateur
│   ├── sift-live.ts            #   point d'entrée wiring live (Tauri only)
│   ├── chrome.ts               #   shell global (nav rail, routing)
│   ├── home-sources.ts         #   écran Accueil
│   ├── ecartes-view.ts         #   écran Écartés
│   ├── report-view.ts          #   écran Revue (son-d'abord, waveform)
│   ├── filing.ts               #   rail de classement
│   ├── batch-tracklist.ts      #   tracklist batch
│   ├── journal.ts              #   journal d'actions post-batch
│   ├── progress-zone.ts        #   progression encodage
│   ├── library-detail.ts · identify-shared.ts · selftest.ts · dom.ts
│   ├── ipc.ts · styles.css
├── shared/contracts.ts         # types IPC partagés (miroir manuel des structs Rust)
├── scripts/fetch-ffmpeg.mjs    # télécharge le binaire FFmpeg par OS
├── src-tauri/src/              # backend Rust (lib = sift_lib)
│   ├── analysis/               #   décodage Symphonia + DSP (verdict, peaks, spectre, phase…)
│   ├── metadata/               #   Discogs + cover + apply_identity
│   ├── scanner.rs · watcher.rs · sources.rs · worker.rs · queue.rs
│   ├── filing.rs · actions.rs · encode.rs · naming.rs · tagging.rs
│   ├── dedup.rs · fingerprint.rs · ecartes.rs · library.rs · genres.rs · ffmpeg.rs
│   ├── db.rs · settings.rs · lib.rs · main.rs
│   ├── ipc.rs · ipc_filing.rs · ipc_identify.rs · ipc_library.rs
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

## Site (Vercel)

https://dj-assistant-navy.vercel.app — intégration Git : chaque push sur `main` lance
`npm run build` (`vercel.json` : Vite, sortie `dist/`). Deux choses y vivent :

- `/manuel.html` et `/manuel.pdf` — le manuel dans le design de l'app, généré depuis
  `docs/manuel.{html,pdf}` par `scripts/build-manuel.mjs` (`prebuild`) dans `public/`, gitignoré.
- la racine — la maquette de l'interface, sans backend : les appels IPC échouent silencieusement
  hors app native. Une démo, pas l'app.
