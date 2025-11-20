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

/// Namespace for boids simulation
/// This empty struct serves as a namespace for boids-related functions
/// when exported to WASM, providing clear module separation
#[wasm_bindgen]
pub struct BoidsTests;

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

/// Boids simulation update function.
/// Uses flat arrays to avoid complex WASM struct passing that can cause memory issues.
#[wasm_bindgen]
impl BoidsTests {
    /// Update boids simulation using flat arrays
    /// Input: [x1, y1, vx1, vy1, x2, y2, vx2, vy2, ...]
    /// Returns: [x1, y1, vx1, vy1, x2, y2, vx2, vy2, ...]
    pub fn update_boids_flat(
        boids_data: &[f32], // Flat array: [x, y, vx, vy, x, y, vx, vy, ...]
        separation_radius: f32,
        alignment_radius: f32,
        cohesion_radius: f32,
        separation_strength: f32,
        alignment_strength: f32,
        cohesion_strength: f32,
        max_speed: f32,
        max_force: f32,
        boundary_margin: f32,
        boundary_strength: f32,
        world_width: f32,
        world_height: f32,
        dt: f32,
        min_speed: f32,
        jitter: f32,
    ) -> Vec<f32> {
        boids::update_boids_flat_impl(
            boids_data,
            separation_radius,
            alignment_radius,
            cohesion_radius,
            separation_strength,
            alignment_strength,
            cohesion_strength,
            max_speed,
            max_force,
            boundary_margin,
            boundary_strength,
            world_width,
            world_height,
            dt,
            min_speed,
            jitter,
        )
    }

    /// Create N random boids as a flat array
    /// Returns: [x1, y1, vx1, vy1, x2, y2, vx2, vy2, ...]
    pub fn create_boids_flat(
        count: usize,
        min_x: f32,
        max_x: f32,
        min_y: f32,
        max_y: f32,
        max_speed: f32,
    ) -> Vec<f32> {
        use js_sys::Math;

        let mut result = Vec::with_capacity(count * 4);

        for _ in 0..count {
            let x = min_x + Math::random() as f32 * (max_x - min_x);
            let y = min_y + Math::random() as f32 * (max_y - min_y);

            let angle = Math::random() as f32 * 2.0 * std::f32::consts::PI;
            let speed = Math::random() as f32 * max_speed;

            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed;

            result.push(x);
            result.push(y);
            result.push(vx);
            result.push(vy);
        }

        result
    }
}
