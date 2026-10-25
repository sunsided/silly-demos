mod boids;
mod collision;
mod geometry;
mod rand;
mod utils;
mod vec2;
mod voronoi;

use crate::utils::set_panic_hook;
use wasm_bindgen::prelude::*;

// Called by our JS entry point to run the example,
#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    set_panic_hook();

    Ok(())
}

/// Namespace for collision detection tests
/// This empty struct serves as a namespace for collision-related functions
/// when exported to WASM, allowing for better organization and avoiding
/// global namespace pollution in JavaScript/TypeScript
#[wasm_bindgen]
pub struct CollisionTests;

/// Namespace for geometry tests  
/// This empty struct serves as a namespace for geometry-related functions
/// when exported to WASM, providing clear module separation
#[wasm_bindgen]
pub struct GeometryTests;

#[wasm_bindgen]
pub struct CircleCollisionResult {
    #[wasm_bindgen(readonly)]
    pub intersect: bool,
    #[wasm_bindgen(readonly)]
    pub distance: f32,
    #[wasm_bindgen(readonly)]
    pub dx: f32,
    #[wasm_bindgen(readonly)]
    pub dy: f32,
    #[wasm_bindgen(readonly)]
    pub penetration: f32,
}

/// Circle–circle collision with distance and penetration.
/// Inputs and outputs are all f32 to keep the boundary cheap.
#[wasm_bindgen]
impl CollisionTests {
    pub fn circle_collision(
        x1: f32,
        y1: f32,
        r1: f32,
        x2: f32,
        y2: f32,
        r2: f32,
    ) -> CircleCollisionResult {
        collision::circle_collision_impl(x1, y1, r1, x2, y2, r2)
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
