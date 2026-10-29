//! Curve approximation to a stated error (The NURBS Book, section 9.4.4): the curve of a given
//! degree that passes within a tolerance of every data point, with as few control points as knot
//! removal can leave.
//!
//! What it does: [`Curve::approximate`] interpolates the data at a low degree, then repeats:
//! remove every knot it can while each data point stays within the tolerance (algorithm A9.8),
//! raise the degree of the knot vector by one, fit by least squares on it, and move each data
//! point's parameter to the nearest point of the new curve. When the degree asked for is reached
//! it removes knots once more and stops. If a step fails (the least-squares system is not
//! determined, or a point ends up beyond the tolerance), it starts again from an interpolant one
//! degree higher, as `NLib`'s `N_FitCrvApprox` does.
//!
//! Guarantees:
//!
//! - **Within the tolerance, certified.** Each data point carries a certified upper bound on its
//!   distance from the current curve at its parameter. Interpolation and least squares set it by
//!   interval evaluation (`crate::enclose`); a knot removal adds the certified change of the curve
//!   at that parameter, which re-inserting the knot in interval arithmetic bounds control point by
//!   control point: the change at u is Σ `N_l`(u)·(`P̃_l` − `P_l`), at most Σ `N_l`(u)·|`P̃_l` − `P_l`|
//!   with each `N_l`(u) bounded above by an interval enclosure. A removal is kept only if every
//!   bound stays within the tolerance, so the result is within it at every data point; its
//!   `error` is the smaller of that bound and a fresh interval evaluation, point by point.
//!   `NLib` bounds a removal's change by the same sum in floating point, with the removal's error
//!   at one control point from the book's formula.
//! - **Never a best-so-far curve.** If even the interpolant of the degree asked for is not within
//!   the tolerance, the result is [`NurbsError::OutsideTolerance`]; `NLib` returns whatever its
//!   last attempt left.
//! - **Parameters improve or stay.** A parameter moves by Newton's method on (C(u) − Q)·C′(u) only
//!   while each step brings the curve point nearer the data point, inside the domain; the ends
//!   stay at the ends.
//! - **Deterministic.** Knots are tried smallest bound first, the first on a tie; every loop runs
//!   in a fixed order; Newton's method stops after at most [`NEWTON_STEPS`] steps.
//!
//! Method: W. Tiller's knot removal (*Computer-Aided Design* 24(8) (1992), 445–453,
//! <https://doi.org/10.1016/0010-4485(92)90012-Y>) used for approximation as Piegl and Tiller's
//! section 9.4.4 and algorithm A9.8 give it, with least squares at each degree (section 9.4.1)
//! and point inversion by Newton's method (section 6.1) (The NURBS Book, 2nd ed., Springer (1997),
//! <https://doi.org/10.1007/978-3-642-59223-2>), in the order of steps `NLib` takes.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_FitCrvApprox` and
//!   `N_FitRemoveKnots`, open curves only (closed data sets are not joined here); each removal's
//!   effect on the data is bounded by re-insertion in interval arithmetic instead of the book's
//!   formula in doubles; the starting degree follows `NLib`'s advice (1 for multiple knots, which
//!   keep corners, 2 otherwise); Newton's method keeps only steps that bring the point nearer,
//!   with `NLib`'s limit of 50 steps (`NL_ITLIM`) and no tolerances; failure is an error.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: each removal pass tries O(n) knots, each try O(n·D) to remove and re-insert and O(m·p)
//! to check the data; each least-squares step O(m·n²); each projection O(p²·D) a step. All of it
//! is charged to the meter as it goes.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: Q and u for the data and their parameters, p for the degree, r and s for a knot's index and multiplicity, w for the curve under work"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::enclose::{basis_upper, curve_errors, largest};
use crate::fit::{
    CurveFit, FitVersion, KnotMode, Parametrization, averaged_knots, check_degree,
    curve_parameters, data,
};
use crate::index::{add, get, run_end, sub};
use crate::insert::{Insertion, insert_kernel};
use crate::interp::interpolate_points;
use crate::lsq::fixed_ends;
use crate::ops::{charge_op, homs, larger, smaller};
use crate::point::Point;
use crate::remove::remove_once;

