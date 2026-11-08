//! Angles as text: degrees, minutes and seconds, the compact `dd.mmss` form, decimal degrees,
//! gon, and quadrant bearings, read and written exactly (17.2).
//!
//! What it does: [`parse`] reads an angle in one of four [`Notation`]s into whole angle units, and
//! [`parse_quadrant`] a surveyor's quadrant bearing, `N 12°34'56" E`. Each reports whether the
//! text was a whole number of units or had to be rounded, half to even, to the nearest
//! ([`Rounded`]). [`write`] writes an angle to a number of decimal places, again reporting whether
//! the text is exact, and [`shortest`] the shortest exact text when there is one. A unit is 0.0004″,
//! so every angle has an exact text in degrees, minutes and seconds with at most four decimals of
//! a second; in decimal degrees an angle has one only if it is a whole number of millionths of a
//! degree (9 units), with at most six decimals, and in gon only if it is a whole number of
//! hundred-thousandths of a gon (81 units), with at most five, because 9,000,000 and 8,100,000 hold
//! the factor 3, which no power of ten does.
//!
//! The notations:
//!
//! - [`Notation::Dms`]: `-12°34'56.78"`. A sign, then degrees, minutes and seconds in that order,
//!   each a decimal number followed by its symbol: for degrees `°`, `d`, `D`, or the look-alikes
//!   `º` and `˚`; for minutes `'`, `′`, `’` or `´`; for seconds `"`, `″`, `”` or two of the
//!   minutes' `'`. Any of the three may be left out, but not all, and only the last given may have
//!   a fraction. Minutes and seconds are below 60. Spaces may come between the parts. Written with
//!   `°`, `'` and `"`, minutes and seconds in two digits.
//! - [`Notation::Compact`]: `12.345678`, degrees, then after the point two digits of minutes, two
//!   of seconds and the decimals of the second (the form of `LandXML`'s angular unit "decimal
//!   dd.mm.ss" and of survey calculators); fewer than four digits are padded with zeros, so
//!   `12.3` is 12°30′.
//! - [`Notation::Degrees`]: `12.345678°`, a decimal number with an optional `°` or `d`.
//! - [`Notation::Gon`]: `123.45678 gon`, a decimal number with an optional `gon` or `g`.
//!
//! Guarantees:
//!
//! - **Exact or reported.** A text that is a whole number of units is read exactly, and one that is
//!   not is rounded once, half to even, to the nearest unit, and says so: Certified within half a
//!   unit (17.2). Writing rounds the same way and says whether the text is exactly the angle.
//! - **Round trips.** For every angle, [`parse`] of [`shortest`]'s text gives the angle back,
//!   exactly; and for every reading on an instrument's grid (0.01″, 0.000001°, 0.00001 gon), the
//!   text [`write`] gives at that many places reads back as the same angle. Tested exhaustively
//!   over whole ranges of each grid.
//! - **Refused, never guessed.** A text that does not follow its notation is an
//!   [`AngleTextError`] naming the byte where it went wrong: an unexpected character, a number
//!   without its symbol, parts out of order or repeated, a fraction before the last part, 60 or
//!   more minutes or seconds, more than 30 decimals, or an angle beyond `i64` units. Nothing in the
//!   text is skipped or guessed.
//!
//! Method: the notations surveyors and their instruments write (chapter 17.2,
//! `projects/engines/geometry/architecture/17-survey-terrain-civil.md`), read by exact decimal
//! arithmetic on integers, with the rounding rule of IEEE 754's roundTiesToEven (IEEE 754-2019,
//! <https://doi.org/10.1109/IEEESTD.2019.8766229>), as `geom-math`'s decimals are; the symbols each
//! part accepts, the order of the parts and the rule that only the last may have a fraction follow
//! `GeographicLib`'s `DMS::Decode`, whose parser rounds through doubles, which this one does not.
//! Consulted: `GeographicLib` `DMS.cpp`, MIT,
//!   <https://github.com/geographiclib/geographiclib/blob/48959dfc4a581ad29f76b526726a801473767c98/src/DMS.cpp>,
//!   for the symbols it accepts for degrees, minutes, seconds and signs, the order of the parts,
//!   and its rules that only the last part has a fraction and that minutes and seconds are below
//!   60. No code copied.
//!
//! Cost: linear in the length of the text, with no allocation: fixed-width integer arithmetic,
//! at most 128 bits wide.

