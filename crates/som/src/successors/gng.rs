//! Fritzke's growing neural gas (1995): starts with two nodes and grows.
//! Every node keeps the error it has accumulated; every λ samples a node
//! is inserted between the worst node and its worst neighbor, so nodes
//! appear where the data is poorly covered. Edges are learned as in a
//! topology representing network, and a node left without edges goes.
//!
//! With utility (GNG-U, Fritzke 1997) each node also tracks how much the
//! error would grow without it; a node whose utility is far below the
//! worst error is removed, so the network follows a density that changes
//! instead of keeping nodes where the data used to be.

use super::graph::Graph;
use crate::codebook::{Codebook, distance2};

/// Rates and limits, with the values from Fritzke's papers as defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Nodes the network may grow to.
    pub max_nodes: usize,
    /// How far the nearest node moves towards a sample (εb).
    pub eps_winner: f32,
    /// How far its neighbors move (εn).
    pub eps_neighbor: f32,
    /// Updates an edge may go unrenewed before it is removed.
    pub max_age: u32,
    /// Samples between two insertions (λ).
    pub insert_every: u32,
    /// Error kept by the two nodes a new one is inserted between (α).
    pub split_decay: f32,
    /// Every node's error and utility are multiplied by this after each sample (d).
    pub decay: f32,
    /// GNG-U: remove the least useful node when the worst error exceeds
    /// this many times its utility; `None` for plain GNG.
    pub utility: Option<f32>,
}

impl Params {
    /// Fritzke's defaults for at most `max_nodes` nodes: εb 0.05, εn
    /// 0.0006, edge age 88, λ 300, α 0.5, d 0.9995; `utility` the GNG-U
    /// ratio k (3 in the paper) or `None`.
    pub fn defaults(max_nodes: usize, utility: Option<f32>) -> Params {
        Params {
            max_nodes: max_nodes.max(2),
            eps_winner: 0.05,
            eps_neighbor: 0.0006,
            max_age: 88,
            insert_every: 300,
            split_decay: 0.5,
            decay: 0.9995,
            utility,
        }
    }
}

/// A growing neural gas.
#[derive(Clone, Debug)]
pub struct Gng {
    /// The nodes' positions.
    pub codebook: Codebook,
    /// The learned edges.
    pub graph: Graph,
    /// Rates and limits.
    pub params: Params,
    /// Accumulated squared error per node.
    error: Vec<f32>,
    /// GNG-U: how much the error would grow per node without it.
    utility: Vec<f32>,
    /// Samples presented so far.
    pub t: u64,
}

impl Gng {
    /// Two nodes at `a` and `b`, joined by nothing yet.
    pub fn new(a: &[f32], b: &[f32], params: Params) -> Gng {
        let mut weights = a.to_vec();
        weights.extend_from_slice(b);
        Gng {
            codebook: Codebook {
                dim: a.len(),
                weights,
            },
            graph: Graph::default(),
            params,
            error: vec![0.0; 2],
            utility: vec![0.0; 2],
            t: 0,
        }
    }

    /// The number of nodes.
    pub fn len(&self) -> usize {
        self.codebook.len()
    }

    /// Never true: a growing gas keeps at least two nodes.
    pub fn is_empty(&self) -> bool {
        self.codebook.is_empty()
    }

    /// One sample, as in Fritzke (1995): the nearest node's edges age and
    /// it gains the squared error; it moves by εb and its neighbors by εn;
    /// the nearest two are joined; old edges and the nodes they leave
    /// alone go; every λ samples a node is inserted; errors decay. GNG-U
    /// also credits the nearest node with its utility and may remove the
    /// least useful node.
    pub fn step(&mut self, x: &[f32]) {
        let dim = self.codebook.dim;
        let (s1, s2) = self.codebook.bmu2(x);
        let d1 = distance2(self.codebook.node(s1), x);
        let d2 = distance2(self.codebook.node(s2), x);
        self.graph.age_at(s1 as u32);
        self.error[s1] += d1;
        self.utility[s1] += d2 - d1;
        let move_towards = |weights: &mut [f32], i: usize, eps: f32| {
            for (w, &v) in weights[i * dim..(i + 1) * dim].iter_mut().zip(x) {
                *w += eps * (v - *w);
            }
        };
        move_towards(&mut self.codebook.weights, s1, self.params.eps_winner);
        let neighbors: Vec<u32> = self.graph.neighbors(s1 as u32).collect();
        for n in neighbors {
            move_towards(
                &mut self.codebook.weights,
                n as usize,
                self.params.eps_neighbor,
            );
        }
        self.graph.connect(s1 as u32, s2 as u32);
        self.graph.prune(self.params.max_age);
        self.drop_isolated();
        self.t += 1;
        if self.t.is_multiple_of(self.params.insert_every as u64) {
            self.insert();
        }
        if let Some(k) = self.params.utility {
            self.drop_useless(k);
        }
        for e in &mut self.error {
            *e *= self.params.decay;
        }
        for u in &mut self.utility {
            *u *= self.params.decay;
        }
    }

