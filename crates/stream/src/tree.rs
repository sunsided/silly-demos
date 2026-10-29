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
//! Left at that, a leaf splits into the finest structure its points
//! support, up to the cap, and in a hierarchy that cuts topics in two: the
//! subtopics below are real clusters too. A careful tree
//! ([`TreeParams::careful`]) chooses the coarsest strong level instead,
//! where the criterion's gains fall off ([`SplitRule::Coarsest`]); splits
//! only once two random halves of a leaf's points agree on that choice,
//! waiting for more points otherwise, in the spirit of ODAC's rule of
//! deciding once the evidence separates the options; and re-checks a few
//! points per point taken, moving those that would now descend elsewhere,
//! in the spirit of SPFresh's LIRE rebalancing. The research behind this
//! is in `docs/research/online-hierarchical-splitting.md`.
//!
//! Or a tree can split quickly and mend what it got wrong
//! ([`TreeParams::repaired`]). Every node keeps the count, the mean and the
//! sum of squared norms of the points through it, so the squared error of
//! any grouping of siblings follows exactly from theirs, without a point.
//! Each time a node's points have doubled it re-decides its children from
//! that alone: siblings that turn out to be one cluster are merged, a split
//! that wasn't needed is flattened, a missing level is put in, and a child
//! that holds what should be siblings has its children lifted among them.
//! Moves between leaves carry the points' statistics along.
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
    /// How a split chooses its number of children.
    pub rule: SplitRule,
    /// k-means runs per number of children; the best is kept.
    pub restarts: usize,
    /// Split only when two random halves of a leaf's points choose the same
    /// number of children; otherwise wait for more points.
    pub confirm: bool,
    /// Points re-checked per point taken: each is descended again and moved
    /// if it would now land in another leaf. 0 re-checks none.
    pub recheck: f32,
    /// Re-decide a node's children whenever its points have doubled since
    /// they were last decided, from the children's statistics alone: merge
    /// children that turn out to be one cluster, or put a level between
    /// children that group into fewer clusters.
    pub repair: bool,
}

/// How a split chooses its number of children k.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitRule {
    /// The k the information criterion likes best, up to the cap: the finest
    /// structure the points support.
    Criterion,
    /// The coarsest strong level: the fewest children that have improved the
    /// criterion and after which one more would add less than this share of
    /// the average improvement per child so far. Where clusters nest, the
    /// gains fall off sharply after the top level, and this stops there; an
    /// early loss (six clusters in a ring don't halve well) doesn't stop it.
    Coarsest(f32),
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
            rule: SplitRule::Criterion,
            restarts: 1,
            confirm: false,
            recheck: 0.0,
            repair: false,
        }
    }
}

impl TreeParams {
    /// The default tree made careful: children up to 10 chosen as the
    /// coarsest strong level (a fifth of the average gain) from the best of
    /// 2 k-means runs, every split confirmed on two halves of the leaf's
    /// points, and half a point re-checked per point taken. On the 64-D
    /// embeddings this finds topics and subtopics at 0.99 and 0.98 where the
    /// default tree finds 0.93 and 0.94, for seven times its splitting cost.
    /// A single run per k is too noisy for the coarsest rule, and more runs
    /// or more children bought nothing.
    pub fn careful() -> TreeParams {
        TreeParams {
            max_children: 10,
            rule: SplitRule::Coarsest(0.2),
            restarts: 2,
            confirm: true,
            recheck: 0.5,
            ..TreeParams::default()
        }
    }

    /// The default tree, quick to split, with repairs: on the 64-D
    /// embeddings it finds topics and subtopics at 1.00 where the default
    /// tree finds 0.94, for its own splitting cost and less than a distance
    /// per point on repairs; the careful tree pays seven times as much
    /// splitting to wait for the evidence instead.
    pub fn repaired() -> TreeParams {
        TreeParams {
            repair: true,
            ..TreeParams::default()
        }
    }
}

/// Points of a leaf that a split clusters at most; a sample beyond that.
const SPLIT_SAMPLE: usize = 2000;
/// Rounds of Lloyd's k-means at most.
const ROUNDS: usize = 15;
/// A child needs at least this share of its parent's points.
const MIN_SHARE: f32 = 0.03;
/// The share of the best gain a coarser level must have gained for a repair
/// to take it: the children's own grouping is the best gain, and a level
/// that holds less than half of it is no reason to undo them. Three pairs of
/// six clusters in a ring gain a little, and four groups lose, so the
/// coarsest rule alone would stop at the pairs.
const STRONG: f32 = 0.5;
/// How much a lifted child's children must gain per child, against the
/// level's average gain per child.
const LIFT: f32 = 0.5;
/// Repairs at one node per check at most.
const REPAIRS: usize = 4;
/// The coarsest rule's share for repairs, for a tree that splits by the
/// criterion: repairs look for the level that tree skips.
const REPAIR_SHARE: f32 = 0.2;

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

