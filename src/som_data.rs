//! WASM bindings for the Kohonen lab's data maps: colours and countries.

use som::codebook::Codebook;
use som::data::Normaliser;
use som::init::{Init, initialise};
use som::lattice::{Kind, Lattice};
use som::metrics::{History, quantization_error, topographic_error};
use som::rng::Rng;
use som::schedule::{Decay, Schedule};
use som::train::Som;
use som::views;
use wasm_bindgen::prelude::*;

/// Up to this many rows, a batch epoch covers all of them; above, a fixed
/// random subset of this many, so an epoch counts the same against the
/// schedule however many rows there are (an image has tens of thousands).
const BATCH_ROWS: usize = 2000;
/// Up to this many rows, the errors are measured on all of them; above,
/// on a fixed random subset of [`EVAL_SAMPLES`].
const EVAL_ALL_UP_TO: usize = 2000;
const EVAL_SAMPLES: usize = 1000;
/// Online samples between two measurements.
const METRIC_EVERY: u64 = 250;
/// History entries kept.
const HISTORY_CAP: usize = 400;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Training {
    Online,
    Batch,
}

/// A self-organising map over rows of N-dimensional data (colours,
/// countries by their indicators), drawn as a picture of its nodes rather
/// than in input space. NaN in the data marks a missing value.
#[wasm_bindgen]
pub struct SomData {
    som: Som,
    init: Init,
    training: Training,
    seed: u64,
    rng: Rng,
    normalise: bool,
    normaliser: Normaliser,
    /// The rows as given, flat.
    raw: Vec<f32>,
    /// The rows normalised, which is what the map trains on.
    data: Vec<f32>,
    batch: Vec<f32>,
    eval: Vec<f32>,
    history: History,
    errors: (f32, f32),
}

