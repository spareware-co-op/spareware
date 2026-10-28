//! Least-squares approximation of curves and surfaces (The NURBS Book, sections 9.4.1 to 9.4.3):
//! fewer control points than data points, the curve or surface nearest the data in the
//! least-squares sense, with ends or other constraints met exactly.
//!
//! What it does: [`Curve::least_squares`] fits a curve of n + 1 control points to m + 1 data
//! points: parameters by the rule asked for, knots by equations 9.68 and 9.69, the end points
//! interpolated and the interior control points minimising Σ |`Q_k` − C(`u_k`)|². [`Curve::least_squares_at`]
//! is algorithm A9.6's general form: weighted data at the caller's parameters on the caller's
//! basis, with points and derivatives anywhere met exactly ([`Constraint`]). [`Surface::least_squares`]
//! fits a grid (A9.7): least squares through each column of the grid, then through each row of
//! the result, the corners interpolated. Each returns its certified error (`crate::fit`).
//!
//! Guarantees:
//!
//! - **By QR, never normal equations.** Every system goes through `geom-linalg`'s complete
//!   orthogonal decomposition (Householder QR with column pivoting), as chapter 15.6 asks: the
//!   book and `NLib` form Nᵀ·N, which squares the condition number. Constraints are met by the
//!   null-space method: a particular solution of the constraints and a basis of their null space
//!   from one decomposition, then least squares in that space.
//! - **Determined or refused.** A system whose numerical rank is below its number of unknowns
//!   (some span with no data, the Schoenberg–Whitney condition broken) is
//!   [`NurbsError::Undetermined`], decided by a rank tolerance of max(rows, columns)·u relative to
//!   the largest pivot, `NumPy`'s default for `lstsq` (<https://numpy.org/doc/stable/reference/generated/numpy.linalg.lstsq.html>);
//!   constraints that repeat or contradict each other are [`NurbsError::Constraints`]. `NLib`
//!   solves on with whatever its LU gives.
//! - **Certified.** `error` bounds the distance of every data point from the returned curve at its
//!   parameter, and `constraint_error` how far each constraint is missed, both by interval
//!   evaluation (`crate::enclose`); the least-squares optimum is only as good as rounding allows,
//!   but the bounds are always true.
//! - **Deterministic.** `geom-linalg`'s fixed pivot and loop order; sums in index order.
//!
//! Method: Piegl and Tiller's least-squares curve and surface approximation, section 9.4.1
//! (equations 9.63 to 9.67), algorithm A9.6 (weighted and constrained) and A9.7 (surfaces) (The
//! NURBS Book, 2nd ed., Springer (1997), <https://doi.org/10.1007/978-3-642-59223-2>), solved by
//! the complete orthogonal decomposition and the null-space method for equality-constrained
//! least squares (G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins
//! (2013), sections 5.4.2, 5.5 and 6.2) in place of the book's normal equations and Lagrange
//! multipliers.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: the problem statements of `N_FitCrvApproxLstSq`,
//!   `N_FitCrvApproxKnots` and `N_FitCrvWeightedLstSq` (which end points are fixed, what is
//!   weighted, which constraints are allowed), solved by `geom-linalg`'s QR instead of `NLib`'s
//!   banded normal equations; constraints of any derivative order at any parameter; rank checked;
//!   every fit returns its certified error.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: a fit with m + 1 data points and n + 1 control points takes O(m·n²) for the
//! decomposition and O(m·n) per coordinate, charged to the meter first; a surface takes one
//! decomposition per direction and one solve per row or column and coordinate.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: Q and u for the data and their parameters, p for the degree, n and m for the last indices, b for the right-hand sides, x and z for the solution and the null space"
)]

use alloc::vec::Vec;

use geom_linalg::{Cod, LinalgError, Matrix, Work};
use geom_num::math;
use geom_num::real::Real;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::enclose::{curve_derivative, curve_errors, distance_bound, largest, surface_error};
use crate::fit::{
    Constraint, CurveFit, FitVersion, Parametrization, SurfaceFit, check_degree, curve_parameters,
    data, least_squares_knots, surface_parameters,
};
use crate::index::{add, get, mul, real, sub};
use crate::ops::larger;
use crate::point::Point;
use crate::surface::Surface;

