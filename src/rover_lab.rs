//! WASM bindings for the rover exploration lab (the `rover` crate).

use rover::floorplan::FloorPlan;
use rover::map::Known;
use rover::nav;
use rover::sim::{Mode, Sim, Status};
use wasm_bindgen::prelude::*;

/// A rover exploring the lab floor plan. The simulation lives in WASM;
/// JS steps it and reads flat snapshots for drawing.
#[wasm_bindgen]
pub struct RoverLab {
    /// The floor plan, robot, map and controller being simulated.
    sim: Sim,
}

/// The controller for `id`: "reactive", or the explorer for anything else.
fn mode_from(id: &str) -> Mode {
    match id {
        "reactive" => Mode::Reactive,
        _ => Mode::Explorer,
    }
}

#[wasm_bindgen]
impl RoverLab {
    /// `mode` is "explorer" or "reactive".
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str) -> Self {
        Self {
            sim: Sim::new(FloorPlan::lab(), mode_from(mode)),
        }
    }

    /// Starts over with an empty map.
    pub fn reset(&mut self) {
        self.sim.reset();
    }

    /// Switches controller mid-run; the explorer then comes back to where it
    /// was switched on.
    pub fn set_mode(&mut self, mode: &str) {
        self.sim.set_mode(mode_from(mode));
    }

    /// Beams per scan, at least 2. Takes effect with the next scan.
    pub fn set_laser_samples(&mut self, samples: usize) {
        self.sim.laser.samples = samples.max(2);
    }

    /// Advances the simulation by `dt` seconds of simulated time.
    pub fn step(&mut self, dt: f32) {
        self.sim.step(dt.clamp(0.0, 10.0));
    }

    /// Floor plan size in meters, `[width, height]`.
    pub fn size(&self) -> Vec<f32> {
        vec![self.sim.plan.width, self.sim.plan.height]
    }

    /// Wall segments `[ax, ay, bx, by, ...]` in meters.
    pub fn walls(&self) -> Vec<f32> {
        self.sim
            .plan
            .walls
            .iter()
            .flat_map(|(a, b)| [a.x, a.y, b.x, b.y])
            .collect()
    }

    /// Where the explorer returns to, `[x, y, heading]`.
    pub fn home(&self) -> Vec<f32> {
        let p = self.sim.explorer.home();
        vec![p.x, p.y, p.theta]
    }

    /// Map grid `[columns, rows, cell size in m]`.
    pub fn grid(&self) -> Vec<f32> {
        let g = &self.sim.map.grid;
        vec![g.width as f32, g.height as f32, g.cell]
    }

    /// What the robot knows, one byte per cell, row by row: 0 unknown,
    /// 1 free, 2 wall, 3 frontier (free, next to unknown, reachable).
    pub fn knowledge(&self) -> Vec<u8> {
        let g = &self.sim.map.grid;
        let mut out: Vec<u8> = g.data.iter().map(|&k| k as u8).collect();
        for c in self.sim.frontiers.iter().flat_map(|f| f.cells.iter()) {
            out[g.index(*c)] = 3;
        }
        out
    }

    /// Clearance per cell in meters, row by row: the distance to the nearest
    /// wall the robot knows of, capped at 2 m, as the explorer plans with
    /// it; −1 for cells not known to be free. Computed on each call.
    pub fn clearance(&self) -> Vec<f32> {
        let map = &self.sim.map.grid;
        let clear = nav::clearance(map, 2.0);
        map.data
            .iter()
            .zip(&clear.data)
            .map(|(&k, &d)| if k == Known::Free { d } else { -1.0 })
            .collect()
    }

    /// The explorer's clearance thresholds `[hard, soft]` in meters: its
    /// center never comes closer to a wall than `hard`, and paths cost extra
    /// within `soft`.
    pub fn room(&self) -> Vec<f32> {
        let r = self.sim.explorer.room();
        vec![r.hard, r.soft]
    }

    /// Robot pose `[x, y, heading]`; heading clockwise from +x.
    pub fn pose(&self) -> Vec<f32> {
        let p = self.sim.robot.pose;
        vec![p.x, p.y, p.theta]
    }

    /// The robot's body radius, in meters.
    pub fn radius(&self) -> f32 {
        self.sim.robot.body.radius
    }

    /// Last scan's ranges, beam 0 at −fov/2 (left), clockwise.
    pub fn ranges(&self) -> Vec<f32> {
        self.sim.ranges.clone()
    }

    /// Laser `[field of view in rad, range in m]`.
    pub fn laser(&self) -> Vec<f32> {
        vec![self.sim.laser.fov, self.sim.laser.range]
    }

    /// The explorer's planned path `[x0, y0, ...]`, or empty.
    pub fn path(&self) -> Vec<f32> {
        match (&self.sim.mode, &self.sim.explorer.path) {
            (Mode::Explorer, Some(p)) => p.points.iter().flat_map(|q| [q.x, q.y]).collect(),
            _ => Vec::new(),
        }
    }

    /// The frontier cell the explorer is heading for `[x, y]`, or empty.
    pub fn target(&self) -> Vec<f32> {
        match (&self.sim.mode, self.sim.explorer.target) {
            (Mode::Explorer, Some(t)) => vec![t.x, t.y],
            _ => Vec::new(),
        }
    }

    /// Frontier cells the explorer gave up on `[x0, y0, ...]`.
    pub fn given_up(&self) -> Vec<f32> {
        self.sim
            .explorer
            .given_up()
            .iter()
            .flat_map(|p| [p.x, p.y])
            .collect()
    }

    /// Driven trail `[x0, y0, ...]`.
    pub fn trail(&self) -> Vec<f32> {
        self.sim.trail.iter().flat_map(|p| [p.x, p.y]).collect()
    }

    /// Stats `[coverage 0..1, distance m, time s, frontier clusters]`.
    pub fn stats(&self) -> Vec<f32> {
        vec![
            self.sim.coverage(),
            self.sim.distance,
            self.sim.time,
            self.sim.frontiers.len() as f32,
        ]
    }

    /// "exploring", "returning" or "done".
    pub fn status(&self) -> String {
        match self.sim.status {
            Status::Exploring => "exploring",
            Status::Returning => "returning",
            Status::Done => "done",
        }
        .to_owned()
    }

    /// Whether the robot ran into a wall in the last physics step.
    pub fn bumped(&self) -> bool {
        self.sim.robot.bumped
    }
}

impl Default for RoverLab {
    fn default() -> Self {
        Self::new("explorer")
    }
}

// Keeps the byte encoding in `knowledge` in step with the map's.
const _: () =
    assert!(Known::Unknown as u8 == 0 && Known::Free as u8 == 1 && Known::Wall as u8 == 2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearance_covers_the_grid_and_marks_cells_not_known_free() {
        let mut lab = RoverLab::new("explorer");
        lab.step(2.0);
        let clear = lab.clearance();
        let g = lab.grid();
        assert_eq!(clear.len(), (g[0] * g[1]) as usize);
        let k = lab.knowledge();
        for (d, k) in clear.iter().zip(&k) {
            let free = *k == Known::Free as u8 || *k == 3;
            assert_eq!(*d >= 0.0, free);
            assert!(*d <= 2.0);
        }
        let room = lab.room();
        assert!(room[0] < room[1]);
    }
}
