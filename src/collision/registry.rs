//! Which methods apply to which shape pair. Each method is written for one
//! kind order; the registry also offers it for the mirrored pair by swapping
//! the inputs and flipping the result.

use super::methods::{boxes, capsule, circle, ellipse, gjk, polygon, sat};
use super::probe::{Probe, Recorder, Silent};
use super::report::Contact;
use super::shape::{Shape, ShapeKind};
use ShapeKind::*;

type Run<P> = fn(&Shape, &Shape, &mut P) -> Contact;

pub struct Method {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    /// Whether the method takes shapes of kinds `(a, b)` in this order.
    pub applies: fn(ShapeKind, ShapeKind) -> bool,
    record: Run<Recorder>,
    silent: Run<Silent>,
}

/// Picks the monomorphized entry point for a probe type.
pub trait Runner: Probe + Sized {
    fn entry(m: &Method) -> Run<Self>;
}

impl Runner for Recorder {
    fn entry(m: &Method) -> Run<Self> {
        m.record
    }
}

impl Runner for Silent {
    fn entry(m: &Method) -> Run<Self> {
        m.silent
    }
}

macro_rules! method {
    ($id:literal, $name:literal, $summary:literal, $applies:expr, $($f:ident)::+) => {
        Method {
            id: $id,
            name: $name,
            summary: $summary,
            applies: $applies,
            record: $($f)::+::<Recorder>,
            silent: $($f)::+::<Silent>,
        }
    };
}

/// Pair-specific methods first, generic ones last.
pub static METHODS: &[Method] = &[
    method!(
        "circle-squared",
        "Squared distance",
        "Compare |cB − cA|² with (rA + rB)². No square root, so it only answers yes or no.",
        |a, b| a == Circle && b == Circle,
        circle::squared
    ),
    method!(
        "circle-distance",
        "Center distance",
        "One square root buys the gap or penetration depth and the contact normal.",
        |a, b| a == Circle && b == Circle,
        circle::distance
    ),
    method!(
        "aabb-minmax",
        "Interval overlap",
        "Boxes overlap iff their x and y intervals both overlap: four comparisons, no products.",
        |a, b| a == Aabb && b == Aabb,
        boxes::aabb_minmax
    ),
    method!(
        "circle-aabb-clamp",
        "Clamp to box",
        "Clamping the center to the box's extents gives the closest point on the box.",
        |a, b| a == Circle && b == Aabb,
        boxes::circle_aabb
    ),
    method!(
        "circle-obb-clamp",
        "Clamp in box frame",
        "Rotate the center into the box's local frame, then clamp as for an axis-aligned box.",
        |a, b| a == Circle && b == Obb,
        boxes::circle_obb
    ),
    method!(
        "circle-polygon-voronoi",
        "Nearest edge region",
        "The closest point lies on the nearest edge, inside its span or at a vertex.",
        |a, b| a == Circle && matches!(b, Triangle | Polygon),
        polygon::circle_voronoi
    ),
    method!(
        "circle-ellipse-closest",
        "Closest point on ellipse",
        "Bisection for the exact closest point on the ellipse to the circle's center (Eberly).",
        |a, b| a == Circle && b == Ellipse,
        ellipse::circle_ellipse
    ),
    method!(
        "circle-capsule",
        "Point to core segment",
        "A capsule is a segment grown by a radius: compare the center's distance to the segment.",
        |a, b| a == Circle && b == Capsule,
        capsule::circle_capsule
    ),
    method!(
        "capsule-capsule",
        "Segment to segment",
        "Closest points between the two core segments, minus both radii.",
        |a, b| a == Capsule && b == Capsule,
        capsule::capsule_capsule
    ),
    method!(
        "capsule-polygon",
        "Core segment to polygon",
        "Distance from the core segment to the polygon's edges, minus the radius.",
        |a, b| a == Capsule && b.is_polygonal(),
        capsule::capsule_polygon
    ),
    method!(
        "sat",
        "Separating axis test",
        "Project both shapes onto every edge normal; any gap proves separation. Least overlap gives the push-out.",
        |a, b| { (a.is_polygonal() && b.is_polygonal()) || (a == Circle && b.is_polygonal()) },
        sat::sat
    ),
    method!(
        "edge-crossing",
        "Edge crossing",
        "Brute force: test every edge pair for a crossing, then check containment.",
        |a, b| a.is_polygonal() && b.is_polygonal(),
        polygon::edge_crossing
    ),
    method!(
        "gjk",
        "GJK (yes/no)",
        "Walks a simplex in the Minkowski difference A − B towards the origin; stops at the first separating direction.",
        |_, _| true,
        gjk::gjk_boolean
    ),
    method!(
        "gjk-epa",
        "GJK + EPA",
        "GJK until converged for the distance; EPA expands the simplex for the penetration depth.",
        |_, _| true,
        gjk::gjk_epa
    ),
];

/// A method resolved for a pair, possibly with A and B swapped.
#[derive(Clone, Copy)]
pub struct Resolved {
    pub method: &'static Method,
    swapped: bool,
}

impl Resolved {
    pub fn run<P: Runner>(&self, a: &Shape, b: &Shape, p: &mut P) -> Contact {
        let f = P::entry(self.method);
        if self.swapped {
            f(b, a, p).flipped()
        } else {
            f(a, b, p)
        }
    }
}

pub fn resolve(method: &'static Method, a: ShapeKind, b: ShapeKind) -> Option<Resolved> {
    if (method.applies)(a, b) {
        Some(Resolved {
            method,
            swapped: false,
        })
    } else if (method.applies)(b, a) {
        Some(Resolved {
            method,
            swapped: true,
        })
    } else {
        None
    }
}

pub fn methods_for(a: ShapeKind, b: ShapeKind) -> impl Iterator<Item = Resolved> {
    METHODS.iter().filter_map(move |m| resolve(m, a, b))
}

pub fn find(id: &str, a: ShapeKind, b: ShapeKind) -> Option<Resolved> {
    METHODS
        .iter()
        .find(|m| m.id == id)
        .and_then(|m| resolve(m, a, b))
}
