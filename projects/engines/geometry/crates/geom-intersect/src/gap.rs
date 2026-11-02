//! How far a curve is from a point, and a pcurve mapped through its surface from a 3D curve,
//! certified: what the checker's group 2 (8.3) and the intersection curve's fit (12.12) bound.
//!
//! What it does: [`at_point`] encloses |C(t) − p| for a curve C and a point p, and [`pcurve_at`] a
//! pcurve's point. [`mapped`] reads one
//! piece [a, b] of a span for f(t) = S(p(t)) − C(t), a pcurve p mapped through its surface S
//! against a 3D curve C: an enclosure of |f(m)| at the middle m, and an upper bound on |f| over the
//! whole piece by the mean-value form.
//!
//! Guarantees:
//!
//! - **Certified.** Every value is an enclosure from the kinds' own bounds, never a sample.
//! - **The mean-value form.** Every value of f on the piece lies within h·sup|f′| of f(m), h the
//!   half-width, with f′ = `S_u`·p′ᵤ + `S_v`·p′ᵥ − C′ enclosed over the pcurve's box on the piece;
//!   so the bound shrinks with the square of the piece's width where the three agree to first
//!   order.
//! - **Refuses what it cannot bound.** A pcurve whose box leaves the surface's domain, or a kind
//!   whose derivative is not bounded there, gives `None`, never a bound.
//!
//! Method: the mean-value form of interval analysis (R. E. Moore, R. B. Kearfott and M. J. Cloud,
//! *Introduction to Interval Analysis*, SIAM, 2009, chapter 6,
//! <https://doi.org/10.1137/1.9780898717716>), as chapter 12.12 bounds a pcurve against its 3D
//! curve (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: a few reads of each kind per call, to first order. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the mean-value form's: S the surface, p the pcurve, C the curve, f their gap, m and h the piece's middle and half-width"
)]

use geom_curve::{CurveKind, DerivativeBound, ParamCurve};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::general::{minus_box, partials};
use crate::project::boxed;

/// The side a parameter is read from, inside `domain`.
fn side(t: f64, (_, hi): (f64, f64)) -> Limit {
    if t < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// An enclosure of |C(t) − p|, `curve` read at `t` from inside its domain.
///
/// # Errors
///
/// The kind's: a parameter outside the domain is [`IntersectError::InvalidInput`]; the meter's.
pub fn at_point(
    curve: &CurveKind<3>,
    t: f64,
    p: &[Interval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, IntersectError> {
    let read = curve.enclose(t, side(t, curve.domain()), 0, meter)?;
    let at = read.first().ok_or(IntersectError::Internal)?;
    Ok(Interval::norm(&minus_box(at, p)?)?)
}

/// An enclosure of the pcurve's point at `t`, read from inside its domain: a known curve's vertex
/// on one face (8.3's group 3, G177).
///
/// # Errors
///
/// The kind's: a parameter outside the domain is [`IntersectError::InvalidInput`]; the meter's.
pub fn pcurve_at(
    pcurve: &CurveKind<2>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 2], IntersectError> {
    let read = pcurve.enclose(t, side(t, pcurve.domain()), 0, meter)?;
    read.first().copied().ok_or(IntersectError::Internal)
}

/// A box of (u, v) as a region.
fn region(b: &[Interval; 2]) -> Region {
    let [u, v] = *b;
    ((u.lo(), u.hi()), (v.lo(), v.hi()))
}

/// Whether `r` lies inside `domain`.
fn inside(((a, b), (c, d)): Region, ((u0, u1), (v0, v1)): Region) -> bool {
    u0 <= a && b <= u1 && v0 <= c && d <= v1
}

/// For f(t) = S(p(t)) − C(t) on the piece `(a, b)` (module documentation): an enclosure of |f| at
/// the middle, and an upper bound on |f| over the piece; `None` where the kinds cannot bound it.
///
/// # Errors
///
/// The kinds' and the meter's.
pub fn mapped(
    surface: &SurfaceKind,
    (pcurve, curve): (&CurveKind<2>, &CurveKind<3>),
    (a, b): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(Interval, f64)>, IntersectError> {
    let whole = curve.domain();
    let domain = surface.domain();
    let m = a / 2.0 + b / 2.0;
    let half = Interval::point(b)?.sub(Interval::point(a)?)?.hi() / 2.0;
    let (pe, ce) = (
        pcurve.enclose(m, side(m, whole), 0, meter)?,
        curve.enclose(m, side(m, whole), 0, meter)?,
    );
    let (Some(pm), Some(cm)) = (pe.first(), ce.first()) else {
        return Ok(None);
    };
    let here = region(pm);
    let over = region(&boxed(&pcurve.bound((a, b), meter)?)?);
    if !inside(here, domain) || !inside(over, domain) {
        return Ok(None);
    }
    let Some(at) = partials(surface, here, 0, meter)? else {
        return Ok(None);
    };
    let sm = at.first().ok_or(IntersectError::Internal)?;
    let fm = minus_box(sm, cm)?;
    let (DerivativeBound::Bounded(dp), DerivativeBound::Bounded(dc)) = (
        pcurve.derivative_bound((a, b), 1, meter)?,
        curve.derivative_bound((a, b), 1, meter)?,
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
        let (Some(x), Some(y), Some(z)) = (su.get(k), sv.get(k), dc.get(k)) else {
            return Err(IntersectError::Internal);
        };
        *s = x.mul(du)?.add(y.mul(dv)?)?.sub(*z)?;
    }
    let middle = Interval::norm(&fm)?;
    let upper = middle
        .add(Interval::norm(&slope)?.mul(Interval::point(half)?)?)?
        .hi();
    Ok(Some((middle, upper)))
}
