//! The undo engine over the `actions` journal. One user action (Ranger/Jeter/Re-sourcer)
//! is one `batch_id` grouping several rows (convert, move, …). `revert_batch` is the single
//! guarded inversion primitive; `undo_last` (LIFO) and the journal both go through it, so
//! there is exactly one place that knows how to safely reverse work. Pure DB + filesystem.

use rusqlite::{params, Connection};
use serde::Serialize;

/// A raw action row as loaded for reverting: (id, track_id, type, from_path, to_path, meta).
/// `meta` is the free-form JSON column (v7): the `tag_edit` action stores its old-tags snapshot
/// there; every other type leaves it NULL.
type ActionRow = (i64, Option<i64>, String, Option<String>, Option<String>, Option<String>);

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
    record_with_meta(conn, batch_id, track_id, kind, from_path, to_path, None)
}

/// Like `record`, plus the free-form `meta` JSON column (v7). Used by `apply_tags` to stash the
/// old-tags snapshot a `tag_edit` revert needs. `record` is the thin no-meta wrapper.
pub fn record_with_meta(
    conn: &Connection,
    batch_id: &str,
    track_id: Option<i64>,
    kind: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
    meta: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO actions(track_id, type, from_path, to_path, batch_id, meta, session_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6,
                (SELECT value FROM settings WHERE key='current_session_id'))",
        params![track_id, kind, from_path, to_path, batch_id, meta],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Reverse one action's filesystem effect. Guards refuse to overwrite or act on stale
/// state; on a guard failure nothing is changed and `Blocked` is returned.
/// `pub(crate)`: also called directly by `ecartes::restore_track` (FIX-5), which reverses a
/// single `trash` action outside the `revert_batch` LIFO flow — same guards, same primitive,
/// no separate reimplementation.
pub(crate) fn revert_one_fs(
    kind: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
    meta: Option<&str>,
) -> Result<(), RevertError> {
    use std::path::Path;
    match kind {
        // file was moved from `from` to `to` — rename back (intra-disk, fast)
        "move" => {
            let from = from_path.ok_or_else(|| RevertError::Blocked("missing from_path".into()))?;
            let to = to_path.ok_or_else(|| RevertError::Blocked("missing to_path".into()))?;
            let to_exists = Path::new(to).exists();
            let from_exists = Path::new(from).exists();
            if !to_exists && from_exists {
                // File is already at origin (e.g. sync service restored it) — revert is
                // effectively done; let the caller mark it undone without touching the FS.
                return Ok(());
            }
            if !to_exists {
                return Err(RevertError::Blocked(format!("source gone: {to}")));
            }
            if from_exists {
                // Both from and to exist — genuine conflict, refuse to overwrite.
                return Err(RevertError::Blocked(format!("destination occupied: {from}")));
            }
            if let Some(parent) = Path::new(from).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| RevertError::Blocked(format!("mkdir {}: {e}", parent.display())))?;
            }
            std::fs::rename(to, from).map_err(|e| RevertError::Blocked(format!("move back: {e}")))
        }
        // file was trashed via copy→verify→delete (cross-disk safe); restore the same way
        "trash" => {
            let from = from_path.ok_or_else(|| RevertError::Blocked("missing from_path".into()))?;
            let to = to_path.ok_or_else(|| RevertError::Blocked("missing to_path".into()))?;
            let to_exists = Path::new(to).exists();
            let from_exists = Path::new(from).exists();
            if !to_exists && from_exists {
                // already at origin (e.g. manual restore) — nothing to do
                return Ok(());
            }
            if !to_exists {
                return Err(RevertError::Blocked(format!("trash file gone: {to}")));
            }
            if from_exists {
                return Err(RevertError::Blocked(format!("destination occupied: {from}")));
            }
            if let Some(parent) = Path::new(from).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| RevertError::Blocked(format!("mkdir {}: {e}", parent.display())))?;
            }
            let src_len = std::fs::metadata(to)
                .map_err(|e| RevertError::Blocked(format!("stat trash file: {e}")))?
                .len();
            std::fs::copy(to, from)
                .map_err(|e| RevertError::Blocked(format!("copy from trash: {e}")))?;
            let dst_len = match std::fs::metadata(from) {
                Ok(m) => m.len(),
                Err(e) => {
                    let _ = std::fs::remove_file(from);
                    return Err(RevertError::Blocked(format!("stat restored copy: {e}")));
                }
            };
            if dst_len != src_len {
                let _ = std::fs::remove_file(from);
                return Err(RevertError::Blocked(format!(
                    "trash restore size mismatch (src {src_len} != dst {dst_len})"
                )));
            }
            std::fs::remove_file(to)
                .map_err(|e| RevertError::Blocked(format!("remove from trash after restore: {e}")))
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
        // the file's tags were rewritten in place (Apply ID3 tags); `from_path` is the file and
        // `meta` holds the snapshot of the OLD tags captured before the write. Restore them exactly.
        // Guards: refuse cleanly if the file is gone or the snapshot is missing/corrupt; restore_tags
        // saves last, so a mid-restore failure leaves the file unchanged.
        "tag_edit" => {
            let path = from_path.ok_or_else(|| RevertError::Blocked("tag_edit missing from_path".into()))?;
            if !Path::new(path).exists() {
                return Err(RevertError::Blocked(format!("file gone: {path}")));
            }
            let meta = meta.ok_or_else(|| RevertError::Blocked("tag_edit missing tag snapshot".into()))?;
            let snap: crate::tagging::TagsSnapshot = serde_json::from_str(meta)
                .map_err(|e| RevertError::Blocked(format!("bad tag snapshot: {e}")))?;
            crate::tagging::restore_tags(path, &snap).map_err(RevertError::Blocked)
        }
        other => Err(RevertError::Blocked(format!("unknown action type: {other}"))),
    }
}

