//! Global interpolation of curves and surfaces (The NURBS Book, sections 9.2.1, 9.2.2 and 9.2.5):
//! the curve through every data point, optionally with given end derivatives, and the surface
//! through a grid.
//!
//! What it does: [`Curve::interpolate`] chooses parameters by the rule asked for, knots by
//! averaging, and solves the collocation system Σ `N_i`(`u_k`)·`P_i` = `Q_k` for the control points
//! (A9.1). [`Curve::interpolate_at`] does the same at parameters and on a basis the caller gives.
//! [`Curve::interpolate_with_ends`] adds two control points so the curve also has given first
//! derivatives at its ends. [`Surface::interpolate`] interpolates a grid row by row and then
//! column by column through the intermediate points, one system per direction for all of them
//! (A9.4). Each returns its certified error (`crate::fit`).
//!
//! Guarantees:
//!
//! - **Interpolates, certified.** The error returned bounds the distance of every data point from
//!   the exact curve or surface of the returned doubles at its parameter; it is rounding, not
//!   approximation. With end derivatives, `constraint_error` bounds |C′(0) − `D_0`| and
//!   |C′(1) − `D_m`| the same way.
//! - **Well posed or refused.** Parameters that do not increase, or that leave a basis function
//!   with no parameter in its support (the Schoenberg–Whitney condition), are refused as
//!   [`NurbsError::ParameterOrder`] or [`NurbsError::Undetermined`]; `NLib` solves on and returns
//!   whatever the elimination gives.
//! - **Stable.** The systems are solved inside their band without pivoting (`crate::band`), which
//!   is backward stable for these totally positive matrices.
//! - **Deterministic.** Fixed orders throughout; the same data give the same bits on every target.
//!
//! Method: Piegl and Tiller's algorithms A9.1 and A9.4 and section 9.2.2 (The NURBS Book, 2nd ed.,
//! Springer (1997), <https://doi.org/10.1007/978-3-642-59223-2>), as `NLib` implements them;
//! the end derivatives enter as P₁ − P₀ = (`u_(p+1)` − `u_1`)/p · `D_0` and
//! `P_n` − `P_(n−1)` = (`u_(n+p)` − `u_n`)/p · `D_m`, the first and last derivative control points
//! of A3.3.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_FitCrvInterp`,
//!   `N_FitCrvInterpGivenParams` and `N_FitCrvDerivs`, for any dimension; solved by our own band
//!   elimination instead of `NLib`'s banded LU; missing end derivatives are an error where
//!   `N_FitCrvDerivs` estimates them from a cubic through four points; the end-derivative
//!   factors use the knots at both ends, where `NLib` assumes a domain of [0, 1]; every fit
//!   returns its certified error.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: O(n·p²·D) to solve a curve and O(n·p²) to certify it; a surface of r × s points costs
//! O(r·s·(p² + q²)·D); charged to the meter first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: Q and u for the data and their parameters, p and q for degrees, n and m for the last indices, a and b for the end-derivative factors"
)]

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::NurbsError;
use crate::band::Band;
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::enclose::{curve_derivative, curve_errors, distance_bound, largest, surface_error};
use crate::fit::{
    CurveFit, FitVersion, Parametrization, SurfaceFit, averaged_knots, check_degree,
    check_increasing, curve_parameters, data, end_derivative_knots, surface_parameters,
};
use crate::index::{add, get, mul, real, sub};
use crate::ops::larger;
use crate::point::Point;
use crate::surface::Surface;

/// One row of a collocation system: its first nonzero column and its values.
struct Row {
    first: usize,
    values: [f64; WIDTH],
    len: usize,
}

impl Row {
    /// The p + 1 basis functions of `basis` that do not vanish at `u`.
    fn basis(basis: &Basis, u: f64) -> Result<Row, NurbsError> {
        let p = basis.degree();
        let mut values = [0.0; WIDTH];
        let len = add(p, 1)?;
        let span = basis.eval(
            u,
            Limit::FromRight,
            values.get_mut(..len).ok_or(NurbsError::Defect)?,
        )?;
        Ok(Row {
            first: sub(span, p)?,
            values,
            len,
        })
    }

