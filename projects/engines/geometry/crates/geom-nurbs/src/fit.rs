//! Fitting NURBS to data (The NURBS Book, chapter 9): what every fit shares, the parameters of the
//! data points, and the knot vectors built from them.
//!
//! What it does: a fit takes data points `Q_k` and returns a [`CurveFit`] or a [`SurfaceFit`]: the
//! nonrational curve or surface, the parameter of each data point, and `error`, a certified upper
//! bound on `max_k` |`Q_k` − C(`u_k`)| at those parameters (so also on the distance from every data
//! point to the curve). [`parameters`] gives a polygon's parameters by the uniform, chord-length
//! or centripetal rule, and [`grid_parameters`] a grid's, averaged over its rows and columns.
//! [`averaged_knots`] builds the knot vector of an interpolation, each interior knot the average
//! of p consecutive parameters, and [`least_squares_knots`] that of a least-squares fit with fewer
//! control points than data points, placed so every knot span holds a parameter. Every fit takes a
//! [`FitVersion`]: the operation log records it, and a change to any fit's result is a new version
//! while every old one keeps its behaviour (chapter 26.2).
//!
//! Guarantees:
//!
//! - **Certified errors.** Every fit's `error` is computed after the fit, by evaluating the curve
//!   or surface at each data parameter in interval arithmetic (`crate::enclose`), so it bounds the
//!   exact distance from each data point to the exact curve the returned doubles define. A fit
//!   never reports a smaller error than it has, and a fit to a stated error that cannot meet it is
//!   [`NurbsError::OutsideTolerance`], never a best-so-far curve (chapter 13.21).
//! - **Parameters on [0, 1], strictly increasing.** The first parameter is exactly 0 and the last
//!   exactly 1. Two neighbouring points whose parameters would be equal (they coincide, or round to
//!   one value) are [`NurbsError::CoincidentData`]; `NLib` divides by a zero total in silence or
//!   leaves equal parameters that make its systems singular.
//! - **Knots for the data.** [`averaged_knots`] gives the knots under which interpolation at the
//!   parameters is well posed (Schoenberg–Whitney's condition holds); [`least_squares_knots`]
//!   gives knots under which every span holds at least one parameter, so the least-squares system
//!   has full rank. Both are clamped on [0, 1].
//! - **Deterministic.** Sums in index order; distances by a scaled sum of squares and `geom-num`'s
//!   correctly rounded square root; no fused multiply-add.
//!
//! Method: The NURBS Book, 2nd ed., section 9.2.1: the uniform rule, chord length (equations 9.4
//! and 9.5) and the centripetal rule (equation 9.6, after E. T. Y. Lee, "Choosing nodes in
//! parametric curve interpolation", *Computer-Aided Design* 21(6) (1989), 363–370,
//! <https://doi.org/10.1016/0010-4485(89)90003-1>); knots by averaging (equation 9.8); a grid's
//! parameters averaged over its rows and columns (algorithm A9.3); and section 9.4.1's knot
//! placement for least squares (equations 9.68 and 9.69). L. Piegl and W. Tiller, *The NURBS
//! Book*, 2nd ed., Springer (1997), <https://doi.org/10.1007/978-3-642-59223-2>, as `NLib`
//! implements them and as `geomdl` (MIT) numbers them.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_FitCalcCrvParamValues`,
//!   `N_FitCrvCalcKnotVector` and `N_FitCalcKnotVectorEndDerivs`; parameters that do not
//!   increase are an error instead of a silent zero division; the least-squares knots follow the
//!   book's equations 9.68 and 9.69, where `N_FitCalcKnotVectorCrvApprox` averages clusters of
//!   parameters, with the position j·(m + 1)/(n − p + 1) split in integers so each knot is
//!   rounded once.
//! Consulted: NURBS-Python (geomdl), MIT,
//!   <https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py>,
//!   for the book's equation numbers of the parameter rules and the least-squares knot placement.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: linear in the number of data points (times D for the distances), plus p additions per
//! averaged knot; each result is one allocation, charged to the meter first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: Q and u for the data and their parameters, p for the degree, m for the last index, d for chords"
)]

use alloc::vec::Vec;

use geom_num::math;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::basis::{Basis, MAX_DEGREE};
use crate::budget::{charge, vec_for};
use crate::curve::Curve;
use crate::index::{add, get, mul, real, sub};
use crate::point::Point;
use crate::surface::Surface;