/// The most Newton steps one projection takes: `NLib`'s iteration limit, `NL_ITLIM`.
pub const NEWTON_STEPS: usize = 50;

/// A curve under approximation: its knots, degree and homogeneous points (weight 1).
struct Work<const D: usize> {
    knots: Vec<f64>,
    p: usize,
    pts: Vec<Hom<f64, D>>,
}

impl<const D: usize> Work<D> {
    fn of(meter: &mut WorkMeter<'_>, curve: &Curve<D>) -> Result<Self, NurbsError> {
        Ok(Work {
            knots: curve.basis().knots().to_vec(),
            p: curve.degree(),
            pts: homs::<f64, D>(meter, curve.homogeneous(), None)?,
        })
    }

    fn curve(&self, meter: &mut WorkMeter<'_>) -> Result<Curve<D>, NurbsError> {
        let mut pts = vec_for(meter, self.pts.len())?;
        pts.extend(self.pts.iter().map(|h| h.c));
        Curve::from_homogeneous(Basis::new(self.p, self.knots.clone())?, pts, None)
    }
}

/// The result of removing one copy of a knot, inside the window of knots `offset` to
/// `offset` + 2p + 2 and control points `offset` to `offset` + p + 1, the only ones the removal and
/// its re-insertion read or change: the window's new knots and points, and for each old control
/// point of the window a certified bound on how far the new curve's point moved from it.
struct Removal<const D: usize> {
    offset: usize,
    knots: Vec<f64>,
    pts: Vec<Hom<f64, D>>,
    moved: Vec<f64>,
}

/// Removes one copy of the interior knot whose last copy is at index `r` (multiplicity `s` ≤ p),
/// and bounds the change by re-inserting it in intervals; `None` if re-insertion does not give
/// back the knots. A5.8 removes the knot from control points r − p − 1 to r − s + 1 and knots
/// r − p to r − s + p + 1, and re-inserting it touches no more, so the window of knots
/// r − p − 1 to r + p + 1 and control points r − p − 1 to r is enough, and the result is the same
/// to the bit as on the whole curve.
fn remove_one<const D: usize>(
    meter: &mut WorkMeter<'_>,
    w: &Work<D>,
    r: usize,
    s: usize,
) -> Result<Option<Removal<D>>, NurbsError> {
    let p = w.p;
    let offset = sub(r, add(p, 1)?)?;
    let knots = w
        .knots
        .get(offset..add(r, add(p, 2)?)?)
        .ok_or(NurbsError::Defect)?;
    let pts = w.pts.get(offset..add(r, 1)?).ok_or(NurbsError::Defect)?;
    charge_op(meter, pts.len(), D, knots.len())?;
    let x = get(&w.knots, r)?;
    let (k2, q2) = remove_once(meter, knots, p, pts, x, (sub(r, offset)?, s))?;
    let mut enclosed = vec_for(meter, q2.len())?;
    for h in &q2 {
        enclosed.push(Hom::<Interval, D>::of(h.c, h.w)?);
    }
    // After the removal, x's span from the right is r − 1 (the last copy left, or the knot before
    // it when none is), and it has s − 1 copies.
    let job = Insertion {
        u: x,
        span: sub(sub(r, 1)?, offset)?,
        s: sub(s, 1)?,
        times: 1,
    };
    let (back, pw) = insert_kernel(meter, &k2, p, &enclosed, &job)?;
    if back != knots || pw.len() != pts.len() {
        return Ok(None);
    }
    let mut moved = vec_for(meter, pw.len())?;
    for (e, old) in pw.iter().zip(pts) {
        let zero = Interval::of(0.0)?;
        let mut mags = [zero; D];
        for (m, (&ed, &od)) in mags.iter_mut().zip(e.c.iter().zip(&old.c)) {
            let d = ed.minus(Interval::of(od)?)?;
            *m = Interval::of(larger(d.lo().abs(), d.hi().abs()))?;
        }
        moved.push(
            Interval::norm(&mags)
                .map_err(|_| NurbsError::NotFinite)?
                .hi(),
        );
    }
    Ok(Some(Removal {
        offset,
        knots: k2,
        pts: q2,
        moved,
    }))
}

