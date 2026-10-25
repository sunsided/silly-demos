//! Inverse kinematics for a planar chain of rigid segments.
//!
//! The chain is stored as joint positions `p[0..=n]`: `p[0]` is pinned to the
//! base and `p[n]` is the end effector. Every solver starts from the current
//! pose, so solving once per frame warm-starts from the previous answer.
//!
//! - FABRIK (Aristidou & Lasenby, 2011) drags the chain from the target back
//!   to the base, then from the base out again, re-stretching each segment to
//!   its length on the way. It moves joint positions, never angles.
//! - CCD (cyclic coordinate descent) visits the joints from the tip inwards
//!   and turns each one so the end effector points at the target.
//! - The Jacobian methods treat the joint angles as the unknowns. The Jacobian
//!   maps small angle changes to end effector motion; each iteration steps
//!   the angles along its transpose, or along its damped least-squares
//!   pseudo-inverse.
//!
//! None of these know the arm has a body: left alone they happily fold it
//! back through itself. With a clearance set, every iteration is checked
//! afterwards, and a step that brings two links closer than the clearance is
//! cut back by blending the joint angles towards the previous pose. When
//! that leaves the arm stuck against itself, it reroutes: it swings over to
//! an evenly curled arc, bent the other way, whose tip lies on the target.
//! An arc that turns less than a full circle never touches itself.

use crate::collision::methods::{closest_between_segments, closest_on_segment};
use crate::collision::probe::Silent;
use crate::vec2::Vec2;

/// Distance in px at which the end effector counts as on target.
pub const TOLERANCE: f32 = 0.25;

/// Least progress (px) per iteration in contact that isn't counted as stuck.
const STUCK_GAIN: f32 = 0.05;

/// Fraction of the remaining way a rerouting arm moves per iteration.
const REROUTE_RATE: f32 = 0.25;

/// Largest joint angle difference (rad) at which a rerouting arm has arrived.
const ARRIVED: f32 = 0.05;

/// Largest joint turn per Jacobian iteration, in radians. The Jacobian only
/// describes small motions; bigger steps overshoot.
const MAX_TURN: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Solver {
    Fabrik,
    Ccd,
    JacobianTranspose,
    DampedLeastSquares,
}

impl Solver {
    pub const ALL: [Self; 4] = [
        Self::Fabrik,
        Self::Ccd,
        Self::JacobianTranspose,
        Self::DampedLeastSquares,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Fabrik => "fabrik",
            Self::Ccd => "ccd",
            Self::JacobianTranspose => "jacobian-transpose",
            Self::DampedLeastSquares => "dls",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// What a call to [`Chain::solve`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub iterations: u32,
    /// Whether a step was cut back to keep the links apart.
    pub contact: bool,
    /// Whether the arm is swinging round to get unstuck.
    pub rerouting: bool,
}

/// An arm on its way to an arc pose after getting stuck.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reroute {
    /// +1 or −1: which way the arc bends.
    curl: f32,
    /// Whether the other curl was tried already.
    flipped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub joints: Vec<Vec2>,
    pub lengths: Vec<f32>,
    reroute: Option<Reroute>,
    /// Target and error where the arm last got stuck, so it doesn't reroute
    /// again and again for a target it can't reach without crossing.
    stuck_at: Option<(Vec2, f32)>,
}

impl Chain {
    /// A straight chain of `segments` segments, each `length` px, from `base` along `dir`.
    pub fn straight(base: Vec2, segments: usize, length: f32, dir: Vec2) -> Self {
        let u = direction(dir);
        Self {
            joints: (0..=segments)
                .map(|i| base + u * (length * i as f32))
                .collect(),
            lengths: vec![length; segments],
            reroute: None,
            stuck_at: None,
        }
    }

    pub fn base(&self) -> Vec2 {
        self.joints[0]
    }

    pub fn end(&self) -> Vec2 {
        self.joints[self.joints.len() - 1]
    }

    /// Distance from the base to the end effector when fully stretched.
    pub fn reach(&self) -> f32 {
        self.lengths.iter().sum()
    }

