//! Self-organising maps, as Teuvo Kohonen introduced them.
//!
//! A lattice of nodes, each with a weight vector in input space, is pulled
//! towards the samples it is shown: the node nearest a sample (its
//! best-matching unit) moves most, its lattice neighbours less, under a
//! Gaussian kernel whose radius shrinks over training. The lattice keeps
//! its topology while its weights spread over the data, so neighbouring
//! nodes end up responsible for neighbouring inputs.

pub mod density;
pub mod lattice;
pub mod rng;
