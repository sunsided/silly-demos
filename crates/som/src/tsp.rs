//! The travelling salesman problem with elastic rings: a ring of nodes laid
//! over the cities is pulled onto them until it reads as a tour. Three such
//! methods, a Kohonen ring, Durbin and Willshaw's elastic net and
//! Angéniol's growing ring, beside the classical heuristics and an exact
//! solver for a handful of cities to measure them against.
//!
//! Every method works in the cities' unit square and is stepped a few
//! iterations at a time, so a page can draw it while it works.

pub mod baseline;
pub mod cities;
pub mod elastic;
pub mod exact;
pub mod growing;
pub mod ring_som;
pub mod shapes;
pub mod tour;

use crate::rng::Rng;
use baseline::{LocalSearch, nearest_neighbour};
use cities::{Cities, Matrix};
use elastic::Elastic;
use exact::{MAX_EXACT, held_karp};
use growing::Growing;
use ring_som::RingSom;

/// A method's tunable value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    /// The key the page passes the value by.
    pub id: &'static str,
    /// The name shown beside its slider.
    pub label: &'static str,
    /// The value a run starts with unless told otherwise.
    pub default: f32,
    /// The smallest value [`Run::new`] accepts.
    pub min: f32,
    /// The largest value [`Run::new`] accepts.
    pub max: f32,
}

impl Param {
    /// A parameter from its id, label, default and range; `const` so methods can list theirs in
    /// statics.
    pub const fn new(
        id: &'static str,
        label: &'static str,
        default: f32,
        min: f32,
        max: f32,
    ) -> Self {
        Self {
            id,
            label,
            default,
            min,
            max,
        }
    }
}

/// A way of finding a tour: one of the three rings or one of the baselines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Kohonen's self-organizing ring.
    RingSom,
    /// Durbin and Willshaw's elastic net.
    Elastic,
    /// Angéniol's ring that grows and prunes its nodes.
    Growing,
    /// Greedy nearest neighbor from the first city.
    NearestNeighbour,
    /// 2-opt and Or-opt local search from a nearest-neighbor start.
    LocalSearch,
    /// Held–Karp dynamic programming, for at most [`MAX_EXACT`] cities.
    Exact,
}

impl Method {
    /// Every method, in the order the page lists them.
    pub const ALL: [Method; 6] = [
        Method::RingSom,
        Method::Elastic,
        Method::Growing,
        Method::NearestNeighbour,
        Method::LocalSearch,
        Method::Exact,
    ];

    /// The method with this id; `None` for an unknown one.
    pub fn parse(id: &str) -> Option<Method> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// The id the page knows the method by.
    pub fn id(self) -> &'static str {
        match self {
            Method::RingSom => "ring-som",
            Method::Elastic => "elastic",
            Method::Growing => "growing",
            Method::NearestNeighbour => "nn",
            Method::LocalSearch => "two-opt",
            Method::Exact => "exact",
        }
    }

    /// The method's tunable values; empty for the baselines.
    pub fn params(self) -> &'static [Param] {
        match self {
            Method::RingSom => ring_som::PARAMS,
            Method::Elastic => elastic::PARAMS,
            Method::Growing => growing::PARAMS,
            _ => &[],
        }
    }

    /// Each parameter's default, in `params()` order.
    pub fn defaults(self) -> Vec<f32> {
        self.params().iter().map(|p| p.default).collect()
    }
}

/// One method working on one set of cities.
#[derive(Clone, Debug)]
pub struct Run {
    /// The cities being toured, in the page's coordinates.
    cities: Cities,
    /// The method and its progress.
    state: State,
}

/// Each method's working state.
#[derive(Clone, Debug)]
enum State {
    /// A Kohonen ring.
    RingSom(RingSom),
    /// An elastic net.
    Elastic(Elastic),
    /// A growing ring.
    Growing(Growing),
    /// A baseline: its distance matrix, and the tour once it has one.
    Greedy(Matrix, Option<Vec<u32>>),
    /// Local search: its distance matrix, the search, and the iterations run.
    LocalSearch(Matrix, LocalSearch, u64),
    /// The exact solver: its distance matrix, and the optimal tour once found.
    Exact(Matrix, Option<Vec<u32>>),
    /// The exact solver with more cities than it takes.
    TooBig,
}

