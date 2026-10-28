//! Narrowphase algorithms. Each method takes two shapes in a fixed kind order
//! (see the registry) and reports through a [`Probe`].

pub mod boxes;
pub mod capsule;
pub mod circle;
pub mod ellipse;
pub mod gjk;
pub mod polygon;
pub mod sat;

/// Imports every method module needs.
mod prelude {
    pub use super::super::probe::Probe;
    pub use super::super::report::{Contact, Note, Tone, fmt};
    pub use super::super::shape::{Poly, Shape};
    pub use crate::vec2::Vec2;
}

use prelude::*;

/// Closest point to `c` on the segment `a`–`b`, with its parameter along the segment.
pub(crate) fn closest_on_segment<P: Probe>(c: Vec2, a: Vec2, b: Vec2, p: &mut P) -> (Vec2, f32) {
    let e = b - a;
    let ee = e.length_squared();
    p.dots(2);
    if ee <= f32::EPSILON {
        return (a, 0.0);
    }
    p.div();
    let t = ((c - a).dot(e) / ee).clamp(0.0, 1.0);
    (a + e * t, t)
}

/// Closest points between segments `p1`–`q1` and `p2`–`q2`
/// (Ericson, Real-Time Collision Detection, 5.1.9).
pub(crate) fn closest_between_segments<P: Probe>(
    p1: Vec2,
    q1: Vec2,
    p2: Vec2,
    q2: Vec2,
    p: &mut P,
) -> (Vec2, Vec2) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    p.dots(3);
    let eps = f32::EPSILON;
    let (s, t) = if a <= eps && e <= eps {
        (0.0, 0.0)
    } else if a <= eps {
        p.div();
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        p.dots(1);
        if e <= eps {
            p.div();
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            p.dots(1);
            let denom = a * e - b * b;
            let s = if denom > eps {
                p.div();
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p.div();
            let t = (b * s + f) / e;
            if t < 0.0 {
                p.div();
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                p.div();
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    (p1 + d1 * s, p2 + d2 * t)
}

/// Whether `c` lies inside (or on) a counter-clockwise convex polygon.
pub(crate) fn inside_convex<P: Probe>(poly: &Poly, c: Vec2, p: &mut P) -> bool {
    (0..poly.len()).all(|i| {
        let (a, b) = poly.edge(i);
        p.dots(1);
        (b - a).cross(c - a) >= 0.0
    })
}

/// Whether segments `a`–`b` and `c`–`d` share a point.
pub(crate) fn segments_cross<P: Probe>(a: Vec2, b: Vec2, c: Vec2, d: Vec2, p: &mut P) -> bool {
    p.dots(4);
    let (ab, cd) = (b - a, d - c);
    let (d1, d2) = (ab.cross(c - a), ab.cross(d - a));
    let (d3, d4) = (cd.cross(a - c), cd.cross(b - c));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    // Collinear or touching: an endpoint lies on the other segment.
    let on = |p: Vec2, a: Vec2, b: Vec2, side: f32| {
        side == 0.0
            && p.x >= a.x.min(b.x)
            && p.x <= a.x.max(b.x)
            && p.y >= a.y.min(b.y)
            && p.y <= a.y.max(b.y)
    };
    on(c, a, b, d1) || on(d, a, b, d2) || on(a, c, d, d3) || on(b, c, d, d4)
}

/// Dimension between witness points labelled with the signed distance `d`.
pub(crate) fn gap_dimension(a: Vec2, b: Vec2, offset: f32, d: f32) -> Note {
    if d <= 0.0 {
        Note::dimension(a, b, offset, Tone::Signal, format!("overlap {}", fmt(-d)))
    } else {
        Note::dimension(a, b, offset, Tone::Cobalt, format!("gap {}", fmt(d)))
    }
}

/// Draws the projection of both shapes onto `axis` on a line beside them,
/// with projector lines from the extreme points and the gap or overlap.
pub(crate) fn sketch_axis<P: Probe>(
    a: &Shape,
    b: &Shape,
    axis: Vec2,
    (a0, a1): (f32, f32),
    (b0, b1): (f32, f32),
    caption: impl FnOnce() -> String,
    p: &mut P,
) {
    if !p.sketching() {
        return;
    }
    // Any line perpendicular to the axis through the origin keeps the projected
    // values as distances along it; push it past both shapes.
    let side = axis.perp();
    let k = a.support(side).dot(side).max(b.support(side).dot(side)) + 40.0;
    let o = side * k;
    let at = |t: f32| o + axis * t;
    let (lo, hi) = (a0.min(b0), a1.max(b1));
    let overlap = a1.min(b1) - a0.max(b0);
    let tone = if overlap >= 0.0 {
        Tone::Signal
    } else {
        Tone::Cobalt
    };

    p.note(|| Note::dashed(at(lo - 30.0), at(hi + 30.0), Tone::Soft));
    for (shape, t0, t1) in [(a, a0, a1), (b, b0, b1)] {
        for (dir, t) in [(-axis, t0), (axis, t1)] {
            let from = shape.support(dir);
            p.note(|| Note::dashed(from, at(t), Tone::Faint));
        }
    }
    p.note(|| Note::Interval {
        origin: o - side * 5.0,
        dir: axis,
        t0: a0,
        t1: a1,
        tone: Tone::Ink,
    });
    p.note(|| Note::Interval {
        origin: o + side * 5.0,
        dir: axis,
        t0: b0,
        t1: b1,
        tone: Tone::Cobalt,
    });
    let (g0, g1) = if overlap >= 0.0 {
        (a0.max(b0), a1.min(b1))
    } else {
        (a1.min(b1), a0.max(b0))
    };
    let text = if overlap >= 0.0 {
        format!("overlap {}", fmt(overlap))
    } else {
        format!("gap {}", fmt(-overlap))
    };
    p.note(|| Note::dimension(at(g0), at(g1), 22.0, tone, text));
    p.note(|| Note::label(at(hi + 30.0) + side * 14.0, Tone::Soft, caption()));
}
