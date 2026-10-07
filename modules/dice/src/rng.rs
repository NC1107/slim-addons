//! A deterministic splitmix64 counter, seeded from the run's `entropy` and the
//! request's input bytes.
//!
//! The host ABI forbids any wasm import, so there is no OS randomness source
//! here. The host hands each run a fresh `entropy` string instead, and the seed
//! is an FNV-1a hash of it and the input, so a roll varies from run to run. With
//! no entropy (a host older than the field) the seed is the input alone, as it
//! was before. `next_u64` advances a splitmix64 counter from the seed, the same
//! construction as code-exec's `getrandom` shim.

pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Seeds the generator by folding `entropy`, a separator and `input`
    /// through FNV-1a into a u64. An empty `entropy` folds `input` alone.
    pub fn seeded(entropy: &[u8], input: &[u8]) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut fold = |bytes: &[u8]| {
            for &byte in bytes {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
        };
        if !entropy.is_empty() {
            fold(entropy);
            fold(&[0]);
        }
        fold(input);
        Self { state: hash }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a value in `1..=sides`.
    pub fn roll(&mut self, sides: u32) -> u32 {
        1 + (self.next_u64() % sides as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(entropy: &str, input: &str) -> u64 {
        SplitMix64::seeded(entropy.as_bytes(), input.as_bytes()).next_u64()
    }

    #[test]
    fn the_same_entropy_and_input_roll_the_same() {
        assert_eq!(first("abc", "d20"), first("abc", "d20"));
    }

    #[test]
    fn different_entropy_changes_the_roll() {
        let seen: std::collections::BTreeSet<u64> =
            (0..50).map(|i| first(&format!("run-{i}"), "d20")).collect();
        assert_eq!(seen.len(), 50);
    }

    #[test]
    fn no_entropy_folds_the_input_alone_as_before() {
        assert_eq!(first("", "d20"), first("", "d20"));
        assert_ne!(first("", "d20"), first("x", "d20"));
    }

    #[test]
    fn entropy_and_input_cannot_trade_bytes() {
        assert_ne!(first("ab", "c"), first("a", "bc"));
    }
}
