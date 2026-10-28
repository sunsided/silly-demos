//! Self-organizing maps, as Teuvo Kohonen introduced them.
//!
//! A lattice of nodes, each with a weight vector in input space, is pulled
//! towards the samples it is shown: the node nearest a sample (its
//! best-matching unit) moves most, its lattice neighbors less, under a
//! Gaussian kernel whose radius shrinks over training. The lattice keeps
//! its topology while its weights spread over the data, so neighboring
//! nodes end up responsible for neighboring inputs.

pub mod codebook;
pub mod data;
pub mod density;
pub mod init;
pub mod lattice;
pub mod metrics;
pub mod rng;
pub mod schedule;
pub mod successors;
pub mod train;
pub mod tsp;
pub mod views;
