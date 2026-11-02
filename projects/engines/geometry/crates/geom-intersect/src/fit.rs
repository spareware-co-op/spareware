//! The procedural intersection curve's caches and their bounds (12.12): a marched branch fitted
//! to a 3D NURBS and two pcurves on one parameter, each span certified, and any point of the true
//! curve recovered from the caches by refinement on the definition.
//!
//! What it does: [`fit`] takes a [`Branch`] of [`crate::march`] and its two surfaces. It fits the
//! 3D curve through the surfaces' points at the branch's vertices by The NURBS Book's approximation
//! with knot removal (`geom_nurbs::Curve::approximate`), then both pcurves by least squares on the
//! same parameters and basis, their ends held, so all three share t. For each span of the 3D cache
//! it bounds, by the mean-value form, how far each mapped pcurve Sᵢ(pᵢ(t)) is from the cache C(t),
//! and from those two distances, the angle at which the surfaces cross and their curvature, how
//! far C(t) is from the true curve. A span over the ceiling is refined: a point of the true curve
//! is added between each two data points in it, and the whole fit is made again. [`true_point`]
//! finds the point of the true curve in the plane across the cache at t, proven unique.
//!
//! Guarantees:
//!
//! - **Every span is certified within the ceiling, or there is no curve.** A span's bound is
//!   proven, as below; a fit whose spans do not all come within `ceiling` after [`ROUNDS`] rounds of
//!   refinement is [`IntersectError::BudgetExceeded`], never a curve with a wider bound.
//! - **The pcurve bounds.** Over a piece [a, b] of a span with middle m and half-width h, every
//!   value of f(t) = Sᵢ(pᵢ(t)) − C(t) is within h·sup|f′| of f(m), and f′ = `S_u`·p′ᵤ + `S_v`·p′ᵥ − C′
//!   is enclosed from the kinds' own bounds over the pcurve's box. A span is cut into up to
//!   2^[`PIECES`] pieces until its bounds come within the ceiling.
//! - **The curve bound.** C(t) is within d₁ of S₁ and d₂ of S₂. Within r of C(t), where the surfaces
//!   cross at a sine of at least s and turn their normals by at most κ per unit length, the crossing
//!   is within e/(s − 2κ·r₀) of C(t), e = d₁ + d₂ and r₀ = 2e/s, whenever s − 2κ·r₀ ≥ s/2: 12.2's
//!   bound, as [`crate::bound`] proves it. s and κ are enclosed over the pcurve's box widened by
//!   (r₀ + dᵢ)/σᵢ in parameters, σᵢ a lower bound on how fast Sᵢ moves there, so that box holds
//!   every point within r₀ of C(t). κ is |dN|/|dS| ≤ √(|`n_u`|² + |`n_v`|²)·√(E + G)/|n|², n = `S_u` × `S_v`,
//!   and σ ≥ |n|/√(E + G), from the first fundamental form's least eigenvalue, at least EG−F²
//!   over E + G.
//! - **One true point.** [`true_point`] proves its point the only zero of S₁ − S₂ and the plane
//!   condition in its box, by the interval engine (12.3).
//! - **The definition kept.** The curve's chain is the branch's vertices, each with its parameter
//!   on the caches; points added in refinement are fit data only.
//!
//! Method: chapter 12.12 (`projects/engines/geometry/architecture/12-intersections.md`); the fit is
//! The NURBS Book's section 9.4 (Piegl and Tiller, 2nd ed., Springer 1997,
//! <https://doi.org/10.1007/978-3-642-59223-2>) as `geom-nurbs` builds it; the bound is 12.2's.
//!
//! Cost: each round is one approximation of the branch's points and two least-squares fits, and per
//! span up to 2^[`PIECES`] pieces, each a few reads of the surfaces to second order. Refinement
//! doubles the data in a failing span at most [`ROUNDS`] times. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.12's: C the cache, p the pcurves, S the surfaces, e, s, κ and r the bound's"
)]

