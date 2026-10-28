//! Path planning over the known map.
//!
//! The robot may only drive through cells it has seen free, with enough
//! room around them for its body: a clearance field holds each cell's
//! distance to the nearest known wall. Plans come from Dijkstra's algorithm,
//! which finds the nearest of many goal cells by travel distance in one run
//! (the 2013 lab took the Manhattan distance to the nearest frontier,
//! ignoring walls), with a penalty that keeps paths off the walls.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use crate::geom::Point;
use crate::grid::{Cell, Grid, neighbors4, neighbors8, trace};
use crate::map::Known;

/// Distance in meters from every cell to the nearest known wall cell's
/// edge (8-neighbor chamfer distance), capped at `cap` (at least 0).
pub fn clearance(map: &Grid<Known>, cap: f32) -> Grid<f32> {
    let cap = cap.max(0.0);
    let (w, h) = (map.width as usize, map.height as usize);
    let mut d: Vec<f32> = map
        .data
        .iter()
        .map(|&k| if k == Known::Wall { 0.0 } else { f32::INFINITY })
        .collect();
    const DIAG: f32 = std::f32::consts::SQRT_2;
    // Forward pass from the left and upper neighbors, then backward from
    // the right and lower ones.
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 {
                v = v.min(d[i - 1] + 1.0);
            }
            if y > 0 {
                v = v.min(d[i - w] + 1.0);
                if x > 0 {
                    v = v.min(d[i - w - 1] + DIAG);
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1] + DIAG);
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w {
                v = v.min(d[i + 1] + 1.0);
            }
            if y + 1 < h {
                v = v.min(d[i + w] + 1.0);
                if x + 1 < w {
                    v = v.min(d[i + w + 1] + DIAG);
                }
                if x > 0 {
                    v = v.min(d[i + w - 1] + DIAG);
                }
            }
            d[i] = v;
        }
    }
    // Cell-center distances in cells, to meters from the wall cell's edge.
    for v in &mut d {
        *v = ((*v - 0.5) * map.cell).clamp(0.0, cap);
    }
    Grid {
        width: map.width,
        height: map.height,
        cell: map.cell,
        data: d,
    }
}

/// How much room the robot needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    /// Clearance below which a cell can't be entered: the body radius.
    pub hard: f32,
    /// Clearance below which a cell costs extra, growing towards `hard`.
    pub soft: f32,
}

impl Room {
    /// Whether the robot may enter `c`: seen free, with at least `hard` clearance.
    pub fn passable(&self, map: &Grid<Known>, clear: &Grid<f32>, c: Cell) -> bool {
        map.get(c) == Some(&Known::Free) && clear.get(c).is_some_and(|&d| d >= self.hard)
    }

    /// Cost factor for driving through a cell with clearance `d`.
    fn penalty(&self, d: f32) -> f32 {
        if d >= self.soft {
            1.0
        } else {
            1.0 + 6.0 * (self.soft - d) / (self.soft - self.hard)
        }
    }
}

/// A planned route through the known map.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    /// Corner points, start first, after line-of-sight smoothing.
    pub points: Vec<Point>,
    /// The goal cell reached.
    pub goal: Cell,
    /// Travel cost in meters, penalties included.
    pub cost: f32,
}

/// A cell on Dijkstra's open list with its cost so far, ordered so the cheapest pops first from a
/// max-heap.
#[derive(PartialEq)]
struct Open(f32, Cell);

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0)
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Plans from `from` to the nearest cell where `goal` holds, through
/// passable cells. A robot already somewhere tighter than `room.hard` may
/// use cells as tight as where it is, so it can drive out; the penalty near
/// walls keeps it from staying in tight spots longer than it has to.
pub fn plan(
    map: &Grid<Known>,
    clear: &Grid<f32>,
    room: Room,
    from: Point,
    goal: impl Fn(Cell) -> bool,
) -> Option<Path> {
    let start = map.cell_of(from);
    if !map.contains(start) {
        return None;
    }
    let room = Room {
        hard: room.hard.min(clear.get(start).copied().unwrap_or(0.0)),
        ..room
    };
    let mut cost = map.like(f32::INFINITY);
    let mut came = map.like(None::<Cell>);
    let mut open = BinaryHeap::new();
    cost.set(start, 0.0);
    open.push(Open(0.0, start));
    while let Some(Open(g, c)) = open.pop() {
        if g > cost.get(c).copied().unwrap_or(f32::INFINITY) {
            continue;
        }
        if goal(c) && (c == start || room.passable(map, clear, c)) {
            return Some(Path {
                points: smooth(map, clear, room, from, &walk_back(&came, c)),
                goal: c,
                cost: g,
            });
        }
        for (n, step) in neighbors8(c) {
            if !room.passable(map, clear, n) {
                continue;
            }
            let ng = g + step * map.cell * room.penalty(clear.get(n).copied().unwrap_or(0.0));
            if ng < cost.get(n).copied().unwrap_or(f32::INFINITY) {
                cost.set(n, ng);
                came.set(n, Some(c));
                open.push(Open(ng, n));
            }
        }
    }
    None
}

