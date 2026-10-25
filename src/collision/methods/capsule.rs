//! Capsules are segments inflated by a radius, so every capsule test reduces
//! to a distance between the core segment and the other shape's core.

use super::gap_dimension;
use super::prelude::*;
use super::{closest_between_segments, closest_on_segment, inside_convex, segments_cross};

fn capsule(s: &Shape) -> (Vec2, Vec2, f32) {
    match *s {
        Shape::Capsule { p, q, r } => (p, q, r),
        _ => unreachable!("capsule method on other shapes"),
    }
}

/// Contact between two cores at closest points `ca`, `cb`, inflated by `ra` and `rb`.
fn inflate<P: Probe>(ca: Vec2, cb: Vec2, ra: f32, rb: f32, p: &mut P) -> Contact {
    let d = cb - ca;
    let len = d.length();
    p.dots(1);
    p.sqrt();
    p.note(|| Note::dashed(ca, cb, Tone::Soft));
    p.note(|| Note::Point {
        p: ca,
        tone: Tone::Ink,
    });
    p.note(|| Note::Point {
        p: cb,
        tone: Tone::Ink,
    });
    if len <= f32::EPSILON {
        return Contact::signed(-(ra + rb), None, None);
    }
    p.div();
    let n = d / len;
    let (pa, pb) = (ca + n * ra, cb - n * rb);
    let gap = len - ra - rb;
    p.note(|| Note::dimension(ca, cb, -24.0, Tone::Soft, format!("core {}", fmt(len))));
    p.note(|| gap_dimension(pa, pb, 24.0, gap));
    Contact::signed(gap, Some(n), Some((pa, pb)))
}

fn sketch_core<P: Probe>(a: Vec2, b: Vec2, p: &mut P) {
    p.note(|| Note::dashed(a, b, Tone::Verdigris));
}

/// Circle against a capsule: point-to-segment distance.
pub fn circle_capsule<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let Shape::Circle { c, r } = *a else {
        unreachable!("circle–capsule method on other shapes");
    };
    let (s0, s1, rb) = capsule(b);
    sketch_core(s0, s1, p);
    let (q, _) = closest_on_segment(c, s0, s1, p);
    inflate(c, q, r, rb, p)
}

/// Capsule against capsule: closest points between the two core segments.
pub fn capsule_capsule<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let ((a0, a1, ra), (b0, b1, rb)) = (capsule(a), capsule(b));
    sketch_core(a0, a1, p);
    sketch_core(b0, b1, p);
    let (ca, cb) = closest_between_segments(a0, a1, b0, b1, p);
    if (cb - ca).length_squared() <= f32::EPSILON {
        // Crossing cores: overlapping, but the depth needs a different method.
        p.note(|| Note::label(ca + Vec2::new(10.0, -14.0), Tone::Signal, "cores cross"));
        return Contact::boolean(true);
    }
    inflate(ca, cb, ra, rb, p)
}

/// Capsule against a polygon: the core segment's distance to the polygon,
/// minus the radius. A core that reaches into the polygon only gives a yes.
pub fn capsule_polygon<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (s0, s1, r) = capsule(a);
    let poly = b.polygon().expect("polygonal shape");
    sketch_core(s0, s1, p);
    if inside_convex(&poly, s0, p) {
        p.note(|| Note::label(s0 + Vec2::new(10.0, -14.0), Tone::Signal, "core inside"));
        return Contact::boolean(true);
    }
    let mut best = (f32::MAX, s0, s0);
    for i in 0..poly.len() {
        let (v0, v1) = poly.edge(i);
        if segments_cross(s0, s1, v0, v1, p) {
            p.note(|| Note::segment(v0, v1, Tone::Signal));
            p.note(|| {
                Note::label(
                    (v0 + v1) * 0.5 + Vec2::new(10.0, -14.0),
                    Tone::Signal,
                    "core crosses edge",
                )
            });
            return Contact::boolean(true);
        }
        let (ca, cb) = closest_between_segments(s0, s1, v0, v1, p);
        p.note(|| Note::dashed(ca, cb, Tone::Faint));
        let d2 = (cb - ca).length_squared();
        p.dots(1);
        if d2 < best.0 {
            best = (d2, ca, cb);
        }
    }
    inflate(best.1, best.2, r, 0.0, p)
}
