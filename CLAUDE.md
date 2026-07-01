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
- **rust-analyzer-lsp** (plugin) → connecteur LSP `.rs` (rustup component, pas une skill).

### Outillage global additionnel (installé niveau utilisateur, dispo partout)

#### UI / Design — ordre de priorité strict
- **impeccable** (plugin) → priorité n°1 pour retouche/polish d'un écran existant.
  Register `product` (PRODUCT.md créé 30/06). `/impeccable critique|audit|polish …`.
- **interface-design** (skill) → priorité n°2 retouche, source de vérité tokens
  (`.interface-design/system.md` — section Mode Batch PÉRIMÉE, reste valide).
- **design-flow** (skill) → priorité n°1 pour un **nouveau chantier UI** (nouveau
  screen, refonte significative). Orchestre en séquence : `grill-me` → `design-brief`
  → `information-architecture` → `design-tokens` → `brief-to-tasks` → `frontend-design`
  → `design-review`. Les 7 steps sont aussi invocables seules.
  ⚠️ NE PAS invoquer `design-tokens` sans vérifier `styles.css` — tokens déjà posés.
- **design-review** (skill) → audit post-implémentation systématique.
- **ui-ux-pro-max** (plugin) → Quick Reference (a11y/perf) ponctuelle UNIQUEMENT.
- **design-taste-frontend** → NE JAMAIS invoquer sur Sift (landing pages/marketing).
- **stitch-generate-design** / **enhance-prompt** / **stitch-loop** → exploration de
  directions visuelles via Google Stitch (génère HTML). Porter en vanilla TS manuellement.
  `stitch::react-components` / `shadcn-ui` / `remotion` = hors scope Sift.

#### Backend / méthode
- **architect** (agent) → design d'archi avant gros refactor / nouvelle feature.
- **ecc** (plugin Everything-Claude-Code) → agents/skills/workflows génériques.
- **tech-debt-audit** (skill, `/tech-debt-audit`) → audit de dette sur tout le repo (Rust + TS).
- **working-with-legacy-code** / **refactoring-patterns** → pour D3 (split de
  sift-live.ts, ~942 lignes) : couvrir le god file de tests avant de le découper.

## Décisions techniques
Voir **`docs/ressources-externes.md`** : Symphonia hybride (decode analyse) + FFmpeg
(conversion), `rusty-chromaprint` conservé, Qdrant écarté, détection de key hors scope,
DSL de masks pour le rangement inspiré de MediaMonkey.

## Méthode
Détective (théorie → preuve → correctif), **fail fast**, **pas de fallback** silencieux,
changements chirurgicaux. Vérifier avant d'agir.

**RÈGLE IMPÉRATIVE — routage skills (pas une suggestion, un arrêt obligatoire avant
toute tâche non-triviale, tous domaines confondus : Rust, frontend, UI/design, audit,
refactor, release, méthode).**
1. NE PAS commencer à agir directement sur une tâche substantielle.
2. Identifier le domaine de la tâche (Rust/backend, frontend/UI, audit de dette,
   refactor/legacy, méthode de planification, release, etc.).
3. Consulter `docs/skills-registre.md` pour ce domaine — c'est là que vivent les
   verdicts déjà vérifiés (quelles skills sont adaptées à Sift vs hors-scope/fausses
   pistes), plutôt que de deviner depuis le nom d'une skill ou de l'auto-déclenchement
   heuristique seul.
4. Invoquer EXPLICITEMENT la ou les skills pertinentes trouvées — les nommer dans le
   raisonnement avant d'agir, ne pas se contenter qu'elles se déclenchent en silence.
5. Si aucune skill ne correspond après consultation, continuer sans en inventer une.
Exemples de domaines et leur routage (liste non exhaustive, voir le registre complet) :
- Rust/backend → `rust-best-practices`, `error-handling-patterns`, `rust-engineer`.
- UI/design retouche/polish → `impeccable` (priorité 1) ou `interface-design` (priorité 2).
- UI/design nouveau chantier → `design-flow` (orchestre tout) ou steps individuels :
  `grill-me` → `design-brief` → `information-architecture` → `brief-to-tasks` →
  `frontend-design` → `design-review`. JAMAIS `design-taste-frontend` /
  `redesign-existing-projects` / `gpt-taste` / `top-design` sur Sift.
- Exploration direction visuelle (prototype rapide) → `enhance-prompt` puis
  `stitch-generate-design` (génère HTML Stitch), puis porter en vanilla TS.
