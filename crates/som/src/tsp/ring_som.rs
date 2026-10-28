//! A Kohonen map on a closed ring of nodes: shown cities one at a time, the
//! nearest node and its ring neighbors move towards them, and as the
//! neighborhood shrinks the ring settles on a tour.

use super::tour::nearest;
use super::{Param, center_circle};
use crate::rng::Rng;
use crate::schedule::{Decay, Schedule, kernel};

/// The ring's tunables, in the order [`RingSom::new`] reads them.
pub const PARAMS: &[Param] = &[
    Param::new("nodes", "Nodes per city", 2.0, 1.0, 4.0),
    Param::new("samples", "Samples per city", 100.0, 10.0, 500.0),
    Param::new("alpha", "α start", 0.8, 0.05, 1.0),
    Param::new("sigma", "σ start, share of the ring", 0.1, 0.01, 0.5),
];

/// Where α and σ end: σ in nodes, so the last updates move one node.
const ALPHA_END: f32 = 0.02;
/// σ's end, in nodes; see [`ALPHA_END`].
const SIGMA_END: f32 = 0.5;

/// A Kohonen ring at work on a set of cities in the unit square.
#[derive(Clone, Debug)]
pub struct RingSom {
    /// The ring's nodes in the unit square, in ring order.
    pub nodes: Vec<[f32; 2]>,
    /// α and σ over the run: as many steps as samples to show, decaying exponentially.
    schedule: Schedule,
    /// Cities shown so far.
    t: u64,
    /// Picks which city to show next.
    rng: Rng,
}

impl RingSom {
    /// `values` follows [`PARAMS`]; σ start is a share of the ring.
    pub fn new(cities: &[[f32; 2]], values: &[f32], rng: Rng) -> Self {
        let n = cities.len();
        let m = ((values[0] * n as f32).round() as usize).max(3);
        let steps = (values[1] * n as f32).round() as u64;
        Self {
            nodes: center_circle(cities, m),
            schedule: Schedule {
                steps: steps.max(1),
                alpha: (values[2], ALPHA_END),
                sigma: ((values[3] * m as f32).max(SIGMA_END), SIGMA_END),
                decay: Decay::Exponential,
                continuous: false,
            },
            t: 0,
            rng,
        }
    }

    /// Whether every sample has been shown.
    pub fn done(&self) -> bool {
        self.schedule.finished(self.t)
    }

    /// Cities shown so far.
    pub fn iterations(&self) -> u64 {
        self.t
    }

    /// The current σ, in nodes.
    pub fn sigma(&self) -> f32 {
        self.schedule.at(self.t).1
    }

    /// One iteration is one city shown.
    pub fn step(&mut self, cities: &[[f32; 2]]) {
        if self.done() {
            return;
        }
        let (alpha, sigma) = self.schedule.at(self.t);
        let x = cities[self.rng.below(cities.len())];
        let m = self.nodes.len();
        let bmu = nearest(&self.nodes, x);
        // Beyond 4σ the kernel is below 3·10⁻⁴: not worth the update.
        let reach = ((4.0 * sigma).ceil() as usize).min(m / 2);
        for d in 0..=reach {
            let h = alpha * kernel((d * d) as f32, sigma);
            let mut pull = |j: usize| {
                let w = &mut self.nodes[j];
                w[0] += h * (x[0] - w[0]);
                w[1] += h * (x[1] - w[1]);
            };
            pull((bmu + d) % m);
            // The far side of an even ring is one node, reached once.
            if d > 0 && !(m.is_multiple_of(2) && d == m / 2) {
                pull((bmu + m - d) % m);
            }
        }
        self.t += 1;
    }
}
