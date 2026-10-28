//! The classical heuristics the rings are measured against: a greedy
//! nearest-neighbour tour, and that tour improved by 2-opt and Or-opt
//! moves until none helps.

use super::cities::Matrix;

/// Improvements smaller than this are rounding, not progress.
const EPS: f64 = 1e-9;

/// Start at `start` and always go to the nearest city not yet visited.
pub fn nearest_neighbour(m: &Matrix, start: usize) -> Vec<u32> {
    let n = m.len();
    let mut visited = vec![false; n];
    let mut tour = Vec::with_capacity(n);
    let mut at = start as u32;
    visited[start] = true;
    tour.push(at);
    for _ in 1..n {
        let next = (0..n as u32)
            .filter(|&c| !visited[c as usize])
            .min_by(|&a, &b| m.get(at, a).total_cmp(&m.get(at, b)))
            .expect("an unvisited city is left");
        visited[next as usize] = true;
        tour.push(next);
        at = next;
    }
    tour
}

/// Local search from the nearest-neighbour tour, one position at a time so
/// it can be watched: a step tries every 2-opt move whose first edge leaves
/// the city at the current position, then every Or-opt move of the 1 to 3
/// cities after it, and applies the first that shortens the tour. A full
/// round of positions without one leaves a tour no such move improves.
#[derive(Clone, Debug)]
pub struct LocalSearch {
    /// The tour so far, as city indices; always a permutation of all cities.
    pub tour: Vec<u32>,
    /// The tour position the next step tries.
    position: usize,
    /// Positions tried since the last improvement.
    stale: usize,
}

impl LocalSearch {
    /// A search starting from the nearest-neighbor tour from city 0.
    pub fn new(m: &Matrix) -> Self {
        Self {
            tour: nearest_neighbour(m, 0),
            position: 0,
            stale: 0,
        }
    }

    /// Whether a full round of positions has passed without an improvement.
    pub fn done(&self) -> bool {
        self.stale >= self.tour.len()
    }

    /// Tries the current position; `true` if the tour got shorter.
    pub fn step(&mut self, m: &Matrix) -> bool {
        if self.done() {
            return false;
        }
        let i = self.position;
        let improved = two_opt_at(m, &mut self.tour, i) || or_opt_at(m, &mut self.tour, i);
        self.position = (i + 1) % self.tour.len();
        self.stale = if improved { 0 } else { self.stale + 1 };
        improved
    }
}

/// The first 2-opt move removing the edge from position `i` that shortens
/// the tour: replace edges (a, b) and (c, d) with (a, c) and (b, d) by
/// reversing the stretch from b to c.
pub fn two_opt_at(m: &Matrix, tour: &mut [u32], i: usize) -> bool {
    let n = tour.len();
    let (a, b) = (tour[i], tour[(i + 1) % n]);
    for k in 2..n - 1 {
        let j = (i + k) % n;
        let (c, d) = (tour[j], tour[(j + 1) % n]);
        let delta = m.get(a, c) + m.get(b, d) - m.get(a, b) - m.get(c, d);
        if delta < -EPS {
            reverse(tour, (i + 1) % n, j);
            return true;
        }
    }
    false
}

/// Reverses the tour between positions `from` and `to`, both included,
/// going forwards and wrapping round the end.
fn reverse(tour: &mut [u32], from: usize, to: usize) {
    let n = tour.len();
    let len = (to + n - from) % n + 1;
    for k in 0..len / 2 {
        tour.swap((from + k) % n, (from + len - 1 - k) % n);
    }
}

/// The first Or-opt move that shortens the tour: take the 1 to 3 cities
/// after position `i` out and put them, either way round, between two
/// other neighbouring cities.
pub fn or_opt_at(m: &Matrix, tour: &mut Vec<u32>, i: usize) -> bool {
    let n = tour.len();
    for len in 1..=3 {
        // The segment, the city before it and the one after must differ,
        // and at least one edge must remain to insert into.
        if n < len + 3 {
            break;
        }
        let prev = tour[i];
        let seg: Vec<u32> = (1..=len).map(|k| tour[(i + k) % n]).collect();
        let next = tour[(i + len + 1) % n];
        let (first, last) = (seg[0], seg[len - 1]);
        let gain = m.get(prev, first) + m.get(last, next) - m.get(prev, next);
        // Every edge (x, y) of the tour with the segment taken out, except
        // the one that closes the gap, where it was.
        let rest: Vec<u32> = (0..n - len).map(|k| tour[(i + len + 1 + k) % n]).collect();
        for k in 0..rest.len() - 1 {
            let (x, y) = (rest[k], rest[k + 1]);
            let forward = m.get(x, first) + m.get(last, y) - m.get(x, y);
            let backward = m.get(x, last) + m.get(first, y) - m.get(x, y);
            let (cost, flip) = if backward < forward {
                (backward, true)
            } else {
                (forward, false)
            };
            if cost < gain - EPS {
                let mut out = Vec::with_capacity(n);
                out.extend_from_slice(&rest[..=k]);
                if flip {
                    out.extend(seg.iter().rev());
                } else {
                    out.extend_from_slice(&seg);
                }
                out.extend_from_slice(&rest[k + 1..]);
                *tour = out;
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::tsp::cities::{Cities, Metric};
    use crate::tsp::tour::is_valid;

    fn random(n: usize, seed: u64) -> Cities {
        let mut rng = Rng::new(seed);
        let xy: Vec<f32> = (0..2 * n).map(|_| rng.next_f32() * 1000.0).collect();
        Cities::new(&xy, Metric::Euclid).unwrap()
    }

    fn finish(m: &Matrix) -> LocalSearch {
        let mut search = LocalSearch::new(m);
        while !search.done() {
            search.step(m);
        }
        search
    }

    #[test]
    fn nearest_neighbour_goes_to_the_nearest() {
        let c = Cities::new(&[0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 3.0, 0.0], Metric::Euclid).unwrap();
        assert_eq!(nearest_neighbour(&c.matrix(), 0), vec![0, 2, 3, 1]);
    }

    #[test]
    fn reversing_wraps_round_the_end() {
        let mut t = [0, 1, 2, 3, 4, 5];
        reverse(&mut t, 4, 1);
        assert_eq!(t, [5, 4, 2, 3, 1, 0]);
    }

    #[test]
    fn local_search_never_lengthens_and_ends_two_opt_optimal() {
        for seed in 0..10 {
            let m = random(60, seed).matrix();
            let start = m.tour_length(&nearest_neighbour(&m, 0));
            let mut search = finish(&m);
            assert!(is_valid(&search.tour, 60));
            assert!(m.tour_length(&search.tour) <= start + EPS, "seed {seed}");
            for i in 0..60 {
                assert!(
                    !two_opt_at(&m, &mut search.tour, i),
                    "seed {seed}: 2-opt move left at {i}"
                );
            }
        }
    }

    #[test]
    fn or_opt_moves_a_city_out_of_a_detour() {
        // On a line, 0 1 3 2 4 doubles back over 2–3; moving 3 fixes it.
        let c = Cities::new(
            &[0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0, 2.0, 5.0],
            Metric::Euclid,
        )
        .unwrap();
        let m = c.matrix();
        let mut tour = vec![0, 1, 3, 2, 4, 5];
        let before = m.tour_length(&tour);
        assert!(or_opt_at(&m, &mut tour, 1));
        assert!(is_valid(&tour, 6));
        assert!(m.tour_length(&tour) < before);
    }
}
