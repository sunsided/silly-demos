//! Frontiers: the edges between seen free space and the unknown.
//!
//! Like the 2013 lab, the search flood-fills the free space reachable from
//! the robot, so frontiers behind walls it has seen don't count. Frontier
//! cells (free cells with an unknown edge neighbor) are then grouped into
//! clusters of touching cells (Yamauchi, 1997).
//!
//! Unknown cells right next to a known wall don't make a frontier: they are
//! mostly the far side of a wall, which the laser only sees end-on at the
//! wall's end, and unknown space worth exploring always reaches further.

use std::collections::VecDeque;

use crate::geom::Point;
use crate::grid::{Cell, Grid, neighbors4, neighbors8};
use crate::map::Known;

/// A group of touching frontier cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Its frontier cells: free cells with an unknown edge neighbor not beside a known wall.
    pub cells: Vec<Cell>,
    /// The mean of its cells' centers, in meters.
    pub centroid: Point,
    /// The unknown cells it borders: what a scan has to reach to push it back.
    pub unknown: Vec<Cell>,
}

/// Frontier clusters reachable from `from` with at least `min_cells` cells.
pub fn find(map: &Grid<Known>, from: Cell, min_cells: usize) -> Vec<Cluster> {
    let mut frontier = map.like(false);
    let mut seen = map.like(false);
    let mut queue = VecDeque::new();
    if map.get(from) == Some(&Known::Free) {
        seen.set(from, true);
        queue.push_back(from);
    }
    let mut cells = Vec::new();
    while let Some(c) = queue.pop_front() {
        for n in neighbors4(c) {
            match map.get(n) {
                Some(Known::Unknown) if !beside_wall(map, n) => {
                    if frontier.get(c) == Some(&false) {
                        frontier.set(c, true);
                        cells.push(c);
                    }
                }
                Some(Known::Free) if seen.get(n) == Some(&false) => {
                    seen.set(n, true);
                    queue.push_back(n);
                }
                _ => {}
            }
        }
    }

    let mut clusters = Vec::new();
    let mut grouped = map.like(false);
    let mut listed = map.like(false);
    for start in cells {
        if grouped.get(start) == Some(&true) {
            continue;
        }
        let mut cells = Vec::new();
        grouped.set(start, true);
        queue.push_back(start);
        while let Some(c) = queue.pop_front() {
            cells.push(c);
            for (n, _) in neighbors8(c) {
                if frontier.get(n) == Some(&true) && grouped.get(n) == Some(&false) {
                    grouped.set(n, true);
                    queue.push_back(n);
                }
            }
        }
        if cells.len() >= min_cells {
            let sum = cells
                .iter()
                .fold(Point::default(), |s, &c| s + map.center(c));
            let centroid = sum * (1.0 / cells.len() as f32);
            let mut unknown = Vec::new();
            for &c in &cells {
                for n in neighbors4(c) {
                    if map.get(n) == Some(&Known::Unknown)
                        && !beside_wall(map, n)
                        && listed.get(n) == Some(&false)
                    {
                        listed.set(n, true);
                        unknown.push(n);
                    }
                }
            }
            clusters.push(Cluster {
                cells,
                centroid,
                unknown,
            });
        }
    }
    clusters
}

/// Whether `c` has a known wall among its edge neighbors.
fn beside_wall(map: &Grid<Known>, c: Cell) -> bool {
    neighbors4(c)
        .iter()
        .any(|&n| map.get(n) == Some(&Known::Wall))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10 × 10 map: free on the left half, unknown on the right.
    fn half_seen() -> Grid<Known> {
        let mut g = Grid::new(10, 10, 0.1, Known::Unknown);
        for y in 0..10 {
            for x in 0..5 {
                g.set((x, y), Known::Free);
            }
        }
        g
    }

    #[test]
    fn the_edge_of_the_seen_area_is_one_frontier() {
        let clusters = find(&half_seen(), (1, 1), 3);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].cells.len(), 10);
        assert!(clusters[0].cells.iter().all(|c| c.0 == 4));
        assert_eq!(clusters[0].unknown.len(), 10);
        assert!(clusters[0].unknown.iter().all(|c| c.0 == 5));
    }

    #[test]
    fn frontiers_behind_a_wall_are_not_reachable() {
        let mut g = half_seen();
        for y in 0..10 {
            g.set((2, y), Known::Wall);
        }
        assert!(find(&g, (0, 0), 1).is_empty());
        assert_eq!(find(&g, (3, 3), 1).len(), 1);
    }

    #[test]
    fn the_hidden_back_of_a_wall_is_no_frontier() {
        // Everything seen but the back row of a two-cell-thick wall.
        let mut g = Grid::new(10, 10, 0.1, Known::Free);
        for x in 2..8 {
            g.set((x, 5), Known::Wall);
            g.set((x, 6), Known::Unknown);
        }
        assert!(find(&g, (0, 0), 1).is_empty());
    }

    #[test]
    fn small_clusters_are_dropped() {
        let mut g = Grid::new(10, 10, 0.1, Known::Free);
        g.set((5, 5), Known::Unknown);
        assert!(find(&g, (0, 0), 5).is_empty());
        assert_eq!(find(&g, (0, 0), 4)[0].cells.len(), 4);
    }
}
