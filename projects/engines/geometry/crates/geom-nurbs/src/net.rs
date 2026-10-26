//! Knot insertion, refinement, Bezier extraction and degree elevation of NURBS surfaces (The
//! NURBS Book, algorithms A5.3, A5.5, A5.7 and A5.10).
//!
//! What it does: each surface algorithm of the book is its curve algorithm applied to every column
//! of the net (direction u) or every row (direction v), with the same coefficients, because the
//! coefficients depend only on the knots of that direction. So [`Surface::insert_knot`],
//! [`Surface::refine`], [`Surface::to_bezier`] and [`Surface::elevate`] run the curve kernels of
//! `crate::insert`, `crate::bezier` and `crate::elevate` on each line of the net, in doubles for the
//! result and in intervals for its certified rounding (`crate::ops`).
//!
//! Guarantees:
//!
//! - **The same surface, within the reported deviation.** In real arithmetic these operations do
//!   not change the surface; the reported deviation bounds, with certainty, how far rounding moved
//!   it, by the partition of unity of the tensor-product basis.
//! - **The curve operations' conditions, in the direction asked for.** Each refusal is the curve
//!   operation's, for the basis of that direction.
//! - **Line by line, as the book.** Every line gets exactly the operations its curve would, in the
//!   same order, so a surface's line equals the curve operation on that line, bit for bit.
//!
//! Method: Piegl and Tiller's algorithms A5.3, A5.5, A5.7 and A5.10 (The NURBS Book, 2nd ed.,
//! sections 5.2 to 5.5, <https://doi.org/10.1007/978-3-642-59223-2>), which apply the curve
//! algorithms A5.1, A5.4, A5.6 and A5.9 to the rows or columns of the net, as `NLib`'s surface tools
//! do with its curve routines.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfTool.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: `NLib`'s surface insertion, refinement, Bezier extraction and elevation
//!   are rebuilt on the ported curve kernels applied line by line, in place of its separate surface
//!   loops, with the curve kernels' changes; every result carries its certified deviation.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: the curve operation's cost times the number of lines, each twice; the new net and its
//! enclosure are allocated once, and each line's result once.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, Limit};
use crate::bezier::{decompose, span_count};
use crate::budget::vec_for;
use crate::curve::derivative_points;
use crate::elevate::{check_elevation, elevate_kernel};
use crate::index::{add, get, mul, sub};
use crate::insert::{Insertion, check_refinement, insert_kernel, refine_kernel};
use crate::ops::{Changed, Direction, charge_op, homs, p_max, rounding, split};
use crate::surface::Surface;

/// A net in `N`: its points row by row, and its numbers of rows and columns.
struct Net<N, const D: usize> {
    points: Vec<Hom<N, D>>,
    rows: usize,
    cols: usize,
}

impl<N: Num, const D: usize> Net<N, D> {
    /// Line `index` of the net in direction `dir`: column `index` for u, row `index` for v.
    fn line(
        &self,
        meter: &mut WorkMeter<'_>,
        dir: Direction,
        index: usize,
    ) -> Result<Vec<Hom<N, D>>, NurbsError> {
        let (len, step, start) = match dir {
            Direction::U => (self.rows, self.cols, index),
            Direction::V => (self.cols, 1, mul(index, self.cols)?),
        };
        let mut out = vec_for(meter, len)?;
        for k in 0..len {
            out.push(get(&self.points, add(start, mul(k, step)?)?)?);
        }
        Ok(out)
    }

    /// How many lines run in direction `dir`.
    fn lines(&self, dir: Direction) -> usize {
        match dir {
            Direction::U => self.cols,
            Direction::V => self.rows,
        }
    }
}

/// The net with each line in direction `dir` replaced by `f` of it: `f` returns the line's new
/// knots and points, the same knots for every line.
fn map_lines<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    net: &Net<N, D>,
    dir: Direction,
    mut f: impl FnMut(
        &mut WorkMeter<'_>,
        &[Hom<N, D>],
    ) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError>,
) -> Result<(Vec<f64>, Net<N, D>), NurbsError> {
    let count = net.lines(dir);
    let mut knots = Vec::new();
    let mut results = vec_for(meter, count)?;
    for index in 0..count {
        let line = net.line(meter, dir, index)?;
        let (k, out) = f(meter, &line)?;
        knots = k;
        results.push(out);
    }
    let len = results.first().map_or(0, Vec::len);
    let (rows, cols) = match dir {
        Direction::U => (len, net.cols),
        Direction::V => (net.rows, len),
    };
    let mut points = vec_for(meter, mul(rows, cols)?)?;
    for i in 0..rows {
        for j in 0..cols {
            let (line, k) = match dir {
                Direction::U => (j, i),
                Direction::V => (i, j),
            };
            let h = get(results.get(line).ok_or(NurbsError::Defect)?, k)?;
            points.push(h);
        }
    }
    Ok((knots, Net { points, rows, cols }))
}

