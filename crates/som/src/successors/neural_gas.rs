//! Martinetz and Schulten's neural gas (1991): a map without a lattice.
//! Shown a sample, every node moves towards it by an amount that falls
//! with its distance rank, not with a position on a grid, so nothing can
//! fold or twist. Joining each sample's two nearest nodes as it goes
//! (competitive Hebbian learning) makes it a topology representing
//! network: the edges it keeps are those of the data's own shape.

use super::graph::Graph;
use crate::codebook::{Codebook, distance2};
use crate::schedule::{Decay, Schedule};

/// Training lengths and rates, after Martinetz and Schulten (1991, 1994).
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// The step size ε (`alpha`) and the rank range λ (`sigma`), from their
    /// start to their end values over `steps` samples; reuses the map's
    /// schedule, with λ in ranks rather than lattice units.
    pub schedule: Schedule,
    /// The age an edge may reach before it goes, from start to end over
    /// the same steps.
    pub max_age: (f32, f32),
}

impl Params {
    /// The published defaults for `n` nodes over `steps` samples: ε 0.5 →
    /// 0.005, λ n/2 → 0.01, edge ages 20 → 200 (scaled with the run).
    pub fn defaults(n: usize, steps: u64) -> Params {
        Params {
            schedule: Schedule {
                steps,
                alpha: (0.5, 0.005),
                sigma: ((n as f32 / 2.0).max(1.0), 0.01),
                decay: Decay::Exponential,
                continuous: false,
            },
            max_age: (20.0, 200.0),
        }
    }
}

/// A neural gas and the edges it has learned.
#[derive(Clone, Debug)]
pub struct NeuralGas {
    /// The nodes' positions.
    pub codebook: Codebook,
    /// The learned edges.
    pub graph: Graph,
    /// Rates and their schedule.
    pub params: Params,
    /// Samples presented so far.
    pub t: u64,
    /// Scratch: node indices sorted by distance to the current sample.
    order: Vec<usize>,
    /// Scratch: each node's squared distance to the current sample.
    dist: Vec<f32>,
}

impl NeuralGas {
    /// A gas on `codebook`'s nodes, with no edges yet.
    pub fn new(codebook: Codebook, params: Params) -> NeuralGas {
        let n = codebook.len();
        NeuralGas {
            codebook,
            graph: Graph::default(),
            params,
            t: 0,
            order: (0..n).collect(),
            dist: vec![0.0; n],
        }
    }

    /// Whether the schedule has run out; never with a continuous one.
    pub fn finished(&self) -> bool {
        self.params.schedule.finished(self.t)
    }

    /// One sample: every node moves towards `x` by ε · exp(−k / λ), k its
    /// distance rank (0 for the nearest); the nearest two are joined, the
    /// nearest's other edges age, and edges past the current age limit go.
    /// Does nothing once finished.
    pub fn step(&mut self, x: &[f32]) {
        if self.finished() || self.codebook.is_empty() {
            return;
        }
        let (eps, lambda) = self.params.schedule.at(self.t);
        let dim = self.codebook.dim;
        for (d, w) in self
            .dist
            .iter_mut()
            .zip(self.codebook.weights.chunks_exact(dim))
        {
            *d = distance2(w, x);
        }
        let dist = &self.dist;
        self.order.sort_by(|&a, &b| dist[a].total_cmp(&dist[b]));
        for (rank, &i) in self.order.iter().enumerate() {
            let h = eps * (-(rank as f32) / lambda).exp();
            // Past e⁻¹⁵ the move is nothing, and ranks only grow from here.
            if h < 1e-7 {
                break;
            }
            for (w, &v) in self.codebook.weights[i * dim..(i + 1) * dim]
                .iter_mut()
                .zip(x)
            {
                *w += h * (v - *w);
            }
        }
        if self.order.len() >= 2 {
            let (s1, s2) = (self.order[0] as u32, self.order[1] as u32);
            self.graph.connect(s1, s2);
            self.graph.age_at(s1);
            self.graph.connect(s1, s2);
            self.graph.prune(self.max_age());
        }
        self.t += 1;
    }

    /// The age limit for edges now.
    fn max_age(&self) -> u32 {
        let s = &self.params.schedule;
        let f = (self.t.min(s.steps) as f64 / s.steps as f64) as f32;
        let (start, end) = self.params.max_age;
        (start * (end / start).powf(f)).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn gas(n: usize, steps: u64, seed: u64) -> (NeuralGas, Rng) {
        let mut rng = Rng::new(seed);
        let weights = (0..2 * n).map(|_| rng.next_f32()).collect();
        let cb = Codebook { dim: 2, weights };
        (NeuralGas::new(cb, Params::defaults(n, steps)), rng)
    }

    #[test]
    fn nodes_move_a_share_of_the_way_that_falls_with_rank() {
        let cb = Codebook {
            dim: 1,
            weights: vec![0.0, 0.5, 2.0],
        };
        let mut g = NeuralGas::new(cb, Params::defaults(3, 100));
        g.step(&[1.0]);
        let before = [0.0, 0.5, 2.0];
        // The share of the way to the sample each node moved.
        let share: Vec<f32> = (0..3)
            .map(|i| (g.codebook.weights[i] - before[i]) / (1.0 - before[i]))
            .collect();
        // Node 1 is nearest (rank 0); nodes 0 and 2 tie, 0 first (ranks 1, 2).
        assert!(
            share[1] > share[0] && share[0] > share[2] && share[2] > 0.0,
            "{share:?}"
        );
        assert!(g.graph.connected(1, 0));
    }

    #[test]
    fn a_square_is_covered_evenly_and_its_edges_join_neighbors() {
        let (mut g, mut rng) = gas(25, 20_000, 3);
        while !g.finished() {
            g.step(&[rng.next_f32(), rng.next_f32()]);
        }
        let samples: Vec<f32> = (0..2000).map(|_| rng.next_f32()).collect();
        let qe = crate::metrics::quantization_error(&g.codebook, &samples);
        assert!(qe < 0.13, "QE {qe}");
        let te = super::super::graph::graph_error(&g.codebook, &g.graph, &samples);
        assert!(te < 0.05, "TE {te}");
        // Every edge is short: no node is joined across the square.
        for e in g.graph.edges() {
            let d = distance2(g.codebook.node(e.a as usize), g.codebook.node(e.b as usize)).sqrt();
            assert!(d < 0.5, "edge of {d}");
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_gas() {
        let run = || {
            let (mut g, mut rng) = gas(10, 2000, 7);
            while !g.finished() {
                g.step(&[rng.next_f32(), rng.next_f32()]);
            }
            g.codebook.weights
        };
        assert_eq!(run(), run());
    }
}
