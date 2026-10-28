//! Tiny deterministic pseudo-random helpers built on xorshift32.

/// One xorshift32 step. Zero maps to zero, so seeds must be non-zero.
#[inline]
pub const fn hash_u32(mut x: u32) -> u32 {
    // xorshift32
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

/// Advances `state` and returns a uniform value in `[0, 1]` from its low 24 bits.
#[inline]
pub const fn frand01(state: &mut u32) -> f32 {
    *state = hash_u32(*state);
    ((*state as u64 & 0x00FF_FFFF) as f32) / ((0x0100_0000u32 - 1) as f32)
}
