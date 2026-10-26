//! A kind converted to NURBS, and the map from the kind's parameter to the NURBS curve's.
//!
//! What it does: [`NurbsForm`] is what [`crate::ParamCurve::to_nurbs`] returns: the NURBS curve and
//! the [`ParamMap`] taking a parameter t of the kind to the parameter u of the NURBS curve at the
//! same point. Most kinds convert with the same parameter. The conics cannot: a circle traced
//! uniformly in angle is not a rational curve in its angle, so its exact NURBS form, the rational
//! quadratic, runs on the tangent half-angle substitution instead. [`ParamMap::enclose`] encloses
//! u for any t, so a caller (and the kind contract) can compare the kind and its NURBS form at the
//! same point.
//!
//! The conic maps: the domain is cut at breaks t₀ < t₁ < … < tₙ, and piece k is the NURBS curve's
//! span [k, k + 1]. With h = (tₖ₊₁ − tₖ)/2 and τ the tangent of half the angle, τ(x) = tan πx for
//! an ellipse (t in half-turns) and tanh x for a hyperbola, u = k + τ((t − tₖ)/2)/τ(h) on an even
//! piece and u = k + 1 − τ((tₖ₊₁ − t)/2)/τ(h) on an odd one (whose weights run the other way, so
//! that the weights stay bounded along the curve).
//!
//! Guarantees:
//!
//! - **Certified.** The enclosure holds the exact u: τ is enclosed with `geom-num`'s proven `sinpi`,
//!   `cospi` and `exp` enclosures, and the result is cut to the piece's span, which holds the exact
//!   u because τ is increasing.
//! - **Refused outside.** A parameter outside the breaks is an error.
//!
//! Method: the rational quadratic form of a conic and its parametrisation (L. Piegl and W. Tiller,
//! *The NURBS Book*, 2nd ed., sections 7.2 and 7.3, <https://doi.org/10.1007/978-3-642-59223-2>),
//! where u is the stereographic (tangent half-angle) parameter of the circle and, for the
//! hyperbola, of the unit hyperbola.
//!
//! Cost: a binary search over the breaks and a few interval operations.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the maps': t the kind's parameter, u the NURBS curve's, a and b an interval, k a piece, l and r a segment's parameter and factor"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_nurbs::Curve;

use crate::error::CurveError;
use crate::form::count;
use crate::tight::{self, point};

/// A kind as NURBS: the curve, and the map from the kind's parameter to the curve's.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsForm<const D: usize> {
    /// The NURBS curve.
    pub curve: Curve<D>,
    /// The parameter map.
    pub map: ParamMap,
}

/// The map from a kind's parameter t to its NURBS form's parameter u (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub enum ParamMap {
    /// u = t.
    Same,
    /// An ellipse's tangent half-angle map, with t in half-turns, over these breaks.
    Circular(Vec<f64>),
    /// A hyperbola's tangent half-angle map over these breaks.
    Hyperbolic(Vec<f64>),
    /// A composite curve's map: each piece's parameters go affinely onto its segment's, through
    /// the segment's own map, then shift.
    Pieces(Vec<MapPiece>),
}

/// A segment's own map, inside a composite's: as [`ParamMap`]'s first three.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalMap {
    /// u = t.
    Same,
    /// An ellipse's tangent half-angle map over these breaks.
    Circular(Vec<f64>),
    /// A hyperbola's tangent half-angle map over these breaks.
    Hyperbolic(Vec<f64>),
}

/// One segment of a composite's map: parameters [start, end] go affinely onto the segment's
/// `local` interval, through its `map`, and `shift` is added to the result.
#[derive(Clone, Debug, PartialEq)]
pub struct MapPiece {
    /// The composite parameter where the segment starts.
    pub start: f64,
    /// The composite parameter where it ends.
    pub end: f64,
    /// The segment's own interval.
    pub local: (f64, f64),
    /// What the segment's map gives is shifted by this.
    pub shift: f64,
    /// The segment's own map.
    pub map: LocalMap,
}

/// tan(πx), enclosed: exact at 0 and at a quarter of a half-turn.
pub(crate) fn tan_pi(x: Interval) -> Result<Interval, CurveError> {
    if x.lo() == x.hi() {
        if x.lo() == 0.0 {
            return point(0.0);
        }
        if x.lo() == 0.25 {
            return point(1.0);
        }
    }
    tight::div(x.sinpi()?, x.cospi()?)
}

