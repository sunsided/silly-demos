//! The edges a network learns between its nodes by competitive Hebbian
//! learning: the two nodes nearest a sample are joined, and an edge that
//! no sample renews grows old and goes.

use crate::codebook::Codebook;

/// One edge and how many updates of either end have passed since a sample
/// last renewed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The lower node index.
    pub a: u32,
    /// The higher node index.
    pub b: u32,
    /// Updates of an end node since the edge was last renewed.
    pub age: u32,
}

/// An undirected graph over node indices, without parallel edges or loops.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    /// Every edge once, `a < b`.
    edges: Vec<Edge>,
}

impl Graph {
    /// The edges, each once with `a < b`.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The number of edges.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether there are no edges.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Drops every edge.
    pub fn clear(&mut self) {
        self.edges.clear();
    }

    /// The edge position of `i`–`j`, if they are joined.
    fn find(&self, i: u32, j: u32) -> Option<usize> {
        let (a, b) = (i.min(j), i.max(j));
        self.edges.iter().position(|e| e.a == a && e.b == b)
    }

    /// Whether `i` and `j` are joined.
    pub fn connected(&self, i: u32, j: u32) -> bool {
        self.find(i, j).is_some()
    }

    /// Joins `i` and `j` with a fresh edge, or renews the one there is.
    /// Does nothing for `i == j`.
    pub fn connect(&mut self, i: u32, j: u32) {
        if i == j {
            return;
        }
        match self.find(i, j) {
            Some(k) => self.edges[k].age = 0,
            None => self.edges.push(Edge {
                a: i.min(j),
                b: i.max(j),
                age: 0,
            }),
        }
    }

    /// Removes the edge `i`–`j`, if there is one.
    pub fn disconnect(&mut self, i: u32, j: u32) {
        if let Some(k) = self.find(i, j) {
            self.edges.swap_remove(k);
        }
    }

    /// Ages every edge at `i` by one update.
    pub fn age_at(&mut self, i: u32) {
        for e in &mut self.edges {
            if e.a == i || e.b == i {
                e.age += 1;
            }
        }
    }

    /// Removes every edge older than `max_age`.
    pub fn prune(&mut self, max_age: u32) {
        self.edges.retain(|e| e.age <= max_age);
    }

    /// The nodes joined to `i`.
    pub fn neighbors(&self, i: u32) -> impl Iterator<Item = u32> + '_ {
        self.edges.iter().filter_map(move |e| match () {
            _ if e.a == i => Some(e.b),
            _ if e.b == i => Some(e.a),
            _ => None,
        })
    }

    /// How many edges `i` has.
    pub fn degree(&self, i: u32) -> usize {
        self.neighbors(i).count()
    }

    /// Forgets node `i`: drops its edges and renumbers the nodes after it
    /// down by one, as when it is removed from a codebook.
    pub fn remove_node(&mut self, i: u32) {
        self.edges.retain(|e| e.a != i && e.b != i);
        for e in &mut self.edges {
            if e.a > i {
                e.a -= 1;
            }
            if e.b > i {
                e.b -= 1;
            }
        }
    }

    /// The edges as flat pairs a, b, for drawing.
    pub fn pairs(&self) -> Vec<u32> {
        self.edges.iter().flat_map(|e| [e.a, e.b]).collect()
    }
}

/// The share of samples whose nearest and second-nearest nodes are not
/// joined: the topographic error of a learned graph, where a map uses its
/// lattice instead. 0 without samples or with fewer than two nodes.
pub fn graph_error(cb: &Codebook, graph: &Graph, samples: &[f32]) -> f32 {
    let n = samples.len() / cb.dim;
    if n == 0 || cb.len() < 2 {
        return 0.0;
    }
    let broken = samples
        .chunks_exact(cb.dim)
        .filter(|x| {
            let (a, b) = cb.bmu2(x);
            !graph.connected(a as u32, b as u32)
        })
        .count();
    broken as f32 / n as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_undirected_and_unique() {
        let mut g = Graph::default();
        g.connect(3, 1);
        g.connect(1, 3);
        g.connect(2, 2);
        assert_eq!(g.edges(), &[Edge { a: 1, b: 3, age: 0 }]);
        assert!(g.connected(3, 1));
        assert!(!g.connected(2, 2));
    }

    #[test]
    fn old_edges_go_and_renewed_ones_stay() {
        let mut g = Graph::default();
        g.connect(0, 1);
        g.connect(0, 2);
        for _ in 0..5 {
            g.age_at(0);
        }
        g.connect(0, 1);
        g.prune(4);
        assert!(g.connected(0, 1));
        assert!(!g.connected(0, 2));
    }

    #[test]
    fn removing_a_node_renumbers_the_rest() {
        let mut g = Graph::default();
        g.connect(0, 1);
        g.connect(1, 2);
        g.connect(2, 3);
        g.remove_node(1);
        assert_eq!(g.pairs(), vec![1, 2]);
        assert_eq!(g.degree(0), 0);
    }

    #[test]
    fn graph_error_counts_unjoined_nearest_pairs() {
        let cb = Codebook {
            dim: 1,
            weights: vec![0.0, 1.0, 2.0],
        };
        let mut g = Graph::default();
        g.connect(0, 1);
        // 0.4 is nearest 0 then 1 (joined); 1.6 is nearest 2 then 1 (not).
        assert_eq!(graph_error(&cb, &g, &[0.4, 1.6]), 0.5);
        assert_eq!(graph_error(&cb, &g, &[]), 0.0);
    }
}
