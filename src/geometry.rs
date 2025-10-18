use crate::PointLineResult;

/// Point-line distance and closest point calculation implementation.
/// line_x1, line_y1: start point of line segment
/// line_x2, line_y2: end point of line segment  
/// point_x, point_y: the point to test
pub fn point_line_test_impl(
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
        closest_x: final_closest_x,
        closest_y: final_closest_y,
        on_segment,
        side,
    }
}