/// Reverse a whole user action (all live rows of `batch_id`), newest-first, then set the
/// track back to `pending` (folder cleared) and mark the rows `undone`. Blocked if the
/// batch has no live rows, or if a newer live action on the same track exists outside it.
pub fn revert_batch(conn: &Connection, batch_id: &str) -> Result<(), RevertError> {
    // Load this batch's live rows, newest first.
    let mut stmt = conn.prepare(
        "SELECT id, track_id, type, from_path, to_path, meta FROM actions
         WHERE batch_id=?1 AND undone=0 ORDER BY id DESC",
    )?;
    let rows: Vec<ActionRow> = stmt
        .query_map(params![batch_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
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

    // Reverse each row's filesystem effect (newest first), marking each row undone AS SOON AS its
    // revert succeeds. This keeps a PARTIAL failure (an FS error on a later row) consistent and
    // RE-TRYABLE: the rows already reverted stay marked undone, so a re-run resumes with only the
    // still-live rows instead of blocking on an already-restored file. Fail-fast on the FS error.
    for (id, _tid, kind, from_path, to_path, meta) in &rows {
        if let Err(e) = revert_one_fs(kind, from_path.as_deref(), to_path.as_deref(), meta.as_deref()) {
            // Surface the underlying FS failure (it carries the OS error string, e.g. Windows
            // "Access is denied. (os error 5)") instead of letting it vanish behind the `?`. The
            // convert step's `remove_file` is the one that strands a `.aiff` next to a restored
            // `.aif` when it is blocked by a held handle — this log is how we SEE why.
            log::error!(
                "revert_batch {batch_id}: FS step '{kind}' failed (from={from_path:?} to={to_path:?}): {e}"
            );
            return Err(e);
        }
        conn.execute("UPDATE actions SET undone=1 WHERE id=?1", params![id])?;
    }

    // Every row reverted: restore the track to pending and clear the filing-time columns so the
    // re-queued track carries no stale target/confidence. The metadata row (the IDENTIFICATION work:
    // artist/title/version/label/year/genres/discogs_release_id) is KEPT — reverting a FILING (the
    // file move/encode) must not throw away the identification. The result is the already-supported
    // "pending + identified" state (same as an identified-not-yet-filed track), so on reopen the
    // identity is restored and the B9 "tags not written" marker correctly shows (the file was rolled
    // back without the Discogs tags). (analyzed_at is left intact — the file is unchanged.)
    //
    // A tag_edit-only batch is NOT a filing: it never moved the file nor set 'filed', so reverting it
    // must touch ONLY the file's tags (done above) — never flip the track to pending. Skip the whole
    // block for such a batch. (Filing batches still NEVER journal a tag action.)
    let tag_only = rows.iter().all(|(_, _, kind, _, _, _)| kind.as_str() == "tag_edit");
    if let Some(tid) = track_id {
        if !tag_only {
            conn.execute(
                "UPDATE tracks SET status='pending', folder=NULL, target_format=NULL, confidence=NULL
                 WHERE id=?1",
                params![tid],
            )?;
        }
    }
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

/// One entry of the consultable journal: a live batch, summarized by its FIRST action.
/// `track_count` = number of distinct tracks in the batch (used by the front to gate
/// "last batch" confirmation on > 10 tracks). `session_id` = NULL for pre-migration rows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalEntry {
    pub batch_id: String,
    pub track_id: Option<i64>,
    /// The batch's FIRST action type (convert|move|trash|reject) — determines the display
    /// category. MIN instead of MAX so a convert+trash filing shows as "convert", not "trash".
    pub kind: String,
    pub from_path: Option<String>,
    pub to_path: Option<String>,
    pub ts: String,
    pub session_id: Option<String>,
    pub track_count: i64,
}

/// Recent live (not-yet-undone) batches, newest first, one entry per batch (summarized by
/// the batch's FIRST action row — MIN id — so a convert+trash filing shows kind="convert").
/// `session_id_filter` = Some(sid) to restrict to one session; None = all sessions.
/// `tag_edit` batches are excluded (they have no category in the Journal view).
pub fn list_journal(conn: &Connection, limit: i64, session_id_filter: Option<&str>) -> Vec<JournalEntry> {
    let mut stmt = match conn.prepare(
        "SELECT a.batch_id, a.track_id, a.type, a.from_path, a.to_path, a.ts,
                a.session_id, g.cnt
         FROM actions a
         JOIN (
             SELECT batch_id, MIN(id) AS mid, count(DISTINCT track_id) AS cnt
             FROM actions
             WHERE undone=0 AND batch_id IS NOT NULL AND type NOT IN ('tag_edit')
             GROUP BY batch_id
         ) g ON a.id = g.mid
         WHERE (?2 IS NULL OR a.session_id = ?2)
         ORDER BY a.id DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![limit, session_id_filter], |r| {
        Ok(JournalEntry {
            batch_id: r.get(0)?,
            track_id: r.get(1)?,
            kind: r.get(2)?,
            from_path: r.get(3)?,
            to_path: r.get(4)?,
            ts: r.get(5)?,
            session_id: r.get(6)?,
            track_count: r.get(7)?,
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
        revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap()), None).unwrap();
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
        let err = revert_one_fs("move", Some(from.to_str().unwrap()), Some(to.to_str().unwrap()), None);
        assert!(matches!(err, Err(RevertError::Blocked(_))));
        assert!(to.exists()); // nothing moved
    }

    #[test]
    fn revert_convert_deletes_converted_file() {
        let dir = tempfile::tempdir().unwrap();
        let converted = dir.path().join("out.aiff");
        std::fs::write(&converted, b"x").unwrap();
        revert_one_fs("convert", Some("/orig.flac"), Some(converted.to_str().unwrap()), None).unwrap();
        assert!(!converted.exists());
    }

    #[test]
    fn revert_reject_is_noop() {
        assert!(revert_one_fs("reject", None, None, None).is_ok());
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    /// The judge of the whole feature: applying then reverting a `tag_edit` must restore the file's
    /// original tags EXACTLY, while leaving the track's status and metadata row untouched (a tag edit
    /// is not a filing — it never moved the file nor set 'filed').
    #[test]
    fn revert_tag_edit_restores_tags_without_touching_status_or_metadata() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("track.mp3");
        std::fs::copy(&src, &file).unwrap();
        let path = file.to_str().unwrap();

        // A PENDING track with a metadata row — both must survive a tag_edit revert.
        conn.execute("INSERT INTO tracks(path, status) VALUES(?1, 'pending')", params![path]).unwrap();
        let tid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title) VALUES(?1, 'orig-a', 'orig-t')",
            params![tid],
        )
        .unwrap();

        // Capture old tags, apply new ones, journal the snapshot as a tag_edit (as apply_tags does).
        let before = crate::tagging::read_tags_full(path).unwrap();
        crate::tagging::write_tags_full(path, "NEW A", "NEW T", Some("NEW L"), Some(2030), &["Acid".to_string()], None).unwrap();
        let meta = serde_json::to_string(&before).unwrap();
        record_with_meta(&conn, "tg", Some(tid), "tag_edit", Some(path), None, Some(&meta)).unwrap();

        revert_batch(&conn, "tg").unwrap();

        // Tags restored to the original snapshot, exactly.
        assert_eq!(crate::tagging::read_tags_full(path).unwrap(), before);
        // Status and metadata row untouched.
        let status: String = conn.query_row("SELECT status FROM tracks WHERE id=?1", params![tid], |r| r.get(0)).unwrap();
        assert_eq!(status, "pending", "a tag_edit revert must not change status");
        let meta_rows: i64 = conn.query_row("SELECT count(*) FROM metadata WHERE track_id=?1", params![tid], |r| r.get(0)).unwrap();
        assert_eq!(meta_rows, 1, "a tag_edit revert must not drop metadata");
        // Row marked undone.
        let live: i64 = conn.query_row("SELECT count(*) FROM actions WHERE batch_id='tg' AND undone=0", [], |r| r.get(0)).unwrap();
        assert_eq!(live, 0);
    }

    #[test]
    fn revert_tag_edit_blocked_when_file_gone() {
        // Missing file → Blocked, nothing changes (the snapshot can't be applied to a vanished file).
        let err = revert_one_fs("tag_edit", Some("/no/such/file.mp3"), None, Some("{}"));
        assert!(matches!(err, Err(RevertError::Blocked(_))));
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

    /// Faithful reproduction of a real non-conformant filing (see filing.rs `execute_file`): the
    /// source is CONVERTED into the bin and the original is moved to `.sift-trash`, journalled as
    /// `convert`(source → converted) THEN `trash`(source → trash_path). revert_batch processes
    /// newest-first, so it must restore the original from trash BEFORE deleting the converted file —
    /// proving the no-data-loss ordering the relevé deduced by reading the code.
    #[test]
    fn revert_batch_conversion_restores_original_and_deletes_converted() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();

        // Physical post-filing state: converted file in the bin, original sitting in `.sift-trash`,
        // and the original source location empty.
        let source = dir.path().join("orig.flac");
        let converted = dir.path().join("House/orig.aiff");
        let trashed = dir.path().join(".sift-trash/1__orig.flac");
        std::fs::create_dir_all(converted.parent().unwrap()).unwrap();
        std::fs::create_dir_all(trashed.parent().unwrap()).unwrap();
        std::fs::write(&converted, b"converted-cdj").unwrap();
        std::fs::write(&trashed, b"original-flac").unwrap();
        assert!(!source.exists(), "source location is empty after the original was trashed");

        // DB: the filed track (+ a metadata row) and the two journalled actions, real order.
        conn.execute(
            "INSERT INTO tracks(path, status, folder, target_format, confidence)
             VALUES(?1, 'filed', 'House', 'aiff_16_44', 'green')",
            params![source.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title) VALUES(?1, 'A', 'B')",
            params![track_id],
        )
        .unwrap();
        record(&conn, "bc", Some(track_id), "convert", Some(source.to_str().unwrap()), Some(converted.to_str().unwrap())).unwrap();
        record(&conn, "bc", Some(track_id), "trash", Some(source.to_str().unwrap()), Some(trashed.to_str().unwrap())).unwrap();

        revert_batch(&conn, "bc").unwrap();

        // Original restored to its source (content intact); converted transcode deleted; trash emptied.
        assert!(source.exists(), "original must be restored to its source");
        assert_eq!(std::fs::read(&source).unwrap(), b"original-flac", "restored bytes are the original");
        assert!(!converted.exists(), "converted file must be deleted");
        assert!(!trashed.exists(), "trashed original must have been moved back");

        // Track back to pending, filing columns cleared, metadata PRESERVED, all rows undone.
        let (status, folder, tf, cf): (String, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, folder, target_format, confidence FROM tracks WHERE id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(folder, None);
        assert_eq!(tf, None);
        assert_eq!(cf, None);
        // Reverting a FILING must NOT erase the identification: the metadata row survives so the
        // track comes back "pending + identified" (no need to re-fetch Discogs).
        let (meta, artist): (i64, Option<String>) = conn
            .query_row(
                "SELECT count(*), max(artist) FROM metadata WHERE track_id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(meta, 1, "metadata identity preserved on a filing revert");
        assert_eq!(artist.as_deref(), Some("A"), "the identified artist survives the revert");
        let live: i64 = conn
            .query_row("SELECT count(*) FROM actions WHERE batch_id='bc' AND undone=0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 0, "all rows marked undone");
    }

    /// Partial-failure recovery: if an FS error hits a LATER action in the batch (here `convert`,
    /// processed second), the work already done (here `trash`, processed first) must stay marked
    /// undone so a re-run RESUMES instead of blocking on an already-restored file. Reproduces the
    /// real convert+trash filing; the convert revert is made to fail by pointing its `to` at a
    /// non-empty directory (`remove_file` errors), standing in for any transient FS error.
    #[test]
    fn revert_batch_resumes_after_partial_fs_failure() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();

        let source = dir.path().join("orig.flac");
        let converted = dir.path().join("House/orig.aiff");
        let trashed = dir.path().join(".sift-trash/1__orig.flac");
        std::fs::create_dir_all(trashed.parent().unwrap()).unwrap();
        std::fs::write(&trashed, b"original-flac").unwrap();
        // Make the convert revert FAIL on the first pass: `converted` is a non-empty DIRECTORY, so
        // remove_file(converted) errors (stand-in for a locked/undeletable file).
        std::fs::create_dir_all(&converted).unwrap();
        std::fs::write(converted.join("inner"), b"x").unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status, folder, target_format, confidence)
             VALUES(?1, 'filed', 'House', 'aiff_16_44', 'green')",
            params![source.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        record(&conn, "bp", Some(track_id), "convert", Some(source.to_str().unwrap()), Some(converted.to_str().unwrap())).unwrap();
        record(&conn, "bp", Some(track_id), "trash", Some(source.to_str().unwrap()), Some(trashed.to_str().unwrap())).unwrap();

        // First pass: trash reverts (original restored), convert FAILS. The partial work must be
        // PERSISTED row-by-row — trash marked undone — not discarded.
        let err = revert_batch(&conn, "bp");
        assert!(matches!(err, Err(RevertError::Blocked(_))), "convert remove_file fails on a dir");
        assert!(source.exists(), "the trash step already restored the original");
        let trash_undone: i64 = conn
            .query_row("SELECT undone FROM actions WHERE batch_id='bp' AND type='trash'", [], |r| r.get(0))
            .unwrap();
        let convert_undone: i64 = conn
            .query_row("SELECT undone FROM actions WHERE batch_id='bp' AND type='convert'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(trash_undone, 1, "the succeeded action is marked undone immediately");
        assert_eq!(convert_undone, 0, "the failed action stays live for a retry");
        let status: String = conn
            .query_row("SELECT status FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "filed", "status is NOT reset until the batch is fully reverted");

        // Clear the FS error (the path becomes a normal file), then re-run: it RESUMES with only the
        // still-live convert row and FINISHES — no block on the already-restored trash.
        std::fs::remove_dir_all(&converted).unwrap();
        std::fs::write(&converted, b"converted-cdj").unwrap();

        revert_batch(&conn, "bp").unwrap();
        assert!(!converted.exists(), "converted file deleted on the retry");
        let live: i64 = conn
            .query_row("SELECT count(*) FROM actions WHERE batch_id='bp' AND undone=0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 0, "all rows undone after the retry");
        let status: String = conn
            .query_row("SELECT status FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "pending", "track reset once the batch is fully reverted");
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

        let entries = list_journal(&conn, 10, None);
        let ids: Vec<&str> = entries.iter().map(|e| e.batch_id.as_str()).collect();
        assert_eq!(ids, vec!["b2", "b1"]); // newest first, one per batch
        assert_eq!(entries[0].kind, "convert"); // representative (first) action of the batch
    }

    /// Seed a non-conformant `.aif` filing in `dir`, SAME folder: `Track.aif` was converted into
    /// `Track.aiff` (forced extension) and the original trashed — the real `execute_file` order is
    /// `convert` then `trash`. Returns (original .aif, converted .aiff, batch_id).
    fn seed_aif_filing(conn: &Connection, dir: &Path, batch: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let original = dir.join("Track.aif");
        let converted = dir.join("Track.aiff");
        let trashed = dir.join(".sift-trash/1__Track.aif");
        std::fs::create_dir_all(trashed.parent().unwrap()).unwrap();
        std::fs::write(&converted, b"converted-cdj").unwrap();
        std::fs::write(&trashed, b"original-aif").unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status, folder, target_format, confidence)
             VALUES(?1, 'filed', 'House', 'aiff_16_44', 'green')",
            params![original.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        record(conn, batch, Some(track_id), "convert", Some(original.to_str().unwrap()), Some(converted.to_str().unwrap())).unwrap();
        record(conn, batch, Some(track_id), "trash", Some(original.to_str().unwrap()), Some(trashed.to_str().unwrap())).unwrap();
        (original, converted)
    }

    /// 2a — COLD reproduction of the `.aif`→`.aiff` filing. With nothing holding the converted file,
    /// a cold revert must leave EXACTLY ONE file (the restored `Track.aif`) and delete `Track.aiff`.
    /// Proves the inversion logic eliminates the duplicate when no FS step is blocked.
    #[test]
    fn cold_revert_of_aif_filing_leaves_single_file() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (original, converted) = seed_aif_filing(&conn, dir.path(), "ba");
        assert!(!original.exists(), "before revert the original .aif lives in trash");

        revert_batch(&conn, "ba").unwrap();

        assert!(original.exists(), "original .aif restored");
        assert_eq!(std::fs::read(&original).unwrap(), b"original-aif");
        assert!(!converted.exists(), "converted .aiff deleted — no .aif/.aiff duplicate");
    }

    /// 2b-i — DISCRIMINATES suspicion n°1 (the analysis worker holds the freshly-filed `.aiff` open
    /// and blocks its deletion). The worker opens audio with plain `std::fs::File::open` (see
    /// analysis/decode.rs and lofty's `Probe::open`). Holding the converted `.aiff` the SAME way
    /// during the revert: if std's Windows share mode includes FILE_SHARE_DELETE, `remove_file`
    /// succeeds despite the open handle and the revert leaves a single file — REFUTING "a std-reading
    /// worker causes the duplicate". The assertion is the verdict; if it ever fails, std blocks and
    /// the suspicion is instead confirmed.
    #[cfg(windows)]
    #[test]
    fn windows_std_reader_does_not_block_revert() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (original, converted) = seed_aif_filing(&conn, dir.path(), "bw");

        // Hold the .aiff open exactly like the analysis worker (plain std open), then revert.
        let handle = std::fs::File::open(&converted).unwrap();
        let res = revert_batch(&conn, "bw");
        drop(handle);

        assert!(res.is_ok(), "a std-opened reader does not block the revert: {res:?}");
        assert!(original.exists(), "original .aif restored");
        assert!(!converted.exists(), "converted .aiff deleted despite the open std handle");
    }

    /// 2b-ii — PROVES the trigger. A handle opened WITHOUT share-delete (the way an external locker
    /// such as the Windows Search indexer, an AV scanner, or Explorer's preview pane holds a file)
    /// blocks `remove_file`. The revert restores `Track.aif` from trash, then FAILS to delete
    /// `Track.aiff` → the exact `.aif` + `.aiff` duplicate in one folder the user reported. Dropping
    /// the handle and re-running completes the revert to a single file, proving the lock is the sole
    /// cause. This ASSERTS the current (buggy) duplicate — it is a reproduction, not a fix.
    #[cfg(windows)]
    #[test]
    fn windows_held_handle_reproduces_aif_aiff_duplicate() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001; // read sharing only — NO delete sharing

        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (original, converted) = seed_aif_filing(&conn, dir.path(), "bl");

        // Hold the .aiff with NO delete-share — models an external locker holding the re-enqueued file.
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&converted)
            .unwrap();

        let res = revert_batch(&conn, "bl");
        let err = res.expect_err("a delete-blocking handle must block the convert revert");
        eprintln!("REPRO os error on remove_file(.aiff): {err}");
        assert!(matches!(err, RevertError::Blocked(_)));

        // The reported bug: both `.aif` (restored) and `.aiff` (undeletable) coexist in one folder.
        assert!(original.exists(), "original .aif restored from trash");
        assert!(converted.exists(), "converted .aiff still present → the .aif/.aiff duplicate");

        // Release the handle and re-run: the revert RESUMES and finishes — single file remains.
        drop(handle);
        revert_batch(&conn, "bl").unwrap();
        assert!(original.exists() && !converted.exists(), "single file once the lock is gone");
    }
}
