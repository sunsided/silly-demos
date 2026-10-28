//! A hierarchy of clusters built from a stream, without the pairwise
//! distance matrix. Every node is a cluster with a center; a point descends
//! to the nearest child at every node and settles in a leaf, so placing it
//! meets only the children on its way down, a few dozen distances however
//! many points there are. A leaf that has gathered enough points clusters
//! them into children of its own, and a query searches the same way down.
//!
//! The tree can grow two ways. Online, leaves split as their points arrive,
//! from the points they have so far, and later points follow the centers.
//! In passes, as a batch system would, all stored points are clustered at
//! the root, then each child's points, and so on, one pass per level.
//!
//! A leaf splits by k-means (k-means++ seeding, then Lloyd's rounds), run
//! for every k up to a cap, keeping the k with the best Bayesian
//! information criterion: squared error against the parameters k centers
//! add. So every node finds its own number of children. The ball k-means
//! the flat sketch refines with, which moves a center by only the points
//! within half the way to the next center, doesn't carry over: in 64
//! dimensions hardly any point lies that close, and centers follow a few.
//!
//! A one-pass facility-location sketch of each leaf's points, as the flat
//! [`crate::sketch`] keeps, doesn't do here. In 64 dimensions the noise
//! between two points of one cluster outweighs the gap between neighboring
//! clusters, so the heaviest facility, sitting at the node's mean, is
//! nearer to almost every new point than any other and takes them all: the
//! sketch collapses into one facility and a node's children are lost.

use crate::rng::Rng;
use crate::vector::{dist2, pull, weighted_mean};

/// What the tree is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeParams {
    /// Points a leaf takes before it tries to split; it tries again at
    /// twice as many if it found nothing to split into.
    pub split_at: u32,
    /// The deepest a node may be; the root is depth 0.
    pub max_depth: usize,
    /// Children a node may have at most.
    pub max_children: usize,
    /// A factor on the information criterion's penalty per child: above 1
    /// the tree splits less readily, below 1 more.
    pub price: f32,
}

impl Default for TreeParams {
    /// Split from 300 points, four levels of at most 8 children, the
    /// criterion's penalty as it is.
    fn default() -> Self {
        TreeParams {
            split_at: 300,
            max_depth: 4,
            max_children: 8,
            price: 1.0,
        }
    }
}

/// Points of a leaf that a split clusters at most; a sample beyond that.
const SPLIT_SAMPLE: usize = 2000;
/// Rounds of the ball k-means at most.
const ROUNDS: usize = 15;
/// A child needs at least this share of its parent's points.
const MIN_SHARE: f32 = 0.03;

/// k-means of `points` into `k` clusters: k-means++ seeding, then Lloyd's
/// rounds of assigning every point to its nearest center and moving each
/// center to the mean of its points. Returns the centers, the sum of
/// squared distances to them and the points each holds, and adds the
/// distances computed to `checks`.
pub fn kmeans(
    points: &[&[f32]],
    k: usize,
    rng: &mut Rng,
    checks: &mut u64,
) -> (Vec<Vec<f32>>, f32, Vec<usize>) {
    let n = points.len();
    let k = k.min(n).max(1);
    let dim = points[0].len();
    let mut centers: Vec<Vec<f32>> = vec![points[rng.below(n)].to_vec()];
    let mut nearest: Vec<f32> = points.iter().map(|p| dist2(p, &centers[0])).collect();
    *checks += n as u64;
    while centers.len() < k {
        let total: f32 = nearest.iter().sum();
        let mut r = rng.next_f32() * total;
        let pick = nearest
            .iter()
            .position(|&d| {
                r -= d;
                r < 0.0
            })
            .unwrap_or(n - 1);
        centers.push(points[pick].to_vec());
        let last = centers.len() - 1;
        for (d, p) in nearest.iter_mut().zip(points) {
            *d = d.min(dist2(p, &centers[last]));
        }
        *checks += n as u64;
    }
    let mut assigned = vec![usize::MAX; n];
    let mut sse = 0.0;
    for _ in 0..ROUNDS {
        let mut changed = false;
        sse = 0.0;
        for (i, p) in points.iter().enumerate() {
            let (c, d) = (0..centers.len())
                .map(|c| (c, dist2(p, &centers[c])))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("a center");
            changed |= assigned[i] != c;
            assigned[i] = c;
            sse += d;
        }
        *checks += (n * centers.len()) as u64;
        if !changed {
            break;
        }
        let mut sums = vec![(vec![0.0f32; dim], 0usize); centers.len()];
        for (p, &c) in points.iter().zip(&assigned) {
            let (sum, count) = &mut sums[c];
            sum.iter_mut().zip(p.iter()).for_each(|(s, v)| *s += v);
            *count += 1;
        }
        for (center, (sum, count)) in centers.iter_mut().zip(sums) {
            if count > 0 {
                *center = sum.into_iter().map(|v| v / count as f32).collect();
            }
        }
    }
    let mut counts = vec![0usize; centers.len()];
    for &a in &assigned {
        if a < counts.len() {
            counts[a] += 1;
        }
    }
    (centers, sse, counts)
}

