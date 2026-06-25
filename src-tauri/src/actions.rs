//! The undo engine over the `actions` journal. One user action (Ranger/Jeter/Re-sourcer)
//! is one `batch_id` grouping several rows (convert, move, …). `revert_batch` is the single
//! guarded inversion primitive; `undo_last` (LIFO) and the journal both go through it, so
//! there is exactly one place that knows how to safely reverse work. Pure DB + filesystem.

use rusqlite::{params, Connection};
use serde::Serialize;

/// A raw action row as loaded for reverting: (id, track_id, type, from_path, to_path).
type ActionRow = (i64, Option<i64>, String, Option<String>, Option<String>);

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

/// Reverse a whole user action (all live rows of `batch_id`), newest-first, then set the
/// track back to `pending` (folder cleared) and mark the rows `undone`. Blocked if the
/// batch has no live rows, or if a newer live action on the same track exists outside it.
pub fn revert_batch(conn: &Connection, batch_id: &str) -> Result<(), RevertError> {
    // Load this batch's live rows, newest first.
    let mut stmt = conn.prepare(
        "SELECT id, track_id, type, from_path, to_path FROM actions
         WHERE batch_id=?1 AND undone=0 ORDER BY id DESC",
    )?;
    let rows: Vec<ActionRow> = stmt
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

    // Restore track + mark rows undone. Also clear the filing-time columns so the re-queued
    // track carries no stale target/confidence, and drop the metadata row written at filing
    // time so a later reconcile starts fresh. (analyzed_at is left intact — the file is
    // unchanged, so its analysis/verdict stay valid and need no recompute.) Note: the
    // embedded tag write done at filing time is NOT reversed (no tag action is journaled).
    if let Some(tid) = track_id {
        conn.execute(
            "UPDATE tracks SET status='pending', folder=NULL, target_format=NULL, confidence=NULL
             WHERE id=?1",
            params![tid],
        )?;
        conn.execute("DELETE FROM metadata WHERE track_id=?1", params![tid])?;
    }
    conn.execute(
        "UPDATE actions SET undone=1 WHERE batch_id=?1 AND undone=0",
        params![batch_id],
    )?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn record_inserts_a_row() {
        let conn = db();
        // track_id None: record() is exercised independently of any track row (FK-safe)
        let id = record(&conn, "b1", None, "move", Some("/a"), Some("/b")).unwrap();
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

    #[test]
    fn revert_move_puts_file_back() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let to = dir.path().join("bin/orig.mp3");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&to, b"x").unwrap(); // currently at destination
        revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap())).unwrap();
        assert!(from.exists() && !to.exists());
    }

    #[test]
    fn revert_move_blocked_when_origin_occupied() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let to = dir.path().join("bin/orig.mp3");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&from, b"old").unwrap(); // origin already taken → must not overwrite
        std::fs::write(&to, b"new").unwrap();
        let err = revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap()));
        assert!(matches!(err, Err(RevertError::Blocked(_))));
        assert!(to.exists()); // nothing moved
    }

    #[test]
    fn revert_convert_deletes_converted_file() {
        let dir = tempfile::tempdir().unwrap();
        let converted = dir.path().join("out.aiff");
        std::fs::write(&converted, b"x").unwrap();
        revert_one_fs("convert", Some("/orig.flac"), Some(converted.to_str().unwrap())).unwrap();
        assert!(!converted.exists());
    }

    #[test]
    fn revert_reject_is_noop() {
        assert!(revert_one_fs("reject", None, None).is_ok());
    }

    /// Insert a filed track + its convert/move batch, with the file physically at `to`.
    fn seed_filed(conn: &Connection, dir: &Path, batch: &str) -> (i64, std::path::PathBuf, std::path::PathBuf) {
        conn.execute(
            "INSERT INTO tracks(path, status, folder, target_format, confidence)
             VALUES(?1, 'filed', 'House', 'aiff_16_44', 'green')",
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
        // filing-time columns cleared on undo
        let (tf, cf): (Option<String>, Option<String>) = conn
            .query_row("SELECT target_format, confidence FROM tracks WHERE id=?1", params![track_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(tf, None);
        assert_eq!(cf, None);
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
}