#[wasm_bindgen]
impl SomData {
    /// No data yet; a 10×10 grid from random rows, trained online with α
    /// 0.5 → 0.01 and σ 5 → 0.5 over 30 000 samples. Columns are normalised.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let lattice = Lattice::new(Kind::Grid, 10, 10).expect("a 10×10 grid is in bounds");
        Self {
            som: Som {
                codebook: Codebook {
                    dim: 1,
                    weights: Vec::new(),
                },
                lattice,
                schedule: Schedule {
                    steps: 30_000,
                    alpha: (0.5, 0.01),
                    sigma: (5.0, 0.5),
                    decay: Decay::Exponential,
                    continuous: false,
                },
                t: 0,
            },
            init: Init::Random,
            training: Training::Online,
            seed: seed as u64,
            rng: Rng::new(seed as u64),
            normalise: true,
            normaliser: Normaliser::identity(1),
            raw: Vec::new(),
            data: Vec::new(),
            batch: Vec::new(),
            eval: Vec::new(),
            history: History::new(HISTORY_CAP),
            errors: (0.0, 0.0),
        }
    }

    /// The rows to train on, `dim` values each, flat; NaN marks a missing
    /// value. Starts over. `false`, changing nothing, for no rows, a length
    /// that isn't a multiple of `dim`, an infinite value, or a row with no
    /// value at all.
    pub fn set_data(&mut self, values: &[f32], dim: u32) -> bool {
        let dim = dim as usize;
        if dim == 0 || values.is_empty() || !values.len().is_multiple_of(dim) {
            return false;
        }
        if values.iter().any(|v| v.is_infinite()) {
            return false;
        }
        if values
            .chunks_exact(dim)
            .any(|row| row.iter().all(|v| v.is_nan()))
        {
            return false;
        }
        self.raw = values.to_vec();
        self.som.codebook.dim = dim;
        self.prepare();
        self.restart();
        true
    }

    /// Whether to train on z-scores per column (for columns in different
    /// units) or on the values as given (colours). Starts over.
    pub fn set_normalise(&mut self, on: bool) {
        self.normalise = on;
        if !self.raw.is_empty() {
            self.prepare();
            self.restart();
        }
    }

    /// "grid" or "hex", 2 to 400 nodes. Starts over.
    pub fn set_lattice(&mut self, kind: &str, width: u32, height: u32) -> bool {
        let Some(lattice) = Kind::parse(kind)
            .filter(|k| matches!(k, Kind::Grid | Kind::Hex))
            .and_then(|k| Lattice::new(k, width as usize, height as usize))
        else {
            return false;
        };
        self.som.lattice = lattice;
        self.restart();
        true
    }

    /// "random", "cluster" or "pca". Starts over.
    pub fn set_init(&mut self, kind: &str) -> bool {
        let Some(init) = Init::parse(kind) else {
            return false;
        };
        self.init = init;
        self.restart();
        true
    }

    /// "online" (one random row per step) or "batch" (all rows per step).
    pub fn set_training(&mut self, mode: &str) -> bool {
        self.training = match mode {
            "online" => Training::Online,
            "batch" => Training::Batch,
            _ => return false,
        };
        true
    }

    /// As `SomLab::set_schedule`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_schedule(
        &mut self,
        steps: u32,
        alpha_start: f32,
        alpha_end: f32,
        sigma_start: f32,
        sigma_end: f32,
        decay: &str,
        continuous: bool,
    ) -> bool {
        let Some(decay) = Decay::parse(decay) else {
            return false;
        };
        let schedule = Schedule {
            steps: steps as u64,
            alpha: (alpha_start, alpha_end),
            sigma: (sigma_start, sigma_end),
            decay,
            continuous,
        };
        if !schedule.is_valid() {
            return false;
        }
        self.som.schedule = schedule;
        true
    }

    /// Starts over with other random numbers.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// Back to t = 0 with a freshly initialised map; replays the same run
    /// for the same seed.
    pub fn restart(&mut self) {
        self.rng = Rng::new(self.seed);
        self.som.t = 0;
        self.history.clear();
        if self.data.is_empty() {
            return;
        }
        let dim = self.som.codebook.dim;
        self.batch = self.subset(BATCH_ROWS, BATCH_ROWS);
        self.eval = self.subset(EVAL_ALL_UP_TO, EVAL_SAMPLES);
        // Initialisations copy rows or fit axes to them; gaps would carry
        // NaN into the weights, so fill them with the column means first.
        let means = Normaliser::fit(&self.data, dim).mean;
        let filled: Vec<f32> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, &v)| if v.is_nan() { means[i % dim] } else { v })
            .collect();
        self.som.codebook = initialise(self.init, &self.som.lattice, &filled, dim, &mut self.rng);
        self.measure(true);
    }

    /// `n` random rows online or `n` batch epochs (over all rows, or a fixed
    /// subset of 2000 when there are more). Returns how many
    /// were done: fewer once the schedule finishes, 0 without data.
    pub fn step(&mut self, n: u32) -> u32 {
        if self.data.is_empty() {
            return 0;
        }
        let dim = self.som.codebook.dim;
        let rows = self.data.len() / dim;
        let mut done = 0;
        while done < n && !self.som.finished() {
            match self.training {
                Training::Online => {
                    let i = self.rng.below(rows);
                    self.som
                        .step_online(&self.data[i * dim..(i + 1) * dim], false);
                    if self.som.t.is_multiple_of(METRIC_EVERY) {
                        self.measure(true);
                    }
                }
                Training::Batch => {
                    self.som.epoch_batch(&self.batch, false);
                    self.measure(true);
                }
            }
            done += 1;
        }
        done
    }

    /// The nodes' weights in the data's own units, node after node.
    pub fn codebook(&self) -> Vec<f32> {
        self.normaliser.invert(&self.som.codebook.weights)
    }

    /// Each node's mean distance to its lattice neighbours, in the units
    /// the map trains in.
    pub fn umatrix(&self) -> Vec<f32> {
        if self.data.is_empty() {
            return Vec::new();
        }
        views::umatrix(&self.som.codebook, &self.som.lattice)
    }

    /// Component `k` of every node, in data units; empty for k ≥ dim.
    pub fn plane(&self, k: u32) -> Vec<f32> {
        let k = k as usize;
        if self.data.is_empty() || k >= self.som.codebook.dim {
            return Vec::new();
        }
        let (mean, scale) = (self.normaliser.mean[k], self.normaliser.scale[k]);
        views::plane(&self.som.codebook, k)
            .into_iter()
            .map(|v| v * scale + mean)
            .collect()
    }

    /// How many rows land on each node.
    pub fn hits(&self) -> Vec<u32> {
        if self.data.is_empty() {
            return Vec::new();
        }
        views::hits(&self.som.codebook, &self.data)
    }

    /// The best-matching node of every row, in order.
    pub fn bmus(&self) -> Vec<u32> {
        if self.data.is_empty() {
            return Vec::new();
        }
        views::bmus(&self.som.codebook, &self.data)
    }

    /// RGBA pixels with each colour replaced by its best-matching node's,
    /// alpha kept. Only for colour maps (three columns, not normalised);
    /// anything else comes back unchanged.
    pub fn quantize(&self, pixels: &[u8]) -> Vec<u8> {
        let cb = &self.som.codebook;
        if cb.dim != 3 || self.normalise || cb.is_empty() {
            return pixels.to_vec();
        }
        let mut out = pixels.to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            let node = cb.node(cb.bmu(&[px[0] as f32, px[1] as f32, px[2] as f32]));
            for (c, w) in px.iter_mut().zip(node) {
                *c = w.round().clamp(0.0, 255.0) as u8;
            }
        }
        out
    }

    /// [t, T, α, σ, quantization error, topographic error].
    pub fn progress(&self) -> Vec<f32> {
        let (alpha, sigma) = self.som.schedule.at(self.som.t);
        vec![
            self.som.t as f32,
            self.som.schedule.steps as f32,
            alpha,
            sigma,
            self.errors.0,
            self.errors.1,
        ]
    }

    /// The errors over time as (t, QE, TE) triples.
    pub fn history(&self) -> Vec<f32> {
        self.history.values().to_vec()
    }

    /// "training", "converged", or "empty" without data.
    pub fn status(&self) -> String {
        if self.data.is_empty() {
            "empty"
        } else if self.som.finished() {
            "converged"
        } else {
            "training"
        }
        .into()
    }
}

