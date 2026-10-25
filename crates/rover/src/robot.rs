//! A differential-drive robot body: velocity commands, acceleration limits
//! and collisions with the true walls.

use crate::geom::{Point, Pose};
use crate::grid::Grid;

/// What the controller asks for: forward speed (m/s) and turn rate (rad/s,
/// clockwise positive, i.e. positive turns right).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub v: f32,
    pub w: f32,
}

/// Body and drive limits; sized after a VolksBot (about 0.4 m across).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub radius: f32,
    pub max_v: f32,
    pub max_w: f32,
    /// m/s²
    pub accel: f32,
    /// rad/s²
    pub turn_accel: f32,
}

impl Default for Body {
    fn default() -> Self {
        Self {
            radius: 0.2,
            max_v: 0.5,
            max_w: 1.5,
            accel: 1.0,
            turn_accel: 4.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Robot {
    pub pose: Pose,
    pub body: Body,
    /// Current speed and turn rate.
    pub v: f32,
    pub w: f32,
    /// Whether the last step was blocked by a wall.
    pub bumped: bool,
}

impl Robot {
    pub fn new(pose: Pose, body: Body) -> Self {
        Self {
            pose,
            body,
            v: 0.0,
            w: 0.0,
            bumped: false,
        }
    }

    /// Advances `dt` seconds towards `cmd`. A move that would bring the body
    /// into a wall is refused: the robot stops where it is, still turning. A
    /// body already overlapping a wall (placed there) may only move in ways
    /// that leave it overlapping less, so it can drive out but not deeper in.
    pub fn step(&mut self, cmd: Command, dt: f32, walls: &Grid<bool>) {
        let b = self.body;
        let towards = |now: f32, want: f32, max: f32, rate: f32| {
            let want = want.clamp(-max, max);
            now + (want - now).clamp(-rate * dt, rate * dt)
        };
        self.v = towards(self.v, cmd.v, b.max_v, b.accel);
        self.w = towards(self.w, cmd.w, b.max_w, b.turn_accel);

        // Unicycle model, moving along the mid-step heading.
        let mid = self.pose.theta + self.w * dt * 0.5;
        let moved = self.pose.position() + Point::from_angle(mid) * (self.v * dt);
        self.pose.theta += self.w * dt;
        let after = overlap(walls, moved, b.radius);
        self.bumped = after > 0.0 && after >= overlap(walls, self.pose.position(), b.radius);
        if self.bumped {
            self.v = 0.0;
        } else {
            self.pose.x = moved.x;
            self.pose.y = moved.y;
        }
    }
}

/// How far a disc of `radius` at `p` reaches into the walls: `radius` less
/// the distance to the nearest wall cell, or 0 if it touches none.
pub fn overlap(walls: &Grid<bool>, p: Point, radius: f32) -> f32 {
    let lo = walls.cell_of(Point::new(p.x - radius, p.y - radius));
    let hi = walls.cell_of(Point::new(p.x + radius, p.y + radius));
    let half = walls.cell * 0.5;
    let mut deepest = 0.0f32;
    for y in lo.1..=hi.1 {
        for x in lo.0..=hi.0 {
            if walls.get((x, y)).copied().unwrap_or(true) {
                // Distance from the disc centre to the cell square.
                let c = walls.center((x, y));
                let dx = ((p.x - c.x).abs() - half).max(0.0);
                let dy = ((p.y - c.y).abs() - half).max(0.0);
                deepest = deepest.max(radius - (dx * dx + dy * dy).sqrt());
            }
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_straight_and_stops_at_a_wall() {
        // A wall at x = 2.0..2.1 m.
        let mut walls = Grid::new(60, 40, 0.05, false);
        for y in 0..40 {
            walls.set((40, y), true);
            walls.set((41, y), true);
        }
        let mut r = Robot::new(Pose::new(1.0, 1.0, 0.0), Body::default());
        let go = Command { v: 0.5, w: 0.0 };
        for _ in 0..60 {
            r.step(go, 1.0 / 60.0, &walls);
        }
        assert!(r.pose.x > 1.3 && r.pose.x < 1.6, "{:?}", r.pose);
        assert!((r.pose.y - 1.0).abs() < 1e-5);
        for _ in 0..600 {
            r.step(go, 1.0 / 60.0, &walls);
        }
        assert!(r.bumped);
        assert!(r.pose.x <= 2.0 - r.body.radius + 1e-4, "{:?}", r.pose);
    }

    #[test]
    fn a_body_placed_in_a_wall_can_back_out_but_not_push_in() {
        // A wall at x = 2.0..2.1 m, and a robot reaching 0.1 m into it.
        let mut walls = Grid::new(60, 40, 0.05, false);
        for y in 0..40 {
            walls.set((40, y), true);
            walls.set((41, y), true);
        }
        let mut r = Robot::new(Pose::new(1.9, 1.0, 0.0), Body::default());
        for _ in 0..30 {
            r.step(Command { v: 0.3, w: 0.0 }, 1.0 / 60.0, &walls);
        }
        assert!(r.bumped);
        assert!((r.pose.x - 1.9).abs() < 1e-6, "pushed in: {:?}", r.pose);
        let mut r = Robot::new(Pose::new(1.9, 1.0, std::f32::consts::PI), Body::default());
        for _ in 0..30 {
            r.step(Command { v: 0.3, w: 0.0 }, 1.0 / 60.0, &walls);
        }
        assert!(r.pose.x < 1.85, "stuck: {:?}", r.pose);
    }

    #[test]
    fn a_positive_turn_rate_turns_right_on_screen() {
        let walls = Grid::new(40, 40, 0.05, false);
        let mut r = Robot::new(Pose::new(1.0, 1.0, 0.0), Body::default());
        for _ in 0..60 {
            r.step(Command { v: 0.3, w: 1.0 }, 1.0 / 60.0, &walls);
        }
        // Heading east and turning right means drifting down the screen (+y).
        assert!(r.pose.theta > 0.0 && r.pose.y > 1.0, "{:?}", r.pose);
    }
}
