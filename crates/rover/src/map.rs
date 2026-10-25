//! What the robot knows: an occupancy grid filled in from laser scans.

use crate::geom::{Point, Pose};
use crate::grid::{Grid, trace};
use crate::laser::Laser;

/// Knowledge about one cell. The laser is error-free, so a cell never needs
/// more than this: once seen, it is what it was seen to be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Known {
    #[default]
    Unknown = 0,
    Free = 1,
    Wall = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    pub grid: Grid<Known>,
    /// Number of cells seen free, kept up to date for coverage readouts.
    pub free: usize,
}

impl Map {
    pub fn new(width: i32, height: i32, cell: f32) -> Self {
        Self {
            grid: Grid::new(width, height, cell, Known::Unknown),
            free: 0,
        }
    }

    /// Traces every beam of a scan taken at `pose`: the cells a beam passes
    /// through are free, and the cell where it stopped short of its full
    /// range is a wall. Returns how many cells changed.
    pub fn integrate(&mut self, laser: &Laser, pose: Pose, ranges: &[f32]) -> usize {
        let from = pose.position();
        let mut changed = 0;
        for (i, &r) in ranges.iter().enumerate() {
            let dir = Point::from_angle(pose.theta + laser.beam_angle(i));
            let grid = &mut self.grid;
            let free = &mut self.free;
            // Everything the beam crossed before `r` is free.
            trace(grid.cell, from, dir, r, |c, t| {
                if t >= r {
                    return true;
                }
                match grid.get(c) {
                    Some(Known::Unknown) => {
                        grid.set(c, Known::Free);
                        *free += 1;
                        changed += 1;
                        false
                    }
                    Some(_) => false,
                    None => true,
                }
            });
            // A beam stopping short of its range entered a wall cell at `r`.
            if r < laser.range {
                let c = grid.cell_of(from + dir * (r + grid.cell * 0.01));
                if grid.get(c) == Some(&Known::Unknown) {
                    grid.set(c, Known::Wall);
                    changed += 1;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scan_clears_the_way_and_marks_the_wall() {
        let mut walls = Grid::new(60, 20, 0.05, false);
        for y in 0..20 {
            walls.set((40, y), true);
        }
        let laser = Laser {
            fov: 0.0,
            samples: 1,
            range: 4.0,
        };
        let pose = Pose::new(0.52, 0.52, 0.0);
        let ranges = laser.scan(&walls, pose);
        let mut map = Map::new(60, 20, 0.05);
        map.integrate(&laser, pose, &ranges);
        let row = 10;
        assert_eq!(map.grid.get((10, row)), Some(&Known::Free));
        assert_eq!(map.grid.get((39, row)), Some(&Known::Free));
        assert_eq!(map.grid.get((40, row)), Some(&Known::Wall));
        assert_eq!(map.grid.get((41, row)), Some(&Known::Unknown));
        assert_eq!(map.free, 30);
    }
}
