//! Durbin and Willshaw's elastic net (1987): a ring of nodes, every one
//! pulled by every city, each city's pull shared among the nodes by a
//! Gaussian of width K, while the ring's tension keeps neighbors close.
//! As K shrinks, each city ends up pulling a node of its own onto itself.

use super::{Param, center_circle, tour::worst_fit};

/// The net's tunables, in the order [`Elastic::new`] reads them.
pub const PARAMS: &[Param] = &[
    Param::new("nodes", "Nodes per city", 2.5, 1.5, 4.0),
    Param::new("alpha", "α, city pull", 0.2, 0.01, 1.0),
    Param::new("beta", "β, ring tension", 2.0, 0.0, 10.0),
    Param::new("k", "K start", 0.2, 0.05, 0.5),
];

/// K shrinks by this factor every [`K_EVERY`] iterations.
const K_DECAY: f32 = 0.99;
/// Iterations between K shrinking, and between checks for being done.
const K_EVERY: u64 = 25;
/// Done when every city has a node this near (unit square), or K is spent.
const FIT: f32 = 0.01;
/// Done once K falls below this.
const K_END: f32 = 0.005;

/// Durbin and Willshaw's elastic net at work on a set of cities in the unit square.
#[derive(Clone, Debug)]
pub struct Elastic {
    /// The ring's nodes in the unit square, in ring order.
    pub nodes: Vec<[f32; 2]>,
    /// α, how hard the cities pull.
    alpha: f32,
    /// β, how hard the ring's tension pulls neighbors together.
    beta: f32,
    /// K, the width of the Gaussian sharing each city's pull; shrinks over time.
    pub k: f32,
    /// Iterations run.
    t: u64,
    /// Set once every city has a node within [`FIT`], or K is below [`K_END`].
    done: bool,
    /// Scratch: each node's move this iteration.
    delta: Vec<[f32; 2]>,
    /// Scratch: one city's weights over the nodes.
    weights: Vec<f32>,
}

impl Elastic {
    /// `values` follows [`PARAMS`].
    pub fn new(cities: &[[f32; 2]], values: &[f32]) -> Self {
        let m = ((values[0] * cities.len() as f32).round() as usize).max(3);
        Self {
            nodes: center_circle(cities, m),
            alpha: values[1],
            beta: values[2],
            k: values[3],
            t: 0,
            done: false,
            delta: vec![[0.0; 2]; m],
            weights: vec![0.0; m],
        }
    }

    /// Whether the net has fitted the cities or run out of K.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Iterations run so far.
    pub fn iterations(&self) -> u64 {
        self.t
    }

    /// One iteration moves every node once:
    /// Δyⱼ = α Σᵢ wᵢⱼ (xᵢ − yⱼ) + β K (yⱼ₊₁ − 2yⱼ + yⱼ₋₁),
    /// with wᵢⱼ the Gaussian of |xᵢ − yⱼ| at width K, normalized over j.
    pub fn step(&mut self, cities: &[[f32; 2]]) {
        if self.done {
            return;
        }
        let m = self.nodes.len();
        let inv = 1.0 / (2.0 * self.k * self.k);
        self.delta.fill([0.0; 2]);
        for x in cities {
            // Relative to the nearest node, so the weights never all underflow.
            let mut nearest = f32::INFINITY;
            for (w, y) in self.weights.iter_mut().zip(&self.nodes) {
                *w = (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
                nearest = nearest.min(*w);
            }
            let mut sum = 0.0;
            for w in &mut self.weights {
                let e = (*w - nearest) * inv;
                // Past e⁻³⁰ a node's share is nothing.
                *w = if e < 30.0 { (-e).exp() } else { 0.0 };
                sum += *w;
            }
            let scale = self.alpha / sum;
            for ((d, y), &w) in self.delta.iter_mut().zip(&self.nodes).zip(&self.weights) {
                if w > 0.0 {
                    d[0] += scale * w * (x[0] - y[0]);
                    d[1] += scale * w * (x[1] - y[1]);
                }
            }
        }
        let tension = self.beta * self.k;
        for j in 0..m {
            let (prev, here, next) = (
                self.nodes[(j + m - 1) % m],
                self.nodes[j],
                self.nodes[(j + 1) % m],
            );
            self.delta[j][0] += tension * (next[0] - 2.0 * here[0] + prev[0]);
            self.delta[j][1] += tension * (next[1] - 2.0 * here[1] + prev[1]);
        }
        for (y, d) in self.nodes.iter_mut().zip(&self.delta) {
            y[0] += d[0];
            y[1] += d[1];
        }
        self.t += 1;
        if self.t.is_multiple_of(K_EVERY) {
            self.k *= K_DECAY;
            self.done = self.k < K_END || worst_fit(cities, &self.nodes) < FIT;
        }
    }
}
