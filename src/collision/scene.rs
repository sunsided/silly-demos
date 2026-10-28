//! A crowd of drifting bodies around shapes A and B, and the quadtree stage
//! that decides whether A and B are worth testing at all.

use super::methods::gjk::gjk_boolean;
use super::probe::Silent;
use super::quadtree::{Aabb, Quadtree, overlaps};
use super::report::{Note, Tone};
use super::shape::{Body, Shape, ShapeKind};
use crate::rand::frand01;
use crate::vec2::Vec2;

/// How far the quadtree's root reaches past the area the crowd bounces in.
const ROOT_MARGIN: f32 = 12.0;

/// Small random shapes drifting inside a rectangle centered on the origin.
#[derive(Default)]
pub struct Crowd {
    /// The drifting bodies; their bounding volumes are refreshed every step.
    pub bodies: Vec<Body>,
    /// Velocity of each body in px per second, parallel to `bodies`.
    velocities: Vec<Vec2>,
    /// Half extents of the area the crowd bounces around in.
    pub half: Vec2,
}

impl Crowd {
    /// `count` small random shapes scattered over the area, each with a random
    /// heading and speed; the same `seed` gives the same crowd.
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
    /// Bodies in the tree: A, B and the crowd.
    pub bodies: u32,
    /// Quadtree nodes, inner and leaf.
    pub nodes: u32,
    /// Level of the deepest node below the root.
    pub depth: u32,
    /// In-leaf pair checks, before duplicates are removed.
    pub checks: u32,
    /// Unique candidate pairs the narrowphase would see.
    pub candidates: u32,
    /// What brute force would test: n(n − 1)/2.
    pub all_pairs: u32,
    /// Candidate pairs involving A or B.
    pub partners: u32,
    /// Candidates whose bounding boxes really overlap.
    pub box_overlaps: u32,
    /// Candidates whose shapes really collide (GJK).
    pub collisions: u32,
}

/// Boxes of A, B and the crowd (in that order) and the root cell covering them all.
pub fn boxes(a: &Body, b: &Body, crowd: &Crowd) -> (Vec<Aabb>, Aabb) {
    let boxes: Vec<Aabb> = [a, b]
        .into_iter()
        .chain(&crowd.bodies)
        .map(|body| body.aabb)
        .collect();
    // A fixed root keeps the cells still while things move; bodies poking
    // out are clamped into the border cells by the tree.
    let margin = Vec2::new(ROOT_MARGIN, ROOT_MARGIN);
    (boxes, (-crowd.half - margin, crowd.half + margin))
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

    // How much of the candidate list is real work: boxes that overlap, and shapes that touch.
    let body = |i: u32| match i {
        0 => a,
        1 => b,
        _ => &crowd.bodies[i as usize - 2],
    };
    let mut box_overlaps = 0;
    let mut collisions = 0;
    for &(i, j) in &pairs {
        if overlaps(&boxes[i as usize], &boxes[j as usize]) {
            box_overlaps += 1;
            collisions += gjk_boolean(&body(i).shape, &body(j).shape, &mut Silent).intersect as u32;
        }
    }

    let stats = SceneStats {
        bodies: n,
        nodes: tree.nodes.len() as u32,
        depth: tree.depth(),
        checks,
        candidates: pairs.len() as u32,
        all_pairs: n * n.saturating_sub(1) / 2,
        partners: partners.len() as u32,
        box_overlaps,
        collisions,
    };
    (shared, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quadtree must not hide a real collision: the same number of
    /// colliding pairs as testing every pair directly.
    #[test]
    fn candidates_find_every_collision() {
        let half = Vec2::new(400.0, 300.0);
        let crowd = Crowd::new(250, half, 7);
        let circle = |x| {
            Body::new(Shape::Circle {
                c: Vec2::new(x, 0.0),
                r: 80.0,
            })
        };
        let (a, b) = (circle(-60.0), circle(60.0));
        let (_, stats) = quadtree_stage(&a, &b, &crowd, &mut Vec::new());

        let bodies: Vec<&Body> = [&a, &b].into_iter().chain(&crowd.bodies).collect();
        let mut brute = 0;
        for (i, x) in bodies.iter().enumerate() {
            for y in &bodies[i + 1..] {
                brute += gjk_boolean(&x.shape, &y.shape, &mut Silent).intersect as u32;
            }
        }
        assert!(brute > 0, "the scene should contain some collisions");
        assert_eq!(stats.collisions, brute);
        assert!(stats.collisions <= stats.box_overlaps && stats.box_overlaps <= stats.candidates);
    }
}
