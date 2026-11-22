//! Every step this crate works on a length, in one place. A length is construct-params'
//! [`Length`] throughout: this module is the only code here that knows how a `Length` holds its
//! value, so a change to its unit changes this file and no caller.
//!
//! Halving (for the two centrelines) is the one step that can fail: half of a length is exact
//! only if the length unit holds it. Today's `Length` counts whole millimetres, so half of an odd
//! number of millimetres is refused, naming the line. Piece A39 moves `Length` to the geometry
//! kernel's length grid (1/6400 mm), which holds the half of every length with up to two decimals
//! of a millimetre; only a length off that step can then be refused. Nothing is ever rounded.

use construct_params::{Length, Value};

/// No length.
pub(crate) const ZERO: Length = Length::from_mm(0);

/// `a + b`, if it fits.
pub(crate) fn sum(a: Length, b: Length) -> Option<Length> {
    a.checked_add(b)
}

/// `a - b`, if it fits.
pub(crate) fn difference(a: Length, b: Length) -> Option<Length> {
    a.checked_sub(b)
}

/// The same length on the other side, if it fits.
pub(crate) fn negated(a: Length) -> Option<Length> {
    ZERO.checked_sub(a)
}

/// Half of `a`, if the length unit holds it exactly.
pub(crate) fn half(a: Length) -> Option<Length> {
    let units = a.mm();
    if units.checked_rem(2) == Some(0) {
        units.checked_div(2).map(Length::from_mm)
    } else {
        None
    }
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
