//! The inverted file with product quantization (Jégou, Douze and Schmid,
//! 2011), as FAISS's IVF-PQ: built in a batch, the way most deployed
//! indexes are. k-means on a sample makes the coarse centroids and every
//! point goes in the list of its nearest. What is left of a point after
//! its centroid, the residual, is cut into `subspaces` parts and each part
//! replaced by the nearest of `codes` centers learned for it, so a
//! 64-dimensional vector of 256 bytes is kept as 16 four-bit codes, 8
//! bytes.
//!
//! A query probes its `nprobe` nearest lists. For each it computes, once,
//! the distance from its own residual's parts to every part center, a
//! small table, and then a point's distance is a sum of table entries, a
//! quarter of the work of a true distance per point and a 32nd of the
//! memory. The estimate is coarse, so the best few hundred by it are
//! compared exactly, reading their full vectors, as a disk-based index
//! reads them from disk.

use stream::rng::Rng;
use stream::tree::kmeans;

use crate::{Found, Nearest, dist2, row};

/// What the index is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PqParams {
    /// Coarse centroids, and lists.
    pub lists: usize,
    /// Parts each residual is cut into.
    pub subspaces: usize,
    /// Centers per part; 16 makes a code four bits.
    pub codes: usize,
    /// Candidates by the estimate compared exactly; 0 for none.
    pub rerank: usize,
    /// Points the centroids and part centers are learned from at most.
    pub sample: usize,
}

impl Default for PqParams {
    /// Eight bytes per vector, a hundred lists for tens of thousands of points.
    fn default() -> PqParams {
        PqParams {
            lists: 128,
            subspaces: 16,
            codes: 16,
            rerank: 100,
            sample: 4096,
        }
    }
}

/// The lists and the codes.
#[derive(Clone, Debug)]
pub struct IvfPq {
    /// How it is tuned.
    pub params: PqParams,
    /// Values per vector.
    dim: usize,
    /// The coarse centroids.
    centers: Vec<Vec<f32>>,
    /// The ids in each list.
    lists: Vec<Vec<u32>>,
    /// The part centers: per subspace, `codes` of them, flat.
    books: Vec<Vec<f32>>,
    /// Every point's codes, `subspaces` each, by id.
    codes: Vec<u8>,
    /// Distances computed to build it, in full-length equivalents.
    pub build_cost: f64,
}

impl IvfPq {
    /// Builds the index over points `0..n` of `store`.
    pub fn build(params: PqParams, dim: usize, n: usize, store: &[f32], seed: u64) -> IvfPq {
        assert!(
            n > 0 && dim.is_multiple_of(params.subspaces),
            "parts of equal length"
        );
        let mut rng = Rng::new(seed);
        let part = dim / params.subspaces;
        let step = n.div_ceil(params.sample).max(1);
        let sample: Vec<u32> = (0..n as u32).step_by(step).collect();
        let rows: Vec<&[f32]> = sample.iter().map(|&i| row(store, dim, i)).collect();
        let mut checks = 0;
        let (centers, _, _) = kmeans(&rows, params.lists, &mut rng, &mut checks);
        let mut cost = checks as f64;
        // Every point to its nearest centroid.
        let mut lists = vec![Vec::new(); centers.len()];
        let mut home = vec![0usize; n];
        for (id, h) in home.iter_mut().enumerate() {
            let x = row(store, dim, id as u32);
            *h = nearest(x, &centers);
            lists[*h].push(id as u32);
        }
        cost += (n * centers.len()) as f64;
        let residual = |id: u32, h: usize| -> Vec<f32> {
            row(store, dim, id)
                .iter()
                .zip(&centers[h])
                .map(|(x, c)| x - c)
                .collect()
        };
        let residuals: Vec<Vec<f32>> = sample
            .iter()
            .map(|&i| residual(i, home[i as usize]))
            .collect();
        let mut books = Vec::with_capacity(params.subspaces);
        for s in 0..params.subspaces {
            let parts: Vec<&[f32]> = residuals
                .iter()
                .map(|r| &r[s * part..(s + 1) * part])
                .collect();
            let mut checks = 0;
            let (found, _, _) = kmeans(&parts, params.codes, &mut rng, &mut checks);
            cost += checks as f64 * part as f64 / dim as f64;
            books.push(found.concat());
        }
        let mut codes = vec![0u8; n * params.subspaces];
        for id in 0..n {
            let r = residual(id as u32, home[id]);
            for (s, book) in books.iter().enumerate() {
                let x = &r[s * part..(s + 1) * part];
                codes[id * params.subspaces + s] = (0..book.len() / part)
                    .min_by(|&a, &b| {
                        dist2(x, &book[a * part..(a + 1) * part])
                            .total_cmp(&dist2(x, &book[b * part..(b + 1) * part]))
                    })
                    .expect("a part center")
                    as u8;
            }
        }
        cost += (n * params.codes) as f64;
        IvfPq {
            params,
            dim,
            centers,
            lists,
            books,
            codes,
            build_cost: cost,
        }
    }

