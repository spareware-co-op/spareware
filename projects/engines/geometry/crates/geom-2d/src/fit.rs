//! Spline fitting and biarcs: fit points to a stored spline, and a spline to circular arcs.
//!
//! What it does: [`fit_spline`] turns fit points on the grid into a [`Spline`] of chapter 14.1: the
//! control points are grid points, the knots fractions of bounded size, and the creation intent
//! (the fit points, the end tangents, the tolerance and [`FIT_VERSION`]) is kept with it, to be
//! refitted only by a versioned algorithm. [`biarc`] joins two points with given tangent directions
//! by two circular arcs that touch each end's tangent exactly, and [`spline_to_biarcs`] covers a
//! spline with a chain of biarcs, subdividing until every sample lies within a tolerance.
//!
//! Guarantees:
//!
//! - **Versioned.** The fit is version [`FIT_VERSION`] of `geom-nurbs`'s interpolation (its
//!   `V1`, piece G58), with chord-length parameters rounded to multiples of 2^-20 and averaged
//!   knots, so the same points give the same bytes on every target. A change of any rule is a new
//!   version, and an old [`Spline`] keeps the version that made it.
//! - **Through the points, within a stated bound.** [`Fit::deviation`] bounds, in whole grid steps,
//!   how far the stored spline can lie from the exact interpolating curve: the certified error of
//!   the solve plus the move of rounding each control point to the grid, which moves the curve by
//!   at most the largest such move (the convex-hull property). A fit past the caller's tolerance is
//!   refused ([`FitError::Tolerance`]), never returned.
//! - **Biarc arcs are exact.** Each arc's circle has a rational centre and passes through a grid
//!   point, and meets the tangent at its outer end exactly: the centre is the crossing of two lines
//!   with whole-number coefficients. The joint is a grid point, so the two arcs meet it exactly,
//!   but their tangents there differ by [`Biarc::gap`], which is measured in doubles from the exact
//!   centres and is zero only by luck.
//! - **Bounded.** The subdivision uses a stack, not recursion, at most [`MAX_DEPTH`] deep, and
//!   charges the meter for each piece it makes.
//!
//! Method: Piegl and Tiller's global interpolation (The NURBS Book, 2nd ed., Springer (1997),
//! sections 9.2.1 and 9.2.2, <https://doi.org/10.1007/978-3-642-59223-2>) as `geom-nurbs` has it; the equal-arm biarc of Bolton, "Biarc curves", Computer-Aided
//! Design 7 (1975): the arm length is the root of a quadratic, found with `geom-num`'s square root,
//! and the joint is rounded to the grid before any exact step.
//!
//! Cost: a fit is `geom-nurbs`'s, O(n·p²); a biarc is constant; covering a spline is O(pieces ·
//! samples), each piece costing a fixed number of evaluations.

#![allow(
    clippy::many_single_char_names,
    reason = "p, t, d, u and the like are the book's and the formula's"
)]

use alloc::vec::Vec;

use geom_math::F2;
use geom_num::NumError;
use geom_num::arc::Round;
use geom_num::carrier::Rational2;
use geom_num::grid::{P2, Steps};
use geom_nurbs::{Basis, Curve, Limit, NurbsError};
use geom_run::{Stopped, WorkMeter};

use crate::carrier::Circle2;
use crate::entity::{EntityError, FitIntent, Frac, Spline};

/// The version of this module's fitting algorithm, written into every fitted [`Spline`]'s intent.
pub const FIT_VERSION: u16 = 1;

/// Parameters are rounded to multiples of 1 / this, a power of two, so they are exact doubles.
const PARAM_DENOMINATOR: i64 = 1 << 20;

/// The deepest the subdivision of [`spline_to_biarcs`] goes.
pub const MAX_DEPTH: usize = 16;

