//! Per-track sub-genre list (Discogs "style"), stored ordered in `track_genres`. Replacing a
//! track's genres is a full delete+insert so re-identifying never accumulates stale rows.

use rusqlite::{params, Connection};

/// Replace a track's genre list with `genres` (ordered). Empty `genres` clears them.
pub fn set_genres(conn: &Connection, track_id: i64, genres: &[String]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM track_genres WHERE track_id=?1", params![track_id])?;
    for (ord, g) in genres.iter().enumerate() {
        let g = g.trim();
        if g.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO track_genres(track_id, genre, ord) VALUES(?1,?2,?3)",
            params![track_id, g, ord as i64],
        )?;
    }
    Ok(())
}

/// A track's genres, ordered by `ord`.
pub fn get_genres(conn: &Connection, track_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT genre FROM track_genres WHERE track_id=?1 ORDER BY ord")?;
    let rows = stmt.query_map(params![track_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Batch equivalent of `get_genres` for many tracks in ONE query (FIX-22) — used by
/// `library::list_filed`, which used to call `get_genres` once per row, growing linearly with
/// the library size on every filtered fetch.
pub fn get_genres_batch(
    conn: &Connection,
    track_ids: &[i64],
) -> rusqlite::Result<std::collections::HashMap<i64, Vec<String>>> {
    let mut out: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    if track_ids.is_empty() {
        return Ok(out);
    }
    let placeholders = vec!["?"; track_ids.len()].join(",");
    let sql = format!(
        "SELECT track_id, genre FROM track_genres WHERE track_id IN ({placeholders}) ORDER BY track_id, ord"
    );
    let mut stmt = conn.prepare(&sql)?;
    let bound: Vec<&dyn rusqlite::ToSql> = track_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(bound.as_slice(), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (track_id, genre) = row?;
        out.entry(track_id).or_default().push(genre);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        // a track row to satisfy the FK
        conn.execute("INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')", []).unwrap();
        conn
    }

    #[test]
    fn set_then_get_round_trips_in_order() {
        let conn = db();
        set_genres(&conn, 1, &["Deep House".into(), "House".into()]).unwrap();
        assert_eq!(get_genres(&conn, 1).unwrap(), vec!["Deep House".to_string(), "House".to_string()]);
    }

    #[test]
    fn re_set_replaces_without_accumulating() {
        let conn = db();
        set_genres(&conn, 1, &["Techno".into(), "Acid".into()]).unwrap();
        set_genres(&conn, 1, &["Ambient".into()]).unwrap();
        assert_eq!(get_genres(&conn, 1).unwrap(), vec!["Ambient".to_string()]);
    }

    #[test]
    fn get_missing_is_empty() {
        let conn = db();
        assert_eq!(get_genres(&conn, 1).unwrap(), Vec::<String>::new());
    }
}
