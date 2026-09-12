//! Watched-folder records. The queue counts hang off these.
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;

/// Strips Windows verbatim/extended-length prefixes (`\\?\C:\…`, `\\?\UNC\…`) that
/// `std::fs::canonicalize` emits — keeps stored paths consistent with what `notify`
/// reports for live events (it can't watch verbatim paths).
fn strip_verbatim(p: &str) -> String {
    if let Some(rest) = p.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = p.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        p.to_string()
    }
}

/// A watched folder as shown on the Accueil screen.
#[derive(Debug, Serialize, PartialEq)]
pub struct Source {
    pub id: i64,
    pub path: String,
    pub pending_count: i64,
    /// Total de fichiers reconnus par le scan, TOUS statuts confondus — 0 signifie « aucun
    /// fichier audio reconnu ici » (issue #55, badge « 0 audio » du rail), un état qu'un
    /// `pending_count` à 0 ne sait pas distinguer d'un dossier entièrement traité.
    pub track_count: i64,
    pub accessible: bool,
    pub watched: bool,
    pub color_key: Option<String>,
}

/// The 5 categorical hue keys a source colour override may take. EXACT mirror of
/// `SOURCE_HUE_CYCLE` (`frontend/source-color.ts`) and of the `.sift-rail-src-dot-<hue>`
/// classes in `styles.css` — pinned frontend-side by `test/source-color.test.ts`, Rust-side
/// by `hue_keys_mirror_frontend_cycle` below (reads the .ts file, order included). The taxonomy
/// is a closed set (`DESIGN.md` § 4): any value outside it is rejected at the IPC boundary by
/// `validate_color_key`, never stored.
pub const SOURCE_HUE_KEYS: [&str; 5] = ["indigo", "purple", "pink", "teal", "yellow"];

/// Rejects a colour override that isn't one of the 5 hue keys. `None` (clear the override,
/// back to add-order auto-assignment) is always valid. Pure so the IPC boundary stays testable
/// without a Tauri `State`; called by `ipc::set_source_color` before anything reaches the DB.
///
/// Why this exists: the `color_key` column has no `CHECK` (`db.rs`), and the only guard was the
/// frontend menu — a raw IPC caller could write arbitrary text into the base. `esc()` closes the
/// render path (`frontend/rail-source-entry.ts`), but the base itself was unguarded.
pub fn validate_color_key(color_key: Option<&str>) -> Result<(), String> {
    match color_key {
        None => Ok(()),
        Some(key) if SOURCE_HUE_KEYS.contains(&key) => Ok(()),
        Some(key) => Err(format!(
            "couleur de source invalide: {key:?} — attendu l'une de {SOURCE_HUE_KEYS:?} ou null"
        )),
    }
}