/// Why a fit or a conversion was refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FitError {
    /// Fewer fit points than the degree needs, or a degree the fitter does not take.
    Count,
    /// Two neighbouring fit points coincide, or the rounded parameters do not increase.
    Coincident,
    /// The solve failed or a number overflowed.
    Numeric,
    /// The fit, with its rounding, is further from the points than the tolerance allows.
    Tolerance {
        /// The bound the fit reaches, in grid steps.
        deviation: Steps,
    },
    /// A control point or fraction does not fit the grid or the bounded fraction size.
    Range,
    /// The meter stopped the work.
    Stopped(Stopped),
    /// The three points are on one line or the tangents are parallel: no biarc, only a line or a
    /// curve of the other sign.
    NoBiarc,
    /// An entity could not be built from the result.
    Entity(EntityError),
}

impl From<EntityError> for FitError {
    fn from(e: EntityError) -> Self {
        FitError::Entity(e)
    }
}

impl From<NumError> for FitError {
    fn from(_: NumError) -> Self {
        FitError::Range
    }
}

fn nurbs(e: NurbsError) -> FitError {
    match e {
        NurbsError::Stopped(s) => FitError::Stopped(s),
        NurbsError::DataCount { .. } | NurbsError::Degree { .. } => FitError::Count,
        NurbsError::CoincidentData { .. } | NurbsError::ParameterOrder { .. } => {
            FitError::Coincident
        }
        _ => FitError::Numeric,
    }
}

/// A fitted spline and what is known of how well it fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fit {
    /// The spline, with its creation intent.
    pub spline: Spline,
    /// An upper bound, in grid steps, on its distance from the exact interpolating curve.
    pub deviation: Steps,
}

/// The larger of two doubles, by comparison (the kernel keeps `f64::max` out of its code, whose tie
/// rule is not named).
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The smaller of two doubles, by comparison.
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// A coordinate as a double: exact, as grid coordinates are below 2^40.
fn coordinate(c: i64) -> f64 {
    // A grid coordinate is below 2^40 in size, so the double holds it exactly.
    #[allow(clippy::cast_precision_loss, reason = "below 2^40, exact in a double")]
    {
        c as f64
    }
}

fn float_point(p: P2) -> Result<F2, FitError> {
    F2::new(coordinate(p.x.get()), coordinate(p.y.get())).map_err(|_| FitError::Numeric)
}

/// The whole number nearest `x`, ties to even (`geom-num`'s rounding), or `None` past 2^40.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the rounded value is checked below 2^40 in size first, so the cast is exact"
)]
fn nearest(x: f64) -> Option<i64> {
    let r = geom_num::math::round_ties_even(x).ok()?.get();
    if r.abs() < 1_099_511_627_776.0 {
        Some(r as i64)
    } else {
        None
    }
}

/// The fraction nearest `x` among those over `denominator`.
#[allow(clippy::cast_precision_loss, reason = "denominator below 2^32")]
fn fraction(x: f64, denominator: i64) -> Result<Frac, FitError> {
    let n = nearest(x * denominator as f64).ok_or(FitError::Range)?;
    Ok(Frac::new(n, denominator)?)
}

