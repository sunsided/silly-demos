//! WASM bindings for the Kohonen lab's Successors mode: the map beside the
//! vector quantizers that followed it (`som::successors`), all trained on
//! the same samples from one paintable density.

use som::codebook::Codebook;
use som::density::Density;
use som::init::{Init, initialize};
use som::lattice::{Kind, Lattice};
use som::metrics::{History, quantization_error, topographic_error};
use som::rng::Rng;
use som::schedule::{Decay, Schedule};
use som::successors::gng::{self, Gng};
use som::successors::graph::graph_error;
use som::successors::kmeans::KMeans;
use som::successors::neural_gas::{self, NeuralGas};
use som::train::Som;
use wasm_bindgen::prelude::*;

use crate::voronoi;

/// Samples the errors are measured on, redrawn when the density changes.
const EVAL_SAMPLES: usize = 1000;
/// Samples between two measurements.
const METRIC_EVERY: u64 = 250;
/// History entries kept per model; when full, every other one is dropped.
const HISTORY_CAP: usize = 400;
/// Samples between two updates of a moving density.
const MOTION_EVERY: u64 = 100;

/// A tunable method parameter: which model it belongs to, its range, and
/// whether changing it starts over (the scheduled ones) or applies at once.
struct Param {
    /// The id `set_param` takes.
    id: &'static str,
    /// The model it tunes, or "gng" for both growing gases.
    model: &'static str,
    /// What the page calls it.
    label: &'static str,
    /// Its published value.
    default: f32,
    /// The least it may be.
    min: f32,
    /// The most it may be.
    max: f32,
    /// Takes effect at once instead of starting over.
    live: bool,
}

/// Every tunable parameter. The defaults are the published ones: the map's
/// usual start, Martinetz and Schulten's for neural gas, Fritzke's for the
/// growing gases (GNG-U's ratio k from 1997).
const PARAMS: [Param; 9] = [
    Param {
        id: "som.alpha",
        model: "som",
        label: "α start",
        default: 0.5,
        min: 0.05,
        max: 1.0,
        live: false,
    },
    Param {
        id: "som.sigma",
        model: "som",
        label: "σ start, share of the grid",
        default: 0.5,
        min: 0.05,
        max: 1.0,
        live: false,
    },
    Param {
        id: "ng.eps",
        model: "ng",
        label: "ε start",
        default: 0.5,
        min: 0.05,
        max: 1.0,
        live: false,
    },
    Param {
        id: "ng.lambda",
        model: "ng",
        label: "λ start, share of the nodes",
        default: 0.5,
        min: 0.02,
        max: 1.0,
        live: false,
    },
    Param {
        id: "gng.insert",
        model: "gng",
        label: "Samples per insertion λ",
        default: 300.0,
        min: 20.0,
        max: 2000.0,
        live: true,
    },
    Param {
        id: "gng.age",
        model: "gng",
        label: "Edge age limit",
        default: 88.0,
        min: 5.0,
        max: 500.0,
        live: true,
    },
    Param {
        id: "gng.eps_b",
        model: "gng",
        label: "Winner step εb",
        default: 0.05,
        min: 0.001,
        max: 0.3,
        live: true,
    },
    Param {
        id: "gng.eps_n",
        model: "gng",
        label: "Neighbor step εn",
        default: 0.0006,
        min: 0.0,
        max: 0.02,
        live: true,
    },
    Param {
        id: "gngu.k",
        model: "gng-u",
        label: "Removal ratio k",
        default: 3.0,
        min: 1.0,
        max: 20.0,
        live: true,
    },
];

/// The value of parameter `id` in `values` (ordered as [`PARAMS`]).
fn param(values: &[f32], id: &str) -> f32 {
    values[PARAMS
        .iter()
        .position(|p| p.id == id)
        .expect("a known parameter")]
}
/// The models, in the order the page lays them out.
const MODELS: [Model; 5] = [
    Model::Som,
    Model::Ng,
    Model::Gng,
    Model::GngU,
    Model::KMeans,
];

/// One of the quantizers the lab trains side by side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Model {
    /// Kohonen's map on a grid.
    Som,
    /// Neural gas with competitive Hebbian edges.
    Ng,
    /// Growing neural gas.
    Gng,
    /// Growing neural gas with utility.
    GngU,
    /// Online k-means.
    KMeans,
}

