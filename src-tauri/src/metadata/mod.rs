//! Metadata source abstraction. A `MetadataProvider` turns a `Query` into ranked `Candidate`s;
//! `apply_identity` persists a chosen candidate into the DB. Discogs is the first provider
//! (see discogs.rs); the trait keeps a future AcoustID/MusicBrainz provider a drop-in.

pub mod cover;
pub mod discogs;

use serde::{Deserialize, Serialize};
use crate::naming::{Canonical, Confidence};
use rusqlite::{params, Connection};

/// User-edited metadata fields for a filed track (Bibliothèque inline edit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEdit {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_path: Option<String>,
}

/// DB-only part (unit-tested): upsert the editable metadata fields + replace genres.
/// Preserves `discogs_release_id` and `source` — a manual edit must not wipe the release link.
/// On INSERT (no prior metadata row) those columns stay NULL.
pub fn update_metadata_db(conn: &Connection, track_id: i64, e: &MetadataEdit) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, label, year, cover_path)
         VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(track_id) DO UPDATE SET
            artist=excluded.artist,
            title=excluded.title,
            label=excluded.label,
            year=excluded.year,
            cover_path=CASE WHEN excluded.cover_path IS NOT NULL THEN excluded.cover_path ELSE cover_path END",
        params![track_id, e.artist, e.title, e.label, e.year, e.cover_path],
    )?;
    crate::genres::set_genres(conn, track_id, &e.genres)?;
    if e.cover_path.is_some() {
        conn.execute("UPDATE tracks SET has_cover=1 WHERE id=?1", params![track_id])?;
    }
    Ok(())
}

/// What we search for: the track's current best-guess artist/title, plus the version
/// (remix/dub) used to pick the matching release among a release's tracklist.
pub struct Query {
    pub artist: String,
    pub title: String,
    pub version: Option<String>,
}

/// A normalized identification result, ranked best-first by the provider.
// Serialize → sent to the UI; Deserialize → the UI returns the chosen one to apply_identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>, // Discogs "style" (sub-genres), ordered
    pub country: Option<String>,
    pub format: Option<String>,
    pub cover_url: Option<String>,
    pub release_id: String,
    pub source: String, // "discogs"
}

/// What the UI needs after applying a candidate: the (name-driving) canonical plus the extra
/// tag fields, so it can refresh the preview, cover, and genre chips.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedIdentity {
    pub canonical: Canonical,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>,
    pub cover_path: Option<String>,
}

/// Split a trailing "(Version)" group off a Discogs title:
/// `"Love Foolosophy (Knee Deep Remix)"` → `("Love Foolosophy", Some("Knee Deep Remix"))`.
/// Returns `(trimmed title, None)` when there is no clean trailing parenthetical (no group, nested
/// parens, or an empty base). Mirrors the front's display-time split so a stored title and a
/// freshly-fetched one render the same base + version — this is what makes the chosen remix survive
/// a close+reopen (the file tags still hold the old name until filing).
fn split_title_version(title: &str) -> (String, Option<String>) {
    let t = title.trim();
    if t.ends_with(')') {
        if let Some(open) = t.rfind('(') {
            let inner = &t[open + 1..t.len() - 1];
            let base = t[..open].trim();
            if !inner.is_empty() && !inner.contains('(') && !inner.contains(')') && !base.is_empty() {
                return (base.to_string(), Some(inner.trim().to_string()));
            }
        }
    }
    (t.to_string(), None)
}

/// Persist a chosen candidate for `track_id`: upsert the single-value fields into `metadata`,
/// replace the track's sub-genres, and (when provided) record the downloaded cover path. The
/// cover download itself happens in the command layer (network); this fn is pure DB so it is
/// unit-tested. Returns the payload the UI refreshes from.
pub fn apply_identity(
    conn: &Connection,
    track_id: i64,
    c: &Candidate,
    cover_path: Option<String>,
) -> rusqlite::Result<AppliedIdentity> {
    // Store the clean base title + the extracted remix/dub in the `version` column, so a reopen
    // reads back the chosen identity (the file tags still hold the old name until filing). The
    // returned canonical keeps the FULL title — the front splits it for its live display.
    let (base_title, version) = split_title_version(&c.title);
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, version, label, year, cover_path, discogs_release_id, source)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(track_id) DO UPDATE SET
            artist=excluded.artist, title=excluded.title, version=excluded.version,
            label=excluded.label, year=excluded.year, cover_path=excluded.cover_path,
            discogs_release_id=excluded.discogs_release_id, source=excluded.source",
        params![track_id, c.artist, base_title, version, c.label, c.year, cover_path, c.release_id, c.source],
    )?;
    crate::genres::set_genres(conn, track_id, &c.styles)?;
    if cover_path.is_some() {
        conn.execute("UPDATE tracks SET has_cover=1 WHERE id=?1", params![track_id])?;
    }
    Ok(AppliedIdentity {
        canonical: Canonical {
            artist: c.artist.clone(),
            title: c.title.clone(),
            version: None,
            confidence: Confidence::Green, // a Discogs match is a high-confidence rename
        },
        label: c.label.clone(),
        year: c.year,
        styles: c.styles.clone(),
        cover_path,
    })
}

/// Why a provider call failed — mapped to stable IPC error codes by the command layer.
#[derive(Debug)]
pub enum ProviderError {
    NoToken,
    RateLimited { retry_after_s: u64 },
    Network(String),
    Parse(String),
}

impl ProviderError {
    /// Stable code string the UI maps to a message (NO_TOKEN, RATE_LIMITED:<s>, NETWORK:…, PARSE:…).
    pub fn code(&self) -> String {
        match self {
            ProviderError::NoToken => "NO_TOKEN".into(),
            ProviderError::RateLimited { retry_after_s } => format!("RATE_LIMITED:{retry_after_s}"),
            ProviderError::Network(m) => format!("NETWORK:{m}"),
            ProviderError::Parse(m) => format!("PARSE:{m}"),
        }
    }
}