/// The version of the fitting operations a result is made by (chapter 26.2): a fix or improvement
/// that changes any fit's result is a new variant, and every earlier variant keeps its behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FitVersion {
    /// The first version (piece G58).
    V1,
}

impl FitVersion {
    /// The version new operations record.
    pub const CURRENT: FitVersion = FitVersion::V1;

    /// The version's number, as the operation log records it.
    #[must_use]
    pub const fn number(self) -> u32 {
        match self {
            FitVersion::V1 => 1,
        }
    }
}

/// How the parameters of data points are chosen (The NURBS Book, section 9.2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Parametrization {
    /// Equally spaced: `u_k` = k / m. Only for data spaced about evenly; uneven data gives loops.
    Uniform,
    /// Proportional to the length of the polygon through the points (equations 9.4 and 9.5): the
    /// usual choice.
    ChordLength,
    /// Proportional to the square roots of the chord lengths (equation 9.6, Lee 1989): better for
    /// data that turns sharply.
    Centripetal,
}

/// How a fit to a stated error places knots when it raises the degree (section 9.4.4): single
/// interior knots give a smooth curve; repeated ones keep corners and straight runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnotMode {
    /// Each interior knot stays single: the curve is as smooth as its degree allows.
    Single,
    /// Each knot's multiplicity rises with the degree, so the continuity at each knot stays what
    /// it was: corners and straight runs of the data are kept.
    Multiple,
}

/// A requirement a least-squares fit meets exactly (algorithm A9.6): the curve's derivative of
/// `order` at `parameter` is `value` (order 0: the curve passes through the point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraint<const D: usize> {
    /// Where the requirement holds, inside the domain.
    pub parameter: f64,
    /// The derivative's order: 0 for a point, 1 for a first derivative, and so on.
    pub order: usize,
    /// The point or derivative the curve must have there.
    pub value: [f64; D],
}

/// A curve fitted to data points.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveFit<const D: usize> {
    /// The fitted curve: nonrational, clamped.
    pub curve: Curve<D>,
    /// The parameter of each data point, in the data's order.
    pub parameters: Vec<f64>,
    /// A certified upper bound on `max_k` |`Q_k` − C(`u_k`)|, so on every data point's distance from
    /// the curve.
    pub error: f64,
    /// A certified upper bound on how far the curve misses each derivative or constraint the fit
    /// had to meet (the norm of the difference); 0 when it had none.
    pub constraint_error: f64,
    /// The version that made it.
    pub version: FitVersion,
}

/// A surface fitted to a grid of data points.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceFit<const D: usize> {
    /// The fitted surface: nonrational, clamped in both directions.
    pub surface: Surface<D>,
    /// The parameter of each row of the grid.
    pub u: Vec<f64>,
    /// The parameter of each column of the grid.
    pub v: Vec<f64>,
    /// A certified upper bound on max |`Q_kl` − S(`u_k`, `v_l`)| over the grid.
    pub error: f64,
    /// The version that made it.
    pub version: FitVersion,
}

/// The data points' coordinates, each checked finite.
pub(crate) fn data<P: Point<D>, const D: usize>(
    meter: &mut WorkMeter<'_>,
    points: &[P],
) -> Result<Vec<[f64; D]>, NurbsError> {
    charge(meter, points.len(), D, false, 0)?;
    let mut out = vec_for(meter, points.len())?;
    for (index, p) in points.iter().enumerate() {
        let c = p.coords();
        if !c.iter().all(|x| x.is_finite()) {
            return Err(NurbsError::DataNotFinite { index });
        }
        out.push(c);
    }
    Ok(out)
}

/// A degree from 1 to [`MAX_DEGREE`], or [`NurbsError::Degree`].
pub(crate) fn check_degree(degree: usize) -> Result<(), NurbsError> {
    if degree == 0 || degree > MAX_DEGREE {
        return Err(NurbsError::Degree { degree });
    }
    Ok(())
}

