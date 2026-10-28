//! Circle–circle: the classic center-distance test.

use super::gap_dimension;
use super::prelude::*;

/// Unpacks both circles as `(ca, ra, cb, rb)`; panics on other shapes.
fn circles(a: &Shape, b: &Shape) -> (Vec2, f32, Vec2, f32) {
    match (*a, *b) {
        (Shape::Circle { c: ca, r: ra }, Shape::Circle { c: cb, r: rb }) => (ca, ra, cb, rb),
        _ => unreachable!("circle–circle method on other shapes"),
    }
}

/// Compares squared distances, so a yes/no answer needs no square root at all.
pub fn squared<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (ca, ra, cb, rb) = circles(a, b);
    let d2 = (cb - ca).length_squared();
    p.dots(1);
    let sum = ra + rb;
    let hit = d2 <= sum * sum;
    p.note(|| {
        Note::dimension(
            ca,
            cb,
            -ra.max(rb) - 28.0,
            Tone::Soft,
            format!("d² {d2:.0}"),
        )
    });
    p.note(|| {
        Note::label(
            (ca + cb) * 0.5 + Vec2::new(0.0, ra.max(rb) + 28.0),
            if hit { Tone::Signal } else { Tone::Cobalt },
            format!("(rA + rB)² {:.0}", sum * sum),
        )
    });
    Contact::boolean(hit)
}

/// Takes the square root for the actual gap or penetration depth.
pub fn distance<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (ca, ra, cb, rb) = circles(a, b);
    let d = cb - ca;
    let len = d.length();
    p.dots(1);
    p.sqrt();
    let gap = len - ra - rb;
    p.note(|| {
        Note::dimension(
            ca,
            cb,
            -ra.max(rb) - 28.0,
            Tone::Soft,
            format!("d {}", fmt(len)),
        )
    });
    if len <= f32::EPSILON {
        return Contact::signed(gap, None, None);
    }
    p.div();
    let n = d / len;
    let (pa, pb) = (ca + n * ra, cb - n * rb);
    p.note(|| gap_dimension(pa, pb, ra.max(rb) + 28.0, gap));
    Contact::signed(gap, Some(n), Some((pa, pb)))
}
