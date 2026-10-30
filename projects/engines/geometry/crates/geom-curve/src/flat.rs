//! Many evaluations in one call, into flat arrays (chapter 23.6).
//!
//! What it does: [`fill`] writes N points of dimension D into one flat array in structure-of-arrays
//! order, all x values, then all y values, then all z values, with one result per point beside
//! it. [`crate::ParamCurve::points`] (N parameters of a curve) and `geom-surface`'s
//! `ParamSurface::points` (N parameter pairs of a surface, all u values then all v values) are built
//! on it, so every curve and surface kind evaluates N parameters per call.
//!
//! Guarantees:
//!
//! - **The same bits as one call at a time.** Each point is the value its own evaluation gives,
//!   copied unchanged; nothing is computed here, so no result depends on N, on its neighbours or
//!   on the array layout.
//! - **One failure fails only itself.** A point whose evaluation fails gets that failure in its
//!   result and zeros in every coordinate (never a NaN); every other point is written as it would
//!   be alone.
//! - **Shapes are checked first.** Unless the results hold N entries and the flat array D·N
//!   values, nothing is written and the call returns [`CurveError::ArrayLength`].
//!
//! Method: structure-of-arrays evaluation as chapter 23.6 asks
//! (`projects/engines/geometry/architecture/23-performance.md`): each lane is the scalar
//! evaluation, so the layout can never change a value.
//!
//! Cost: one evaluation per point and D stores; no allocation; nothing recurses.

use crate::error::CurveError;

/// Writes the points `points` yields into `out` in structure-of-arrays order, coordinate k of
/// point i at `out[k·n + i]` with n the number of points, and each point's result into
/// `results[i]`: `Ok(())`, or its failure with its coordinates zero.
///
/// # Errors
///
/// [`CurveError::ArrayLength`] if `results` does not hold n entries or `out` does not hold D·n
/// values (checked in that order, before anything is written).
pub fn fill<const D: usize, I>(
    out: &mut [f64],
    results: &mut [Result<(), CurveError>],
    points: I,
) -> Result<(), CurveError>
where
    I: ExactSizeIterator<Item = Result<[f64; D], CurveError>>,
{
    let n = points.len();
    if results.len() != n {
        return Err(CurveError::ArrayLength {
            expected: n,
            given: results.len(),
        });
    }
    let values = n.saturating_mul(D);
    if out.len() != values {
        return Err(CurveError::ArrayLength {
            expected: values,
            given: out.len(),
        });
    }
    for (i, (point, result)) in points.zip(results.iter_mut()).enumerate() {
        let coordinates = match point {
            Ok(p) => {
                *result = Ok(());
                p
            }
            Err(e) => {
                *result = Err(e);
                [0.0; D]
            }
        };
        // Column k starts at k·n; i < n and k < D, so every place is inside `out`.
        for (column, value) in out.chunks_exact_mut(n.max(1)).zip(coordinates) {
            if let Some(slot) = column.get_mut(i) {
                *slot = value;
            }
        }
    }
    Ok(())
}
