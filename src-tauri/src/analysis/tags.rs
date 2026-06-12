//! Declared audio properties + tag metadata via `lofty` (read-only).

use crate::analysis::Rail;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::ItemKey;

/// What we read from the container without decoding: declared rail, bitrate, duration,
/// channels, ID3 version, CDJ-tag sanity, embedded cover presence.
#[derive(Debug, Clone, PartialEq)]
pub struct TagInfo {
    pub declared_rail: Rail,
    pub declared_bitrate: Option<u32>,
    pub duration_sec: f32,
    pub channels: u16,
    pub id3_version: Option<String>,
    pub tags_cdj_ok: bool,
    pub has_cover: bool,
}

/// Lossless vs lossy from the file extension (container/codec lineage).
pub fn rail_from_ext(ext: &str) -> Rail {
    match ext.to_ascii_lowercase().as_str() {
        "flac" | "wav" | "aif" | "aiff" | "alac" => Rail::Lossless,
        "mp3" | "aac" | "m4a" | "ogg" | "opus" => Rail::Lossy,
        _ => Rail::Unknown,
    }
}

/// Reads tag/property info. On unreadable container, returns a conservative Unknown info
/// (the caller still has decode results + codec_error).
pub fn read(path: &str) -> TagInfo {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let rail = rail_from_ext(ext);

    match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => {
            let props = tagged.properties();
            let has_cover = tagged.tags().iter().any(|t| !t.pictures().is_empty());
            let id3_version = if ext.eq_ignore_ascii_case("mp3") {
                Some("ID3".to_string())
            } else {
                None
            };
            let tags_cdj_ok = tagged.tags().iter().any(|t| {
                t.get_string(&ItemKey::TrackArtist).is_some()
                    && t.get_string(&ItemKey::TrackTitle).is_some()
            });
            TagInfo {
                declared_rail: rail,
                declared_bitrate: props.audio_bitrate(),
                duration_sec: props.duration().as_secs_f32(),
                channels: props.channels().unwrap_or(0) as u16,
                id3_version,
                tags_cdj_ok,
                has_cover,
            }
        }
        Err(_) => TagInfo {
            declared_rail: rail,
            declared_bitrate: None,
            duration_sec: 0.0,
            channels: 0,
            id3_version: None,
            tags_cdj_ok: false,
            has_cover: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_from_ext_classifies_known_formats() {
        assert_eq!(rail_from_ext("flac"), Rail::Lossless);
        assert_eq!(rail_from_ext("FLAC"), Rail::Lossless);
        assert_eq!(rail_from_ext("mp3"), Rail::Lossy);
        assert_eq!(rail_from_ext("xyz"), Rail::Unknown);
    }

    #[test]
    fn read_missing_file_is_conservative() {
        let info = read("does-not-exist.flac");
        assert_eq!(info.declared_rail, Rail::Lossless);
        assert_eq!(info.channels, 0);
        assert!(!info.has_cover);
    }
}
