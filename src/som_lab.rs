//! WASM bindings for the Kohonen lab (the `som` crate).

use som::codebook::Codebook;
use som::density::Density;
use som::init::{Init, initialise};
use som::lattice::{Kind, Lattice};
use som::metrics::{History, quantization_error, topographic_error};
use som::rng::Rng;
use som::schedule::{Decay, Schedule};
use som::train::{Som, StepTrace};
use wasm_bindgen::prelude::*;

use crate::voronoi;

/// Samples a batch epoch trains on, redrawn when the density changes.
const BATCH_SAMPLES: usize = 2000;
/// Samples the errors are measured on, redrawn when the density changes.
const EVAL_SAMPLES: usize = 1000;
/// Online samples between two measurements.
const METRIC_EVERY: u64 = 250;
/// History entries kept; when full, every other one is dropped.
const HISTORY_CAP: usize = 400;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Training {
    Online,
    Batch,
}

/// A self-organising map training on a 2D density. The map lives in WASM;
/// JS steps it, paints the density and reads flat snapshots for drawing.
#[wasm_bindgen]
pub struct SomLab {
    som: Som,
    init: Init,
    training: Training,
    density: Density,
    seed: u64,
    rng: Rng,
    batch: Vec<f32>,
    eval: Vec<f32>,
    trace: Option<StepTrace>,
    /// (t, QE, TE) triples.
    history: History,
    errors: (f32, f32),
}

