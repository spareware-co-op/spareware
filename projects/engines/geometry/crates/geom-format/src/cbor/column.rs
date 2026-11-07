//! Columns: the little-endian `i64` and `f64` arrays SKG stores coordinates and other reals in, read
//! into vectors.
//!
//! What it does: [`int_column`] and [`float_column`] read the byte string a tag 79 or tag 86 holds
//! (RFC 8746) into values, appending to the caller's vector, after reserving the memory through the
//! caller's work meter. 24.3 stores grid coordinates as `i64` columns and other reals as `f64`
//! columns.
//!
//! Guarantees:
//!
//! - **Exact.** Each value is the little-endian reading of its eight bytes, bit for bit.
//! - **Finite reals only.** A NaN or an infinity in a float column is refused with the offset of its
//!   first byte, and nothing of that column is appended.
//! - **A whole number of values.** A byte string whose length is not a multiple of eight is
//!   refused.
//! - **Metered and fallible.** The memory is reserved from the meter and from the allocator before
//!   anything is appended; a refused reservation stops the meter rather than aborting. Each
//!   [`SCAN_BYTES_PER_CHARGE`] bytes charge the meter one unit.
//!
//! Method: RFC 8746 section 2's typed arrays, tags 79 (signed 64-bit, little endian) and 86 (binary64,
//! little endian), <https://www.rfc-editor.org/rfc/rfc8746#section-2>, read with Rust's
//! `from_le_bytes`, which every target computes the same way; SKG's columns,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` (24.3).
//!
//! Cost: O(n) time in the column's bytes, in one pass over chunks of [`SCAN_BYTES_PER_CHARGE`]; one
//! reservation of exactly the values appended. Recursion: none.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use super::decode::SCAN_BYTES_PER_CHARGE;
use super::error::{CborError, CborErrorKind};

/// The bytes of one column value.
const VALUE_BYTES: usize = 8;

/// The values one charge covers.
const VALUES_PER_CHARGE: usize = SCAN_BYTES_PER_CHARGE / VALUE_BYTES;

/// Appends the `i64` values of a tag 79 column's byte string to `out`.
///
/// # Errors
///
/// [`CborErrorKind::ColumnLength`] (offset 0) if `bytes` is not a whole number of values, or the
/// meter's stop; `out` is unchanged on any error but a stop part-way through.
pub fn int_column(
    bytes: &[u8],
    out: &mut Vec<i64>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CborError> {
    let values = whole_values(bytes)?;
    reserve(out, values.len(), bytes.len(), meter)?;
    for chunk in values.chunks(VALUES_PER_CHARGE) {
        meter.charge(1).map_err(|s| stopped(0, s))?;
        out.extend(chunk.iter().map(|v| i64::from_le_bytes(*v)));
    }
    Ok(())
}

/// Appends the `f64` values of a tag 86 column's byte string to `out`.
///
/// # Errors
///
/// [`CborErrorKind::ColumnLength`] (offset 0) if `bytes` is not a whole number of values,
/// [`CborErrorKind::NotFinite`] at the first byte of the first NaN or infinity, or the meter's
/// stop; `out` is unchanged on any error but a stop part-way through.
pub fn float_column(
    bytes: &[u8],
    out: &mut Vec<f64>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CborError> {
    let values = whole_values(bytes)?;
    reserve(out, values.len(), bytes.len(), meter)?;
    let before = out.len();
    for (index, chunk) in values.chunks(VALUES_PER_CHARGE).enumerate() {
        meter.charge(1).map_err(|s| stopped(0, s))?;
        if let Some(row) = chunk
            .iter()
            .position(|v| !f64::from_le_bytes(*v).is_finite())
        {
            out.truncate(before);
            let offset = index
                .saturating_mul(VALUES_PER_CHARGE)
                .saturating_add(row)
                .saturating_mul(VALUE_BYTES);
            return Err(CborError {
                offset,
                kind: CborErrorKind::NotFinite,
            });
        }
        out.extend(chunk.iter().map(|v| f64::from_le_bytes(*v)));
    }
    Ok(())
}

/// The eight-byte values of `bytes`, if it is a whole number of them.
fn whole_values(bytes: &[u8]) -> Result<&[[u8; VALUE_BYTES]], CborError> {
    let (values, rest) = bytes.as_chunks::<VALUE_BYTES>();
    if rest.is_empty() {
        Ok(values)
    } else {
        Err(CborError {
            offset: 0,
            kind: CborErrorKind::ColumnLength,
        })
    }
}

/// Reserves room for `count` more values in `out`, `logical_bytes` of them, from the meter and then
/// the allocator.
fn reserve<T>(
    out: &mut Vec<T>,
    count: usize,
    logical_bytes: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CborError> {
    meter
        .reserve(u64::try_from(logical_bytes).unwrap_or(u64::MAX))
        .map_err(|s| stopped(0, s))?;
    out.try_reserve_exact(count)
        .map_err(|_| stopped(0, meter.out_of_memory()))
}

/// The error for a meter that stopped.
fn stopped(offset: usize, s: geom_run::Stopped) -> CborError {
    CborError {
        offset,
        kind: CborErrorKind::Stopped(s),
    }
}
