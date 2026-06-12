//! Inter-channel (L/R) Pearson correlation → phase compatibility + dual-mono detection.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_channels_correlate_to_one() {
        let mut a = PhaseAccumulator::new();
        a.push(&[0.1, 0.1, -0.3, -0.3, 0.7, 0.7]);
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