    /// Bytes of code per vector: `subspaces` codes of log₂ `codes` bits.
    pub fn code_bytes(&self) -> f32 {
        self.params.subspaces as f32 * (self.params.codes as f32).log2() / 8.0
    }

    /// The `k` nearest points to `q` found in the `nprobe` nearest lists by
    /// the codes, the best [`PqParams::rerank`] of them compared exactly.
    pub fn search(&self, q: &[f32], k: usize, nprobe: usize, store: &[f32]) -> Found {
        let (subspaces, part) = (self.params.subspaces, self.dim / self.params.subspaces);
        let mut probes = Nearest::new(nprobe);
        for (i, c) in self.centers.iter().enumerate() {
            probes.insert(i as u32, dist2(q, c));
        }
        let mut cost = self.centers.len() as f32;
        let wanted = self.params.rerank.max(k);
        let mut estimate = Nearest::new(wanted);
        let mut table = vec![0.0f32; subspaces * self.params.codes];
        let mut seen = Vec::new();
        for &(l, _) in &probes.items {
            let c = &self.centers[l as usize];
            let r: Vec<f32> = q.iter().zip(c).map(|(x, c)| x - c).collect();
            for (s, book) in self.books.iter().enumerate() {
                let x = &r[s * part..(s + 1) * part];
                for (j, center) in book.chunks_exact(part).enumerate() {
                    table[s * self.params.codes + j] = dist2(x, center);
                }
            }
            // A table holds `codes` part distances per subspace: `codes` full ones.
            cost += self.params.codes as f32;
            for &id in &self.lists[l as usize] {
                let code = &self.codes[id as usize * subspaces..(id as usize + 1) * subspaces];
                let d: f32 = code
                    .iter()
                    .enumerate()
                    .map(|(s, &j)| table[s * self.params.codes + j as usize])
                    .sum();
                seen.push(id);
                estimate.insert(id, d);
            }
            // A lookup per part, against a multiply-add per dimension.
            cost += self.lists[l as usize].len() as f32 * subspaces as f32 / self.dim as f32;
        }
        if self.params.rerank == 0 {
            return Found {
                ids: estimate.ids(k),
                cost,
                seen,
            };
        }
        let mut exact = Nearest::new(k);
        for &(id, _) in &estimate.items {
            exact.insert(id, dist2(q, row(store, self.dim, id)));
        }
        cost += estimate.items.len() as f32;
        Found {
            ids: exact.ids(k),
            cost,
            seen,
        }
    }
}

/// The index of the center of `centers` nearest `x`.
fn nearest(x: &[f32], centers: &[Vec<f32>]) -> usize {
    (0..centers.len())
        .min_by(|&a, &b| dist2(x, &centers[a]).total_cmp(&dist2(x, &centers[b])))
        .expect("a center")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::points;
    use crate::{brute_force, recall};
    use stream::embed::EmbedSpec;

    /// Mean recall@10 of `index` over 50 queries after the first `n` points.
    fn mean_recall(index: &IvfPq, store: &[f32], n: usize, nprobe: usize) -> (f32, f32) {
        let ids: Vec<u32> = (0..n as u32).collect();
        let (mut r, mut cost) = (0.0, 0.0);
        for i in 0..50 {
            let q = row(store, 64, (n + i) as u32);
            let found = index.search(q, 10, nprobe, store);
            r += recall(&found.ids, &brute_force(q, 10, &ids, store, 64));
            cost += found.cost;
        }
        (r / 50.0, cost / 50.0)
    }

    #[test]
    fn codes_alone_find_some_neighbors_and_reranking_the_rest() {
        let spec = EmbedSpec::sphere64();
        let n = 8000;
        let store = points(spec, n + 50, 13);
        let reranked = IvfPq::build(PqParams::default(), spec.dim, n, &store, 14);
        assert_eq!(reranked.code_bytes(), 8.0);
        let codes_only = IvfPq {
            params: PqParams {
                rerank: 0,
                ..reranked.params
            },
            ..reranked.clone()
        };
        let (coarse, _) = mean_recall(&codes_only, &store, n, 16);
        let (fine, cost) = mean_recall(&reranked, &store, n, 16);
        assert!(
            coarse > 0.2 && fine > coarse + 0.2 && fine > 0.85,
            "{coarse} codes only, {fine} reranked"
        );
        assert!(cost < n as f32 / 4.0, "{cost}");
    }
}