impl Model {
    /// The model for a UI id: "som", "ng", "gng", "gng-u" or "kmeans".
    fn parse(id: &str) -> Option<Model> {
        MODELS.into_iter().find(|m| m.id() == id)
    }

    /// The model's UI id.
    fn id(self) -> &'static str {
        match self {
            Model::Som => "som",
            Model::Ng => "ng",
            Model::Gng => "gng",
            Model::GngU => "gng-u",
            Model::KMeans => "kmeans",
        }
    }

    /// The model's position in [`MODELS`] and in the per-model arrays.
    fn index(self) -> usize {
        MODELS
            .iter()
            .position(|&m| m == self)
            .expect("every model is listed")
    }
}

/// The five quantizers, built together from the same samples.
struct Models {
    /// Kohonen's map.
    som: Som,
    /// Neural gas.
    ng: NeuralGas,
    /// Growing neural gas.
    gng: Gng,
    /// Growing neural gas with utility.
    gng_u: Gng,
    /// Online k-means.
    kmeans: KMeans,
}

impl Models {
    /// The codebook of `model`.
    fn codebook(&self, model: Model) -> &Codebook {
        match model {
            Model::Som => &self.som.codebook,
            Model::Ng => &self.ng.codebook,
            Model::Gng => &self.gng.codebook,
            Model::GngU => &self.gng_u.codebook,
            Model::KMeans => &self.kmeans.codebook,
        }
    }
}

