//! The B-spline curves and surfaces of an `SMLib` database, read as `SmBSplineCurve::ReadFromDB` and
//! `SmBSplineSurface::ReadFromDB` read their ASCII form.
//!
//! What it does: [`Curve::read`] reads the header (last control point index, degree, rational
//! flag, dimension, and from database version 33 on the knot type and form), the control points
//! one per line and the knots one per line; [`Surface::read`] the same in two directions, the
//! control points v index fastest. Values are kept as written: a rational net's points are the
//! weighted ones, w·P, with their weights beside them.
//!
//! Guarantees:
//!
//! - **What `SMLib` accepts, and only that.** Degrees from 1 to `NL_MAXDEG` (17), at least degree +
//!   1 control points in each direction, and a rational flag of 0 or 1; anything else, a count the
//!   rest of the file cannot hold, or a value that does not read is an [`SmbError`].
//! - **The values written.** Every control point and knot is the double the stream reads; nothing
//!   is normalised, refitted or reordered.
//!
//! Method: `SmBSplineCurve::ReadFromDB` and `SmBSplineSurface::ReadFromDB`
//! (<https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBSplineSurface.cpp>),
//! as solidcore ports them.
//!
//! Source: PORT from NVIDIA USD `BRep` `SMLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBSplineCurve.cpp>
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBSplineSurface.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated from the ASCII branches of the two `ReadFromDB`s through
//!   solidcore's port; the result is plain arrays of the values read, not an `NLib` curve or
//!   surface.
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/nurbs/b_spline_curve/p2.rs>
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/nurbs/b_spline_surface/p1.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the ASCII readers alone, every size checked before it allocates, and
//!   the values kept as read.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: linear in the control points and knots read.

use super::{Reader, SmbError};

/// `NL_MAXDEG`: the highest degree `NLib` evaluates.
pub const MAX_DEGREE: usize = 17;

/// A B-spline curve as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    /// The degree.
    pub degree: usize,
    /// The dimension, 2 or 3: a 3D curve read as a UV curve is made 2D, as `SMLib` makes it.
    pub dim: usize,
    /// The control points, each `dim` values: w·P for a rational curve.
    pub points: Vec<Vec<f64>>,
    /// The weights of a rational curve.
    pub weights: Option<Vec<f64>>,
    /// The knots: control points plus degree plus one of them.
    pub knots: Vec<f64>,
    /// From database version 33 on, the knot type and the form, as written.
    pub kind: [u64; 2],
}

/// A B-spline surface as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    /// The degrees in u and v.
    pub degree: [usize; 2],
    /// The control points' counts in u and v.
    pub count: [usize; 2],
    /// The control points, u by u with v fastest: w·P for a rational surface.
    pub points: Vec<[f64; 3]>,
    /// The weights of a rational surface, in the same order.
    pub weights: Option<Vec<f64>>,
    /// The u knots and the v knots.
    pub knots: [Vec<f64>; 2],
    /// From database version 33 on, the knot type and the form, as written.
    pub kind: [u64; 2],
}

/// A degree read: from 1 to [`MAX_DEGREE`].
fn degree(p: i16) -> Result<usize, SmbError> {
    usize::try_from(p)
        .ok()
        .filter(|p| (1..=MAX_DEGREE).contains(p))
        .ok_or(SmbError::Shape("a degree outside 1 to NL_MAXDEG"))
}

/// The control points' count from the last index `n`, at least `degree` + 1.
fn count(n: i64, degree: usize) -> Result<usize, SmbError> {
    usize::try_from(n)
        .ok()
        .filter(|&n| n >= degree)
        .and_then(|n| n.checked_add(1))
        .ok_or(SmbError::Shape("fewer control points than degree + 1"))
}

/// The rational flag: 0 or 1.
fn rational(flag: i16) -> Result<bool, SmbError> {
    match flag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(SmbError::Shape("a rational flag other than 0 or 1")),
    }
}

/// The knot type and form of database version 33 on, one per line.
fn kind(r: &mut Reader) -> [u64; 2] {
    let mut kind = [0, 0];
    if r.version > 32 {
        let [t, f] = &mut kind;
        r.ulong(t);
        r.next_line();
        r.ulong(f);
        r.next_line();
    }
    kind
}

/// `n` knots, one per line.
fn knots(r: &mut Reader, n: usize) -> Result<Vec<f64>, SmbError> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut k = f64::NAN;
        r.double(&mut k);
        r.next_line();
        r.check()?;
        out.push(k);
    }
    Ok(out)
}

