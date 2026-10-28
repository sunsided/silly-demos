//! WASM bindings for the clustering lab (the `stream` crate): one labeled
//! stream, clustered as it arrives by balls, by ellipses and by the sketch,
//! side by side, and scored against the labels.

use std::collections::VecDeque;

use stream::balls::{BallParams, Balls};
use stream::ellipses::{EllipseParams, Ellipses};
use stream::metrics::{adjusted_rand, batch_kmeans};
use stream::rng::Rng;
use stream::sketch::{Sketch, SketchParams};
use stream::source::{PAINTED, Scenario, Source};
use stream::{Log, Point};
use wasm_bindgen::prelude::*;

/// The newest points kept for drawing.
const WINDOW: usize = 1500;
/// The newest points the scores are measured on.
const SCORED: usize = 600;
/// Points between two measurements.
const MEASURE_EVERY: u64 = 100;
/// Measurements kept; when full, every other one is dropped.
const HISTORY_CAP: usize = 400;
/// Values per history entry: t, true k, each model's k, each model's
/// adjusted Rand index, and batch k-means' index.
const HISTORY_STRIDE: usize = 3 + 2 * MODELS.len();
/// Values per cluster in [`ClusterLab::clusters`].
const CLUSTER_STRIDE: usize = 8;

/// The models, in the order the page lays them out and the arrays list them.
const MODELS: [Model; 3] = [Model::Balls, Model::Ellipses, Model::Sketch];

/// One of the clusterers the lab runs side by side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Model {
    /// Round balls.
    Balls,
    /// Ellipses with a covariance each.
    Ellipses,
    /// The facility-location sketch refined by ball k-means.
    Sketch,
}

impl Model {
    /// The model for a UI id: "balls", "ellipses" or "sketch".
    fn parse(id: &str) -> Option<Model> {
        match id {
            "balls" => Some(Model::Balls),
            "ellipses" => Some(Model::Ellipses),
            "sketch" => Some(Model::Sketch),
            _ => None,
        }
    }
}

