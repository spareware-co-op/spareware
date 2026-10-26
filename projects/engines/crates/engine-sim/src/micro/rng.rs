//! The random source of a run, injected by the caller.
//!
//! The simulation never makes its own random numbers: it asks a [`Rng`] for each one, in a fixed
//! order, so the same source gives the same run. A browser passes a closure over `Math.random`;
//! a test passes [`Mulberry32`], the seeded generator the truth-test harness uses.

/// A source of uniform random numbers in [0, 1), one call at a time (what `Math.random()` gives).
pub trait Rng {
    /// The next number, at least 0 and below 1.
    fn next_f64(&mut self) -> f64;
}

impl<F: FnMut() -> f64> Rng for F {
    fn next_f64(&mut self) -> f64 {
        self()
    }
}

/// The seeded 32-bit generator of the truth-test harness (tools/transport-truth/js-core.mjs, `loadCore`).
///
/// Each call adds 0x6D2B79F5 to the state, mixes it with two multiplies and three xor-shifts and
/// divides the 32-bit result by 2^32, exactly as the harness's `rand` does, so a Rust run replays
/// a recorded JavaScript run number for number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// A generator at the given seed (`seed >>> 0` in the harness).
    pub fn new(seed: u32) -> Self {
        Mulberry32 { state: seed }
    }
}

impl Rng for Mulberry32 {
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}
