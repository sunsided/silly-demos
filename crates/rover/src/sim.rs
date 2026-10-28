//! The whole simulation: true world, robot, scans, map and controller.

use std::collections::VecDeque;

use crate::control::{Explorer, Phase, Reactive};
use crate::floorplan::FloorPlan;
use crate::frontier::{self, Cluster};
use crate::geom::Point;
use crate::grid::{Grid, neighbours4};
use crate::laser::Laser;
use crate::map::Map;
use crate::robot::{Body, Command, Robot};

/// Grid cell size, m.
pub const CELL: f32 = 0.05;
/// Wall thickness, m.
const WALL: f32 = 0.1;
/// Physics step, s.
const PHYSICS: f32 = 1.0 / 60.0;
/// Scans and control decisions per second; a Hokuyo URG scans at 10 Hz.
const SENSE_HZ: f32 = 10.0;
/// Smallest frontier cluster worth driving to, in cells.
const MIN_FRONTIER: usize = 4;

/// Which controller drives the robot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The 2013 lab's [`Reactive`] controller, stopping once no frontier is left.
    Reactive,
    /// The planning [`Explorer`], which maps everything and drives home.
    Explorer,
}

/// What the run is doing, for readouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Driving to frontiers, or wandering for the reactive controller.
    Exploring,
    /// The explorer, map complete, driving back to where it started.
    Returning,
    /// Nothing left to explore (and, for the explorer, back at the start).
    Done,
}

/// The world, the robot in it, what it has mapped so far, and the controller driving it.
#[derive(Clone, Debug)]
pub struct Sim {
    /// The floor plan the walls come from.
    pub plan: FloorPlan,
    /// The true walls, rasterized at [`CELL`]; what the laser and collisions see.
    pub walls: Grid<bool>,
    /// The scanner on the robot.
    pub laser: Laser,
    /// The robot's body and motion.
    pub robot: Robot,
    /// What the robot has mapped, on the same grid as `walls`.
    pub map: Map,
    /// The latest scan, one range per beam, m.
    pub ranges: Vec<f32>,
    /// Frontier clusters reachable in the map at the latest scan.
    pub frontiers: Vec<Cluster>,
    /// Which controller is driving.
    pub mode: Mode,
    /// The planning controller; kept even in reactive mode, and restarted on a mode switch.
    pub explorer: Explorer,
    /// Positions along the way, one each time the robot is more than 5 cm from the last.
    pub trail: Vec<Point>,
    /// Distance driven, m.
    pub distance: f32,
    /// Simulated time, s.
    pub time: f32,
    /// What the run is doing, for readouts.
    pub status: Status,
    /// The command decided at the latest scan, held until the next.
    cmd: Command,
    /// Seconds since the latest scan.
    since_sense: f32,
    /// Free cells of the true floor plan the robot could see, for coverage.
    reachable: usize,
}

impl Sim {
    /// A fresh run on `plan` with an empty map, after a first scan from the start.
    pub fn new(plan: FloorPlan, mode: Mode) -> Self {
        let walls = plan.rasterize(CELL, WALL);
        let reachable = flood_free(&walls, walls.cell_of(plan.start.position()));
        let body = Body::default();
        let mut sim = Self {
            map: Map::new(walls.width, walls.height, CELL),
            robot: Robot::new(plan.start, body),
            explorer: Explorer::new(plan.start, body),
            laser: Laser::default(),
            ranges: Vec::new(),
            frontiers: Vec::new(),
            trail: vec![plan.start.position()],
            distance: 0.0,
            time: 0.0,
            status: Status::Exploring,
            cmd: Command::default(),
            since_sense: 0.0,
            reachable,
            walls,
            plan,
            mode,
        };
        sim.sense();
        sim
    }

    /// Starts over with an empty map, keeping mode and laser.
    pub fn reset(&mut self) {
        let laser = self.laser;
        *self = Self::new(self.plan.clone(), self.mode);
        self.laser = laser;
        self.sense();
    }

    /// Switches controller; a new explorer starts, and comes home to where the robot is now.
    pub fn set_mode(&mut self, mode: Mode) {
        if mode != self.mode {
            self.mode = mode;
            let start = self.robot.pose;
            // The explorer comes back to where it was switched on.
            self.explorer = Explorer::new(start, self.robot.body);
            self.status = Status::Exploring;
        }
    }

    /// Advances the simulation by `dt` seconds; a finished run stands still,
    /// clock included.
    pub fn step(&mut self, dt: f32) {
        if self.status == Status::Done {
            return;
        }
        let mut left = dt;
        while left > 1e-6 {
            let h = left.min(PHYSICS);
            left -= h;
            self.since_sense += h;
            if self.since_sense >= 1.0 / SENSE_HZ {
                self.since_sense -= 1.0 / SENSE_HZ;
                self.sense();
                self.cmd = self.decide();
            }
            let before = self.robot.pose.position();
            self.robot.step(self.cmd, h, &self.walls);
            let now = self.robot.pose.position();
            self.distance += now.distance(before);
            self.time += h;
            if self.trail.last().is_none_or(|p| p.distance(now) > 0.05) {
                self.trail.push(now);
            }
        }
    }

