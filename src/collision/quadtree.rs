//! Region quadtree over bounding boxes, for scene-wide pair pruning. A body
//! goes into every leaf its box overlaps; only bodies that share a leaf
//! become candidate pairs for the narrowphase.

use crate::vec2::Vec2;

/// A leaf splits once it holds more items than this...
pub const CAPACITY: usize = 4;
/// ...unless it is already this deep.
pub const MAX_DEPTH: u32 = 6;

/// An axis-aligned box as `(min, max)`.
pub type Aabb = (Vec2, Vec2);

pub fn overlaps(a: &Aabb, b: &Aabb) -> bool {
    a.0.x <= b.1.x && b.0.x <= a.1.x && a.0.y <= b.1.y && b.0.y <= a.1.y
}

pub struct Node {
    pub bounds: Aabb,
    pub depth: u32,
    /// Index of the first of four consecutive children, once split.
    pub children: Option<usize>,
    /// Items in this node; emptied when it splits.
    pub items: Vec<u32>,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

pub struct Quadtree {
    pub nodes: Vec<Node>,
    /// Box-against-quadrant overlap tests made while building.
    pub tests: u32,
}

impl Quadtree {
    /// Builds the tree over `boxes` inside the fixed `bounds`. Boxes reaching
    /// outside are clamped to it, so they land in the border cells; clamping
    /// keeps overlapping boxes overlapping, so no pair is missed.
    pub fn build(bounds: Aabb, boxes: &[Aabb]) -> Self {
        let clamp = |v: Vec2| {
            Vec2::new(
                v.x.clamp(bounds.0.x, bounds.1.x),
                v.y.clamp(bounds.0.y, bounds.1.y),
            )
        };
        let boxes: Vec<Aabb> = boxes
            .iter()
            .map(|&(lo, hi)| (clamp(lo), clamp(hi)))
            .collect();
        let mut tree = Self {
            nodes: vec![Node {
                bounds,
                depth: 0,
                children: None,
                items: (0..boxes.len() as u32).collect(),
            }],
            tests: 0,
        };
        tree.split(0, &boxes);
        tree
    }

    fn split(&mut self, i: usize, boxes: &[Aabb]) {
        let node = &self.nodes[i];
        if node.items.len() <= CAPACITY || node.depth >= MAX_DEPTH {
            return;
        }
        let ((lo, hi), depth) = (node.bounds, node.depth + 1);
        let mid = (lo + hi) * 0.5;
        let quadrants = [
            (lo, mid),
            (Vec2::new(mid.x, lo.y), Vec2::new(hi.x, mid.y)),
            (Vec2::new(lo.x, mid.y), Vec2::new(mid.x, hi.y)),
            (mid, hi),
        ];
        let items = std::mem::take(&mut self.nodes[i].items);
        let first = self.nodes.len();
        for q in quadrants {
            let inside = items
                .iter()
                .copied()
                .filter(|&it| overlaps(&boxes[it as usize], &q))
                .collect();
            self.tests += items.len() as u32;
            self.nodes.push(Node {
                bounds: q,
                depth,
                children: None,
                items: inside,
            });
        }
        self.nodes[i].children = Some(first);
        for child in first..first + 4 {
            self.split(child, boxes);
        }
    }

    pub fn leaves(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.is_leaf())
    }

    pub fn depth(&self) -> u32 {
        self.nodes.iter().map(|n| n.depth).max().unwrap_or(0)
    }

    /// Unique candidate pairs `(i, j)` with `i < j` that share a leaf, and the
    /// number of in-leaf pair checks made before removing duplicates.
    pub fn pairs(&self) -> (Vec<(u32, u32)>, u32) {
        let mut pairs = Vec::new();
        let mut checks = 0;
        for leaf in self.leaves() {
            for (k, &a) in leaf.items.iter().enumerate() {
                for &b in &leaf.items[k + 1..] {
                    checks += 1;
                    pairs.push((a.min(b), a.max(b)));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        (pairs, checks)
    }
}

/// The baseline: test every pair of boxes. Returns the overlapping pairs.
pub fn brute_force(boxes: &[Aabb]) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for (i, a) in boxes.iter().enumerate() {
        for (j, b) in boxes.iter().enumerate().skip(i + 1) {
            if overlaps(a, b) {
                pairs.push((i as u32, j as u32));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::frand01;

    #[test]
    fn candidates_cover_every_overlapping_pair() {
        let mut seed = 42;
        let mut r = |lo: f32, hi: f32| lo + (hi - lo) * frand01(&mut seed);
        let boxes: Vec<Aabb> = (0..300)
            .map(|_| {
                let c = Vec2::new(r(-400.0, 400.0), r(-300.0, 300.0));
                let h = Vec2::new(r(4.0, 40.0), r(4.0, 40.0));
                (c - h, c + h)
            })
            .collect();
        let bounds = (Vec2::new(-450.0, -350.0), Vec2::new(450.0, 350.0));
        let tree = Quadtree::build(bounds, &boxes);
        let (candidates, _) = tree.pairs();
        for pair in brute_force(&boxes) {
            assert!(candidates.binary_search(&pair).is_ok(), "{pair:?} missing");
        }
        // The point of the tree: far fewer candidates than all pairs.
        assert!(candidates.len() < 300 * 299 / 2 / 10);
    }

    #[test]
    fn boxes_outside_the_root_still_pair_up() {
        let root = (Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
        let b = |x0: f32, y0: f32, x1: f32, y1: f32| (Vec2::new(x0, y0), Vec2::new(x1, y1));
        // Two overlapping boxes entirely right of the root, plus filler to force splits.
        let mut boxes = vec![b(120.0, 40.0, 140.0, 60.0), b(130.0, 50.0, 150.0, 70.0)];
        boxes.extend((0..20).map(|i| {
            let x = (i % 5) as f32 * 20.0;
            let y = (i / 5) as f32 * 25.0;
            b(x, y, x + 5.0, y + 5.0)
        }));
        let (candidates, _) = Quadtree::build(root, &boxes).pairs();
        assert!(candidates.binary_search(&(0, 1)).is_ok());
    }
}
