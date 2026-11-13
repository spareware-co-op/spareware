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
//! - **A span of one break is bounded from both sides.** The offsets' slopes jump at a station
//!   and the basis's tangent may turn at one of its breaks, so a derivative bound over one such
//!   parameter alone holds the derivative from each side, and a bound on the points there holds
//!   the offset of each side, as the trait's rule for a span of one parameter asks
//!   ([`ParamCurve`]).
//! - **Converted wherever it is defined.** Where the normal vanishes, or cannot be told from zero
//!   even over the narrowest piece the conversion cuts (the domain's width times 2^−52, or two
//!   neighbouring doubles where those are farther apart), the offset is undefined (a cusp of the basis, or a tangent parallel to the reference direction),
//!   and the NURBS form is refused as `Degenerate`, never as `Unbounded`, which names an infinite
//!   domain. A normal that only comes near zero, as on a nearly flat helix offset along its axis,
//!   is cut finer there until its pieces are bounded (issue #526); only a remainder that
//!   overflows the doubles even on the narrowest pieces is refused, as `NotFinite`.
//! - **Exact split and reversal.** A split trims the basis; a reversal reverses the basis and
//!   negates the distances (the left normal of the reversed curve is the right normal).
//! - **Transforms keep the kind only when exact.** A grid transform of an offset whose basis moves
//!   exactly gives an offset (a mirror negating the side); any other transform gives the moved
//!   NURBS cache, its deviation the cache's stretched by the transform (at most by its linear
//!   part's Frobenius norm) plus the move's.
//!
//! Method: offset curves as defined in ISO 10303-42 (`offset_curve_2d`, `offset_curve_3d`) and
//! IFC 4.3 (`IfcOffsetCurveByDistances`), evaluated with automatic differentiation in interval
//! arithmetic; offsets of conics and splines are neither conics nor splines (R. T. Farouki and
//! C. A. Neff, "Analytic properties of plane offset curves", Computer Aided Geometric Design 7
//! (1990), <https://doi.org/10.1016/0167-8396(90)90023-K>), hence the certified cache.
//!
//! Cost: an evaluation of order k is the basis's of order k + 1 plus O(k²) interval operations;
//! the cache is the Taylor conversion of `crate::taylor`, eight to 128 pieces per smooth interval
//! of the basis, narrower pieces tried where wider ones cannot bound the offset's derivatives;
//! where even 128 cannot, only the pieces that need it are halved, each at most about 52 times
//! (to 2^−52 of the domain, or to neighbouring doubles), depth first on an explicit stack of at
//! most one piece per halving, every piece examined charged to the meter.
//!
//! What the work costs: an offset is the one curve kind whose conversion decides, from its data,
//! how many times it evaluates itself, so the meter is what bounds it, and each evaluation is
//! charged what it costs, at about 285 instructions a unit. An evaluation has three parts, and
//! each has its charge:
//!
//! - **Its jets' terms.** [`TERM_WORK`] a term of each of its 3·D + 1 recurrences: one interval
//!   product and one interval sum, about 190 instructions.
//! - **Its jets themselves.** [`JET_WORK`] a recurrence whatever the order: the jets it builds,
//!   adds, scales and frees around the terms, about 1,300 to 1,400 instructions a recurrence at
//!   order 0. The sums and scalings, linear in the order, ride on the terms' margin.
//! - **What it asks of its basis.** Over a span, a closed-form bound of the basis and of each of
//!   its derivatives one order higher, each an extent a coordinate: 4,500 to 7,700 instructions
//!   an extent where no arctangent is dear, of which the basis charges one unit (and more where
//!   one is dear, `crate::extent::FAR_CORNER`) and the offset [`BOUND_WORK`]. At a point, the
//!   basis's enclosure to one order higher, about 1,100 to 1,250 instructions a coordinate an
//!   order, of which the basis charges one unit an order and the offset [`POINT_WORK`] a
//!   coordinate an order.
//!
//! [`Offset::jet_work`] and [`Offset::basis_work`] are the counts, fixed by the dimension, the
//! order and whether the evaluation is at a point or over a span, whatever the numbers are.
//!
//! Measured, in instructions of a release build (callgrind, 100 evaluations at each order 0 to 10
//! of the offset of an ellipse with no zero coefficient, issue #852). In 3D, an evaluation over a
//! span costs 62,115 at order 0, 121,699 at order 2, 225,859 at order 5 and 443,855 at order 10,
//! and is charged 180, 350, 680 and 1,430 units: 345, 348, 332 and 310 instructions a unit. At a
//! point it costs 24,827, 46,733, 92,925 and 210,920, and is charged 86, 162, 351 and 866: 289,
//! 288, 265 and 244 a unit. In 2D, 300 to 337 a unit over a span and 240 to 283 at a point.
//!
//! Before issue #765 the jets were free and a piece examined cost a unit, so a budget of
//! 2,000,000 units ran out only after 77 thousand million instructions. Issue #765 charged two
//! units a term and nothing else, which is right at the orders 9 and 10 of a conversion's pieces
//! and far too little below them: an evaluation of order 0 over a span was charged 26 units in
//! 3D, 2,389 instructions a unit, and the conversion's halving (`Offset::refined`'s first pass,
//! 75,000 such evaluations on a budget of 2,000,000 units) ran for 3.5 to 11.9 thousand million
//! instructions before the budget stopped it, the dearest past the 10 s a fuzzing campaign allows
//! an input (issue #852). At the charges above, a conversion's piece of degree 9 costs what it
//! did, within 2% (6,998 units in 3D against 7,006, 4,826 in 2D against 4,928), and the halving
//! stops within 530 to 750 million instructions where the basis's arctangents are ordinary or
//! charged.
//!
//! One cost stays under its charge (issue #853): `crate::extent` leaves unpriced the arctangents
//! just short of 2⁴⁷ apart that take the 192-bit path, and an ellipse whose coefficients fall there
//! asks them again in every evaluation. Two of issue #852's stored inputs do, three and about
//! seven an evaluation, and their halvings stop after 1,246 and 1,744 million instructions.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the basis, o the offset, n the normal, d a distance, l and v the lateral and vertical offsets, t a parameter"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::affine::Entries;
use crate::error::CurveError;
use crate::form::check_parameter;
use crate::jet::{Jet, MAX_ORDER};
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

