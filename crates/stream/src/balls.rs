//! Streaming clusters as balls: each cluster is a center and a radius, the
//! root mean square distance of its points. A point joins the nearest ball
//! if it lies within that ball's reach, a few radii out, and otherwise
//! spawns a ball of its own. Balls that come to overlap are merged, and
//! balls that no longer get points fade and are pruned, so the number of
//! clusters is whatever the stream makes of it.
//!
//! Finding the nearest ball uses the ball neighborhoods of Xia et al.
//! (Ball k-means, 2020): if a point lies within ball b's reach R, any
//! center nearer to it than b's is less than 2R from b's center, so only
//! b's neighbors, the balls that close, need checking. Each ball also links
//! to its few nearest balls, so the balls form a graph that a greedy walk
//! from the ball the last point joined can cross to b in a few steps, the
//! same idea HNSW builds its search on.
//! A ball's place in the lists is refreshed when it appears, merges, or
//! has moved or grown enough to matter, one distance per other ball, with
//! a margin that keeps the search exact in between.

use crate::{Event, EventKind, Point, dist2, record};

/// What the balls are tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallParams {
    /// A point joins a ball within this many radii of its center.
    pub reach: f32,
    /// The radius a new ball starts with, before its points say otherwise.
    /// Small, so a ball seeded in the gap between two clusters doesn't
    /// reach both and grow to cover them.
    pub r_init: f32,
    /// The smallest radius a ball is treated as having, so a ball of a few
    /// identical points still has a reach.
    pub r_min: f32,
    /// Two balls merge when their centers are closer than this many times
    /// the sum of their radii. Above 1, so a ball spawned out in another's
    /// tail is taken back in.
    pub merge: f32,
    /// Every weight is multiplied by `1 − decay` per point: forgetting,
    /// so balls follow clusters that move.
    pub decay: f32,
    /// The weight a ball needs to count as a cluster.
    pub support: f32,
    /// A ball whose weight falls below this, once past its youth, is pruned.
    pub prune: f32,
}

impl Default for BallParams {
    /// Reach 2.5 radii, radius 0.02 to start and 0.015 at least, merging
    /// below 1.4 times the summed radii, a half-life of about 1400 points,
    /// support 10 and pruning below weight 1.
    fn default() -> Self {
        BallParams {
            reach: 2.5,
            r_init: 0.02,
            r_min: 0.015,
            merge: 1.4,
            decay: 0.0005,
            support: 10.0,
            prune: 1.0,
        }
    }
}

/// Points a new ball is spared from pruning.
const YOUTH: u64 = 1000;
/// Points between two pruning sweeps.
const PRUNE_EVERY: u64 = 100;
/// How far a ball may move or grow before its neighbor links are refreshed.
const SLACK: f32 = 0.01;
/// Nearest balls every ball links to besides its neighborhood, so the
/// balls form a graph a greedy walk can cross.
const LINKS: usize = 3;

/// One cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    /// A stable id, for coloring the same cluster the same way over time.
    pub id: u32,
    /// The center: the decayed mean of its points.
    pub center: Point,
    /// The decayed mean squared distance of its points from the center.
    pub m2: f32,
    /// The decayed number of points it has taken.
    pub weight: f32,
    /// When it was spawned, in points of the stream.
    pub born: u64,
    /// How far it has moved plus how much its reach has grown since its
    /// place in the neighbor lists was last refreshed.
    slack: f32,
}

impl Ball {
    /// The radius: root mean square distance of its points, at least `r_min`.
    pub fn radius(&self, p: &BallParams) -> f32 {
        self.m2.sqrt().max(p.r_min)
    }

    /// How far out a point may lie and still join.
    pub fn reach(&self, p: &BallParams) -> f32 {
        p.reach * self.radius(p)
    }
}

/// How the last point was placed, for showing a single step.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The point.
    pub point: Point,
    /// Ids of the balls the greedy walk went through, in order.
    pub path: Vec<u32>,
    /// Every ball whose distance was computed, with the distance.
    pub checked: Vec<(u32, f32)>,
    /// Whether the walk ended outside every reach and all balls were scanned.
    pub full_scan: bool,
    /// The id of the ball the point joined or spawned.
    pub ball: u32,
    /// Whether it spawned that ball.
    pub spawned: bool,
}

