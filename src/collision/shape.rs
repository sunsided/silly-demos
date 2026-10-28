//! Convex 2D shapes in world space, with support points, polygon conversion
//! and bounding volumes.

use crate::vec2::Vec2;

/// Upper bound on polygon vertices; keeps shapes `Copy` and allocation free.
pub const MAX_VERTS: usize = 16;

/// Which kind of shape a [`Shape`] is, without its parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    /// A disk.
    Circle,
    /// An axis-aligned box.
    Aabb,
    /// A rotated box.
    Obb,
    /// A triangle, stored as a [`Poly`].
    Triangle,
    /// A convex polygon of up to [`MAX_VERTS`] vertices.
    Polygon,
    /// A rotated ellipse.
    Ellipse,
    /// A segment inflated by a radius.
    Capsule,
}

impl ShapeKind {
    /// Every kind, in declaration order.
    pub const ALL: [ShapeKind; 7] = [
        ShapeKind::Circle,
        ShapeKind::Aabb,
        ShapeKind::Obb,
        ShapeKind::Triangle,
        ShapeKind::Polygon,
        ShapeKind::Ellipse,
        ShapeKind::Capsule,
    ];

    /// Stable lowercase name used across the WASM boundary.
    pub const fn id(self) -> &'static str {
        match self {
            ShapeKind::Circle => "circle",
            ShapeKind::Aabb => "aabb",
            ShapeKind::Obb => "obb",
            ShapeKind::Triangle => "triangle",
            ShapeKind::Polygon => "polygon",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Capsule => "capsule",
        }
    }

    /// Inverse of [`ShapeKind::id`]; `None` for unknown names.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Shapes with straight edges that convert to a [`Poly`].
    pub const fn is_polygonal(self) -> bool {
        matches!(
            self,
            ShapeKind::Aabb | ShapeKind::Obb | ShapeKind::Triangle | ShapeKind::Polygon
        )
    }
}

/// A convex polygon with counter-clockwise (positive area) winding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    /// Vertex storage; only the first `n` entries are meaningful.
    v: [Vec2; MAX_VERTS],
    /// Number of vertices in use, at most [`MAX_VERTS`].
    n: usize,
}

impl Poly {
    /// Takes vertices that are already convex and counter-clockwise.
    pub fn new(points: &[Vec2]) -> Self {
        let n = points.len().min(MAX_VERTS);
        let mut v = [Vec2::default(); MAX_VERTS];
        v[..n].copy_from_slice(&points[..n]);
        Self { v, n }
    }

    /// Convex hull (Andrew's monotone chain) of arbitrary points.
    pub fn hull(points: &[Vec2]) -> Self {
        let mut pts: Vec<Vec2> = points.iter().copied().take(MAX_VERTS).collect();
        pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        pts.dedup();
        if pts.len() < 3 {
            return Self::new(&pts);
        }
        // Appends `p`, popping points that would make a clockwise turn; never
        // pops below `floor` so the upper chain keeps the lower one intact.
        fn push(hull: &mut Vec<Vec2>, p: Vec2, floor: usize) {
            while hull.len() >= floor + 2 {
                let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
                if (b - a).cross(p - a) > 0.0 {
                    break;
                }
                hull.pop();
            }
            hull.push(p);
        }
        let mut hull = Vec::with_capacity(pts.len() * 2);
        for &p in &pts {
            push(&mut hull, p, 0);
        }
        let lower = hull.len() - 1;
        for &p in pts.iter().rev().skip(1) {
            push(&mut hull, p, lower);
        }
        hull.pop();
        Self::new(&hull)
    }

    /// The vertices in counter-clockwise order.
    pub fn verts(&self) -> &[Vec2] {
        &self.v[..self.n]
    }

    /// Number of vertices.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Edge `i` from vertex `i` to vertex `i + 1`.
    pub fn edge(&self, i: usize) -> (Vec2, Vec2) {
        (self.v[i], self.v[(i + 1) % self.n])
    }

    /// The same polygon moved by `d`.
    pub fn translated(&self, d: Vec2) -> Self {
        let mut out = *self;
        for v in &mut out.v[..out.n] {
            *v += d;
        }
        out
    }

    /// Mean of the vertices (not the area centroid); the origin for an empty polygon.
    pub fn centroid(&self) -> Vec2 {
        let sum = self.verts().iter().fold(Vec2::default(), |s, &v| s + v);
        sum / self.n.max(1) as f32
    }
}

