# M1 — Watcher + file « à traiter » — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surveiller des dossiers (Soulseek `Completed`, etc.) et remplir automatiquement la file « à traiter » (`tracks WHERE status='pending'`), visible en live dans l'UI — sans analyse ni rangement (M2+).

**Architecture:** Backend Rust découpé en modules à responsabilité unique (`scanner` = logique pure de diff, `sources`/`queue` = accès DB, `watcher` = live via `notify`), exposés au front par des commands Tauri typées. Le front (vanilla JS existant) reçoit des events `queue:changed` et re-rend les sections concernées en mode Tauri ; en navigateur (démo Vercel) il garde sa maquette.

**Tech Stack:** Rust + Tauri 2, `rusqlite` (bundled), `walkdir`, `notify-debouncer-full`, `tauri-plugin-dialog`, TypeScript (contrats), JS vanilla (UI).

**Spec de référence :** [docs/superpowers/specs/2026-06-12-m1-watcher-queue-design.md](../specs/2026-06-12-m1-watcher-queue-design.md)

**Conventions du repo (M0) :**
- Migrations : append-only dans `MIGRATIONS` (`src-tauri/src/db.rs`), versionnées par `PRAGMA user_version`. NE JAMAIS éditer une migration livrée.
- Commands : structs `#[derive(Serialize)]` dans `ipc.rs`, miroir TS dans `shared/contracts.ts`, helper d'appel dans `frontend/ipc.ts`.
- Tests Rust : `cargo test --manifest-path src-tauri/Cargo.toml`.
- Type-check front : `npx tsc --noEmit`.
- Pas de trailer Co-Authored-By sur les commits.

---

## File Structure

**Créés :**
- `src-tauri/src/scanner.rs` — filtre audio, walk disque, réconciliation (logique pure, cœur testé).
- `src-tauri/src/sources.rs` — CRUD des dossiers surveillés + comptage `pending`.
- `src-tauri/src/queue.rs` — lecture de la file (`pending`).
- `src-tauri/src/watcher.rs` — surveillance live (`notify-debouncer-full`), état des watchers.
- `frontend/sift-live.ts` — wiring live (Tauri only) : sources + file + bouton « ajouter un dossier ».

**Modifiés :**
- `src-tauri/src/db.rs` — migration v2 (colonnes `tracks` + `sources.created_at`).
- `src-tauri/src/ipc.rs` — commands `add_source`, `list_sources`, `remove_source`, `list_queue`, `rescan_source`.
- `src-tauri/src/lib.rs` — déclarer les modules, enregistrer commands + plugin dialog, démarrer les watchers au setup.
- `src-tauri/Cargo.toml` — deps `walkdir`, `notify-debouncer-full`, `tauri-plugin-dialog` ; dev-dep `tempfile`.
- `src-tauri/capabilities/default.json` — permission `dialog:default`.
- `shared/contracts.ts` — interfaces `Source`, `QueueItem`.
- `frontend/ipc.ts` — helpers d'appel + abonnement event.
- `frontend/main.ts` — importer `sift-live` en mode Tauri.
- `frontend/app.js` — 2 hooks de re-render (`window.__siftHome`, `window.__siftQueue`).
- `package.json` — dep `@tauri-apps/plugin-dialog`.

---

## Task 1: Migration v2 — colonnes de la file

**Files:**
- Modify: `src-tauri/src/db.rs` (const `MIGRATIONS`, +tests)

- [ ] **Step 1: Write the failing tests**

Ajouter dans le module `tests` de `src-tauri/src/db.rs` :

```rust
    #[test]
    fn migrations_reach_v2() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
        assert!(MIGRATIONS.len() >= 2, "M1 adds migration v2");
    }

    #[test]
    fn tracks_has_m1_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["source_id", "filename", "size_bytes", "mtime"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::`
Expected: FAIL (`migrations_reach_v2` panics on `>= 2`; `tracks_has_m1_columns` missing columns).

- [ ] **Step 3: Append migration v2**

Dans `src-tauri/src/db.rs`, ajouter une entrée à la fin du tableau `MIGRATIONS` (après la v1, virgule comprise). SQLite n'autorise pas de `DEFAULT` non-constant sur `ADD COLUMN` → `created_at` est nullable (rempli à l'INSERT) :

```rust
    // v2 — M1 watcher/queue: link tracks to a source + cheap "seen" identity (size+mtime)
    r#"
    ALTER TABLE tracks ADD COLUMN source_id INTEGER REFERENCES sources(id) ON DELETE CASCADE;
    ALTER TABLE tracks ADD COLUMN filename TEXT;
    ALTER TABLE tracks ADD COLUMN size_bytes INTEGER;
    ALTER TABLE tracks ADD COLUMN mtime INTEGER;
    ALTER TABLE sources ADD COLUMN created_at TEXT;
    CREATE INDEX idx_tracks_source ON tracks(source_id);
    CREATE INDEX idx_tracks_status ON tracks(status);
    "#,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::`
