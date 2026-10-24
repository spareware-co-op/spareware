//! Inputs for the sign of an exact integer: [`of_i128`] (`Sign::of_i128`).
//!
//! Class `typical`: negative, zero and positive values in rotation, with `i128::MIN` and
//! `i128::MAX` first, so each of the function's three branches is taken a third of the time.
//!
//! Method: one input per branch of a three-way comparison, at both ends of the type
//! (boundary-value analysis, <https://en.wikipedia.org/wiki/Boundary-value_analysis>).
//!
//! Guarantees: the batch is full, the same on every target, and takes all three branches and
//! both ends of `i128` (`tests/inputs.rs`); every result is checked against an exact sign by
//! `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use super::{Class, batch_of, rng};

/// Values for `Sign::of_i128`.
#[must_use]
pub fn of_i128(class: Class) -> Vec<i128> {
    let mut g = rng("sign::of_i128", class);
    match class {
        Class::Typical => batch_of(|n| {
            Some(match n {
                0 => i128::MIN,
                1 => i128::MAX,
                _ => {
                    let magnitude = i128::from(g.next_u64() >> 1).max(1);
                    match n % 3 {
                        0 => -magnitude,
                        1 => 0,
                        _ => magnitude,
                    }
                }
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
