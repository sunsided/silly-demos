//! Streaming clusters as ellipses: each cluster keeps a decayed mean and
//! covariance, and a point's distance to it is the Mahalanobis distance,
//! d²(x) = (x − μ)ᵀ Σ⁻¹ (x − μ), so a cluster's reach is a number of
//! standard deviations along its own axes. A long thin cluster is one
//! long thin ellipse instead of a chain of balls.
//!
//! The rules mirror the balls'. A point joins the best-fitting ellipse
//! that reaches it, best by Gaussian log-likelihood (d² + ln |Σ|, so a big
//! loose ellipse doesn't outbid a tight one), or spawns one. Ellipses merge
//! when the merged ellipse is no bigger than the two together, split when
//! the 2-means halves inside one are far apart in their own pooled
//! standard deviations, and fade and are pruned when starved.
//!
//! The search stays exact with the neighborhoods of the balls, bounded by
//! each ellipse's largest semi-axis: a point within an ellipse's reach lies
//! within `reach · √λmax` of its center, its bounding radius ρ, so two
//! ellipses that both reach a point have centers less than the sum of
//! their ρ apart. Once a walk finds one ellipse that reaches the point,
//! its neighbors hold every other.

use crate::cov::Cov;
use crate::{Event, EventKind, Log, Point, dist2};

/// What the ellipses are tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipseParams {
    /// A point joins an ellipse within this many standard deviations, in
    /// Mahalanobis distance.
    pub reach: f32,
    /// The standard deviation a new ellipse starts with, round.
    pub s_init: f32,
    /// Added to every ellipse's variance in every direction, so one of a few
    /// points that lie on a line still has an area.
    pub s_min: f32,
    /// Two ellipses that reach each other merge when the merged ellipse's
    /// area is at most this many times their areas summed. Two pieces of
    /// one cluster about fill the merged one; two clusters side by side
    /// leave a gap it has to cover.
    pub merge: f32,
    /// Every weight is multiplied by `1 − decay` per point.
    pub decay: f32,
    /// The share of all the ellipses' weight one needs to count as a cluster.
    pub share: f32,
    /// An ellipse whose weight falls below this, once past its youth, is pruned.
    pub prune: f32,
    /// An ellipse splits when its halves are farther apart than this many of
    /// their pooled standard deviations. One Gaussian's 2-means halves sit
    /// about 2.7 apart, however long and thin, since Mahalanobis distance
    /// doesn't change when a cluster is stretched or turned.
    pub split: f32,
}

impl Default for EllipseParams {
    /// Reach 3.5 standard deviations (about 1 in 450 points of a Gaussian
    /// lies beyond), 0.02 to start and 0.012 added, merging at no more than
    /// the summed area, the balls' forgetting, share and pruning, and
    /// splitting at 4 pooled standard deviations.
    fn default() -> Self {
        EllipseParams {
            reach: 3.5,
            s_init: 0.02,
            s_min: 0.012,
            merge: 1.0,
            decay: 0.002,
            share: 0.02,
            prune: 1.0,
            split: 4.0,
        }
    }
}

/// The least share of the way an ellipse moves towards a point it takes.
pub const MIN_RATE: f32 = 0.01;
/// The weight an ellipse needs before its halves are trusted to split it.
const SPLIT_SUPPORT: f32 = 10.0;
/// Points a new ellipse is spared from pruning.
const YOUTH: u64 = 1000;
/// Points between two pruning sweeps.
const PRUNE_EVERY: u64 = 100;
/// How far an ellipse may move, or its bounding radius change, from where
/// it was last linked before its links are refreshed.
const SLACK: f32 = 0.01;
/// The margin on a neighborhood, in [`SLACK`]s, as for the balls.
const MARGIN: f32 = 8.0;
/// Nearest ellipses every ellipse links to besides its neighborhood.
const LINKS: usize = 3;

/// A decayed mean and covariance, updated one point at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moments {
    /// The mean.
    pub mean: Point,
    /// The covariance, without the floor.
    pub cov: Cov,
    /// The decayed number of points taken; 0 for none yet.
    pub weight: f32,
}

