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
        return crossing_cores(a0, a1, b0, b1, ra, rb, p);
    }
    inflate(ca, cb, ra, rb, p)
}

/// Crossing cores: the Minkowski difference of two segments is a
/// parallelogram that now contains the origin. The distance to its nearest
/// edge is how far the cores must move apart; both radii add on top.
fn crossing_cores<P: Probe>(
    a0: Vec2,
    a1: Vec2,
    b0: Vec2,
    b1: Vec2,
    ra: f32,
    rb: f32,
    p: &mut P,
) -> Contact {
    // Corners as (point on A, point on B); consecutive corners differ in one end point.
    let corners = [(a0, b0), (a1, b0), (a1, b1), (a0, b1)];
    let mut best = (f32::MAX, Vec2::default(), a0, b0);
    for i in 0..4 {
        let ((ai, bi), (aj, bj)) = (corners[i], corners[(i + 1) % 4]);
        let (q, t) = closest_on_segment(Vec2::default(), ai - bi, aj - bj, p);
        let d2 = q.length_squared();
        p.dots(1);
        if d2 < best.0 {
            best = (d2, q, ai.lerp(aj, t), bi.lerp(bj, t));
        }
    }
    let (d2, q, ca, cb) = best;
    let d = d2.sqrt();
    p.sqrt();
    if p.sketching() {
        let outline: Vec<Vec2> = corners.iter().map(|&(a, b)| a - b).collect();
        p.note(|| Note::Polygon {
            points: outline,
            tone: Tone::Soft,
            dashed: true,
            fill: false,
        });
        p.note(|| Note::label(Vec2::new(0.0, -16.0), Tone::Soft, "core A − core B"));
        p.note(|| Note::Point {
            p: Vec2::default(),
            tone: Tone::Ink,
        });
        p.note(|| Note::label(ca + Vec2::new(10.0, -14.0), Tone::Signal, "cores cross"));
    }
    // On the boundary (or a flat parallelogram for collinear cores) the
    // shortest way out is sideways, across the core.
    let normal = if d > f32::EPSILON {
        p.div();
        Some(q / d)
    } else {
        let e = a1 - a0;
        (e.length_squared() > f32::EPSILON).then(|| {
            p.sqrt();
            p.div();
            e.perp().normalized()
        })
    };
    let depth = d + ra + rb;
    let Some(n) = normal else {
        return Contact::signed(-depth, None, None);
    };
    let (pa, pb) = (ca + n * ra, cb - n * rb);
    p.note(|| gap_dimension(pa, pb, 24.0, -depth));
    Contact::signed(-depth, Some(n), Some((pa, pb)))
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
