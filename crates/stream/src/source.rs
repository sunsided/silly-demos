//! Streams of labeled points: each point comes with the cluster that made
//! it, so a method's clusters can be scored against the truth.

use crate::Point;
use crate::rng::Rng;

/// The label of a point dropped by hand, which belongs to no true cluster.
pub const PAINTED: u32 = u32::MAX;

/// Samples a drifting stream takes for one full cycle of its story.
pub const DRIFT_CYCLE: u64 = 12_000;

/// The kinds of stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Five round Gaussian blobs: what every method should get right.
    Blobs,
    /// Four long, thin, rotated Gaussians: a round ball fits them badly.
    Anisotropic,
    /// Two concentric rings: one cluster each, but no center inside it.
    Rings,
    /// Two interleaved half circles: close, curved and not ball-shaped.
    Moons,
    /// Clusters that move, split, die and appear over a cycle.
    Drift,
}

impl Scenario {
    /// Every scenario, in UI order.
    pub const ALL: [Scenario; 5] = [
        Scenario::Blobs,
        Scenario::Anisotropic,
        Scenario::Rings,
        Scenario::Moons,
        Scenario::Drift,
    ];

    /// The scenario for a UI id; `None` for anything else.
    pub fn parse(id: &str) -> Option<Scenario> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The scenario's UI id.
    pub fn id(self) -> &'static str {
        match self {
            Scenario::Blobs => "blobs",
            Scenario::Anisotropic => "anisotropic",
            Scenario::Rings => "rings",
            Scenario::Moons => "moons",
            Scenario::Drift => "drift",
        }
    }
}

/// A Gaussian cluster: center, the two standard deviations along its axes,
/// and the axis angle.
#[derive(Clone, Copy, Debug)]
struct Gaussian {
    /// Its mean.
    center: Point,
    /// Standard deviation along its first axis.
    sx: f32,
    /// Standard deviation along its second axis.
    sy: f32,
    /// The first axis' angle from +x, radians.
    angle: f32,
}

impl Gaussian {
    /// A round Gaussian of standard deviation `s` at `center`.
    fn round(center: Point, s: f32) -> Gaussian {
        Gaussian {
            center,
            sx: s,
            sy: s,
            angle: 0.0,
        }
    }

    /// One sample.
    fn sample(&self, rng: &mut Rng) -> Point {
        let (u, v) = (rng.normal() * self.sx, rng.normal() * self.sy);
        let (s, c) = self.angle.sin_cos();
        [
            self.center[0] + c * u - s * v,
            self.center[1] + s * u + c * v,
        ]
    }
}

/// A seeded stream of labeled points in the unit square.
#[derive(Clone, Debug)]
pub struct Source {
    /// What the stream looks like.
    scenario: Scenario,
    /// The fixed Gaussians of the blob scenarios; empty for the others.
    gaussians: Vec<Gaussian>,
    /// The stream's random numbers.
    rng: Rng,
    /// Points made so far.
    t: u64,
}

/// How far the blob scenarios keep their centers apart.
const MIN_SEPARATION: f32 = 0.22;

impl Source {
    /// A stream of `scenario` from `seed`.
    pub fn new(scenario: Scenario, seed: u64) -> Source {
        let mut rng = Rng::new(seed);
        let gaussians = match scenario {
            Scenario::Blobs => centers(&mut rng, 5)
                .into_iter()
                .map(|c| Gaussian::round(c, 0.035))
                .collect(),
            Scenario::Anisotropic => centers(&mut rng, 4)
                .into_iter()
                .map(|c| Gaussian {
                    center: c,
                    sx: 0.07,
                    sy: 0.012,
                    angle: rng.next_f32() * std::f32::consts::PI,
                })
                .collect(),
            _ => Vec::new(),
        };
        Source {
            scenario,
            gaussians,
            rng,
            t: 0,
        }
    }

    /// Points made so far.
    pub fn t(&self) -> u64 {
        self.t
    }

    /// How many true clusters are making points now.
    pub fn active(&self) -> usize {
        match self.scenario {
            Scenario::Blobs | Scenario::Anisotropic => self.gaussians.len(),
            Scenario::Rings | Scenario::Moons => 2,
            Scenario::Drift => drift(self.t).len(),
        }
    }

