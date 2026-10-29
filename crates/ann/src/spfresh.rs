//! An inverted file that grows online, after SPFresh (Xu et al., SOSP
//! 2023): the points sit in postings, each around a centroid, a point
//! joins the posting with the nearest centroid, and a query scans the
//! postings of its `nprobe` nearest centroids. A posting that grows past
//! its limit is split in two by 2-means, and the split's LIRE rebalancing
//! keeps every point in the posting nearest it:
//!
//! * a point of the split posting may belong elsewhere only if the old
//!   centroid was at least as near as its new one (SPFresh's Eq. 1);
//! * a point of a nearby posting may belong in one of the halves only if
//!   a new centroid is nearer it than the old one was (Eq. 2), and than
//!   its own centroid;
//!
//! and each candidate looks up its nearest posting and moves if that is
//! another. A posting left nearly empty is merged into its nearest. The
//! search over centroids is a scan here, where SPFresh keeps them in a
//! graph of their own; at a few hundred postings the scan is the cheaper.

use stream::rng::Rng;

use crate::{Found, Nearest, Tally, dist2, row};

/// What the postings are tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpfreshParams {
    /// Points a posting holds before it splits.
    pub limit: usize,
    /// Postings near a split whose points are checked for moving into it.
    pub reach: usize,
    /// Points a posting needs to be kept; fewer and it is merged away.
    pub least: usize,
    /// Whether a split checks points for moving at all (LIRE).
    pub rebalance: bool,
}

impl Default for SpfreshParams {
    /// A limit of a few hundred, and 8 nearby postings: SPFresh checks 64
    /// among millions, where here 64 would be half of them all, for no
    /// more points moved than 8 find.
    fn default() -> SpfreshParams {
        SpfreshParams {
            limit: 256,
            reach: 8,
            least: 10,
            rebalance: true,
        }
    }
}

/// One posting: its points and their sum, the centroid their mean.
#[derive(Clone, Debug)]
struct Posting {
    /// The mean of its points.
    center: Vec<f32>,
    /// The sum of its points.
    sum: Vec<f32>,
    /// Its points' ids.
    members: Vec<u32>,
}

impl Posting {
    /// An empty posting around `center`.
    fn new(center: Vec<f32>) -> Posting {
        let dim = center.len();
        Posting {
            center,
            sum: vec![0.0; dim],
            members: Vec::new(),
        }
    }

    /// Adds point `x` with id `id`, moving the centroid to the new mean.
    fn add(&mut self, id: u32, x: &[f32]) {
        self.members.push(id);
        self.sum.iter_mut().zip(x).for_each(|(s, v)| *s += v);
        self.recenter();
    }

    /// Takes point `x` with id `id` out.
    fn remove(&mut self, id: u32, x: &[f32]) {
        if let Some(at) = self.members.iter().position(|&m| m == id) {
            self.members.swap_remove(at);
            self.sum.iter_mut().zip(x).for_each(|(s, v)| *s -= v);
            self.recenter();
        }
    }

    /// The centroid from the sum; left where it is when empty.
    fn recenter(&mut self) {
        let n = self.members.len() as f32;
        if n > 0.0 {
            for (c, s) in self.center.iter_mut().zip(&self.sum) {
                *c = s / n;
            }
        }
    }
}

/// The postings, grown point by point.
#[derive(Clone, Debug)]
pub struct Spfresh {
    /// How they are tuned.
    pub params: SpfreshParams,
    /// Values per vector.
    dim: usize,
    /// The postings.
    postings: Vec<Posting>,
    /// The posting each point is in, by id.
    of: Vec<u32>,
    /// Random numbers for the 2-means.
    rng: Rng,
    /// Distances computed to place points.
    pub place_checks: u64,
    /// Distances computed to split postings.
    pub split_checks: u64,
    /// Distances computed to check and move points after splits.
    pub reassign_checks: u64,
    /// Postings split.
    pub splits: u64,
    /// Postings merged away.
    pub merges: u64,
    /// Points checked for moving after splits.
    pub checked: u64,
    /// Points moved.
    pub moved: u64,
}