pub trait MetadataProvider {
    /// Ranked candidates (best first). Empty vec = no results (not an error).
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')", []).unwrap();
        conn
    }

    fn sample() -> Candidate {
        Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: None,
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        }
    }

    #[test]
    fn apply_writes_metadata_and_genres() {
        let conn = db();
        let applied = apply_identity(&conn, 1, &sample(), Some("/cache/12345.jpg".into())).unwrap();

        type MetaRow = (String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>);
        let (artist, label, year, cover, rel, src): MetaRow =
            conn.query_row(
                "SELECT artist, label, year, cover_path, discogs_release_id, source FROM metadata WHERE track_id=1",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            ).unwrap();
        assert_eq!(artist, "Larry Heard");
        assert_eq!(label.as_deref(), Some("Alleviated"));
        assert_eq!(year, Some(1986));
        assert_eq!(cover.as_deref(), Some("/cache/12345.jpg"));
        assert_eq!(rel.as_deref(), Some("12345"));
        assert_eq!(src.as_deref(), Some("discogs"));

        assert_eq!(crate::genres::get_genres(&conn, 1).unwrap(), vec!["Deep House".to_string(), "House".to_string()]);

        assert_eq!(applied.canonical.artist, "Larry Heard");
        assert_eq!(applied.styles, vec!["Deep House".to_string(), "House".to_string()]);
        assert_eq!(applied.cover_path.as_deref(), Some("/cache/12345.jpg"));
    }

    #[test]
    fn re_apply_replaces_genres() {
        let conn = db();
        apply_identity(&conn, 1, &sample(), None).unwrap();
        let mut other = sample();
        other.styles = vec!["Techno".into()];
        apply_identity(&conn, 1, &other, None).unwrap();
        assert_eq!(crate::genres::get_genres(&conn, 1).unwrap(), vec!["Techno".to_string()]);
    }

    #[test]
    fn split_title_version_extracts_trailing_paren() {
        assert_eq!(
            split_title_version("Love Foolosophy (Knee Deep Remix)"),
            ("Love Foolosophy".to_string(), Some("Knee Deep Remix".to_string())),
        );
        assert_eq!(split_title_version("Mystery of Love"), ("Mystery of Love".to_string(), None));
        // Empty base (the whole title is the parenthetical) → no split.
        assert_eq!(split_title_version("(Instrumental)"), ("(Instrumental)".to_string(), None));
    }

    #[test]
    fn apply_persists_base_title_and_version() {
        let conn = db();
        let mut c = sample();
        c.title = "Can You Feel It (Larry Heard Remix)".into();
        apply_identity(&conn, 1, &c, None).unwrap();
        let (title, version): (String, Option<String>) = conn
            .query_row("SELECT title, version FROM metadata WHERE track_id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(title, "Can You Feel It");
        assert_eq!(version.as_deref(), Some("Larry Heard Remix"));
    }

    #[test]
    fn candidate_serde_round_trips() {
        let c = Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: Some("Vinyl, 12\"".into()),
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Candidate = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn update_metadata_db_preserves_release_link_and_replaces_genres() {
        let conn = db();
        // Seed a metadata row that already has a Discogs release link.
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, discogs_release_id, source)
             VALUES(1,'Old Artist','Old Title','999','discogs')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["Old".into()]).unwrap();

        let edit = MetadataEdit {
            artist: "New Artist".into(),
            title: "New Title".into(),
            label: Some("Trax".into()),
            year: Some(1990),
            genres: vec!["House".into(), "Deep House".into()],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        type Row = (String, String, Option<String>, Option<i64>, Option<String>, Option<String>);
        let (artist, title, label, year, rel_id, source): Row = conn
            .query_row(
                "SELECT artist, title, label, year, discogs_release_id, source FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap();

        assert_eq!(artist, "New Artist");
        assert_eq!(title, "New Title");
        assert_eq!(label.as_deref(), Some("Trax"));
        assert_eq!(year, Some(1990));
        // Release link must survive the manual edit.
        assert_eq!(rel_id.as_deref(), Some("999"), "discogs_release_id must be preserved");
        assert_eq!(source.as_deref(), Some("discogs"), "source must be preserved");

        let genres = crate::genres::get_genres(&conn, 1).unwrap();
        assert_eq!(genres, vec!["House".to_string(), "Deep House".to_string()]);
    }

    #[test]
    fn update_metadata_db_insert_path_leaves_release_null() {
        let conn = db();
        // No prior metadata row — INSERT path.
        let edit = MetadataEdit {
            artist: "Chez Damier".into(),
            title: "Can You Feel It".into(),
            label: Some("KMS".into()),
            year: Some(1992),
            genres: vec!["House".into()],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        type Row = (String, String, Option<String>, Option<i64>, Option<String>, Option<String>);
        let (artist, title, label, year, rel_id, source): Row = conn
            .query_row(
                "SELECT artist, title, label, year, discogs_release_id, source FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap();

        assert_eq!(artist, "Chez Damier");
        assert_eq!(title, "Can You Feel It");
        assert_eq!(label.as_deref(), Some("KMS"));
        assert_eq!(year, Some(1992));
        // No prior Discogs data → these stay NULL on INSERT.
        assert!(rel_id.is_none(), "discogs_release_id should be NULL on first insert");
        assert!(source.is_none(), "source should be NULL on first insert");

        let genres = crate::genres::get_genres(&conn, 1).unwrap();
        assert_eq!(genres, vec!["House".to_string()]);
    }
}
