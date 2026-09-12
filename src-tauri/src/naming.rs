//! Pure naming logic (no I/O): reconcile a track's embedded tags and its filename into
//! one canonical {artist, title, version} record, and render the output filename from a
//! template. The single source of truth that drives BOTH the filename and the tags
//! written at filing time (see M4 spec). Exhaustively unit-tested; never touches disk.

use serde::{Deserialize, Serialize};

/// How sure we are about the reconciled metadata. Green = file in one click; Yellow =
/// surface for a quick validation pass before committing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Green,
    Yellow,
}

/// The reconciled, canonical metadata for one track. Both the output filename and the
/// embedded tags are derived from this — they can never diverge.
///
/// `label` rides along ONLY for the tag write (the Discogs release label, editable in the Revue
/// pane): it is NOT name-driving — `render_filename`/`tag_title` ignore it (there is no `{label}`
/// placeholder), so widening this struct never changes a filename. `reconcile` sets it to `None`
/// (tags/filename carry no label); the front seeds it from the persisted release facts and lets the
/// user edit it, and `apply_tags` writes it via `write_tags_full`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canonical {
    pub artist: String,
    pub title: String,
    pub version: Option<String>,
    pub label: Option<String>,
    pub confidence: Confidence,
}

/// Tokens that mark a string as sloppy download metadata rather than a clean field.
const JUNK_TOKENS: &[&str] = &[
    "kbps", "khz", "flac", "http", "www", "320", "256", "192", "128", "rip", "track ", "[", "]",
    "{", "}", "_",
];

/// True if `s` contains any junk token (case-insensitive). Used by the cleanliness gate.
pub fn has_junk(s: &str) -> bool {
    let low = s.to_lowercase();
    JUNK_TOKENS.iter().any(|t| low.contains(t))
}

/// A {artist, title} source is clean when both are non-blank and free of junk tokens.
pub fn is_clean(artist: &str, title: &str) -> bool {
    !artist.trim().is_empty() && !title.trim().is_empty() && !has_junk(artist) && !has_junk(title)
}

/// Pulls a trailing "(...)" off `s` as a version — e.g. "Mystery of Love (Original Mix)" ->
/// ("Mystery of Love", Some("Original Mix")). Pure syntax, no cleanliness requirement: the
/// version only needs its own parens to be well-formed, unlike `parse_filename`'s artist/title.
fn extract_trailing_version(s: &str) -> (String, Option<String>) {
    match (s.rfind('('), s.rfind(')')) {
        (Some(open), Some(close)) if close > open && close == s.len() - 1 => {
            let v = s[open + 1..close].trim().to_string();
            (s[..open].trim().to_string(), Some(v))
        }
        _ => (s.to_string(), None),
    }
}

/// Parse a filename stem (no extension) into (artist, title, version?). Returns None when
/// there is no " - " separator or the parsed fields aren't clean. Pure string work.
pub fn parse_filename(stem: &str) -> Option<(String, String, Option<String>)> {
    let (artist_raw, rest) = stem.split_once(" - ")?;
    let artist = artist_raw.trim().to_string();
    let (title_raw, version) = extract_trailing_version(rest.trim());

    if !is_clean(&artist, &title_raw) {
        return None;
    }
    Some((artist, title_raw, version))
}

/// Best-effort version/mix extraction from a filename stem, independent of overall
/// cleanliness. Unlike `parse_filename`, junk elsewhere in the stem (bitrate, uploader
/// brackets) must not cost us the "(Extended Mix)" trailing the title — Discogs' tracklist
/// matching needs this version hint to pick the right mix even when the tags are clean but
/// the filename carries noise the version parens aren't part of.
fn extract_version_hint(stem: &str) -> Option<String> {
    let rest = match stem.split_once(" - ") {
        Some((_, r)) => r,
        None => stem,
    };
    extract_trailing_version(rest.trim()).1
}