use core::fmt;

use crate::angle::{
    AngleUnits, Bearing, FULL_TURN, HALF_TURN, PER_DEGREE, PER_GON, PER_MINUTE, PER_SECOND,
    QUARTER_TURN,
};
use crate::grid::Rounded;

/// The most decimal places a text may have: their value times the largest unit, 9,000,000, stays
/// below 10^37, which `u128` holds.
const MAX_PLACES: u32 = 30;

/// The most decimal places [`write`] writes: an `i64` magnitude times 10^18 stays below 10^37.
pub const MAX_WRITE_PLACES: u8 = 18;

/// Powers of ten, 10^0 to 10^30, as `u128`.
const TEN: [u128; 31] = [
    1,
    10,
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    10_000_000_000_000,
    100_000_000_000_000,
    1_000_000_000_000_000,
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    1_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
    100_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000_000,
];

// The largest fraction a text may hold, below 10^30, times the largest unit, 9,000,000, is below
// 10^37 and fits u128 (about 3.4 · 10^38), and so does the largest magnitude, 2^63, times the
// largest power `write` uses, 10^18: below 10^37 too (the unit test `the_budgets_fit_u128`).
const _: () = assert!(MAX_PLACES == 30 && MAX_WRITE_PLACES == 18);

/// How an angle is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Notation {
    /// Degrees, minutes and seconds: `12°34'56.78"`.
    Dms,
    /// Degrees, then minutes, seconds and their decimals after one point: `12.345678`.
    Compact,
    /// Decimal degrees: `12.345678°`.
    Degrees,
    /// Gon, a four-hundredth of a turn: `123.45678 gon`.
    Gon,
}

/// Why a text is not an angle, or an angle cannot be written as asked. Positions are byte offsets
/// into the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleTextError {
    /// The text holds no number.
    Empty,
    /// A character that does not belong to the notation there.
    Unexpected {
        /// Where it is.
        at: usize,
    },
    /// A number of degrees, minutes and seconds with no symbol after it.
    MissingSymbol {
        /// Where the symbol should be.
        at: usize,
    },
    /// A part of degrees, minutes and seconds out of order or given twice.
    OutOfOrder {
        /// Where the part is.
        at: usize,
    },
    /// A part after one with a fraction: only the last part may have one.
    FractionNotLast {
        /// Where the later part is.
        at: usize,
    },
    /// Minutes of 60 or more.
    MinutesOutOfRange {
        /// Where the minutes are.
        at: usize,
    },
    /// Seconds of 60 or more.
    SecondsOutOfRange {
        /// Where the seconds are.
        at: usize,
    },
    /// More than 30 decimal places, or more digits before the point than `u64` holds.
    TooManyDigits {
        /// Where the number is.
        at: usize,
    },
    /// An angle beyond `i64` units.
    TooLarge,
    /// A quadrant bearing whose angle is negative or more than 90°.
    QuadrantAngle,
    /// More than [`MAX_WRITE_PLACES`] decimal places asked of [`write`].
    TooManyPlaces,
}