use alloc::vec::Vec;

use geom_curve::tight::larger;
use geom_curve::{
    Basis as Leaf, CurveError, DerivativeBound, IntersectionCurve, Marched, ParamCurve, Span,
    SurfaceRef, Trimmed,
};
use geom_num::interval::Interval;
use geom_num::newton::inverse;
use geom_nurbs::{Constraint, Curve, FitVersion, KnotMode, Limit};
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind, partial_index};

use crate::IntersectError;
use crate::general::{around, cross, minus_box, normals, partials};
use crate::march::Branch;
use crate::project::{boxed, dot, exact, image};
use crate::result::CurveEnd;
use crate::solve::{Field, solve};

/// The cache's degree, as 12.12 fits it: cubic, or less for a branch of fewer points.
pub const DEGREE: usize = 3;

/// The rounds of refinement a fit may take before it is an error.
pub const ROUNDS: usize = 8;

/// The depth to which a span is halved for its bound: at most 2^`PIECES` pieces.
pub const PIECES: usize = 4;

/// The side to read a parameter from, inside `domain`.
fn side(x: f64, (_, hi): (f64, f64)) -> Limit {
    if x < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// `a + b`, rounded up.
fn up(a: f64, b: f64) -> Result<f64, IntersectError> {
    Ok(Interval::point(a)?.add(Interval::point(b)?)?.hi())
}

/// The point of `surface` at (u, v), in doubles.
fn at(surface: &SurfaceKind, u: f64, v: f64) -> Result<[f64; 3], IntersectError> {
    let (us, vs) = surface.domain();
    Ok(surface.point(u, v, (side(u, us), side(v, vs)))?)
}

/// The caches fitted through `data`, and each datum's parameter.
type Fitted = (Curve<3>, [Curve<2>; 2], Vec<f64>);

/// The caches through `data`, (u₁, v₁, u₂, v₂) on the curve, the 3D one within `tol` of the
/// surfaces' points there.
fn caches(
    first: &SurfaceKind,
    data: &[[f64; 4]],
    tol: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Fitted, IntersectError> {
    let mut space = Vec::with_capacity(data.len());
    for x in data {
        let [u, v, ..] = *x;
        space.push(at(first, u, v)?);
    }
    let degree = DEGREE.min(data.len().saturating_sub(1)).max(1);
    let f3 = Curve::approximate(&space, degree, tol, KnotMode::Single, FitVersion::V1, meter)
        .map_err(CurveError::from)?;
    let (Some(&t0), Some(&t1)) = (f3.parameters.first(), f3.parameters.last()) else {
        return Err(IntersectError::Internal);
    };
    let mut pcurves = Vec::with_capacity(2);
    for k in [0, 2] {
        let pts: Vec<[f64; 2]> = data
            .iter()
            .map(|x| {
                let (u, v) = (x.get(k).copied(), x.get(k.saturating_add(1)).copied());
                [u.unwrap_or(f64::NAN), v.unwrap_or(f64::NAN)]
            })
            .collect();
        let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
            return Err(IntersectError::Internal);
        };
        let held = [
            Constraint {
                parameter: t0,
                order: 0,
                value: a,
            },
            Constraint {
                parameter: t1,
                order: 0,
                value: b,
            },
        ];
        let fit = Curve::<2>::least_squares_at(
            &pts,
            None,
            &f3.parameters,
            f3.curve.basis().clone(),
            &held,
            FitVersion::V1,
            meter,
        )
        .map_err(CurveError::from)?;
        pcurves.push(fit.curve);
    }
    let [p, q]: [Curve<2>; 2] = pcurves.try_into().map_err(|_| IntersectError::Internal)?;
    Ok((f3.curve, [p, q], f3.parameters))
}

/// The spans of `cache`: its distinct knots inside its domain, in order.
fn spans_of(cache: &Curve<3>) -> Vec<(f64, f64)> {
    let (lo, hi) = ParamCurve::domain(cache);
    let mut cuts: Vec<f64> = cache
        .basis()
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo < k && k < hi)
        .collect();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len().saturating_add(1));
    let mut from = lo;
    for k in cuts {
        out.push((from, k));
        from = k;
    }
    out.push((from, hi));
    out
}

