//! The offset curve kind: a basis curve moved along its normal, in 2D, in 3D against a reference
//! direction, or by distances that vary along it, with a certified NURBS cache.
//!
//! What it does: [`Offset`] stores a trimmed basis curve C and an [`OffsetLaw`]:
//!
//! - `Planar { distance }` (2D): O = C + d·N/|N| with N = (−C′.y, C′.x), the left normal; a
//!   positive distance offsets to the left of the direction of travel.
//! - `Reference { distance, direction }` (3D): O = C + d·(C′ × V)/|C′ × V|, the offset along
//!   the cross product of the tangent and a reference direction V, as ISO 10303-42's
//!   `offset_curve_3d` does.
//! - `Distances(stations)`: O = C + l(t)·H/|H| + v(t)·ẑ, with H = (−C′.y, C′.x, 0) the horizontal
//!   left normal, and the lateral offset l and vertical offset v interpolated linearly between
//!   stations at basis parameters and held constant beyond the first and last: IFC 4.3's
//!   `IfcOffsetCurveByDistances`, with each distance along the basis converted to its parameter
//!   by the reader (in 2D, v is 0).
//!
//! The offset is procedural (chapter 7): its points and derivatives of every order come from the
//! basis's derivatives one order higher through interval Taylor jets (`crate::jet`), and its
//! NURBS form is a cache of Taylor pieces with a certified remainder (`crate::taylor`), computed
//! on demand or kept with [`Offset::cached`].
//!
//! Guarantees:
//!
//! - **Certified, or refused.** Where the normal may vanish (a cusp of the basis, or a span too
//!   wide to separate it from zero) a derivative bound is `Unbounded` and an enclosure is refused,
//!   never guessed; a bound on the points still holds there, from the basis's bound widened by
//!   the offset's largest size.
//! - **Exact split and reversal.** A split trims the basis; a reversal reverses the basis and
//!   negates the distances (the left normal of the reversed curve is the right normal).
//! - **Transforms keep the kind only when exact.** A grid transform of an offset whose basis moves
//!   exactly gives an offset (a mirror negating the side); any other transform gives the moved
//!   NURBS cache, with both deviations.
//!
//! Method: offset curves as defined in ISO 10303-42 (`offset_curve_2d`, `offset_curve_3d`) and
//! IFC 4.3 (`IfcOffsetCurveByDistances`), evaluated with automatic differentiation in interval
//! arithmetic; offsets of conics and splines are neither conics nor splines (R. T. Farouki and
//! C. A. Neff, "Analytic properties of plane offset curves", Computer Aided Geometric Design 7
//! (1990), <https://doi.org/10.1016/0167-8396(90)90023-K>), hence the certified cache.
//!
//! Cost: an evaluation of order k is the basis's of order k + 1 plus O(k²) interval operations;
//! the cache is the Taylor conversion of `crate::taylor`, eight to 128 pieces per smooth interval
//! of the basis, narrower pieces tried where wider ones cannot bound the offset's derivatives.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the basis, o the offset, n the normal, d a distance, l and v the lateral and vertical offsets, t a parameter"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::check_parameter;
use crate::jet::Jet;
use crate::map::NurbsForm;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::taylor;
use crate::tight::{self, point};
use crate::trimmed::Trimmed;

/// A station of an offset by distances: at the basis parameter `at`, the offsets `lateral` (left
/// of the direction of travel, horizontally) and `vertical` (along z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// The basis parameter.
    pub at: f64,
    /// The lateral offset.
    pub lateral: f64,
    /// The vertical offset.
    pub vertical: f64,
}

/// How a curve is offset (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub enum OffsetLaw {
    /// A constant distance along the left normal, in 2D.
    Planar {
        /// The distance.
        distance: f64,
    },
    /// A constant distance along the tangent crossed with a reference direction, in 3D.
    Reference {
        /// The distance.
        distance: f64,
        /// The reference direction.
        direction: [f64; 3],
    },
    /// Lateral and vertical offsets interpolated between stations.
    Distances(Vec<Station>),
}