/// |a − b|, by a sum of squares scaled by the largest coordinate difference, so it neither
/// overflows nor underflows while the differences are finite.
pub(crate) fn distance<const D: usize>(a: [f64; D], b: [f64; D]) -> Result<f64, NurbsError> {
    let mut d = [0.0; D];
    let mut scale = 0.0_f64;
    for (x, (p, q)) in d.iter_mut().zip(a.iter().zip(&b)) {
        *x = p - q;
        if !x.is_finite() {
            return Err(NurbsError::NotFinite);
        }
        if x.abs() > scale {
            scale = x.abs();
        }
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut sum = 0.0;
    for x in d {
        let s = x / scale;
        sum += s * s;
    }
    let r = scale * math::sqrt(sum)?.get();
    if r.is_finite() {
        Ok(r)
    } else {
        Err(NurbsError::NotFinite)
    }
}

/// The chords between neighbouring points: |`Q_k` − `Q_(k−1)`| for k = 1 to m, or their square
/// roots for the centripetal rule; `None` for the uniform rule.
fn chords<const D: usize>(
    meter: &mut WorkMeter<'_>,
    points: impl Iterator<Item = [f64; D]> + Clone,
    count: usize,
    rule: Parametrization,
) -> Result<Option<Vec<f64>>, NurbsError> {
    if rule == Parametrization::Uniform {
        return Ok(None);
    }
    let mut out = vec_for(meter, count.saturating_sub(1))?;
    let mut previous: Option<[f64; D]> = None;
    for q in points {
        if let Some(p) = previous {
            let d = distance(q, p)?;
            out.push(if rule == Parametrization::Centripetal {
                math::sqrt(d)?.get()
            } else {
                d
            });
        }
        previous = Some(q);
    }
    Ok(Some(out))
}

/// Parameters from chords: `u_k` = (`d_1` + … + `d_k`) / (`d_1` + … + `d_m`), or k / m without
/// chords; `None` if the chords sum to zero.
fn from_chords(
    meter: &mut WorkMeter<'_>,
    chords: Option<&[f64]>,
    count: usize,
) -> Result<Option<Vec<f64>>, NurbsError> {
    let mut u = vec_for(meter, count)?;
    let last = sub(count, 1)?;
    match chords {
        None => {
            let m = real(last)?;
            for k in 0..count {
                u.push(real(k)? / m);
            }
        }
        Some(d) => {
            let total = d.iter().fold(0.0, |s, x| s + x);
            if !total.is_finite() {
                return Err(NurbsError::NotFinite);
            }
            if total == 0.0 {
                return Ok(None);
            }
            u.push(0.0);
            let mut sum = 0.0;
            for (k, x) in d.iter().enumerate() {
                sum += x;
                // The last parameter is exactly 1, whatever the rounding of the running sum.
                u.push(if add(k, 1)? == last { 1.0 } else { sum / total });
            }
        }
    }
    Ok(Some(u))
}

/// The index of the first parameter that is not above the one before it, if any.
fn first_not_increasing(u: &[f64]) -> Option<usize> {
    u.windows(2)
        .position(|w| matches!(w, [a, b] if b <= a))
        .map(|k| k.saturating_add(1))
}

/// The parameters of a polygon's points on [0, 1] by `rule` (The NURBS Book, section 9.2.1).
///
/// # Errors
///
/// [`NurbsError::DataCount`] for fewer than two points; [`NurbsError::DataNotFinite`];
/// [`NurbsError::CoincidentData`] where two neighbouring points give one parameter (for chord
/// length and the centripetal rule, also when every point coincides); [`NurbsError::NotFinite`] if
/// a chord overflows; [`NurbsError::Stopped`] if the meter stops it.
pub fn parameters<P: Point<D>, const D: usize>(
    points: &[P],
    rule: Parametrization,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<f64>, NurbsError> {
    let q = data(meter, points)?;
    curve_parameters(meter, &q, rule)
}

/// [`parameters`] on checked coordinates.
pub(crate) fn curve_parameters<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    rule: Parametrization,
) -> Result<Vec<f64>, NurbsError> {
    if q.len() < 2 {
        return Err(NurbsError::DataCount {
            points: q.len(),
            needed: 2,
        });
    }
    let d = chords(meter, q.iter().copied(), q.len(), rule)?;
    let u = from_chords(meter, d.as_deref(), q.len())?
        .ok_or(NurbsError::CoincidentData { index: 1 })?;
    match first_not_increasing(&u) {
        Some(index) => Err(NurbsError::CoincidentData { index }),
        None => Ok(u),
    }
}

/// The parameters of a grid's rows and columns on [0, 1] by `rule`: for each direction, the
/// parameters of every curve of the grid running that way, averaged over the curves that have a
/// length (The NURBS Book, algorithm A9.3). The grid holds `size.0` rows of `size.1` points, row
/// by row; the first list has one parameter per row, the second one per column.
///
/// # Errors
///
/// [`NurbsError::PointCount`] if the grid does not hold rows × columns points;
/// [`NurbsError::DataCount`] for fewer than two rows or columns; [`NurbsError::DataNotFinite`];
/// [`NurbsError::CoincidentData`] (with the row or column) where averaged parameters do not
/// increase, or where every curve one way has no length; [`NurbsError::NotFinite`];
/// [`NurbsError::Stopped`].
pub fn grid_parameters<P: Point<D>, const D: usize>(
    points: &[P],
    size: (usize, usize),
    rule: Parametrization,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<f64>, Vec<f64>), NurbsError> {
    let q = data(meter, points)?;
    surface_parameters(meter, &q, size, rule)
}

/// [`grid_parameters`] on checked coordinates.
pub(crate) fn surface_parameters<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    (rows, cols): (usize, usize),
    rule: Parametrization,
) -> Result<(Vec<f64>, Vec<f64>), NurbsError> {
    let expected = mul(rows, cols)?;
    if q.len() != expected {
        return Err(NurbsError::PointCount {
            points: q.len(),
            expected,
        });
    }
    let fewest = if rows < cols { rows } else { cols };
    if fewest < 2 {
        return Err(NurbsError::DataCount {
            points: fewest,
            needed: 2,
        });
    }
    // Along u, the curves are the grid's columns (stride `cols`); along v, its rows.
    let u = averaged_parameters(meter, rows, cols, rule, |k, l| {
        get(q, add(mul(k, cols)?, l)?)
    })?;
    let v = averaged_parameters(meter, cols, rows, rule, |l, k| {
        get(q, add(mul(k, cols)?, l)?)
    })?;
    Ok((u, v))
}