/// The map and its successors on one density. Each step draws one sample
/// and shows it to all five, so they learn from exactly the same data.
#[wasm_bindgen]
pub struct SuccessorLab {
    /// Where samples come from; painted from JS.
    density: Density,
    /// The seed every restart starts its random numbers from.
    seed: u64,
    /// The random numbers for sampling and initialization.
    rng: Rng,
    /// Samples the errors are measured on, x, y each.
    eval: Vec<f32>,
    /// Nodes asked for; the map rounds it to a grid and the others match it.
    nodes: usize,
    /// Samples per run.
    steps: u64,
    /// Keep learning past `steps`: the map and neural gas at their end
    /// rates, the growing gases and k-means as before.
    continuous: bool,
    /// The models, or `None` while the density is empty.
    models: Option<Models>,
    /// Samples presented so far.
    t: u64,
    /// (t, QE, TE) over time, per model in [`MODELS`] order.
    history: Vec<History>,
    /// The latest (QE, TE) per model; TE is NaN for k-means.
    errors: [(f32, f32); 5],
    /// The method parameters, ordered as [`PARAMS`].
    values: Vec<f32>,
    /// A [`som::density::MOVING`] density the samples follow, and the
    /// samples one cycle of it takes; `None` for a still one.
    motion: Option<(&'static str, u64)>,
}

#[wasm_bindgen]
impl SuccessorLab {
    /// 100 nodes each over 20 000 samples of a uniform square, learning on
    /// past the end.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let mut lab = Self {
            density: Density::preset("uniform").expect("a known preset"),
            seed: seed as u64,
            rng: Rng::new(seed as u64),
            eval: Vec::new(),
            nodes: 100,
            steps: 20_000,
            continuous: true,
            models: None,
            t: 0,
            history: (0..MODELS.len())
                .map(|_| History::new(HISTORY_CAP))
                .collect(),
            errors: [(0.0, 0.0); 5],
            values: PARAMS.iter().map(|p| p.default).collect(),
            motion: None,
        };
        lab.restart();
        lab
    }

    /// Nodes per model, 4 to 400; the map takes the nearest grid and the
    /// others take as many as it has. Restarts. `false`, changing nothing,
    /// out of range.
    pub fn set_nodes(&mut self, n: u32) -> bool {
        if !(4..=400).contains(&n) {
            return false;
        }
        self.nodes = n as usize;
        self.restart();
        true
    }

    /// Samples per run, 1000 to 1 000 000. Restarts. `false`, changing
    /// nothing, out of range.
    pub fn set_steps(&mut self, steps: u32) -> bool {
        if !(1000..=1_000_000).contains(&steps) {
            return false;
        }
        self.steps = steps as u64;
        self.restart();
        true
    }

    /// Whether to keep learning past the end of the run; takes effect at once.
    pub fn set_continuous(&mut self, on: bool) {
        self.continuous = on;
        if let Some(m) = &mut self.models {
            m.som.schedule.continuous = on;
            m.ng.params.schedule.continuous = on;
        }
    }

    /// Replaces the density with a preset: "uniform", "annulus", "moons",
    /// "blobs", "lshape", "gradient" or "blank". The models keep training on it.
    pub fn set_density(&mut self, preset: &str) -> bool {
        let Some(density) = Density::preset(preset) else {
            return false;
        };
        self.motion = None;
        self.replace_density(density);
        true
    }

    /// Replaces the density with `side`×`side` non-negative weights,
    /// row-major from y = 0. `false`, changing nothing, for a side outside
    /// 2 to 256, the wrong number of cells, or a negative or non-finite weight.
    pub fn set_density_cells(&mut self, cells: &[f32], side: u32) -> bool {
        let Some(density) = Density::from_cells(side as usize, cells) else {
            return false;
        };
        self.motion = None;
        self.replace_density(density);
        true
    }

    /// Adds density within `radius` of (x, y) in the unit square; a negative
    /// `amount` erases. The models keep training, so erasing a region shows
    /// which of them leave it.
    pub fn paint(&mut self, x: f32, y: f32, radius: f32, amount: f32) {
        let version = self.density.version();
        self.density.paint(x, y, radius, amount);
        if self.density.version() != version {
            self.after_density_change();
        }
    }

    /// The density's side in cells.
    pub fn density_side(&self) -> u32 {
        self.density.side() as u32
    }

    /// The density, `density_side()` cells a side, row-major from y = 0,
    /// scaled to a maximum of 1.
    pub fn density(&self) -> Vec<f32> {
        let cells = self.density.cells();
        let max = cells.iter().cloned().fold(0.0, f32::max);
        if max <= 0.0 {
            return vec![0.0; cells.len()];
        }
        cells.iter().map(|c| c / max).collect()
    }

    /// Makes the density move: "orbit", a blob circling the center, or
    /// "jump", a block switching between the left and right halves, one
    /// cycle every `period` samples (1000 to 1 000 000). "none" stops it
    /// where it is. Takes effect at once, without starting over. `false`,
    /// changing nothing, for another name or period.
    pub fn set_motion(&mut self, kind: &str, period: u32) -> bool {
        if kind == "none" {
            self.motion = None;
            return true;
        }
        let Some(&kind) = som::density::MOVING.iter().find(|&&k| k == kind) else {
            return false;
        };
        if !(1000..=1_000_000).contains(&period) {
            return false;
        }
        self.motion = Some((kind, period as u64));
        self.move_density();
        self.after_density_change();
        true
    }

    /// The method parameters as JSON: `[{ "id", "model", "label", "value",
    /// "default", "min", "max", "live" }]`; `model` "gng" covers both
    /// growing gases, and `live` ones apply without starting over.
    pub fn params(&self) -> String {
        let rows: Vec<String> = PARAMS
            .iter()
            .zip(&self.values)
            .map(|(p, v)| {
                format!(
                    r#"{{"id":"{}","model":"{}","label":"{}","value":{v},"default":{},"min":{},"max":{},"live":{}}}"#,
                    p.id, p.model, p.label, p.default, p.min, p.max, p.live
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }

    /// Sets a method parameter. The growing gases' apply at once, to both
    /// of them where shared; the map's and neural gas' start over, since
    /// they set where a schedule starts. `false`, changing nothing, for an
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
        if p.live {
            if let Some(m) = &mut self.models {
                for g in [&mut m.gng.params, &mut m.gng_u.params] {
                    Self::tune(g, &self.values);
                }
                m.gng.params.utility = None;
            }
        } else {
            self.restart();
        }
        true
    }

    /// Starts over with other random numbers; the density stays.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// Back to sample 0 with fresh models. The random numbers start over
    /// from the seed, so a restart replays the same run.
    pub fn restart(&mut self) {
        self.rng = Rng::new(self.seed);
        self.t = 0;
        self.move_density();
        self.draw_eval();
        self.history.iter_mut().for_each(History::clear);
        self.models = self.build();
        self.measure(true);
    }

    /// Shows `n` samples to every model; returns how many, fewer once the
    /// run is over (unless continuous) and 0 while the density is empty.
    pub fn step(&mut self, n: u32) -> u32 {
        let mut done = 0;
        while done < n && self.models.is_some() && (self.continuous || self.t < self.steps) {
            if self.motion.is_some() && self.t.is_multiple_of(MOTION_EVERY) {
                self.move_density();
            }
            let Some(x) = self.density.sample(&mut self.rng) else {
                break;
            };
            let m = self.models.as_mut().expect("checked above");
            m.som.step_online(&x, false);
            m.ng.step(&x);
            m.gng.step(&x);
            m.gng_u.step(&x);
            m.kmeans.step(&x);
            self.t += 1;
            if self.t.is_multiple_of(METRIC_EVERY) {
                if self.motion.is_some() {
                    self.draw_eval();
                }
                self.measure(true);
            }
            done += 1;
        }
        done
    }

    /// x, y per node of `model` ("som", "ng", "gng", "gng-u" or "kmeans");
    /// empty for an unknown id or while the density is empty.
    pub fn weights(&self, model: &str) -> Vec<f32> {
        match (Model::parse(model), &self.models) {
            (Some(id), Some(m)) => m.codebook(id).weights.clone(),
            _ => Vec::new(),
        }
    }

    /// The links of `model` as pairs of node indices: the map's lattice,
    /// the learned edges of the gases, none for k-means.
    pub fn edges(&self, model: &str) -> Vec<u32> {
        let (Some(id), Some(m)) = (Model::parse(model), &self.models) else {
            return Vec::new();
        };
        match id {
            Model::Som => m
                .som
                .lattice
                .edges()
                .into_iter()
                .flat_map(|(a, b)| [a, b])
                .collect(),
            Model::Ng => m.ng.graph.pairs(),
            Model::Gng => m.gng.graph.pairs(),
            Model::GngU => m.gng_u.graph.pairs(),
            Model::KMeans => Vec::new(),
        }
    }

    /// The Voronoi edges of `model`'s nodes, the regions each one
    /// quantizes to, as segments x1, y1, x2, y2.
    pub fn voronoi(&self, model: &str) -> Vec<f32> {
        let points: Vec<(f32, f32)> = self
            .weights(model)
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[x, y]| (x, y))
            .collect();
        voronoi::edges_of(&points)
    }

    /// [QE, TE, nodes, edges] of `model`, as last measured. QE is the mean
    /// distance from a sample to its nearest node; TE the share of samples
    /// whose two nearest nodes aren't linked (NaN for k-means, which has no
    /// links). Empty for an unknown id.
    pub fn errors(&self, model: &str) -> Vec<f32> {
        let Some(id) = Model::parse(model) else {
            return Vec::new();
        };
        let (qe, te) = self.errors[id.index()];
        let nodes = self.weights(model).len() / 2;
        let edges = self.edges(model).len() / 2;
        vec![qe, te, nodes as f32, edges as f32]
    }

    /// `model`'s errors over time as (t, QE, TE) triples; empty for an unknown id.
    pub fn history(&self, model: &str) -> Vec<f32> {
        Model::parse(model).map_or_else(Vec::new, |id| self.history[id.index()].values().to_vec())
    }

    /// [samples so far, samples per run].
    pub fn progress(&self) -> Vec<f32> {
        vec![self.t as f32, self.steps as f32]
    }

    /// "training", "converged" once a non-continuous run is over, or
    /// "empty" while the density is.
    pub fn status(&self) -> String {
        if self.models.is_none() {
            "empty"
        } else if !self.continuous && self.t >= self.steps {
            "converged"
        } else {
            "training"
        }
        .into()
    }
}

