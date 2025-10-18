use crate::CircleCollisionResult;

/// Circle–circle collision implementation
/// Inputs and outputs are all f32 to keep the boundary cheap.
pub fn circle_collision_impl(x1: f32, y1: f32, r1: f32, x2: f32, y2: f32, r2: f32) -> CircleCollisionResult {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let d2 = dx * dx + dy * dy;

    // Avoid sqrt(0) + division noise; treat coincident centers as zero distance.
    let distance = if d2 > 0.0 { d2.sqrt() } else { 0.0 };

    let sum = r1 + r2;
    let intersect = distance <= sum;
    let penetration = if intersect { sum - distance } else { 0.0 };

    CircleCollisionResult {
        intersect,
        distance,
        dx,
        dy,
        penetration,
    }
}