/// A `geom-linalg` failure as the fit's error. The work for every call is charged to the meter
/// and handed over in full, so running out of it is a defect.
pub(crate) fn linalg(meter: &mut WorkMeter<'_>, e: LinalgError) -> NurbsError {
    match e {
        LinalgError::Singular | LinalgError::NotPositiveDefinite => NurbsError::Undetermined,
        LinalgError::NotFinite => NurbsError::NotFinite,
        LinalgError::OutOfMemory => NurbsError::Stopped(meter.out_of_memory()),
        LinalgError::Shape | LinalgError::OutOfWork | LinalgError::Defect => NurbsError::Defect,
    }
}

/// `n` as a `u64` for costs.
fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The rank tolerance for an m × n system: max(m, n)·u, u = 2^−53.
fn rank_tolerance(m: usize, n: usize) -> Result<Real, NurbsError> {
    let k = real(m.max(n))?;
    Ok(Real::new(k * f64::EPSILON / 2.0)?)
}

/// A decomposition of `a` with work for it and `solves` least-squares solves, charged to the
/// meter; the [`Work`] left covers the solves.
fn decompose(
    meter: &mut WorkMeter<'_>,
    a: &Matrix,
    solves: usize,
) -> Result<(Cod, Work), NurbsError> {
    let (m, n) = (wide(a.rows()), wide(a.cols()));
    let factor = m
        .saturating_mul(n)
        .saturating_mul(m.min(n))
        .saturating_mul(2)
        .saturating_add(n.saturating_mul(n).saturating_mul(n));
    let solve = m.saturating_mul(n).saturating_mul(2);
    let null = n.saturating_mul(n).saturating_mul(n);
    let total = factor
        .saturating_add(solve.saturating_mul(wide(solves)))
        .saturating_add(null);
    meter.charge(total)?;
    let mut work = Work::new(total);
    let tol = rank_tolerance(a.rows(), a.cols())?;
    let cod = Cod::factor(a, tol, &mut work).map_err(|e| linalg(meter, e))?;
    Ok((cod, work))
}

/// A matrix of `rows` × `cols` zeros, its memory reserved on the meter.
fn matrix(meter: &mut WorkMeter<'_>, rows: usize, cols: usize) -> Result<Matrix, NurbsError> {
    meter.reserve(wide(rows).saturating_mul(wide(cols)).saturating_mul(8))?;
    Matrix::zeros(rows, cols).map_err(|e| linalg(meter, e))
}

