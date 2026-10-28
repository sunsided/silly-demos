//! Vectors of any dimension, stored flat: what embeddings are.

/// The squared Euclidean distance between two vectors of the same length.
/// On unit vectors it is 2 − 2·cos, so ordering by it orders by cosine.
pub fn dist2(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Moves `center` a share `rate` of the way towards `x`.
pub fn pull(center: &mut [f32], x: &[f32], rate: f32) {
    for (c, &v) in center.iter_mut().zip(x) {
        *c += rate * (v - *c);
    }
}

/// Scales `v` to unit length; leaves the zero vector alone.
pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// The weighted mean of `rows` (each `dim` long) with `weights`; zeros for
/// no weight.
pub fn weighted_mean<'a>(rows: impl Iterator<Item = (&'a [f32], f32)>, dim: usize) -> Vec<f32> {
    let mut sum = vec![0.0f32; dim];
    let mut total = 0.0;
    for (row, w) in rows {
        for (s, &v) in sum.iter_mut().zip(row) {
            *s += w * v;
        }
        total += w;
    }
    if total > 0.0 {
        sum.iter_mut().for_each(|s| *s /= total);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_on_unit_vectors_follows_the_cosine() {
        let mut a = vec![1.0, 2.0, 2.0];
        let mut b = vec![2.0, -1.0, 0.5];
        normalize(&mut a);
        normalize(&mut b);
        let cos: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((dist2(&a, &b) - (2.0 - 2.0 * cos)).abs() < 1e-6);
    }

    #[test]
    fn means_and_pulls() {
        let rows = [([0.0, 0.0], 1.0), ([3.0, 6.0], 2.0)];
        let mean = weighted_mean(rows.iter().map(|(r, w)| (&r[..], *w)), 2);
        assert_eq!(mean, vec![2.0, 4.0]);
        let mut c = vec![0.0, 0.0];
        pull(&mut c, &[2.0, 4.0], 0.5);
        assert_eq!(c, vec![1.0, 2.0]);
    }
}