/// The balls and their bookkeeping.
#[derive(Clone, Debug)]
pub struct Balls {
    /// What they are tuned by.
    pub params: BallParams,
    /// The balls, in no particular order.
    balls: Vec<Ball>,
    /// Per ball, the indices of the balls within its neighborhood.
    neighbors: Vec<Vec<usize>>,
    /// The id the next ball gets.
    next_id: u32,
    /// The ball the last point joined, where the next walk starts.
    last: Option<usize>,
    /// Points taken so far.
    t: u64,
    /// Recent spawns, merges and prunes.
    pub events: Vec<Event>,
    /// How the last point was placed.
    pub trace: Option<Trace>,
    /// Distances computed to place points.
    pub search_checks: u64,
    /// Distances computed to keep the neighbor lists.
    pub upkeep_checks: u64,
}

impl Balls {
    /// No balls yet.
    pub fn new(params: BallParams) -> Balls {
        Balls {
            params,
            balls: Vec::new(),
            neighbors: Vec::new(),
            next_id: 0,
            last: None,
            t: 0,
            events: Vec::new(),
            trace: None,
            search_checks: 0,
            upkeep_checks: 0,
        }
    }

    /// The balls.
    pub fn balls(&self) -> &[Ball] {
        &self.balls
    }

    /// Points taken so far.
    pub fn t(&self) -> u64 {
        self.t
    }

    /// The balls with enough weight to count as clusters.
    pub fn clusters(&self) -> impl Iterator<Item = &Ball> {
        self.balls
            .iter()
            .filter(|b| b.weight >= self.params.support)
    }

    /// The index of the ball nearest `x` among those counting as clusters,
    /// by plain scan; `None` without any.
    pub fn assign(&self, x: Point) -> Option<u32> {
        self.clusters()
            .min_by(|a, b| dist2(a.center, x).total_cmp(&dist2(b.center, x)))
            .map(|b| b.id)
    }

    /// Takes one point: joins the nearest ball within reach or spawns one,
    /// then merges, decays and prunes. Returns the id of the ball it went to.
    pub fn insert(&mut self, x: Point) -> u32 {
        self.t += 1;
        let (target, trace) = self.search(x);
        let (joined, id) = match target {
            Some(i) => {
                let joined = self.balls[i].id;
                self.join(i, x);
                self.last = Some(i);
                self.merge_into(i);
                (joined, self.last.map_or(joined, |l| self.balls[l].id))
            }
            None => {
                let id = self.spawn(x);
                (id, id)
            }
        };
        self.trace = Some(Trace {
            ball: joined,
            spawned: target.is_none(),
            ..trace
        });
        let keep = 1.0 - self.params.decay;
        for b in &mut self.balls {
            b.weight *= keep;
        }
        if self.t.is_multiple_of(PRUNE_EVERY) {
            self.prune();
        }
        id
    }

    /// The ball `x` should join, if any is in reach, and how it was found.
    fn search(&mut self, x: Point) -> (Option<usize>, Trace) {
        let mut trace = Trace {
            point: x,
            path: Vec::new(),
            checked: Vec::new(),
            full_scan: false,
            ball: 0,
            spawned: false,
        };
        let Some(start) = self
            .last
            .filter(|&i| i < self.balls.len())
            .or((!self.balls.is_empty()).then_some(0))
        else {
            return (None, trace);
        };
        // Each ball's distance is computed at most once per point.
        let mut memo = vec![f32::NAN; self.balls.len()];
        let mut dist = |i: usize, trace: &mut Trace| {
            if memo[i].is_nan() {
                self.search_checks += 1;
                memo[i] = dist2(self.balls[i].center, x).sqrt();
                trace.checked.push((self.balls[i].id, memo[i]));
            }
            memo[i]
        };
        let mut cur = start;
        let mut d_cur = dist(cur, &mut trace);
        trace.path.push(self.balls[cur].id);
        loop {
            let mut best = (cur, d_cur);
            for &j in &self.neighbors[cur] {
                let d = dist(j, &mut trace);
                if d < best.1 {
                    best = (j, d);
                }
            }
            if best.0 == cur {
                break;
            }
            (cur, d_cur) = best;
            trace.path.push(self.balls[cur].id);
        }
        // Within reach of `cur`: its neighbors held every nearer center.
        if d_cur <= self.balls[cur].reach(&self.params) {
            return (Some(cur), trace);
        }
        // Outside: a far ball might still reach it, so look at them all.
        trace.full_scan = true;
        let mut best: Option<(usize, f32)> = None;
        for i in 0..self.balls.len() {
            let d = dist(i, &mut trace);
            if d <= self.balls[i].reach(&self.params) && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        (best.map(|(i, _)| i), trace)
    }

    /// Moves ball `i` to take in `x`.
    fn join(&mut self, i: usize, x: Point) {
        let p = self.params;
        let b = &mut self.balls[i];
        let before = (b.center, b.reach(&p));
        b.weight += 1.0;
        let rate = 1.0 / b.weight;
        let d2 = dist2(b.center, x);
        b.center = [
            b.center[0] + rate * (x[0] - b.center[0]),
            b.center[1] + rate * (x[1] - b.center[1]),
        ];
        b.m2 += rate * (d2 - b.m2);
        b.slack += dist2(before.0, b.center).sqrt() + (b.reach(&p) - before.1).max(0.0);
        if b.slack > SLACK {
            self.link(i);
        }
    }

    /// A new ball at `x`; returns its id.
    fn spawn(&mut self, x: Point) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.balls.push(Ball {
            id,
            center: x,
            m2: self.params.r_init * self.params.r_init,
            weight: 1.0,
            born: self.t,
            slack: 0.0,
        });
        self.last = Some(self.balls.len() - 1);
        record(
            &mut self.events,
            Event {
                kind: EventKind::Spawn,
                at: x,
                t: self.t,
            },
        );
        self.neighbors.push(Vec::new());
        self.link(self.balls.len() - 1);
        id
    }

