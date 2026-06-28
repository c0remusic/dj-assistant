use rusqlite::Connection;

/// Ordered list of migrations. Index + 1 == the schema version it brings the DB to.
/// NEVER reorder or edit an existing entry once shipped — only append.
const MIGRATIONS: &[&str] = &[
    // v1 — initial schema (matches docs/plan-implementation.md "Données")
    r#"
    CREATE TABLE tracks (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        hash TEXT,
        fingerprint TEXT,
        format TEXT,
        bitrate INTEGER,
        duration REAL,
        declared_fmt TEXT,
        real_quality TEXT,
        verdict TEXT,                 -- ok | fake | grey
        status TEXT NOT NULL DEFAULT 'pending', -- pending | filed | resourcing | trash
        folder TEXT,
        clip_runs INTEGER,
        clip_pct REAL,
        true_peak_dbtp REAL,
        dc_offset REAL,
        phase_correlation REAL,
        truncated INTEGER,            -- bool 0/1
        silence_head_ms INTEGER,
        silence_tail_ms INTEGER,
        has_cover INTEGER,            -- bool 0/1
        tags_cdj_ok INTEGER,          -- bool 0/1
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE metadata (
        track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        artist TEXT, title TEXT, label TEXT, year INTEGER,
        genre TEXT, bpm INTEGER, cover_path TEXT,
        discogs_release_id TEXT, source TEXT
    );
    CREATE TABLE custom_tags (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        tag TEXT NOT NULL,
        PRIMARY KEY (track_id, tag)
    );
    CREATE TABLE actions (
        id INTEGER PRIMARY KEY,
        track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
        type TEXT NOT NULL,           -- convert | move | trash | reject
        from_path TEXT,
        to_path TEXT,
        ts TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE sources (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        watched INTEGER NOT NULL DEFAULT 1  -- bool 0/1
    );
    "#,
    // v2 — M1 watcher/queue: link tracks to a source + cheap "seen" identity (size+mtime)
    r#"
    ALTER TABLE tracks ADD COLUMN source_id INTEGER REFERENCES sources(id) ON DELETE CASCADE;
    ALTER TABLE tracks ADD COLUMN filename TEXT;
    ALTER TABLE tracks ADD COLUMN size_bytes INTEGER;
    ALTER TABLE tracks ADD COLUMN mtime INTEGER;
    ALTER TABLE sources ADD COLUMN created_at TEXT;
    CREATE INDEX idx_tracks_source ON tracks(source_id);
    CREATE INDEX idx_tracks_status ON tracks(status);
    "#,
    // v3 — M2b analysis worker: report columns missing from v1 + the "analyzed" marker.
    r#"
    ALTER TABLE tracks ADD COLUMN cutoff_hz REAL;
    ALTER TABLE tracks ADD COLUMN dual_mono INTEGER;     -- 0/1
    ALTER TABLE tracks ADD COLUMN container_ok INTEGER;  -- 0/1
    ALTER TABLE tracks ADD COLUMN codec_error TEXT;
    ALTER TABLE tracks ADD COLUMN id3_version TEXT;
    ALTER TABLE tracks ADD COLUMN analyzed_at TEXT;      -- NULL = not yet analysed
    CREATE INDEX idx_tracks_analyzed ON tracks(analyzed_at);
    "#,
    // v4 — M4 filing loop: per-track target/confidence, version metadata, undo bookkeeping
    // on actions, and a key/value settings store (library root, filename template, purge).
    r#"
    ALTER TABLE tracks ADD COLUMN target_format TEXT;     -- 'mp3_320' | 'aiff_16_44' | 'wav_16_44'
    ALTER TABLE tracks ADD COLUMN confidence TEXT;        -- 'green' | 'yellow'
    ALTER TABLE metadata ADD COLUMN version TEXT;         -- 'Original Mix', 'Remix'…
    ALTER TABLE actions ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;  -- 0/1
    ALTER TABLE actions ADD COLUMN batch_id TEXT;         -- groups one filing's rows
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
    // v5 — cache the full analysis report (JSON, sans spectrogram) so re-opening an already-
    // analysed track is instant (no re-decode). Cleared by the scanner when a file changes.
    r#"
    ALTER TABLE tracks ADD COLUMN report_json TEXT;
    "#,
    // v6 — M6a Discogs identification: per-track sub-genres (Discogs "style"), multiple per
    // track, ordered. metadata.genre stays for back-compat but track_genres is the source.
    r#"
    CREATE TABLE track_genres (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        genre    TEXT NOT NULL,
        ord      INTEGER NOT NULL,
        PRIMARY KEY (track_id, genre)
    );
    CREATE INDEX idx_track_genres_track ON track_genres(track_id);
    "#,
    // v7 — revertable "Apply ID3 tags": a free-form JSON column on the journal where the
    // tag_edit action stores the OLD tags captured before the write, so a revert can restore
    // them. Other action types leave it NULL.
    r#"
    ALTER TABLE actions ADD COLUMN meta TEXT;
    "#,
];

/// Applies any migrations the DB hasn't seen yet, tracked via PRAGMA user_version.
/// Idempotent: running twice is a no-op the second time.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current {
            conn.execute_batch(sql)?;
            conn.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        }
    }
    Ok(())
}

/// Opens (creating if needed) the DB at `path`, enables foreign keys, runs migrations.
pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL + a busy timeout so concurrent access waits instead of erroring (prep for moving
    // off the single-connection model; harmless with one connection today).
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
    )?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Current schema version (PRAGMA user_version).
pub fn schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

/// Count of user tables (excludes sqlite internal tables).
pub fn table_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrations_bring_db_to_latest_version() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_create_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(table_count(&conn).unwrap(), 7); // v4 adds `settings`, v6 adds `track_genres`
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // second run must not error or duplicate
        assert_eq!(table_count(&conn).unwrap(), 7);
    }

    #[test]
    fn migrations_reach_v2() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
        assert!(MIGRATIONS.len() >= 2, "M1 adds migration v2");
    }

    #[test]
    fn tracks_has_m1_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["source_id", "filename", "size_bytes", "mtime"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn tracks_has_m2b_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["cutoff_hz", "dual_mono", "container_ok", "codec_error", "id3_version", "analyzed_at"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn tracks_has_m4_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["target_format", "confidence"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn actions_and_settings_have_m4_shape() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["undone", "batch_id"] {
            assert!(acols.contains(&c.to_string()), "actions missing column {c}");
        }
        // settings table exists and is writable
        conn.execute("INSERT INTO settings(key,value) VALUES('k','v')", [])
            .expect("settings table usable");
    }

    #[test]
    fn actions_has_v7_meta_column() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(acols.contains(&"meta".to_string()), "actions missing column meta");
    }
}
