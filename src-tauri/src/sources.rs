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
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
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