    pub fn error(&self, target: Vec2) -> f32 {
        (target - self.end()).length()
    }

    pub fn translate(&mut self, d: Vec2) {
        for p in &mut self.joints {
            *p += d;
        }
    }

    /// Changes the segment count and length. Remaining segments keep their
    /// direction; added ones continue the last.
    pub fn resize(&mut self, segments: usize, length: f32) {
        let mut dirs: Vec<Vec2> = self
            .joints
            .windows(2)
            .map(|w| direction(w[1] - w[0]))
            .collect();
        let last = dirs.last().copied().unwrap_or(Vec2::new(1.0, 0.0));
        dirs.resize(segments, last);
        self.lengths = vec![length; segments];
        self.joints.truncate(1);
        for u in dirs {
            let p = self.end() + u * length;
            self.joints.push(p);
        }
        self.reroute = None;
        self.stuck_at = None;
    }

    /// Runs up to `iterations` iterations of `solver` towards `target`, stopping
    /// early once within [`TOLERANCE`] or when an iteration no longer helps.
    /// `damping` (px) is the damped least-squares λ. With a `clearance` (px),
    /// links are kept at least that far apart.
    pub fn solve(
        &mut self,
        solver: Solver,
        target: Vec2,
        iterations: u32,
        damping: f32,
        clearance: Option<f32>,
    ) -> Outcome {
        let mut out = Outcome::default();
        if self.lengths.is_empty() {
            return out;
        }
        if clearance.is_none() {
            self.reroute = None;
        }
        // A pose that already overlaps (clearance just switched on, or the arm
        // resized) untangles by rerouting, unchecked until the links clear.
        if let Some(c) = clearance
            && self.reroute.is_none()
            && self.overlaps(c)
        {
            self.start_reroute();
        }
        let mut error = self.error(target);
        while out.iterations < iterations && (error > TOLERANCE || self.reroute.is_some()) {
            out.iterations += 1;
            let clearance = clearance.filter(|&c| !self.overlaps(c));
            if let Some(r) = self.reroute {
                self.reroute(r, target, clearance);
                error = self.error(target);
                continue;
            }
            let before = self.joints.clone();
            match solver {
                Solver::Fabrik => self.fabrik(target),
                Solver::Ccd => self.ccd(target),
                Solver::JacobianTranspose => self.jacobian(target, None),
                Solver::DampedLeastSquares => self.jacobian(target, Some(damping)),
            }
            let mut contact = false;
            if let Some(c) = clearance
                && self.overlaps(c)
            {
                contact = true;
                out.contact = true;
                let to = bends(&self.joints);
                self.blend(&before, &to, 0.5, clearance, |ch| ch.error(target) < error);
            }
            let next = self.error(target);
            let gain = error - next;
            error = next;
            if contact && gain < STUCK_GAIN && self.worth_rerouting(target, next) {
                self.stuck_at = Some((target, next));
                self.start_reroute();
            } else if gain < 1e-3 {
                break;
            }
        }
        out.rerouting = self.reroute.is_some();
        out
    }

    /// Starts rerouting, bent the other way than the arm is curled now.
    fn start_reroute(&mut self) {
        let curl: f32 = bends(&self.joints)[1..].iter().sum();
        self.reroute = Some(Reroute {
            curl: if curl > 0.0 { -1.0 } else { 1.0 },
            flipped: false,
        });
    }

    /// Whether a stuck arm should reroute: not if it already tried for about
    /// the same target and got no closer.
    fn worth_rerouting(&self, target: Vec2, error: f32) -> bool {
        let link = self.lengths[0];
        self.stuck_at
            .is_none_or(|(t, e)| (target - t).length() > link || error < e - link * 0.5)
    }

    /// One rerouting step towards the arc for the current target. If the way
    /// there is blocked, tries the other curl, then gives up.
    fn reroute(&mut self, r: Reroute, target: Vec2, clearance: Option<f32>) {
        let before = self.joints.clone();
        let to = self.arc(r.curl, target - self.base());
        if self.blend(&before, &to, REROUTE_RATE, clearance, |_| true) {
            let arrived = bends(&self.joints)
                .iter()
                .zip(&to)
                .all(|(a, b)| wrap(b - a).abs() < ARRIVED);
            if arrived {
                self.reroute = None;
            }
        } else if !r.flipped {
            self.reroute = Some(Reroute {
                curl: -r.curl,
                flipped: true,
            });
        } else {
            self.reroute = None;
        }
    }

