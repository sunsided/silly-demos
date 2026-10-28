//! Mock embeddings with a known hierarchy: points drawn from a tree of
//! topics, subtopics and leaf clusters, each level a tighter spread around
//! its parent. In many dimensions and normalized to unit length they stand
//! in for image embeddings compared by cosine; in two they can be watched.

use crate::rng::Rng;
use crate::vector::normalize;

/// Children per node of the true tree: topics, subtopics per topic, leaf
/// clusters per subtopic.
pub const BRANCHING: [usize; 3] = [6, 4, 3];
/// Levels of the true tree below the root.
pub const LEVELS: usize = BRANCHING.len();

/// How the embeddings are made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbedSpec {
    /// Dimensions per vector.
    pub dim: usize,
    /// Normalize every vector to unit length (cosine geometry).
    pub sphere: bool,
    /// How far each level's centers lie from their parent's, root first.
    pub spread: [f32; LEVELS],
    /// The spread of points around their leaf center, per dimension.
    pub noise: f32,
}

impl EmbedSpec {
    /// 64 dimensions on the unit sphere: topics far apart, subtopics closer,
    /// leaves closer still, so each level is a little harder to tell apart.
    /// The noise is about as large as the gap between neighboring leaves,
    /// so a point's nearest neighbors are often in the next leaf over.
    pub fn sphere64() -> EmbedSpec {
        EmbedSpec {
            dim: 64,
            sphere: true,
            spread: [1.0, 0.45, 0.2],
            noise: 0.06,
        }
    }

    /// The same tree in the unit square, for drawing directly.
    pub fn plane() -> EmbedSpec {
        EmbedSpec {
            dim: 2,
            sphere: false,
            spread: [0.28, 0.085, 0.028],
            noise: 0.007,
        }
    }
}

/// A generator of labeled embeddings from one seeded true tree.
#[derive(Clone, Debug)]
pub struct Embedder {
    /// How they are made.
    pub spec: EmbedSpec,
    /// Every leaf cluster's center, flat, `dim` values each, in label order.
    leaves: Vec<f32>,
    /// Random numbers for the points.
    rng: Rng,
}

impl Embedder {
    /// The true tree for `spec` from `seed`.
    pub fn new(spec: EmbedSpec, seed: u64) -> Embedder {
        let mut rng = Rng::new(seed);
        let dim = spec.dim;
        let root: Vec<f32> = if spec.sphere {
            vec![0.0; dim]
        } else {
            vec![0.5; dim]
        };
        let mut level = vec![root];
        for (depth, &children) in BRANCHING.iter().enumerate() {
            let mut next = Vec::with_capacity(level.len() * children);
            for parent in &level {
                // In the plane, siblings go evenly round their parent, turned
                // at random, so no two land on top of each other.
                let turn = rng.next_f32() * std::f32::consts::TAU;
                for k in 0..children {
                    let offset: Vec<f32> = if spec.sphere {
                        let mut o: Vec<f32> = (0..dim).map(|_| rng.normal()).collect();
                        normalize(&mut o);
                        o
                    } else {
                        let a = turn + std::f32::consts::TAU * k as f32 / children as f32;
                        vec![a.cos(), a.sin()]
                    };
                    let r = spec.spread[depth];
                    next.push(parent.iter().zip(&offset).map(|(p, o)| p + r * o).collect());
                }
            }
            level = next;
        }
        Embedder {
            spec,
            leaves: level.into_iter().flatten().collect(),
            rng,
        }
    }

    /// Leaf clusters of the true tree.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len() / self.spec.dim
    }

    /// The labels of leaf `leaf` at every level, topic first.
    pub fn labels(leaf: usize) -> [u32; LEVELS] {
        let mut out = [0u32; LEVELS];
        let mut divisor = 1;
        for level in (0..LEVELS).rev() {
            out[level] = (leaf / divisor) as u32;
            divisor *= BRANCHING[level];
        }
        out
    }

    /// The next point and the leaf it came from.
    pub fn next_point(&mut self) -> (Vec<f32>, usize) {
        let dim = self.spec.dim;
        let leaf = self.rng.below(self.leaf_count());
        let center = &self.leaves[leaf * dim..(leaf + 1) * dim];
        let mut x: Vec<f32> = center
            .iter()
            .map(|c| c + self.rng.normal() * self.spec.noise)
            .collect();
        if self.spec.sphere {
            normalize(&mut x);
        }
        (x, leaf)
    }
}

/// A fixed projection from `dim` dimensions to 2 for drawing: the two
/// principal axes of `sample` (power iteration), each scaled so the sample
/// spans about the unit square. For 2D data, the identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    /// The sample mean.
    mean: Vec<f32>,
    /// The two axes, each `dim` long.
    axes: [Vec<f32>; 2],
    /// Offset and scale per axis, into [0, 1].
    fit: [(f32, f32); 2],
}

