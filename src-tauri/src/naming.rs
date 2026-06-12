//! Pure naming logic (no I/O): reconcile a track's embedded tags and its filename into
//! one canonical {artist, title, version} record, and render the output filename from a
//! template. The single source of truth that drives BOTH the filename and the tags
//! written at filing time (see M4 spec). Exhaustively unit-tested; never touches disk.
//!
//! The public API is consumed by the filing orchestration in a later M4 slice (M4-3);
//! until then it is unused by the binary, hence the module-level dead_code allow.
#![allow(dead_code)]

use serde::Serialize;

/// How sure we are about the reconciled metadata. Green = file in one click; Yellow =
/// surface for a quick validation pass before committing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Green,
    Yellow,
}

/// The reconciled, canonical metadata for one track. Both the output filename and the
/// embedded tags are derived from this — they can never diverge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Canonical {
    pub artist: String,
    pub title: String,
    pub version: Option<String>,
    pub confidence: Confidence,
}

/// Tokens that mark a string as sloppy download metadata rather than a clean field.
const JUNK_TOKENS: &[&str] = &[
    "kbps", "khz", "flac", "http", "www", "320", "256", "192", "128", "rip", "track ",
    "[", "]", "{", "}", "_",
];

/// True if `s` contains any junk token (case-insensitive). Used by the cleanliness gate.
pub fn has_junk(s: &str) -> bool {
    let low = s.to_lowercase();
    JUNK_TOKENS.iter().any(|t| low.contains(t))
}

/// A {artist, title} source is clean when both are non-blank and free of junk tokens.
pub fn is_clean(artist: &str, title: &str) -> bool {
    !artist.trim().is_empty()
        && !title.trim().is_empty()
        && !has_junk(artist)
        && !has_junk(title)
}

/// Parse a filename stem (no extension) into (artist, title, version?). Returns None when
/// there is no " - " separator or the parsed fields aren't clean. Pure string work.
pub fn parse_filename(stem: &str) -> Option<(String, String, Option<String>)> {
    let (artist_raw, rest) = stem.split_once(" - ")?;
    let artist = artist_raw.trim().to_string();

    // Pull a trailing "(...)" off the title as the version.
    let rest = rest.trim();
    let (title_raw, version) = match (rest.rfind('('), rest.rfind(')')) {
        (Some(open), Some(close)) if close > open && close == rest.len() - 1 => {
            let v = rest[open + 1..close].trim().to_string();
            (rest[..open].trim().to_string(), Some(v))
        }
        _ => (rest.to_string(), None),
    };

    if !is_clean(&artist, &title_raw) {
        return None;
    }
    Some((artist, title_raw, version))
}

/// Normalize for the "do tags and filename agree?" comparison: lowercase, collapse
/// whitespace. Internal to reconcile.
fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reconcile embedded tags and the filename stem into one canonical record + confidence.
/// See the M4 spec's four-case matrix. Tags are preferred when clean; the version always
/// comes from the filename when present (tags rarely carry it cleanly at this stage).
pub fn reconcile(tag_artist: &str, tag_title: &str, stem: &str) -> Canonical {
    let tags_clean = is_clean(tag_artist, tag_title);
    let parsed = parse_filename(stem); // Some only if the name is clean
    let name_version = parsed.as_ref().and_then(|(_, _, v)| v.clone());

    match (tags_clean, &parsed) {
        // both clean: agree -> green; disagree -> yellow (tags shown as default)
        (true, Some((pa, pt, _))) => {
            let agree = norm(tag_artist) == norm(pa) && norm(tag_title) == norm(pt);
            Canonical {
                artist: tag_artist.trim().to_string(),
                title: tag_title.trim().to_string(),
                version: name_version,
                confidence: if agree { Confidence::Green } else { Confidence::Yellow },
            }
        }
        // tags clean only -> green from tags
        (true, None) => Canonical {
            artist: tag_artist.trim().to_string(),
            title: tag_title.trim().to_string(),
            version: None,
            confidence: Confidence::Green,
        },
        // name clean only -> green from name
        (false, Some((pa, pt, v))) => Canonical {
            artist: pa.clone(),
            title: pt.clone(),
            version: v.clone(),
            confidence: Confidence::Green,
        },
        // neither clean -> yellow, best guess = raw stem as title for the user to edit
        (false, None) => Canonical {
            artist: String::new(),
            title: stem.trim().to_string(),
            version: None,
            confidence: Confidence::Yellow,
        },
    }
}

