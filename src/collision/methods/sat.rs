//! Separating Axis Theorem: two convex shapes are disjoint iff their
//! projections are disjoint on some axis. For polygons the edge normals are
//! the only candidates; a circle adds the axis towards the nearest vertex.

use super::prelude::*;
use super::sketch_axis;
use crate::collision::shape::MAX_VERTS;

/// Candidate axes contributed by one shape.
struct Axes {
    v: [Vec2; MAX_VERTS],
    n: usize,
}

fn axes<P: Probe>(shape: &Shape, p: &mut P) -> Axes {
    let mut out = Axes {
        v: [Vec2::default(); MAX_VERTS],
        n: 0,
    };
    match *shape {
        // Box edges come in parallel pairs; their normals are already unit length.
        Shape::Aabb { .. } => {
            out.v[..2].copy_from_slice(&[Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)]);
            out.n = 2;
        }
        Shape::Obb { u, .. } => {
            out.v[..2].copy_from_slice(&[u, u.perp()]);
            out.n = 2;
        }
        Shape::Triangle(poly) | Shape::Polygon(poly) => {
            for i in 0..poly.len() {
                let (a, b) = poly.edge(i);
                let e = b - a;
                if e.length_squared() > f32::EPSILON {
                    p.sqrt();
                    p.div();
                    out.v[out.n] = Vec2::new(e.y, -e.x).normalized();
                    out.n += 1;
                }
            }
        }
        _ => {}
    }
    out
}

/// Interval of `shape` projected onto the unit `axis`.
fn project<P: Probe>(shape: &Shape, poly: Option<&Poly>, axis: Vec2, p: &mut P) -> (f32, f32) {
    match (shape, poly) {
        (Shape::Circle { c, r }, _) => {
            p.dots(1);
            let m = c.dot(axis);
            (m - r, m + r)
        }
        (_, Some(poly)) => {
            p.dots(poly.len() as u32);
            poly.verts()
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                    let t = v.dot(axis);
                    (lo.min(t), hi.max(t))
                })
        }
        _ => unreachable!("SAT takes circles and polygons"),
    }
}

/// For a circle against a polygon: the axis from the polygon vertex nearest the center.
fn circle_axis<P: Probe>(circle: &Shape, poly: &Poly, p: &mut P) -> Option<Vec2> {
    let Shape::Circle { c, .. } = *circle else {
        return None;
    };
    p.dots(poly.len() as u32);
    let nearest = poly.verts().iter().copied().min_by(|a, b| {
        (*a - c)
            .length_squared()
            .total_cmp(&(*b - c).length_squared())
    })?;
    let d = c - nearest;
    if d.length_squared() <= f32::EPSILON {
        return None;
    }
    p.sqrt();
    p.div();
    Some(d.normalized())
}

/// The axis of least overlap so far.
struct Best {
    depth: f32,
    /// The axis oriented from A towards B.
    normal: Vec2,
    axis: Vec2,
    index: usize,
    intervals: ((f32, f32), (f32, f32)),
}

/// SAT with early out on the first separating axis; on overlap the axis of
/// least penetration gives the minimum translation vector.
pub fn sat<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (pa, pb) = (a.polygon(), b.polygon());
    let (aa, ab) = (axes(a, p), axes(b, p));
    let extra = match (&pa, &pb) {
        (None, Some(poly)) => circle_axis(a, poly, p),
        (Some(poly), None) => circle_axis(b, poly, p),
        _ => None,
    };
    let candidates = aa.v[..aa.n]
        .iter()
        .chain(&ab.v[..ab.n])
        .chain(extra.as_ref());
    let total = aa.n + ab.n + extra.is_some() as usize;

    let mut best: Option<Best> = None;
    for (i, &axis) in candidates.enumerate() {
        p.axis();
        let ia = project(a, pa.as_ref(), axis, p);
        let ib = project(b, pb.as_ref(), axis, p);
        // Distance B must move along +axis, or along -axis, to separate.
        let (forward, backward) = (ia.1 - ib.0, ib.1 - ia.0);
        if forward < 0.0 || backward < 0.0 {
            sketch_axis(
                a,
                b,
                axis,
                ia,
                ib,
                || format!("separating axis {} of {total}", i + 1),
                p,
            );
            return Contact::boolean(false);
        }
        let (push, dir) = if forward < backward {
            (forward, axis)
        } else {
            (backward, -axis)
        };
        if best.as_ref().is_none_or(|b| push < b.depth) {
            best = Some(Best {
                depth: push,
                normal: dir,
                axis,
                index: i,
                intervals: (ia, ib),
            });
        }
    }
    let Some(Best {
        depth,
        normal,
        axis,
        index: i,
        intervals: (ia, ib),
    }) = best
    else {
        return Contact::boolean(true);
    };
    sketch_axis(
        a,
        b,
        axis,
        ia,
        ib,
        || format!("all {total} axes overlap, least on axis {}", i + 1),
        p,
    );
    Contact::signed(-depth, Some(normal), None)
}
