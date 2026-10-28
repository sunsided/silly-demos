//! Clustering a stream: points arrive one at a time, each is seen once, and
//! the clusters, including how many there are, have to be found as they
//! come, without a pass over all the data and without being told `k`.
//!
//! Two methods work on the same stream. [`balls`] keeps each cluster as a
//! ball and spawns, merges and prunes balls as points arrive, checking only
//! a ball's neighbors. [`sketch`] keeps a one-pass sketch of weighted
//! centers by online facility location and refines it into clusters with a
//! penalized ball k-means, so `k` is what balances error against the cost
//! of a cluster. [`source`] makes labeled streams to try them on and
//! [`metrics`] scores them against the labels.

pub mod balls;
pub mod metrics;
pub mod rng;
pub mod sketch;
pub mod source;

/// A point in the unit square.
pub type Point = [f32; 2];

/// The squared distance between two points.
pub fn dist2(a: Point, b: Point) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// Something that happened to a model's clusters, for drawing as it happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// What happened.
    pub kind: EventKind,
    /// Where, in the unit square.
    pub at: Point,
    /// After how many points of the stream.
    pub t: u64,
}

/// The kinds of [`Event`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// A new cluster appeared.
    Spawn,
    /// Two clusters became one.
    Merge,
    /// A cluster that no longer got points was removed.
    Prune,
}

impl EventKind {
    /// The kind's UI id.
    pub fn id(self) -> &'static str {
        match self {
            EventKind::Spawn => "spawn",
            EventKind::Merge => "merge",
            EventKind::Prune => "prune",
        }
    }
}

/// Events kept per model; older ones are dropped.
pub const EVENT_CAP: usize = 64;

/// Appends `event`, dropping the oldest beyond [`EVENT_CAP`].
pub fn record(events: &mut Vec<Event>, event: Event) {
    events.push(event);
    if events.len() > EVENT_CAP {
        events.remove(0);
    }
}