impl Moments {
    /// Takes `x` in: the mean moves by a share `rate` of the way, at least
    /// [`MIN_RATE`], and the covariance follows,
    /// Σ ← (1 − r)(Σ + r δδᵀ). With no points yet, `x` seeds it with `prior`.
    fn take(&mut self, x: Point, prior: Cov) {
        if self.weight == 0.0 {
            *self = Moments {
                mean: x,
                cov: prior,
                weight: 1.0,
            };
            return;
        }
        self.weight += 1.0;
        let rate = (1.0 / self.weight).max(MIN_RATE);
        let d = [x[0] - self.mean[0], x[1] - self.mean[1]];
        self.mean = [self.mean[0] + rate * d[0], self.mean[1] + rate * d[1]];
        self.cov = self.cov.add(Cov::outer(d), rate).scale(1.0 - rate);
    }
}

/// The moments of `a` and `b` together, as if one had taken both's points.
fn pooled(a: &Moments, b: &Moments) -> Moments {
    let w = a.weight + b.weight;
    let mean = [
        (a.mean[0] * a.weight + b.mean[0] * b.weight) / w,
        (a.mean[1] * a.weight + b.mean[1] * b.weight) / w,
    ];
    let spread = |m: &Moments| {
        let d = [m.mean[0] - mean[0], m.mean[1] - mean[1]];
        m.cov.add(Cov::outer(d), 1.0).scale(m.weight / w)
    };
    Moments {
        mean,
        cov: spread(a).add(spread(b), 1.0),
        weight: w,
    }
}

/// One cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct Ellipse {
    /// A stable id, for coloring the same cluster the same way over time.
    pub id: u32,
    /// Its mean, covariance and weight.
    pub moments: Moments,
    /// When it was spawned, in points of the stream.
    pub born: u64,
    /// Where it was, and its bounding radius, when last linked.
    anchor: (Point, f32),
    /// A two-means of its own points, to notice when they come apart.
    halves: [Moments; 2],
}

impl Ellipse {
    /// The center.
    pub fn center(&self) -> Point {
        self.moments.mean
    }

    /// The decayed number of points it has taken.
    pub fn weight(&self) -> f32 {
        self.moments.weight
    }

    /// The covariance with the floor added: what distances are measured with.
    pub fn shape(&self, p: &EllipseParams) -> Cov {
        floored(self.moments.cov, p)
    }

    /// The squared Mahalanobis distance of `x`.
    pub fn d2(&self, x: Point, p: &EllipseParams) -> f32 {
        let c = self.center();
        self.shape(p).mahalanobis2([x[0] - c[0], x[1] - c[1]])
    }

    /// How well `x` fits, lower better: d² + ln |Σ|, the negative Gaussian
    /// log-likelihood up to constants.
    pub fn score(&self, x: Point, p: &EllipseParams) -> f32 {
        self.d2(x, p) + self.shape(p).det().ln()
    }

    /// Whether `x` lies within reach.
    pub fn reaches(&self, x: Point, p: &EllipseParams) -> bool {
        self.d2(x, p) <= p.reach * p.reach
    }

    /// The radius of a circle around the center holding everything within
    /// reach: reach times the largest semi-axis.
    pub fn bound(&self, p: &EllipseParams) -> f32 {
        p.reach * self.shape(p).largest().sqrt()
    }

    /// The ellipse's two halves.
    pub fn halves(&self) -> &[Moments; 2] {
        &self.halves
    }
}

/// `cov` with `s_min²` added in every direction.
fn floored(cov: Cov, p: &EllipseParams) -> Cov {
    cov.add(Cov::round(p.s_min), 1.0)
}

/// How the last point was placed, for showing a single step.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The point.
    pub point: Point,
    /// Ids of the ellipses the greedy walk went through, in order.
    pub path: Vec<u32>,
    /// Every ellipse whose distance was computed, with the Mahalanobis distance.
    pub checked: Vec<(u32, f32)>,
    /// Whether no ellipse near the walk's end reached it and all were scanned.
    pub full_scan: bool,
    /// The id of the ellipse the point joined or spawned.
    pub ellipse: u32,
    /// Whether it spawned that ellipse.
    pub spawned: bool,
}

