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
