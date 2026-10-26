//! A probability density over the unit square that samples are drawn from:
//! a 64×64 grid of non-negative weights, set from a preset or painted.

use crate::rng::Rng;

/// Cells per side of the presets.
pub const SIZE: usize = 64;

/// Cells per side a custom density may have.
pub const MAX_SIDE: usize = 256;

/// Presets keep this far from the edges, so the map stays on the sheet.
const MARGIN: f32 = 0.05;

#[derive(Clone, Debug)]
pub struct Density {
    side: usize,
    /// Row-major; row 0 covers y in [0, 1 / side).
    cells: Vec<f32>,
    /// Running sums of `cells`, rebuilt on the first sample after an edit.
    cumulative: Vec<f32>,
    dirty: bool,
    version: u64,
}

impl Density {
    /// `uniform`, `annulus`, `moons`, `blobs`, `lshape`, `gradient` or `blank`.
    pub fn preset(name: &str) -> Option<Density> {
        let weight: fn(f32, f32) -> f32 = match name {
            "uniform" => |x, y| inside(x, y) as u8 as f32,
            "annulus" => |x, y| {
                let r = (x - 0.5).hypot(y - 0.5);
                (0.25..=0.45).contains(&r) as u8 as f32
            },
            "moons" => moons,
            "blobs" => |x, y| {
                const CENTRES: [(f32, f32); 3] = [(0.3, 0.3), (0.7, 0.35), (0.45, 0.72)];
                let w: f32 = CENTRES
                    .iter()
                    .map(|&(cx, cy)| {
                        (-((x - cx).powi(2) + (y - cy).powi(2)) / (2.0 * 0.07f32.powi(2))).exp()
                    })
                    .sum();
                if w < 1e-3 { 0.0 } else { w }
            },
            "lshape" => |x, y| {
                let upright = (0.15..=0.4).contains(&x) && (0.15..=0.85).contains(&y);
                let foot = (0.15..=0.85).contains(&x) && (0.6..=0.85).contains(&y);
                (upright || foot) as u8 as f32
            },
            "gradient" => |x, y| if inside(x, y) { x } else { 0.0 },
            "blank" => |_, _| 0.0,
            _ => return None,
        };
        let cells = (0..SIZE * SIZE)
            .map(|i| {
                let (x, y) = centre(SIZE, i);
                weight(x, y)
            })
            .collect();
        Some(Density {
            side: SIZE,
            cells,
            cumulative: Vec::new(),
            dirty: true,
            version: 0,
        })
    }

    /// A density from `side`×`side` non-negative weights, row-major from
    /// y = 0 (an image's darkness, say). `None` for a side outside 2 to
    /// [`MAX_SIDE`], the wrong number of cells, or a weight that is negative
    /// or not finite.
    pub fn from_cells(side: usize, cells: &[f32]) -> Option<Density> {
        if !(2..=MAX_SIDE).contains(&side) || cells.len() != side * side {
            return None;
        }
        if cells.iter().any(|c| !c.is_finite() || *c < 0.0) {
            return None;
        }
        Some(Density {
            side,
            cells: cells.to_vec(),
            cumulative: Vec::new(),
            dirty: true,
            version: 0,
        })
    }

    pub fn side(&self) -> usize {
        self.side
    }

    /// Adds `amount` within `radius` of (x, y), fading smoothly to 0 at the
    /// rim; a negative amount erases. Weights never go below 0. Non-finite
    /// or non-positive-radius strokes are ignored.
    pub fn paint(&mut self, x: f32, y: f32, radius: f32, amount: f32) {
        if !(x.is_finite() && y.is_finite() && radius.is_finite() && amount.is_finite())
            || radius <= 0.0
        {
            return;
        }
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let (cx, cy) = centre(self.side, i);
            let d = (cx - x).hypot(cy - y);
            if d < radius {
                let falloff = (1.0 - (d / radius).powi(2)).powi(2);
                *cell = (*cell + amount * falloff).max(0.0);
            }
        }
        self.dirty = true;
        self.version += 1;
    }

    pub fn mass(&self) -> f32 {
        self.cells.iter().sum()
    }

    pub fn cells(&self) -> &[f32] {
        &self.cells
    }

    /// Changes with every edit, so holders of samples know to draw new ones.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// A point drawn from the density: a cell in proportion to its weight,
    /// then uniformly within it. `None` when there is no mass.
    pub fn sample(&mut self, rng: &mut Rng) -> Option<[f32; 2]> {
        if self.dirty {
            let mut total = 0.0;
            self.cumulative = self
                .cells
                .iter()
                .map(|w| {
                    total += w;
                    total
                })
                .collect();
            self.dirty = false;
        }
        let total = *self.cumulative.last()?;
        if total <= 0.0 {
            return None;
        }
        let u = rng.next_f32() * total;
        let side = self.side;
        let i = self
            .cumulative
            .partition_point(|&c| c <= u)
            .min(side * side - 1);
        let cell = 1.0 / side as f32;
        let x = ((i % side) as f32 + rng.next_f32()) * cell;
        let y = ((i / side) as f32 + rng.next_f32()) * cell;
        // Rounding can land exactly on 1.
        Some([x.min(1.0 - f32::EPSILON), y.min(1.0 - f32::EPSILON)])
    }
}

fn centre(side: usize, i: usize) -> (f32, f32) {
    let cell = 1.0 / side as f32;
    (
        ((i % side) as f32 + 0.5) * cell,
        ((i / side) as f32 + 0.5) * cell,
    )
}

fn inside(x: f32, y: f32) -> bool {
    (MARGIN..=1.0 - MARGIN).contains(&x) && (MARGIN..=1.0 - MARGIN).contains(&y)
}

