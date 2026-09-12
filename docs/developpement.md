# Sift — développement

Pour qui veut **construire** Sift. L'utilisateur final n'a rien à faire ici : le README du
dépôt donne les installeurs et le manuel.

Conventions, garde-fous et architecture détaillée : [`CLAUDE.md`](../CLAUDE.md) (lu par les
agents, valable pour les humains). Vocabulaire métier : [`CONTEXT.md`](../CONTEXT.md).

## Pile technique

| Brique | Choix |
|---|---|
| Shell desktop | **Tauri v2** (Rust + WebView), frontend **Vite** + TypeScript vanilla, sans framework |
| Décodage analyse | **Symphonia** (pur Rust, in-process) → `rustfft` — pas de spawn par fichier |
| Conversion / encodage | **FFmpeg** en sidecar bundlé (Tauri `externalBin`), build LGPL sur macOS |
| Waveform / lecture | **wavesurfer.js** v7 (lecture native, key-lock `preservesPitch`) |
| Empreinte | **`rusty-chromaprint`** (dédup local) |
| État | **SQLite** (rusqlite, bundled) — migrations append-only via `PRAGMA user_version` |
| Clé USB | **`fatfs`** (FAT32 au-delà de 32 Go sur Windows, MIT), `diskutil` sur macOS |
| Rekordbox | XML + écriture directe `master.db` (chaîne backup / vérification / rollback) |

## Prérequis

- **Node** ≥ 24 + npm
- **Rust** stable, épinglé par `rust-toolchain.toml` (toolchain MSVC sur Windows) — https://rustup.rs
- Tauri v2 (CLI en devDependency)

## Lancer

```bash
npm ci
npm run fetch-ffmpeg     # sidecar FFmpeg → src-tauri/binaries/ (Windows : depuis PowerShell, pas Git Bash ;
                         # macOS : compile FFmpeg depuis les sources, plusieurs minutes)
node scripts/make-fixtures.mjs   # fixtures audio des tests (dépend du sidecar)
npm run tauri dev        # backend Rust + fenêtre native, Vite sur 5173
```

- Frontend seul dans un navigateur : `npm run dev` — même UI, mais les appels IPC échouent
  silencieusement hors app native : ça ne prouve rien sur le wiring live.
- Tests : `npm run test` (Vitest, logique pure en env Node) ·
  `cargo test --manifest-path src-tauri/Cargo.toml`
- Lints : `npm run lint`, `lint:tokens`, `lint:accents`, `lint:css-comments`, `check:security`,
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Type-check : `npx tsc --noEmit`
- Installeurs non signés : `npm run tauri build` → `src-tauri/target/release/bundle/`
- Storybook (états UI) : `npm run storybook`

La liste complète des commandes et leurs pièges : `CLAUDE.md` § Commandes.

## Structure

```
sift/
├── index.html                  # entrée Vite (markup + rail de l'app)
├── frontend/                   # UI — un module par écran, wiring live dans sift-live.ts
├── shared/contracts.ts         # types IPC, miroir manuel des structs Rust (tests Rust le tiennent)
├── src-tauri/src/              # backend Rust (crate lib = sift_lib)
│   ├── analysis/               #   décodage Symphonia + DSP (verdict, spectre, peaks, phase…)
│   ├── metadata/               #   Discogs + pochette
│   ├── usb_format/             #   FAT32 brut (Windows), diskutil (macOS)
│   ├── rekordbox_*.rs          #   XML + master.db
│   └── ipc*.rs                 #   frontière IPC
├── scripts/                    # fetch-ffmpeg, fixtures, lints, notes de version, manuel
├── test/                       # Vitest
├── docs/                       # ce qui fait autorité : manuel, design system, specs d'écran
└── .github/workflows/          # test.yml (toute branche) · build.yml (main) · release.yml (tags)
```

Le détail module par module : `CLAUDE.md` § Architecture.

## CI et release

- `test.yml` sur toute branche : tsc → Vitest → ESLint → `cargo fmt --check` → clippy → `cargo test`.
- `build.yml` sur `main` : installeurs non signés Windows + macOS (Apple Silicon et Intel), en artefacts.
- `release.yml` sur un tag `vX.Y.Z` : construit les trois cibles, extrait la section `## vX.Y.Z`
  de `CHANGELOG.md` (échoue si elle manque) et publie un **brouillon** — à publier à la main,
  sinon l'auto-update ne voit rien. Détail : `CLAUDE.md` § Release.

Les builds ne sont pas signés (code-signing Windows et notarization macOS différés) :
[`install-non-signe.md`](install-non-signe.md) documente le contournement.

## Site (Vercel)

https://sift-music.vercel.app — projet Vercel `dj-assistant` (domaine `sift-music.vercel.app` ajouté le
2026-09-10 par `vercel domains add` — `sift.vercel.app` appartient à un autre projet ;
`sift-dj.vercel.app` et `dj-assistantapp.vercel.app` répondent encore), intégration Git : chaque push
sur `main` est un déploiement de production (`npm run build`, `vercel.json` : Vite, sortie
`dist/`). `scripts/build-site.mjs` (`prebuild` / `postbuild`) y ajoute la page d'accueil
(`docs/accueil.html`), le manuel (`docs/manuel.html`, `manuel.pdf`) et la capture d'écran, via
`public/` gitignoré ; sur Vercel seulement (`VERCEL=1`), la racine devient l'accueil et l'app
passe en `/app.html` — partout ailleurs `dist/index.html` reste l'app, que Tauri embarque.
`/app.html` est la maquette de l'interface sans backend — une démo, pas l'app.

Les URL `dj-assistant-<hash>-c0re-s-projects.vercel.app` des statuts GitHub sont protégées par le
SSO Vercel : seul le domaine ci-dessus est public. `vercel project ls` donne le domaine courant.

## Jalons

M0 → M8 tous livrés (scaffolding, watcher et file, analyseur, lecteur, encodeur et rangement,
écartés, dédup par empreinte, identification Discogs, bibliothèque, export Rekordbox et clé
USB, écriture directe `master.db`). Auto-update Tauri en place depuis la v0.0.2. L'historique
détaillé par version : [`CHANGELOG.md`](../CHANGELOG.md).

Les documents de planification (plans de jalons, specs, revues) ne sont plus suivis par git
depuis le 2026-07-31 : ils décrivaient un état que le code a dépassé. Ils restent lisibles dans
l'historique, qui n'a pas été réécrit.
