//! WASM bindings for the rover exploration lab (the `rover` crate).

use rover::floorplan::FloorPlan;
use rover::map::Known;
use rover::sim::{Mode, Sim, Status};
use wasm_bindgen::prelude::*;

/// A rover exploring the lab floor plan. The simulation lives in WASM;
/// JS steps it and reads flat snapshots for drawing.
#[wasm_bindgen]
pub struct RoverLab {
    sim: Sim,
}

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

    /// Floor plan size in metres, `[width, height]`.
    pub fn size(&self) -> Vec<f32> {
        vec![self.sim.plan.width, self.sim.plan.height]
    }

    /// Wall segments `[ax, ay, bx, by, ...]` in metres.
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

    /// Robot pose `[x, y, heading]`; heading clockwise from +x.
    pub fn pose(&self) -> Vec<f32> {
        let p = self.sim.robot.pose;
        vec![p.x, p.y, p.theta]
    }

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
