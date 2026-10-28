//! The exported API, compiled to wasm32 and run in Node by
//! `wasm-pack test --node` (`task test:wasm`).
//!
//! The unit tests cover the algorithms natively; these drive each class the
//! way the pages do, on the 32-bit target the site actually ships, where
//! `usize` is 32 bits wide and a panic aborts the module.

#![cfg(target_arch = "wasm32")]

use silly_demos::{
    BoidsParams, BoidsSim, ClusterLab, CollisionLab, GeometryTests, IkChain, RoverLab, SomData,
    SomLab, SuccessorLab, TspLab, VoronoiTests,
};
use wasm_bindgen_test::*;

/// `broadphase` flags of `CollisionLab::run`.
const CIRCLES: u32 = 1;
const BOXES: u32 = 2;
const QUADTREE: u32 = 4;

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

#[wasm_bindgen_test]
fn collision_methods_agree_on_overlapping_and_separate_shapes() {
    let mut lab = CollisionLab::new();
    // The default pair: two circles of radius 80, 280 apart.
    let ids = lab.methods();
    assert!(!ids.is_empty());
    for id in &ids {
        let r = lab.run(id, 0).expect("method of the current pair");
        assert!(!r.intersect(), "{id}");
        if let Some(d) = r.distance() {
            assert!((d - 120.0).abs() < 0.5, "{id}: {d}");
        }
    }
    // Move B over A: every method sees it, with or without the broadphase.
    assert!(lab.set_shape(1, "circle", &[-60.0, 0.0, 80.0]));
    for id in &lab.methods() {
        for broadphase in [0, CIRCLES | BOXES] {
            let r = lab.run(id, broadphase).unwrap();
            assert!(r.intersect(), "{id} with {broadphase}");
            assert!(finite(&r.normal()) && finite(&r.points()), "{id}");
            assert_eq!(r.counts().len(), 6);
        }
        assert_eq!(lab.bench(id, 0, 10), 10, "{id}");
    }
    assert!(lab.run("no-such-method", 0).is_none());
    assert!(!lab.set_shape(0, "no-such-shape", &[]));
}

#[wasm_bindgen_test]
fn bad_shape_parameters_are_refused_and_the_old_shape_kept() {
    let mut lab = CollisionLab::new();
    let kinds = CollisionLab::shape_kinds();
    assert!(kinds.len() >= 5);
    let before = lab.outline(0);
    for kind in &kinds {
        assert!(!lab.set_shape(0, kind, &[]), "{kind}: no parameters");
        assert!(!lab.set_shape(0, kind, &[f32::NAN; 12]), "{kind}: NaN");
        assert_eq!(lab.outline(0), before, "{kind}");
    }
    assert!(before.len() >= 6 && before.len().is_multiple_of(2) && finite(&before));
    for id in lab.methods() {
        assert!(!CollisionLab::method_name(&id).is_empty(), "{id}");
    }
}

#[wasm_bindgen_test]
fn the_quadtree_never_misses_an_overlap() {
    let mut lab = CollisionLab::new();
    lab.set_crowd(200, 800.0, 600.0, 7);
    for _ in 0..30 {
        lab.step_crowd(1.0 / 30.0);
    }
    let candidates = lab.bench_quadtree(1);
    let overlaps = lab.bench_brute_force(1);
    assert!(
        candidates >= overlaps,
        "{candidates} candidates for {overlaps} overlaps"
    );
    let r = lab.run(&lab.methods()[0], QUADTREE).unwrap();
    assert_eq!(r.scene().len(), 9);
    let outlines = lab.crowd_outlines();
    assert!(finite(&outlines));
}

#[wasm_bindgen_test]
fn point_to_segment_distance_and_side() {
    // Segment along the x axis; a point 3 above its middle and one past its end.
    let r = GeometryTests::point_line_test(0.0, 0.0, 10.0, 0.0, 5.0, -3.0);
    assert!((r.distance - 3.0).abs() < 1e-5);
    assert!(r.on_segment);
    assert_eq!((r.closest_x, r.closest_y), (5.0, 0.0));
    let past = GeometryTests::point_line_test(0.0, 0.0, 10.0, 0.0, 13.0, 4.0);
    assert!((past.distance - 5.0).abs() < 1e-5);
    assert!(!past.on_segment);
    assert!(r.side * GeometryTests::point_line_test(0.0, 0.0, 10.0, 0.0, 5.0, 3.0).side < 0.0);
}

