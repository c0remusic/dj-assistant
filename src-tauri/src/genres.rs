//! Per-track sub-genre list (Discogs "style"), stored ordered in `track_genres`. Replacing a
//! track's genres is a full delete+insert so re-identifying never accumulates stale rows.

use rusqlite::{params, Connection};

/// Replace a track's genre list with `genres` (ordered). Empty `genres` clears them.
pub fn set_genres(conn: &Connection, track_id: i64, genres: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM track_genres WHERE track_id=?1",
        params![track_id],
    )?;
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

/// Max track_ids bound in a single `IN (?,?,...)` query in `get_genres_batch`. SQLite caps the
/// number of bound parameters per prepared statement (measured at 32766 on this build, and as
/// low as 999 on builds using SQLite's older default `SQLITE_MAX_VARIABLE_NUMBER`); a library
/// with more `filed` tracks than that used to make `get_genres_batch` — and therefore
/// `library::list_filed`, its only caller — return an Err instead of results. Comfortably under
/// both limits.
const GENRE_BATCH_CHUNK_SIZE: usize = 500;

/// Batch equivalent of `get_genres` for many tracks (FIX-22) — used by `library::list_filed`,
/// which used to call `get_genres` once per row, growing linearly with the library size on every
/// filtered fetch. Runs one query per `GENRE_BATCH_CHUNK_SIZE`-sized chunk of `track_ids` to stay
/// under SQLite's bound-parameter limit; results are merged into a single map, so callers see the
/// same shape regardless of `track_ids.len()`.
pub fn get_genres_batch(
    conn: &Connection,
    track_ids: &[i64],
) -> rusqlite::Result<std::collections::HashMap<i64, Vec<String>>> {
    let mut out: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    if track_ids.is_empty() {
        return Ok(out);
    }
    for chunk in track_ids.chunks(GENRE_BATCH_CHUNK_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "SELECT track_id, genre FROM track_genres WHERE track_id IN ({placeholders}) ORDER BY track_id, ord"
        );
        let mut stmt = conn.prepare(&sql)?;
        let bound: Vec<&dyn rusqlite::ToSql> =
            chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(bound.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (track_id, genre) = row?;
            out.entry(track_id).or_default().push(genre);
        }
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
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn set_then_get_round_trips_in_order() {
        let conn = db();
        set_genres(&conn, 1, &["Deep House".into(), "House".into()]).unwrap();
        assert_eq!(
            get_genres(&conn, 1).unwrap(),
            vec!["Deep House".to_string(), "House".to_string()]
        );
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

    #[test]
    fn get_genres_batch_returns_empty_for_empty_input() {
        let conn = db();
        let out = get_genres_batch(&conn, &[]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn get_genres_batch_small_volume_ordered_per_track() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(2,'/y.flac','pending')",
            [],
        )
        .unwrap();
        set_genres(&conn, 1, &["Deep House".into(), "House".into()]).unwrap();
        set_genres(&conn, 2, &["Techno".into()]).unwrap();
        let out = get_genres_batch(&conn, &[1, 2]).unwrap();
        assert_eq!(
            out.get(&1).unwrap(),
            &vec!["Deep House".to_string(), "House".to_string()]
        );
        assert_eq!(out.get(&2).unwrap(), &vec!["Techno".to_string()]);
    }

    /// Reproduces the bug: a single `IN (?,?,?...)` query binds one parameter per track_id,
    /// which blows past SQLite's bound-parameter limit for large libraries (measured at 32766
    /// on this build) and returns an Err instead of results — this silently broke
    /// `library::list_filed` for any library with > ~33k `filed` tracks.
    #[test]
    fn get_genres_batch_handles_more_than_sqlite_param_limit() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        // Explicitly OFF (this build enforces FKs by default) so track_genres rows don't need a
        // matching `tracks` row — keeps this test fast at 35k rows; irrelevant to the bug under test.
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        const N: i64 = 35_000;
        let track_ids: Vec<i64> = (1..=N).collect();

        let tx = conn.unchecked_transaction().unwrap();
        for id in 1..=100i64 {
            tx.execute(
                "INSERT INTO track_genres(track_id, genre, ord) VALUES(?1,'House',0)",
                params![id],
            )
            .unwrap();
        }
        tx.commit().unwrap();

        let out = get_genres_batch(&conn, &track_ids).unwrap();
        assert_eq!(out.len(), 100);
        for id in 1..=100i64 {
            assert_eq!(out.get(&id).unwrap(), &vec!["House".to_string()]);
        }
        for id in 101..=N {
            assert!(!out.contains_key(&id));
        }
    }

    /// Chunk-boundary cases: exactly one chunk, one-past-a-chunk-boundary, and a single element —
    /// none of these should behave differently from the "obviously small" or "obviously huge" cases.
    #[test]
    fn get_genres_batch_chunk_boundaries() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();

        for n in [
            1i64,
            GENRE_BATCH_CHUNK_SIZE as i64,
            GENRE_BATCH_CHUNK_SIZE as i64 + 1,
        ] {
            let track_ids: Vec<i64> = (1..=n).collect();
            let tx = conn.unchecked_transaction().unwrap();
            tx.execute("DELETE FROM track_genres", []).unwrap();
            for id in &track_ids {
                tx.execute(
                    "INSERT INTO track_genres(track_id, genre, ord) VALUES(?1,'Ambient',0)",
                    params![id],
                )
                .unwrap();
            }
            tx.commit().unwrap();

            let out = get_genres_batch(&conn, &track_ids).unwrap();
            assert_eq!(out.len() as i64, n, "n={n}");
            for id in &track_ids {
                assert_eq!(
                    out.get(id).unwrap(),
                    &vec!["Ambient".to_string()],
                    "n={n} id={id}"
                );
            }
        }
    }
}
