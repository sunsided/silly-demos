//! WASM bindings for the vector search lab (`ann`): mock embeddings
//! streamed into the indexes that grow online, HNSW, an SPFresh-style
//! inverted file and the repaired tree of clusters, then the batch ones,
//! IVF-PQ and Vamana, built over the same points; all searched for the
//! same queries' nearest neighbors and counted in distances.

use ann::hnsw::{Hnsw, HnswParams};
use ann::pq::{IvfPq, PqParams};
use ann::spfresh::{Spfresh, SpfreshParams};
use ann::vamana::{Vamana, VamanaParams};
use ann::{Found, brute_force, recall};
use stream::embed::{EmbedSpec, Embedder, LEVELS, Projection};
use stream::rng::Rng;
use stream::tree::{Tree, TreeParams};
use wasm_bindgen::prelude::*;

/// Points the projection is fitted on.
const PROJECTION_SAMPLE: usize = 1500;
/// Queries the search is measured with.
const QUERIES: usize = 100;
/// Neighbors searched for.
const K: usize = 10;

/// A tunable parameter and its range. All start the lab over.
struct Param {
    /// The id `set_param` takes.
    id: &'static str,
    /// What the page calls it.
    label: &'static str,
    /// The index it tunes, or "stream".
    index: &'static str,
    /// Its default.
    default: f32,
    /// The least it may be.
    min: f32,
    /// The most it may be.
    max: f32,
    /// A slider step that lands on the default from the minimum.
    step: f32,
}

/// Every tunable parameter: the stream's, then each index's.
const PARAMS: [Param; 11] = [
    Param {
        id: "points",
        label: "Points",
        index: "stream",
        default: 20_000.0,
        min: 2000.0,
        max: 40_000.0,
        step: 1000.0,
    },
    Param {
        id: "noise",
        label: "Noise, 64-D",
        index: "stream",
        default: 0.06,
        min: 0.02,
        max: 0.1,
        step: 0.005,
    },
    Param {
        id: "m",
        label: "Links per node",
        index: "hnsw",
        default: 16.0,
        min: 4.0,
        max: 48.0,
        step: 4.0,
    },
    Param {
        id: "ef_construction",
        label: "Candidates while inserting",
        index: "hnsw",
        default: 100.0,
        min: 20.0,
        max: 400.0,
        step: 20.0,
    },
    Param {
        id: "limit",
        label: "Points before a posting splits",
        index: "spfresh",
        default: 256.0,
        min: 64.0,
        max: 1024.0,
        step: 64.0,
    },
    Param {
        id: "reach",
        label: "Nearby postings rebalanced",
        index: "spfresh",
        default: 8.0,
        min: 0.0,
        max: 64.0,
        step: 4.0,
    },
    Param {
        id: "lists",
        label: "Lists",
        index: "pq",
        default: 128.0,
        min: 16.0,
        max: 512.0,
        step: 16.0,
    },
    Param {
        id: "rerank",
        label: "Candidates compared exactly",
        index: "pq",
        default: 100.0,
        min: 0.0,
        max: 500.0,
        step: 50.0,
    },
    Param {
        id: "r",
        label: "Links per node at most",
        index: "vamana",
        default: 32.0,
        min: 8.0,
        max: 64.0,
        step: 8.0,
    },
    Param {
        id: "alpha",
        label: "Prune's α",
        index: "vamana",
        default: 1.0,
        min: 1.0,
        max: 1.6,
        step: 0.1,
    },
    Param {
        id: "split_at",
        label: "Points before a leaf splits",
        index: "tree",
        default: 300.0,
        min: 50.0,
        max: 2000.0,
        step: 50.0,
    },
];

/// Which index a call is about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    /// HNSW, grown online.
    Hnsw,
    /// The SPFresh-style inverted file, grown online.
    Spfresh,
    /// IVF-PQ, built once every point is in.
    Pq,
    /// Vamana, built in two passes once every point is in.
    Vamana,
    /// The repaired tree of clusters, grown online.
    Tree,
}