Expected: PASS (all `db::` tests, incl. existing `migrations_create_all_tables` — still 5 tables, ALTER adds no table).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat(m1): db migration v2 — tracks.source_id/filename/size/mtime + indexes"
```

---

## Task 2: Filtre d'extensions audio

**Files:**
- Create: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/src/lib.rs` (déclarer `mod scanner;`)

- [ ] **Step 1: Create the module with a failing test**

Créer `src-tauri/src/scanner.rs` :

```rust
//! Disk scanning + reconciliation. Pure-ish logic: given a folder and the DB,
//! computes which audio files to add / update / drop from the queue.
use std::path::Path;

/// Audio extensions Sift queues. Everything else on disk is ignored.
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "aif", "aiff", "m4a", "aac", "ogg", "opus"];

/// True if `path` has a recognised audio extension (case-insensitive).
pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn audio_extensions_are_recognised() {
        assert!(is_audio(Path::new("a/b/track.mp3")));
        assert!(is_audio(Path::new("track.FLAC"))); // case-insensitive
        assert!(is_audio(Path::new("x.aiff")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("notes.txt")));
        assert!(!is_audio(Path::new("no_extension")));
    }
}
```

Déclarer le module dans `src-tauri/src/lib.rs` (à côté des autres `mod` en haut) :

```rust
mod db;
mod ffmpeg;
mod ipc;
mod scanner;
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scanner::tests::audio_extensions_are_recognised`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scanner.rs src-tauri/src/lib.rs
git commit -m "feat(m1): scanner audio-extension filter"
```

---

## Task 3: Scan disque + réconciliation (cœur)

**Files:**
- Modify: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/Cargo.toml` (dep `walkdir`, dev-dep `tempfile`)

- [ ] **Step 1: Add dependencies**

```bash
cargo add --manifest-path src-tauri/Cargo.toml walkdir
cargo add --manifest-path src-tauri/Cargo.toml --dev tempfile
```

- [ ] **Step 2: Write the failing tests**

Remplacer le bloc `#[cfg(test)] mod tests` de `scanner.rs` par (garde le test d'extensions, ajoute le reste) :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::path::Path;

    #[test]
    fn audio_extensions_are_recognised() {
        assert!(is_audio(Path::new("a/b/track.mp3")));
        assert!(is_audio(Path::new("track.FLAC")));
        assert!(is_audio(Path::new("x.aiff")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("notes.txt")));
        assert!(!is_audio(Path::new("no_extension")));
    }

    /// In-memory DB with the live schema + one source row to attach tracks to.
    fn db_with_source() -> (Connection, i64) {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO sources (path) VALUES ('root')", []).unwrap();
        (conn, conn.last_insert_rowid())
    }

    fn pending_count(conn: &Connection, source_id: i64) -> i64 {
        conn.query_row(
            "SELECT count(*) FROM tracks WHERE source_id=?1 AND status='pending'",
            [source_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn scan_dir_finds_audio_recursively_and_ignores_non_audio() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("album")).unwrap();
        fs::write(root.join("a.mp3"), b"x").unwrap();
        fs::write(root.join("album/b.flac"), b"yy").unwrap();
        fs::write(root.join("album/cover.jpg"), b"img").unwrap();

        let mut found: Vec<String> = scan_dir(root).into_iter().map(|f| f.filename).collect();
        found.sort();
        assert_eq!(found, vec!["a.mp3".to_string(), "b.flac".to_string()]);
    }

    #[test]
    fn reconcile_adds_updates_and_removes() {
        let (conn, sid) = db_with_source();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("keep.mp3"), b"123").unwrap();
        fs::write(root.join("change.wav"), b"123").unwrap();

        // First pass: both files are new.
        let s1 = reconcile(&conn, sid, root).unwrap();
        assert_eq!(s1.added, 2);
        assert_eq!(pending_count(&conn, sid), 2);

        // Mark them filed so we can prove "unchanged" does NOT reset status.
        conn.execute("UPDATE tracks SET status='filed'", []).unwrap();

        // Change one file's size, delete the other, add a third.
        fs::write(root.join("change.wav"), b"123456789").unwrap();
        fs::remove_file(root.join("keep.mp3")).unwrap();
        fs::write(root.join("new.aiff"), b"z").unwrap();

        let s2 = reconcile(&conn, sid, root).unwrap();
        assert_eq!(s2.added, 1, "new.aiff");
        assert_eq!(s2.updated, 1, "change.wav size differs → re-pending");
        // keep.mp3 gone but it was 'filed' (not pending) → NOT removed by reconcile.
        assert_eq!(s2.removed, 0);

        // change.wav is back to pending; new.aiff pending; keep.mp3 still filed.
        let status: String = conn
            .query_row("SELECT status FROM tracks WHERE filename='change.wav'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn reconcile_drops_pending_files_that_vanished() {
        let (conn, sid) = db_with_source();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("gone.mp3"), b"1").unwrap();
        reconcile(&conn, sid, root).unwrap();
        assert_eq!(pending_count(&conn, sid), 1);

        fs::remove_file(root.join("gone.mp3")).unwrap();
        let s = reconcile(&conn, sid, root).unwrap();
        assert_eq!(s.removed, 1);
        assert_eq!(pending_count(&conn, sid), 0);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scanner::`
Expected: FAIL to COMPILE (`scan_dir`, `reconcile`, `DiskFile`, `ReconcileStats` undefined).

- [ ] **Step 4: Implement scan + reconcile**

Dans `src-tauri/src/scanner.rs`, sous `is_audio`, ajouter :

```rust
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// One audio file found on disk. `path` is the absolute path string (the DB identity key).
pub struct DiskFile {
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub mtime: i64,
}

/// What a reconciliation pass changed.
#[derive(Debug, Default, PartialEq)]
pub struct ReconcileStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Walks `root` recursively (no symlink-following) and returns every audio file.
/// Unreadable entries are skipped, never fatal. `root` is expected to be absolute
/// (callers canonicalise it once when the source is added) so paths stay consistent
/// with the ones `notify` reports for the live watcher.
pub fn scan_dir(root: &Path) -> Vec<DiskFile> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_audio(e.path()))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(DiskFile {
                path: e.path().to_string_lossy().into_owned(),
                filename: e.file_name().to_string_lossy().into_owned(),
                size_bytes: meta.len() as i64,
                mtime: mtime_secs(&meta),
            })
        })
        .collect()
}

