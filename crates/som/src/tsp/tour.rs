//! Tours as orders of city indices, and reading one off a ring.

/// Whether `tour` visits each of `0..n` exactly once.
pub fn is_valid(tour: &[u32], n: usize) -> bool {
    let mut seen = vec![false; n];
    tour.len() == n
        && tour
            .iter()
            .all(|&c| (c as usize) < n && !std::mem::replace(&mut seen[c as usize], true))
}

/// The tour a ring of nodes stands for: every city goes to its nearest node
/// and cities are visited in the order of their nodes round the ring. Cities
/// sharing a node go in order along the ring's direction there, then by
/// index, so the result is always a valid tour, however the ring lies. An
/// empty ring gives the cities in index order.
pub fn from_ring(cities: &[[f32; 2]], ring: &[[f32; 2]]) -> Vec<u32> {
    let m = ring.len();
    if m == 0 {
        return (0..cities.len() as u32).collect();
    }
    let mut keyed: Vec<(usize, f32, u32)> = cities
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let j = nearest(ring, *c);
            let (a, b) = (ring[j], ring[(j + 1) % m]);
            let along = (c[0] - a[0]) * (b[0] - a[0]) + (c[1] - a[1]) * (b[1] - a[1]);
            (j, along, i as u32)
        })
        .collect();
    keyed.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(x.2.cmp(&y.2)));
    keyed.into_iter().map(|(_, _, i)| i).collect()
}

/// The index of the node nearest `p`; the first of equals.
pub fn nearest(nodes: &[[f32; 2]], p: [f32; 2]) -> usize {
    let mut best = (0, f32::INFINITY);
    for (j, q) in nodes.iter().enumerate() {
        let d = dist2(*q, p);
        if d < best.1 {
            best = (j, d);
        }
    }
    best.0
}

pub fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// How far the city farthest from any node is from its nearest one.
pub fn worst_fit(cities: &[[f32; 2]], nodes: &[[f32; 2]]) -> f32 {
    cities
        .iter()
        .map(|c| dist2(nodes[nearest(nodes, *c)], *c))
        .fold(0.0, f32::max)
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn validity_means_a_permutation() {
        assert!(is_valid(&[2, 0, 1], 3));
        assert!(!is_valid(&[0, 0, 1], 3));
        assert!(!is_valid(&[0, 1], 3));
        assert!(!is_valid(&[0, 1, 3], 3));
    }

    #[test]
    fn a_ring_is_read_in_node_order() {
        let cities = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
        let ring = [[0.0, -0.9], [0.9, 0.0], [0.0, 0.9], [-0.9, 0.0]];
        assert_eq!(from_ring(&cities, &ring), vec![3, 0, 1, 2]);
    }

    #[test]
    fn cities_sharing_a_node_go_along_the_ring() {
        // Both cities are nearest node 0; the ring heads in +x from there.
        let cities = [[0.3, 0.1], [0.1, 0.1], [5.0, 5.0]];
        let ring = [[0.0, 0.0], [1.0, 0.0], [5.0, 5.0]];
        assert_eq!(from_ring(&cities, &ring), vec![1, 0, 2]);
    }

    #[test]
    fn any_ring_gives_a_valid_tour() {
        let mut rng = Rng::new(5);
        for m in [0, 1, 2, 7, 40] {
            let cities: Vec<[f32; 2]> = (0..25).map(|_| [rng.next_f32(), rng.next_f32()]).collect();
            let ring: Vec<[f32; 2]> = (0..m).map(|_| [rng.next_f32(), rng.next_f32()]).collect();
            assert!(is_valid(&from_ring(&cities, &ring), 25), "{m} nodes");
        }
    }
}
