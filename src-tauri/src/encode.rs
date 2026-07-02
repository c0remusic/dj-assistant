//! Transcoding to the two CDJ rails (MP3 320 CBR / AIFF 16-bit 44.1 kHz) via the bundled
//! ffmpeg, plus the rail-based target choice, a conformance test (skip re-encoding files
//! already in target shape), and a hard no-upscale guard. The caller passes the source
//! rail (it already has it from the analysis report), so this module is independent of
//! the analysis pipeline. ffmpeg is driven exactly like `analysis/decode.rs`.

use crate::analysis::Rail;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use lofty::file::AudioFile;
use lofty::probe::Probe;
use serde::{Deserialize, Serialize};

/// The output shapes. Lossless rail → AIFF or WAV 16-bit/44.1; lossy rail → MP3 320 CBR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    #[serde(rename = "mp3_320")]
    Mp3320,
    #[serde(rename = "aiff_16_44")]
    Aiff1644,
    #[serde(rename = "wav_16_44")]
    Wav1644,
}

impl Target {
    /// Output file extension for this target.
    pub fn ext(self) -> &'static str {
        match self {
            Target::Mp3320 => "mp3",
            Target::Aiff1644 => "aiff",
            Target::Wav1644 => "wav",
        }
    }

    /// The rail this target belongs to (used by the no-upscale guard).
    pub fn rail(self) -> Rail {
        match self {
            Target::Mp3320 => Rail::Lossy,
            Target::Aiff1644 | Target::Wav1644 => Rail::Lossless,
        }
    }
}

/// Why an encode could not proceed.
#[derive(Debug, Clone, PartialEq)]
pub enum EncodeError {
    /// Refused: would fabricate lossless from a lossy source.
    Upscale,
    /// ffmpeg failed (spawn, terminal log error, or empty output).
    Ffmpeg(String),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Upscale => write!(f, "refused: cannot upscale lossy to lossless"),
            EncodeError::Ffmpeg(m) => write!(f, "ffmpeg: {m}"),
        }
    }
}

/// Rail-based default target. Lossless → AIFF 16/44.1; everything else (lossy/unknown) →
/// MP3 320 (never crosses up into lossless on its own).
pub fn target_for(rail: Rail) -> Target {
    match rail {
        Rail::Lossless => Target::Aiff1644,
        _ => Target::Mp3320,
    }
}

/// Reject a target that would upscale a lossy source into a lossless container.
pub fn guard_no_upscale(source_rail: Rail, target: Target) -> Result<(), EncodeError> {
    if source_rail == Rail::Lossy && target.rail() == Rail::Lossless {
        return Err(EncodeError::Upscale);
    }
    Ok(())
}

/// Lowercased file extension (no dot), or "" when absent.
fn ext_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// True when `path` is already in `target` shape, so filing can tag+move without
/// re-encoding. MP3 target = already MP3 (any bitrate; never re-encode MP3→MP3). AIFF
/// target = .aif/.aiff at 44.1 kHz / 16-bit.
pub fn is_conformant(path: &str, target: Target) -> bool {
    let ext = ext_of(path);
    let pcm_16_44 = |containers: &[&str]| -> bool {
        if !containers.contains(&ext.as_str()) {
            return false;
        }
        match Probe::open(path).and_then(|p| p.read()) {
            Ok(t) => {
                let props = t.properties();
                props.sample_rate() == Some(44100) && props.bit_depth() == Some(16)
            }
            Err(_) => false,
        }
    };
    match target {
        Target::Mp3320 => ext == "mp3",
        Target::Aiff1644 => pcm_16_44(&["aif", "aiff"]),
        Target::Wav1644 => pcm_16_44(&["wav"]),
    }
}