impl<const D: usize> Surface<D> {
    /// The basis of direction `dir`.
    fn basis_of(&self, dir: Direction) -> &Basis {
        match dir {
            Direction::U => self.basis_u(),
            Direction::V => self.basis_v(),
        }
    }

    /// The net in `N`, rows and columns as stored.
    fn net<N: Num>(&self, meter: &mut WorkMeter<'_>) -> Result<Net<N, D>, NurbsError> {
        Ok(Net {
            points: homs::<N, D>(meter, self.homogeneous(), self.weights())?,
            rows: self.basis_u().count(),
            cols: self.basis_v().count(),
        })
    }

    /// The surface with the same basis in the other direction, the new basis in `dir`, and the
    /// result net, with its certified rounding against `enclosure`.
    fn rebuild(
        &self,
        meter: &mut WorkMeter<'_>,
        dir: Direction,
        basis: Basis,
        result: &Net<f64, D>,
        enclosure: &Net<Interval, D>,
    ) -> Result<Changed<Surface<D>>, NurbsError> {
        let rational = self.is_rational();
        let pm = if rational {
            p_max(self.homogeneous(), self.weights())?
        } else {
            0.0
        };
        let deviation = rounding(&result.points, &enclosure.points, rational, pm)?;
        let (points, weights) = split(meter, &result.points, rational)?;
        let (bu, bv) = match dir {
            Direction::U => (basis, self.basis_v().clone()),
            Direction::V => (self.basis_u().clone(), basis),
        };
        Ok(Changed {
            value: Surface::from_homogeneous(bu, bv, points, weights)?,
            deviation,
        })
    }

    /// The number of points of a result with `len` points per line in direction `dir`.
    fn net_size(&self, dir: Direction, len: usize) -> Result<usize, NurbsError> {
        match dir {
            Direction::U => mul(len, self.basis_v().count()),
            Direction::V => mul(self.basis_u().count(), len),
        }
    }

