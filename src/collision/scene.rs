//! A crowd of drifting bodies around shapes A and B, and the quadtree stage
//! that decides whether A and B are worth testing at all.

use super::quadtree::{Aabb, Quadtree};
use super::report::{Note, Tone};
use super::shape::{Body, Shape, ShapeKind};
use crate::rand::frand01;
use crate::vec2::Vec2;

/// Small random shapes drifting inside a rectangle centered on the origin.
#[derive(Default)]
pub struct Crowd {
    pub bodies: Vec<Body>,
    velocities: Vec<Vec2>,
    /// Half extents of the area the crowd bounces around in.
    pub half: Vec2,
}

impl Crowd {
    pub fn new(count: usize, half: Vec2, mut seed: u32) -> Self {
        let mut r = |lo: f32, hi: f32| lo + (hi - lo) * frand01(&mut seed);
        let mut crowd = Self {
            half,
            ..Self::default()
        };
        for _ in 0..count {
            let kind = ShapeKind::ALL[(r(0.0, 7.0) as usize).min(6)];
            let (x, y) = (r(-half.x, half.x), r(-half.y, half.y));
            let s = r(8.0, 24.0);
            let mut j = || r(-s, s);
            let params: Vec<f32> = match kind {
                ShapeKind::Circle => vec![x, y, s],
                ShapeKind::Aabb => vec![x, y, s, s * 0.7],
                ShapeKind::Obb | ShapeKind::Ellipse => vec![x, y, s, s * 0.6, s],
                ShapeKind::Triangle => vec![x + j(), y + j(), x + j(), y + j(), x + j(), y + j()],
                ShapeKind::Polygon => (0..10)
                    .map(|i| if i % 2 == 0 { x + j() } else { y + j() })
                    .collect(),
                ShapeKind::Capsule => vec![x - s, y + j() * 0.5, x + s, y + j() * 0.5, s * 0.4],
            };
            let shape = Shape::from_params(kind, &params).expect("valid crowd shape");
            crowd.bodies.push(Body::new(shape));
            let angle = r(0.0, std::f32::consts::TAU);
            crowd
                .velocities
                .push(Vec2::new(angle.cos(), angle.sin()) * r(8.0, 30.0));
        }
        crowd
    }

    /// Moves every body by `dt` seconds, bouncing off the area's edges.
    pub fn step(&mut self, dt: f32) {
        for (body, v) in self.bodies.iter_mut().zip(&mut self.velocities) {
            let c = body.shape.center();
            if (c.x < -self.half.x && v.x < 0.0) || (c.x > self.half.x && v.x > 0.0) {
                v.x = -v.x;
            }
            if (c.y < -self.half.y && v.y < 0.0) || (c.y > self.half.y && v.y > 0.0) {
                v.y = -v.y;
            }
            *body = Body::new(body.shape.translated(*v * dt));
        }
    }
}

/// Scene-level numbers for the quadtree stage.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneStats {
    pub bodies: u32,
    pub nodes: u32,
    pub depth: u32,
    /// In-leaf pair checks, before duplicates are removed.
    pub checks: u32,
    /// Unique candidate pairs the narrowphase would see.
    pub candidates: u32,
    /// What brute force would test: n(n − 1)/2.
    pub all_pairs: u32,
    /// Candidate pairs involving A or B.
    pub partners: u32,
}

/// Boxes of A, B and the crowd (in that order) and the root cell covering them all.
pub fn boxes(a: &Body, b: &Body, crowd: &Crowd) -> (Vec<Aabb>, Aabb) {
    let boxes: Vec<Aabb> = [a, b]
        .into_iter()
        .chain(&crowd.bodies)
        .map(|body| body.aabb)
        .collect();
    let root = boxes
        .iter()
        .fold((-crowd.half, crowd.half), |(lo, hi), (bl, bh)| {
            (
                Vec2::new(lo.x.min(bl.x), lo.y.min(bl.y)),
                Vec2::new(hi.x.max(bh.x), hi.y.max(bh.y)),
            )
        });
    (boxes, root)
}

/// Builds the quadtree over the whole scene. Returns whether A and B share
/// a leaf (and so must be tested), the stats, and notes for drawing: the leaf
/// cells (amber where A and B share one, verdigris where only one sits), and
/// the crowd bodies they pair with.
pub fn quadtree_stage(
    a: &Body,
    b: &Body,
    crowd: &Crowd,
    notes: &mut Vec<Note>,
) -> (bool, SceneStats) {
    let (boxes, root) = boxes(a, b, crowd);
    let tree = Quadtree::build(root, &boxes);
    let (pairs, checks) = tree.pairs();
    let n = boxes.len() as u32;
    let shared = pairs.binary_search(&(0, 1)).is_ok();
    let partners: Vec<(u32, u32)> = pairs.iter().copied().filter(|&(i, _)| i < 2).collect();

    // Leaves only: inner cells are drawn by their children's edges anyway.
    for leaf in tree.leaves() {
        let (lo, hi) = leaf.bounds;
        let (has_a, has_b) = (leaf.items.contains(&0), leaf.items.contains(&1));
        // Leaves holding both make A–B a candidate pair; those holding one show where each sits.
        let tone = match (has_a, has_b) {
            (true, true) => Tone::Amber,
            (true, false) | (false, true) => Tone::Verdigris,
            _ => Tone::Faint,
        };
        notes.push(Note::Polygon {
            points: vec![lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)],
            tone,
            dashed: false,
            fill: has_a || has_b,
        });
    }
    // Outline the crowd bodies A or B would still be tested against; the rest stay ghosted.
    let mut partner_bodies: Vec<u32> = partners
        .iter()
        .map(|&(_, j)| j)
        .filter(|&j| j >= 2)
        .collect();
    partner_bodies.sort_unstable();
    partner_bodies.dedup();
    for j in partner_bodies {
        notes.push(Note::Polygon {
            points: crowd.bodies[j as usize - 2].shape.outline(),
            tone: Tone::Verdigris,
            dashed: false,
            fill: false,
        });
    }

    let stats = SceneStats {
        bodies: n,
        nodes: tree.nodes.len() as u32,
        depth: tree.depth(),
        checks,
        candidates: pairs.len() as u32,
        all_pairs: n * n.saturating_sub(1) / 2,
        partners: partners.len() as u32,
    };
    (shared, stats)
}