/// Inserts a file as `pending`, or updates it. Status is reset to `pending` ONLY if
/// size or mtime changed (an unchanged re-scan must not disturb an already-filed track).
/// Returns true if a NEW row was inserted.
pub fn upsert_file(conn: &Connection, source_id: i64, f: &DiskFile) -> rusqlite::Result<bool> {
    let existing: Option<(i64, i64)> = conn
        .query_row(
            "SELECT size_bytes, mtime FROM tracks WHERE path=?1",
            rusqlite::params![f.path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match existing {
        None => {
            conn.execute(
                "INSERT INTO tracks (path, filename, size_bytes, mtime, source_id, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', datetime('now'))",
                rusqlite::params![f.path, f.filename, f.size_bytes, f.mtime, source_id],
            )?;
            Ok(true)
        }
        Some((size, mtime)) if size == f.size_bytes && mtime == f.mtime => Ok(false),
        Some(_) => {
            conn.execute(
                "UPDATE tracks SET filename=?2, size_bytes=?3, mtime=?4, source_id=?5, status='pending'
                 WHERE path=?1",
                rusqlite::params![f.path, f.filename, f.size_bytes, f.mtime, source_id],
            )?;
            Ok(false)
        }
    }
}

/// Removes a single file from the queue if (and only if) its row is still `pending`.
/// Returns rows affected. Used by the live watcher on delete events.
pub fn forget_path(conn: &Connection, path: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM tracks WHERE path=?1 AND status='pending'",
        rusqlite::params![path],
    )
}

/// Full diff of a source folder against the DB: add new files, re-pending changed ones,
/// drop pending rows whose file vanished. Non-pending rows (e.g. already filed) are left
/// untouched even if missing from disk.
pub fn reconcile(conn: &Connection, source_id: i64, root: &Path) -> rusqlite::Result<ReconcileStats> {
    let disk = scan_dir(root);

    let mut existing: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut stmt =
            conn.prepare("SELECT path, size_bytes, mtime FROM tracks WHERE source_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![source_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1).unwrap_or(0), r.get::<_, i64>(2).unwrap_or(0)),
            ))
        })?;
        for row in rows {
            let (p, sm) = row?;
            existing.insert(p, sm);
        }
    }

    let mut stats = ReconcileStats::default();
    let mut seen: HashSet<String> = HashSet::new();
    for f in &disk {
        seen.insert(f.path.clone());
        match existing.get(&f.path) {
            None => {
                upsert_file(conn, source_id, f)?;
                stats.added += 1;
            }
            Some(&(s, m)) if s == f.size_bytes && m == f.mtime => {}
            Some(_) => {
                upsert_file(conn, source_id, f)?;
                stats.updated += 1;
            }
        }
    }

    for path in existing.keys() {
        if !seen.contains(path) {
            stats.removed += forget_path(conn, path)?;
        }
    }
    Ok(stats)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scanner::`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/scanner.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(m1): scanner scan_dir + reconcile (add/update/remove) with tests"
```

---

## Task 4: Module `sources` (CRUD + comptage)

**Files:**
- Create: `src-tauri/src/sources.rs`
- Modify: `src-tauri/src/lib.rs` (`mod sources;`)

- [ ] **Step 1: Create module with failing tests**

Créer `src-tauri/src/sources.rs` :