    /// The surface with `u` inserted `times` times into the knots of direction `dir` (A5.3).
    ///
    /// # Errors
    ///
    /// As for [`crate::Curve::insert_knot`], for the basis of direction `dir`.
    pub fn insert_knot(
        &self,
        dir: Direction,
        u: f64,
        times: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Surface<D>>, NurbsError> {
        let basis = self.basis_of(dir);
        let (lo, hi) = basis.domain();
        if u.is_nan() || u <= lo || u >= hi {
            return Err(NurbsError::KnotOutside);
        }
        let p = basis.degree();
        let s = basis.multiplicity(u);
        if add(s, times)? > p {
            return Err(NurbsError::TooManyKnots);
        }
        let len = add(basis.count(), times)?;
        charge_op(
            meter,
            self.net_size(dir, len)?,
            D,
            add(basis.knots().len(), times)?,
        )?;
        let job = Insertion {
            u,
            span: basis.span(u, Limit::FromRight)?,
            s,
            times,
        };
        let knots = basis.knots();
        let net_f = self.net::<f64>(meter)?;
        let net_i = self.net::<Interval>(meter)?;
        let (uq, result) = map_lines(meter, &net_f, dir, |m, line| {
            insert_kernel(m, knots, p, line, &job)
        })?;
        let (_, enclosure) = map_lines(meter, &net_i, dir, |m, line| {
            insert_kernel(m, knots, p, line, &job)
        })?;
        self.rebuild(meter, dir, Basis::new(p, uq)?, &result, &enclosure)
    }

    /// The surface with the knots `x` inserted into direction `dir` (A5.5).
    ///
    /// # Errors
    ///
    /// As for [`crate::Curve::refine`], for the basis of direction `dir`.
    pub fn refine(
        &self,
        dir: Direction,
        x: &[f64],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Surface<D>>, NurbsError> {
        let basis = self.basis_of(dir);
        check_refinement(basis, x)?;
        if x.is_empty() {
            return Ok(Changed {
                value: self.clone(),
                deviation: 0.0,
            });
        }
        let len = add(basis.count(), x.len())?;
        charge_op(
            meter,
            self.net_size(dir, len)?,
            D,
            add(basis.knots().len(), x.len())?,
        )?;
        let net_f = self.net::<f64>(meter)?;
        let net_i = self.net::<Interval>(meter)?;
        let (uq, result) = map_lines(meter, &net_f, dir, |m, line| {
            refine_kernel(m, basis, line, x)
        })?;
        let (_, enclosure) = map_lines(meter, &net_i, dir, |m, line| {
            refine_kernel(m, basis, line, x)
        })?;
        self.rebuild(
            meter,
            dir,
            Basis::new(basis.degree(), uq)?,
            &result,
            &enclosure,
        )
    }

    /// The surface with its degree in direction `dir` raised by `t` (A5.10).
    ///
    /// # Errors
    ///
    /// As for [`crate::Curve::elevate`], for the basis of direction `dir`.
    pub fn elevate(
        &self,
        dir: Direction,
        t: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Surface<D>>, NurbsError> {
        let basis = self.basis_of(dir);
        let q = add(basis.degree(), t)?;
        if q > crate::MAX_DEGREE {
            return Err(NurbsError::Degree { degree: q });
        }
        check_elevation(basis)?;
        if t == 0 {
            return Ok(Changed {
                value: self.clone(),
                deviation: 0.0,
            });
        }
        let s = span_count(basis);
        let len = add(basis.count(), mul(t, s)?)?;
        let knots = add(basis.knots().len(), mul(t, add(s, 1)?)?)?;
        charge_op(meter, self.net_size(dir, len)?, D, knots)?;
        let net_f = self.net::<f64>(meter)?;
        let net_i = self.net::<Interval>(meter)?;
        let (uq, result) = map_lines(meter, &net_f, dir, |m, line| {
            elevate_kernel(m, basis, line, t)
        })?;
        let (_, enclosure) = map_lines(meter, &net_i, dir, |m, line| {
            elevate_kernel(m, basis, line, t)
        })?;
        self.rebuild(meter, dir, Basis::new(q, uq)?, &result, &enclosure)
    }

    /// The surface as Bezier patches, row by row: patch (i, j) covers the i-th span in u and the
    /// j-th in v (A5.7).
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`] unless both bases are clamped; [`NurbsError::Stopped`] if the
    /// meter stops it; [`NurbsError::NotFinite`] if a point overflows.
    pub fn to_bezier(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Vec<Surface<D>>>, NurbsError> {
        let (bu, bv) = (self.basis_u(), self.basis_v());
        if !(bu.is_clamped() && bv.is_clamped()) {
            return Err(NurbsError::NotClamped);
        }
        let (p, q) = (bu.degree(), bv.degree());
        let (su, sv) = (span_count(bu), span_count(bv));
        let per_patch = mul(add(p, 1)?, add(q, 1)?)?;
        let patches = mul(su, sv)?;
        let knots = mul(patches, mul(add(add(p, q)?, 2)?, 2)?)?;
        charge_op(meter, mul(patches, per_patch)?, D, knots)?;
        let rational = self.is_rational();
        let pm = if rational {
            p_max(self.homogeneous(), self.weights())?
        } else {
            0.0
        };
        let net_f = self.net::<f64>(meter)?;
        let net_i = self.net::<Interval>(meter)?;
        let pieces_f = patches_of(meter, &net_f, bu, bv)?;
        let pieces_i = patches_of(meter, &net_i, bu, bv)?;
        let mut deviation = 0.0;
        let mut out = vec_for(meter, pieces_f.len())?;
        for (patch, enc) in pieces_f.iter().zip(&pieces_i) {
            let dev = rounding(&patch.points, &enc.points, rational, pm)?;
            if dev > deviation {
                deviation = dev;
            }
            let ku = bezier_knots(meter, p, patch.u)?;
            let kv = bezier_knots(meter, q, patch.v)?;
            let (points, weights) = split(meter, &patch.points, rational)?;
            out.push(Surface::from_homogeneous(
                Basis::new(p, ku)?,
                Basis::new(q, kv)?,
                points,
                weights,
            )?);
        }
        Ok(Changed {
            value: out,
            deviation,
        })
    }
}

impl<const D: usize> Surface<D> {
    /// The partial derivative ∂^(k+l)S / ∂u^k ∂v^l of this nonrational surface, as a surface of
    /// degrees (p − k, q − l) on the knots of each basis less k (or l) at each end (A3.7: A3.3 on
    /// every column, then on every row).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Rational`] for a rational surface; [`NurbsError::Degree`] if k ≥ p or l ≥ q
    /// (k = l = 0 gives the surface itself); [`NurbsError::Discontinuous`] if a lower derivative
    /// jumps at a knot; [`NurbsError::NotFinite`] if a point overflows; [`NurbsError::Stopped`] if
    /// the meter stops it.
    pub fn derivative_surface(
        &self,
        (k, l): (usize, usize),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Surface<D>, NurbsError> {
        if self.is_rational() {
            return Err(NurbsError::Rational);
        }
        let (bu, bv) = (self.basis_u(), self.basis_v());
        let (p, q) = (bu.degree(), bv.degree());
        for (order, degree) in [(k, p), (l, q)] {
            if order > 0 && order >= degree {
                return Err(NurbsError::Degree {
                    degree: degree.saturating_sub(order),
                });
            }
        }
        charge_op(
            meter,
            self.homogeneous().len(),
            D,
            add(bu.knots().len(), bv.knots().len())?,
        )?;
        let (mut rows, cols) = (bu.count(), bv.count());
        let mut net = vec_for(meter, self.homogeneous().len())?;
        net.extend_from_slice(self.homogeneous());
        let mut ku = bu.knots().to_vec();
        let mut kv = bv.knots().to_vec();
        if k > 0 {
            let mut columns = vec_for(meter, cols)?;
            for j in 0..cols {
                let mut line = vec_for(meter, rows)?;
                for i in 0..rows {
                    line.push(get(&net, add(mul(i, cols)?, j)?)?);
                }
                let (knots, pts) = derivative_points(meter, bu.knots(), p, &line, k)?;
                ku = knots;
                columns.push(pts);
            }
            rows = columns.first().map_or(0, Vec::len);
            net.clear();
            for i in 0..rows {
                for column in &columns {
                    net.push(get(column, i)?);
                }
            }
        }
        if l > 0 {
            let mut out = vec_for(meter, net.len())?;
            for i in 0..rows {
                let start = mul(i, cols)?;
                let line = net
                    .get(start..add(start, cols)?)
                    .ok_or(NurbsError::Defect)?;
                let (knots, pts) = derivative_points(meter, bv.knots(), q, line, l)?;
                kv = knots;
                out.extend(pts);
            }
            net = out;
        }
        Surface::from_homogeneous(
            Basis::new(sub(p, k)?, ku)?,
            Basis::new(sub(q, l)?, kv)?,
            net,
            None,
        )
    }
}

/// One Bezier patch: its spans in u and v, and its (p + 1) × (q + 1) points row by row.
struct Patch<N, const D: usize> {
    u: (f64, f64),
    v: (f64, f64),
    points: Vec<Hom<N, D>>,
}

/// The clamped Bezier knots [a, …, a, b, …, b] of degree `p`.
fn bezier_knots(
    meter: &mut WorkMeter<'_>,
    p: usize,
    (a, b): (f64, f64),
) -> Result<Vec<f64>, NurbsError> {
    let order = add(p, 1)?;
    let mut k = vec_for(meter, mul(order, 2)?)?;
    k.extend(core::iter::repeat_n(a, order));
    k.extend(core::iter::repeat_n(b, order));
    Ok(k)
}

/// A5.7 on a net: A5.6 on every column, then on every row of each strip of columns' pieces.
fn patches_of<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    net: &Net<N, D>,
    bu: &Basis,
    bv: &Basis,
) -> Result<Vec<Patch<N, D>>, NurbsError> {
    let (p, q) = (bu.degree(), bv.degree());
    let (su, sv) = (span_count(bu), span_count(bv));
    // columns[j][piece]: the u-pieces of column j.
    let mut columns = vec_for(meter, net.cols)?;
    for j in 0..net.cols {
        let line = net.line(meter, Direction::U, j)?;
        columns.push(decompose(meter, bu, &line)?);
    }
    let mut out = vec_for(meter, mul(su, sv)?)?;
    for piece in 0..su {
        // The strip: p + 1 rows (the piece's points) of the net's columns.
        let mut rows = vec_for(meter, add(p, 1)?)?;
        let mut span_u = (0.0, 0.0);
        for r in 0..=p {
            let mut row = vec_for(meter, net.cols)?;
            for column in &columns {
                let (a, b, pts) = get_piece(column, piece)?;
                span_u = (a, b);
                row.push(get(pts, r)?);
            }
            rows.push(decompose(meter, bv, &row)?);
        }
        for vp in 0..sv {
            let mut points = vec_for(meter, mul(add(p, 1)?, add(q, 1)?)?)?;
            let mut span_v = (0.0, 0.0);
            for row in &rows {
                let (a, b, pts) = get_piece(row, vp)?;
                span_v = (a, b);
                for c in 0..=q {
                    points.push(get(pts, c)?);
                }
            }
            out.push(Patch {
                u: span_u,
                v: span_v,
                points,
            });
        }
    }
    Ok(out)
}

/// Piece `index` of a decomposition: its span and its points.
fn get_piece<N: Num, const D: usize>(
    pieces: &[crate::bezier::Piece<N, D>],
    index: usize,
) -> Result<(f64, f64, &[Hom<N, D>]), NurbsError> {
    let (a, b, pts) = pieces.get(index).ok_or(NurbsError::Defect)?;
    Ok((*a, *b, pts.as_slice()))
}
