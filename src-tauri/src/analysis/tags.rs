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

/// Lossless vs lossy from the file's ACTUAL content (lofty's content-sniffing probe, magic
/// bytes — NOT the extension). Used only where extension trust actually matters: the filing
/// no-upscale guard (`filing.rs::plan_file`), to catch a lossy file mislabeled with a lossless
/// extension (e.g. an MP3 renamed `.flac`) before it gets "converted" into a fabricated lossless
/// file. The analysis pipeline itself doesn't need this — Symphonia already decodes real content
/// regardless of extension. `Rail::Unknown` on anything not confidently identified (unreadable
/// file, exotic/ambiguous container) — this function must never manufacture a mismatch it can't
/// back with a confident read.
pub fn rail_from_content(path: &str) -> Rail {
    use lofty::file::FileType;
    fn try_read(path: &str) -> lofty::error::Result<lofty::file::TaggedFile> {
        Probe::open(path)?.guess_file_type()?.read()
    }
    match try_read(path) {
        Ok(tagged) => match tagged.file_type() {
            FileType::Flac | FileType::Wav | FileType::Aiff | FileType::Ape | FileType::WavPack => {
                Rail::Lossless
            }
            FileType::Mpeg | FileType::Vorbis | FileType::Opus | FileType::Speex => Rail::Lossy,
            _ => Rail::Unknown,
        },
        Err(_) => Rail::Unknown,
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
                t.get_string(ItemKey::TrackArtist).is_some()
                    && t.get_string(ItemKey::TrackTitle).is_some()
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

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        std::path::Path::new(&p).exists().then_some(p)
    }

    /// The exact BUG-1 scenario: an MP3 renamed with a `.flac` extension. `rail_from_ext`
    /// (extension only) is fooled and says Lossless; `rail_from_content` (magic bytes) must see
    /// through the renamed extension and correctly report Lossy.
    #[test]
    fn rail_from_content_sees_through_a_renamed_mp3() {
        let Some(mp3) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let disguised = dir.path().join("disguised.flac");
        std::fs::copy(&mp3, &disguised).unwrap();
        let path = disguised.to_str().unwrap();

        assert_eq!(rail_from_ext("flac"), Rail::Lossless, "extension alone is fooled");
        assert_eq!(rail_from_content(path), Rail::Lossy, "content sniffing is not fooled");
    }

    /// A genuine FLAC must not be misclassified by content sniffing (no false positive).
    #[test]
    fn rail_from_content_confirms_a_real_flac() {
        let Some(flac) = fixture("real_lossless.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        assert_eq!(rail_from_content(&flac), Rail::Lossless);
    }
}
