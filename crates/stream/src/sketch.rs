//! A one-pass sketch refined into clusters. Online facility location
//! (Meyerson, 2001; Shindler, Wong and Meyerson, 2011) keeps a few dozen
//! weighted centers, its facilities: a point at squared distance d² from
//! the nearest opens a new facility with probability d² / f, and otherwise
//! adds its weight to the nearest. When there are too many facilities, the
//! cost f of opening one grows and the facilities are run through the same
//! rule again, which merges them.
//!
//! Every few hundred points the sketch is refined into clusters by a ball
//! k-means (Ostrovsky et al., 2006) with a price on every cluster: a
//! facility farther than √λ from every center becomes a center (as in
//! DP-means, Kulis and Jordan, 2012), centers move to the weighted mean of
//! the facilities inside their ball, half the way to the next center, and
//! centers closer than half √λ merge. Clusters holding less than a small
//! share of the points are dissolved into the rest, so the number of
//! clusters comes from λ and the data, not from a given k.

use crate::rng::Rng;
use crate::{Event, EventKind, Log, Point, dist2};

/// What the sketch is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchParams {
    /// λ: what one more cluster costs, in summed squared distance.
    pub cluster_cost: f32,
    /// Facilities kept before the cost of opening one grows.
    pub cap: usize,
    /// The factor that cost grows by when there are too many.
    pub growth: f32,
    /// Every weight is multiplied by `1 − decay` per point.
    pub decay: f32,
    /// The share of the sketch's weight a cluster needs to count; lighter
    /// ones are dissolved into their neighbors at each refinement.
    pub share: f32,
    /// Points between two refinements.
    pub every: u64,
}

impl Default for SketchParams {
    /// λ 0.02 (a cluster pays for itself once it saves 0.14 in distance per
    /// point), 60 facilities growing their cost by 1.5, the same forgetting
    /// as the balls, clusters of at least 2 % of the points and a
    /// refinement every 200 points.
    fn default() -> Self {
        SketchParams {
            cluster_cost: 0.02,
            cap: 60,
            growth: 1.5,
            decay: 0.002,
            share: 0.02,
            every: 200,
        }
    }
}

/// The least share of the way a facility moves towards a point it takes.
/// The mean of everything it ever took (a rate of one over its weight)
/// would lag far behind a cluster that moves; this floor keeps it close.
pub const MIN_RATE: f32 = 0.01;

/// The cost of opening a facility at the start; it grows from there.
const F_START: f32 = 1e-5;
/// Facilities lighter than this are forgotten.
const FORGOTTEN: f32 = 0.01;
/// Rounds of the refinement at most.
const ROUNDS: usize = 10;

/// A weighted center of the sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facility {
    /// Where it is.
    pub center: Point,
    /// The decayed number of points it stands for.
    pub weight: f32,
}

/// A refined cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// A stable id, carried over between refinements where a cluster stays.
    pub id: u32,
    /// The weighted mean of the facilities in its ball.
    pub center: Point,
    /// The weight of all facilities assigned to it.
    pub weight: f32,
    /// The weighted mean squared distance of its facilities from the center.
    pub m2: f32,
}

/// How the last point was placed, for showing a single step.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The point.
    pub point: Point,
    /// The nearest facility before the point, if there was one.
    pub nearest: Option<Point>,
    /// The squared distance to it; infinite without facilities.
    pub d2: f32,
    /// The cost of opening a facility then.
    pub f: f32,
    /// The chance of opening one, min(d² / f, 1).
    pub p: f32,
    /// The draw it was decided by.
    pub draw: f32,
    /// Whether the point opened a facility.
    pub opened: bool,
    /// Whether the sketch had to be collapsed after it.
    pub collapsed: bool,
}

/// The sketch, its clusters and their bookkeeping.
#[derive(Clone, Debug)]
pub struct Sketch {
    /// What it is tuned by.
    pub params: SketchParams,
    /// The cost of opening a facility now.
    f: f32,
    /// The facilities.
    facilities: Vec<Facility>,
    /// The clusters of the last refinement.
    clusters: Vec<Cluster>,
    /// The id the next new cluster gets.
    next_id: u32,
    /// Its random numbers.
    rng: Rng,
    /// Points taken so far.
    t: u64,
    /// Recent spawns, merges and prunes of clusters.
    pub log: Log,
    /// How the last point was placed.
    pub trace: Option<Trace>,
    /// Distances computed to place points.
    pub search_checks: u64,
}