impl SuccessorLab {
    /// New evaluation samples from the current density.
    fn draw_eval(&mut self) {
        self.eval = (0..EVAL_SAMPLES)
            .map_while(|_| self.density.sample(&mut self.rng))
            .flatten()
            .collect();
    }

    /// For a moving density, the density at the current sample's phase.
    fn move_density(&mut self) {
        if let Some((kind, period)) = self.motion {
            self.density =
                Density::moving(kind, self.t as f32 / period as f32).expect("a moving density");
        }
    }

    /// Writes the growing-gas parameters in `values` into `params`; GNG-U's
    /// ratio goes in only where `params` has utility.
    fn tune(params: &mut gng::Params, values: &[f32]) {
        params.insert_every = param(values, "gng.insert").round() as u32;
        params.max_age = param(values, "gng.age").round() as u32;
        params.eps_winner = param(values, "gng.eps_b");
        params.eps_neighbor = param(values, "gng.eps_n");
        params.utility = Some(param(values, "gngu.k"));
    }

    /// Swaps in a new density and lets training go on over it.
    fn replace_density(&mut self, density: Density) {
        self.density = density;
        self.after_density_change();
    }

    /// Fresh evaluation samples after the density changed; models are built
    /// the first time there is anything to learn and dropped when nothing is left.
    fn after_density_change(&mut self) {
        self.draw_eval();
        if self.eval.is_empty() {
            self.models = None;
        } else if self.models.is_none() {
            self.restart();
            return;
        }
        self.measure(false);
    }

