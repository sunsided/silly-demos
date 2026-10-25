//! A square-celled grid over the floor plan, and ray traversal through it.

use crate::geom::Point;

/// Cell coordinates: column, row.
pub type Cell = (i32, i32);

/// A `width` × `height` grid of `cell` m squares, with its origin at (0, 0).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid<T> {
    pub width: i32,
    pub height: i32,
    pub cell: f32,
    pub data: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn new(width: i32, height: i32, cell: f32, fill: T) -> Self {
        Self {
            width,
            height,
            cell,
            data: vec![fill; (width * height) as usize],
        }
    }

    /// A grid of the same shape filled with `fill`.
    pub fn like<U: Clone>(&self, fill: U) -> Grid<U> {
        Grid::new(self.width, self.height, self.cell, fill)
    }

    pub fn contains(&self, (x, y): Cell) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn index(&self, (x, y): Cell) -> usize {
        (y * self.width + x) as usize
    }

    pub fn get(&self, c: Cell) -> Option<&T> {
        self.contains(c).then(|| &self.data[self.index(c)])
    }

    /// Sets cell `c`; cells outside the grid are ignored.
    pub fn set(&mut self, c: Cell, value: T) {
        if self.contains(c) {
            let i = self.index(c);
            self.data[i] = value;
        }
    }

    /// The cell containing `p`; may lie outside the grid.
    pub fn cell_of(&self, p: Point) -> Cell {
        (
            (p.x / self.cell).floor() as i32,
            (p.y / self.cell).floor() as i32,
        )
    }

    pub fn center(&self, (x, y): Cell) -> Point {
        Point::new((x as f32 + 0.5) * self.cell, (y as f32 + 0.5) * self.cell)
    }

    /// Cells with an index, row by row.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| (x, y)))
    }
}

/// The four edge neighbours of `c`.
pub fn neighbours4((x, y): Cell) -> [Cell; 4] {
    [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
}

/// The eight neighbours of `c`, with the length of the step to each in cells.
pub fn neighbours8((x, y): Cell) -> [(Cell, f32); 8] {
    const D: f32 = std::f32::consts::SQRT_2;
    [
        ((x + 1, y), 1.0),
        ((x - 1, y), 1.0),
        ((x, y + 1), 1.0),
        ((x, y - 1), 1.0),
        ((x + 1, y + 1), D),
        ((x - 1, y + 1), D),
        ((x + 1, y - 1), D),
        ((x - 1, y - 1), D),
    ]
}

/// Walks the cells of size `cell` that a ray from `from` along the unit
/// vector `dir` passes through, up to `max` m (Amanatides & Woo's grid
/// traversal). `visit` gets each cell and the distance at which the ray
/// enters it, and stops the walk by returning true. Returns that distance,
/// or `None` if the walk ran out.
pub fn trace(
    cell: f32,
    from: Point,
    dir: Point,
    max: f32,
    mut visit: impl FnMut(Cell, f32) -> bool,
) -> Option<f32> {
    let mut c = (
        (from.x / cell).floor() as i32,
        (from.y / cell).floor() as i32,
    );
    let step = |d: f32| if d > 0.0 { 1 } else { -1 };
    let (sx, sy) = (step(dir.x), step(dir.y));
    // Distance along the ray to the next vertical and horizontal cell edge,
    // and how far apart those edges are along the ray.
    let edge = |p: f32, d: f32, i: i32, s: i32| {
        if d == 0.0 {
            f32::INFINITY
        } else {
            let next = (i + if s > 0 { 1 } else { 0 }) as f32 * cell;
            (next - p) / d
        }
    };
    let mut tx = edge(from.x, dir.x, c.0, sx);
    let mut ty = edge(from.y, dir.y, c.1, sy);
    let dx = if dir.x == 0.0 {
        f32::INFINITY
    } else {
        cell / dir.x.abs()
    };
    let dy = if dir.y == 0.0 {
        f32::INFINITY
    } else {
        cell / dir.y.abs()
    };
    let mut t = 0.0;
    while t <= max {
        if visit(c, t) {
            return Some(t);
        }
        if tx < ty {
            t = tx;
            tx += dx;
            c.0 += sx;
        } else {
            t = ty;
            ty += dy;
            c.1 += sy;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_visits_cells_in_order_with_entry_distances() {
        let mut seen = Vec::new();
        trace(
            1.0,
            Point::new(0.5, 0.5),
            Point::new(1.0, 0.0),
            3.0,
            |c, t| {
                seen.push((c, t));
                false
            },
        );
        assert_eq!(
            seen,
            vec![((0, 0), 0.0), ((1, 0), 0.5), ((2, 0), 1.5), ((3, 0), 2.5)]
        );
    }

    #[test]
    fn trace_stops_where_visit_says() {
        let dir = Point::from_angle(std::f32::consts::FRAC_PI_4);
        let hit = trace(0.5, Point::new(0.1, 0.1), dir, 10.0, |c, _| c == (2, 2));
        let t = hit.expect("reaches (2, 2)");
        // Enters cell (2, 2) at x = y = 1.0, 0.9 m along each axis.
        assert!((t - 0.9 * std::f32::consts::SQRT_2).abs() < 1e-4, "{t}");
    }
}
