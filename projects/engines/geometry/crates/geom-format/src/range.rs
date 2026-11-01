//! Range checks on numbers read from a file, made before any body is built, so the fixed-width
//! predicates stay exact (24.7, "Range checks keep the predicates sound").
//!
//! What it does: a file can pass every checksum and still hold a coordinate near 2^50, and release
//! builds wrap integers silently. So every reader checks: grid coordinates within ±(2^40 − 1)
//! steps ([`grid`], [`grid_column`]); doubles, control points included, finite and within the same
//! range ([`real`], [`real_column`]); and planes reduced, with normals within 83 bits and offsets
//! within 125 ([`plane`]). A failure is [`RangeError::OutOfRange`] with the record, the byte
//! offset, the field and how many bits the value needs, or [`RangeError::Malformed`] for a plane
//! that is not reduced.
//!
//! Guarantees:
//!
//! - **Exactly the frame.** [`grid`] accepts a value exactly when `geom_num::grid::Coord::new`
//!   does, `|v| ≤ 2^40 − 1`, proven by Kani for every `i64` (`proofs`), and tested at ±(2^40 − 1)
//!   and one beyond.
//! - **Exactly the finite doubles of the frame.** [`real`] accepts `x` exactly when it is finite
//!   and `|x| ≤ 2^40 − 1`, proven by Kani for every `f64`.
//! - **The bits the predicates assume.** [`plane`] accepts a plane exactly when its normal is not
//!   zero, each normal component needs at most 83 bits and the offset at most 125 (the bounds of
//!   `geom_num::plane`, from 40-bit coordinates), and the greatest common divisor of the four is 1.
//! - **No panic, no overflow**: magnitudes are taken with `unsigned_abs`, offsets with checked
//!   arithmetic.
//!
//! Method: chapter 24.7 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`, with the bit budgets of
//! chapter 6.2 (`projects/engines/geometry/architecture/06-numbers.md`); the greatest common
//! divisor by Euclid's algorithm (Knuth, TAOCP vol. 2, 4.5.2, Algorithm A).
//!
//! Cost: constant per value; a column is one pass; [`plane`]'s divisor takes at most a few hundred
//! remainder steps on 128-bit values. No allocation, no recursion.

use core::fmt;

use geom_num::grid::{COORD_BITS, Coord};

/// The most bits a stored plane normal component may need: two products of 41-bit coordinate
/// differences, 2^83 (`geom_num::plane`'s bound for `Plane::through`).
pub const NORMAL_BITS: u32 = 2 * (COORD_BITS + 1) + 1;

/// The most bits a stored plane offset may need: 2^125 (`geom_num::plane`'s bound).
pub const OFFSET_BITS: u32 = NORMAL_BITS + COORD_BITS + 2;

/// The frame's limit as a double, 2^40 − 1 (`COORD_MAX`), which a double holds exactly; the tests
/// check the two agree.
pub const REAL_LIMIT: f64 = 1_099_511_627_775.0;

/// A value outside what the predicates' bit budgets allow (24.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfRange {
    /// The record's type.
    pub record: u64,
    /// The byte offset of the value.
    pub offset: u64,
    /// The field it belongs to.
    pub field: &'static str,
    /// How many bits its magnitude needs; 1025 for a NaN or an infinity.
    pub bits: u32,
}

/// Why a stored number cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    /// A value past its bit budget.
    OutOfRange(OutOfRange),
    /// A plane whose four numbers share a divisor, or whose normal is zero.
    Malformed {
        /// The record's type.
        record: u64,
        /// The byte offset of the plane.
        offset: u64,
        /// The field it belongs to.
        field: &'static str,
    },
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeError::OutOfRange(o) => write!(
                f,
                "record {} at byte {}: {} needs {} bits",
                o.record, o.offset, o.field, o.bits
            ),
            RangeError::Malformed {
                record,
                offset,
                field,
            } => write!(
                f,
                "record {record} at byte {offset}: {field} is not a reduced plane"
            ),
        }
    }
}

impl core::error::Error for RangeError {}

/// How many bits the magnitude of `v` needs: 0 for 0.
#[must_use]
pub const fn bits(v: i128) -> u32 {
    v.unsigned_abs().bit_width()
}

/// Whether `v` is a grid coordinate of the frame, `|v| ≤ 2^40 − 1`.
#[must_use]
pub const fn grid(v: i64) -> bool {
    Coord::new(v).is_ok()
}

/// Whether `x` is finite and `|x| ≤ 2^40 − 1`.
#[must_use]
pub fn real(x: f64) -> bool {
    (-REAL_LIMIT..=REAL_LIMIT).contains(&x)
}

