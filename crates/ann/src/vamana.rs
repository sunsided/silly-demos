//! Vamana, the graph behind DiskANN (Subramanya et al., NeurIPS 2019): one
//! flat graph of every point, at most `r` links each, searched greedily
//! from the medoid. It is built in passes over all the points. Each point
//! searches the graph for itself and keeps, of the nodes the search
//! visited, those its robust prune allows: the nearest first, then any not
//! already `alpha` times nearer to one kept, and so on. The first pass
//! prunes with `alpha` = 1 from a random graph, the second with the set
//! `alpha`.
//!
//! DiskANN sets `alpha` to 1.2, so the prune rules out fewer candidates
//! and long links survive it, for walks that cross the space in few hops.
//! On these embeddings that backfires: the topics lie far apart and the
//! points of one close together, so the extra links a node keeps are all
//! near ones, they fill its `r`, and the few links between topics are
//! crowded out; queries in another topic than the medoid's are not
//! reached. With `alpha` = 1, a link to another topic is ruled out only by
//! a kept link that is nearer to it, and some always survive.

use std::cell::RefCell;

use stream::rng::Rng;

use crate::{Found, Nearest, Tally, Visited, dist2, row};

/// What the graph is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VamanaParams {
    /// Links per node at most.
    pub r: usize,
    /// Candidates kept while building.
    pub l: usize,
    /// How much nearer a kept link must be to rule a candidate out.
    pub alpha: f32,
}

impl Default for VamanaParams {
    /// Smaller than DiskANN's billion-scale settings, for tens of thousands,
    /// and `alpha` = 1 for clustered points (see the module).
    fn default() -> VamanaParams {
        VamanaParams {
            r: 32,
            l: 64,
            alpha: 1.0,
        }
    }
}

/// The graph, built over the points with ids `0..n`.
#[derive(Clone, Debug)]
pub struct Vamana {
    /// How it is tuned.
    pub params: VamanaParams,
    /// Values per vector.
    dim: usize,
    /// Each node's links.
    links: Vec<Vec<u32>>,
    /// Where every search starts: the point nearest the mean.
    medoid: u32,
    /// The order the passes visit the points in.
    order: Vec<u32>,
    /// The pass under way, 0 or 1; 2 once built.
    pass: usize,
    /// The next point of [`Vamana::order`] in this pass.
    cursor: usize,
    /// The marks of the current search.
    visited: RefCell<Visited>,
    /// Distances computed to build the graph.
    pub build_checks: u64,
}

