//! Duplicate detection — name first (here), sound confirmation layered on later (see the M5
//! spec). The cheap name pre-filter normalizes each track's name (from its filename) into a
//! key (`naming::name_key`) and flags collisions: `name_dups` marks the queue, `find_duplicate`
//! reports the best name match for one track. The acoustic confirmation upgrades the match
//! `kind` from `name` to `both` when the sound agrees.
#![allow(dead_code)]

use crate::naming;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A duplicate match for one track. `kind`: `name` (names agree) or `both` (name + sound).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DupMatch {
    pub id: i64,
    pub status: String,
    pub folder: Option<String>,
    pub filename: Option<String>,
    pub kind: String,
    pub score: f32,
}

/// Name key for a track derived from its FILENAME only (no tag read — cheap). Uses the
/// filename parser when the name is clean, else normalizes the whole stem.
fn key_for_path(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match naming::parse_filename(stem) {
        Some((a, t, _)) => naming::name_key(&a, &t),
        None => naming::name_key("", stem),
    }
}

/// Pending track ids whose name key collides with another pending or filed track. Pure
/// string work over the `tracks` table — no file I/O, no migration. Drives the queue badge.
pub fn name_dups(conn: &Connection) -> rusqlite::Result<HashSet<i64>> {
    let mut stmt =
        conn.prepare("SELECT id, path, status FROM tracks WHERE status IN ('pending','filed')")?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    // key -> list of (id, is_pending)
    let mut groups: HashMap<String, Vec<(i64, bool)>> = HashMap::new();
    for (id, path, status) in rows {
        groups
            .entry(key_for_path(&path))
            .or_default()
            .push((id, status == "pending"));
    }
    let mut dups = HashSet::new();
    for (_key, group) in groups {
        if group.len() >= 2 {
            for (id, is_pending) in group {
                if is_pending {
                    dups.insert(id);
                }
            }
        }
    }
    Ok(dups)
}

/// The best duplicate match for `track_id` by name (other pending or filed track sharing its
/// name key). `None` if no name collides. Slice A returns `kind = "name"`; the acoustic layer
/// (slice B) upgrades to `both` when the sound confirms.
pub fn find_duplicate(conn: &Connection, track_id: i64) -> rusqlite::Result<Option<DupMatch>> {
    let path: String = match conn
        .query_row("SELECT path FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
    {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let key = key_for_path(&path);

    let mut stmt = conn.prepare(
        "SELECT id, path, status, folder, filename FROM tracks
         WHERE status IN ('pending','filed') AND id<>?1",
    )?;
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![track_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Prefer a filed match (it's "already in your library") over another pending one.
    let mut best: Option<(i64, String, Option<String>, Option<String>)> = None;
    for (id, cand_path, status, folder, filename) in rows {
        if key_for_path(&cand_path) != key {
            continue;
        }
        let is_filed = status == "filed";
        let take = match &best {
            None => true,
            Some((_, bstatus, _, _)) => is_filed && bstatus != "filed",
        };
        if take {
            best = Some((id, status, folder, filename));
            // a filed match is the strongest by-name signal; stop early
            if is_filed {
                break;
            }
        }
    }

    Ok(best.map(|(id, status, folder, filename)| DupMatch {
        id,
        status,
        folder,
        filename,
        kind: "name".to_string(),
        score: 1.0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    fn add(conn: &Connection, path: &str, status: &str) -> i64 {
        conn.execute(
            "INSERT INTO tracks(path, filename, status) VALUES(?1, ?2, ?3)",
            params![path, Path::new(path).file_name().and_then(|n| n.to_str()), status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn name_dups_flags_pending_homonyms() {
        let conn = db();
        let a = add(&conn, "/dl/Larry Heard - Mystery of Love.mp3", "pending");
        let b = add(&conn, "/dl/larry_heard mystery of love.flac", "pending");
        let _c = add(&conn, "/dl/Chez Damier - Can You Feel It.aiff", "pending");
        let dups = name_dups(&conn).unwrap();
        assert!(dups.contains(&a) && dups.contains(&b));
        assert_eq!(dups.len(), 2); // c is unique
    }

    #[test]
    fn name_dups_flags_pending_against_filed() {
        let conn = db();
        let p = add(&conn, "/dl/Theo Parrish - Falling Up.mp3", "pending");
        let _f = add(&conn, "/lib/Theo Parrish - Falling Up.aiff", "filed");
        let dups = name_dups(&conn).unwrap();
        assert!(dups.contains(&p));
        assert_eq!(dups.len(), 1); // only the pending one is flagged
    }

    #[test]
    fn find_duplicate_prefers_filed_match() {
        let conn = db();
        let cur = add(&conn, "/dl/Theo Parrish - Falling Up.mp3", "pending");
        let _other_pending = add(&conn, "/dl2/theo parrish falling up.wav", "pending");
        conn.execute("UPDATE tracks SET folder='House' WHERE path='/lib/x.aiff'", []).ok();
        let filed = add(&conn, "/lib/Theo Parrish - Falling Up.aiff", "filed");
        conn.execute("UPDATE tracks SET folder='House' WHERE id=?1", params![filed]).unwrap();

        let m = find_duplicate(&conn, cur).unwrap().unwrap();
        assert_eq!(m.id, filed);
        assert_eq!(m.status, "filed");
        assert_eq!(m.folder.as_deref(), Some("House"));
        assert_eq!(m.kind, "name");
    }

    #[test]
    fn find_duplicate_none_when_unique() {
        let conn = db();
        let cur = add(&conn, "/dl/Unique Artist - Unique Title.mp3", "pending");
        add(&conn, "/dl/Someone Else - Other Song.mp3", "pending");
        assert!(find_duplicate(&conn, cur).unwrap().is_none());
    }
}