/// Fits a spline of `degree` through `points`, in order, within `tolerance` grid steps.
///
/// The end tangents are kept in the intent and do not shape this version's curve.
///
/// # Errors
///
/// [`FitError::Count`], [`FitError::Coincident`], [`FitError::Numeric`], [`FitError::Range`],
/// [`FitError::Stopped`], and [`FitError::Tolerance`] if the bound the fit reaches is above
/// `tolerance`.
#[allow(
    clippy::cast_precision_loss,
    reason = "knot counts and the parameter grid are far below 2^52"
)]
pub fn fit_spline(
    points: &[P2],
    start_tangent: Option<[i64; 2]>,
    end_tangent: Option<[i64; 2]>,
    degree: u8,
    tolerance: Steps,
    meter: &mut WorkMeter<'_>,
) -> Result<Fit, FitError> {
    let p = usize::from(degree);
    let data = points
        .iter()
        .map(|q| float_point(*q))
        .collect::<Result<Vec<F2>, FitError>>()?;
    let raw =
        geom_nurbs::fit::parameters(&data, geom_nurbs::fit::Parametrization::ChordLength, meter)
            .map_err(nurbs)?;
    // Parameters on a grid of 2^-20, so every later number is a short fraction.
    let mut u = Vec::with_capacity(raw.len());
    for x in &raw {
        let n = nearest(x * PARAM_DENOMINATOR as f64).ok_or(FitError::Range)?;
        u.push(n as f64 / PARAM_DENOMINATOR as f64);
    }
    if !u.windows(2).all(|w| matches!(w, [a, b] if a < b)) {
        return Err(FitError::Coincident);
    }
    // Averaged knots, each a multiple of 1 / (p · 2^20), stored exactly.
    let averaged = geom_nurbs::fit::averaged_knots(&u, p, meter).map_err(nurbs)?;
    let knot_denominator = PARAM_DENOMINATOR
        .checked_mul(i64::from(degree))
        .ok_or(FitError::Range)?;
    let mut knots = Vec::with_capacity(averaged.knots().len());
    for k in averaged.knots() {
        knots.push(fraction(*k, knot_denominator)?);
    }
    let exact_knots = knots
        .iter()
        .map(|k| coordinate(k.numerator()) / coordinate(k.denominator()))
        .collect::<Vec<f64>>();
    let basis = Basis::new(p, exact_knots).map_err(nurbs)?;
    let solved =
        Curve::<2>::interpolate_at(&data, &u, basis, geom_nurbs::fit::FitVersion::V1, meter)
            .map_err(nurbs)?;
    // Control points to the grid; the curve moves by at most the largest move.
    let mut control = Vec::with_capacity(data.len());
    let mut moved = 0.0_f64;
    for i in 0..solved.curve.basis().count() {
        let c = solved.curve.control_point(i).map_err(nurbs)?;
        let (x, y) = (
            nearest(c[0]).ok_or(FitError::Range)?,
            nearest(c[1]).ok_or(FitError::Range)?,
        );
        let dx = c[0] - coordinate(x);
        let dy = c[1] - coordinate(y);
        moved = larger(moved, geom_num::math::sqrt(dx * dx + dy * dy)?.get());
        control.push(P2::new(x, y)?);
    }
    // The bound in whole steps: the move and the solve's certified error, rounded up.
    let total = moved + solved.error;
    let steps = nearest(total).ok_or(FitError::Range)?;
    let steps = if coordinate(steps) < total {
        steps.checked_add(1).ok_or(FitError::Range)?
    } else {
        steps
    };
    let deviation = Steps(u64::try_from(steps).map_err(|_| FitError::Range)?);
    if deviation.0 > tolerance.0 {
        return Err(FitError::Tolerance { deviation });
    }
    let intent = FitIntent {
        points: points.to_vec(),
        start_tangent,
        end_tangent,
        tolerance,
        version: FIT_VERSION,
    };
    let spline = Spline::new(degree, control, None, knots, Some(intent))?;
    Ok(Fit { spline, deviation })
}

/// One arc of a biarc: the circle, and its ends in the counter-clockwise order an arc entity wants.
#[derive(Clone, Copy, Debug)]
pub struct BiarcArc {
    /// The circle: a rational centre, through a grid point.
    pub circle: Circle2,
    /// Where the arc starts, counter-clockwise.
    pub start: P2,
    /// Where the arc ends, counter-clockwise.
    pub end: P2,
    /// Whether the curve runs along it from `start` to `end` (true) or from `end` to `start`.
    pub forward: bool,
}

/// Two circular arcs from one point to another that meet each end's tangent exactly.
#[derive(Clone, Copy, Debug)]
pub struct Biarc {
    /// The arc from the first point to the joint.
    pub first: BiarcArc,
    /// The arc from the joint to the second point.
    pub second: BiarcArc,
    /// The joint, a grid point.
    pub joint: P2,
    /// The angle in radians between the two arcs' tangents at the joint, from the exact centres in
    /// doubles. The joint is rounded to the grid, so this is not zero in general.
    pub gap: f64,
}

