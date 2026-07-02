//! Read model for the "to process" queue = tracks WHERE status='pending'.
use rusqlite::Connection;
use serde::Serialize;

/// One row in the live queue. `verdict` is NULL until the worker (M2b) analyses it.
#[derive(Debug, Serialize, PartialEq)]
pub struct QueueItem {
    pub id: i64,
    pub path: String,
    pub filename: Option<String>,
    pub source_id: Option<i64>,
    pub verdict: Option<String>,
    /// Declared rail ("lossless" | "lossy" | "unknown"), NULL until analysed. Drives the batch
    /// grouping + output format (lossless → AIFF, lossy → MP3 320). Stored in `real_quality`.
    pub rail: Option<String>,
    /// Identified artist/title from the `metadata` table (NULL until identified). Lets the batch
    /// list show the file's name BEFORE (filename) next to the Discogs name AFTER.
    pub artist: Option<String>,
    pub title: Option<String>,
    /// True when this track shares a name with another pending/filed track (dedup name
    /// pre-filter). Set by the IPC layer (see ipc::list_queue), default false.
    #[serde(default)]
    pub dup: bool,
}

/// All pending tracks, oldest first.
pub fn list_pending(conn: &Connection) -> rusqlite::Result<Vec<QueueItem>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.filename, t.source_id, t.verdict, t.real_quality, m.artist, m.title
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id
         WHERE t.status='pending' ORDER BY t.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(QueueItem {
            id: r.get(0)?,
            path: r.get(1)?,
            filename: r.get(2)?,
            source_id: r.get(3)?,
            verdict: r.get(4)?,
            rail: r.get(5)?,
            artist: r.get(6)?,
            title: r.get(7)?,
            dup: false,
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
