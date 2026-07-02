//! The destination bins: every subdirectory (recursive) under the configured library
//! root. Walks the tree with `walkdir`, skipping hidden dirs (e.g. the `.sift-trash`
//! corbeille). Also creates new bins and resolves collision-free destination paths. Pure
//! filesystem work; the root path comes from `settings::LIBRARY_ROOT`.
//!
//! Also exposes `list_filed` / `folder_facets` for the M6b library browser (read-only
//! DB queries over the `filed` tracks).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ── M6b library browser ──────────────────────────────────────────────────────

/// A filed track for the library browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTrack {
    pub id: i64,
    pub path: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub duration: Option<f64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub label: Option<String>,
    pub genres: Vec<String>,
    pub discogs_release_id: Option<String>,
    pub cover_path: Option<String>,
    pub has_cover: bool,
    pub verdict: Option<String>,
    pub folder: Option<String>,
}

/// Server-side filters for the library list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFilter {
    /// Restrict to one folder (exact match on `tracks.folder`).
    pub folder: Option<String>,
    /// `lossless` (aiff/wav/flac/aif) or `mp3`; `None`/other = all.
    pub quality: Option<String>,
    /// Restrict by genre (exact, via track_genres).
    pub genre: Option<String>,
    /// Free text over artist/title/path (case-insensitive contains).
    pub q: Option<String>,
}

/// A facet bucket (folder or genre) with its filed-track count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFolder {
    pub name: String,
    pub count: i64,
}

/// Both facet lists for the library sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFacets {
    pub folders: Vec<LibraryFolder>,
    pub genres: Vec<LibraryFolder>,
}

/// All `filed` tracks joined to their metadata + genres, filtered. Read-only.
pub fn list_filed(
    conn: &rusqlite::Connection,
    f: &LibraryFilter,
) -> rusqlite::Result<Vec<LibraryTrack>> {
    let mut sql = String::from(
        "SELECT t.id, t.path, t.format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
                m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed'",
    );
    if f.folder.is_some() {
        sql.push_str(" AND t.folder = :folder");
    }
    if let Some(q) = &f.quality {
        match q.as_str() {
            "lossless" => sql.push_str(" AND lower(t.format) IN ('aiff','aif','wav','flac')"),
            "mp3" => sql.push_str(" AND lower(t.format) = 'mp3'"),
            _ => {}
        }
    }
    if f.q.is_some() {
        sql.push_str(" AND (m.artist LIKE :like OR m.title LIKE :like OR t.path LIKE :like)");
    }
    if f.genre.is_some() {
        sql.push_str(" AND t.id IN (SELECT track_id FROM track_genres WHERE genre = :genre)");
    }
    sql.push_str(" ORDER BY m.artist, m.title, t.path");

    let like = f.q.as_ref().map(|q| format!("%{q}%"));
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<(&str, &dyn rusqlite::ToSql)> = {
        let mut p: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
        if let Some(folder) = &f.folder {
            p.push((":folder", folder));
        }
        if let Some(l) = &like {
            p.push((":like", l));
        }
        if let Some(g) = &f.genre {
            p.push((":genre", g));
        }
        p
    };
    let rows = stmt
        .query_map(params.as_slice(), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<i64>>(11)?,
                r.get::<_, Option<i64>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // FIX-22: one batched genres query for every row instead of one query per row.
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let mut genres_by_track = crate::genres::get_genres_batch(conn, &ids)?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, path, format, bitrate, duration, verdict, folder, has_cover, artist, title, label, year, bpm, cover_path, rel) in rows {
        out.push(LibraryTrack {
            id,
            path,
            artist,
            title,
            format,
            bitrate,
            duration,
            bpm,
            year,
            label,
            genres: genres_by_track.remove(&id).unwrap_or_default(),
            discogs_release_id: rel,
            cover_path,
            has_cover: has_cover.unwrap_or(0) != 0,
            verdict,
            folder,
        });
    }
    Ok(out)
}

/// Counts of `filed` tracks grouped by folder and by genre. Read-only.
pub fn folder_facets(conn: &rusqlite::Connection) -> rusqlite::Result<LibraryFacets> {
    let folders = query_facets(
        conn,
        "SELECT folder, COUNT(*) FROM tracks \
         WHERE status='filed' AND folder IS NOT NULL AND folder <> '' \
         GROUP BY folder ORDER BY folder",
    )?;
    let genres = query_facets(
        conn,
        "SELECT g.genre, COUNT(*) FROM track_genres g \
         JOIN tracks t ON t.id = g.track_id AND t.status='filed' \
         GROUP BY g.genre ORDER BY g.genre",
    )?;
    Ok(LibraryFacets { folders, genres })
}

