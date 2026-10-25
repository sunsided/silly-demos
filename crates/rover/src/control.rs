//! The two controllers: the 2013 reactive one, and a planning explorer.

use crate::frontier::Cluster;
use crate::geom::{Point, Pose, wrap};
use crate::grid::{Cell, Grid};
use crate::laser::Laser;
use crate::map::Known;
use crate::nav::{self, Path, Room};
use crate::robot::{Body, Command};

/// The 2013 lab's controller, ported term by term from `simple.c`.
///
/// It averages laser sectors and mixes them into a forward speed and a turn
/// rate with proportional terms, tuned by hand until the robot made it
/// round the room. Two quirks of the original are kept on purpose, since
/// they shape how it drives:
///
/// - Its `LERP` macro computes `t · max + min` rather than
///   `min + t · (max − min)`, so a term like `LERP_SATURATE(…, 0.5, 0)` is a
///   constant 0.5, and the speed term isn't clamped and goes negative (the
///   robot backs off) when the way ahead is closer than 0.7 m.
/// - Player measures laser angles counter-clockwise, so the sectors the
///   code calls "right" are on the robot's left. It then sends the negated
///   turn rate, which makes the whole thing consistent again.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reactive;

/// `LASER_RANGE_MIN` in the original: not the sensor's limit but the
/// distance the terms treat as touching.
const NEAR: f32 = 0.35;

/// The original `LERP`: `(x − a) / (b − a) · max + min`.
fn lerp(a: f32, b: f32, x: f32, min: f32, max: f32) -> f32 {
    (x - a) / (b - a) * max + min
}

/// The original `LERP_SATURATE`: `lerp` with `x` clamped to `a..=b`.
fn lerp_saturate(a: f32, b: f32, x: f32, min: f32, max: f32) -> f32 {
    lerp(a, b, x.clamp(a, b), min, max)
}

impl Reactive {
    /// Stands still without a scan to go on.
    pub fn command(&self, laser: &Laser, ranges: &[f32]) -> Command {
        if ranges.len() != laser.samples || ranges.is_empty() {
            return Command::default();
        }
        // Angles as the original wrote them: degrees, counter-clockwise, so
        // the beam is at the mirrored angle in this crate's clockwise frame.
        let at = |deg: f32| ranges[laser.beam_index(-deg.to_radians())];
        let avg = |a: f32, b: f32| {
            let (i, j) = (
                laser.beam_index(-a.to_radians()),
                laser.beam_index(-b.to_radians()),
            );
            let (i, j) = (i.min(j), i.max(j));
            ranges[i..=j].iter().sum::<f32>() / (j - i + 1) as f32
        };
        let max = laser.range;
        let half_fov = laser.fov.to_degrees() * 0.5;

        let front_exact = at(0.0);
        let front = avg(-22.5, 22.5);
        let front_wide = avg(-45.0, 45.0);
        let v = lerp(NEAR * 2.0, max * 3.0 / 4.0, front_wide, 0.0, 0.4);

        let right_front_exact = at(50.0);
        let right_front = avg(22.5, 67.5);
        let right = avg(67.5, 112.5);
        let left_front_exact = at(-50.0);
        let left_front = avg(-22.5, -67.5);
        // Computed but unused in the original, as are the sector sums.
        let _ = (
            avg(112.5, half_fov),
            avg(-67.5, -112.5),
            avg(-112.5, -half_fov),
        );

        let mut w = 0.0;
        // "If there is danger front right, drift left."
        w -= lerp(NEAR, max, right_front, 0.0, 1.0);
        w -= lerp_saturate(NEAR, 1.0, right_front_exact, 0.0, 1.0);
        // "If there is danger front left, drift right."
        w += lerp(NEAR, max, left_front, 0.0, 1.0);
        w += lerp_saturate(NEAR, 1.0, left_front_exact, 0.0, 1.0);
        // "If the right is free, turn right."
        w += lerp(NEAR, 2.0, right, 0.0, 0.4);
        // "Add a tendency to turn left, weighted by the urge to turn right."
        w -= lerp_saturate(NEAR, 1.0, front, 0.5, 0.0) * lerp(NEAR, 2.0, right, 0.0, 0.4);
        // "Avoid an obstacle dead ahead by turning left."
        w -= lerp_saturate(NEAR, 1.0, front_exact, 0.5, 0.0);
        // "Don't turn left when there is no obstacle."
        w += lerp_saturate(1.0, 1.0 + NEAR, front_exact, 0.0, 0.5);

        // The original sent −w to Player, whose turn rate is
        // counter-clockwise; that is +w clockwise here.
        Command { v, w }
    }
}

/// What the explorer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Heading for the nearest frontier.
    Explore,
    /// No frontier left: driving back to the start.
    Return,
    /// Back home, map complete.
    Done,
}