/// Transcode `src` into `dst` for `target`, overwriting `dst`. Surfaces any terminal
/// ffmpeg error and refuses to report success on an empty/missing output. Does NOT apply
/// the no-upscale guard — callers (M4-3) guard before choosing a lossless target.
pub fn encode(src: &str, dst: &str, target: Target) -> Result<(), EncodeError> {
    let codec_args: &[&str] = match target {
        Target::Mp3320 => &["-vn", "-c:a", "libmp3lame", "-b:a", "320k", "-ar", "44100"],
        Target::Aiff1644 => &["-vn", "-c:a", "pcm_s16be", "-ar", "44100"],
        Target::Wav1644 => &["-vn", "-c:a", "pcm_s16le", "-ar", "44100"],
    };

    let mut child = FfmpegCommand::new()
        .input(src)
        .args(codec_args)
        .arg("-y")
        .output(dst)
        .spawn()
        .map_err(|e| EncodeError::Ffmpeg(format!("spawn failed: {e}")))?;

    let iter = child
        .iter()
        .map_err(|e| EncodeError::Ffmpeg(format!("iter failed: {e}")))?;

    let mut err: Option<String> = None;
    for ev in iter {
        match ev {
            FfmpegEvent::Log(LogLevel::Error, msg) => {
                err.get_or_insert(msg); // keep the FIRST error (usually the most informative)
            }
            // ffmpeg-sidecar emits this synthetic event whenever no output stream is routed
            // to stdout — always the case for file output. Not a real failure; the
            // output-file check below is the source of truth.
            FfmpegEvent::Error(msg) if msg != "No streams found" => {
                err.get_or_insert(msg);
            }
            _ => {}
        }
    }
    let _ = child.wait();

    if let Some(e) = err {
        return Err(EncodeError::Ffmpeg(e));
    }
    match std::fs::metadata(dst) {
        Ok(m) if m.len() > 0 => Ok(()),
        _ => Err(EncodeError::Ffmpeg("no output produced".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    /// A missing fixture quietly `return`s a "passing" test on a dev machine without
    /// `fixtures/` checked out — but in CI a missing fixture means the checkout is broken, not
    /// a supported skip, and must fail loudly instead of reporting a false green (FIX-16).
    fn skip_if_no_fixture(name: &str) {
        eprintln!("skip: no fixture ({name})");
        if std::env::var("CI").is_ok() {
            panic!("fixture missing in CI: fixtures/{name} — checkout is broken, not a supported skip");
        }
    }

    #[test]
    fn target_follows_rail() {
        assert_eq!(target_for(Rail::Lossless), Target::Aiff1644);
        assert_eq!(target_for(Rail::Lossy), Target::Mp3320);
        assert_eq!(target_for(Rail::Unknown), Target::Mp3320);
    }

    #[test]
    fn target_ext_matches() {
        assert_eq!(Target::Mp3320.ext(), "mp3");
        assert_eq!(Target::Aiff1644.ext(), "aiff");
        assert_eq!(Target::Wav1644.ext(), "wav");
        assert_eq!(Target::Wav1644.rail(), Rail::Lossless);
    }

    #[test]
    fn encodes_flac_to_conformant_wav() {
        let Some(src) = fixture("real_lossless.flac") else {
            skip_if_no_fixture("real_lossless.flac");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.wav");
        let dst = dst.to_str().unwrap();
        encode(&src, dst, Target::Wav1644).expect("encode wav");
        assert!(is_conformant(dst, Target::Wav1644), "encoded WAV must be 16-bit/44.1");
    }

    #[test]
    fn guard_blocks_lossy_to_lossless_only() {
        assert_eq!(guard_no_upscale(Rail::Lossy, Target::Aiff1644), Err(EncodeError::Upscale));
        assert!(guard_no_upscale(Rail::Lossy, Target::Mp3320).is_ok());
        assert!(guard_no_upscale(Rail::Lossless, Target::Aiff1644).is_ok());
        assert!(guard_no_upscale(Rail::Lossless, Target::Mp3320).is_ok()); // downscale allowed
    }

    #[test]
    fn mp3_is_conformant_to_mp3_target() {
        let Some(p) = fixture("real_320.mp3") else {
            skip_if_no_fixture("real_320.mp3");
            return;
        };
        assert!(is_conformant(&p, Target::Mp3320));
    }

    #[test]
    fn flac_is_not_conformant_to_either_target() {
        let Some(p) = fixture("real_lossless.flac") else {
            skip_if_no_fixture("real_lossless.flac");
            return;
        };
        assert!(!is_conformant(&p, Target::Mp3320)); // wrong codec
        assert!(!is_conformant(&p, Target::Aiff1644)); // wrong container
    }

    #[test]
    fn encodes_flac_to_conformant_aiff() {
        let Some(src) = fixture("real_lossless.flac") else {
            skip_if_no_fixture("real_lossless.flac");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path(); // point ffmpeg-sidecar at the bundled dev binary
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.aiff");
        let dst = dst.to_str().unwrap();
        encode(&src, dst, Target::Aiff1644).expect("encode aiff");
        // equivalence: the output is exactly the target shape
        assert!(is_conformant(dst, Target::Aiff1644), "encoded AIFF must be 16-bit/44.1");
    }

    #[test]
    fn encodes_flac_to_mp3_320() {
        let Some(src) = fixture("real_lossless.flac") else {
            skip_if_no_fixture("real_lossless.flac");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path(); // point ffmpeg-sidecar at the bundled dev binary
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.mp3");
        let dst = dst.to_str().unwrap();
        encode(&src, dst, Target::Mp3320).expect("encode mp3");
        assert!(is_conformant(dst, Target::Mp3320));
        assert!(std::fs::metadata(dst).unwrap().len() > 0);
    }
}
