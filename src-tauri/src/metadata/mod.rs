//! Metadata source abstraction. A `MetadataProvider` turns a `Query` into ranked `Candidate`s;
//! `apply_identity` persists a chosen candidate into the DB. Discogs is the first provider
//! (see discogs.rs); the trait keeps a future AcoustID/MusicBrainz provider a drop-in.
#![allow(dead_code)]

pub mod cover;
pub mod discogs;

use serde::{Deserialize, Serialize};

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