/// Lloyd's rounds of the 2-means that splits a posting.
const ROUNDS: usize = 8;
/// Splits one insertion may set off, so a cascade stays bounded.
const CASCADE: usize = 16;

impl Spfresh {
    /// No postings yet, for `dim`-long vectors.
    pub fn new(params: SpfreshParams, dim: usize, seed: u64) -> Spfresh {
        Spfresh {
            params,
            dim,
            postings: Vec::new(),
            of: Vec::new(),
            rng: Rng::new(seed),
            place_checks: 0,
            split_checks: 0,
            reassign_checks: 0,
            splits: 0,
            merges: 0,
            checked: 0,
            moved: 0,
        }
    }

    /// Postings.
    pub fn postings(&self) -> usize {
        self.postings.len()
    }

    /// Points held.
    pub fn len(&self) -> usize {
        self.of.len()
    }

    /// Whether no point is held.
    pub fn is_empty(&self) -> bool {
        self.of.is_empty()
    }

    /// The posting of each point, by id.
    pub fn assignments(&self) -> &[u32] {
        &self.of
    }

    /// The nearest posting to `x` and its distance, scanning the centroids.
    fn nearest(&self, x: &[f32], checks: &mut u64) -> (usize, f32) {
        *checks += self.postings.len() as u64;
        self.postings
            .iter()
            .enumerate()
            .map(|(i, p)| (i, dist2(x, &p.center)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a posting")
    }

    /// Inserts point `id`, which must be the next id, splitting what grows too long.
    pub fn insert(&mut self, id: u32, store: &[f32]) {
        assert_eq!(id as usize, self.of.len(), "ids come in order");
        let x = row(store, self.dim, id);
        if self.postings.is_empty() {
            self.postings.push(Posting::new(x.to_vec()));
        }
        let mut checks = 0;
        let (p, _) = self.nearest(x, &mut checks);
        self.place_checks += checks;
        self.of.push(p as u32);
        self.postings[p].add(id, x);
        let mut due = vec![p];
        let mut splits = 0;
        while let Some(p) = due.pop() {
            if splits == CASCADE {
                break;
            }
            if p < self.postings.len() && self.postings[p].members.len() > self.params.limit {
                splits += 1;
                due.extend(self.split(p, store));
            }
        }
    }

    /// Splits posting `p` in two by 2-means and rebalances around it.
    /// Returns the postings that grew past the limit doing so.
    fn split(&mut self, p: usize, store: &[f32]) -> Vec<usize> {
        let old = self.postings[p].center.clone();
        let members = std::mem::take(&mut self.postings[p].members);
        let rows: Vec<&[f32]> = members.iter().map(|&m| row(store, self.dim, m)).collect();
        let (centers, sides) = two_means(&rows, &mut self.rng, &mut self.split_checks);
        if sides.iter().all(|&s| s == sides[0]) {
            // One cluster: nothing to split into; it stays long.
            self.postings[p].members = members;
            return Vec::new();
        }
        self.splits += 1;
        let q = self.postings.len();
        let [a, b] = centers;
        self.postings[p] = Posting::new(a);
        self.postings.push(Posting::new(b));
        for (&m, &side) in members.iter().zip(&sides) {
            let at = if side == 0 { p } else { q };
            self.postings[at].add(m, row(store, self.dim, m));
            self.of[m as usize] = at as u32;
        }
        if !self.params.rebalance {
            return Vec::new();
        }
        let mut checks = 0;
        let mut candidates: Vec<u32> = Vec::new();
        // Eq. 1: points of the halves whose new centroid is no nearer than the old.
        for &half in &[p, q] {
            for &m in &self.postings[half].members {
                let x = row(store, self.dim, m);
                checks += 2;
                if dist2(x, &old) <= dist2(x, &self.postings[half].center) {
                    candidates.push(m);
                }
            }
        }
        // Eq. 2: points of nearby postings a new centroid is nearer than the
        // old one was and than their own.
        let mut near = Nearest::new(self.params.reach);
        for (i, posting) in self.postings.iter().enumerate() {
            if self.params.reach > 0 && i != p && i != q {
                checks += 1;
                near.insert(i as u32, dist2(&old, &posting.center));
            }
        }
        for &(b, _) in &near.items {
            for &m in &self.postings[b as usize].members {
                let x = row(store, self.dim, m);
                checks += 4;
                let new =
                    dist2(x, &self.postings[p].center).min(dist2(x, &self.postings[q].center));
                if new < dist2(x, &old) && new < dist2(x, &self.postings[b as usize].center) {
                    candidates.push(m);
                }
            }
        }
        self.checked += candidates.len() as u64;
        let mut grown = Vec::new();
        for m in candidates {
            let x = row(store, self.dim, m);
            let (to, _) = self.nearest(x, &mut checks);
            let from = self.of[m as usize] as usize;
            if to != from {
                self.postings[from].remove(m, x);
                self.postings[to].add(m, x);
                self.of[m as usize] = to as u32;
                self.moved += 1;
                if self.postings[to].members.len() > self.params.limit {
                    grown.push(to);
                }
            }
        }
        self.reassign_checks += checks;
        self.merge_small(store, &mut grown);
        grown
    }

    /// Merges every posting with fewer than the least points into its
    /// nearest: deletes it and places its points by nearest centroid, which
    /// leaves every other point's nearest posting as it was. Renumbers
    /// `grown` to follow.
    fn merge_small(&mut self, store: &[f32], grown: &mut Vec<usize>) {
        while self.postings.len() > 1 {
            let Some(small) = (0..self.postings.len())
                .find(|&i| self.postings[i].members.len() < self.params.least)
            else {
                break;
            };
            self.merges += 1;
            let gone = self.postings.swap_remove(small);
            let last = self.postings.len();
            // The posting that was last now sits at `small`.
            if small < last {
                for &m in &self.postings[small].members {
                    self.of[m as usize] = small as u32;
                }
            }
            grown.retain(|&g| g != small);
            for g in grown.iter_mut() {
                if *g == last {
                    *g = small;
                }
            }
            let mut checks = 0;
            for m in gone.members {
                let x = row(store, self.dim, m);
                let (to, _) = self.nearest(x, &mut checks);
                self.postings[to].add(m, x);
                self.of[m as usize] = to as u32;
                if self.postings[to].members.len() > self.params.limit && !grown.contains(&to) {
                    grown.push(to);
                }
            }
            self.reassign_checks += checks;
        }
    }

    /// Points not in the posting with the nearest centroid, against
    /// SPFresh's NPA, which LIRE restores for the points it checks.
    pub fn misplaced(&self, store: &[f32]) -> usize {
        let mut checks = 0;
        (0..self.of.len() as u32)
            .filter(|&m| {
                self.nearest(row(store, self.dim, m), &mut checks).0 != self.of[m as usize] as usize
            })
            .count()
    }

    /// The `k` nearest points to `q` found in the postings of the `nprobe`
    /// nearest centroids.
    pub fn search(&self, q: &[f32], k: usize, nprobe: usize, store: &[f32]) -> Found {
        if self.postings.is_empty() {
            return Found::default();
        }
        let mut probes = Nearest::new(nprobe);
        for (i, p) in self.postings.iter().enumerate() {
            probes.insert(i as u32, dist2(q, &p.center));
        }
        let mut tally = Tally::seeing();
        tally.checks = self.postings.len() as u64;
        let mut best = Nearest::new(k);
        for &(p, _) in &probes.items {
            for &m in &self.postings[p as usize].members {
                tally.see(m);
                best.insert(m, dist2(q, row(store, self.dim, m)));
            }
        }
        tally.found(best.ids(k))
    }
}

/// 2-means of `rows`: seeded by a random row and the row farthest from it,
/// then Lloyd's rounds. Returns both centers and each row's side, and adds
/// the distances computed to `checks`.
fn two_means(rows: &[&[f32]], rng: &mut Rng, checks: &mut u64) -> ([Vec<f32>; 2], Vec<u8>) {
    let n = rows.len();
    let dim = rows[0].len();
    let first = rows[rng.below(n)];
    let far = (0..n)
        .max_by(|&a, &b| dist2(rows[a], first).total_cmp(&dist2(rows[b], first)))
        .expect("rows");
    *checks += n as u64;
    let mut centers = [first.to_vec(), rows[far].to_vec()];
    let mut sides = vec![0u8; n];
    for round in 0..ROUNDS {
        let mut changed = false;
        for (s, x) in sides.iter_mut().zip(rows) {
            let side = (dist2(x, &centers[1]) < dist2(x, &centers[0])) as u8;
            changed |= *s != side;
            *s = side;
        }
        *checks += 2 * n as u64;
        if !changed && round > 0 {
            break;
        }
        for (k, center) in centers.iter_mut().enumerate() {
            let mut sum = vec![0.0f32; dim];
            let mut count = 0;
            for (x, _) in rows.iter().zip(&sides).filter(|&(_, &s)| s as usize == k) {
                sum.iter_mut().zip(x.iter()).for_each(|(s, v)| *s += v);
                count += 1;
            }
            if count > 0 {
                *center = sum.into_iter().map(|v| v / count as f32).collect();
            }
        }
    }
    (centers, sides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::points;
    use crate::{brute_force, recall};
    use stream::embed::EmbedSpec;

    #[test]
    fn postings_split_and_stay_within_the_limit() {
        let spec = EmbedSpec::sphere64();
        let n = 8000;
        let store = points(spec, n, 7);
        let mut ivf = Spfresh::new(SpfreshParams::default(), spec.dim, 8);
        for id in 0..n as u32 {
            ivf.insert(id, &store);
        }
        assert!(ivf.splits > 20, "{} splits", ivf.splits);
        assert!(ivf.postings.iter().all(|p| p.members.len() <= 256 + 1));
        let held: usize = ivf.postings.iter().map(|p| p.members.len()).sum();
        assert_eq!(held, n, "every point in one posting");
        for (i, p) in ivf.postings.iter().enumerate() {
            assert!(p.members.iter().all(|&m| ivf.of[m as usize] == i as u32));
        }
    }

    #[test]
    fn rebalancing_keeps_points_in_their_nearest_posting() {
        let spec = EmbedSpec::sphere64();
        let n = 20_000;
        let store = points(spec, n, 9);
        let lire = SpfreshParams::default();
        let none = SpfreshParams {
            rebalance: false,
            ..lire
        };
        let misplaced = |params| {
            let mut ivf = Spfresh::new(params, spec.dim, 10);
            for id in 0..n as u32 {
                ivf.insert(id, &store);
            }
            ivf.misplaced(&store)
        };
        let (with, without) = (misplaced(lire), misplaced(none));
        // Centroids drift as points join, so some end up misplaced anyway.
        assert!(
            (with as f32) < 0.6 * without as f32,
            "{with} misplaced with LIRE, {without} without"
        );
    }

    #[test]
    fn probing_more_postings_finds_more_neighbors() {
        let spec = EmbedSpec::sphere64();
        let n = 8000;
        let store = points(spec, n + 50, 11);
        let mut ivf = Spfresh::new(SpfreshParams::default(), spec.dim, 12);
        for id in 0..n as u32 {
            ivf.insert(id, &store);
        }
        let ids: Vec<u32> = (0..n as u32).collect();
        let at = |nprobe| {
            (0..50)
                .map(|i| {
                    let q = row(&store, spec.dim, (n + i) as u32);
                    recall(
                        &ivf.search(q, 10, nprobe, &store).ids,
                        &brute_force(q, 10, &ids, &store, spec.dim),
                    )
                })
                .sum::<f32>()
                / 50.0
        };
        let (one, eight) = (at(1), at(8));
        assert!(one < eight && eight > 0.9, "{one} at 1, {eight} at 8");
    }
}
