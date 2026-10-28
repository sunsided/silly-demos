//! Putting data columns on a common scale, with NaN marking a missing value.

/// Per-column z-scores: `(x − mean) / scale`, the scale being the standard
/// deviation (1 for a constant column). NaN stays NaN.
#[derive(Clone, Debug, PartialEq)]
pub struct Normalizer {
    /// Per-column mean, subtracted first.
    pub mean: Vec<f32>,
    /// Per-column divisor, never 0.
    pub scale: Vec<f32>,
}

impl Normalizer {
    /// Mean and standard deviation of each of the `dim` columns of the
    /// flat rows in `values`, over the values present.
    pub fn fit(values: &[f32], dim: usize) -> Normalizer {
        let mut sum = vec![0.0f64; dim];
        let mut sq = vec![0.0f64; dim];
        let mut n = vec![0usize; dim];
        for row in values.chunks_exact(dim) {
            for (d, &v) in row.iter().enumerate() {
                if !v.is_nan() {
                    sum[d] += v as f64;
                    sq[d] += (v as f64) * (v as f64);
                    n[d] += 1;
                }
            }
        }
        let mut mean = Vec::with_capacity(dim);
        let mut scale = Vec::with_capacity(dim);
        for d in 0..dim {
            if n[d] == 0 {
                mean.push(0.0);
                scale.push(1.0);
                continue;
            }
            let m = sum[d] / n[d] as f64;
            let sd = (sq[d] / n[d] as f64 - m * m).max(0.0).sqrt();
            mean.push(m as f32);
            scale.push(if sd < 1e-12 { 1.0 } else { sd as f32 });
        }
        Normalizer { mean, scale }
    }

    /// Leaves values as they are.
    pub fn identity(dim: usize) -> Normalizer {
        Normalizer {
            mean: vec![0.0; dim],
            scale: vec![1.0; dim],
        }
    }

    /// Flat rows in raw units to z-scores, column by column.
    pub fn apply(&self, values: &[f32]) -> Vec<f32> {
        let dim = self.mean.len();
        values
            .iter()
            .enumerate()
            .map(|(i, v)| (v - self.mean[i % dim]) / self.scale[i % dim])
            .collect()
    }

    /// Z-scores back to raw units; undoes [`Normalizer::apply`].
    pub fn invert(&self, values: &[f32]) -> Vec<f32> {
        let dim = self.mean.len();
        values
            .iter()
            .enumerate()
            .map(|(i, v)| v * self.scale[i % dim] + self.mean[i % dim])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizer_ignores_missing_values() {
        let n = Normalizer::fit(&[1.0, 10.0, 3.0, 20.0, f32::NAN, 30.0], 2);
        assert_eq!(n.mean, vec![2.0, 20.0]);
        assert!((n.scale[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn constant_columns_get_unit_scale() {
        let n = Normalizer::fit(&[5.0, 5.0, 5.0], 1);
        assert_eq!((n.mean[0], n.scale[0]), (5.0, 1.0));
        assert_eq!(n.apply(&[5.0]), vec![0.0]);
    }

    #[test]
    fn apply_then_invert_round_trips() {
        let values = [1.0, 200.0, -3.0, f32::NAN, 7.5, 0.0];
        let n = Normalizer::fit(&values, 2);
        let back = n.invert(&n.apply(&values));
        for (a, b) in values.iter().zip(&back) {
            assert!(
                (a.is_nan() && b.is_nan()) || (a - b).abs() < 1e-4,
                "{a} vs {b}"
            );
        }
        let z = n.apply(&values);
        let mean0 = (z[0] + z[2] + z[4]) / 3.0;
        assert!(mean0.abs() < 1e-5);
    }

    #[test]
    fn identity_changes_nothing() {
        let n = Normalizer::identity(3);
        assert_eq!(n.apply(&[1.0, 2.0, 3.0]), vec![1.0, 2.0, 3.0]);
    }
}
