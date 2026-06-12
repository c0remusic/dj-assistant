# M4-3a — Migration v4 + settings + library (bins) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The persistence + folder groundwork for filing: one append-only migration adding every column M4 needs, a key/value `settings` store (library root, filename template, trash purge days), and a `library` module that lists the recursive bins under the configured root and creates new ones.

**Architecture:** Append migration v4 to the existing ordered `MIGRATIONS` array in `db.rs` (tracked by `PRAGMA user_version`, never reordered). New `settings.rs` is a thin typed wrapper over a `settings(key, value)` table. New `library.rs` walks the library-root directory tree with `walkdir` (already a dependency, used by `scanner.rs`) to enumerate bins, and creates/sanitizes new bin folders. All testable with an in-memory DB (`settings`, migration) and a `tempfile::tempdir()` (`library`); no ffmpeg, no app handle.

**Tech Stack:** Rust (edition 2021, MSRV 1.77.2), `rusqlite` 0.32, `walkdir` 2.5, `serde`, `tempfile` (dev-dep). No new dependencies.

**This is plan 3a of 4 for M4.** M4-3b (`actions` undo engine) and M4-3c (`filing` + IPC) build on this. The columns added here (`tracks.target_format`, `tracks.confidence`, `metadata.version`, `actions.undone`, `actions.batch_id`) are consumed by 3b/3c.

---

## Conventions for this plan

- cargo not on PATH: `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` (PowerShell), run from `src-tauri/`.
- `cargo test --lib db`, `--lib settings`, `--lib library` per module. Avoids linking `sift.exe`.
- Tests inline `#[cfg(test)] mod tests`. DB tests use `Connection::open_in_memory()`; library tests use `tempfile::tempdir()`.
- New modules `settings.rs` and `library.rs` carry `#![allow(dead_code)]` (consumed by 3b/3c).
- Commit after each task. No `Co-Authored-By` trailer.

---

## File structure

- Modify: `src-tauri/src/db.rs` — append the v4 migration string to `MIGRATIONS`; add two column-presence tests.
- Create: `src-tauri/src/settings.rs` — `get`, `set`, key constants, template default.
- Create: `src-tauri/src/library.rs` — `Bin`, `list_bins`, `create_bin`, `ensure_unique`.
- Modify: `src-tauri/src/lib.rs` — add `mod settings;` and `mod library;`.

---

## Task 1: Migration v4 (all M4 columns + settings table)

**Files:**
- Modify: `src-tauri/src/db.rs`

`MIGRATIONS` is an ordered array; index+1 is the version it reaches. Append one entry. Per
the spec: `tracks.target_format`, `tracks.confidence`, `metadata.version`,
`actions.undone`, `actions.batch_id`, and a `settings` table.

- [ ] **Step 1: Append the v4 migration**

In `src-tauri/src/db.rs`, add this entry to the `MIGRATIONS` array immediately after the
v3 entry (the one ending with `idx_tracks_analyzed`), before the closing `];`:

```rust
    // v4 — M4 filing loop: per-track target/confidence, version metadata, undo bookkeeping
    // on actions, and a key/value settings store (library root, filename template, purge).
    r#"
    ALTER TABLE tracks ADD COLUMN target_format TEXT;     -- 'mp3_320' | 'aiff_16_44'
    ALTER TABLE tracks ADD COLUMN confidence TEXT;        -- 'green' | 'yellow'
    ALTER TABLE metadata ADD COLUMN version TEXT;         -- 'Original Mix', 'Remix'…
    ALTER TABLE actions ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;  -- 0/1
    ALTER TABLE actions ADD COLUMN batch_id TEXT;         -- groups one filing's rows
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
```

- [ ] **Step 2: Add column/table presence tests**

In `db.rs`'s `mod tests`, add (the existing tests use this exact `pragma_table_info`
pattern — mirror it):

```rust
    #[test]
    fn tracks_has_m4_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["target_format", "confidence"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn actions_and_settings_have_m4_shape() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["undone", "batch_id"] {
            assert!(acols.contains(&c.to_string()), "actions missing column {c}");
        }
        // settings table exists and is writable
        conn.execute("INSERT INTO settings(key,value) VALUES('k','v')", [])
            .expect("settings table usable");
    }
```

Also update the existing `migrations_create_all_tables` test: it asserts 5 tables; v4 adds
`settings` → change the expected count to **6**. Find:

```rust
        assert_eq!(table_count(&conn).unwrap(), 5);
```

There are two such assertions (`migrations_create_all_tables` and `migrations_are_idempotent`). Change **both** to `6`.

- [ ] **Step 3: Run the DB tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib db`
Expected: PASS — `schema_version == 4`, all column/table tests green, table count 6.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat(m4-3a): migration v4 — M4 columns + settings table"
```

