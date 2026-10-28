//! Brent's methods for one unknown: a zero of a function in a bracket, and a minimum of a function
//! on an interval, with function values only.
//!
//! What it does: [`zero`] narrows a bracket `[a, b]` across which `f` changes sign, by bisection,
//! the secant step and inverse quadratic interpolation, taking an interpolated step only while it
//! stays well inside the bracket and keeps shrinking it fast enough, so it converges as surely as
//! bisection and usually much faster. [`minimum`] narrows an interval holding a minimum of `f` by
//! golden-section steps and successive parabolic interpolation, under the same safeguards. Chapter
//! 12.4 asks for exactly this in one unknown ("bracketing in the manner of Brent (1973)"); a caller
//! that has a derivative takes a Newton step only inside the bracket this returns.
//!
//! Guarantees:
//!
//! - **The bracket is kept.** [`zero`] returns `x` with `f(x) = 0` exactly, or `x` and `other`
//!   with `f(x)` and `f(other)` of opposite signs and |x − other| at most 4u|x| + `tol`
//!   (u = 2^−53): a sign change of `f` as computed lies between them, whatever the rounding inside
//!   `f`.
//! - **The interval is kept.** [`minimum`] evaluates `f` only inside `(a, b)`, never closer than
//!   its tolerance to either end, and returns the point with the least value it evaluated; on a
//!   function unimodal on `[a, b]` that point is within 2(√u|x| + `tol`/3) of the minimum.
//! - **A cap, never a guess.** Each call evaluates `f` at most `cap` times; reaching the cap is
//!   [`BrentError::CapReached`], never the best point so far. A function error is returned as it
//!   is ([`BrentError::Num`]).
//! - **Never NaN, deterministic.** Every value is a finite [`Real`]; the steps use only IEEE 754
//!   basic operations in a fixed order, so every target computes the same points. An
//!   interpolation whose arithmetic would overflow is replaced by the bisection or golden-section
//!   step, as an unacceptable interpolation is.
//!
//! Method: R. P. Brent, *Algorithms for Minimization without Derivatives*, Prentice-Hall (1973),
//! chapter 4 (procedure `zero`) and chapter 5 (procedure `localmin`),
//! <https://maths-people.anu.edu.au/~brent/pub/pub011.html>, written in our own words from the
//! algorithms as G. E. Forsythe, M. A. Malcolm and C. B. Moler give them in *Computer Methods for
//! Mathematical Computations*, Prentice-Hall (1977), routines `zeroin` and `fmin`. Netlib's copies
//! of those two routines carry no licence file, so they were read only as a description of the
//! method, as chapter 29.4's "Brent 1973" row allows for such sources, for the order of Brent's
//! tests and his stopping rules; no code was taken.
//!
//! Cost: each step is a few dozen floating-point operations and one evaluation of `f`; the number
//! of steps is at most about (log₂((b − a)/δ))² for [`zero`] and of the order of
//! log((b − a)/δ)/log(1/0.618) golden-section steps for [`minimum`] (δ the tolerance), and in any
//! case at most `cap`. No allocation, no recursion.

use crate::error::NumError;
use crate::real::Real;

/// The unit roundoff u = 2^−53, the relative precision Brent's tolerances are stated in.
const U: f64 = 1.0 / 9_007_199_254_740_992.0;

/// √u = 2^−26.5, the relative spacing [`minimum`] keeps between its evaluations (Brent's `eps`
/// for `localmin`): the double nearest to it, 2^−27 · √2 with √2's nearest double, bits
/// `0x3E46_A09E_667F_3BCD`.
const SQRT_U: f64 = f64::from_bits(0x3E46_A09E_667F_3BCD);

/// (3 − √5)/2, the golden-section ratio of [`minimum`]: the double nearest to it, bits
/// `0x3FD8_7221_91A0_2D61`.
const GOLDEN: f64 = f64::from_bits(0x3FD8_7221_91A0_2D61);

/// Why a search could not give its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrentError {
    /// `f(a)` and `f(b)` have the same sign and neither is zero: `[a, b]` is not a bracket.
    SameSign,
    /// `a` is not below `b`, or the tolerance is negative.
    BadInterval,
    /// `f` was evaluated `cap` times without meeting the tolerance.
    CapReached,
    /// The function, or a value it returned, failed.
    Num(NumError),
}

impl From<NumError> for BrentError {
    fn from(e: NumError) -> Self {
        BrentError::Num(e)
    }
}

impl core::fmt::Display for BrentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BrentError::SameSign => f.write_str("f has the same sign at both ends of the interval"),
            BrentError::BadInterval => {
                f.write_str("the interval is empty or reversed, or the tolerance is negative")
            }
            BrentError::CapReached => f.write_str("the evaluation cap was reached"),
            BrentError::Num(e) => e.fmt(f),
        }
    }
}