    /// Joint angles (see [`bends`]) of an evenly bent arc with its tip at `d`
    /// from the base, or as close as an arc short of a full circle gets.
    fn arc(&self, curl: f32, d: Vec2) -> Vec<f32> {
        let n = self.lengths.len();
        // Base-to-tip vector of the arc with every joint bent by `b`.
        let chord = |b: f32| {
            let mut angle = 0.0;
            let mut e = Vec2::default();
            for (i, &l) in self.lengths.iter().enumerate() {
                if i > 0 {
                    angle += b;
                }
                e += Vec2::new(angle.cos(), angle.sin()) * l;
            }
            e
        };
        // The chord shrinks as the bend grows; stop short of closing the circle.
        let reach = d.length();
        let (mut lo, mut hi) = (0.0, std::f32::consts::TAU * 0.9 / n as f32);
        for _ in 0..30 {
            let mid = (lo + hi) * 0.5;
            if chord(mid * curl).length() > reach {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let b = lo * curl;
        let e = chord(b);
        let mut to = vec![b; n];
        to[0] = d.y.atan2(d.x) - e.y.atan2(e.x);
        to
    }

    /// Moves the pose a fraction `k` of the way towards `goal` in joint angles,
    /// keeping the links `clearance` apart. Used to ease a shown pose after
    /// the solved one.
    pub fn follow(&mut self, goal: &Chain, k: f32, clearance: Option<f32>) {
        if self.lengths != goal.lengths || self.base() != goal.base() {
            self.joints.clone_from(&goal.joints);
            self.lengths.clone_from(&goal.lengths);
            return;
        }
        let clearance = clearance.filter(|&c| !self.overlaps(c));
        let before = self.joints.clone();
        self.blend(&before, &bends(&goal.joints), k.min(1.0), clearance, |_| {
            true
        });
    }

    /// Whether any two links come closer than `clearance`. Neighbors share a
    /// joint, so for them only folding back counts: either far end coming
    /// within `clearance` of the other link.
    pub fn overlaps(&self, clearance: f32) -> bool {
        let p = &self.joints;
        let n = self.lengths.len();
        let near = |a: Vec2, b: Vec2| (a - b).length_squared() < clearance * clearance;
        (0..n).any(|i| {
            (i + 1..n).any(|j| {
                if j == i + 1 {
                    near(
                        p[i],
                        closest_on_segment(p[i], p[j], p[j + 1], &mut Silent).0,
                    ) || near(
                        p[j + 1],
                        closest_on_segment(p[j + 1], p[i], p[i + 1], &mut Silent).0,
                    )
                } else {
                    let (a, b) =
                        closest_between_segments(p[i], p[i + 1], p[j], p[j + 1], &mut Silent);
                    near(a, b)
                }
            })
        })
    }

    /// Rebuilds the pose from `from` with its joint angles (see [`bends`])
    /// moved a fraction `s` towards `to`, halving `s` until the links clear
    /// `clearance` and `accept` holds. Keeps `from` and returns false if
    /// nothing works.
    fn blend(
        &mut self,
        from: &[Vec2],
        to: &[f32],
        mut s: f32,
        clearance: Option<f32>,
        accept: impl Fn(&Self) -> bool,
    ) -> bool {
        let start = bends(from);
        for _ in 0..8 {
            let mut angle = 0.0;
            self.joints[0] = from[0];
            for (i, (a, b)) in start.iter().zip(to).enumerate() {
                angle += a + wrap(b - a) * s;
                self.joints[i + 1] =
                    self.joints[i] + Vec2::new(angle.cos(), angle.sin()) * self.lengths[i];
            }
            if clearance.is_none_or(|c| !self.overlaps(c)) && accept(self) {
                return true;
            }
            s *= 0.5;
        }
        self.joints.copy_from_slice(from);
        false
    }

    fn fabrik(&mut self, target: Vec2) {
        let n = self.lengths.len();
        let base = self.base();

        // Out of reach: the best pose is the chain stretched towards the target.
        if (target - base).length() >= self.reach() {
            let u = direction(target - base);
            for i in 0..n {
                self.joints[i + 1] = self.joints[i] + u * self.lengths[i];
            }
            return;
        }

        // Backward pass: pin the tip to the target and pull each joint after it.
        self.joints[n] = target;
        for i in (0..n).rev() {
            let u = direction(self.joints[i] - self.joints[i + 1]);
            self.joints[i] = self.joints[i + 1] + u * self.lengths[i];
        }
        // Forward pass: pin the root back to the base and pull outwards.
        self.joints[0] = base;
        for i in 0..n {
            let u = direction(self.joints[i + 1] - self.joints[i]);
            self.joints[i + 1] = self.joints[i] + u * self.lengths[i];
        }
    }

    fn ccd(&mut self, target: Vec2) {
        for i in (0..self.lengths.len()).rev() {
            let pivot = self.joints[i];
            let to_end = self.end() - pivot;
            let to_target = target - pivot;
            let angle = to_end.cross(to_target).atan2(to_end.dot(to_target));
            let (sin, cos) = angle.sin_cos();
            for p in &mut self.joints[i + 1..] {
                let r = *p - pivot;
                *p = pivot + Vec2::new(r.x * cos - r.y * sin, r.x * sin + r.y * cos);
            }
        }
    }

    /// One Jacobian step: along the transpose when `damping` is `None`, else
    /// along the damped least-squares solution `Jᵀ (J Jᵀ + λ² I)⁻¹ e`.
    fn jacobian(&mut self, target: Vec2, damping: Option<f32>) {
        let n = self.lengths.len();
        let end = self.end();
        let e = target - end;

        // Column i: how the end effector moves per radian at joint i.
        let cols: Vec<Vec2> = self.joints[..n].iter().map(|&p| (end - p).perp()).collect();

        let mut turns: Vec<f32> = match damping {
            None => {
                // Step α Jᵀ e, with α minimizing |e − α J Jᵀ e|.
                let grad: Vec<f32> = cols.iter().map(|c| c.dot(e)).collect();
                let moved = cols
                    .iter()
                    .zip(&grad)
                    .fold(Vec2::default(), |acc, (c, g)| acc + *c * *g);
                let denom = moved.length_squared();
                if denom < 1e-12 {
                    return;
                }
                let alpha = e.dot(moved) / denom;
                grad.iter().map(|g| alpha * g).collect()
            }
            Some(lambda) => {
                // J Jᵀ + λ² I is a symmetric 2×2 matrix [a b; b d].
                let l2 = lambda * lambda;
                let (mut a, mut b, mut d) = (l2, 0.0, l2);
                for c in &cols {
                    a += c.x * c.x;
                    b += c.x * c.y;
                    d += c.y * c.y;
                }
                let det = a * d - b * b;
                if det.abs() < 1e-12 {
                    return;
                }
                let f = Vec2::new((d * e.x - b * e.y) / det, (a * e.y - b * e.x) / det);
                cols.iter().map(|c| c.dot(f)).collect()
            }
        };

        let largest = turns.iter().fold(0.0f32, |m, t| m.max(t.abs()));
        if largest > MAX_TURN {
            let k = MAX_TURN / largest;
            turns.iter_mut().for_each(|t| *t *= k);
        }

        // The linear model ignores that turning also curls the chain in, so
        // a full step can land further away (e.g. reaching for an
        // out-of-reach target). Halve it until it helps, or give up.
        let before = self.joints.clone();
        let error = e.length();
        let mut scale = 1.0;
        for _ in 0..8 {
            self.turn(&before, &turns, scale);
            if self.error(target) < error {
                return;
            }
            scale *= 0.5;
        }
        self.joints = before;
    }

    /// Rebuilds the chain from the pose `from`, with joint i turned by
    /// `turns[i] * scale`. Turning joint i turns every segment after it.
    fn turn(&mut self, from: &[Vec2], turns: &[f32], scale: f32) {
        let mut angle = 0.0;
        for (i, (w, turn)) in from.windows(2).zip(turns).enumerate() {
            let s = w[1] - w[0];
            angle += turn * scale;
            let (sin, cos) = angle.sin_cos();
            self.joints[i + 1] =
                self.joints[i] + Vec2::new(s.x * cos - s.y * sin, s.x * sin + s.y * cos);
        }
    }
}

/// The pose as angles: the first link's direction, then each joint's bend.
fn bends(joints: &[Vec2]) -> Vec<f32> {
    let mut last = 0.0;
    joints
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            let angle = d.y.atan2(d.x);
            let bend = wrap(angle - last);
            last = angle;
            bend
        })
        .collect()
}