impl fmt::Display for AngleTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AngleTextError::Empty => f.write_str("the text holds no number"),
            AngleTextError::Unexpected { at } => write!(f, "unexpected character at byte {at}"),
            AngleTextError::MissingSymbol { at } => {
                write!(
                    f,
                    "a number with no degree, minute or second symbol at byte {at}"
                )
            }
            AngleTextError::OutOfOrder { at } => {
                write!(f, "degrees, minutes and seconds out of order at byte {at}")
            }
            AngleTextError::FractionNotLast { at } => {
                write!(f, "a part after one with a fraction at byte {at}")
            }
            AngleTextError::MinutesOutOfRange { at } => {
                write!(f, "60 or more minutes at byte {at}")
            }
            AngleTextError::SecondsOutOfRange { at } => {
                write!(f, "60 or more seconds at byte {at}")
            }
            AngleTextError::TooManyDigits { at } => write!(f, "too many digits at byte {at}"),
            AngleTextError::TooLarge => f.write_str("the angle is beyond i64 angle units"),
            AngleTextError::QuadrantAngle => {
                f.write_str("a quadrant bearing's angle must be from 0° to 90°")
            }
            AngleTextError::TooManyPlaces => {
                write!(
                    f,
                    "at most {MAX_WRITE_PLACES} decimal places can be written"
                )
            }
        }
    }
}

/// A decimal number as read: its whole part, and its fraction as digits and their count.
#[derive(Clone, Copy, Debug)]
struct Number {
    /// Where it starts.
    at: usize,
    /// The digits before the point.
    whole: u64,
    /// The digits after the point, as an integer.
    fraction: u128,
    /// How many digits came after the point.
    places: u32,
}

/// A place in a text being read.
struct Cursor<'t> {
    text: &'t str,
    at: usize,
}

impl<'t> Cursor<'t> {
    /// The text not yet read.
    fn rest(&self) -> &'t str {
        self.text.get(self.at..).unwrap_or("")
    }

    /// Steps over `n` bytes; `n` is the length of text just matched, so this stays within it.
    fn step(&mut self, n: usize) {
        self.at = self.at.saturating_add(n);
    }

    /// Steps over any spaces: ASCII spaces, tabs and line ends.
    fn spaces(&mut self) {
        let n = self.rest().bytes().take_while(|b| is_space(*b)).count();
        self.step(n);
    }

    /// Whether the text goes on with one of `options`, stepping over the first that it does.
    fn eat(&mut self, options: &[&str]) -> bool {
        for option in options {
            if self.rest().starts_with(option) {
                self.step(option.len());
                return true;
            }
        }
        false
    }

    /// A sign: whether it is negative.
    fn sign(&mut self) -> bool {
        if self.eat(&MINUS) {
            return true;
        }
        self.eat(&["+"]);
        false
    }

    /// Digits, as an integer and their count, or `None` past `limit` digits or `u128`.
    fn digits(&mut self, limit: u32) -> Result<(u128, u32), AngleTextError> {
        let at = self.at;
        let mut value = 0u128;
        let mut count = 0u32;
        for byte in self.rest().bytes() {
            // The digit a byte stands for, or none for any byte that is not '0' to '9'.
            let Some(digit) = byte.checked_sub(b'0').filter(|d| *d < 10) else {
                break;
            };
            count = count.saturating_add(1);
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u128::from(digit)))
                .filter(|_| count <= limit)
                .ok_or(AngleTextError::TooManyDigits { at })?;
        }
        // Each digit is one byte.
        self.step(usize::try_from(count).unwrap_or(usize::MAX));
        Ok((value, count))
    }

    /// A decimal number, or `None` if no digit starts here. A point must have a digit before it
    /// and after it.
    fn number(&mut self) -> Result<Option<Number>, AngleTextError> {
        let at = self.at;
        let (whole, count) = self.digits(MAX_PLACES)?;
        if count == 0 {
            return Ok(None);
        }
        let whole = u64::try_from(whole).map_err(|_| AngleTextError::TooManyDigits { at })?;
        let (fraction, places) = if self.rest().starts_with('.') {
            self.step(1);
            let (fraction, places) = self.digits(MAX_PLACES)?;
            if places == 0 {
                return Err(AngleTextError::Unexpected {
                    at: self.at.saturating_sub(1),
                });
            }
            (fraction, places)
        } else {
            (0, 0)
        };
        Ok(Some(Number {
            at,
            whole,
            fraction,
            places,
        }))
    }

    /// The end of the text, after any spaces, or the place where something else is.
    fn end(&mut self) -> Result<(), AngleTextError> {
        self.spaces();
        if self.rest().is_empty() {
            Ok(())
        } else {
            Err(AngleTextError::Unexpected { at: self.at })
        }
    }
}

