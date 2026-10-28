//! WASM bindings for the clustering lab (the `stream` crate): one labeled
//! stream, clustered as it arrives by the ball heuristic and by the
//! sketch, side by side, and scored against the labels.

use std::collections::VecDeque;

use stream::balls::{BallParams, Balls};
use stream::metrics::{adjusted_rand, batch_kmeans};
use stream::rng::Rng;
use stream::sketch::{Sketch, SketchParams};
use stream::source::{PAINTED, Scenario, Source};
use stream::{Event, Point};
use wasm_bindgen::prelude::*;

/// The newest points kept for drawing.
const WINDOW: usize = 1500;
/// The newest points the scores are measured on.
const SCORED: usize = 600;
/// Points between two measurements.
const MEASURE_EVERY: u64 = 100;
/// Measurements kept; when full, every other one is dropped.
const HISTORY_CAP: usize = 400;
/// Values per history entry: t, true k, k of the balls, k of the sketch,
/// and the adjusted Rand index of the balls, the sketch and batch k-means.
const HISTORY_STRIDE: usize = 7;

/// A tunable parameter: the model it belongs to and its range. All apply
/// at once, without starting over.
struct Param {
    /// The id `set_param` takes.
    id: &'static str,
    /// "balls", "sketch", or "both" for the shared forgetting.
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

/// Every tunable parameter, with the defaults of [`BallParams`] and [`SketchParams`].
const PARAMS: [Param; 11] = [
    Param {
        id: "balls.reach",
        model: "balls",
        label: "Reach, in radii",
        default: 2.5,
        min: 1.0,
        max: 5.0,
        step: 0.1,
    },
    Param {
        id: "balls.r_init",
        model: "balls",
        label: "Starting radius",
        default: 0.02,
        min: 0.005,
        max: 0.1,
        step: 0.005,
    },
    Param {
        id: "balls.merge",
        model: "balls",
        label: "Merge if the radius grows ≤ ×",
        default: 1.2,
        min: 1.0,
        max: 2.0,
        step: 0.05,
    },
    Param {
        id: "balls.share",
        model: "balls",
        label: "Share of points to count",
        default: 0.02,
        min: 0.005,
        max: 0.2,
        step: 0.005,
    },
    Param {
        id: "balls.prune",
        model: "balls",
        label: "Prune below weight",
        default: 1.0,
        min: 0.1,
        max: 5.0,
        step: 0.1,
    },
    Param {
        id: "balls.split",
        model: "balls",
        label: "Split at halves apart, × radii",
        default: 1.6,
        min: 1.0,
        max: 4.0,
        step: 0.1,
    },
    Param {
        id: "sketch.cost",
        model: "sketch",
        label: "Price of a cluster λ",
        default: 0.02,
        min: 0.002,
        max: 0.1,
        step: 0.002,
    },
    Param {
        id: "sketch.cap",
        model: "sketch",
        label: "Facilities kept",
        default: 60.0,
        min: 10.0,
        max: 200.0,
        step: 5.0,
    },
    Param {
        id: "sketch.every",
        model: "sketch",
        label: "Points per refinement",
        default: 200.0,
        min: 50.0,
        max: 1000.0,
        step: 50.0,
    },
    Param {
        id: "sketch.share",
        model: "sketch",
        label: "Share of points to count",
        default: 0.02,
        min: 0.005,
        max: 0.2,
        step: 0.005,
    },
    Param {
        id: "decay",
        model: "both",
        label: "Forgetting per point",
        default: 0.002,
        min: 0.0,
        max: 0.005,
        step: 0.0001,
    },
];

/// One stream clustered two ways. Each step draws a point and gives it to
/// both models; painting drops a point of no true cluster into both.
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
    /// Five blobs, both models at their defaults.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let mut lab = Self {
            scenario: Scenario::Blobs,
            seed: seed as u64,
            source: Source::new(Scenario::Blobs, seed as u64),
            balls: Balls::new(BallParams::default()),
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
        self.sketch = Sketch::new(self.sketch_params(), self.seed ^ 0x5bd1_e995);
        self.rng = Rng::new(self.seed ^ 0x9e37_79b9);
        self.window.clear();
        self.t = 0;
        self.history.clear();
    }

    /// The parameters as JSON: `[{ "id", "model", "label", "value",
    /// "default", "min", "max", "step" }]`, `model` "balls", "sketch" or "both".
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
        self.sketch.params = self.sketch_params();
        true
    }

    /// Draws `n` points from the stream and gives each to both models.
    /// Returns `n`.
    pub fn step(&mut self, n: u32) -> u32 {
        for _ in 0..n {
            let (x, label) = self.source.next_point();
            self.take(x, label);
        }
        n
    }

    /// Drops a point at (x, y) in the unit square into both models, as if it
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
    /// ("balls" or "sketch") puts it in now, or 4294967295 for none; empty
    /// for an unknown model.
    pub fn assignments(&self, model: &str) -> Vec<u32> {
        let assign = |p: Point| match model {
            "balls" => self.balls.assign(p),
            _ => self.sketch.assign(p),
        };
        if model != "balls" && model != "sketch" {
            return Vec::new();
        }
        self.window
            .iter()
            .map(|&(p, _, _)| assign(p).unwrap_or(u32::MAX))
            .collect()
    }

    /// The clusters of `model`, light ones included: id, x, y, radius,
    /// weight, and 1 if it has the weight to count, else 0. Empty for an
    /// unknown model.
    pub fn clusters(&self, model: &str) -> Vec<f32> {
        match model {
            "balls" => {
                let p = self.balls.params;
                let floor = self.balls.floor();
                self.balls
                    .balls()
                    .iter()
                    .flat_map(|b| {
                        let counted = (b.weight >= floor) as u8 as f32;
                        [
                            b.id as f32,
                            b.center[0],
                            b.center[1],
                            b.radius(&p),
                            b.weight,
                            counted,
                        ]
                    })
                    .collect()
            }
            // Refinement dissolves the light clusters, so every one counts.
            "sketch" => self
                .sketch
                .all_clusters()
                .iter()
                .flat_map(|c| {
                    [
                        c.id as f32,
                        c.center[0],
                        c.center[1],
                        c.m2.sqrt(),
                        c.weight,
                        1.0,
                    ]
                })
                .collect(),
            _ => Vec::new(),
        }
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

    /// Recent events of `model`: kind (0 spawn, 1 merge, 2 prune, 3 split), x, y and
    /// how many points ago it happened. Empty for an unknown model.
    pub fn events(&self, model: &str) -> Vec<f32> {
        let events: &[Event] = match model {
            "balls" => &self.balls.log.events,
            "sketch" => &self.sketch.log.events,
            _ => return Vec::new(),
        };
        events
            .iter()
            .flat_map(|e| {
                [
                    e.kind as u8 as f32,
                    e.at[0],
                    e.at[1],
                    (self.balls.t().max(self.t) - e.t) as f32,
                ]
            })
            .collect()
    }

    /// How the last point was placed by `model`, as JSON; "null" before any
    /// point or for an unknown model. For "balls": `{ point, path, checked:
    /// [[id, distance]], fullScan, ball, spawned }`; for "sketch": `{ point,
    /// nearest, d2, f, p, draw, opened, collapsed }`.
    pub fn trace(&self, model: &str) -> String {
        match model {
            "balls" => self.balls.trace.as_ref().map_or_else(
                || "null".into(),
                |t| {
                    let path: Vec<String> = t.path.iter().map(u32::to_string).collect();
                    let checked: Vec<String> = t.checked.iter().map(|(id, d)| format!("[{id},{d}]")).collect();
                    format!(
                        r#"{{"point":[{},{}],"path":[{}],"checked":[{}],"fullScan":{},"ball":{},"spawned":{}}}"#,
                        t.point[0],
                        t.point[1],
                        path.join(","),
                        checked.join(","),
                        t.full_scan,
                        t.ball,
                        t.spawned
                    )
                },
            ),
            "sketch" => self.sketch.trace.as_ref().map_or_else(
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
            _ => "null".into(),
        }
    }

    /// How many spawns, merges, prunes and splits `model` has had in all;
    /// empty for an unknown model.
    pub fn event_counts(&self, model: &str) -> Vec<f64> {
        let counts = match model {
            "balls" => self.balls.log.counts,
            "sketch" => self.sketch.log.counts,
            _ => return Vec::new(),
        };
        counts.iter().map(|&c| c as f64).collect()
    }

    /// The measurements over time, flat, seven values each: points so far,
    /// the true number of clusters, the balls' and the sketch's number, and
    /// the adjusted Rand index of the balls, the sketch and batch k-means
    /// told the true number, on the newest 600 points.
    pub fn history(&self) -> Vec<f32> {
        self.history.clone()
    }

    /// [points so far, true clusters now, the balls' clusters, the sketch's
    /// clusters, distances per point the balls computed to place points,
    /// distances per point they computed to keep their links, distances per
    /// point the sketch computed, balls in all].
    pub fn stats(&self) -> Vec<f32> {
        let per = |c: u64| {
            if self.t == 0 {
                0.0
            } else {
                c as f32 / self.t as f32
            }
        };
        vec![
            self.t as f32,
            self.source.active() as f32,
            self.balls.clusters().count() as f32,
            self.sketch.clusters().count() as f32,
            per(self.balls.search_checks),
            per(self.balls.upkeep_checks),
            per(self.sketch.search_checks),
            self.balls.balls().len() as f32,
        ]
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

    /// Gives one point to both models and the window, measuring now and then.
    fn take(&mut self, x: Point, label: u32) {
        self.balls.insert(x);
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

    /// Scores both models and batch k-means on the newest points with a
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
        let none = u32::MAX;
        let balls: Vec<u32> = points
            .iter()
            .map(|&p| self.balls.assign(p).unwrap_or(none))
            .collect();
        let sketch: Vec<u32> = points
            .iter()
            .map(|&p| self.sketch.assign(p).unwrap_or(none))
            .collect();
        let mut distinct = truth.clone();
        distinct.sort_unstable();
        distinct.dedup();
        let batch = batch_kmeans(&points, distinct.len(), &mut self.rng);
        self.history.extend([
            self.t as f32,
            self.source.active() as f32,
            self.balls.clusters().count() as f32,
            self.sketch.clusters().count() as f32,
            adjusted_rand(&balls, &truth),
            adjusted_rand(&sketch, &truth),
            adjusted_rand(&batch, &truth),
        ]);
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

    /// The last history entry.
    fn last(lab: &ClusterLab) -> Vec<f32> {
        let h = lab.history();
        h[h.len() - HISTORY_STRIDE..].to_vec()
    }

    #[test]
    fn both_find_the_five_blobs_and_score_like_batch_kmeans() {
        let mut lab = ClusterLab::new(1);
        lab.step(8000);
        let h = last(&lab);
        assert_eq!(&h[1..4], &[5.0, 5.0, 5.0], "true, balls, sketch");
        assert!(
            h[4] > 0.95 && h[5] > 0.95 && h[6] > 0.95,
            "ARI {:?}",
            &h[4..7]
        );
    }

    #[test]
    fn rings_are_where_balls_struggle() {
        let mut lab = ClusterLab::new(1);
        assert!(lab.set_scenario("rings"));
        lab.step(8000);
        let h = last(&lab);
        // A ring has no center inside it: balls cut rings into arcs or wrap
        // both in one ball, and either way the score is poor.
        assert!(h[4] < 0.5, "balls ARI {}", h[4]);
    }

    #[test]
    fn the_drifting_story_is_followed() {
        let mut lab = ClusterLab::new(2);
        assert!(lab.set_scenario("drift"));
        // Into the last third of a cycle: four clusters, one just born.
        lab.step(11_500);
        let h = last(&lab);
        assert_eq!(h[1], 4.0);
        assert!((h[2] - 4.0).abs() <= 1.0, "balls {}", h[2]);
        assert!(h[4] > 0.8, "balls ARI {}", h[4]);
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
        assert_eq!(lab.trace("kmeans"), "null");
    }

    #[test]
    fn painted_points_reach_both_models_but_not_the_scores() {
        let mut lab = ClusterLab::new(1);
        for _ in 0..200 {
            assert!(lab.paint(0.95, 0.05));
        }
        let pts = lab.points();
        assert_eq!(pts.len(), 200 * 4);
        assert_eq!(pts[3], PAINTED as f32);
        assert!(
            lab.clusters("balls")
                .chunks(6)
                .any(|c| (c[1] - 0.95).abs() < 0.02)
        );
        // All points painted: nothing to score, so ARI of an empty set is 1.
        assert_eq!(last(&lab)[4], 1.0);
    }

    #[test]
    fn traces_and_params_are_json() {
        let mut lab = ClusterLab::new(1);
        assert_eq!(lab.trace("balls"), "null");
        lab.step(50);
        for model in ["balls", "sketch"] {
            let v: serde_json::Value = serde_json::from_str(&lab.trace(model)).unwrap();
            assert!(v["point"].is_array(), "{model}");
        }
        let params: serde_json::Value = serde_json::from_str(&lab.params()).unwrap();
        assert_eq!(params.as_array().unwrap().len(), 11);
        for p in &PARAMS {
            let k = (p.default - p.min) / p.step;
            assert!(
                (k - k.round()).abs() < 1e-3,
                "{} default off its steps",
                p.id
            );
        }
        assert!(lab.set_param("balls.reach", 3.0));
        assert_eq!(lab.balls.params.reach, 3.0);
    }
}
