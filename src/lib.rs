mod boids;
mod collision;
mod geometry;
mod rand;
mod utils;
mod vec2;
mod voronoi;

use crate::collision::broadphase::{QUADTREE, Stage};
use crate::collision::probe::{Counts, Recorder, Silent};
use crate::collision::quadtree::{self, Quadtree};
use crate::collision::registry;
use crate::collision::report::Contact;
use crate::collision::scene::{self, Crowd, SceneStats};
use crate::collision::shape::{Body, Shape, ShapeKind};
use crate::utils::set_panic_hook;
use crate::vec2::Vec2;
use wasm_bindgen::prelude::*;

// Called by our JS entry point to run the example,
#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    set_panic_hook();

    Ok(())
}

/// Namespace for geometry tests
/// This empty struct serves as a namespace for geometry-related functions
/// when exported to WASM, providing clear module separation
#[wasm_bindgen]
pub struct GeometryTests;

/// Two shapes and the collision methods that apply to them.
#[wasm_bindgen]
pub struct CollisionLab {
    bodies: [Body; 2],
    crowd: Crowd,
}

#[wasm_bindgen]
impl CollisionLab {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let circle = |x| {
            Body::new(Shape::Circle {
                c: Vec2::new(x, 0.0),
                r: 80.0,
            })
        };
        Self {
            bodies: [circle(-140.0), circle(140.0)],
            crowd: Crowd::default(),
        }
    }

    /// Shape kind ids, in menu order.
    pub fn shape_kinds() -> Vec<String> {
        ShapeKind::ALL.iter().map(|k| k.id().to_owned()).collect()
    }

    /// Replaces shape `slot` (0 = A, 1 = B). See `Shape::from_params` for the
    /// parameter layout per kind. Returns false (and keeps the old shape) on bad input.
    pub fn set_shape(&mut self, slot: usize, kind: &str, params: &[f32]) -> bool {
        let shape = ShapeKind::from_id(kind).and_then(|k| Shape::from_params(k, params));
        match (self.bodies.get_mut(slot), shape) {
            (Some(body), Some(shape)) => {
                *body = Body::new(shape);
                true
            }
            _ => false,
        }
    }

    /// Boundary of shape `slot` as a flat closed polyline `[x0, y0, x1, y1, ...]`.
    pub fn outline(&self, slot: usize) -> Vec<f32> {
        self.bodies
            .get(slot)
            .map(|b| b.shape.outline().iter().flat_map(|v| [v.x, v.y]).collect())
            .unwrap_or_default()
    }

    /// Ids of the methods for the current pair, pair-specific ones first.
    pub fn methods(&self) -> Vec<String> {
        let [a, b] = self.bodies.map(|b| b.shape.kind());
        registry::methods_for(a, b)
            .map(|m| m.method.id.to_owned())
            .collect()
    }

    pub fn method_name(id: &str) -> String {
        registry::METHODS
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.name.to_owned())
            .unwrap_or_default()
    }

    pub fn method_summary(id: &str) -> String {
        registry::METHODS
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.summary.to_owned())
            .unwrap_or_default()
    }

    /// Runs method `id` with the broadphase stages in `broadphase`
    /// (1 = bounding circles, 2 = bounding boxes, 4 = scene quadtree), recording counts and notes.
    pub fn run(&self, id: &str, broadphase: u32) -> Option<CollisionReport> {
        let m = self.resolve(id)?;
        let [a, b] = &self.bodies;
        let mut rec = Recorder::default();
        let mut scene_notes = Vec::new();
        let (shared, scene) = if broadphase & QUADTREE != 0 {
            let (shared, stats) = scene::quadtree_stage(a, b, &self.crowd, &mut scene_notes);
            (shared, Some(stats))
        } else {
            (true, None)
        };
        let (contact, stage) = if shared {
            collision::test(&m, a, b, broadphase, &mut rec)
        } else {
            (Contact::boolean(false), Stage::Quadtree)
        };
        let mut notes = Vec::new();
        let mut labels = Vec::new();
        for note in scene_notes.iter().chain(&rec.notes) {
            note.encode(&mut notes, &mut labels);
        }
        Some(CollisionReport {
            contact,
            stage: stage as u32,
            counts: rec.counts,
            scene,
            notes,
            labels,
        })
    }

    /// Runs method `id` `n` times without instrumentation, for timing from JS.
    /// The quadtree stage is left out: it is a scene-wide cost, timed by `bench_quadtree`.
    /// Returns how many runs reported an intersection so the work can't be optimized away.
    pub fn bench(&self, id: &str, broadphase: u32, n: u32) -> u32 {
        let Some(m) = self.resolve(id) else {
            return 0;
        };
        let mut hits = 0;
        for _ in 0..n {
            let (a, b) = std::hint::black_box((&self.bodies[0], &self.bodies[1]));
            hits += collision::test(&m, a, b, broadphase & !QUADTREE, &mut Silent)
                .0
                .intersect as u32;
        }
        hits
    }

    /// Fills the scene with `count` small drifting shapes inside a
    /// `width` × `height` area centered on the origin.
    pub fn set_crowd(&mut self, count: usize, width: f32, height: f32, seed: u32) {
        let half = Vec2::new(
            (width * 0.5 - 12.0).max(1.0),
            (height * 0.5 - 12.0).max(1.0),
        );
        self.crowd = Crowd::new(count, half, seed);
    }

    /// Advances the crowd by `dt` seconds.
    pub fn step_crowd(&mut self, dt: f32) {
        self.crowd.step(dt);
    }

    /// Crowd outlines as `[n, x0, y0, ..., x(n-1), y(n-1)]` per body, back to back.
    pub fn crowd_outlines(&self) -> Vec<f32> {
        let mut out = Vec::new();
        for body in &self.crowd.bodies {
            let outline = body.shape.outline();
            out.push(outline.len() as f32);
            out.extend(outline.iter().flat_map(|v| [v.x, v.y]));
        }
        out
    }

    /// Builds the scene quadtree and collects its candidate pairs `n` times,
    /// for timing from JS. Returns the candidate count.
    pub fn bench_quadtree(&self, n: u32) -> u32 {
        let mut found = 0;
        for _ in 0..n {
            let [a, b] = std::hint::black_box(&self.bodies);
            let (boxes, root) = scene::boxes(a, b, &self.crowd);
            found = Quadtree::build(root, &boxes).pairs().0.len() as u32;
        }
        found
    }

    /// Tests every pair of bounding boxes in the scene `n` times, for timing from JS.
    pub fn bench_brute_force(&self, n: u32) -> u32 {
        let mut found = 0;
        for _ in 0..n {
            let [a, b] = std::hint::black_box(&self.bodies);
            let (boxes, _) = scene::boxes(a, b, &self.crowd);
            found = quadtree::brute_force(&boxes).len() as u32;
        }
        found
    }
}

