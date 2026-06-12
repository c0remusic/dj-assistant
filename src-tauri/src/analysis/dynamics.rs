//! Online dynamics analyzers over mono f32 blocks: DC offset, clipping, true-peak.

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
        let mut a = ClipAccumulator::new(0.99, 3);
        a.push(&[1.0, 1.0, 1.0, 0.1, 1.0, 0.1, 0.1, 0.1, 0.1, 0.1]);
        let (runs, pct) = a.finish();
        assert_eq!(runs, 1, "only the length-3 run counts");
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
