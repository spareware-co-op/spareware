//! Reals that are not lengths (a parameter, a weight, a ratio): read from decimal text to the
//! nearest double, and written back as the shortest decimal that reads back to the same double.
//!
//! What it does: [`read`] parses decimal text exactly (`geom-math`'s `Decimal`) and gives the
//! nearest double, ties to even, keeping the sign of a zero; [`nearest`] does the same for a
//! `Decimal`. [`shortest`] writes a double as [`Shortest`]: the decimal with the fewest
//! significant digits that reads back to exactly that double, and among those the closest to it,
//! with whether the double was −0. Chapter 24.8 asks that such a real "come back as the same
//! decimal on export when it had at most 15 significant digits": a decimal of at most 15 digits is
//! the only one of so few digits that reads back to its nearest double (two such decimals are at
//! least 10^−15 of their size apart, a double's rounding interval at most 2^−52), so the shortest
//! is that decimal.
//!
//! **Both directions are the core library's own, correctly rounded and the same on every target.**
//! The core library's decimal parser rounds once, half to even, from the exact decimal (the
//! Eisel–Lemire algorithm with an exact fallback), and its float formatting's shortest mode gives
//! the shortest digits that read back, closest among the shortest (Grisu with a fallback to
//! Steele and White's Dragon4). Both are integer algorithms in `core`, with no platform maths and
//! no allocation, so the kernel needs no further dependency for them. solidcore's STEP writer
//! writes its reals the same way.
//!
//! Guarantees:
//!
//! - **Round trip, bit for bit.** For every finite double x, `read` of `shortest(x)`'s text is x,
//!   with the same bits, −0 and the subnormals included.
//! - **Shortest, then closest.** No decimal with fewer significant digits reads back to x, and
//!   among those with as few, the one written is the closest to x.
//! - **Nearest on reading.** `nearest(d)` is the double nearest the decimal's exact value, ties to
//!   even; a decimal beyond the largest finite double is [`ExchangeError::RealOutOfRange`], never
//!   an infinity, and a decimal below the smallest subnormal's half reads as a zero of its sign.
//! - **Finite only.** NaN and the infinities are [`ExchangeError::NotFinite`].
//!
//! Method: correctly rounded decimal-to-binary conversion (W. D. Clinger, "How to read floating
//! point numbers accurately", PLDI 1990, <https://doi.org/10.1145/93542.93557>; D. Lemire, "Number
//! parsing at a gigabyte per second", *Software: Practice and Experience* 51 (2021),
//! <https://doi.org/10.1002/spe.2984>) and shortest binary-to-decimal conversion (G. L. Steele Jr.
//! and J. L. White, "How to print floating-point numbers accurately", PLDI 1990,
//! <https://doi.org/10.1145/93542.93559>; F. Loitsch, "Printing floating-point numbers quickly and
//! accurately with integers", PLDI 2010, <https://doi.org/10.1145/1806596.1806623>), as Rust's
//! core library implements them.
//!
//! Consulted: Rust core library, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/library/core/src/num/imp/flt2dec/mod.rs>,
//!   for the guarantees of the shortest mode `{:e}` formats with (read back correctly, shortest,
//!   closest among the shortest), at the release the toolchain pins (1.99.0).
//! Consulted: Rust core library, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/library/core/src/num/imp/dec2flt/mod.rs>,
//!   for the guarantee that `f64`'s parser rounds once, half to even, at the same release.
//! Consulted: solidcore (step-export), Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/78d1eb12d222652219313c12787b65389c4d9db8/crates/step-export/src/lib.rs>,
//!   for writing a STEP real by the core library's shortest formatting (`real`). No code copied.
//!
//! Cost: [`read`] and [`nearest`] are one exact parse (one pass over the text) and the core
//! parser's, at most a few hundred integer operations; [`shortest`] is the core formatter's
//! shortest mode into a 32-byte buffer on the stack and one exact parse of its digits. No
//! allocation.

use core::fmt::{self, Write as _};
use core::str::FromStr as _;

use geom_math::Decimal;