/// An upper bound on the curve's change at a parameter in the span `span`: Σ `N_l`·`moved_l` over
/// the functions that do not vanish there and lie in the removal's window, with `upper` bounding
/// them, rounded up.
fn moved_at<const D: usize>(
    rm: &Removal<D>,
    span: usize,
    p: usize,
    upper: &[f64; WIDTH],
) -> Result<f64, NurbsError> {
    let mut sum = Interval::of(0.0)?;
    let first = sub(span, p)?;
    for (t, &n) in upper.iter().take(add(p, 1)?).enumerate() {
        let l = add(first, t)?;
        let Some(local) = l.checked_sub(rm.offset) else {
            continue;
        };
        if let Some(&x) = rm.moved.get(local) {
            sum = sum.plus(Interval::of(n)?.times(Interval::of(x)?)?)?;
        }
    }
    Ok(sum.hi())
}

/// `v` with the elements from `start` to `end` replaced by `with`.
fn replaced<T: Copy>(
    meter: &mut WorkMeter<'_>,
    v: &[T],
    (start, end): (usize, usize),
    with: &[T],
) -> Result<Vec<T>, NurbsError> {
    let mut out = vec_for(meter, add(sub(v.len(), sub(end, start)?)?, with.len())?)?;
    out.extend_from_slice(v.get(..start).ok_or(NurbsError::Defect)?);
    out.extend_from_slice(with);
    out.extend_from_slice(v.get(end..).ok_or(NurbsError::Defect)?);
    Ok(out)
}

/// Whether the bound `e` is beyond the tolerance `tol`: a bound equal to it is within. The one test
/// of a bound against the tolerance, for every step of the approximation.
fn beyond(e: f64, tol: f64) -> bool {
    e > tol
}

/// a + b rounded up.
fn add_up(a: f64, b: f64) -> Result<f64, NurbsError> {
    Ok(Interval::of(a)?.plus(Interval::of(b)?)?.hi())
}

/// A candidate knot: its value and, while it may still be removable, the largest movement its
/// removal makes.
struct Candidate {
    value: f64,
    bound: Option<f64>,
}

/// The index of the last copy of `value` in `knots` and its multiplicity; a defect if `value` is
/// not a knot (`get` refuses an index past the end, and a run that starts at a knot is not empty).
fn last_copy(knots: &[f64], value: f64) -> Result<(usize, usize), NurbsError> {
    let first = knots.partition_point(|&k| k < value);
    if get(knots, first)? != value {
        return Err(NurbsError::Defect);
    }
    let end = run_end(knots, first)?;
    Ok((sub(end, 1)?, sub(end, first)?))
}

/// The bound of removing one copy of `value`, or `None` if it cannot be removed.
fn bound_of<const D: usize>(
    meter: &mut WorkMeter<'_>,
    w: &Work<D>,
    value: f64,
) -> Result<Option<f64>, NurbsError> {
    let (r, s) = last_copy(&w.knots, value)?;
    if s > w.p {
        return Ok(None);
    }
    Ok(remove_one(meter, w, r, s)?.map(|rm| largest(&rm.moved)))
}

/// The data of a removal pass: the points in order of parameter, and each point's span and upper
/// bounds on its basis functions.
struct Points {
    order: Vec<usize>,
    sorted: Vec<f64>,
    spans: Vec<usize>,
    upper: Vec<[f64; WIDTH]>,
}

