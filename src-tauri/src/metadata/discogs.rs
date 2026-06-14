//! Discogs implementation of MetadataProvider. The HTTP call (`search`) is a thin wrapper over
//! `ureq`; the response→Candidate mapping (`parse_search`) is pure and unit-tested via a
//! captured fixture, so the matching logic is covered without any network access.
#![allow(dead_code)]

use crate::metadata::{Candidate, MetadataProvider, ProviderError, Query};
use serde_json::Value;

const USER_AGENT: &str = concat!("Sift/", env!("CARGO_PKG_VERSION"));

pub struct Discogs {
    pub token: String,
}

/// Discogs "title" is `"Artist - Title"`. Split on the first " - "; if absent, the whole
/// string is the title and the artist is empty.
fn split_title(s: &str) -> (String, String) {
    match s.find(" - ") {
        Some(i) => (s[..i].trim().to_string(), s[i + 3..].trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    }
}

fn first_string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn string_array(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// Map a Discogs search response into ranked Candidates. Pure: no I/O. Results with an empty
/// title are dropped; provider order is preserved.
pub fn parse_search(v: &Value) -> Vec<Candidate> {
    let Some(results) = v.get("results").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in results {
        let raw_title = r.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
        if raw_title.is_empty() {
            continue;
        }
        let (artist, title) = split_title(raw_title);
        let format = {
            let parts = string_array(r, "format");
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        };
        let year = r
            .get("year")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<i64>().ok());
        out.push(Candidate {
            artist,
            title,
            label: first_string(r, "label"),
            year,
            styles: string_array(r, "style"),
            country: r.get("country").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            format,
            cover_url: r.get("cover_image").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            release_id: r.get("id").map(|x| x.to_string()).unwrap_or_default(),
            source: "discogs".into(),
        });
    }
    out
}

impl MetadataProvider for Discogs {
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError> {
        if self.token.trim().is_empty() {
            return Err(ProviderError::NoToken);
        }
        let resp = ureq::get("https://api.discogs.com/database/search")
            .set("User-Agent", USER_AGENT)
            .set("Authorization", &format!("Discogs token={}", self.token))
            .query("type", "release")
            .query("artist", &q.artist)
            .query("track", &q.title)
            .query("per_page", "8")
            .call();
        match resp {
            Ok(r) => {
                let v: Value = r.into_json().map_err(|e| ProviderError::Parse(e.to_string()))?;
                Ok(parse_search(&v))
            }
            Err(ureq::Error::Status(429, r)) => {
                let retry = r.header("Retry-After").and_then(|s| s.parse::<u64>().ok()).unwrap_or(60);
                Err(ProviderError::RateLimited { retry_after_s: retry })
            }
            Err(ureq::Error::Status(code, _)) => Err(ProviderError::Network(format!("HTTP {code}"))),
            Err(ureq::Error::Transport(t)) => Err(ProviderError::Network(t.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "results": [
        {
          "id": 12345,
          "title": "Larry Heard - Mystery Of Love",
          "year": "1986",
          "country": "US",
          "label": ["Alleviated Records", "Alleviated"],
          "genre": ["Electronic"],
          "style": ["Deep House", "House"],
          "format": ["Vinyl", "12\""],
          "cover_image": "https://img.discogs.com/x.jpg"
        },
        {
          "id": 999,
          "title": "Larry Heard - Mystery Of Love (Remix)",
          "label": ["Alleviated"],
          "style": ["House"],
          "cover_image": "https://img.discogs.com/y.jpg"
        },
        { "id": 7, "title": "" }
      ]
    }"#;

    #[test]
    fn parse_maps_style_to_styles_and_ignores_broad_genre() {
        let v: Value = serde_json::from_str(FIXTURE).unwrap();
        let cands = parse_search(&v);
        assert_eq!(cands.len(), 2, "title-less result is filtered out");
        let first = &cands[0];
        assert_eq!(first.artist, "Larry Heard");
        assert_eq!(first.title, "Mystery Of Love");
        assert_eq!(first.styles, vec!["Deep House".to_string(), "House".to_string()]);
        assert_eq!(first.year, Some(1986));
        assert_eq!(first.label.as_deref(), Some("Alleviated Records"));
        assert_eq!(first.country.as_deref(), Some("US"));
        assert_eq!(first.format.as_deref(), Some("Vinyl, 12\""));
        assert_eq!(first.release_id, "12345");
        assert_eq!(first.source, "discogs");
    }

    #[test]
    fn parse_keeps_provider_order_and_handles_missing_optionals() {
        let v: Value = serde_json::from_str(FIXTURE).unwrap();
        let cands = parse_search(&v);
        assert_eq!(cands[1].release_id, "999");
        assert_eq!(cands[1].year, None);
        assert_eq!(cands[1].country, None);
    }
}
