//! Fuzz target: the proven maths functions are correctly rounded, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a function (sinpi, cospi,
//! exp, ln or atan2, piece G47) and its argument or arguments. [`check`] calls it in `geom_num::math`
//! and holds it to its table's claim, "correctly rounded": the result is the double nearest the
//! exact value. The exact value comes from `geom_test::fixed`'s 400-bit fixed point: cos and sin
//! of πt and e^x by their series, ln x by Newton's method on e^y, and atan2 by Newton's method on
//! x·sin θ − y·cos θ. Typed refusals (a NaN or infinite argument, an overflow, a logarithm of a
//! non-positive number) are legal where the table says so.
//!
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Arguments come through the rule bytes of the `curves` target, and
//!   for sinpi and cospi also near whole numbers and halves, where the results are exact or tiny.
//! - **Decided only where 400 bits can.** A result smaller than 2^−250 in magnitude, or an exact
//!   value within the oracle's error of a midpoint between two doubles, is not judged: the oracle
//!   could not tell. Everywhere else a result that is not the nearest double is a [`Finding`].
//! - **A panic is a failure.**
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//!
//! Method: correct rounding as `geom_num::math`'s table states it, checked against independent
//! high-precision evaluation (chapter 28.9's reference implementations;
//! `projects/engines/geometry/architecture/28-testing.md`), driven by coverage-guided fuzzing
//! (28.5).
//!
//! Cost: one function call and at most a dozen 400-bit Newton steps or series per input.

// The lint is for fixed-width arithmetic, which can overflow. The oracle's arithmetic is on
// dashu's arbitrary-precision integers, which cannot.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "arbitrary-precision arithmetic in the oracle, which cannot overflow, and the formulas' names (x, y, t, c, s, f)"
)]

use dashu_int::IBig;
use geom_num::math;
use geom_test::fixed::{self, cos_sin, cos_sin_pi, div, exp, mul, one};
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::curves::value;

/// The most bytes [`decode`] reads from any input: 20.
///
/// A function byte and two arguments, each a rule byte and at most eight bytes more, plus a
/// near-whole-number byte: 1 + 2 · 9 + 1 = 20. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 1 + 2 * 9 + 1;

/// The smallest result judged: 2^−250.
const TINY: f64 = f64::from_bits((1023 - 250) << 52);

/// The functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// sin(πx).
    Sinpi,
    /// cos(πx).
    Cospi,
    /// eˣ.
    Exp,
    /// ln x.
    Ln,
    /// The angle of (x, y).
    Atan2,
}

/// One input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Case {
    /// The function.
    pub function: Function,
    /// Its first argument (y for atan2).
    pub a: f64,
    /// The second (x for atan2).
    pub b: f64,
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let tag = src.byte();
    let function = match tag % 5 {
        0 => Function::Sinpi,
        1 => Function::Cospi,
        2 => Function::Exp,
        3 => Function::Ln,
        _ => Function::Atan2,
    };
    let (mut a, b) = (value(&mut src), value(&mut src));
    // For sinpi and cospi, now and then a whole number or a half plus a little.
    let near = src.byte();
    if matches!(function, Function::Sinpi | Function::Cospi) && near & 1 == 1 {
        a = f64::from(near >> 2) / 2.0 + a * 1e-6;
    }
    let read = bytes.len().saturating_sub(src.remaining());
    (Case { function, a, b }, read)
}

/// ln m in the fixed point for m in [1, 2]: Newton's method y ← y + 2(m − eʸ)/(m + eʸ) from 1/2,
/// cubically convergent, to the last bit.
fn ln_small(m: &IBig) -> Option<IBig> {
    let mut y = one() >> 1;
    for _ in 0..16 {
        let e = exp(&y);
        let step = div(&((m - &e) << 1), &(m + &e))?;
        if step == IBig::ZERO {
            break;
        }
        y += step;
    }
    Some(y)
}

