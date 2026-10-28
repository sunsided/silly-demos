//! Scoring clusters against the labels of the points they were made from,
//! and the batch k-means they are compared with.

use crate::rng::Rng;
use crate::{Point, dist2};

/// Rounds of Lloyd's algorithm [`batch_kmeans`] runs at most.
const LLOYD_ROUNDS: usize = 25;

/// Batch k-means told the true `k`: k-means++ seeding, then Lloyd's
/// algorithm until the assignment stops changing. Returns each point's
/// cluster index; empty for no points, and `k` is capped at their number.
pub fn batch_kmeans(points: &[Point], k: usize, rng: &mut Rng) -> Vec<u32> {
    let n = points.len();
    let k = k.min(n);
    if k == 0 {
        return Vec::new();
    }
    let mut centers = vec![points[rng.below(n)]];
    let mut nearest: Vec<f32> = points.iter().map(|&p| dist2(p, centers[0])).collect();
    while centers.len() < k {
        let total: f32 = nearest.iter().sum();
        let mut r = rng.next_f32() * total;
        let pick = nearest
            .iter()
            .position(|&d| {
                r -= d;
                r < 0.0
            })
            .unwrap_or(n - 1);
        centers.push(points[pick]);
        for (d, &p) in nearest.iter_mut().zip(points) {
            *d = d.min(dist2(p, points[pick]));
        }
    }
    let mut labels = vec![u32::MAX; n];
    for _ in 0..LLOYD_ROUNDS {
        let mut changed = false;
        for (label, &p) in labels.iter_mut().zip(points) {
            let best = (0..k)
                .min_by(|&a, &b| dist2(p, centers[a]).total_cmp(&dist2(p, centers[b])))
                .expect("k > 0") as u32;
            changed |= *label != best;
            *label = best;
        }
        if !changed {
            break;
        }
        let mut sums = vec![([0.0f32; 2], 0usize); k];
        for (&l, &p) in labels.iter().zip(points) {
            let s = &mut sums[l as usize];
            s.0[0] += p[0];
            s.0[1] += p[1];
            s.1 += 1;
        }
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s.1 > 0 {
                *c = [s.0[0] / s.1 as f32, s.0[1] / s.1 as f32];
            }
        }
    }
    labels
}

/// The adjusted Rand index of two labelings of the same points: 1 when they
/// group the points identically, about 0 when they agree no more than
/// chance would, and below 0 when they agree less. Labels are arbitrary
/// numbers; only which points share one matters. 1 for fewer than two
/// points, or when both put all points in one group.
pub fn adjusted_rand(a: &[u32], b: &[u32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 1.0;
    }
    let index = |labels: &[u32]| {
        let mut seen: Vec<u32> = labels[..n].to_vec();
        seen.sort_unstable();
        seen.dedup();
        let ix: Vec<usize> = labels[..n]
            .iter()
            .map(|l| seen.binary_search(l).expect("seen"))
            .collect();
        (ix, seen.len())
    };
    let ((ia, ka), (ib, kb)) = (index(a), index(b));
    let mut table = vec![0u64; ka * kb];
    for (&x, &y) in ia.iter().zip(&ib) {
        table[x * kb + y] += 1;
    }
    let pairs = |c: u64| (c * c.saturating_sub(1) / 2) as f64;
    let rows: f64 = (0..ka)
        .map(|i| pairs((0..kb).map(|j| table[i * kb + j]).sum()))
        .sum();
    let cols: f64 = (0..kb)
        .map(|j| pairs((0..ka).map(|i| table[i * kb + j]).sum()))
        .sum();
    let both: f64 = table.iter().map(|&c| pairs(c)).sum();
    let total = pairs(n as u64);
    let expected = rows * cols / total;
    let max = (rows + cols) / 2.0;
    if (max - expected).abs() < 1e-12 {
        return 1.0;
    }
    ((both - expected) / (max - expected)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_groupings_score_one_whatever_the_labels() {
        assert_eq!(adjusted_rand(&[0, 0, 1, 1, 2], &[7, 7, 3, 3, 9]), 1.0);
    }

    #[test]
    fn a_known_value() {
        // scikit-learn's adjusted_rand_score([0,0,1,2], [0,0,1,1]) = 0.5714...
        let ari = adjusted_rand(&[0, 0, 1, 2], &[0, 0, 1, 1]);
        assert!((ari - 0.571_428_6).abs() < 1e-5, "{ari}");
    }

    #[test]
    fn batch_kmeans_separates_two_blobs() {
        let mut rng = Rng::new(2);
        let points: Vec<Point> = (0..400)
            .map(|i| {
                let c = if i % 2 == 0 { 0.25 } else { 0.75 };
                [c + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03]
            })
            .collect();
        let truth: Vec<u32> = (0..400).map(|i| (i % 2) as u32).collect();
        let labels = batch_kmeans(&points, 2, &mut rng);
        assert_eq!(adjusted_rand(&labels, &truth), 1.0);
        assert!(batch_kmeans(&[], 3, &mut rng).is_empty());
    }

    #[test]
    fn unrelated_groupings_score_near_zero() {
        let mut rng = crate::rng::Rng::new(1);
        let a: Vec<u32> = (0..5000).map(|_| rng.below(4) as u32).collect();
        let b: Vec<u32> = (0..5000).map(|_| rng.below(4) as u32).collect();
        assert!(adjusted_rand(&a, &b).abs() < 0.02);
    }
}
