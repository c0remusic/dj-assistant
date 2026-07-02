//! End-to-end M2a characterization on fabricated fixtures. Pins verdict + signal behavior.
//! Skips gracefully if fixtures are missing (run `node scripts/make-fixtures.mjs`).
use sift_lib::analysis::{analyze, Rail, Verdict};
use std::path::Path;

fn fixture(name: &str) -> Option<String> {
    let p = format!("fixtures/{name}");
    if Path::new(&p).exists() { Some(p) } else { None }
}

#[test]
fn real_lossless_flac_is_ok() {
    let Some(p) = fixture("real_lossless.flac") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossless);
    assert!(r.cutoff_hz > 18000.0, "full-band cutoff, got {}", r.cutoff_hz);
    assert_ne!(r.verdict, Verdict::Fake, "genuine lossless must not be fake");
}

#[test]
fn fake_lossless_flac_is_fake() {
    let Some(p) = fixture("fake_lossless.flac") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossless);
    assert!(r.cutoff_hz < 18000.0, "transcoded cliff, got {}", r.cutoff_hz);
    assert_eq!(r.verdict, Verdict::Fake);
}

#[test]
fn honest_320_mp3_is_not_fake() {
    let Some(p) = fixture("real_320.mp3") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossy);
    assert_eq!(r.verdict, Verdict::Ok, "lossy is never fake via cutoff path");
}

#[test]
fn over_encoded_320_is_fake() {
    let Some(p) = fixture("over_encoded_320.mp3") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossy);
    assert!(r.cutoff_hz < 18500.0, "real cutoff well below a genuine 320, got {}", r.cutoff_hz);
    assert_eq!(r.verdict, Verdict::Fake, "declared 320 but cuts low → over-encoded fraud");
}

#[test]
fn truncated_wav_is_flagged() {
    let Some(p) = fixture("truncated.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert!(r.truncated, "abrupt 1.5 s cut should flag truncation");
}

#[test]
fn silence_pad_measured() {
    let Some(p) = fixture("silence_pad.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert!(r.silence_head_ms >= 800 && r.silence_head_ms <= 1200, "head {}", r.silence_head_ms);
    assert!(r.silence_tail_ms >= 1300 && r.silence_tail_ms <= 1700, "tail {}", r.silence_tail_ms);
}

#[test]
fn dual_mono_detected() {
    let Some(p) = fixture("dual_mono.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p, false).expect("analyze");
    assert!(r.dual_mono, "duplicated-mono stereo should be dual_mono");
    assert!(r.phase_correlation > 0.99, "corr {}", r.phase_correlation);
}

// Authentic anchors (only run if the user dropped real files in fixtures/)
#[test]
fn anchor_real_lossless_not_fake() {
    let Some(p) = fixture("anchor_real_lossless.flac") else { return; };
    let r = analyze(&p, false).expect("analyze");
    assert_ne!(r.verdict, Verdict::Fake);
}

// The analysis must map FFT bins to Hz using the file's TRUE sample rate. For a 48 k file,
// `hz_per_bin` must be 48000/FFT_SIZE — not the legacy hardcoded 44100/FFT_SIZE. This is the
// crux of authenticity: a wrong rate skews every frequency, including the cutoff verdict.
#[test]
fn analysis_uses_native_sample_rate_for_frequency_mapping() {
    let (Some(p44), Some(p48)) = (fixture("fake_lossless.flac"), fixture("fake_lossless_48k.flac"))
    else {
        eprintln!("skip: no fixture");
        return;
    };
    // hz_per_bin scales linearly with the sample rate used by the analyzer. Derive the
    // expected 48 k value from the 44.1 k reference, so we don't hardcode the bin count.
    let h44 = analyze(&p44, true).expect("analyze 44k").spectrogram.hz_per_bin;
    let r48 = analyze(&p48, true).expect("analyze 48k"); // with_spectrogram → hz_per_bin populated
    assert_eq!(r48.sample_rate, 48000, "report should carry the native rate");
    let expected = h44 * 48000.0 / 44100.0;
    assert!(
        (r48.spectrogram.hz_per_bin - expected).abs() < 0.5,
        "hz_per_bin {} should scale with native 48 k (expected {}), not stay at the hardcoded 44.1 k",
        r48.spectrogram.hz_per_bin,
        expected
    );
}