    /// The next point and the label of the cluster that made it.
    pub fn next_point(&mut self) -> (Point, u32) {
        let rng = &mut self.rng;
        let (p, label) = match self.scenario {
            Scenario::Blobs | Scenario::Anisotropic => {
                let k = rng.below(self.gaussians.len());
                (self.gaussians[k].sample(rng), k as u32)
            }
            Scenario::Rings => {
                let k = rng.below(2);
                let r = [0.16, 0.36][k] + rng.normal() * 0.015;
                let a = rng.next_f32() * std::f32::consts::TAU;
                ([0.5 + r * a.cos(), 0.5 + r * a.sin()], k as u32)
            }
            Scenario::Moons => {
                let k = rng.below(2);
                let a = rng.next_f32() * std::f32::consts::PI;
                let (x, y) = if k == 0 {
                    (0.38 + 0.22 * a.cos(), 0.45 - 0.22 * a.sin())
                } else {
                    (0.62 - 0.22 * a.cos(), 0.55 + 0.22 * a.sin())
                };
                ([x + rng.normal() * 0.02, y + rng.normal() * 0.02], k as u32)
            }
            Scenario::Drift => {
                let live = drift(self.t);
                let (label, g) = live[rng.below(live.len())];
                (g.sample(rng), label)
            }
        };
        self.t += 1;
        (p, label)
    }
}

/// `n` centers inside [0.15, 0.85]², at least [`MIN_SEPARATION`] apart where
/// a few hundred tries allow.
fn centers(rng: &mut Rng, n: usize) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(n);
    let mut tries = 0;
    while out.len() < n {
        let c = [0.15 + rng.next_f32() * 0.7, 0.15 + rng.next_f32() * 0.7];
        tries += 1;
        if tries > 500
            || out
                .iter()
                .all(|o| crate::dist2(*o, c).sqrt() >= MIN_SEPARATION)
        {
            out.push(c);
        }
    }
    out
}

/// The drifting story's clusters at sample `t`, with their labels. Over each
/// cycle of [`DRIFT_CYCLE`] samples: cluster 0 circles the center; cluster 1
/// splits into 1 and 4 a quarter of the way in, which then move apart;
/// cluster 2 dies halfway; cluster 3 appears at two thirds.
fn drift(t: u64) -> Vec<(u32, Gaussian)> {
    let phase = (t % DRIFT_CYCLE) as f32 / DRIFT_CYCLE as f32;
    let s = 0.03;
    let a = std::f32::consts::TAU * phase;
    let mut live = vec![(
        0,
        Gaussian::round([0.5 + 0.3 * a.cos(), 0.5 + 0.3 * a.sin()], s),
    )];
    if phase < 0.25 {
        live.push((1, Gaussian::round([0.5, 0.5], s)));
    } else {
        let apart = ((phase - 0.25) / 0.75 * 0.4).min(0.2);
        live.push((1, Gaussian::round([0.5 - apart, 0.5], s)));
        live.push((4, Gaussian::round([0.5 + apart, 0.5], s)));
    }
    if phase < 0.5 {
        live.push((2, Gaussian::round([0.2, 0.2], s)));
    }
    if phase >= 2.0 / 3.0 {
        live.push((3, Gaussian::round([0.8, 0.2], s)));
    }
    live
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_label_their_points_and_stay_near_the_square() {
        let mut src = Source::new(Scenario::Blobs, 1);
        let mut seen = [0; 5];
        for _ in 0..5000 {
            let (p, label) = src.next_point();
            seen[label as usize] += 1;
            assert!(p.iter().all(|v| (-0.1..=1.1).contains(v)), "{p:?}");
        }
        assert!(seen.iter().all(|&n| n > 800), "{seen:?}");
        assert_eq!(src.active(), 5);
    }

    #[test]
    fn the_drifting_story_changes_its_cluster_count() {
        let count = |t: u64| drift(t).len();
        assert_eq!(count(0), 3);
        assert_eq!(count(DRIFT_CYCLE / 3), 4, "after the split");
        assert_eq!(count(DRIFT_CYCLE * 3 / 5), 3, "after cluster 2 dies");
        assert_eq!(count(DRIFT_CYCLE * 9 / 10), 4, "after cluster 3 appears");
        assert_eq!(count(DRIFT_CYCLE), 3, "the cycle starts over");
    }

    #[test]
    fn the_same_seed_gives_the_same_stream() {
        let run = || {
            let mut s = Source::new(Scenario::Anisotropic, 9);
            (0..100).map(|_| s.next_point()).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
        assert_eq!(Scenario::parse("moons"), Some(Scenario::Moons));
        assert_eq!(Scenario::parse("spiral"), None);
    }
}