/// The cells from the start to `c`, following each cell's predecessor in `came`.
fn walk_back(came: &Grid<Option<Cell>>, mut c: Cell) -> Vec<Cell> {
    let mut cells = vec![c];
    while let Some(&Some(p)) = came.get(c) {
        cells.push(p);
        c = p;
    }
    cells.reverse();
    cells
}

/// Drops path cells that can be skipped in a straight line. A shortcut must
/// keep as much room to the walls as the tightest cell of the stretch it
/// replaces, up to `room.soft`, so it never cuts closer to a wall than the
/// planned path did.
fn smooth(
    map: &Grid<Known>,
    clear: &Grid<f32>,
    room: Room,
    from: Point,
    cells: &[Cell],
) -> Vec<Point> {
    let room_at = |c: Cell| clear.get(c).copied().unwrap_or(0.0);
    let mut points = vec![from];
    let mut anchor = from;
    let mut i = 1;
    while i < cells.len() {
        // Furthest cell in sight of the anchor.
        let mut j = i;
        let mut tightest = room.soft.min(room_at(cells[i]));
        while j + 1 < cells.len() {
            let need = tightest.min(room_at(cells[j + 1]));
            if !in_sight(map, clear, need, anchor, map.center(cells[j + 1])) {
                break;
            }
            tightest = need;
            j += 1;
        }
        anchor = map.center(cells[j]);
        points.push(anchor);
        i = j + 1;
    }
    points
}

/// Whether the straight line `a`–`b` crosses only free cells with at least `need` m of room.
fn in_sight(map: &Grid<Known>, clear: &Grid<f32>, need: f32, a: Point, b: Point) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-6 {
        return true;
    }
    trace(map.cell, a, d * (1.0 / len), len, |c, _| {
        map.get(c) != Some(&Known::Free) || clear.get(c).is_none_or(|&v| v < need)
    })
    .is_none()
}

/// Lookouts: free cells with a clear line of sight to one of `targets`
/// (unknown cells) at most `reach` m long, found by casting rays out from
/// each target. A laser beam from a lookout towards its target has to enter
/// the target, so a scan there reveals it. Rays only leave within 60° of
/// the side the target is open to (its free neighbors): seen at a grazing
/// angle along a wall, a cell is too thin a target for the beams to hit.
/// Away from the target, a ray also keeps `margin` m from known walls, so a
/// robot standing a little off the lookout still sees past wall ends.
/// Each cell holds the index of the first target that sees it.
pub fn lookouts(
    map: &Grid<Known>,
    clear: &Grid<f32>,
    targets: &[Cell],
    reach: f32,
    margin: f32,
) -> Grid<Option<u32>> {
    const RAYS: usize = 64;
    let mut out = map.like(None);
    for (i, &t) in targets.iter().enumerate() {
        let from = map.center(t);
        let open: Vec<Point> = neighbors4(t)
            .into_iter()
            .filter(|&n| map.get(n) == Some(&Known::Free))
            .map(|n| Point::new((n.0 - t.0) as f32, (n.1 - t.1) as f32))
            .collect();
        for k in 0..RAYS {
            let dir = Point::from_angle(std::f32::consts::TAU * k as f32 / RAYS as f32);
            if open.iter().all(|n| n.dot(dir) < 0.5) {
                continue;
            }
            trace(map.cell, from, dir, reach, |c, d| {
                if c == t {
                    return false;
                }
                let tight = d > 0.3 && clear.get(c).is_none_or(|&v| v < margin);
                if map.get(c) != Some(&Known::Free) || tight {
                    return true;
                }
                if out.get(c) == Some(&None) {
                    out.set(c, Some(i as u32));
                }
                false
            });
        }
    }
    out
}

