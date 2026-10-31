//! The exact-input writer: what an oracle is given, so it sees what the kernel saw (chapter 28.10;
//! piece G113).
//!
//! What it does: [`steps`] writes grid steps as the whole numbers an exact oracle takes. [`float`]
//! writes a double as the shortest decimal that reads back to the same bits, and [`read_float`]
//! reads one. [`to_millimetres`] converts grid steps to the unit floating-point oracles are tuned
//! for, and returns the rounding the conversion cost, which the comparison must add to its bound:
//! 1/6400 mm is not a binary fraction, so converting to millimetres rounds.
//!
//! Guarantees:
//!
//! - **Bit for bit.** `read_float(&float(x)?) == x` for every finite double, signed zero included.
//! - **No silent infinity or NaN**: [`float`] and [`read_float`] refuse them.
//! - **The rounding is stated.** [`Millimetres::rounding`] bounds the difference between the double
//!   and the exact value, rounded up.
//!
//! Method: Rust's float formatter writes the shortest digits that round-trip, and its parser is
//! correctly rounded; a step is an exact fraction of a millimetre, from
//! `geom_num::grid::GridStep::quarter_nm` (`projects/engines/geometry/architecture/06-numbers.md`).
//!
//! Cost: constant per number.

use geom_num::GridStep;

/// Quarter-nanometres in a millimetre.
const QUARTER_NM_PER_MM: f64 = 4_000_000.0;

/// Grid steps, for an exact oracle: whole numbers, never rounded.
#[must_use]
pub fn steps(n: i64) -> String {
    n.to_string()
}

/// `x` as the shortest decimal that reads back to its bits; `None` if it is not finite.
#[must_use]
pub fn float(x: f64) -> Option<String> {
    x.is_finite().then(|| format!("{x:e}"))
}

/// A number [`float`] wrote, or any finite decimal; `None` otherwise.
#[must_use]
pub fn read_float(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Grid steps in millimetres, with what the conversion cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Millimetres {
    /// The value, the double nearest the exact one.
    pub value: f64,
    /// An upper bound on the distance from `value` to the exact value.
    pub rounding: f64,
}

/// `n` steps of `step` in millimetres. Two roundings reach the double, the product's and the
/// quotient's, each within half a unit in the last place, so 2·ε·|value| bounds their sum generously:
/// the exact bound (1 + u)² − 1 is below it.
#[must_use]
pub fn to_millimetres(n: i64, step: GridStep) -> Millimetres {
    let product = i128::from(n).saturating_mul(i128::from(step.quarter_nm()));
    // The first of the two roundings the doc comment counts.
    #[allow(
        clippy::cast_precision_loss,
        reason = "the rounding is stated and bounded"
    )]
    let value = product as f64 / QUARTER_NM_PER_MM;
    Millimetres {
        value,
        rounding: (2.0 * f64::EPSILON * value.abs()).next_up(),
    }
}