```rust
//! Watched-folder records. The queue counts hang off these.
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;

/// A watched folder as shown on the Accueil screen.
#[derive(Debug, Serialize, PartialEq)]
pub struct Source {
    pub id: i64,
    pub path: String,
    pub pending_count: i64,
    pub accessible: bool,
}

/// Canonicalises `path` (so disk-scan and live-watch keys stay consistent), inserts it,
/// and returns the new source id. If the path is already a source, returns the existing id.
pub fn add(conn: &Connection, path: &str) -> rusqlite::Result<i64> {
    let canon = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    conn.execute(
        "INSERT INTO sources (path, watched, created_at) VALUES (?1, 1, datetime('now'))
         ON CONFLICT(path) DO NOTHING",
        rusqlite::params![canon],
    )?;
    conn.query_row(
        "SELECT id FROM sources WHERE path=?1",
        rusqlite::params![canon],
        |r| r.get(0),
    )
}

/// All sources with their live pending count and whether the folder still exists on disk.
pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Source>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.path,
                (SELECT count(*) FROM tracks t WHERE t.source_id=s.id AND t.status='pending')
         FROM sources s ORDER BY s.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let path: String = r.get(1)?;
        let accessible = Path::new(&path).is_dir();
        Ok(Source {
            id: r.get(0)?,
            path,
            pending_count: r.get(2)?,
            accessible,
        })
    })?;
    rows.collect()
}

/// Removes a source. Its tracks cascade-delete (FK ON DELETE CASCADE); in M1 those are all
/// `pending`, so the queue is cleaned of items from a folder we no longer watch.
pub fn remove(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sources WHERE id=?1", rusqlite::params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn add_is_idempotent_on_same_path() {
        let conn = db();
        let id1 = add(&conn, ".").unwrap();
        let id2 = add(&conn, ".").unwrap();
        assert_eq!(id1, id2);
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn list_reports_pending_count() {
        let conn = db();
        let id = add(&conn, ".").unwrap();
        conn.execute(
            "INSERT INTO tracks (path, source_id, status) VALUES ('p/x.mp3', ?1, 'pending')",
            rusqlite::params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (path, source_id, status) VALUES ('p/y.mp3', ?1, 'filed')",
            rusqlite::params![id],
        )
        .unwrap();
        let sources = list(&conn).unwrap();
        assert_eq!(sources[0].pending_count, 1); // only the pending one
    }

    #[test]
    fn remove_cascades_tracks() {
        let conn = db();
        let id = add(&conn, ".").unwrap();
        conn.execute(
            "INSERT INTO tracks (path, source_id, status) VALUES ('p/x.mp3', ?1, 'pending')",
            rusqlite::params![id],
        )
        .unwrap();
        remove(&conn, id).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 0);
        let n: i64 = conn
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "tracks cascade-deleted with the source");
    }
}
```

Déclarer dans `src-tauri/src/lib.rs` : ajouter `mod sources;` à la liste des modules.

> Note: `remove_cascades_tracks` repose sur `PRAGMA foreign_keys = ON`. `db::open` l'active, mais `open_in_memory` dans les tests ne passe pas par `open`. Le test l'active explicitement à l'étape suivante si besoin — voir Step 3.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sources::`
Expected: FAIL — `remove_cascades_tracks` échoue (FK off en mémoire, tracks pas supprimés).

- [ ] **Step 3: Enable foreign keys in the test DB**

Dans `sources.rs` tests, modifier `fn db()` pour activer les FK comme le fait `db::open` :

```rust
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sources::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sources.rs src-tauri/src/lib.rs
git commit -m "feat(m1): sources module — add/list/remove with pending counts"
```

---

## Task 5: Module `queue` (lecture de la file)

**Files:**
- Create: `src-tauri/src/queue.rs`
- Modify: `src-tauri/src/lib.rs` (`mod queue;`)

- [ ] **Step 1: Create module with failing test**

Créer `src-tauri/src/queue.rs` :

```rust
//! Read model for the "to process" queue = tracks WHERE status='pending'.
use rusqlite::Connection;
use serde::Serialize;

/// One row in the live queue. No analysis fields yet (M2+).
#[derive(Debug, Serialize, PartialEq)]
pub struct QueueItem {
    pub id: i64,
    pub path: String,
    pub filename: Option<String>,
    pub source_id: Option<i64>,
}

/// All pending tracks, oldest first.
pub fn list_pending(conn: &Connection) -> rusqlite::Result<Vec<QueueItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, filename, source_id FROM tracks
         WHERE status='pending' ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(QueueItem {
            id: r.get(0)?,
            path: r.get(1)?,
            filename: r.get(2)?,
            source_id: r.get(3)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn list_pending_returns_only_pending() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks (path, filename, status) VALUES ('a.mp3','a.mp3','pending')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (path, filename, status) VALUES ('b.mp3','b.mp3','filed')",
            [],
        )
        .unwrap();
        let q = list_pending(&conn).unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].filename, Some("a.mp3".to_string()));
    }
}
```

