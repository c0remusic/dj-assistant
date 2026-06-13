//! M2a audio analysis engine. One FFmpeg decode → online accumulators → AnalysisReport.
//! Pure: no DB writes, no UI. See docs/superpowers/specs/2026-06-12-m2a-analysis-engine-design.md
use serde::{Deserialize, Serialize};

pub mod decode;
pub mod dynamics;
pub mod peaks;
pub mod phase;
pub mod spectrum;
pub mod structure;
pub mod tags;
pub mod verdict;

/// Real signal lineage, independent of the declared extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rail {
    Lossless,
    Lossy,
    Unknown,
}

/// Authenticity verdict, derived from cutoff + declared rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Fake,
    Grey,
}

/// Time×frequency magnitude grid (dB, quantized to u8) for the UI spectrogram (M2c).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spectrogram {
    pub frames: usize,
    pub bins: usize,
    pub hz_per_bin: f32,
    pub sec_per_frame: f32,
    /// `frames * bins` values, row-major by frame. 0 = -100 dBFS, 255 = 0 dBFS.
    pub mag_db: Vec<u8>,
}

/// The full analysis result for one file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisReport {
    pub path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_sec: f32,
    pub declared_format: String,
    pub declared_bitrate: Option<u32>,
    pub declared_rail: Rail,
    pub cutoff_hz: f32,
    pub verdict: Verdict,
    pub peaks: Vec<f32>,
    pub spectrogram: Spectrogram,
    pub clip_runs: u32,
    pub clip_pct: f32,
    pub true_peak_dbtp: f32,
    pub dc_offset: f32,
    pub phase_correlation: f32,
    pub dual_mono: bool,
    pub container_ok: bool,
    pub codec_error: Option<String>,
    pub truncated: bool,
    pub silence_head_ms: u32,
    pub silence_tail_ms: u32,
    pub id3_version: Option<String>,
    pub tags_cdj_ok: bool,
    pub has_cover: bool,
}

use dynamics::{ClipAccumulator, DcAccumulator, TruePeakAccumulator};
use peaks::PeaksAccumulator;
use phase::PhaseAccumulator;
use spectrum::SpectrumAccumulator;
use structure::{SilenceAccumulator, TruncationAccumulator};

const FFT_SIZE: usize = 4096;
const PEAKS_WINDOW: usize = 512; // ~11.6 ms @ 44.1k
const CLIP_THRESHOLD: f32 = 0.99;
const CLIP_MIN_RUN: usize = 3;
const SILENCE_THRESHOLD: f32 = 0.001; // ~ -60 dBFS

/// Runs the full analysis: one decode, all analyzers in a single streaming pass.
/// `with_spectrogram`: build the (heavy) display spectrogram grid. The verdict and all
/// scalar signals are identical either way — only the display grid is gated. Batch (M2b)
/// passes false; the Revue UI / debug overlay pass true on demand.
pub fn analyze(path: &str, with_spectrogram: bool) -> Result<AnalysisReport, String> {
    let started = std::time::Instant::now();
    // declared properties / tags (no decode)
    let tag = tags::read(path);
    let target_ch = if tag.channels >= 2 { 2 } else { 1 };

    let sr = 44100u32;
    let mut dc = DcAccumulator::new();
    let mut clip = ClipAccumulator::new(CLIP_THRESHOLD, CLIP_MIN_RUN);
    let mut tp = TruePeakAccumulator::new();
    let mut sil = SilenceAccumulator::new(sr, SILENCE_THRESHOLD);
    let mut trunc = TruncationAccumulator::new(sr);
    let mut pk = PeaksAccumulator::new(PEAKS_WINDOW);
    let mut spec = SpectrumAccumulator::new(sr, FFT_SIZE, with_spectrogram);
    let mut ph = PhaseAccumulator::new();

    let info = decode::decode_pcm(path, target_ch, |block| {
        if target_ch == 2 {
            ph.push(block); // interleaved L,R
            let mono: Vec<f32> = block
                .chunks_exact(2)
                .map(|lr| 0.5 * (lr[0] + lr[1]))
                .collect();
            dc.push(&mono); clip.push(&mono); tp.push(&mono);
            sil.push(&mono); trunc.push(&mono); pk.push(&mono); spec.push(&mono);
        } else {
            dc.push(block); clip.push(block); tp.push(block);
            sil.push(block); trunc.push(block); pk.push(block); spec.push(block);
        }
    })?;

    let (clip_runs, clip_pct) = clip.finish();
    let (silence_head_ms, silence_tail_ms) = sil.finish();
    let truncated = trunc.finish(info.codec_error.is_some());
    let spec_res = spec.finish();
    let phase_correlation = if target_ch == 2 { ph.correlation() } else { 0.0 };
    let dual_mono = target_ch == 2 && ph.dual_mono();

    let declared_format = std::path::Path::new(path)
        .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let cutoff_hz = spec_res.cutoff_hz;
    let verdict = verdict::verdict(cutoff_hz, tag.declared_rail, tag.declared_bitrate);

    log::info!(
        "analyze {} : {} ms (decode+dsp, {} ch, {:.1}s, spectro={})",
        path,
        started.elapsed().as_millis(),
        info.channels,
        tag.duration_sec,
        with_spectrogram
    );

    Ok(AnalysisReport {
        path: path.to_string(),
        sample_rate: info.sample_rate,
        channels: info.channels,
        duration_sec: tag.duration_sec,
        declared_format,
        declared_bitrate: tag.declared_bitrate,
        declared_rail: tag.declared_rail,
        cutoff_hz,
        verdict,
        peaks: pk.finish(),
        spectrogram: spec_res.spectrogram,
        clip_runs,
        clip_pct,
        true_peak_dbtp: tp.finish(),
        dc_offset: dc.finish(),
        phase_correlation,
        dual_mono,
        container_ok: info.codec_error.is_none(),
        codec_error: info.codec_error,
        truncated,
        silence_head_ms,
        silence_tail_ms,
        id3_version: tag.id3_version,
        tags_cdj_ok: tag.tags_cdj_ok,
        has_cover: tag.has_cover,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_to_json() {
        let r = AnalysisReport {
            path: "x.flac".into(),
            sample_rate: 44100,
            channels: 2,
            duration_sec: 1.0,
            declared_format: "flac".into(),
            declared_bitrate: None,
            declared_rail: Rail::Lossless,
            cutoff_hz: 21000.0,
            verdict: Verdict::Ok,
            peaks: vec![0.0, 1.0],
            spectrogram: Spectrogram { frames: 0, bins: 0, hz_per_bin: 0.0, sec_per_frame: 0.0, mag_db: vec![] },
            clip_runs: 0,
            clip_pct: 0.0,
            true_peak_dbtp: -1.0,
            dc_offset: 0.0,
            phase_correlation: 1.0,
            dual_mono: false,
            container_ok: true,
            codec_error: None,
            truncated: false,
            silence_head_ms: 0,
            silence_tail_ms: 0,
            id3_version: None,
            tags_cdj_ok: true,
            has_cover: false,
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"verdict\":\"ok\""));
        assert!(j.contains("\"declared_rail\":\"lossless\""));
    }
}