#[wasm_bindgen_test]
fn boids_keep_their_snapshot_in_shape_and_their_speed_in_bounds() {
    let mut sim = BoidsSim::new(150, 800.0, 600.0, 42);
    let params = BoidsParams::new();
    sim.set_params(&params);
    for _ in 0..120 {
        sim.step(1.0 / 60.0);
    }
    let state = sim.state();
    assert_eq!(state.len(), 150 * 5);
    assert!(finite(&state));
    for b in state.chunks(5) {
        let speed = b[2].hypot(b[3]);
        assert!(speed <= params.max_speed + 1e-3, "{speed}");
    }
    assert!((0.0..=1.0).contains(&sim.polarization()));
    sim.set_count(20);
    assert_eq!(sim.count(), 20);
    assert_eq!(sim.state().len(), 20 * 5);
}

#[wasm_bindgen_test]
fn voronoi_and_delaunay_cover_the_points() {
    // 60 moving points, plus 4 fixed ones in the corners.
    let points = VoronoiTests::voronoi_create_points(60, 800.0, 600.0, 3, 20.0);
    let n = 64;
    assert_eq!(points.len(), n * 4);
    let moved = VoronoiTests::voronoi_step_points(&points, 800.0, 600.0, 0.5);
    assert_eq!(moved.len(), points.len());
    let tris = VoronoiTests::delaunay_indices(&moved);
    assert!(!tris.is_empty() && tris.len().is_multiple_of(3));
    assert!(tris.iter().all(|&i| (i as usize) < n));
    // A triangulation of n points has at most 2n − 5 triangles.
    assert!(tris.len() / 3 <= 2 * n - 5);
    let edges = VoronoiTests::voronoi_edges(&moved);
    assert!(!edges.is_empty() && edges.len().is_multiple_of(4));
    assert!(finite(&edges));
}

#[wasm_bindgen_test]
fn every_ik_solver_reaches_a_target_within_reach() {
    for id in IkChain::solvers() {
        let mut chain = IkChain::new(6, 45.0);
        chain.set_limits(Some(16.0), Some(11.0));
        for _ in 0..120 {
            chain.solve(&id, 120.0, -100.0, 10, 20.0);
            chain.follow(1.0 / 60.0, 0.0);
        }
        assert!(
            chain.error(120.0, -100.0) < 1.0,
            "{id}: {}",
            chain.error(120.0, -100.0)
        );
        let joints = chain.joints();
        assert_eq!(joints.len(), 7 * 2);
        // No joint below the floor, 11 below the base at the origin.
        assert!(
            joints.chunks(2).all(|p| p[1] <= 11.0 + 1e-3),
            "{id}: {joints:?}"
        );
    }
    assert_eq!(
        IkChain::new(6, 45.0).solve("no-such-solver", 1.0, 1.0, 10, 1.0),
        0
    );
}

#[wasm_bindgen_test]
fn the_rover_explores_the_floor_and_comes_home() {
    let mut lab = RoverLab::new("explorer");
    let grid = lab.grid();
    let cells = (grid[0] * grid[1]) as usize;
    // A fixed number of 1 s steps, so a clock that stopped advancing can't
    // keep the loop going.
    for _ in 0..900 {
        if lab.status() == "done" {
            break;
        }
        lab.step(1.0);
    }
    assert_eq!(lab.status(), "done", "not done after 900 s");
    let stats = lab.stats();
    assert!(stats[0] > 0.999, "explored {}", stats[0]);
    assert_eq!(stats[3], 0.0, "frontiers left");
    let (pose, home) = (lab.pose(), lab.home());
    assert!((pose[0] - home[0]).hypot(pose[1] - home[1]) < 0.25);
    // Snapshots the page reads every frame.
    assert_eq!(lab.knowledge().len(), cells);
    assert_eq!(lab.clearance().len(), cells);
    assert!(finite(&lab.trail()) && finite(&lab.ranges()));
    assert!(lab.path().is_empty() && lab.target().is_empty());
    // A finished run stands still.
    lab.step(5.0);
    assert_eq!(lab.stats()[2], stats[2]);
}

#[wasm_bindgen_test]
fn the_reactive_rover_moves_and_bad_inputs_are_harmless() {
    let mut lab = RoverLab::new("reactive");
    lab.set_laser_samples(0);
    lab.step(f32::NAN);
    lab.step(-1.0);
    assert_eq!(lab.stats()[2], 0.0);
    lab.set_laser_samples(121);
    for _ in 0..30 {
        lab.step(1.0);
    }
    assert!(lab.stats()[1] > 3.0, "drove {} m", lab.stats()[1]);
    assert_eq!(lab.ranges().len(), 121);
    lab.reset();
    assert_eq!(lab.stats()[2], 0.0);
}