/// An offset curve (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Offset<const D: usize> {
    basis: Trimmed<D>,
    law: OffsetLaw,
    cache: Option<(NurbsForm<D>, f64)>,
}

/// The degree of the cache's pieces.
const CACHE_DEGREE: usize = 9;

/// Where an offset is evaluated: one parameter from one side, or every parameter of a span.
#[derive(Clone, Copy)]
enum At {
    Point(f64, Limit),
    Span(f64, f64),
}

impl<const D: usize> Offset<D> {
    /// The offset of `basis` by `law`.
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] for a planar law outside 2D or a reference law outside 3D, or any
    /// law outside 2D and 3D; [`CurveError::NotFinite`]; [`CurveError::Degenerate`] for a zero
    /// reference direction, no stations, stations not strictly increasing, or a vertical offset
    /// in 2D.
    pub fn new(basis: Trimmed<D>, law: OffsetLaw) -> Result<Offset<D>, CurveError> {
        if D != 2 && D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        match &law {
            OffsetLaw::Planar { distance } => {
                if D != 2 {
                    return Err(CurveError::Dimension { dimension: D });
                }
                if !distance.is_finite() {
                    return Err(CurveError::NotFinite);
                }
            }
            OffsetLaw::Reference {
                distance,
                direction,
            } => {
                if D != 3 {
                    return Err(CurveError::Dimension { dimension: D });
                }
                if !(distance.is_finite() && direction.iter().all(|x| x.is_finite())) {
                    return Err(CurveError::NotFinite);
                }
                if direction.iter().all(|&x| x == 0.0) {
                    return Err(CurveError::Degenerate);
                }
            }
            OffsetLaw::Distances(stations) => {
                if stations.is_empty() {
                    return Err(CurveError::Degenerate);
                }
                for s in stations {
                    if !(s.at.is_finite() && s.lateral.is_finite() && s.vertical.is_finite()) {
                        return Err(CurveError::NotFinite);
                    }
                    if D == 2 && s.vertical != 0.0 {
                        return Err(CurveError::Degenerate);
                    }
                }
                for k in 1..stations.len() {
                    let a: &Station = stations
                        .get(k.saturating_sub(1))
                        .ok_or(CurveError::Defect)?;
                    let b: &Station = stations.get(k).ok_or(CurveError::Defect)?;
                    if a.at >= b.at {
                        return Err(CurveError::Degenerate);
                    }
                }
            }
        }
        Ok(Offset {
            basis,
            law,
            cache: None,
        })
    }

    /// The basis.
    #[must_use]
    pub fn basis(&self) -> &Trimmed<D> {
        &self.basis
    }

    /// The law.
    #[must_use]
    pub fn law(&self) -> &OffsetLaw {
        &self.law
    }

    /// The offset with its NURBS cache computed and kept: later conversions return it.
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::to_nurbs`].
    pub fn cached(&self, meter: &mut WorkMeter<'_>) -> Result<Offset<D>, CurveError> {
        let c = self.to_nurbs(meter)?;
        Ok(Offset {
            basis: self.basis.clone(),
            law: self.law.clone(),
            cache: Some((c.value, c.deviation)),
        })
    }

    /// The cache, if kept: the NURBS form and its certified deviation.
    #[must_use]
    pub fn cache(&self) -> Option<(&NurbsForm<D>, f64)> {
        self.cache.as_ref().map(|(n, d)| (n, *d))
    }

    /// The parameters where the offset may not be smooth: the basis's breaks and the stations,
    /// sorted, some perhaps outside the domain (`taylor::cuts_between` keeps the ones inside).
    fn breaks(&self) -> Vec<f64> {
        let mut b = self.basis.breaks();
        if let OffsetLaw::Distances(s) = &self.law {
            b.extend(s.iter().map(|x| x.at));
        }
        b.sort_by(f64::total_cmp);
        b.dedup();
        b
    }

    /// The basis's jets (orders 0 … `order` + 1) per coordinate.
    fn basis_jets(
        &self,
        at: At,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Jet>, CurveError> {
        let k = order.checked_add(1).ok_or(CurveError::Defect)?;
        let mut d: Vec<[Interval; D]> = Vec::with_capacity(k.saturating_add(1));
        match at {
            At::Point(t, side) => d = self.basis.enclose(t, side, k, meter)?,
            At::Span(lo, hi) => {
                for j in 0..=k {
                    let b = if j == 0 {
                        self.basis.bound((lo, hi), meter)?
                    } else {
                        match self.basis.derivative_bound((lo, hi), j, meter)? {
                            DerivativeBound::Bounded(b) => b,
                            DerivativeBound::Unbounded => return Err(CurveError::Unbounded),
                        }
                    };
                    let mut row = [point(0.0)?; D];
                    for (r, (&l, &h)) in row.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
                        *r = Interval::new(l, h)?;
                    }
                    d.push(row);
                }
            }
        }
        let mut out = Vec::with_capacity(D);
        for i in 0..D {
            let mut col = Vec::with_capacity(d.len());
            for row in &d {
                col.push(*row.get(i).ok_or(CurveError::Defect)?);
            }
            out.push(Jet::from_derivatives(&col)?);
        }
        Ok(out)
    }

    /// The lateral and vertical offsets' jets (orders 0 and 1, the rest 0) at `at`, which lies in
    /// one station piece.
    fn station_jets(stations: &[Station], at: At, order: usize) -> Result<(Jet, Jet), CurveError> {
        let (t, side) = match at {
            At::Point(t, side) => (Interval::point(t)?, side),
            At::Span(lo, hi) => (Interval::new(lo, hi)?, Limit::FromRight),
        };
        let (first, last) = match (stations.first(), stations.last()) {
            (Some(f), Some(l)) => (*f, *l),
            _ => return Err(CurveError::Defect),
        };
        let probe = match at {
            At::Point(x, _) => x,
            At::Span(lo, hi) => f64::midpoint(lo, hi),
        };
        let constant = |s: Station| -> Result<(Jet, Jet), CurveError> {
            Ok((
                Jet::constant(point(s.lateral)?, order)?,
                Jet::constant(point(s.vertical)?, order)?,
            ))
        };
        if probe < first.at || (probe == first.at && side == Limit::FromLeft) {
            return constant(first);
        }
        // The piece [a, b] holding the probe, from the side at a station; none past the last.
        let mut i: usize = 0;
        while let (Some(_), Some(b)) = (stations.get(i), stations.get(i.saturating_add(1))) {
            if probe < b.at || (probe == b.at && side == Limit::FromLeft) {
                break;
            }
            i = i.saturating_add(1);
        }
        let (Some(a), Some(b)) = (stations.get(i), stations.get(i.saturating_add(1))) else {
            return constant(last);
        };
        let width = tight::sub(point(b.at)?, point(a.at)?)?;
        // A linear function: its value over t, its slope as the first derivative, then zeros.
        let linear = |x: f64, y: f64| -> Result<Jet, CurveError> {
            let slope = tight::div(tight::sub(point(y)?, point(x)?)?, width)?;
            let value = tight::add(point(x)?, tight::mul(slope, tight::sub(t, point(a.at)?)?)?)?;
            let mut d = alloc::vec![point(0.0)?; order.saturating_add(1)];
            if let Some(v) = d.first_mut() {
                *v = value;
            }
            if let Some(v) = d.get_mut(1) {
                *v = slope;
            }
            Jet::from_derivatives(&d)
        };
        Ok((
            linear(a.lateral, b.lateral)?,
            linear(a.vertical, b.vertical)?,
        ))
    }

    /// The offset's jets per coordinate, to `order`, at `at`.
    fn jets(
        &self,
        at: At,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Jet>, CurveError> {
        let c = self.basis_jets(at, order, meter)?;
        let mut cp = Vec::with_capacity(D);
        let mut c0 = Vec::with_capacity(D);
        for j in &c {
            cp.push(j.derivative()?);
            c0.push(j.truncate(order));
        }
        let zero = Jet::constant(point(0.0)?, order)?;
        let get = |v: &[Jet], i: usize| -> Result<Jet, CurveError> {
            v.get(i).cloned().ok_or(CurveError::Defect)
        };
        // The direction n (not normalised) and the offsets along it and along z.
        let (n, along, up): (Vec<Jet>, Jet, Option<Jet>) = match &self.law {
            OffsetLaw::Planar { distance } => (
                alloc::vec![get(&cp, 1)?.scale(point(-1.0)?)?, get(&cp, 0)?],
                Jet::constant(point(*distance)?, order)?,
                None,
            ),
            OffsetLaw::Reference {
                distance,
                direction,
            } => {
                let [v0, v1, v2] = *direction;
                let (x, y, z) = (get(&cp, 0)?, get(&cp, 1)?, get(&cp, 2)?);
                let (a, b, e) = (point(v0)?, point(v1)?, point(v2)?);
                (
                    alloc::vec![
                        y.scale(e)?.sub(&z.scale(b)?)?,
                        z.scale(a)?.sub(&x.scale(e)?)?,
                        x.scale(b)?.sub(&y.scale(a)?)?,
                    ],
                    Jet::constant(point(*distance)?, order)?,
                    None,
                )
            }
            OffsetLaw::Distances(stations) => {
                let (l, v) = Offset::<D>::station_jets(stations, at, order)?;
                let mut n = alloc::vec![get(&cp, 1)?.scale(point(-1.0)?)?, get(&cp, 0)?];
                if D == 3 {
                    n.push(zero.clone());
                }
                (n, l, if D == 3 { Some(v) } else { None })
            }
        };
        let mut nn = zero.clone();
        for j in &n {
            nn = nn.add(&j.mul(j)?)?;
        }
        let len = nn.sqrt()?;
        let mut out = Vec::with_capacity(D);
        for (i, ci) in c0.iter().enumerate() {
            let ni = n.get(i).cloned().unwrap_or_else(|| zero.clone());
            let mut o = ci.add(&along.mul(&ni.div(&len)?)?)?;
            if i == 2
                && let Some(v) = &up
            {
                o = o.add(v)?;
            }
            out.push(o);
        }
        Ok(out)
    }

    /// The offset's derivatives to `order` at `at`, enclosed, by order then coordinate.
    fn derivative_enclosures(
        &self,
        at: At,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        let jets = self.jets(at, order, meter)?;
        let mut out = alloc::vec![[point(0.0)?; D]; order.saturating_add(1)];
        for (i, j) in jets.iter().enumerate() {
            for (k, x) in j.derivatives()?.into_iter().enumerate() {
                if let Some(slot) = out.get_mut(k).and_then(|r| r.get_mut(i)) {
                    *slot = x;
                }
            }
        }
        Ok(out)
    }

    /// The span cut at the breaks inside it and into a few pieces each, for tighter jets.
    fn pieces(&self, (lo, hi): (f64, f64)) -> Result<Vec<(f64, f64)>, CurveError> {
        let c = taylor::cuts_between((lo, hi), &self.breaks(), 4)?;
        let mut out = Vec::with_capacity(c.len());
        for p in c.windows(2) {
            if let (Some(&x), Some(&y)) = (p.first(), p.get(1)) {
                out.push((x, y));
            }
        }
        Ok(out)
    }

    /// The largest size of the offset itself (|d|, or the largest lateral offset and the range of
    /// the vertical one), for the fallback point bound.
    fn reach(&self) -> Result<(f64, Interval), CurveError> {
        Ok(match &self.law {
            OffsetLaw::Planar { distance } | OffsetLaw::Reference { distance, .. } => {
                (distance.abs(), point(0.0)?)
            }
            OffsetLaw::Distances(s) => {
                let mut lat: f64 = 0.0;
                let (mut vlo, mut vhi) = (f64::INFINITY, f64::NEG_INFINITY);
                for x in s {
                    lat = tight::larger(lat, x.lateral.abs());
                    vlo = tight::smaller(vlo, x.vertical);
                    vhi = tight::larger(vhi, x.vertical);
                }
                (lat, Interval::new(vlo, vhi)?)
            }
        })
    }
}

impl<const D: usize> ParamCurve<D> for Offset<D> {
    fn domain(&self) -> (f64, f64) {
        self.basis.interval()
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        let mut out = [[0.0; D]; 1];
        self.derivatives(t, side, &mut out)?;
        let [p] = out;
        Ok(p)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        let m = &mut WorkMeter::new(geom_run::Budget::UNLIMITED);
        let e = self.enclose(t, side, out.len().saturating_sub(1), m)?;
        for (o, x) in out.iter_mut().zip(&e) {
            *o = x.map(tight::middle);
        }
        Ok(())
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        check_parameter(self.domain(), t)?;
        self.derivative_enclosures(At::Point(t, side), order, meter)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        crate::nurbs::check_span(span, self.domain())?;
        let mut out = Bounds {
            lo: [f64::INFINITY; D],
            hi: [f64::NEG_INFINITY; D],
        };
        for (lo, hi) in self.pieces(span)? {
            let b = match self.derivative_enclosures(At::Span(lo, hi), 0, meter) {
                Ok(e) => {
                    let x = e.first().ok_or(CurveError::Defect)?;
                    Bounds {
                        lo: x.map(Interval::lo),
                        hi: x.map(Interval::hi),
                    }
                }
                Err(CurveError::Unbounded) => {
                    // The normal may vanish: the basis's box widened by the offset's size, and
                    // in z by the vertical offset's range.
                    let mut b = self.basis.bound((lo, hi), meter)?;
                    let (r, v) = self.reach()?;
                    for (i, (l, h)) in b.lo.iter_mut().zip(b.hi.iter_mut()).enumerate() {
                        let mut nl = tight::sub(point(*l)?, point(r)?)?.lo();
                        let mut nh = tight::add(point(*h)?, point(r)?)?.hi();
                        if i == 2 {
                            nl = tight::add(point(nl)?, point(v.lo())?)?.lo();
                            nh = tight::add(point(nh)?, point(v.hi())?)?.hi();
                        }
                        *l = nl;
                        *h = nh;
                    }
                    b
                }
                Err(e) => return Err(e),
            };
            for ((l, h), (&x, &y)) in out
                .lo
                .iter_mut()
                .zip(out.hi.iter_mut())
                .zip(b.lo.iter().zip(&b.hi))
            {
                *l = tight::smaller(*l, x);
                *h = tight::larger(*h, y);
            }
        }
        Ok(out)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        crate::nurbs::check_span(span, self.domain())?;
        let mut out = Bounds {
            lo: [f64::INFINITY; D],
            hi: [f64::NEG_INFINITY; D],
        };
        for (lo, hi) in self.pieces(span)? {
            let e = match self.derivative_enclosures(At::Span(lo, hi), order, meter) {
                Ok(e) => e,
                Err(CurveError::Unbounded) => return Ok(DerivativeBound::Unbounded),
                Err(e) => return Err(e),
            };
            let x = e.get(order).ok_or(CurveError::Defect)?;
            for ((l, h), v) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(x) {
                *l = tight::smaller(*l, v.lo());
                *h = tight::larger(*h, v.hi());
            }
        }
        Ok(DerivativeBound::Bounded(out))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        if let Some((n, d)) = &self.cache {
            return Ok(Changed {
                value: n.clone(),
                deviation: *d,
            });
        }
        let breaks = self.breaks();
        let b = self.bound(self.domain(), meter)?;
        let scale =
            b.lo.iter()
                .chain(&b.hi)
                .fold(1.0_f64, |s, x| tight::larger(s, x.abs()));
        for per in [8, 16, 32, 64, 128] {
            let cuts = taylor::cuts_between(self.domain(), &breaks, per)?;
            match taylor::from_cuts(self, CACHE_DEGREE, &cuts, meter) {
                Ok(n) if n.deviation <= scale * 1e-10 || per == 128 => return Ok(n),
                Ok(_) => {}
                // Pieces so wide that the offset's derivatives cannot be bounded on them (its
                // normal not separated from zero): narrower ones may be (piece G69).
                Err(CurveError::Unbounded) if per < 128 => {}
                Err(e) => return Err(e),
            }
        }
        Err(CurveError::Defect)
    }

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (l, r) = self.basis.halves(t)?;
        let part = |basis| -> Result<Changed<CurveKind<D>>, CurveError> {
            Ok(Changed {
                value: CurveKind::Offset(Offset::new(basis, self.law.clone())?),
                deviation: 0.0,
            })
        };
        Ok((part(l)?, part(r)?))
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let b = self.basis.reverse(meter)?;
        let law = match &self.law {
            OffsetLaw::Planar { distance } => OffsetLaw::Planar {
                distance: -distance,
            },
            OffsetLaw::Reference {
                distance,
                direction,
            } => OffsetLaw::Reference {
                distance: -distance,
                direction: *direction,
            },
            OffsetLaw::Distances(s) => OffsetLaw::Distances(
                s.iter()
                    .rev()
                    .map(|x: &Station| Station {
                        at: -x.at,
                        lateral: -x.lateral,
                        vertical: x.vertical,
                    })
                    .collect(),
            ),
        };
        if b.deviation != 0.0 {
            return self.moved_cache(None, meter);
        }
        Ok(Changed {
            value: CurveKind::Offset(Offset::new(b.value, law)?),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        if let TypedTransform::Exact(g) = transform {
            let b = self.basis.moved(transform, meter)?;
            let in_plane = g.acts_in_plane();
            let flip = g.reverses_orientation();
            let law = match &self.law {
                OffsetLaw::Planar { distance } => Some(OffsetLaw::Planar {
                    distance: if flip { -distance } else { *distance },
                }),
                OffsetLaw::Reference {
                    distance,
                    direction,
                } => {
                    let m = g.matrix();
                    let mut v = [0.0; 3];
                    for (o, row) in v.iter_mut().zip(&m) {
                        let mut s = 0.0;
                        for (&e, &x) in row.iter().zip(direction) {
                            s += f64::from(e) * x;
                        }
                        *o = if flip { -s } else { s };
                    }
                    Some(OffsetLaw::Reference {
                        distance: *distance,
                        direction: v,
                    })
                }
                OffsetLaw::Distances(s) if in_plane => Some(OffsetLaw::Distances(
                    s.iter()
                        .map(|x: &Station| Station {
                            at: x.at,
                            lateral: if flip { -x.lateral } else { x.lateral },
                            vertical: x.vertical,
                        })
                        .collect(),
                )),
                OffsetLaw::Distances(_) => None,
            };
            if let Some(law) = law
                && b.deviation == 0.0
            {
                return Ok(Changed {
                    value: CurveKind::Offset(Offset::new(b.value, law)?),
                    deviation: 0.0,
                });
            }
        }
        self.moved_cache(Some(transform), meter)
    }
}

impl<const D: usize> Offset<D> {
    /// The NURBS cache, reversed or moved, as a NURBS kind with both deviations.
    fn moved_cache(
        &self,
        transform: Option<&TypedTransform>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let n = self.to_nurbs(meter)?;
        let c = &n.value.curve;
        let moved = match transform {
            Some(t) => ParamCurve::transformed(c, t, meter)?,
            None => ParamCurve::reversed(c, meter)?,
        };
        Ok(Changed {
            value: moved.value,
            deviation: tight::add(point(n.deviation)?, point(moved.deviation)?)?.hi(),
        })
    }
}