/// The Bayesian information criterion of a hard mixture of spherical
/// Gaussians with a shared variance, as X-means scores one (Pelleg and
/// Moore, 2000): −2 log-likelihood, that is −2 Σⱼ nⱼ ln(nⱼ/n) for the
/// mixture weights plus n·d·ln(sse / (n·d)) for the fit, plus `price`
/// times the penalty k·(d + 1)·ln n for the centers and weights. `counts`
/// holds the points per cluster. Lower is better.
///
/// The weights' term is what keeps one round cluster whole: halving it
/// lowers the squared error by about a fifth in two dimensions, but spends
/// n·ln 2 on telling the halves apart.
pub fn bic(sse: f32, counts: &[usize], dim: usize, price: f32) -> f32 {
    let n: usize = counts.iter().sum();
    let nf = n as f32;
    let nd = (n * dim) as f32;
    let used = counts.iter().filter(|&&c| c > 0);
    let weights: f32 = used.clone().map(|&c| c as f32 * (c as f32 / nf).ln()).sum();
    let k = used.count();
    -2.0 * weights + nd * (sse.max(1e-12) / nd).ln() + price * (k * (dim + 1)) as f32 * nf.ln()
}

/// The children `points` split into: k-means for every k from 1 to
/// `max_children`, keeping the k with the best [`bic`]; `None` when that is
/// a single cluster or fewer than two children would each hold a share of
/// the points.
pub fn split_points(
    points: &[&[f32]],
    max_children: usize,
    price: f32,
    rng: &mut Rng,
    checks: &mut u64,
) -> Option<Vec<Vec<f32>>> {
    let n = points.len();
    if n < 4 {
        return None;
    }
    let dim = points[0].len();
    let mut best: Option<(f32, Vec<Vec<f32>>)> = None;
    for k in 1..=max_children.min(n / 2) {
        let (centers, sse, counts) = kmeans(points, k, rng, checks);
        let score = bic(sse, &counts, dim, price);
        if best.as_ref().is_none_or(|(b, _)| score < *b) {
            best = Some((score, centers));
        }
    }
    let (_, centers) = best?;
    if centers.len() < 2 {
        return None;
    }
    let mut counts = vec![0usize; centers.len()];
    for p in points {
        let c = (0..centers.len())
            .min_by(|&a, &b| dist2(p, &centers[a]).total_cmp(&dist2(p, &centers[b])))
            .expect("a center");
        counts[c] += 1;
    }
    *checks += (n * centers.len()) as u64;
    let kept: Vec<Vec<f32>> = centers
        .into_iter()
        .zip(&counts)
        .filter(|&(_, &c)| c as f32 >= MIN_SHARE * n as f32)
        .map(|(center, _)| center)
        .collect();
    (kept.len() >= 2).then_some(kept)
}

