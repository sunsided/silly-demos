//! The arrangement of a map's nodes: a chain, a ring, or a rectangular or
//! hexagonal grid.

/// Nodes a lattice may have; more would make every brute-force search and
/// the n² batch kernel slow in the browser.
pub const MAX_NODES: usize = 400;

/// The shape of a lattice, which sets its coordinates, distances and links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Nodes in a line, each linked to the one before and after.
    Chain,
    /// A chain whose ends are linked.
    Ring,
    /// A rectangular grid; neighbours include the diagonals.
    Grid,
    /// A hexagonal grid in offset rows: odd rows shift half a node right.
    Hex,
}

impl Kind {
    /// The kind for a UI id, `chain`, `ring`, `grid` or `hex`; `None` for anything else.
    pub fn parse(id: &str) -> Option<Kind> {
        match id {
            "chain" => Some(Kind::Chain),
            "ring" => Some(Kind::Ring),
            "grid" => Some(Kind::Grid),
            "hex" => Some(Kind::Hex),
            _ => None,
        }
    }

    /// Whether the lattice is a single row of nodes.
    fn is_1d(self) -> bool {
        matches!(self, Kind::Chain | Kind::Ring)
    }
}

/// Node `i` sits at column `i % width`, row `i / width`.
#[derive(Clone, Debug)]
pub struct Lattice {
    /// The shape of the lattice.
    kind: Kind,
    /// Nodes per row.
    width: usize,
    /// Rows; 1 for chains and rings.
    height: usize,
    /// Each node's position on the lattice, cached at construction.
    coords: Vec<(f32, f32)>,
}

/// The vertical distance between hex rows, so diagonal hex neighbors are 1 apart.
const ROW_SPACING_HEX: f32 = 0.866_025_4; // √3 / 2

impl Lattice {
    /// `None` unless the lattice has 2 to [`MAX_NODES`] nodes (a ring at
    /// least 3). Chains and rings ignore `height`.
    pub fn new(kind: Kind, width: usize, height: usize) -> Option<Lattice> {
        let height = if kind.is_1d() { 1 } else { height };
        let n = width.checked_mul(height)?;
        let min = if kind == Kind::Ring { 3 } else { 2 };
        if !(min..=MAX_NODES).contains(&n) {
            return None;
        }
        let coords = (0..n)
            .map(|i| {
                let (col, row) = ((i % width) as f32, (i / width) as f32);
                match kind {
                    Kind::Hex => (
                        col + if (i / width) % 2 == 1 { 0.5 } else { 0.0 },
                        row * ROW_SPACING_HEX,
                    ),
                    _ => (col, row),
                }
            })
            .collect();
        Some(Lattice {
            kind,
            width,
            height,
            coords,
        })
    }