impl Run {
    /// `values` follows `method.params()`; `None` if it doesn't fit them.
    pub fn new(method: Method, cities: Cities, values: &[f32], seed: u64) -> Option<Run> {
        let params = method.params();
        let fits = values.len() == params.len()
            && params
                .iter()
                .zip(values)
                .all(|(p, &v)| v.is_finite() && (p.min..=p.max).contains(&v));
        if !fits {
            return None;
        }
        let rng = Rng::new(seed);
        let unit = cities.unit();
        let state = match method {
            Method::RingSom => State::RingSom(RingSom::new(unit, values, rng)),
            Method::Elastic => State::Elastic(Elastic::new(unit, values)),
            Method::Growing => State::Growing(Growing::new(unit, values, rng)),
            Method::NearestNeighbour => State::Greedy(cities.matrix(), None),
            Method::LocalSearch => {
                let m = cities.matrix();
                let search = LocalSearch::new(&m);
                State::LocalSearch(m, search, 0)
            }
            Method::Exact if cities.len() > MAX_EXACT => State::TooBig,
            Method::Exact => State::Exact(cities.matrix(), None),
        };
        Some(Run { cities, state })
    }

    /// The cities being toured.
    pub fn cities(&self) -> &Cities {
        &self.cities
    }

    /// Runs up to `n` iterations; returns how many ran, 0 once done.
    pub fn step(&mut self, n: u32) -> u32 {
        let unit = self.cities.unit();
        let mut ran = 0;
        while ran < n && !self.done() {
            match &mut self.state {
                State::RingSom(s) => s.step(unit),
                State::Elastic(s) => s.step(unit),
                State::Growing(s) => s.step(unit),
                State::Greedy(m, tour) => *tour = Some(nearest_neighbour(m, 0)),
                State::LocalSearch(m, search, t) => {
                    search.step(m);
                    *t += 1;
                }
                State::Exact(m, tour) => *tour = held_karp(m).map(|(t, _)| t),
                State::TooBig => break,
            }
            ran += 1;
        }
        ran
    }

    /// Whether the method has nothing more to do.
    pub fn done(&self) -> bool {
        match &self.state {
            State::RingSom(s) => s.done(),
            State::Elastic(s) => s.done(),
            State::Growing(s) => s.done(),
            State::Greedy(_, tour) | State::Exact(_, tour) => tour.is_some(),
            State::LocalSearch(_, search, _) => search.done(),
            State::TooBig => true,
        }
    }

    /// Whether this is the exact solver turned down for having too many cities.
    pub fn too_big(&self) -> bool {
        matches!(self.state, State::TooBig)
    }

    /// Iterations run so far; a greedy or exact baseline counts its single pass as 1.
    pub fn iterations(&self) -> u64 {
        match &self.state {
            State::RingSom(s) => s.iterations(),
            State::Elastic(s) => s.iterations(),
            State::Growing(s) => s.iterations(),
            State::Greedy(_, tour) | State::Exact(_, tour) => tour.is_some() as u64,
            State::LocalSearch(_, _, t) => *t,
            State::TooBig => 0,
        }
    }

    /// The ring's nodes in the cities' coordinates; empty for a baseline.
    pub fn ring(&self) -> Vec<[f32; 2]> {
        let nodes = match &self.state {
            State::RingSom(s) => s.nodes.clone(),
            State::Elastic(s) => s.nodes.clone(),
            State::Growing(s) => s.nodes(),
            _ => return Vec::new(),
        };
        nodes.into_iter().map(|p| self.cities.to_input(p)).collect()
    }

    /// The current tour: read off the ring, or the baseline's so far
    /// (empty before a greedy or exact baseline has run, or when too big).
    pub fn tour(&self) -> Vec<u32> {
        let unit = self.cities.unit();
        match &self.state {
            State::RingSom(s) => tour::from_ring(unit, &s.nodes),
            State::Elastic(s) => tour::from_ring(unit, &s.nodes),
            State::Growing(s) => tour::from_ring(unit, &s.nodes()),
            State::Greedy(_, tour) | State::Exact(_, tour) => tour.clone().unwrap_or_default(),
            State::LocalSearch(_, search, _) => search.tour.clone(),
            State::TooBig => Vec::new(),
        }
    }

    /// The current tour's length in the cities' metric; NaN without one.
    pub fn length(&self) -> f64 {
        let tour = self.tour();
        if tour.is_empty() {
            f64::NAN
        } else {
            self.cities.tour_length(&tour)
        }
    }

    /// What the method is down to: σ (in nodes) for the Kohonen ring, K for
    /// the elastic net, the node count for the growing ring; NaN otherwise.
    pub fn readout(&self) -> f32 {
        match &self.state {
            State::RingSom(s) => s.sigma(),
            State::Elastic(s) => s.k,
            State::Growing(s) => s.len() as f32,
            _ => f32::NAN,
        }
    }
}

/// The mean position of the cities.
pub fn centroid(cities: &[[f32; 2]]) -> [f32; 2] {
    let n = cities.len().max(1) as f32;
    let (x, y) = cities
        .iter()
        .fold((0.0, 0.0), |(x, y), c| (x + c[0], y + c[1]));
    [x / n, y / n]
}

