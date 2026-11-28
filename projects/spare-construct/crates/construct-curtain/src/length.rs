//! Every step this crate works on a length, in one place. A length is construct-params'
//! [`Length`], a whole number of the geometry kernel's grid steps (1/6,400 mm); this module is the
//! only code here that knows how a `Length` holds its value, so a change to its unit changes this
//! file and no caller.
//!
//! Sums and differences are checked. A half is exact or refused ([`half`]): a mullion stands
//! centred on its grid line, so its width must halve on the grid. The one rounding this crate
//! makes is [`share`], a length divided equally (a grid of a fixed count of panels, the length of
//! a line at an angle): the nearest step, a half up, each once.

use construct_params::{Area, Length, Value};

/// No length.
pub(crate) const ZERO: Length = Length::from_steps(0);

/// The model's tolerance, 0.01 mm (main's ruling of 22 Nov 2026): 64 grid steps. Two grid lines
/// nearer than this are one line, and a line nearer a border than this is the border.
pub const TOLERANCE: Length = Length::from_steps(64);

/// `a + b`, if it fits.
pub(crate) fn sum(a: Length, b: Length) -> Option<Length> {
    a.checked_add(b)
}

/// `a - b`, if it fits.
pub(crate) fn difference(a: Length, b: Length) -> Option<Length> {
    a.checked_sub(b)
}

/// The same length the other way.
pub(crate) fn negated(a: Length) -> Option<Length> {
    ZERO.checked_sub(a)
}

/// Half of `a`, if the grid holds it exactly.
pub(crate) fn half(a: Length) -> Option<Length> {
    let steps = a.steps();
    if steps.checked_rem(2) == Some(0) {
        steps.checked_div(2).map(Length::from_steps)
    } else {
        None
    }
}

/// `a` taken `n` times, if it fits.
pub(crate) fn times(a: Length, n: i64) -> Option<Length> {
    a.steps().checked_mul(n).map(Length::from_steps)
}

/// How many whole `b` fit in `a`, both above zero.
pub(crate) fn whole_times(a: Length, b: Length) -> Option<i64> {
    a.steps().checked_div(b.steps())
}

/// The nearest step to `extent * k / n`, a half up, for `extent` and `k` at or above zero and `n`
/// above zero: where line `k` of `n` equal divisions of `extent` lies.
pub(crate) fn share(extent: Length, k: i64, n: i64) -> Option<Length> {
    let twice_n = i128::from(n).checked_mul(2)?;
    let top = i128::from(extent.steps())
        .checked_mul(i128::from(k))?
        .checked_mul(2)?
        .checked_add(i128::from(n))?;
    let steps = top.checked_div_euclid(twice_n)?;
    i64::try_from(steps).ok().map(Length::from_steps)
}

/// Half of `twice`, to the nearest step, a half up: the middle of a pattern centred on a length
/// that may be an odd number of steps.
pub(crate) fn half_up(twice: i128) -> Option<Length> {
    let steps = twice.checked_add(1)?.checked_div_euclid(2)?;
    i64::try_from(steps).ok().map(Length::from_steps)
}

/// The length of a line `dx` by `dy` steps, to the nearest step, a half up: exact along an axis.
pub(crate) fn hypot(dx: i64, dy: i64) -> Option<Length> {
    let (x, y) = (u128::from(dx.unsigned_abs()), u128::from(dy.unsigned_abs()));
    let squares = x.checked_mul(x)?.checked_add(y.checked_mul(y)?)?;
    let root = squares.isqrt();
    // (root + 1/2)^2 = root^2 + root + 1/4, so the true length rounds up when what is left over
    // passes `root`.
    let rest = squares.checked_sub(root.checked_mul(root)?)?;
    let nearest = if rest > root {
        root.checked_add(1)?
    } else {
        root
    };
    i64::try_from(nearest).ok().map(Length::from_steps)
}

/// `a` by `b`, if it fits.
pub(crate) fn area(a: Length, b: Length) -> Option<Area> {
    a.checked_mul(b)
}

/// A length in construct-params' stored spelling (`length:<payload>`).
pub(crate) fn write(a: Length) -> String {
    Value::Length(a).stored()
}

/// A length from construct-params' stored spelling; `None` for anything else.
pub(crate) fn read(text: &str) -> Option<Length> {
    match Value::from_stored(text)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}
