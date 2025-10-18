mod utils;

use crate::utils::set_panic_hook;
use wasm_bindgen::prelude::*;

// Called by our JS entry point to run the example,
#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    set_panic_hook();

    Ok(())
}

#[wasm_bindgen]
pub struct CollisionResult {
    intersect: bool,
    distance: f32,
    dx: f32,
    dy: f32,
    penetration: f32,
}

#[wasm_bindgen]
impl CollisionResult {
    #[wasm_bindgen(getter)]
    pub fn intersect(&self) -> bool {
        self.intersect
    }

    #[wasm_bindgen(getter)]
    pub fn distance(&self) -> f32 {
        self.distance
    }

    #[wasm_bindgen(getter)]
    pub fn dx(&self) -> f32 {
        self.dx
    }

    #[wasm_bindgen(getter)]
    pub fn dy(&self) -> f32 {
        self.dy
    }

    #[wasm_bindgen(getter)]
    pub fn penetration(&self) -> f32 {
        self.penetration
    }
}

/// Circle–circle collision with distance and penetration.
/// Inputs and outputs are all f32 to keep the boundary cheap.
#[wasm_bindgen]
pub fn circle_collision(x1: f32, y1: f32, r1: f32, x2: f32, y2: f32, r2: f32) -> CollisionResult {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let d2 = dx * dx + dy * dy;

    // Avoid sqrt(0) + division noise; treat coincident centers as zero distance.
    let distance = if d2 > 0.0 { d2.sqrt() } else { 0.0 };

    let sum = r1 + r2;
    let intersect = distance <= sum;
    let penetration = if intersect { sum - distance } else { 0.0 };

    CollisionResult {
        intersect,
        distance,
        dx,
        dy,
        penetration,
    }
}