/// The centre of the circle through `a` and `j` that touches the direction `t` at `a`, as a
/// rational point, or `None` if `j` lies on the line through `a` along `t`.
fn centre(a: P2, t: [i64; 2], j: P2) -> Result<Option<Rational2>, FitError> {
    let (ax, ay) = (i128::from(a.x.get()), i128::from(a.y.get()));
    let d = [
        i128::from(j.x.get())
            .checked_sub(ax)
            .ok_or(FitError::Range)?,
        i128::from(j.y.get())
            .checked_sub(ay)
            .ok_or(FitError::Range)?,
    ];
    let n = [
        i128::from(t[1]).checked_neg().ok_or(FitError::Range)?,
        i128::from(t[0]),
    ];
    let nd = n[0]
        .checked_mul(d[0])
        .and_then(|x| n[1].checked_mul(d[1]).and_then(|y| x.checked_add(y)))
        .ok_or(FitError::Range)?;
    if nd == 0 {
        return Ok(None);
    }
    let dd = d[0]
        .checked_mul(d[0])
        .and_then(|x| d[1].checked_mul(d[1]).and_then(|y| x.checked_add(y)))
        .ok_or(FitError::Range)?;
    // C = A + n · |D|² / (2 n·D)  =  (2 nd · A + n · dd) / (2 nd)
    let w = nd.checked_mul(2).ok_or(FitError::Range)?;
    let x = w
        .checked_mul(ax)
        .and_then(|v| n[0].checked_mul(dd).and_then(|u| v.checked_add(u)))
        .ok_or(FitError::Range)?;
    let y = w
        .checked_mul(ay)
        .and_then(|v| n[1].checked_mul(dd).and_then(|u| v.checked_add(u)))
        .ok_or(FitError::Range)?;
    Ok(Some(Rational2::new(x, y, w)?))
}

/// The centre as doubles.
fn centre_f(c: &Rational2) -> [f64; 2] {
    let ([x, y], w) = c.coordinates();
    #[allow(
        clippy::cast_precision_loss,
        reason = "a double estimate for the gap only"
    )]
    let (x, y, w) = (x as f64, y as f64, w as f64);
    [x / w, y / w]
}

/// Whether the curve turns counter-clockwise from `at` along `t` towards `to`.
fn turns_left(at: P2, t: [i64; 2], to: P2) -> bool {
    let dx = i128::from(to.x.get()).checked_sub(i128::from(at.x.get()));
    let dy = i128::from(to.y.get()).checked_sub(i128::from(at.y.get()));
    let cross = dx.zip(dy).and_then(|(dx, dy)| {
        i128::from(t[0])
            .checked_mul(dy)?
            .checked_sub(i128::from(t[1]).checked_mul(dx)?)
    });
    cross.is_some_and(|c| c > 0)
}

fn arc(on: P2, c: Rational2, from: P2, to: P2, left: bool) -> BiarcArc {
    let (start, end) = if left { (from, to) } else { (to, from) };
    BiarcArc {
        circle: Circle2::Round(Round::new(on, c)),
        start,
        end,
        forward: left,
    }
}