impl CollisionLab {
    fn resolve(&self, id: &str) -> Option<registry::Resolved> {
        let [a, b] = self.bodies.map(|b| b.shape.kind());
        registry::find(id, a, b)
    }
}

impl Default for CollisionLab {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of one recorded collision test.
#[wasm_bindgen]
pub struct CollisionReport {
    contact: Contact,
    stage: u32,
    counts: Counts,
    scene: Option<SceneStats>,
    notes: Vec<f32>,
    labels: Vec<String>,
}

#[wasm_bindgen]
impl CollisionReport {
    #[wasm_bindgen(getter)]
    pub fn intersect(&self) -> bool {
        self.contact.intersect
    }

    /// Signed distance (negative = penetration depth), if the method measures it.
    #[wasm_bindgen(getter)]
    pub fn distance(&self) -> Option<f32> {
        self.contact.distance
    }

    /// Unit normal from A towards B as `[x, y]`, or empty.
    pub fn normal(&self) -> Vec<f32> {
        self.contact
            .normal
            .map(|n| vec![n.x, n.y])
            .unwrap_or_default()
    }

    /// Witness points `[ax, ay, bx, by]`, or empty.
    pub fn points(&self) -> Vec<f32> {
        self.contact
            .points
            .map(|(a, b)| vec![a.x, a.y, b.x, b.y])
            .unwrap_or_default()
    }

    /// 0 = rejected by bounding circles, 1 = by bounding boxes, 2 = reached the narrowphase,
    /// 3 = rejected by the quadtree (A and B share no leaf).
    #[wasm_bindgen(getter)]
    pub fn stage(&self) -> u32 {
        self.stage
    }

    /// `[dots, sqrts, divs, axes, supports, iterations]`.
    pub fn counts(&self) -> Vec<u32> {
        let c = self.counts;
        vec![c.dots, c.sqrts, c.divs, c.axes, c.supports, c.iterations]
    }

    /// Quadtree stats `[bodies, nodes, depth, checks, candidates, all_pairs, partners]`,
    /// or empty when the quadtree stage is off.
    pub fn scene(&self) -> Vec<u32> {
        self.scene
            .map(|s| {
                vec![
                    s.bodies,
                    s.nodes,
                    s.depth,
                    s.checks,
                    s.candidates,
                    s.all_pairs,
                    s.partners,
                ]
            })
            .unwrap_or_default()
    }

