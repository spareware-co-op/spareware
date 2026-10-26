//! Checked reads, writes and index arithmetic for the NURBS algorithms' arrays.
//!
//! What it does: the algorithms of The NURBS Book index knots, basis functions and control points
//! by sums and differences of the degree, the span and loop counters. The kernel's lints forbid
//! unchecked indexing and unchecked integer arithmetic (principle 13), so every such index goes
//! through these helpers, which return [`NurbsError::Defect`] instead of panicking.
//!
//! Guarantees:
//!
//! - **No panic, no wrong element.** A read or write out of bounds, or an index sum or difference
//!   that would overflow, is `Err(Defect)`; every other call reads or writes exactly the element
//!   named.
//! - **Unreachable for valid inputs.** Each algorithm's indices are bounded by sizes its input
//!   checks have established, so `Defect` marks a bug in this crate and the tests treat it as one.
//!
//! Method: Rust's checked slice access (`get`, `get_mut`) and checked integer arithmetic
//! (<https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add>).
//!
//! Cost: one comparison per call; nothing allocates.

use crate::NurbsError;

/// `s[i]`.
pub(crate) fn get<T: Copy>(s: &[T], i: usize) -> Result<T, NurbsError> {
    s.get(i).copied().ok_or(NurbsError::Defect)
}

/// `s[i] = v`.
pub(crate) fn set<T>(s: &mut [T], i: usize, v: T) -> Result<(), NurbsError> {
    *s.get_mut(i).ok_or(NurbsError::Defect)? = v;
    Ok(())
}

/// `&mut s[i]`.
pub(crate) fn at<T>(s: &mut [T], i: usize) -> Result<&mut T, NurbsError> {
    s.get_mut(i).ok_or(NurbsError::Defect)
}

/// `a + b`.
pub(crate) fn add(a: usize, b: usize) -> Result<usize, NurbsError> {
    a.checked_add(b).ok_or(NurbsError::Defect)
}

/// `a − b`, which must not be negative.
pub(crate) fn sub(a: usize, b: usize) -> Result<usize, NurbsError> {
    a.checked_sub(b).ok_or(NurbsError::Defect)
}

/// `a · b`.
pub(crate) fn mul(a: usize, b: usize) -> Result<usize, NurbsError> {
    a.checked_mul(b).ok_or(NurbsError::Defect)
}

/// A whole number below 2^32 as a double, exactly. The algorithms convert degrees, derivative
/// orders and binomial coefficients, all far below 2^32.
pub(crate) fn real(k: usize) -> Result<f64, NurbsError> {
    u32::try_from(k)
        .map(f64::from)
        .map_err(|_| NurbsError::Defect)
}

/// The end (one past the last index) of the run of values equal to `v[start]` that begins at
/// `start`; `start` itself if it is past the end.
pub(crate) fn run_end(v: &[f64], start: usize) -> Result<usize, NurbsError> {
    let Some(&value) = v.get(start) else {
        return Ok(start);
    };
    let mut end = add(start, 1)?;
    while v.get(end).is_some_and(|&x| x == value) {
        end = add(end, 1)?;
    }
    Ok(end)
}