    /// The five models on the evaluation samples, or `None` without any.
    fn build(&mut self) -> Option<Models> {
        if self.eval.is_empty() {
            return None;
        }
        let side = (self.nodes as f32).sqrt();
        let width = side.ceil() as usize;
        let height = ((self.nodes as f32 / width as f32).round() as usize).max(2);
        let lattice = Lattice::new(Kind::Grid, width, height).expect("4 to 400 nodes make a grid");
        let n = lattice.len();
        let codebook = initialize(Init::Random, &lattice, &self.eval, 2, &mut self.rng);
        let som = Som {
            codebook: codebook.clone(),
            schedule: Schedule {
                steps: self.steps,
                alpha: (param(&self.values, "som.alpha"), 0.01),
                sigma: (
                    (width.max(height) as f32 * param(&self.values, "som.sigma")).max(0.5),
                    0.5,
                ),
                decay: Decay::Exponential,
                continuous: self.continuous,
            },
            lattice,
            t: 0,
        };
        let mut ng_params = neural_gas::Params::defaults(n, self.steps);
        ng_params.schedule.continuous = self.continuous;
        ng_params.schedule.alpha.0 = param(&self.values, "ng.eps");
        ng_params.schedule.sigma.0 = (n as f32 * param(&self.values, "ng.lambda")).max(0.02);
        let ng = NeuralGas::new(codebook, ng_params);
        let pick = |rng: &mut Rng, eval: &[f32]| {
            let i = rng.below(eval.len() / 2);
            [eval[2 * i], eval[2 * i + 1]]
        };
        let (a, b) = (
            pick(&mut self.rng, &self.eval),
            pick(&mut self.rng, &self.eval),
        );
        let mut plain = gng::Params::defaults(n, None);
        Self::tune(&mut plain, &self.values);
        plain.utility = None;
        let mut with_utility = plain;
        Self::tune(&mut with_utility, &self.values);
        let gng = Gng::new(&a, &b, plain);
        let gng_u = Gng::new(&a, &b, with_utility);
        let kmeans = KMeans::new(n, &self.eval, 2, &mut self.rng);
        Some(Models {
            som,
            ng,
            gng,
            gng_u,
            kmeans,
        })
    }

    /// Updates every model's errors, and records them when `record`.
    fn measure(&mut self, record: bool) {
        let Some(m) = &self.models else {
            return;
        };
        for model in MODELS {
            let cb = m.codebook(model);
            let qe = quantization_error(cb, &self.eval);
            let te = match model {
                Model::Som => topographic_error(cb, &m.som.lattice, &self.eval),
                Model::Ng => graph_error(cb, &m.ng.graph, &self.eval),
                Model::Gng => graph_error(cb, &m.gng.graph, &self.eval),
                Model::GngU => graph_error(cb, &m.gng_u.graph, &self.eval),
                Model::KMeans => f32::NAN,
            };
            self.errors[model.index()] = (qe, te);
            if record {
                self.history[model.index()].push(self.t as f32, qe, te);
            }
        }
    }
}

impl Default for SuccessorLab {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: [&str; 5] = ["som", "ng", "gng", "gng-u", "kmeans"];

    fn finite(v: &[f32]) -> bool {
        v.iter().all(|v| v.is_finite())
    }

    #[test]
    fn every_model_gets_the_same_node_count() {
        let mut lab = SuccessorLab::new(1);
        lab.set_continuous(false);
        assert!(lab.set_nodes(50));
        while lab.step(5000) > 0 {}
        assert_eq!(lab.status(), "converged");
        // 50 asks for an 8 × 6 grid: 48 nodes, and the others match it.
        for id in IDS {
            assert_eq!(lab.weights(id).len(), 96, "{id}");
            assert!(finite(&lab.weights(id)), "{id}");
        }
    }

    #[test]
    fn all_of_them_quantize_a_square_well() {
        let mut lab = SuccessorLab::new(2);
        lab.set_continuous(false);
        while lab.step(5000) > 0 {}
        for id in IDS {
            let qe = lab.errors(id)[0];
            assert!(qe < 0.07, "{id}: QE {qe}");
        }
        assert!(lab.errors("kmeans")[1].is_nan());
        assert!(lab.errors("gng")[1] < 0.1);
        assert_eq!(lab.edges("kmeans"), Vec::<u32>::new());
        assert!(!lab.edges("ng").is_empty());
    }

