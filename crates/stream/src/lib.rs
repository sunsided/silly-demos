//! Clustering a stream: points arrive one at a time, each is seen once, and
//! the clusters, including how many there are, have to be found as they
//! come, without a pass over all the data and without being told `k`.
//!
//! Three methods work on the same stream. [`balls`] keeps each cluster as a
//! ball and spawns, merges, splits and prunes balls as points arrive,
//! checking only a ball's neighbors; [`ellipses`] does the same with a
//! covariance per cluster and Mahalanobis distances, so long thin clusters
//! fit. [`sketch`] keeps a one-pass sketch of weighted
//! centers by online facility location and refines it into clusters with a
//! penalized ball k-means, so `k` is what balances error against the cost
//! of a cluster. [`source`] makes labeled streams to try them on and
//! [`metrics`] scores them against the labels.

pub mod balls;
pub mod cov;
pub mod ellipses;
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
    /// A cluster whose points had come apart became two.
    Split,
}

impl EventKind {
    /// The kind's UI id.
    pub fn id(self) -> &'static str {
        match self {
            EventKind::Spawn => "spawn",
            EventKind::Merge => "merge",
            EventKind::Prune => "prune",
            EventKind::Split => "split",
        }
    }
}

/// Events kept per model; older ones are dropped.
pub const EVENT_CAP: usize = 64;

/// A model's events: the recent ones for drawing, and how many of each
/// kind there have been in all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Log {
    /// The newest events, at most [`EVENT_CAP`], oldest first.
    pub events: Vec<Event>,
    /// How many spawns, merges, prunes and splits there have been, in that order.
    pub counts: [u64; 4],
}

impl Log {
    /// Records `event`, dropping the oldest beyond [`EVENT_CAP`].
    pub fn record(&mut self, event: Event) {
        self.counts[event.kind as usize] += 1;
        self.events.push(event);
        if self.events.len() > EVENT_CAP {
            self.events.remove(0);
        }
    }

    /// How many events of `kind` there have been.
    pub fn count(&self, kind: EventKind) -> u64 {
        self.counts[kind as usize]
    }
}
