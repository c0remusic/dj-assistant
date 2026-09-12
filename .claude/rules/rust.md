---
paths: ["src-tauri/**"]
---

# Audit Rust — override projet Sift

⚠️ **Ce fichier est devenu autonome, et c'est une perte.** Il était écrit pour se
charger via `~/.claude/agents/auditor.md` EN PLUS du module global
`~/.claude/rules/audit/rust.md` — les deux ont été supprimés par le reset vanilla
de `~/.claude` du 2026-07-31 (commit `6f6c132`, récupérables au tag
`pre-reset-vanilla`). Il ne porte QUE les spécificités de Sift qui précisent ou
relâchent un générique Rust **qui n'est plus chargé nulle part** : lu seul, il
sous-couvre. Le lire comme un complément, pas comme la règle Rust complète.
Source : contenu spécifique déplacé depuis l'ex-agent
`dj-assistant-m6a/.claude/agents/rust-engineer.md`.

Cible : backend Sift (`src-tauri/`, crate `sift_lib`) — app desktop Tauri v2.
Pas un projet de systems-programming, pas une lib publiée, pas un service async.

## Forme réelle du code (vérifiée, pas supposée)

- **Edition 2021** (`src-tauri/Cargo.toml:8`). Modules plats par domaine (voir
  `CLAUDE.md` racine pour la liste), pas un workspace multi-crate.