impl Which {
    /// The index for a UI id.
    fn parse(id: &str) -> Option<Which> {
        match id {
            "hnsw" => Some(Which::Hnsw),
            "spfresh" => Some(Which::Spfresh),
            "pq" => Some(Which::Pq),
            "vamana" => Some(Which::Vamana),
            "tree" => Some(Which::Tree),
            _ => None,
        }
    }

    /// The search effort settings measured: `ef` for HNSW, lists or
    /// postings probed for the inverted files, `L` for Vamana, the beam for
    /// the tree.
    fn efforts(self) -> &'static [usize] {
        match self {
            Which::Hnsw | Which::Vamana => &[10, 16, 24, 32, 48, 64, 128],
            Which::Spfresh | Which::Tree => &[1, 2, 3, 4, 6, 8],
            Which::Pq => &[1, 2, 4, 8, 16, 32],
        }
    }
}

/// A stream of embeddings, the indexes grown from it, and the ones built
/// over it once it is in.
#[wasm_bindgen]
pub struct VectorLab {
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
    /// Queries, from the same stream, never inserted.
    queries: Vec<Vec<f32>>,
    /// HNSW.
    hnsw: Hnsw,
    /// The inverted file that splits online.
    spfresh: Spfresh,
    /// The repaired tree.
    tree: Tree,
    /// IVF-PQ, once built.
    pq: Option<IvfPq>,
    /// Vamana, once started.
    vamana: Option<Vamana>,
    /// The parameter values, ordered as [`PARAMS`].
    values: Vec<f32>,
}

