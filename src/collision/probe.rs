use super::report::Note;

/// Operation counts a method reports alongside its result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Dot and cross products (including projections onto axes).
    pub dots: u32,
    pub sqrts: u32,
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
    fn dots(&mut self, _n: u32) {}
    fn sqrt(&mut self) {}
    fn div(&mut self) {}
    fn axis(&mut self) {}
    fn support(&mut self) {}
    fn iteration(&mut self) {}

    /// Whether notes are kept; methods skip sketch-only work otherwise.
    fn sketching(&self) -> bool {
        false
    }

    fn note(&mut self, _note: impl FnOnce() -> Note) {}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Silent;

impl Probe for Silent {}

#[derive(Clone, Debug, Default)]
pub struct Recorder {
    pub counts: Counts,
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