impl SomData {
    /// All rows when there are at most `all_up_to`, otherwise `n` picked at
    /// random.
    fn subset(&mut self, all_up_to: usize, n: usize) -> Vec<f32> {
        let dim = self.som.codebook.dim;
        let rows = self.data.len() / dim;
        if rows <= all_up_to {
            return self.data.clone();
        }
        (0..n)
            .flat_map(|_| {
                let i = self.rng.below(rows);
                self.data[i * dim..(i + 1) * dim].to_vec()
            })
            .collect()
    }

    /// Fits the normaliser to the raw rows and normalises them.
    fn prepare(&mut self) {
        let dim = self.som.codebook.dim;
        self.normaliser = if self.normalise {
            Normaliser::fit(&self.raw, dim)
        } else {
            Normaliser::identity(dim)
        };
        self.data = self.normaliser.apply(&self.raw);
    }

    fn measure(&mut self, record: bool) {
        let cb = &self.som.codebook;
        if cb.is_empty() || self.eval.is_empty() {
            return;
        }
        self.errors = (
            quantization_error(cb, &self.eval),
            topographic_error(cb, &self.som.lattice, &self.eval),
        );
        if record {
            self.history
                .push(self.som.t as f32, self.errors.0, self.errors.1);
        }
    }
}

impl Default for SomData {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use som::rng::Rng;

    fn finite(v: &[f32]) -> bool {
        v.iter().all(|v| v.is_finite())
    }

    fn uniform_rgb(n: usize, seed: u64) -> Vec<f32> {
        let mut rng = Rng::new(seed);
        (0..n * 3).map(|_| rng.next_f32() * 255.0).collect()
    }