/// tanh x, enclosed: increasing, so from its ends; tanh x = (e^{2x} − 1)/(e^{2x} + 1).
pub(crate) fn tanh(x: Interval) -> Result<Interval, CurveError> {
    let at = |y: f64| -> Result<Interval, CurveError> {
        if y == 0.0 {
            return point(0.0);
        }
        let e = point(2.0 * y)?.exp()?;
        let one = point(1.0)?;
        tight::div(tight::sub(e, one)?, tight::add(e, one)?)
    };
    Ok(Interval::new(at(x.lo())?.lo(), at(x.hi())?.hi())?)
}

impl ParamMap {
    /// An enclosure of u for the kind's parameter `t`.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`] if `t` is not within the breaks.
    pub fn enclose(&self, t: f64) -> Result<Interval, CurveError> {
        match self {
            ParamMap::Same => point(t),
            ParamMap::Circular(b) => conic(b, true, t),
            ParamMap::Hyperbolic(b) => conic(b, false, t),
            ParamMap::Pieces(pieces) => composite(pieces, t),
        }
    }
}

/// A composite's map at `t`: the first piece holding it.
fn composite(pieces: &[MapPiece], t: f64) -> Result<Interval, CurveError> {
    let piece = pieces
        .iter()
        .find(|p| p.start <= t && t <= p.end)
        .ok_or(CurveError::ParameterOutside)?;
    let (a, b) = piece.local;
    let r = tight::div(
        tight::sub(point(b)?, point(a)?)?,
        tight::sub(point(piece.end)?, point(piece.start)?)?,
    )?;
    let l = tight::add(
        point(a)?,
        tight::mul(tight::sub(point(t)?, point(piece.start)?)?, r)?,
    )?;
    let (lo, hi) = (
        if l.lo() < a { a } else { l.lo() },
        if l.hi() > b { b } else { l.hi() },
    );
    let at = |x: f64| -> Result<Interval, CurveError> {
        match &piece.map {
            LocalMap::Same => point(x),
            LocalMap::Circular(br) => conic(br, true, x),
            LocalMap::Hyperbolic(br) => conic(br, false, x),
        }
    };
    // Each map is increasing, so its ends bound it over [lo, hi].
    let (u0, u1) = (at(lo)?, at(if hi < lo { lo } else { hi })?);
    let u = Interval::new(u0.lo(), if u1.hi() < u0.lo() { u0.lo() } else { u1.hi() })?;
    tight::add(u, point(piece.shift)?)
}

/// A conic's map at `t`, over its breaks (module documentation).
fn conic(breaks: &[f64], circular: bool, t: f64) -> Result<Interval, CurveError> {
    let (Some(&first), Some(&last)) = (breaks.first(), breaks.last()) else {
        return Err(CurveError::Defect);
    };
    if !(t.is_finite() && first <= t && t <= last) {
        return Err(CurveError::ParameterOutside);
    }
    // The first piece whose end is at or beyond t.
    let k = breaks.partition_point(|&b| b < t).saturating_sub(1);
    let (Some(&t0), Some(&t1)) = (breaks.get(k), breaks.get(k.saturating_add(1))) else {
        return Err(CurveError::Defect);
    };
    let half = point(0.5)?;
    let tau = |x: Interval| if circular { tan_pi(x) } else { tanh(x) };
    let sigma = tau(tight::mul(tight::sub(point(t1)?, point(t0)?)?, half)?)?;
    let kf = count(k)?;
    let u = if k & 1 == 0 {
        let s = tau(tight::mul(tight::sub(point(t)?, point(t0)?)?, half)?)?;
        tight::add(point(kf)?, tight::div(s, sigma)?)?
    } else {
        let s = tau(tight::mul(tight::sub(point(t1)?, point(t)?)?, half)?)?;
        tight::sub(point(kf + 1.0)?, tight::div(s, sigma)?)?
    };
    // The exact u lies in the piece's span [k, k + 1].
    let lo = if u.lo() < kf { kf } else { u.lo() };
    let hi = if u.hi() > kf + 1.0 { kf + 1.0 } else { u.hi() };
    Ok(Interval::new(lo, if hi < lo { lo } else { hi })?)
}