/// ln x for a positive double, x = m·2^k split exactly (m in [1, 2)): ln m + k·ln 2, so no bit
/// of a tiny or huge x is lost to the fixed point.
fn ln_fixed(x: f64) -> Option<IBig> {
    let (x, shift) = if x < f64::MIN_POSITIVE {
        // A subnormal: scale it into the normal range exactly.
        (x * f64::from_bits((1023 + 64) << 52), 64i64)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let biased = i64::try_from(bits >> 52 & 0x7FF).ok()?;
    let k = biased - 1023 - shift;
    let m = f64::from_bits(bits & ((1 << 52) - 1) | 1023 << 52);
    let ln2 = ln_small(&(one() << 1))?;
    Some(ln_small(&fixed::of(m)?)? + ln2 * IBig::from(k))
}

/// The angle θ of (x, y) in the fixed point: Newton's method on x·sin θ − y·cos θ from `start`.
fn atan2_fixed(y: &IBig, x: &IBig, start: f64) -> Option<IBig> {
    let mut t = fixed::of(start)?;
    for _ in 0..12 {
        let (c, s) = cos_sin(&t);
        let f = mul(x, &s) - mul(y, &c);
        let fp = mul(x, &c) + mul(y, &s);
        let step = div(&f, &fp)?;
        if step == IBig::ZERO {
            break;
        }
        t -= step;
    }
    Some(t)
}

/// Whether `got` is the double nearest `v`, allowing the oracle's error `err`: v must lie
/// between the midpoints to got's neighbours, widened by `err`. `None` where a neighbour is not
/// finite, which leaves nothing to decide.
fn nearest(got: f64, v: &IBig, err: &IBig) -> Option<bool> {
    let g = fixed::of(got)?;
    let up = fixed::of(got.next_up())?;
    let down = fixed::of(got.next_down())?;
    let (mid_up, mid_down) = ((&g + &up) >> 1, (&g + &down) >> 1);
    Some(*v <= mid_up + err && *v >= mid_down - err)
}

/// The oracle's error at a value of size `v`: 2^−320 absolute (π's 100 digits) and 2^−370
/// relative (squarings and Newton steps).
fn error(v: &IBig) -> IBig {
    (one() >> 320) + (fixed::abs(v) >> 370)
}

/// Whether the fixed point holds `x` closely: zero, or at least 10⁻⁶⁰ in magnitude.
fn held(x: f64) -> bool {
    x == 0.0 || x.abs() >= 1e-60
}

/// Correct rounding for the case's function (module documentation).
///
/// # Errors
///
/// A [`Finding`] when a result is not the nearest double where the oracle can tell.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, 0)
}

/// [`check`] with the function's result moved by `ulps` doubles: 0 for [`check`], and a wrong
/// result for tests to show the law catches it (28.4, fault injection).
///
/// # Errors
///
/// As [`check`].
pub fn check_with(case: &Case, ulps: i32) -> Result<(), Finding> {
    let Case { function, a, b } = *case;
    let got = match function {
        Function::Sinpi => math::sinpi(a),
        Function::Cospi => math::cospi(a),
        Function::Exp => math::exp(a),
        Function::Ln => math::ln(a),
        Function::Atan2 => math::atan2(a, b),
    };
    let Ok(got) = got else { return Ok(()) };
    let mut got = got.get();
    for _ in 0..ulps.unsigned_abs() {
        got = if ulps > 0 {
            got.next_up()
        } else {
            got.next_down()
        };
    }
    // Too small to judge in 400 bits.
    if got.abs() < TINY {
        return Ok(());
    }
    let exact = match function {
        Function::Sinpi => fixed::of(a).map(|t| cos_sin_pi(&t).1),
        Function::Cospi => fixed::of(a).map(|t| cos_sin_pi(&t).0),
        Function::Exp => fixed::of(a).map(|x| exp(&x)),
        Function::Ln => ln_fixed(a),
        Function::Atan2 => match (fixed::of(a), fixed::of(b)) {
            // Arguments too small for the fixed point to hold are not judged.
            (Some(y), Some(x)) if held(a) && held(b) && (a != 0.0 || b != 0.0) => {
                atan2_fixed(&y, &x, got)
            }
            _ => None,
        },
    };
    let Some(v) = exact else { return Ok(()) };
    match nearest(got, &v, &error(&v)) {
        Some(false) => Err(Finding::new(
            "a correctly rounded function returns the nearest double",
            || {
                format!(
                    "{function:?}({a}, {b}) = {got}, exact {}",
                    fixed::rational(&v).to_f64().value()
                )
            },
        )),
        _ => Ok(()),
    }
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