fn query_facets(
    conn: &rusqlite::Connection,
    sql: &str,
) -> rusqlite::Result<Vec<LibraryFolder>> {
    let mut stmt = conn.prepare(sql)?;
    let mapped = stmt.query_map([], |r| {
        Ok(LibraryFolder {
            name: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    mapped.collect()
}

/// One destination folder under the library root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bin {
    /// Path relative to the root, forward-slash separated (e.g. "House/Deep").
    pub rel: String,
    /// Display name = last path component (e.g. "Deep").
    pub name: String,
    /// Nesting depth under root (1 = direct child).
    pub depth: usize,
}

/// Whether a directory name is hidden (leading dot) — excluded from bins.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// List all bins (recursive subdirectories) under `root`, sorted by relative path. Returns
/// an empty list if root doesn't exist. Hidden directories and their subtrees are skipped.
pub fn list_bins(root: &Path) -> Vec<Bin> {
    let mut bins = Vec::new();
    let walker = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            // skip hidden dirs entirely (prunes their subtree too)
            !e.file_name().to_str().map(is_hidden).unwrap_or(false)
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel = rel_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if rel.is_empty() {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or_default().to_string();
        let depth = entry.depth();
        bins.push(Bin { rel, name, depth });
    }
    bins.sort_by(|a, b| a.rel.cmp(&b.rel));
    bins
}

/// Join `rel` under `root`, sanitizing every path segment and refusing anything that would
/// escape the root: `..`, absolute paths, or drive prefixes (both `/` and `\` separators
/// are accepted from the UI). Returns the contained absolute path. This is the single
/// containment guard every filesystem-mutating command must funnel destinations through —
/// `bin_rel` / `parent_rel` arrive from the (untrusted) webview and are otherwise free to
/// point anywhere (`..\..\Startup`, `C:\Windows\…`), which `Path::join` would honour.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for raw in rel.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." {
            return Err("path escapes the library root".into());
        }
        let safe = crate::naming::sanitize(raw);
        if safe.is_empty() {
            return Err("invalid path component".into());
        }
        out.push(safe);
    }
    Ok(out)
}

/// Create a new bin folder named `name` (sanitized) under `root/parent_rel`. `parent_rel`
/// "" means directly under root. Both `name` and every component of `parent_rel` are
/// sanitized and contained under `root` (see `safe_join`). Returns the created Bin.
pub fn create_bin(root: &Path, parent_rel: &str, name: &str) -> Result<Bin, String> {
    let safe = crate::naming::sanitize(name);
    if safe.is_empty() {
        return Err("empty bin name".into());
    }
    let abs = safe_join(root, parent_rel)?.join(&safe);
    std::fs::create_dir_all(&abs).map_err(|e| format!("create bin: {e}"))?;
    let rel = abs
        .strip_prefix(root)
        .map_err(|_| "bin outside root".to_string())?
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/");
    let depth = rel.split('/').count();
    Ok(Bin { rel, name: safe, depth })
}