/// A zero found by [`zero`]: `f(x)` and `f(other)` have opposite signs, or `f(x)` is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Zero {
    /// The end of the final bracket where |f| is the smaller: the zero's approximation.
    pub x: Real,
    /// `f(x)`.
    pub fx: Real,
    /// The other end of the final bracket.
    pub other: Real,
    /// How many times `f` was evaluated.
    pub evaluations: u32,
}

/// A minimum found by [`minimum`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Minimum {
    /// The point with the least value of those evaluated.
    pub x: Real,
    /// `f(x)`.
    pub fx: Real,
    /// How many times `f` was evaluated.
    pub evaluations: u32,
}

/// The evaluations of one search, against its cap.
struct Counter<F> {
    f: F,
    used: u32,
    cap: u32,
}

impl<F: FnMut(Real) -> Result<Real, NumError>> Counter<F> {
    /// `f(x)`, or `CapReached` once `cap` evaluations have been made.
    fn eval(&mut self, x: f64) -> Result<f64, BrentError> {
        if self.used >= self.cap {
            return Err(BrentError::CapReached);
        }
        self.used = self.used.saturating_add(1);
        Ok((self.f)(Real::new(x)?)?.get())
    }
}

/// `v` as a `Real`; every point and value here is finite (the inputs are, and each step is
/// checked), so this fails only through a defect, reported as [`NumError::NotFinite`].
fn real(v: f64) -> Result<Real, BrentError> {
    Ok(Real::new(v)?)
}

/// `x` with the sign of `s`, `s` = 0 counting as positive: Fortran's sign-transfer intrinsic.
fn with_sign(x: f64, s: f64) -> f64 {
    if s >= 0.0 { x.abs() } else { -x.abs() }
}