/// Cells within `reach` m of a `seed` cell, walking through free cells.
pub fn near(map: &Grid<Known>, seeds: impl IntoIterator<Item = Cell>, reach: f32) -> Grid<bool> {
    let mut dist = map.like(f32::INFINITY);
    let mut queue = VecDeque::new();
    for s in seeds {
        if map.contains(s) {
            dist.set(s, 0.0);
            queue.push_back(s);
        }
    }
    let steps = reach / map.cell;
    while let Some(c) = queue.pop_front() {
        let dc = dist.get(c).copied().unwrap_or(f32::INFINITY);
        for (n, step) in neighbors8(c) {
            if map.get(n) == Some(&Known::Free)
                && dc + step <= steps
                && dc + step < dist.get(n).copied().unwrap_or(0.0)
            {
                dist.set(n, dc + step);
                queue.push_back(n);
            }
        }
    }
    Grid {
        width: dist.width,
        height: dist.height,
        cell: dist.cell,
        data: dist.data.iter().map(|d| d.is_finite()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 × 2 m, all free, with a wall across x = 2 m except a gap at the bottom.
    fn room_with_door() -> Grid<Known> {
        let mut g = Grid::new(80, 40, 0.05, Known::Free);
        for y in 0..28 {
            g.set((40, y), Known::Wall);
        }
        for x in 0..80 {
            g.set((x, 0), Known::Wall);
            g.set((x, 39), Known::Wall);
        }
        g
    }

    const ROOM: Room = Room {
        hard: 0.2,
        soft: 0.4,
    };

    #[test]
    fn clearance_measures_from_the_wall_edge() {
        let g = room_with_door();
        let d = clearance(&g, 5.0);
        assert_eq!(d.get((40, 10)), Some(&0.0));
        // Cell center 0.5 cells from its neighbor's edge, then 1 cell per cell.
        assert!((d.get((43, 10)).unwrap() - 0.125).abs() < 1e-5);
    }

    #[test]
    fn the_path_goes_through_the_door_and_keeps_its_distance() {
        let g = room_with_door();
        let d = clearance(&g, 5.0);
        let from = Point::new(0.5, 0.5);
        let target = g.cell_of(Point::new(3.5, 0.5));
        let path = plan(&g, &d, ROOM, from, |c| c == target).expect("a way through the door");
        assert_eq!(path.goal, target);
        // Through the gap below y = 1.4 m, which is 0.6 m tall.
        assert!(path.points.iter().any(|p| p.y > 1.4), "{:?}", path.points);
        assert!(path.points.len() < 10, "smoothed: {:?}", path.points);
        for p in &path.points[1..] {
            assert!(*d.get(g.cell_of(*p)).unwrap() >= ROOM.hard);
        }
    }

    #[test]
    fn a_door_narrower_than_the_robot_blocks_the_way() {
        let mut g = room_with_door();
        for y in 28..36 {
            g.set((40, y), Known::Wall);
        }
        let d = clearance(&g, 5.0);
        let target = g.cell_of(Point::new(3.5, 0.5));
        assert!(plan(&g, &d, ROOM, Point::new(0.5, 0.5), |c| c == target).is_none());
    }

    #[test]
    fn lookouts_need_a_clear_line_of_sight() {
        let g = room_with_door();
        // An unknown cell left of the wall, at the top.
        let mut g2 = g.clone();
        g2.set((30, 5), Known::Unknown);
        let seen = lookouts(&g2, &clearance(&g2, 5.0), &[(30, 5)], 1.0, 0.15);
        assert_eq!(seen.get((30, 15)), Some(&Some(0)));
        assert_eq!(seen.get((27, 8)), Some(&Some(0)));
        // Behind the wall, or out of reach: no.
        assert_eq!(seen.get((45, 5)), Some(&None));
        assert_eq!(seen.get((30, 30)), Some(&None));
        // A face of unknown cells along the north wall only looks out at the
        // room, not along itself.
        let mut g3 = g.clone();
        for x in 10..31 {
            g3.set((x, 1), Known::Unknown);
        }
        let seen = lookouts(&g3, &clearance(&g3, 5.0), &[(20, 1)], 1.0, 0.15);
        assert_eq!(seen.get((20, 10)), Some(&Some(0)));
        assert_eq!(seen.get((12, 2)), Some(&None), "grazing along the face");
        // A line of sight scraping past the end of the wall at x = 2 m doesn't count.
        let mut g4 = g.clone();
        g4.set((45, 31), Known::Unknown);
        let seen = lookouts(&g4, &clearance(&g4, 5.0), &[(45, 31)], 2.0, 0.15);
        assert_eq!(seen.get((44, 40 - 12)), Some(&Some(0)));
        assert_eq!(seen.get((35, 25)), Some(&None), "scrapes the wall end");
    }

    #[test]
    fn near_spreads_through_free_cells_only() {
        let g = room_with_door();
        let m = near(&g, [(39, 10)], 0.3);
        assert_eq!(m.get((35, 10)), Some(&true));
        assert_eq!(m.get((41, 10)), Some(&false), "the wall is in the way");
    }
}