/// Replace characters illegal in Windows/macOS filenames with a space, then collapse
/// runs of whitespace and trim. Keeps the name human-readable.
pub fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render `template` against a canonical record and append `.ext`. Supported placeholders:
/// `{artist}`, `{title}`, `{version}`. `{version}` expands to " (Version)" when present,
/// to "" when absent (no empty parens). The whole stem is sanitized for the filesystem.
pub fn render_filename(template: &str, c: &Canonical, ext: &str) -> String {
    let version_str = match &c.version {
        Some(v) if !v.trim().is_empty() => format!(" ({})", v.trim()),
        _ => String::new(),
    };
    let stem = template
        .replace("{artist}", &c.artist)
        .replace("{title}", &c.title)
        .replace("{version}", &version_str);
    format!("{}.{}", sanitize(&stem), ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junk_flags_quality_and_uploader_tokens() {
        assert!(has_junk("Mystery of Love 320kbps"));
        assert!(has_junk("track 01"));
        assert!(has_junk("audio_320"));
        assert!(has_junk("Some Title [DJ Uploader]"));
        assert!(has_junk("FLAC rip"));
        assert!(has_junk("http://site"));
    }

    #[test]
    fn junk_passes_clean_text() {
        assert!(!has_junk("Mystery of Love"));
        assert!(!has_junk("Can You Feel It"));
        assert!(!has_junk("Larry Heard"));
    }

    #[test]
    fn clean_requires_both_fields_and_no_junk() {
        assert!(is_clean("Larry Heard", "Mystery of Love"));
        assert!(!is_clean("", "Mystery of Love")); // empty artist
        assert!(!is_clean("Larry Heard", "   ")); // blank title
        assert!(!is_clean("Larry Heard", "Mystery 320kbps")); // junk title
    }

    #[test]
    fn parses_artist_title_version() {
        let (a, t, v) = parse_filename("Larry Heard - Mystery of Love (Original Mix)").unwrap();
        assert_eq!(a, "Larry Heard");
        assert_eq!(t, "Mystery of Love");
        assert_eq!(v.as_deref(), Some("Original Mix"));
    }

    #[test]
    fn parses_without_version() {
        let (a, t, v) = parse_filename("Chez Damier - Can You Feel It").unwrap();
        assert_eq!(a, "Chez Damier");
        assert_eq!(t, "Can You Feel It");
        assert_eq!(v, None);
    }

    #[test]
    fn rejects_unparseable_or_junky_stem() {
        assert!(parse_filename("01_audio_320").is_none()); // junk + no separator
        assert!(parse_filename("randomgibberish").is_none()); // no " - " separator
    }

    #[test]
    fn both_clean_and_agree_is_green_from_tags() {
        let c = reconcile(
            "Larry Heard",
            "Mystery of Love",
            "Larry Heard - Mystery of Love (Original Mix)",
        );
        assert_eq!(c.artist, "Larry Heard");
        assert_eq!(c.title, "Mystery of Love");
        assert_eq!(c.version.as_deref(), Some("Original Mix")); // version comes from name
        assert_eq!(c.confidence, Confidence::Green);
    }

    #[test]
    fn tags_clean_name_junky_is_green_from_tags() {
        let c = reconcile("Theo Parrish", "Falling Up", "01_audio_320");
        assert_eq!(c.artist, "Theo Parrish");
        assert_eq!(c.title, "Falling Up");
        assert_eq!(c.confidence, Confidence::Green);
    }

    #[test]
    fn name_clean_tags_junky_is_green_from_name() {
        let c = reconcile("", "track 01", "Chez Damier - Can You Feel It");
        assert_eq!(c.artist, "Chez Damier");
        assert_eq!(c.title, "Can You Feel It");
        assert_eq!(c.confidence, Confidence::Green);
    }

    #[test]
    fn both_clean_but_disagree_is_yellow() {
        let c = reconcile(
            "Larry Heard",
            "Mystery of Love",
            "Robert Owens - Bring Down the Walls",
        );
        assert_eq!(c.confidence, Confidence::Yellow);
        assert_eq!(c.artist, "Larry Heard"); // tags shown as the default pick
    }

    #[test]
    fn neither_clean_is_yellow_best_guess() {
        let c = reconcile("", "", "01_audio_320");
        assert_eq!(c.confidence, Confidence::Yellow);
        // best guess falls back to the raw stem as the title so the user has something to edit
        assert_eq!(c.title, "01_audio_320");
        assert_eq!(c.artist, "");
    }

    #[test]
    fn sanitize_strips_path_unsafe_chars() {
        assert_eq!(sanitize("AC/DC: Back?"), "AC DC Back");
        assert_eq!(sanitize("a   b"), "a b"); // collapse whitespace
    }

    #[test]
    fn renders_with_version() {
        let c = Canonical {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            version: Some("Original Mix".into()),
            confidence: Confidence::Green,
        };
        assert_eq!(
            render_filename("{artist} - {title}{version}", &c, "aiff"),
            "Larry Heard - Mystery of Love (Original Mix).aiff"
        );
    }

    #[test]
    fn renders_without_version_no_empty_parens() {
        let c = Canonical {
            artist: "Chez Damier".into(),
            title: "Can You Feel It".into(),
            version: None,
            confidence: Confidence::Green,
        };
        assert_eq!(
            render_filename("{artist} - {title}{version}", &c, "mp3"),
            "Chez Damier - Can You Feel It.mp3"
        );
    }
}