/// A biarc from `p0` leaving along `t0` to `p1` arriving along `t1`, the arms equal (Bolton's
/// choice).
///
/// # Errors
///
/// [`FitError::NoBiarc`] if the tangents are parallel, a tangent is zero, or the joint falls on a
/// tangent line (then one arc would be a line); [`FitError::Range`] past the exact budget.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "p, t, v, d are the formula's; coordinates are below 2^40"
)]
pub fn biarc(p0: P2, t0: [i64; 2], p1: P2, t1: [i64; 2]) -> Result<Biarc, FitError> {
    let unit = |t: [i64; 2]| -> Option<[f64; 2]> {
        let (x, y) = (coordinate(t[0]), coordinate(t[1]));
        let l = geom_num::math::sqrt(x * x + y * y).ok()?.get();
        (l > 0.0 && l.is_finite()).then(|| [x / l, y / l])
    };
    let (u0, u1) = (
        unit(t0).ok_or(FitError::NoBiarc)?,
        unit(t1).ok_or(FitError::NoBiarc)?,
    );
    let v = [
        coordinate(p1.x.get()) - coordinate(p0.x.get()),
        coordinate(p1.y.get()) - coordinate(p0.y.get()),
    ];
    let dot = |a: [f64; 2], b: [f64; 2]| a[0] * b[0] + a[1] * b[1];
    let k = 2.0 * (1.0 - dot(u0, u1));
    if k <= 1e-12 {
        return Err(FitError::NoBiarc);
    }
    let s = [u0[0] + u1[0], u0[1] + u1[1]];
    let vs = dot(v, s);
    let disc = vs * vs + k * dot(v, v);
    let d = (-vs + geom_num::math::sqrt(disc)?.get()) / k;
    if !(d.is_finite() && d > 0.0) {
        return Err(FitError::NoBiarc);
    }
    let a = [
        coordinate(p0.x.get()) + d * u0[0],
        coordinate(p0.y.get()) + d * u0[1],
    ];
    let b = [
        coordinate(p1.x.get()) - d * u1[0],
        coordinate(p1.y.get()) - d * u1[1],
    ];
    let j = P2::new(
        nearest(f64::midpoint(a[0], b[0])).ok_or(FitError::Range)?,
        nearest(f64::midpoint(a[1], b[1])).ok_or(FitError::Range)?,
    )?;
    let c0 = centre(p0, t0, j)?.ok_or(FitError::NoBiarc)?;
    let c1 = centre(p1, t1, j)?.ok_or(FitError::NoBiarc)?;
    let left0 = turns_left(p0, t0, j);
    // Arriving at p1 along t1 from j: the second arc turns left if p1 is to the left of the line
    // from j along the tangent it has at j, which is the side p1 is on seen from p1 backwards.
    let left1 = !turns_left(p1, t1, j);
    let first = arc(p0, c0, p0, j, left0);
    let second = arc(p1, c1, j, p1, left1);
    // The tangents at j, from the centres.
    let (f0, f1) = (centre_f(&c0), centre_f(&c1));
    let jf = [coordinate(j.x.get()), coordinate(j.y.get())];
    let r0 = [jf[0] - f0[0], jf[1] - f0[1]];
    let r1 = [jf[0] - f1[0], jf[1] - f1[1]];
    let sign0 = if left0 { 1.0 } else { -1.0 };
    let sign1 = if left1 { 1.0 } else { -1.0 };
    let g0 = [-r0[1] * sign0, r0[0] * sign0];
    let g1 = [-r1[1] * sign1, r1[0] * sign1];
    let cross = g0[0] * g1[1] - g0[1] * g1[0];
    let along = dot(g0, g1);
    let gap = geom_num::math::atan2(cross.abs(), along)?.get();
    Ok(Biarc {
        first,
        second,
        joint: j,
        gap,
    })
}

/// A chain of biarcs covering a spline.
#[derive(Clone, Debug)]
pub struct Cover {
    /// The biarcs, in order along the spline.
    pub biarcs: Vec<Biarc>,
    /// The largest distance found between a sample of the spline and the chain, in grid steps.
    pub worst: f64,
}

/// Covers `spline` with biarcs, subdividing its parameter range until each piece's samples lie
/// within `tolerance` steps of the piece's biarc.
///
/// # Errors
///
/// [`FitError::NoBiarc`] where a piece has parallel tangents and cannot be split further;
/// [`FitError::Numeric`]; [`FitError::Range`]; [`FitError::Stopped`].
pub fn spline_to_biarcs(
    spline: &Spline,
    tolerance: Steps,
    meter: &mut WorkMeter<'_>,
) -> Result<Cover, FitError> {
    let knots = spline
        .knots()
        .iter()
        .map(|k| coordinate(k.numerator()) / coordinate(k.denominator()))
        .collect::<Vec<f64>>();
    let points = spline
        .control()
        .iter()
        .map(|p| float_point(*p))
        .collect::<Result<Vec<F2>, FitError>>()?;
    let basis = Basis::new(usize::from(spline.degree()), knots).map_err(nurbs)?;
    let curve = Curve::<2>::new(basis, &points).map_err(nurbs)?;
    let (lo, hi) = curve.basis().domain();
    // Pieces still to do, latest first, with their depth.
    let mut todo: Vec<(f64, f64, usize)> = alloc::vec![(lo, hi, 0)];
    let mut done: Vec<(f64, f64)> = Vec::new();
    let mut worst = 0.0_f64;
    let tol = coordinate(i64::try_from(tolerance.0).map_err(|_| FitError::Range)?);
    while let Some((a, b, depth)) = todo.pop() {
        meter.charge(1).map_err(FitError::Stopped)?;
        let piece = piece_biarc(&curve, a, b)?;
        let off = match &piece {
            Some(bi) => deviation_of(&curve, a, b, bi)?,
            None => f64::INFINITY,
        };
        if off <= tol {
            worst = larger(worst, off);
            done.push((a, b));
            continue;
        }
        if depth >= MAX_DEPTH {
            return Err(if piece.is_none() {
                FitError::NoBiarc
            } else {
                FitError::Tolerance {
                    deviation: Steps(
                        nearest(off)
                            .and_then(|n| u64::try_from(n).ok())
                            .unwrap_or(u64::MAX),
                    ),
                }
            });
        }
        let m = f64::midpoint(a, b);
        // The later half is pushed first so the earlier half is made first.
        todo.push((m, b, depth.saturating_add(1)));
        todo.push((a, m, depth.saturating_add(1)));
    }
    let mut biarcs = Vec::with_capacity(done.len());
    for (a, b) in done {
        if let Some(bi) = piece_biarc(&curve, a, b)? {
            biarcs.push(bi);
        }
    }
    Ok(Cover { biarcs, worst })
}

