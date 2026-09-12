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
        Self {
            window: window.max(1),
            cur_max: 0.0,
            count: 0,
            out: Vec::new(),
        }
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
        if self.count > 0 {
            self.out.push(self.cur_max);
        }
        self.out
    }
}

/// Caps an envelope to `max_points` by max-pooling consecutive values, and reports the pooling
/// factor so the caller can derive the EFFECTIVE sample step (`window * factor`).
///
/// Why: at PEAKS_WINDOW=512 a 6.5-minute track yields ~33 500 points, serialized as ~10.75 JSON
/// characters each — 21% of report_json — to draw a waveform a few hundred pixels wide. Max-pooling
/// (never averaging) is what keeps a transient visible: an envelope is read for its peaks, and a
/// mean would flatten exactly the spikes the waveform exists to show.
///
/// The factor is returned rather than recomputed downstream because the last pool is partial
/// whenever `len` is not a multiple of the factor: `out.len() * factor` overshoots the real sample
/// count, and any consumer dividing by it would misreport coverage.
pub fn cap(v: Vec<f32>, max_points: usize) -> (Vec<f32>, usize) {
    let max_points = max_points.max(1);
    if v.len() <= max_points {
        return (v, 1);
    }
    let factor = v.len().div_ceil(max_points);
    let out = v
        .chunks(factor)
        .map(|c| c.iter().fold(0.0f32, |a, &b| a.max(b)))
        .collect();
    (out, factor)
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

    #[test]
    fn cap_leaves_a_short_envelope_untouched() {
        let v = vec![0.1, 0.2, 0.3];
        let (out, factor) = cap(v.clone(), 4000);
        assert_eq!(out, v);
        assert_eq!(
            factor, 1,
            "no pooling means the effective step is unchanged"
        );
    }

    #[test]
    fn cap_pools_by_max_and_never_exceeds_the_ceiling() {
        let v: Vec<f32> = (0..10_000).map(|i| (i % 100) as f32 / 100.0).collect();
        let (out, factor) = cap(v, 4000);
        assert!(out.len() <= 4000, "got {} points", out.len());
        assert_eq!(factor, 3, "10000 over 4000 needs a factor of 3");
        // A transient must survive pooling: the max of each chunk is kept, never the mean.
        assert!((out.iter().fold(0.0f32, |a, &b| a.max(b)) - 0.99).abs() < 1e-6);
    }

    #[test]
    fn cap_keeps_the_loudest_sample_of_each_pool() {
        // One spike buried among silence: max-pooling keeps it, averaging would erase it.
        let mut v = vec![0.0f32; 9];
        v[4] = 1.0;
        let (out, factor) = cap(v, 3);
        assert_eq!(factor, 3);
        assert_eq!(out.len(), 3);
        assert!(
            (out[1] - 1.0).abs() < 1e-6,
            "the spike must land in its pool, undimmed"
        );
    }
}
