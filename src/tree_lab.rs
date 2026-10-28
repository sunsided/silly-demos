//! WASM bindings for the clustering lab's hierarchy mode (`stream::tree`):
//! mock embeddings streamed into a tree of clusters that grows online, the
//! same points built into a tree in passes, both scored against the true
//! hierarchy and searched for nearest neighbors.

use stream::embed::{BRANCHING, EmbedSpec, Embedder, LEVELS, Projection};
use stream::metrics::adjusted_rand;
use stream::rng::Rng;
use stream::tree::{Tree, TreeParams, brute_force, recall};
use wasm_bindgen::prelude::*;

/// Points the projection is fitted on.
const PROJECTION_SAMPLE: usize = 1500;
/// Queries the search is measured with.
const QUERIES: usize = 100;
/// Neighbors searched for.
const K: usize = 10;
/// Beam widths the search is measured at.
const BEAMS: [usize; 4] = [1, 2, 4, 8];
/// Values per node in [`TreeLab::nodes`].
const NODE_STRIDE: usize = 8;

/// A tunable parameter and its range. All start the tree over.
struct Param {
    /// The id `set_param` takes.
    id: &'static str,
    /// What the page calls it.
    label: &'static str,
    /// Its default.
    default: f32,
    /// The least it may be.
    min: f32,
    /// The most it may be.
    max: f32,
    /// A slider step that lands on the default from the minimum.
    step: f32,
}

/// Every tunable parameter: the stream's, then [`TreeParams`]' defaults.
const PARAMS: [Param; 6] = [
    Param {
        id: "points",
        label: "Points",
        default: 20_000.0,
        min: 2000.0,
        max: 40_000.0,
        step: 1000.0,
    },
    Param {
        id: "noise",
        label: "Noise, 64-D",
        default: 0.06,
        min: 0.02,
        max: 0.1,
        step: 0.005,
    },
    Param {
        id: "split_at",
        label: "Points before a leaf splits",
        default: 300.0,
        min: 50.0,
        max: 2000.0,
        step: 50.0,
    },
    Param {
        id: "max_children",
        label: "Children at most",
        default: 8.0,
        min: 2.0,
        max: 12.0,
        step: 1.0,
    },
    Param {
        id: "max_depth",
        label: "Levels at most",
        default: 4.0,
        min: 1.0,
        max: 6.0,
        step: 1.0,
    },
    Param {
        id: "price",
        label: "Price of a child (× the criterion's)",
        default: 1.0,
        min: 0.25,
        max: 4.0,
        step: 0.25,
    },
];

/// Which tree a call is about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    /// Grown online as the points arrived.
    Online,
    /// Built in passes over the stored points.
    Passes,
}

impl Which {
    /// The tree for a UI id: "online" or "passes".
    fn parse(id: &str) -> Option<Which> {
        match id {
            "online" => Some(Which::Online),
            "passes" => Some(Which::Passes),
            _ => None,
        }
    }
}

/// A stream of labeled embeddings, a tree grown from it online and one
/// built from the same points in passes.
#[wasm_bindgen]
pub struct TreeLab {
    /// "plane" or "sphere64".
    space: &'static str,
    /// The seed every restart starts its random numbers from.
    seed: u64,
    /// The generator of the stream.
    embedder: Embedder,
    /// Every point so far, flat.
    store: Vec<f32>,
    /// The leaf each point came from.
    leaves: Vec<u32>,
    /// Every point projected to 2D, flat.
    projected: Vec<f32>,
    /// The drawing projection.
    projection: Projection,
    /// The tree grown online.
    online: Tree,
    /// The tree built in passes, once there is one.
    passes: Option<Tree>,
    /// Queries for the search, generated with the stream's tree.
    queries: Vec<Vec<f32>>,
    /// The parameter values, ordered as [`PARAMS`].
    values: Vec<f32>,
}