impl Curve {
    /// Reads a curve whose type header has been read, for a B-rep wanting dimension `dim`.
    ///
    /// # Errors
    ///
    /// [`SmbError::Shape`] for a header `SMLib` refuses, a value that does not read, or a count past
    /// the data.
    pub(super) fn read(reader: &mut Reader, dim: usize) -> Result<Curve, SmbError> {
        let (mut last, mut deg, mut flag, mut written) = (-1_i64, 0_i16, -1_i16, 0_i16);
        reader.long(&mut last);
        reader.next_line();
        reader.short(&mut deg);
        reader.next_line();
        reader.short(&mut flag);
        reader.next_line();
        reader.short(&mut written);
        reader.next_line();
        let kind = kind(reader);
        reader.check()?;
        let degree = degree(deg)?;
        let count = count(last, degree)?;
        let rational = rational(flag)?;
        let written: usize = match written {
            2 => 2,
            3 => 3,
            _ => return Err(SmbError::Shape("a curve dimension other than 2 or 3")),
        };
        let knot_count = count
            .checked_add(degree)
            .and_then(|k| k.checked_add(1))
            .ok_or(SmbError::Shape("a curve too large to size"))?;
        let per = written.saturating_add(usize::from(rational));
        reader.room(count.saturating_mul(per).saturating_add(knot_count))?;
        let mut points = Vec::with_capacity(count);
        let mut weights = Vec::with_capacity(if rational { count } else { 0 });
        let mut values = [f64::NAN; 4];
        for _ in 0..count {
            for value in values.iter_mut().take(per) {
                reader.double(value);
            }
            reader.next_line();
            reader.check()?;
            // A 3D curve read where a 2D one is wanted loses its z, as `ConvertTo2D` drops it.
            let point: Vec<f64> = values.iter().copied().take(written.min(dim)).collect();
            if rational {
                weights.push(values.get(written).copied().unwrap_or(f64::NAN));
            }
            points.push(point);
        }
        let knots = knots(reader, knot_count)?;
        Ok(Curve {
            degree,
            dim: written.min(dim),
            points,
            weights: rational.then_some(weights),
            knots,
            kind,
        })
    }
}

impl Surface {
    /// Reads a surface whose type header has been read.
    ///
    /// # Errors
    ///
    /// [`SmbError::Shape`] for a header `SMLib` refuses, a value that does not read, or a count
    /// past the data.
    pub(super) fn read(reader: &mut Reader) -> Result<Surface, SmbError> {
        let (mut last_u, mut last_v) = (-1_i64, -1_i64);
        let (mut deg_u, mut deg_v, mut flag) = (0_i16, 0_i16, -1_i16);
        reader.long(&mut last_u);
        reader.long(&mut last_v);
        reader.next_line();
        reader.short(&mut deg_u);
        reader.short(&mut deg_v);
        reader.next_line();
        reader.short(&mut flag);
        reader.next_line();
        let kind = kind(reader);
        reader.check()?;
        let degree = [degree(deg_u)?, degree(deg_v)?];
        let [du, dv] = degree;
        let count = [count(last_u, du)?, count(last_v, dv)?];
        let [cu, cv] = count;
        let rational = rational(flag)?;
        let too_big = SmbError::Shape("a surface too large to size");
        let points_count = cu.checked_mul(cv).ok_or(too_big)?;
        let knot_counts = [
            cu.checked_add(du)
                .and_then(|k| k.checked_add(1))
                .ok_or(too_big)?,
            cv.checked_add(dv)
                .and_then(|k| k.checked_add(1))
                .ok_or(too_big)?,
        ];
        let [ku, kv] = knot_counts;
        reader.room(
            points_count
                .saturating_mul(3)
                .saturating_add(ku)
                .saturating_add(kv),
        )?;
        let mut points = Vec::with_capacity(points_count);
        let mut weights = Vec::with_capacity(if rational { points_count } else { 0 });
        for _ in 0..points_count {
            let mut point = [f64::NAN; 3];
            for value in &mut point {
                reader.double(value);
            }
            let mut weight = f64::NAN;
            if rational {
                reader.double(&mut weight);
                weights.push(weight);
            }
            reader.next_line();
            reader.check()?;
            points.push(point);
        }
        let knots = [knots(reader, ku)?, knots(reader, kv)?];
        Ok(Surface {
            degree,
            count,
            points,
            weights: rational.then_some(weights),
            knots,
            kind,
        })
    }
}