/// Normalize for the "do tags and filename agree?" comparison: lowercase, collapse
/// whitespace. Internal to reconcile.
fn norm(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
                label: None,
                confidence: if agree {
                    Confidence::Green
                } else {
                    Confidence::Yellow
                },
            }
        }
        // tags clean only -> green from tags. Filename didn't parse as a whole (junk
        // elsewhere), but a trailing "(...)" version is still worth pulling independently —
        // see extract_version_hint.
        (true, None) => Canonical {
            artist: tag_artist.trim().to_string(),
            title: tag_title.trim().to_string(),
            version: extract_version_hint(stem),
            label: None,
            confidence: Confidence::Green,
        },
        // name clean only -> green from name
        (false, Some((pa, pt, v))) => Canonical {
            artist: pa.clone(),
            title: pt.clone(),
            version: v.clone(),
            label: None,
            confidence: Confidence::Green,
        },
        // neither clean -> yellow, best guess = a *cleaned* stem as title for the user to edit
        (false, None) => Canonical {
            artist: String::new(),
            title: clean_stem(stem),
            version: None,
            label: None,
            confidence: Confidence::Yellow,
        },
    }
}

/// Best-effort tidy of a messy filename stem for the editable title prefill: drop a leading
/// track number, replace underscores with spaces, remove `[bracketed]` junk (uploaders/labels)
/// and quality tokens (320kbps, FLAC, kHz…), then collapse whitespace. Conservative — it only
/// improves the starting point; the user still confirms (yellow).
pub fn clean_stem(stem: &str) -> String {
    let mut s = stem.replace('_', " ");
    // drop [ ... ] segments
    while let (Some(a), Some(b)) = (s.find('['), s.find(']')) {
        if b > a {
            s.replace_range(a..=b, " ");
        } else {
            break;
        }
    }
    // drop ( ... ) segments only when their content is known source/quality noise (never a
    // blind strip: "(Original Mix)"/"(feat. X)" are meaningful and must survive).
    const NOISE_PAREN: &[&str] = &["rip", "bootleg", "promo", "unofficial"];
    while let (Some(a), Some(b)) = (s.find('('), s.find(')')) {
        if b <= a {
            break;
        }
        let inner = s[a + 1..b].to_lowercase();
        if NOISE_PAREN.iter().any(|k| inner.contains(k)) {
            s.replace_range(a..=b, " ");
        } else {
            break;
        }
    }
    // strip a leading track number ("01 ", "1.", "12 - ") — only 1–3 digits + a separator
    {
        let t = s.trim_start();
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if (1..=3).contains(&digits) {
            let rest = t[digits..].trim_start_matches([' ', '.', '-', ')', '_']);
            if !rest.is_empty() && rest.len() < t.len() {
                s = rest.to_string();
            }
        }
    }
    // drop quality/junk tokens word-by-word (case-insensitive)
    const DROP: &[&str] = &[
        "kbps", "320", "256", "192", "128", "flac", "wav", "aiff", "khz", "hz", "hq", "cbr", "vbr",
        "rip",
    ];
    let kept: Vec<&str> = s
        .split_whitespace()
        .filter(|w| {
            let lw = w.to_lowercase();
            !DROP.iter().any(|d| lw == *d)
        })
        .collect();
    kept.join(" ").trim().to_string()
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

/// " (Version)" suffix shared by `render_filename` and `tag_title` — "" when absent, no empty
/// parens.
fn version_suffix(c: &Canonical) -> String {
    match &c.version {
        Some(v) if !v.trim().is_empty() => format!(" ({})", v.trim()),
        _ => String::new(),
    }
}

/// Render `template` against a canonical record and append `.ext`. Supported placeholders:
/// `{artist}`, `{title}`, `{version}`. `{version}` expands to " (Version)" when present,
/// to "" when absent (no empty parens). The whole stem is sanitized for the filesystem.
pub fn render_filename(template: &str, c: &Canonical, ext: &str) -> String {
    let stem = template
        .replace("{artist}", &c.artist)
        .replace("{title}", &c.title)
        .replace("{version}", &version_suffix(c));
    format!("{}.{}", sanitize(&stem), ext)
}

/// The title as it should be WRITTEN TO THE ID3/tag Title field — title + the same " (Version)"
/// suffix `render_filename` puts in the filename. Both must derive from this one function: a
/// track named "Title (Extended Mix).aiff" previously had the version silently absent from its
/// own Title tag (write_tags_full call sites passed `c.title` alone), so a CDJ/Rekordbox reading
/// the file's tags directly — not the filename — never saw it. No filesystem sanitization here,
/// unlike render_filename: this never touches a path.
pub fn tag_title(c: &Canonical) -> String {
    format!("{}{}", c.title, version_suffix(c))
}

/// Fold the common accented Latin letters to ASCII (no extra crate) so "Béatrice" and
/// "Beatrice" key the same.
fn fold_char(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
        'ç' => 'c',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' | 'ì' => 'i',
        'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ñ' => 'n',
        other => other,
    }
}

