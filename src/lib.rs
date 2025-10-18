mod utils;
mod collision;
mod geometry;

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
    pub intersect: bool,
    pub distance: f32,
    pub dx: f32,
    pub dy: f32,
    pub penetration: f32,
}

/// Circle–circle collision with distance and penetration.
/// Inputs and outputs are all f32 to keep the boundary cheap.
#[wasm_bindgen]
impl CollisionTests {
    pub fn circle_collision(x1: f32, y1: f32, r1: f32, x2: f32, y2: f32, r2: f32) -> CircleCollisionResult {
        collision::circle_collision_impl(x1, y1, r1, x2, y2, r2)
    }
}

#[wasm_bindgen]
pub struct PointLineResult {
    pub distance: f32,
    pub closest_x: f32,
    pub closest_y: f32,
    pub on_segment: bool,
    pub side: f32, // positive = right side, negative = left side, 0 = on line
}

/// Point-line distance and closest point calculation.
/// line_x1, line_y1: start point of line segment
/// line_x2, line_y2: end point of line segment  
/// point_x, point_y: the point to test
#[wasm_bindgen]
impl GeometryTests {
    pub fn point_line_test(
        line_x1: f32, line_y1: f32, 
        line_x2: f32, line_y2: f32,
        point_x: f32, point_y: f32
    ) -> PointLineResult {
        geometry::point_line_test_impl(line_x1, line_y1, line_x2, line_y2, point_x, point_y)
    }
}