    #[test]
    fn set_data_refuses_bad_input() {
        let mut lab = SomData::new(1);
        assert_eq!(lab.status(), "empty");
        assert_eq!(lab.step(10), 0);
        assert!(!lab.set_data(&[1.0, 2.0], 0));
        assert!(!lab.set_data(&[1.0, 2.0, 3.0], 2));
        assert!(!lab.set_data(&[], 2));
        assert!(!lab.set_data(&[1.0, f32::INFINITY], 2));
        assert!(!lab.set_data(&[1.0, 2.0, f32::NAN, f32::NAN], 2));
        assert_eq!(lab.status(), "empty");
        assert!(lab.set_data(&[1.0, 2.0, f32::NAN, 4.0], 2));
        assert_eq!(lab.status(), "training");
    }

    #[test]
    fn only_grids_and_hex() {
        let mut lab = SomData::new(1);
        assert!(!lab.set_lattice("chain", 10, 1));
        assert!(!lab.set_lattice("ring", 10, 1));
        assert!(lab.set_lattice("hex", 16, 10));
        assert!(!lab.set_lattice("grid", 30, 30));
    }

    #[test]
    fn views_have_the_right_lengths() {
        let mut lab = SomData::new(1);
        assert!(lab.set_lattice("grid", 4, 3));
        assert!(lab.set_data(&uniform_rgb(50, 1), 3));
        lab.step(100);
        assert_eq!(lab.codebook().len(), 36);
        assert_eq!(lab.umatrix().len(), 12);
        assert_eq!(lab.plane(2).len(), 12);
        assert!(lab.plane(3).is_empty());
        assert_eq!(lab.hits().iter().sum::<u32>(), 50);
        assert_eq!(lab.bmus().len(), 50);
        assert!(lab.bmus().iter().all(|&b| b < 12));
        assert_eq!(lab.progress().len(), 6);
    }

    #[test]
    fn rgb_map_orders_itself() {
        let mut lab = SomData::new(1);
        lab.set_normalise(false);
        assert!(lab.set_data(&uniform_rgb(2000, 2), 3));
        while lab.step(1000) > 0 {}
        assert_eq!(lab.status(), "converged");
        let te = lab.progress()[5];
        assert!(te < 0.1, "TE {te}");
        assert!(lab.codebook().iter().all(|v| (-1.0..=256.0).contains(v)));
    }

    #[test]
    fn normalised_codebook_comes_back_in_data_units() {
        let mut lab = SomData::new(1);
        let mut rng = Rng::new(4);
        let data: Vec<f32> = (0..500)
            .flat_map(|_| {
                [
                    1000.0 + rng.next_f32() * 100.0,
                    0.001 + rng.next_f32() * 0.0001,
                ]
            })
            .collect();
        assert!(lab.set_data(&data, 2));
        lab.step(3000);
        let cb = lab.codebook();
        let mean = |k: usize| cb.iter().skip(k).step_by(2).sum::<f32>() / (cb.len() / 2) as f32;
        assert!((mean(0) - 1050.0).abs() < 105.0, "{}", mean(0));
        assert!((mean(1) - 0.00105).abs() < 0.000105, "{}", mean(1));
        let p = lab.plane(0);
        assert!((p.iter().sum::<f32>() / p.len() as f32 - mean(0)).abs() < 1e-2);
    }

    #[test]
    fn quantize_maps_pixels_to_node_colours() {
        let mut lab = SomData::new(1);
        lab.set_normalise(false);
        assert!(lab.set_lattice("grid", 2, 1));
        // A small final σ, so the two nodes stop pulling on each other.
        assert!(lab.set_schedule(5000, 0.5, 0.01, 1.0, 0.05, "exp", false));
        let pixels: Vec<f32> = (0..200)
            .flat_map(|i| {
                if i % 2 == 0 {
                    [255.0, 0.0, 0.0]
                } else {
                    [0.0, 0.0, 255.0]
                }
            })
            .collect();
        assert!(lab.set_data(&pixels, 3));
        while lab.step(1000) > 0 {}
        let cb = lab.codebook();
        let red = if cb[0] > cb[3] { 0 } else { 1 };
        let out = lab.quantize(&[200, 30, 10, 77]);
        let expect: Vec<u8> = cb[red * 3..red * 3 + 3]
            .iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .chain([77])
            .collect();
        assert_eq!(out, expect);
        assert!(expect[0] > 250 && expect[2] < 5);
    }