/// The biarc of the curve between parameters `a` and `b`, ends and tangents rounded to the grid.
fn piece_biarc(curve: &Curve<2>, a: f64, b: f64) -> Result<Option<Biarc>, FitError> {
    let mut d0 = [[0.0; 2]; 2];
    let mut d1 = [[0.0; 2]; 2];
    curve.derivs(a, Limit::FromRight, &mut d0).map_err(nurbs)?;
    curve.derivs(b, Limit::FromLeft, &mut d1).map_err(nurbs)?;
    let tan = |d: [f64; 2]| -> Option<[i64; 2]> {
        let n = nearest(d[0] * 1024.0 / larger(d[0].abs() + d[1].abs(), 1e-300))?;
        let m = nearest(d[1] * 1024.0 / larger(d[0].abs() + d[1].abs(), 1e-300))?;
        Some([n, m])
    };
    let (Some(t0), Some(t1)) = (tan(d0[1]), tan(d1[1])) else {
        return Ok(None);
    };
    let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
        nearest(d0[0][0]),
        nearest(d0[0][1]),
        nearest(d1[0][0]),
        nearest(d1[0][1]),
    ) else {
        return Ok(None);
    };
    match biarc(P2::new(x0, y0)?, t0, P2::new(x1, y1)?, t1) {
        Ok(bi) => Ok(Some(bi)),
        Err(FitError::NoBiarc) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The largest distance from sampled points of the curve between `a` and `b` to the biarc's
/// circles, in grid steps; at most the arcs' own radii error, estimated in doubles.
fn deviation_of(curve: &Curve<2>, a: f64, b: f64, bi: &Biarc) -> Result<f64, FitError> {
    let mut worst = 0.0_f64;
    for k in 1..8_u32 {
        let t = a + (b - a) * f64::from(k) / 8.0;
        let q = curve.point(t, Limit::FromRight).map_err(nurbs)?;
        let near = [&bi.first, &bi.second]
            .iter()
            .map(|arc| distance_to(arc, q))
            .fold(f64::INFINITY, smaller);
        worst = larger(worst, near);
    }
    Ok(worst)
}

fn distance_to(arc: &BiarcArc, q: [f64; 2]) -> f64 {
    let Circle2::Round(round) = arc.circle else {
        return f64::INFINITY;
    };
    let c = centre_f(&round.centre());
    let on = round.on();
    let (rx, ry) = (coordinate(on.x.get()) - c[0], coordinate(on.y.get()) - c[1]);
    let (qx, qy) = (q[0] - c[0], q[1] - c[1]);
    let norm = |x: f64, y: f64| {
        geom_num::math::sqrt(x * x + y * y).map_or(f64::INFINITY, geom_num::Real::get)
    };
    norm(qx, qy) - norm(rx, ry)
}