    /// Scans, folds the scan into the map, and finds the frontiers reachable from the robot.
    fn sense(&mut self) {
        self.ranges = self.laser.scan(&self.walls, self.robot.pose);
        self.map
            .integrate(&self.laser, self.robot.pose, &self.ranges);
        let here = self.map.grid.cell_of(self.robot.pose.position());
        self.frontiers = frontier::find(&self.map.grid, here, MIN_FRONTIER);
    }

    /// What the current controller says to do, updating the status on the way.
    fn decide(&mut self) -> Command {
        match self.mode {
            Mode::Reactive => {
                // As in 2013: stop once no frontier is left.
                if self.frontiers.is_empty() {
                    self.status = Status::Done;
                    return Command::default();
                }
                self.status = Status::Exploring;
                Reactive.command(&self.laser, &self.ranges)
            }
            Mode::Explorer => {
                let cmd = self.explorer.command(
                    1.0 / SENSE_HZ,
                    self.robot.pose,
                    self.robot.body,
                    &self.map.grid,
                    &self.frontiers,
                    &self.laser,
                    &self.ranges,
                );
                self.status = match self.explorer.phase {
                    Phase::Explore => Status::Exploring,
                    Phase::Return => Status::Returning,
                    Phase::Done => Status::Done,
                };
                cmd
            }
        }
    }

    /// Share of the floor's free cells seen so far, 0..=1.
    pub fn coverage(&self) -> f32 {
        if self.reachable == 0 {
            return 1.0;
        }
        (self.map.free as f32 / self.reachable as f32).min(1.0)
    }
}

/// Free cells connected to `from` in the true wall grid.
fn flood_free(walls: &Grid<bool>, from: (i32, i32)) -> usize {
    let mut seen = walls.like(false);
    let mut queue = VecDeque::from([from]);
    seen.set(from, true);
    let mut n = 0;
    while let Some(c) = queue.pop_front() {
        n += 1;
        for m in neighbours4(c) {
            if walls.get(m) == Some(&false) && seen.get(m) == Some(&false) {
                seen.set(m, true);
                queue.push_back(m);
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `mode` for up to `seconds` of simulated time.
    fn run(mode: Mode, seconds: f32) -> Sim {
        let mut sim = Sim::new(FloorPlan::lab(), mode);
        while sim.time < seconds && sim.status != Status::Done {
            sim.step(1.0);
        }
        sim
    }

    #[test]
    fn the_explorer_maps_the_floor_and_comes_home() {
        let sim = run(Mode::Explorer, 1200.0);
        assert_eq!(
            sim.status,
            Status::Done,
            "after {:.0} s, {:.0} % seen",
            sim.time,
            sim.coverage() * 100.0
        );
        assert!(sim.coverage() > 0.999, "{:.2} %", sim.coverage() * 100.0);
        assert!(sim.frontiers.is_empty(), "frontiers left");
        let home = sim.plan.start.position();
        assert!(sim.robot.pose.position().distance(home) < 0.25);
        // A finished run stands still, clock included.
        let (t, pose) = (sim.time, sim.robot.pose);
        let mut sim = sim;
        sim.step(5.0);
        assert_eq!((sim.time, sim.robot.pose), (t, pose));
    }

    #[test]
    fn the_explorer_finishes_from_awkward_starts_with_a_coarse_laser() {
        // In a room corner, up against the pillar, and at the end of the corridor.
        for (x, y, theta) in [(1.7, 1.7, 1.0), (14.7, 9.2, 3.0), (8.9, 11.2, -1.5)] {
            let mut plan = FloorPlan::lab();
            plan.start = crate::geom::Pose::new(x, y, theta);
            let mut sim = Sim::new(plan, Mode::Explorer);
            sim.laser.samples = 61;
            sim.reset();
            while sim.time < 900.0 && sim.status != Status::Done {
                sim.step(1.0);
            }
            assert_eq!(sim.status, Status::Done, "from ({x}, {y})");
            assert!(
                sim.coverage() > 0.999,
                "from ({x}, {y}): {:.2} %",
                sim.coverage() * 100.0
            );
            assert!(sim.frontiers.is_empty(), "from ({x}, {y}): frontiers left");
            assert!(
                sim.robot
                    .pose
                    .position()
                    .distance(sim.plan.start.position())
                    < 0.25
            );
        }
    }

    #[test]
    fn the_reactive_controller_keeps_moving() {
        let sim = run(Mode::Reactive, 120.0);
        assert!(sim.distance > 10.0, "{:.1} m", sim.distance);
        assert!(sim.coverage() > 0.2, "{:.1} %", sim.coverage() * 100.0);
    }
}