/// Whether `byte` is an ASCII space, tab or line end.
const fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// The spaces [`is_space`] accepts, as characters.
const SPACES: [char; 4] = [' ', '\t', '\n', '\r'];

/// `value`'s last `count` decimal digits, with leading zeros: a fraction's digits after the point.
fn fixed_digits(f: &mut fmt::Formatter<'_>, value: u128, count: usize) -> fmt::Result {
    for place in (0..count).rev() {
        let scale = TEN.get(place).copied().ok_or(fmt::Error)?;
        let digit = value.checked_div(scale).ok_or(fmt::Error)? % 10;
        // A digit is below 10, so it is a u8 and an ASCII digit.
        let byte = u8::try_from(digit)
            .ok()
            .ok_or(fmt::Error)?
            .saturating_add(b'0');
        fmt::Write::write_char(f, char::from(byte))?;
    }
    Ok(())
}

/// `n / d` rounded to nearest, ties to even, and whether it was exact; `d` is at least 1 (a power
/// of ten, or a notation's last unit), so neither division falls back.
fn divided(n: u128, d: u128) -> (u128, bool) {
    let (quotient, rest) = (n.checked_div(d).unwrap_or(0), n.checked_rem(d).unwrap_or(n));
    // Twice the remainder cannot overflow: it is below d ≤ 10^30.
    let twice = rest.saturating_mul(2);
    let up = twice > d || (twice == d && quotient % 2 == 1);
    let rounded = if up {
        quotient.saturating_add(1)
    } else {
        quotient
    };
    (rounded, rest == 0)
}

/// What a number is in units of `per` units each: exact for its whole part, and its fraction
/// rounded to nearest, ties to even.
fn units_of(n: &Number, per: i64) -> (u128, bool) {
    let per = per.unsigned_abs();
    // A u64 times a unit of at most 9,000,000 is below 2^88.
    let whole = u128::from(n.whole).saturating_mul(u128::from(per));
    let scale = TEN
        .get(usize::try_from(n.places).unwrap_or(usize::MAX))
        .copied()
        .unwrap_or(1);
    // Below 10^30 · 9,000,000 < 2^123: MAX_PLACES's bound.
    let (part, exact) = divided(n.fraction.saturating_mul(u128::from(per)), scale);
    (whole.saturating_add(part), exact)
}

/// A magnitude in units and its sign, as an angle, or [`AngleTextError::TooLarge`].
fn signed(
    negative: bool,
    magnitude: u128,
    exact: bool,
) -> Result<Rounded<AngleUnits>, AngleTextError> {
    let v = i64::try_from(magnitude).map_err(|_| AngleTextError::TooLarge)?;
    let v = if negative { v.saturating_neg() } else { v };
    Ok(Rounded::new(AngleUnits(v), exact))
}

/// The three parts of degrees, minutes and seconds, in order, each with its symbols: the degree
/// sign, `d` and `D`, and the masculine ordinal and ring above that are typed for it; the
/// apostrophe, prime, right single quote and acute accent; the quotation mark, double prime and
/// right double quote (`GeographicLib`'s sets, less those it reads as other things).
const PARTS: [(&[&str], i64); 3] = [
    (&["\u{b0}", "d", "D", "\u{ba}", "\u{2da}"], PER_DEGREE),
    (&["'", "\u{2032}", "\u{2019}", "\u{b4}"], PER_MINUTE),
    (&["\"", "\u{2033}", "\u{201d}"], PER_SECOND),
];

/// The minus signs: the hyphen, the minus sign and the en dash.
const MINUS: [&str; 3] = ["-", "\u{2212}", "\u{2013}"];

