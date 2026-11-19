//! A spline's derivatives and points as fixed linear maps of its control points (piece G220;
//! chapter 15.3).
//!
//! What it does: the weights and knots of a spline stay fixed in a solve, so the curve's point at a
//! parameter, and each of its derivatives, is a sum of the control points times numbers that do not
//! change: C⁽ᵏ⁾(u) = Σ cᵢ Pᵢ. [`end_coefficients`] gives those numbers for the first derivatives at
//! a clamped end (the finish read as the start of the spline run backwards), rational or not, and
//! [`point_coefficients`] the numbers for the point itself and for its first derivative at any
//! parameter. The constraints that read rational splines, and the ones that pin a point or take
//! an arc length, write their residuals over these numbers, so the Jacobian is generated from the
//! residual and these numbers are the only thing computed beside it.
//!
//! Guarantees:
//!
//! - **Rational or not.** The numbers come from the basis functions and their derivatives
//!   (`geom-nurbs`, A2.3) and the quotient rule of rational curves (Piegl and Tiller, eq. 4.8), so
//!   one path serves every weight.
//! - **Fixed.** They depend on degree, knots, weights and the parameter, never on the control
//!   points.
//! - **No panics, no recursion, no unsafe code.**
//!
//! Method: L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., Springer (1997),
//! <https://doi.org/10.1007/978-3-642-59223-2>, algorithm A2.3 and equations 4.5 to 4.8: with
//! A = Σ Nᵢ wᵢ Pᵢ and W = Σ Nᵢ wᵢ, C = A / W and C⁽ᵏ⁾ = (A⁽ᵏ⁾ − Σⱼ₌₁ᵏ C(k, j) W⁽ʲ⁾ C⁽ᵏ⁻ʲ⁾) / W, which
//! is linear in the points when the weights are fixed.
//!
//! Cost: O(degree²·level) for an end, O(degree²) for a parameter.

extern crate alloc;

use alloc::vec::Vec;

use geom_nurbs::{Basis, Curve, Limit};

use crate::model::{ItemId, Sketch, SketchError, SplineEnd};

/// The binomial coefficient C(k, j) for k at most 3, as a double.
fn binomial(k: usize, j: usize) -> f64 {
    match (k, j) {
        (2, 1) => 2.0,
        (3, 1 | 2) => 3.0,
        _ => 1.0,
    }
}

/// `order` + 1 rows of the coefficients of C⁽ᵏ⁾ on the basis functions that are not zero at `u`,
/// row k, column i being the number that multiplies the control point `span` − degree + i, and the
/// span they sit on.
fn rows(
    basis: &Basis,
    weights: &[f64],
    u: f64,
    order: usize,
    id: ItemId,
) -> Result<(usize, Vec<f64>), SketchError> {
    let wrong = |_| SketchError::WrongKind(id);
    let width = basis.degree().checked_add(1).ok_or(SketchError::Internal)?;
    let mut ders: Vec<f64> = Vec::new();
    ders.try_reserve_exact(
        width
            .checked_mul(order.checked_add(1).ok_or(SketchError::Internal)?)
            .ok_or(SketchError::Internal)?,
    )
    .map_err(|_| SketchError::OutOfMemory)?;
    ders.resize(
        width
            .checked_mul(order.checked_add(1).ok_or(SketchError::Internal)?)
            .ok_or(SketchError::Internal)?,
        0.0,
    );
    let span = basis
        .eval_derivs(u, Limit::FromRight, order, &mut ders)
        .map_err(wrong)?;
    let first = span
        .checked_sub(basis.degree())
        .ok_or(SketchError::Internal)?;
    let weight = |i: usize| -> Result<f64, SketchError> {
        if weights.is_empty() {
            Ok(1.0)
        } else {
            first
                .checked_add(i)
                .and_then(|j| weights.get(j))
                .copied()
                .ok_or(SketchError::Internal)
        }
    };
    let der = |k: usize, i: usize| -> Result<f64, SketchError> {
        k.checked_mul(width)
            .and_then(|r| r.checked_add(i))
            .and_then(|j| ders.get(j))
            .copied()
            .ok_or(SketchError::Internal)
    };
    // W⁽ᵏ⁾ for each k.
    let mut big: Vec<f64> = Vec::new();
    for k in 0..=order {
        let mut sum = 0.0;
        for i in 0..width {
            sum += der(k, i)? * weight(i)?;
        }
        big.push(sum);
    }
    let w0 = *big.first().ok_or(SketchError::Internal)?;
    if w0.is_nan() || w0 <= 0.0 {
        return Err(SketchError::WrongKind(id));
    }
    let cell = |k: usize, i: usize| -> Result<usize, SketchError> {
        k.checked_mul(width)
            .and_then(|r| r.checked_add(i))
            .ok_or(SketchError::Internal)
    };
    let total = cell(order.checked_add(1).ok_or(SketchError::Internal)?, 0)?;
    let mut out: Vec<f64> = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| SketchError::OutOfMemory)?;
    out.resize(total, 0.0);
    for k in 0..=order {
        for i in 0..width {
            let mut top = der(k, i)? * weight(i)?;
            for j in 1..=k {
                let lower = out
                    .get(cell(k.checked_sub(j).ok_or(SketchError::Internal)?, i)?)
                    .copied()
                    .ok_or(SketchError::Internal)?;
                let w = *big.get(j).ok_or(SketchError::Internal)?;
                top -= binomial(k, j) * w * lower;
            }
            *out.get_mut(cell(k, i)?).ok_or(SketchError::Internal)? = top / w0;
        }
    }
    Ok((first, out))
}