/// k-means of `points` weighted by `weights` into `k` groups, as
/// [`kmeans`] does it; returns the group of every point.
fn weighted_kmeans(
    points: &[&[f32]],
    weights: &[f32],
    k: usize,
    rng: &mut Rng,
    checks: &mut u64,
) -> Vec<usize> {
    let n = points.len();
    let k = k.min(n).max(1);
    let dim = points[0].len();
    let mut centers: Vec<Vec<f32>> = vec![points[rng.below(n)].to_vec()];
    let mut nearest: Vec<f32> = points.iter().map(|p| dist2(p, &centers[0])).collect();
    *checks += n as u64;
    while centers.len() < k {
        let total: f32 = nearest.iter().zip(weights).map(|(d, w)| d * w).sum();
        let mut r = rng.next_f32() * total;
        let pick = nearest
            .iter()
            .zip(weights)
            .position(|(d, w)| {
                r -= d * w;
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
    for _ in 0..ROUNDS {
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let c = (0..centers.len())
                .min_by(|&a, &b| dist2(p, &centers[a]).total_cmp(&dist2(p, &centers[b])))
                .expect("a center");
            changed |= assigned[i] != c;
            assigned[i] = c;
        }
        *checks += (n * centers.len()) as u64;
        if !changed {
            break;
        }
        for (c, center) in centers.iter_mut().enumerate() {
            let rows = points
                .iter()
                .zip(weights)
                .zip(&assigned)
                .filter(|&(_, &a)| a == c);
            if rows.clone().any(|_| true) {
                *center = weighted_mean(rows.map(|((p, &w), _)| (&p[..], w)), dim);
            }
        }
    }
    assigned
}

/// The squared error that grouping adds to the points of nodes with
/// `centers` and `counts` (their weights): each node's count times the
/// squared distance from its center to its group's mean.
fn between(centers: &[&[f32]], counts: &[f32], groups: &[usize], k: usize) -> f64 {
    let dim = centers[0].len();
    (0..k)
        .map(|g| {
            let rows = || {
                centers
                    .iter()
                    .zip(counts)
                    .zip(groups)
                    .filter(move |&(_, &a)| a == g)
            };
            let mean = weighted_mean(rows().map(|((c, &w), _)| (&c[..], w)), dim);
            rows()
                .map(|((c, &w), _)| w as f64 * dist2(c, &mean) as f64)
                .sum::<f64>()
        })
        .sum()
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

/// The best of `restarts` k-means runs of `points` for every k from 1 to
/// `max_k`, with its [`bic`]: (score, centers) per k.
fn candidates(
    points: &[&[f32]],
    max_k: usize,
    p: &TreeParams,
    rng: &mut Rng,
    checks: &mut u64,
) -> Vec<(f32, Vec<Vec<f32>>)> {
    let dim = points[0].len();
    (1..=max_k)
        .map(|k| {
            (0..p.restarts.max(1))
                .map(|_| {
                    let (centers, sse, counts) = kmeans(points, k, rng, checks);
                    (bic(sse, &counts, dim, p.price), centers)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("at least one run")
        })
        .collect()
}

/// The number of children `rule` picks from the scores of k = 1, 2, …; the
/// coarsest rule only stops at a k that has gained at least `strong` of the
/// best gain.
fn pick(scores: &[f32], rule: SplitRule, strong: f32) -> usize {
    match rule {
        SplitRule::Criterion => scores
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map_or(1, |(i, _)| i + 1),
        SplitRule::Coarsest(share) => {
            // The first k that has gained `strong` of the best gain and
            // whose next child would add less than `share` of the average
            // gain per child so far.
            let k_max = scores.len();
            let best = scores.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            for k in 2..=k_max {
                let gained = scores[0] - scores[k - 1];
                if gained <= 0.0 || gained < strong * (scores[0] - best) {
                    continue;
                }
                let next = if k < k_max {
                    scores[k - 1] - scores[k]
                } else {
                    f32::NEG_INFINITY
                };
                if next < share * gained / (k - 1) as f32 {
                    return k;
                }
            }
            1
        }
    }
}

/// The children `points` split into: k-means for every k up to
/// `max_children`, the number of children chosen by the tree's
/// [`SplitRule`]; `None` when that is one, or when fewer than two children
/// would each hold a share of the points.
pub fn split_points(
    points: &[&[f32]],
    p: &TreeParams,
    rng: &mut Rng,
    checks: &mut u64,
) -> Option<Vec<Vec<f32>>> {
    let n = points.len();
    if n < 4 {
        return None;
    }
    let mut runs = candidates(points, p.max_children.min(n / 2), p, rng, checks);
    let scores: Vec<f32> = runs.iter().map(|r| r.0).collect();
    let k = pick(&scores, p.rule, 0.0);
    let centers = std::mem::take(&mut runs[k - 1].1);
    if centers.len() < 2 {
        return None;
    }
    let mut counts = vec![0usize; centers.len()];
    for x in points {
        let c = (0..centers.len())
            .min_by(|&a, &b| dist2(x, &centers[a]).total_cmp(&dist2(x, &centers[b])))
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
    /// The sum of the squared norms of the points through it: with the
    /// count and the center, their squared error about the center.
    sq: f64,
    /// Its count when its children were last decided; 0 for a leaf.
    settled: u32,
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
            sq: 0.0,
            settled: 0,
        }
    }

    /// Whether it has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// The squared error of its points about its center.
    fn within(&self) -> f64 {
        let norm: f64 = self.center.iter().map(|&c| (c as f64) * (c as f64)).sum();
        (self.sq - self.count as f64 * norm).max(0.0)
    }

    /// Takes in point `x`: the center stays the mean of the points through it.
    fn add(&mut self, x: &[f32]) {
        self.count += 1;
        pull(&mut self.center, x, 1.0 / self.count as f32);
        self.sq += norm2(x);
    }

    /// Lets point `x` go, the inverse of [`Node::add`].
    fn remove(&mut self, x: &[f32]) {
        if self.count <= 1 {
            self.count = 0;
            self.sq = 0.0;
            return;
        }
        let n = self.count as f32;
        for (c, &v) in self.center.iter_mut().zip(x) {
            *c = (n * *c - v) / (n - 1.0);
        }
        self.count -= 1;
        self.sq = (self.sq - norm2(x)).max(0.0);
    }
}

/// The squared norm of `x`, in double precision for running sums.
fn norm2(x: &[f32]) -> f64 {
    x.iter().map(|&v| (v as f64) * (v as f64)).sum()
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
    /// The repairs' own random numbers, so that checking for a repair that
    /// changes nothing leaves the tree as it would have grown without.
    repair_rng: Rng,
    /// Distances computed to place points on their way down.
    pub place_checks: u64,
    /// Distances computed to split leaves: clustering and moving members.
    pub split_checks: u64,
    /// Points the tree has taken.
    pub points: u64,
    /// Passes over the stored points it took, for a tree built in passes.
    pub passes: u32,
    /// Splits put off because the two halves of a leaf disagreed.
    pub deferred: u64,
    /// Points moved by re-checking, and the distances that took.
    pub moved: u64,
    /// Distances computed re-checking points.
    pub recheck_checks: u64,
    /// Repairs that changed the tree: children merged, flattened or grouped
    /// under a new level.
    pub repairs: u64,
    /// Distances computed deciding repairs.
    pub repair_checks: u64,
    /// Re-checks owed: grows by `recheck` per point taken.
    debt: f32,
    /// The next point id to re-check.
    cursor: u32,
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
            repair_rng: Rng::new(seed ^ 0x5eed),
            place_checks: 0,
            split_checks: 0,
            points: 0,
            passes: 0,
            deferred: 0,
            moved: 0,
            recheck_checks: 0,
            repairs: 0,
            repair_checks: 0,
            debt: 0.0,
            cursor: 0,
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
        let mut path = Vec::new();
        loop {
            self.nodes[node].add(x);
            if self.nodes[node].is_leaf() {
                break;
            }
            path.push(node);
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
        if self.params.recheck > 0.0 {
            self.recheck(store);
        }
        if self.params.repair {
            // The shallowest node on the way down whose points have doubled.
            let due = path.into_iter().find(|&n| {
                let n = &self.nodes[n];
                !n.is_leaf() && n.count >= 2 * n.settled.max(1)
            });
            if let Some(node) = due {
                self.repair(node, store);
            }
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
        if p.confirm && !self.halves_agree(&points) {
            // Not enough evidence yet: try again with half as many points more.
            let leaf = &mut self.nodes[node];
            leaf.next_try += leaf.next_try / 2;
            self.deferred += 1;
            return false;
        }
        let Some(centers) = split_points(&points, &p, &mut self.rng, &mut self.split_checks) else {
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
        self.nodes[node].settled = self.nodes[node].count;
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
                let rows = || {
                    child
                        .members
                        .iter()
                        .map(|&i| &store[i as usize * dim..(i as usize + 1) * dim])
                };
                let center = weighted_mean(rows().map(|r| (r, 1.0)), dim);
                let sq = rows().map(norm2).sum();
                child.center = center;
                child.sq = sq;
            }
        }
        true
    }

    /// Whether two random halves of `points` split into the same number of
    /// children (or both into none): the evidence a careful split waits for.
    fn halves_agree(&mut self, points: &[&[f32]]) -> bool {
        let mut order: Vec<usize> = (0..points.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, self.rng.below(i + 1));
        }
        let half = order.len() / 2;
        let p = self.params;
        let count = |ids: &[usize], tree: &mut Tree| {
            let part: Vec<&[f32]> = ids.iter().map(|&i| points[i]).collect();
            split_points(&part, &p, &mut tree.rng, &mut tree.split_checks).map_or(1, |c| c.len())
        };
        let a = count(&order[..half], self);
        let b = count(&order[half..], self);
        a == b
    }

    /// Re-checks the points owed: descends each again from the root and
    /// moves it if it lands in another leaf than the one it is in, as
    /// centers have moved since it was placed. A move takes the point out
    /// of the nodes above its old leaf and into those above its new one, up
    /// to where the two paths meet.
    fn recheck(&mut self, store: &[f32]) {
        self.debt += self.params.recheck;
        let n = self.leaf_of.len() as u32;
        while self.debt >= 1.0 && n > 0 {
            self.debt -= 1.0;
            let id = self.cursor % n;
            self.cursor = self.cursor.wrapping_add(1);
            let x = self.point(store, id);
            let mut node = 0;
            while !self.nodes[node].is_leaf() {
                self.recheck_checks += self.nodes[node].children.len() as u64;
                node = self.nearest_child(node, x);
            }
            let from = self.leaf_of[id as usize] as usize;
            if node != from {
                let members = &mut self.nodes[from].members;
                if let Some(at) = members.iter().position(|&m| m == id) {
                    members.swap_remove(at);
                }
                let above: Vec<usize> = self.path_up(from).collect();
                let meet = self
                    .path_up(node)
                    .find(|n| above.contains(n))
                    .expect("the root is above both");
                for &n in above.iter().take_while(|&&n| n != meet) {
                    self.nodes[n].remove(x);
                }
                let mut n = node;
                while n != meet {
                    self.nodes[n].add(x);
                    n = self.nodes[n].parent.expect("below the meeting node");
                }
                self.nodes[node].members.push(id);
                self.leaf_of[id as usize] = node as u32;
                self.moved += 1;
            }
        }
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
        root.sq = ids
            .iter()
            .map(|&i| norm2(&store[i as usize * dim..(i as usize + 1) * dim]))
            .sum();
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

    /// Node `node` and its ancestors up to the root, deepest first.
    fn path_up(&self, node: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(node), |&n| self.nodes[n].parent)
    }

    /// Re-decides the children of node `node` from their statistics alone,
    /// regrouping them ([`Tree::regroup`]) or lifting one's children among
    /// them ([`Tree::lift`]) until neither changes anything. Returns the
    /// repairs made.
    fn repair(&mut self, mut node: usize, store: &[f32]) -> u32 {
        self.nodes[node].settled = self.nodes[node].count;
        let mut made = 0;
        // Each repair leaves fewer nodes or a shallower tree; a few suffice.
        for _ in 0..REPAIRS {
            let Some(at) = self.regroup(node, store).or_else(|| self.lift(node)) else {
                break;
            };
            node = at;
            made += 1;
        }
        self.repairs += made as u64;
        made
    }

    /// The [`bic`] of nodes `parts` taken as one level of clusters, from
    /// their counts and squared errors.
    fn level_score(&self, parts: &[usize]) -> f32 {
        let sse: f64 = parts.iter().map(|&c| self.nodes[c].within()).sum();
        let counts: Vec<usize> = parts
            .iter()
            .map(|&c| self.nodes[c].count as usize)
            .collect();
        bic(sse as f32, &counts, self.dim, self.params.price)
    }

    /// The share the coarsest rule takes for repairs: the tree's own, or
    /// [`REPAIR_SHARE`] for a tree that splits by the criterion.
    fn repair_share(&self) -> f32 {
        match self.params.rule {
            SplitRule::Coarsest(share) => share,
            SplitRule::Criterion => REPAIR_SHARE,
        }
    }

    /// Groups the children of node `node` by weighted k-means on their
    /// centers for every number of groups, scores each grouping by the
    /// [`bic`] of the points the groups would hold (exact, from the
    /// children's counts, centers and squared errors), and takes the
    /// coarsest strong level. If that is fewer groups than children, the
    /// tree was split too early or too finely here:
    ///
    /// * one group: the split was not needed, and the children are
    ///   flattened into the node;
    /// * some children standing alone: the others were cut too finely, and
    ///   each group of several is merged into one child that takes over
    ///   their children (or members), keeping the levels even;
    /// * every group of several: a level is missing, and one is put between
    ///   the node and its children.
    ///
    /// Returns the node's index after the change, if it made one.
    fn regroup(&mut self, node: usize, store: &[f32]) -> Option<usize> {
        let children = self.nodes[node].children.clone();
        let m = children.len();
        if m < 2 {
            return None;
        }
        let centers: Vec<&[f32]> = children
            .iter()
            .map(|&c| &self.nodes[c].center[..])
            .collect();
        let weights: Vec<f32> = children
            .iter()
            .map(|&c| self.nodes[c].count as f32)
            .collect();
        let within: f64 = children.iter().map(|&c| self.nodes[c].within()).sum();
        let mut groupings = Vec::with_capacity(m);
        let mut scores = Vec::with_capacity(m);
        for k in 1..=m {
            let groups = if k == 1 {
                vec![0; m]
            } else if k == m {
                (0..m).collect()
            } else {
                (0..self.params.restarts.max(2))
                    .map(|_| {
                        let g = weighted_kmeans(
                            &centers,
                            &weights,
                            k,
                            &mut self.repair_rng,
                            &mut self.repair_checks,
                        );
                        (between(&centers, &weights, &g, k), g)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .expect("at least one run")
                    .1
            };
            let sse = within + between(&centers, &weights, &groups, k);
            let mut counts = vec![0usize; k];
            for (&g, &w) in groups.iter().zip(&weights) {
                counts[g] += w as usize;
            }
            scores.push(bic(sse as f32, &counts, self.dim, self.params.price));
            groupings.push(groups);
        }
        let k = pick(&scores, SplitRule::Coarsest(self.repair_share()), STRONG);
        let mut groups: Vec<Vec<usize>> = vec![Vec::new(); k];
        for (&c, &g) in children.iter().zip(&groupings[k - 1]) {
            groups[g].push(c);
        }
        groups.retain(|g| !g.is_empty());
        if groups.len() == m {
            return None;
        }
        if groups.len() == 1 {
            self.nodes[node].children.clear();
            self.absorb(node, &children, store);
        } else {
            let alone = groups.iter().any(|g| g.len() == 1);
            let mut kept = Vec::with_capacity(groups.len());
            for group in groups {
                if group.len() == 1 {
                    kept.push(group[0]);
                    continue;
                }
                let joined = self.join(node, &group);
                if alone {
                    self.absorb(joined, &group, store);
                } else {
                    for &c in &group {
                        self.nodes[c].parent = Some(joined);
                    }
                    self.nodes[joined].children = group;
                }
                kept.push(joined);
            }
            self.nodes[node].children = kept;
        }
        Some(self.compact()[node])
    }

    /// Lifts the children of one child of node `node` up among its
    /// siblings, if that child holds what should be siblings: a split that
    /// took two clusters for one, its children the two. Lifting a child
    /// changes the level's [`bic`] by an exact amount; it is taken if the
    /// gain per child it adds is at least [`LIFT`] times the level's
    /// average gain per child, where the children of a true cluster add a
    /// tenth of it at most. The child that gains most is lifted.
    ///
    /// Returns the node's index after the change, if it made one.
    fn lift(&mut self, node: usize) -> Option<usize> {
        let children = self.nodes[node].children.clone();
        let m = children.len();
        if m < 2 {
            return None;
        }
        let parent = &self.nodes[node];
        let one = bic(
            parent.within() as f32,
            &[parent.count as usize],
            self.dim,
            self.params.price,
        );
        let now = self.level_score(&children);
        let average = (one - now) / (m - 1) as f32;
        if average <= 0.0 {
            return None;
        }
        let (best, ratio) = children
            .iter()
            .filter(|&&c| self.nodes[c].children.len() >= 2)
            .map(|&c| {
                let kids = &self.nodes[c].children;
                let mut parts: Vec<usize> = children.iter().copied().filter(|&o| o != c).collect();
                parts.extend(kids);
                let gain = (now - self.level_score(&parts)) / (kids.len() - 1) as f32;
                (c, gain / average)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))?;
        if ratio < LIFT {
            return None;
        }
        let kids = std::mem::take(&mut self.nodes[best].children);
        for &g in &kids {
            self.nodes[g].parent = Some(node);
        }
        let n = &mut self.nodes[node];
        n.children.retain(|&c| c != best);
        n.children.extend(kids);
        Some(self.compact()[node])
    }

    /// A new child of `node` holding the points of nodes `group`: their
    /// summed counts and squared norms, and the mean of their centers.
    fn join(&mut self, node: usize, group: &[usize]) -> usize {
        let dim = self.dim;
        let center = weighted_mean(
            group
                .iter()
                .map(|&c| (&self.nodes[c].center[..], self.nodes[c].count as f32)),
            dim,
        );
        let mut joined = Node::new(
            center,
            self.nodes[node].depth + 1,
            Some(node),
            self.params.split_at,
        );
        joined.count = group.iter().map(|&c| self.nodes[c].count).sum();
        joined.sq = group.iter().map(|&c| self.nodes[c].sq).sum();
        joined.settled = joined.count;
        self.nodes.push(joined);
        self.nodes.len() - 1
    }

    /// Makes node `target`, its statistics already those of `sources`'
    /// points, take over their children and members in their stead. Where
    /// both come together, the members descend to the leaves below.
    fn absorb(&mut self, target: usize, sources: &[usize], store: &[f32]) {
        let mut children = Vec::new();
        let mut members = Vec::new();
        for &s in sources {
            children.append(&mut self.nodes[s].children);
            members.append(&mut self.nodes[s].members);
        }
        for &c in &children {
            self.nodes[c].parent = Some(target);
        }
        let t = &mut self.nodes[target];
        t.children = children;
        if t.is_leaf() {
            // Split again once the evidence for it has doubled.
            t.next_try = (2 * members.len() as u32).max(self.params.split_at);
            t.settled = 0;
            for &id in &members {
                self.leaf_of[id as usize] = target as u32;
            }
            self.nodes[target].members = members;
            return;
        }
        t.settled = t.count;
        for id in members {
            let x = self.point(store, id);
            let mut node = target;
            while !self.nodes[node].is_leaf() {
                self.repair_checks += self.nodes[node].children.len() as u64;
                node = self.nearest_child(node, x);
                self.nodes[node].add(x);
            }
            self.nodes[node].members.push(id);
            self.leaf_of[id as usize] = node as u32;
        }
    }

    /// Drops the nodes no longer in the tree and renumbers the rest in
    /// breadth-first order, depths and leaves anew. Returns every old
    /// index's new one, `usize::MAX` for the dropped.
    fn compact(&mut self) -> Vec<usize> {
        let mut order = vec![0usize];
        let mut i = 0;
        while i < order.len() {
            order.extend(self.nodes[order[i]].children.iter().copied());
            i += 1;
        }
        let mut index = vec![usize::MAX; self.nodes.len()];
        for (new, &old) in order.iter().enumerate() {
            index[old] = new;
        }
        let mut old = std::mem::take(&mut self.nodes);
        self.nodes = order
            .iter()
            .map(|&o| {
                let mut n = std::mem::replace(&mut old[o], Node::new(Vec::new(), 0, None, 0));
                n.parent = n.parent.map(|p| index[p]);
                n.children.iter_mut().for_each(|c| *c = index[*c]);
                n
            })
            .collect();
        for i in 0..self.nodes.len() {
            self.nodes[i].depth = self.nodes[i].parent.map_or(0, |p| self.nodes[p].depth + 1);
            for &id in &self.nodes[i].members {
                self.leaf_of[id as usize] = i as u32;
            }
        }
        index
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
        self.search_seen(q, k, beam, store, &mut Vec::new())
    }

    /// [`Tree::search`], also putting the candidates it compared `q` with
    /// in `seen`.
    pub fn search_seen(
        &self,
        q: &[f32],
        k: usize,
        beam: usize,
        store: &[f32],
        seen: &mut Vec<u32>,
    ) -> (Vec<u32>, u64) {
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
                seen.push(id);
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
        let centers = split_points(&points, &TreeParams::default(), &mut rng, &mut checks).unwrap();
        assert_eq!(centers.len(), 3);
        assert!(checks > 0);
        let one: Vec<Vec<f32>> = (0..300)
            .map(|_| vec![0.5 + rng.normal() * 0.03, 0.5 + rng.normal() * 0.03])
            .collect();
        let points: Vec<&[f32]> = one.iter().map(|r| &r[..]).collect();
        assert!(
            split_points(&points, &TreeParams::default(), &mut rng, &mut checks).is_none(),
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
    fn the_coarsest_rule_stops_where_the_gains_fall_off() {
        // Gains of 50, 40, 45, then 3: the level ends at k = 4.
        let scores = [0.0, -50.0, -90.0, -135.0, -138.0, -141.0];
        assert_eq!(pick(&scores, SplitRule::Coarsest(0.2), 0.0), 4);
        assert_eq!(pick(&scores, SplitRule::Criterion, 0.0), 6);
        assert_eq!(
            pick(&[0.0, 5.0], SplitRule::Coarsest(0.2), 0.0),
            1,
            "no gain, no split"
        );
        // Halving a ring of six loses; going on to six gains.
        let ring = [0.0, 93.0, -14.0, -40.0, -127.0, -295.0, -262.0, -230.0];
        assert_eq!(pick(&ring, SplitRule::Coarsest(0.2), 0.0), 6);
        // Three pairs of the ring gain a little, four groups lose: too weak
        // a level for a repair, against what six gain.
        let pairs = [-8435.0, -7918.0, -8498.0, -7995.0, -9013.0, -9940.0];
        assert_eq!(pick(&pairs, SplitRule::Coarsest(0.2), 0.0), 3);
        assert_eq!(pick(&pairs, SplitRule::Coarsest(0.2), STRONG), 6);
    }

    #[test]
    fn a_careful_tree_finds_the_topic_level() {
        let (mut plain, mut careful) = (0.0, 0.0);
        for seed in 1..=3 {
            let (store, labels) = data(EmbedSpec::sphere64(), 20_000, seed);
            let mut a = Tree::new(TreeParams::default(), 64, seed);
            let mut b = Tree::new(TreeParams::careful(), 64, seed);
            for id in 0..20_000 {
                a.insert(id, &store);
                b.insert(id, &store);
            }
            plain += ari_at(&a, &labels, 1) / 3.0;
            careful += ari_at(&b, &labels, 1) / 3.0;
            assert!(
                leaf_ari(&b, &labels) > 0.85,
                "seed {seed}: leaves {}",
                leaf_ari(&b, &labels)
            );
            assert!(b.deferred > 0, "some splits waited for evidence");
            assert!(b.moved > 0 && b.recheck_checks > 0);
            check(&b, &store);
            for (id, &leaf) in b.leaf_of.iter().enumerate() {
                assert!(
                    b.nodes[leaf as usize].members.contains(&(id as u32)),
                    "point {id} where it says"
                );
            }
        }
        assert!(
            careful > plain && careful > 0.97,
            "topics: plain {plain}, careful {careful}"
        );
    }

    #[test]
    fn repair_mends_a_tree_split_in_haste() {
        // Splitting from 100 points, one tree takes two topics for one and
        // cuts others in two; the other waits for evidence and still takes
        // two for one. Repairs merge the halves and lift the pairs apart.
        for (name, base) in [
            ("plain", TreeParams::default()),
            ("careful", TreeParams::careful()),
        ] {
            let (mut hasty, mut mended) = ([0.0; 2], [0.0; 2]);
            for seed in 1..=3 {
                let (store, labels) = data(EmbedSpec::sphere64(), 20_000, seed);
                let p = TreeParams {
                    split_at: 100,
                    ..base
                };
                let mut a = Tree::new(p, 64, seed);
                let mut b = Tree::new(TreeParams { repair: true, ..p }, 64, seed);
                for id in 0..20_000 {
                    a.insert(id, &store);
                    b.insert(id, &store);
                }
                for d in 0..2 {
                    hasty[d] += ari_at(&a, &labels, d + 1) / 3.0;
                    mended[d] += ari_at(&b, &labels, d + 1) / 3.0;
                }
                assert!(b.repairs > 0, "{name} seed {seed}: no repairs");
                assert!(
                    (b.repair_checks as f32) < 5.0 * 20_000.0,
                    "{name} seed {seed}: {} distances repairing",
                    b.repair_checks
                );
                assert!(
                    leaf_ari(&b, &labels) > 0.9,
                    "{name} seed {seed}: leaves {}",
                    leaf_ari(&b, &labels)
                );
                check(&b, &store);
            }
            for d in 0..2 {
                assert!(
                    mended[d] > 0.98 && mended[d] > hasty[d],
                    "{name} depth {}: {} hasty, {} repaired",
                    d + 1,
                    hasty[d],
                    mended[d]
                );
            }
        }
    }

    #[test]
    fn repair_mends_the_plane_too() {
        let (store, labels) = data(EmbedSpec::plane(), 10_000, 1);
        let p = TreeParams {
            split_at: 50,
            ..TreeParams::repaired()
        };
        let mut t = Tree::new(p, 2, 1);
        for id in 0..10_000 {
            t.insert(id, &store);
        }
        check(&t, &store);
        for d in 1..=3 {
            assert!(
                ari_at(&t, &labels, d) > 0.98,
                "depth {d}: {}",
                ari_at(&t, &labels, d)
            );
        }
    }

    #[test]
    fn a_grouping_s_error_follows_from_its_parts() {
        // The squared error of the root's points is that of its children's
        // points about their centers plus what grouping them all adds.
        let (tree, _, _) = grow(EmbedSpec::plane(), 5_000);
        let kids = &tree.nodes[0].children;
        let centers: Vec<&[f32]> = kids.iter().map(|&c| &tree.nodes[c].center[..]).collect();
        let counts: Vec<f32> = kids.iter().map(|&c| tree.nodes[c].count as f32).collect();
        let parts: f64 = kids.iter().map(|&c| tree.nodes[c].within()).sum();
        let all = parts + between(&centers, &counts, &vec![0; kids.len()], 1);
        let root = tree.nodes[0].within();
        assert!((all - root).abs() < 1e-3 * root, "{all} against {root}");
        assert!(
            between(
                &centers,
                &counts,
                &(0..kids.len()).collect::<Vec<_>>(),
                kids.len()
            ) < 1e-9
        );
    }

    /// The tree's bookkeeping holds: every point where its leaf says, every
    /// node's count its children's sum, its center the mean of the points
    /// below it and its squared norms theirs.
    fn check(t: &Tree, store: &[f32]) {
        for (id, &leaf) in t.leaf_of.iter().enumerate() {
            assert!(
                t.nodes[leaf as usize].members.contains(&(id as u32)),
                "point {id}"
            );
        }
        for (i, n) in t.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(t.nodes[p].children.contains(&i));
                assert_eq!(n.depth, t.nodes[p].depth + 1);
            }
            if n.is_leaf() {
                assert_eq!(n.count as usize, n.members.len(), "leaf {i}");
            } else {
                let sum: u32 = n.children.iter().map(|&c| t.nodes[c].count).sum();
                assert_eq!(n.count, sum, "node {i}");
            }
        }
        // Every point's sums, added to each node on its way up from its leaf.
        let mut sums = vec![vec![0.0f64; t.dim]; t.nodes.len()];
        let mut sq = vec![0.0f64; t.nodes.len()];
        for (id, &leaf) in t.leaf_of.iter().enumerate() {
            let x = t.point(store, id as u32);
            for n in t.path_up(leaf as usize) {
                sums[n].iter_mut().zip(x).for_each(|(s, &v)| *s += v as f64);
                sq[n] += norm2(x);
            }
        }
        for (i, n) in t.nodes.iter().enumerate() {
            if n.count == 0 {
                continue;
            }
            for (c, s) in n.center.iter().zip(&sums[i]) {
                let mean = s / n.count as f64;
                assert!(
                    (*c as f64 - mean).abs() < 1e-3,
                    "node {i}: center {c} against mean {mean}"
                );
            }
            assert!(
                (n.sq - sq[i]).abs() < 1e-3 * sq[i].max(1.0),
                "node {i}: squared norms {} against {}",
                n.sq,
                sq[i]
            );
        }
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