    /// Merges every neighbor of ball `i` that overlaps it into the heavier
    /// of the two.
    fn merge_into(&mut self, i: usize) {
        let p = self.params;
        let near: Vec<usize> = self.neighbors[i].clone();
        let mut i = i;
        for j in near {
            if j >= self.balls.len() || j == i {
                continue;
            }
            let (a, b) = (&self.balls[i], &self.balls[j]);
            if dist2(a.center, b.center).sqrt() >= p.merge * (a.radius(&p) + b.radius(&p)) {
                continue;
            }
            let (keep, gone) = if a.weight >= b.weight { (i, j) } else { (j, i) };
            let (k, g) = (self.balls[keep].clone(), self.balls[gone].clone());
            let w = k.weight + g.weight;
            let center = [
                (k.center[0] * k.weight + g.center[0] * g.weight) / w,
                (k.center[1] * k.weight + g.center[1] * g.weight) / w,
            ];
            // The pooled spread: each ball's own, plus how far its center is from the new one.
            let m2 = (k.weight * (k.m2 + dist2(k.center, center))
                + g.weight * (g.m2 + dist2(g.center, center)))
                / w;
            let kb = &mut self.balls[keep];
            kb.center = center;
            kb.m2 = m2;
            kb.weight = w;
            record(
                &mut self.events,
                Event {
                    kind: EventKind::Merge,
                    at: center,
                    t: self.t,
                },
            );
            self.remove(gone);
            i = if keep > gone { keep - 1 } else { keep };
            self.last = Some(i);
            self.link(i);
            break;
        }
    }