/// A convex shape in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A disk.
    Circle {
        /// Center.
        c: Vec2,
        /// Radius, non-negative.
        r: f32,
    },
    /// An axis-aligned box.
    Aabb {
        /// Corner with the smallest coordinates.
        min: Vec2,
        /// Corner with the largest coordinates.
        max: Vec2,
    },
    /// `u` is the unit local x axis; the local y axis is `u.perp()`.
    Obb {
        /// Center.
        c: Vec2,
        /// Half extents along `u` and `u.perp()`.
        half: Vec2,
        /// Unit local x axis.
        u: Vec2,
    },
    /// A triangle as a counter-clockwise [`Poly`].
    Triangle(Poly),
    /// A convex polygon as a counter-clockwise [`Poly`].
    Polygon(Poly),
    /// Semi-axes `a` along `u` and `b` along `u.perp()`.
    Ellipse {
        /// Center.
        c: Vec2,
        /// Semi-axis along `u`.
        a: f32,
        /// Semi-axis along `u.perp()`.
        b: f32,
        /// Unit direction of the `a` axis.
        u: Vec2,
    },
    /// Segment `p`–`q` swept by a disk of radius `r`.
    Capsule {
        /// One end of the core segment.
        p: Vec2,
        /// Other end of the core segment.
        q: Vec2,
        /// Radius of the swept disk.
        r: f32,
    },
}

impl Shape {
    /// Builds a shape from the flat parameter list used across the WASM boundary:
    ///
    /// - circle: `x, y, r`
    /// - aabb: `x, y, half_w, half_h`
    /// - obb / ellipse: `x, y, half_w, half_h, angle`
    /// - triangle / polygon: `x0, y0, x1, y1, ...` (convex hull is taken)
    /// - capsule: `x0, y0, x1, y1, r`
    pub fn from_params(kind: ShapeKind, p: &[f32]) -> Option<Self> {
        let at = |i: usize| p.get(i).copied().filter(|v| v.is_finite());
        let v = |i: usize| Some(Vec2::new(at(i)?, at(i + 1)?));
        let dir = |i: usize| at(i).map(|a| Vec2::new(a.cos(), a.sin()));
        Some(match kind {
            ShapeKind::Circle => Shape::Circle {
                c: v(0)?,
                r: at(2)?.abs(),
            },
            ShapeKind::Aabb => {
                let c = v(0)?;
                let h = Vec2::new(at(2)?.abs(), at(3)?.abs());
                Shape::Aabb {
                    min: c - h,
                    max: c + h,
                }
            }
            ShapeKind::Obb => Shape::Obb {
                c: v(0)?,
                half: Vec2::new(at(2)?.abs(), at(3)?.abs()),
                u: dir(4)?,
            },
            ShapeKind::Ellipse => Shape::Ellipse {
                c: v(0)?,
                a: at(2)?.abs(),
                b: at(3)?.abs(),
                u: dir(4)?,
            },
            ShapeKind::Triangle | ShapeKind::Polygon => {
                let pts: Vec<Vec2> = (0..p.len() / 2).map_while(|i| v(i * 2)).collect();
                if pts.is_empty() {
                    return None;
                }
                let hull = Poly::hull(&pts);
                if kind == ShapeKind::Triangle {
                    Shape::Triangle(hull)
                } else {
                    Shape::Polygon(hull)
                }
            }
            ShapeKind::Capsule => Shape::Capsule {
                p: v(0)?,
                q: v(2)?,
                r: at(4)?.abs(),
            },
        })
    }

    /// The same shape moved by `d`.
    pub fn translated(&self, d: Vec2) -> Self {
        match *self {
            Shape::Circle { c, r } => Shape::Circle { c: c + d, r },
            Shape::Aabb { min, max } => Shape::Aabb {
                min: min + d,
                max: max + d,
            },
            Shape::Obb { c, half, u } => Shape::Obb { c: c + d, half, u },
            Shape::Triangle(p) => Shape::Triangle(p.translated(d)),
            Shape::Polygon(p) => Shape::Polygon(p.translated(d)),
            Shape::Ellipse { c, a, b, u } => Shape::Ellipse { c: c + d, a, b, u },
            Shape::Capsule { p, q, r } => Shape::Capsule {
                p: p + d,
                q: q + d,
                r,
            },
        }
    }

