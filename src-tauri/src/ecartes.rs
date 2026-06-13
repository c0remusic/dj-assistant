//! The "Écartés" read model + bin actions: tracks the user rejected (`status='resourcing'`,
//! to re-source) or trashed (`status='trash'`). Lists them with the inputs the UI turns into
//! a reason badge (verdict, truncated) plus a clean artist/title for the Soulseek re-download,
//! and manages the bin: restore a trashed file, or purge the bin for good. Sending a
//! resourcing track to the bin reuses `filing::trash_track`.
#![allow(dead_code)]

use crate::filing;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

/// One rejected/trashed track for the Écartés view.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EcarteItem {
    pub id: i64,
    pub path: String,
    pub filename: Option<String>,
    /// 'resourcing' | 'trash'
    pub status: String,
    /// 'ok' | 'fake' | 'grey' | null (from analysis)
    pub verdict: Option<String>,
    pub truncated: bool,
    /// Clean artist/title for the Soulseek copy (reconciled from tags + filename).
    pub artist: String,
    pub title: String,
}

/// All rejected/trashed tracks, oldest first.
pub fn list_ecartes(conn: &Connection) -> rusqlite::Result<Vec<EcarteItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, filename, status, verdict, truncated
         FROM tracks WHERE status IN ('resourcing','trash') ORDER BY id",
    )?;
    let rows: Vec<(i64, String, Option<String>, String, Option<String>, Option<i64>)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, path, filename, status, verdict, truncated) in rows {
        let (artist, title) = match filing::reconcile_track(conn, id) {
            Ok(c) => (c.artist, c.title),
            Err(_) => (String::new(), String::new()),
        };
        out.push(EcarteItem {
            id,
            path,
            filename,
            status,
            verdict,
            truncated: truncated.unwrap_or(0) != 0,
            artist,
            title,
        });
    }
    Ok(out)
}

/// Move a trashed track's file back from `.sift-trash` to its original location and re-queue
/// it (`status='pending'`). Guarded: refuses if the trashed file is gone or the original
/// location is occupied. Marks the `trash` action undone.
pub fn restore_track(conn: &Connection, track_id: i64) -> Result<(), String> {
    let (action_id, from, to): (i64, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT id, from_path, to_path FROM actions
             WHERE track_id=?1 AND type='trash' AND undone=0 ORDER BY id DESC LIMIT 1",
            params![track_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "no trashed file to restore".to_string(),
            o => o.to_string(),
        })?;
    let from = from.ok_or("missing original path")?;
    let to = to.ok_or("missing trash path")?;
    if !Path::new(&to).exists() {
        return Err(format!("trashed file gone: {to}"));
    }
    if Path::new(&from).exists() {
        return Err(format!("original location occupied: {from}"));
    }
    if let Some(parent) = Path::new(&from).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&to, &from).map_err(|e| e.to_string())?;
    conn.execute("UPDATE actions SET undone=1 WHERE id=?1", params![action_id])
        .map_err(|e| e.to_string())?;
    conn.execute("UPDATE tracks SET status='pending' WHERE id=?1", params![track_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Permanently delete the files of all trashed tracks and mark them `purged`. Irreversible.
/// Returns how many were purged.
pub fn purge_trash(conn: &Connection) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, a.id, a.to_path FROM tracks t
             JOIN actions a ON a.track_id=t.id AND a.type='trash' AND a.undone=0
             WHERE t.status='trash'",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(i64, i64, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
    let mut n = 0;
    for (tid, aid, to) in rows {
        if let Some(p) = &to {
            let _ = std::fs::remove_file(p);
        }
        conn.execute("UPDATE actions SET undone=1 WHERE id=?1", params![aid])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE tracks SET status='purged' WHERE id=?1", params![tid])
            .map_err(|e| e.to_string())?;
        n += 1;
    }
    // Sweep any trashed track without a live trash action (orphaned journal) so it doesn't
    // linger in Écartés forever — there's no file path to delete, just clear the status.
    conn.execute("UPDATE tracks SET status='purged' WHERE status='trash'", [])
        .map_err(|e| e.to_string())?;
    Ok(n)
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
    fn list_only_resourcing_and_trash() {
        let conn = db();
        conn.execute("INSERT INTO tracks(path, status) VALUES('a.mp3','pending')", []).unwrap();
        conn.execute("INSERT INTO tracks(path, status, verdict) VALUES('b.mp3','resourcing','fake')", []).unwrap();
        conn.execute("INSERT INTO tracks(path, status, truncated) VALUES('c.wav','trash',1)", []).unwrap();
        conn.execute("INSERT INTO tracks(path, status) VALUES('d.aiff','filed')", []).unwrap();

        let items = list_ecartes(&conn).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].status, "resourcing");
        assert_eq!(items[0].verdict.as_deref(), Some("fake"));
        assert_eq!(items[1].status, "trash");
        assert!(items[1].truncated);
    }

    #[test]
    fn restore_moves_file_back_and_repends() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let trash = dir.path().join(".sift-trash/1__orig.mp3");
        std::fs::create_dir_all(trash.parent().unwrap()).unwrap();
        std::fs::write(&trash, b"x").unwrap(); // file currently in the bin
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'trash')",
            params![from.to_str().unwrap()],
        )
        .unwrap();
        let tid = conn.last_insert_rowid();
        crate::actions::record(&conn, "b1", Some(tid), "trash", Some(from.to_str().unwrap()), Some(trash.to_str().unwrap())).unwrap();

        restore_track(&conn, tid).unwrap();

        assert!(from.exists() && !trash.exists());
        let status: String = conn.query_row("SELECT status FROM tracks WHERE id=?1", params![tid], |r| r.get(0)).unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn restore_blocked_when_origin_occupied() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("orig.mp3");
        let trash = dir.path().join(".sift-trash/1__orig.mp3");
        std::fs::create_dir_all(trash.parent().unwrap()).unwrap();
        std::fs::write(&from, b"old").unwrap(); // origin already taken
        std::fs::write(&trash, b"new").unwrap();
        conn.execute("INSERT INTO tracks(path, status) VALUES(?1, 'trash')", params![from.to_str().unwrap()]).unwrap();
        let tid = conn.last_insert_rowid();
        crate::actions::record(&conn, "b1", Some(tid), "trash", Some(from.to_str().unwrap()), Some(trash.to_str().unwrap())).unwrap();

        assert!(restore_track(&conn, tid).is_err());
        assert!(trash.exists()); // nothing moved
    }

    #[test]
    fn purge_deletes_and_marks_purged() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let trash = dir.path().join(".sift-trash/1__x.mp3");
        std::fs::create_dir_all(trash.parent().unwrap()).unwrap();
        std::fs::write(&trash, b"x").unwrap();
        conn.execute("INSERT INTO tracks(path, status) VALUES('orig.mp3','trash')", []).unwrap();
        let tid = conn.last_insert_rowid();
        crate::actions::record(&conn, "b1", Some(tid), "trash", Some("orig.mp3"), Some(trash.to_str().unwrap())).unwrap();

        assert_eq!(purge_trash(&conn).unwrap(), 1);
        assert!(!trash.exists());
        let status: String = conn.query_row("SELECT status FROM tracks WHERE id=?1", params![tid], |r| r.get(0)).unwrap();
        assert_eq!(status, "purged");
    }
}