/// The least-squares fit with fixed ends: data rows k = 0 to m of `width` values each (`b`, row by
/// row) at parameters `u` on the clamped `basis`, whose ends are the first and last parameter.
/// Row 0 and row m are the first and last control rows; the n − 1 interior ones minimise the sum of
/// squares (equations 9.63 to 9.67). Returns the n + 1 control rows, row by row.
pub(crate) fn fixed_ends(
    meter: &mut WorkMeter<'_>,
    b: &[f64],
    width: usize,
    u: &[f64],
    basis: &Basis,
) -> Result<Vec<f64>, NurbsError> {
    let rows = u.len();
    let count = basis.count();
    if rows < 2 || b.len() != mul(rows, width)? || count < 2 || count > rows {
        return Err(NurbsError::Defect);
    }
    let p = basis.degree();
    let (m, n) = (sub(rows, 1)?, sub(count, 1)?);
    let first = b.get(..width).ok_or(NurbsError::Defect)?;
    let last = b
        .get(mul(m, width)?..mul(rows, width)?)
        .ok_or(NurbsError::Defect)?;
    let mut out = vec_for(meter, mul(count, width)?)?;
    out.extend_from_slice(first);
    if n >= 2 {
        let (inner_rows, inner_cols) = (sub(m, 1)?, sub(n, 1)?);
        let mut a = matrix(meter, inner_rows, inner_cols)?;
        // The right-hand sides, by column: R_k = Q_k − N_0(u_k)·Q_0 − N_n(u_k)·Q_m.
        let mut rhs = vec_for(meter, mul(inner_rows, width)?)?;
        let mut vals = [0.0; WIDTH];
        let len = add(p, 1)?;
        for k in 1..m {
            let span = basis.eval(
                get(u, k)?,
                Limit::FromRight,
                vals.get_mut(..len).ok_or(NurbsError::Defect)?,
            )?;
            let (mut n0, mut nn) = (0.0, 0.0);
            for (t, &v) in vals.iter().take(len).enumerate() {
                let i = add(sub(span, p)?, t)?;
                if i == 0 {
                    n0 = v;
                } else if i == n {
                    nn = v;
                } else {
                    a.set(sub(k, 1)?, sub(i, 1)?, v)
                        .map_err(|e| linalg(meter, e))?;
                }
            }
            for c in 0..width {
                let q = get(b, add(mul(k, width)?, c)?)?;
                rhs.push(q - n0 * get(first, c)? - nn * get(last, c)?);
            }
        }
        let (cod, mut work) = decompose(meter, &a, width)?;
        if cod.rank() < inner_cols {
            return Err(NurbsError::Undetermined);
        }
        let mut solutions = vec_for(meter, width)?;
        let mut column = vec_for(meter, inner_rows)?;
        for c in 0..width {
            column.clear();
            for k in 0..inner_rows {
                column.push(get(&rhs, add(mul(k, width)?, c)?)?);
            }
            solutions.push(
                cod.least_squares(&column, &mut work)
                    .map_err(|e| linalg(meter, e))?,
            );
        }
        for i in 0..inner_cols {
            for x in &solutions {
                out.push(get(x, i)?);
            }
        }
    }
    out.extend_from_slice(last);
    if !out.iter().all(|x| x.is_finite()) {
        return Err(NurbsError::NotFinite);
    }
    Ok(out)
}

/// Rows of points back as points.
fn points_of<const D: usize>(
    meter: &mut WorkMeter<'_>,
    flat: &[f64],
) -> Result<Vec<[f64; D]>, NurbsError> {
    let mut out = vec_for(meter, flat.len().checked_div(D).unwrap_or(0))?;
    for chunk in flat.chunks_exact(D.max(1)) {
        let mut c = [0.0; D];
        c.copy_from_slice(chunk.get(..D).ok_or(NurbsError::Defect)?);
        out.push(c);
    }
    Ok(out)
}

/// The fit of `curve` to `q` at `u`, its errors certified.
fn finish<const D: usize>(
    meter: &mut WorkMeter<'_>,
    curve: Curve<D>,
    q: &[[f64; D]],
    u: Vec<f64>,
    constraints: &[Constraint<D>],
) -> Result<CurveFit<D>, NurbsError> {
    let error = largest(&curve_errors(meter, &curve, q, &u)?);
    let mut constraint_error = 0.0_f64;
    for c in constraints {
        constraint_error = larger(
            constraint_error,
            distance_bound(c.value, curve_derivative(&curve, c.parameter, c.order)?)?,
        );
    }
    Ok(CurveFit {
        curve,
        parameters: u,
        error,
        constraint_error,
        version: FitVersion::V1,
    })
}