    /// Which [`ShapeKind`] this shape is.
    pub fn kind(&self) -> ShapeKind {
        match self {
            Shape::Circle { .. } => ShapeKind::Circle,
            Shape::Aabb { .. } => ShapeKind::Aabb,
            Shape::Obb { .. } => ShapeKind::Obb,
            Shape::Triangle(_) => ShapeKind::Triangle,
            Shape::Polygon(_) => ShapeKind::Polygon,
            Shape::Ellipse { .. } => ShapeKind::Ellipse,
            Shape::Capsule { .. } => ShapeKind::Capsule,
        }
    }

    /// A representative center: the true center for round shapes and boxes, the
    /// vertex mean for polygons and the core's midpoint for capsules.
    pub fn center(&self) -> Vec2 {
        match *self {
            Shape::Circle { c, .. } | Shape::Obb { c, .. } | Shape::Ellipse { c, .. } => c,
            Shape::Aabb { min, max } => (min + max) * 0.5,
            Shape::Triangle(p) | Shape::Polygon(p) => p.centroid(),
            Shape::Capsule { p, q, .. } => (p + q) * 0.5,
        }
    }

    /// The point of the shape furthest along `d` (which need not be unit length).
    pub fn support(&self, d: Vec2) -> Vec2 {
        match *self {
            Shape::Circle { c, r } => c + d.normalized() * r,
            Shape::Aabb { min, max } => Vec2::new(
                if d.x >= 0.0 { max.x } else { min.x },
                if d.y >= 0.0 { max.y } else { min.y },
            ),
            Shape::Obb { c, half, u } => {
                let v = u.perp();
                let sx = if d.dot(u) >= 0.0 { half.x } else { -half.x };
                let sy = if d.dot(v) >= 0.0 { half.y } else { -half.y };
                c + u * sx + v * sy
            }
            Shape::Triangle(p) | Shape::Polygon(p) => {
                let mut best = p.v[0];
                let mut best_dot = best.dot(d);
                for &v in &p.verts()[1..] {
                    let dot = v.dot(d);
                    if dot > best_dot {
                        best = v;
                        best_dot = dot;
                    }
                }
                best
            }
            Shape::Ellipse { c, a, b, u } => {
                let v = u.perp();
                let (dx, dy) = (d.dot(u) * a, d.dot(v) * b);
                let len = (dx * dx + dy * dy).sqrt();
                if len == 0.0 {
                    return c;
                }
                c + u * (a * dx / len) + v * (b * dy / len)
            }
            Shape::Capsule { p, q, r } => {
                let end = if d.dot(q - p) >= 0.0 { q } else { p };
                end + d.normalized() * r
            }
        }
    }

    /// Straight-edged shapes as a counter-clockwise polygon.
    pub fn polygon(&self) -> Option<Poly> {
        match *self {
            Shape::Aabb { min, max } => Some(Poly::new(&[
                min,
                Vec2::new(max.x, min.y),
                max,
                Vec2::new(min.x, max.y),
            ])),
            Shape::Obb { c, half, u } => {
                let (x, y) = (u * half.x, u.perp() * half.y);
                Some(Poly::new(&[c - x - y, c + x - y, c + x + y, c - x + y]))
            }
            Shape::Triangle(p) | Shape::Polygon(p) => Some(p),
            _ => None,
        }
    }

    /// Tight axis-aligned bounds as `(min, max)`.
    pub fn aabb(&self) -> (Vec2, Vec2) {
        match *self {
            Shape::Circle { c, r } => (c - Vec2::new(r, r), c + Vec2::new(r, r)),
            Shape::Aabb { min, max } => (min, max),
            Shape::Ellipse { c, a, b, u } => {
                let v = u.perp();
                let e = Vec2::new(
                    ((a * u.x).powi(2) + (b * v.x).powi(2)).sqrt(),
                    ((a * u.y).powi(2) + (b * v.y).powi(2)).sqrt(),
                );
                (c - e, c + e)
            }
            Shape::Capsule { p, q, r } => {
                let rr = Vec2::new(r, r);
                (
                    Vec2::new(p.x.min(q.x), p.y.min(q.y)) - rr,
                    Vec2::new(p.x.max(q.x), p.y.max(q.y)) + rr,
                )
            }
            _ => {
                let poly = self.polygon().expect("polygonal shape");
                poly.verts().iter().fold(
                    (Vec2::new(f32::MAX, f32::MAX), Vec2::new(f32::MIN, f32::MIN)),
                    |(lo, hi), v| {
                        (
                            Vec2::new(lo.x.min(v.x), lo.y.min(v.y)),
                            Vec2::new(hi.x.max(v.x), hi.y.max(v.y)),
                        )
                    },
                )
            }
        }
    }