/// True when `a` and `b` denote the same on-disk file. Prefers `canonicalize` (resolves
/// case/`.`/`..`/symlinks — needed on Windows where paths are case-insensitive), and falls
/// back to a plain `PathBuf` compare when either side can't be canonicalized (doesn't exist).
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Return a path that does not already exist, appending " (N)" before the extension when
/// the given path is taken. Used so filing never overwrites an existing file. `ignore` is an
/// optional "self" path that does NOT count as a collision — pass the source file when filing
/// in place so a conformant track keeps its own name instead of gaining a parasitic " (2)".
pub fn ensure_unique(path: &Path, ignore: Option<&Path>) -> PathBuf {
    let is_self = |p: &Path| ignore.is_some_and(|ig| same_path(p, ig));
    if !path.exists() || is_self(path) {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 2..10_000 {
        let candidate = match ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        };
        if !candidate.exists() || is_self(&candidate) {
            return candidate;
        }
    }
    // pathological fallback: timestamped name
    parent.join(format!("{stem} ({}).bak", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_recursive_bins_sorted_skipping_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House/Deep")).unwrap();
        std::fs::create_dir_all(root.join("House/Acid")).unwrap();
        std::fs::create_dir_all(root.join("Techno")).unwrap();
        std::fs::create_dir_all(root.join(".sift-trash/42")).unwrap();

        let bins = list_bins(root);
        let rels: Vec<&str> = bins.iter().map(|b| b.rel.as_str()).collect();
        assert_eq!(rels, vec!["House", "House/Acid", "House/Deep", "Techno"]);
        // hidden subtree excluded
        assert!(!rels.iter().any(|r| r.contains("sift-trash")));
        // depth + name sane
        let deep = bins.iter().find(|b| b.rel == "House/Deep").unwrap();
        assert_eq!(deep.name, "Deep");
        assert_eq!(deep.depth, 2);
    }

    #[test]
    fn missing_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nope");
        assert!(list_bins(&root).is_empty());
    }

    #[test]
    fn create_bin_makes_sanitized_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House")).unwrap();

        let bin = create_bin(root, "House", "Deep/Soulful?").unwrap();
        assert_eq!(bin.rel, "House/Deep Soulful"); // "/" and "?" sanitized to spaces→collapsed
        assert!(root.join("House/Deep Soulful").is_dir());
    }

    #[test]
    fn create_bin_at_root_level() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bin = create_bin(root, "", "Disco").unwrap();
        assert_eq!(bin.rel, "Disco");
        assert_eq!(bin.depth, 1);
        assert!(root.join("Disco").is_dir());
    }

    #[test]
    fn safe_join_contains_under_root() {
        let root = Path::new("C:/lib");
        // traversal (either separator) is refused
        assert!(safe_join(root, "../evil").is_err());
        assert!(safe_join(root, "House/../../x").is_err());
        assert!(safe_join(root, "..\\evil").is_err());
        // normal nested path is contained
        let j = safe_join(root, "House/Deep").unwrap();
        assert!(j.ends_with("Deep") && j.starts_with("C:/lib"));
        // an absolute/drive-prefixed rel is sanitized into components under root, not honoured
        let a = safe_join(root, "C:/Windows/System32").unwrap();
        assert!(a.starts_with("C:/lib"));
        // "" and "." resolve to the root itself
        assert_eq!(safe_join(root, "").unwrap(), root.to_path_buf());
    }

    #[test]
    fn create_bin_rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(create_bin(dir.path(), "../../etc", "evil").is_err());
    }

    #[test]
    fn ensure_unique_appends_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.mp3");
        // free → unchanged
        assert_eq!(ensure_unique(&base, None), base);
        // occupied → " (2)"
        std::fs::write(&base, b"x").unwrap();
        assert_eq!(ensure_unique(&base, None), dir.path().join("Track (2).mp3"));
    }

    #[test]
    fn ensure_unique_keeps_name_when_collision_is_the_ignored_self() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.aiff");
        std::fs::write(&base, b"x").unwrap();
        // the file exists, but it IS the source we're filing in place → keep the name, no " (2)"
        assert_eq!(ensure_unique(&base, Some(&base)), base);
    }

    // ── M6b library browser tests ────────────────────────────────────────────

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn list_filed_joins_metadata_and_genres() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, format, bitrate, duration, verdict, status, folder, has_cover) \
             VALUES(1, '/lib/House/a.aiff', 'aiff', 1411, 360.0, 'ok', 'filed', 'House', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, format, status) VALUES(2, '/in/pending.mp3', 'mp3', 'pending')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, label, year, bpm, cover_path, discogs_release_id) \
             VALUES(1, 'Mr Fingers', 'Can You Feel It', 'Trax', 1986, 120, '/cache/1.jpg', '12345')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["House".into(), "Deep House".into()]).unwrap();

        let rows = list_filed(&conn, &LibraryFilter::default()).unwrap();

        assert_eq!(rows.len(), 1, "only filed tracks");
        let t = &rows[0];
        assert_eq!(t.id, 1);
        assert_eq!(t.artist.as_deref(), Some("Mr Fingers"));
        assert_eq!(t.title.as_deref(), Some("Can You Feel It"));
        assert_eq!(t.format.as_deref(), Some("aiff"));
        assert_eq!(t.bitrate, Some(1411));
        assert_eq!(t.verdict.as_deref(), Some("ok"));
        assert_eq!(t.folder.as_deref(), Some("House"));
        assert_eq!(t.discogs_release_id.as_deref(), Some("12345"));
        assert_eq!(t.genres, vec!["House".to_string(), "Deep House".to_string()]);
    }

    #[test]
    fn folder_facets_counts_filed_by_folder_and_genre() {
        let conn = db();
        for (id, folder) in [(1, "House"), (2, "House"), (3, "Techno")] {
            conn.execute(
                "INSERT INTO tracks(id, path, status, folder) VALUES(?1, ?2, 'filed', ?3)",
                rusqlite::params![id, format!("/lib/{folder}/{id}.aiff"), folder],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO tracks(id, path, status, folder) VALUES(9, '/in/p.mp3', 'pending', 'House')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 2, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 3, &["Techno".into()]).unwrap();

        let f = folder_facets(&conn).unwrap();

        let house = f.folders.iter().find(|x| x.name == "House").unwrap();
        assert_eq!(house.count, 2, "only filed House tracks");
        assert!(
            f.folders
                .iter()
                .find(|x| x.name == "Techno")
                .map(|x| x.count)
                == Some(1)
        );
        let g_house = f.genres.iter().find(|x| x.name == "House").unwrap();
        assert_eq!(g_house.count, 2);
    }
}
