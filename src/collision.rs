use crate::CircleCollisionResult;
use crate::vec2::Vec2;

/// Circle–circle collision implementation
/// Inputs and outputs are all f32 to keep the boundary cheap.
pub fn circle_collision_impl(
    x1: f32,
    y1: f32,
    r1: f32,
    x2: f32,
    y2: f32,
    r2: f32,
) -> CircleCollisionResult {
    let c1 = Vec2::new(x1, y1);
    let c2 = Vec2::new(x2, y2);
    let d = c2 - c1;
    let d2 = d.length_squared();
    let distance = if d2 > 0.0 { d2.sqrt() } else { 0.0 };
    let sum = r1 + r2;
    let intersect = distance <= sum;
    let penetration = if intersect { sum - distance } else { 0.0 };
    CircleCollisionResult {
        intersect,
        distance,
        dx: d.x,
        dy: d.y,
        penetration,
    }
}
