//! Metadata source abstraction. A `MetadataProvider` turns a `Query` into ranked `Candidate`s;
//! `apply_identity` persists a chosen candidate into the DB. Discogs is the first provider
//! (see discogs.rs); the trait keeps a future AcoustID/MusicBrainz provider a drop-in.
#![allow(dead_code)]

pub mod cover;
pub mod discogs;

use serde::{Deserialize, Serialize};
use crate::naming::{Canonical, Confidence};
use rusqlite::{params, Connection};

/// What we search for: the track's current best-guess artist/title.
pub struct Query {
    pub artist: String,
    pub title: String,
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
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, label, year, cover_path, discogs_release_id, source)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(track_id) DO UPDATE SET
            artist=excluded.artist, title=excluded.title, label=excluded.label,
            year=excluded.year, cover_path=COALESCE(excluded.cover_path, metadata.cover_path),
            discogs_release_id=excluded.discogs_release_id, source=excluded.source",
        params![track_id, c.artist, c.title, c.label, c.year, cover_path, c.release_id, c.source],
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

        let (artist, label, year, cover, rel, src): (String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>) =
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
}
