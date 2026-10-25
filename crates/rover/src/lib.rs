//! A simulated rover that maps and explores a floor plan.
//!
//! A differential-drive robot with a laser scanner drives through a floor
//! plan it doesn't know. Every scan is traced into an occupancy map that
//! starts out unknown (the fog of war); the edges between seen free space
//! and the unknown are the frontiers left to explore.
//!
//! Two controllers steer it:
//!
//! - [`control::Reactive`] is a port of a 2013 university lab solution:
//!   proportional terms on averaged laser sectors that keep it moving and
//!   off the walls. The original found the nearest frontier as well, but
//!   only drew it; steering used nothing but the current scan, and the map
//!   only told it when to stop.
//! - [`control::Explorer`] plans: it finds the nearest spot, by travel
//!   distance, with a clear view into the unknown behind a frontier, plans a
//!   path there through known free space kept clear of the walls by the
//!   robot's radius, follows it with pure pursuit, and drives back to where
//!   it started once no frontier is left.
//!
//! Coordinates are in metres in a screen-like frame: x to the right, y
//! downwards, and angles clockwise from +x, so a positive turn rate turns
//! right.

pub mod control;
pub mod floorplan;
pub mod frontier;
pub mod geom;
pub mod grid;
pub mod laser;
pub mod map;
pub mod nav;
pub mod robot;
pub mod sim;

pub use sim::Sim;