/// The bits a double's magnitude needs, for a report: its binary exponent plus one for values of
/// 1 or more, 0 below, and 1025 for a NaN or an infinity.
fn real_bits(x: f64) -> u32 {
    let exponent = (x.to_bits() >> 52) & 0x7ff;
    let exponent = u32::try_from(exponent).unwrap_or(u32::MAX);
    exponent.saturating_sub(1022)
}

/// The byte offset of value `index` of a column of 8-byte values that starts at `offset`.
fn at(offset: u64, index: usize) -> u64 {
    u64::try_from(index)
        .ok()
        .and_then(|i| i.checked_mul(8))
        .and_then(|i| offset.checked_add(i))
        .unwrap_or(u64::MAX)
}

/// Checks a column of grid coordinates whose bytes start at `offset` of record type `record`.
///
/// # Errors
///
/// [`RangeError::OutOfRange`] for the first value outside the frame.
pub fn grid_column(
    values: &[i64],
    record: u64,
    offset: u64,
    field: &'static str,
) -> Result<(), RangeError> {
    match values.iter().position(|&v| !grid(v)) {
        None => Ok(()),
        Some(i) => Err(RangeError::OutOfRange(OutOfRange {
            record,
            offset: at(offset, i),
            field,
            bits: values.get(i).map_or(0, |&v| bits(i128::from(v))),
        })),
    }
}

/// Checks a column of doubles whose bytes start at `offset` of record type `record`.
///
/// # Errors
///
/// [`RangeError::OutOfRange`] for the first value that is not finite or is outside the frame.
pub fn real_column(
    values: &[f64],
    record: u64,
    offset: u64,
    field: &'static str,
) -> Result<(), RangeError> {
    match values.iter().position(|&x| !real(x)) {
        None => Ok(()),
        Some(i) => Err(RangeError::OutOfRange(OutOfRange {
            record,
            offset: at(offset, i),
            field,
            bits: values.get(i).map_or(0, |&x| real_bits(x)),
        })),
    }
}

/// Checks a stored plane `n · p + d = 0` at `offset` of record type `record`.
///
/// # Errors
///
/// [`RangeError::OutOfRange`] for a component past its bits, then [`RangeError::Malformed`] for a
/// zero normal or four numbers with a common divisor above 1.
pub fn plane(
    normal: [i128; 3],
    offset_d: i128,
    record: u64,
    offset: u64,
    field: &'static str,
) -> Result<(), RangeError> {
    let out = |bits| {
        Err(RangeError::OutOfRange(OutOfRange {
            record,
            offset,
            field,
            bits,
        }))
    };
    if let Some(&n) = normal.iter().find(|&&n| bits(n) > NORMAL_BITS) {
        return out(bits(n));
    }
    if bits(offset_d) > OFFSET_BITS {
        return out(bits(offset_d));
    }
    let [nx, ny, nz] = normal;
    let divisor = [nx, ny, nz, offset_d]
        .iter()
        .fold(0_u128, |g, &v| gcd(g, v.unsigned_abs()));
    if normal.iter().all(|&n| n == 0) || divisor != 1 {
        return Err(RangeError::Malformed {
            record,
            offset,
            field,
        });
    }
    Ok(())
}

/// The greatest common divisor, by Euclid's remainders; `gcd(0, b) = b`.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while let Some(r) = a.checked_rem(b) {
        a = b;
        b = r;
    }
    a
}

/// Bounded proofs (chapter 28.7; piece G92), proven by `kernel-fuzz.yml`'s `kani-format` job.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::{REAL_LIMIT, bits, grid, real};

    /// The grid check accepts exactly ±(2^40 − 1), for every `i64`.
    #[kani::proof]
    fn grid_accepts_exactly_the_frame() {
        let v: i64 = kani::any();
        let inside = v >= -((1_i64 << 40) - 1) && v <= (1_i64 << 40) - 1;
        assert!(grid(v) == inside);
    }

    /// The real check accepts exactly the finite doubles within ±(2^40 − 1), for every `f64`.
    #[kani::proof]
    fn real_accepts_exactly_the_finite_frame() {
        let x = f64::from_bits(kani::any());
        let inside = x.is_finite() && x.abs() <= REAL_LIMIT;
        assert!(real(x) == inside);
    }

    /// `bits` is the width of the magnitude: 2^(bits − 1) ≤ |v| < 2^bits, for every `i128`.
    #[kani::proof]
    fn bits_is_the_magnitude_width() {
        let v: i128 = kani::any();
        let m = v.unsigned_abs();
        let b = bits(v);
        assert!(b <= 128);
        if b == 0 {
            assert!(m == 0);
        } else {
            assert!(m >> (b - 1) == 1);
        }
    }
}
