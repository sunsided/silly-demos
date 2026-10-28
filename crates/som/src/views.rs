//! What a trained map shows when drawn as a picture: the distances between
//! neighboring nodes (U-matrix), one component across all nodes (a
//! component plane), and which nodes the samples land on.

use crate::codebook::Codebook;
use crate::lattice::Lattice;

/// Per node, the mean distance to its lattice neighbors: low inside a
/// cluster, high on the boundary between two.
pub fn umatrix(cb: &Codebook, lattice: &Lattice) -> Vec<f32> {
    (0..cb.len())
        .map(|i| {
            let (sum, n) = (0..cb.len())
                .filter(|&j| lattice.neighbors(i, j))
                .map(|j| {
                    let d: f32 = cb
                        .node(i)
                        .iter()
                        .zip(cb.node(j))
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    d.sqrt()
                })
                .fold((0.0, 0), |(s, n), d| (s + d, n + 1));
            if n == 0 { 0.0 } else { sum / n as f32 }
        })
        .collect()
}

/// Component `k` of every node; empty when there is no such component.
pub fn plane(cb: &Codebook, k: usize) -> Vec<f32> {
    if k >= cb.dim {
        return Vec::new();
    }
    cb.weights.iter().skip(k).step_by(cb.dim).copied().collect()
}

/// How many samples land on each node.
pub fn hits(cb: &Codebook, samples: &[f32]) -> Vec<u32> {
    let mut h = vec![0; cb.len()];
    for b in bmus(cb, samples) {
        h[b as usize] += 1;
    }
    h
}

/// The best-matching unit of every sample, in order.
pub fn bmus(cb: &Codebook, samples: &[f32]) -> Vec<u32> {
    samples
        .chunks_exact(cb.dim)
        .map(|x| cb.bmu(x) as u32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lattice::Kind;

    fn two_clusters() -> (Codebook, Lattice) {
        let weights = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0].to_vec();
        (
            Codebook { dim: 1, weights },
            Lattice::new(Kind::Chain, 6, 1).unwrap(),
        )
    }

    #[test]
    fn umatrix_peaks_between_two_clusters() {
        let (cb, lattice) = two_clusters();
        let u = umatrix(&cb, &lattice);
        assert_eq!(u, vec![0.0, 0.0, 5.0, 5.0, 0.0, 0.0]);
    }

    #[test]
    fn plane_reads_one_component() {
        let cb = Codebook {
            dim: 2,
            weights: vec![1.0, 2.0, 3.0, 4.0],
        };
        assert_eq!(plane(&cb, 1), vec![2.0, 4.0]);
        assert!(plane(&cb, 2).is_empty());
    }

    #[test]
    fn hits_sum_to_the_sample_count() {
        let (cb, _) = two_clusters();
        let samples = [0.1, 9.0, 11.0, 4.0, f32::NAN];
        let h = hits(&cb, &samples);
        assert_eq!(h.iter().sum::<u32>(), 5);
        assert_eq!(h.len(), 6);
    }

    #[test]
    fn bmus_match_bmu() {
        let (cb, _) = two_clusters();
        let samples = [0.1, 9.0, 11.0];
        let b = bmus(&cb, &samples);
        assert_eq!(
            b,
            samples
                .iter()
                .map(|x| cb.bmu(&[*x]) as u32)
                .collect::<Vec<_>>()
        );
    }
}