impl<const D: usize> Curve<D> {
    /// The curve of degree `degree` with `count` control points nearest `points` in the
    /// least-squares sense, its ends at the first and last point: parameters by `rule`, knots by
    /// equations 9.68 and 9.69 (section 9.4.1), with its certified error.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`]; [`NurbsError::DataCount`] for fewer than two points;
    /// [`NurbsError::ControlCount`] unless p + 1 ≤ `count` ≤ the number of points;
    /// [`NurbsError::DataNotFinite`]; [`NurbsError::CoincidentData`];
    /// [`NurbsError::Undetermined`]; [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn least_squares<P: Point<D>>(
        points: &[P],
        count: usize,
        degree: usize,
        rule: Parametrization,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CurveFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        check_degree(degree)?;
        let q = data(meter, points)?;
        let u = curve_parameters(meter, &q, rule)?;
        let basis = least_squares_knots(&u, count, degree, meter)?;
        let flat: Vec<f64> = q.iter().flatten().copied().collect();
        let ctrl = fixed_ends(meter, &flat, D, &u, &basis)?;
        let pts = points_of(meter, &ctrl)?;
        let curve = Curve::from_homogeneous(basis, pts, None)?;
        finish(meter, curve, &q, u, &[])
    }

    /// The curve on `basis` minimising Σ `w_k`·|`Q_k` − C(`u_k`)|² over `points` at `parameters`
    /// (weights 1 when `weights` is `None`) among the curves that meet every constraint exactly
    /// (algorithm A9.6), with its certified errors. Nothing else is fixed: to keep the ends, pass
    /// them as constraints of order 0. Parameters may come in any order, inside the domain.
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] unless there are as many parameters (and weights) as points;
    /// [`NurbsError::DataNotFinite`] for a point, or a constraint's value (indexed after the
    /// points); [`NurbsError::DataWeight`]; [`NurbsError::ParameterOutside`] for a parameter or a
    /// constraint outside the domain; [`NurbsError::Constraints`] for more constraints than
    /// control points, or constraints that repeat or contradict each other;
    /// [`NurbsError::Undetermined`] if the data with the constraints do not determine the curve;
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn least_squares_at<P: Point<D>>(
        points: &[P],
        weights: Option<&[f64]>,
        parameters: &[f64],
        basis: Basis,
        constraints: &[Constraint<D>],
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CurveFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        let q = data(meter, points)?;
        for given in [Some(parameters.len()), weights.map(<[f64]>::len)]
            .into_iter()
            .flatten()
        {
            if given != q.len() {
                return Err(NurbsError::BufferLength {
                    given,
                    needed: q.len(),
                });
            }
        }
        let mut scale = vec_for(meter, q.len())?;
        for index in 0..q.len() {
            let w = match weights {
                Some(w) => get(w, index)?,
                None => 1.0,
            };
            if !(w.is_finite() && w > 0.0) {
                return Err(NurbsError::DataWeight { index });
            }
            scale.push(math::sqrt(w)?.get());
        }
        for (i, c) in constraints.iter().enumerate() {
            if !c.value.iter().all(|x| x.is_finite()) {
                return Err(NurbsError::DataNotFinite {
                    index: add(q.len(), i)?,
                });
            }
        }
        let count = basis.count();
        if constraints.len() > count {
            return Err(NurbsError::Constraints);
        }
        let mut u = vec_for(meter, parameters.len())?;
        u.extend_from_slice(parameters);
        let x = constrained(meter, &q, &scale, &u, &basis, constraints)?;
        let pts = points_of(meter, &x)?;
        let curve = Curve::from_homogeneous(basis, pts, None)?;
        finish(meter, curve, &q, u, constraints)
    }
}

/// The collocation matrix of `basis` at `u`, row k scaled by `scale[k]`.
fn collocation(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    u: &[f64],
    scale: &[f64],
) -> Result<Matrix, NurbsError> {
    let p = basis.degree();
    let mut a = matrix(meter, u.len(), basis.count())?;
    let mut vals = [0.0; WIDTH];
    let len = add(p, 1)?;
    for (k, (&x, &s)) in u.iter().zip(scale).enumerate() {
        let span = basis.eval(
            x,
            Limit::FromRight,
            vals.get_mut(..len).ok_or(NurbsError::Defect)?,
        )?;
        for (t, &v) in vals.iter().take(len).enumerate() {
            a.set(k, add(sub(span, p)?, t)?, s * v)
                .map_err(|e| linalg(meter, e))?;
        }
    }
    Ok(a)
}

/// The constraint matrix: one row per constraint, the derivatives of the basis functions of the
/// constraint's order at its parameter.
fn constraint_matrix<const D: usize>(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    constraints: &[Constraint<D>],
) -> Result<Matrix, NurbsError> {
    let p = basis.degree();
    let mut m_mat = matrix(meter, constraints.len(), basis.count())?;
    let mut ders: Vec<f64> = Vec::new();
    for (row, c) in constraints.iter().enumerate() {
        let len = mul(add(c.order, 1)?, add(p, 1)?)?;
        ders.clear();
        ders.try_reserve_exact(len)
            .map_err(|_| NurbsError::Stopped(meter.out_of_memory()))?;
        ders.resize(len, 0.0);
        let span = basis.eval_derivs(c.parameter, Limit::FromRight, c.order, &mut ders)?;
        let start = mul(c.order, add(p, 1)?)?;
        for t in 0..=p {
            let v = get(&ders, add(start, t)?)?;
            m_mat
                .set(row, add(sub(span, p)?, t)?, v)
                .map_err(|e| linalg(meter, e))?;
        }
    }
    Ok(m_mat)
}