/// A spline end's first control points, outward from the end (the first `level` + 1), and the
/// coefficients of its derivatives 1 to `level` on them: row k − 1 holds the `level` + 1 numbers
/// that multiply the points in C⁽ᵏ⁾.
///
/// # Errors
///
/// [`SketchError::WrongKind`] for a spline the constraints cannot read at that level (see
/// [`Sketch::spline_parts`]); [`SketchError::OutOfMemory`].
pub(crate) fn end_coefficients(
    sketch: &Sketch,
    spline: ItemId,
    end: SplineEnd,
    level: usize,
) -> Result<(Vec<ItemId>, Vec<f64>), SketchError> {
    let parts = sketch.spline_parts(spline, level)?;
    let mut ids: Vec<ItemId> = Vec::new();
    let mut ks: Vec<f64> = Vec::new();
    let mut ws: Vec<f64> = Vec::new();
    ids.try_reserve(parts.points.len())
        .and_then(|()| ks.try_reserve(parts.knots.len()))
        .and_then(|()| ws.try_reserve(parts.weights.len()))
        .map_err(|_| SketchError::OutOfMemory)?;
    match end {
        SplineEnd::Start => {
            ids.extend_from_slice(parts.points);
            ks.extend_from_slice(parts.knots);
            ws.extend_from_slice(parts.weights);
        }
        SplineEnd::Finish => {
            ids.extend(parts.points.iter().rev().copied());
            ks.extend(parts.knots.iter().rev().map(|k| -k));
            ws.extend(parts.weights.iter().rev().copied());
        }
    }
    let basis = Basis::new(parts.degree, ks).map_err(|_| SketchError::WrongKind(spline))?;
    let u0 = basis.domain().0;
    let (first, table) = rows(&basis, &ws, u0, level, spline)?;
    if first != 0 {
        return Err(SketchError::WrongKind(spline));
    }
    let width = parts.degree.checked_add(1).ok_or(SketchError::Internal)?;
    let columns = level.checked_add(1).ok_or(SketchError::Internal)?;
    let mut coefficients: Vec<f64> = Vec::new();
    coefficients
        .try_reserve_exact(level.checked_mul(columns).ok_or(SketchError::Internal)?)
        .map_err(|_| SketchError::OutOfMemory)?;
    for k in 1..=level {
        for i in 0..columns {
            // A rational end can read more derivatives than the degree: the basis functions
            // past the degree are not there, and their numbers are zero.
            coefficients.push(if i < width {
                k.checked_mul(width)
                    .and_then(|r| r.checked_add(i))
                    .and_then(|j| table.get(j))
                    .copied()
                    .ok_or(SketchError::Internal)?
            } else {
                0.0
            });
        }
    }
    ids.truncate(columns);
    // Fewer control points than columns: the missing ones repeat the first, with zero numbers.
    while ids.len() < columns {
        let first = *ids.first().ok_or(SketchError::Internal)?;
        ids.push(first);
    }
    Ok((ids, coefficients))
}

/// For the `curve` of the spline `spline` at parameter `u`: for each control point, the number that multiplies it
/// in C(u), and the number that multiplies it in C′(u). Both lists have a number for every control
/// point, zero for the ones whose basis function vanishes at `u`.
///
/// # Errors
///
/// [`SketchError::WrongKind`] if `u` is outside the domain of `curve`;
/// [`SketchError::OutOfMemory`].
pub(crate) fn point_coefficients(
    curve: &Curve<2>,
    spline: ItemId,
    u: f64,
) -> Result<(Vec<f64>, Vec<f64>), SketchError> {
    let weights: Vec<f64> = curve.weights().map(<[f64]>::to_vec).unwrap_or_default();
    let (first, table) = rows(curve.basis(), &weights, u, 1, spline)?;
    let n = curve.basis().count();
    let width = curve
        .basis()
        .degree()
        .checked_add(1)
        .ok_or(SketchError::Internal)?;
    let mut value: Vec<f64> = Vec::new();
    let mut slope: Vec<f64> = Vec::new();
    value
        .try_reserve_exact(n)
        .and_then(|()| slope.try_reserve_exact(n))
        .map_err(|_| SketchError::OutOfMemory)?;
    value.resize(n, 0.0);
    slope.resize(n, 0.0);
    for i in 0..width {
        let at = first.checked_add(i).ok_or(SketchError::Internal)?;
        *value.get_mut(at).ok_or(SketchError::Internal)? =
            table.get(i).copied().ok_or(SketchError::Internal)?;
        *slope.get_mut(at).ok_or(SketchError::Internal)? = table
            .get(width.checked_add(i).ok_or(SketchError::Internal)?)
            .copied()
            .ok_or(SketchError::Internal)?;
    }
    Ok((value, slope))
}
