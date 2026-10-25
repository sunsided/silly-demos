//! Floor plans as wall segments, and their rasterised occupancy grid.

use crate::geom::{Point, Pose, segment_distance};
use crate::grid::Grid;

/// A floor plan: wall segments in metres, and where the robot starts.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorPlan {
    pub width: f32,
    pub height: f32,
    pub walls: Vec<(Point, Point)>,
    pub start: Pose,
}

/// Margin around the building, so the outer walls sit inside the grid.
const MARGIN: f32 = 0.2;

impl FloorPlan {
    /// A lab floor of about 16 × 12 m: rooms off a central hall, a
    /// free-standing I-beam divider, a shelf along the south wall, a corridor
    /// leading south, and an annex to the east with a pillar in it.
    pub fn lab() -> Self {
        let outline = [
            (0.0, 0.0),
            (12.0, 0.0),
            (12.0, 7.0),
            (16.0, 7.0),
            (16.0, 10.0),
            (10.0, 10.0),
            (10.0, 12.0),
            (7.0, 12.0),
            (7.0, 10.0),
            (0.0, 10.0),
        ];
        let mut walls: Vec<_> = (0..outline.len())
            .map(|i| (outline[i], outline[(i + 1) % outline.len()]))
            .collect();
        walls.extend([
            // Room in the top-left corner, with a door in its south wall.
            ((4.0, 0.0), (4.0, 2.5)),
            ((0.0, 2.5), (1.6, 2.5)),
            ((2.9, 2.5), (4.0, 2.5)),
            // Room at the top, open towards the hall on its west side.
            ((8.0, 0.0), (8.0, 3.2)),
            ((5.4, 3.2), (8.0, 3.2)),
            // Two rooms on the east side, one above the other.
            ((9.5, 4.0), (12.0, 4.0)),
            ((8.0, 5.2), (8.0, 7.0)),
            ((8.0, 7.0), (9.2, 7.0)),
            ((10.6, 7.0), (12.0, 7.0)),
            // Free-standing I-beam divider.
            ((2.8, 4.6), (2.8, 7.6)),
            ((2.0, 4.6), (3.6, 4.6)),
            ((2.0, 7.6), (3.6, 7.6)),
            // Shelf along the south wall, and a cubby in the corner.
            ((1.2, 9.4), (5.2, 9.4)),
            ((1.2, 9.4), (1.2, 10.0)),
            ((5.2, 9.4), (5.2, 10.0)),
            ((0.0, 9.4), (0.6, 9.4)),
            ((0.6, 9.4), (0.6, 10.0)),
            // Pillar in the east annex.
            ((14.0, 8.2), (14.6, 8.2)),
            ((14.6, 8.2), (14.6, 8.8)),
            ((14.6, 8.8), (14.0, 8.8)),
            ((14.0, 8.8), (14.0, 8.2)),
        ]);
        let at = |(x, y): (f32, f32)| Point::new(x + MARGIN, y + MARGIN);
        Self {
            width: 16.0 + 2.0 * MARGIN,
            height: 12.0 + 2.0 * MARGIN,
            walls: walls.into_iter().map(|(a, b)| (at(a), at(b))).collect(),
            start: Pose::new(5.5 + MARGIN, 6.0 + MARGIN, 0.0),
        }
    }

    /// Occupancy grid of `cell` m cells: true where a wall `thickness` m
    /// thick covers the cell's centre.
    pub fn rasterize(&self, cell: f32, thickness: f32) -> Grid<bool> {
        let mut grid = Grid::new(
            (self.width / cell).ceil() as i32,
            (self.height / cell).ceil() as i32,
            cell,
            false,
        );
        let half = thickness * 0.5;
        for &(a, b) in &self.walls {
            let lo = grid.cell_of(Point::new(a.x.min(b.x) - half, a.y.min(b.y) - half));
            let hi = grid.cell_of(Point::new(a.x.max(b.x) + half, a.y.max(b.y) + half));
            for y in lo.1..=hi.1 {
                for x in lo.0..=hi.0 {
                    if segment_distance(grid.center((x, y)), a, b) <= half {
                        grid.set((x, y), true);
                    }
                }
            }
        }
        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walls_are_solid_and_the_start_is_clear() {
        let plan = FloorPlan::lab();
        let grid = plan.rasterize(0.05, 0.1);
        assert_eq!((grid.width, grid.height), (328, 248));
        // The north wall is a two-cell band all along.
        let row = grid.cell_of(Point::new(0.0, MARGIN)).1;
        let x = grid.cell_of(Point::new(6.0, 0.0)).0;
        assert!(grid.get((x, row)) == Some(&true) || grid.get((x, row - 1)) == Some(&true));
        assert_eq!(grid.get(grid.cell_of(plan.start.position())), Some(&false));
    }
}