    /// Removes node `i` from the codebook, its error and utility, and the graph.
    fn remove(&mut self, i: usize) {
        let dim = self.codebook.dim;
        self.codebook.weights.drain(i * dim..(i + 1) * dim);
        self.error.remove(i);
        self.utility.remove(i);
        self.graph.remove_node(i as u32);
    }

    /// Removes nodes without edges, keeping at least two nodes.
    fn drop_isolated(&mut self) {
        let mut i = 0;
        while i < self.len() && self.len() > 2 {
            if self.graph.degree(i as u32) == 0 {
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Inserts a node halfway between the node with the largest error and
    /// its neighbor with the largest error, unless the network is full or
    /// that node has no neighbor.
    fn insert(&mut self) {
        if self.len() >= self.params.max_nodes {
            return;
        }
        let q = argmax(&self.error);
        let Some(f) = self
            .graph
            .neighbors(q as u32)
            .max_by(|&a, &b| self.error[a as usize].total_cmp(&self.error[b as usize]))
        else {
            return;
        };
        let f = f as usize;
        let r = self.len();
        let mid: Vec<f32> = self
            .codebook
            .node(q)
            .iter()
            .zip(self.codebook.node(f))
            .map(|(a, b)| (a + b) / 2.0)
            .collect();
        self.codebook.weights.extend(mid);
        self.graph.disconnect(q as u32, f as u32);
        self.graph.connect(q as u32, r as u32);
        self.graph.connect(r as u32, f as u32);
        self.error[q] *= self.params.split_decay;
        self.error[f] *= self.params.split_decay;
        self.error.push(self.error[q]);
        self.utility.push((self.utility[q] + self.utility[f]) / 2.0);
    }

    /// GNG-U: removes the node of least utility when the largest error is
    /// more than `k` times that utility, keeping at least two nodes.
    fn drop_useless(&mut self, k: f32) {
        if self.len() <= 2 {
            return;
        }
        let worst = self.error.iter().cloned().fold(0.0, f32::max);
        let i = argmin(&self.utility);
        if worst > k * self.utility[i] {
            self.remove(i);
            self.drop_isolated();
        }
    }
}

/// The index of the largest value; the first of equals.
fn argmax(values: &[f32]) -> usize {
    (0..values.len()).fold(0, |best, i| if values[i] > values[best] { i } else { best })
}

/// The index of the smallest value; the first of equals.
fn argmin(values: &[f32]) -> usize {
    (0..values.len()).fold(0, |best, i| if values[i] < values[best] { i } else { best })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::successors::graph::graph_error;

    fn grow(
        utility: Option<f32>,
        steps: usize,
        sample: impl Fn(&mut Rng) -> [f32; 2],
    ) -> (Gng, Rng) {
        let mut rng = Rng::new(5);
        let mut g = Gng::new(
            &sample(&mut rng),
            &sample(&mut rng),
            Params::defaults(50, utility),
        );
        for _ in 0..steps {
            g.step(&sample(&mut rng));
        }
        (g, rng)
    }

    fn square(rng: &mut Rng) -> [f32; 2] {
        [rng.next_f32(), rng.next_f32()]
    }

    #[test]
    fn it_grows_to_its_limit_and_covers_a_square() {
        let (g, mut rng) = grow(None, 40_000, square);
        assert_eq!(g.len(), 50);
        assert_eq!(g.error.len(), 50);
        let samples: Vec<f32> = (0..2000).map(|_| rng.next_f32()).collect();
        let qe = crate::metrics::quantization_error(&g.codebook, &samples);
        assert!(qe < 0.1, "QE {qe}");
        assert!(graph_error(&g.codebook, &g.graph, &samples) < 0.1);
    }

    #[test]
    fn edges_never_point_at_missing_nodes() {
        let (g, _) = grow(Some(3.0), 20_000, square);
        let n = g.len() as u32;
        assert!(g.graph.edges().iter().all(|e| e.a < e.b && e.b < n));
        assert_eq!(g.utility.len(), g.len());
    }

    #[test]
    fn with_utility_it_leaves_a_region_the_data_has_left() {
        // Train on the left half, then only on the right half.
        let left = |rng: &mut Rng| [rng.next_f32() * 0.5, rng.next_f32()];
        let right = |rng: &mut Rng| [0.5 + rng.next_f32() * 0.5, rng.next_f32()];
        let stranded = |utility: Option<f32>| {
            let (mut g, mut rng) = grow(utility, 30_000, left);
            for _ in 0..30_000 {
                g.step(&right(&mut rng));
            }
            (0..g.len())
                .filter(|&i| g.codebook.node(i)[0] < 0.45)
                .count()
        };
        let (plain, with_utility) = (stranded(None), stranded(Some(3.0)));
        assert!(plain > 10, "plain GNG keeps {plain} nodes on the left");
        assert!(with_utility <= 2, "GNG-U keeps {with_utility}");
    }
}