use crate::error::ExchangeError;

/// A fixed buffer on the stack for formatted text, which refuses rather than truncates.
pub(crate) struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    /// An empty buffer.
    pub(crate) const fn new() -> Self {
        Text {
            bytes: [0; N],
            len: 0,
        }
    }

    /// The text written so far.
    pub(crate) fn as_str(&self) -> &str {
        self.bytes
            .get(..self.len)
            .and_then(|b| core::str::from_utf8(b).ok())
            .unwrap_or("")
    }
}

impl<const N: usize> fmt::Write for Text<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
        let slot = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        slot.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// A double written as the shortest decimal that reads back to it: the decimal, and whether the
/// double was −0, which a decimal's one zero cannot say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortest {
    decimal: Decimal,
    negative_zero: bool,
}

impl Shortest {
    /// The decimal: zero for either zero.
    #[must_use]
    pub const fn decimal(self) -> Decimal {
        self.decimal
    }

    /// Whether the double was −0.
    #[must_use]
    pub const fn is_negative_zero(self) -> bool {
        self.negative_zero
    }

    /// The double it was written from.
    ///
    /// # Errors
    ///
    /// None for a value made by [`shortest`]; [`nearest`]'s errors otherwise.
    pub fn to_f64(self) -> Result<f64, ExchangeError> {
        if self.negative_zero {
            return Ok(-0.0);
        }
        nearest(self.decimal)
    }
}

impl fmt::Display for Shortest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative_zero {
            f.write_str("-0")
        } else {
            write!(f, "{}", self.decimal)
        }
    }
}

/// The shortest decimal that reads back to `value`, closest to it among the shortest.
///
/// # Errors
///
/// [`ExchangeError::NotFinite`] for NaN or an infinity; [`ExchangeError::Decimal`] only for a
/// defect: the core formatter writes at most 17 digits and an exponent within ±324, which every
/// `Decimal` holds.
pub fn shortest(value: f64) -> Result<Shortest, ExchangeError> {
    if !value.is_finite() {
        return Err(ExchangeError::NotFinite);
    }
    // At most a sign, 17 digits, a point, `e`, a sign and three exponent digits: 24 bytes.
    let mut text = Text::<32>::new();
    write!(text, "{value:e}")
        .map_err(|_| ExchangeError::Numeric(geom_num::NumError::BudgetExceeded))?;
    let decimal = Decimal::parse(text.as_str())?;
    Ok(Shortest {
        decimal,
        negative_zero: value == 0.0 && value.is_sign_negative(),
    })
}

/// The double nearest the decimal's exact value, ties to even; zero is +0.
///
/// # Errors
///
/// [`ExchangeError::RealOutOfRange`] if the value is beyond the largest finite double (it would
/// round to an infinity).
pub fn nearest(value: Decimal) -> Result<f64, ExchangeError> {
    // A decimal's text is at most a sign, 38 digits, a point and 30 zeros, or a sign, 38 digits, a
    // point, `e`, a sign and an exponent of three digits: at most 72 bytes.
    let mut text = Text::<80>::new();
    write!(text, "{value}")
        .map_err(|_| ExchangeError::Numeric(geom_num::NumError::BudgetExceeded))?;
    let x = f64::from_str(text.as_str())
        .map_err(|_| ExchangeError::Numeric(geom_num::NumError::BudgetExceeded))?;
    if x.is_finite() {
        Ok(x)
    } else {
        Err(ExchangeError::RealOutOfRange)
    }
}

/// Decimal text read exactly and then to the nearest double, ties to even, keeping the sign of a
/// zero (`-0`, `-0.0e5` read as −0).
///
/// # Errors
///
/// [`ExchangeError::Decimal`] for text that is not a decimal number;
/// [`ExchangeError::RealOutOfRange`] beyond the largest finite double.
pub fn read(text: &str) -> Result<f64, ExchangeError> {
    let value = Decimal::parse(text)?;
    if value.is_zero() {
        return Ok(if text.starts_with('-') { -0.0 } else { 0.0 });
    }
    nearest(value)
}