/// The ellipses and their bookkeeping.
#[derive(Clone, Debug)]
pub struct Ellipses {
    /// What they are tuned by.
    pub params: EllipseParams,
    /// The ellipses, in no particular order.
    ellipses: Vec<Ellipse>,
    /// Per ellipse, the indices of the ellipses within its neighborhood.
    neighbors: Vec<Vec<usize>>,
    /// The id the next ellipse gets.
    next_id: u32,
    /// The ellipse the last point joined, where the next walk starts.
    last: Option<usize>,
    /// Points taken so far.
    t: u64,
    /// Recent spawns, merges, prunes and splits, and their counts.
    pub log: Log,
    /// How the last point was placed.
    pub trace: Option<Trace>,
    /// Distances computed to place points.
    pub search_checks: u64,
    /// Distances computed to keep the neighbor lists.
    pub upkeep_checks: u64,
}

impl Ellipses {
    /// No ellipses yet.
    pub fn new(params: EllipseParams) -> Ellipses {
        Ellipses {
            params,
            ellipses: Vec::new(),
            neighbors: Vec::new(),
            next_id: 0,
            last: None,
            t: 0,
            log: Log::default(),
            trace: None,
            search_checks: 0,
            upkeep_checks: 0,
        }
    }

    /// The ellipses.
    pub fn ellipses(&self) -> &[Ellipse] {
        &self.ellipses
    }

    /// Points taken so far.
    pub fn t(&self) -> u64 {
        self.t
    }

    /// The weight an ellipse needs now to count as a cluster.
    pub fn floor(&self) -> f32 {
        self.params.share * self.ellipses.iter().map(Ellipse::weight).sum::<f32>()
    }

    /// The ellipses with enough weight to count as clusters.
    pub fn clusters(&self) -> impl Iterator<Item = &Ellipse> {
        let floor = self.floor();
        self.ellipses.iter().filter(move |e| e.weight() >= floor)
    }

    /// The id of the counted ellipse `x` fits best, by plain scan; `None`
    /// without any.
    pub fn assign(&self, x: Point) -> Option<u32> {
        let p = self.params;
        self.clusters()
            .min_by(|a, b| a.score(x, &p).total_cmp(&b.score(x, &p)))
            .map(|e| e.id)
    }

    /// Takes one point: joins the best-fitting ellipse within reach or
    /// spawns one, then merges, splits, decays and prunes. Returns the id of
    /// the ellipse it went to.
    pub fn insert(&mut self, x: Point) -> u32 {
        self.t += 1;
        let (target, trace) = self.search(x);
        let (joined, id) = match target {
            Some(i) => {
                let joined = self.ellipses[i].id;
                self.join(i, x);
                self.last = Some(i);
                self.merge_into(i);
                if let Some(l) = self.last {
                    self.split(l);
                }
                (joined, self.last.map_or(joined, |l| self.ellipses[l].id))
            }
            None => {
                let id = self.spawn(x);
                (id, id)
            }
        };
        self.trace = Some(Trace {
            ellipse: joined,
            spawned: target.is_none(),
            ..trace
        });
        let keep = 1.0 - self.params.decay;
        for e in &mut self.ellipses {
            e.moments.weight *= keep;
            for h in &mut e.halves {
                h.weight *= keep;
            }
        }
        if self.t.is_multiple_of(PRUNE_EVERY) {
            self.prune();
        }
        id
    }