Déclarer `mod queue;` dans `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml queue::`
Expected: PASS once compiled (module + test added together; if it fails to compile, fix typos).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/queue.rs src-tauri/src/lib.rs
git commit -m "feat(m1): queue read model — list_pending"
```

---

## Task 6: Commands IPC + plugin dialog

**Files:**
- Modify: `src-tauri/src/ipc.rs` (commands)
- Modify: `src-tauri/src/lib.rs` (register commands + plugin)
- Modify: `src-tauri/Cargo.toml` (`tauri-plugin-dialog`)
- Modify: `src-tauri/capabilities/default.json` (permission)
- Modify: `package.json` (`@tauri-apps/plugin-dialog`)

- [ ] **Step 1: Add the dialog plugin deps**

```bash
cargo add --manifest-path src-tauri/Cargo.toml tauri-plugin-dialog
npm install @tauri-apps/plugin-dialog
```

- [ ] **Step 2: Add commands to `ipc.rs`**

Ajouter en haut de `src-tauri/src/ipc.rs`, étendre l'`use` crate :

```rust
use crate::{db, ffmpeg, queue, scanner, sources};
use tauri::{AppHandle, Emitter, Manager};
```

Ajouter à la fin de `src-tauri/src/ipc.rs` :

```rust
/// Adds a watched folder, then kicks off a background full scan + reconcile.
/// Returns the source immediately (count 0); the scan emits `queue:changed` when done.
#[tauri::command]
pub fn add_source(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    path: String,
) -> Result<sources::Source, String> {
    let id = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        sources::add(&conn, &path).map_err(|e| e.to_string())?
    };
    spawn_scan(app, id);
    // Return the freshly-added source (pending_count starts at 0 pre-scan).
    let conn = conn.lock().map_err(|e| e.to_string())?;
    sources::list(&conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "source not found after insert".to_string())
}

#[tauri::command]
pub fn list_sources(conn: State<'_, Mutex<Connection>>) -> Result<Vec<sources::Source>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    sources::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_source(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        sources::remove(&conn, id).map_err(|e| e.to_string())?;
    }
    crate::watcher::stop(&app, id);
    app.emit("queue:changed", ()).ok();
    Ok(())
}

#[tauri::command]
pub fn list_queue(conn: State<'_, Mutex<Connection>>) -> Result<Vec<queue::QueueItem>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    queue::list_pending(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rescan_source(app: AppHandle, id: i64) -> Result<(), String> {
    spawn_scan(app, id);
    Ok(())
}

/// Runs a reconcile for `source_id` on a background thread (walkdir is blocking IO),
/// then starts the live watcher and notifies the front. Errors are logged, not fatal.
fn spawn_scan(app: AppHandle, source_id: i64) {
    std::thread::spawn(move || {
        let path: Option<String> = {
            let state = app.state::<Mutex<Connection>>();
            let conn = match state.lock() {
                Ok(c) => c,
                Err(_) => return,
            };
            conn.query_row(
                "SELECT path FROM sources WHERE id=?1",
                rusqlite::params![source_id],
                |r| r.get(0),
            )
            .ok()
        };
        let Some(path) = path else { return };

        let result = {
            let state = app.state::<Mutex<Connection>>();
            let conn = match state.lock() {
                Ok(c) => c,
                Err(_) => return,
            };
            scanner::reconcile(&conn, source_id, std::path::Path::new(&path))
        };
        match result {
            Ok(stats) => log::info!("scan source {source_id}: {stats:?}"),
            Err(e) => log::error!("scan source {source_id} failed: {e}"),
        }
        crate::watcher::start(&app, source_id, &path);
        app.emit("queue:changed", ()).ok();
    });
}
```

> Note: `app.state::<Mutex<Connection>>()` requires `use tauri::Manager;` (added in Step 2's `use`).

- [ ] **Step 3: Register commands + plugin in `lib.rs`**

Dans `src-tauri/src/lib.rs`, dans le `.setup(...)` après `app.manage(Mutex::new(conn));`, démarrer les watchers des sources existantes :

```rust
            app.manage(Mutex::new(conn));
            watcher::init_state(app.handle());
            watcher::start_all(app.handle());
            Ok(())
```

Déclarer le module en haut : ajouter `mod watcher;` à la liste des `mod`.

Enregistrer le plugin dialog (chaîner avant `.setup`) :

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
```

Étendre `invoke_handler` :

```rust
        .invoke_handler(tauri::generate_handler![
            ipc::app_info,
            ipc::db_health,
            ipc::ffmpeg_version,
            ipc::report_smoke,
            ipc::add_source,
            ipc::list_sources,
            ipc::remove_source,
            ipc::list_queue,
            ipc::rescan_source
        ])
```

> `watcher::*` n'existe pas encore → ça ne compile pas tant que Task 7 n'est pas faite. C'est attendu : Task 6 et 7 forment une paire. On commit après Task 7.

- [ ] **Step 4: Add the dialog capability**

Dans `src-tauri/capabilities/default.json`, ajouter `"dialog:default"` au tableau `permissions` :

```json
  "permissions": [
    "core:default",
    "dialog:default"
  ]
```

(Pas de commit ici — la compilation dépend de Task 7.)

---

## Task 7: Watcher live (`notify-debouncer-full`)

**Files:**
- Create: `src-tauri/src/watcher.rs`
- Modify: `src-tauri/Cargo.toml` (`notify-debouncer-full`) — fait en Step 1