impl Vamana {
    /// Starts building over points `0..n` of `store`: a random graph of `r`
    /// links each and the medoid. [`Vamana::build`] does the passes.
    pub fn new(params: VamanaParams, dim: usize, n: usize, store: &[f32], seed: u64) -> Vamana {
        let mut rng = Rng::new(seed);
        let r = params.r.min(n.saturating_sub(1));
        let links = (0..n as u32)
            .map(|i| {
                let mut out = Vec::with_capacity(r);
                while out.len() < r {
                    let j = rng.below(n) as u32;
                    if j != i && !out.contains(&j) {
                        out.push(j);
                    }
                }
                out
            })
            .collect();
        let mean: Vec<f32> = (0..dim)
            .map(|d| (0..n).map(|i| store[i * dim + d]).sum::<f32>() / n.max(1) as f32)
            .collect();
        let medoid = (0..n as u32)
            .min_by(|&a, &b| {
                dist2(row(store, dim, a), &mean).total_cmp(&dist2(row(store, dim, b), &mean))
            })
            .unwrap_or(0);
        let mut order: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            order.swap(i, rng.below(i + 1));
        }
        Vamana {
            params,
            dim,
            links,
            medoid,
            order,
            pass: 0,
            cursor: 0,
            visited: RefCell::new(Visited::default()),
            build_checks: n as u64,
        }
    }

    /// Whether both passes are done.
    pub fn is_built(&self) -> bool {
        self.pass >= 2
    }

    /// The share of the build done, 0 to 1.
    pub fn progress(&self) -> f32 {
        let n = self.order.len().max(1);
        ((self.pass * n + self.cursor) as f32 / (2 * n) as f32).min(1.0)
    }

    /// The links of node `id`.
    pub fn links(&self, id: u32) -> &[u32] {
        &self.links[id as usize]
    }

    /// Links stored.
    pub fn edges(&self) -> usize {
        self.links.iter().map(Vec::len).sum()
    }

    /// Visits up to `budget` more points of the passes. Returns how many.
    pub fn build(&mut self, budget: usize, store: &[f32]) -> usize {
        let mut done = 0;
        while done < budget && !self.is_built() {
            let alpha = if self.pass == 0 {
                1.0
            } else {
                self.params.alpha
            };
            let p = self.order[self.cursor];
            self.place(p, alpha, store);
            done += 1;
            self.cursor += 1;
            if self.cursor == self.order.len() {
                self.pass += 1;
                self.cursor = 0;
            }
        }
        done
    }

    /// Relinks point `p`: searches for it, prunes what the search visited
    /// and its old links into its new ones, and links each back to it,
    /// pruning any that then hold too many.
    fn place(&mut self, p: u32, alpha: f32, store: &[f32]) {
        let x = row(store, self.dim, p);
        let mut tally = Tally::default();
        let (_, mut pool) = self.walk(x, self.params.l, store, &mut tally);
        for &n in &self.links[p as usize] {
            if !pool.iter().any(|&(v, _)| v == n) {
                tally.checks += 1;
                pool.push((n, dist2(x, row(store, self.dim, n))));
            }
        }
        let chosen = self.prune(p, pool, alpha, store, &mut tally);
        self.links[p as usize] = chosen.clone();
        for n in chosen {
            if self.links[n as usize].contains(&p) {
                continue;
            }
            if self.links[n as usize].len() < self.params.r {
                self.links[n as usize].push(p);
                continue;
            }
            let y = row(store, self.dim, n);
            let pool: Vec<(u32, f32)> = self.links[n as usize]
                .iter()
                .chain([&p])
                .map(|&v| {
                    tally.checks += 1;
                    (v, dist2(y, row(store, self.dim, v)))
                })
                .collect();
            self.links[n as usize] = self.prune(n, pool, alpha, store, &mut tally);
        }
        self.build_checks += tally.checks;
    }

    /// The robust prune of `pool` (ids with their distances to `p`) to at
    /// most `r` links: take the nearest, rule out every candidate `alpha`
    /// times nearer to it than to `p`, and repeat.
    fn prune(
        &self,
        p: u32,
        mut pool: Vec<(u32, f32)>,
        alpha: f32,
        store: &[f32],
        tally: &mut Tally,
    ) -> Vec<u32> {
        pool.retain(|&(v, _)| v != p);
        pool.sort_by(|a, b| a.1.total_cmp(&b.1));
        pool.dedup_by_key(|e| e.0);
        let mut kept = Vec::with_capacity(self.params.r);
        // Squared distances: alpha applies to distances, so its square here.
        let a2 = alpha * alpha;
        let mut live = vec![true; pool.len()];
        for i in 0..pool.len() {
            if !live[i] {
                continue;
            }
            let (v, _) = pool[i];
            kept.push(v);
            if kept.len() == self.params.r {
                break;
            }
            let y = row(store, self.dim, v);
            for j in i + 1..pool.len() {
                if live[j] {
                    tally.checks += 1;
                    if a2 * dist2(y, row(store, self.dim, pool[j].0)) <= pool[j].1 {
                        live[j] = false;
                    }
                }
            }
        }
        kept
    }

    /// The greedy search from the medoid keeping `l` candidates: expand the
    /// nearest unexpanded one until all are expanded. Returns the
    /// candidates and every node expanded, with distances to `q`.
    fn walk(
        &self,
        q: &[f32],
        l: usize,
        store: &[f32],
        tally: &mut Tally,
    ) -> (Nearest, Vec<(u32, f32)>) {
        let mut visited = self.visited.borrow_mut();
        visited.clear(self.links.len());
        let mut list = Nearest::new(l);
        let mut expanded = Vec::new();
        if self.links.is_empty() {
            return (list, expanded);
        }
        tally.see(self.medoid);
        visited.mark(self.medoid);
        list.insert(self.medoid, dist2(q, row(store, self.dim, self.medoid)));
        // Expanded flags ride along: the list holds at most `l`, scanned linearly.
        let mut done: Vec<u32> = Vec::new();
        while let Some(&(c, dc)) = list.items.iter().find(|(v, _)| !done.contains(v)) {
            done.push(c);
            expanded.push((c, dc));
            for &n in &self.links[c as usize] {
                if visited.mark(n) {
                    tally.see(n);
                    list.insert(n, dist2(q, row(store, self.dim, n)));
                }
            }
        }
        (list, expanded)
    }

    /// The `k` nearest points to `q` found keeping `l` candidates (at least `k`).
    pub fn search(&self, q: &[f32], k: usize, l: usize, store: &[f32]) -> Found {
        let mut tally = Tally::seeing();
        let (list, _) = self.walk(q, l.max(k), store, &mut tally);
        tally.found(list.ids(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::points;
    use crate::{brute_force, recall};
    use stream::embed::{EmbedSpec, Embedder};

    #[test]
    fn two_passes_make_a_graph_search_can_trust() {
        let spec = EmbedSpec::sphere64();
        let n = 6000;
        let store = points(spec, n + 50, 5);
        let mut g = Vamana::new(VamanaParams::default(), spec.dim, n, &store, 6);
        while g.build(1000, &store) > 0 {}
        assert!(g.is_built() && g.progress() == 1.0);
        assert!(g.links.iter().all(|l| l.len() <= 32 && !l.is_empty()));
        let ids: Vec<u32> = (0..n as u32).collect();
        let (mut r, mut cost) = (0.0, 0.0);
        for i in 0..50 {
            let q = row(&store, spec.dim, (n + i) as u32);
            let found = g.search(q, 10, 64, &store);
            r += recall(&found.ids, &brute_force(q, 10, &ids, &store, spec.dim));
            cost += found.cost;
        }
        let (r, cost) = (r / 50.0, cost / 50.0);
        assert!(r > 0.95 && cost < n as f32 / 4.0, "recall {r} at {cost}");
    }

    #[test]
    fn long_links_crowd_out_the_way_between_topics() {
        let spec = EmbedSpec::sphere64();
        let n = 20_000;
        let mut e = Embedder::new(spec, 1);
        let mut topic = Vec::new();
        let store: Vec<f32> = (0..n)
            .flat_map(|_| {
                let (x, leaf) = e.next_point();
                topic.push(Embedder::labels(leaf)[0]);
                x
            })
            .collect();
        let across = |alpha| {
            let params = VamanaParams {
                alpha,
                ..VamanaParams::default()
            };
            let mut g = Vamana::new(params, spec.dim, n, &store, 5);
            while g.build(5000, &store) > 0 {}
            (0..n as u32)
                .flat_map(|i| g.links(i).iter().map(move |&j| (i, j)))
                .filter(|&(i, j)| topic[i as usize] != topic[j as usize])
                .count()
        };
        let (one, more) = (across(1.0), across(1.2));
        assert!(
            one > 3 * more,
            "{one} links across topics at 1, {more} at 1.2"
        );
    }
}
