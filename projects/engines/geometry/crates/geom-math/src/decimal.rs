//! Exact decimal numbers: text such as `5921000.1234` or `-1.5e-3` read as significand·10^exponent
//! with no rounding at all, so survey and map coordinates given as decimal text reach the grid
//! through one rounding, never through `f64` (chapters 17.1 and 24).
//!
//! A [`Decimal`] is an integer significand of at most [`DIGITS_MAX`] = 38 digits times a power of
//! ten from 10^−340 to 10^340, kept in lowest terms: the significand has no trailing zero (zero is
//! 0·10^0). So equal values have equal fields, `==` is equality of the numbers, and a value has
//! one text form ([`Decimal`]'s `Display`).
//!
//! **Text.** An optional sign, digits with at most one decimal point (at least one digit before or
//! after it), and an optional exponent: `e` or `E`, an optional sign and digits. That is the
//! grammar of the standard library's `f64::from_str` without its `inf`, `infinity` and `NaN`
//! words: `12`, `+12.`, `.5`, `-0.000125`, `6.02E23`. Nothing else is accepted: no spaces, no
//! digit separators, no decimal comma, no hexadecimal. Leading zeros and trailing fraction zeros
//! may be any in number; they are not significant digits.
//!
//! **Why the crate reads the text itself.** The kernel has no allocator here and every value must
//! be exact. `f64::from_str` rounds to binary; the standard library's integer parsers have no
//! fraction or exponent; and the well-known decimal crates either allocate (`bigdecimal`,
//! `dashu-float`, `num-bigint`) or hold at most 28 fractional digits (`rust_decimal`, which would
//! refuse 1.2e−30, a value a double holds), and none is in the kernel's vetted allow-list
//! (`deny-no-std.toml`). The grammar above needs only one pass over the bytes, accumulating the
//! significand in a `u128` with checked steps, and the exponent's digits are read by the standard
//! library's own `i32::from_str`.
//!
//! **Limits.** 38 significant digits hold any measured value (a double has at most 17), and keep
//! the significand inside `i128`; the exponent range covers every double (10^−324 to 10^308). Text
//! beyond either is refused with [`DecimalError`], never rounded.
//!
//! Guarantees:
//!
//! - **Exact.** [`Decimal::parse`] returns exactly the value the text writes, or refuses it with
//!   [`DecimalError`]; nothing is rounded.
//! - **One form per value.** Every `Decimal` is in lowest terms, so two decimals are equal exactly
//!   when their values are, `Hash` agrees with `==`, and `Display` writes text that
//!   [`Decimal::parse`] reads back to the same `Decimal`.
//! - **No panics, no allocation.** Every integer step is checked; `Display` writes into the given
//!   formatter.
//!
//! Method: decimal scientific notation as IEEE 754-2019 defines its character sequences (section
//! 5.12.3, <https://doi.org/10.1109/IEEESTD.2019.8766229>), read digit by digit into an exact
//! integer and a power of ten, with trailing zeros moved into the exponent (D. E. Knuth, *The Art
//! of Computer Programming*, vol. 2, section 4.4, radix conversion,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Cost: [`Decimal::parse`] is one pass over the text, one checked multiplication and addition per
//! significant digit (zeros inside the significand are multiplied in at most 38 at a time), and the
//! standard library's parse of the exponent's digits; no allocation. `Display` writes at most 38
//! digits and, in plain notation, at most 30 zeros.

use core::fmt;
use core::str::FromStr;

/// The most significant digits a [`Decimal`] holds: 38, so the significand is below 10^38 < 2^127.
pub const DIGITS_MAX: u32 = 38;

/// The smallest exponent a nonzero [`Decimal`] may have, −340: below every double's 10^−324.
pub const EXPONENT_MIN: i32 = -340;

/// The largest exponent a nonzero [`Decimal`] may have, 340: above every double's 10^308.
pub const EXPONENT_MAX: i32 = 340;

/// 10^38, the bound on a significand's magnitude.
const SIGNIFICAND_BOUND: u128 = 100_000_000_000_000_000_000_000_000_000_000_000_000;

/// The widest plain notation `Display` writes: up to 30 zeros between the digits and the point.
const PLAIN_ZEROS: i32 = 30;

/// Why a text is not a [`Decimal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecimalError {
    /// The text breaks the grammar at byte `at` (the text's length when it ends too early): an
    /// unexpected character, a second decimal point, no digits, or an exponent with no digits.
    Syntax {
        /// The byte offset of the first character that does not fit.
        at: usize,
    },
    /// The value needs more than [`DIGITS_MAX`] significant digits.
    TooManyDigits,
    /// The value's exponent, in lowest terms, lies outside [`EXPONENT_MIN`] to [`EXPONENT_MAX`],
    /// or the written exponent does not fit an `i32`.
    ExponentRange,
}

impl fmt::Display for DecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecimalError::Syntax { at } => {
                write!(f, "not a decimal number: byte {at} does not fit")
            }
            DecimalError::TooManyDigits => {
                f.write_str("the decimal number has more than 38 significant digits")
            }
            DecimalError::ExponentRange => {
                f.write_str("the decimal number's exponent is outside −340 to 340")
            }
        }
    }
}