#[wasm_bindgen]
impl VectorLab {
    /// 20 000 points of the 64-dimensional embeddings, default indexes.
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
            queries: Vec::new(),
            hnsw: Hnsw::new(HnswParams::default(), spec.dim, 1),
            spfresh: Spfresh::new(SpfreshParams::default(), spec.dim, 1),
            tree: Tree::new(TreeParams::repaired(), spec.dim, 1),
            pq: None,
            vamana: None,
            values: PARAMS.iter().map(|p| p.default).collect(),
        };
        lab.restart();
        lab
    }

    /// "plane" (2D) or "sphere64" (64-dimensional unit vectors). Starts
    /// over. `false`, changing nothing, for another id.
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

    /// The parameters as JSON: `[{ "id", "label", "index", "value",
    /// "default", "min", "max", "step" }]`.
    pub fn params(&self) -> String {
        let rows: Vec<String> = PARAMS
            .iter()
            .zip(&self.values)
            .map(|(p, v)| {
                format!(
                    r#"{{"id":"{}","label":"{}","index":"{}","value":{v},"default":{},"min":{},"max":{},"step":{}}}"#,
                    p.id, p.label, p.index, p.default, p.min, p.max, p.step
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

    /// Back to no points and empty indexes, the projection fitted afresh.
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
        let hnsw = HnswParams {
            m: self.value("m") as usize,
            ef_construction: self.value("ef_construction") as usize,
        };
        self.hnsw = Hnsw::new(hnsw, spec.dim, self.seed ^ 0x1111);
        let spfresh = SpfreshParams {
            limit: self.value("limit") as usize,
            reach: self.value("reach") as usize,
            ..SpfreshParams::default()
        };
        self.spfresh = Spfresh::new(spfresh, spec.dim, self.seed ^ 0x2222);
        let tree = TreeParams {
            split_at: self.value("split_at") as u32,
            ..TreeParams::repaired()
        };
        self.tree = Tree::new(tree, spec.dim, self.seed ^ 0x3333);
        self.pq = None;
        self.vamana = None;
    }

    /// Does up to `n` more points' work: streams them into the online
    /// indexes until the set number is in, then builds IVF-PQ (in one go)
    /// and Vamana (`n` points of its passes at a time). Returns the work
    /// done, 0 once everything is built.
    pub fn step(&mut self, n: u32) -> u32 {
        let total = self.value("points") as usize;
        if self.leaves.len() < total {
            let mut done = 0;
            while done < n && self.leaves.len() < total {
                let (x, leaf) = self.embedder.next_point();
                let id = self.leaves.len() as u32;
                self.projected.extend(self.projection.apply(&x));
                self.store.extend(x);
                self.leaves.push(leaf as u32);
                self.hnsw.insert(id, &self.store);
                self.spfresh.insert(id, &self.store);
                self.tree.insert(id, &self.store);
                done += 1;
            }
            return done;
        }
        let dim = self.spec().dim;
        let points = self.leaves.len();
        if self.pq.is_none() {
            // A part per dimension in the plane, four dimensions a part in 64.
            let subspaces = if dim == 2 { 2 } else { dim / 4 };
            let params = PqParams {
                lists: self.value("lists") as usize,
                rerank: self.value("rerank") as usize,
                subspaces,
                ..PqParams::default()
            };
            self.pq = Some(IvfPq::build(
                params,
                dim,
                points,
                &self.store,
                self.seed ^ 0x4444,
            ));
            return 1;
        }
        if self.vamana.is_none() {
            let params = VamanaParams {
                r: self.value("r") as usize,
                alpha: self.value("alpha"),
                ..VamanaParams::default()
            };
            self.vamana = Some(Vamana::new(
                params,
                dim,
                points,
                &self.store,
                self.seed ^ 0x5555,
            ));
        }
        let vamana = self.vamana.as_mut().expect("started");
        vamana.build(n as usize, &self.store) as u32
    }

    /// What is under way: "streaming", "building" or "done".
    pub fn phase(&self) -> String {
        if self.leaves.len() < self.value("points") as usize {
            "streaming"
        } else if self.vamana.as_ref().is_some_and(Vamana::is_built) {
            "done"
        } else {
            "building"
        }
        .to_string()
    }

    /// [points so far, points set, dimensions, share of Vamana built].
    pub fn progress(&self) -> Vec<f32> {
        vec![
            self.leaves.len() as f32,
            self.value("points"),
            self.spec().dim as f32,
            self.vamana.as_ref().map_or(0.0, Vamana::progress),
        ]
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

    /// Queries.
    pub fn queries(&self) -> u32 {
        self.queries.len() as u32
    }

    /// Query `query` projected, x and y; empty for no such query.
    pub fn query_point(&self, query: u32) -> Vec<f32> {
        self.queries
            .get(query as usize)
            .map_or(Vec::new(), |q| self.projection.apply(q).to_vec())
    }

    /// The search efforts measured for index `which`; empty for an unknown one.
    pub fn efforts(&self, which: &str) -> Vec<u32> {
        Which::parse(which).map_or(Vec::new(), |w| {
            w.efforts().iter().map(|&e| e as u32).collect()
        })
    }

    /// Searches index `which` for the 10 nearest neighbors of all 100
    /// queries at each of its efforts: [effort, recall@10, distances per
    /// query] each, then the distances a scan takes. Empty for an unknown
    /// index, one not built yet, or no points.
    pub fn search(&self, which: &str) -> Vec<f32> {
        let Some(w) = Which::parse(which) else {
            return Vec::new();
        };
        if !self.ready(w) || self.leaves.is_empty() {
            return Vec::new();
        }
        let truth: Vec<Vec<u32>> = (0..self.queries.len()).map(|i| self.truth(i)).collect();
        let mut out = Vec::new();
        for &effort in w.efforts() {
            let (mut r, mut c) = (0.0, 0.0);
            for (q, t) in self.queries.iter().zip(&truth) {
                let found = self.find(w, q, effort).expect("ready");
                r += recall(&found.ids, t);
                c += found.cost;
            }
            let n = self.queries.len() as f32;
            out.extend([effort as f32, r / n, c / n]);
        }
        out.push(self.leaves.len() as f32);
        out
    }

    /// One search of index `which` for query `query` at `effort`: the
    /// number found, the ids found, then every point compared with. Empty
    /// for an unknown index or query, or one not built yet.
    pub fn trace(&self, which: &str, query: u32, effort: u32) -> Vec<u32> {
        let Some(q) = self.queries.get(query as usize) else {
            return Vec::new();
        };
        let Some(found) = Which::parse(which).and_then(|w| self.find(w, q, effort as usize)) else {
            return Vec::new();
        };
        let mut out = vec![found.ids.len() as u32];
        out.extend(&found.ids);
        out.extend(&found.seen);
        out
    }

    /// The true 10 nearest neighbors of query `query`; empty for no such query.
    pub fn neighbors(&self, query: u32) -> Vec<u32> {
        if (query as usize) < self.queries.len() {
            self.truth(query as usize)
        } else {
            Vec::new()
        }
    }

    /// What index `which` costs: distances per point to build it; bytes
    /// held in memory per point; then two counts of its own: layers and
    /// links per point (HNSW), postings and points moved (SPFresh), lists
    /// and bytes of code (IVF-PQ), links per point and share built
    /// (Vamana), nodes and repairs (tree). Empty for an unknown index or
    /// one not started.
    pub fn cost(&self, which: &str) -> Vec<f32> {
        let Some(w) = Which::parse(which) else {
            return Vec::new();
        };
        let n = self.leaves.len().max(1) as f32;
        let dim = self.spec().dim as f32;
        // A vector of f32s, and an id or a link of four bytes.
        let vector = 4.0 * dim;
        match w {
            Which::Hnsw => {
                let links = self.hnsw.edges() as f32 / n;
                vec![
                    self.hnsw.build_checks as f32 / n,
                    vector + 4.0 * links,
                    self.hnsw.layers().len() as f32,
                    links,
                ]
            }
            Which::Spfresh => {
                let s = &self.spfresh;
                vec![
                    (s.place_checks + s.split_checks + s.reassign_checks) as f32 / n,
                    vector + 4.0 + vector * s.postings() as f32 / n,
                    s.postings() as f32,
                    s.moved as f32,
                ]
            }
            Which::Pq => {
                let Some(p) = &self.pq else {
                    return Vec::new();
                };
                vec![
                    (p.build_cost / n as f64) as f32,
                    p.code_bytes() + 4.0 + vector * p.params.lists as f32 / n,
                    p.params.lists as f32,
                    p.code_bytes(),
                ]
            }
            Which::Vamana => {
                let Some(v) = &self.vamana else {
                    return Vec::new();
                };
                let links = v.edges() as f32 / n;
                vec![
                    v.build_checks as f32 / n,
                    vector + 4.0 * links,
                    links,
                    v.progress(),
                ]
            }
            Which::Tree => {
                let t = &self.tree;
                vec![
                    (t.place_checks + t.split_checks + t.recheck_checks + t.repair_checks) as f32
                        / n,
                    vector + 4.0 + vector * t.nodes.len() as f32 / n,
                    t.nodes.len() as f32,
                    t.repairs as f32,
                ]
            }
        }
    }
}

impl VectorLab {
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

    /// The value of parameter `id`.
    fn value(&self, id: &str) -> f32 {
        self.values[PARAMS
            .iter()
            .position(|p| p.id == id)
            .expect("a known parameter")]
    }

    /// Whether index `w` can be searched: the online ones always, the batch
    /// ones once built.
    fn ready(&self, w: Which) -> bool {
        match w {
            Which::Pq => self.pq.is_some(),
            Which::Vamana => self.vamana.as_ref().is_some_and(Vamana::is_built),
            _ => true,
        }
    }

    /// Searches index `w` for `q` at `effort`, if it can be searched.
    fn find(&self, w: Which, q: &[f32], effort: usize) -> Option<Found> {
        if !self.ready(w) {
            return None;
        }
        Some(match w {
            Which::Hnsw => self.hnsw.search(q, K, effort, &self.store),
            Which::Spfresh => self.spfresh.search(q, K, effort, &self.store),
            Which::Pq => self.pq.as_ref()?.search(q, K, effort, &self.store),
            Which::Vamana => self.vamana.as_ref()?.search(q, K, effort, &self.store),
            Which::Tree => {
                let mut seen = Vec::new();
                let (ids, checks) = self.tree.search_seen(q, K, effort, &self.store, &mut seen);
                Found {
                    ids,
                    cost: checks as f32,
                    seen,
                }
            }
        })
    }

    /// The true nearest neighbors of query `i`, by a scan.
    fn truth(&self, i: usize) -> Vec<u32> {
        let ids: Vec<u32> = (0..self.leaves.len() as u32).collect();
        brute_force(&self.queries[i], K, &ids, &self.store, self.spec().dim)
    }
}

impl Default for VectorLab {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&str; 5] = ["hnsw", "spfresh", "pq", "vamana", "tree"];

    fn run(space: &str, points: f32) -> VectorLab {
        let mut lab = VectorLab::new(1);
        assert!(lab.set_space(space));
        assert!(lab.set_param("points", points));
        while lab.step(2000) > 0 {}
        lab
    }

    #[test]
    fn the_lab_streams_then_builds() {
        let mut lab = VectorLab::new(1);
        assert!(lab.set_param("points", 3000.0));
        assert_eq!(lab.phase(), "streaming");
        lab.step(500);
        assert!(lab.search("pq").is_empty() && lab.cost("vamana").is_empty());
        assert!(
            !lab.search("hnsw").is_empty(),
            "online indexes search as they grow"
        );
        while lab.phase() == "streaming" {
            lab.step(1000);
        }
        assert_eq!(lab.phase(), "building");
        while lab.step(1000) > 0 {}
        assert_eq!(lab.phase(), "done");
        assert_eq!(lab.progress(), vec![3000.0, 3000.0, 64.0, 1.0]);
    }

    #[test]
    fn every_index_finds_the_neighbors_for_a_fraction_of_a_scan() {
        let lab = run("sphere64", 10_000.0);
        for which in ALL {
            let s = lab.search(which);
            let efforts = lab.efforts(which).len();
            assert_eq!(s.len(), 3 * efforts + 1, "{which}");
            let best = (0..efforts).map(|i| s[3 * i + 1]).fold(0.0, f32::max);
            let cheap = (0..efforts)
                .filter(|&i| s[3 * i + 1] > 0.9)
                .map(|i| s[3 * i + 2])
                .fold(f32::INFINITY, f32::min);
            assert!(best > 0.9 && cheap < 10_000.0 / 5.0, "{which}: {s:?}");
            // More effort never finds less, give or take a query.
            for i in 1..efforts {
                assert!(s[3 * i + 1] + 0.02 >= s[3 * i - 2], "{which}: {s:?}");
            }
            let c = lab.cost(which);
            assert_eq!(c.len(), 4, "{which}");
            assert!(
                c.iter().all(|v| v.is_finite() && *v >= 0.0),
                "{which}: {c:?}"
            );
        }
        // Codes take a fraction of the memory the vectors do.
        assert!(lab.cost("pq")[1] < lab.cost("hnsw")[1] / 5.0);
    }

    #[test]
    fn a_trace_holds_what_was_found_and_compared() {
        let lab = run("plane", 3000.0);
        for which in ALL {
            let t = lab.trace(which, 0, lab.efforts(which)[2]);
            let found = t[0] as usize;
            assert_eq!(found, 10, "{which}");
            let seen = &t[1 + found..];
            assert!(
                t[1..1 + found].iter().all(|id| seen.contains(id)),
                "{which}"
            );
            assert!(seen.iter().all(|&id| id < 3000), "{which}");
        }
        assert_eq!(lab.neighbors(0).len(), 10);
        assert_eq!(lab.query_point(0).len(), 2);
        assert!(lab.trace("hnsw", 1000, 10).is_empty());
    }

    #[test]
    fn refusals_change_nothing() {
        let mut lab = VectorLab::new(1);
        lab.step(100);
        assert!(!lab.set_space("torus"));
        assert!(!lab.set_param("points", 10.0));
        assert!(!lab.set_param("alpha", 2.0));
        assert!(!lab.set_param("beam", 2.0));
        assert_eq!(lab.progress()[0], 100.0);
        assert!(lab.search("annoy").is_empty() && lab.efforts("annoy").is_empty());
        let params: serde_json::Value = serde_json::from_str(&lab.params()).unwrap();
        assert_eq!(params.as_array().unwrap().len(), PARAMS.len());
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