/// The averaged parameters of `curves` curves of `len` points, point k of curve l being
/// `at(k, l)`: each curve's own parameters, summed over the curves with a length, divided by
/// their number.
fn averaged_parameters<const D: usize>(
    meter: &mut WorkMeter<'_>,
    len: usize,
    curves: usize,
    rule: Parametrization,
    at: impl Fn(usize, usize) -> Result<[f64; D], NurbsError>,
) -> Result<Vec<f64>, NurbsError> {
    if rule == Parametrization::Uniform {
        return from_chords(meter, None, len)?.ok_or(NurbsError::Defect);
    }
    let mut sum = vec_for(meter, len)?;
    sum.resize(len, 0.0);
    let mut used = 0_usize;
    let mut column = vec_for(meter, len)?;
    for l in 0..curves {
        column.clear();
        for k in 0..len {
            column.push(at(k, l)?);
        }
        let d = chords(meter, column.iter().copied(), len, rule)?;
        if let Some(u) = from_chords(meter, d.as_deref(), len)? {
            for (s, x) in sum.iter_mut().zip(&u) {
                *s += x;
            }
            used = add(used, 1)?;
        }
    }
    if used == 0 {
        return Err(NurbsError::CoincidentData { index: 1 });
    }
    let n = real(used)?;
    let last = sub(len, 1)?;
    for (k, s) in sum.iter_mut().enumerate() {
        // Exactly 0 and 1 at the ends, as each curve's own parameters are.
        *s = if k == 0 {
            0.0
        } else if k == last {
            1.0
        } else {
            *s / n
        };
    }
    match first_not_increasing(&sum) {
        Some(index) => Err(NurbsError::CoincidentData { index }),
        None => Ok(sum),
    }
}

