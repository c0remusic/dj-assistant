//! Windowed FFT (rustfft) → long-term average spectrum (LTAS), cutoff-frequency detection,
//! and a downsampled spectrogram. Online over mono f32 blocks.

use crate::analysis::Spectrogram;
use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

/// Result of the spectral pass.
pub struct SpectrumResult {
    pub cutoff_hz: f32,
    pub spectrogram: Spectrogram,
}

/// Online windowed-FFT accumulator. Buffers samples into `fft_size` Hann frames (50% hop),
/// accumulates the LTAS, and stores time-downsampled spectrogram columns.
pub struct SpectrumAccumulator {
    sr: u32,
    fft_size: usize,
    hop: usize,
    fft: Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    buf: Vec<f32>,
    ltas: Vec<f64>,
    frames_total: u64,
    spec_stride: u64,
    spec_cols: Vec<Vec<u8>>,
    bins: usize,
}

impl SpectrumAccumulator {
    pub fn new(sr: u32, fft_size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (fft_size as f32 - 1.0)).cos())
            .collect();
        let bins = fft_size / 2;
        Self {
            sr,
            fft_size,
            hop: fft_size / 2,
            fft,
            window,
            buf: Vec::with_capacity(fft_size * 2),
            ltas: vec![0.0; bins],
            frames_total: 0,
            spec_stride: 4,
            spec_cols: Vec::new(),
            bins,
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        self.buf.extend_from_slice(mono);
        while self.buf.len() >= self.fft_size {
            self.process_frame();
            self.buf.drain(0..self.hop);
        }
    }

    fn process_frame(&mut self) {
        let mut scratch: Vec<Complex<f32>> = (0..self.fft_size)
            .map(|i| Complex { re: self.buf[i] * self.window[i], im: 0.0 })
            .collect();
        self.fft.process(&mut scratch);
        let mut mags = vec![0.0f32; self.bins];
        for k in 0..self.bins {
            let m2 = scratch[k].norm_sqr();
            self.ltas[k] += m2 as f64;
            mags[k] = m2;
        }
        if self.frames_total % self.spec_stride == 0 {
            let col: Vec<u8> = mags.iter().map(|&m2| {
                let db = if m2 <= 1e-12 { -100.0 } else { 10.0 * m2.log10() };
                let clamped = db.clamp(-100.0, 0.0);
                ((clamped + 100.0) / 100.0 * 255.0) as u8
            }).collect();
            self.spec_cols.push(col);
        }
        self.frames_total += 1;
    }

    /// Detect the cutoff: scan bins from Nyquist downward; the cutoff is the top edge of
    /// sustained energy (LTAS in dB above a floor set relative to the spectral peak).
    fn detect_cutoff(&self) -> f32 {
        if self.frames_total == 0 { return 0.0; }
        let hz_per_bin = self.sr as f32 / self.fft_size as f32;
        let avg_db: Vec<f32> = self.ltas.iter().map(|&s| {
            let avg = s / self.frames_total as f64;
            if avg <= 1e-12 { -120.0 } else { 10.0 * (avg as f32).log10() }
        }).collect();
        let peak_db = avg_db.iter().cloned().fold(-120.0f32, f32::max);
        let floor = peak_db - 50.0;
        let run_needed = 3usize;
        let mut run = 0usize;
        for k in (1..self.bins).rev() {
            if avg_db[k] > floor {
                run += 1;
                if run >= run_needed {
                    let top_bin = k + run_needed - 1;
                    return top_bin as f32 * hz_per_bin;
                }
            } else {
                run = 0;
            }
        }
        0.0
    }

    pub fn finish(self) -> SpectrumResult {
        let cutoff_hz = self.detect_cutoff();
        SpectrumResult {
            cutoff_hz,
            spectrogram: self.build_spectrogram(),
        }
    }

    /// Builds a display-sized spectrogram: caps time columns to `MAX_COLS` and pools the
    /// frequency bins down to ~`DISPLAY_BINS` (max-pool). Keeps the UI payload small and
    /// bounded regardless of track length. Cutoff detection is unaffected — it runs on the
    /// full-resolution LTAS, not on these display columns.
    fn build_spectrogram(&self) -> Spectrogram {
        const MAX_COLS: usize = 800;
        const DISPLAY_BINS: usize = 256;

        let src_cols = self.spec_cols.len();
        if src_cols == 0 || self.bins == 0 {
            return Spectrogram { frames: 0, bins: 0, hz_per_bin: 0.0, sec_per_frame: 0.0, mag_db: vec![] };
        }

        let col_stride = src_cols.div_ceil(MAX_COLS).max(1);
        let bin_pool = self.bins.div_ceil(DISPLAY_BINS).max(1);
        let out_bins = self.bins.div_ceil(bin_pool);

        let src_hz_per_bin = self.sr as f32 / self.fft_size as f32;
        let hz_per_bin = src_hz_per_bin * bin_pool as f32;
        let sec_per_frame =
            (self.hop as f32 / self.sr as f32) * self.spec_stride as f32 * col_stride as f32;

        let mut out_cols: Vec<Vec<u8>> = Vec::with_capacity(src_cols.div_ceil(col_stride));
        let mut ci = 0;
        while ci < src_cols {
            let col = &self.spec_cols[ci];
            let mut pooled = vec![0u8; out_bins];
            for b in 0..self.bins {
                let ob = b / bin_pool;
                if col[b] > pooled[ob] { pooled[ob] = col[b]; }
            }
            out_cols.push(pooled);
            ci += col_stride;
        }

        let frames = out_cols.len();
        let mut mag_db = Vec::with_capacity(frames * out_bins);
        for col in &out_cols { mag_db.extend_from_slice(col); }
        Spectrogram { frames, bins: out_bins, hz_per_bin, sec_per_frame, mag_db }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 44100;

    /// Hard band-limited signal: a dense sum of equal-amplitude sine tones spaced every
    /// 100 Hz from 100 Hz up to `top_hz`. There is **no** energy above `top_hz`, so the
    /// detector should report a cutoff at ~`top_hz` — this models the sharp lowpass cliff
    /// of a lossy transcode (the real fake-detection target), not a gentle analog rolloff.
    fn band_limited_tones(sr: u32, secs: f32, top_hz: f32) -> Vec<f32> {
        let n = (sr as f32 * secs) as usize;
        let freqs: Vec<f32> = (1..)
            .map(|k| k as f32 * 100.0)
            .take_while(|&f| f <= top_hz)
            .collect();
        let amp = 0.5 / freqs.len() as f32;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                freqs.iter().map(|&f| (2.0 * PI * f * t).sin()).sum::<f32>() * amp
            })
            .collect()
    }

    #[test]
    fn cutoff_detected_near_hard_band_edge() {
        let sig = band_limited_tones(SR, 2.0, 6000.0);
        let mut a = SpectrumAccumulator::new(SR, 4096);
        a.push(&sig);
        let report = a.finish();
        assert!(report.cutoff_hz > 5000.0 && report.cutoff_hz < 7500.0,
            "cutoff {} should sit at the ~6 kHz hard edge", report.cutoff_hz);
        assert!(report.spectrogram.frames > 0);
        assert!(report.spectrogram.bins > 0);
    }

    #[test]
    fn full_band_noise_reports_high_cutoff() {
        let n = (SR as f32 * 2.0) as usize;
        let mut seed = 777u32;
        let mut sig = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            sig.push((seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0);
        }
        let mut a = SpectrumAccumulator::new(SR, 4096);
        a.push(&sig);
        let report = a.finish();
        assert!(report.cutoff_hz > 18000.0, "cutoff {} should be near Nyquist", report.cutoff_hz);
    }
}