/// The two-quote spelling of seconds, checked before a single quote is taken for minutes.
const SECONDS_BY_QUOTES: &str = "''";

/// Degrees, minutes and seconds after the sign.
fn dms(cursor: &mut Cursor<'_>) -> Result<(u128, bool), AngleTextError> {
    let mut total = 0u128;
    let mut exact = true;
    let mut next = 0usize;
    let mut fractional = false;
    let mut any = false;
    loop {
        cursor.spaces();
        let Some(n) = cursor.number()? else { break };
        if fractional {
            return Err(AngleTextError::FractionNotLast { at: n.at });
        }
        cursor.spaces();
        let at = cursor.at;
        let part = if cursor.eat(&[SECONDS_BY_QUOTES]) {
            Some(2)
        } else {
            PARTS.iter().position(|(symbols, _)| cursor.eat(symbols))
        };
        let Some(part) = part else {
            return Err(AngleTextError::MissingSymbol { at });
        };
        if part < next {
            return Err(AngleTextError::OutOfOrder { at });
        }
        if part > 0 && n.whole >= 60 {
            return Err(if part == 1 {
                AngleTextError::MinutesOutOfRange { at: n.at }
            } else {
                AngleTextError::SecondsOutOfRange { at: n.at }
            });
        }
        let per = PARTS.get(part).map_or(PER_SECOND, |(_, per)| *per);
        let (units, whole) = units_of(&n, per);
        total = total.checked_add(units).ok_or(AngleTextError::TooLarge)?;
        exact &= whole;
        fractional = n.places > 0;
        next = part.saturating_add(1);
        any = true;
    }
    if any {
        Ok((total, exact))
    } else {
        Err(AngleTextError::Empty)
    }
}

/// The compact form after the sign: degrees, a point, then minutes, seconds and their decimals.
fn compact(cursor: &mut Cursor<'_>) -> Result<(u128, bool), AngleTextError> {
    let n = cursor.number()?.ok_or(AngleTextError::Empty)?;
    let degrees = u128::from(n.whole).saturating_mul(u128::from(PER_DEGREE.unsigned_abs()));
    let (mmss, rest, places) = if n.places >= 4 {
        let scale = TEN
            .get(usize::try_from(n.places.saturating_sub(4)).unwrap_or(usize::MAX))
            .copied()
            .unwrap_or(1);
        let (mmss, rest) = (
            n.fraction.checked_div(scale).unwrap_or(0),
            n.fraction.checked_rem(scale).unwrap_or(0),
        );
        (mmss, rest, n.places.saturating_sub(4))
    } else {
        let pad = TEN
            .get(usize::try_from(4u32.saturating_sub(n.places)).unwrap_or(usize::MAX))
            .copied()
            .unwrap_or(1);
        (n.fraction.saturating_mul(pad), 0, 0)
    };
    let (minutes, seconds) = (mmss / 100, mmss % 100);
    // The minutes are the first two digits after the point, the seconds the next two.
    let after_point =
        n.at.saturating_add(usize::try_from(decimal_length(n.whole)).unwrap_or(usize::MAX));
    if minutes >= 60 {
        return Err(AngleTextError::MinutesOutOfRange {
            at: after_point.saturating_add(1),
        });
    }
    if seconds >= 60 {
        return Err(AngleTextError::SecondsOutOfRange {
            at: after_point.saturating_add(3),
        });
    }
    let whole_seconds = minutes
        .saturating_mul(u128::from(PER_MINUTE.unsigned_abs()))
        .saturating_add(seconds.saturating_mul(u128::from(PER_SECOND.unsigned_abs())));
    let fraction = Number {
        at: n.at,
        whole: 0,
        fraction: rest,
        places,
    };
    let (part, exact) = units_of(&fraction, PER_SECOND);
    let total = degrees
        .checked_add(whole_seconds)
        .and_then(|t| t.checked_add(part))
        .ok_or(AngleTextError::TooLarge)?;
    Ok((total, exact))
}