- **MSRV : NON MESURÉE.** `src-tauri/Cargo.toml:9` déclare
  `rust-version = "1.77.2"`, mais `rust-toolchain.toml` **à la racine du dépôt**
  (pas dans `src-tauri/`, où il n'existe pas — vérifié le 2026-07-31) épingle le
  canal `1.96.0` depuis le 2026-07-28 : plus personne ne compile à la version
  déclarée, et rien — ni la CI ni un job dédié — ne la vérifie. Ce fichier
  affirmait « MSRV 1.77.2 » sous un titre « vérifiée, pas supposée » ; c'était
  faux, et contradictoire avec le `CLAUDE.md` racine qui dit « à re-mesurer ».
  Ne pas traiter 1.77.2 comme une contrainte tant qu'un build réel ne l'a pas
  rétablie. Ce qui l'établirait, au choix : `cargo +1.77.2 check` sur un
  toolchain réellement installé, ou `cargo msrv find` (le crate `cargo-msrv`
  n'est pas installé sur cette machine — le vérifier avant de le prescrire).
  Le jour où le chiffre est rétabli, il se réécrit ICI et dans le `CLAUDE.md`
  racine dans le même geste, jamais dans un seul des deux.
- **Sync, aucun runtime async.** Ni `tokio` ni `async-std` dans l'arbre. La
  concurrence est un petit pool `std::thread::spawn` dimensionné sur
  `std::thread::available_parallelism()`, coordonné par `Arc<Mutex<...>> +
  Condvar` (`worker.rs`) — pas de channels, pas d'executor. Ne pas proposer de
  patterns async : ils ne s'appliquent pas ici. → Relâche la section
  « Concurrence » du module global (les items async/tokio ne s'appliquent pas).
- **Accès SQLite = `Mutex<Connection>` derrière un Tauri `State`.** Utiliser
  `db::lock_conn(&conn)` (extrait 2026-07, remplace ~40 sites de lock dupliqués)
  plutôt que `.lock().map_err(...)` à la main.
- **Pas de FFI au sens C/`bindgen`/`cbindgen`.** La seule frontière qui y
  ressemble est le sidecar FFmpeg (`ffmpeg-sidecar` spawne un binaire bundlé en
  sous-process) — c'est de l'I/O de process, pas un passage d'ABI. Le codebase a
  **5** blocs `unsafe`, recomptés le 2026-08-29 : deux dans `lib.rs`
  (`DwmExtendFrameIntoClientArea`, API Win32 de titlebar, commentaire `// SAFETY:`
  depuis 2026-07-17, commit `c94685c` ; `DwmSetWindowAttribute`, coins arrondis
  Win11, issue #41), et **trois dans
  `usb_format/raw_volume.rs`** (handles Win32 sur volume brut), également commentés.
  Ce fichier a longtemps annoncé « exactement un » sous un titre « vérifiée, pas
  supposée » — c'était faux. Un bloc touché préserve ou met à jour son commentaire,
  jamais le supprimer. Pas de surface `no_std`/embarqué/WASM — ne pas appliquer
  ces patterns.
- **Erreurs : enums à la main** (`Debug, Clone, PartialEq` + `Display` manuel +
  `impl std::error::Error`), ex. `MasterDbError` (`rekordbox_masterdb.rs`). À la
  frontière IPC Tauri, les commandes renvoient `Result<T, String>` via
  `.map_err(|e| e.to_string())`, pas un `Serialize` dérivé sur le type d'erreur.
  Le projet **n'utilise pas** `thiserror`/`anyhow` — ne pas les introduire sans
  le signaler d'abord. → Surcharge l'item global « `Box<dyn Error>` → thiserror » :
  ici l'absence de thiserror est un choix, pas un défaut.
- **`unwrap()`/`expect()` hors `#[cfg(test)]` = interdit dur** (méthode projet :
  fail fast, pas de fallback silencieux) — plus strict que le global (« documenter
  l'invariant et utiliser `expect` »). `Option<T>` seulement pour de l'optionnel
  réel, jamais comme échappatoire d'erreur.
- **Perf : pas de `criterion`.** Les quatre benchmarks (`bench_volume.rs`,
  `bench_dedup.rs`, `bench_sqlite.rs`, `bench_cpu_budget.rs`) sont des
  `#[cfg(test)] mod` lancés via `cargo test --release -- --ignored --nocapture` —
  suivre ce pattern plutôt que d'introduire criterion. `profile.dev` met déjà les
  dépendances à `opt-level = 3` pour les hot paths DSP ; bencher en `--release`.
- **Tests : `#[cfg(test)] mod tests` inline par module.** Pas de `proptest`,
  `mockall`, ni `cargo-fuzz` dans l'arbre — ne pas les introduire pour une seule
  tâche sans le signaler (décision de dépendance, pas un ajout au passage).

## Ownership (précisions projet)

- Types owned au repos (`String`, `Vec<T>`, `PathBuf`) ; emprunt en paramètre.
- `.clone()` = signal à revoir, pas un pansement — SAUF quand il évite de tenir
  un `MutexGuard` à travers une frontière IPC ou une section critique longue :
  là, cloner est souvent le choix correct. Juger par ce qu'il évite.
- App desktop, pas un parser zero-copy : données owned = souvent le bon compromis
  quand un lifetime devient compliqué.
- `Arc<Mutex<T>>` est le pattern établi pour l'état partagé cross-thread (pool ↔
  connexion DB ↔ commandes Tauri) — le suivre, pas introduire une autre primitive.

## Robustesse du worker pool (`worker.rs`, audit 2026-07-17, commit `c94685c`)

Le pool de threads d'analyse tourne en continu et sans supervision (pas de
`join()`, pas de channel de contrôle) — deux modes de dégradation silencieuse
identifiés et corrigés, à respecter dans tout nouveau code sur ce chemin :

- **`Mutex<Connection>` empoisonné : logger avant de bailer, jamais un retour
  muet.** `refill`/`read_path`/`persist_result` faisaient `let Ok(x) = ...
  else { return }` sur un lock potentiellement empoisonné — violation directe
  du principe fail-fast/pas-de-fallback-silencieux (un panic ailleurs pendant
  un lock rendrait tout le pool aveugle sans aucune trace). Pattern à suivre :
  `match state.lock() { Ok(x) => x, Err(_) => { log::error!("..."); return
  None; } }`, jamais juste `.ok()?`/`else { return }` nu sur un `Mutex` partagé
  avec un pool de threads.
- **Travail lourd sur fichier utilisateur arbitraire dans un thread de pool :
  envelopper dans `std::panic::catch_unwind`.** `analysis::analyze()` décode
  des fichiers audio venant du disque utilisateur (surface d'entrée non
  maîtrisée — Symphonia/FFT sur fichier corrompu peut paniquer). Sans
  `catch_unwind`, un panic tue le thread `spawn`é pour toujours (jamais
  relancé, jamais loggé) : le pool rétrécit thread par thread jusqu'à 0 en
  silence. Pattern établi dans `worker_loop` :
  `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work()))
  .unwrap_or_else(|payload| { log::error!(...); Err(...) })` — traite le panic
  comme un `Err` normal (repris au refill suivant), pas comme une fin de
  thread. À reproduire pour toute future tâche lourde ajoutée dans un
  `worker_loop`-like tournant sur de l'I/O utilisateur non maîtrisé.

## Fan-out d'agents sur du Rust (incident 2026-07-04/05, migré depuis l'ex-registre)

**Jamais deux agents `cargo`/`tauri dev` concurrents sur ce repo** — le cache
incrémental Rust ne supporte pas les builds concurrents (LNK2019 observé,
mémoire `avoid-concurrent-cargo-tauri-dev`). Toujours arrêter `tauri dev` avant
un fan-out qui touche `src-tauri/`. Cargo sérialise via son lock (deux agents
peuvent compiler l'un après l'autre sans casser), mais donner à chaque agent
Rust parallèle un périmètre de fichiers strictement disjoint reste nécessaire
(cf. gabarit `Exécuteur borné` de la skill `sizing-templates`).

## Avant de dire « fini » (commandes exactes du projet)

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

- Tout bloc `unsafe` neuf ou touché a un `// SAFETY:` expliquant l'invariant.
- Aucune dépendance `thiserror`/`anyhow`/`tokio`/`criterion`/`proptest`
  introduite en douce — ce sont des décisions projet, à remonter, pas à ajouter
  dans une tâche.
- Si `cargo test` échoue en `LNK2001`/`LNK2019` (« symbole externe non résolu
  anon.<hash>.llvm.<hash> ») sur des symboles sans rapport avec le diff en
  cours (`tauri_runtime_wry`, `PathResolver`, `menu plugin`...), et que
  `cargo clippy` reste vert : c'est un cache incrémental Windows corrompu
  (typiquement après un `tauri dev` interrompu/tué en cours de build), pas un
  bug de code. Fix : `cargo clean --manifest-path src-tauri/Cargo.toml -p sift`
  (le nom du package — pas `sift_lib`), pas un `cargo clean` complet. Ne pas
  chercher le bug dans le diff avant d'avoir écarté cette cause.
