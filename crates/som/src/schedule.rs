//! How the learning rate α and the neighborhood radius σ shrink over
//! training, and the neighborhood kernel σ shapes.

/// How a scheduled value travels from its start to its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decay {
    /// `start · (end / start)^(t / T)`: the same factor every step.
    Exponential,
    /// `start + (end − start) · t / T`.
    Linear,
}

impl Decay {
    /// The decay for a UI id, `exp` or `linear`; `None` for anything else.
    pub fn parse(id: &str) -> Option<Decay> {
        match id {
            "exp" => Some(Decay::Exponential),
            "linear" => Some(Decay::Linear),
            _ => None,
        }
    }
}

/// α and σ from their start to their end values over `steps` presented
/// samples, then held at the end values.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    /// Samples over which α and σ decay.
    pub steps: u64,
    /// Learning rate α at the start and the end, each in (0, 1].
    pub alpha: (f32, f32),
    /// Neighborhood radius σ at the start and the end, in lattice units.
    pub sigma: (f32, f32),
    /// The shape of the decay from start to end.
    pub decay: Decay,
    /// Never finish: once decayed, keep learning at the end values, so the
    /// map follows a density that changes.
    pub continuous: bool,
}

impl Schedule {
    /// All values finite and positive, α at most 1, and at least one step.
    /// An end above its start is allowed: it grows instead.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f32| v.is_finite() && v > 0.0;
        let fraction = |v: f32| ok(v) && v <= 1.0;
        self.steps >= 1
            && fraction(self.alpha.0)
            && fraction(self.alpha.1)
            && ok(self.sigma.0)
            && ok(self.sigma.1)
    }

    /// (α, σ) after `t` presented samples.
    pub fn at(&self, t: u64) -> (f32, f32) {
        let f = (t.min(self.steps) as f64 / self.steps as f64) as f32;
        let lerp = |(start, end): (f32, f32)| match self.decay {
            Decay::Exponential => start * (end / start).powf(f),
            Decay::Linear => start + (end - start) * f,
        };
        (lerp(self.alpha), lerp(self.sigma))
    }

    /// Whether training is over after `t` presented samples; never for a continuous schedule.
    pub fn finished(&self, t: u64) -> bool {
        !self.continuous && t >= self.steps
    }
}

/// The Gaussian neighborhood kernel: 1 at the best-matching unit, falling
/// off with squared lattice distance `d2`.
pub fn kernel(d2: f32, sigma: f32) -> f32 {
    (-d2 / (2.0 * sigma * sigma)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(decay: Decay) -> Schedule {
        Schedule {
            steps: 1000,
            alpha: (0.5, 0.01),
            sigma: (6.0, 0.5),
            decay,
            continuous: false,
        }
    }

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn exponential_hits_both_ends() {
        let s = schedule(Decay::Exponential);
        assert!(close(s.at(0), (0.5, 6.0)));
        assert!(close(s.at(1000), (0.01, 0.5)));
        assert!(close(s.at(2000), (0.01, 0.5)));
        // Halfway is the geometric mean.
        assert!(close(s.at(500), ((0.5f32 * 0.01).sqrt(), 3.0f32.sqrt())));
        assert!(!s.finished(999) && s.finished(1000));
    }

    #[test]
    fn linear_is_halfway_at_half_time() {
        let s = schedule(Decay::Linear);
        assert!(close(s.at(500), (0.255, 3.25)));
    }

    #[test]
    fn continuous_never_finishes() {
        let s = Schedule {
            continuous: true,
            ..schedule(Decay::Exponential)
        };
        assert!(!s.finished(1_000_000));
        assert!(close(s.at(1_000_000), (0.01, 0.5)));
    }

    #[test]
    fn invalid_schedules_are_refused() {
        assert!(schedule(Decay::Linear).is_valid());
        let base = schedule(Decay::Linear);
        assert!(
            !Schedule {
                alpha: (f32::NAN, 0.1),
                ..base.clone()
            }
            .is_valid()
        );
        assert!(
            !Schedule {
                sigma: (0.0, 0.1),
                ..base.clone()
            }
            .is_valid()
        );
        assert!(
            !Schedule {
                sigma: (1.0, f32::INFINITY),
                ..base.clone()
            }
            .is_valid()
        );
        // α is a fraction of the way to the sample; above 1 overshoots.
        assert!(
            !Schedule {
                alpha: (1.5, 0.1),
                ..base.clone()
            }
            .is_valid()
        );
        assert!(!Schedule { steps: 0, ..base }.is_valid());
    }

    #[test]
    fn decays_parse_from_their_ids() {
        assert_eq!(Decay::parse("exp"), Some(Decay::Exponential));
        assert_eq!(Decay::parse("linear"), Some(Decay::Linear));
        assert_eq!(Decay::parse("x"), None);
    }

    #[test]
    fn kernel_is_one_at_the_center_and_falls_off() {
        assert_eq!(kernel(0.0, 2.0), 1.0);
        assert!((kernel(4.0, 2.0) - (-0.5f32).exp()).abs() < 1e-6);
    }
}