#[wasm_bindgen]
impl TreeLab {
    /// 20 000 points of the 64-dimensional embeddings, default tree.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let spec = EmbedSpec::sphere64();
        let mut lab = Self {
            space: "sphere64",
            seed: seed as u64,
            embedder: Embedder::new(spec, seed as u64),
            store: Vec::new(),
            leaves: Vec::new(),
            projected: Vec::new(),
            projection: Projection::fit(&[], 2, &mut Rng::new(1)),
            online: Tree::new(TreeParams::default(), spec.dim, seed as u64),
            passes: None,
            queries: Vec::new(),
            values: PARAMS.iter().map(|p| p.default).collect(),
        };
        lab.restart();
        lab
    }

    /// "plane" (the tree in 2D) or "sphere64" (64-dimensional unit vectors).
    /// Starts over. `false`, changing nothing, for another id.
    pub fn set_space(&mut self, id: &str) -> bool {
        self.space = match id {
            "plane" => "plane",
            "sphere64" => "sphere64",
            _ => return false,
        };
        self.restart();
        true
    }

    /// Starts over with other random numbers.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// The parameters as JSON: `[{ "id", "label", "value", "default",
    /// "min", "max", "step" }]`.
    pub fn params(&self) -> String {
        let rows: Vec<String> = PARAMS
            .iter()
            .zip(&self.values)
            .map(|(p, v)| {
                format!(
                    r#"{{"id":"{}","label":"{}","value":{v},"default":{},"min":{},"max":{},"step":{}}}"#,
                    p.id, p.label, p.default, p.min, p.max, p.step
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }

    /// Sets a parameter and starts over. `false`, changing nothing, for an
    /// unknown id or a value outside its range.
    pub fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(k) = PARAMS.iter().position(|p| p.id == id) else {
            return false;
        };
        let p = &PARAMS[k];
        if !(value.is_finite() && (p.min..=p.max).contains(&value)) {
            return false;
        }
        self.values[k] = value;
        self.restart();
        true
    }

    /// Back to no points and empty trees, the projection fitted afresh.
    pub fn restart(&mut self) {
        let spec = self.spec();
        self.embedder = Embedder::new(spec, self.seed);
        let mut sampler = self.embedder.clone();
        let sample: Vec<f32> = (0..PROJECTION_SAMPLE)
            .flat_map(|_| sampler.next_point().0)
            .collect();
        self.projection = Projection::fit(&sample, spec.dim, &mut Rng::new(self.seed ^ 0xa5a5));
        self.queries = (0..QUERIES).map(|_| sampler.next_point().0).collect();
        self.store.clear();
        self.leaves.clear();
        self.projected.clear();
        self.online = Tree::new(self.tree_params(), spec.dim, self.seed ^ 0x1234);
        self.passes = None;
    }

    /// Streams up to `n` more points into the online tree, stopping at the
    /// set number; once there, builds the tree in passes. Returns how many.
    pub fn step(&mut self, n: u32) -> u32 {
        let total = self.value("points") as usize;
        let mut done = 0;
        while done < n && self.leaves.len() < total {
            let (x, leaf) = self.embedder.next_point();
            let id = self.leaves.len() as u32;
            self.projected.extend(self.projection.apply(&x));
            self.store.extend(x);
            self.leaves.push(leaf as u32);
            self.online.insert(id, &self.store);
            done += 1;
        }
        if self.leaves.len() >= total && self.passes.is_none() {
            self.build_passes();
        }
        done
    }

    /// Builds the tree in passes over every point so far.
    pub fn build_passes(&mut self) {
        let ids: Vec<u32> = (0..self.leaves.len() as u32).collect();
        if ids.is_empty() {
            return;
        }
        self.passes = Some(Tree::in_passes(
            self.tree_params(),
            self.spec().dim,
            &self.store,
            &ids,
            self.seed ^ 0x4321,
        ));
    }

    /// Whether the tree in passes has been built.
    pub fn has_passes(&self) -> bool {
        self.passes.is_some()
    }

    /// Every point so far, projected: x, y in the unit square.
    pub fn projected(&self) -> Vec<f32> {
        self.projected.clone()
    }

    /// Every point's true label at `level` (0 topics, 1 subtopics, 2 leaves).
    pub fn true_labels(&self, level: u32) -> Vec<u32> {
        let level = (level as usize).min(LEVELS - 1);
        self.leaves
            .iter()
            .map(|&l| Embedder::labels(l as usize)[level])
            .collect()
    }

    /// For each point, the node of tree `which` ("online" or "passes") it is
    /// in at `depth` (its leaf where that is shallower); empty for an
    /// unknown tree or one not built yet.
    pub fn assignments(&self, which: &str, depth: u32) -> Vec<u32> {
        let Some(tree) = self.tree(which) else {
            return Vec::new();
        };
        tree.leaf_of
            .iter()
            .map(|&leaf| tree.ancestor(leaf as usize, depth as usize) as u32)
            .collect()
    }

    /// The nodes of tree `which`, eight values each: parent (−1 for the
    /// root), depth, points through it, its center projected (x, y), the
    /// true topic most of its points are from, the share they hold, and 1
    /// for a leaf. Empty for an unknown tree or one not built yet.
    pub fn nodes(&self, which: &str) -> Vec<f32> {
        let Some(tree) = self.tree(which) else {
            return Vec::new();
        };
        // Points per node and topic, counted up from every point's leaf.
        let topics = BRANCHING[0];
        let mut tally = vec![0u32; tree.nodes.len() * topics];
        for (i, &leaf) in tree.leaf_of.iter().enumerate() {
            let topic = Embedder::labels(self.leaves[i] as usize)[0] as usize;
            let mut node = Some(leaf as usize);
            while let Some(n) = node {
                tally[n * topics + topic] += 1;
                node = tree.nodes[n].parent;
            }
        }
        let mut out = Vec::with_capacity(tree.nodes.len() * NODE_STRIDE);
        for (i, n) in tree.nodes.iter().enumerate() {
            let row = &tally[i * topics..(i + 1) * topics];
            let total: u32 = row.iter().sum();
            let (topic, &most) = row
                .iter()
                .enumerate()
                .max_by_key(|&(_, c)| *c)
                .expect("topics");
            let [x, y] = self.projection.apply(&n.center);
            out.extend([
                n.parent.map_or(-1.0, |p| p as f32),
                n.depth as f32,
                total as f32,
                x,
                y,
                topic as f32,
                if total > 0 {
                    most as f32 / total as f32
                } else {
                    0.0
                },
                n.is_leaf() as u8 as f32,
            ]);
        }
        out
    }

    /// How tree `which` matches the true hierarchy: the adjusted Rand index
    /// at depths 1, 2 and 3 against topics, subtopics and leaves, of its
    /// leaves against the true leaves, then its nodes at depths 1 to 4 and
    /// its leaves. Empty for an unknown tree or one not built yet.
    pub fn quality(&self, which: &str) -> Vec<f32> {
        let Some(tree) = self.tree(which) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for depth in 1..=LEVELS {
            let found: Vec<u32> = tree
                .leaf_of
                .iter()
                .map(|&l| tree.ancestor(l as usize, depth) as u32)
                .collect();
            out.push(adjusted_rand(&found, &self.true_labels(depth as u32 - 1)));
        }
        out.push(adjusted_rand(&tree.leaf_of, &self.true_labels(2)));
        for depth in 1..=4 {
            out.push(tree.nodes.iter().filter(|n| n.depth == depth).count() as f32);
        }
        out.push(tree.leaves().count() as f32);
        out
    }

    /// What tree `which` cost: distances per point to place points, per
    /// point to split nodes, and passes over the points. Empty for an
    /// unknown tree or one not built yet.
    pub fn cost(&self, which: &str) -> Vec<f32> {
        let Some(tree) = self.tree(which) else {
            return Vec::new();
        };
        let per = |c: u64| {
            if tree.points == 0 {
                0.0
            } else {
                c as f32 / tree.points as f32
            }
        };
        vec![
            per(tree.place_checks),
            per(tree.split_checks),
            tree.passes as f32,
        ]
    }

    /// Search through tree `which` for the 10 nearest neighbors of 100 fresh
    /// queries, at beams of 1, 2, 4 and 8: [beam, recall@10, distances per
    /// query] each, then the distances a scan takes. Empty for an unknown
    /// tree, one not built yet, or no points.
    pub fn search(&self, which: &str) -> Vec<f32> {
        let Some(tree) = self.tree(which) else {
            return Vec::new();
        };
        if self.leaves.is_empty() {
            return Vec::new();
        }
        let dim = self.spec().dim;
        let ids: Vec<u32> = (0..self.leaves.len() as u32).collect();
        let truth: Vec<Vec<u32>> = self
            .queries
            .iter()
            .map(|q| brute_force(q, K, &ids, &self.store, dim))
            .collect();
        let mut out = Vec::new();
        for beam in BEAMS {
            let (mut r, mut c) = (0.0, 0u64);
            for (q, t) in self.queries.iter().zip(&truth) {
                let (found, checks) = tree.search(q, K, beam, &self.store);
                r += recall(&found, t);
                c += checks;
            }
            let n = self.queries.len() as f32;
            out.extend([beam as f32, r / n, c as f32 / n]);
        }
        out.push(ids.len() as f32);
        out
    }

    /// [points so far, points set, dimensions].
    pub fn progress(&self) -> Vec<f32> {
        vec![
            self.leaves.len() as f32,
            self.value("points"),
            self.spec().dim as f32,
        ]
    }
}

impl TreeLab {
    /// How the embeddings are made, from the space and the noise.
    fn spec(&self) -> EmbedSpec {
        match self.space {
            "plane" => EmbedSpec::plane(),
            _ => EmbedSpec {
                noise: self.value("noise"),
                ..EmbedSpec::sphere64()
            },
        }
    }

    /// The tree's parameters from the current values.
    fn tree_params(&self) -> TreeParams {
        TreeParams {
            split_at: self.value("split_at") as u32,
            max_depth: self.value("max_depth") as usize,
            max_children: self.value("max_children") as usize,
            price: self.value("price"),
        }
    }

    /// The value of parameter `id`.
    fn value(&self, id: &str) -> f32 {
        self.values[PARAMS
            .iter()
            .position(|p| p.id == id)
            .expect("a known parameter")]
    }

    /// The tree `which` names, if built.
    fn tree(&self, which: &str) -> Option<&Tree> {
        match Which::parse(which)? {
            Which::Online => Some(&self.online),
            Which::Passes => self.passes.as_ref(),
        }
    }
}

impl Default for TreeLab {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(space: &str, points: f32) -> TreeLab {
        let mut lab = TreeLab::new(1);
        assert!(lab.set_space(space));
        assert!(lab.set_param("points", points));
        while lab.step(5000) > 0 {}
        lab
    }

    #[test]
    fn both_trees_find_the_topics_and_the_leaves() {
        let lab = run("sphere64", 20_000.0);
        assert!(lab.has_passes());
        for which in ["online", "passes"] {
            let q = lab.quality(which);
            assert!(q[0] > 0.85 && q[3] > 0.85, "{which}: {q:?}");
        }
    }

    #[test]
    fn search_beats_a_scan_by_far() {
        let lab = run("sphere64", 20_000.0);
        let s = lab.search("online");
        assert_eq!(s.len(), 3 * BEAMS.len() + 1);
        let (wide_recall, wide_cost, scan) = (s[3 * 3 + 1], s[3 * 3 + 2], s[12]);
        assert!(wide_recall > 0.95 && wide_cost < scan / 5.0, "{s:?}");
        assert!(s[1] <= wide_recall);
    }

    #[test]
    fn nodes_count_their_points_and_know_their_topic() {
        let lab = run("plane", 6000.0);
        let nodes = lab.nodes("online");
        assert_eq!(nodes.len() % NODE_STRIDE, 0);
        assert_eq!(nodes[0], -1.0, "the root has no parent");
        assert_eq!(nodes[2], 6000.0, "every point passes the root");
        // Children's points add up to their parent's.
        let n = nodes.len() / NODE_STRIDE;
        for p in 0..n {
            let kids: f32 = (0..n)
                .filter(|&c| nodes[c * NODE_STRIDE] == p as f32)
                .map(|c| nodes[c * NODE_STRIDE + 2])
                .sum();
            let leaf = nodes[p * NODE_STRIDE + 7] == 1.0;
            assert!(leaf || kids == nodes[p * NODE_STRIDE + 2], "node {p}");
        }
        assert_eq!(lab.projected().len(), 2 * 6000);
        assert_eq!(lab.assignments("online", 1).len(), 6000);
    }

    #[test]
    fn refusals_change_nothing() {
        let mut lab = TreeLab::new(1);
        lab.step(100);
        assert!(!lab.set_space("torus"));
        assert!(!lab.set_param("points", 10.0));
        assert!(!lab.set_param("depth", 2.0));
        assert_eq!(lab.progress()[0], 100.0);
        assert!(lab.nodes("passes").is_empty(), "not built yet");
        assert!(lab.quality("ternary").is_empty());
        let params: serde_json::Value = serde_json::from_str(&lab.params()).unwrap();
        assert_eq!(params.as_array().unwrap().len(), 6);
        for p in &PARAMS {
            let k = (p.default - p.min) / p.step;
            assert!(
                (k - k.round()).abs() < 1e-3,
                "{} default off its steps",
                p.id
            );
        }
    }
}