/// An exact decimal number, significand·10^exponent, in lowest terms (see the module
/// documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decimal {
    significand: i128,
    exponent: i32,
}

impl Decimal {
    /// Zero, 0·10^0.
    pub const ZERO: Decimal = Decimal {
        significand: 0,
        exponent: 0,
    };

    /// The decimal significand·10^exponent, put in lowest terms.
    ///
    /// # Errors
    ///
    /// [`DecimalError::TooManyDigits`] if the significand has more than 38 digits once its
    /// trailing zeros are moved into the exponent; [`DecimalError::ExponentRange`] if the exponent
    /// is then outside −340 to 340.
    pub fn new(significand: i128, exponent: i32) -> Result<Decimal, DecimalError> {
        if significand == 0 {
            return Ok(Decimal::ZERO);
        }
        let mut m = significand.unsigned_abs();
        let mut e = exponent;
        // At most 38 steps: a u128 has at most 39 decimal digits, and m is not zero.
        while m.is_multiple_of(10) {
            m /= 10;
            e = e.checked_add(1).ok_or(DecimalError::ExponentRange)?;
        }
        Decimal::checked(significand.is_negative(), m, e)
    }

    /// The decimal ±m·10^e for m > 0 with no trailing zero, checked against both limits.
    fn checked(negative: bool, m: u128, e: i32) -> Result<Decimal, DecimalError> {
        if m >= SIGNIFICAND_BOUND {
            return Err(DecimalError::TooManyDigits);
        }
        if !(EXPONENT_MIN..=EXPONENT_MAX).contains(&e) {
            return Err(DecimalError::ExponentRange);
        }
        // m < 10^38 < 2^127 fits i128, and so does its negation.
        let v = i128::try_from(m).map_err(|_| DecimalError::TooManyDigits)?;
        let significand = if negative { v.checked_neg() } else { Some(v) };
        Ok(Decimal {
            significand: significand.ok_or(DecimalError::TooManyDigits)?,
            exponent: e,
        })
    }

    /// The decimal the text writes, exactly (see the module documentation for the grammar).
    ///
    /// # Errors
    ///
    /// [`DecimalError::Syntax`] if the text breaks the grammar; [`DecimalError::TooManyDigits`]
    /// for more than 38 significant digits; [`DecimalError::ExponentRange`] if the exponent does
    /// not fit an `i32` or, in lowest terms, lies outside −340 to 340.
    pub fn parse(text: &str) -> Result<Decimal, DecimalError> {
        let bytes = text.as_bytes();
        let (negative, start) = match bytes.first() {
            Some(b'-') => (true, 1),
            Some(b'+') => (false, 1),
            _ => (false, 0),
        };
        // The significand: digits, at most one point, at least one digit.
        let mut m: u128 = 0;
        let mut digits: u32 = 0;
        let mut zeros: u32 = 0;
        let mut fraction: i64 = 0;
        let mut point = false;
        let mut any = false;
        let mut at = start;
        while let Some(&b) = bytes.get(at) {
            match char::from(b).to_digit(10) {
                Some(d) => {
                    any = true;
                    if point {
                        fraction = fraction.checked_add(1).ok_or(DecimalError::ExponentRange)?;
                    }
                    if d == 0 {
                        // A zero after a nonzero digit may be trailing: held back until a nonzero
                        // digit follows.
                        if m != 0 {
                            zeros = zeros.checked_add(1).ok_or(DecimalError::TooManyDigits)?;
                        }
                    } else {
                        let size = digits.checked_add(zeros).and_then(|n| n.checked_add(1));
                        digits = size
                            .filter(|&n| n <= DIGITS_MAX)
                            .ok_or(DecimalError::TooManyDigits)?;
                        // digits ≤ 38 bounds every factor: m·10^(zeros + 1) + d < 10^38.
                        m = 10u128
                            .checked_pow(zeros.checked_add(1).ok_or(DecimalError::TooManyDigits)?)
                            .and_then(|scale| m.checked_mul(scale))
                            .and_then(|v| v.checked_add(u128::from(d)))
                            .ok_or(DecimalError::TooManyDigits)?;
                        zeros = 0;
                    }
                }
                None => match b {
                    b'.' if !point => point = true,
                    b'e' | b'E' => break,
                    _ => return Err(DecimalError::Syntax { at }),
                },
            }
            at = at.checked_add(1).ok_or(DecimalError::Syntax { at })?;
        }
        if !any {
            return Err(DecimalError::Syntax { at });
        }
        // The exponent: all that follows the marker, read by the standard library.
        let written: i64 = if at < bytes.len() {
            let after = at.checked_add(1).ok_or(DecimalError::Syntax { at })?;
            let tail = text
                .get(after..)
                .ok_or(DecimalError::Syntax { at: after })?;
            // The exponent's digits start after an optional sign: at `after` or one byte later.
            let first = after
                .checked_add(usize::from(matches!(bytes.get(after), Some(b'+' | b'-'))))
                .ok_or(DecimalError::Syntax { at: after })?;
            let digits: &[u8] = bytes.get(first..).unwrap_or(&[]);
            if let Some(bad) = digits
                .iter()
                .position(|&c| char::from(c).to_digit(10).is_none())
            {
                let at = first.checked_add(bad).unwrap_or(first);
                return Err(DecimalError::Syntax { at });
            }
            if digits.is_empty() {
                return Err(DecimalError::Syntax { at: bytes.len() });
            }
            i64::from(i32::from_str(tail).map_err(|_| DecimalError::ExponentRange)?)
        } else {
            0
        };
        if m == 0 {
            return Ok(Decimal::ZERO);
        }
        // The value is m·10^(written − fraction + zeros): every term below 2^32 in magnitude, so
        // the sum fits i64, and it must then fit i32.
        let e = written
            .checked_sub(fraction)
            .and_then(|v| v.checked_add(i64::from(zeros)))
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(DecimalError::ExponentRange)?;
        Decimal::checked(negative, m, e)
    }