impl Sketch {
    /// An empty sketch with its own random numbers from `seed`.
    pub fn new(params: SketchParams, seed: u64) -> Sketch {
        Sketch {
            params,
            f: F_START,
            facilities: Vec::new(),
            clusters: Vec::new(),
            next_id: 0,
            rng: Rng::new(seed),
            t: 0,
            log: Log::default(),
            trace: None,
            search_checks: 0,
        }
    }

    /// The facilities.
    pub fn facilities(&self) -> &[Facility] {
        &self.facilities
    }

    /// Every cluster of the last refinement, light ones included.
    pub fn all_clusters(&self) -> &[Cluster] {
        &self.clusters
    }

    /// The clusters. Refinement dissolves the light ones, so they all count.
    pub fn clusters(&self) -> impl Iterator<Item = &Cluster> {
        self.clusters.iter()
    }

    /// The cost of opening a facility now.
    pub fn facility_cost(&self) -> f32 {
        self.f
    }

    /// The id of the counted cluster nearest `x`; `None` without any.
    pub fn assign(&self, x: Point) -> Option<u32> {
        self.clusters()
            .min_by(|a, b| dist2(a.center, x).total_cmp(&dist2(b.center, x)))
            .map(|c| c.id)
    }

    /// Takes one point into the sketch, collapsing it when it has grown too
    /// big, and refines the clusters every so many points.
    pub fn insert(&mut self, x: Point) {
        self.t += 1;
        let keep = 1.0 - self.params.decay;
        for fac in &mut self.facilities {
            fac.weight *= keep;
        }
        self.facilities.retain(|fac| fac.weight >= FORGOTTEN);
        // Forgetting lets the cost come down again as old facilities fade.
        self.f = (self.f * keep).max(F_START);
        self.search_checks += self.facilities.len() as u64;
        let nearest = nearest(&self.facilities, x);
        let near_point = nearest.map(|(i, _)| self.facilities[i].center);
        let d2 = nearest.map_or(f32::INFINITY, |(_, d)| d);
        let p = (d2 / self.f).min(1.0);
        let draw = self.rng.next_f32();
        let opened = draw < p;
        match nearest {
            Some((i, _)) if !opened => {
                let fac = &mut self.facilities[i];
                fac.weight += 1.0;
                let rate = (1.0 / fac.weight).max(MIN_RATE);
                fac.center = [
                    fac.center[0] + rate * (x[0] - fac.center[0]),
                    fac.center[1] + rate * (x[1] - fac.center[1]),
                ];
            }
            _ => self.facilities.push(Facility {
                center: x,
                weight: 1.0,
            }),
        }
        let collapsed = self.facilities.len() > self.params.cap;
        while self.facilities.len() > self.params.cap {
            self.collapse();
        }
        self.trace = Some(Trace {
            point: x,
            nearest: near_point,
            d2,
            f: self.f,
            p,
            draw,
            opened,
            collapsed,
        });
        if self.t.is_multiple_of(self.params.every) {
            self.refine();
        }
    }

    /// Raises the cost of a facility and runs the facilities through the
    /// opening rule again, in a random order, each with its weight.
    fn collapse(&mut self) {
        self.f *= self.params.growth;
        let mut old = std::mem::take(&mut self.facilities);
        for i in (1..old.len()).rev() {
            let j = self.rng.below(i + 1);
            old.swap(i, j);
        }
        for fac in old {
            let near = nearest(&self.facilities, fac.center);
            let p = near.map_or(1.0, |(_, d)| (fac.weight * d / self.f).min(1.0));
            match near {
                Some((i, _)) if self.rng.next_f32() >= p => {
                    let into = &mut self.facilities[i];
                    let w = into.weight + fac.weight;
                    into.center = [
                        (into.center[0] * into.weight + fac.center[0] * fac.weight) / w,
                        (into.center[1] * into.weight + fac.center[1] * fac.weight) / w,
                    ];
                    into.weight = w;
                }
                _ => self.facilities.push(fac),
            }
        }
    }

