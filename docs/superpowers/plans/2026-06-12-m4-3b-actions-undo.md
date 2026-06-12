# M4-3b — Actions / undo engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An undo engine over the `actions` journal: record each filesystem step of a filing, and reverse a whole user action (grouped by `batch_id`) with a single guarded primitive — powering both Ctrl+Z (LIFO) and the consultable journal.

**Architecture:** New `actions.rs`. One user action (Ranger / Jeter / Re-sourcer) = one `batch_id` grouping several `actions` rows (convert, move, …). `record` appends a row. `revert_batch(batch_id)` is the single inversion primitive: it reverses each row's filesystem effect newest-first, restores the track to `pending`, and marks the rows `undone` — all guarded so it never overwrites or acts on stale state. `undo_last` = revert the most recent live batch (LIFO); the journal lists batches and reverts a chosen one through the same primitive (clean-architecture: one well-bounded operation, two entry points). Pure DB + filesystem; testable by inserting rows and creating temp files — no ffmpeg, no app handle, no dependency on the filing orchestration (M4-3c), which will only *call* `record` and `undo_last`.

**Tech Stack:** Rust (edition 2021, MSRV 1.77.2), `rusqlite` 0.32, `serde`, `tempfile` (dev-dep). No new dependencies.

**This is plan 3b of 4 for M4** (naming ✓ → encode/tagging ✓ → 3a migration/settings/library ✓ → **3b actions/undo** → 3c filing + IPC → M4-4 frontend). M4-3c calls `actions::record` during filing and exposes `undo_last`/`revert_batch`/`list_journal` over IPC.

---

## Conventions for this plan

- cargo not on PATH: `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` (PowerShell), run from `src-tauri/`.
- `cargo test --lib actions` runs just this module; avoids linking `sift.exe`.
- Tests inline `#[cfg(test)] mod tests`; in-memory DB via `Connection::open_in_memory()` + `crate::db::run_migrations`; temp files via `tempfile::tempdir()`.
- `actions.rs` carries `#![allow(dead_code)]` (consumed by M4-3c).
- Action `type` values reuse the existing set: `convert | move | trash | reject`.
- Commit after each task. No `Co-Authored-By` trailer.

---

## File structure

- Create: `src-tauri/src/actions.rs` — `RevertError`, `record`, `revert_batch`, `undo_last`, `JournalEntry`, `list_journal`, internal `revert_one_fs`.
- Modify: `src-tauri/src/lib.rs` — add `mod actions;`.

The `actions` table (with `undone`, `batch_id`) already exists from migration v4.

---

## Task 1: `actions` module skeleton + `record`

**Files:**
- Create: `src-tauri/src/actions.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add `mod actions;` as the first module (alphabetical, before `pub mod analysis;` is fine, or right after it — keep the list tidy):

```rust
mod actions;
pub mod analysis;
mod db;
```

- [ ] **Step 2: Create `actions.rs` with `RevertError` + `record` + test**

```rust
//! The undo engine over the `actions` journal. One user action (Ranger/Jeter/Re-sourcer)
//! is one `batch_id` grouping several rows (convert, move, …). `revert_batch` is the single
//! guarded inversion primitive; `undo_last` (LIFO) and the journal both go through it, so
//! there is exactly one place that knows how to safely reverse work. Pure DB + filesystem.
#![allow(dead_code)]

use rusqlite::{params, Connection};

/// Why a revert could not proceed (nothing is changed when this is returned).
#[derive(Debug, Clone, PartialEq)]
pub enum RevertError {
    /// Unsafe to revert (collision, missing source, or a newer action depends on it).
    Blocked(String),
    /// Database error.
    Db(String),
}

impl std::fmt::Display for RevertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RevertError::Blocked(m) => write!(f, "revert blocked: {m}"),
            RevertError::Db(m) => write!(f, "db error: {m}"),
        }
    }
}

impl From<rusqlite::Error> for RevertError {
    fn from(e: rusqlite::Error) -> Self {
        RevertError::Db(e.to_string())
    }
}

/// Append one journaled action row and return its id. `batch_id` groups the rows of a
/// single user action; `kind` is one of convert|move|trash|reject.
pub fn record(
    conn: &Connection,
    batch_id: &str,
    track_id: Option<i64>,
    kind: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO actions(track_id, type, from_path, to_path, batch_id)
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![track_id, kind, from_path, to_path, batch_id],
    )?;
    Ok(conn.last_insert_rowid())
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
    fn record_inserts_a_row() {
        let conn = db();
        let id = record(&conn, "b1", Some(7), "move", Some("/a"), Some("/b")).unwrap();
        assert!(id > 0);
        let (kind, undone): (String, i64) = conn
            .query_row(
                "SELECT type, undone FROM actions WHERE id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "move");
        assert_eq!(undone, 0);
    }
}
```

- [ ] **Step 3: Run the test**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: PASS (1 test).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/actions.rs src-tauri/src/lib.rs
git commit -m "feat(m4-3b): actions module + record"
```

