//! Hierarchical navigable small worlds (Malkov and Yashunin, 2016): every
//! point is a node of a graph linked to a few near nodes, and a random few
//! are also nodes of a sparser graph above, and fewer still of one above
//! that. A search starts at the top, walks greedily to the node nearest
//! the query, drops a layer and walks again, and on the bottom layer keeps
//! the `ef` nearest it has met while any unexpanded one could still be
//! nearer. A point is inserted by searching for its own neighbors.
//!
//! The neighbors a node keeps are chosen by the paper's heuristic: a
//! candidate is kept only if it is nearer the node than to any neighbor
//! already kept, so the links spread in all directions instead of bunching
//! in the nearest cluster, and walks can cross between clusters.

use std::cell::RefCell;

use stream::rng::Rng;

use crate::{Found, Nearest, Tally, Visited, dist2, row};

/// What the graph is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HnswParams {
    /// Links per node on the upper layers; twice as many on the bottom one.
    pub m: usize,
    /// Candidates kept while inserting a point: wider finds better links.
    pub ef_construction: usize,
}

impl Default for HnswParams {
    /// The values hnswlib suggests for embeddings of this size.
    fn default() -> HnswParams {
        HnswParams {
            m: 16,
            ef_construction: 100,
        }
    }
}

/// A graph of the points on layers, the bottom holding every one.
#[derive(Clone, Debug)]
pub struct Hnsw {
    /// How it is tuned.
    pub params: HnswParams,
    /// Values per vector.
    dim: usize,
    /// Each node's links, per layer from the bottom up, by id.
    links: Vec<Vec<Vec<u32>>>,
    /// The node on the top layer every search starts from.
    entry: Option<u32>,
    /// Random numbers for the layers.
    rng: Rng,
    /// The marks of the current search.
    visited: RefCell<Visited>,
    /// Distances computed to insert points.
    pub build_checks: u64,
}

impl Hnsw {
    /// An empty graph for `dim`-long vectors.
    pub fn new(params: HnswParams, dim: usize, seed: u64) -> Hnsw {
        Hnsw {
            params,
            dim,
            links: Vec::new(),
            entry: None,
            rng: Rng::new(seed),
            visited: RefCell::new(Visited::default()),
            build_checks: 0,
        }
    }

    /// Points in the graph.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// The highest layer, or 0 when empty.
    pub fn top(&self) -> usize {
        self.entry
            .map_or(0, |e| self.links[e as usize].len().saturating_sub(1))
    }

    /// Nodes per layer, from the bottom up.
    pub fn layers(&self) -> Vec<usize> {
        let mut out = vec![0; self.top() + 1];
        for node in &self.links {
            for count in out.iter_mut().take(node.len()) {
                *count += 1;
            }
        }
        out
    }

    /// Links kept at most on `layer`.
    fn cap(&self, layer: usize) -> usize {
        if layer == 0 {
            2 * self.params.m
        } else {
            self.params.m
        }
    }

    /// Links stored, over all nodes and layers.
    pub fn edges(&self) -> usize {
        self.links.iter().flatten().map(Vec::len).sum()
    }

    /// Inserts point `id`, which must be the next id: the graph holds ids
    /// 0, 1, 2… in the order they came.
    pub fn insert(&mut self, id: u32, store: &[f32]) {
        assert_eq!(id as usize, self.links.len(), "ids come in order");
        // Layer ℓ holds a share 1/m^ℓ of the points.
        let u = 1.0 - self.rng.next_f32();
        let level = (-u.ln() / (self.params.m as f32).ln()) as usize;
        self.links.push(vec![Vec::new(); level + 1]);
        let Some(entry) = self.entry else {
            self.entry = Some(id);
            return;
        };
        let q = row(store, self.dim, id).to_vec();
        let top = self.top();
        let mut tally = Tally::default();
        tally.see(entry);
        let mut at = (entry, dist2(&q, row(store, self.dim, entry)));
        for layer in (level + 1..=top).rev() {
            at = self.greedy(&q, at, layer, store, &mut tally);
        }
        let mut entries = vec![at];
        for layer in (0..=level.min(top)).rev() {
            let found = self.layer_search(
                &q,
                &entries,
                self.params.ef_construction,
                layer,
                store,
                &mut tally,
            );
            let chosen = self.choose(&found.items, self.params.m, store, &mut tally);
            for &(n, d) in &chosen {
                let cap = self.cap(layer);
                let theirs = &mut self.links[n as usize][layer];
                theirs.push(id);
                if theirs.len() > cap {
                    // Too many: n keeps the ones the heuristic would choose.
                    let p = row(store, self.dim, n).to_vec();
                    let mut held: Vec<(u32, f32)> = self.links[n as usize][layer]
                        .iter()
                        .map(|&o| {
                            if o == id {
                                (o, d)
                            } else {
                                tally.see(o);
                                (o, dist2(&p, row(store, self.dim, o)))
                            }
                        })
                        .collect();
                    held.sort_by(|a, b| a.1.total_cmp(&b.1));
                    let kept = self.choose(&held, cap, store, &mut tally);
                    self.links[n as usize][layer] = kept.into_iter().map(|(o, _)| o).collect();
                }
            }
            self.links[id as usize][layer] = chosen.iter().map(|&(n, _)| n).collect();
            entries = found.items;
        }
        if level > top {
            self.entry = Some(id);
        }
        self.build_checks += tally.checks;
    }

