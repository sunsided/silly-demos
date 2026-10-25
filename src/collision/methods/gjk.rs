//! Gilbert–Johnson–Keerthi distance algorithm and the Expanding Polytope
//! Algorithm. Both only query the shapes' support functions, so they work for
//! every convex pair and serve as the reference the other methods are checked against.

use super::prelude::*;

const MAX_GJK_ITERATIONS: usize = 32;
/// Caps EPA on curved shapes, which it otherwise keeps refining.
const MAX_EPA_ITERATIONS: usize = 48;
/// The starting triangle grows by one vertex per EPA iteration.
const MAX_POLYTOPE: usize = 3 + MAX_EPA_ITERATIONS;
/// Closer than this to the origin counts as touching.
const TOUCH: f32 = 1e-4;
/// Relative convergence tolerance for the GJK distance.
const GJK_TOLERANCE: f32 = 1e-5;
/// Absolute convergence tolerance for the EPA depth, in px.
const EPA_TOLERANCE: f32 = 1e-3;

/// A point of the Minkowski difference `A − B` with the support points it came from.
#[derive(Clone, Copy, Debug, Default)]
struct Vertex {
    w: Vec2,
    a: Vec2,
    b: Vec2,
}

fn support<P: Probe>(sa: &Shape, sb: &Shape, d: Vec2, p: &mut P) -> Vertex {
    p.support();
    p.support();
    let (a, b) = (sa.support(d), sb.support(-d));
    Vertex { w: a - b, a, b }
}

/// Up to three Minkowski vertices with barycentric weights of the point closest to the origin.
#[derive(Clone, Copy, Debug, Default)]
struct Simplex {
    v: [Vertex; 3],
    l: [f32; 3],
    n: usize,
}

impl Simplex {
    fn closest(&self) -> Vec2 {
        (0..self.n).fold(Vec2::default(), |s, i| s + self.v[i].w * self.l[i])
    }

    fn witnesses(&self) -> (Vec2, Vec2) {
        (0..self.n).fold((Vec2::default(), Vec2::default()), |(a, b), i| {
            (a + self.v[i].a * self.l[i], b + self.v[i].b * self.l[i])
        })
    }

    /// Closest point to the origin on the segment `v[i]`–`v[j]` as a parameter in `[0, 1]`.
    fn segment<P: Probe>(&self, i: usize, j: usize, p: &mut P) -> f32 {
        let (a, e) = (self.v[i].w, self.v[j].w - self.v[i].w);
        let ee = e.length_squared();
        p.dots(2);
        if ee <= f32::EPSILON {
            return 0.0;
        }
        p.div();
        (-a.dot(e) / ee).clamp(0.0, 1.0)
    }

    fn keep_segment(&mut self, i: usize, j: usize, t: f32) {
        let (vi, vj) = (self.v[i], self.v[j]);
        if t <= 0.0 {
            (self.v[0], self.l[0], self.n) = (vi, 1.0, 1);
        } else if t >= 1.0 {
            (self.v[0], self.l[0], self.n) = (vj, 1.0, 1);
        } else {
            self.v[0] = vi;
            self.v[1] = vj;
            self.l = [1.0 - t, t, 0.0];
            self.n = 2;
        }
    }

    /// Shrinks the simplex to the feature nearest the origin. Returns false
    /// when the triangle encloses the origin, i.e. the shapes overlap.
    fn reduce<P: Probe>(&mut self, p: &mut P) -> bool {
        match self.n {
            1 => self.l[0] = 1.0,
            2 => {
                let t = self.segment(0, 1, p);
                self.keep_segment(0, 1, t);
            }
            _ => {
                let [a, b, c] = [self.v[0].w, self.v[1].w, self.v[2].w];
                let area = (b - a).cross(c - a);
                p.dots(4);
                let s = area.signum();
                if area.abs() > f32::EPSILON
                    && s * (b - a).cross(-a) >= 0.0
                    && s * (c - b).cross(-b) >= 0.0
                    && s * (a - c).cross(-c) >= 0.0
                {
                    return false;
                }
                let mut best = (f32::MAX, 0, 1, 0.0);
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    let t = self.segment(i, j, p);
                    let d = self.v[i].w.lerp(self.v[j].w, t).length_squared();
                    p.dots(1);
                    if d < best.0 {
                        best = (d, i, j, t);
                    }
                }
                self.keep_segment(best.1, best.2, best.3);
            }
        }
        true
    }
}