/// A normalized key answering "is this the same track by name?": artist + title, accent-
/// folded, lowercased, punctuation dropped, whitespace collapsed. Two spellings of the same
/// track collapse to the same key; different titles stay distinct. Drives dedup's name pre-
/// filter. Pure, no I/O.
pub fn name_key(artist: &str, title: &str) -> String {
    fn norm(s: &str) -> String {
        // lowercase first (unicode-aware: É → é) so the accent fold catches both cases
        let folded: String = s
            .to_lowercase()
            .chars()
            .map(fold_char)
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect();
        folded.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    // Space-join (no separator) ON PURPOSE: it lets "Larry Heard - Mystery of Love" match a
    // file named "larry_heard mystery of love" with no " - " split — a common cross-naming
    // duplicate. The theoretical ("","x") vs ("x","") collision is accepted as harmless here.
    format!("{} {}", norm(artist), norm(title))
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors shared/contracts.ts's `Canonical`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn canonical_shape_matches_contracts_ts() {
        let v = Canonical {
            artist: String::new(),
            title: String::new(),
            version: None,
            label: None,
            confidence: Confidence::Green,
        };
        let Canonical {
            artist,
            title,
            version,
            label,
            confidence,
        } = v;
        let _ = (artist, title, version, label, confidence);
    }

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
        // best guess: the stem cleaned (track no + "_" + quality token dropped)
        assert_eq!(c.title, "audio");
        assert_eq!(c.artist, "");
    }

    #[test]
    fn clean_stem_tidies_messy_filenames() {
        assert_eq!(
            clean_stem("01_larry_heard_mystery_320"),
            "larry heard mystery"
        );
        assert_eq!(clean_stem("Some Title [DJ Uploader] FLAC"), "Some Title");
        assert_eq!(clean_stem("1979 - something"), "1979 - something"); // 4 digits: not a track no
    }

    #[test]
    fn clean_stem_drops_source_noise_parens_but_keeps_meaningful_ones() {
        assert_eq!(clean_stem("Title (Vinyl Rip)"), "Title");
        assert_eq!(clean_stem("Title (Bootleg)"), "Title");
        // Not source noise — must survive, it's the actual mix name.
        assert_eq!(clean_stem("Title (Original Mix)"), "Title (Original Mix)");
        assert_eq!(clean_stem("Title (feat. Someone)"), "Title (feat. Someone)");
    }

    #[test]
    fn tags_clean_but_stem_junky_still_recovers_version() {
        // Tags are clean (green from tags), but the filename carries an unrelated junk
        // token ("01_" prefix) that used to blow away version extraction entirely because
        // parse_filename requires the WHOLE stem to be clean. The trailing "(Extended Mix)"
        // must survive that — it's what best_track_match needs to pick the right mix.
        let c = reconcile(
            "Theo Parrish",
            "Falling Up",
            "01_Theo Parrish - Falling Up (Extended Mix)",
        );
        assert_eq!(c.artist, "Theo Parrish");
        assert_eq!(c.title, "Falling Up");
        assert_eq!(c.version.as_deref(), Some("Extended Mix"));
        assert_eq!(c.confidence, Confidence::Green);
    }

    #[test]
    fn name_key_collapses_spellings_and_separates_titles() {
        // same track, different spelling/punctuation/case/accents → same key
        assert_eq!(
            name_key("Larry Heard", "Mystery of Love"),
            name_key("larry_heard", "Mystery  of  Love!"),
        );
        assert_eq!(name_key("Béatrice", "Été"), name_key("Beatrice", "Ete"));
        // different titles → different keys
        assert_ne!(
            name_key("Larry Heard", "Mystery of Love"),
            name_key("Larry Heard", "Can You Feel It"),
        );
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
            label: None,
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
            label: None,
            confidence: Confidence::Green,
        };
        assert_eq!(
            render_filename("{artist} - {title}{version}", &c, "mp3"),
            "Chez Damier - Can You Feel It.mp3"
        );
    }
}
