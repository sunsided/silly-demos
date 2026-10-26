//! How well a map fits its data: how far samples are from their nodes
//! (quantization error), and how often the map's order breaks
//! (topographic error).

use crate::codebook::Codebook;
use crate::lattice::Lattice;

/// Mean distance from each sample to its best-matching unit. `samples` is
/// flat, `cb.dim` values per sample.
pub fn quantization_error(cb: &Codebook, samples: &[f32]) -> f32 {
    mean(samples.chunks_exact(cb.dim).map(|x| {
        let w = cb.node(cb.bmu(x));
        w.iter()
            .zip(x)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt()
    }))
}

/// The share of samples whose nearest and second-nearest nodes are not
/// neighbours on the lattice: 0 for a map whose order matches the data's,
/// higher where it is folded or twisted.
pub fn topographic_error(cb: &Codebook, lattice: &Lattice, samples: &[f32]) -> f32 {
    mean(samples.chunks_exact(cb.dim).map(|x| {
        let (a, b) = cb.bmu2(x);
        if lattice.neighbours(a, b) { 0.0 } else { 1.0 }
    }))
}

fn mean(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    if n == 0 { 0.0 } else { sum / n as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lattice::{Kind, Lattice};

    fn chain_along_x() -> (Codebook, Lattice, Vec<f32>) {
        let weights = (0..10).flat_map(|i| [i as f32 / 9.0, 0.0]).collect();
        let samples = (0..100).flat_map(|i| [i as f32 / 99.0, 0.0]).collect();
        (
            Codebook { dim: 2, weights },
            Lattice::new(Kind::Chain, 10, 1).unwrap(),
            samples,
        )
    }

    #[test]
    fn ordered_chain_has_zero_topographic_error() {
        let (cb, lattice, samples) = chain_along_x();
        assert_eq!(topographic_error(&cb, &lattice, &samples), 0.0);
    }

    #[test]
    fn swapped_chain_has_topographic_error() {
        let (mut cb, lattice, samples) = chain_along_x();
        cb.weights.swap(3 * 2, 7 * 2);
        assert!(topographic_error(&cb, &lattice, &samples) > 0.0);
    }

    #[test]
    fn quantization_error_of_exact_hits_is_zero() {
        let (cb, _, _) = chain_along_x();
        assert_eq!(quantization_error(&cb, &cb.weights.clone()), 0.0);
        let off = [0.0, 0.5];
        assert!((quantization_error(&cb, &off) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn no_samples_means_no_error() {
        let (cb, lattice, _) = chain_along_x();
        assert_eq!(quantization_error(&cb, &[]), 0.0);
        assert_eq!(topographic_error(&cb, &lattice, &[]), 0.0);
    }
}
