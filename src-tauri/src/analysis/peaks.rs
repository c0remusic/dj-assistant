//! Downsampled abs-max envelope over mono f32 blocks, for the M2c waveform.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_peak_per_window() {
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
        a.push(&[0.1, 0.2, 0.7]);
        let p = a.finish();
        assert_eq!(p.len(), 1);
        assert!((p[0] - 0.7).abs() < 1e-6);
    }
}
