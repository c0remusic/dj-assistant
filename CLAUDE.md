# Sift — CLAUDE.md

> Worktree **`dj-assistant-m6a`** (branche `m6a-discogs`, **dev actif**). L'autre
> worktree `../dj-assistant` = branche `main` (base stable). Même repo Git.
> Contexte projet complet : la skill **`sift`** le charge.

## Quoi
App desktop **Tauri v2** (Win+Mac), gratuite, de prépa de musique pour DJ : analyse
(détection faux lossless), dédoublonnage, identification, rangement.
Principe : « déplacer = encoder + ranger ».

## Stack
Tauri v2 (Rust) · frontend Vite vanilla · **Symphonia** (décode analyse) + FFmpeg sidecar
bundlé (encode) · SQLite (`rusqlite`) · `rustfft` · `lofty` · `rusty-chromaprint` · `ureq`.
Lib = `sift_lib`. MSRV Rust 1.77.2.

## Commandes (Windows — npm via `cmd /c "npm …"`)
- Dev : `npm run tauri dev` (Vite 5173 + backend Rust)
- Build installeurs : `npm run tauri build` → `src-tauri/target/release/bundle/`
- Tests Rust : `cargo test --manifest-path src-tauri/Cargo.toml`
- Lint : `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Type-check front : `npx tsc --noEmit`

## Outillage (skills/agent/plugin — déjà câblés, personnalisés Sift)
- **rust-best-practices** (skill) → tout code Rust écrit/revu.
- **error-handling-patterns** (skill) → erreurs Rust/Tauri (`Result` + serde IPC, fail-fast ; retry réservé à Discogs/AcoustID).
- **release-skills** (skill) → release : bumper les **3** fichiers de version en synchro
  (`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`), depuis `main`.
- **rust-engineer** (agent) → Rust pointu (async/perf/unsafe).
- **rust-lsp** (plugin) → diagnostics rust-analyzer + hooks rustfmt/clippy/check.

### Outillage global additionnel (installé niveau utilisateur, dispo partout)
- **ui-ux-pro-max** + **impeccable** (plugins) → front Vite vanilla : design, polish,
  audit/critique d'UI (`/impeccable …`). À utiliser pour toute évolution d'interface.
- **architect** (agent) → design d'archi avant gros refactor / nouvelle feature.
- **ecc** (plugin Everything-Claude-Code) → agents/skills/workflows génériques.
- **tech-debt-audit** (skill, `/tech-debt-audit`) → audit de dette sur tout le repo (Rust + TS).

## Décisions techniques
Voir **`docs/ressources-externes.md`** : Symphonia hybride (decode analyse) + FFmpeg
(conversion), `rusty-chromaprint` conservé, Qdrant écarté, détection de key hors scope,
DSL de masks pour le rangement inspiré de MediaMonkey.

## Méthode
Détective (théorie → preuve → correctif), **fail fast**, **pas de fallback** silencieux,
changements chirurgicaux. Vérifier avant d'agir.

**Skills : routage systématique, pas vérification exhaustive.** Avant de planifier une
tâche, identifier son domaine et charger la skill correspondante si elle existe — pas
relire toute la liste à chaque micro-tâche. Si aucune skill ne correspond, continuer
sans (ne pas en inventer). Registre complet (skills projet + globales + plugins +
agents, avec domaine d'usage) : **`docs/skills-registre.md`** — toujours le consulter
en cas de doute plutôt que de supposer qu'un outil existe ou non.

## Structure frontend/ (état réel)
- `main.ts` — boot
- `app.js` — maquette navigateur (source de vérité UI initiale)
- `sift-live.ts` — point d'entrée wiring live (Tauri only) ; délègue aux modules ci-dessous
- `chrome.ts` — shell global (nav rail, routing écrans)
- `home-sources.ts` — écran Accueil (sources, watcher)
- `ecartes-view.ts` — écran Écartés
- `report-view.ts` — écran Revue (son-d'abord, waveform, verdict)
- `filing.ts` — rail de classement (destination, format, actions filer/écarter)
- `batch-tracklist.ts` — tracklist batch (multi-sélection, barre de progression)
- `journal.ts` — journal d'actions post-batch (toasts, revert)
- `progress-zone.ts` — zone de progression encodage
- `library-detail.ts` — écran Bibliothèque (M6b)
- `identify-shared.ts` — UI partagée identification Discogs
- `dom.ts` — helpers DOM partagés
- `ipc.ts` — wrappers IPC Tauri typés
- `selftest.ts` — smoke tests IPC au démarrage
- `styles.css` — tokens CSS + composants

## Structure src-tauri/src/ (état réel)
Fichiers plats (pas de sous-dossiers sauf `analysis/` et `metadata/`) :
- **`analysis/`** — `decode.rs` (Symphonia) · `mod.rs` · `dynamics.rs` · `peaks.rs` · `phase.rs` · `spectrum.rs` · `structure.rs` · `tags.rs` · `verdict.rs`
- **`metadata/`** — `mod.rs` · `discogs.rs` · `cover.rs`
- `lib.rs` · `main.rs` · `db.rs` · `settings.rs`
- `scanner.rs` · `watcher.rs` · `sources.rs` · `worker.rs` · `queue.rs`
- `filing.rs` · `actions.rs` · `encode.rs` · `naming.rs` · `tagging.rs`
- `dedup.rs` · `fingerprint.rs` · `ecartes.rs` · `library.rs` · `genres.rs`
- `ffmpeg.rs`
- `ipc.rs` · `ipc_filing.rs` · `ipc_identify.rs` · `ipc_library.rs`

## Front — événements répétés
- Renderer déclenché par un événement en rafale (progress, watcher, scroll, resize) :
  **créer les nœuds une fois, muter ensuite**. Jamais d'`innerHTML =` dans un handler
  appelé en boucle (sature le thread UI → feedback noyé, bug invisible à la lecture).
- En écrivant un handler sur événement, **nommer la fréquence supposée** de l'événement,
  pour que le risque de saturation soit visible à la revue, pas découvert au runtime.
