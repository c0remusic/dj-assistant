//! Pure verdict logic, keyed on the detected cutoff + DECLARED rail/bitrate.
//!
//! Two frauds are flagged as `Fake`:
//! 1. **Fake lossless** — declared FLAC/WAV/AIFF but the spectrum shows a lossy lowpass cliff.
//! 2. **Over-encoded lossy** — declared e.g. 320 kbps MP3 but the cutoff is far below what
//!    that bitrate produces → it was re-encoded UP from a lower-quality source.
//!
//! An honestly-labelled low-bitrate MP3 (cutoff matches its bitrate) stays `Ok` here — its
//! "below the user's quality threshold" handling is a separate axis (M4 rules).

use crate::analysis::{Rail, Verdict};

/// Decision bands (Hz) for a file DECLARED lossless. `cutoff_hz` is stored raw upstream so
/// these thresholds stay reconfigurable without re-analysis (Réglages, M2b+).
pub const LOSSLESS_OK_HZ: f32 = 20000.0; // ≥ → authentic lossless
pub const LOSSY_CLIFF_HZ: f32 = 19500.0; // ≤ → lossy lowpass cliff → fake
// (LOSSY_CLIFF_HZ, LOSSLESS_OK_HZ) → grey zone

/// Minimum cutoff a *genuine* MP3 of the given bitrate should reach (≈ encoder lowpass minus
/// a margin for genre/encoder spread). A declared bitrate whose real cutoff is below this is
/// over-encoded (transcoded up from a worse source).
pub fn min_cutoff_hz_for_bitrate(kbps: u32) -> f32 {
    match kbps {
        b if b >= 320 => 19000.0,
        b if b >= 256 => 18000.0,
        b if b >= 192 => 16500.0,
        b if b >= 160 => 15500.0,
        b if b >= 128 => 14500.0,
        _ => 12000.0,
    }
}

/// Equivalent lossy bitrate for a measured cutoff, read off the SAME tiers `verdict()` uses to
/// call a bitrate over-encoded (FIX-11: this used to be duplicated in report-view.ts with a
/// shifted table — e.g. a cutoff the verdict logic scored against the 192kbps band showed as
/// "≈256 kbps" in the UI). Rust is the single source of truth; the front just displays this.
pub fn estimate_kbps(cutoff_hz: f32) -> u32 {
    const TIERS: [u32; 5] = [320, 256, 192, 160, 128];
    for b in TIERS {
        if cutoff_hz >= min_cutoff_hz_for_bitrate(b) {
            return b;
        }
    }
    128
}

/// Maps cutoff + declared rail + declared bitrate to a verdict.
pub fn verdict(cutoff_hz: f32, declared: Rail, declared_bitrate: Option<u32>) -> Verdict {
    match declared {
        Rail::Lossless => {
            if cutoff_hz >= LOSSLESS_OK_HZ {
                Verdict::Ok
            } else if cutoff_hz <= LOSSY_CLIFF_HZ {
                Verdict::Fake
            } else {
                Verdict::Grey
            }
        }
        Rail::Lossy => match declared_bitrate {
            // declared bitrate the real spectrum can't support → over-encoded fraud
            Some(b) if cutoff_hz < min_cutoff_hz_for_bitrate(b) => Verdict::Fake,
            _ => Verdict::Ok,
        },
        Rail::Unknown => Verdict::Grey,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_with_full_band_is_ok() {
        assert_eq!(verdict(21000.0, Rail::Lossless, None), Verdict::Ok);
    }

    #[test]
    fn lossless_with_lossy_cliff_is_fake() {
        assert_eq!(verdict(16000.0, Rail::Lossless, None), Verdict::Fake);
        assert_eq!(verdict(19000.0, Rail::Lossless, None), Verdict::Fake);
    }

    #[test]
    fn lossless_in_grey_band_is_grey() {
        assert_eq!(verdict(19800.0, Rail::Lossless, None), Verdict::Grey);
    }

    #[test]
    fn honest_mp3_matching_its_bitrate_is_ok() {
        // genuine 320 (~20.5k), genuine 128 (~16k)
        assert_eq!(verdict(20500.0, Rail::Lossy, Some(320)), Verdict::Ok);
        assert_eq!(verdict(16000.0, Rail::Lossy, Some(128)), Verdict::Ok);
    }

    #[test]
    fn over_encoded_mp3_is_fake() {
        // declared 320 but cuts at 16k (transcoded up from ~128) → fraud
        assert_eq!(verdict(16000.0, Rail::Lossy, Some(320)), Verdict::Fake);
        // declared 256 but cuts at 15k
        assert_eq!(verdict(15000.0, Rail::Lossy, Some(256)), Verdict::Fake);
    }

    #[test]
    fn lossy_without_known_bitrate_is_ok() {
        // can't judge over-encoding without a declared bitrate → don't false-flag
        assert_eq!(verdict(13000.0, Rail::Lossy, None), Verdict::Ok);
    }

    #[test]
    fn unknown_rail_is_grey() {
        assert_eq!(verdict(16000.0, Rail::Unknown, None), Verdict::Grey);
    }
}