/// The work units a term of a jet recurrence is charged (module documentation, "What the work
/// costs"): one, for the term's interval product and sum, about 190 instructions.
pub const TERM_WORK: u64 = 1;

/// The work units each jet recurrence of an evaluation is charged whatever its order (module
/// documentation, "What the work costs"): five, for the jets it builds, adds, scales and frees
/// around its terms, about 1,300 to 1,400 instructions a recurrence.
pub const JET_WORK: u64 = 5;

/// The work units an evaluation over a span is charged for each coordinate of each bound it asks
/// of its basis (module documentation, "What the work costs"): 19, which with the unit the basis
/// charges for the extent itself is 20 for 4,500 to 7,700 instructions.
pub const BOUND_WORK: u64 = 19;

/// The work units an evaluation at a point is charged for each coordinate of each order of its
/// basis's enclosure there (module documentation, "What the work costs"): four, about 1,100 to
/// 1,250 instructions.
pub const POINT_WORK: u64 = 4;

/// Where an offset is evaluated: one parameter from one side, or every parameter of a span. A
/// span lies in one station piece; where it is one station alone, the side says which of the two
/// pieces that meet there gives the offsets' slopes.
#[derive(Clone, Copy)]
enum At {
    Point(f64, Limit),
    Span(f64, f64, Limit),
}

