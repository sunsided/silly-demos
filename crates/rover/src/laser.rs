//! A planar laser range finder, error-free, traced through the true walls.

use crate::geom::{Point, Pose};
use crate::grid::{Grid, trace};

/// Beam layout of the scanner. The defaults follow a Hokuyo URG-04LX: 240°
/// field of view, up to 4 m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laser {
    /// Full field of view in radians, centred on the heading.
    pub fov: f32,
    pub samples: usize,
    /// Longest distance measured; beams that hit nothing report this.
    pub range: f32,
}

impl Default for Laser {
    fn default() -> Self {
        Self {
            fov: 240f32.to_radians(),
            samples: 241,
            range: 4.0,
        }
    }
}

impl Laser {
    /// Angle of beam `i` relative to the heading, clockwise positive (to the
    /// right), from −fov/2 to +fov/2.
    pub fn beam_angle(&self, i: usize) -> f32 {
        if self.samples < 2 {
            return 0.0;
        }
        -self.fov * 0.5 + self.fov * i as f32 / (self.samples - 1) as f32
    }

    /// Index of the beam closest to `angle` (relative to the heading,
    /// clockwise); 0 with fewer than two beams.
    pub fn beam_index(&self, angle: f32) -> usize {
        if self.samples < 2 {
            return 0;
        }
        let f = (angle + self.fov * 0.5) / self.fov * (self.samples - 1) as f32;
        (f.round().max(0.0) as usize).min(self.samples - 1)
    }

    /// Distances along every beam from `pose` to the nearest wall in `walls`.
    pub fn scan(&self, walls: &Grid<bool>, pose: Pose) -> Vec<f32> {
        let from = pose.position();
        (0..self.samples)
            .map(|i| {
                let dir = Point::from_angle(pose.theta + self.beam_angle(i));
                trace(walls.cell, from, dir, self.range, |c, _| {
                    walls.get(c).copied().unwrap_or(true)
                })
                .map_or(self.range, |t| t.min(self.range))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beam_stops_at_the_wall_in_front() {
        // A wall at x = 2.0..2.1 m.
        let mut walls = Grid::new(60, 20, 0.05, false);
        for y in 0..20 {
            walls.set((40, y), true);
            walls.set((41, y), true);
        }
        let laser = Laser {
            fov: 0.0,
            samples: 1,
            range: 4.0,
        };
        let ranges = laser.scan(&walls, Pose::new(0.5, 0.5, 0.0));
        assert!((ranges[0] - 1.5).abs() < 1e-4, "{ranges:?}");
        // Facing away, it sees nothing within range.
        let ranges = laser.scan(&walls, Pose::new(0.5, 0.5, std::f32::consts::PI));
        assert!(ranges[0] <= 4.0);
    }

    #[test]
    fn beam_angles_span_the_field_of_view() {
        let laser = Laser::default();
        assert!((laser.beam_angle(0) + laser.fov / 2.0).abs() < 1e-6);
        assert!((laser.beam_angle(laser.samples - 1) - laser.fov / 2.0).abs() < 1e-6);
        assert_eq!(laser.beam_index(0.0), laser.samples / 2);
        assert_eq!(laser.beam_index(10.0), laser.samples - 1);
        for samples in [0, 1] {
            assert_eq!(Laser { samples, ..laser }.beam_index(0.5), 0);
        }
    }
}