    /// Refines the facilities into clusters, starting from the last
    /// refinement's, and records which clusters appeared, merged or went.
    pub fn refine(&mut self) {
        let lambda = self.params.cluster_cost;
        let mut facs = self.facilities.clone();
        facs.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        let mut centers: Vec<Point> = self.clusters.iter().map(|c| c.center).collect();
        let mut assigned: Vec<usize> = Vec::new();
        for _ in 0..ROUNDS {
            // Assign, opening a center for any facility no center reaches.
            assigned.clear();
            for fac in &facs {
                match nearest(&centers, fac.center) {
                    Some((i, d)) if d <= lambda => assigned.push(i),
                    _ => {
                        centers.push(fac.center);
                        assigned.push(centers.len() - 1);
                    }
                }
            }
            let next = ball_means(&facs, &assigned, &centers);
            let merged = merge_close(next, lambda);
            let moved = merged.len() != centers.len()
                || merged
                    .iter()
                    .zip(&centers)
                    .any(|(a, b)| dist2(a.0, *b) > 1e-10);
            centers = merged.into_iter().map(|(c, _)| c).collect();
            if !moved {
                break;
            }
        }
        // Dissolve clusters too light to count, lightest first, into the rest.
        let total: f32 = facs.iter().map(|f| f.weight).sum();
        let (weight, spread) = loop {
            let mut weight = vec![0.0f32; centers.len()];
            let mut spread = vec![0.0f32; centers.len()];
            for fac in &facs {
                if let Some((i, d)) = nearest(&centers, fac.center) {
                    weight[i] += fac.weight;
                    spread[i] += fac.weight * d;
                }
            }
            let lightest = (0..centers.len()).min_by(|&a, &b| weight[a].total_cmp(&weight[b]));
            match lightest {
                Some(i) if centers.len() > 1 && weight[i] < self.params.share * total => {
                    centers.remove(i);
                }
                _ => break (weight, spread),
            }
        };
        let fresh: Vec<(Point, f32, f32)> = (0..centers.len())
            .filter(|&i| weight[i] > 0.0)
            .map(|i| (centers[i], weight[i], spread[i] / weight[i]))
            .collect();
        self.relabel(fresh);
    }

    /// Gives the refined clusters ids: each keeps the id of the nearest old
    /// cluster within √λ that no nearer one has taken, or gets a new one.
    fn relabel(&mut self, fresh: Vec<(Point, f32, f32)>) {
        let reach = self.params.cluster_cost;
        let old = std::mem::take(&mut self.clusters);
        let mut taken = vec![false; old.len()];
        let mut owner_of_old: Vec<Option<usize>> = vec![None; old.len()];
        let mut clusters = Vec::with_capacity(fresh.len());
        for (k, &(center, weight, m2)) in fresh.iter().enumerate() {
            let pick = old
                .iter()
                .enumerate()
                .filter(|(i, o)| !taken[*i] && dist2(o.center, center) <= reach)
                .min_by(|a, b| dist2(a.1.center, center).total_cmp(&dist2(b.1.center, center)));
            let id = match pick {
                Some((i, o)) => {
                    taken[i] = true;
                    owner_of_old[i] = Some(k);
                    o.id
                }
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.log.record(Event {
                        kind: EventKind::Spawn,
                        at: center,
                        t: self.t,
                    });
                    id
                }
            };
            clusters.push(Cluster {
                id,
                center,
                weight,
                m2,
            });
        }
        for (i, o) in old.iter().enumerate() {
            if owner_of_old[i].is_some() {
                continue;
            }
            // Gone: merged if a new cluster now covers it, pruned otherwise.
            let covered = fresh.iter().any(|f| dist2(f.0, o.center) <= reach);
            let kind = if covered {
                EventKind::Merge
            } else {
                EventKind::Prune
            };
            self.log.record(Event {
                kind,
                at: o.center,
                t: self.t,
            });
        }
        self.clusters = clusters;
    }
}