    #[test]
    fn utility_follows_an_erased_half_and_plain_growth_does_not() {
        let mut lab = SuccessorLab::new(3);
        while lab.step(5000) > 0 && lab.progress()[0] < 20_000.0 {}
        // Erase the left half and keep training.
        for y in 0..=20 {
            lab.paint(0.2, y as f32 / 20.0, 0.3, -10.0);
        }
        while lab.progress()[0] < 60_000.0 {
            lab.step(5000);
        }
        let left = |id: &str| lab.weights(id).chunks(2).filter(|p| p[0] < 0.4).count();
        assert!(
            left("gng") > 10,
            "GNG keeps {} nodes on the erased side",
            left("gng")
        );
        assert!(left("gng-u") <= 3, "GNG-U keeps {}", left("gng-u"));
    }

    #[test]
    fn unknown_models_and_bad_sizes_are_refused() {
        let mut lab = SuccessorLab::new(1);
        assert!(lab.weights("lvq").is_empty());
        assert!(lab.errors("lvq").is_empty());
        assert!(lab.history("lvq").is_empty());
        assert!(!lab.set_nodes(3));
        assert!(!lab.set_nodes(401));
        assert!(!lab.set_steps(10));
        assert!(!lab.set_density("nebula"));
        assert_eq!(lab.weights("som").len(), 200);
    }

    #[test]
    fn parameters_come_as_json_and_refuse_what_is_out_of_range() {
        let mut lab = SuccessorLab::new(1);
        let params: serde_json::Value = serde_json::from_str(&lab.params()).unwrap();
        assert_eq!(params.as_array().unwrap().len(), 9);
        assert_eq!(params[4]["id"], "gng.insert");
        assert_eq!(params[4]["live"], true);
        assert!(!lab.set_param("gng.speed", 1.0));
        assert!(!lab.set_param("gng.age", 1.0));
        assert!(!lab.set_param("gngu.k", f32::NAN));
        assert!(
            lab.params()
                .contains(r#""id":"gng.age","model":"gng","label":"Edge age limit","value":88"#)
        );
    }

    #[test]
    fn growing_gas_settings_apply_at_once_and_the_map_s_start_over() {
        let mut lab = SuccessorLab::new(1);
        lab.step(3000);
        assert!(lab.set_param("gng.insert", 50.0));
        assert_eq!(lab.progress()[0], 3000.0, "a live change keeps the run");
        let m = lab.models.as_ref().unwrap();
        assert_eq!(
            (m.gng.params.insert_every, m.gng_u.params.insert_every),
            (50, 50)
        );
        assert_eq!(
            (m.gng.params.utility, m.gng_u.params.utility),
            (None, Some(3.0))
        );
        assert!(lab.set_param("som.alpha", 0.9));
        assert_eq!(lab.progress()[0], 0.0, "a schedule change starts over");
        assert_eq!(lab.models.as_ref().unwrap().som.schedule.alpha.0, 0.9);
    }

    #[test]
    fn utility_keeps_up_with_a_jumping_density() {
        let mut lab = SuccessorLab::new(4);
        assert!(lab.set_motion("jump", 40_000));
        assert!(!lab.set_motion("drift", 40_000));
        assert!(!lab.set_motion("jump", 10));
        // Left for 20 000 samples, then right until 38 000.
        while lab.progress()[0] < 38_000.0 {
            lab.step(2000);
        }
        let left = |id: &str| lab.weights(id).chunks(2).filter(|p| p[0] < 0.5).count();
        assert!(left("gng") > 10, "GNG keeps {}", left("gng"));
        assert!(left("gng-u") <= 3, "GNG-U keeps {}", left("gng-u"));
        let density = lab.density();
        let right_mass: f32 = density
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 64 >= 32)
            .map(|(_, v)| v)
            .sum();
        assert!(
            right_mass > 0.0 && right_mass == density.iter().sum::<f32>(),
            "the block is on the right"
        );
    }

    #[test]
    fn an_empty_density_has_no_models_until_it_is_painted() {
        let mut lab = SuccessorLab::new(1);
        assert!(lab.set_density("blank"));
        assert_eq!(lab.status(), "empty");
        assert_eq!(lab.step(10), 0);
        lab.paint(0.5, 0.5, 0.2, 1.0);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.step(10), 10);
    }
}