    /// From `at`, the nearest node to `q` a greedy walk on `layer` reaches.
    fn greedy(
        &self,
        q: &[f32],
        mut at: (u32, f32),
        layer: usize,
        store: &[f32],
        tally: &mut Tally,
    ) -> (u32, f32) {
        loop {
            let mut moved = false;
            for &n in &self.links[at.0 as usize][layer] {
                tally.see(n);
                let d = dist2(q, row(store, self.dim, n));
                if d < at.1 {
                    at = (n, d);
                    moved = true;
                }
            }
            if !moved {
                return at;
            }
        }
    }

    /// The `ef` nearest nodes to `q` on `layer` found from `entries`:
    /// expand the nearest unexpanded candidate until none could still beat
    /// the farthest kept.
    fn layer_search(
        &self,
        q: &[f32],
        entries: &[(u32, f32)],
        ef: usize,
        layer: usize,
        store: &[f32],
        tally: &mut Tally,
    ) -> Nearest {
        let mut visited = self.visited.borrow_mut();
        visited.clear(self.links.len());
        let mut kept = Nearest::new(ef);
        // Candidates to expand, nearest last so popping takes it.
        let mut open: Vec<(u32, f32)> = Vec::new();
        for &(e, d) in entries {
            if visited.mark(e) {
                kept.insert(e, d);
                open.push((e, d));
            }
        }
        open.sort_by(|a, b| b.1.total_cmp(&a.1));
        while let Some((c, dc)) = open.pop() {
            if dc > kept.bound() {
                break;
            }
            for &n in &self.links[c as usize][layer] {
                if !visited.mark(n) {
                    continue;
                }
                tally.see(n);
                let d = dist2(q, row(store, self.dim, n));
                if kept.insert(n, d).is_some() {
                    let at = open.partition_point(|&(_, e)| e > d);
                    open.insert(at, (n, d));
                }
            }
        }
        kept
    }

    /// Up to `m` of `candidates` (nearest first, with their distances to
    /// the node) by the heuristic: each is kept if it is nearer the node
    /// than to every one kept before it.
    fn choose(
        &self,
        candidates: &[(u32, f32)],
        m: usize,
        store: &[f32],
        tally: &mut Tally,
    ) -> Vec<(u32, f32)> {
        let mut kept: Vec<(u32, f32)> = Vec::with_capacity(m);
        for &(c, d) in candidates {
            if kept.len() >= m {
                break;
            }
            let x = row(store, self.dim, c);
            let spread = kept.iter().all(|&(k, _)| {
                tally.checks += 1;
                dist2(x, row(store, self.dim, k)) > d
            });
            if spread {
                kept.push((c, d));
            }
        }
        kept
    }

    /// The `k` nearest points to `q` found keeping `ef` candidates on the
    /// bottom layer (at least `k`).
    pub fn search(&self, q: &[f32], k: usize, ef: usize, store: &[f32]) -> Found {
        let Some(entry) = self.entry else {
            return Found::default();
        };
        let mut tally = Tally::seeing();
        tally.see(entry);
        let mut at = (entry, dist2(q, row(store, self.dim, entry)));
        for layer in (1..=self.top()).rev() {
            at = self.greedy(q, at, layer, store, &mut tally);
        }
        let found = self.layer_search(q, &[at], ef.max(k), 0, store, &mut tally);
        tally.found(found.ids(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::points;
    use crate::{brute_force, recall};
    use stream::embed::EmbedSpec;

    #[test]
    fn layers_thin_out_by_m() {
        let spec = EmbedSpec::sphere64();
        let store = points(spec, 4000, 1);
        let mut g = Hnsw::new(HnswParams::default(), spec.dim, 2);
        for id in 0..4000 {
            g.insert(id, &store);
        }
        let layers = g.layers();
        assert_eq!(layers[0], 4000);
        assert!((150..350).contains(&layers[1]), "{layers:?}");
        assert!(g.links.iter().all(|n| n[0].len() <= 32 && !n[0].is_empty()));
    }

    #[test]
    fn search_finds_nearly_every_neighbor_for_a_few_hundred_distances() {
        let spec = EmbedSpec::sphere64();
        let n = 6000;
        let store = points(spec, n + 50, 3);
        let mut g = Hnsw::new(HnswParams::default(), spec.dim, 4);
        for id in 0..n as u32 {
            g.insert(id, &store);
        }
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
        let per_point = g.build_checks as f32 / n as f32;
        assert!(per_point < 3000.0, "{per_point} per point to build");
    }
}
