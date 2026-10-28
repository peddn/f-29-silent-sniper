//! Deterministischer Zufallsgenerator (PCG32, XSH-RR).
//!
//! Jedes Subsystem bekommt einen eigenen Strom (`stream`), damit z. B. eine
//! zusätzliche Zufallszahl in der Navigation nicht die späteren
//! Bedrohungs-Würfe verschiebt.

const MULT: u64 = 6_364_136_223_846_793_005;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self { state: 0, inc: (stream << 1) | 1 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Gleichverteilt in [0, 1) mit 53 Bit Auflösung.
    pub fn next_f64(&mut self) -> f64 {
        let hi = u64::from(self.next_u32()) << 21;
        let lo = u64::from(self.next_u32()) >> 11;
        (hi | lo) as f64 * (1.0 / (1_u64 << 53) as f64)
    }

    /// Gleichverteilt in [lo, hi).
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Zustand für Hashes/Savegames.
    pub fn raw_state(&self) -> (u64, u64) {
        (self.state, self.inc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_sequence() {
        // Referenzwerte der PCG32-Referenzimplementierung (pcg32-demo, Seed 42, Stream 54).
        let mut rng = Pcg32::new(42, 54);
        let expected = [0xa15c_02b7_u32, 0x7b47_f409, 0xba1d_3330, 0x83d2_f293, 0xbfa4_784b, 0xcbed_606e];
        for e in expected {
            assert_eq!(rng.next_u32(), e);
        }
    }

    #[test]
    fn f64_range() {
        let mut rng = Pcg32::new(1, 1);
        for _ in 0..10_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn streams_are_independent() {
        let mut a = Pcg32::new(7, 1);
        let mut b = Pcg32::new(7, 2);
        assert_ne!(a.next_u32(), b.next_u32());
    }
}
