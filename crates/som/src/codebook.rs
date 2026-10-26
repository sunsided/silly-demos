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
    /// here have at most a few hundred nodes.
    pub fn bmu2(&self, x: &[f32]) -> (usize, usize) {
        let (mut best, mut second) = ((0, f32::INFINITY), (0, f32::INFINITY));
        for (i, w) in self.weights.chunks_exact(self.dim).enumerate() {
            let d: f32 = w.iter().zip(x).map(|(a, b)| (a - b) * (a - b)).sum();
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
}
