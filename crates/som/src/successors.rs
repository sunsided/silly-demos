//! What came after the self-organizing map: vector quantizers that keep
//! its idea of nodes pulled towards samples but drop the fixed lattice.
//! Neural gas ranks nodes by distance instead, growing neural gas grows
//! its nodes and learns its own edges, and k-means, the baseline, has no
//! neighborhood at all.

pub mod gng;
pub mod graph;
pub mod kmeans;
pub mod neural_gas;