/// A tunable parameter: the model it belongs to and its range. All apply
/// at once, without starting over.
struct Param {
    /// The id `set_param` takes.
    id: &'static str,
    /// "balls", "ellipses", "sketch", or "all" for the shared forgetting.
    model: &'static str,
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

impl Param {
    /// A parameter; a `const fn` so the table below stays a table.
    const fn new(
        id: &'static str,
        model: &'static str,
        label: &'static str,
        default: f32,
        min: f32,
        max: f32,
        step: f32,
    ) -> Param {
        Param {
            id,
            model,
            label,
            default,
            min,
            max,
            step,
        }
    }
}

/// Every tunable parameter, with the defaults of [`BallParams`],
/// [`EllipseParams`] and [`SketchParams`].
const PARAMS: [Param; 17] = [
    Param::new(
        "balls.reach",
        "balls",
        "Reach, in radii",
        2.5,
        1.0,
        5.0,
        0.1,
    ),
    Param::new(
        "balls.r_init",
        "balls",
        "Starting radius",
        0.02,
        0.005,
        0.1,
        0.005,
    ),
    Param::new(
        "balls.merge",
        "balls",
        "Merge if the radius grows ≤ ×",
        1.2,
        1.0,
        2.0,
        0.05,
    ),
    Param::new(
        "balls.split",
        "balls",
        "Split at halves apart, × radii",
        1.6,
        1.0,
        4.0,
        0.1,
    ),
    Param::new(
        "balls.share",
        "balls",
        "Share of points to count",
        0.02,
        0.005,
        0.2,
        0.005,
    ),
    Param::new(
        "balls.prune",
        "balls",
        "Prune below weight",
        1.0,
        0.1,
        5.0,
        0.1,
    ),
    Param::new(
        "ellipses.reach",
        "ellipses",
        "Reach, in standard deviations",
        3.5,
        1.5,
        6.0,
        0.1,
    ),
    Param::new(
        "ellipses.s_init",
        "ellipses",
        "Starting spread",
        0.02,
        0.005,
        0.1,
        0.005,
    ),
    Param::new(
        "ellipses.merge",
        "ellipses",
        "Merge if the area is ≤ × both",
        1.0,
        0.5,
        2.0,
        0.05,
    ),
    Param::new(
        "ellipses.split",
        "ellipses",
        "Split at halves apart, in std devs",
        4.0,
        2.0,
        8.0,
        0.1,
    ),
    Param::new(
        "ellipses.share",
        "ellipses",
        "Share of points to count",
        0.02,
        0.005,
        0.2,
        0.005,
    ),
    Param::new(
        "ellipses.prune",
        "ellipses",
        "Prune below weight",
        1.0,
        0.1,
        5.0,
        0.1,
    ),
    Param::new(
        "sketch.cost",
        "sketch",
        "Price of a cluster λ",
        0.02,
        0.002,
        0.1,
        0.002,
    ),
    Param::new(
        "sketch.cap",
        "sketch",
        "Facilities kept",
        60.0,
        10.0,
        200.0,
        5.0,
    ),
    Param::new(
        "sketch.every",
        "sketch",
        "Points per refinement",
        200.0,
        50.0,
        1000.0,
        50.0,
    ),
    Param::new(
        "sketch.share",
        "sketch",
        "Share of points to count",
        0.02,
        0.005,
        0.2,
        0.005,
    ),
    Param::new(
        "decay",
        "all",
        "Forgetting per point",
        0.002,
        0.0,
        0.005,
        0.0001,
    ),
];

/// One stream clustered three ways. Each step draws a point and gives it to
/// every model; painting drops a point of no true cluster into all of them.
#[wasm_bindgen]
pub struct ClusterLab {
    /// What the stream looks like.
    scenario: Scenario,
    /// The seed every restart starts its random numbers from.
    seed: u64,
    /// The labeled stream.
    source: Source,
    /// The ball heuristic.
    balls: Balls,
    /// The same with ellipses.
    ellipses: Ellipses,
    /// The sketch and its clusters.
    sketch: Sketch,
    /// Random numbers for the batch k-means reference.
    rng: Rng,
    /// The newest points with their labels and when they came.
    window: VecDeque<(Point, u32, u64)>,
    /// Points given to the models so far, painted ones included.
    t: u64,
    /// The measurements, flat, [`HISTORY_STRIDE`] values each.
    history: Vec<f32>,
    /// The parameter values, ordered as [`PARAMS`].
    values: Vec<f32>,
}

#[wasm_bindgen]
impl ClusterLab {
    /// Five blobs, every model at its defaults.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let mut lab = Self {
            scenario: Scenario::Blobs,
            seed: seed as u64,
            source: Source::new(Scenario::Blobs, seed as u64),
            balls: Balls::new(BallParams::default()),
            ellipses: Ellipses::new(EllipseParams::default()),
            sketch: Sketch::new(SketchParams::default(), seed as u64),
            rng: Rng::new(seed as u64),
            window: VecDeque::new(),
            t: 0,
            history: Vec::new(),
            values: PARAMS.iter().map(|p| p.default).collect(),
        };
        lab.restart();
        lab
    }

    /// "blobs", "anisotropic", "rings", "moons" or "drift". Starts over.
    /// `false`, changing nothing, for another id.
    pub fn set_scenario(&mut self, id: &str) -> bool {
        let Some(scenario) = Scenario::parse(id) else {
            return false;
        };
        self.scenario = scenario;
        self.restart();
        true
    }

    /// Starts over with other random numbers.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// Back to an empty stream and fresh models, keeping the parameters.
    /// The random numbers start over from the seed, so it replays the same run.
    pub fn restart(&mut self) {
        self.source = Source::new(self.scenario, self.seed);
        self.balls = Balls::new(self.ball_params());
        self.ellipses = Ellipses::new(self.ellipse_params());
        self.sketch = Sketch::new(self.sketch_params(), self.seed ^ 0x5bd1_e995);
        self.rng = Rng::new(self.seed ^ 0x9e37_79b9);
        self.window.clear();
        self.t = 0;
        self.history.clear();
    }

    /// The parameters as JSON: `[{ "id", "model", "label", "value",
    /// "default", "min", "max", "step" }]`, `model` "balls", "ellipses",
    /// "sketch" or "all".
    pub fn params(&self) -> String {
        let rows: Vec<String> = PARAMS
            .iter()
            .zip(&self.values)
            .map(|(p, v)| {
                format!(
                    r#"{{"id":"{}","model":"{}","label":"{}","value":{v},"default":{},"min":{},"max":{},"step":{}}}"#,
                    p.id, p.model, p.label, p.default, p.min, p.max, p.step
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }

    /// Sets a parameter; it applies at once. `false`, changing nothing,
    /// for an unknown id or a value outside its range.
    pub fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(k) = PARAMS.iter().position(|p| p.id == id) else {
            return false;
        };
        let p = &PARAMS[k];
        if !(value.is_finite() && (p.min..=p.max).contains(&value)) {
            return false;
        }
        self.values[k] = value;
        self.balls.params = self.ball_params();
        self.ellipses.params = self.ellipse_params();
        self.sketch.params = self.sketch_params();
        true
    }

    /// Draws `n` points from the stream and gives each to every model.
    /// Returns `n`.
    pub fn step(&mut self, n: u32) -> u32 {
        for _ in 0..n {
            let (x, label) = self.source.next_point();
            self.take(x, label);
        }
        n
    }

    /// Drops a point at (x, y) in the unit square into every model, as if it
    /// had come from the stream; it belongs to no true cluster. `false` for
    /// a point outside the square or not finite.
    pub fn paint(&mut self, x: f32, y: f32) -> bool {
        if !(x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
        {
            return false;
        }
        self.take([x, y], PAINTED);
        true
    }

    /// The newest points, oldest first: x, y, age in points, and the true
    /// label (4294967295 for painted points).
    pub fn points(&self) -> Vec<f32> {
        self.window
            .iter()
            .flat_map(|&(p, label, t)| [p[0], p[1], (self.t - t) as f32, label as f32])
            .collect()
    }

    /// For each of [`ClusterLab::points`], the id of the cluster `model`
    /// ("balls", "ellipses" or "sketch") puts it in now, or 4294967295 for
    /// none; empty for an unknown model.
    pub fn assignments(&self, model: &str) -> Vec<u32> {
        let Some(m) = Model::parse(model) else {
            return Vec::new();
        };
        self.window
            .iter()
            .map(|&(p, _, _)| self.assign(m, p).unwrap_or(u32::MAX))
            .collect()
    }

    /// The clusters of `model`, light ones included, eight values each: id,
    /// x, y, the standard deviation along the long axis and along the
    /// short one (both the radius for balls), the long axis' angle from +x
    /// in radians, weight, and 1 if it has the weight to count, else 0.
    /// Empty for an unknown model.
    pub fn clusters(&self, model: &str) -> Vec<f32> {
        let mut out = Vec::with_capacity(CLUSTER_STRIDE * 64);
        match Model::parse(model) {
            Some(Model::Balls) => {
                let p = self.balls.params;
                let floor = self.balls.floor();
                for b in self.balls.balls() {
                    let r = b.radius(&p);
                    let counted = (b.weight >= floor) as u8 as f32;
                    out.extend([
                        b.id as f32,
                        b.center[0],
                        b.center[1],
                        r,
                        r,
                        0.0,
                        b.weight,
                        counted,
                    ]);
                }
            }
            Some(Model::Ellipses) => {
                let p = self.ellipses.params;
                let floor = self.ellipses.floor();
                for e in self.ellipses.ellipses() {
                    let (big, small, angle) = e.shape(&p).axes();
                    let c = e.center();
                    let counted = (e.weight() >= floor) as u8 as f32;
                    out.extend([
                        e.id as f32,
                        c[0],
                        c[1],
                        big.sqrt(),
                        small.sqrt(),
                        angle,
                        e.weight(),
                        counted,
                    ]);
                }
            }
            // Refinement dissolves the light clusters, so every one counts.
            Some(Model::Sketch) => {
                for c in self.sketch.all_clusters() {
                    let r = c.m2.sqrt();
                    out.extend([
                        c.id as f32,
                        c.center[0],
                        c.center[1],
                        r,
                        r,
                        0.0,
                        c.weight,
                        1.0,
                    ]);
                }
            }
            None => {}
        }
        out
    }

    /// The balls' reach in radii and the ellipses' in standard deviations,
    /// for drawing how far each takes points.
    pub fn reaches(&self) -> Vec<f32> {
        vec![self.balls.params.reach, self.ellipses.params.reach]
    }

    /// The sketch's facilities: x, y, weight.
    pub fn facilities(&self) -> Vec<f32> {
        self.sketch
            .facilities()
            .iter()
            .flat_map(|f| [f.center[0], f.center[1], f.weight])
            .collect()
    }

    /// The cost of opening a facility now.
    pub fn facility_cost(&self) -> f32 {
        self.sketch.facility_cost()
    }

    /// Recent events of `model`: kind (0 spawn, 1 merge, 2 prune, 3 split),
    /// x, y and how many points ago it happened. Empty for an unknown model.
    pub fn events(&self, model: &str) -> Vec<f32> {
        let Some(m) = Model::parse(model) else {
            return Vec::new();
        };
        self.log(m)
            .events
            .iter()
            .flat_map(|e| {
                [
                    e.kind as u8 as f32,
                    e.at[0],
                    e.at[1],
                    (self.t - e.t.min(self.t)) as f32,
                ]
            })
            .collect()
    }

    /// How the last point was placed by `model`, as JSON; "null" before any
    /// point or for an unknown model. For "balls" and "ellipses": `{ point,
    /// path, checked: [[id, distance]], fullScan, cluster, spawned }`, the
    /// distance Mahalanobis for ellipses; for "sketch": `{ point, nearest,
    /// d2, f, p, draw, opened, collapsed }`.
    pub fn trace(&self, model: &str) -> String {
        let walk = |point: Point,
                    path: &[u32],
                    checked: &[(u32, f32)],
                    full_scan: bool,
                    cluster: u32,
                    spawned: bool| {
            let path: Vec<String> = path.iter().map(u32::to_string).collect();
            let checked: Vec<String> = checked
                .iter()
                .map(|(id, d)| format!("[{id},{d}]"))
                .collect();
            format!(
                r#"{{"point":[{},{}],"path":[{}],"checked":[{}],"fullScan":{},"cluster":{},"spawned":{}}}"#,
                point[0],
                point[1],
                path.join(","),
                checked.join(","),
                full_scan,
                cluster,
                spawned
            )
        };
        match Model::parse(model) {
            Some(Model::Balls) => self.balls.trace.as_ref().map_or_else(
                || "null".into(),
                |t| walk(t.point, &t.path, &t.checked, t.full_scan, t.ball, t.spawned),
            ),
            Some(Model::Ellipses) => self.ellipses.trace.as_ref().map_or_else(
                || "null".into(),
                |t| walk(t.point, &t.path, &t.checked, t.full_scan, t.ellipse, t.spawned),
            ),
            Some(Model::Sketch) => self.sketch.trace.as_ref().map_or_else(
                || "null".into(),
                |t| {
                    let nearest = t.nearest.map_or_else(|| "null".into(), |n| format!("[{},{}]", n[0], n[1]));
                    let finite = |v: f32| if v.is_finite() { v.to_string() } else { "null".into() };
                    format!(
                        r#"{{"point":[{},{}],"nearest":{},"d2":{},"f":{},"p":{},"draw":{},"opened":{},"collapsed":{}}}"#,
                        t.point[0],
                        t.point[1],
                        nearest,
                        finite(t.d2),
                        t.f,
                        t.p,
                        t.draw,
                        t.opened,
                        t.collapsed
                    )
                },
            ),
            None => "null".into(),
        }
    }

    /// How many spawns, merges, prunes and splits `model` has had in all;
    /// empty for an unknown model.
    pub fn event_counts(&self, model: &str) -> Vec<f64> {
        Model::parse(model).map_or_else(Vec::new, |m| {
            self.log(m).counts.iter().map(|&c| c as f64).collect()
        })
    }

    /// The measurements over time, flat, nine values each: points so far,
    /// the true number of clusters, the number the balls, the ellipses and
    /// the sketch find, and the adjusted Rand index of the balls, the
    /// ellipses, the sketch and batch k-means told the true number, on the
    /// newest 600 points.
    pub fn history(&self) -> Vec<f32> {
        self.history.clone()
    }

    /// [points so far, true clusters now], then for the balls, the ellipses
    /// and the sketch in turn: [clusters found, distances per point computed
    /// to place points, distances per point computed to keep links (0 for
    /// the sketch), clusters in all, light ones included].
    pub fn stats(&self) -> Vec<f32> {
        let per = |c: u64| {
            if self.t == 0 {
                0.0
            } else {
                c as f32 / self.t as f32
            }
        };
        let mut out = vec![self.t as f32, self.source.active() as f32];
        out.extend([
            self.balls.clusters().count() as f32,
            per(self.balls.search_checks),
            per(self.balls.upkeep_checks),
            self.balls.balls().len() as f32,
        ]);
        out.extend([
            self.ellipses.clusters().count() as f32,
            per(self.ellipses.search_checks),
            per(self.ellipses.upkeep_checks),
            self.ellipses.ellipses().len() as f32,
        ]);
        out.extend([
            self.sketch.clusters().count() as f32,
            per(self.sketch.search_checks),
            0.0,
            self.sketch.all_clusters().len() as f32,
        ]);
        out
    }
}

impl ClusterLab {
    /// The balls' parameters from the current values.
    fn ball_params(&self) -> BallParams {
        let v = |id| self.value(id);
        BallParams {
            reach: v("balls.reach"),
            r_init: v("balls.r_init"),
            r_min: BallParams::default().r_min,
            merge: v("balls.merge"),
            decay: v("decay"),
            share: v("balls.share"),
            prune: v("balls.prune"),
            split: v("balls.split"),
        }
    }

    /// The ellipses' parameters from the current values.
    fn ellipse_params(&self) -> EllipseParams {
        let v = |id| self.value(id);
        EllipseParams {
            reach: v("ellipses.reach"),
            s_init: v("ellipses.s_init"),
            s_min: EllipseParams::default().s_min,
            merge: v("ellipses.merge"),
            decay: v("decay"),
            share: v("ellipses.share"),
            prune: v("ellipses.prune"),
            split: v("ellipses.split"),
        }
    }

    /// The sketch's parameters from the current values.
    fn sketch_params(&self) -> SketchParams {
        let v = |id| self.value(id);
        SketchParams {
            cluster_cost: v("sketch.cost"),
            cap: v("sketch.cap").round() as usize,
            growth: SketchParams::default().growth,
            decay: v("decay"),
            share: v("sketch.share"),
            every: v("sketch.every").round() as u64,
        }
    }

    /// The value of parameter `id`.
    fn value(&self, id: &str) -> f32 {
        self.values[PARAMS
            .iter()
            .position(|p| p.id == id)
            .expect("a known parameter")]
    }

    /// The cluster `model` puts `x` in now.
    fn assign(&self, model: Model, x: Point) -> Option<u32> {
        match model {
            Model::Balls => self.balls.assign(x),
            Model::Ellipses => self.ellipses.assign(x),
            Model::Sketch => self.sketch.assign(x),
        }
    }

    /// How many clusters `model` counts now.
    fn count(&self, model: Model) -> usize {
        match model {
            Model::Balls => self.balls.clusters().count(),
            Model::Ellipses => self.ellipses.clusters().count(),
            Model::Sketch => self.sketch.clusters().count(),
        }
    }

    /// `model`'s events.
    fn log(&self, model: Model) -> &Log {
        match model {
            Model::Balls => &self.balls.log,
            Model::Ellipses => &self.ellipses.log,
            Model::Sketch => &self.sketch.log,
        }
    }

    /// Gives one point to every model and the window, measuring now and then.
    fn take(&mut self, x: Point, label: u32) {
        self.balls.insert(x);
        self.ellipses.insert(x);
        self.sketch.insert(x);
        self.t += 1;
        self.window.push_back((x, label, self.t));
        if self.window.len() > WINDOW {
            self.window.pop_front();
        }
        if self.t.is_multiple_of(MEASURE_EVERY) {
            self.measure();
        }
    }

    /// Scores every model and batch k-means on the newest points with a
    /// true label, and records the counts.
    fn measure(&mut self) {
        let scored: Vec<(Point, u32)> = self
            .window
            .iter()
            .rev()
            .filter(|(_, label, _)| *label != PAINTED)
            .take(SCORED)
            .map(|&(p, l, _)| (p, l))
            .collect();
        let truth: Vec<u32> = scored.iter().map(|&(_, l)| l).collect();
        let points: Vec<Point> = scored.iter().map(|&(p, _)| p).collect();
        let mut distinct = truth.clone();
        distinct.sort_unstable();
        distinct.dedup();
        let batch = batch_kmeans(&points, distinct.len(), &mut self.rng);
        self.history
            .extend([self.t as f32, self.source.active() as f32]);
        for m in MODELS {
            self.history.push(self.count(m) as f32);
        }
        for m in MODELS {
            let labels: Vec<u32> = points
                .iter()
                .map(|&p| self.assign(m, p).unwrap_or(u32::MAX))
                .collect();
            self.history.push(adjusted_rand(&labels, &truth));
        }
        self.history.push(adjusted_rand(&batch, &truth));
        let n = self.history.len() / HISTORY_STRIDE;
        if n > HISTORY_CAP {
            // Keep every other entry, counted back from the newest.
            self.history = self
                .history
                .chunks(HISTORY_STRIDE)
                .enumerate()
                .filter(|(i, _)| (n - 1 - i).is_multiple_of(2))
                .flat_map(|(_, c)| c.to_vec())
                .collect();
        }
    }
}

impl Default for ClusterLab {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The last history entry: [t, true k, k balls, k ellipses, k sketch,
    /// ARI balls, ARI ellipses, ARI sketch, ARI batch k-means].
    fn last(lab: &ClusterLab) -> Vec<f32> {
        let h = lab.history();
        h[h.len() - HISTORY_STRIDE..].to_vec()
    }

    #[test]
    fn every_model_finds_the_five_blobs_and_scores_like_batch_kmeans() {
        let mut lab = ClusterLab::new(1);
        lab.step(8000);
        let h = last(&lab);
        assert_eq!(
            &h[1..5],
            &[5.0, 5.0, 5.0, 5.0],
            "true, balls, ellipses, sketch"
        );
        assert!(h[5..9].iter().all(|&a| a > 0.95), "ARI {:?}", &h[5..9]);
    }

    #[test]
    fn ellipses_hold_long_thin_clusters_that_balls_break() {
        let mut lab = ClusterLab::new(1);
        assert!(lab.set_scenario("anisotropic"));
        lab.step(15_000);
        let h = last(&lab);
        assert_eq!(h[3], 4.0, "ellipses");
        assert!(h[2] > 4.0, "balls {}", h[2]);
        assert!(
            h[6] > 0.9 && h[6] > h[5] + 0.2,
            "ARI ellipses {} vs balls {}",
            h[6],
            h[5]
        );
    }

    #[test]
    fn rings_are_where_balls_struggle() {
        let mut lab = ClusterLab::new(1);
        assert!(lab.set_scenario("rings"));
        lab.step(8000);
        // A ring has no center inside it: balls cut rings into arcs or wrap
        // both in one ball, and either way the score is poor.
        assert!(last(&lab)[5] < 0.5, "balls ARI {}", last(&lab)[5]);
    }

    #[test]
    fn the_drifting_story_is_followed() {
        let mut lab = ClusterLab::new(2);
        assert!(lab.set_scenario("drift"));
        // Into the last third of a cycle: four clusters, one just born.
        lab.step(11_500);
        let h = last(&lab);
        assert_eq!(h[1], 4.0);
        for (k, ari, name) in [(h[2], h[5], "balls"), (h[3], h[6], "ellipses")] {
            assert!((k - 4.0).abs() <= 1.0, "{name} {k}");
            assert!(ari > 0.8, "{name} ARI {ari}");
        }
    }

    #[test]
    fn refusals_change_nothing() {
        let mut lab = ClusterLab::new(1);
        lab.step(10);
        assert!(!lab.set_scenario("spiral"));
        assert!(!lab.set_param("balls.speed", 1.0));
        assert!(!lab.set_param("decay", 1.0));
        assert!(!lab.paint(1.5, 0.5));
        assert!(!lab.paint(f32::NAN, 0.5));
        assert_eq!(lab.stats()[0], 10.0);
        assert!(lab.assignments("kmeans").is_empty());
        assert!(lab.clusters("kmeans").is_empty());
        assert!(lab.events("kmeans").is_empty());
        assert_eq!(lab.trace("kmeans"), "null");
    }

    #[test]
    fn painted_points_reach_every_model_but_not_the_scores() {
        let mut lab = ClusterLab::new(1);
        for _ in 0..200 {
            assert!(lab.paint(0.95, 0.05));
        }
        let pts = lab.points();
        assert_eq!(pts.len(), 200 * 4);
        assert_eq!(pts[3], PAINTED as f32);
        for model in ["balls", "ellipses"] {
            let near = lab
                .clusters(model)
                .chunks(CLUSTER_STRIDE)
                .any(|c| (c[1] - 0.95).abs() < 0.02);
            assert!(near, "{model}");
        }
        // All points painted: nothing to score, so ARI of an empty set is 1.
        assert_eq!(last(&lab)[5], 1.0);
    }

    #[test]
    fn traces_clusters_and_params_have_their_shapes() {
        let mut lab = ClusterLab::new(1);
        assert_eq!(lab.trace("balls"), "null");
        lab.step(50);
        for model in ["balls", "ellipses", "sketch"] {
            let v: serde_json::Value = serde_json::from_str(&lab.trace(model)).unwrap();
            assert!(v["point"].is_array(), "{model}");
            assert_eq!(lab.clusters(model).len() % CLUSTER_STRIDE, 0, "{model}");
        }
        assert_eq!(lab.stats().len(), 2 + 4 * MODELS.len());
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
        assert!(lab.set_param("ellipses.reach", 3.0));
        assert_eq!(lab.ellipses.params.reach, 3.0);
    }
}
