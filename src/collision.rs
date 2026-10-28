//! Narrowphase collision tests between convex 2D shapes, with interchangeable
//! methods per shape pair, operation counts and construction notes for drawing.

pub mod broadphase;
pub mod methods;
pub mod probe;
pub mod quadtree;
pub mod registry;
pub mod report;
pub mod scene;
pub mod shape;

#[cfg(test)]
mod tests;

use broadphase::Stage;
use registry::{Resolved, Runner};
use report::Contact;
use shape::Body;

/// Runs the enabled broadphase stages, then the method if nothing rejected the pair.
pub fn test<P: Runner>(m: &Resolved, a: &Body, b: &Body, mask: u32, p: &mut P) -> (Contact, Stage) {
    match broadphase::reject(a, b, mask, p) {
        Some(stage) => (Contact::boolean(false), stage),
        None => (m.run(&a.shape, &b.shape, p), Stage::Narrowphase),
    }
}
