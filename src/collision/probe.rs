//! What a collision method observes while it runs: operation counts, and the
//! construction notes it draws, recorded by a [`Recorder`] or dropped by [`Silent`].

use super::report::Note;

/// Operation counts a method reports alongside its result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Dot and cross products (including projections onto axes).
    pub dots: u32,
    /// Square roots, including those inside normalisation.
    pub sqrts: u32,
    /// Divisions, including those inside normalisation.
    pub divs: u32,
    /// Separating axes tested.
    pub axes: u32,
    /// Support function evaluations (one per shape).
    pub supports: u32,
    /// Iterations of an iterative solver (GJK, EPA, root finding).
    pub iterations: u32,
}

/// Instrumentation hooks the algorithms call. [`Silent`] compiles them away for
/// benchmarking; [`Recorder`] counts operations and keeps construction notes.
pub trait Probe {
    /// Counts `n` dot or cross products.
    fn dots(&mut self, _n: u32) {}
    /// Counts one square root.
    fn sqrt(&mut self) {}
    /// Counts one division.
    fn div(&mut self) {}
    /// Counts one separating axis tested.
    fn axis(&mut self) {}
    /// Counts one support function evaluation.
    fn support(&mut self) {}
    /// Counts one solver iteration.
    fn iteration(&mut self) {}

    /// Whether notes are kept; methods skip sketch-only work otherwise.
    fn sketching(&self) -> bool {
        false
    }

    /// Keeps a construction note. The closure only runs when the probe keeps notes,
    /// so building one costs nothing otherwise.
    fn note(&mut self, _note: impl FnOnce() -> Note) {}
}

/// Probe that ignores every hook, for benchmarking.
#[derive(Clone, Copy, Debug, Default)]
pub struct Silent;

impl Probe for Silent {}

/// Probe that counts operations and keeps notes, for the step-by-step view.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    /// Operations counted so far.
    pub counts: Counts,
    /// Construction notes in the order they were made.
    pub notes: Vec<Note>,
}

impl Probe for Recorder {
    fn dots(&mut self, n: u32) {
        self.counts.dots += n;
    }
    fn sqrt(&mut self) {
        self.counts.sqrts += 1;
    }
    fn div(&mut self) {
        self.counts.divs += 1;
    }
    fn axis(&mut self) {
        self.counts.axes += 1;
    }
    fn support(&mut self) {
        self.counts.supports += 1;
    }
    fn iteration(&mut self) {
        self.counts.iterations += 1;
    }
    fn sketching(&self) -> bool {
        true
    }
    fn note(&mut self, note: impl FnOnce() -> Note) {
        self.notes.push(note());
    }
}
