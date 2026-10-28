//! Small deterministic random number generator (xorshift64*).
//! The same seed always plays out the same trail, which makes bugs repeatable.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// The generator's whole state, for saved games.
    pub fn state(&self) -> u64 {
        self.0
    }

    pub fn from_state(state: u64) -> Self {
        Self(state | 1)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// 0..n (0 when n is 0).
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { self.next_u32() % n }
    }

    /// lo..=hi
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            lo
        } else {
            lo + self.below((hi - lo + 1) as u32) as i32
        }
    }

    /// True with probability `permille`/1000.
    pub fn chance(&mut self, permille: u32) -> bool {
        self.below(1000) < permille
    }

    /// Index chosen by weight; None when all weights are zero.
    pub fn weighted(&mut self, weights: &[u32]) -> Option<usize> {
        let total: u32 = weights.iter().sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.below(total);
        for (i, &w) in weights.iter().enumerate() {
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        None
    }
}