#[wasm_bindgen_test]
fn the_unfolding_scenarios_behave_on_wasm32_as_natively() {
    // Classic unfold: a 12×12 grid orders itself.
    let mut classic = SomLab::new(1);
    while classic.step(1000) > 0 {}
    assert_eq!(classic.status(), "converged");
    assert!(classic.progress()[5] < 0.05, "TE {}", classic.progress()[5]);
    // Bow-tie, as the page sets it up: seed 17 freezes a twist in. The page
    // relies on runs being the same on every target.
    let mut bow_tie = SomLab::new(17);
    assert!(bow_tie.set_schedule(10_000, 0.5, 0.01, 1.0, 0.3, "exp", false));
    assert!(bow_tie.set_init("cluster"));
    while bow_tie.step(1000) > 0 {}
    assert!(bow_tie.progress()[5] > 0.1, "TE {}", bow_tie.progress()[5]);
    // The history keeps its newest point.
    let h = classic.history();
    assert_eq!(h[h.len() - 3], 20_000.0);
}

#[wasm_bindgen_test]
fn som_lab_refuses_what_it_cant_use() {
    let mut lab = SomLab::new(1);
    lab.step(10);
    let before = lab.weights();
    assert!(!lab.set_lattice("grid", 30, 30));
    assert!(!lab.set_lattice("ring", 2, 1));
    assert!(!lab.set_schedule(1000, 1.5, 0.01, 6.0, 0.5, "exp", false));
    assert!(!lab.set_density_cells(&[1.0; 15], 4));
    assert!(!lab.set_density_cells(&[f32::NAN; 16], 4));
    assert_eq!(lab.weights(), before);
    // An all-white image is an empty density, not an error.
    assert!(lab.set_density_cells(&[0.0; 16], 4));
    assert_eq!((lab.status().as_str(), lab.step(10)), ("empty", 0));
    lab.paint(0.5, 0.5, 0.2, 1.0);
    assert_eq!(lab.status(), "training");
    assert!(finite(&lab.weights()) && finite(&lab.voronoi()));
}

#[wasm_bindgen_test]
fn a_color_map_orders_itself_and_repaints_pixels() {
    let mut lab = SomData::new(1);
    assert_eq!(lab.status(), "empty");
    lab.set_normalize(false);
    assert!(lab.set_lattice("grid", 10, 10));
    // A fixed spread of colors over the cube.
    let colors: Vec<f32> = (0..2000u32)
        .flat_map(|i| {
            [
                (i * 37 % 256) as f32,
                (i * 91 % 256) as f32,
                (i * 173 % 256) as f32,
            ]
        })
        .collect();
    assert!(lab.set_data(&colors, 3));
    while lab.step(1000) > 0 {}
    assert!(lab.progress()[5] < 0.1, "TE {}", lab.progress()[5]);
    let out = lab.quantize(&[10, 200, 30, 99, 250, 250, 250, 7]);
    assert_eq!(out.len(), 8);
    assert_eq!((out[3], out[7]), (99, 7));
    assert_eq!(lab.hits().iter().sum::<u32>(), 2000);
    assert_eq!(lab.umatrix().len(), 100);
}

#[wasm_bindgen_test]
fn som_data_refuses_what_it_cant_use() {
    let mut lab = SomData::new(1);
    assert!(!lab.set_data(&[1.0, 2.0, 3.0], 2));
    assert!(!lab.set_data(&[f32::INFINITY, 1.0], 2));
    assert!(!lab.set_data(&[f32::NAN, f32::NAN], 2));
    assert!(!lab.set_lattice("chain", 10, 1));
    assert_eq!(lab.step(10), 0);
    assert!(lab.codebook().is_empty() && lab.plane(0).is_empty() && lab.bmus().is_empty());
}