/// How many decimal digits `v` has: 1 for 0.
fn decimal_length(v: u64) -> u32 {
    v.checked_ilog10().map_or(1, |d| d.saturating_add(1))
}

/// A decimal number of `per` units each, then one of `symbols` or none.
fn decimal(
    cursor: &mut Cursor<'_>,
    per: i64,
    symbols: &[&str],
) -> Result<(u128, bool), AngleTextError> {
    let n = cursor.number()?.ok_or(AngleTextError::Empty)?;
    cursor.spaces();
    cursor.eat(symbols);
    Ok(units_of(&n, per))
}

/// The angle a text holds, in `notation`: exact when the text is a whole number of angle units,
/// and otherwise rounded to the nearest, ties to even.
///
/// # Errors
///
/// An [`AngleTextError`] naming where the text stops following the notation.
pub fn parse(text: &str, notation: Notation) -> Result<Rounded<AngleUnits>, AngleTextError> {
    let mut cursor = Cursor { text, at: 0 };
    cursor.spaces();
    let negative = cursor.sign();
    cursor.spaces();
    let (magnitude, exact) = match notation {
        Notation::Dms => dms(&mut cursor)?,
        Notation::Compact => compact(&mut cursor)?,
        Notation::Degrees => decimal(&mut cursor, PER_DEGREE, &["\u{b0}", "d", "D"])?,
        Notation::Gon => decimal(&mut cursor, PER_GON, &["gon", "g"])?,
    };
    cursor.end()?;
    signed(negative, magnitude, exact)
}

/// An angle as text, to a number of decimal places, as [`write`] and [`shortest`] make it: its
/// `Display` writes the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Written {
    /// Whether a minus sign comes first (never for zero).
    negative: bool,
    /// The magnitude in 10^−places of the notation's last unit: the second, the degree or the gon.
    magnitude: u128,
    /// The notation.
    notation: Notation,
    /// The decimal places.
    places: u8,
}

impl Written {
    /// The notation it is written in.
    #[must_use]
    pub const fn notation(&self) -> Notation {
        self.notation
    }

    /// The decimal places it is written to: of the second for [`Notation::Dms`] and
    /// [`Notation::Compact`], of the degree or the gon for the others.
    #[must_use]
    pub const fn places(&self) -> u8 {
        self.places
    }
}

impl fmt::Display for Written {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scale = TEN
            .get(usize::from(self.places))
            .copied()
            .ok_or(fmt::Error)?;
        let whole = self.magnitude.checked_div(scale).ok_or(fmt::Error)?;
        let fraction = self.magnitude.checked_rem(scale).ok_or(fmt::Error)?;
        let width = usize::from(self.places);
        if self.negative && self.magnitude > 0 {
            f.write_str("-")?;
        }
        match self.notation {
            Notation::Dms | Notation::Compact => {
                let (degrees, minutes, seconds) = (whole / 3600, whole / 60 % 60, whole % 60);
                if self.notation == Notation::Dms {
                    write!(f, "{degrees}\u{b0}{minutes:02}'{seconds:02}")?;
                    if width > 0 {
                        f.write_str(".")?;
                        fixed_digits(f, fraction, width)?;
                    }
                    f.write_str("\"")
                } else {
                    write!(f, "{degrees}.{minutes:02}{seconds:02}")?;
                    fixed_digits(f, fraction, width)
                }
            }
            Notation::Degrees | Notation::Gon => {
                write!(f, "{whole}")?;
                if width > 0 {
                    f.write_str(".")?;
                    fixed_digits(f, fraction, width)?;
                }
                f.write_str(if self.notation == Notation::Gon {
                    " gon"
                } else {
                    "\u{b0}"
                })
            }
        }
    }
}

/// The units of the notation's last written unit: the second, the degree or the gon.
const fn last_unit(notation: Notation) -> i64 {
    match notation {
        Notation::Dms | Notation::Compact => PER_SECOND,
        Notation::Degrees => PER_DEGREE,
        Notation::Gon => PER_GON,
    }
}

