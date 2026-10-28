//! City sets to try the solvers on, in a 0..1000 square: random ones, and
//! shapes, two of which have shortest tours known in closed form.

use crate::rng::Rng;

/// The side of the square the sets are made in.
pub const SIDE: f32 = 1000.0;

/// `n` cities of `kind`: "uniform", "clusters", "circle", "grid" or
/// "spiral", as x, y pairs. A grid has the nearest even number of cities
/// that makes a rectangle at least 2 by 2, so its size may differ from
/// `n`. `None` for an unknown kind or fewer than 3 cities.
pub fn generate(kind: &str, n: usize, rng: &mut Rng) -> Option<Vec<f32>> {
    if n < 3 {
        return None;
    }
    let mut xy = Vec::with_capacity(2 * n);
    match kind {
        "uniform" => {
            for _ in 0..2 * n {
                xy.push(rng.next_f32() * SIDE);
            }
        }
        "clusters" => {
            const BLOBS: usize = 5;
            const SPREAD: f32 = 60.0;
            let centres: Vec<[f32; 2]> = (0..BLOBS)
                .map(|_| {
                    [
                        150.0 + rng.next_f32() * 700.0,
                        150.0 + rng.next_f32() * 700.0,
                    ]
                })
                .collect();
            for k in 0..n {
                let c = centres[k % BLOBS];
                let (dx, dy) = gaussian(rng);
                xy.push((c[0] + dx * SPREAD).clamp(0.0, SIDE));
                xy.push((c[1] + dy * SPREAD).clamp(0.0, SIDE));
            }
        }
        "circle" => {
            for k in 0..n {
                let a = std::f32::consts::TAU * k as f32 / n as f32;
                xy.push(SIDE / 2.0 + CIRCLE_RADIUS * a.cos());
                xy.push(SIDE / 2.0 + CIRCLE_RADIUS * a.sin());
            }
        }
        "grid" => {
            let (w, h) = grid_size(n);
            let spacing = SIDE / w.max(h) as f32;
            for row in 0..h {
                for col in 0..w {
                    xy.push((col as f32 + 0.5) * spacing);
                    xy.push((row as f32 + 0.5) * spacing);
                }
            }
        }
        "spiral" => {
            const TURNS: f32 = 3.0;
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let a = std::f32::consts::TAU * TURNS * t;
                let r = 0.45 * SIDE * t;
                xy.push(SIDE / 2.0 + r * a.cos());
                xy.push(SIDE / 2.0 + r * a.sin());
            }
        }
        _ => return None,
    }
    Some(xy)
}

/// The length of the shortest tour of [`generate`]'s `kind` with `n`
/// cities, where it is known in closed form: round a circle, the polygon;
/// over a grid with an even number of cities, one spacing per city.
pub fn optimum(kind: &str, n: usize) -> Option<f64> {
    if n < 3 {
        return None;
    }
    match kind {
        "circle" => {
            let side = 2.0 * CIRCLE_RADIUS as f64 * (std::f64::consts::PI / n as f64).sin();
            Some(n as f64 * side)
        }
        "grid" => {
            let (w, h) = grid_size(n);
            Some((w * h) as f64 * (SIDE / w.max(h) as f32) as f64)
        }
        _ => None,
    }
}

const CIRCLE_RADIUS: f32 = 400.0;

/// The rectangle nearest `n` cities with an even count and both sides at
/// least 2, as square as that allows.
fn grid_size(n: usize) -> (usize, usize) {
    let mut best = (2, 2);
    let mut best_cost = usize::MAX;
    for h in 2..=n.max(4) {
        for w in h..=(n / h + 2).max(h) {
            if (w * h) % 2 != 0 {
                continue;
            }
            // Nearest count first, then the squarer one.
            let cost = (w * h).abs_diff(n) * 1000 + (w - h);
            if cost < best_cost {
                best_cost = cost;
                best = (w, h);
            }
        }
    }
    best
}

/// Two independent standard normals (Box–Muller).
fn gaussian(rng: &mut Rng) -> (f32, f32) {
    let u = 1.0 - rng.next_f32();
    let v = rng.next_f32();
    let r = (-2.0 * u.ln()).sqrt();
    let a = std::f32::consts::TAU * v;
    (r * a.cos(), r * a.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_have_their_size_and_stay_in_the_square() {
        for kind in ["uniform", "clusters", "circle", "spiral"] {
            let xy = generate(kind, 50, &mut Rng::new(1)).unwrap();
            assert_eq!(xy.len(), 100, "{kind}");
            assert!(xy.iter().all(|v| (0.0..=SIDE).contains(v)), "{kind}");
        }
        assert!(generate("blob", 50, &mut Rng::new(1)).is_none());
        assert!(generate("uniform", 2, &mut Rng::new(1)).is_none());
    }

    #[test]
    fn grids_are_even_rectangles_near_the_size_asked_for() {
        assert_eq!(grid_size(100), (10, 10));
        assert_eq!(grid_size(3), (2, 2));
        assert_eq!(grid_size(30), (6, 5));
        let (w, h) = grid_size(51);
        assert_eq!((w * h) % 2, 0);
        assert!((w * h).abs_diff(51) <= 1);
        assert_eq!(generate("grid", 100, &mut Rng::new(1)).unwrap().len(), 200);
    }

    #[test]
    fn the_closed_form_optima() {
        // A square's perimeter round a circle of radius 400: 4 · 400√2.
        assert!((optimum("circle", 4).unwrap() - 1600.0 * 2f64.sqrt()).abs() < 1e-6);
        // 10 × 10 cities 100 apart.
        assert_eq!(optimum("grid", 100), Some(10_000.0));
        assert_eq!(optimum("spiral", 100), None);
    }
}
