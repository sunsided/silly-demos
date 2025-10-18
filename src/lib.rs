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

#[wasm_bindgen]
pub struct PointLineResult {
    distance: f32,
    closest_x: f32,
    closest_y: f32,
    on_segment: bool,
    side: f32, // positive = right side, negative = left side, 0 = on line
}

#[wasm_bindgen]
impl PointLineResult {
    #[wasm_bindgen(getter)]
    pub fn distance(&self) -> f32 {
        self.distance
    }

    #[wasm_bindgen(getter)]
    pub fn closest_x(&self) -> f32 {
        self.closest_x
    }

    #[wasm_bindgen(getter)]
    pub fn closest_y(&self) -> f32 {
        self.closest_y
    }

    #[wasm_bindgen(getter)]
    pub fn on_segment(&self) -> bool {
        self.on_segment
    }

    #[wasm_bindgen(getter)]
    pub fn side(&self) -> f32 {
        self.side
    }
}

/// Point-line distance and closest point calculation.
/// line_x1, line_y1: start point of line segment
/// line_x2, line_y2: end point of line segment  
/// point_x, point_y: the point to test
#[wasm_bindgen]
pub fn point_line_test(
    line_x1: f32, line_y1: f32, 
    line_x2: f32, line_y2: f32,
    point_x: f32, point_y: f32
) -> PointLineResult {
    let dx = line_x2 - line_x1;
    let dy = line_y2 - line_y1;
    
    // Handle degenerate case where line segment is a point
    let line_length_sq = dx * dx + dy * dy;
    if line_length_sq < 1e-6 {
        let dist_x = point_x - line_x1;
        let dist_y = point_y - line_y1;
        return PointLineResult {
            distance: (dist_x * dist_x + dist_y * dist_y).sqrt(),
            closest_x: line_x1,
            closest_y: line_y1,
            on_segment: true,
            side: 0.0,
        };
    }
    
    // Calculate parametric position along line
    let t = ((point_x - line_x1) * dx + (point_y - line_y1) * dy) / line_length_sq;
    
    // Find closest point on infinite line
    let closest_x = line_x1 + t * dx;
    let closest_y = line_y1 + t * dy;
    
    // Check if closest point is on the line segment
    let on_segment = t >= 0.0 && t <= 1.0;
    
    // Calculate actual closest point (clamped to segment if needed)
    let (final_closest_x, final_closest_y) = if on_segment {
        (closest_x, closest_y)
    } else if t < 0.0 {
        (line_x1, line_y1)
    } else {
        (line_x2, line_y2)
    };
    
    // Calculate distance
    let dist_x = point_x - final_closest_x;
    let dist_y = point_y - final_closest_y;
    let distance = (dist_x * dist_x + dist_y * dist_y).sqrt();
    
    // Calculate which side of the line the point is on
    // Cross product gives us the signed area (positive = right side)
    let side = dx * (point_y - line_y1) - dy * (point_x - line_x1);
    
    PointLineResult {
        distance,
        closest_x,
        closest_y,
        on_segment,
        side,
    }
}