    /// The ellipse `x` should join, if any reaches it, and how it was found.
    fn search(&mut self, x: Point) -> (Option<usize>, Trace) {
        let p = self.params;
        let mut trace = Trace {
            point: x,
            path: Vec::new(),
            checked: Vec::new(),
            full_scan: false,
            ellipse: 0,
            spawned: false,
        };
        let n = self.ellipses.len();
        let Some(start) = self.last.filter(|&i| i < n).or((n > 0).then_some(0)) else {
            return (None, trace);
        };
        // Each ellipse's fit is computed at most once per point: (d², score).
        let mut memo: Vec<Option<(f32, f32)>> = vec![None; n];
        let mut fit = |i: usize, trace: &mut Trace| {
            *memo[i].get_or_insert_with(|| {
                self.search_checks += 1;
                let e = &self.ellipses[i];
                let d2 = e.d2(x, &p);
                trace.checked.push((e.id, d2.sqrt()));
                (d2, d2 + e.shape(&p).det().ln())
            })
        };
        let reaches = |f: (f32, f32)| f.0 <= p.reach * p.reach;
        // Walk downhill in score.
        let mut cur = start;
        let mut f_cur = fit(cur, &mut trace);
        trace.path.push(self.ellipses[cur].id);
        loop {
            let mut best = (cur, f_cur);
            for &j in &self.neighbors[cur] {
                let f = fit(j, &mut trace);
                if f.1 < best.1.1 {
                    best = (j, f);
                }
            }
            if best.0 == cur {
                break;
            }
            (cur, f_cur) = best;
            trace.path.push(self.ellipses[cur].id);
        }
        // An ellipse that reaches x: the walk's end, or one of its neighbors.
        let anchor = if reaches(f_cur) {
            Some(cur)
        } else {
            self.neighbors[cur]
                .iter()
                .copied()
                .find(|&j| reaches(fit(j, &mut trace)))
        };
        let candidates: Vec<usize> = match anchor {
            // Every ellipse that reaches x is the anchor or in its neighborhood.
            Some(a) => std::iter::once(a)
                .chain(self.neighbors[a].iter().copied())
                .collect(),
            None => {
                trace.full_scan = true;
                (0..n).collect()
            }
        };
        let best = candidates
            .into_iter()
            .map(|i| (i, fit(i, &mut trace)))
            .filter(|&(_, f)| reaches(f))
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map(|(i, _)| i);
        (best, trace)
    }

    /// Moves ellipse `i` to take in `x`, and its nearer half.
    fn join(&mut self, i: usize, x: Point) {
        let p = self.params;
        let prior = Cov::round(p.s_init);
        let e = &mut self.ellipses[i];
        e.moments.take(x, prior);
        let [h0, h1] = &mut e.halves;
        let half =
            if h0.weight == 0.0 || (h1.weight > 0.0 && dist2(h0.mean, x) <= dist2(h1.mean, x)) {
                h0
            } else {
                h1
            };
        // A half starts as a point: its spread is its own points'.
        half.take(x, Cov::default());
        let moved = dist2(e.anchor.0, e.center()).sqrt() + (e.bound(&p) - e.anchor.1).abs();
        if moved > SLACK {
            self.link(i);
        }
    }

