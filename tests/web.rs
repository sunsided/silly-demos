//! The exported API, compiled to wasm32 and run in Node by
//! `wasm-pack test --node` (`task test:wasm`).
//!
//! The unit tests cover the algorithms natively; these drive each class the
//! way the pages do, on the 32-bit target the site actually ships, where
//! `usize` is 32 bits wide and a panic aborts the module.

#![cfg(target_arch = "wasm32")]

use silly_demos::{
    BoidsParams, BoidsSim, CollisionLab, GeometryTests, IkChain, RoverLab, VoronoiTests,
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
    while lab.status() != "done" {
        lab.step(1.0);
        assert!(lab.stats()[2] < 900.0, "not done after 900 s");
    }
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

/// On wasm32 `usize` is 32 bits: a grid this size has more cells than it can
/// count, and must say so rather than wrap around (it would not natively).
#[wasm_bindgen_test]
#[should_panic(expected = "grid cell count overflows usize")]
fn a_grid_too_big_for_32_bits_panics_instead_of_wrapping() {
    let _ = rover::grid::Grid::new(70_000, 70_000, 0.05, false);
}
