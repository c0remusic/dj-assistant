//! Typed access to the `settings(key, value)` table: the few app-wide preferences the
//! filing loop needs (library root, filename template, trash purge window). String values
//! only; callers parse as needed. Created in migration v4.

use rusqlite::{params, Connection};

/// Absolute path of the library root under which bins live.
pub const LIBRARY_ROOT: &str = "library_root";
/// Output filename template (placeholders {artist} {title} {version}).
pub const FILENAME_TEMPLATE: &str = "filename_template";
/// Days a trashed file is kept in `.sift-trash` before purge.
pub const TRASH_PURGE_DAYS: &str = "trash_purge_days";
/// Discogs personal access token (entered in Réglages). Empty/unset = identification disabled.
pub const DISCOGS_TOKEN: &str = "discogs_token";
/// Key under which the current session's unique ID is stored at app launch.
/// Written once at startup; read by the `actions` INSERT via SQL subquery.
pub const CURRENT_SESSION_ID: &str = "current_session_id";

/// The default filename template when the setting is unset.
pub const DEFAULT_TEMPLATE: &str = "{artist} - {title}{version}";

/// Read a setting, or None if unset.
pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key=?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// Read a setting or fall back to `default`.
pub fn get_or(conn: &Connection, key: &str, default: &str) -> rusqlite::Result<String> {
    Ok(get(conn, key)?.unwrap_or_else(|| default.to_string()))
}

/// Upsert a setting.
pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
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
    fn get_missing_is_none() {
        let conn = db();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), None);
    }

    #[test]
    fn set_then_get_round_trips() {
        let conn = db();
        set(&conn, LIBRARY_ROOT, "/music/dj").unwrap();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), Some("/music/dj".to_string()));
    }

    #[test]
    fn set_overwrites() {
        let conn = db();
        set(&conn, LIBRARY_ROOT, "/a").unwrap();
        set(&conn, LIBRARY_ROOT, "/b").unwrap();
        assert_eq!(get(&conn, LIBRARY_ROOT).unwrap(), Some("/b".to_string()));
    }

    #[test]
    fn get_or_falls_back() {
        let conn = db();
        assert_eq!(get_or(&conn, FILENAME_TEMPLATE, DEFAULT_TEMPLATE).unwrap(), DEFAULT_TEMPLATE);
    }
}