    /// The significand: no trailing zero unless the value is zero, magnitude below 10^38.
    #[must_use]
    pub const fn significand(self) -> i128 {
        self.significand
    }

    /// The exponent: from −340 to 340, and 0 for zero.
    #[must_use]
    pub const fn exponent(self) -> i32 {
        self.exponent
    }

    /// Whether the value is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.significand == 0
    }
}

impl FromStr for Decimal {
    type Err = DecimalError;

    fn from_str(text: &str) -> Result<Decimal, DecimalError> {
        Decimal::parse(text)
    }
}

/// Writes `count` copies of `c` (none for a count of zero or less).
fn repeat(f: &mut fmt::Formatter<'_>, c: char, count: i32) -> fmt::Result {
    for _ in 0..count {
        write!(f, "{c}")?;
    }
    Ok(())
}

impl fmt::Display for Decimal {
    /// Plain notation when it needs at most 30 zeros next to the digits (`1500`, `-0.00125`,
    /// `12.5`); otherwise the significand's digits with an exponent (`15e40`, `-125e-45`). Either
    /// reads back to the same `Decimal`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The digits of |significand|, least significant first: at most 38 (39 for safety).
        let mut buf = [0u8; 39];
        let mut len = 0usize;
        let mut rest = self.significand.unsigned_abs();
        loop {
            let slot = buf.get_mut(len).ok_or(fmt::Error)?;
            // rest % 10 is a digit, below 10.
            *slot = u8::try_from(rest % 10).map_err(|_| fmt::Error)?;
            len = len.checked_add(1).ok_or(fmt::Error)?;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        let digits = buf.get(..len).ok_or(fmt::Error)?;
        // The digits from the `from`-th most significant up to the `to`-th.
        let write_digits = |f: &mut fmt::Formatter<'_>, from: usize, to: usize| -> fmt::Result {
            for &d in digits.iter().rev().skip(from).take(to.saturating_sub(from)) {
                write!(f, "{d}")?;
            }
            Ok(())
        };
        if self.significand.is_negative() {
            f.write_str("-")?;
        }
        let e = self.exponent;
        // n ≤ 39 and |e| ≤ 340: the sum and its negation fit i32.
        let n = i32::try_from(len).map_err(|_| fmt::Error)?;
        let whole = n.checked_add(e).ok_or(fmt::Error)?;
        let lead = whole.checked_neg().ok_or(fmt::Error)?;
        let scientific = |f: &mut fmt::Formatter<'_>| -> fmt::Result {
            write_digits(f, 0, len)?;
            write!(f, "e{e}")
        };
        if (0..=PLAIN_ZEROS).contains(&e) {
            write_digits(f, 0, len)?;
            repeat(f, '0', e)
        } else if e.is_negative() {
            match whole {
                // The point falls inside the digits.
                1.. => {
                    let cut = usize::try_from(whole).map_err(|_| fmt::Error)?;
                    write_digits(f, 0, cut)?;
                    f.write_str(".")?;
                    write_digits(f, cut, len)
                }
                _ if (..=PLAIN_ZEROS).contains(&lead) => {
                    f.write_str("0.")?;
                    repeat(f, '0', lead)?;
                    write_digits(f, 0, len)
                }
                _ => scientific(f),
            }
        } else {
            scientific(f)
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "tests may use native arithmetic; the rules are for kernel code"
)]
mod tests {
    use super::*;

    #[test]
    fn display_covers_every_notation() {
        let text = |m: i128, e: i32| Decimal::new(m, e).unwrap().to_string();
        assert_eq!(text(15, 2), "1500");
        assert_eq!(text(-125, -1), "-12.5");
        assert_eq!(text(125, -5), "0.00125");
        assert_eq!(text(15, 40), "15e40");
        assert_eq!(text(-125, -45), "-125e-45");
        assert_eq!(text(0, 7), "0");
    }
}