    /// A bounding circle as `(center, radius)`; tight for round shapes,
    /// centroid-based for polygons.
    pub fn bounding_circle(&self) -> (Vec2, f32) {
        match *self {
            Shape::Circle { c, r } => (c, r),
            Shape::Ellipse { c, a, b, .. } => (c, a.max(b)),
            Shape::Capsule { p, q, r } => ((p + q) * 0.5, (q - p).length() * 0.5 + r),
            Shape::Obb { c, half, .. } => (c, half.length()),
            _ => {
                let poly = self.polygon().expect("polygonal shape");
                let c = self.center();
                let r2 = poly
                    .verts()
                    .iter()
                    .map(|&v| (v - c).length_squared())
                    .fold(0.0, f32::max);
                (c, r2.sqrt())
            }
        }
    }

    /// The boundary as a closed polyline, for drawing.
    pub fn outline(&self) -> Vec<Vec2> {
        const SEGMENTS: usize = 64;
        let arc = |c: Vec2, r: f32, from: f32, n: usize| {
            (0..=n).map(move |i| {
                let t = from + std::f32::consts::PI * i as f32 / n as f32;
                c + Vec2::new(t.cos(), t.sin()) * r
            })
        };
        match *self {
            Shape::Circle { c, r } => (0..SEGMENTS)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
                    c + Vec2::new(t.cos(), t.sin()) * r
                })
                .collect(),
            Shape::Ellipse { c, a, b, u } => {
                let v = u.perp();
                (0..SEGMENTS)
                    .map(|i| {
                        let t = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
                        c + u * (a * t.cos()) + v * (b * t.sin())
                    })
                    .collect()
            }
            Shape::Capsule { p, q, r } => {
                let d = q - p;
                let from = if d.length_squared() > 0.0 {
                    d.y.atan2(d.x) - std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
                let half = SEGMENTS / 2;
                arc(q, r, from, half)
                    .chain(arc(p, r, from + std::f32::consts::PI, half))
                    .collect()
            }
            _ => self.polygon().expect("polygonal shape").verts().to_vec(),
        }
    }
}

/// A shape with its bounding volumes, which a real engine would cache
/// alongside the body; broadphase tests read these instead of recomputing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The world-space shape the volumes bound.
    pub shape: Shape,
    /// Cached [`Shape::bounding_circle`] as `(center, radius)`.
    pub circle: (Vec2, f32),
    /// Cached [`Shape::aabb`] as `(min, max)`.
    pub aabb: (Vec2, Vec2),
}

impl Body {
    /// Wraps `shape` and computes its bounding volumes.
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            circle: shape.bounding_circle(),
            aabb: shape.aabb(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_is_counter_clockwise_and_drops_interior_points() {
        let pts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(2.0, 0.0),
        ];
        let hull = Poly::hull(&pts);
        assert_eq!(hull.len(), 4);
        let area: f32 = (0..hull.len())
            .map(|i| {
                let (a, b) = hull.edge(i);
                a.cross(b)
            })
            .sum();
        assert!(area > 0.0);
    }

    #[test]
    fn ellipse_support_lies_on_boundary() {
        let e = Shape::Ellipse {
            c: Vec2::new(1.0, 2.0),
            a: 3.0,
            b: 1.0,
            u: Vec2::new(0.6, 0.8),
        };
        for i in 0..16 {
            let t = i as f32 * 0.4;
            let s = e.support(Vec2::new(t.cos(), t.sin())) - Vec2::new(1.0, 2.0);
            let (x, y) = (s.dot(Vec2::new(0.6, 0.8)), s.dot(Vec2::new(-0.8, 0.6)));
            assert!(((x / 3.0).powi(2) + y.powi(2) - 1.0).abs() < 1e-4);
        }
    }
}
