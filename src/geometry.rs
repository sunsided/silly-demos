use crate::PointLineResult;
use crate::vec2::Vec2;

/// Point-line distance and closest point calculation implementation.
/// line_x1, line_y1: start point of line segment
/// line_x2, line_y2: end point of line segment  
/// point_x, point_y: the point to test
pub fn point_line_test_impl(
    line_x1: f32,
    line_y1: f32,
    line_x2: f32,
    line_y2: f32,
    point_x: f32,
    point_y: f32,
) -> PointLineResult {
    let a = Vec2::new(line_x1, line_y1);
    let b = Vec2::new(line_x2, line_y2);
    let p = Vec2::new(point_x, point_y);
    let ab = b - a;
    let ap = p - a;
    let ab_len2 = ab.length_squared();
    if ab_len2 < 1e-6 {
        let dist = (p - a).length();
        return PointLineResult {
            distance: dist,
            closest_x: a.x,
            closest_y: a.y,
            on_segment: true,
            side: 0.0,
        };
    }
    let t = ap.dot(ab) / ab_len2;
    let closest = if t < 0.0 {
        a
    } else if t > 1.0 {
        b
    } else {
        a + ab * t
    };
    let distance = (p - closest).length();
    let on_segment = (0.0..=1.0).contains(&t);
    let side = ab.x * (p.y - a.y) - ab.y * (p.x - a.x);
    PointLineResult {
        distance,
        closest_x: closest.x,
        closest_y: closest.y,
        on_segment,
        side,
    }
}