#[wasm_bindgen]
impl SomLab {
    /// The classic unfolding: a 12×12 grid from random samples of a
    /// uniform square, α 0.5 → 0.01 and σ 6 → 0.5 over 20 000 samples.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let lattice = Lattice::new(Kind::Grid, 12, 12).expect("a 12×12 grid is in bounds");
        let mut lab = Self {
            som: Som {
                codebook: Codebook {
                    dim: 2,
                    weights: Vec::new(),
                },
                lattice,
                schedule: Schedule {
                    steps: 20_000,
                    alpha: (0.5, 0.01),
                    sigma: (6.0, 0.5),
                    decay: Decay::Exponential,
                    continuous: false,
                },
                t: 0,
            },
            init: Init::Random,
            training: Training::Online,
            density: Density::preset("uniform").expect("a known preset"),
            seed: seed as u64,
            rng: Rng::new(seed as u64),
            batch: Vec::new(),
            eval: Vec::new(),
            trace: None,
            history: History::new(HISTORY_CAP),
            errors: (0.0, 0.0),
        };
        lab.restart();
        lab
    }

    /// `kind` is "chain", "ring", "grid" or "hex"; chains and rings ignore
    /// `height`. Restarts. `false`, changing nothing, for an unknown kind or
    /// a size outside 2 to 400 nodes (a ring at least 3).
    pub fn set_lattice(&mut self, kind: &str, width: u32, height: u32) -> bool {
        let Some(lattice) =
            Kind::parse(kind).and_then(|k| Lattice::new(k, width as usize, height as usize))
        else {
            return false;
        };
        self.som.lattice = lattice;
        self.restart();
        true
    }

    /// "random", "cluster" or "pca". Restarts.
    pub fn set_init(&mut self, kind: &str) -> bool {
        let Some(init) = Init::parse(kind) else {
            return false;
        };
        self.init = init;
        self.restart();
        true
    }

    /// "online" (one sample per step) or "batch" (one epoch per step).
    /// Training continues from where it is.
    pub fn set_training(&mut self, mode: &str) -> bool {
        self.training = match mode {
            "online" => Training::Online,
            "batch" => Training::Batch,
            _ => return false,
        };
        true
    }

    /// α and σ from start to end over `steps` presented samples; `decay` is
    /// "exp" or "linear". A continuous schedule never finishes. Training
    /// continues from where it is. `false` for anything non-finite or not
    /// positive, α above 1, or no steps.
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

    /// Replaces the density: "uniform", "annulus", "moons", "blobs",
    /// "lshape", "gradient" or "blank". The map keeps training on it.
    pub fn set_density(&mut self, preset: &str) -> bool {
        let Some(density) = Density::preset(preset) else {
            return false;
        };
        self.density = density;
        self.draw_samples();
        self.measure(false);
        true
    }

    /// Replaces the density with `side`×`side` non-negative weights,
    /// row-major from y = 0 (an image's darkness, say), and starts over.
    /// `false`, changing nothing, for a side outside 2 to 256, the wrong
    /// number of cells, or a weight that is negative or not finite.
    pub fn set_density_cells(&mut self, cells: &[f32], side: u32) -> bool {
        let Some(density) = Density::from_cells(side as usize, cells) else {
            return false;
        };
        self.density = density;
        self.restart();
        true
    }

    /// Adds density within `radius` of (x, y) in the unit square; a negative
    /// `amount` erases. The first stroke on an empty sheet starts over, so
    /// the map starts from the painted samples.
    pub fn paint(&mut self, x: f32, y: f32, radius: f32, amount: f32) {
        let version = self.density.version();
        let was_empty = self.eval.is_empty();
        self.density.paint(x, y, radius, amount);
        if self.density.version() == version {
            return;
        }
        if was_empty {
            self.restart();
        } else {
            self.draw_samples();
            self.measure(false);
        }
    }

    /// Starts over with other random numbers; the density, painted
    /// strokes included, stays.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// Back to t = 0 with a freshly initialised map. The random numbers
    /// start over from the seed too, so a restart replays the same run.
    pub fn restart(&mut self) {
        self.rng = Rng::new(self.seed);
        self.draw_samples();
        self.som.codebook = initialise(self.init, &self.som.lattice, &self.eval, 2, &mut self.rng);
        self.som.t = 0;
        self.trace = None;
        self.history.clear();
        self.measure(true);
    }

    /// `n` online samples or `n` batch epochs; only the last is traced.
    /// Returns how many were done: fewer once the schedule finishes, 0
    /// without density.
    pub fn step(&mut self, n: u32) -> u32 {
        if self.eval.is_empty() {
            return 0;
        }
        let mut done = 0;
        while done < n && !self.som.finished() {
            let trace = done + 1 == n;
            let traced = match self.training {
                Training::Online => {
                    let Some(x) = self.density.sample(&mut self.rng) else {
                        break;
                    };
                    let traced = self.som.step_online(&x, trace);
                    if self.som.t.is_multiple_of(METRIC_EVERY) {
                        self.measure(true);
                    }
                    traced
                }
                Training::Batch => {
                    let traced = self.som.epoch_batch(&self.batch, trace);
                    self.measure(true);
                    traced
                }
            };
            if trace {
                self.trace = traced;
            }
            done += 1;
        }
        done
    }

    /// x, y per node.
    pub fn weights(&self) -> Vec<f32> {
        self.som.codebook.weights.clone()
    }

    /// Lattice links as pairs of node indices.
    pub fn edges(&self) -> Vec<u32> {
        self.som
            .lattice
            .edges()
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect()
    }

    /// Cells per side of the density.
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

    /// The Voronoi edges of the nodes, the regions each one quantizes to,
    /// as segments x1, y1, x2, y2.
    pub fn voronoi(&self) -> Vec<f32> {
        let points: Vec<(f32, f32)> = self
            .som
            .codebook
            .weights
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[x, y]| (x, y))
            .collect();
        voronoi::edges_of(&points)
    }

    /// The sample of the last traced online step, or empty.
    pub fn trace_sample(&self) -> Vec<f32> {
        self.trace
            .as_ref()
            .and_then(|t| t.sample.clone())
            .unwrap_or_default()
    }

    /// Its best-matching unit, or −1.
    pub fn trace_bmu(&self) -> i32 {
        self.trace
            .as_ref()
            .and_then(|t| t.bmu)
            .map_or(-1, |b| b as i32)
    }

    /// The kernel value h of every node in the last traced step, or empty.
    pub fn trace_kernel(&self) -> Vec<f32> {
        self.trace
            .as_ref()
            .map(|t| t.kernel.clone())
            .unwrap_or_default()
    }

    /// The weights before the last traced step, or empty.
    pub fn trace_previous(&self) -> Vec<f32> {
        self.trace
            .as_ref()
            .map(|t| t.previous.clone())
            .unwrap_or_default()
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

    /// "training", "converged", or "empty" when there is no density.
    pub fn status(&self) -> String {
        if self.eval.is_empty() {
            "empty"
        } else if self.som.finished() {
            "converged"
        } else {
            "training"
        }
        .into()
    }
}

impl SomLab {
    /// New batch and evaluation sets from the current density.
    fn draw_samples(&mut self) {
        let mut draw = |n: usize| -> Vec<f32> {
            (0..n)
                .map_while(|_| self.density.sample(&mut self.rng))
                .flatten()
                .collect()
        };
        self.batch = draw(BATCH_SAMPLES);
        self.eval = draw(EVAL_SAMPLES);
    }

    /// Updates the errors, and records them when `record`.
    fn measure(&mut self, record: bool) {
        let cb = &self.som.codebook;
        if cb.is_empty() {
            return;
        }
        self.errors = (
            quantization_error(cb, &self.eval),
            topographic_error(cb, &self.som.lattice, &self.eval),
        );
        if !record {
            return;
        }
        self.history
            .push(self.som.t as f32, self.errors.0, self.errors.1);
    }
}

impl Default for SomLab {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seed whose bow-tie run freezes a twist in.
    const BOW_TIE_SEED: u32 = 17;