impl Points {
    fn new(meter: &mut WorkMeter<'_>, basis: &Basis, u: &[f64]) -> Result<Self, NurbsError> {
        let mut order = vec_for(meter, u.len())?;
        order.extend(0..u.len());
        order.sort_by(|&a, &b| {
            let (x, y) = (u.get(a).copied(), u.get(b).copied());
            x.partial_cmp(&y)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        let mut sorted = vec_for(meter, u.len())?;
        for &k in &order {
            sorted.push(get(u, k)?);
        }
        let mut spans = vec_for(meter, u.len())?;
        let mut upper = vec_for(meter, u.len())?;
        for &x in u {
            let (span, n) = basis_upper(basis, x)?;
            spans.push(span);
            upper.push(n);
        }
        Ok(Points {
            order,
            sorted,
            spans,
            upper,
        })
    }

    /// The points whose parameters lie in [a, b], by their place in parameter order.
    fn between(&self, a: f64, b: f64) -> core::ops::Range<usize> {
        let lo = self.sorted.partition_point(|&x| x < a);
        let hi = self.sorted.partition_point(|&x| x <= b);
        lo..hi
    }

    /// The point at place `i` in parameter order.
    fn at(&self, i: usize) -> Result<usize, NurbsError> {
        get(&self.order, i)
    }
}

/// Knot removal against the data (A9.8): removes knots, smallest bound first, while every data
/// point's bound `er` stays within `tol`, updating `er`.
fn remove_knots<const D: usize>(
    meter: &mut WorkMeter<'_>,
    w: &mut Work<D>,
    u: &[f64],
    er: &mut [f64],
    tol: f64,
) -> Result<(), NurbsError> {
    let p = w.p;
    let basis = Basis::new(p, w.knots.clone())?;
    let mut points = Points::new(meter, &basis, u)?;
    // The distinct knots. The ends, p + 1 copies on every basis approximation makes (clamped), are
    // never removable: `bound_of` gives them no bound.
    let mut candidates = vec_for(meter, w.knots.len())?;
    let mut i = 0;
    while i < w.knots.len() {
        let end = run_end(&w.knots, i)?;
        let value = get(&w.knots, i)?;
        let bound = bound_of(meter, w, value)?;
        candidates.push(Candidate { value, bound });
        i = end;
    }
    let mut trial: Vec<(usize, f64)> = vec_for(meter, er.len())?;
    loop {
        if w.pts.len() <= add(p, 1)? {
            break;
        }
        // The smallest bound, the first on a tie (`min_by` keeps the first of equal ones). Bounds
        // are interval upper ends, never NaN and never −0, so `total_cmp` orders them as `<` does.
        let best = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, c)| c.bound.map(|b| (index, b)))
            .min_by(|x, y| x.1.total_cmp(&y.1));
        let Some((index, _)) = best else {
            break;
        };
        let value = candidates.get(index).ok_or(NurbsError::Defect)?.value;
        let (r, s) = last_copy(&w.knots, value)?;
        let removal = remove_one(meter, w, r, s)?;
        // The data the removal can move: parameters in the support of the window's points.
        let a = get(&w.knots, sub(r, add(p, 1)?)?)?;
        let b = get(&w.knots, add(r, add(p, 1)?)?)?;
        let mut keep = false;
        if let Some(rm) = &removal {
            trial.clear();
            keep = true;
            for i in points.between(a, b) {
                let k = points.at(i)?;
                let span = get(&points.spans, k)?;
                let upper = points.upper.get(k).ok_or(NurbsError::Defect)?;
                let t = add_up(get(er, k)?, moved_at(rm, span, p, upper)?)?;
                if beyond(t, tol) {
                    keep = false;
                    break;
                }
                trial.push((k, t));
            }
        }
        let (Some(rm), true) = (removal, keep) else {
            if let Some(c) = candidates.get_mut(index) {
                c.bound = None;
            }
            continue;
        };
        for &(k, t) in &trial {
            *er.get_mut(k).ok_or(NurbsError::Defect)? = t;
        }
        let window_end = add(rm.offset, add(rm.knots.len(), 1)?)?;
        w.knots = replaced(meter, &w.knots, (rm.offset, window_end), &rm.knots)?;
        let points_end = add(rm.offset, add(rm.pts.len(), 1)?)?;
        w.pts = replaced(meter, &w.pts, (rm.offset, points_end), &rm.pts)?;
        // Spans at or after the removed knot move down by one; the basis functions change only
        // over the window, so only the points there are evaluated again.
        for span in &mut points.spans {
            if *span >= r {
                *span = sub(*span, 1)?;
            }
        }
        let basis = Basis::new(p, w.knots.clone())?;
        for i in points.between(a, b) {
            let k = points.at(i)?;
            let (span, n) = basis_upper(&basis, get(u, k)?)?;
            *points.spans.get_mut(k).ok_or(NurbsError::Defect)? = span;
            *points.upper.get_mut(k).ok_or(NurbsError::Defect)? = n;
        }
        if s == 1 {
            candidates.remove(index);
        }
        // Knots near the one removed have new bounds; those already found not removable stay so.
        let near_lo = get(&w.knots, r.saturating_sub(add(p, 2)?))?;
        let near_hi = get(&w.knots, add(r, add(p, 2)?)?.min(sub(w.knots.len(), 1)?))?;
        for c in &mut candidates {
            if c.bound.is_some() && c.value >= near_lo && c.value <= near_hi {
                c.bound = bound_of(meter, w, c.value)?;
            }
        }
    }
    Ok(())
}

