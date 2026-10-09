//! Deterministic xorshift64 RNG with splitmix64 seeding.

/// Small deterministic RNG used for every random draw in the crate.
///
/// Two generators built from the same `(seed, stream)` pair produce identical
/// sequences on every platform.
#[derive(Clone, Debug)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    /// Generator for `seed` on stream 0.
    pub fn new(seed: i32) -> Self {
        Self::from_parts(seed, 0)
    }

    /// Generator for `seed` on an independent `stream` (e.g. a node index), so
    /// draws for one stream never shift draws on another.
    pub fn from_parts(seed: i32, stream: u32) -> Self {
        let mixed = splitmix64((seed as u32 as u64) | ((stream as u64 + 1) << 32));
        Self {
            state: if mixed == 0 { 1 } else { mixed },
        }
    }

    /// Next raw 64-bit value (xorshift64).
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform `f32` in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform `f32` in `[min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_deterministic() {
        let mut a = SeededRng::new(1234);
        let mut b = SeededRng::new(1234);
        for _ in 0..16 {
            assert_eq!(a.next_f32(), b.next_f32());
        }
    }

    #[test]
    fn streams_differ() {
        let mut a = SeededRng::from_parts(1, 0);
        let mut b = SeededRng::from_parts(1, 1);
        let all_equal = (0..8).all(|_| a.next_f32() == b.next_f32());
        assert!(!all_equal);
    }

    #[test]
    fn next_f32_in_unit_interval() {
        let mut rng = SeededRng::new(7);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "{v} out of [0,1)");
        }
    }
}
