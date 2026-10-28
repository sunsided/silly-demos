//! Box tests: interval overlap for two AABBs, and clamping the circle's
//! center into the box for circle–box.

use super::gap_dimension;
use super::prelude::*;

/// Two AABBs overlap iff their x and y intervals both overlap: four comparisons.
pub fn aabb_minmax<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (
        Shape::Aabb {
            min: amin,
            max: amax,
        },
        Shape::Aabb {
            min: bmin,
            max: bmax,
        },
    ) = (*a, *b)
    else {
        unreachable!("AABB method on other shapes");
    };
    // Signed gap per axis; negative values are overlaps.
    let gx = (bmin.x - amax.x).max(amin.x - bmax.x);
    let gy = (bmin.y - amax.y).max(amin.y - bmax.y);

    if p.sketching() {
        let below = amax.y.max(bmax.y) + 36.0;
        let left = amin.x.min(bmin.x) - 36.0;
        for (lo, hi, tone, k) in [
            (amin, amax, Tone::Ink, -4.0),
            (bmin, bmax, Tone::Cobalt, 4.0),
        ] {
            p.note(|| Note::Interval {
                origin: Vec2::new(0.0, below + k),
                dir: Vec2::new(1.0, 0.0),
                t0: lo.x,
                t1: hi.x,
                tone,
            });
            p.note(|| Note::Interval {
                origin: Vec2::new(left + k, 0.0),
                dir: Vec2::new(0.0, 1.0),
                t0: lo.y,
                t1: hi.y,
                tone,
            });
        }
        let x = (amax.x.min(bmax.x), amin.x.max(bmin.x));
        let y = (amax.y.min(bmax.y), amin.y.max(bmin.y));
        let tone = |g: f32| if g <= 0.0 { Tone::Signal } else { Tone::Cobalt };
        let text = |g: f32| {
            if g <= 0.0 {
                format!("overlap {}", fmt(-g))
            } else {
                format!("gap {}", fmt(g))
            }
        };
        p.note(|| {
            Note::dimension(
                Vec2::new(x.0.min(x.1), below),
                Vec2::new(x.0.max(x.1), below),
                20.0,
                tone(gx),
                text(gx),
            )
        });
        p.note(|| {
            Note::dimension(
                Vec2::new(left, y.0.min(y.1)),
                Vec2::new(left, y.0.max(y.1)),
                20.0,
                tone(gy),
                text(gy),
            )
        });
    }

    if gx > 0.0 || gy > 0.0 {
        // Closest points: on a separated axis the facing sides, otherwise the overlap's midpoint.
        let pick = |ga: f32, amin: f32, amax: f32, bmin: f32, bmax: f32| {
            if ga > 0.0 {
                if bmin > amax {
                    (amax, bmin)
                } else {
                    (amin, bmax)
                }
            } else {
                let m = (amin.max(bmin) + amax.min(bmax)) * 0.5;
                (m, m)
            }
        };
        let (ax, bx) = pick(gx, amin.x, amax.x, bmin.x, bmax.x);
        let (ay, by) = pick(gy, amin.y, amax.y, bmin.y, bmax.y);
        let (pa, pb) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        let d = gx.max(0.0).hypot(gy.max(0.0));
        p.sqrt();
        p.div();
        return Contact::signed(d, Some((pb - pa) / d), Some((pa, pb)));
    }
    // Overlapping: push out along the axis needing the smaller move.
    let px = (amax.x - bmin.x).min(bmax.x - amin.x);
    let py = (amax.y - bmin.y).min(bmax.y - amin.y);
    let normal = if px < py {
        Vec2::new(
            if amax.x - bmin.x < bmax.x - amin.x {
                1.0
            } else {
                -1.0
            },
            0.0,
        )
    } else {
        Vec2::new(
            0.0,
            if amax.y - bmin.y < bmax.y - amin.y {
                1.0
            } else {
                -1.0
            },
        )
    };
    Contact::signed(-px.min(py), Some(normal), None)
}