/// One node of the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The running mean of every point that has passed through it.
    pub center: Vec<f32>,
    /// How many points have passed through it.
    pub count: u32,
    /// Its depth; the root is 0.
    pub depth: usize,
    /// Its parent; `None` for the root.
    pub parent: Option<usize>,
    /// Its children; empty for a leaf.
    pub children: Vec<usize>,
    /// The points a leaf holds, by id.
    pub members: Vec<u32>,
    /// The members a leaf needs before it next tries to split.
    next_try: u32,
}

impl Node {
    /// A node at `center`, `depth` deep, under `parent`.
    fn new(center: Vec<f32>, depth: usize, parent: Option<usize>, split_at: u32) -> Node {
        Node {
            center,
            count: 0,
            depth,
            parent,
            children: Vec::new(),
            members: Vec::new(),
            next_try: split_at,
        }
    }

    /// Whether it has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// The tree, its leaves' members and its bookkeeping.
#[derive(Clone, Debug)]
pub struct Tree {
    /// What it is tuned by.
    pub params: TreeParams,
    /// Dimensions per point.
    pub dim: usize,
    /// The nodes; 0 is the root.
    pub nodes: Vec<Node>,
    /// The leaf each point is in, by point id.
    pub leaf_of: Vec<u32>,
    /// Its random numbers.
    rng: Rng,
    /// Distances computed to place points on their way down.
    pub place_checks: u64,
    /// Distances computed to split leaves: clustering and moving members.
    pub split_checks: u64,
    /// Points the tree has taken.
    pub points: u64,
    /// Passes over the stored points it took, for a tree built in passes.
    pub passes: u32,
}

impl Tree {
    /// An empty tree: a root at the origin.
    pub fn new(params: TreeParams, dim: usize, seed: u64) -> Tree {
        Tree {
            params,
            dim,
            nodes: vec![Node::new(vec![0.0; dim], 0, None, params.split_at)],
            leaf_of: Vec::new(),
            rng: Rng::new(seed),
            place_checks: 0,
            split_checks: 0,
            points: 0,
            passes: 0,
        }
    }