    /// Construction notes; see `Note::encode` for the layout.
    pub fn notes(&self) -> Vec<f32> {
        self.notes.clone()
    }

    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
}

#[wasm_bindgen]
pub struct PointLineResult {
    #[wasm_bindgen(readonly)]
    pub distance: f32,
    #[wasm_bindgen(readonly)]
    pub closest_x: f32,
    #[wasm_bindgen(readonly)]
    pub closest_y: f32,
    #[wasm_bindgen(readonly)]
    pub on_segment: bool,
    #[wasm_bindgen(readonly)]
    pub side: f32, // positive = right side, negative = left side, 0 = on line
}

/// Point-line distance and closest point calculation.
/// line_x1, line_y1: start point of line segment
/// line_x2, line_y2: end point of line segment  
/// point_x, point_y: the point to test
#[wasm_bindgen]
impl GeometryTests {
    pub fn point_line_test(
        line_x1: f32,
        line_y1: f32,
        line_x2: f32,
        line_y2: f32,
        point_x: f32,
        point_y: f32,
    ) -> PointLineResult {
        geometry::point_line_test_impl(line_x1, line_y1, line_x2, line_y2, point_x, point_y)
    }
}

/// Flocking parameters. Distances in px, speeds in px/s, forces in px/s².
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct BoidsParams {
    pub separation_radius: f32,
    pub alignment_radius: f32,
    pub cohesion_radius: f32,
    pub separation_weight: f32,
    pub alignment_weight: f32,
    pub cohesion_weight: f32,
    pub min_speed: f32,
    pub max_speed: f32,
    pub max_force: f32,
    pub edge_margin: f32,
    pub edge_weight: f32,
    pub wander: f32,
    /// Full field-of-view angle in degrees.
    pub view_angle: f32,
}

#[wasm_bindgen]
impl BoidsParams {
    /// Creates parameters with sensible defaults.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::from(boids::FlockParams::default())
    }
}

impl Default for BoidsParams {
    fn default() -> Self {
        Self::new()
    }
}

impl From<boids::FlockParams> for BoidsParams {
    fn from(p: boids::FlockParams) -> Self {
        Self {
            separation_radius: p.separation_radius,
            alignment_radius: p.alignment_radius,
            cohesion_radius: p.cohesion_radius,
            separation_weight: p.separation_weight,
            alignment_weight: p.alignment_weight,
            cohesion_weight: p.cohesion_weight,
            min_speed: p.min_speed,
            max_speed: p.max_speed,
            max_force: p.max_force,
            edge_margin: p.edge_margin,
            edge_weight: p.edge_weight,
            wander: p.wander,
            view_angle: p.view_angle,
        }
    }
}

impl From<&BoidsParams> for boids::FlockParams {
    fn from(p: &BoidsParams) -> Self {
        Self {
            separation_radius: p.separation_radius,
            alignment_radius: p.alignment_radius,
            cohesion_radius: p.cohesion_radius,
            separation_weight: p.separation_weight,
            alignment_weight: p.alignment_weight,
            cohesion_weight: p.cohesion_weight,
            min_speed: p.min_speed,
            max_speed: p.max_speed.max(p.min_speed),
            max_force: p.max_force,
            edge_margin: p.edge_margin,
            edge_weight: p.edge_weight,
            wander: p.wander,
            view_angle: p.view_angle,
        }
    }
}

/// A boids flock living in WASM memory. The simulation state stays on the
/// Rust side; JS only reads a flat snapshot for drawing.
#[wasm_bindgen]
pub struct BoidsSim {
    flock: boids::Flock,
}

#[wasm_bindgen]
impl BoidsSim {
    #[wasm_bindgen(constructor)]
    pub fn new(count: usize, width: f32, height: f32, seed: u32) -> Self {
        Self {
            flock: boids::Flock::new(count, width, height, seed as u64),
        }
    }

    pub fn set_params(&mut self, params: &BoidsParams) {
        self.flock.params = params.into();
    }

    pub fn set_count(&mut self, count: usize) {
        self.flock.set_count(count);
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.flock.resize(width, height);
    }

    /// Respawns every boid at a random position.
    pub fn scatter(&mut self) {
        self.flock.scatter();
    }

    /// Advances the simulation by `dt` seconds, in substeps of at most 1/60 s.
    pub fn step(&mut self, dt: f32) {
        let substeps = (dt * 60.0).ceil().clamp(1.0, 4.0) as usize;
        let h = dt.min(4.0 / 60.0) / substeps as f32;
        for _ in 0..substeps {
            self.flock.step(h);
        }
    }

    pub fn count(&self) -> usize {
        self.flock.boids.len()
    }

    /// How aligned the flock is, from 0 (random) to 1 (all flying the same way).
    pub fn polarization(&self) -> f32 {
        self.flock.polarization()
    }

    /// Flat snapshot with stride 5: `[x, y, vx, vy, neighbors, ...]`.
    pub fn state(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.flock.boids.len() * 5);
        for b in &self.flock.boids {
            out.extend_from_slice(&[b.pos.x, b.pos.y, b.vel.x, b.vel.y, b.neighbors as f32]);
        }
        out
    }
}
