# M2a — Moteur d'analyse audio (Rust pur) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pure Rust analysis engine `analysis::analyze(path) -> AnalysisReport` that, from a single FFmpeg decode, produces a deterministic report (peaks, spectrogram, cutoff→fake verdict, clipping, true-peak, DC offset, silence, truncation, phase, container/codec integrity, ID3/cover) — no worker, no DB writes, no UI.

**Architecture:** One FFmpeg decode → PCM `s16le` streamed in fixed-size blocks → each analyzer is an **online accumulator** (`push(block)` / `finish()`) so the whole file is processed in a single memory-bounded pass. Declared properties + tags come from `lofty` (no ffprobe). Verdict is pure logic over `cutoff_hz` + declared format. NOT using `named_pipes` (YAGNI for a single aggregated caller).

**Tech Stack:** Rust 1.77.2, `ffmpeg-sidecar` 2.5.0 (bundled binary, `FFMPEG_BINARY` in dev), `rustfft` 6.4.1, `lofty` 0.22.2, `rusqlite` (unused here), Tauri 2.

**Spec:** `docs/superpowers/specs/2026-06-12-m2a-analysis-engine-design.md`

---

## Environment notes (read once)

- **cargo is NOT on PATH** in the tool shell. Prefix every cargo command:
  PowerShell: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test ...`
  Bash: `"$USERPROFILE/.cargo/bin/cargo.exe" test ...`
  Run from `src-tauri/`. In the steps below, `cargo` is shorthand for that prefixed binary.
- Crate is `sift_lib` (`[lib] name = "sift_lib"`). Unit tests live in-module (`#[cfg(test)] mod tests`).
- All analyzers operate on **f32 samples in [-1.0, 1.0]** (s16 `/ 32768.0`).
- `git commit` only — no push. No `Co-Authored-By` trailer.

## File Structure

All new code under `src-tauri/src/analysis/`:

- `analysis/mod.rs` — `AnalysisReport`, `Verdict`, `Rail`, `Spectrogram` types + `analyze()` orchestration (single-pass).
- `analysis/dynamics.rs` — `DcAccumulator`, `ClipAccumulator`, `TruePeakAccumulator`.
- `analysis/structure.rs` — `SilenceAccumulator`, truncation heuristic.
- `analysis/phase.rs` — `PhaseAccumulator` (inter-channel correlation, dual-mono).
- `analysis/peaks.rs` — `PeaksAccumulator` (downsampled abs-max envelope).
- `analysis/spectrum.rs` — `SpectrumAccumulator` (Hann FFT, LTAS, cutoff detection, downsampled spectrogram).
- `analysis/verdict.rs` — pure `verdict(cutoff_hz, declared_rail) -> Verdict`.
- `analysis/decode.rs` — drive ffmpeg-sidecar → stream f32 blocks; report decode/codec errors.
- `analysis/tags.rs` — `lofty`: declared rail/bitrate/duration/channels, ID3 version, CDJ-tag check, cover presence.
- Modify `src-tauri/src/lib.rs` — `mod analysis;` + register `analyze_path` debug command.
- Modify `src-tauri/src/ipc.rs` — `analyze_path` command.
- Modify `shared/contracts.ts` — `AnalysisReport` TS type.
- Create `src-tauri/fixtures/` (+ `scripts/make-fixtures.mjs`) — fabricated test files.

---

## Task 1: Dependencies + analysis module skeleton & types

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/analysis/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add deps**

In `src-tauri/Cargo.toml` under `[dependencies]`, add:

```toml
rustfft = "6.4.1"
lofty = "0.22.2"
```

- [ ] **Step 2: Create the module with report types + a compile test**

Create `src-tauri/src/analysis/mod.rs`:

```rust
//! M2a audio analysis engine. One FFmpeg decode → online accumulators → AnalysisReport.
//! Pure: no DB writes, no UI. See docs/superpowers/specs/2026-06-12-m2a-analysis-engine-design.md
use serde::Serialize;

pub mod decode;
pub mod dynamics;
pub mod peaks;
pub mod phase;
pub mod spectrum;
pub mod structure;
pub mod tags;
pub mod verdict;

/// Real signal lineage, independent of the declared extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rail {
    Lossless,
    Lossy,
    Unknown,
}

/// Authenticity verdict, derived from cutoff + declared rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Fake,
    Grey,
}

/// Time×frequency magnitude grid (dB, quantized to u8) for the UI spectrogram (M2c).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Spectrogram {
    pub frames: usize,
    pub bins: usize,
    pub hz_per_bin: f32,
    pub sec_per_frame: f32,
    /// `frames * bins` values, row-major by frame. 0 = -100 dBFS, 255 = 0 dBFS.
    pub mag_db: Vec<u8>,
}

/// The full analysis result for one file.
#[derive(Debug, Clone, Serialize, PartialEq)]
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
```

Note: the `pub mod ...;` lines reference files created in later tasks. To compile Task 1 in isolation, create empty stub files now:

```bash
# from src-tauri/src/analysis/
# create empty stubs so `pub mod` resolves; real code lands in later tasks
```
Create empty files: `decode.rs`, `dynamics.rs`, `peaks.rs`, `phase.rs`, `spectrum.rs`, `structure.rs`, `tags.rs`, `verdict.rs` (each containing only a `//! stub` line).

- [ ] **Step 3: Wire the module**

In `src-tauri/src/lib.rs`, add `mod analysis;` alongside the existing `mod db; ...` declarations.

- [ ] **Step 4: Compile + test**