/// The knot vector one degree higher: single interior knots kept single, or every knot's
/// multiplicity raised by one.
fn raised(knots: &[f64], p: usize, mode: KnotMode) -> Result<Basis, NurbsError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < knots.len() {
        let end = run_end(knots, i)?;
        let value = get(knots, i)?;
        let copies = sub(end, i)?;
        let at_end = i == 0 || end == knots.len();
        let extra = match mode {
            KnotMode::Single => usize::from(at_end),
            KnotMode::Multiple => 1,
        };
        let n = add(copies, extra)?;
        out.try_reserve(n).map_err(|_| NurbsError::Defect)?;
        out.extend(core::iter::repeat_n(value, n));
        i = end;
    }
    Basis::new(add(p, 1)?, out)
}

/// The parameter of the point of `curve` nearest `q`, by Newton's method from `u0` (The NURBS
/// Book, section 6.1), each step kept only if it brings the curve point nearer.
fn project<const D: usize>(
    meter: &mut WorkMeter<'_>,
    curve: &Curve<D>,
    q: [f64; D],
    u0: f64,
) -> Result<f64, NurbsError> {
    let (a, b) = curve.basis().domain();
    let dist2 = |c: &[f64; D]| -> f64 {
        c.iter()
            .zip(&q)
            .fold(0.0, |s, (x, y)| s + (x - y) * (x - y))
    };
    let cost = u64::try_from(add(curve.degree(), D)?).map_err(|_| NurbsError::Defect)?;
    let mut d = [[0.0; D]; 3];
    curve.derivs(u0, Limit::FromRight, &mut d)?;
    let mut u = u0;
    let [c0, _, _] = &d;
    let mut best = dist2(c0);
    for _ in 0..NEWTON_STEPS {
        meter.charge(cost.saturating_mul(cost))?;
        let [c, c1, c2] = d;
        let (mut f, mut g) = (0.0, 0.0);
        for k in 0..D {
            let diff = get(&c, k)? - get(&q, k)?;
            f += get(&c1, k)? * diff;
            g += get(&c2, k)? * diff + get(&c1, k)? * get(&c1, k)?;
        }
        if !(g > 0.0 && f.is_finite()) {
            break;
        }
        let next = (u - f / g).clamp(a, b);
        if !next.is_finite() || next == u {
            break;
        }
        let mut dn = [[0.0; D]; 3];
        curve.derivs(next, Limit::FromRight, &mut dn)?;
        let [n0, _, _] = &dn;
        let dist = dist2(n0);
        if dist.partial_cmp(&best) != Some(core::cmp::Ordering::Less) {
            break;
        }
        u = next;
        best = dist;
        d = dn;
    }
    Ok(u)
}