    /// The point with id `id` in `store` (flat, `dim` per point).
    fn point<'a>(&self, store: &'a [f32], id: u32) -> &'a [f32] {
        &store[id as usize * self.dim..(id as usize + 1) * self.dim]
    }

    /// The child of `node` nearest `x`.
    fn nearest_child(&self, node: usize, x: &[f32]) -> usize {
        *self.nodes[node]
            .children
            .iter()
            .min_by(|&&a, &&b| {
                dist2(&self.nodes[a].center, x).total_cmp(&dist2(&self.nodes[b].center, x))
            })
            .expect("a node with children")
    }

    /// Grows the tree online by point `id` of `store`: down to the nearest
    /// child at every node, updating the centers on the way, into a leaf;
    /// then splits that leaf if it is ready. Returns the leaf it ends in.
    pub fn insert(&mut self, id: u32, store: &[f32]) -> usize {
        let x = self.point(store, id);
        let mut node = 0;
        loop {
            let n = &mut self.nodes[node];
            n.count += 1;
            pull(&mut n.center, x, 1.0 / n.count as f32);
            if n.is_leaf() {
                break;
            }
            self.place_checks += self.nodes[node].children.len() as u64;
            node = self.nearest_child(node, x);
        }
        self.nodes[node].members.push(id);
        if self.leaf_of.len() <= id as usize {
            self.leaf_of.resize(id as usize + 1, 0);
        }
        self.leaf_of[id as usize] = node as u32;
        self.points += 1;
        let leaf = &self.nodes[node];
        if leaf.members.len() as u32 >= leaf.next_try && leaf.depth < self.params.max_depth {
            self.split(node, store);
        }
        self.leaf_of[id as usize] as usize
    }

    /// Splits leaf `node` into the children its members cluster into,
    /// moving each member to its nearest child; with nothing to split into
    /// it stays a leaf and tries again at twice as many members.
    fn split(&mut self, node: usize, store: &[f32]) -> bool {
        let p = self.params;
        let mut sample: Vec<u32> = self.nodes[node].members.clone();
        if sample.len() > SPLIT_SAMPLE {
            for i in 0..SPLIT_SAMPLE {
                let j = i + self.rng.below(sample.len() - i);
                sample.swap(i, j);
            }
            sample.truncate(SPLIT_SAMPLE);
        }
        let points: Vec<&[f32]> = sample.iter().map(|&id| self.point(store, id)).collect();
        let Some(centers) = split_points(
            &points,
            p.max_children,
            p.price,
            &mut self.rng,
            &mut self.split_checks,
        ) else {
            self.nodes[node].next_try *= 2;
            return false;
        };
        let depth = self.nodes[node].depth + 1;
        let first = self.nodes.len();
        for center in centers {
            self.nodes
                .push(Node::new(center, depth, Some(node), p.split_at));
        }
        self.nodes[node].children = (first..self.nodes.len()).collect();
        let members = std::mem::take(&mut self.nodes[node].members);
        self.split_checks += (members.len() * self.nodes[node].children.len()) as u64;
        for id in members {
            let child = self.nearest_child(node, self.point(store, id));
            let c = &mut self.nodes[child];
            c.members.push(id);
            c.count += 1;
            self.leaf_of[id as usize] = child as u32;
        }
        // Children start from their members' mean, not the clustering's sample.
        let dim = self.dim;
        for c in self.nodes[node].children.clone() {
            let child = &mut self.nodes[c];
            if !child.members.is_empty() {
                child.center = weighted_mean(
                    child
                        .members
                        .iter()
                        .map(|&i| (&store[i as usize * dim..(i as usize + 1) * dim], 1.0)),
                    dim,
                );
            }
        }
        true
    }

    /// A tree built in passes over `ids` of `store`, as a batch system would:
    /// every node of a level with enough points clusters them into children
    /// and hands each point to its nearest child; one pass per level.
    pub fn in_passes(
        params: TreeParams,
        dim: usize,
        store: &[f32],
        ids: &[u32],
        seed: u64,
    ) -> Tree {
        let mut tree = Tree::new(params, dim, seed);
        let n = ids.iter().map(|&i| i as usize + 1).max().unwrap_or(0);
        tree.leaf_of = vec![0; n];
        tree.points = ids.len() as u64;
        let root = &mut tree.nodes[0];
        root.members = ids.to_vec();
        root.count = ids.len() as u32;
        root.center = weighted_mean(
            ids.iter()
                .map(|&i| (&store[i as usize * dim..(i as usize + 1) * dim], 1.0)),
            dim,
        );
        let mut level = vec![0usize];
        for _ in 0..params.max_depth {
            let mut next = Vec::new();
            let mut any = false;
            for &node in &level {
                if (tree.nodes[node].members.len() as u32) < params.split_at {
                    continue;
                }
                any = true;
                if tree.split(node, store) {
                    next.extend(tree.nodes[node].children.iter().copied());
                }
            }
            if !any {
                break;
            }
            tree.passes += 1;
            level = next;
        }
        for (i, node) in tree.nodes.iter().enumerate() {
            for &id in &node.members {
                tree.leaf_of[id as usize] = i as u32;
            }
        }
        tree
    }

    /// The ancestor of node `node` at `depth`, or `node` itself if it is
    /// shallower.
    pub fn ancestor(&self, mut node: usize, depth: usize) -> usize {
        while self.nodes[node].depth > depth {
            node = self.nodes[node]
                .parent
                .expect("only the root has no parent");
        }
        node
    }

    /// The leaves, by node index.
    pub fn leaves(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.nodes.len()).filter(|&i| self.nodes[i].is_leaf())
    }

    /// The `k` points nearest `q` found by descending with a beam: at every
    /// level the `beam` nodes nearest `q` are opened, until only leaves are
    /// left; their members are the candidates. Returns the ids, nearest
    /// first, and the distances computed.
    pub fn search(&self, q: &[f32], k: usize, beam: usize, store: &[f32]) -> (Vec<u32>, u64) {
        let mut checks = 0u64;
        let mut frontier: Vec<(usize, f32)> = vec![(0, 0.0)];
        while frontier.iter().any(|&(n, _)| !self.nodes[n].is_leaf()) {
            let mut next = Vec::new();
            for &(n, d) in &frontier {
                if self.nodes[n].is_leaf() {
                    next.push((n, d));
                    continue;
                }
                for &c in &self.nodes[n].children {
                    checks += 1;
                    next.push((c, dist2(&self.nodes[c].center, q)));
                }
            }
            next.sort_by(|a, b| a.1.total_cmp(&b.1));
            next.truncate(beam.max(1));
            frontier = next;
        }
        let mut candidates: Vec<(u32, f32)> = frontier
            .iter()
            .flat_map(|&(n, _)| self.nodes[n].members.iter())
            .map(|&id| {
                checks += 1;
                (id, dist2(self.point(store, id), q))
            })
            .collect();
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
        (
            candidates.into_iter().take(k).map(|(id, _)| id).collect(),
            checks,
        )
    }
}

