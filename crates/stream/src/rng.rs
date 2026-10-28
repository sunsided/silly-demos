//! A small seeded random number generator, so streams repeat exactly.

/// PCG32 (XSH RR): small, fast and good enough for sampling.
#[derive(Clone, Debug)]
pub struct Rng {
    /// The generator's 64-bit state.
    state: u64,
}

/// The LCG multiplier of PCG32.
const MULTIPLIER: u64 = 6364136223846793005;
/// The LCG increment of PCG32.
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

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in 0..n; `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u32() as u64 * n as u64) >> 32) as usize
    }

    /// A standard normal value (Box–Muller, one of the pair).
    pub fn normal(&mut self) -> f32 {
        let u = 1.0 - self.next_f32();
        let v = self.next_f32();
        (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        assert!((0..100).all(|_| a.next_u32() == b.next_u32()));
    }

    #[test]
    fn normals_have_unit_spread() {
        let mut rng = Rng::new(3);
        let xs: Vec<f32> = (0..20_000).map(|_| rng.normal()).collect();
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / xs.len() as f32;
        assert!(
            mean.abs() < 0.03 && (var - 1.0).abs() < 0.05,
            "{mean} {var}"
        );
    }
}