    /// The row with `a` at column `i` and `b` at column i + 1.
    fn pair(i: usize, a: f64, b: f64) -> Row {
        let mut values = [0.0; WIDTH];
        let [x, y, ..] = &mut values;
        *x = a;
        *y = b;
        Row {
            first: i,
            values,
            len: 2,
        }
    }

    /// The row with 1 at column `i`.
    fn unit(i: usize) -> Row {
        let mut values = [0.0; WIDTH];
        let [x, ..] = &mut values;
        *x = 1.0;
        Row {
            first: i,
            values,
            len: 1,
        }
    }
}

/// Solves the square system whose rows are `rows` for `width` right-hand sides, given and returned
/// row by row in `b`.
fn solve_rows(
    meter: &mut WorkMeter<'_>,
    rows: &[Row],
    b: &mut [f64],
    width: usize,
) -> Result<(), NurbsError> {
    let n = rows.len();
    let (mut lower, mut upper) = (0_usize, 0_usize);
    for (i, row) in rows.iter().enumerate() {
        let last = sub(add(row.first, row.len)?, 1)?;
        if last >= n {
            return Err(NurbsError::Defect);
        }
        // How far the row reaches left and right of the diagonal, 0 where it does not.
        lower = lower.max(i.saturating_sub(row.first));
        upper = upper.max(last.saturating_sub(i));
    }
    let mut band = Band::zeros(meter, n, lower, upper)?;
    for (i, row) in rows.iter().enumerate() {
        for (k, &v) in row.values.iter().take(row.len).enumerate() {
            if v != 0.0 {
                band.set(i, add(row.first, k)?, v)?;
            }
        }
    }
    band.solve(meter, b, width)
}

/// Points as one flat list, row by row.
fn flatten<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
) -> Result<Vec<f64>, NurbsError> {
    let mut out = vec_for(meter, mul(q.len(), D)?)?;
    for c in q {
        out.extend_from_slice(c);
    }
    Ok(out)
}

/// A flat list back as points.
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

/// The control points interpolating `q` at `u` on `basis` (one point per basis function).
pub(crate) fn interpolate_points<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    u: &[f64],
    basis: &Basis,
) -> Result<Vec<[f64; D]>, NurbsError> {
    let mut rows = vec_for(meter, u.len())?;
    for &x in u {
        rows.push(Row::basis(basis, x)?);
    }
    let mut b = flatten(meter, q)?;
    solve_rows(meter, &rows, &mut b, D)?;
    points_of(meter, &b)
}

/// The fit of `curve` to `q` at `u`, its error certified.
fn finish<const D: usize>(
    meter: &mut WorkMeter<'_>,
    curve: Curve<D>,
    q: &[[f64; D]],
    u: Vec<f64>,
    constraint_error: f64,
) -> Result<CurveFit<D>, NurbsError> {
    let error = largest(&curve_errors(meter, &curve, q, &u)?);
    Ok(CurveFit {
        curve,
        parameters: u,
        error,
        constraint_error,
        version: FitVersion::V1,
    })
}