/// Plans to the nearest lookout (see [`nav::lookouts`]): a spot with a clear
/// view into the unknown behind a frontier. Arriving within reach of a
/// frontier isn't enough; next to a wall face the laser only grazes it.
/// Follows the plan, looks round on arrival, and returns home once nothing
/// is left to explore.
#[derive(Clone, Debug, PartialEq)]
pub struct Explorer {
    pub phase: Phase,
    pub path: Option<Path>,
    /// The unknown cell behind a frontier that the current path leads to a
    /// view of.
    pub target: Option<Point>,
    home: Pose,
    room: Room,
    since_plan: f32,
    /// How far (rad) it has turned on the spot at the end of its path to
    /// look at the target frontier.
    looked: f32,
    /// Unknown cells it went to look at but couldn't uncover; unknown cells
    /// near them are skipped from then on.
    given_up: Vec<Point>,
}

/// Longest line of sight from a lookout to the unknown cell it watches, m;
/// well inside the laser's range.
const LOOKOUT_RANGE: f32 = 2.0;
/// Seconds between replans while following a path.
const REPLAN: f32 = 1.0;
/// Pure-pursuit lookahead, m.
const LOOKAHEAD: f32 = 0.45;
/// Heading error (rad) above which the robot turns on the spot.
const TURN_IN_PLACE: f32 = 0.8;
/// Unknown cells this close to a given-up one are skipped too, m.
const GIVEN_UP_RADIUS: f32 = 0.2;

impl Explorer {
    pub fn new(home: Pose, body: Body) -> Self {
        Self {
            phase: Phase::Explore,
            path: None,
            target: None,
            home,
            room: Room {
                hard: body.radius + 0.03,
                soft: body.radius + 0.35,
            },
            since_plan: f32::INFINITY,
            looked: 0.0,
            given_up: Vec::new(),
        }
    }

    /// One control tick of `dt` seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn command(
        &mut self,
        dt: f32,
        pose: Pose,
        body: Body,
        map: &Grid<Known>,
        frontiers: &[Cluster],
        laser: &Laser,
        ranges: &[f32],
    ) -> Command {
        self.since_plan += dt;
        let here = pose.position();
        let at_end = self
            .path
            .as_ref()
            .and_then(|p| p.points.last())
            .is_none_or(|&end| end.distance(here) < 0.1);

        // At the end of the path with the frontier still there: turn on the
        // spot far enough to have looked all round (it may have been in the
        // blind spot behind), then give up on it.
        if self.phase == Phase::Explore
            && at_end
            && let Some(target) = self.target
        {
            let all_round = std::f32::consts::TAU - laser.fov + 0.3;
            if self.looked < all_round {
                let w = body.max_w * 0.7;
                self.looked += w * dt;
                return Command { v: 0.0, w };
            }
            // Only targets that are still unknown after the look count as given up.
            if map.get(map.cell_of(target)) == Some(&Known::Unknown) {
                self.given_up.push(target);
            }
            self.target = None;
            self.looked = 0.0;
            self.path = None;
        } else {
            self.looked = 0.0;
        }

        if self.phase != Phase::Done && (self.since_plan >= REPLAN || self.path.is_none()) {
            self.replan(map, frontiers, here);
        }
        if self.phase == Phase::Return && here.distance(self.home.position()) < 0.2 {
            self.phase = Phase::Done;
            self.path = None;
        }
        match &self.path {
            Some(path) if self.phase != Phase::Done => pursue(path, pose, body, laser, ranges),
            _ => Command::default(),
        }
    }

    /// Where it returns to once nothing is left to explore.
    pub fn home(&self) -> Pose {
        self.home
    }

    pub fn given_up(&self) -> &[Point] {
        &self.given_up
    }

    fn replan(&mut self, map: &Grid<Known>, frontiers: &[Cluster], here: Point) {
        self.since_plan = 0.0;
        let clear = nav::clearance(map, 2.0);
        if self.phase == Phase::Explore {
            // Go where a scan would reach into the unknown behind a frontier.
            let targets: Vec<Cell> = frontiers
                .iter()
                .flat_map(|f| f.unknown.iter().copied())
                .filter(|&c| {
                    let p = map.center(c);
                    self.given_up
                        .iter()
                        .all(|g| g.distance(p) > GIVEN_UP_RADIUS)
                })
                .collect();
            let lookouts = nav::lookouts(map, &clear, &targets, LOOKOUT_RANGE, 0.15);
            match nav::plan(map, &clear, self.room, here, |c| {
                lookouts.get(c).is_some_and(|t| t.is_some())
            }) {
                Some(path) => {
                    let target = lookouts
                        .get(path.goal)
                        .copied()
                        .flatten()
                        .map(|i| map.center(targets[i as usize]));
                    // A new frontier gets a fresh look round.
                    if target
                        .zip(self.target)
                        .is_none_or(|(a, b)| a.distance(b) > GIVEN_UP_RADIUS)
                    {
                        self.looked = 0.0;
                    }
                    self.target = target;
                    self.path = Some(path);
                    return;
                }
                None => {
                    self.phase = Phase::Return;
                    self.target = None;
                }
            }
        }
        if self.phase == Phase::Return {
            let home: Cell = map.cell_of(self.home.position());
            let goals = nav::near(map, [home], 0.1);
            self.path = nav::plan(map, &clear, self.room, here, |c| {
                goals.get(c) == Some(&true)
            });
            if self.path.is_none() {
                self.phase = Phase::Done;
            }
        }
    }
}