- [ ] **Step 1: Add the dependency**

```bash
cargo add --manifest-path src-tauri/Cargo.toml notify-debouncer-full
```

- [ ] **Step 2: Create `watcher.rs`**

Créer `src-tauri/src/watcher.rs` :

```rust
//! Live folder watching. One debounced recursive watcher per source; its ~500 ms settle
//! window also coalesces the burst of events a file move produces (no explicit
//! stability polling — see the M1 design doc). On audio create/modify → upsert pending;
//! on delete → forget the pending row. Each batch emits `queue:changed`.
use crate::scanner;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use notify_debouncer_full::notify::RecommendedWatcher;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

type Handle = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Map of source_id → live debouncer. Stored in Tauri managed state so handles stay alive.
#[derive(Default)]
pub struct Watchers(pub Mutex<HashMap<i64, Handle>>);

/// Registers the empty watcher state. Call once in setup, before `start_all`.
pub fn init_state(app: &AppHandle) {
    app.manage(Watchers::default());
}

/// Starts (or restarts) watchers for every source currently in the DB.
pub fn start_all(app: &AppHandle) {
    let rows: Vec<(i64, String)> = {
        let state = app.state::<Mutex<Connection>>();
        let Ok(conn) = state.lock() else { return };
        let Ok(mut stmt) = conn.prepare("SELECT id, path FROM sources WHERE watched=1") else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) else {
            return;
        };
        rows.filter_map(|r| r.ok()).collect()
    };
    for (id, path) in rows {
        start(app, id, &path);
    }
}

/// Starts a recursive debounced watcher on `path` for `source_id`. Replaces any existing one.
pub fn start(app: &AppHandle, source_id: i64, path: &str) {
    if !std::path::Path::new(path).is_dir() {
        log::warn!("watch skipped, not a dir: {path}");
        return;
    }
    let app2 = app.clone();
    let debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: DebounceEventResult| handle_events(&app2, source_id, res),
    );
    let mut debouncer = match debouncer {
        Ok(d) => d,
        Err(e) => {
            log::error!("debouncer init failed for {path}: {e}");
            return;
        }
    };
    if let Err(e) = debouncer
        .watcher()
        .watch(std::path::Path::new(path), RecursiveMode::Recursive)
    {
        log::error!("watch failed for {path}: {e}");
        return;
    }
    let state = app.state::<Watchers>();
    if let Ok(mut map) = state.0.lock() {
        map.insert(source_id, debouncer); // dropping the old handle stops the old watch
    }
}

/// Stops and drops the watcher for `source_id`, if any.
pub fn stop(app: &AppHandle, source_id: i64) {
    let state = app.state::<Watchers>();
    if let Ok(mut map) = state.0.lock() {
        map.remove(&source_id);
    }
}

/// Applies a debounced batch of FS events to the DB, then notifies the front.
fn handle_events(app: &AppHandle, source_id: i64, res: DebounceEventResult) {
    let events = match res {
        Ok(ev) => ev,
        Err(errs) => {
            for e in errs {
                log::warn!("watch error: {e}");
            }
            return;
        }
    };
    let state = app.state::<Mutex<Connection>>();
    let Ok(conn) = state.lock() else { return };
    let mut touched = false;

    for ev in events {
        for path in &ev.paths {
            match ev.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    if path.is_file() && scanner::is_audio(path) {
                        if let Ok(meta) = path.metadata() {
                            let f = scanner::DiskFile {
                                path: path.to_string_lossy().into_owned(),
                                filename: path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                size_bytes: meta.len() as i64,
                                mtime: meta
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64)
                                    .unwrap_or(0),
                            };
                            if scanner::upsert_file(&conn, source_id, &f).is_ok() {
                                touched = true;
                            }
                        }
                    }
                }
                EventKind::Remove(_) => {
                    let p = path.to_string_lossy();
                    if let Ok(n) = scanner::forget_path(&conn, &p) {
                        if n > 0 {
                            touched = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    drop(conn);
    if touched {
        app.emit("queue:changed", ()).ok();
    }
}
```

> The exact import paths (`RecommendedWatcher`, `RecommendedCache`, `DebounceEventResult`) match `notify-debouncer-full` ≥ 0.3. If `cargo build` reports an unresolved import, run `cargo doc --open -p notify-debouncer-full` (or check docs.rs) and adjust the `use` to the installed version's re-exports — the logic is unchanged.

- [ ] **Step 3: Build the whole backend**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles clean (Tasks 6 + 7 together resolve `watcher::*`). Fix any import-path mismatch per the note above.