impl Projection {
    /// The principal axes of `sample` (flat, `dim` values per row).
    pub fn fit(sample: &[f32], dim: usize, rng: &mut Rng) -> Projection {
        let n = (sample.len() / dim).max(1);
        if dim == 2 {
            let mut axes = [vec![0.0; 2], vec![0.0; 2]];
            axes[0][0] = 1.0;
            axes[1][1] = 1.0;
            return Projection {
                mean: vec![0.0; 2],
                axes,
                fit: [(0.0, 1.0), (0.0, 1.0)],
            };
        }
        let mean: Vec<f32> = (0..dim)
            .map(|d| sample.chunks_exact(dim).map(|r| r[d]).sum::<f32>() / n as f32)
            .collect();
        let centered: Vec<f32> = sample
            .chunks_exact(dim)
            .flat_map(|r| r.iter().zip(&mean).map(|(v, m)| v - m))
            .collect();
        let mut axes: [Vec<f32>; 2] = [Vec::new(), Vec::new()];
        for k in 0..2 {
            let mut v: Vec<f32> = (0..dim).map(|_| rng.normal()).collect();
            for _ in 0..60 {
                // v ← C v, with C the sample covariance, deflated by the axes found so far.
                let mut next = vec![0.0f32; dim];
                for row in centered.chunks_exact(dim) {
                    let dot: f32 = row.iter().zip(&v).map(|(a, b)| a * b).sum();
                    for (o, &r) in next.iter_mut().zip(row) {
                        *o += dot * r;
                    }
                }
                for prev in &axes[..k] {
                    let dot: f32 = next.iter().zip(prev).map(|(a, b)| a * b).sum();
                    for (o, &p) in next.iter_mut().zip(prev) {
                        *o -= dot * p;
                    }
                }
                normalize(&mut next);
                v = next;
            }
            axes[k] = v;
        }
        let mut projection = Projection {
            mean,
            axes,
            fit: [(0.0, 1.0), (0.0, 1.0)],
        };
        // Fit the sample into [0.05, 0.95] on both axes.
        for k in 0..2 {
            let coords: Vec<f32> = sample
                .chunks_exact(dim)
                .map(|r| projection.raw(r, k))
                .collect();
            let (lo, hi) = coords
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &c| {
                    (lo.min(c), hi.max(c))
                });
            let span = (hi - lo).max(1e-6);
            projection.fit[k] = (lo - 0.05 * span / 0.9, 0.9 / span);
        }
        // The same scale on both axes, so distances look like distances.
        let scale = projection.fit[0].1.min(projection.fit[1].1);
        for k in 0..2 {
            projection.fit[k].1 = scale;
        }
        projection
    }

    /// The coordinate of `x` along axis `k`, before fitting.
    fn raw(&self, x: &[f32], k: usize) -> f32 {
        x.iter()
            .zip(&self.mean)
            .zip(&self.axes[k])
            .map(|((v, m), a)| (v - m) * a)
            .sum()
    }

    /// `x` in the unit square.
    pub fn apply(&self, x: &[f32]) -> [f32; 2] {
        [0, 1].map(|k| (self.raw(x, k) - self.fit[k].0) * self.fit[k].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::dist2;

    #[test]
    fn labels_count_down_the_tree() {
        assert_eq!(Embedder::labels(0), [0, 0, 0]);
        assert_eq!(Embedder::labels(3), [0, 1, 3]);
        assert_eq!(Embedder::labels(12), [1, 4, 12]);
        assert_eq!(Embedder::labels(71), [5, 23, 71]);
    }

    #[test]
    fn points_are_unit_vectors_mostly_nearest_their_own_leaf() {
        let mut e = Embedder::new(EmbedSpec::sphere64(), 1);
        assert_eq!(e.leaf_count(), 72);
        let mut own = 0;
        for _ in 0..200 {
            let (x, leaf) = e.next_point();
            let norm: f32 = x.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5);
            let dim = 64;
            let nearest = (0..72)
                .min_by(|&a, &b| {
                    let d = |l: usize| dist2(&x, &e.leaves[l * dim..(l + 1) * dim]);
                    d(a).total_cmp(&d(b))
                })
                .unwrap();
            own += (nearest == leaf) as usize;
        }
        // The noise is about the gap between leaves: most, not all, are nearest their own.
        assert!(own > 140, "{own} of 200");
    }

    #[test]
    fn the_projection_keeps_topics_apart() {
        let mut e = Embedder::new(EmbedSpec::sphere64(), 2);
        let mut sample = Vec::new();
        let mut leaves = Vec::new();
        for _ in 0..1200 {
            let (x, leaf) = e.next_point();
            sample.extend(x);
            leaves.push(leaf);
        }
        let p = Projection::fit(&sample, 64, &mut Rng::new(3));
        let pts: Vec<[f32; 2]> = sample
            .as_chunks::<64>()
            .0
            .iter()
            .map(|r| p.apply(r))
            .collect();
        assert!(
            pts.iter()
                .all(|q| q.iter().all(|v| (-0.05..=1.05).contains(v)))
        );
        // Points of one topic sit closer together than points of different topics.
        let topic = |i: usize| Embedder::labels(leaves[i])[0];
        let (mut same, mut ns, mut diff, mut nd) = (0.0, 0, 0.0, 0);
        for i in 0..300 {
            for j in i + 1..300 {
                let d = crate::dist2(pts[i], pts[j]).sqrt();
                if topic(i) == topic(j) {
                    same += d;
                    ns += 1;
                } else {
                    diff += d;
                    nd += 1;
                }
            }
        }
        assert!(
            same / ns as f32 * 1.5 < diff / nd as f32,
            "{} vs {}",
            same / ns as f32,
            diff / nd as f32
        );
    }
}
