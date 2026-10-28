//! A small seeded random number generator, so runs repeat exactly.

/// PCG32 (XSH RR): small, fast and good enough for sampling.
#[derive(Clone, Debug)]
pub struct Rng {
    /// The LCG state; each output is a permuted function of it.
    state: u64,
}

/// The 64-bit LCG multiplier from Knuth's MMIX.
const MULTIPLIER: u64 = 6364136223846793005;
/// The LCG increment, odd so the generator has full period.
const INCREMENT: u64 = 1442695040888963407;

impl Rng {
    /// A generator seeded with `seed`; the same seed gives the same sequence.
    pub fn new(seed: u64) -> Self {
        let mut rng = Self { state: 0 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// The next 32 uniformly random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        // 24 bits fill an f32 mantissa exactly, so the result never rounds up to 1.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in 0..n; `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u32() as u64 * n as u64) >> 32) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            assert_eq!(a.next_f32(), b.next_f32());
        }
        assert_ne!(Rng::new(7).next_f32(), Rng::new(8).next_f32());
    }

    #[test]
    fn next_f32_is_in_the_unit_interval() {
        let mut rng = Rng::new(1);
        for _ in 0..10_000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn below_stays_below() {
        let mut rng = Rng::new(3);
        assert!((0..1000).all(|_| rng.below(7) < 7));
    }
}