    /// A new ellipse at `x`; returns its id.
    fn spawn(&mut self, x: Point) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut moments = Moments::default();
        moments.take(x, Cov::round(self.params.s_init));
        self.ellipses.push(Ellipse {
            id,
            moments,
            born: self.t,
            anchor: (x, 0.0),
            halves: [Moments::default(); 2],
        });
        self.neighbors.push(Vec::new());
        self.last = Some(self.ellipses.len() - 1);
        self.log.record(Event {
            kind: EventKind::Spawn,
            at: x,
            t: self.t,
        });
        self.link(self.ellipses.len() - 1);
        id
    }

    /// Merges a neighbor of ellipse `i` into the heavier of the two when they
    /// reach each other's centers and the merged ellipse is no bigger than
    /// `merge` times their areas together.
    fn merge_into(&mut self, i: usize) {
        let p = self.params;
        let area = |c: Cov| floored(c, &p).det().max(0.0).sqrt();
        for j in self.neighbors[i].clone() {
            if j >= self.ellipses.len() || j == i {
                continue;
            }
            let (a, b) = (&self.ellipses[i], &self.ellipses[j]);
            if !a.reaches(b.center(), &p) && !b.reaches(a.center(), &p) {
                continue;
            }
            let both = pooled(&a.moments, &b.moments);
            if area(both.cov) > p.merge * (area(a.moments.cov) + area(b.moments.cov)) {
                continue;
            }
            let (keep, gone) = if a.weight() >= b.weight() {
                (i, j)
            } else {
                (j, i)
            };
            let k = &mut self.ellipses[keep];
            k.moments = both;
            k.halves = [Moments::default(); 2];
            self.log.record(Event {
                kind: EventKind::Merge,
                at: both.mean,
                t: self.t,
            });
            self.remove(gone);
            let i = if keep > gone { keep - 1 } else { keep };
            self.last = Some(i);
            self.link(i);
            break;
        }
    }

    /// Splits ellipse `i` in two when its halves are more than `split` of
    /// their pooled standard deviations apart, each with enough weight to be
    /// trusted. The heavier half keeps the ellipse's id.
    fn split(&mut self, i: usize) {
        let p = self.params;
        let e = &self.ellipses[i];
        let [h0, h1] = e.halves;
        if e.weight() < 2.0 * SPLIT_SUPPORT
            || h0.weight < SPLIT_SUPPORT / 2.0
            || h1.weight < SPLIT_SUPPORT / 2.0
        {
            return;
        }
        let w = h0.weight + h1.weight;
        let within = floored(h0.cov.scale(h0.weight / w).add(h1.cov, h1.weight / w), &p);
        let gap = [h0.mean[0] - h1.mean[0], h0.mean[1] - h1.mean[1]];
        if within.mahalanobis2(gap) <= p.split * p.split {
            return;
        }
        let (heavy, light) = if h0.weight >= h1.weight {
            (h0, h1)
        } else {
            (h1, h0)
        };
        let share = e.weight() / w;
        let at = e.center();
        let born = e.born;
        let old_id = e.id;
        let id = self.next_id;
        self.next_id += 1;
        let make = |h: Moments, id: u32, born: u64| Ellipse {
            id,
            moments: Moments {
                weight: h.weight * share,
                ..h
            },
            born,
            anchor: (h.mean, 0.0),
            halves: [Moments::default(); 2],
        };
        self.ellipses[i] = make(heavy, old_id, born);
        self.ellipses.push(make(light, id, self.t));
        self.neighbors.push(Vec::new());
        self.log.record(Event {
            kind: EventKind::Split,
            at,
            t: self.t,
        });
        self.link(i);
        self.link(self.ellipses.len() - 1);
    }

    /// Removes faded ellipses past their youth.
    fn prune(&mut self) {
        let p = self.params;
        let mut i = 0;
        while i < self.ellipses.len() {
            let e = &self.ellipses[i];
            if e.weight() < p.prune && self.t - e.born > YOUTH {
                self.log.record(Event {
                    kind: EventKind::Prune,
                    at: e.center(),
                    t: self.t,
                });
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Removes ellipse `i`, keeping `last` pointing at the same ellipse where
    /// it survives and dropping it from every neighbor list.
    fn remove(&mut self, i: usize) {
        self.ellipses.remove(i);
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

    /// Brings ellipse `i`'s place in the neighbor lists up to date: its own
    /// list anew (the ellipses whose bounding circles overlap its own, plus
    /// a margin, and its [`LINKS`] nearest), and itself added to every other
    /// list it now belongs in.
    fn link(&mut self, i: usize) {
        let p = self.params;
        let n = self.ellipses.len();
        let own = self.ellipses[i].bound(&p);
        let mut by_distance: Vec<(f32, usize)> = Vec::with_capacity(n);
        for j in 0..n {
            if j == i {
                continue;
            }
            self.upkeep_checks += 1;
            let d = dist2(self.ellipses[i].center(), self.ellipses[j].center()).sqrt();
            let limit = own + self.ellipses[j].bound(&p) + MARGIN * SLACK;
            by_distance.push((d, j));
            if d < limit && !self.neighbors[j].contains(&i) {
                self.neighbors[j].push(i);
            }
        }
        by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.neighbors[i] = by_distance
            .iter()
            .enumerate()
            .filter(|&(rank, &(d, j))| {
                rank < LINKS || d < own + self.ellipses[j].bound(&p) + MARGIN * SLACK
            })
            .map(|(_, &(_, j))| j)
            .collect();
        let e = &mut self.ellipses[i];
        e.anchor = (e.center(), e.bound(&p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{Scenario, Source};

    fn run(scenario: Scenario, n: usize, seed: u64) -> Ellipses {
        let mut src = Source::new(scenario, seed);
        let mut ellipses = Ellipses::new(EllipseParams::default());
        for _ in 0..n {
            ellipses.insert(src.next_point().0);
        }
        ellipses
    }

    #[test]
    fn moments_follow_a_stretched_cloud() {
        let mut rng = crate::rng::Rng::new(1);
        let mut m = Moments::default();
        for _ in 0..20_000 {
            m.take(
                [0.5 + rng.normal() * 0.1, 0.5 + rng.normal() * 0.02],
                Cov::round(0.02),
            );
        }
        let (big, small, angle) = m.cov.axes();
        assert!(
            (big.sqrt() - 0.1).abs() < 0.015 && (small.sqrt() - 0.02).abs() < 0.004,
            "{big} {small}"
        );
        assert!(
            angle.abs() < 0.1 || (angle.abs() - std::f32::consts::PI).abs() < 0.1,
            "{angle}"
        );
    }

    #[test]
    fn long_thin_clusters_make_one_ellipse_each() {
        for seed in 1..=3 {
            let e = run(Scenario::Anisotropic, 15_000, seed);
            assert_eq!(e.clusters().count(), 4, "seed {seed}");
        }
    }

    #[test]
    fn and_each_is_turned_like_its_cluster() {
        let e = run(Scenario::Anisotropic, 15_000, 1);
        for c in e.clusters() {
            let (big, small, _) = c.moments.cov.axes();
            assert!(
                big.sqrt() > 3.0 * small.sqrt(),
                "a long ellipse: {} × {}",
                big.sqrt(),
                small.sqrt()
            );
        }
    }

    #[test]
    fn blobs_and_the_drifting_story_come_out_as_with_balls() {
        assert_eq!(run(Scenario::Blobs, 8000, 1).clusters().count(), 5);
        let e = run(Scenario::Drift, 11_500, 2);
        assert!(
            (e.clusters().count() as i32 - 4).abs() <= 1,
            "{}",
            e.clusters().count()
        );
        assert!(
            e.log.count(EventKind::Split) > 0,
            "the center cluster splits"
        );
    }

    #[test]
    fn the_walk_finds_the_same_ellipse_as_a_full_scan() {
        let mut src = Source::new(Scenario::Drift, 2);
        let mut ellipses = Ellipses::new(EllipseParams::default());
        for _ in 0..6000 {
            let x = src.next_point().0;
            let p = ellipses.params;
            let scan = ellipses
                .ellipses()
                .iter()
                .filter(|e| e.reaches(x, &p))
                .min_by(|a, b| a.score(x, &p).total_cmp(&b.score(x, &p)))
                .map(|e| e.id);
            let mut probe = ellipses.clone();
            probe.insert(x);
            let trace = probe.trace.clone().unwrap();
            match scan {
                Some(id) => assert!(
                    !trace.spawned && trace.ellipse == id,
                    "walk joined {} for best {id}",
                    trace.ellipse
                ),
                None => assert!(trace.spawned),
            }
            ellipses = probe;
        }
    }

    #[test]
    fn a_far_cluster_does_not_merge_but_its_tail_does() {
        let p = EllipseParams::default();
        let blob = |c: f32, w: f32| Moments {
            mean: [c, 0.5],
            cov: Cov::round(0.03),
            weight: w,
        };
        let area = |c: Cov| floored(c, &p).det().sqrt();
        let (a, b) = (blob(0.3, 100.0), blob(0.7, 100.0));
        assert!(area(pooled(&a, &b).cov) > area(a.cov) + area(b.cov));
        let tail = Moments {
            mean: [0.38, 0.5],
            cov: Cov::round(0.01),
            weight: 2.0,
        };
        assert!(area(pooled(&a, &tail).cov) <= area(a.cov) + area(tail.cov));
    }
}
