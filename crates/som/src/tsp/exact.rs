//! The shortest tour, found exactly by Held and Karp's dynamic programme
//! over subsets: affordable up to a dozen or so cities.

use super::cities::Matrix;

/// Cities the exact solver takes: 2¹² subsets × 12 ends is instant.
pub const MAX_EXACT: usize = 13;

/// The shortest tour and its length, starting at city 0; `None` above
/// [`MAX_EXACT`] cities.
pub fn held_karp(m: &Matrix) -> Option<(Vec<u32>, f64)> {
    let n = m.len();
    if !(2..=MAX_EXACT).contains(&n) {
        return None;
    }
    // best[set][j]: the shortest path from 0 through every city in `set`
    // (cities 1..n as bits 0..n-1), ending at j, which is in `set`.
    let k = n - 1;
    let full = 1usize << k;
    let mut best = vec![f64::INFINITY; full * k];
    let mut from = vec![u8::MAX; full * k];
    for j in 0..k {
        best[(1 << j) * k + j] = m.get(0, j as u32 + 1);
    }
    for set in 1..full {
        for j in 0..k {
            if set & (1 << j) == 0 {
                continue;
            }
            let here = best[set * k + j];
            if !here.is_finite() {
                continue;
            }
            for next in 0..k {
                if set & (1 << next) != 0 {
                    continue;
                }
                let to = set | (1 << next);
                let d = here + m.get(j as u32 + 1, next as u32 + 1);
                if d < best[to * k + next] {
                    best[to * k + next] = d;
                    from[to * k + next] = j as u8;
                }
            }
        }
    }
    let all = full - 1;
    let (mut end, length) = (0..k)
        .map(|j| (j, best[all * k + j] + m.get(j as u32 + 1, 0)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let mut tour = Vec::with_capacity(n);
    let mut set = all;
    loop {
        tour.push(end as u32 + 1);
        let prev = from[set * k + end];
        set &= !(1 << end);
        if prev == u8::MAX {
            break;
        }
        end = prev as usize;
    }
    tour.push(0);
    tour.reverse();
    Some((tour, length))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::tsp::cities::{Cities, Metric};
    use crate::tsp::tour::is_valid;

    fn random(n: usize, rng: &mut Rng) -> Matrix {
        let xy: Vec<f32> = (0..2 * n).map(|_| rng.next_f32() * 100.0).collect();
        Cities::new(&xy, Metric::Euclid).unwrap().matrix()
    }

    /// Every tour from city 0, by Heap's algorithm over the rest.
    fn brute_force(m: &Matrix) -> f64 {
        let mut rest: Vec<u32> = (1..m.len() as u32).collect();
        let mut best = f64::INFINITY;
        fn permute(k: usize, rest: &mut Vec<u32>, m: &Matrix, best: &mut f64) {
            if k == rest.len() {
                let mut tour = vec![0];
                tour.extend_from_slice(rest);
                *best = best.min(m.tour_length(&tour));
                return;
            }
            for i in k..rest.len() {
                rest.swap(k, i);
                permute(k + 1, rest, m, best);
                rest.swap(k, i);
            }
        }
        permute(0, &mut rest, m, &mut best);
        best
    }

    #[test]
    fn matches_brute_force() {
        let mut rng = Rng::new(11);
        for seed in 0..20 {
            let n = 4 + seed % 5;
            let m = random(n, &mut rng);
            let (tour, length) = held_karp(&m).unwrap();
            assert!(is_valid(&tour, n));
            assert!((m.tour_length(&tour) - length).abs() < 1e-9);
            assert!(
                (length - brute_force(&m)).abs() < 1e-9,
                "seed {seed}, {n} cities"
            );
        }
    }

    #[test]
    fn refuses_more_than_it_can_afford() {
        let mut rng = Rng::new(1);
        assert!(held_karp(&random(MAX_EXACT, &mut rng)).is_some());
        assert!(held_karp(&random(MAX_EXACT + 1, &mut rng)).is_none());
    }
}
