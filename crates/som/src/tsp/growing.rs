//! Angéniol, de La Croix Vaubois and Le Texier's self-organising ring
//! (1988): it starts as a single node and grows. Each city in turn pulls
//! its nearest node and that node's ring neighbours; a node that wins two
//! cities in one pass splits, and one that wins none for three passes
//! goes. The gain's reach G shrinks after every pass.

use super::tour::{nearest, worst_fit};
use super::{Param, centroid};
use crate::rng::Rng;

/// The ring's tunables, in the order [`Growing::new`] reads them.
pub const PARAMS: &[Param] = &[
    Param::new("alpha", "α, G shrink per pass", 0.2, 0.01, 0.5),
    Param::new("g", "G start", 10.0, 1.0, 50.0),
];

/// Passes a node may go without winning before it is removed.
const IDLE_PASSES: u32 = 3;
/// Done when every city has a node this near (unit square), or after this many passes.
const FIT: f32 = 0.01;
/// Passes after which the ring stops, fitted or not.
const MAX_PASSES: u32 = 500;
/// Nodes the ring may grow to, per city.
const MAX_NODES_PER_CITY: usize = 3;

/// One node of the growing ring.
#[derive(Clone, Debug)]
struct Node {
    /// Its position in the unit square.
    at: [f32; 2],
    /// Cities won in this pass.
    wins: u32,
    /// Passes since it last won.
    idle: u32,
    /// A fresh copy can't win until the next pass.
    inhibited: bool,
}

/// Angéniol's growing ring at work on a set of cities in the unit square.
#[derive(Clone, Debug)]
pub struct Growing {
    /// The ring's nodes in ring order; never fewer than one.
    nodes: Vec<Node>,
    /// α, the share G loses after every pass.
    alpha: f32,
    /// G, the reach of a winner's pull along the ring, in nodes; shrinks every pass.
    pub g: f32,
    /// The order cities are shown in this pass, reshuffled after each.
    order: Vec<usize>,
    /// Position in `order` within the current pass.
    next: usize,
    /// Passes completed.
    passes: u32,
    /// Iterations run, one city each.
    t: u64,
    /// Set once every city has a node within [`FIT`], or after [`MAX_PASSES`].
    done: bool,
    /// Shuffles `order` before every pass.
    rng: Rng,
}

impl Growing {
    /// `values` follows [`PARAMS`].
    pub fn new(cities: &[[f32; 2]], values: &[f32], rng: Rng) -> Self {
        let mut ring = Self {
            nodes: vec![Node {
                at: centroid(cities),
                wins: 0,
                idle: 0,
                inhibited: false,
            }],
            alpha: values[0],
            g: values[1],
            order: (0..cities.len()).collect(),
            next: 0,
            passes: 0,
            t: 0,
            done: false,
            rng,
        };
        ring.shuffle();
        ring
    }

    /// Whether the ring has fitted the cities or run out of passes.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Iterations run so far, one city each.
    pub fn iterations(&self) -> u64 {
        self.t
    }

    /// The nodes' positions in the unit square, in ring order.
    pub fn nodes(&self) -> Vec<[f32; 2]> {
        self.nodes.iter().map(|n| n.at).collect()
    }

    /// Number of nodes now on the ring.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Always false: the ring keeps at least one node.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Fisher–Yates shuffles the order the next pass shows cities in.
    fn shuffle(&mut self) {
        for i in (1..self.order.len()).rev() {
            let j = self.rng.below(i + 1);
            self.order.swap(i, j);
        }
    }

    /// One iteration presents one city.
    pub fn step(&mut self, cities: &[[f32; 2]]) {
        if self.done {
            return;
        }
        let x = cities[self.order[self.next]];
        let m = self.nodes.len();
        let winner = (0..m)
            .filter(|&j| !self.nodes[j].inhibited)
            .min_by(|&a, &b| {
                let d = |j: usize| {
                    (self.nodes[j].at[0] - x[0]).powi(2) + (self.nodes[j].at[1] - x[1]).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap_or_else(|| nearest(&self.nodes(), x));
        // f(G, d) = exp(−d²/G²) / √2 over ring distance d; past 4G it is below 10⁻⁷.
        let reach = ((4.0 * self.g).ceil() as usize).min(m / 2);
        let g2 = self.g * self.g;
        for d in 0..=reach {
            let f = (-((d * d) as f32) / g2).exp() * std::f32::consts::FRAC_1_SQRT_2;
            let mut pull = |j: usize| {
                let w = &mut self.nodes[j].at;
                w[0] += f * (x[0] - w[0]);
                w[1] += f * (x[1] - w[1]);
            };
            pull((winner + d) % m);
            if d > 0 && !(m.is_multiple_of(2) && d == m / 2) {
                pull((winner + m - d) % m);
            }
        }
        let node = &mut self.nodes[winner];
        node.wins += 1;
        node.idle = 0;
        if node.wins == 2 && self.nodes.len() < MAX_NODES_PER_CITY * cities.len() {
            let copy = Node {
                at: self.nodes[winner].at,
                wins: 0,
                idle: 0,
                inhibited: true,
            };
            self.nodes.insert(winner + 1, copy);
        }
        self.t += 1;
        self.next += 1;
        if self.next == self.order.len() {
            self.end_pass(cities);
        }
    }

    /// Ends a pass: counts idle passes, drops nodes idle too long (keeping at least three), shrinks
    /// G, reshuffles and checks whether the ring is done.
    fn end_pass(&mut self, cities: &[[f32; 2]]) {
        for n in &mut self.nodes {
            if n.wins == 0 && !n.inhibited {
                n.idle += 1;
            }
            n.wins = 0;
            n.inhibited = false;
        }
        if self.nodes.len() > 3 {
            let keep = self.nodes.len().saturating_sub(3);
            let mut removed = 0;
            self.nodes.retain(|n| {
                let go = n.idle >= IDLE_PASSES && removed < keep;
                removed += go as usize;
                !go
            });
        }
        self.g *= 1.0 - self.alpha;
        self.passes += 1;
        self.next = 0;
        self.shuffle();
        self.done = self.passes >= MAX_PASSES || worst_fit(cities, &self.nodes()) < FIT;
    }
}
