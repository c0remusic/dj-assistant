//! Pure verdict logic. Couples the detected cutoff with the DECLARED rail:
//! a real MP3 320 also cuts ~20 kHz — that's honest, not fake. "Fake" = declared lossless
//! but the spectrum shows a lossy lowpass cliff.

use crate::analysis::{Rail, Verdict};

/// Decision bands (Hz) for a file DECLARED lossless. `cutoff_hz` is stored raw upstream so
/// these thresholds stay reconfigurable without re-analysis (Réglages, M2b+).
pub const LOSSLESS_OK_HZ: f32 = 20000.0; // ≥ → authentic lossless
pub const LOSSY_CLIFF_HZ: f32 = 19500.0; // ≤ → lossy lowpass cliff → fake
// (LOSSY_CLIFF_HZ, LOSSLESS_OK_HZ) → grey zone

/// Maps cutoff + declared rail to a verdict.
pub fn verdict(cutoff_hz: f32, declared: Rail) -> Verdict {
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
        Rail::Lossy => Verdict::Ok,
        Rail::Unknown => Verdict::Grey,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_with_full_band_is_ok() {
        assert_eq!(verdict(21000.0, Rail::Lossless), Verdict::Ok);
    }

    #[test]
    fn lossless_with_lossy_cliff_is_fake() {
        assert_eq!(verdict(16000.0, Rail::Lossless), Verdict::Fake);
        assert_eq!(verdict(19000.0, Rail::Lossless), Verdict::Fake);
    }

    #[test]
    fn lossless_in_grey_band_is_grey() {
        assert_eq!(verdict(19800.0, Rail::Lossless), Verdict::Grey);
    }

    #[test]
    fn lossy_is_never_fake_via_this_path() {
        assert_eq!(verdict(16000.0, Rail::Lossy), Verdict::Ok);
        assert_eq!(verdict(20000.0, Rail::Lossy), Verdict::Ok);
    }

    #[test]
    fn unknown_rail_is_grey() {
        assert_eq!(verdict(16000.0, Rail::Unknown), Verdict::Grey);
    }
}