/// The least-squares coefficients y of each coordinate in the null space `z` of the
/// constraints: minimise |N·(`x_p` + Z·y) − b| for each particular solution `x_p` and scaled data
/// column b.
fn null_space_fit(
    meter: &mut WorkMeter<'_>,
    n_mat: &Matrix,
    z: &Matrix,
    xp: &[Vec<f64>],
    columns: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, NurbsError> {
    let (rows, count, free) = (n_mat.rows(), n_mat.cols(), z.cols());
    meter.charge(
        wide(rows)
            .saturating_mul(wide(count))
            .saturating_mul(wide(free).saturating_add(wide(xp.len()))),
    )?;
    // B = (scaled N)·Z.
    let mut b = matrix(meter, rows, free)?;
    for k in 0..rows {
        let row = n_mat.row(k).map_err(|e| linalg(meter, e))?;
        for j in 0..free {
            let mut sum = 0.0;
            for (i, &a) in row.iter().enumerate() {
                sum += a * z.get(i, j).map_err(|e| linalg(meter, e))?;
            }
            b.set(k, j, sum).map_err(|e| linalg(meter, e))?;
        }
    }
    let (cod_b, mut work_b) = decompose(meter, &b, xp.len())?;
    if cod_b.rank() < free {
        return Err(NurbsError::Undetermined);
    }
    let mut ys = vec_for(meter, xp.len())?;
    for (xpd, column) in xp.iter().zip(columns) {
        let fitted = n_mat.mul_vec(xpd).map_err(|e| linalg(meter, e))?;
        let mut c = vec_for(meter, column.len())?;
        for (ck, f) in column.iter().zip(&fitted) {
            c.push(ck - f);
        }
        ys.push(
            cod_b
                .least_squares(&c, &mut work_b)
                .map_err(|e| linalg(meter, e))?,
        );
    }
    Ok(ys)
}

/// Writes coordinate `d` of control point `i`, refusing a value that is not finite.
fn put<const D: usize>(x: &mut [f64], i: usize, d: usize, v: f64) -> Result<(), NurbsError> {
    if !v.is_finite() {
        return Err(NurbsError::NotFinite);
    }
    *x.get_mut(add(mul(i, D)?, d)?).ok_or(NurbsError::Defect)? = v;
    Ok(())
}

/// The control points, row by row, of A9.6's problem by the null-space method.
fn constrained<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    scale: &[f64],
    u: &[f64],
    basis: &Basis,
    constraints: &[Constraint<D>],
) -> Result<Vec<f64>, NurbsError> {
    let count = basis.count();
    let n_mat = collocation(meter, basis, u, scale)?;
    // The scaled data, one column per coordinate.
    let mut columns = vec_for(meter, D)?;
    for d in 0..D {
        let mut c = vec_for(meter, q.len())?;
        for (qk, &s) in q.iter().zip(scale) {
            c.push(s * get(qk, d)?);
        }
        columns.push(c);
    }
    let mut x = vec_for(meter, mul(count, D)?)?;
    x.resize(mul(count, D)?, 0.0);
    if constraints.is_empty() {
        let (cod, mut work) = decompose(meter, &n_mat, D)?;
        if cod.rank() < count {
            return Err(NurbsError::Undetermined);
        }
        for (d, column) in columns.iter().enumerate() {
            let sol = cod
                .least_squares(column, &mut work)
                .map_err(|e| linalg(meter, e))?;
            for (i, &v) in sol.iter().enumerate() {
                put::<D>(&mut x, i, d, v)?;
            }
        }
        return Ok(x);
    }
    let s = constraints.len();
    let m_mat = constraint_matrix(meter, basis, constraints)?;
    let (cod_m, mut work_m) = decompose(meter, &m_mat, D)?;
    if cod_m.rank() < s {
        return Err(NurbsError::Constraints);
    }
    // A particular solution of the constraints, coordinate by coordinate.
    let mut xp = vec_for(meter, D)?;
    for d in 0..D {
        let mut t = vec_for(meter, s)?;
        for c in constraints {
            t.push(get(&c.value, d)?);
        }
        xp.push(
            cod_m
                .least_squares(&t, &mut work_m)
                .map_err(|e| linalg(meter, e))?,
        );
    }
    if count == s {
        for (d, xpd) in xp.iter().enumerate() {
            for (i, &v) in xpd.iter().enumerate() {
                put::<D>(&mut x, i, d, v)?;
            }
        }
        return Ok(x);
    }
    let z = cod_m
        .null_space(&mut work_m)
        .map_err(|e| linalg(meter, e))?;
    let ys = null_space_fit(meter, &n_mat, &z, &xp, &columns)?;
    for (d, (xpd, y)) in xp.iter().zip(&ys).enumerate() {
        for (i, &base) in xpd.iter().enumerate() {
            let mut v = base;
            for (j, &yj) in y.iter().enumerate() {
                v += z.get(i, j).map_err(|e| linalg(meter, e))? * yj;
            }
            put::<D>(&mut x, i, d, v)?;
        }
    }
    Ok(x)
}

