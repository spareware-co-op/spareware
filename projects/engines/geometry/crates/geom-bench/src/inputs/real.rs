//! Inputs for finite doubles in total order: [`total_max`], [`total_min`] and [`cmp`] on
//! `Real`.
//!
//! Class `typical`: pairs in rotation of two ordinary values, a value with itself, the two zeros in
//! each order (where totalOrder differs from `<`), and values of opposite sign, so every branch
//! of the comparison is taken.
//!
//! Method: IEEE 754-2019 totalOrder (section 5.10) as Rust's `f64::total_cmp` implements it
//! (<https://doc.rust-lang.org/std/primitive.f64.html#method.total_cmp>); one input per case of
//! the ordering, including the signed zeros.
//!
//! Guarantees: the batch is full, the same on every target, and holds both zeros in both
//! orders and every ordering (`tests/inputs.rs`); every result is checked against
//! `f64::total_cmp` by `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use geom_num::real::Real;

use super::eft::double;
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A pair for a total-order operation, of kind `n` mod 5.
fn pair(g: &mut SplitMix64, n: usize) -> Option<(Real, Real)> {
    let (a, b) = match n % 5 {
        0 => (double(g, -30, 30), double(g, -30, 30)),
        1 => {
            let x = double(g, -30, 30);
            (x, x)
        }
        2 => (0.0, -0.0),
        3 => (-0.0, 0.0),
        _ => {
            let x = double(g, -30, 30);
            (x, -x)
        }
    };
    Some((Real::new(a).ok()?, Real::new(b).ok()?))
}

/// Pairs for `Real::total_max`.
#[must_use]
pub fn total_max(class: Class) -> Vec<(Real, Real)> {
    pairs("real::total_max", class)
}

/// Pairs for `Real::total_min`.
#[must_use]
pub fn total_min(class: Class) -> Vec<(Real, Real)> {
    pairs("real::total_min", class)
}

/// Pairs for `Ord::cmp` on `Real`.
#[must_use]
pub fn cmp(class: Class) -> Vec<(Real, Real)> {
    pairs("real::cmp", class)
}

/// Pairs of class `class` from the seed `op`.
fn pairs(op: &str, class: Class) -> Vec<(Real, Real)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|n| pair(&mut g, n)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