/// The ids of the `k` points of `ids` nearest `q`, by scanning them all.
pub fn brute_force(q: &[f32], k: usize, ids: &[u32], store: &[f32], dim: usize) -> Vec<u32> {
    let mut all: Vec<(u32, f32)> = ids
        .iter()
        .map(|&id| {
            (
                id,
                dist2(&store[id as usize * dim..(id as usize + 1) * dim], q),
            )
        })
        .collect();
    all.sort_by(|a, b| a.1.total_cmp(&b.1));
    all.into_iter().take(k).map(|(id, _)| id).collect()
}

/// The share of `truth` that `found` holds.
pub fn recall(found: &[u32], truth: &[u32]) -> f32 {
    if truth.is_empty() {
        return 1.0;
    }
    truth.iter().filter(|t| found.contains(t)).count() as f32 / truth.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{EmbedSpec, Embedder};
    use crate::metrics::adjusted_rand;

    /// `n` points of `spec`, with their true labels per level.
    fn data(spec: EmbedSpec, n: usize, seed: u64) -> (Vec<f32>, Vec<[u32; 3]>) {
        let mut e = Embedder::new(spec, seed);
        let mut store = Vec::with_capacity(n * spec.dim);
        let mut labels = Vec::with_capacity(n);
        for _ in 0..n {
            let (x, leaf) = e.next_point();
            store.extend(x);
            labels.push(Embedder::labels(leaf));
        }
        (store, labels)
    }

    fn grow(spec: EmbedSpec, n: usize) -> (Tree, Vec<f32>, Vec<[u32; 3]>) {
        let (store, labels) = data(spec, n, 1);
        let mut tree = Tree::new(TreeParams::default(), spec.dim, 2);
        for id in 0..n as u32 {
            tree.insert(id, &store);
        }
        (tree, store, labels)
    }

    /// The adjusted Rand index of the tree at `depth` against the true
    /// labels of level `depth − 1`.
    fn ari_at(tree: &Tree, labels: &[[u32; 3]], depth: usize) -> f32 {
        let found: Vec<u32> = (0..labels.len())
            .map(|i| tree.ancestor(tree.leaf_of[i] as usize, depth) as u32)
            .collect();
        let truth: Vec<u32> = labels.iter().map(|l| l[depth - 1]).collect();
        adjusted_rand(&found, &truth)
    }

    #[test]
    fn the_criterion_picks_the_number_of_groups_there_are() {
        let mut rng = Rng::new(1);
        let rows: Vec<Vec<f32>> = (0..300)
            .map(|i| {
                let c = [0.2, 0.5, 0.8][i % 3];
                vec![c + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03]
            })
            .collect();
        let points: Vec<&[f32]> = rows.iter().map(|r| &r[..]).collect();
        let mut checks = 0;
        let centers = split_points(&points, 8, 1.0, &mut rng, &mut checks).unwrap();
        assert_eq!(centers.len(), 3);
        assert!(checks > 0);
        let one: Vec<Vec<f32>> = (0..300)
            .map(|_| vec![0.5 + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03])
            .collect();
        let points: Vec<&[f32]> = one.iter().map(|r| &r[..]).collect();
        assert!(
            split_points(&points, 8, 1.0, &mut rng, &mut checks).is_none(),
            "one blob stays whole"
        );
    }

    /// The adjusted Rand index of the tree's leaves against the true leaves.
    fn leaf_ari(tree: &Tree, labels: &[[u32; 3]]) -> f32 {
        let truth: Vec<u32> = labels.iter().map(|l| l[2]).collect();
        adjusted_rand(&tree.leaf_of[..labels.len()], &truth)
    }

    #[test]
    fn online_growth_finds_the_topics_and_the_leaves_in_64_dimensions() {
        let (tree, _, labels) = grow(EmbedSpec::sphere64(), 20_000);
        // The levels between needn't line up with the true ones: the tree may
        // cut six topics into eight nodes and make up for it a level down.
        assert!(
            ari_at(&tree, &labels, 1) > 0.85,
            "topics: {}",
            ari_at(&tree, &labels, 1)
        );
        assert!(
            leaf_ari(&tree, &labels) > 0.85,
            "leaves: {}",
            leaf_ari(&tree, &labels)
        );
        let per_point = tree.place_checks as f32 / tree.points as f32;
        assert!(per_point < 40.0, "{per_point} distances per point");
        for (id, &leaf) in tree.leaf_of.iter().enumerate() {
            assert!(tree.nodes[leaf as usize].members.contains(&(id as u32)));
        }
    }

    #[test]
    fn passes_build_a_tree_as_good_as_online_growth() {
        let (online, store, labels) = grow(EmbedSpec::sphere64(), 20_000);
        let ids: Vec<u32> = (0..20_000).collect();
        let passes = Tree::in_passes(TreeParams::default(), 64, &store, &ids, 2);
        assert!(passes.passes >= 3, "{} passes", passes.passes);
        for (name, tree) in [("online", &online), ("passes", &passes)] {
            assert!(
                ari_at(tree, &labels, 1) > 0.85,
                "{name} topics: {}",
                ari_at(tree, &labels, 1)
            );
            assert!(
                leaf_ari(tree, &labels) > 0.85,
                "{name} leaves: {}",
                leaf_ari(tree, &labels)
            );
        }
    }

    #[test]
    fn search_finds_true_neighbors_for_a_fraction_of_the_distances() {
        let spec = EmbedSpec::sphere64();
        let (tree, store, _) = grow(spec, 20_000);
        let ids: Vec<u32> = (0..20_000).collect();
        let mut e = Embedder::new(spec, 1);
        let queries: Vec<Vec<f32>> = (0..50).map(|_| e.next_point().0).collect();
        let score = |beam: usize| {
            let (mut r, mut c) = (0.0, 0u64);
            for q in &queries {
                let (found, checks) = tree.search(q, 10, beam, &store);
                r += recall(&found, &brute_force(q, 10, &ids, &store, 64));
                c += checks;
            }
            (r / queries.len() as f32, c as f32 / queries.len() as f32)
        };
        let (narrow, wide) = (score(1), score(8));
        assert!(
            narrow.0 > 0.6 && wide.0 >= narrow.0 && wide.0 > 0.95,
            "recall {narrow:?} → {wide:?}"
        );
        assert!(
            narrow.1 < 20_000.0 / 20.0 && wide.1 > narrow.1,
            "distances {narrow:?} → {wide:?}"
        );
    }

    #[test]
    fn the_plane_grows_a_tree_too() {
        let (tree, _, labels) = grow(EmbedSpec::plane(), 10_000);
        assert!(
            ari_at(&tree, &labels, 1) > 0.75,
            "topics: {}",
            ari_at(&tree, &labels, 1)
        );
        assert!(
            leaf_ari(&tree, &labels) > 0.6,
            "leaves: {}",
            leaf_ari(&tree, &labels)
        );
    }
}
