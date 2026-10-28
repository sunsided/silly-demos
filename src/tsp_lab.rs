//! WASM bindings for the Kohonen lab's Tours mode (`som::tsp`).

use som::tsp::cities::{Cities, MAX_CITIES, Metric};
use som::tsp::{Method, Run, shapes};
use wasm_bindgen::prelude::*;

/// One method working on one set of cities: a ring pulled onto them, or a
/// classical heuristic. The page steps it and reads its ring and tour; a
/// race is one lab per method on the same cities.
#[wasm_bindgen]
pub struct TspLab {
    /// The cities to tour, or `None` before any are set.
    cities: Option<Cities>,
    /// The method each restart runs.
    method: Method,
    /// The method's parameter values, in the order of `method.params()`.
    values: Vec<f32>,
    /// Seed every restart starts the random numbers from.
    seed: u64,
    /// The run in progress, or `None` without cities.
    run: Option<Run>,
}

#[wasm_bindgen]
impl TspLab {
    /// No cities yet; the elastic net, with its defaults.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        Self {
            cities: None,
            method: Method::Elastic,
            values: Method::Elastic.defaults(),
            seed: seed as u64,
            run: None,
        }
    }

    /// x, y per city. `metric` is "euclid", "tsplib" (rounded, as TSPLIB's
    /// optima are measured) or "geo", where `lonlat` holds longitude and
    /// latitude per city and tours are measured in km; `lonlat` is ignored
    /// otherwise. Restarts. `false`, changing nothing, for fewer than 3 or
    /// more than 1000 cities, a non-finite value or an unknown metric.
    pub fn set_cities(&mut self, xy: &[f32], metric: &str, lonlat: &[f32]) -> bool {
        let metric = match metric {
            "euclid" => Metric::Euclid,
            "tsplib" => Metric::Tsplib,
            "geo" if lonlat.len() == xy.len() => Metric::Geo(
                lonlat
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[lon, lat]| [lon as f64, lat as f64])
                    .collect(),
            ),
            _ => return false,
        };
        let Some(cities) = Cities::new(xy, metric) else {
            return false;
        };
        self.cities = Some(cities);
        self.restart();
        true
    }

    /// "ring-som", "elastic", "growing", "nn" (nearest neighbor), "two-opt"
    /// (nearest neighbor, then 2-opt and Or-opt) or "exact" (Held–Karp, up
    /// to 13 cities). Resets the parameters to the method's defaults and
    /// restarts. `false`, changing nothing, for an unknown method.
    pub fn set_method(&mut self, id: &str) -> bool {
        let Some(method) = Method::parse(id) else {
            return false;
        };
        self.method = method;
        self.values = method.defaults();
        self.restart();
        true
    }

    /// The current method's parameters as JSON:
    /// `[{ "id", "label", "value", "default", "min", "max" }]`.
    pub fn params(&self) -> String {
        let rows: Vec<String> = self
            .method
            .params()
            .iter()
            .zip(&self.values)
            .map(|(p, v)| {
                format!(
                    r#"{{"id":"{}","label":"{}","value":{v},"default":{},"min":{},"max":{}}}"#,
                    p.id, p.label, p.default, p.min, p.max
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }

    /// Sets one of the current method's parameters and restarts. `false`,
    /// changing nothing, for an unknown id or a value out of its range.
    pub fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(k) = self.method.params().iter().position(|p| p.id == id) else {
            return false;
        };
        let p = self.method.params()[k];
        if !(value.is_finite() && (p.min..=p.max).contains(&value)) {
            return false;
        }
        self.values[k] = value;
        self.restart();
        true
    }

    /// Starts over with other random numbers.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed as u64;
        self.restart();
    }

    /// Back to the start; the random numbers start over from the seed, so
    /// a restart replays the same run.
    pub fn restart(&mut self) {
        self.run = self
            .cities
            .clone()
            .and_then(|c| Run::new(self.method, c, &self.values, self.seed));
    }

    /// Runs up to `n` iterations: for the Kohonen ring a city shown, for
    /// the elastic net a move of every node, for the growing ring a city
    /// presented, for 2-opt one position tried. Returns how many ran, 0
    /// once done or without cities.
    pub fn step(&mut self, n: u32) -> u32 {
        self.run.as_mut().map_or(0, |r| r.step(n))
    }

    /// x, y per ring node, in the cities' coordinates; empty for a
    /// heuristic.
    pub fn ring(&self) -> Vec<f32> {
        self.run
            .as_ref()
            .map_or_else(Vec::new, |r| r.ring().into_iter().flatten().collect())
    }

    /// The current tour as city indices; empty without one.
    pub fn tour(&self) -> Vec<u32> {
        self.run.as_ref().map_or_else(Vec::new, Run::tour)
    }

    /// The current tour's length in the metric; NaN without one.
    pub fn length(&self) -> f64 {
        self.run.as_ref().map_or(f64::NAN, Run::length)
    }

    /// Iterations run so far (a single pass counts as 1 for the greedy and
    /// exact methods); 0 without a run.
    pub fn iterations(&self) -> f64 {
        self.run.as_ref().map_or(0.0, |r| r.iterations() as f64)
    }

    /// σ in nodes for the Kohonen ring, K for the elastic net, the node
    /// count for the growing ring; NaN for a heuristic.
    pub fn readout(&self) -> f32 {
        self.run.as_ref().map_or(f32::NAN, Run::readout)
    }

    /// "empty" without cities, "too-big" for the exact solver above 13
    /// cities, then "ready", "running" and "done".
    pub fn status(&self) -> String {
        match &self.run {
            None => "empty",
            Some(r) if r.too_big() => "too-big",
            Some(r) if r.done() => "done",
            Some(r) if r.iterations() == 0 => "ready",
            Some(_) => "running",
        }
        .into()
    }

    /// `n` cities of `kind` ("uniform", "clusters", "circle", "grid" or
    /// "spiral") in a 0..1000 square, as x, y pairs; a grid rounds `n` to
    /// an even rectangle. Empty for an unknown kind or `n` outside 3..=1000.
    pub fn shape(kind: &str, n: u32, seed: u32) -> Vec<f32> {
        let n = n as usize;
        if n > MAX_CITIES {
            return Vec::new();
        }
        let xy =
            shapes::generate(kind, n, &mut som::rng::Rng::new(seed as u64)).unwrap_or_default();
        if xy.len() / 2 > MAX_CITIES {
            Vec::new()
        } else {
            xy
        }
    }

    /// The shortest tour of [`TspLab::shape`]'s `kind` with `n` cities
    /// where it is known in closed form (circle, grid); NaN otherwise.
    pub fn shape_optimum(kind: &str, n: u32) -> f64 {
        shapes::optimum(kind, n as usize).unwrap_or(f64::NAN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use som::tsp::tour::is_valid;

    struct Instance {
        name: String,
        optimum: f64,
        xy: Vec<f32>,
        opt_tour: Vec<u32>,
    }

    fn tsplib() -> Vec<Instance> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web/static/data/tsplib.json");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("tsplib.json")).unwrap();
        let numbers = |v: &serde_json::Value| -> Vec<f64> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect()
        };
        json["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| Instance {
                name: i["name"].as_str().unwrap().into(),
                optimum: i["optimum"].as_f64().unwrap(),
                xy: numbers(&i["xy"]).into_iter().map(|v| v as f32).collect(),
                opt_tour: numbers(&i["optTour"])
                    .into_iter()
                    .map(|v| v as u32)
                    .collect(),
            })
            .collect()
    }

    fn finish(lab: &mut TspLab) {
        while lab.step(10_000) > 0 {}
    }

    #[test]
    fn the_published_optimal_tours_measure_their_optima() {
        let instances = tsplib();
        assert_eq!(instances.len(), 4);
        for i in instances {
            let cities = Cities::new(&i.xy, Metric::Tsplib).unwrap();
            assert!(is_valid(&i.opt_tour, cities.len()), "{}", i.name);
            assert_eq!(cities.tour_length(&i.opt_tour), i.optimum, "{}", i.name);
        }
    }

    #[test]
    fn rings_get_close_to_the_tsplib_optima() {
        for i in tsplib() {
            for method in ["ring-som", "elastic", "growing", "two-opt"] {
                let mut lab = TspLab::new(1);
                assert!(lab.set_cities(&i.xy, "tsplib", &[]));
                assert!(lab.set_method(method));
                finish(&mut lab);
                assert_eq!(lab.status(), "done");
                let gap = lab.length() / i.optimum - 1.0;
                // Measured at 0–9.8 %.
                assert!(
                    (0.0..0.12).contains(&gap),
                    "{} {method}: {:.1} %",
                    i.name,
                    gap * 100.0
                );
            }
        }
    }

    #[test]
    fn refusals_keep_the_state() {
        let mut lab = TspLab::new(1);
        assert_eq!(lab.status(), "empty");
        assert!(lab.length().is_nan() && lab.tour().is_empty());
        let xy = TspLab::shape("uniform", 20, 1);
        assert!(lab.set_cities(&xy, "euclid", &[]));
        lab.step(5);
        assert_eq!(lab.status(), "running");
        assert!(!lab.set_cities(&xy[..4], "euclid", &[]));
        assert!(!lab.set_cities(&xy, "manhattan", &[]));
        assert!(!lab.set_cities(&xy, "geo", &xy[..10]));
        assert!(!lab.set_method("lkh"));
        assert!(!lab.set_param("k", 5.0));
        assert!(!lab.set_param("tension", 1.0));
        assert_eq!(lab.status(), "running");
        assert_eq!(lab.iterations(), 5.0);
    }

    #[test]
    fn parameters_come_as_json_and_reset_with_the_method() {
        let mut lab = TspLab::new(1);
        let params: serde_json::Value = serde_json::from_str(&lab.params()).unwrap();
        assert_eq!(params.as_array().unwrap().len(), 4);
        assert_eq!(params[3]["id"], "k");
        assert!(lab.set_param("k", 0.3));
        assert!(
            lab.params()
                .contains(r#""id":"k","label":"K start","value":0.3"#)
        );
        assert!(lab.set_method("elastic"));
        assert!(lab.params().contains(r#""value":0.2,"default":0.2"#));
        assert!(lab.set_method("nn"));
        assert_eq!(lab.params(), "[]");
    }

    #[test]
    fn the_exact_solver_reports_when_it_is_too_big() {
        let mut lab = TspLab::new(1);
        lab.set_method("exact");
        lab.set_cities(&TspLab::shape("uniform", 14, 1), "euclid", &[]);
        assert_eq!(lab.status(), "too-big");
        assert_eq!(lab.step(1), 0);
        lab.set_cities(&TspLab::shape("uniform", 13, 1), "euclid", &[]);
        assert_eq!(lab.status(), "ready");
        assert_eq!(lab.step(1), 1);
        assert_eq!(lab.status(), "done");
    }

    #[test]
    fn geo_tours_are_measured_in_km() {
        // Berlin, Paris, Rome.
        let lonlat = [13.405, 52.52, 2.3522, 48.8566, 12.4964, 41.9028];
        let mut lab = TspLab::new(1);
        assert!(lab.set_cities(&[528.0, 61.0, 497.0, 78.0, 526.0, 108.0], "geo", &lonlat));
        lab.set_method("nn");
        lab.step(1);
        // 878 + 1106 + 1181 km round the triangle.
        assert!((lab.length() - 3165.0).abs() < 30.0, "{}", lab.length());
    }

    #[test]
    fn shapes_refuse_what_they_cant_make() {
        assert!(TspLab::shape("blob", 20, 1).is_empty());
        assert!(TspLab::shape("uniform", 2, 1).is_empty());
        assert!(TspLab::shape("uniform", 1001, 1).is_empty());
        assert_eq!(TspLab::shape("grid", 100, 1).len(), 200);
        assert_eq!(TspLab::shape_optimum("grid", 100), 10_000.0);
        assert!(TspLab::shape_optimum("uniform", 100).is_nan());
    }

    /// tests/web.rs checks wasm32 gets exactly this too.
    #[test]
    fn the_elastic_net_finds_berlin52s_optimum() {
        let berlin = tsplib().into_iter().find(|i| i.name == "berlin52").unwrap();
        let mut lab = TspLab::new(1);
        lab.set_cities(&berlin.xy, "tsplib", &[]);
        finish(&mut lab);
        assert_eq!(lab.length(), 7542.0);
        assert_eq!(lab.iterations(), 7600.0);
    }
}