/// What the cache's halving makes of a piece (`Offset::halved`).
#[derive(Clone, Copy)]
enum Verdict {
    /// Keep it.
    Keep,
    /// Halve it, or keep it once it is as narrow as the domain resolves.
    Finer,
    /// Halve it, or refuse with the error once it is as narrow as the domain resolves.
    FinerOr(CurveError),
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
            At::Span(lo, hi, _) => {
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
            At::Span(lo, hi, side) => (Interval::new(lo, hi)?, side),
        };
        let (first, last) = match (stations.first(), stations.last()) {
            (Some(f), Some(l)) => (*f, *l),
            _ => return Err(CurveError::Defect),
        };
        let probe = match at {
            At::Point(x, _) => x,
            At::Span(lo, hi, _) => f64::midpoint(lo, hi),
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

    /// The work units one evaluation of the offset's jets to `order` is charged: for each of its
    /// recurrences, [`TERM_WORK`] a term and [`JET_WORK`] once (module documentation, "What the
    /// work costs").
    ///
    /// The formula O = C + d·N/|N| takes 3·D + 1 products, quotients and roots of jets: D squares
    /// for |N|², its root, and for each coordinate a quotient by |N| and a product with the
    /// distance. Each is a recurrence of (order + 1)·(order + 2)/2 terms (`crate::jet`), so the
    /// count is fixed by the dimension and the order, whatever the numbers are. The sums and the
    /// scalings, linear in the order, ride on those terms.
    ///
    /// # Errors
    ///
    /// [`CurveError::Defect`] for an order whose count passes `u64`, which no caller asks: the
    /// offset's jets of an order from 170 up are refused first (the basis's go one order higher,
    /// and past 170 their factorials pass the doubles), and the count passes `u64` only from
    /// about 1.9·10⁹.
    pub fn jet_work(order: usize) -> Result<u64, CurveError> {
        let wide = |x: usize| u64::try_from(x).map_err(|_| CurveError::Defect);
        let n = wide(order)?.checked_add(1).ok_or(CurveError::Defect)?;
        let terms = n
            .checked_mul(n.checked_add(1).ok_or(CurveError::Defect)?)
            .ok_or(CurveError::Defect)?
            .checked_div(2)
            .ok_or(CurveError::Defect)?;
        let recurrences = wide(D)?
            .checked_mul(3)
            .and_then(|x| x.checked_add(1))
            .ok_or(CurveError::Defect)?;
        terms
            .checked_mul(TERM_WORK)
            .and_then(|x| x.checked_add(JET_WORK))
            .and_then(|x| x.checked_mul(recurrences))
            .ok_or(CurveError::Defect)
    }

    /// The work units one evaluation of the offset to `order` is charged for what it asks of its
    /// basis, on top of what the basis charges itself (module documentation, "What the work
    /// costs"): the basis's `order` + 2 bounds over a span (its own and its derivatives' to
    /// `order` + 1), [`BOUND_WORK`] a coordinate each, or its enclosure of as many orders at a
    /// point, [`POINT_WORK`] a coordinate an order.
    ///
    /// # Errors
    ///
    /// [`CurveError::Defect`] for an order whose count passes `u64`, which no caller asks
    /// ([`Offset::jet_work`]).
    pub fn basis_work(order: usize, over_span: bool) -> Result<u64, CurveError> {
        let wide = |x: usize| u64::try_from(x).map_err(|_| CurveError::Defect);
        let each = if over_span { BOUND_WORK } else { POINT_WORK };
        wide(order)?
            .checked_add(2)
            .and_then(|orders| orders.checked_mul(each))
            .and_then(|x| wide(D).ok().and_then(|d| x.checked_mul(d)))
            .ok_or(CurveError::Defect)
    }

    /// The offset's jets per coordinate, to `order`, at `at`.
    fn jets(
        &self,
        at: At,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Jet>, CurveError> {
        // The basis's jets go one order higher, and past `jet::MAX_ORDER` building them is
        // NotFinite whatever the basis gives: refused here as the jets refuse it, before the
        // basis is asked and before the charge, whose count would pass `u64` far beyond.
        if order >= MAX_ORDER {
            return Err(CurveError::NotFinite);
        }
        // Charged before the work, so a budget stops an evaluation it cannot pay for: the jets
        // and what they ask of the basis, in one charge.
        let basis = Self::basis_work(order, matches!(at, At::Span(..)))?;
        meter.charge(
            Self::jet_work(order)?
                .checked_add(basis)
                .ok_or(CurveError::Defect)?,
        )?;
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

    /// The span cut at the breaks inside it and into a few pieces each, for tighter jets, each
    /// with the side its stations are read from: the right. A span that is one station is read
    /// from each side the domain has there when a derivative is asked (`order` above 0): the
    /// offsets' slopes jump at a station, and the trait's rule for a span of one parameter asks
    /// for both sides inside the domain, and at an end of it for the one side there is
    /// ([`ParamCurve`]). The offsets themselves do not jump, so the points keep the one piece. A
    /// break of the basis needs no second piece: the basis's own bounds over it hold both sides.
    fn pieces(
        &self,
        (lo, hi): (f64, f64),
        order: usize,
    ) -> Result<Vec<(f64, f64, Limit)>, CurveError> {
        let (a, b) = self.basis.interval();
        if let OffsetLaw::Distances(s) = &self.law
            && order > 0
            && lo == hi
            && s.iter().any(|x| x.at == lo)
        {
            let sides = [
                (lo < b).then_some(Limit::FromRight),
                (a < lo).then_some(Limit::FromLeft),
            ];
            return Ok(sides.into_iter().flatten().map(|s| (lo, hi, s)).collect());
        }
        let c = taylor::cuts_between((lo, hi), &self.breaks(), 4)?;
        let mut out = Vec::with_capacity(c.len());
        for p in c.windows(2) {
            if let (Some(&x), Some(&y)) = (p.first(), p.get(1)) {
                out.push((x, y, Limit::FromRight));
            }
        }
        Ok(out)
    }

    /// Whether the offset's normal is certainly apart from zero over `span`, so the offset is
    /// defined there and its derivatives can be bounded: the jets of order 0 over each of the
    /// span's pieces, whose square root of |N|² refuses where it may be zero (`crate::jet`).
    fn defined_over(
        &self,
        span: (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, CurveError> {
        for (lo, hi, side) in self.pieces(span, 0)? {
            match self.derivative_enclosures(At::Span(lo, hi, side), 0, meter) {
                Ok(_) => {}
                Err(CurveError::Unbounded) => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }

    /// The cuts `coarse` (strictly increasing, the domain's ends included) with pieces halved
    /// where the cache needs it, only near where the normal comes close to zero and turns fast, so
    /// the cuts stay few (issue #526). First each piece is halved until its normal is certainly
    /// apart from zero ([`Offset::defined_over`]); then until the Taylor remainder over it
    /// (`taylor::remainder`) is bounded and at most `target`. Halving stops at the narrowest piece
    /// the domain resolves, its width times 2^−52, the spacing of doubles relative to their size;
    /// a piece there whose remainder is bounded but above `target` is kept, its deviation stated.
    /// The first pass runs over the whole domain before the second, so a normal that vanishes is
    /// found as such, not as a remainder that overflows near it.
    ///
    /// # Errors
    ///
    /// [`CurveError::Degenerate`] where the normal cannot be told from zero even over the narrowest
    /// piece, so the offset is not defined there (a cusp of the basis, or its tangent parallel to
    /// the reference direction); [`CurveError::NotFinite`] where the remainder overflows even
    /// there; [`CurveError::Stopped`]; the basis's errors.
    fn refined(
        &self,
        coarse: &[f64],
        target: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<f64>, CurveError> {
        let defined = self.halved(coarse, meter, |o, piece, m| {
            Ok(if o.defined_over(piece, m)? {
                Verdict::Keep
            } else {
                Verdict::FinerOr(CurveError::Degenerate)
            })
        })?;
        // A remainder that overflows may come from a piece whose normal pass 1 only just
        // separated from zero, its enclosure's lower end far below the normal itself: halves of
        // it bound tighter.
        self.halved(&defined, meter, |o, piece, m| {
            Ok(match taylor::remainder(o, piece, CACHE_DEGREE, m) {
                Ok(r) if r.is_some_and(|r| r.hi() > target) => Verdict::Finer,
                Ok(_) => Verdict::Keep,
                Err(CurveError::Unbounded) => Verdict::FinerOr(CurveError::Degenerate),
                Err(CurveError::NotFinite) => Verdict::FinerOr(CurveError::NotFinite),
                Err(e) => return Err(e),
            })
        })
    }

    /// The cuts `cuts` (strictly increasing) with each piece halved, depth first, as `judge`'s
    /// [`Verdict`] on it says, while it can be: while it is wider than the domain's width times
    /// 2^−52 and its middle is a double strictly inside it.
    ///
    /// Depth first with the left half on top, so the cuts come out in order. Each halving pushes
    /// two pieces and pops one, so the stack holds at most one piece per halving, and a piece no
    /// wider than the domain is as narrow as the domain resolves after about 52 halvings. Every
    /// piece examined is charged to the meter.
    ///
    /// # Errors
    ///
    /// `judge`'s, and the error of a [`Verdict::FinerOr`] on a piece that cannot be halved;
    /// [`CurveError::Stopped`].
    fn halved(
        &self,
        cuts: &[f64],
        meter: &mut WorkMeter<'_>,
        mut judge: impl FnMut(&Self, (f64, f64), &mut WorkMeter<'_>) -> Result<Verdict, CurveError>,
    ) -> Result<Vec<f64>, CurveError> {
        let (a, b) = self.domain();
        let narrowest = (b - a) * f64::EPSILON;
        let mut out: Vec<f64> = Vec::with_capacity(cuts.len());
        let mut stack: Vec<(f64, f64)> = Vec::new();
        for w in cuts.windows(2) {
            let (Some(&x), Some(&y)) = (w.first(), w.get(1)) else {
                return Err(CurveError::Defect);
            };
            if out.is_empty() {
                out.push(x);
            }
            stack.push((x, y));
            while let Some((lo, hi)) = stack.pop() {
                meter.charge(1)?;
                let mid = f64::midpoint(lo, hi);
                let halves = hi - lo > narrowest && lo < mid && mid < hi;
                match judge(self, (lo, hi), meter)? {
                    Verdict::Finer | Verdict::FinerOr(_) if halves => {
                        stack.push((mid, hi));
                        stack.push((lo, mid));
                    }
                    Verdict::FinerOr(e) => return Err(e),
                    Verdict::Keep | Verdict::Finer => out.push(hi),
                }
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
    /// The basis's breaks and the stations.
    fn breaks(&self) -> Vec<f64> {
        Offset::breaks(self)
    }

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
        for (lo, hi, side) in self.pieces(span, 0)? {
            let b = match self.derivative_enclosures(At::Span(lo, hi, side), 0, meter) {
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
        for (lo, hi, side) in self.pieces(span, order)? {
            let e = match self.derivative_enclosures(At::Span(lo, hi, side), order, meter) {
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
        let target = scale * 1e-10;
        for per in [8, 16, 32, 64, 128] {
            let cuts = taylor::cuts_between(self.domain(), &breaks, per)?;
            match taylor::from_cuts(self, CACHE_DEGREE, &cuts, meter) {
                Ok(n) if n.deviation <= target || per == 128 => return Ok(n),
                Ok(_) => {}
                // Pieces so wide that the offset's derivatives cannot be bounded on them (its
                // normal not separated from zero): narrower ones may be (piece G69).
                Err(CurveError::Unbounded) if per < 128 => {}
                // Still so at the finest even cut, where the normal comes near zero and turns
                // fast: only the pieces that need it are cut finer (issue #526). Where even the
                // narrowest cannot bound it, or a point's normal cannot be told from zero, the
                // offset is undefined there: degenerate, never unbounded, as its domain is finite.
                Err(CurveError::Unbounded) => {
                    let fine = self.refined(&cuts, target, meter)?;
                    return match taylor::from_cuts(self, CACHE_DEGREE, &fine, meter) {
                        Err(CurveError::Unbounded) => Err(CurveError::Degenerate),
                        other => other,
                    };
                }
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
        // The cache's deviation is a distance before the move: a transform stretches it by at most
        // its linear part's norm (a scale of 2^60 made it 2^60 times longer, which the fuzz
        // campaign found); a reversal keeps it.
        let (moved, stretch) = match transform {
            Some(t) => (
                ParamCurve::transformed(c, t, meter)?,
                Entries::of(t)?.stretch()?,
            ),
            None => (ParamCurve::reversed(c, meter)?, 1.0),
        };
        let cache = tight::mul(point(n.deviation)?, point(stretch)?)?;
        Ok(Changed {
            value: moved.value,
            deviation: tight::add(cache, point(moved.deviation)?)?.hi(),
        })
    }
}