- Review post-implémentation → `design-review`.
- Refactor/legacy (ex: D3, split de sift-live.ts) → `working-with-legacy-code`,
  `refactoring-patterns`, `clean-code`, `software-design-philosophy`.
- Audit de dette → `tech-debt-audit` (manuel `/tech-debt-audit`).
- Planification d'une tâche non-triviale → `superpowers` (writing-plans, etc.) ou
  `feature-dev` (manuel `/feature-dev`) pour une feature précise avec questions
  de clarification.
Cette règle prime sur toute impulsion à agir directement sans passer par la skill
correspondante — même pour un changement qui semble petit.

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

## Audit des dépendances (versions à jour)

Vérifie que toutes les dépendances du projet sont à jour, sans rien
casser et sans update aveugle.

Méthode :
1. Lance `cargo outdated` pour lister précisément les crates en retard
   (compare versions actuelles du Cargo.toml vs dernières stables).
   Si l'outil n'est pas installé : `cargo install cargo-outdated`.
2. Pour CHAQUE crate en retard, classe l'écart :
   - patch/minor sans breaking → update sûr
   - bump majeur (ex: ureq 2.x→3.x, symphonia 0.5→0.6) → STOP,
     ne touche pas tout de suite, signale-le.
3. Pour tout bump majeur, récupère le changelog à jour via Context7
   ou le repo, et résume les breaking changes qui touchent réellement
     nos call sites — pas une liste générique.
4. Ne propose un `cargo update` que crate par crate, chirurgicalement.
   Jamais un update global d'un coup.

Versions en usage (migration majeure faite le 2026-07-01, build + 173 tests verts) :
tauri 2.11.3 · rusqlite 0.40.1 · symphonia 0.6.0 · rustfft 6.4.1 ·
lofty 0.24.0 · rusty-chromaprint 0.3.0 · notify-debouncer-full 0.7.0 · ureq 3.3.0
Ces versions sont désormais celles du `Cargo.toml` (plus des cibles à atteindre) —
elles servent de référence pour le prochain audit `cargo outdated`.

Versions JS en usage (migration TypeScript 6 + Vite 5→8 faite le 2026-07-01,
4 commits, tsc + build + tauri dev verts) :
typescript 6.0.3 · vite 8.1.2
Méthode : un palier majeur = un `npm i -D <pkg>@<major>` + Context7 (breaking
changes filtrés à notre config réelle) + validation build/dev + commit dédié.

Règles :
- fail-fast : si une crate ne build plus après update, ne mets pas de
  fallback ni de pin de contournement — remonte l'erreur exacte (fichier:ligne).
- surgical : un seul changement de version par étape, build + test entre chaque.
- ne mets jamais à jour une dep "parce qu'elle est en retard" sans que
  je valide le risque de migration d'abord.

## Documentation lookups (Context7)

Avant d'écrire ou de modifier du code touchant une librairie externe,
récupère sa doc à jour via Context7 — ne te fie jamais à la mémoire
d'entraînement pour une API, une signature, un nom de feature ou une
config de version.

Déclenche un lookup Context7 automatiquement, sans qu'on le demande, dès que :
- j'introduis ou je configure une librairie (Tauri v2, rusqlite, Symphonia,
  rustfft, lofty, rusty-chromaprint, ureq, Vite, ou toute crate/package Ableton/Max)
- je demande un setup, une config ou un exemple d'usage
- j'utilise une API dont la signature exacte ou le comportement de version compte
- une erreur de build vient d'un mauvais usage d'API plausiblement périmé

Méthode :
1. Si je n'ai pas donné l'ID, résous-le avec resolve-library-id à partir
   de ma question.
2. Quand je suis dans une tâche longue et que le contexte est déjà chargé,
   spawn l'agent docs-researcher au lieu d'appeler l'outil inline — il tourne
   en contexte séparé et renvoie juste la réponse, pour ne pas saturer.
3. Si le lookup échoue ou que la librairie n'est pas indexée, dis-le
   explicitement (fail-fast) — ne devine pas une API à partir de la mémoire.

IDs connus pour cette stack (à confirmer à la résolution, ne pas inventer) :
/tauri-apps/tauri · /rusqlite/rusqlite · /algesten/ureq

## Front — événements répétés
- Renderer déclenché par un événement en rafale (progress, watcher, scroll, resize) :
  **créer les nœuds une fois, muter ensuite**. Jamais d'`innerHTML =` dans un handler
  appelé en boucle (sature le thread UI → feedback noyé, bug invisible à la lecture).
- En écrivant un handler sur événement, **nommer la fréquence supposée** de l'événement,
  pour que le risque de saturation soit visible à la revue, pas découvert au runtime.