---

## Task 2: `settings` module (key/value store)

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add `mod settings;` after `mod scanner;`:

```rust
mod scanner;
mod settings;
mod sources;
```

- [ ] **Step 2: Create `settings.rs` with get/set + tests**

```rust
//! Typed access to the `settings(key, value)` table: the few app-wide preferences the
//! filing loop needs (library root, filename template, trash purge window). String values
//! only; callers parse as needed. Created in migration v4.
#![allow(dead_code)]

use rusqlite::{params, Connection};

/// Absolute path of the library root under which bins live.
pub const LIBRARY_ROOT: &str = "library_root";
/// Output filename template (placeholders {artist} {title} {version}).
pub const FILENAME_TEMPLATE: &str = "filename_template";
/// Days a trashed file is kept in `.sift-trash` before purge.
pub const TRASH_PURGE_DAYS: &str = "trash_purge_days";

/// The default filename template when the setting is unset.
pub const DEFAULT_TEMPLATE: &str = "{artist} - {title}{version}";

/// Read a setting, or None if unset.
pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key=?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// Read a setting or fall back to `default`.
pub fn get_or(conn: &Connection, key: &str, default: &str) -> rusqlite::Result<String> {
    Ok(get(conn, key)?.unwrap_or_else(|| default.to_string()))
}

/// Upsert a setting.
pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn get_missing_is_none() {
        let conn = db();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), None);
    }

    #[test]
    fn set_then_get_round_trips() {
        let conn = db();
        set(&conn, LIBRARY_ROOT, "/music/dj").unwrap();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), Some("/music/dj".to_string()));
    }

    #[test]
    fn set_overwrites() {
        let conn = db();
        set(&conn, LIBRARY_ROOT, "/a").unwrap();
        set(&conn, LIBRARY_ROOT, "/b").unwrap();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), Some("/b".to_string()));
    }

    #[test]
    fn get_or_falls_back() {
        let conn = db();
        assert_eq!(get_or(&conn, FILENAME_TEMPLATE, DEFAULT_TEMPLATE).unwrap(), DEFAULT_TEMPLATE);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib settings`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/lib.rs
git commit -m "feat(m4-3a): settings key/value store"
```

---

## Task 3: `library` module — list bins (recursive)

**Files:**
- Create: `src-tauri/src/library.rs`
- Modify: `src-tauri/src/lib.rs`

A bin is any subdirectory under the library root (recursive). Hidden directories (leading
`.`, e.g. `.sift-trash`) are excluded. Returns them sorted by path, with a portable
forward-slash relative path, a display name (last component), and depth (for UI
indentation).

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add `mod library;` after `mod ipc;`:

```rust
mod ipc;
mod library;
mod naming;
```

- [ ] **Step 2: Create `library.rs` with `Bin` + `list_bins` + tests**

```rust
//! The destination bins: every subdirectory (recursive) under the configured library
//! root. Walks the tree with `walkdir`, skipping hidden dirs (e.g. the `.sift-trash`
//! corbeille). Also creates new bins and resolves collision-free destination paths. Pure
//! filesystem work; the root path comes from `settings::LIBRARY_ROOT`.
#![allow(dead_code)]

use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// One destination folder under the library root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bin {
    /// Path relative to the root, forward-slash separated (e.g. "House/Deep").
    pub rel: String,
    /// Display name = last path component (e.g. "Deep").
    pub name: String,
    /// Nesting depth under root (1 = direct child).
    pub depth: usize,
}

/// Whether a directory name is hidden (leading dot) — excluded from bins.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// List all bins (recursive subdirectories) under `root`, sorted by relative path. Returns
/// an empty list if root doesn't exist. Hidden directories and their subtrees are skipped.
pub fn list_bins(root: &Path) -> Vec<Bin> {
    let mut bins = Vec::new();
    let walker = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            // skip hidden dirs entirely (prunes their subtree too)
            !e.file_name().to_str().map(is_hidden).unwrap_or(false)
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel = rel_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if rel.is_empty() {
            continue;
        }
        let name = entry
            .file_name()
            .to_str()
            .unwrap_or_default()
            .to_string();
        let depth = entry.depth();
        bins.push(Bin { rel, name, depth });
    }
    bins.sort_by(|a, b| a.rel.cmp(&b.rel));
    bins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_recursive_bins_sorted_skipping_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House/Deep")).unwrap();
        std::fs::create_dir_all(root.join("House/Acid")).unwrap();
        std::fs::create_dir_all(root.join("Techno")).unwrap();
        std::fs::create_dir_all(root.join(".sift-trash/42")).unwrap();

        let bins = list_bins(root);
        let rels: Vec<&str> = bins.iter().map(|b| b.rel.as_str()).collect();
        assert_eq!(rels, vec!["House", "House/Acid", "House/Deep", "Techno"]);
        // hidden subtree excluded
        assert!(!rels.iter().any(|r| r.contains("sift-trash")));
        // depth + name sane
        let deep = bins.iter().find(|b| b.rel == "House/Deep").unwrap();
        assert_eq!(deep.name, "Deep");
        assert_eq!(deep.depth, 2);
    }

    #[test]
    fn missing_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nope");
        assert!(list_bins(&root).is_empty());
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib library`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/library.rs src-tauri/src/lib.rs
git commit -m "feat(m4-3a): library bins listing (recursive, skips hidden)"
```