    /// Removes faded balls past their youth.
    fn prune(&mut self) {
        let p = self.params;
        let mut i = 0;
        while i < self.balls.len() {
            let b = &self.balls[i];
            if b.weight < p.prune && self.t - b.born > YOUTH {
                record(
                    &mut self.events,
                    Event {
                        kind: EventKind::Prune,
                        at: b.center,
                        t: self.t,
                    },
                );
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Removes ball `i`, keeping `last` pointing at the same ball where it survives
    /// and dropping it from every neighbor list.
    fn remove(&mut self, i: usize) {
        self.balls.remove(i);
        self.neighbors.remove(i);
        for list in &mut self.neighbors {
            list.retain(|&j| j != i);
            for j in list.iter_mut() {
                if *j > i {
                    *j -= 1;
                }
            }
        }
        self.last = match self.last {
            Some(l) if l == i => None,
            Some(l) if l > i => Some(l - 1),
            other => other,
        };
    }

    /// Brings ball `i`'s place in the neighbor lists up to date after it
    /// appeared or moved: its own list anew, and itself added to every
    /// other ball's list it now belongs in. Costs one distance per ball,
    /// where a full rebuild costs one per pair.
    fn link(&mut self, i: usize) {
        let p = self.params;
        let n = self.balls.len();
        let limit = |b: &Ball| 2.0 * b.reach(&p) + 4.0 * SLACK;
        let mut by_distance: Vec<(f32, usize)> = Vec::with_capacity(n);
        for j in 0..n {
            if j == i {
                continue;
            }
            self.upkeep_checks += 1;
            let d = dist2(self.balls[i].center, self.balls[j].center).sqrt();
            by_distance.push((d, j));
            if d < limit(&self.balls[j]) && !self.neighbors[j].contains(&i) {
                self.neighbors[j].push(i);
            }
        }
        by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
        let own = limit(&self.balls[i]);
        self.neighbors[i] = by_distance
            .iter()
            .enumerate()
            .filter(|&(rank, &(d, _))| rank < LINKS || d < own)
            .map(|(_, &(_, j))| j)
            .collect();
        self.balls[i].slack = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{Scenario, Source};

    fn run(scenario: Scenario, n: usize) -> Balls {
        let mut src = Source::new(scenario, 1);
        let mut balls = Balls::new(BallParams::default());
        for _ in 0..n {
            balls.insert(src.next_point().0);
        }
        balls
    }

    #[test]
    fn five_blobs_make_five_clusters() {
        let balls = run(Scenario::Blobs, 6000);
        assert_eq!(balls.clusters().count(), 5);
    }

    #[test]
    fn the_walk_finds_the_same_ball_as_a_full_scan() {
        let mut src = Source::new(Scenario::Drift, 2);
        let mut balls = Balls::new(BallParams::default());
        for _ in 0..5000 {
            let x = src.next_point().0;
            if balls.balls().is_empty() {
                balls.insert(x);
                continue;
            }
            let p = balls.params;
            let scan = balls
                .balls()
                .iter()
                .filter(|b| dist2(b.center, x).sqrt() <= b.reach(&p))
                .min_by(|a, b| dist2(a.center, x).total_cmp(&dist2(b.center, x)))
                .map(|b| b.id);
            let mut probe = balls.clone();
            probe.insert(x);
            let trace = probe.trace.clone().unwrap();
            if let Some(id) = scan {
                assert!(
                    !trace.spawned && trace.ball == id,
                    "walk joined {} for nearest {id}",
                    trace.ball
                );
            } else {
                assert!(trace.spawned);
            }
            balls = probe;
        }
    }

    #[test]
    fn the_walk_checks_a_few_of_many_balls() {
        // 36 small blobs on a grid, points from any of them in random order.
        let mut rng = crate::rng::Rng::new(6);
        let mut balls = Balls::new(BallParams::default());
        let n = 30_000;
        for _ in 0..n {
            let k = rng.below(36);
            let c = [0.12 + 0.15 * (k % 6) as f32, 0.12 + 0.15 * (k / 6) as f32];
            balls.insert([c[0] + rng.normal() * 0.012, c[1] + rng.normal() * 0.012]);
        }
        assert_eq!(balls.clusters().count(), 36);
        let per_point = balls.search_checks as f32 / n as f32;
        let upkeep = balls.upkeep_checks as f32 / n as f32;
        // About half: a flat graph, walked from wherever the last point went.
        assert!(
            per_point < 0.6 * 36.0,
            "{per_point} of 36 balls checked per point"
        );
        assert!(upkeep < 2.0, "{upkeep} upkeep checks per point");
    }

    #[test]
    fn a_dying_cluster_is_pruned() {
        let mut balls = Balls::new(BallParams::default());
        let mut rng = crate::rng::Rng::new(4);
        let blob = |c: f32, rng: &mut crate::rng::Rng| {
            [c + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03]
        };
        for i in 0..4000 {
            balls.insert(blob(if i % 2 == 0 { 0.3 } else { 0.7 }, &mut rng));
        }
        assert_eq!(balls.clusters().count(), 2);
        // From its steady weight near 1000, a ball fades below 1 in ~14 000 points.
        for _ in 0..16_000 {
            balls.insert(blob(0.7, &mut rng));
        }
        assert_eq!(balls.clusters().count(), 1);
        assert!(balls.balls().iter().all(|b| b.center[0] > 0.5));
        assert!(balls.events.iter().any(|e| e.kind == EventKind::Prune));
    }

    #[test]
    fn balls_that_come_together_merge() {
        let mut balls = Balls::new(BallParams::default());
        let mut rng = crate::rng::Rng::new(5);
        // Two seeds close together grow into one blob.
        balls.insert([0.5, 0.5]);
        balls.insert([0.53, 0.5]);
        for _ in 0..2000 {
            balls.insert([0.515 + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03]);
        }
        assert_eq!(balls.clusters().count(), 1);
    }
}