/// A zero of `f` in the bracket `[a, b]`: a point where `f` is exactly zero, or a bracket of a
/// sign change no wider than 4u|x| + `tol`, by Brent's method (module documentation), evaluating
/// `f` at most `cap` times.
///
/// # Errors
///
/// [`BrentError::BadInterval`] if `a ≥ b` or `tol < 0`; [`BrentError::SameSign`] if `f(a)` and
/// `f(b)` are both non-zero with one sign; [`BrentError::CapReached`] if the tolerance is not met
/// within `cap` evaluations; [`BrentError::Num`] if `f` fails or a point leaves the finite range.
#[allow(
    clippy::many_single_char_names,
    reason = "the names are Brent's: a, b and c the points, d and e the last two steps, p, q, r and s the interpolation"
)]
pub fn zero<F>(f: F, a: Real, b: Real, tol: Real, cap: u32) -> Result<Zero, BrentError>
where
    F: FnMut(Real) -> Result<Real, NumError>,
{
    let (mut a, mut b, tol) = (a.get(), b.get(), tol.get());
    if a >= b || tol < 0.0 {
        return Err(BrentError::BadInterval);
    }
    let mut f = Counter { f, used: 0, cap };
    let mut fa = f.eval(a)?;
    let mut fb = f.eval(b)?;
    if fa == 0.0 {
        return Ok(Zero {
            x: real(a)?,
            fx: real(fa)?,
            other: real(a)?,
            evaluations: f.used,
        });
    }
    if fb != 0.0 && (fa > 0.0) == (fb > 0.0) {
        return Err(BrentError::SameSign);
    }
    // c is the other end of the bracket [b, c]; a is the previous b; d the last step, e the one
    // before it.
    let (mut c, mut fc) = (a, fa);
    let mut d = b - a;
    let mut e = d;
    loop {
        if (fb > 0.0) == (fc > 0.0) && fb != 0.0 {
            // b and c no longer bracket the zero: a, the previous b, does.
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            // Keep b the better end.
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        // Brent's tolerance: 2u|b| + tol/2, so the final bracket is at most 4u|b| + tol wide.
        let tol1 = 2.0 * U * b.abs() + 0.5 * tol;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol1 || fb == 0.0 {
            return Ok(Zero {
                x: real(b)?,
                fx: real(fb)?,
                other: real(c)?,
                evaluations: f.used,
            });
        }
        let mut step = None;
        if e.abs() >= tol1 && fa.abs() > fb.abs() {
            // Interpolate: the secant through a and b, or the inverse quadratic through a, b, c.
            let s = fb / fa;
            let (mut p, mut q) = if a == c {
                (2.0 * xm * s, 1.0 - s)
            } else {
                let q = fa / fc;
                let r = fb / fc;
                (
                    s * (2.0 * xm * q * (q - r) - (b - a) * (r - 1.0)),
                    (q - 1.0) * (r - 1.0) * (s - 1.0),
                )
            };
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            // Accept it only if it falls well inside the bracket and is less than half the step
            // before last. Those two tests also refuse every interpolation whose arithmetic failed,
            // with no test of their own: p ≥ 0 here (or NaN). If q is ±0 the first right-hand side
            // is ±0, and 2p < ±0 is false. If q is infinite, |tol1·q| is infinite or NaN, so the
            // first right-hand side is NaN or −∞ (never +∞: +∞ − ∞ is NaN), and the test is false.
            // If p is +∞ or NaN, or q NaN, every comparison is false. When both tests pass,
            // 2p < 3|xm||q| makes |p/q| < 1.5|xm|, a finite step.
            if 2.0 * p < 3.0 * xm * q - (tol1 * q).abs() && p < (0.5 * e * q).abs() {
                step = Some(p / q);
            }
        }
        if let Some(s) = step {
            e = d;
            d = s;
        } else {
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol1 {
            d
        } else {
            with_sign(tol1, xm)
        };
        fb = f.eval(b)?;
    }
}

/// A minimum of `f` on `[a, b]`, to within 2(√u|x| + `tol`/3) for a unimodal `f`, by Brent's
/// method (module documentation), evaluating `f` at most `cap` times.
///
/// # Errors
///
/// [`BrentError::BadInterval`] if `a ≥ b` or `tol < 0`; [`BrentError::CapReached`] if the
/// tolerance is not met within `cap` evaluations; [`BrentError::Num`] if `f` fails or a point
/// leaves the finite range.
#[allow(
    clippy::many_single_char_names,
    reason = "the names are Brent's: a and b the interval, u, v, w and x the points, d and e the last two steps, p, q and r the parabola"
)]
pub fn minimum<F>(f: F, a: Real, b: Real, tol: Real, cap: u32) -> Result<Minimum, BrentError>
where
    F: FnMut(Real) -> Result<Real, NumError>,
{
    let (mut a, mut b, tol) = (a.get(), b.get(), tol.get());
    if a >= b || tol < 0.0 {
        return Err(BrentError::BadInterval);
    }
    let mut f = Counter { f, used: 0, cap };
    // x: the least value so far; w: the second least; v: the previous w. d is the last step and e
    // the one before it.
    let mut x = a + GOLDEN * (b - a);
    let (mut w, mut v) = (x, x);
    let mut fx = f.eval(x)?;
    let (mut fw, mut fv) = (fx, fx);
    let (mut d, mut e) = (0.0f64, 0.0f64);
    loop {
        let xm = f64::midpoint(a, b);
        let tol1 = SQRT_U * x.abs() + tol / 3.0;
        let tol2 = 2.0 * tol1;
        if (x - xm).abs() <= tol2 - 0.5 * (b - a) {
            return Ok(Minimum {
                x: real(x)?,
                fx: real(fx)?,
                evaluations: f.used,
            });
        }
        let mut parabolic = None;
        if e.abs() > tol1 {
            // The parabola through x, w and v; its vertex is x + p/q.
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            let mut q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            }
            q = q.abs();
            let previous = e;
            e = d;
            // Accept the vertex only if q is finite, the vertex lies inside (a, b) and the step is
            // less than half the step before last. An infinite q must be refused by name: it would
            // pass the three comparisons below with a zero step. The comparisons refuse the rest
            // with no test of their own: q ≥ 0 here, so for q = 0 the first is |p| < 0, false; a p
            // that is ±∞ or NaN fails it against a finite right-hand side. When all pass,
            // |p/q| < |previous|/2, a finite step.
            if q.is_finite()
                && p.abs() < (0.5 * q * previous).abs()
                && p > q * (a - x)
                && p < q * (b - x)
            {
                let s = p / q;
                let u = x + s;
                // Never evaluate f too close to a or b.
                parabolic = Some(if u - a < tol2 || b - u < tol2 {
                    with_sign(tol1, xm - x)
                } else {
                    s
                });
            }
        }
        d = if let Some(s) = parabolic {
            s
        } else {
            // A golden-section step into the larger part.
            e = if x >= xm { a - x } else { b - x };
            GOLDEN * e
        };
        // Never evaluate f closer than tol1 to x.
        let u = if d.abs() >= tol1 {
            x + d
        } else {
            x + with_sign(tol1, d)
        };
        let fu = f.eval(u)?;
        if fu <= fx {
            if u >= x {
                a = x;
            } else {
                b = x;
            }
            v = w;
            fv = fw;
            w = x;
            fw = fx;
            x = u;
            fx = fu;
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || w == x {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || v == x || v == w {
                v = u;
                fv = fu;
            }
        }
    }
}
