//! SplitMix64: a small, fully specified PRNG, so a seed reproduces the same
//! simulation on every platform and toolchain. Not for cryptography.

#[derive(Debug, Clone)]
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 random bits (Steele, Lea & Flood 2014; Vigna's reference).
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` for `n > 0` (Lemire's multiply-shift; the bias of
    /// at most n / 2⁶⁴ is irrelevant for simulation). Returns 0 for `n = 0`.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        let wide = u128::from(self.next_u64()) * u128::from(n);
        // The high half of a u64 × u64 product is below n, so it fits.
        u64::try_from(wide >> 64).unwrap_or(0)
    }

    /// Uniform in `-half..=half` for `half >= 0`.
    pub(crate) fn symmetric(&mut self, half: u32) -> i64 {
        let span = u64::from(half) * 2 + 1;
        i64::try_from(self.below(span)).unwrap_or(0) - i64::from(half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_output() {
        // First outputs of the reference implementation for seed 1234567.
        let mut r = SplitMix64::new(1_234_567);
        assert_eq!(r.next_u64(), 6_457_827_717_110_365_317);
        assert_eq!(r.next_u64(), 3_203_168_211_198_807_973);
        assert_eq!(r.next_u64(), 9_817_491_932_198_370_423);
    }

    #[test]
    fn ranges_hold() {
        let mut r = SplitMix64::new(7);
        for _ in 0..10_000 {
            assert!(r.below(10) < 10);
            let s = r.symmetric(3);
            assert!((-3..=3).contains(&s));
        }
        assert_eq!(r.below(0), 0);
        assert_eq!(r.symmetric(0), 0);
    }
}
