//! Inputs for root finding and quadrature (piece G51, decision 32.44): brackets for Brent's zero
//! search on a fixed cubic, and intervals for Gauss-Legendre quadrature of a fixed quintic.
//!
//! The functions are fixed per operation ([`cubic`] and
//! [`quintic`]) and use the four basic operations only. Class `typical`
//! only:
//!
//! - Brackets: `a` in [1, 2) and `b` in (2.2, 3], around the cubic's one real zero near 2.0946,
//!   so every search converges.
//! - Intervals: `a` in [−4, 0) and `b` in (0, 4], so every interval has positive length.
//!
//! The searches' and rules' refusals (a bracket without a sign change, a cap reached, an empty
//! interval, a rule not tabulated) are tested in `geom-num` itself (`tests/brent.rs`,
//! `tests/quadrature.rs`).
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the classic test function x³ − 2x − 5 of root finding (Wallis's, used by Brent in
//! *Algorithms for Minimization without Derivatives*, 1973, chapter 4), and a quintic, which an
//! eight-point rule integrates exactly.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: two draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::real::Real;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;
use geom_num::NumError;

/// A value in [0, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 12) as f64;
    v / (1u64 << 52) as f64
}

/// Pairs `(a, b)` with `a` in [`a0`, `a0 + aw`) and `b` in (`b1 − bw`, `b1`], for the operation
/// named `op`.
fn pairs(op: &str, class: Class, (a0, aw): (f64, f64), (b1, bw): (f64, f64)) -> Vec<(Real, Real)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| {
        let a = a0 + aw * unit(&mut g);
        let b = b1 - bw * unit(&mut g);
        Some((Real::new(a).ok()?, Real::new(b).ok()?))
    })
}

/// Brackets for `brent::zero` on [`cubic`].
#[must_use]
pub fn zero(class: Class) -> Vec<(Real, Real)> {
    pairs("brent::zero", class, (1.0, 1.0), (3.0, 0.8))
}

/// Intervals for `quadrature::gauss_legendre` of [`quintic`].
#[must_use]
pub fn gauss_legendre(class: Class) -> Vec<(Real, Real)> {
    pairs("quadrature::gauss_legendre", class, (-4.0, 4.0), (4.0, 4.0))
}

/// x³ − 2x − 5.
///
/// # Errors
///
/// `NotFinite` for a value too large to be a double.
pub fn cubic(x: Real) -> Result<Real, NumError> {
    let x = x.get();
    Real::new((x * x - 2.0) * x - 5.0)
}

/// x⁵ − 3x² + 1.
///
/// # Errors
///
/// `NotFinite` for a value too large to be a double.
pub fn quintic(x: Real) -> Result<Real, NumError> {
    let x = x.get();
    let x2 = x * x;
    Real::new(x2 * x2 * x - 3.0 * x2 + 1.0)
}
