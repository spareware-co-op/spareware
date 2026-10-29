//! What a NURBS operation that builds new control points charges to the work meter, and how it
//! reserves the memory for them (chapter 23.10).
//!
//! What it does: an operation that builds a curve or surface (knot insertion and refinement,
//! Bezier extraction, knot removal, degree change, derivative curves) knows the size of its result
//! before it computes it. [`charge`] charges one work unit per control point and per knot of the
//! result, and reserves their logical bytes: 8 bytes per coordinate and per weight, and 8 per knot.
//! [`vec_for`] then reserves the real memory fallibly, so a refusal is the meter's
//! `OutOfMemory`, never an abort.
//!
//! Guarantees:
//!
//! - **The same charge on every machine.** The charges are counts of the result's control points
//!   and knots and fixed sizes per kind, never pointer widths or allocator sizes, so a budget
//!   failure is the same everywhere (chapter 23.10).
//! - **Charged before the work.** Each operation charges before it allocates or computes, so an
//!   operation over its budget does no work.
//!
//! Method: the work units and logical bytes of chapter 23.10
//! (`projects/engines/geometry/architecture/23-performance.md`), through `geom-run`'s
//! [`WorkMeter`]; fallible reservation with `Vec::try_reserve_exact`
//! (<https://doc.rust-lang.org/alloc/vec/struct.Vec.html#method.try_reserve_exact>).
//!
//! Cost: constant per charge; [`vec_for`] makes one allocation.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::NurbsError;

/// Charges for a result of `points` control points of dimension `dim` (with `weights` weights)
/// and `knots` knots.
pub(crate) fn charge(
    meter: &mut WorkMeter<'_>,
    points: usize,
    dim: usize,
    weights: bool,
    knots: usize,
) -> Result<(), NurbsError> {
    let wide = |k: usize| u64::try_from(k).map_err(|_| NurbsError::Defect);
    let per_point = if weights { dim.saturating_add(1) } else { dim };
    let work = wide(points)?.saturating_add(wide(knots)?);
    let bytes = wide(points)?
        .saturating_mul(wide(per_point)?)
        .saturating_add(wide(knots)?)
        .saturating_mul(8);
    meter.charge(work)?;
    meter.reserve(bytes)?;
    Ok(())
}

/// An empty vector with room for `len` elements, reserved fallibly.
pub(crate) fn vec_for<T>(meter: &mut WorkMeter<'_>, len: usize) -> Result<Vec<T>, NurbsError> {
    let mut v = Vec::new();
    if v.try_reserve_exact(len).is_err() {
        return Err(NurbsError::Stopped(meter.out_of_memory()));
    }
    Ok(v)
}