    /// The shape of the lattice.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Nodes per row.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Rows; 1 for chains and rings.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.coords.len()
    }

    /// Whether there are no nodes; never true of a lattice from [`Lattice::new`].
    pub fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }

    /// Where node `i` sits on the lattice; hex rows are √3/2 apart, so
    /// neighbouring hex nodes are 1 apart like grid ones.
    pub fn coords(&self, i: usize) -> (f32, f32) {
        self.coords[i]
    }

    /// Squared lattice distance, the kernel's argument. A ring takes the
    /// shorter way round.
    pub fn dist2(&self, i: usize, j: usize) -> f32 {
        if self.kind == Kind::Ring {
            let d = i.abs_diff(j);
            let d = d.min(self.len() - d) as f32;
            return d * d;
        }
        let (a, b) = (self.coords[i], self.coords[j]);
        (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
    }

    /// Whether `i` and `j` are adjacent, for the topographic error. A grid
    /// counts its diagonals, so a diagonal step is not an error.
    pub fn neighbours(&self, i: usize, j: usize) -> bool {
        if i == j {
            return false;
        }
        match self.kind {
            Kind::Chain | Kind::Ring => self.dist2(i, j) == 1.0,
            Kind::Grid => {
                let (a, b) = (self.coords[i], self.coords[j]);
                (a.0 - b.0).abs().max((a.1 - b.1).abs()) == 1.0
            }
            Kind::Hex => self.dist2(i, j) < 1.0 + 1e-3,
        }
    }

    /// The links to draw, each once. Grids draw only their axis links;
    /// hex grids draw all six.
    pub fn edges(&self) -> Vec<(u32, u32)> {
        let n = self.len();
        match self.kind {
            Kind::Chain => (1..n).map(|i| (i as u32 - 1, i as u32)).collect(),
            Kind::Ring => (0..n).map(|i| (i as u32, ((i + 1) % n) as u32)).collect(),
            Kind::Grid => (0..n)
                .flat_map(|i| {
                    let right = (i % self.width + 1 < self.width).then_some(i + 1);
                    let down = (i + self.width < n).then_some(i + self.width);
                    right
                        .into_iter()
                        .chain(down)
                        .map(move |j| (i as u32, j as u32))
                })
                .collect(),
            Kind::Hex => (0..n)
                .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
                .filter(|&(i, j)| self.neighbours(i, j))
                .map(|(i, j)| (i as u32, j as u32))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_neighbours(l: &Lattice, i: usize) -> usize {
        (0..l.len())
            .filter(|&j| j != i && l.neighbours(i, j))
            .count()
    }

    #[test]
    fn ring_distance_wraps() {
        let l = Lattice::new(Kind::Ring, 10, 1).unwrap();
        assert_eq!(l.dist2(0, 9), 1.0);
        assert_eq!(l.dist2(0, 5), 25.0);
        assert!(l.neighbours(0, 9));
        let chain = Lattice::new(Kind::Chain, 10, 1).unwrap();
        assert_eq!(chain.dist2(0, 9), 81.0);
        assert!(!chain.neighbours(0, 9));
    }

    #[test]
    fn hex_interior_has_six_neighbours() {
        let l = Lattice::new(Kind::Hex, 7, 7).unwrap();
        let i = 3 * 7 + 3;
        assert_eq!(count_neighbours(&l, i), 6);
        for j in (0..l.len()).filter(|&j| j != i && l.neighbours(i, j)) {
            assert!((l.dist2(i, j) - 1.0).abs() < 1e-5, "{j}");
        }
    }

    #[test]
    fn grid_neighbours_include_diagonals() {
        let l = Lattice::new(Kind::Grid, 5, 5).unwrap();
        assert_eq!(count_neighbours(&l, 2 * 5 + 2), 8);
        assert_eq!(count_neighbours(&l, 0), 3);
    }

    #[test]
    fn edge_counts() {
        let edges = |k, w, h| Lattice::new(k, w, h).unwrap().edges().len();
        assert_eq!(edges(Kind::Chain, 10, 1), 9);
        assert_eq!(edges(Kind::Ring, 10, 1), 10);
        assert_eq!(edges(Kind::Grid, 4, 3), 4 * 2 + 3 * 3);
        let hex = Lattice::new(Kind::Hex, 3, 3).unwrap();
        let pairs = (0..hex.len())
            .flat_map(|i| (i + 1..hex.len()).map(move |j| (i, j)))
            .filter(|&(i, j)| hex.neighbours(i, j))
            .count();
        assert_eq!(hex.edges().len(), pairs);
    }

    #[test]
    fn sizes_are_bounded() {
        assert!(Lattice::new(Kind::Grid, 1, 1).is_none());
        assert!(Lattice::new(Kind::Grid, 21, 20).is_none());
        let chain = Lattice::new(Kind::Chain, 400, 7).unwrap();
        assert_eq!(chain.len(), 400);
        assert!(Lattice::new(Kind::Ring, 401, 1).is_none());
        // A ring of two would link its nodes twice.
        assert!(Lattice::new(Kind::Ring, 2, 1).is_none());
        assert!(Lattice::new(Kind::Ring, 3, 1).is_some());
    }

    #[test]
    fn kinds_parse_from_their_ids() {
        assert_eq!(Kind::parse("hex"), Some(Kind::Hex));
        assert_eq!(Kind::parse("nope"), None);
    }
}