/// The knot vector for interpolating at `parameters` with degree `degree`: p + 1 copies of the
/// first and of the last parameter, and between them, for j = 1 to m − p, the average of the p
/// parameters from `u_j` on (The NURBS Book, equation 9.8).
///
/// # Errors
///
/// [`NurbsError::Degree`] for a degree of 0 or above [`MAX_DEGREE`]; [`NurbsError::DataCount`]
/// for fewer than p + 1 parameters; [`NurbsError::ParameterOrder`] if they do not increase or
/// are not finite; [`NurbsError::Stopped`].
pub fn averaged_knots(
    parameters: &[f64],
    degree: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Basis, NurbsError> {
    check_degree(degree)?;
    check_increasing(parameters, true)?;
    let count = parameters.len();
    if count <= degree {
        return Err(NurbsError::DataCount {
            points: count,
            needed: add(degree, 1)?,
        });
    }
    let interior = sub(count, add(degree, 1)?)?;
    averages(meter, parameters, degree, (1, interior), count)
}

/// The clamped knot vector of `points` control points with ends at the first and last parameter
/// and the interior knots, for j from `from` on (`count` of them), each the average of the p
/// parameters from `u_j` on.
fn averages(
    meter: &mut WorkMeter<'_>,
    u: &[f64],
    p: usize,
    (from, count): (usize, usize),
    points: usize,
) -> Result<Basis, NurbsError> {
    let len = add(points, add(p, 1)?)?;
    charge(meter, 0, 0, false, len)?;
    let mut knots = vec_for(meter, len)?;
    let first = get(u, 0)?;
    let last = get(u, sub(u.len(), 1)?)?;
    knots.resize(add(p, 1)?, first);
    let divisor = real(p)?;
    for j in from..add(from, count)? {
        let mut sum = 0.0;
        for i in j..add(j, p)? {
            sum += get(u, i)?;
        }
        knots.push(sum / divisor);
    }
    knots.resize(len, last);
    Basis::new(p, knots)
}

/// The knot vector for interpolating at `parameters` with derivatives given at both ends (The
/// NURBS Book, section 9.2.2, as `NLib`'s `N_FitCalcKnotVectorEndDerivs` builds it): m + 3
/// control points, and for j = 0 to m − p + 1 an interior knot averaging the p parameters from
/// `u_j` on. The degree is at least 2 and at most m + 2.
pub(crate) fn end_derivative_knots(
    meter: &mut WorkMeter<'_>,
    parameters: &[f64],
    degree: usize,
) -> Result<Basis, NurbsError> {
    let m = sub(parameters.len(), 1)?;
    // m − p + 2 interior knots: none when p = m + 2.
    let interior = sub(add(m, 2)?, degree)?;
    averages(meter, parameters, degree, (0, interior), add(m, 3)?)
}

/// The knot vector of a least-squares fit with `count` control points and degree `degree` to data
/// at `parameters`: clamped at the first and last parameter, with interior knots, for j = 1 to
/// n − p, at the position j·d along the parameters, d = (m + 1)/(n − p + 1), interpolated between
/// the two parameters around it (The NURBS Book, equations 9.68 and 9.69). Every knot span then
/// holds at least one parameter, the condition for the least-squares system to have full rank.
///
/// # Errors
///
/// [`NurbsError::Degree`]; [`NurbsError::ControlCount`] unless p + 1 ≤ `count` ≤ the number of
/// parameters; [`NurbsError::ParameterOrder`] if the parameters do not increase or are not
/// finite; [`NurbsError::Stopped`].
pub fn least_squares_knots(
    parameters: &[f64],
    count: usize,
    degree: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Basis, NurbsError> {
    check_degree(degree)?;
    check_increasing(parameters, true)?;
    let least = add(degree, 1)?;
    if count < least || count > parameters.len() {
        return Err(NurbsError::ControlCount {
            count,
            least,
            most: parameters.len(),
        });
    }
    let len = add(count, least)?;
    charge(meter, 0, 0, false, len)?;
    let mut knots = vec_for(meter, len)?;
    let start = get(parameters, 0)?;
    let end = get(parameters, sub(parameters.len(), 1)?)?;
    knots.resize(least, start);
    // j·d = j·(m + 1)/(n − p + 1) = i + α, with i and the numerator of α found exactly in
    // integers; n − p + 1 = count − p.
    let den = sub(count, degree)?;
    let den_real = real(den)?;
    for j in 1..den {
        let num = mul(j, parameters.len())?;
        let i = num.checked_div(den).ok_or(NurbsError::Defect)?;
        let alpha = real(num.checked_rem(den).ok_or(NurbsError::Defect)?)? / den_real;
        let lo = get(parameters, sub(i, 1)?)?;
        let hi = get(parameters, i)?;
        // lo + α·(hi − lo) is at least lo; it is kept at most hi, which rounding could pass.
        let knot = lo + alpha * (hi - lo);
        knots.push(if knot > hi { hi } else { knot });
    }
    knots.resize(len, end);
    Basis::new(degree, knots)
}

/// Checks that parameters are finite and increase: strictly if `strict`, otherwise never
/// decrease.
pub(crate) fn check_increasing(u: &[f64], strict: bool) -> Result<(), NurbsError> {
    let mut previous: Option<f64> = None;
    for (index, &x) in u.iter().enumerate() {
        if !x.is_finite() {
            return Err(NurbsError::ParameterOrder { index });
        }
        if let Some(p) = previous
            && (x < p || (strict && x == p))
        {
            return Err(NurbsError::ParameterOrder { index });
        }
        previous = Some(x);
    }
    Ok(())
}