#[wasm_bindgen_test]
fn the_country_data_trains_on_wasm32() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../web/static/data/countries.json")).unwrap();
    let logs: Vec<bool> = json["indicators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["log"].as_bool().unwrap())
        .collect();
    let countries = json["countries"].as_array().unwrap();
    let rows: Vec<f32> = ["y1992", "y2022"]
        .iter()
        .flat_map(|year| countries.iter().map(move |c| &c[*year]))
        .flat_map(|values| {
            values
                .as_array()
                .unwrap()
                .iter()
                .zip(&logs)
                .map(|(v, &log)| match v.as_f64() {
                    Some(v) if log => (v as f32).log10(),
                    Some(v) => v as f32,
                    None => f32::NAN,
                })
        })
        .collect();
    let mut lab = SomData::new(1);
    assert!(lab.set_lattice("hex", 16, 10));
    assert!(lab.set_data(&rows, logs.len() as u32));
    while lab.step(1000) > 0 {}
    assert!(lab.progress()[5] < 0.1, "TE {}", lab.progress()[5]);
    assert_eq!(lab.bmus().len(), 2 * countries.len());
    assert!(finite(&lab.codebook()));
}

/// The elastic net finds berlin52's optimum natively (src/tsp_lab.rs); the
/// same seed must take the same 7600 iterations to it here.
#[wasm_bindgen_test]
fn the_elastic_net_finds_berlin52s_optimum_on_wasm32_as_natively() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../web/static/data/tsplib.json")).unwrap();
    let berlin = data["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "berlin52")
        .unwrap();
    let xy: Vec<f32> = berlin["xy"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let mut lab = TspLab::new(1);
    assert!(lab.set_cities(&xy, "tsplib", &[]));
    assert!(lab.set_method("elastic"));
    while lab.step(1000) > 0 {}
    assert_eq!(lab.status(), "done");
    assert_eq!(lab.length(), 7542.0);
    assert_eq!(lab.iterations(), 7600.0);
}

#[wasm_bindgen_test]
fn tsp_lab_refuses_what_it_cant_use() {
    let mut lab = TspLab::new(1);
    assert_eq!(lab.status(), "empty");
    assert_eq!(lab.step(10), 0);
    let xy = TspLab::shape("uniform", 14, 1);
    assert!(lab.set_cities(&xy, "euclid", &[]));
    assert!(!lab.set_cities(&xy[..4], "euclid", &[]));
    assert!(!lab.set_cities(&[0.0, 0.0, 1.0, f32::NAN, 2.0, 2.0], "euclid", &[]));
    assert!(!lab.set_method("lkh"));
    assert!(!lab.set_param("k", -1.0));
    assert_eq!(lab.tour().len(), 14, "the 14 cities are kept");
    assert!(lab.set_method("exact"));
    assert_eq!(lab.status(), "too-big");
    assert!(lab.tour().is_empty());
}

/// GNG-U's point, on the target the site ships: after the left half of the
/// density is erased, it moves its nodes off it while plain GNG keeps them.
#[wasm_bindgen_test]
fn growing_gas_with_utility_follows_an_erased_half_on_wasm32() {
    let mut lab = SuccessorLab::new(3);
    while lab.progress()[0] < 20_000.0 {
        lab.step(5000);
    }
    for y in 0..=20 {
        lab.paint(0.2, y as f32 / 20.0, 0.3, -10.0);
    }
    while lab.progress()[0] < 60_000.0 {
        lab.step(5000);
    }
    let left = |id: &str| lab.weights(id).chunks(2).filter(|p| p[0] < 0.4).count();
    assert!(left("gng") > 10, "GNG keeps {}", left("gng"));
    assert!(left("gng-u") <= 3, "GNG-U keeps {}", left("gng-u"));
    assert!(lab.errors("kmeans")[1].is_nan());
    assert!(lab.weights("lvq").is_empty());
}

/// The clustering lab finds the five blobs on wasm32, and the drifting
/// story's split, as it does natively.
#[wasm_bindgen_test]
fn the_stream_is_clustered_on_wasm32_as_natively() {
    let mut lab = ClusterLab::new(1);
    lab.step(8000);
    let s = lab.stats();
    assert_eq!(&s[1..4], &[5.0, 5.0, 5.0], "true, balls, sketch");
    assert!(lab.set_scenario("drift"));
    lab.step(11_500);
    assert!(lab.event_counts("balls")[3] > 0.0, "a split");
    assert!(!lab.set_scenario("spiral"));
    assert!(lab.assignments("kmeans").is_empty());
    assert!(!lab.paint(2.0, 0.5));
}

/// On wasm32 `usize` is 32 bits: a grid this size has more cells than it can
/// count, and must say so rather than wrap around (it would not natively).
#[wasm_bindgen_test]
#[should_panic(expected = "grid cell count overflows usize")]
fn a_grid_too_big_for_32_bits_panics_instead_of_wrapping() {
    let _ = rover::grid::Grid::new(70_000, 70_000, 0.05, false);
}
