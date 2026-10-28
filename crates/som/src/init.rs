//! Where a map's nodes start.

use crate::codebook::Codebook;
use crate::lattice::Lattice;
use crate::rng::Rng;

/// How [`initialize`] places the nodes before training.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Init {
    /// Each node on a randomly picked sample: no order at all to start with.
    Random,
    /// All nodes within 0.02 of the data's mean. With a small starting σ,
    /// this is how a map ends up twisted.
    Cluster,
    /// The lattice laid flat along the data's two main axes, ±2 standard
    /// deviations: already ordered, so training only has to fit it.
    Pca,
}

impl Init {
    /// The init for a UI id, `random`, `cluster` or `pca`; `None` for anything else.
    pub fn parse(id: &str) -> Option<Init> {
        match id {
            "random" => Some(Init::Random),
            "cluster" => Some(Init::Cluster),
            "pca" => Some(Init::Pca),
            _ => None,
        }
    }
}

/// How far from the data's mean [`Init::Cluster`] scatters nodes, in input units.
const CLUSTER_RADIUS: f32 = 0.02;

/// A codebook for `lattice` from `samples` (flat, `dim` values each).
/// Without samples every node starts at 0.5 in every dimension.
pub fn initialize(
    init: Init,
    lattice: &Lattice,
    samples: &[f32],
    dim: usize,
    rng: &mut Rng,
) -> Codebook {
    let n = lattice.len();
    let count = samples.len() / dim;
    if count == 0 {
        return Codebook {
            dim,
            weights: vec![0.5; n * dim],
        };
    }
    let mean = mean(samples, dim);
    let weights = match init {
        Init::Random => (0..n)
            .flat_map(|_| {
                let i = rng.below(count);
                samples[i * dim..(i + 1) * dim].to_vec()
            })
            .collect(),
        Init::Cluster => (0..n)
            .flat_map(|_| {
                // A point in the unit cube, pulled into the unit ball.
                let mut v: Vec<f32> = (0..dim).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
                let norm = v.iter().map(|a| a * a).sum::<f32>().sqrt();
                if norm > 1.0 {
                    v.iter_mut().for_each(|a| *a /= norm);
                }
                mean.iter()
                    .zip(v)
                    .map(|(m, a)| m + CLUSTER_RADIUS * a)
                    .collect::<Vec<_>>()
            })
            .collect(),
        Init::Pca => pca(lattice, samples, dim, &mean),
    };
    Codebook { dim, weights }
}

/// The per-dimension mean of flat samples, `dim` values each; there must be at least one.
fn mean(samples: &[f32], dim: usize) -> Vec<f32> {
    let count = (samples.len() / dim) as f32;
    let mut m = vec![0.0; dim];
    for x in samples.chunks_exact(dim) {
        m.iter_mut().zip(x).for_each(|(a, b)| *a += b);
    }
    m.iter_mut().for_each(|a| *a /= count);
    m
}

