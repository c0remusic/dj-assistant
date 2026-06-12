//! Disk scanning + reconciliation. Pure-ish logic: given a folder and the DB,
//! computes which audio files to add / update / drop from the queue.
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
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
                "UPDATE tracks SET filename=?2, size_bytes=?3, mtime=?4, source_id=?5,
                        status='pending', analyzed_at=NULL
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
        let sid = conn.last_insert_rowid();
        (conn, sid)
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
    fn changed_file_clears_analyzed_marker() {
        let (conn, sid) = db_with_source();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = root.join("t.wav");
        fs::write(&p, b"123").unwrap();
        reconcile(&conn, sid, root).unwrap();
        // pretend it was analysed
        conn.execute("UPDATE tracks SET analyzed_at=datetime('now'), verdict='ok'", []).unwrap();

        // content changes → re-pending AND analyzed_at cleared (forces re-analysis)
        fs::write(&p, b"123456789").unwrap();
        reconcile(&conn, sid, root).unwrap();
        let analyzed: Option<String> = conn
            .query_row("SELECT analyzed_at FROM tracks WHERE filename='t.wav'", [], |r| r.get(0))
            .unwrap();
        assert!(analyzed.is_none(), "analyzed_at must reset when the file changes");
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