- [ ] **Step 4: Run the full Rust test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS (db, scanner, sources, queue tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ipc.rs src-tauri/src/lib.rs src-tauri/src/watcher.rs \
        src-tauri/capabilities/default.json src-tauri/Cargo.toml src-tauri/Cargo.lock \
        package.json package-lock.json
git commit -m "feat(m1): IPC commands (add/list/remove/rescan source, list_queue) + live watcher + dialog plugin"
```

---

## Task 8: Front — contrats, helpers IPC, wiring live

**Files:**
- Modify: `shared/contracts.ts`
- Modify: `frontend/ipc.ts`
- Create: `frontend/sift-live.ts`
- Modify: `frontend/main.ts`
- Modify: `frontend/app.js` (2 hooks)

- [ ] **Step 1: Add the contract types**

Ajouter à la fin de `shared/contracts.ts` :

```ts
export interface Source {
  id: number;
  path: string;
  pending_count: number;
  accessible: boolean;
}

export interface QueueItem {
  id: number;
  path: string;
  filename: string | null;
  source_id: number | null;
}
```

- [ ] **Step 2: Add IPC helpers**

Ajouter à la fin de `frontend/ipc.ts` :

```ts
import type { Source, QueueItem } from "../shared/contracts";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const addSource = (path: string): Promise<Source> =>
  invoke("add_source", { path });
export const listSources = (): Promise<Source[]> => invoke("list_sources");
export const removeSource = (id: number): Promise<void> =>
  invoke("remove_source", { id });
export const listQueue = (): Promise<QueueItem[]> => invoke("list_queue");
export const rescanSource = (id: number): Promise<void> =>
  invoke("rescan_source", { id });

/** Subscribe to backend "queue:changed" pings. Returns an unlisten fn. */
export const onQueueChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("queue:changed", () => cb());
```

- [ ] **Step 3: Add the two render hooks in `app.js`**

Dans `frontend/app.js`, à la **fin** de `renderHome()` (juste après la ligne `content.innerHTML='<div class="home-body">...';`), ajouter :

```js
    if (window.__siftHome) window.__siftHome();
```

À la **fin** de `renderRevue()` (après le bloc `qd.addEventListener(...)`), ajouter :

```js
    if (window.__siftQueue) window.__siftQueue();
```

Ces hooks ne font rien en navigateur (démo) et sont remplis par `sift-live` en mode Tauri.

- [ ] **Step 4: Create `frontend/sift-live.ts`**

```ts
// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  addSource,
  listSources,
  removeSource,
  listQueue,
  onQueueChanged,
} from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import type { Source, QueueItem } from "../shared/contracts";

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );

/** Replaces app.js's mockup "Dossiers surveillés" block with real sources + warning. */
async function renderHomeSources() {
  // app.js builds the block as the last .col-h + following .srow/button nodes; we
  // simply append a real panel after the content and hide the mockup one.
  const content = document.getElementById("content");
  if (!content) return;
  let sources: Source[] = [];
  try {
    sources = await listSources();
  } catch (e) {
    console.error("listSources failed", e);
    return;
  }

  // Remove a previously injected panel, if any.
  document.getElementById("sift-sources")?.remove();

  const rows = sources
    .map((s) => {
      const warn = s.accessible
        ? ""
        : ' <span style="color:var(--color-text-danger);font-size:11px">⚠ inaccessible</span>';
      return `<div class="srow"><span class="v"><i class="ti ti-folder"></i> ${esc(
        s.path,
      )}${warn}</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:var(--color-text-info)">${
        s.pending_count
      } en file</span><button data-sift="rmsrc" data-id="${s.id}" style="font-size:11px;padding:2px 7px;color:var(--color-text-danger)">retirer</button></span></div>`;
    })
    .join("");

  const panel = document.createElement("div");
  panel.id = "sift-sources";
  panel.innerHTML =
    '<div class="col-h" style="margin-top:12px">Dossiers surveillés</div>' +
    '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin:0 0 8px;font-size:11px;color:var(--color-text-warning)"><i class="ti ti-info-circle" style="font-size:14px;flex:none"></i><span>Pointe Sift sur ton dossier <strong>Completed</strong> (pas <em>Incomplete</em>) — les fichiers en cours de téléchargement ne doivent pas entrer dans la file.</span></div>' +
    (rows || '<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun dossier surveillé.</div>') +
    '<div style="margin:8px 0 0"><button data-sift="addsrc"><i class="ti ti-plus" style="font-size:13px;vertical-align:-2px"></i> ajouter un dossier</button></div>';

  // Hide app.js's mockup folder section (its "ajouter un dossier" button) to avoid duplicates.
  content
    .querySelectorAll<HTMLButtonElement>("button")
    .forEach((b) => {
      if (b.textContent && b.textContent.includes("ajouter un dossier") && !b.dataset.sift) {
        b.closest("div")?.style.setProperty("display", "none");
      }
    });

  content.querySelector(".home-left")?.appendChild(panel);
}

/** Replaces the mockup queue list with real pending items (Revue screen). */
async function renderQueue() {
  const ql = document.getElementById("ql");
  if (!ql) return;
  let items: QueueItem[] = [];
  try {
    items = await listQueue();
  } catch (e) {
    console.error("listQueue failed", e);
    return;
  }
  ql.innerHTML =
    items
      .map(
        (it) =>
          `<div class="qi"><i class="ti ti-circle"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span></div>`,
      )
      .join("") ||
    '<div style="font-size:12px;color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>';
}