impl<const D: usize> Surface<D> {
    /// The surface of degrees `degrees` with `counts` control points in u and v nearest a grid of
    /// `size.0` rows of `size.1` points in the least-squares sense (A9.7): parameters by `rule`
    /// averaged over the grid, knots by equations 9.68 and 9.69 in each direction, least squares
    /// with fixed ends through each column of the grid, then through each row of the result, so
    /// the corners are interpolated; with its certified error.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`]; [`NurbsError::PointCount`] if the grid does not hold rows × columns
    /// points; [`NurbsError::DataCount`] for fewer than two rows or columns;
    /// [`NurbsError::ControlCount`] unless each count is from degree + 1 to the number of rows
    /// or columns; [`NurbsError::DataNotFinite`]; [`NurbsError::CoincidentData`];
    /// [`NurbsError::Undetermined`]; [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn least_squares<P: Point<D>>(
        points: &[P],
        size: (usize, usize),
        counts: (usize, usize),
        degrees: (usize, usize),
        rule: Parametrization,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<SurfaceFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        check_degree(degrees.0)?;
        check_degree(degrees.1)?;
        let q = data(meter, points)?;
        let (u, v) = surface_parameters(meter, &q, size, rule)?;
        let (rows, cols) = size;
        let bu = least_squares_knots(&u, counts.0, degrees.0, meter)?;
        let bv = least_squares_knots(&v, counts.1, degrees.1, meter)?;
        let (nu, nv) = (bu.count(), bv.count());
        // Along u: each grid row is a data row holding every column.
        let flat: Vec<f64> = q.iter().flatten().copied().collect();
        let r = fixed_ends(meter, &flat, mul(cols, D)?, &u, &bu)?;
        // Along v: transpose, so column l of R is data row l, holding every u control row.
        let mut t = vec_for(meter, r.len())?;
        for l in 0..cols {
            for i in 0..nu {
                let start = add(mul(i, mul(cols, D)?)?, mul(l, D)?)?;
                t.extend_from_slice(r.get(start..add(start, D)?).ok_or(NurbsError::Defect)?);
            }
        }
        let pv = fixed_ends(meter, &t, mul(nu, D)?, &v, &bv)?;
        let mut net = vec_for(meter, mul(nu, nv)?)?;
        for i in 0..nu {
            for j in 0..nv {
                let start = add(mul(j, mul(nu, D)?)?, mul(i, D)?)?;
                let mut c = [0.0; D];
                c.copy_from_slice(pv.get(start..add(start, D)?).ok_or(NurbsError::Defect)?);
                net.push(c);
            }
        }
        let _ = rows;
        let surface = Surface::from_homogeneous(bu, bv, net, None)?;
        let error = surface_error(meter, &surface, &q, (&u, &v))?;
        Ok(SurfaceFit {
            surface,
            u,
            v,
            error,
            version: FitVersion::V1,
        })
    }
}
