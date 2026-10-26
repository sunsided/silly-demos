//! The weight vectors of a map's nodes, and the search for the nodes
//! nearest a sample.

/// One weight vector of `dim` values per node, node after node.
#[derive(Clone, Debug, PartialEq)]
pub struct Codebook {
    pub dim: usize,
    pub weights: Vec<f32>,
}

impl Codebook {
    pub fn len(&self) -> usize {
        self.weights.len() / self.dim
    }

    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    pub fn node(&self, i: usize) -> &[f32] {
        &self.weights[i * self.dim..(i + 1) * self.dim]
    }

    /// The best-matching unit: the node nearest `x`.
    pub fn bmu(&self, x: &[f32]) -> usize {
        self.bmu2(x).0
    }

    /// The nearest node and the second nearest. A brute-force scan; maps
    /// here have at most a few hundred nodes. Missing (NaN) dimensions of
    /// `x` are left out of the distance.
    pub fn bmu2(&self, x: &[f32]) -> (usize, usize) {
        let (mut best, mut second) = ((0, f32::INFINITY), (0, f32::INFINITY));
        for (i, w) in self.weights.chunks_exact(self.dim).enumerate() {
            let d = distance2(w, x);
            if d < best.1 {
                second = best;
                best = (i, d);
            } else if d < second.1 {
                second = (i, d);
            }
        }
        (best.0, second.0)
    }
}

/// Squared distance over the dimensions `x` has, scaled up by
/// `dim / present` so samples with gaps compare on the same footing.
pub fn distance2(w: &[f32], x: &[f32]) -> f32 {
    let (mut sum, mut present) = (0.0f32, 0usize);
    for (a, b) in w.iter().zip(x) {
        if !b.is_nan() {
            sum += (a - b) * (a - b);
            present += 1;
        }
    }
    if present == 0 || present == x.len() {
        sum
    } else {
        sum * x.len() as f32 / present as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmu2_returns_the_two_nearest() {
        let cb = Codebook {
            dim: 2,
            weights: vec![0.0, 0.0, 1.0, 0.0, 0.4, 0.0, 5.0, 5.0],
        };
        assert_eq!(cb.len(), 4);
        assert_eq!(cb.node(2), &[0.4, 0.0]);
        assert_eq!(cb.bmu(&[0.9, 0.1]), 1);
        assert_eq!(cb.bmu2(&[0.9, 0.1]), (1, 2));
        assert_eq!(cb.bmu2(&[0.1, 0.0]), (0, 2));
    }

    #[test]
    fn bmu_ignores_missing_dimensions() {
        let cb = Codebook {
            dim: 3,
            weights: vec![0.0, 0.0, 0.0, 1.0, 1.0, 9.0],
        };
        assert_eq!(cb.bmu(&[1.0, 1.0, f32::NAN]), 1);
        assert_eq!(cb.bmu2(&[0.1, f32::NAN, f32::NAN]), (0, 1));
    }
}