/// A box of (u, v) as a region.
fn region(b: &[Interval; 2]) -> Region {
    let [u, v] = *b;
    ((u.lo(), u.hi()), (v.lo(), v.hi()))
}

/// `b` widened by `by` on every side, kept inside `domain`.
fn widened(b: Region, by: f64, ((u0, u1), (v0, v1)): Region) -> Region {
    let ((a, c), (d, e)) = b;
    (
        (larger(u0, a - by), smaller(u1, c + by)),
        (larger(v0, d - by), smaller(v1, e + by)),
    )
}

/// The smaller of two doubles.
fn smaller(a: f64, b: f64) -> f64 {
    geom_curve::tight::smaller(a, b)
}

/// Whether `b` lies inside `domain`.
fn inside(((a, c), (d, e)): Region, ((u0, u1), (v0, v1)): Region) -> bool {
    u0 <= a && c <= u1 && v0 <= d && e <= v1
}

/// How far `S(p(t))` may be from `C(t)` over `piece` (module documentation), and the pcurve's box
/// there; `None` where a bound cannot be had.
fn gap(
    surface: &SurfaceKind,
    pcurve: &Trimmed<2>,
    cache: &Curve<3>,
    (a, b): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(f64, Region)>, IntersectError> {
    let domain = surface.domain();
    let m = a / 2.0 + b / 2.0;
    let half = Interval::point(b)?.sub(Interval::point(a)?)?.hi() / 2.0;
    let whole = ParamCurve::domain(cache);
    let (pe, ce) = (
        pcurve.enclose(m, side(m, whole), 0, meter)?,
        cache.enclose(m, side(m, whole), 0, meter)?,
    );
    let ([pm, ..], [cm, ..]) = (pe.as_slice(), ce.as_slice()) else {
        return Err(IntersectError::Internal);
    };
    let (pm, cm) = (*pm, *cm);
    let here = region(&pm);
    let over = region(&boxed(&pcurve.bound((a, b), meter)?)?);
    if !inside(here, domain) || !inside(over, domain) {
        return Ok(None);
    }
    let Some(sm) = partials(surface, here, 0, meter)? else {
        return Ok(None);
    };
    let [sm, ..] = sm.as_slice() else {
        return Err(IntersectError::Internal);
    };
    let fm = minus_box(sm, &cm)?;
    let (DerivativeBound::Bounded(dp), DerivativeBound::Bounded(dc)) = (
        pcurve.derivative_bound((a, b), 1, meter)?,
        cache.derivative_bound((a, b), 1, meter)?,
    ) else {
        return Ok(None);
    };
    let ([du, dv], dc) = (boxed(&dp)?, boxed(&dc)?);
    let Some(sp) = partials(surface, over, 1, meter)? else {
        return Ok(None);
    };
    let [_, su, sv, ..] = sp.as_slice() else {
        return Err(IntersectError::Internal);
    };
    let mut slope = [Interval::point(0.0)?; 3];
    for (k, s) in slope.iter_mut().enumerate() {
        let (Some(x), Some(y), Some(c)) = (su.get(k), sv.get(k), dc.get(k)) else {
            return Err(IntersectError::Internal);
        };
        *s = x.mul(du)?.add(y.mul(dv)?)?.sub(*c)?;
    }
    let d = Interval::norm(&fm)?
        .add(Interval::norm(&slope)?.mul(Interval::point(half)?)?)?
        .hi();
    Ok(Some((d, over)))
}

/// Over `b` on `surface`: a lower bound on how fast S moves, σ ≥ |n|/√(E + G), and an upper bound
/// on how fast its normal turns per unit length, κ ≤ √(|`n_u`|² + |`n_v`|²)·√(E + G)/|n|² (module
/// documentation); `None` where |n| may vanish or the partials cannot be bounded.
fn turning(
    surface: &SurfaceKind,
    b: Region,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(f64, f64)>, IntersectError> {
    let Some(p) = partials(surface, b, 2, meter)? else {
        return Ok(None);
    };
    let get = |i: usize, j: usize| -> Result<[Interval; 3], IntersectError> {
        p.get(partial_index(i, j)?)
            .copied()
            .ok_or(IntersectError::Internal)
    };
    let (su, sv) = (get(1, 0)?, get(0, 1)?);
    let (suu, suv, svv) = (get(2, 0)?, get(1, 1)?, get(0, 2)?);
    let n = cross(&su, &sv)?;
    let add3 = |x: [Interval; 3], y: [Interval; 3]| -> Result<[Interval; 3], IntersectError> {
        let [a, b, c] = x;
        let [d, e, f] = y;
        Ok([a.add(d)?, b.add(e)?, c.add(f)?])
    };
    let nu = add3(cross(&suu, &sv)?, cross(&su, &suv)?)?;
    let nv = add3(cross(&suv, &sv)?, cross(&su, &svv)?)?;
    // Norms, never square roots of sums: a sum of squares rounded outward may dip below zero.
    let size = Interval::norm(&n)?;
    if size.lo() <= 0.0 {
        return Ok(None);
    }
    let [a, b, c] = su;
    let [d, e, f] = sv;
    let trace = Interval::norm(&[a, b, c, d, e, f])?;
    let [a, b, c] = nu;
    let [d, e, f] = nv;
    let turn = Interval::norm(&[a, b, c, d, e, f])?;
    let kappa = turn.mul(trace)?.div(size.square()?)?.hi();
    let sigma = size.div(trace)?.lo();
    Ok((sigma > 0.0 && kappa.is_finite()).then_some((sigma, kappa)))
}

/// A lower bound on the sine of the angle between the normals over `a` on `first` and `b` on
/// `second`.
fn sine(
    (first, a): (&SurfaceKind, Region),
    (second, b): (&SurfaceKind, Region),
    meter: &mut WorkMeter<'_>,
) -> Result<f64, IntersectError> {
    let (n1, n2) = (normals(first, a, meter)?, normals(second, b, meter)?);
    let c = Interval::norm(&cross(&n1, &n2)?)?;
    let size = Interval::norm(&n1)?.mul(Interval::norm(&n2)?)?;
    if size.hi() <= 0.0 || !size.hi().is_finite() {
        return Ok(0.0);
    }
    Ok(larger(0.0, c.lo() / size.hi() * (1.0 - f64::EPSILON)))
}

/// How far the crossing may be from a point within `e` of both surfaces, where they cross at a sine
/// of at least `s` and turn by at most `kappa` (12.2's bound, as [`crate::bound`] proves it), or
/// `None` where the angle cannot carry it; with r₀ = 2e/s, the reach it needs that to hold over.
fn moved(e: f64, s: f64, kappa: f64) -> Result<Option<(f64, f64)>, IntersectError> {
    if e <= 0.0 {
        return Ok(Some((0.0, 0.0)));
    }
    if s <= 0.0 {
        return Ok(None);
    }
    let (e, si, k) = (
        Interval::point(e)?,
        Interval::point(s)?,
        Interval::point(kappa)?,
    );
    let two = Interval::point(2.0)?;
    let r0 = two.mul(e)?.div(si)?;
    let left = si.sub(two.mul(k)?.mul(r0)?)?;
    if left.lo() * 2.0 <= s {
        return Ok(None);
    }
    Ok(Some((e.div(left)?.hi(), r0.hi())))
}

/// The bounds of one piece of a span, or `None` where they cannot be had.
fn piece(
    pair: [&SurfaceKind; 2],
    caches: (&Curve<3>, &[Trimmed<2>; 2]),
    t: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Span>, IntersectError> {
    let (cache, pcurves) = caches;
    let mut d = [0.0; 2];
    let mut boxes = Vec::with_capacity(2);
    for ((surface, pcurve), slot) in pair.iter().zip(pcurves).zip(&mut d) {
        let Some((gap, over)) = gap(surface, pcurve, cache, t, meter)? else {
            return Ok(None);
        };
        *slot = gap;
        boxes.push(over);
    }
    let [d1, d2] = d;
    let e = up(d1, d2)?;
    // Over boxes wide enough to hold every point within the reach of the cache, where the angle
    // and the curvature must hold: twice the reach the last pass needed, and twice the width in
    // parameters that the speed there asks, so a slightly slower wide box still holds it.
    let mut reach = 0.0;
    for _ in 0..3 {
        let mut wide = Vec::with_capacity(2);
        let mut kappa: f64 = 0.0;
        for ((surface, over), di) in pair.iter().zip(&boxes).zip(d) {
            let Some((sigma, _)) = turning(surface, *over, meter)? else {
                return Ok(None);
            };
            let by = 2.0 * up(reach, di)? / sigma;
            let w = widened(*over, by, surface.domain());
            let Some((sigma_w, k)) = turning(surface, w, meter)? else {
                return Ok(None);
            };
            if up(reach, di)? / sigma_w > by {
                return Ok(None);
            }
            kappa = larger(kappa, k);
            wide.push(w);
        }
        let (Some(a), Some(b)) = (wide.first(), wide.get(1)) else {
            return Err(IntersectError::Internal);
        };
        let [one, two] = pair;
        let s = sine((one, *a), (two, *b), meter)?;
        let Some((curve, r0)) = moved(e, s, kappa)? else {
            return Ok(None);
        };
        if r0 <= reach {
            return Ok(Some(Span {
                at: t,
                curve,
                pcurves: d,
            }));
        }
        reach = 2.0 * r0;
    }
    Ok(None)
}

/// The bounds of the span `t`, halved up to [`PIECES`] times until within `ceiling`, the largest of
/// its pieces'; `None` where a piece at the last depth has no bound.
fn span(
    pair: [&SurfaceKind; 2],
    caches: (&Curve<3>, &[Trimmed<2>; 2]),
    t: (f64, f64),
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Span>, IntersectError> {
    let mut out = Span {
        at: t,
        curve: 0.0,
        pcurves: [0.0; 2],
    };
    let mut work = alloc::vec![(t, 0usize)];
    while let Some(((a, b), depth)) = work.pop() {
        meter.charge(1)?;
        let read = piece(pair, caches, (a, b), meter)?;
        let fits = read.is_some_and(|s| s.curve <= ceiling);
        let m = a / 2.0 + b / 2.0;
        if !fits && depth < PIECES && a < m && m < b {
            let deeper = depth.saturating_add(1);
            work.push(((m, b), deeper));
            work.push(((a, m), deeper));
            continue;
        }
        let Some(s) = read else {
            return Ok(None);
        };
        let [p, q] = s.pcurves;
        let [op, oq] = out.pcurves;
        out = Span {
            at: t,
            curve: larger(out.curve, s.curve),
            pcurves: [larger(op, p), larger(oq, q)],
        };
    }
    Ok(Some(out))
}

/// The plane across the cache at t: S₁ − S₂ = 0 and T·(S₁ − C(t)) = 0, in (u₁, v₁, u₂, v₂), and the
/// floor a box's reach is counted in.
struct Across<'a> {
    first: &'a SurfaceKind,
    second: &'a SurfaceKind,
    point: [f64; 3],
    tangent: [f64; 3],
    floor: f64,
}

/// A few steps of the double at `p`: the finest a point there can be told apart.
fn steps_at(p: &[f64; 3]) -> f64 {
    let size = p.iter().fold(1.0, |m, x| larger(m, x.abs()));
    4.0 * f64::EPSILON * size
}

impl Across<'_> {
    fn residuals(
        &self,
        s1: &[Interval; 3],
        s2: &[Interval; 3],
    ) -> Result<[Interval; 4], IntersectError> {
        let [a, b, c] = minus_box(s1, s2)?;
        let off = minus_box(s1, &exact(&self.point)?)?;
        let plane = dot(&exact(&self.tangent)?, &off)?;
        Ok([a, b, c, plane])
    }
}

impl Field<4> for Across<'_> {
    fn at(&self, x: [f64; 4], meter: &mut WorkMeter<'_>) -> Result<[Interval; 4], IntersectError> {
        let [u1, v1, u2, v2] = x;
        let (Some(p), Some(q)) = (
            partials(self.first, ((u1, u1), (v1, v1)), 0, meter)?,
            partials(self.second, ((u2, u2), (v2, v2)), 0, meter)?,
        ) else {
            return Err(IntersectError::Internal);
        };
        let ([s1, ..], [s2, ..]) = (p.as_slice(), q.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        self.residuals(s1, s2)
    }

    fn over(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 4]>, IntersectError> {
        let [u1, v1, u2, v2] = *x;
        let (Some(p), Some(q)) = (
            partials(
                self.first,
                ((u1.lo(), u1.hi()), (v1.lo(), v1.hi())),
                0,
                meter,
            )?,
            partials(
                self.second,
                ((u2.lo(), u2.hi()), (v2.lo(), v2.hi())),
                0,
                meter,
            )?,
        ) else {
            return Ok(None);
        };
        let ([s1, ..], [s2, ..]) = (p.as_slice(), q.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        Ok(Some(self.residuals(s1, s2)?))
    }

    fn slope(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 4]; 4]>, IntersectError> {
        let [u1, v1, u2, v2] = *x;
        let (Some(p), Some(q)) = (
            partials(
                self.first,
                ((u1.lo(), u1.hi()), (v1.lo(), v1.hi())),
                1,
                meter,
            )?,
            partials(
                self.second,
                ((u2.lo(), u2.hi()), (v2.lo(), v2.hi())),
                1,
                meter,
            )?,
        ) else {
            return Ok(None);
        };
        let ([_, au, av, ..], [_, bu, bv, ..]) = (p.as_slice(), q.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        let zero = Interval::point(0.0)?;
        let mut rows = [[zero; 4]; 4];
        for (k, row) in rows.iter_mut().take(3).enumerate() {
            let pick = |v: &[Interval; 3]| v.get(k).copied().ok_or(IntersectError::Internal);
            *row = [pick(au)?, pick(av)?, pick(bu)?.neg(), pick(bv)?.neg()];
        }
        let t = exact(&self.tangent)?;
        let last = rows.get_mut(3).ok_or(IntersectError::Internal)?;
        *last = [dot(&t, au)?, dot(&t, av)?, zero, zero];
        Ok(Some(rows))
    }

    fn reach(&self, x: &[Interval; 4], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        let [u1, v1, ..] = *x;
        let b = around(self.first, ((u1.lo(), u1.hi()), (v1.lo(), v1.hi())), meter)?;
        Ok(image(&b) / self.floor)
    }
}

/// The point of the true curve in the plane across `curve`'s 3D cache at `t`, (u₁, v₁, u₂, v₂)
/// proven the only one in a box about the pcurves' points there, or `None` where none is proven
/// (module documentation).
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for `t` outside the curve's domain; the kinds' and the meter's.
pub fn true_point(
    first: &SurfaceKind,
    second: &SurfaceKind,
    curve: &IntersectionCurve<3>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[Interval; 4]>, IntersectError> {
    let domain = ParamCurve::domain(curve);
    if !(domain.0 <= t && t <= domain.1) {
        return Err(IntersectError::InvalidInput);
    }
    let along = side(t, domain);
    let mut d = [[0.0; 3]; 2];
    ParamCurve::derivatives(curve.cache(), t, along, &mut d)?;
    let [point, tangent] = d;
    let field = Across {
        first,
        second,
        point,
        tangent,
        floor: steps_at(&point),
    };
    // A box about the pcurves' points as wide as the span's bounds reach in parameters, and never
    // below a step of the double.
    let span = curve
        .spans()
        .iter()
        .find(|s| s.at.0 <= t && t <= s.at.1)
        .ok_or(IntersectError::Internal)?;
    let mut b = Vec::with_capacity(4);
    for ((surface, pcurve), gap) in [first, second]
        .into_iter()
        .zip(curve.pcurves())
        .zip(span.pcurves)
    {
        let read = pcurve.enclose(t, along, 0, meter)?;
        let [x, ..] = read.as_slice() else {
            return Err(IntersectError::Internal);
        };
        // Kept inside the domain: an enclosure at an end may reach a rounding past it. σ is read
        // over the span's whole pcurve box, a lower bound for any part of it: a kind's bound over
        // a box a few doubles wide is too loose to give one.
        let here = widened(region(x), 0.0, surface.domain());
        let over = widened(
            region(&boxed(&pcurve.bound(span.at, meter)?)?),
            0.0,
            surface.domain(),
        );
        let Some((sigma, _)) = turning(surface, over, meter)? else {
            return Ok(None);
        };
        // The curve is within the span's bound of C(t), and Sᵢ(pᵢ(t)) within its pcurve's, so the
        // point is that far, over σ, in parameters; twice that holds the plane's point too, and a
        // few steps of the double stand in for a bound of zero.
        let ((a, c), (e, f)) = here;
        let size = larger(larger(a.abs(), c.abs()), larger(e.abs(), f.abs()));
        let by = larger(
            2.0 * up(span.curve, gap)? / sigma,
            4.0 * f64::EPSILON * (1.0 + size),
        );
        let w = widened(here, by, surface.domain());
        let ((a, c), (e, f)) = w;
        b.push(Interval::new(a, c)?);
        b.push(Interval::new(e, f)?);
    }
    let domain4: [Interval; 4] = b.try_into().map_err(|_| IntersectError::Internal)?;
    let found = solve(&field, domain4, |_, _| Ok(true), meter)?;
    let [zero] = found.zeros.as_slice() else {
        return Ok(None);
    };
    Ok(found.flagged.is_empty().then_some(zero.enclosure))
}

/// The intersection curve of `branch` on `first` and `second`, which the pool holds as `refs`,
/// every span within `ceiling` (module documentation).
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a ceiling that is not positive or a branch of fewer than
/// two vertices; [`IntersectError::BudgetExceeded`] when [`ROUNDS`] rounds of refinement leave a
/// span over the ceiling, or the meter stops; the fit's and the kinds' errors.
pub fn fit(
    (first, second): (&SurfaceKind, &SurfaceKind),
    refs: [SurfaceRef; 2],
    branch: &Branch,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<IntersectionCurve<3>, IntersectError> {
    if ceiling.is_nan() || ceiling <= 0.0 || branch.vertices.len() < 2 {
        return Err(IntersectError::InvalidInput);
    }
    // The data: the branch's vertices, closed back to the first for a loop, and which of them are
    // the definition's.
    let mut data: Vec<[f64; 4]> = branch.vertices.iter().map(|x| x.map(mid)).collect();
    let mut chain: Vec<Option<usize>> = (0..branch.vertices.len()).map(Some).collect();
    if branch.ends == [CurveEnd::Closed, CurveEnd::Closed] {
        let first_point = *data.first().ok_or(IntersectError::Internal)?;
        data.push(first_point);
        chain.push(None);
    }
    for _ in 0..ROUNDS {
        meter.charge(1)?;
        let (cache, [p, q], parameters) = caches(first, &data, ceiling / 8.0, meter)?;
        let pcurves = [
            Trimmed::whole(Leaf::Nurbs(p))?,
            Trimmed::whole(Leaf::Nurbs(q))?,
        ];
        let mut spans = Vec::new();
        let mut failing = Vec::new();
        for t in spans_of(&cache) {
            match span([first, second], (&cache, &pcurves), t, ceiling, meter)? {
                Some(s) if s.curve <= ceiling => spans.push(s),
                _ => failing.push(t),
            }
        }
        if failing.is_empty() {
            let mut marched = Vec::with_capacity(branch.vertices.len());
            for (k, at) in chain.iter().zip(&parameters) {
                if let Some(i) = k {
                    let x = branch.vertices.get(*i).ok_or(IntersectError::Internal)?;
                    marched.push(Marched { at: *x, t: *at });
                }
            }
            return Ok(IntersectionCurve::new(
                refs, marched, cache, pcurves, spans,
            )?);
        }
        // Refine: a point of the true curve between each two data points in a failing span.
        let mut next = Vec::with_capacity(data.len().saturating_mul(2));
        let mut next_chain = Vec::with_capacity(next.capacity());
        let mut added = false;
        for (k, x) in data.iter().enumerate() {
            next.push(*x);
            next_chain.push(chain.get(k).copied().flatten());
            let (Some(&ta), Some(&tb), Some(y)) = (
                parameters.get(k),
                parameters.get(k.saturating_add(1)),
                data.get(k.saturating_add(1)),
            ) else {
                continue;
            };
            if !failing.iter().any(|&(a, b)| ta < b && a < tb) {
                continue;
            }
            let guess: [f64; 4] = core::array::from_fn(|i| {
                x.get(i).copied().unwrap_or(0.0) / 2.0 + y.get(i).copied().unwrap_or(0.0) / 2.0
            });
            if let Some(z) = settle_point((first, second), guess, x, y, meter)? {
                next.push(z);
                next_chain.push(None);
                added = true;
            }
        }
        if !added {
            return Err(IntersectError::BudgetExceeded);
        }
        data = next;
        chain = next_chain;
    }
    Err(IntersectError::BudgetExceeded)
}

/// A point of the true curve near `guess`, in the plane halfway between the data points `x` and
/// `y` across their chord, by Newton's method in doubles: fit data, which the bounds then judge.
fn settle_point(
    (first, second): (&SurfaceKind, &SurfaceKind),
    guess: [f64; 4],
    x: &[f64; 4],
    y: &[f64; 4],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[f64; 4]>, IntersectError> {
    let ([xu, xv, ..], [yu, yv, ..]) = (*x, *y);
    let (a, b) = (at(first, xu, xv)?, at(first, yu, yv)?);
    let point: [f64; 3] = core::array::from_fn(|i| {
        a.get(i).copied().unwrap_or(0.0) / 2.0 + b.get(i).copied().unwrap_or(0.0) / 2.0
    });
    let tangent: [f64; 3] = core::array::from_fn(|i| {
        b.get(i).copied().unwrap_or(0.0) - a.get(i).copied().unwrap_or(0.0)
    });
    let field = Across {
        first,
        second,
        point,
        tangent,
        floor: steps_at(&point),
    };
    let mut z = guess;
    for _ in 0..8 {
        meter.charge(1)?;
        let r = field.at(z, meter)?.map(mid);
        let Some(j) = field.slope(&exact(&z)?, meter)? else {
            return Ok(None);
        };
        let Some(y) = inverse(j.map(|row| row.map(mid))) else {
            return Ok(None);
        };
        for (w, row) in z.iter_mut().zip(y) {
            let mut s = 0.0;
            for (a, b) in row.iter().zip(r) {
                s += a * b;
            }
            *w -= s;
        }
    }
    let ((u1, v1), (u2, v2)) = (first.domain(), second.domain());
    let [a1, b1, a2, b2] = z;
    let ok = u1.0 <= a1 && a1 <= u1.1 && v1.0 <= b1 && b1 <= v1.1;
    let ok = ok && u2.0 <= a2 && a2 <= u2.1 && v2.0 <= b2 && b2 <= v2.1;
    Ok(ok.then_some(z))
}
