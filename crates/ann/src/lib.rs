//! Approximate nearest-neighbor search over embeddings, four ways, each
//! counted in the distances it computes, so they can be set side by side
//! with the tree of clusters in [`stream::tree`].
//!
//! [`hnsw`] links every point to a few near ones on several layers of
//! thinning graphs and searches greedily down them; it is built point by
//! point as they arrive. [`spfresh`] keeps the points in postings around
//! centroids, as an inverted file does, and splits a posting that grows
//! too long, reassigning the points the split may have stranded, after
//! SPFresh's LIRE. [`pq`] is the batch inverted file with product
//! quantization: vectors are compressed to a few bytes and compared
//! through lookup tables. [`vamana`] is DiskANN's graph, built in passes
//! over all the points with a pruning rule that keeps long edges.
//!
//! Every index works on one flat store of `dim`-long vectors, addressed by
//! id, that it doesn't own: the points live once, in the caller.

pub mod hnsw;
pub mod pq;
pub mod spfresh;
pub mod vamana;

pub use stream::tree::{brute_force, recall};
pub use stream::vector::dist2;

/// Vector `id` of `store`, `dim` values long.
pub fn row(store: &[f32], dim: usize, id: u32) -> &[f32] {
    let i = id as usize * dim;
    &store[i..i + dim]
}

/// What a search found and what it cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// The ids found, nearest first.
    pub ids: Vec<u32>,
    /// The distances computed, in full-length equivalents: a distance
    /// between two vectors counts one, a lookup in a table of distances
    /// between parts of them counts the share of the vector it covers.
    pub cost: f32,
    /// The points the query was compared with, by vector or by code.
    pub seen: Vec<u32>,
}

/// The distances a search or a build computes, and while searching, the
/// points they were to.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    /// Distances computed.
    pub checks: u64,
    /// The points compared with, if kept.
    pub seen: Option<Vec<u32>>,
}

impl Tally {
    /// A tally that keeps the points compared with.
    pub fn seeing() -> Tally {
        Tally {
            checks: 0,
            seen: Some(Vec::new()),
        }
    }

    /// Counts a distance to point `id`.
    pub fn see(&mut self, id: u32) {
        self.checks += 1;
        if let Some(seen) = &mut self.seen {
            seen.push(id);
        }
    }

    /// What was found, with this tally's cost and points.
    pub fn found(self, ids: Vec<u32>) -> Found {
        Found {
            ids,
            cost: self.checks as f32,
            seen: self.seen.unwrap_or_default(),
        }
    }
}

/// A list of `(id, distance)` kept sorted, nearest first, and at most
/// `cap` long: the candidates of a greedy graph search.
#[derive(Clone, Debug)]
pub struct Nearest {
    /// The entries, nearest first.
    pub items: Vec<(u32, f32)>,
    /// The most it holds.
    cap: usize,
}

impl Nearest {
    /// An empty list holding at most `cap`.
    pub fn new(cap: usize) -> Nearest {
        Nearest {
            items: Vec::with_capacity(cap + 1),
            cap: cap.max(1),
        }
    }

    /// Whether `d` would make the list: it isn't full, or `d` beats the farthest.
    pub fn admits(&self, d: f32) -> bool {
        self.items.len() < self.cap || d < self.items[self.items.len() - 1].1
    }

    /// Puts `(id, d)` in its place, dropping the farthest beyond the cap.
    /// Returns where it went, or `None` if it didn't make the list.
    pub fn insert(&mut self, id: u32, d: f32) -> Option<usize> {
        if !self.admits(d) {
            return None;
        }
        let at = self.items.partition_point(|&(_, e)| e <= d);
        self.items.insert(at, (id, d));
        self.items.truncate(self.cap);
        Some(at)
    }

    /// The farthest distance in the list, or infinity while it isn't full.
    pub fn bound(&self) -> f32 {
        if self.items.len() < self.cap {
            f32::INFINITY
        } else {
            self.items[self.items.len() - 1].1
        }
    }

    /// The first `k` ids.
    pub fn ids(&self, k: usize) -> Vec<u32> {
        self.items.iter().take(k).map(|&(id, _)| id).collect()
    }
}

/// Marks of the nodes a search has seen, cleared in constant time by
/// moving to a new generation.
#[derive(Clone, Debug, Default)]
pub struct Visited {
    /// The generation each node was last seen in.
    seen: Vec<u32>,
    /// The current generation; 0 is never current.
    now: u32,
}

impl Visited {
    /// Forgets every mark, and makes room for `n` nodes.
    pub fn clear(&mut self, n: usize) {
        if self.seen.len() < n {
            self.seen.resize(n, 0);
        }
        self.now = self.now.wrapping_add(1);
        if self.now == 0 {
            self.seen.iter_mut().for_each(|s| *s = 0);
            self.now = 1;
        }
    }

    /// Marks node `id`; `true` if it wasn't marked before.
    pub fn mark(&mut self, id: u32) -> bool {
        let s = &mut self.seen[id as usize];
        let fresh = *s != self.now;
        *s = self.now;
        fresh
    }
}

/// What the tests share.
#[cfg(test)]
pub(crate) mod testing {
    use stream::embed::{EmbedSpec, Embedder};

    /// `n` points of `spec` from `seed`, flat.
    pub(crate) fn points(spec: EmbedSpec, n: usize, seed: u64) -> Vec<f32> {
        let mut e = Embedder::new(spec, seed);
        (0..n).flat_map(|_| e.next_point().0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_keeps_the_nearest_in_order() {
        let mut list = Nearest::new(3);
        for (id, d) in [(0, 5.0), (1, 1.0), (2, 3.0), (3, 4.0), (4, 0.5), (5, 9.0)] {
            list.insert(id, d);
        }
        assert_eq!(list.ids(10), vec![4, 1, 2]);
        assert_eq!(list.bound(), 3.0);
        assert!(!list.admits(3.0) && list.admits(2.9));
    }

    #[test]
    fn marks_clear_by_generation() {
        let mut v = Visited::default();
        v.clear(4);
        assert!(v.mark(2) && !v.mark(2));
        v.clear(4);
        assert!(v.mark(2));
    }
}
