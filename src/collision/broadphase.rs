//! Cheap rejection tests on cached bounding volumes, run before the narrowphase.

use super::probe::Probe;
use super::report::{Note, Tone};
use super::shape::Body;
use crate::vec2::Vec2;

/// Bit flags selecting the broadphase stages run before the narrowphase.
pub const BOUNDING_CIRCLE: u32 = 1;
/// Reject pairs whose axis-aligned bounding boxes are apart.
pub const BOUNDING_BOX: u32 = 2;
/// Scene-wide: A and B are only tested if they share a quadtree leaf.
pub const QUADTREE: u32 = 4;

/// How far a test got before it returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Rejected by the bounding-circle test.
    BoundingCircle = 0,
    /// Rejected by the bounding-box test.
    BoundingBox = 1,
    /// Passed every enabled stage; the narrowphase method gave the answer.
    Narrowphase = 2,
    /// Rejected because A and B share no quadtree leaf.
    Quadtree = 3,
}

/// Runs the enabled cheap rejection tests on the cached bounding volumes.
/// Returns the stage that rejected the pair, or `None` if it may collide.
pub fn reject<P: Probe>(a: &Body, b: &Body, mask: u32, p: &mut P) -> Option<Stage> {
    if mask & BOUNDING_CIRCLE != 0 {
        let ((ca, ra), (cb, rb)) = (a.circle, b.circle);
        p.dots(1);
        let apart = (cb - ca).length_squared() > (ra + rb) * (ra + rb);
        let tone = if apart { Tone::Cobalt } else { Tone::Soft };
        p.note(|| Note::Circle {
            c: ca,
            r: ra,
            tone,
            dashed: true,
        });
        p.note(|| Note::Circle {
            c: cb,
            r: rb,
            tone,
            dashed: true,
        });
        if apart {
            return Some(Stage::BoundingCircle);
        }
    }
    if mask & BOUNDING_BOX != 0 {
        let ((amin, amax), (bmin, bmax)) = (a.aabb, b.aabb);
        let apart = amax.x < bmin.x || bmax.x < amin.x || amax.y < bmin.y || bmax.y < amin.y;
        let tone = if apart { Tone::Cobalt } else { Tone::Soft };
        for (lo, hi) in [(amin, amax), (bmin, bmax)] {
            p.note(|| Note::Polygon {
                points: vec![lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)],
                tone,
                dashed: true,
                fill: false,
            });
        }
        if apart {
            return Some(Stage::BoundingBox);
        }
    }
    None
}