struct Gjk {
    intersect: bool,
    simplex: Simplex,
}

/// GJK on `A − B`. With `early_out` it stops as soon as a separating direction
/// is found (boolean query); otherwise it converges to the closest points.
fn gjk<P: Probe>(sa: &Shape, sb: &Shape, early_out: bool, p: &mut P) -> Gjk {
    let mut d = sb.center() - sa.center();
    if d.length_squared() <= f32::EPSILON {
        d = Vec2::new(1.0, 0.0);
    }
    let mut s = Simplex {
        n: 1,
        l: [1.0, 0.0, 0.0],
        ..Simplex::default()
    };
    s.v[0] = support(sa, sb, d, p);
    let mut v = s.v[0].w;

    for _ in 0..MAX_GJK_ITERATIONS {
        p.iteration();
        let vv = v.length_squared();
        p.dots(1);
        if vv <= TOUCH * TOUCH {
            return Gjk {
                intersect: true,
                simplex: s,
            };
        }
        let w = support(sa, sb, -v, p);
        let vw = v.dot(w.w);
        p.dots(1);
        if (early_out && vw > 0.0) || vv - vw <= GJK_TOLERANCE * vv {
            break;
        }
        s.v[s.n] = w;
        s.n += 1;
        if !s.reduce(p) {
            return Gjk {
                intersect: true,
                simplex: s,
            };
        }
        let next = s.closest();
        if next.length_squared() >= vv {
            break;
        }
        v = next;
    }
    Gjk {
        intersect: false,
        simplex: s,
    }
}

struct Epa {
    depth: f32,
    normal: Vec2,
    points: (Vec2, Vec2),
    polytope: Vec<Vec2>,
}

/// Expands the enclosing GJK triangle towards the boundary of `A − B`
/// to find the penetration depth and direction.
fn epa<P: Probe>(sa: &Shape, sb: &Shape, s: &Simplex, p: &mut P) -> Option<Epa> {
    if s.n < 3 {
        return None;
    }
    let mut poly = [Vertex::default(); MAX_POLYTOPE];
    poly[..3].copy_from_slice(&s.v);
    let area = (poly[1].w - poly[0].w).cross(poly[2].w - poly[0].w);
    p.dots(1);
    if area.abs() <= f32::EPSILON {
        return None;
    }
    if area < 0.0 {
        poly.swap(1, 2);
    }

    for len in 3..=MAX_POLYTOPE {
        p.iteration();
        // Edge nearest the origin, with its outward normal.
        let (mut edge, mut depth, mut normal) = (0, f32::MAX, Vec2::default());
        for i in 0..len {
            let e = poly[(i + 1) % len].w - poly[i].w;
            let n = Vec2::new(e.y, -e.x).normalized();
            p.sqrt();
            p.div();
            let d = n.dot(poly[i].w);
            p.dots(2);
            if d < depth {
                (edge, depth, normal) = (i, d, n);
            }
        }
        let w = support(sa, sb, normal, p);
        let gain = w.w.dot(normal) - depth;
        p.dots(1);
        if gain <= EPA_TOLERANCE || len == MAX_POLYTOPE {
            let (vi, vj) = (poly[edge], poly[(edge + 1) % len]);
            let e = vj.w - vi.w;
            let ee = e.length_squared();
            let t = if ee > f32::EPSILON {
                (-vi.w.dot(e) / ee).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p.dots(2);
            p.div();
            return Some(Epa {
                depth,
                normal,
                points: (vi.a.lerp(vj.a, t), vi.b.lerp(vj.b, t)),
                polytope: if p.sketching() {
                    poly[..len].iter().map(|v| v.w).collect()
                } else {
                    Vec::new()
                },
            });
        }
        poly.copy_within(edge + 1..len, edge + 2);
        poly[edge + 1] = w;
    }
    unreachable!("EPA returns by its iteration cap")
}

/// Outline of `A − B`, sampled from the support functions (uncounted; for drawing only).
fn minkowski_outline(sa: &Shape, sb: &Shape) -> Vec<Vec2> {
    (0..72)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / 72.0;
            let d = Vec2::new(t.cos(), t.sin());
            sa.support(d) - sb.support(-d)
        })
        .collect()
}