impl<const D: usize> Curve<D> {
    /// The curve of degree `degree` through `points`, parameters by `rule` and knots by averaging
    /// (A9.1), with its certified error.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`]; [`NurbsError::DataCount`] for fewer than p + 1 points;
    /// [`NurbsError::DataNotFinite`]; [`NurbsError::CoincidentData`]; [`NurbsError::Undetermined`]
    /// if rounding has made the system singular; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn interpolate<P: Point<D>>(
        points: &[P],
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
        if q.len() <= degree {
            return Err(NurbsError::DataCount {
                points: q.len(),
                needed: add(degree, 1)?,
            });
        }
        let u = curve_parameters(meter, &q, rule)?;
        let basis = averaged_knots(&u, degree, meter)?;
        let pts = interpolate_points(meter, &q, &u, &basis)?;
        let curve = Curve::from_homogeneous(basis, pts, None)?;
        finish(meter, curve, &q, u, 0.0)
    }

    /// The curve on `basis` through `points` at `parameters`, one point per basis function, with
    /// its certified error. The parameters must increase strictly and lie in the domain, and
    /// each basis function must be nonzero at the parameter of its row (the Schoenberg–Whitney
    /// condition); averaged knots ([`crate::fit::averaged_knots`]) always meet it.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless there are as many points as basis functions;
    /// [`NurbsError::BufferLength`] unless there are as many parameters as points;
    /// [`NurbsError::DataNotFinite`]; [`NurbsError::ParameterOrder`];
    /// [`NurbsError::ParameterOutside`]; [`NurbsError::Undetermined`] if the system is singular;
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn interpolate_at<P: Point<D>>(
        points: &[P],
        parameters: &[f64],
        basis: Basis,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CurveFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        let q = data(meter, points)?;
        if q.len() != basis.count() {
            return Err(NurbsError::PointCount {
                points: q.len(),
                expected: basis.count(),
            });
        }
        if parameters.len() != q.len() {
            return Err(NurbsError::BufferLength {
                given: parameters.len(),
                needed: q.len(),
            });
        }
        check_increasing(parameters)?;
        let mut u = vec_for(meter, parameters.len())?;
        u.extend_from_slice(parameters);
        let pts = interpolate_points(meter, &q, &u, &basis)?;
        let curve = Curve::from_homogeneous(basis, pts, None)?;
        finish(meter, curve, &q, u, 0.0)
    }

    /// The curve of degree `degree` through `points` whose first derivatives at its ends are
    /// `ends` (The NURBS Book, section 9.2.2): parameters by `rule` on [0, 1], m + 3 control
    /// points, knots averaged as `NLib` does. `constraint_error` bounds how far the end
    /// derivatives miss.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`] unless 2 ≤ p ≤ [`crate::MAX_DEGREE`]; [`NurbsError::DataCount`] for
    /// fewer than two points, or fewer than p − 1; [`NurbsError::DataNotFinite`] (the derivatives
    /// count after the points); [`NurbsError::CoincidentData`]; [`NurbsError::Undetermined`];
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn interpolate_with_ends<P: Point<D>>(
        points: &[P],
        ends: [P; 2],
        degree: usize,
        rule: Parametrization,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CurveFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        check_degree(degree)?;
        if degree < 2 {
            return Err(NurbsError::Degree { degree });
        }
        let q = data(meter, points)?;
        let [d0, dm] = ends.map(Point::coords);
        for (k, d) in [d0, dm].iter().enumerate() {
            if !d.iter().all(|x| x.is_finite()) {
                return Err(NurbsError::DataNotFinite {
                    index: add(q.len(), k)?,
                });
            }
        }
        let needed = sub(degree, 1)?.max(2);
        if q.len() < needed {
            return Err(NurbsError::DataCount {
                points: q.len(),
                needed,
            });
        }
        let u = curve_parameters(meter, &q, rule)?;
        let basis = end_derivative_knots(meter, &u, degree)?;
        let pts = ends_points(meter, &q, &u, &basis, [d0, dm])?;
        let curve = Curve::from_homogeneous(basis, pts, None)?;
        let mut constraint_error = 0.0_f64;
        for (t, d) in [(get(&u, 0)?, d0), (get(&u, sub(u.len(), 1)?)?, dm)] {
            constraint_error = larger(
                constraint_error,
                distance_bound(d, curve_derivative(&curve, t, 1)?)?,
            );
        }
        finish(meter, curve, &q, u, constraint_error)
    }
}

