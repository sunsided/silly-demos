//! k-means, the baseline every vector quantizer is measured against:
//! nodes with no neighborhood at all. Online, as MacQueen (1967) gave it,
//! each sample moves only its nearest node, by one over the number of
//! samples that node has won, so every node ends at the mean of its share.

use crate::codebook::{Codebook, distance2};
use crate::rng::Rng;

/// Online k-means.
#[derive(Clone, Debug)]
pub struct KMeans {
    /// The centroids.
    pub codebook: Codebook,
    /// Samples each centroid has won, counting its seed.
    counts: Vec<u32>,
    /// Samples presented so far.
    pub t: u64,
}

impl KMeans {
    /// Centroids seeded by k-means++ (Arthur and Vassilvitskii, 2007): the
    /// first on a random sample, each next on a sample drawn with
    /// probability proportional to its squared distance to the nearest
    /// centroid so far. `samples` holds `dim` values each; `k` is capped at
    /// their number, and without samples there are no centroids.
    pub fn new(k: usize, samples: &[f32], dim: usize, rng: &mut Rng) -> KMeans {
        let n = samples.len() / dim;
        let k = k.min(n);
        let mut weights: Vec<f32> = Vec::with_capacity(k * dim);
        if k > 0 {
            let first = rng.below(n);
            weights.extend_from_slice(&samples[first * dim..(first + 1) * dim]);
        }
        let mut nearest: Vec<f32> = samples
            .chunks_exact(dim)
            .map(|x| {
                if k > 0 {
                    distance2(&weights[..dim], x)
                } else {
                    0.0
                }
            })
            .collect();
        while weights.len() < k * dim {
            let total: f32 = nearest.iter().sum();
            let pick = if total > 0.0 {
                let mut r = rng.next_f32() * total;
                nearest
                    .iter()
                    .position(|&d| {
                        r -= d;
                        r < 0.0
                    })
                    .unwrap_or(n - 1)
            } else {
                rng.below(n)
            };
            let c = &samples[pick * dim..(pick + 1) * dim];
            for (d, x) in nearest.iter_mut().zip(samples.chunks_exact(dim)) {
                *d = d.min(distance2(c, x));
            }
            weights.extend_from_slice(c);
        }
        KMeans {
            counts: vec![1; k],
            codebook: Codebook { dim, weights },
            t: 0,
        }
    }

    /// MacQueen's update: the nearest centroid moves towards `x` by one
    /// over its win count. The steps shrink as counts grow, so the
    /// centroids settle and later samples barely move them.
    pub fn step(&mut self, x: &[f32]) {
        if self.codebook.is_empty() {
            return;
        }
        let i = self.codebook.bmu(x);
        self.counts[i] += 1;
        let rate = 1.0 / self.counts[i] as f32;
        let dim = self.codebook.dim;
        for (w, &v) in self.codebook.weights[i * dim..(i + 1) * dim]
            .iter_mut()
            .zip(x)
        {
            *w += rate * (v - *w);
        }
        self.t += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeding_picks_distinct_samples_and_caps_k() {
        let samples = [0.0, 0.0, 10.0, 0.0, 0.0, 10.0];
        let km = KMeans::new(5, &samples, 2, &mut Rng::new(1));
        assert_eq!(km.codebook.len(), 3);
        let mut seeds: Vec<[f32; 2]> = km
            .codebook
            .weights
            .chunks(2)
            .map(|c| [c[0], c[1]])
            .collect();
        seeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(seeds, vec![[0.0, 0.0], [0.0, 10.0], [10.0, 0.0]]);
        assert!(KMeans::new(3, &[], 2, &mut Rng::new(1)).codebook.is_empty());
    }

    #[test]
    fn a_centroid_ends_at_the_mean_of_its_samples() {
        let mut km = KMeans::new(1, &[0.0], 1, &mut Rng::new(1));
        for v in [2.0, 4.0, 6.0] {
            km.step(&[v]);
        }
        // Seed 0 counted once, then 2, 4, 6: the mean of 0, 2, 4, 6.
        assert_eq!(km.codebook.weights, vec![3.0]);
    }

    #[test]
    fn two_blobs_get_a_centroid_each() {
        let mut rng = Rng::new(2);
        let blob = |rng: &mut Rng, c: f32| [c + rng.next_f32() * 0.1, rng.next_f32() * 0.1];
        let samples: Vec<f32> = (0..200)
            .flat_map(|i| blob(&mut rng, if i % 2 == 0 { 0.0 } else { 1.0 }))
            .collect();
        let mut km = KMeans::new(2, &samples, 2, &mut rng);
        for _ in 0..2000 {
            let c = if rng.next_f32() < 0.5 { 0.0 } else { 1.0 };
            let x = blob(&mut rng, c);
            km.step(&x);
        }
        let mut xs: Vec<f32> = km.codebook.weights.chunks(2).map(|c| c[0]).collect();
        xs.sort_by(f32::total_cmp);
        assert!(
            (xs[0] - 0.05).abs() < 0.02 && (xs[1] - 1.05).abs() < 0.02,
            "{xs:?}"
        );
    }
}