fn sketch_minkowski<P: Probe>(sa: &Shape, sb: &Shape, s: &Simplex, intersect: bool, p: &mut P) {
    if !p.sketching() {
        return;
    }
    let tone = if intersect {
        Tone::Signal
    } else {
        Tone::Cobalt
    };
    let outline = minkowski_outline(sa, sb);
    let top = outline.iter().fold(f32::MAX, |m, v| m.min(v.y));
    let c = sa.center() - sb.center();
    p.note(|| Note::Polygon {
        points: outline,
        tone: Tone::Soft,
        dashed: true,
        fill: false,
    });
    p.note(|| Note::label(Vec2::new(c.x, top - 12.0), Tone::Soft, "A − B"));
    p.note(|| Note::Point {
        p: Vec2::default(),
        tone: Tone::Ink,
    });
    p.note(|| Note::label(Vec2::new(10.0, 12.0), Tone::Ink, "origin"));
    let pts: Vec<Vec2> = s.v[..s.n].iter().map(|v| v.w).collect();
    for &w in &pts {
        p.note(|| Note::Point { p: w, tone });
    }
    p.note(|| Note::Polygon {
        points: pts,
        tone,
        dashed: false,
        fill: s.n == 3,
    });
}

/// GJK as a yes/no query: stops at the first separating direction.
pub fn gjk_boolean<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let g = gjk(a, b, true, p);
    sketch_minkowski(a, b, &g.simplex, g.intersect, p);
    Contact::boolean(g.intersect)
}

/// GJK to convergence for the distance, EPA for the penetration depth.
pub fn gjk_epa<P: Probe>(a: &Shape, b: &Shape, p: &mut P) -> Contact {
    let g = gjk(a, b, false, p);
    if !g.intersect {
        let v = g.simplex.closest();
        let dist = v.length();
        p.sqrt();
        let (pa, pb) = g.simplex.witnesses();
        sketch_minkowski(a, b, &g.simplex, false, p);
        p.note(|| {
            Note::dimension(
                Vec2::default(),
                v,
                0.0,
                Tone::Cobalt,
                format!("|v| {}", fmt(dist)),
            )
        });
        p.note(|| Note::dimension(pa, pb, 0.0, Tone::Cobalt, format!("gap {}", fmt(dist))));
        return Contact::signed(dist, Some(-v / dist), Some((pa, pb)));
    }
    match epa(a, b, &g.simplex, p) {
        Some(e) => {
            if p.sketching() {
                sketch_minkowski(a, b, &g.simplex, true, p);
                let poly = e.polytope.clone();
                p.note(|| Note::Polygon {
                    points: poly,
                    tone: Tone::Verdigris,
                    dashed: false,
                    fill: false,
                });
                let n = e.normal * e.depth;
                p.note(|| {
                    Note::dimension(
                        Vec2::default(),
                        n,
                        0.0,
                        Tone::Signal,
                        format!("depth {}", fmt(e.depth)),
                    )
                });
                p.note(|| Note::Arrow {
                    a: e.points.1,
                    b: e.points.0,
                    tone: Tone::Signal,
                });
            }
            Contact::signed(-e.depth, Some(e.normal), Some(e.points))
        }
        // Touching, or a degenerate simplex: overlapping by (almost) nothing.
        None => {
            sketch_minkowski(a, b, &g.simplex, true, p);
            Contact::signed(0.0, None, None)
        }
    }
}