/// Two interleaved half rings, as scikit-learn's `make_moons` lays them
/// out (unit radius, the second shifted by (1, −0.5) and flipped), scaled
/// into the square.
fn moons(x: f32, y: f32) -> f32 {
    const SCALE: f32 = 0.8 / 3.0;
    const THICKNESS: f32 = 0.06 / SCALE;
    // Back into make_moons coordinates, y up.
    let mx = (x - 0.1) / SCALE - 1.0;
    let my = (1.0 - y - 0.3) / SCALE - 0.5;
    let upper = my >= 0.0 && (mx.hypot(my) - 1.0).abs() < THICKNESS / 2.0;
    let lower = my <= 0.5 && ((mx - 1.0).hypot(my - 0.5) - 1.0).abs() < THICKNESS / 2.0;
    (upper || lower) as u8 as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    const PRESETS: [&str; 6] = ["uniform", "annulus", "moons", "blobs", "lshape", "gradient"];

    fn samples(d: &mut Density, n: usize) -> Vec<[f32; 2]> {
        let mut rng = Rng::new(11);
        (0..n).map(|_| d.sample(&mut rng).unwrap()).collect()
    }

    #[test]
    fn every_preset_but_blank_has_mass() {
        for name in PRESETS {
            assert!(Density::preset(name).unwrap().mass() > 0.0, "{name}");
        }
        assert_eq!(Density::preset("blank").unwrap().mass(), 0.0);
        assert!(Density::preset("nope").is_none());
    }

    #[test]
    fn blank_samples_none() {
        let mut d = Density::preset("blank").unwrap();
        assert!(d.sample(&mut Rng::new(1)).is_none());
    }

    #[test]
    fn samples_stay_in_unit_square() {
        for name in PRESETS {
            let mut d = Density::preset(name).unwrap();
            for [x, y] in samples(&mut d, 2000) {
                assert!((0.0..1.0).contains(&x) && (0.0..1.0).contains(&y), "{name}");
            }
        }
    }

    #[test]
    fn annulus_samples_avoid_the_hole() {
        let mut d = Density::preset("annulus").unwrap();
        for [x, y] in samples(&mut d, 2000) {
            assert!((x - 0.5).hypot(y - 0.5) > 0.15, "({x}, {y})");
        }
    }

    #[test]
    fn gradient_puts_more_mass_right() {
        let mut d = Density::preset("gradient").unwrap();
        let s = samples(&mut d, 4000);
        let right = s.iter().filter(|p| p[0] > 0.5).count();
        assert!(right > 2 * (s.len() - right), "{right} of {}", s.len());
    }

    #[test]
    fn paint_then_erase_returns_to_zero() {
        let mut d = Density::preset("blank").unwrap();
        d.paint(0.5, 0.5, 0.1, 1.0);
        assert!(d.mass() > 0.0);
        let [x, y] = d.sample(&mut Rng::new(2)).unwrap();
        assert!((x - 0.5).hypot(y - 0.5) < 0.1 + 1.5 / SIZE as f32);
        d.paint(0.5, 0.5, 0.2, -10.0);
        assert_eq!(d.mass(), 0.0);
        assert!(d.sample(&mut Rng::new(2)).is_none());
    }

    #[test]
    fn edits_bump_the_version_and_bad_paint_is_ignored() {
        let mut d = Density::preset("uniform").unwrap();
        let (v, mass) = (d.version(), d.mass());
        d.paint(f32::NAN, 0.5, 0.1, 1.0);
        d.paint(0.5, 0.5, f32::INFINITY, 1.0);
        d.paint(0.5, 0.5, 0.0, 1.0);
        d.paint(0.5, 0.5, 0.1, f32::NAN);
        assert_eq!((d.version(), d.mass()), (v, mass));
        d.paint(0.5, 0.5, 0.1, 1.0);
        assert!(d.version() > v && d.mass() > mass);
        assert_eq!(d.cells().len(), SIZE * SIZE);
    }

    #[test]
    fn from_cells_checks_its_input() {
        assert!(Density::from_cells(1, &[1.0]).is_none());
        assert!(Density::from_cells(257, &vec![1.0; 257 * 257]).is_none());
        assert!(Density::from_cells(4, &[1.0; 15]).is_none());
        assert!(Density::from_cells(2, &[1.0, f32::NAN, 0.0, 0.0]).is_none());
        assert!(Density::from_cells(2, &[1.0, -1.0, 0.0, 0.0]).is_none());
        let d = Density::from_cells(128, &vec![0.5; 128 * 128]).unwrap();
        assert_eq!((d.side(), d.cells().len()), (128, 128 * 128));
        assert_eq!(Density::preset("uniform").unwrap().side(), SIZE);
    }

    #[test]
    fn an_all_white_image_is_an_empty_density() {
        let mut d = Density::from_cells(8, &[0.0; 64]).unwrap();
        assert_eq!(d.mass(), 0.0);
        assert!(d.sample(&mut Rng::new(1)).is_none());
    }

    #[test]
    fn custom_density_samples_where_its_mass_is() {
        let mut cells = [0.0; 16];
        cells[15] = 1.0;
        let mut d = Density::from_cells(4, &cells).unwrap();
        for [x, y] in samples(&mut d, 200) {
            assert!(x >= 0.75 && y >= 0.75, "({x}, {y})");
        }
    }

    #[test]
    fn paint_works_at_any_side() {
        let mut d = Density::from_cells(16, &[0.0; 256]).unwrap();
        d.paint(0.5, 0.5, 0.2, 1.0);
        assert!(d.mass() > 0.0);
        let [x, y] = d.sample(&mut Rng::new(3)).unwrap();
        assert!((x - 0.5).hypot(y - 0.5) < 0.2 + 1.5 / 16.0);
    }
}