    fn finite(v: &[f32]) -> bool {
        v.iter().all(|v| v.is_finite())
    }

    #[test]
    fn step_advances_and_reports() {
        let mut lab = SomLab::new(1);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.step(100), 100);
        assert_eq!(lab.progress()[0], 100.0);
        assert_eq!(lab.progress()[1], 20_000.0);
        assert_eq!(lab.weights().len(), 288);
        assert_eq!(lab.edges().len(), 2 * 264);
        assert_eq!(lab.density().len(), 64 * 64);
        assert_eq!(lab.density().iter().cloned().fold(0.0, f32::max), 1.0);
        assert!(finite(&lab.progress()));
    }

    #[test]
    fn invalid_setters_keep_state() {
        let mut lab = SomLab::new(1);
        lab.step(10);
        let before = lab.weights();
        assert!(!lab.set_lattice("nope", 3, 3));
        assert!(!lab.set_lattice("grid", 30, 30));
        assert!(!lab.set_init("x"));
        assert!(!lab.set_training("x"));
        assert!(!lab.set_density("x"));
        assert!(!lab.set_schedule(0, 0.5, 0.01, 6.0, 0.5, "exp", false));
        assert!(!lab.set_schedule(1000, f32::NAN, 0.01, 6.0, 0.5, "exp", false));
        assert!(!lab.set_schedule(1000, 0.5, 0.01, 6.0, 0.5, "cubic", false));
        assert_eq!(lab.weights(), before);
        assert_eq!(lab.progress()[0], 10.0);
    }

    #[test]
    fn erasing_all_density_stops_training() {
        let mut lab = SomLab::new(1);
        assert!(lab.set_density("blank"));
        assert_eq!(lab.status(), "empty");
        assert_eq!(lab.step(10), 0);
        lab.paint(0.5, 0.5, 0.2, 1.0);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.step(10), 10);
        lab.paint(0.5, 0.5, 0.5, -100.0);
        assert_eq!(lab.status(), "empty");
        assert_eq!(lab.step(10), 0);
        assert!(lab.set_training("batch"));
        assert_eq!(lab.step(1), 0);
        assert!(finite(&lab.weights()) && finite(&lab.progress()));
    }

    #[test]
    fn set_lattice_clears_trace() {
        let mut lab = SomLab::new(1);
        lab.step(5);
        assert_eq!(lab.trace_kernel().len(), 144);
        assert_eq!(lab.trace_previous().len(), 288);
        assert_eq!(lab.trace_sample().len(), 2);
        assert!(lab.trace_bmu() >= 0);
        assert!(lab.set_lattice("chain", 50, 1));
        assert!(lab.trace_kernel().is_empty() && lab.trace_previous().is_empty());
        assert_eq!(lab.trace_bmu(), -1);
        assert_eq!(lab.weights().len(), 100);
        assert_eq!(lab.edges().len(), 2 * 49);
        assert_eq!(lab.progress()[0], 0.0);
        lab.step(1);
        assert_eq!(lab.trace_kernel().len(), 50);
    }

    #[test]
    fn step_after_convergence_does_nothing() {
        let mut lab = SomLab::new(1);
        assert!(lab.set_training("batch"));
        assert!(lab.set_schedule(2000, 0.5, 0.01, 6.0, 0.5, "exp", false));
        assert_eq!(lab.step(1), 1);
        assert_eq!(lab.status(), "converged");
        let before = lab.weights();
        assert_eq!(lab.step(1), 0);
        assert_eq!(lab.weights(), before);
        // A batch trace has no sample.
        assert!(lab.trace_sample().is_empty());
        assert_eq!(lab.trace_bmu(), -1);
        assert_eq!(lab.trace_previous().len(), 288);
    }

    #[test]
    fn metrics_are_recorded_every_250_samples() {
        let mut lab = SomLab::new(1);
        // One entry for the start.
        assert_eq!(lab.history().len(), 3);
        lab.step(1000);
        let h = lab.history();
        assert_eq!(h.len(), 3 * 5);
        assert_eq!(h[3 * 4], 1000.0);
        assert!(finite(&h));
    }

    #[test]
    fn history_is_capped() {
        let mut lab = SomLab::new(1);
        assert!(lab.set_schedule(1000, 0.5, 0.01, 6.0, 0.5, "exp", true));
        for _ in 0..120 {
            assert_eq!(lab.step(1000), 1000);
        }
        assert_eq!(lab.status(), "training");
        let h = lab.history();
        assert!(h.len() / 3 <= 400 && h.len() / 3 > 200, "{}", h.len() / 3);
        // Still in time order, ending at the latest point.
        assert!(
            h.chunks(3)
                .zip(h.chunks(3).skip(1))
                .all(|(a, b)| a[0] < b[0])
        );
        assert_eq!(h[h.len() - 3], 120_000.0);
    }

    #[test]
    fn voronoi_has_edges() {
        let mut lab = SomLab::new(1);
        lab.step(2000);
        let v = lab.voronoi();
        assert!(!v.is_empty() && v.len().is_multiple_of(4));
        assert!(finite(&v));
    }

    #[test]
    fn restart_returns_to_the_start() {
        let mut lab = SomLab::new(3);
        lab.step(500);
        lab.restart();
        assert_eq!(lab.progress()[0], 0.0);
        assert_eq!(lab.history().len(), 3);
        assert!(lab.trace_kernel().is_empty());
        assert!(lab.set_init("cluster"));
        let w = lab.weights();
        let (mx, my) = (0.5, 0.5);
        assert!(w.chunks(2).all(|p| (p[0] - mx).hypot(p[1] - my) < 0.05));
    }

    /// The page's bow-tie scenario (kohonen.ts).
    fn bow_tie(seed: u32) -> SomLab {
        let mut lab = SomLab::new(seed);
        assert!(lab.set_density("uniform"));
        assert!(lab.set_schedule(10_000, 0.5, 0.01, 1.0, 0.3, "exp", false));
        assert!(lab.set_init("cluster"));
        assert!(lab.set_lattice("grid", 12, 12));
        lab.restart();
        lab
    }

    #[test]
    fn restart_replays_the_same_run() {
        let mut a = SomLab::new(9);
        a.step(300);
        a.restart();
        a.step(1000);
        // Same seed, other calls in between: the run depends only on the seed and settings.
        let mut b = SomLab::new(9);
        assert!(b.set_init("random"));
        b.paint(0.5, 0.5, 0.1, 0.0001);
        assert!(b.set_density("uniform"));
        b.restart();
        b.step(1000);
        assert_eq!(a.weights(), b.weights());
    }

    #[test]
    fn the_bow_tie_scenario_keeps_its_twist() {
        let mut lab = bow_tie(BOW_TIE_SEED);
        while lab.step(1000) > 0 {}
        assert_eq!(lab.status(), "converged");
        let te = lab.progress()[5];
        assert!(te > 0.1, "TE {te}");
    }

    #[test]
    fn the_first_stroke_on_an_empty_sheet_starts_over() {
        let mut lab = SomLab::new(1);
        lab.step(20_000);
        assert_eq!(lab.status(), "converged");
        assert!(lab.set_density("blank"));
        lab.paint(0.8, 0.8, 0.1, 1.0);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.progress()[0], 0.0);
        // Initialised from the painted samples, not piled up in the middle.
        assert!(
            lab.weights()
                .chunks(2)
                .all(|p| (p[0] - 0.8).hypot(p[1] - 0.8) < 0.12)
        );
        // Later strokes don't start over.
        lab.step(10);
        lab.paint(0.2, 0.2, 0.1, 1.0);
        assert_eq!(lab.progress()[0], 10.0);
    }

    #[test]
    fn a_new_seed_keeps_the_painted_density() {
        let mut lab = SomLab::new(1);
        assert!(lab.set_density("blank"));
        lab.paint(0.3, 0.3, 0.1, 1.0);
        lab.step(100);
        let density = lab.density();
        lab.set_seed(5);
        assert_eq!(lab.density(), density);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.progress()[0], 0.0);
        // Same as a lab made with that seed, painted the same.
        let mut fresh = SomLab::new(5);
        assert!(fresh.set_density("blank"));
        fresh.paint(0.3, 0.3, 0.1, 1.0);
        assert_eq!(lab.weights(), fresh.weights());
    }

    #[test]
    fn image_density_replaces_the_preset() {
        let mut lab = SomLab::new(1);
        assert_eq!(lab.density_side(), 64);
        let mut cells = vec![0.0; 128 * 128];
        cells[64 * 128 + 64] = 1.0;
        assert!(lab.set_density_cells(&cells, 128));
        assert_eq!(lab.density_side(), 128);
        assert_eq!(lab.density().len(), 128 * 128);
        assert_eq!(lab.status(), "training");
        assert_eq!(lab.progress()[0], 0.0);
    }

    #[test]
    fn an_empty_image_density_is_empty() {
        let mut lab = SomLab::new(1);
        assert!(lab.set_density_cells(&[0.0; 16], 4));
        assert_eq!(lab.status(), "empty");
        assert_eq!(lab.step(10), 0);
    }

    #[test]
    fn bad_image_density_is_refused() {
        let mut lab = SomLab::new(1);
        assert!(!lab.set_density_cells(&[1.0; 15], 4));
        assert!(!lab.set_density_cells(&[f32::NAN; 16], 4));
        assert!(!lab.set_density_cells(&[1.0; 4], 1));
        assert_eq!(lab.density_side(), 64);
    }
}
