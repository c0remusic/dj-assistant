//! Discogs implementation of MetadataProvider. The HTTP call (`search`) is a thin wrapper over
//! `ureq`; the response→Candidate mapping (`parse_search`) is pure and unit-tested via a
//! captured fixture, so the matching logic is covered without any network access.
#![allow(dead_code)]

use crate::metadata::{Candidate, MetadataProvider, ProviderError, Query};
use serde_json::Value;
use std::collections::HashSet;

const USER_AGENT: &str = concat!("Sift/", env!("CARGO_PKG_VERSION"));

/// How many top candidates get a tracklist look-up (one HTTP call each) to find the matching
/// mix. Bounded to stay well under Discogs' 60 req/min while covering the realistic best hits.
const TRACKLIST_PROBE: usize = 6;

pub struct Discogs {
    pub token: String,
}

/// Discogs "title" is `"Artist - Title"`. Split on the first " - "; if absent, the whole
/// string is the title and the artist is empty. The artist is cleaned of Discogs artifacts.
fn split_title(s: &str) -> (String, String) {
    match s.find(" - ") {
        Some(i) => (clean_artist(s[..i].trim()), s[i + 3..].trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    }
}

/// Strip Discogs catalog artifacts from an artist credit that never belong in a real name:
/// the ANV asterisk ("Larry Heard*") and the numeric disambiguation suffix ("Aya (2)").
/// Parenthetical groups that aren't pure digits (e.g. "(Live)") are left untouched.
fn clean_artist(s: &str) -> String {
    let mut result: String = s.chars().filter(|&c| c != '*').collect();
    loop {
        let cut = result.find('(').and_then(|open| {
            result[open..].find(')').and_then(|rel| {
                let close = open + rel;
                let inner = &result[open + 1..close];
                if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                    Some((open, close))
                } else {
                    None
                }
            })
        });
        match cut {
            Some((open, close)) => result.replace_range(open..=close, ""),
            None => break,
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
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
    // Prefer the original release over compilations / DJ-mixes: Discogs' own ranking sometimes
    // puts a "Mixed"/"Compilation" CD above the actual single/12". Stable sort by a format-based
    // relevance keeps Discogs order within ties, so the best match is the real release; the rest
    // stay available under "autres".
    out.sort_by(|a, b| format_relevance(b).cmp(&format_relevance(a)));
    out
}

/// Heuristic relevance from the Discogs `format` descriptors: penalize compilations / mixes,
/// reward singles/EPs and physical vinyl. Higher is more likely the original release.
fn format_relevance(c: &Candidate) -> i32 {
    let mut score = 0;
    let fmt = c.format.as_deref().unwrap_or("");
    for tok in fmt.split(',').map(|t| t.trim().to_lowercase()) {
        match tok.as_str() {
            "compilation" | "mixed" | "dj mix" => score -= 3,
            "single" | "ep" | "maxi-single" => score += 2,
            "vinyl" | "12\"" | "7\"" | "10\"" => score += 1,
            _ => {}
        }
    }
    score
}

/// Lowercased alphanumeric tokens of `s` (punctuation/parens become separators). Used to
/// compare a Discogs tracklist title against the track we're identifying.
fn norm_tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(|t| t.to_string())
        .collect()
}

/// How well a Discogs tracklist title matches the track we want. The version (remix/dub)
/// tokens are weighted heavily — they're what distinguishes one mix from another, so a
/// tracklist entry that actually contains the mix name wins decisively over a plain title.
fn track_match_score(track_title: &str, target_title: &str, target_version: Option<&str>) -> i32 {
    let track: HashSet<String> = norm_tokens(track_title).into_iter().collect();
    let mut score = 0;
    for t in norm_tokens(target_title) {
        if track.contains(&t) {
            score += 1;
        }
    }
    if let Some(v) = target_version {
        for t in norm_tokens(v) {
            if track.contains(&t) {
                score += 3;
            }
        }
    }
    score
}

/// Re-rank candidates by their tracklist match score (primary), falling back to format
/// relevance (secondary) and the original order (stable). `scores[i]` is the best tracklist
/// match for `cands[i]` (0 when no tracklist was fetched or nothing matched).
fn rank_by_match(cands: Vec<Candidate>, scores: &[i32]) -> Vec<Candidate> {
    let mut idx: Vec<usize> = (0..cands.len()).collect();
    idx.sort_by(|&a, &b| {
        scores[b]
            .cmp(&scores[a])
            .then_with(|| format_relevance(&cands[b]).cmp(&format_relevance(&cands[a])))
    });
    idx.into_iter().map(|i| cands[i].clone()).collect()
}

impl Discogs {
    /// Fetch a release's tracklist titles. Best-effort: the caller treats Err as "no tracklist"
    /// and simply doesn't refine that candidate (so a rate-limit on a detail call is non-fatal).
    fn fetch_tracklist(&self, release_id: &str) -> Result<Vec<String>, ProviderError> {
        let url = format!("https://api.discogs.com/releases/{release_id}");
        let resp = ureq::get(&url)
            .set("User-Agent", USER_AGENT)
            .set("Authorization", &format!("Discogs token={}", self.token))
            .call();
        match resp {
            Ok(r) => {
                let v: Value = r.into_json().map_err(|e| ProviderError::Parse(e.to_string()))?;
                let titles = v
                    .get("tracklist")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.get("title").and_then(|x| x.as_str()))
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(titles)
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

impl MetadataProvider for Discogs {
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError> {
        if self.token.trim().is_empty() {
            return Err(ProviderError::NoToken);
        }
        // Use the general full-text query ("artist title") rather than the strict
        // artist+track filters: Discogs' `track` filter matches a release's tracklist and is
        // unreliable (combined with `artist` it often returns nothing even on an exact title).
        // `q` makes the title actually count and is far more forgiving.
        let search = format!("{} {}", q.artist, q.title);
        let search = search.trim();
        let resp = ureq::get("https://api.discogs.com/database/search")
            .set("User-Agent", USER_AGENT)
            .set("Authorization", &format!("Discogs token={}", self.token))
            .query("type", "release")
            .query("q", search)
            .query("per_page", "8")
            .call();
        let cands = match resp {
            Ok(r) => {
                let v: Value = r.into_json().map_err(|e| ProviderError::Parse(e.to_string()))?;
                parse_search(&v)
            }
            Err(ureq::Error::Status(429, r)) => {
                let retry = r.header("Retry-After").and_then(|s| s.parse::<u64>().ok()).unwrap_or(60);
                return Err(ProviderError::RateLimited { retry_after_s: retry });
            }
            Err(ureq::Error::Status(code, _)) => return Err(ProviderError::Network(format!("HTTP {code}"))),
            Err(ureq::Error::Transport(t)) => return Err(ProviderError::Network(t.to_string())),
        };

        // Refine: for the top candidates, fetch their tracklist and score how well it contains
        // the exact mix (title + version). The release that actually holds this mix wins. Detail
        // calls are best-effort — a failed/rate-limited one just leaves that candidate unscored.
        let mut scores = vec![0i32; cands.len()];
        for (i, c) in cands.iter().enumerate().take(TRACKLIST_PROBE) {
            if c.release_id.is_empty() {
                continue;
            }
            if let Ok(titles) = self.fetch_tracklist(&c.release_id) {
                scores[i] = titles
                    .iter()
                    .map(|t| track_match_score(t, &q.title, q.version.as_deref()))
                    .max()
                    .unwrap_or(0);
            }
        }
        Ok(rank_by_match(cands, &scores))
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

    #[test]
    fn original_release_ranks_above_compilation_and_mix() {
        // Discogs returns the compilation/DJ-mix FIRST, but the real vinyl single should win.
        const F: &str = r#"{
          "results": [
            { "id": 1, "title": "Various - Summer Mix 2001", "format": ["CD", "Compilation", "Mixed"], "style": ["House"] },
            { "id": 2, "title": "VA - DJ Mix Vol. 3", "format": ["CD", "Mixed", "DJ Mix"], "style": ["House"] },
            { "id": 3, "title": "Aya - Sean", "format": ["Vinyl", "12\"", "Single"], "style": ["House"] }
          ]
        }"#;
        let v: Value = serde_json::from_str(F).unwrap();
        let cands = parse_search(&v);
        assert_eq!(cands[0].release_id, "3", "the vinyl single outranks comp/mix");
        // the compilation and DJ-mix are still present, just lower
        assert!(cands.iter().any(|c| c.release_id == "1"));
        assert!(cands.iter().any(|c| c.release_id == "2"));
    }

    #[test]
    fn track_score_prefers_the_matching_mix() {
        let target_t = "Sean";
        let ver = Some("Eric's 2WFU Dub");
        let dub = track_match_score("Sean (Eric's 2WFU Dub)", target_t, ver);
        let plain = track_match_score("Sean", target_t, ver);
        let other = track_match_score("Sean (Radio Edit)", target_t, ver);
        assert!(dub > plain, "the exact dub ({dub}) beats the plain title ({plain})");
        assert!(dub > other, "the exact dub ({dub}) beats a different mix ({other})");
    }

    fn cand(id: &str, format: Option<&str>) -> Candidate {
        Candidate {
            artist: "Aya".into(),
            title: "Sean".into(),
            label: None,
            year: None,
            styles: vec![],
            country: None,
            format: format.map(|s| s.to_string()),
            cover_url: None,
            release_id: id.into(),
            source: "discogs".into(),
        }
    }

    #[test]
    fn rank_promotes_release_whose_tracklist_holds_the_mix() {
        // candidate 1 has the better format, but candidate 2's tracklist actually contains the
        // mix (higher match score) → the match must win over format relevance.
        let cands = vec![cand("1", Some("Vinyl, 12\", Single")), cand("2", Some("CD, Album"))];
        let scores = [1, 9];
        let ranked = rank_by_match(cands, &scores);
        assert_eq!(ranked[0].release_id, "2");
        assert_eq!(ranked[1].release_id, "1");
    }

    #[test]
    fn rank_falls_back_to_format_when_scores_tie() {
        // no tracklist matched (all zero) → format relevance breaks the tie (single > album).
        let cands = vec![cand("album", Some("CD, Album")), cand("single", Some("Vinyl, 12\", Single"))];
        let scores = [0, 0];
        let ranked = rank_by_match(cands, &scores);
        assert_eq!(ranked[0].release_id, "single");
    }
}