/// Circle against a box given in the box's local frame (centered, axis aligned).
/// `to_world` maps local directions back to world space.
fn circle_vs_local_box<P: Probe>(
    lc: Vec2,
    r: f32,
    half: Vec2,
    center: Vec2,
    to_world: impl Fn(Vec2) -> Vec2,
    p: &mut P,
) -> (Contact, Vec2) {
    let q = Vec2::new(lc.x.clamp(-half.x, half.x), lc.y.clamp(-half.y, half.y));
    let d = lc - q;
    let dd = d.length_squared();
    p.dots(1);
    let world = |v: Vec2| center + to_world(v);
    if dd > 0.0 {
        let len = dd.sqrt();
        p.sqrt();
        p.div();
        // Normal from the circle towards the box.
        let n = to_world(-d / len);
        let (pa, pb) = (world(lc) + n * r, world(q));
        return (Contact::signed(len - r, Some(n), Some((pa, pb))), world(q));
    }
    // Center inside the box: leave through the nearest face.
    let faces = [
        (half.x - lc.x, Vec2::new(1.0, 0.0)),
        (lc.x + half.x, Vec2::new(-1.0, 0.0)),
        (half.y - lc.y, Vec2::new(0.0, 1.0)),
        (lc.y + half.y, Vec2::new(0.0, -1.0)),
    ];
    let (f, out) = faces
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .expect("four faces");
    let out = to_world(out);
    let c = world(lc);
    let face = c + out * f;
    (
        Contact::signed(-(r + f), Some(-out), Some((c - out * r, face))),
        face,
    )
}

/// Draws the clamp from the circle's center `c` to the box point `q`, plus the
/// gap or overlap when the contact has one.
fn sketch_clamp<P: Probe>(c: Vec2, q: Vec2, contact: &Contact, p: &mut P) {
    p.note(|| Note::dashed(c, q, Tone::Soft));
    p.note(|| Note::Point {
        p: q,
        tone: Tone::Ink,
    });
    p.note(|| Note::label(q + Vec2::new(10.0, -12.0), Tone::Ink, "clamped"));
    if let (Some(d), Some((pa, pb))) = (contact.distance, contact.points) {
        p.note(|| gap_dimension(pa, pb, 24.0, d));
    }
}

/// Clamp the circle's center to the box to get the closest point (Arvo's method).
pub fn circle_aabb<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (Shape::Circle { c, r }, Shape::Aabb { min, max }) = (*a, *b) else {
        unreachable!("circle–AABB method on other shapes");
    };
    let center = (min + max) * 0.5;
    let (contact, q) = circle_vs_local_box(c - center, r, (max - min) * 0.5, center, |v| v, p);
    sketch_clamp(c, q, &contact, p);
    contact
}

/// Same clamp, after rotating the circle's center into the box's frame.
pub fn circle_obb<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (Shape::Circle { c, r }, Shape::Obb { c: center, half, u }) = (*a, *b) else {
        unreachable!("circle–OBB method on other shapes");
    };
    let v = u.perp();
    let rel = c - center;
    p.dots(2);
    let lc = Vec2::new(rel.dot(u), rel.dot(v));
    let (contact, q) = circle_vs_local_box(lc, r, half, center, |l| u * l.x + v * l.y, p);
    if p.sketching() {
        let reach = half.x.max(half.y) + 30.0;
        p.note(|| Note::dashed(center - u * reach, center + u * reach, Tone::Faint));
        p.note(|| Note::dashed(center - v * reach, center + v * reach, Tone::Faint));
        p.note(|| Note::label(center + u * (reach + 14.0), Tone::Soft, "x′"));
        p.note(|| Note::label(center + v * (reach + 14.0), Tone::Soft, "y′"));
    }
    sketch_clamp(c, q, &contact, p);
    contact
}