/// Weights for [`Init::Pca`]: each node's lattice coordinates, scaled to [−1, 1] per lattice axis,
/// mapped onto the plane of the data's two main axes at ±2 standard deviations around `mean`.
fn pca(lattice: &Lattice, samples: &[f32], dim: usize, mean: &[f32]) -> Vec<f32> {
    let count = (samples.len() / dim) as f32;
    let mut cov = vec![0.0f32; dim * dim];
    for x in samples.chunks_exact(dim) {
        for r in 0..dim {
            for c in 0..dim {
                cov[r * dim + c] += (x[r] - mean[r]) * (x[c] - mean[c]) / count;
            }
        }
    }
    // The two main axes by power iteration, the second after deflating the first.
    let (e1, l1) = power_iteration(&cov, dim);
    for r in 0..dim {
        for c in 0..dim {
            cov[r * dim + c] -= l1 * e1[r] * e1[c];
        }
    }
    let (e2, l2) = power_iteration(&cov, dim);
    let (s1, s2) = (2.0 * l1.max(0.0).sqrt(), 2.0 * l2.max(0.0).sqrt());

    // Lattice coordinates scaled to [−1, 1] along each lattice axis.
    let coords: Vec<(f32, f32)> = (0..lattice.len()).map(|i| lattice.coords(i)).collect();
    let span = |f: fn(&(f32, f32)) -> f32| {
        let (lo, hi) = coords
            .iter()
            .map(f)
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        move |v: f32| {
            if hi > lo {
                (v - lo) / (hi - lo) * 2.0 - 1.0
            } else {
                0.0
            }
        }
    };
    let (u, v) = (span(|c| c.0), span(|c| c.1));
    let (e1, e2) = (&e1, &e2);
    coords
        .iter()
        .flat_map(|&(cx, cy)| {
            let (a, b) = (u(cx) * s1, v(cy) * s2);
            (0..dim)
                .map(move |d| mean[d] + a * e1[d] + b * e2[d])
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The dominant eigenvector of a symmetric `dim`×`dim` matrix and its
/// eigenvalue.
fn power_iteration(m: &[f32], dim: usize) -> (Vec<f32>, f32) {
    // Not axis-aligned, so it isn't orthogonal to an axis-aligned eigenvector.
    let mut v: Vec<f32> = (0..dim).map(|i| 1.0 + i as f32 * 0.1).collect();
    let mut lambda = 0.0;
    for _ in 0..100 {
        let w: Vec<f32> = (0..dim)
            .map(|r| (0..dim).map(|c| m[r * dim + c] * v[c]).sum())
            .collect();
        let norm = w.iter().map(|a| a * a).sum::<f32>().sqrt();
        if norm < 1e-12 {
            return (v, 0.0);
        }
        lambda = norm;
        v = w.iter().map(|a| a / norm).collect();
    }
    (v, lambda)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::density::Density;
    use crate::lattice::Kind;

    fn uniform_samples(n: usize) -> Vec<f32> {
        let mut d = Density::preset("uniform").unwrap();
        let mut rng = Rng::new(5);
        (0..n).flat_map(|_| d.sample(&mut rng).unwrap()).collect()
    }

    fn mean(samples: &[f32]) -> [f32; 2] {
        let n = (samples.len() / 2) as f32;
        let sx: f32 = samples.iter().step_by(2).sum();
        let sy: f32 = samples.iter().skip(1).step_by(2).sum();
        [sx / n, sy / n]
    }

    #[test]
    fn pca_init_spans_the_data() {
        let samples = uniform_samples(1000);
        let lattice = Lattice::new(Kind::Grid, 12, 12).unwrap();
        let cb = initialize(Init::Pca, &lattice, &samples, 2, &mut Rng::new(1));
        assert_eq!(cb.len(), 144);
        for axis in 0..2 {
            let values = cb.weights.iter().skip(axis).step_by(2);
            let (lo, hi) = values.fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            assert!(hi - lo > 0.5, "axis {axis}: {lo}..{hi}");
        }
    }

    #[test]
    fn pca_init_lays_a_chain_along_the_main_axis() {
        // Samples on a line from (0.1, 0.1) to (0.9, 0.9).
        let samples: Vec<f32> = (0..200).flat_map(|i| [0.1 + i as f32 * 0.004; 2]).collect();
        let lattice = Lattice::new(Kind::Chain, 20, 1).unwrap();
        let cb = initialize(Init::Pca, &lattice, &samples, 2, &mut Rng::new(1));
        for w in cb.weights.chunks(2) {
            assert!((w[0] - w[1]).abs() < 1e-3, "{w:?}");
        }
        assert!((cb.node(0)[0] - cb.node(19)[0]).abs() > 0.5);
    }

    #[test]
    fn cluster_init_is_tight() {
        let samples = uniform_samples(1000);
        let m = mean(&samples);
        let lattice = Lattice::new(Kind::Grid, 12, 12).unwrap();
        let cb = initialize(Init::Cluster, &lattice, &samples, 2, &mut Rng::new(1));
        for w in cb.weights.chunks(2) {
            assert!((w[0] - m[0]).hypot(w[1] - m[1]) <= 0.02 + 1e-6, "{w:?}");
        }
    }

    #[test]
    fn random_init_picks_samples() {
        let samples = uniform_samples(100);
        let lattice = Lattice::new(Kind::Ring, 30, 1).unwrap();
        let cb = initialize(Init::Random, &lattice, &samples, 2, &mut Rng::new(1));
        for w in cb.weights.chunks(2) {
            assert!(samples.chunks(2).any(|s| s == w), "{w:?}");
        }
    }

    #[test]
    fn no_samples_starts_in_the_middle() {
        let lattice = Lattice::new(Kind::Grid, 3, 3).unwrap();
        for init in [Init::Random, Init::Cluster, Init::Pca] {
            let cb = initialize(init, &lattice, &[], 2, &mut Rng::new(1));
            assert!(cb.weights.iter().all(|&v| v == 0.5), "{init:?}");
        }
    }

    #[test]
    fn inits_parse_from_their_ids() {
        assert_eq!(Init::parse("pca"), Some(Init::Pca));
        assert_eq!(Init::parse("x"), None);
    }
}