---

## Task 2: `revert_one_fs` — the guarded per-row filesystem inverse

**Files:**
- Modify: `src-tauri/src/actions.rs`

The inverse of each action's filesystem effect, with guards that refuse to overwrite or
act on stale state:
- `move` / `trash`: the file currently sits at `to_path`; move it back to `from_path`.
  Guard: `to_path` must exist, `from_path` must NOT exist.
- `convert`: a converted file was produced at `to_path`; delete it. (No-op if already gone.)
- `reject`: status-only, no filesystem effect — nothing to do.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    use std::fs;
    use std::path::Path;

    #[test]
    fn revert_move_puts_file_back() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let to = dir.path().join("bin/orig.mp3");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(&to, b"x").unwrap(); // currently at destination
        revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap())).unwrap();
        assert!(from.exists() && !to.exists());
    }

    #[test]
    fn revert_move_blocked_when_origin_occupied() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let to = dir.path().join("bin/orig.mp3");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(&from, b"old").unwrap(); // origin already taken → must not overwrite
        fs::write(&to, b"new").unwrap();
        let err = revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap()));
        assert!(matches!(err, Err(RevertError::Blocked(_))));
        assert!(to.exists()); // nothing moved
    }

    #[test]
    fn revert_convert_deletes_converted_file() {
        let dir = tempfile::tempdir().unwrap();
        let converted = dir.path().join("out.aiff");
        fs::write(&converted, b"x").unwrap();
        revert_one_fs("convert", Some("/orig.flac"), Some(converted.to_str().unwrap())).unwrap();
        assert!(!converted.exists());
    }

    #[test]
    fn revert_reject_is_noop() {
        assert!(revert_one_fs("reject", None, None).is_ok());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: FAIL — `cannot find function revert_one_fs`.

- [ ] **Step 3: Implement `revert_one_fs`**

Add above the test module:

```rust
/// Reverse one action's filesystem effect. Guards refuse to overwrite or act on stale
/// state; on a guard failure nothing is changed and `Blocked` is returned.
fn revert_one_fs(
    kind: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
) -> Result<(), RevertError> {
    use std::path::Path;
    match kind {
        // file was moved from `from` to `to` — put it back
        "move" | "trash" => {
            let from = from_path.ok_or_else(|| RevertError::Blocked("missing from_path".into()))?;
            let to = to_path.ok_or_else(|| RevertError::Blocked("missing to_path".into()))?;
            if !Path::new(to).exists() {
                return Err(RevertError::Blocked(format!("source gone: {to}")));
            }
            if Path::new(from).exists() {
                return Err(RevertError::Blocked(format!("destination occupied: {from}")));
            }
            if let Some(parent) = Path::new(from).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| RevertError::Blocked(format!("mkdir {}: {e}", parent.display())))?;
            }
            std::fs::rename(to, from).map_err(|e| RevertError::Blocked(format!("move back: {e}")))
        }
        // a converted file was produced at `to` — remove it (idempotent if already gone)
        "convert" => {
            if let Some(to) = to_path {
                if Path::new(to).exists() {
                    std::fs::remove_file(to)
                        .map_err(|e| RevertError::Blocked(format!("remove converted: {e}")))?;
                }
            }
            Ok(())
        }
        // status-only action — nothing on disk to reverse
        "reject" => Ok(()),
        other => Err(RevertError::Blocked(format!("unknown action type: {other}"))),
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/actions.rs
git commit -m "feat(m4-3b): guarded per-row filesystem inverse (revert_one_fs)"
```

---

## Task 3: `revert_batch` — reverse a whole user action

**Files:**
- Modify: `src-tauri/src/actions.rs`

Reverse every live row of a batch, newest-first, then restore the track to `pending` and
mark the rows `undone`. Two batch-level guards: the batch must have live (not-yet-undone)
rows, and no *newer* live action on the same track may exist outside this batch (LIFO
safety — prevents out-of-order conflicts from the journal).

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    /// Insert a filed track + its convert/move batch, with the file physically at `to`.
    fn seed_filed(conn: &Connection, dir: &Path, batch: &str) -> (i64, std::path::PathBuf, std::path::PathBuf) {
        conn.execute(
            "INSERT INTO tracks(path, status, folder) VALUES(?1, 'filed', 'House')",
            params![format!("{}/orig.mp3", dir.display())],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let from = dir.join("orig.mp3");
        let to = dir.join("House/orig.mp3");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&to, b"x").unwrap(); // file lives at destination after filing
        record(conn, batch, Some(track_id), "convert", Some(from.to_str().unwrap()), Some(to.to_str().unwrap())).unwrap();
        record(conn, batch, Some(track_id), "move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap())).unwrap();
        (track_id, from, to)
    }

    #[test]
    fn revert_batch_restores_file_and_status_and_marks_undone() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (track_id, from, to) = seed_filed(&conn, dir.path(), "b1");

        revert_batch(&conn, "b1").unwrap();

        // file moved back; status reset; folder cleared
        assert!(from.exists() && !to.exists());
        let (status, folder): (String, Option<String>) = conn
            .query_row("SELECT status, folder FROM tracks WHERE id=?1", params![track_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(folder, None);
        // rows marked undone
        let live: i64 = conn
            .query_row("SELECT count(*) FROM actions WHERE batch_id='b1' AND undone=0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 0);
    }

    #[test]
    fn revert_batch_blocked_when_newer_action_on_same_track() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (track_id, _from, _to) = seed_filed(&conn, dir.path(), "b1");
        // a newer, live action on the same track (e.g. re-filed since)
        record(&conn, "b2", Some(track_id), "move", Some("/x"), Some("/y")).unwrap();

        let err = revert_batch(&conn, "b1");
        assert!(matches!(err, Err(RevertError::Blocked(_))));
    }

    #[test]
    fn revert_batch_unknown_is_blocked() {
        let conn = db();
        assert!(matches!(revert_batch(&conn, "nope"), Err(RevertError::Blocked(_))));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: FAIL — `cannot find function revert_batch`.

- [ ] **Step 3: Implement `revert_batch`**

Add above the test module:

```rust
/// Reverse a whole user action (all live rows of `batch_id`), newest-first, then set the
/// track back to `pending` (folder cleared) and mark the rows `undone`. Blocked if the
/// batch has no live rows, or if a newer live action on the same track exists outside it.
pub fn revert_batch(conn: &Connection, batch_id: &str) -> Result<(), RevertError> {
    // Load this batch's live rows, newest first.
    let mut stmt = conn.prepare(
        "SELECT id, track_id, type, from_path, to_path FROM actions
         WHERE batch_id=?1 AND undone=0 ORDER BY id DESC",
    )?;
    let rows: Vec<(i64, Option<i64>, String, Option<String>, Option<String>)> = stmt
        .query_map(params![batch_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if rows.is_empty() {
        return Err(RevertError::Blocked(format!("no live actions for batch {batch_id}")));
    }

    let max_id = rows.iter().map(|r| r.0).max().unwrap();
    let track_id = rows.iter().find_map(|r| r.1);

    // LIFO safety: refuse if a newer live action touches the same track outside this batch.
    if let Some(tid) = track_id {
        let newer: i64 = conn.query_row(
            "SELECT count(*) FROM actions
             WHERE track_id=?1 AND undone=0 AND batch_id<>?2 AND id>?3",
            params![tid, batch_id, max_id],
            |r| r.get(0),
        )?;
        if newer > 0 {
            return Err(RevertError::Blocked(
                "a newer action on this track must be undone first".into(),
            ));
        }
    }

    // Reverse each row's filesystem effect (newest first).
    for (_id, _tid, kind, from_path, to_path) in &rows {
        revert_one_fs(kind, from_path.as_deref(), to_path.as_deref())?;
    }

    // Restore track + mark rows undone.
    if let Some(tid) = track_id {
        conn.execute(
            "UPDATE tracks SET status='pending', folder=NULL WHERE id=?1",
            params![tid],
        )?;
    }
    conn.execute(
        "UPDATE actions SET undone=1 WHERE batch_id=?1 AND undone=0",
        params![batch_id],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/actions.rs
git commit -m "feat(m4-3b): revert_batch — guarded whole-action undo"
```

---

## Task 4: `undo_last` (LIFO) + `list_journal`

**Files:**
- Modify: `src-tauri/src/actions.rs`

`undo_last` reverts the most recent live batch and returns its id (or `None` when nothing
is undoable). `list_journal` returns recent batches (one entry per batch, newest first) for
the journal UI.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    #[test]
    fn undo_last_reverts_most_recent_batch() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        // older batch b1 on its own track
        seed_filed(&conn, &dir.path().join("one"), "b1");
        // newer batch b2 on another track
        seed_filed(&conn, &dir.path().join("two"), "b2");

        let undone = undo_last(&conn).unwrap();
        assert_eq!(undone.as_deref(), Some("b2")); // newest first

        // b1 still live, b2 marked undone
        let b1_live: i64 = conn.query_row("SELECT count(*) FROM actions WHERE batch_id='b1' AND undone=0", [], |r| r.get(0)).unwrap();
        let b2_live: i64 = conn.query_row("SELECT count(*) FROM actions WHERE batch_id='b2' AND undone=0", [], |r| r.get(0)).unwrap();
        assert!(b1_live > 0 && b2_live == 0);
    }

    #[test]
    fn undo_last_none_when_empty() {
        let conn = db();
        assert_eq!(undo_last(&conn).unwrap(), None);
    }

    #[test]
    fn journal_lists_batches_newest_first() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        seed_filed(&conn, &dir.path().join("one"), "b1");
        seed_filed(&conn, &dir.path().join("two"), "b2");

        let entries = list_journal(&conn, 10);
        let ids: Vec<&str> = entries.iter().map(|e| e.batch_id.as_str()).collect();
        assert_eq!(ids, vec!["b2", "b1"]); // newest first, one per batch
        assert_eq!(entries[0].kind, "move"); // representative (latest) action of the batch
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: FAIL — `cannot find function undo_last` / `list_journal`.

- [ ] **Step 3: Implement `undo_last`, `JournalEntry`, `list_journal`**

Add `use serde::Serialize;` to the imports at the top of the file (next to the rusqlite
use), then add above the test module:

```rust
/// Revert the most recent live batch (LIFO). Returns the reverted batch id, or None if
/// there is nothing to undo.
pub fn undo_last(conn: &Connection) -> Result<Option<String>, RevertError> {
    let batch: Option<String> = conn
        .query_row(
            "SELECT batch_id FROM actions WHERE undone=0 AND batch_id IS NOT NULL
             ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(RevertError::Db(other.to_string())),
        })?;
    match batch {
        Some(b) => {
            revert_batch(conn, &b)?;
            Ok(Some(b))
        }
        None => Ok(None),
    }
}

/// One entry of the consultable journal: a live batch, summarized by its latest action.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalEntry {
    pub batch_id: String,
    pub track_id: Option<i64>,
    /// The batch's latest action type (convert|move|trash|reject) — what the user sees.
    pub kind: String,
    pub to_path: Option<String>,
    pub ts: String,
}

/// Recent live (not-yet-undone) batches, newest first, one entry per batch (summarized by
/// the batch's latest row). `limit` caps the number of batches returned.
pub fn list_journal(conn: &Connection, limit: i64) -> Vec<JournalEntry> {
    // For each live batch, take the row with the greatest id (its latest step).
    let mut stmt = match conn.prepare(
        "SELECT a.batch_id, a.track_id, a.type, a.to_path, a.ts
         FROM actions a
         JOIN (SELECT batch_id, MAX(id) AS mid FROM actions
               WHERE undone=0 AND batch_id IS NOT NULL GROUP BY batch_id) g
           ON a.id = g.mid
         ORDER BY a.id DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![limit], |r| {
        Ok(JournalEntry {
            batch_id: r.get(0)?,
            track_id: r.get(1)?,
            kind: r.get(2)?,
            to_path: r.get(3)?,
            ts: r.get(4)?,
        })
    });
    match rows {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => Vec::new(),
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: PASS (11 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/actions.rs
git commit -m "feat(m4-3b): undo_last (LIFO) + list_journal"
```

---

## Task 5: Full green + clippy

**Files:**
- None (verification only).

- [ ] **Step 1: Run the module's tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib actions`
Expected: PASS (11 tests).

- [ ] **Step 2: Clippy**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy --lib`
Expected: no warnings referencing `actions.rs` (fix any that are; pre-existing
`analysis/`/`worker.rs` warnings are out of scope).

- [ ] **Step 3: Commit any lint fixes**

```bash
git add src-tauri/src/actions.rs
git commit -m "chore(m4-3b): clippy clean for actions"
```

(Skip if already clean.)

---

## Done criteria

- `actions.rs` exists, declared in `lib.rs`, 11 inline tests green.
- `record`, `revert_batch` (guarded), `undo_last` (LIFO), `list_journal`, and
  `RevertError` / `JournalEntry` are public for M4-3c.
- A revert never overwrites, never acts on stale/missing files, refuses out-of-order
  conflicts, restores the track to `pending`, and marks rows `undone` (no double-undo).

**Next plan:** M4-3c — `filing` orchestration (convert→tag→move via naming/encode/tagging/
library, recording a batch through `actions::record`) + IPC commands + `lib.rs` wiring.
