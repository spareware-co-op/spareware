//! Plane rotations and the norm of two numbers, for the iterative factorisations.
//!
//! What it does: [`hypot`] gives √(a² + b²) without overflow or underflow; [`givens`] gives the
//! rotation (c, s) and r with c·y + s·z = r and −s·y + c·z = 0, which zeroes z against y; [`rotate`]
//! applies a rotation to a pair of numbers, and [`rotate_slices`] to two vectors element by
//! element.
//!
//! Guarantees:
//!
//! - **No spurious overflow or underflow.** [`hypot`] divides both numbers by the larger magnitude
//!   before squaring, so the squares lie in [0, 1] and the sum in [1, 2].
//! - **Deterministic.** IEEE 754 operations in the order written and the correctly rounded square
//!   root of `geom_num::math`; no fused multiply-add.
//! - **Exact identity when nothing is to be zeroed.** A zero z gives c = 1, s = 0 and r = y, so a
//!   rotation that has nothing to do changes no bit.
//!
//! Method: Givens rotations as Golub and Van Loan compute them (G. H. Golub and C. F. Van Loan,
//! *Matrix Computations*, 4th ed., Johns Hopkins (2013), section 5.1.8 and algorithm 5.1.3), with
//! the norm scaled as LAPACK's `xLAPY2` does (E. Anderson et al., *LAPACK Users' Guide*, 3rd ed.,
//! SIAM (1999), <https://www.netlib.org/lapack/lug/>); written in our own words (chapter 29.4).
//!
//! Cost: constant per call; [`rotate_slices`] is linear in the vectors' length.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the books': a and b for the two numbers, c and s for a rotation, x, y, z, r for what it acts on"
)]

use geom_num::math;

use crate::matrix::{LinalgError, finite};

/// √(a² + b²), scaled so that it neither overflows nor underflows for finite a and b.
pub(crate) fn hypot(a: f64, b: f64) -> Result<f64, LinalgError> {
    let (x, y) = (a.abs(), b.abs());
    let (big, small) = if x >= y { (x, y) } else { (y, x) };
    if big == 0.0 {
        return Ok(0.0);
    }
    let q = small / big;
    finite(big * math::sqrt(1.0 + q * q)?.get())
}

/// 2^−1021: at or above this in either input, r is a normal number.
const TINY: f64 = f64::from_bits(0x0020_0000_0000_0000);
/// 2^100, which takes any double below [`TINY`] exactly to a normal one.
const UP: f64 = f64::from_bits(0x4630_0000_0000_0000);
/// 2^−100, the inverse of [`UP`].
const DOWN: f64 = f64::from_bits(0x39B0_0000_0000_0000);

/// The rotation that zeroes `z` against `y`: (c, s, r) with c·y + s·z = r, −s·y + c·z = 0 and
/// c² + s² = 1 to rounding; (1, 0, y) when z is zero.
pub(crate) fn givens(y: f64, z: f64) -> Result<(f64, f64, f64), LinalgError> {
    if z == 0.0 {
        return Ok((1.0, 0.0, y));
    }
    // Where r would be subnormal it keeps too few bits for y/r and z/r to be a unit pair, and the
    // loss adds up over a run, so such a pair is first scaled by a power of two, which is exact.
    if y.abs() < TINY && z.abs() < TINY {
        let (y, z) = (y * UP, z * UP);
        let r = hypot(y, z)?;
        return Ok((y / r, z / r, r * DOWN));
    }
    let r = hypot(y, z)?;
    Ok((y / r, z / r, r))
}

/// (c·x + s·y, −s·x + c·y).
pub(crate) fn rotate(x: f64, y: f64, c: f64, s: f64) -> (f64, f64) {
    (c * x + s * y, -s * x + c * y)
}

/// [`rotate`] on each pair (`x_k`, `y_k`) of two vectors of one length.
pub(crate) fn rotate_slices(x: &mut [f64], y: &mut [f64], c: f64, s: f64) {
    for (a, b) in x.iter_mut().zip(y.iter_mut()) {
        let (p, q) = rotate(*a, *b, c, s);
        *a = p;
        *b = q;
    }
}