---

## Task 4: `library` — create bin + collision-free destination

**Files:**
- Modify: `src-tauri/src/library.rs`

`create_bin` makes a new subfolder under `root/parent_rel`, sanitizing the name with the
`naming::sanitize` helper (reuse — don't duplicate). `ensure_unique` returns a destination
path that doesn't collide, appending ` (2)`, ` (3)`… before the extension when needed
(filing uses this to never overwrite).

- [ ] **Step 1: Write the failing tests**

Add to `library.rs`'s `mod tests`:

```rust
    #[test]
    fn create_bin_makes_sanitized_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House")).unwrap();

        let bin = create_bin(root, "House", "Deep/Soulful?").unwrap();
        assert_eq!(bin.rel, "House/Deep Soulful"); // "/" and "?" sanitized to spaces→collapsed
        assert!(root.join("House/Deep Soulful").is_dir());
    }

    #[test]
    fn create_bin_at_root_level() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bin = create_bin(root, "", "Disco").unwrap();
        assert_eq!(bin.rel, "Disco");
        assert_eq!(bin.depth, 1);
        assert!(root.join("Disco").is_dir());
    }

    #[test]
    fn ensure_unique_appends_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.mp3");
        // free → unchanged
        assert_eq!(ensure_unique(&base), base);
        // occupied → " (2)"
        std::fs::write(&base, b"x").unwrap();
        assert_eq!(ensure_unique(&base), dir.path().join("Track (2).mp3"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib library`
Expected: FAIL — `cannot find function create_bin` / `ensure_unique`.

- [ ] **Step 3: Implement `create_bin` + `ensure_unique`**

Add above the test module in `library.rs`:

```rust
/// Create a new bin folder named `name` (sanitized) under `root/parent_rel`. `parent_rel`
/// "" means directly under root. Returns the created Bin. Errors as a string on mkdir
/// failure.
pub fn create_bin(root: &Path, parent_rel: &str, name: &str) -> Result<Bin, String> {
    let safe = crate::naming::sanitize(name);
    if safe.is_empty() {
        return Err("empty bin name".into());
    }
    let rel = if parent_rel.is_empty() {
        safe.clone()
    } else {
        format!("{}/{}", parent_rel.trim_end_matches('/'), safe)
    };
    let abs = root.join(&rel);
    std::fs::create_dir_all(&abs).map_err(|e| format!("create bin: {e}"))?;
    let depth = rel.split('/').count();
    Ok(Bin { rel, name: safe, depth })
}

/// Return a path that does not already exist, appending " (N)" before the extension when
/// the given path is taken. Used so filing never overwrites an existing file.
pub fn ensure_unique(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 2..10_000 {
        let candidate = match ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    // pathological fallback: timestamped name
    parent.join(format!("{stem} ({}).bak", std::process::id()))
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib library`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/library.rs
git commit -m "feat(m4-3a): create_bin + collision-free ensure_unique"
```

---

## Task 5: Full green + clippy

**Files:**
- None (verification only).

- [ ] **Step 1: Run all three modules' tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib db; & "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib settings; & "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib library`
Expected: all green.

- [ ] **Step 2: Clippy the new modules**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy --lib`
Expected: no warnings referencing `settings.rs` or `library.rs` (fix any that are; the
pre-existing `analysis/`/`worker.rs` warnings are out of scope).

- [ ] **Step 3: Commit any lint fixes**

```bash
git add src-tauri/src/settings.rs src-tauri/src/library.rs
git commit -m "chore(m4-3a): clippy clean for settings + library"
```

(Skip if already clean.)

---

## Done criteria

- Migration v4 applied; `schema_version == 4`; tracks/actions/metadata have the M4 columns;
  `settings` table present (6 tables total).
- `settings::{get, set, get_or}` + key constants + `DEFAULT_TEMPLATE` work.
- `library::{Bin, list_bins, create_bin, ensure_unique}` work against a tempdir.
- No ffmpeg or app-handle dependencies; everything tested in-memory / in tempdir.

**Next plan:** M4-3b — `actions` undo engine (`record`, `revert` with guards, `undo_last`
LIFO, `list_journal`).