/// The index of the point nearest `x` and its squared distance.
fn nearest<P: Copy + Into<Point>>(points: &[P], x: Point) -> Option<(usize, f32)>
where
    Point: From<P>,
{
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| (i, dist2(Point::from(p), x)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

impl From<Facility> for Point {
    fn from(f: Facility) -> Point {
        f.center
    }
}

/// Each center moved to the weighted mean of its facilities that lie in
/// its ball, closer to it than half the distance to the next center
/// (all of them when none do); centers without facilities are dropped.
/// Returns (center, weight) pairs.
fn ball_means(facs: &[Facility], assigned: &[usize], centers: &[Point]) -> Vec<(Point, f32)> {
    let half: Vec<f32> = centers
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            centers
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, &o)| dist2(c, o) / 4.0)
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    let mut sum = vec![([0.0f32; 2], 0.0f32, [0.0f32; 2], 0.0f32); centers.len()];
    for (fac, &i) in facs.iter().zip(assigned) {
        let s = &mut sum[i];
        // Everything assigned, and the part inside the ball.
        s.0[0] += fac.weight * fac.center[0];
        s.0[1] += fac.weight * fac.center[1];
        s.1 += fac.weight;
        if dist2(fac.center, centers[i]) <= half[i] {
            s.2[0] += fac.weight * fac.center[0];
            s.2[1] += fac.weight * fac.center[1];
            s.3 += fac.weight;
        }
    }
    sum.into_iter()
        .filter(|s| s.1 > 0.0)
        .map(|(all, w, ball, wb)| {
            let (c, n) = if wb > 0.0 { (ball, wb) } else { (all, w) };
            ([c[0] / n, c[1] / n], w)
        })
        .collect()
}

/// Merges pairs of clusters whose centers are closer than half the
/// distance √λ at which a facility opens a cluster of its own: two centers
/// that close describe the same cluster.
fn merge_close(mut clusters: Vec<(Point, f32)>, lambda: f32) -> Vec<(Point, f32)> {
    loop {
        let mut best: Option<(usize, usize, f32)> = None;
        for i in 0..clusters.len() {
            for j in i + 1..clusters.len() {
                let d = dist2(clusters[i].0, clusters[j].0);
                if d < lambda / 4.0 && best.is_none_or(|(_, _, c)| d < c) {
                    best = Some((i, j, d));
                }
            }
        }
        let Some((i, j, _)) = best else {
            return clusters;
        };
        let (a, b) = (clusters[i], clusters[j]);
        let w = a.1 + b.1;
        clusters[i] = (
            [
                (a.0[0] * a.1 + b.0[0] * b.1) / w,
                (a.0[1] * a.1 + b.0[1] * b.1) / w,
            ],
            w,
        );
        clusters.remove(j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{Scenario, Source};

    fn run(scenario: Scenario, n: usize) -> Sketch {
        let mut src = Source::new(scenario, 1);
        let mut sketch = Sketch::new(SketchParams::default(), 2);
        for _ in 0..n {
            sketch.insert(src.next_point().0);
        }
        sketch
    }

    #[test]
    fn five_blobs_make_five_clusters() {
        let sketch = run(Scenario::Blobs, 6000);
        assert_eq!(sketch.clusters().count(), 5);
        assert!(sketch.facilities().len() <= SketchParams::default().cap);
    }

    #[test]
    fn the_cost_of_a_facility_grows_to_keep_the_sketch_small() {
        let sketch = run(Scenario::Rings, 4000);
        assert!(sketch.facility_cost() > F_START * 10.0);
        assert!(sketch.facilities().len() <= 60);
    }

    #[test]
    fn close_centers_merge_and_far_ones_do_not() {
        let near = merge_close(vec![([0.5, 0.5], 10.0), ([0.52, 0.5], 10.0)], 0.02);
        assert_eq!(near.len(), 1);
        assert!((near[0].0[0] - 0.51).abs() < 1e-6 && near[0].1 == 20.0);
        let far = merge_close(vec![([0.2, 0.5], 10.0), ([0.8, 0.5], 10.0)], 0.02);
        assert_eq!(far.len(), 2);
    }

    #[test]
    fn clusters_keep_their_ids_across_refinements() {
        let mut src = Source::new(Scenario::Blobs, 1);
        let mut sketch = Sketch::new(SketchParams::default(), 2);
        for _ in 0..4000 {
            sketch.insert(src.next_point().0);
        }
        let ids = |s: &Sketch| {
            let mut v: Vec<u32> = s.clusters().map(|c| c.id).collect();
            v.sort();
            v
        };
        let before = ids(&sketch);
        for _ in 0..2000 {
            sketch.insert(src.next_point().0);
        }
        assert_eq!(ids(&sketch), before);
    }
}
