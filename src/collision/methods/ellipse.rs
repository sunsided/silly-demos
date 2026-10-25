//! Circle–ellipse. Scaling space to turn the ellipse into a circle would
//! squash the other circle into an ellipse, so instead find the exact closest
//! point on the ellipse to the circle's center and compare with the radius.

use super::gap_dimension;
use super::prelude::*;

const MAX_BISECTIONS: usize = 64;

/// Root of Eberly's F(s) = (r0·z0 / (s + r0))² + (z1 / (s + 1))² − 1 by bisection.
fn root<P: Probe>(r0: f32, z0: f32, z1: f32, g: f32, p: &mut P) -> f32 {
    let n0 = r0 * z0;
    let mut s0 = z1 - 1.0;
    let mut s1 = if g < 0.0 {
        0.0
    } else {
        p.sqrt();
        n0.hypot(z1) - 1.0
    };
    let mut s = 0.0;
    for _ in 0..MAX_BISECTIONS {
        p.iteration();
        s = 0.5 * (s0 + s1);
        if s == s0 || s == s1 {
            break;
        }
        p.div();
        p.div();
        let (q0, q1) = (n0 / (s + r0), z1 / (s + 1.0));
        let g = q0 * q0 + q1 * q1 - 1.0;
        if g > 0.0 {
            s0 = s;
        } else if g < 0.0 {
            s1 = s;
        } else {
            break;
        }
    }
    s
}

/// Closest point on the ellipse with semi-axes `e0 >= e1` to a point `y` in the
/// first quadrant (Eberly, "Distance from a Point to an Ellipse"). Works from
/// inside the ellipse too.
fn closest_first_quadrant<P: Probe>(e0: f32, e1: f32, y0: f32, y1: f32, p: &mut P) -> (f32, f32) {
    if y1 > 0.0 {
        if y0 > 0.0 {
            p.div();
            p.div();
            let (z0, z1) = (y0 / e0, y1 / e1);
            let g = z0 * z0 + z1 * z1 - 1.0;
            if g == 0.0 {
                return (y0, y1);
            }
            p.div();
            let r0 = (e0 / e1) * (e0 / e1);
            let s = root(r0, z0, z1, g, p);
            p.div();
            p.div();
            (r0 * y0 / (s + r0), y1 / (s + 1.0))
        } else {
            (0.0, e1)
        }
    } else {
        let (numer, denom) = (e0 * y0, e0 * e0 - e1 * e1);
        if numer < denom {
            p.div();
            p.sqrt();
            let x = numer / denom;
            (e0 * x, e1 * (1.0 - x * x).sqrt())
        } else {
            (e0, 0.0)
        }
    }
}

pub fn circle_ellipse<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let (
        Shape::Circle { c, r },
        Shape::Ellipse {
            c: ec,
            a: ea,
            b: eb,
            u,
        },
    ) = (*a, *b)
    else {
        unreachable!("circle–ellipse method on other shapes");
    };
    let v = u.perp();
    let rel = c - ec;
    p.dots(2);
    let (lx, ly) = (rel.dot(u), rel.dot(v));

    // Fold into the first quadrant with the major axis first, then unfold.
    let swap = ea < eb;
    let (e0, e1, y0, y1) = if swap {
        (eb, ea, ly.abs(), lx.abs())
    } else {
        (ea, eb, lx.abs(), ly.abs())
    };
    let (x0, x1) = closest_first_quadrant(e0, e1, y0, y1, p);
    let (qx, qy) = if swap { (x1, x0) } else { (x0, x1) };
    let q = ec + u * qx.copysign(lx) + v * qy.copysign(ly);

    let inside = (lx / ea).powi(2) + (ly / eb).powi(2) <= 1.0;
    p.div();
    p.div();
    let d = q - c;
    let len = d.length();
    p.dots(1);
    p.sqrt();

    if p.sketching() {
        p.note(|| Note::dashed(ec - u * ea, ec + u * ea, Tone::Faint));
        p.note(|| Note::dashed(ec - v * eb, ec + v * eb, Tone::Faint));
        p.note(|| Note::dashed(c, q, Tone::Soft));
        p.note(|| Note::Point {
            p: q,
            tone: Tone::Ink,
        });
        p.note(|| Note::label(q + Vec2::new(10.0, -12.0), Tone::Ink, "nearest on ellipse"));
    }
    if len <= f32::EPSILON {
        return Contact::signed(-r, None, None);
    }
    p.div();
    let to_q = d / len;
    let contact = if inside {
        Contact::signed(-(r + len), Some(-to_q), Some((c - to_q * r, q)))
    } else {
        Contact::signed(len - r, Some(to_q), Some((c + to_q * r, q)))
    };
    if let (Some(dist), Some((pa, pb))) = (contact.distance, contact.points) {
        p.note(|| gap_dimension(pa, pb, 24.0, dist));
    }
    contact
}