/// `a` wrapped into (−π, π].
fn wrap(a: f32) -> f32 {
    a.sin().atan2(a.cos())
}

/// Unit vector along `v`, or +x when `v` is (nearly) zero.
fn direction(v: Vec2) -> Vec2 {
    let len = v.length();
    if len > 1e-6 {
        v / len
    } else {
        Vec2::new(1.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five 40 px segments, bent so no solver starts at a singularity.
    fn bent() -> Chain {
        let mut c = Chain::straight(Vec2::default(), 5, 40.0, Vec2::new(1.0, 0.0));
        c.solve(Solver::Fabrik, Vec2::new(60.0, 90.0), 50, 0.0, None);
        c
    }

    fn lengths_kept(c: &Chain) -> bool {
        c.joints
            .windows(2)
            .zip(&c.lengths)
            .all(|(w, &l)| ((w[1] - w[0]).length() - l).abs() < 1e-2)
    }

    #[test]
    fn every_solver_reaches_a_reachable_target() {
        let target = Vec2::new(-80.0, 120.0);
        for solver in Solver::ALL {
            let mut c = bent();
            for _ in 0..20 {
                c.solve(solver, target, 100, 10.0, None);
            }
            assert!(
                c.error(target) < 1.0,
                "{solver:?}: error {}",
                c.error(target)
            );
            assert!(lengths_kept(&c), "{solver:?} changed segment lengths");
            assert_eq!(c.base(), Vec2::default(), "{solver:?} moved the base");
        }
    }

    #[test]
    fn out_of_reach_stretches_towards_the_target() {
        let target = Vec2::new(0.0, 500.0);
        for solver in Solver::ALL {
            let mut c = bent();
            for _ in 0..20 {
                c.solve(solver, target, 100, 10.0, None);
            }
            let e = c.error(target);
            assert!((e - 300.0).abs() < 1.0, "{solver:?}: error {e}");
            // Near straight, bending changes the error only to second order,
            // so the angle solvers stall a little short of the line.
            for p in &c.joints {
                assert!(p.x.abs() < 2.0, "{solver:?}: joint {p:?} off the line");
            }
        }
    }

    /// Targets that pull the arm back across its own base.
    const TANGLED: [Vec2; 3] = [
        Vec2::new(-10.0, -20.0),
        Vec2::new(30.0, -40.0),
        Vec2::new(-60.0, 10.0),
    ];

    #[test]
    fn unconstrained_solvers_cross_the_arm_over_itself() {
        let crossed = Solver::ALL.iter().any(|&solver| {
            let mut c = bent();
            TANGLED.iter().any(|&t| {
                (0..30).any(|_| {
                    c.solve(solver, t, 10, 10.0, None);
                    c.overlaps(0.5)
                })
            })
        });
        assert!(crossed, "test targets don't tangle any solver");
    }

    #[test]
    fn clearance_keeps_the_links_apart() {
        for solver in Solver::ALL {
            let mut c = bent();
            for &t in &TANGLED {
                for _ in 0..30 {
                    c.solve(solver, t, 10, 10.0, Some(14.0));
                    assert!(
                        !c.overlaps(14.0),
                        "{solver:?} overlapped reaching for {t:?}"
                    );
                }
            }
            assert!(lengths_kept(&c), "{solver:?} changed segment lengths");
            assert_eq!(c.base(), Vec2::default(), "{solver:?} moved the base");
        }
    }

    #[test]
    fn clearance_still_reaches_open_targets() {
        let target = Vec2::new(-80.0, 120.0);
        for solver in Solver::ALL {
            let mut c = bent();
            for _ in 0..20 {
                c.solve(solver, target, 100, 10.0, Some(14.0));
            }
            assert!(
                c.error(target) < 1.0,
                "{solver:?}: error {}",
                c.error(target)
            );
        }
    }

    #[test]
    fn a_stuck_arm_retracts_and_reaches_round_the_other_side() {
        // Curl the arm down its left side to near the base, then drag the
        // target across to the right: the curl now blocks the direct way.
        let keys = [
            Vec2::new(0.0, -200.0),
            Vec2::new(-120.0, -80.0),
            Vec2::new(-50.0, -15.0),
            Vec2::new(60.0, -15.0),
            Vec2::new(90.0, -30.0),
        ];
        let path = keys
            .windows(2)
            .flat_map(|w| (0..40).map(move |i| w[0].lerp(w[1], i as f32 / 40.0)));
        let target = keys[keys.len() - 1];
        for solver in Solver::ALL {
            let mut c = Chain::straight(Vec2::default(), 6, 45.0, Vec2::new(0.0, -1.0));
            for t in path.clone().chain(std::iter::repeat_n(target, 120)) {
                c.solve(solver, t, 10, 20.0, Some(16.0));
                assert!(
                    !c.overlaps(16.0),
                    "{solver:?} overlapped reaching for {t:?}"
                );
            }
            assert!(
                c.error(target) < 1.0,
                "{solver:?}: error {}",
                c.error(target)
            );
        }
    }

    #[test]
    fn turning_clearance_on_untangles_a_crossed_arm() {
        let mut untangled = Vec::new();
        for solver in Solver::ALL {
            let mut c = bent();
            let mut crossed = None;
            'tangle: for &t in &TANGLED {
                for _ in 0..30 {
                    c.solve(solver, t, 10, 10.0, None);
                    if c.overlaps(14.0) {
                        crossed = Some(t);
                        break 'tangle;
                    }
                }
            }
            let Some(t) = crossed else { continue };
            for _ in 0..120 {
                c.solve(solver, t, 10, 10.0, Some(14.0));
            }
            assert!(!c.overlaps(14.0), "{solver:?} stayed tangled");
            assert!(lengths_kept(&c), "{solver:?} changed segment lengths");
            untangled.push(solver);
        }
        assert!(
            !untangled.is_empty(),
            "test targets don't tangle any solver"
        );
    }

    #[test]
    fn follow_eases_towards_the_goal() {
        let goal = bent();
        let mut shown = Chain::straight(Vec2::default(), 5, 40.0, Vec2::new(1.0, 0.0));
        let start = (shown.end() - goal.end()).length();
        shown.follow(&goal, 0.5, None);
        let half = (shown.end() - goal.end()).length();
        assert!(half > 1.0 && half < start, "{start} -> {half}");
        for _ in 0..40 {
            shown.follow(&goal, 0.5, None);
        }
        assert!((shown.end() - goal.end()).length() < 1e-2);
        assert!(lengths_kept(&shown));
    }

    #[test]
    fn stops_once_on_target() {
        let mut c = bent();
        let target = c.end();
        assert_eq!(c.solve(Solver::Ccd, target, 100, 10.0, None).iterations, 0);
    }

    #[test]
    fn resize_keeps_the_remaining_directions() {
        let mut c = bent();
        let first = direction(c.joints[1] - c.joints[0]);
        let last = direction(c.joints[5] - c.joints[4]);
        c.resize(7, 25.0);
        assert_eq!(c.joints.len(), 8);
        assert!(lengths_kept(&c));
        assert!((direction(c.joints[1] - c.joints[0]) - first).length() < 1e-4);
        assert!((direction(c.joints[7] - c.joints[6]) - last).length() < 1e-4);
    }
}
