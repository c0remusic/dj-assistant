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