/// `angle` written in `notation` to `places` decimal places, rounded to nearest, ties to even, with
/// whether the text is exactly the angle.
///
/// # Errors
///
/// [`AngleTextError::TooManyPlaces`] for more than [`MAX_WRITE_PLACES`] places.
pub fn write(
    angle: AngleUnits,
    notation: Notation,
    places: u8,
) -> Result<Rounded<Written>, AngleTextError> {
    if places > MAX_WRITE_PLACES {
        return Err(AngleTextError::TooManyPlaces);
    }
    let scale = TEN
        .get(usize::from(places))
        .copied()
        .ok_or(AngleTextError::TooManyPlaces)?;
    // |angle| ≤ 2^63 and scale ≤ 10^18, so the product is below 10^37.
    let scaled = u128::from(angle.0.unsigned_abs()).saturating_mul(scale);
    let (magnitude, exact) = divided(scaled, u128::from(last_unit(notation).unsigned_abs()));
    let written = Written {
        negative: angle.0 < 0,
        magnitude,
        notation,
        places,
    };
    Ok(Rounded::new(written, exact))
}

/// The shortest exact text of `angle` in `notation`, or `None` when the notation has none: every
/// angle has one in [`Notation::Dms`] and [`Notation::Compact`], with at most four decimals of a
/// second; in [`Notation::Degrees`] only a whole number of millionths of a degree, with at most
/// six decimals, and in [`Notation::Gon`] only a whole number of hundred-thousandths of a gon, with
/// at most five.
#[must_use]
pub fn shortest(angle: AngleUnits, notation: Notation) -> Option<Written> {
    let most = match notation {
        Notation::Dms | Notation::Compact => 4,
        Notation::Degrees => 6,
        Notation::Gon => 5,
    };
    (0..=most)
        .filter_map(|places| write(angle, notation, places).ok())
        .find(Rounded::is_exact)
        .map(|written| written.value())
}

/// A quadrant bearing as text, `N 12°34'56" E`: its `Display` writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Quadrant {
    /// Whether it is measured from north; otherwise from south.
    north: bool,
    /// Whether it turns to the east; otherwise to the west.
    east: bool,
    /// The angle from north or south, from 0° to 90°.
    angle: Written,
}

impl fmt::Display for Quadrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = if self.north { "N" } else { "S" };
        let to = if self.east { "E" } else { "W" };
        write!(f, "{from} {} {to}", self.angle)
    }
}

/// A quadrant bearing read from text: `N` or `S`, an angle from 0° to 90° in degrees, minutes and
/// seconds, then `E` or `W`, with spaces allowed between them and either case of letter. `N θ E` is
/// the bearing θ, `S θ E` is 180° − θ, `S θ W` is 180° + θ and `N θ W` is 360° − θ.
///
/// # Errors
///
/// An [`AngleTextError`]: as [`parse`] for the angle, [`AngleTextError::Unexpected`] for a missing
/// or wrong letter, and [`AngleTextError::QuadrantAngle`] for an angle with a sign or above 90°.
pub fn parse_quadrant(text: &str) -> Result<Rounded<Bearing>, AngleTextError> {
    let mut cursor = Cursor { text, at: 0 };
    cursor.spaces();
    let north = if cursor.eat(&["N", "n"]) {
        true
    } else if cursor.eat(&["S", "s"]) {
        false
    } else {
        return Err(AngleTextError::Unexpected { at: cursor.at });
    };
    // The text's end without trailing spaces, and the letter before it, read as bytes: E and W
    // are one byte each, and a byte of a longer character never equals either.
    let bytes = text.as_bytes();
    let last = text.trim_end_matches(SPACES).len();
    let letter = last.saturating_sub(1);
    let east = match bytes.get(letter) {
        Some(b'E' | b'e') => true,
        Some(b'W' | b'w') => false,
        _ => return Err(AngleTextError::Unexpected { at: letter }),
    };
    let inner = text.get(..letter).unwrap_or("");
    let mut middle = Cursor {
        text: inner,
        at: cursor.at,
    };
    middle.spaces();
    if middle
        .rest()
        .starts_with(['+', '-', '\u{2212}', '\u{2013}'])
    {
        return Err(AngleTextError::QuadrantAngle);
    }
    let (magnitude, exact) = dms(&mut middle)?;
    middle.end()?;
    let theta = i64::try_from(magnitude).map_err(|_| AngleTextError::QuadrantAngle)?;
    if theta > QUARTER_TURN {
        return Err(AngleTextError::QuadrantAngle);
    }
    // θ ≤ QUARTER_TURN, so each of these is within one turn either way and cannot saturate.
    let units = match (north, east) {
        (true, true) => theta,
        (false, true) => HALF_TURN.saturating_sub(theta),
        (false, false) => HALF_TURN.saturating_add(theta),
        (true, false) => FULL_TURN.saturating_sub(theta),
    };
    Ok(Rounded::new(Bearing::new(AngleUnits(units)), exact))
}