async function pickAndAddFolder() {
  const dir = await open({ directory: true, multiple: false });
  if (typeof dir === "string") {
    try {
      await addSource(dir);
      await refresh();
    } catch (e) {
      console.error("addSource failed", e);
    }
  }
}

async function refresh() {
  await renderHomeSources();
  await renderQueue();
}

export function installLiveWiring() {
  window.__siftHome = renderHomeSources;
  window.__siftQueue = renderQueue;

  // Delegated handlers for the buttons we inject.
  document.getElementById("pa")?.addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-sift]");
    if (!el) return;
    const act = el.dataset.sift;
    if (act === "addsrc") {
      e.stopPropagation();
      void pickAndAddFolder();
    } else if (act === "rmsrc") {
      e.stopPropagation();
      void removeSource(Number(el.dataset.id)).then(refresh);
    }
  });

  void onQueueChanged(refresh);
  void refresh();
}

declare global {
  interface Window {
    __siftHome?: () => void;
    __siftQueue?: () => void;
  }
}
```

- [ ] **Step 5: Wire it from `main.ts`**

Dans `frontend/main.ts`, dans le bloc `if (inTauri) { ... }`, ajouter l'import + l'appel (après le smoke IIFE ou avant — l'ordre n'importe pas) :

```ts
import { installLiveWiring } from "./sift-live";
```

et, à l'intérieur du `if (inTauri) {` (en première ligne du bloc) :

```ts
  installLiveWiring();
```

- [ ] **Step 6: Type-check the frontend**

Run: `npx tsc --noEmit`
Expected: PASS (no type errors). Si `@tauri-apps/plugin-dialog` n'a pas de types résolus, vérifier qu'il est bien dans `dependencies` (Task 6 Step 1).

- [ ] **Step 7: Manual verification in the app**

Run: `npm run tauri dev`
Steps:
1. Accueil → un bouton **« ajouter un dossier »** + la bannière warning Completed s'affichent.
2. Cliquer « ajouter un dossier » → picker natif → choisir un dossier contenant quelques `.mp3`/`.flac`.
3. Le compteur « N en file » de la source se met à jour ; l'onglet **Revue** liste les fichiers (nom brut).
4. Déposer un nouveau fichier audio dans le dossier (hors de l'app) → il **apparaît** en file sans action (watcher + `queue:changed`).
5. Supprimer un fichier du dossier → il **disparaît** de la file.
6. « retirer » une source → ses items quittent la file.

Confirmer dans le terminal dev les logs `scan source N: ReconcileStats { ... }`.

- [ ] **Step 8: Build the web bundle (regression check)**

Run: `npm run build`
Expected: build OK (la démo web compile ; `sift-live`/IPC ne s'exécutent pas hors Tauri).

- [ ] **Step 9: Commit**

```bash
git add shared/contracts.ts frontend/ipc.ts frontend/sift-live.ts frontend/main.ts frontend/app.js
git commit -m "feat(m1): front wiring — real sources + live queue, native folder picker, Completed warning"
```

---

## Task 9: Docs — clôturer le jalon

**Files:**
- Modify: `README.md` (statut M1)
- Modify: `docs/plan-implementation.md` (cocher M1 si tableau de statut présent)

- [ ] **Step 1: Update README milestone table**

Dans `README.md`, passer la ligne M1 à fait :

```markdown
| M1 — Watcher + file « à traiter » | ✅ **fait** — multi-dossiers, scan+diff, watcher live (notify), file = tracks pending, UI Accueil + Revue câblées |
```

- [ ] **Step 2: Commit**

```bash
git add README.md docs/plan-implementation.md
git commit -m "docs(m1): mark M1 done — watcher + queue livré"
```

---

## Self-Review (effectuée à l'écriture)

- **Couverture spec** : surveillance multi-dossiers (Task 4/7), scan complet + diff (Task 3), identité chemin + mtime/size (Task 3 `upsert_file`), suppression des `pending` disparus (Task 3 `reconcile` + watcher remove), pas de stability-check (watcher = debouncer seul, Task 7), file = `tracks pending` (Task 5), UI Accueil + warning Completed + file live (Task 8), events `queue:changed` (Task 6/7), `remove_source` purge les `pending` via cascade (Task 1 FK + Task 4). ✅
- **Hors périmètre** respecté : aucune analyse FFmpeg/verdict/player/rangement.
- **Cohérence des types** : `DiskFile`, `ReconcileStats`, `upsert_file`, `forget_path`, `reconcile`, `Source`, `QueueItem` définis une fois et réutilisés à l'identique (scanner ↔ watcher ↔ ipc ↔ contracts). `queue:changed` est le seul nom d'event, identique partout.
- **Dépendance inter-tâches assumée** : Tasks 6 et 7 ne compilent qu'ensemble (commit après Task 7) — explicité dans les deux tâches.
- **Risque connu** : versions exactes des re-exports de `notify-debouncer-full` (note dans Task 7 Step 2) à ajuster si le build le signale.