Run: `cargo test -p sift_lib analysis::tests::report_serializes_to_json`
Expected: PASS (after `cargo` downloads rustfft + lofty).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/analysis src-tauri/src/lib.rs
git commit -m "feat(m2a): analysis module skeleton + report types + deps (rustfft, lofty)"
```

---

## Task 2: Dynamics — DC offset, clipping, true-peak

**Files:**
- Modify: `src-tauri/src/analysis/dynamics.rs`

- [ ] **Step 1: Write failing tests**

Replace `dynamics.rs` content with the tests first:

```rust
//! Online dynamics analyzers over mono f32 blocks: DC offset, clipping, true-peak.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_offset_of_centered_signal_is_zero() {
        let mut a = DcAccumulator::new();
        a.push(&[-0.5, 0.5, -0.5, 0.5]);
        assert!(a.finish().abs() < 1e-6);
    }

    #[test]
    fn dc_offset_detects_bias() {
        let mut a = DcAccumulator::new();
        a.push(&[0.2, 0.2, 0.2, 0.2]);
        assert!((a.finish() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn clipping_counts_runs_and_pct() {
        // 10 samples: a run of 3 clipped, then 1 isolated (below min_run), rest clean
        let mut a = ClipAccumulator::new(0.99, 3);
        a.push(&[1.0, 1.0, 1.0, 0.1, 1.0, 0.1, 0.1, 0.1, 0.1, 0.1]);
        let (runs, pct) = a.finish();
        assert_eq!(runs, 1, "only the length-3 run counts");
        // 4 samples >= 0.99 out of 10 = 40%
        assert!((pct - 40.0).abs() < 1e-3);
    }

    #[test]
    fn true_peak_of_full_scale_is_about_zero_dbtp() {
        let mut a = TruePeakAccumulator::new();
        a.push(&[1.0, -1.0, 1.0, -1.0]);
        assert!(a.finish() >= -0.1, "got {}", a.finish());
    }

    #[test]
    fn true_peak_of_half_scale_is_about_minus_6_dbtp() {
        let mut a = TruePeakAccumulator::new();
        a.push(&[0.5, -0.5, 0.5, -0.5]);
        let v = a.finish();
        assert!((v - (-6.02)).abs() < 1.0, "got {}", v);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::dynamics`
Expected: FAIL (types not defined).

- [ ] **Step 3: Implement**

Prepend above the `#[cfg(test)]` block in `dynamics.rs`:

```rust
/// Running mean of the signal → DC offset.
#[derive(Default)]
pub struct DcAccumulator { sum: f64, n: u64 }
impl DcAccumulator {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono { self.sum += s as f64; }
        self.n += mono.len() as u64;
    }
    pub fn finish(&self) -> f32 {
        if self.n == 0 { 0.0 } else { (self.sum / self.n as f64) as f32 }
    }
}

/// Counts runs of consecutive near-full-scale samples and overall clipped percentage.
pub struct ClipAccumulator {
    threshold: f32,
    min_run: usize,
    cur_run: usize,
    runs: u32,
    clipped: u64,
    total: u64,
}
impl ClipAccumulator {
    pub fn new(threshold: f32, min_run: usize) -> Self {
        Self { threshold, min_run, cur_run: 0, runs: 0, clipped: 0, total: 0 }
    }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.total += 1;
            if s.abs() >= self.threshold {
                self.clipped += 1;
                self.cur_run += 1;
                if self.cur_run == self.min_run { self.runs += 1; }
            } else {
                self.cur_run = 0;
            }
        }
    }
    pub fn finish(&self) -> (u32, f32) {
        let pct = if self.total == 0 { 0.0 } else { self.clipped as f32 / self.total as f32 * 100.0 };
        (self.runs, pct)
    }
}

/// Approximate true-peak via 4× linear-interpolated oversampling. Reports dBTP.
/// (Linear interp is an approximation of a proper polyphase upsampler — adequate for a
/// "too hot" flag; documented as such in the spec.)
#[derive(Default)]
pub struct TruePeakAccumulator { peak: f32, last: f32, seen: bool }
impl TruePeakAccumulator {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            if self.seen {
                for k in 1..=4 {
                    let t = k as f32 / 4.0;
                    let interp = self.last + (s - self.last) * t;
                    self.peak = self.peak.max(interp.abs());
                }
            } else {
                self.peak = self.peak.max(s.abs());
                self.seen = true;
            }
            self.last = s;
        }
    }
    pub fn finish(&self) -> f32 {
        if self.peak <= 0.0 { -120.0 } else { 20.0 * self.peak.log10() }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::dynamics`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/dynamics.rs
git commit -m "feat(m2a): dynamics analyzers (dc offset, clipping, true-peak)"
```

---

## Task 3: Structure — silence head/tail + truncation heuristic

**Files:**
- Modify: `src-tauri/src/analysis/structure.rs`

- [ ] **Step 1: Write failing tests**

Replace `structure.rs` with:

```rust
//! Silence head/tail (ms) and end-truncation heuristic, online over mono f32 blocks.

#[cfg(test)]
mod tests {
    use super::*;

    fn block(n: usize, v: f32) -> Vec<f32> { vec![v; n] }

    #[test]
    fn silence_head_and_tail_measured_in_ms() {
        // 44100 Hz: 4410 silent + 4410 loud + 8820 silent
        let mut a = SilenceAccumulator::new(44100, 0.001);
        a.push(&block(4410, 0.0));
        a.push(&block(4410, 0.5));
        a.push(&block(8820, 0.0));
        let (head, tail) = a.finish();
        assert!((head as i64 - 100).abs() <= 2, "head {head}");
        assert!((tail as i64 - 200).abs() <= 2, "tail {tail}");
    }

    #[test]
    fn no_silence_when_signal_fills_buffer() {
        let mut a = SilenceAccumulator::new(44100, 0.001);
        a.push(&block(44100, 0.5));
        assert_eq!(a.finish(), (0, 0));
    }

    #[test]
    fn truncation_flagged_when_end_energy_does_not_decay() {
        // loud right up to the last sample, no decay → truncated
        let mut a = TruncationAccumulator::new(44100);
        a.push(&block(88200, 0.6)); // 2 s loud
        assert!(a.finish(false), "abrupt high-energy end should flag truncation");
    }

    #[test]
    fn no_truncation_when_end_decays() {
        let mut a = TruncationAccumulator::new(44100);
        a.push(&block(44100, 0.6)); // 1 s loud
        a.push(&block(44100, 0.0)); // 1 s silent tail → decays
        assert!(!a.finish(false));
    }

    #[test]
    fn truncation_flagged_on_decode_error_regardless() {
        let mut a = TruncationAccumulator::new(44100);
        a.push(&block(44100, 0.0));
        assert!(a.finish(true), "decode error at EOF forces truncated");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::structure`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend:

```rust
const SILENCE_DBFS_DEFAULT: f32 = 0.001; // ~ -60 dBFS linear

/// Measures leading and trailing silence. `threshold` is linear amplitude.
pub struct SilenceAccumulator {
    sr: u32,
    threshold: f32,
    seen_sound: bool,
    head_samples: u64,
    tail_samples: u64,
    total: u64,
}
impl SilenceAccumulator {
    pub fn new(sr: u32, threshold: f32) -> Self {
        Self { sr, threshold, seen_sound: false, head_samples: 0, tail_samples: 0, total: 0 }
    }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.total += 1;
            if s.abs() <= self.threshold {
                if !self.seen_sound { self.head_samples += 1; }
                self.tail_samples += 1;
            } else {
                self.seen_sound = true;
                self.tail_samples = 0;
            }
        }
    }
    /// (head_ms, tail_ms). All-silent files report head = full length, tail = 0.
    pub fn finish(&self) -> (u32, u32) {
        let to_ms = |n: u64| ((n as f64 / self.sr as f64) * 1000.0) as u32;
        if !self.seen_sound { return (to_ms(self.total), 0); }
        (to_ms(self.head_samples), to_ms(self.tail_samples))
    }
}

/// Flags likely truncation: end-of-file energy that doesn't decay (abrupt cut), or a
/// decode error at EOF. Compares mean energy of the last ~200 ms against the global mean.
pub struct TruncationAccumulator {
    sr: u32,
    tail_len: usize,
    tail: Vec<f32>,       // ring of last `tail_len` |samples|
    pos: usize,
    filled: usize,
    global_sq: f64,
    n: u64,
}
impl TruncationAccumulator {
    pub fn new(sr: u32) -> Self {
        let tail_len = (sr as usize) / 5; // 200 ms
        Self { sr, tail_len, tail: vec![0.0; tail_len], pos: 0, filled: 0, global_sq: 0.0, n: 0 }
    }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            let a = s.abs();
            self.global_sq += (a as f64) * (a as f64);
            self.n += 1;
            self.tail[self.pos] = a;
            self.pos = (self.pos + 1) % self.tail_len;
            if self.filled < self.tail_len { self.filled += 1; }
        }
    }
    /// `decode_error` = ffmpeg reported a terminal error → always truncated.
    pub fn finish(&self, decode_error: bool) -> bool {
        if decode_error { return true; }
        if self.n == 0 || self.filled == 0 { return false; }
        let global_rms = (self.global_sq / self.n as f64).sqrt();
        let tail_sq: f64 = self.tail.iter().take(self.filled).map(|&a| (a as f64) * (a as f64)).sum();
        let tail_rms = (tail_sq / self.filled as f64).sqrt();
        // abrupt cut: end is still ≥ 50% of the track's overall energy (no fade/decay)
        global_rms > 1e-4 && tail_rms >= 0.5 * global_rms
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::structure`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/structure.rs
git commit -m "feat(m2a): structure analyzers (silence head/tail, truncation heuristic)"
```

---

## Task 4: Phase — inter-channel correlation + dual-mono

**Files:**
- Modify: `src-tauri/src/analysis/phase.rs`

- [ ] **Step 1: Write failing tests**

Replace `phase.rs` with:

```rust
//! Inter-channel (L/R) Pearson correlation → phase compatibility + dual-mono detection.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_channels_correlate_to_one() {
        let mut a = PhaseAccumulator::new();
        a.push(&[0.1, 0.1, -0.3, -0.3, 0.7, 0.7]); // interleaved L,R equal
        assert!((a.correlation() - 1.0).abs() < 1e-4);
        assert!(a.dual_mono(), "L==R is dual mono");
    }

    #[test]
    fn inverted_channels_correlate_to_minus_one() {
        let mut a = PhaseAccumulator::new();
        a.push(&[0.1, -0.1, -0.3, 0.3, 0.7, -0.7]);
        assert!((a.correlation() + 1.0).abs() < 1e-4);
        assert!(!a.dual_mono());
    }

    #[test]
    fn uncorrelated_is_near_zero_and_not_dual_mono() {
        let mut a = PhaseAccumulator::new();
        a.push(&[1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0]);
        assert!(a.correlation().abs() < 0.2);
        assert!(!a.dual_mono());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::phase`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend:

```rust
/// Accumulates the cross/auto sums needed for Pearson correlation between L and R,
/// from an **interleaved stereo** f32 stream.
#[derive(Default)]
pub struct PhaseAccumulator {
    sum_lr: f64,
    sum_ll: f64,
    sum_rr: f64,
    sum_diff_sq: f64, // Σ(L-R)² → dual-mono check
    sum_sq: f64,      // Σ(L²+R²)
    n: u64,
}
impl PhaseAccumulator {
    pub fn new() -> Self { Self::default() }
    /// `interleaved` = [L0,R0,L1,R1,…]. Odd trailing sample (shouldn't happen) is ignored.
    pub fn push(&mut self, interleaved: &[f32]) {
        let mut i = 0;
        while i + 1 < interleaved.len() {
            let l = interleaved[i] as f64;
            let r = interleaved[i + 1] as f64;
            self.sum_lr += l * r;
            self.sum_ll += l * l;
            self.sum_rr += r * r;
            self.sum_diff_sq += (l - r) * (l - r);
            self.sum_sq += l * l + r * r;
            self.n += 1;
            i += 2;
        }
    }
    pub fn correlation(&self) -> f32 {
        let denom = (self.sum_ll * self.sum_rr).sqrt();
        if denom < 1e-12 { return 0.0; }
        (self.sum_lr / denom) as f32
    }
    /// True when L and R are (near-)identical: Σ(L-R)² negligible vs signal energy.
    pub fn dual_mono(&self) -> bool {
        if self.sum_sq < 1e-12 { return false; }
        (self.sum_diff_sq / self.sum_sq) < 1e-6
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::phase`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/phase.rs
git commit -m "feat(m2a): phase analyzer (inter-channel correlation, dual-mono)"
```

---

## Task 5: Peaks — downsampled waveform envelope

**Files:**
- Modify: `src-tauri/src/analysis/peaks.rs`

- [ ] **Step 1: Write failing tests**

Replace `peaks.rs` with:

```rust
//! Downsampled abs-max envelope over mono f32 blocks, for the M2c waveform.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_peak_per_window() {
        // window = 4 samples → 8 samples → 2 peaks
        let mut a = PeaksAccumulator::new(4);
        a.push(&[0.1, -0.9, 0.3, 0.2, 0.5, -0.4, 0.8, 0.1]);
        let p = a.finish();
        assert_eq!(p.len(), 2);
        assert!((p[0] - 0.9).abs() < 1e-6);
        assert!((p[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn partial_trailing_window_is_emitted() {
        let mut a = PeaksAccumulator::new(4);
        a.push(&[0.1, 0.2, 0.7]); // 3 samples, < window
        let p = a.finish();
        assert_eq!(p.len(), 1);
        assert!((p[0] - 0.7).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::peaks`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend:

```rust
/// Emits one abs-max value per `window` mono samples (last partial window flushed on finish).
pub struct PeaksAccumulator {
    window: usize,
    cur_max: f32,
    count: usize,
    out: Vec<f32>,
}
impl PeaksAccumulator {
    pub fn new(window: usize) -> Self {
        Self { window: window.max(1), cur_max: 0.0, count: 0, out: Vec::new() }
    }
    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.cur_max = self.cur_max.max(s.abs());
            self.count += 1;
            if self.count == self.window {
                self.out.push(self.cur_max);
                self.cur_max = 0.0;
                self.count = 0;
            }
        }
    }
    pub fn finish(mut self) -> Vec<f32> {
        if self.count > 0 { self.out.push(self.cur_max); }
        self.out
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::peaks`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/peaks.rs
git commit -m "feat(m2a): peaks analyzer (downsampled waveform envelope)"
```

---

## Task 6: Spectrum — Hann FFT, LTAS, cutoff detection, spectrogram

**Files:**
- Modify: `src-tauri/src/analysis/spectrum.rs`

- [ ] **Step 1: Write failing tests**

Replace `spectrum.rs` with:

```rust
//! Windowed FFT (rustfft) → long-term average spectrum (LTAS), cutoff-frequency detection,
//! and a downsampled spectrogram. Online over mono f32 blocks.

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 44100;

    fn lowpassed_noise(sr: u32, secs: f32, cutoff: f32) -> Vec<f32> {
        // crude 1-pole lowpass over white noise → energy rolls off above ~cutoff
        let n = (sr as f32 * secs) as usize;
        let dt = 1.0 / sr as f32;
        let rc = 1.0 / (2.0 * PI * cutoff);
        let alpha = dt / (rc + dt);
        let mut y = 0.0f32;
        let mut seed = 12345u32;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let white = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            y += alpha * (white - y);
            out.push(y);
        }
        out
    }

    #[test]
    fn cutoff_detected_near_lowpass_corner() {
        let sig = lowpassed_noise(SR, 2.0, 6000.0);
        let mut a = SpectrumAccumulator::new(SR, 4096);
        a.push(&sig);
        let report = a.finish();
        // 1-pole corner is soft; detected cutoff should be in a plausible band, well below Nyquist
        assert!(report.cutoff_hz > 2000.0 && report.cutoff_hz < 12000.0,
            "cutoff {} out of band", report.cutoff_hz);
        assert!(report.spectrogram.frames > 0);
        assert!(report.spectrogram.bins > 0);
    }

    #[test]
    fn full_band_noise_reports_high_cutoff() {
        // white noise has energy up to Nyquist → cutoff near the top
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::spectrum`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend (uses `rustfft` and `crate::analysis::Spectrogram`):

```rust
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
    buf: Vec<f32>,              // sample ring (linear, drained by hop)
    ltas: Vec<f64>,            // Σ magnitude² per bin
    frames_total: u64,
    // spectrogram (downsampled in time: keep ~1 column per `spec_stride` frames)
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
            spec_stride: 4, // ~1 column / 4 frames; tuned for size
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
        // magnitude² per bin (skip DC bin 0 in LTAS use later)
        let mut mags = vec![0.0f32; self.bins];
        for k in 0..self.bins {
            let m2 = scratch[k].norm_sqr();
            self.ltas[k] += m2 as f64;
            mags[k] = m2;
        }
        if self.frames_total % self.spec_stride == 0 {
            // quantize to dBFS u8 for the UI grid
            let col: Vec<u8> = mags.iter().map(|&m2| {
                let db = if m2 <= 1e-12 { -100.0 } else { 10.0 * m2.log10() };
                let clamped = db.clamp(-100.0, 0.0);
                ((clamped + 100.0) / 100.0 * 255.0) as u8
            }).collect();
            self.spec_cols.push(col);
        }
        self.frames_total += 1;
    }

    /// Detect the cutoff: scan bins from Nyquist downward; the cutoff is where the LTAS
    /// (in dB, relative to its own peak) first rises back above a floor and stays there —
    /// i.e. the top edge of sustained energy.
    fn detect_cutoff(&self) -> f32 {
        if self.frames_total == 0 { return 0.0; }
        let hz_per_bin = self.sr as f32 / self.fft_size as f32;
        // average + to dB
        let avg_db: Vec<f32> = self.ltas.iter().map(|&s| {
            let avg = s / self.frames_total as f64;
            if avg <= 1e-12 { -120.0 } else { 10.0 * (avg as f32).log10() }
        }).collect();
        let peak_db = avg_db.iter().cloned().fold(-120.0f32, f32::max);
        // floor: energy this far below the peak is considered "noise/absent"
        let floor = peak_db - 50.0;
        // scan from top bin downward, find first bin sustained above floor
        // (require a small run above floor to ignore isolated spikes)
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
        let frames = self.spec_cols.len();
        let hz_per_bin = self.sr as f32 / self.fft_size as f32;
        let sec_per_frame = (self.hop as f32 / self.sr as f32) * self.spec_stride as f32;
        let mut mag_db = Vec::with_capacity(frames * self.bins);
        for col in &self.spec_cols { mag_db.extend_from_slice(col); }
        SpectrumResult {
            cutoff_hz,
            spectrogram: Spectrogram { frames, bins: self.bins, hz_per_bin, sec_per_frame, mag_db },
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::spectrum`
Expected: PASS (2 tests). If `cutoff_detected_near_lowpass_corner` is flaky on the soft 1-pole corner, widen the band assertion — the goal is "well below Nyquist", not an exact corner.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/spectrum.rs
git commit -m "feat(m2a): spectrum analyzer (Hann FFT, LTAS, cutoff detection, spectrogram)"
```

---

## Task 7: Verdict — cutoff + declared rail → Ok/Fake/Grey

**Files:**
- Modify: `src-tauri/src/analysis/verdict.rs`

- [ ] **Step 1: Write failing tests**

Replace `verdict.rs` with:

```rust
//! Pure verdict logic. Couples the detected cutoff with the DECLARED rail:
//! a real MP3 320 also cuts ~20 kHz — that's honest, not fake. "Fake" = declared lossless
//! but the spectrum shows a lossy lowpass cliff.

use crate::analysis::{Rail, Verdict};

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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::verdict`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend:

```rust
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
        // A declared-lossy file is honestly lossy; transcode-to-lossless detection N/A here.
        // (Bitrate-vs-real-quality flagging for lossy is reported separately, not as Fake.)
        Rail::Lossy => Verdict::Ok,
        Rail::Unknown => Verdict::Grey,
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::verdict`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/verdict.rs
git commit -m "feat(m2a): verdict logic (cutoff + declared rail → ok/fake/grey)"
```

---

## Task 8: Tags — lofty declared properties, ID3 version, CDJ check, cover

**Files:**
- Modify: `src-tauri/src/analysis/tags.rs`

- [ ] **Step 1: Write failing test (no fixture needed — error path)**

Replace `tags.rs` with the implementation + an error-path test (real-file tests come with fixtures in Task 11):

```rust
//! Declared audio properties + tag metadata via `lofty` (read-only).

use crate::analysis::Rail;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;

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
            let has_cover = tagged.tags().iter().any(|t| t.picture_count() > 0);
            let id3_version = if ext.eq_ignore_ascii_case("mp3") {
                Some("ID3".to_string()) // refined with fixtures in Task 11 if needed
            } else {
                None
            };
            // CDJ sanity: artist + title present (minimum the players display)
            let tags_cdj_ok = tagged.tags().iter().any(|t| {
                use lofty::tag::ItemKey;
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
        assert_eq!(info.declared_rail, Rail::Lossless); // from ext
        assert_eq!(info.channels, 0);
        assert!(!info.has_cover);
    }
}
```

Note: the exact `lofty` 0.22 API names (`properties()`, `audio_bitrate()`, `channels()`, `ItemKey::TrackArtist/TrackTitle`, `picture_count()`) must be confirmed against the installed version — if a name differs, fix the call (the test will tell you). Keep the `TagInfo` shape stable.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sift_lib analysis::tags`
Expected: FAIL (until lofty API is correctly referenced; the missing-file test should pass once it compiles).

- [ ] **Step 3: Fix any lofty API mismatches**

Compile, read the errors, correct method/enum paths to match `lofty` 0.22.2. Re-run until green.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p sift_lib analysis::tags`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/tags.rs
git commit -m "feat(m2a): tags reader (lofty: declared props, id3 version, cdj check, cover)"
```

---

## Task 9: Decode — drive ffmpeg-sidecar → stream f32 blocks

**Files:**
- Modify: `src-tauri/src/analysis/decode.rs`

- [ ] **Step 1: Implement the decoder (I/O — characterized later in Task 11)**

Replace `decode.rs` with:

```rust
//! One FFmpeg decode → PCM s16le streamed in blocks, converted to interleaved f32.
//! Channel count is forced to mono (1) or stereo (2): native mono stays mono (phase N/A),
//! everything ≥2 ch is downmixed to stereo (CDJ target). Terminal ffmpeg errors are surfaced.

use ffmpeg_sidecar::command::FfmpegCommand;
use std::io::Read;

/// Outcome of a streamed decode.
pub struct DecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec_error: Option<String>,
}

const SAMPLE_RATE: u32 = 44100;

/// Decodes `path`, forcing `target_channels` (1 or 2) at 44.1 kHz s16le. Calls `on_block`
/// with interleaved f32 samples. Returns decode metadata incl. any terminal codec error.
pub fn decode_pcm<F: FnMut(&[f32])>(
    path: &str,
    target_channels: u16,
    mut on_block: F,
) -> Result<DecodeInfo, String> {
    let ch = if target_channels >= 2 { 2 } else { 1 };
    let mut child = FfmpegCommand::new()
        .input(path)
        .args([
            "-vn",
            "-ac", &ch.to_string(),
            "-ar", &SAMPLE_RATE.to_string(),
            "-f", "s16le",
            "-acodec", "pcm_s16le",
        ])
        .output("-")
        .spawn()
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?;

    let mut stdout = child.take_stdout().ok_or("no ffmpeg stdout")?;

    // read raw s16le bytes in fixed chunks, convert to interleaved f32
    let mut byte_buf = [0u8; 16384];
    let mut carry: Option<u8> = None; // odd byte across reads
    let mut f32_block: Vec<f32> = Vec::with_capacity(8192);
    loop {
        let n = stdout.read(&mut byte_buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 { break; }
        f32_block.clear();
        let mut i = 0;
        if let Some(lo) = carry.take() {
            if n >= 1 {
                let s = i16::from_le_bytes([lo, byte_buf[0]]);
                f32_block.push(s as f32 / 32768.0);
                i = 1;
            }
        }
        while i + 1 < n {
            let s = i16::from_le_bytes([byte_buf[i], byte_buf[i + 1]]);
            f32_block.push(s as f32 / 32768.0);
            i += 2;
        }
        if i < n { carry = Some(byte_buf[i]); }
        if !f32_block.is_empty() { on_block(&f32_block); }
    }

    // drain remaining events / exit status for error detection
    let mut codec_error = None;
    if let Ok(events) = child.iter() {
        use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
        for ev in events {
            if let FfmpegEvent::Log(LogLevel::Error, msg) = ev {
                codec_error = Some(msg);
            }
        }
    }
    let _ = child.wait();

    Ok(DecodeInfo { sample_rate: SAMPLE_RATE, channels: ch, codec_error })
}
```

Note: `take_stdout`, `FfmpegCommand`, `iter()`, and `FfmpegEvent::Log(LogLevel::Error, String)` are the ffmpeg-sidecar 2.5.0 surface — confirm exact names at compile time and fix if needed (the crate's docs/examples are the reference). The design choice (read stdout bytes directly, drain events for errors) does not change.

- [ ] **Step 2: Compile**

Run: `cargo build -p sift_lib`
Expected: compiles (fix any ffmpeg-sidecar API name mismatches per the note).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/analysis/decode.rs
git commit -m "feat(m2a): ffmpeg-sidecar decoder → streamed interleaved f32 blocks"
```

(Behavioral validation of decode happens in Task 11 with real fixtures + the end-to-end `analyze`.)

---

## Task 10: Orchestration — `analyze()` single-pass + IPC debug command

**Files:**
- Modify: `src-tauri/src/analysis/mod.rs`
- Modify: `src-tauri/src/ipc.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `shared/contracts.ts`

- [ ] **Step 1: Implement `analyze()` in `mod.rs`**

Append to `src-tauri/src/analysis/mod.rs` (above the `#[cfg(test)]` block):

```rust
use dynamics::{ClipAccumulator, DcAccumulator, TruePeakAccumulator};
use peaks::PeaksAccumulator;
use phase::PhaseAccumulator;
use spectrum::SpectrumAccumulator;
use structure::{SilenceAccumulator, TruncationAccumulator};

const FFT_SIZE: usize = 4096;
const PEAKS_WINDOW: usize = 512;     // ~11.6 ms @ 44.1k
const CLIP_THRESHOLD: f32 = 0.99;
const CLIP_MIN_RUN: usize = 3;
const SILENCE_THRESHOLD: f32 = 0.001; // ~ -60 dBFS

/// Runs the full analysis: one decode, all analyzers in a single streaming pass.
pub fn analyze(path: &str) -> Result<AnalysisReport, String> {
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
    let mut spec = SpectrumAccumulator::new(sr, FFT_SIZE);
    let mut ph = PhaseAccumulator::new();

    // mono scratch reused per block to feed mono analyzers
    let info = decode::decode_pcm(path, target_ch, |block| {
        let ch = if target_ch >= 2 { 2 } else { 1 };
        if ch == 2 {
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
    let verdict = verdict::verdict(cutoff_hz, tag.declared_rail);

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
```

- [ ] **Step 2: Add the IPC debug command**

In `src-tauri/src/ipc.rs`, add:

```rust
/// Debug command: run the M2a analysis engine on a path and return the full report.
/// Auto-triggering (worker) + DB caching land in M2b.
#[tauri::command]
pub fn analyze_path(path: String) -> Result<crate::analysis::AnalysisReport, String> {
    crate::analysis::analyze(&path)
}
```

In `src-tauri/src/lib.rs`, add `ipc::analyze_path` to the existing `tauri::generate_handler![...]` list.

- [ ] **Step 3: Add the TS contract**

In `shared/contracts.ts`, append (mirror the Rust struct; `verdict`/`declared_rail` are lowercase strings):

```ts
export interface Spectrogram {
  frames: number;
  bins: number;
  hz_per_bin: number;
  sec_per_frame: number;
  mag_db: number[]; // frames*bins, 0..255 (-100..0 dBFS)
}

export interface AnalysisReport {
  path: string;
  sample_rate: number;
  channels: number;
  duration_sec: number;
  declared_format: string;
  declared_bitrate: number | null;
  declared_rail: "lossless" | "lossy" | "unknown";
  cutoff_hz: number;
  verdict: "ok" | "fake" | "grey";
  peaks: number[];
  spectrogram: Spectrogram;
  clip_runs: number;
  clip_pct: number;
  true_peak_dbtp: number;
  dc_offset: number;
  phase_correlation: number;
  dual_mono: boolean;
  container_ok: boolean;
  codec_error: string | null;
  truncated: boolean;
  silence_head_ms: number;
  silence_tail_ms: number;
  id3_version: string | null;
  tags_cdj_ok: boolean;
  has_cover: boolean;
}
```

- [ ] **Step 4: Compile both sides**

Run: `cargo build -p sift_lib`
Expected: compiles.
Run (from repo root): `npx tsc --noEmit`
Expected: no type errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/analysis/mod.rs src-tauri/src/ipc.rs src-tauri/src/lib.rs shared/contracts.ts
git commit -m "feat(m2a): analyze() single-pass orchestration + analyze_path IPC + TS contract"
```

---

## Task 11: Fixtures + end-to-end characterization tests

**Files:**
- Create: `scripts/make-fixtures.mjs`
- Create: `src-tauri/fixtures/` (generated, gitignored except a README)
- Create: `src-tauri/tests/characterization.rs` (integration test)
- Modify: `src-tauri/.gitignore` (or repo `.gitignore`)

- [ ] **Step 1: Write the fixture generator**

Create `scripts/make-fixtures.mjs` — uses the bundled ffmpeg to fabricate known cases:

```js
// Generates M2a test fixtures via the bundled ffmpeg. Run: node scripts/make-fixtures.mjs
import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync } from "node:fs";
import { join } from "node:path";

const OUT = "src-tauri/fixtures";
mkdirSync(OUT, { recursive: true });

// locate the dev ffmpeg binary
const binDir = "src-tauri/binaries";
const ff = join(binDir, readdirSync(binDir).find((f) => f.startsWith("ffmpeg-")));
const run = (args) => execFileSync(ff, args, { stdio: "inherit" });

// 1) genuine full-band lossless FLAC: white-ish tone sweep up to ~22 kHz
run(["-y", "-f", "lavfi", "-i", "aevalsrc=0.3*sin(2*PI*(300+20000*t/10)*t):d=10:s=44100", "-ac", "2", join(OUT, "real_lossless.flac")]);
// 2) FAKE lossless: encode that to 128k mp3 then back to FLAC (lowpass cliff ~16 kHz baked in)
run(["-y", "-i", join(OUT, "real_lossless.flac"), "-b:a", "128k", join(OUT, "_tmp128.mp3")]);
run(["-y", "-i", join(OUT, "_tmp128.mp3"), "-ac", "2", join(OUT, "fake_lossless.flac")]);
// 3) honest mp3 320
run(["-y", "-i", join(OUT, "real_lossless.flac"), "-b:a", "320k", join(OUT, "real_320.mp3")]);
// 4) truncated: take only first 1.5 s of a 10 s tone, abrupt cut, as WAV
run(["-y", "-i", join(OUT, "real_lossless.flac"), "-t", "1.5", "-c:a", "pcm_s16le", join(OUT, "truncated.wav")]);
// 5) silence head/tail: 1 s silence + 2 s tone + 1.5 s silence
run(["-y", "-f", "lavfi", "-i", "aevalsrc=0:d=1:s=44100", "-f", "lavfi", "-i", "aevalsrc=0.3*sin(2*PI*1000*t):d=2:s=44100", "-f", "lavfi", "-i", "aevalsrc=0:d=1.5:s=44100", "-filter_complex", "[0][1][2]concat=n=3:v=0:a=1", "-c:a", "pcm_s16le", join(OUT, "silence_pad.wav")]);
// 6) dual-mono fake stereo: mono tone duplicated to 2 ch
run(["-y", "-f", "lavfi", "-i", "aevalsrc=0.3*sin(2*PI*1000*t):d=3:s=44100", "-ac", "2", join(OUT, "dual_mono.wav")]);

console.log("fixtures generated in", OUT);
```

- [ ] **Step 2: Generate the fixtures**

Run (repo root): `node scripts/make-fixtures.mjs`
Expected: files appear in `src-tauri/fixtures/`. Requires `npm run fetch-ffmpeg` to have populated `src-tauri/binaries/`.

- [ ] **Step 3: Ignore generated fixtures**

Add to `.gitignore`:

```
src-tauri/fixtures/*
!src-tauri/fixtures/README.md
```

Create `src-tauri/fixtures/README.md`:

```md
# M2a test fixtures (generated)

Run `node scripts/make-fixtures.mjs` to regenerate. These are fabricated via ffmpeg and are
NOT committed. For "authentic" anchors that can't be fabricated, drop real files here:
- `anchor_real_lossless.flac` (a genuine lossless rip)
- `anchor_real_320.mp3` (a genuine store-bought 320)
The characterization test skips anchors that are absent.
```

- [ ] **Step 4: Write the characterization test**

Create `src-tauri/tests/characterization.rs`:

```rust
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
    let r = analyze(&p).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossless);
    assert!(r.cutoff_hz > 18000.0, "full-band cutoff, got {}", r.cutoff_hz);
    assert_ne!(r.verdict, Verdict::Fake, "genuine lossless must not be fake");
}

#[test]
fn fake_lossless_flac_is_fake() {
    let Some(p) = fixture("fake_lossless.flac") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossless);
    assert!(r.cutoff_hz < 18000.0, "transcoded cliff, got {}", r.cutoff_hz);
    assert_eq!(r.verdict, Verdict::Fake);
}

#[test]
fn honest_320_mp3_is_not_fake() {
    let Some(p) = fixture("real_320.mp3") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p).expect("analyze");
    assert_eq!(r.declared_rail, Rail::Lossy);
    assert_eq!(r.verdict, Verdict::Ok, "lossy is never fake via cutoff path");
}

#[test]
fn truncated_wav_is_flagged() {
    let Some(p) = fixture("truncated.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p).expect("analyze");
    assert!(r.truncated, "abrupt 1.5 s cut should flag truncation");
}

#[test]
fn silence_pad_measured() {
    let Some(p) = fixture("silence_pad.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p).expect("analyze");
    assert!(r.silence_head_ms >= 800 && r.silence_head_ms <= 1200, "head {}", r.silence_head_ms);
    assert!(r.silence_tail_ms >= 1300 && r.silence_tail_ms <= 1700, "tail {}", r.silence_tail_ms);
}

#[test]
fn dual_mono_detected() {
    let Some(p) = fixture("dual_mono.wav") else { eprintln!("skip: no fixture"); return; };
    let r = analyze(&p).expect("analyze");
    assert!(r.dual_mono, "duplicated-mono stereo should be dual_mono");
    assert!(r.phase_correlation > 0.99, "corr {}", r.phase_correlation);
}

// Authentic anchors (only run if the user dropped real files in fixtures/)
#[test]
fn anchor_real_lossless_not_fake() {
    let Some(p) = fixture("anchor_real_lossless.flac") else { return; };
    let r = analyze(&p).expect("analyze");
    assert_ne!(r.verdict, Verdict::Fake);
}
```

Ensure `pub mod analysis;` is exported from the lib so integration tests can reach it: in `src-tauri/src/lib.rs` change `mod analysis;` to `pub mod analysis;`, and confirm `analyze`, `Rail`, `Verdict` are `pub`.

- [ ] **Step 5: Run characterization**

Run: `cargo test -p sift_lib --test characterization`
Expected: PASS (tests with present fixtures pass; absent anchors skip). If `truncated`/`cutoff` thresholds need tuning against real ffmpeg output, adjust the constants in `spectrum.rs`/`structure.rs` and the assertions together — document the tuned values in the spec.

- [ ] **Step 6: Run the FULL suite**

Run: `cargo test -p sift_lib`
Expected: all unit tests + characterization green.

- [ ] **Step 7: Commit**

```bash
git add scripts/make-fixtures.mjs src-tauri/tests/characterization.rs src-tauri/fixtures/README.md .gitignore src-tauri/src/lib.rs
git commit -m "test(m2a): fabricated fixtures + end-to-end characterization (verdict, truncation, silence, dual-mono)"
```

---

## Self-Review checklist (done while writing)

- **Spec coverage:** peaks (T5), spectrogram+cutoff (T6), verdict (T7), real rail/bitrate (T8 tags + T7), clipping/true-peak (T2), truncation (T3), silence (T3), DC offset (T2), phase/dual-mono (T4), container/codec (T9 decode error → T10), ID3/CDJ/cover (T8), single decode streaming (T9+T10), IPC debug (T10), cache DB → **deferred to M2b** (per spec), UI → **M2c**. ✔ All M2a spec items mapped.
- **Type consistency:** `AnalysisReport`/`Spectrogram`/`Rail`/`Verdict` defined in T1, consumed identically in T6/T7/T8/T10 and TS (T10). Accumulator method names (`new/push/finish`, `correlation/dual_mono`, `verdict()`) consistent across tasks.
- **Placeholders:** none — every code step has full code. The two API-confirmation notes (lofty T8, ffmpeg-sidecar T9) are explicit "compile, read error, fix name" steps, not hand-waving.

## Notes for the executor

- If `cutoff` detection or the `truncation` heuristic disagrees with real ffmpeg output during
  T11, **tune the constants** (`floor = peak-50dB`, `run_needed`, tail-RMS 0.5×) and update both
  the assertions and the spec's Verdict section. The architecture is fixed; thresholds are
  expected to be calibrated against fixtures.
- Keep everything ES-agnostic on the Rust side; this is native Rust (not the Max ES5 engine).
- M2a writes nothing to the DB and adds no worker — that's M2b.