/// `bearing` as a quadrant bearing, its angle to `places` decimal places of a second, rounded to
/// nearest, ties to even, with whether the text is exact. North and east take the boundaries: 0° is
/// `N 0° E`, 90° is `N 90° E`, 180° is `S 0° W` and 270° is `S 90° W`.
///
/// # Errors
///
/// [`AngleTextError::TooManyPlaces`] for more than [`MAX_WRITE_PLACES`] places.
pub fn write_quadrant(bearing: Bearing, places: u8) -> Result<Rounded<Quadrant>, AngleTextError> {
    let b = bearing.units().0;
    // b is in [0, FULL_TURN), so each difference is within one turn and cannot saturate.
    let (north, east, theta) = if b <= QUARTER_TURN {
        (true, true, b)
    } else if b < HALF_TURN {
        (false, true, HALF_TURN.saturating_sub(b))
    } else if b <= HALF_TURN.saturating_add(QUARTER_TURN) {
        (false, false, b.saturating_sub(HALF_TURN))
    } else {
        (true, false, FULL_TURN.saturating_sub(b))
    };
    let angle = write(AngleUnits(theta), Notation::Dms, places)?;
    Ok(Rounded::new(
        Quadrant {
            north,
            east,
            angle: angle.value(),
        },
        angle.is_exact(),
    ))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    #[test]
    fn ties_round_to_even_and_report_exactness() {
        assert_eq!(divided(15, 10), (2, false));
        assert_eq!(divided(25, 10), (2, false));
        assert_eq!(divided(26, 10), (3, false));
        assert_eq!(divided(24, 10), (2, false));
        assert_eq!(divided(30, 10), (3, true));
        assert_eq!(divided(0, 10), (0, true));
    }

    #[test]
    fn decimal_lengths_count_every_digit() {
        assert_eq!(decimal_length(0), 1);
        assert_eq!(decimal_length(9), 1);
        assert_eq!(decimal_length(10), 2);
        assert_eq!(decimal_length(u64::MAX), 20);
    }

    #[test]
    fn the_budgets_fit_u128() {
        assert!(TEN[MAX_PLACES as usize].checked_mul(9_000_000).is_some());
        assert!(
            TEN[usize::from(MAX_WRITE_PLACES)]
                .checked_mul(1 << 63)
                .is_some()
        );
    }

    #[test]
    fn the_powers_of_ten_are_powers_of_ten() {
        for (i, t) in TEN.iter().enumerate() {
            assert_eq!(*t, 10u128.pow(i as u32));
        }
    }

    #[test]
    fn a_number_too_long_is_refused_where_it_starts() {
        let long = "1".repeat(31);
        let mut cursor = Cursor { text: &long, at: 0 };
        assert_eq!(
            cursor.digits(MAX_PLACES),
            Err(AngleTextError::TooManyDigits { at: 0 })
        );
    }
}