/// Where the fixed-size rings start: `m` nodes on a small circle round the
/// cities' centroid, radius 0.1 of the unit square.
pub fn centre_circle(cities: &[[f32; 2]], m: usize) -> Vec<[f32; 2]> {
    const RADIUS: f32 = 0.1;
    let c = centroid(cities);
    (0..m)
        .map(|j| {
            let a = std::f32::consts::TAU * j as f32 / m as f32;
            [c[0] + RADIUS * a.cos(), c[1] + RADIUS * a.sin()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cities::Metric;
    use tour::is_valid;

    const RINGS: [Method; 3] = [Method::RingSom, Method::Elastic, Method::Growing];

    fn cities(xy: Vec<f32>) -> Cities {
        Cities::new(&xy, Metric::Euclid).unwrap()
    }

    fn shape(kind: &str, n: usize, seed: u64) -> Cities {
        cities(shapes::generate(kind, n, &mut Rng::new(seed)).unwrap())
    }

    fn finish(method: Method, c: &Cities, seed: u64) -> Run {
        let mut run = Run::new(method, c.clone(), &method.defaults(), seed).unwrap();
        while run.step(10_000) > 0 {}
        assert!(run.done(), "{method:?}");
        run
    }

    #[test]
    fn methods_are_known_by_their_ids() {
        for m in Method::ALL {
            assert_eq!(Method::parse(m.id()), Some(m));
        }
        assert_eq!(Method::parse("lkh"), None);
    }

    #[test]
    fn parameters_out_of_range_are_refused() {
        let c = shape("uniform", 20, 1);
        assert!(Run::new(Method::Elastic, c.clone(), &[2.5, 0.2, 2.0, 0.2], 1).is_some());
        assert!(Run::new(Method::Elastic, c.clone(), &[2.5, 0.2, 2.0, 0.9], 1).is_none());
        assert!(Run::new(Method::Elastic, c.clone(), &[2.5, 0.2, 2.0], 1).is_none());
        assert!(Run::new(Method::Elastic, c.clone(), &[2.5, f32::NAN, 2.0, 0.2], 1).is_none());
        assert!(Run::new(Method::NearestNeighbour, c, &[], 1).is_some());
    }

    #[test]
    fn every_method_gives_a_valid_tour_no_shorter_than_the_optimum() {
        for seed in 0..3 {
            let c = shape("uniform", 10, seed);
            let best = finish(Method::Exact, &c, seed).length();
            for m in Method::ALL {
                let run = finish(m, &c, seed);
                assert!(is_valid(&run.tour(), 10), "{m:?} seed {seed}");
                assert!(run.length() >= best - 1e-6, "{m:?} seed {seed}");
            }
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_tour() {
        let c = shape("clusters", 40, 3);
        for m in Method::ALL {
            assert_eq!(finish(m, &c, 9).tour(), finish(m, &c, 9).tour(), "{m:?}");
        }
    }

    #[test]
    fn the_exact_solver_declines_a_big_set() {
        let mut run = Run::new(Method::Exact, shape("uniform", 14, 1), &[], 1).unwrap();
        assert!(run.too_big() && run.done());
        assert_eq!(run.step(1), 0);
        assert!(run.tour().is_empty() && run.length().is_nan());
    }

    #[test]
    fn rings_go_round_a_circle() {
        let c = shape("circle", 60, 1);
        let best = shapes::optimum("circle", 60).unwrap();
        for m in RINGS {
            let length = finish(m, &c, 1).length();
            assert!(length < best * 1.001, "{m:?}: {length} vs {best}");
        }
    }

    #[test]
    fn local_search_gets_near_a_grids_optimum() {
        let c = shape("grid", 100, 1);
        let best = shapes::optimum("grid", 100).unwrap();
        let length = finish(Method::LocalSearch, &c, 1).length();
        assert!(length < best * 1.05, "{length} vs {best}");
    }

    #[test]
    fn rings_stay_near_local_search_on_random_cities() {
        let c = shape("uniform", 100, 1);
        let reference = finish(Method::LocalSearch, &c, 1).length();
        for m in RINGS {
            let gap = finish(m, &c, 1).length() / reference - 1.0;
            // Measured at 2.3, 3.2 and −0.6 %.
            assert!(gap < 0.06, "{m:?}: {:.1} %", gap * 100.0);
        }
    }

    #[test]
    fn rings_are_drawn_where_the_cities_are() {
        let c = cities(vec![100.0, 100.0, 300.0, 100.0, 200.0, 300.0]);
        let run = Run::new(Method::Elastic, c, &Method::Elastic.defaults(), 1).unwrap();
        let ring = run.ring();
        assert!(
            ring.iter()
                .all(|p| (100.0..=300.0).contains(&p[0]) && (100.0..=300.0).contains(&p[1]))
        );
    }
}