/// Canonicalises `path` (so disk-scan and live-watch keys stay consistent), inserts it,
/// and returns the new source id. If the path is already a source, returns the existing id.
pub fn add(conn: &Connection, path: &str) -> rusqlite::Result<i64> {
    let canon = std::fs::canonicalize(path)
        .map(|p| strip_verbatim(&p.to_string_lossy()))
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
                (SELECT count(*) FROM tracks t WHERE t.source_id=s.id AND t.status='pending'),
                (SELECT count(*) FROM tracks t WHERE t.source_id=s.id),
                s.watched, s.color_key
         FROM sources s ORDER BY s.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let path: String = r.get(1)?;
        let accessible = Path::new(&path).is_dir();
        Ok(Source {
            id: r.get(0)?,
            path,
            pending_count: r.get(2)?,
            track_count: r.get(3)?,
            accessible,
            watched: r.get::<_, i64>(4)? != 0,
            color_key: r.get(5)?,
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

/// Toggles live watching for a source (persists the `watched` flag). Returns the source path
/// so the caller can start/stop the live watcher.
pub fn set_watched(conn: &Connection, id: i64, watched: bool) -> rusqlite::Result<String> {
    conn.execute(
        "UPDATE sources SET watched=?2 WHERE id=?1",
        rusqlite::params![id, watched as i64],
    )?;
    conn.query_row(
        "SELECT path FROM sources WHERE id=?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
}

/// Sets (or clears, with `None`) a source's manual color override. Raw store: the value is
/// validated upstream by `validate_color_key` at the IPC boundary (`ipc::set_source_color`),
/// so callers must go through there — this function trusts its input and just writes it.
pub fn set_color(conn: &Connection, id: i64, color_key: Option<String>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sources SET color_key=?2 WHERE id=?1",
        rusqlite::params![id, color_key],
    )?;
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
        assert_eq!(sources[0].track_count, 2); // both, whatever their status
    }

    /// Issue #55 : un dossier surveillé sans AUCUN fichier reconnu doit être distinguable d'un
    /// dossier entièrement traité — les deux ont `pending_count = 0`, seul `track_count` les
    /// sépare (0 contre > 0).
    #[test]
    fn track_count_zero_distinguishes_unrecognized_folder_from_processed_one() {
        let conn = db();
        let empty = add(&conn, ".").unwrap();
        let processed = add(&conn, "..").unwrap();
        conn.execute(
            "INSERT INTO tracks (path, source_id, status) VALUES ('p/x.mp3', ?1, 'filed')",
            rusqlite::params![processed],
        )
        .unwrap();
        let sources = list(&conn).unwrap();
        let e = sources.iter().find(|s| s.id == empty).unwrap();
        let p = sources.iter().find(|s| s.id == processed).unwrap();
        assert_eq!((e.pending_count, e.track_count), (0, 0));
        assert_eq!((p.pending_count, p.track_count), (0, 1));
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

    #[test]
    fn set_color_persists_and_reads_back() {
        let conn = db();
        let id = add(&conn, ".").unwrap();
        set_color(&conn, id, Some("teal".to_string())).unwrap();
        let sources = list(&conn).unwrap();
        let s = sources.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s.color_key.as_deref(), Some("teal"));
    }

    #[test]
    fn color_defaults_to_none() {
        let conn = db();
        let id = add(&conn, ".").unwrap();
        let sources = list(&conn).unwrap();
        let s = sources.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s.color_key, None);
    }

    #[test]
    fn validate_color_key_accepts_none_and_the_five_hues() {
        assert!(
            validate_color_key(None).is_ok(),
            "clear the override is always valid"
        );
        for hue in SOURCE_HUE_KEYS {
            assert!(
                validate_color_key(Some(hue)).is_ok(),
                "{hue} is a cycle key"
            );
        }
    }

    #[test]
    fn validate_color_key_rejects_anything_else() {
        // The frontend menu only offers the 5 keys, but a raw IPC caller isn't bound by it —
        // and the DB column has no CHECK. This boundary is what keeps the base clean.
        for bad in ["", "INDIGO", "red", "\"><script>x</script>", "indigo "] {
            let r = validate_color_key(Some(bad));
            assert!(r.is_err(), "{bad:?} must be rejected");
            assert!(
                r.unwrap_err().contains(bad),
                "the error names the offending value"
            );
        }
    }

    /// `SOURCE_HUE_KEYS` and the frontend `SOURCE_HUE_CYCLE` are one taxonomy in two languages.
    /// This reads the .ts source and pins them equal, ORDER INCLUDED — same discipline as the
    /// serde-mirror contract tests. A rename on either side (like 57f64b2, which renamed the
    /// family) breaks this instead of silently splitting validation from the rendered class.
    #[test]
    fn hue_keys_mirror_frontend_cycle() {
        let ts_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri/ a toujours un parent (la racine du dépôt)")
            .join("frontend")
            .join("source-color.ts");
        let ts = std::fs::read_to_string(&ts_path)
            .unwrap_or_else(|e| panic!("lecture de {} impossible: {e}", ts_path.display()));

        // `export const SOURCE_HUE_CYCLE = ["indigo", "purple", …] as const;` — on isole le
        // contenu entre crochets, sa disparition doit casser, pas passer inaperçue.
        let after = ts
            .split("SOURCE_HUE_CYCLE")
            .nth(1)
            .unwrap_or_else(|| panic!("SOURCE_HUE_CYCLE absent de {}", ts_path.display()));
        let inside = after
            .split_once('[')
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(list, _)| list)
            .unwrap_or_else(|| panic!("SOURCE_HUE_CYCLE n'est pas un littéral de tableau"));
        let ts_keys: Vec<String> = inside
            .split(',')
            .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(
            ts_keys,
            SOURCE_HUE_KEYS.to_vec(),
            "SOURCE_HUE_KEYS (Rust) et SOURCE_HUE_CYCLE (frontend/source-color.ts) ont divergé. \
             C'est la MÊME taxonomie fermée (DESIGN.md § 4) — la valider ici et la rendre là-bas. \
             Les remettre d'accord, ordre compris."
        );
    }
}