    #[test]
    fn quantize_is_identity_for_non_colour_data() {
        let mut lab = SomData::new(1);
        assert!(lab.set_data(&[1.0, 2.0, 3.0, 4.0], 2));
        assert_eq!(lab.quantize(&[1, 2, 3, 4]), vec![1, 2, 3, 4]);
        // Colour data, but normalised: not colours any more.
        assert!(lab.set_data(&uniform_rgb(10, 1), 3));
        assert_eq!(lab.quantize(&[1, 2, 3, 4]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn batch_on_many_rows_still_takes_many_epochs() {
        // An image's pixels outnumber T; a batch epoch over all of them
        // would end training after one step.
        let mut lab = SomData::new(1);
        lab.set_normalise(false);
        assert!(lab.set_lattice("grid", 8, 8));
        assert!(lab.set_training("batch"));
        assert!(lab.set_data(&uniform_rgb(60_000, 3), 3));
        let mut epochs = 0;
        while lab.step(1) > 0 {
            epochs += 1;
        }
        assert!(epochs >= 10, "{epochs} epochs");
        assert!(lab.progress()[5] < 0.1, "TE {}", lab.progress()[5]);
    }

    #[test]
    fn missing_values_train() {
        let mut lab = SomData::new(1);
        let mut rng = Rng::new(5);
        let data: Vec<f32> = (0..300 * 4)
            .map(|_| {
                if rng.next_f32() < 0.2 {
                    f32::NAN
                } else {
                    rng.next_f32()
                }
            })
            .collect();
        // No row may be all NaN; fix the first column on the odd chance.
        let data: Vec<f32> = data
            .chunks(4)
            .flat_map(|r| {
                let mut r = r.to_vec();
                if r.iter().all(|v| v.is_nan()) {
                    r[0] = 0.5;
                }
                r
            })
            .collect();
        for training in ["online", "batch"] {
            for init in ["random", "pca", "cluster"] {
                assert!(lab.set_training(training) && lab.set_init(init));
                assert!(lab.set_data(&data, 4));
                lab.step(500);
                assert!(finite(&lab.codebook()), "{training} {init}");
                assert!(finite(&lab.progress()), "{training} {init}");
                assert!(finite(&lab.umatrix()), "{training} {init}");
            }
        }
    }

    /// The bundled country data, as the page feeds it: both years' rows, log10
    /// of the log-scaled indicators, null as NaN.
    fn country_rows() -> (Vec<f32>, usize, usize) {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/web/static/data/countries.json"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("countries.json")).unwrap();
        let logs: Vec<bool> = json["indicators"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["log"].as_bool().unwrap())
            .collect();
        let countries = json["countries"].as_array().unwrap();
        let mut rows = Vec::new();
        for year in ["y1992", "y2022"] {
            for c in countries {
                for (v, &log) in c[year].as_array().unwrap().iter().zip(&logs) {
                    rows.push(match v.as_f64() {
                        Some(v) if log => (v as f32).log10(),
                        Some(v) => v as f32,
                        None => f32::NAN,
                    });
                }
            }
        }
        (rows, logs.len(), countries.len())
    }

    #[test]
    fn the_country_data_trains_a_map() {
        let (rows, dim, countries) = country_rows();
        assert_eq!(dim, 10);
        assert!(countries >= 120, "{countries} countries");
        let mut lab = SomData::new(1);
        assert!(lab.set_lattice("hex", 16, 10));
        assert!(lab.set_data(&rows, dim as u32));
        while lab.step(1000) > 0 {}
        let te = lab.progress()[5];
        assert!(te < 0.1, "TE {te}");
        let bmus = lab.bmus();
        assert_eq!(bmus.len(), 2 * countries);
        assert!(bmus.iter().all(|&b| b < 160));
        assert!(lab.codebook().iter().all(|v| v.is_finite()));
    }
}
