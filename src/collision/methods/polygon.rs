//! Polygon tests that work directly on the edges.

use super::gap_dimension;
use super::prelude::*;
use super::{closest_on_segment, inside_convex, segments_cross};

/// Circle against a convex polygon: the closest boundary point lies on the
/// nearest edge, either inside its span (edge region) or at an end vertex
/// (vertex region). Inside the polygon, the nearest edge is the way out.
pub fn circle_voronoi<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (Shape::Circle { c, r }, Some(poly)) = (*a, b.polygon()) else {
        unreachable!("circle–polygon method on other shapes");
    };
    let mut inside = true;
    let mut best = (f32::MAX, c, 0, 0.0);
    for i in 0..poly.len() {
        let (v0, v1) = poly.edge(i);
        p.dots(1);
        inside &= (v1 - v0).cross(c - v0) >= 0.0;
        let (q, t) = closest_on_segment(c, v0, v1, p);
        let d2 = (q - c).length_squared();
        p.dots(1);
        if d2 < best.0 {
            best = (d2, q, i, t);
        }
    }
    let (d2, q, edge, t) = best;
    let len = d2.sqrt();
    p.sqrt();

    if p.sketching() {
        let (v0, v1) = poly.edge(edge);
        let region = if t <= 0.0 || t >= 1.0 {
            "vertex region"
        } else {
            "edge region"
        };
        p.note(|| Note::segment(v0, v1, Tone::Verdigris));
        p.note(|| Note::dashed(c, q, Tone::Soft));
        p.note(|| Note::Point {
            p: q,
            tone: Tone::Ink,
        });
        p.note(|| Note::label(q + Vec2::new(10.0, -12.0), Tone::Verdigris, region));
    }

    let contact = if len <= f32::EPSILON {
        Contact::signed(-r, None, None)
    } else {
        p.div();
        // Unit direction from the center to the nearest boundary point.
        let to_edge = (q - c) / len;
        if inside {
            Contact::signed(-(r + len), Some(-to_edge), Some((c - to_edge * r, q)))
        } else {
            Contact::signed(len - r, Some(to_edge), Some((c + to_edge * r, q)))
        }
    };
    if let (Some(d), Some((pa, pb))) = (contact.distance, contact.points) {
        p.note(|| gap_dimension(pa, pb, 24.0, d));
    }
    contact
}

/// The textbook brute force: polygons overlap iff an edge of one crosses an
/// edge of the other, or one contains the other. O(n·m) segment tests.
pub fn edge_crossing<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (Some(pa), Some(pb)) = (a.polygon(), b.polygon()) else {
        unreachable!("edge test on non-polygons");
    };
    for i in 0..pa.len() {
        let (a0, a1) = pa.edge(i);
        for j in 0..pb.len() {
            p.iteration();
            let (b0, b1) = pb.edge(j);
            if segments_cross(a0, a1, b0, b1, p) {
                p.note(|| Note::segment(a0, a1, Tone::Signal));
                p.note(|| Note::segment(b0, b1, Tone::Signal));
                p.note(|| {
                    Note::label(
                        (a0 + a1 + b0 + b1) * 0.25 + Vec2::new(0.0, -14.0),
                        Tone::Signal,
                        "edges cross",
                    )
                });
                return Contact::boolean(true);
            }
        }
    }
    // No crossings: either one polygon is inside the other, or they are apart.
    for (outer, inner) in [(&pb, &pa), (&pa, &pb)] {
        let v = inner.verts()[0];
        if inside_convex(outer, v, p) {
            p.note(|| Note::Point {
                p: v,
                tone: Tone::Signal,
            });
            p.note(|| Note::label(v + Vec2::new(10.0, -12.0), Tone::Signal, "contained"));
            return Contact::boolean(true);
        }
    }
    p.note(|| {
        Note::label(
            a.center().lerp(b.center(), 0.5),
            Tone::Cobalt,
            "no crossings, no containment",
        )
    });
    Contact::boolean(false)
}