/// Pure pursuit: steer towards the path point a lookahead distance ahead,
/// turning on the spot when it is far off the heading, and never driving
/// into something the laser sees right in front.
fn pursue(path: &Path, pose: Pose, body: Body, laser: &Laser, ranges: &[f32]) -> Command {
    let here = pose.position();
    let aim = lookahead(&path.points, here);
    let end = *path.points.last().unwrap_or(&here);
    let alpha = wrap((aim - here).angle() - pose.theta);
    if alpha.abs() > TURN_IN_PLACE {
        return Command {
            v: 0.0,
            w: alpha.signum() * body.max_w * 0.7,
        };
    }
    let mut v = body.max_v * (1.0 - alpha.abs() / TURN_IN_PLACE).max(0.25);
    v *= (end.distance(here) / 0.6).clamp(0.3, 1.0);
    let ahead = (0..ranges.len())
        .filter(|&i| laser.beam_angle(i).abs() < 0.45)
        .map(|i| ranges[i])
        .fold(f32::INFINITY, f32::min);
    if ahead < body.radius + 0.1 {
        v = 0.0;
    }
    Command {
        v,
        w: (2.0 * alpha).clamp(-body.max_w, body.max_w),
    }
}

/// The first point at least `LOOKAHEAD` away along the polyline, after the
/// segment closest to `here`.
fn lookahead(points: &[Point], here: Point) -> Point {
    if points.len() < 2 {
        return points.first().copied().unwrap_or(here);
    }
    // Closest segment.
    let mut best = (f32::INFINITY, 0);
    for i in 0..points.len() - 1 {
        let d = crate::geom::segment_distance(here, points[i], points[i + 1]);
        if d < best.0 {
            best = (d, i);
        }
    }
    // Walk on from there until a point on the path is LOOKAHEAD away.
    for i in best.1..points.len() - 1 {
        let (a, b) = (points[i], points[i + 1]);
        if b.distance(here) >= LOOKAHEAD {
            // Where along a→b the circle around `here` is crossed.
            let d = b - a;
            let f = a - here;
            let (qa, qb, qc) = (d.dot(d), 2.0 * f.dot(d), f.dot(f) - LOOKAHEAD * LOOKAHEAD);
            let disc = qb * qb - 4.0 * qa * qc;
            if qa > 0.0 && disc >= 0.0 {
                let t = ((-qb + disc.sqrt()) / (2.0 * qa)).clamp(0.0, 1.0);
                return a + d * t;
            }
            return b;
        }
    }
    points[points.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_lerp_quirk_is_kept() {
        // Not a proper interpolation from 0.5 to 0: a constant 0.5.
        assert_eq!(lerp_saturate(0.35, 1.0, 0.2, 0.5, 0.0), 0.5);
        assert_eq!(lerp_saturate(0.35, 1.0, 3.0, 0.5, 0.0), 0.5);
        // Unclamped speed: negative when the way ahead is close.
        assert!(lerp(0.7, 3.0, 0.5, 0.0, 0.4) < 0.0);
    }

    #[test]
    fn reactive_turns_away_from_a_wall_on_the_left() {
        let laser = Laser::default();
        // Open everywhere, but a wall close on the left (negative angles).
        let ranges: Vec<f32> = (0..laser.samples)
            .map(|i| {
                let a = laser.beam_angle(i);
                if a < -0.3 && a > -1.5 { 0.4 } else { 4.0 }
            })
            .collect();
        let cmd = Reactive.command(&laser, &ranges);
        assert!(cmd.v > 0.0);
        assert!(cmd.w > 0.0, "turns right, away from the wall: {cmd:?}");
    }

    #[test]
    fn lookahead_picks_the_point_ahead_on_the_path() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
        ];
        let p = lookahead(&pts, Point::new(0.2, 0.0));
        assert!((p.x - 0.65).abs() < 1e-4 && p.y.abs() < 1e-4, "{p:?}");
        let p = lookahead(&pts, Point::new(1.0, 0.7));
        assert_eq!(p, Point::new(1.0, 1.0));
    }
}