/// The control points through `q` at `u` on `basis` (m + 3 functions) with first derivatives
/// `d0` and `dm` at the ends: rows P₀ = `Q_0`; −P₀ + P₁ = a·`D_0`; one row per interior point;
/// `P_(n−1)` − `P_n` = −b·`D_m`; `P_n` = `Q_m`. The last derivative row is negated so its pivot is 1.
fn ends_points<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    u: &[f64],
    basis: &Basis,
    [d0, dm]: [[f64; D]; 2],
) -> Result<Vec<[f64; D]>, NurbsError> {
    let knots = basis.knots();
    let p = basis.degree();
    let n = sub(basis.count(), 1)?;
    let pr = real(p)?;
    let a = (get(knots, add(p, 1)?)? - get(knots, 1)?) / pr;
    let b = (get(knots, add(n, p)?)? - get(knots, n)?) / pr;
    let m = sub(q.len(), 1)?;
    let mut rows = vec_for(meter, add(n, 1)?)?;
    let mut rhs = vec_for(meter, mul(add(n, 1)?, D)?)?;
    rows.push(Row::unit(0));
    rhs.extend_from_slice(&get(q, 0)?);
    rows.push(Row::pair(0, -1.0, 1.0));
    rhs.extend(d0.iter().map(|x| a * x));
    for k in 1..m {
        rows.push(Row::basis(basis, get(u, k)?)?);
        rhs.extend_from_slice(&get(q, k)?);
    }
    rows.push(Row::pair(sub(n, 1)?, 1.0, -1.0));
    rhs.extend(dm.iter().map(|x| -(b * x)));
    rows.push(Row::unit(n));
    rhs.extend_from_slice(&get(q, m)?);
    if !rhs.iter().all(|x| x.is_finite()) {
        return Err(NurbsError::NotFinite);
    }
    solve_rows(meter, &rows, &mut rhs, D)?;
    points_of(meter, &rhs)
}

impl<const D: usize> Surface<D> {
    /// The surface of degrees `degrees` through a grid of `size.0` rows of `size.1` points, row by
    /// row: parameters by `rule` averaged over the grid (A9.3), knots by averaging, and the
    /// control net by interpolating along u through each column, then along v through each row of
    /// the result (A9.4), with its certified error.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`]; [`NurbsError::PointCount`] if the grid does not hold rows × columns
    /// points; [`NurbsError::DataCount`] for fewer rows than p + 1 or columns than q + 1;
    /// [`NurbsError::DataNotFinite`]; [`NurbsError::CoincidentData`];
    /// [`NurbsError::Undetermined`]; [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn interpolate<P: Point<D>>(
        points: &[P],
        size: (usize, usize),
        degrees: (usize, usize),
        rule: Parametrization,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<SurfaceFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        let (p, qd) = degrees;
        check_degree(p)?;
        check_degree(qd)?;
        let q = data(meter, points)?;
        let (rows, cols) = size;
        for (count, degree) in [(rows, p), (cols, qd)] {
            if count <= degree && mul(rows, cols)? == q.len() {
                return Err(NurbsError::DataCount {
                    points: count,
                    needed: add(degree, 1)?,
                });
            }
        }
        let (u, v) = surface_parameters(meter, &q, size, rule)?;
        let bu = averaged_knots(&u, p, meter)?;
        let bv = averaged_knots(&v, qd, meter)?;
        // Along u: the grid's rows are the right-hand sides' rows, each holding every column.
        let mut rows_u = vec_for(meter, rows)?;
        for &x in &u {
            rows_u.push(Row::basis(&bu, x)?);
        }
        let mut r = flatten(meter, &q)?;
        solve_rows(meter, &rows_u, &mut r, mul(cols, D)?)?;
        // Along v: transpose, so each column of R is a right-hand side row.
        let mut t = vec_for(meter, r.len())?;
        for l in 0..cols {
            for i in 0..rows {
                let start = add(mul(i, mul(cols, D)?)?, mul(l, D)?)?;
                t.extend_from_slice(r.get(start..add(start, D)?).ok_or(NurbsError::Defect)?);
            }
        }
        let mut rows_v = vec_for(meter, cols)?;
        for &y in &v {
            rows_v.push(Row::basis(&bv, y)?);
        }
        solve_rows(meter, &rows_v, &mut t, mul(rows, D)?)?;
        // t holds P_(i, j) at row j, block i; the net is row by row in i.
        let mut net = vec_for(meter, mul(rows, cols)?)?;
        for i in 0..rows {
            for j in 0..cols {
                let start = add(mul(j, mul(rows, D)?)?, mul(i, D)?)?;
                let mut c = [0.0; D];
                c.copy_from_slice(t.get(start..add(start, D)?).ok_or(NurbsError::Defect)?);
                net.push(c);
            }
        }
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