impl<const D: usize> Curve<D> {
    /// A curve of degree `degree` within `tol` of every one of `points`, with as few control
    /// points as knot removal leaves (section 9.4.4, as `NLib`'s `N_FitCrvApprox` runs it), its
    /// parameters, and its certified error, at most `tol`. Chord-length parameters start it;
    /// `mode` chooses single knots (smooth) or repeated ones (keeps corners).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`]; [`NurbsError::Tolerance`] if `tol` is negative or not finite;
    /// [`NurbsError::DataCount`] for fewer than p + 1 points (and at least two);
    /// [`NurbsError::DataNotFinite`]; [`NurbsError::CoincidentData`];
    /// [`NurbsError::OutsideTolerance`] if no attempt ends within `tol` (the tolerance is below
    /// the rounding of an interpolant); [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn approximate<P: Point<D>>(
        points: &[P],
        degree: usize,
        tol: f64,
        mode: KnotMode,
        version: FitVersion,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CurveFit<D>, NurbsError> {
        match version {
            FitVersion::V1 => {}
        }
        check_degree(degree)?;
        if !(tol.is_finite() && tol >= 0.0) {
            return Err(NurbsError::Tolerance);
        }
        let q = data(meter, points)?;
        let needed = add(degree, 1)?.max(2);
        if q.len() < needed {
            return Err(NurbsError::DataCount {
                points: q.len(),
                needed,
            });
        }
        let start = match mode {
            KnotMode::Multiple => 1,
            KnotMode::Single => degree.min(2),
        };
        let chord = curve_parameters(meter, &q, Parametrization::ChordLength)?;
        let flat: Vec<f64> = q.iter().flatten().copied().collect();
        for first in start..=degree {
            if let Some(fit) = attempt(meter, &q, &flat, &chord, (first, degree), (tol, mode))? {
                return Ok(fit);
            }
        }
        Err(NurbsError::OutsideTolerance)
    }
}

/// One attempt of section 9.4.4 from an interpolant of degree `first` up to degree `last`;
/// `None` if a step fails and the next attempt should start one degree higher.
fn attempt<const D: usize>(
    meter: &mut WorkMeter<'_>,
    q: &[[f64; D]],
    flat: &[f64],
    chord: &[f64],
    (first, last): (usize, usize),
    (tol, mode): (f64, KnotMode),
) -> Result<Option<CurveFit<D>>, NurbsError> {
    let mut u = chord.to_vec();
    let basis = averaged_knots(&u, first, meter)?;
    let pts = match interpolate_points(meter, q, &u, &basis) {
        Ok(pts) => pts,
        Err(NurbsError::Undetermined) => return Ok(None),
        Err(e) => return Err(e),
    };
    let curve = Curve::from_homogeneous(basis, pts, None)?;
    let mut er = curve_errors(meter, &curve, q, &u)?;
    if er.iter().any(|&e| beyond(e, tol)) {
        return Ok(None);
    }
    let mut w = Work::of(meter, &curve)?;
    let mut p = first;
    loop {
        remove_knots(meter, &mut w, &u, &mut er, tol)?;
        if p == last {
            break;
        }
        let up = raised(&w.knots, p, mode)?;
        if up.count() > q.len() {
            return Ok(None);
        }
        let ctrl = match fixed_ends(meter, flat, D, &u, &up) {
            Ok(x) => x,
            Err(NurbsError::Undetermined) => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut pts = vec_for(meter, up.count())?;
        for chunk in ctrl.chunks_exact(D.max(1)) {
            let mut c = [0.0; D];
            c.copy_from_slice(chunk.get(..D).ok_or(NurbsError::Defect)?);
            pts.push(c);
        }
        let curve = Curve::from_homogeneous(up, pts, None)?;
        let interior = sub(u.len(), 1)?;
        for k in 1..interior {
            let uk = get(&u, k)?;
            let qk = get(q, k)?;
            if let Some(slot) = u.get_mut(k) {
                *slot = project(meter, &curve, qk, uk)?;
            }
        }
        er = curve_errors(meter, &curve, q, &u)?;
        if er.iter().any(|&e| beyond(e, tol)) {
            return Ok(None);
        }
        w = Work::of(meter, &curve)?;
        p = add(p, 1)?;
    }
    let curve = w.curve(meter)?;
    let direct = curve_errors(meter, &curve, q, &u)?;
    let error = er
        .iter()
        .zip(&direct)
        .fold(0.0_f64, |m, (&a, &b)| larger(m, smaller(a, b)));
    if beyond(error, tol) {
        return Ok(None);
    }
    Ok(Some(CurveFit {
        curve,
        parameters: u,
        error,
        constraint_error: 0.0,
        version: FitVersion::V1,
    }))
}
