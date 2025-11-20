#![allow(dead_code)]

use crate::rand::{frand01, hash_u32};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct VoronoiTests;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pt {
    x: f32,
    y: f32,
}

impl Pt {
    #[inline]
    fn sub(self, o: Pt) -> Pt {
        Pt {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }

    #[inline]
    fn add(self, o: Pt) -> Pt {
        Pt {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }

    #[inline]
    fn mul(self, s: f32) -> Pt {
        Pt {
            x: self.x * s,
            y: self.y * s,
        }
    }

    #[inline]
    fn dot(self, o: Pt) -> f32 {
        self.x * o.x + self.y * o.y
    }

    #[inline]
    fn len2(self) -> f32 {
        self.dot(self)
    }
}

#[derive(Clone, Copy)]
struct Circumcircle {
    c: Pt,
    r2: f32,
}

fn circumcircle(a: Pt, b: Pt, c: Pt) -> Option<Circumcircle> {
    // Robust-ish circumcenter calculation; return None for near-collinear
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-6 {
        return None;
    }
    let a2 = a.x * a.x + a.y * a.y;
    let b2 = b.x * b.x + b.y * b.y;
    let c2 = c.x * c.x + c.y * c.y;
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = Pt { x: ux, y: uy };
    Some(Circumcircle {
        c: center,
        r2: center.sub(a).len2(),
    })
}

fn in_circumcircle(p: Pt, a: Pt, b: Pt, c: Pt) -> bool {
    if let Some(cc) = circumcircle(a, b, c) {
        p.sub(cc.c).len2() <= cc.r2
    } else {
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Edge {
    a: usize,
    b: usize,
}

impl Edge {
    fn new(i: usize, j: usize) -> Self {
        if i < j {
            Edge { a: i, b: j }
        } else {
            Edge { a: j, b: i }
        }
    }
}

#[derive(Clone, Copy)]
struct Tri {
    a: usize,
    b: usize,
    c: usize,
}

impl Tri {
    fn edges(&self) -> [Edge; 3] {
        [
            Edge::new(self.a, self.b),
            Edge::new(self.b, self.c),
            Edge::new(self.c, self.a),
        ]
    }
}

fn bowyer_watson(points: &[Pt]) -> Vec<Tri> {
    // Super triangle bounding all points
    let mut minx = f32::INFINITY;
    let mut miny = f32::INFINITY;
    let mut maxx = f32::NEG_INFINITY;
    let mut maxy = f32::NEG_INFINITY;
    for p in points {
        minx = minx.min(p.x);
        miny = miny.min(p.y);
        maxx = maxx.max(p.x);
        maxy = maxy.max(p.y);
    }

    let dx = maxx - minx;
    let dy = maxy - miny;
    let delta = (dx.max(dy) + 1.0) * 10.0;
    let midx = (minx + maxx) * 0.5;
    let midy = (miny + maxy) * 0.5;

    let st_a = Pt {
        x: midx - 2.0 * delta,
        y: midy - delta,
    };
    let st_b = Pt {
        x: midx,
        y: midy + 2.0 * delta,
    };
    let st_c = Pt {
        x: midx + 2.0 * delta,
        y: midy - delta,
    };

    let mut pts: Vec<Pt> = points.to_vec();
    let i_a = pts.len();
    let i_b = pts.len() + 1;
    let i_c = pts.len() + 2;
    pts.push(st_a);
    pts.push(st_b);
    pts.push(st_c);

    let mut tris = vec![Tri {
        a: i_a,
        b: i_b,
        c: i_c,
    }];

    for (pi, p) in points.iter().enumerate() {
        // 1. Collect triangles whose circumcircle contains p
        let mut bad: Vec<usize> = Vec::new();
        for (ti, t) in tris.iter().enumerate() {
            let a = pts[t.a];
            let b = pts[t.b];
            let c = pts[t.c];
            if in_circumcircle(*p, a, b, c) {
                bad.push(ti);
            }
        }
        // 2. Count edges to find boundary (edges seen once)
        use std::collections::HashMap;
        let mut edge_count: HashMap<Edge, u32> = HashMap::new();
        for &ti in &bad {
            for e in tris[ti].edges() {
                *edge_count.entry(e).or_insert(0) += 1;
            }
        }
        let mut boundary: Vec<Edge> = edge_count
            .into_iter()
            .filter_map(|(e, c)| if c == 1 { Some(e) } else { None })
            .collect();

        // 3. Remove bad triangles
        bad.sort_unstable();
        for (removed, idx) in bad.into_iter().enumerate() {
            tris.remove(idx - removed);
        }

        // 4. Re-triangulate the cavity
        for e in boundary.drain(..) {
            tris.push(Tri {
                a: e.a,
                b: e.b,
                c: pi,
            });
        }
    }

    // 5. Remove triangles that include super triangle vertices
    tris.retain(|t| {
        t.a != i_a
            && t.a != i_b
            && t.a != i_c
            && t.b != i_a
            && t.b != i_b
            && t.b != i_c
            && t.c != i_a
            && t.c != i_b
            && t.c != i_c
    });
    tris
}

fn compute_voronoi_edges(points: &[Pt], tris: &[Tri]) -> Vec<(Pt, Pt)> {
    // Map each edge to the first triangle index that saw it, and when seen again connect circumcenters
    use std::collections::HashMap;
    #[derive(Clone, Copy, Eq, PartialEq, Hash)]
    struct EdgeKey(usize, usize);
    impl EdgeKey {
        fn new(a: usize, b: usize) -> Self {
            if a < b { Self(a, b) } else { Self(b, a) }
        }
    }

    // Precompute circumcenters (fallback to centroid if degenerate)
    let mut centers: Vec<Pt> = Vec::with_capacity(tris.len());
    for t in tris {
        if let Some(cc) = circumcircle(points[t.a], points[t.b], points[t.c]) {
            centers.push(cc.c);
        } else {
            let cen = Pt {
                x: (points[t.a].x + points[t.b].x + points[t.c].x) / 3.0,
                y: (points[t.a].y + points[t.b].y + points[t.c].y) / 3.0,
            };
            centers.push(cen);
        }
    }

    let mut edge_map: HashMap<EdgeKey, usize> = HashMap::new();
    let mut segments: Vec<(Pt, Pt)> = Vec::new();
    for (ti, t) in tris.iter().enumerate() {
        let edges = [(t.a, t.b), (t.b, t.c), (t.c, t.a)];
        for &(i, j) in &edges {
            let key = EdgeKey::new(i, j);
            if let Some(other_ti) = edge_map.insert(key, ti) {
                // Edge shared by two triangles -> Voronoi edge between circumcenters
                segments.push((centers[ti], centers[other_ti]));
            }
        }
    }
    segments
}

#[wasm_bindgen]
impl VoronoiTests {
    /// Create seeded points with small velocities. Layout: [x,y,vx,vy,...]
    pub fn voronoi_create_points(
        count: usize,
        width: f32,
        height: f32,
        seed: u32,
        speed: f32,
    ) -> Vec<f32> {
        let mut s = if seed == 0 { 1 } else { seed };
        // We always append 4 fixed corner points with zero velocity at the end
        // of the buffer. The caller-provided `count` represents the number of
        // moving points. Total returned points = count (moving) + 4 (fixed).
        let mut out = Vec::with_capacity((count + 4) * 4);
        for i in 0..count {
            // Positions within bounds
            let rx = frand01(&mut s);
            let ry = frand01(&mut s);
            let x = rx * width;
            let y = ry * height;

            // Angle and speed
            let ang = frand01(&mut s) * std::f32::consts::TAU;
            let spd = (0.2 + 0.8 * frand01(&mut s)) * speed; // small random velocity around provided speed
            let vx = ang.cos() * spd;
            let vy = ang.sin() * spd;
            out.push(x);
            out.push(y);
            out.push(vx);
            out.push(vy);

            // decorrelate with index to keep determinism but variety
            s ^= (i as u32).wrapping_mul(0x9E37_79B9);
        }

        // Append four fixed corner points (vx = vy = 0). These should never be removed.
        // Order: (0,0), (width,0), (0,height), (width,height)
        let corners = [
            (0.0f32, 0.0f32),
            (width, 0.0f32),
            (0.0f32, height),
            (width, height),
        ];
        for &(x, y) in &corners {
            out.push(x);
            out.push(y);
            out.push(0.0);
            out.push(0.0);
        }
        out
    }

    /// Integrate positions; if a point leaves the viewport, respawn at a deterministic random location with small random velocity
    pub fn voronoi_step_points(points: &[f32], width: f32, height: f32, dt: f32) -> Vec<f32> {
        let mut out = points.to_vec();
        let n = points.len() / 4;
        // Treat the last 4 points (if present) as fixed corners.
        // This matches the buffers produced by voronoi_create_points.
        let fixed_corners = n >= 4;

        // Update moving points
        let moving_n = if fixed_corners { n - 4 } else { n };
        for i in 0..moving_n {
            let ix = i * 4;
            let mut x = points[ix];
            let mut y = points[ix + 1];
            let mut vx = points[ix + 2];
            let mut vy = points[ix + 3];
            x += vx * dt;
            y += vy * dt;
            let out_of_bounds = x < 0.0 || x > width || y < 0.0 || y > height;
            if out_of_bounds {
                // Deterministic respawn based on previous state
                let seed = x.to_bits()
                    ^ y.to_bits()
                    ^ vx.to_bits()
                    ^ vy.to_bits()
                    ^ (i as u32 * 0x85EB_CA6B);
                let mut s = hash_u32(seed);
                x = frand01(&mut s) * width;
                y = frand01(&mut s) * height;
                let ang = frand01(&mut s) * std::f32::consts::TAU;
                let spd = 10.0 + 40.0 * frand01(&mut s);
                vx = ang.cos() * spd;
                vy = ang.sin() * spd;
            }
            out[ix] = x;
            out[ix + 1] = y;
            out[ix + 2] = vx;
            out[ix + 3] = vy;
        }

        // Pin the last four as fixed corners with zero velocity, and update
        // their positions to match the current canvas size (handles resizes).
        if fixed_corners {
            let base = moving_n * 4;
            let corners = [
                (0.0f32, 0.0f32),
                (width, 0.0f32),
                (0.0f32, height),
                (width, height),
            ];
            for (k, &(x, y)) in corners.iter().enumerate() {
                let ix = base + k * 4;
                if ix + 3 < out.len() {
                    out[ix] = x;
                    out[ix + 1] = y;
                    out[ix + 2] = 0.0;
                    out[ix + 3] = 0.0;
                }
            }
        }
        out
    }

    /// Compute Delaunay triangulation indices (triplets)
    pub fn delaunay_indices(points_flat: &[f32]) -> Vec<u32> {
        let mut pts: Vec<Pt> = Vec::new();
        for i in (0..points_flat.len()).step_by(4) {
            if i + 1 < points_flat.len() {
                pts.push(Pt {
                    x: points_flat[i],
                    y: points_flat[i + 1],
                });
            }
        }

        if pts.len() < 3 {
            return Vec::new();
        }

        let tris = bowyer_watson(&pts);
        let mut out = Vec::with_capacity(tris.len() * 3);
        for t in tris {
            out.push(t.a as u32);
            out.push(t.b as u32);
            out.push(t.c as u32);
        }
        out
    }

    /// Compute Voronoi edges as line segments [x1,y1,x2,y2,...]
    pub fn voronoi_edges(points_flat: &[f32]) -> Vec<f32> {
        let mut pts: Vec<Pt> = Vec::new();
        for i in (0..points_flat.len()).step_by(4) {
            if i + 1 < points_flat.len() {
                pts.push(Pt {
                    x: points_flat[i],
                    y: points_flat[i + 1],
                });
            }
        }
        if pts.len() < 3 {
            return Vec::new();
        }

        let tris = bowyer_watson(&pts);
        let segs = compute_voronoi_edges(&pts, &tris);
        let mut out = Vec::with_capacity(segs.len() * 4);
        for (a, b) in segs {
            out.push(a.x);
            out.push(a.y);
            out.push(b.x);
            out.push(b.y);
        }
        out
    }
}
