//! Class-A maths (16.5): how two surfaces meet along a join, measured from each surface's own
//! partial derivatives at a parameter of the join or over a span of it.
//!
//! What it does: a join is two [`Side`]s, each a surface, a curve in its parameter space (a
//! pcurve) and whether the face's normal is turned against `S_u` × `S_v`; the two pcurves share
//! one parameter t, as an edge's two fins do. At t each side has its point r = S(p(t)), its unit
//! normal N, its unit tangent e₁ along r′ = `S_u`·u′ + `S_v`·v′ and its across direction
//! e₂ = N × e₁. [`Measure`] names what is compared, chapter 16.5's table:
//!
//! - [`Measure::Gap`] (G0): |`r_a` − `r_b`|, in the coordinates' length.
//! - [`Measure::NormalAngle`] (G1): the angle between `N_a` and `N_b`,
//!   atan2(|`N_a` × `N_b`|, `N_a` · `N_b`), in radians.
//! - [`Measure::Curvature`] (G2): each side's normal curvature along cos φ·e₁ + sin φ·e₂ is
//!   k₁₁·cos²φ + 2·k₁₂·sin φ·cos φ + k₂₂·sin²φ, kᵢⱼ = N · `S_*`(pᵢ, pⱼ) its second fundamental
//!   form on the frame (pᵢ the parameter direction that maps to eᵢ); the measure is the largest
//!   difference over every φ, the spectral norm of the two matrices' difference Δ,
//!   |Δ₁₁ + Δ₂₂|/2 + √(((Δ₁₁ − Δ₂₂)/2)² + Δ₁₂²), in 1/length.
//! - [`Measure::CurvatureChange`] (G3): |`κ′_a` − `κ′_b`|, κ′ the derivative by arc length along
//!   e₂ of the curvature of the normal section along e₂ (the curve the plane through r spanned by
//!   N and e₂ cuts from the surface), in 1/length².
//!
//! Where the normals agree (a G1 join) both sides share e₁ and e₂, so G2 and G3 compare like with
//! like; where they do not, each side is read in its own frame. [`at`] encloses the measure's
//! smooth parts ([`Parts`]) at one t, and [`Parts::value`] the measure itself. [`over`] encloses
//! them at every t of a span: from the surfaces' partials over the box the pcurve's bound gives,
//! and, given the parts at a parameter c of the span, by the mean value theorem as well,
//! P(c) + P′(T)·(T − c), with P′ from the same formulas run on dual numbers; the two enclosures
//! are met. The centred form's width shrinks with the square of the span's, so a caller that
//! splits a join's parameter range (`geom-analysis`) reaches a tight maximum in few pieces.
//!
//! Guarantees:
//!
//! - **Certified.** Every enclosure holds the exact measure at every t it covers, for pcurves that
//!   lie in their surfaces' domains (a valid body's fins, chapter 8.3): the surfaces' partials and
//!   the pcurves' values and derivatives are the kinds' own enclosures, every formula is evaluated
//!   in outward-rounded interval arithmetic (inclusion isotonicity), the derivative of every
//!   smooth part is the derivative of the formula (`geom_num::dual`, chain rule through the
//!   pcurve), and a pcurve's box is cut to its surface's domain, never widened.
//! - **Centred only where smooth.** The mean value form needs each part continuous over the span,
//!   so it is used only where no break of a pcurve lies strictly inside the span and no break of
//!   a surface lies in the pcurve's box; elsewhere the enclosure is the direct one. A pcurve that
//!   runs along a surface's break is read from both sides of it.
//! - **Undefined is `Unbounded`.** Where a normal may vanish (a pole, an apex, a degenerate
//!   point), a tangent may vanish, or a part's derivative cannot be bounded, the result is
//!   [`CurveError::Unbounded`], never a guess; a caller splits further or reports the span.
//! - **Deterministic.** A fixed sequence of operations on the kinds' enclosures.
//!
//! Method: the normal curvature from the second fundamental form, and the shape operator's
//! extreme normal curvatures (M. P. do Carmo, *Differential Geometry of Curves and Surfaces*, 2nd
//! ed., Dover (2016), sections 3-2 and 3-3); the normal section's height solved order by order in
//! the section's arc length, κ = 2·h₂ and κ′ = 6·h₃, from solidcore's `normal_section` (below);
//! the mean value form and centred forms of interval analysis (R. E. Moore, R. B. Kearfott and
//! M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009), section 6.4,
//! <https://doi.org/10.1137/1.9780898717716>) with derivatives by forward-mode automatic
//! differentiation (A. Griewank and A. Walther, *Evaluating Derivatives*, 2nd ed., SIAM (2008),
//! chapter 3, <https://doi.org/10.1137/1.9780898717761>).
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/db9f4ae1850b9bb9faabed46ca86390c242dedd3/crates/solidcore/src/eval/normal_section.rs>
//!   <https://github.com/tangency-labs/solidcore/blob/db9f4ae1850b9bb9faabed46ca86390c242dedd3/crates/solidcore/src/query/continuity.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: `normal_section`'s order-by-order series for the section's height is
//!   kept to its third order and written in closed form, κ′ = N · (S₃(p₂) − 3·S₂(p₂, q)), with
//!   p₂ the parameter direction of e₂, q the one whose image is S₂(p₂, p₂)'s tangential part, S₂
//!   and S₃ the second and third parameter forms, each solve by the first fundamental form instead
//!   of solidcore's 2 × 2 system against the section's two directions; `continuity_at`'s
//!   measures at one parameter are kept (its gap, its normal angle, its cross-boundary curvature
//!   and change), but each point comes from the face's pcurve instead of a dropped foot, the
//!   curvature compares the whole second fundamental form over every direction rather than the
//!   cross-boundary section alone, and everything is generic over the number type so it runs in
//!   interval arithmetic, with derivatives in dual numbers for a centred form over a span, in place
//!   of solidcore's unbounded doubles. Its G4 and its knife-edge flip are not taken.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: [`at`] is one enclosure of each pcurve to order 1 and one of each surface's partials to
//! the measure's order (0 to 3) over a small box, then O(1) interval operations; [`over`] is one
//! bound of each pcurve and its first two derivatives and one of each surface's partials to one
//! order higher, then O(1) operations on dual numbers. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the formulas' names: r a point, n a normal, e and f directions, p and q parameter directions, s a partial"
)]

use alloc::vec::Vec;

use geom_curve::{CurveError, CurveKind, DerivativeBound, ParamCurve, tight};
use geom_num::NumError;
use geom_num::dual::Dual;
use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_nurbs::Limit;
use geom_run::WorkMeter;

use crate::param::{ParamBox, ParamSurface, SurfaceKind, centred, offset_from, partial_index};

/// What a join's continuity is measured by (16.5's table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Measure {
    /// G0: the distance between the two sides' points.
    Gap,
    /// G1: the angle between the two sides' unit normals, in radians.
    NormalAngle,
    /// G2: the largest difference in normal curvature over every tangent direction.
    Curvature,
    /// G3: the difference in the derivative of normal curvature across the join.
    CurvatureChange,
}

impl Measure {
    /// The highest order of the surfaces' partials the measure reads (0 for the gap to 3 for the
    /// curvature change).
    #[must_use]
    pub const fn order(self) -> usize {
        match self {
            Measure::Gap => 0,
            Measure::NormalAngle => 1,
            Measure::Curvature => 2,
            Measure::CurvatureChange => 3,
        }
    }
}

/// One side of a join: a face's surface, its fin's pcurve, and which way the face's normal points.
#[derive(Clone, Copy, Debug)]
pub struct Side<'a> {
    /// The surface.
    pub surface: &'a SurfaceKind,
    /// The curve in the surface's parameter space, on the join's parameter.
    pub pcurve: &'a CurveKind<2>,
    /// Whether the face's normal is −(`S_u` × `S_v`) (a face whose sense is reversed).
    pub turned: bool,
}

/// Enclosures of a measure's smooth parts, from which [`Parts::value`] encloses the measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parts {
    measure: Measure,
    parts: [Interval; 4],
}

impl Parts {
    /// The measure.
    #[must_use]
    pub const fn measure(&self) -> Measure {
        self.measure
    }

    /// The smooth parts: the gap vector (G0); `N_a` × `N_b` and `N_a` · `N_b` (G1); the
    /// differences Δ₁₁, Δ₁₂ and Δ₂₂ of the second fundamental forms (G2); the difference of κ′
    /// (G3). Unused places are 0.
    #[must_use]
    pub const fn parts(&self) -> [Interval; 4] {
        self.parts
    }

    /// An enclosure of the measure: never negative, and for the angle never above π.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if a bound overflows.
    pub fn value(&self) -> Result<Interval, CurveError> {
        let [a, b, c, d] = self.parts;
        match self.measure {
            Measure::Gap => tight::norm(&[a, b, c]),
            Measure::NormalAngle => {
                // y ≥ 0, so the angle lies in [0, π]: the lower end is cut to 0.
                let angle = tight::norm(&[a, b, c])?.atan2(d)?;
                Ok(Interval::new(tight::larger(angle.lo(), 0.0), angle.hi())?)
            }
            Measure::Curvature => {
                let half = tight::point(0.5)?;
                let mean = tight::mul(tight::add(a, c)?, half)?;
                let skew = tight::mul(tight::sub(a, c)?, half)?;
                // A sum of squares is never negative, though its widened lower end may be.
                let squares = tight::add(tight::square(skew)?, tight::square(b)?)?;
                let squares = Interval::new(tight::larger(squares.lo(), 0.0), squares.hi())?;
                let root = tight::sqrt(squares)?;
                tight::add(magnitude(mean)?, root)
            }
            Measure::CurvatureChange => magnitude(a),
        }
    }
}

/// The interval of |x| over x.
fn magnitude(x: Interval) -> Result<Interval, CurveError> {
    if x.lo() >= 0.0 {
        Ok(x)
    } else if x.hi() <= 0.0 {
        Ok(x.neg())
    } else {
        Ok(Interval::new(0.0, tight::larger(x.lo().abs(), x.hi()))?)
    }
}

/// Encloses `measure`'s parts across the join `sides` at its parameter `t`, each pcurve and surface
/// read from `limit` at a break.
///
/// # Errors
///
/// [`CurveError::Unbounded`] where a normal or the tangent may vanish at `t`;
/// [`CurveError::ParameterOutside`] for a `t` outside a pcurve's domain; [`CurveError::SpanOutside`]
/// for a pcurve point outside its surface's domain; [`CurveError::NotFinite`];
/// [`CurveError::Stopped`].
pub fn at(
    measure: Measure,
    sides: [Side<'_>; 2],
    t: f64,
    limit: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Parts, CurveError> {
    let [a, b] = sides;
    let la = local_at(measure, a, t, limit, meter)?;
    let lb = local_at(measure, b, t, limit, meter)?;
    Ok(Parts {
        measure,
        parts: parts(measure, &la, &lb).map_err(undefined)?,
    })
}

/// Encloses `measure`'s parts across the join `sides` at every parameter of `span`; with `centre`,
/// the parts at a parameter c of the span (from [`at`]), also by the mean value theorem, and the
/// two enclosures met.
///
/// # Errors
///
/// [`CurveError::Unbounded`] where a normal or the tangent may vanish over the span, or a
/// pcurve's derivative cannot be bounded there; [`CurveError::SpanOutside`] for a span outside a
/// pcurve's domain, or a pcurve box outside its surface's domain; [`CurveError::ParameterOutside`]
/// for a centre outside the span; [`CurveError::Defect`] for a centre of another measure;
/// [`CurveError::NotFinite`]; [`CurveError::Stopped`].
pub fn over(
    measure: Measure,
    sides: [Side<'_>; 2],
    span: (f64, f64),
    centre: Option<(f64, &Parts)>,
    meter: &mut WorkMeter<'_>,
) -> Result<Parts, CurveError> {
    let (lo, hi) = span;
    if (lo..=hi).is_empty() {
        return Err(CurveError::SpanOutside);
    }
    let [a, b] = sides;
    let (na, da, smooth_a) = local_over(measure, a, span, meter)?;
    let (nb, db, smooth_b) = local_over(measure, b, span, meter)?;
    let direct = parts(measure, &na, &nb).map_err(undefined)?;
    let Some((c, at_c)) = centre else {
        return Ok(Parts {
            measure,
            parts: direct,
        });
    };
    if !(lo..=hi).contains(&c) {
        return Err(CurveError::ParameterOutside);
    }
    if at_c.measure != measure {
        return Err(CurveError::Defect);
    }
    if !(smooth_a && smooth_b) {
        return Ok(Parts {
            measure,
            parts: direct,
        });
    }
    let (Some(da), Some(db)) = (da, db) else {
        return Ok(Parts {
            measure,
            parts: direct,
        });
    };
    let slopes = parts(measure, &da, &db).map_err(undefined)?;
    let step = offset_from(span, c)?;
    let mut out = direct;
    for ((o, s), p) in out.iter_mut().zip(slopes).zip(at_c.parts) {
        let slope = s.partials();
        let [slope] = slope;
        *o = tight::meet(*o, tight::add(p, tight::mul(slope, step)?)?)?;
    }
    Ok(Parts {
        measure,
        parts: out,
    })
}

/// A number error as the curve error the callers expect: a quotient or root that may meet zero
/// is a measure that is not defined there.
fn undefined(e: NumError) -> CurveError {
    match e {
        NumError::DivisorContainsZero | NumError::NegativeSqrt | NumError::OutsideDomain => {
            CurveError::Unbounded
        }
        _ => CurveError::NotFinite,
    }
}

/// A 3D vector of scalars.
type V<T> = [T; 3];

/// One side's partials to order 3 (zero above the measure's order), its pcurve's derivative and
/// its turn, in a scalar `T`.
#[derive(Clone, Copy, Debug)]
struct Local<T> {
    s: V<T>,
    su: V<T>,
    sv: V<T>,
    suu: V<T>,
    suv: V<T>,
    svv: V<T>,
    suuu: V<T>,
    suuv: V<T>,
    suvv: V<T>,
    svvv: V<T>,
    w: [T; 2],
    turned: bool,
}

impl<T: Scalar> Local<T> {
    /// The side from `partials` listed by [`partial_index`] (those past their end are 0), the
    /// pcurve's derivative `w` and the turn.
    fn new(partials: &[V<T>], w: [T; 2], turned: bool) -> Local<T> {
        let zero = T::constant(Real::ZERO);
        let at = |k: usize| partials.get(k).copied().unwrap_or([zero; 3]);
        Local {
            s: at(0),
            su: at(1),
            sv: at(2),
            suu: at(3),
            suv: at(4),
            svv: at(5),
            suuu: at(6),
            suuv: at(7),
            suvv: at(8),
            svvv: at(9),
            w,
            turned,
        }
    }
}

/// `measure`'s parts across the join from its sides `a` and `b`.
fn parts<T: Scalar>(measure: Measure, a: &Local<T>, b: &Local<T>) -> Result<[T; 4], NumError> {
    let zero = T::constant(Real::ZERO);
    match measure {
        Measure::Gap => {
            let [x, y, z] = sub(&a.s, &b.s)?;
            Ok([x, y, z, zero])
        }
        Measure::NormalAngle => {
            let (na, nb) = (normal(a)?.0, normal(b)?.0);
            let [x, y, z] = cross(&na, &nb)?;
            Ok([x, y, z, dot(&na, &nb)?])
        }
        Measure::Curvature => {
            let (fa, fb) = (Frame::of(a)?, Frame::of(b)?);
            let ([a11, a12, a22], [b11, b12, b22]) = (fa.form(a)?, fb.form(b)?);
            Ok([a11.sub(b11)?, a12.sub(b12)?, a22.sub(b22)?, zero])
        }
        Measure::CurvatureChange => {
            let (fa, fb) = (Frame::of(a)?, Frame::of(b)?);
            Ok([fa.change(a)?.sub(fb.change(b)?)?, zero, zero, zero])
        }
    }
}

/// The unit normal of a side, turned where it says, and |`S_u` × `S_v`|².
fn normal<T: Scalar>(l: &Local<T>) -> Result<(V<T>, T), NumError> {
    let n = cross(&l.su, &l.sv)?;
    let d = dot(&n, &n)?;
    let len = d.sqrt()?;
    let len = if l.turned { len.neg() } else { len };
    let [x, y, z] = n;
    Ok(([x.div(len)?, y.div(len)?, z.div(len)?], d))
}

/// A side's frame at a point: its unit normal, the parameter directions p₁ and p₂ whose images are
/// the unit tangent e₁ and the across direction e₂ = N × e₁, and |`S_u` × `S_v`|².
struct Frame<T> {
    n: V<T>,
    p1: [T; 2],
    p2: [T; 2],
    d: T,
}

impl<T: Scalar> Frame<T> {
    /// The frame of side `l`.
    fn of(l: &Local<T>) -> Result<Frame<T>, NumError> {
        let (n, d) = normal(l)?;
        let [a, b] = l.w;
        let tangent = add(&scale(&l.su, a)?, &scale(&l.sv, b)?)?;
        let speed = dot(&tangent, &tangent)?.sqrt()?;
        let [x, y, z] = tangent;
        let e1 = [x.div(speed)?, y.div(speed)?, z.div(speed)?];
        let p1 = [a.div(speed)?, b.div(speed)?];
        let e2 = cross(&n, &e1)?;
        let p2 = solve(l, d, &e2)?;
        Ok(Frame { n, p1, p2, d })
    }

    /// The second fundamental form on the frame: k₁₁, k₁₂ and k₂₂.
    fn form(&self, l: &Local<T>) -> Result<[T; 3], NumError> {
        Ok([
            dot(&self.n, &second(l, self.p1, self.p1)?)?,
            dot(&self.n, &second(l, self.p1, self.p2)?)?,
            dot(&self.n, &second(l, self.p2, self.p2)?)?,
        ])
    }

    /// κ′ along e₂: 6·h₃ of the normal section, N · (S₃(p₂) − 3·S₂(p₂, q)), q the parameter
    /// direction whose image is the tangential part of S₂(p₂, p₂). The section is u(x), v(x) with
    /// (r − r₀) · e₂ = x and (r − r₀) · e₁ = 0; its first coefficients are p₂, its second are
    /// −q/2 (the tangential part of its second-order term cancelled), and the third-order normal
    /// height is N · (S₃(p₂)/6 + S₂(p₂, −q/2)).
    fn change(&self, l: &Local<T>) -> Result<T, NumError> {
        let bend = second(l, self.p2, self.p2)?;
        let q = solve(l, self.d, &bend)?;
        let mixed = second(l, self.p2, q)?;
        let three = T::constant(Real::new(3.0)?);
        let cubic = third(l, self.p2)?;
        dot(&self.n, &sub(&cubic, &scale(&mixed, three)?)?)
    }
}

/// The parameter direction (x, y) with `S_u`·x + `S_v`·y the tangential part of `v`: the first
/// fundamental form's system [E F; F G]·(x, y) = (`S_u` · v, `S_v` · v), by Cramer's rule with
/// EG − F² = `d` = |`S_u` × `S_v`|² (Lagrange's identity).
fn solve<T: Scalar>(l: &Local<T>, d: T, v: &V<T>) -> Result<[T; 2], NumError> {
    let (e, f, g) = (dot(&l.su, &l.su)?, dot(&l.su, &l.sv)?, dot(&l.sv, &l.sv)?);
    let (x, y) = (dot(&l.su, v)?, dot(&l.sv, v)?);
    Ok([
        g.mul(x)?.sub(f.mul(y)?)?.div(d)?,
        e.mul(y)?.sub(f.mul(x)?)?.div(d)?,
    ])
}

/// The second parameter form S₂(p, q) = `S_uu`·p₀q₀ + `S_uv`·(p₀q₁ + p₁q₀) + `S_vv`·p₁q₁.
fn second<T: Scalar>(l: &Local<T>, p: [T; 2], q: [T; 2]) -> Result<V<T>, NumError> {
    let [p0, p1] = p;
    let [q0, q1] = q;
    let uu = scale(&l.suu, p0.mul(q0)?)?;
    let uv = scale(&l.suv, p0.mul(q1)?.add(p1.mul(q0)?)?)?;
    let vv = scale(&l.svv, p1.mul(q1)?)?;
    add(&add(&uu, &uv)?, &vv)
}

/// The third parameter form S₃(p) = `S_uuu`·p₀³ + 3·`S_uuv`·p₀²p₁ + 3·`S_uvv`·p₀p₁² + `S_vvv`·p₁³.
fn third<T: Scalar>(l: &Local<T>, p: [T; 2]) -> Result<V<T>, NumError> {
    let [p0, p1] = p;
    let three = T::constant(Real::new(3.0)?);
    let (a2, b2) = (p0.square()?, p1.square()?);
    let uuu = scale(&l.suuu, a2.mul(p0)?)?;
    let uuv = scale(&l.suuv, three.mul(a2)?.mul(p1)?)?;
    let uvv = scale(&l.suvv, three.mul(p0)?.mul(b2)?)?;
    let vvv = scale(&l.svvv, b2.mul(p1)?)?;
    add(&add(&uuu, &uuv)?, &add(&uvv, &vvv)?)
}

/// a + b.
fn add<T: Scalar>(a: &V<T>, b: &V<T>) -> Result<V<T>, NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (*a, *b);
    Ok([a0.add(b0)?, a1.add(b1)?, a2.add(b2)?])
}

/// a − b.
fn sub<T: Scalar>(a: &V<T>, b: &V<T>) -> Result<V<T>, NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (*a, *b);
    Ok([a0.sub(b0)?, a1.sub(b1)?, a2.sub(b2)?])
}

/// a·s.
fn scale<T: Scalar>(a: &V<T>, s: T) -> Result<V<T>, NumError> {
    let [a0, a1, a2] = *a;
    Ok([a0.mul(s)?, a1.mul(s)?, a2.mul(s)?])
}

/// a · b.
fn dot<T: Scalar>(a: &V<T>, b: &V<T>) -> Result<T, NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (*a, *b);
    a0.mul(b0)?.add(a1.mul(b1)?)?.add(a2.mul(b2)?)
}

/// a × b.
fn cross<T: Scalar>(a: &V<T>, b: &V<T>) -> Result<V<T>, NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (*a, *b);
    Ok([
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ])
}

/// Side `side` at `t`: its pcurve's point and derivative enclosed there, its surface's partials
/// to the measure's order over the point's box.
fn local_at(
    measure: Measure,
    side: Side<'_>,
    t: f64,
    limit: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Local<Interval>, CurveError> {
    let jet = side.pcurve.enclose(t, limit, 1, meter)?;
    let (Some(&[u, v]), Some(&w)) = (jet.first(), jet.get(1)) else {
        return Err(CurveError::Defect);
    };
    let region = inside(((u.lo(), u.hi()), (v.lo(), v.hi())), side.surface.domain())?;
    let p = conditioned(side.surface, region, measure.order(), limit, meter)?;
    Ok(Local::new(&p, w, side.turned))
}

/// One side over a span: [`local_over`]'s three answers.
type Spanned = (Local<Interval>, Option<Local<Dual<Interval, 1>>>, bool);

/// Side `side` over `span`: its surface's partials to the measure's order over the pcurve's box,
/// with its pcurve's derivative there; the same as dual numbers carrying their derivatives along
/// t (`None` where the pcurve's derivatives cannot be bounded); and whether the parts are smooth
/// over the span (no pcurve break strictly inside it, no surface break in the box).
fn local_over(
    measure: Measure,
    side: Side<'_>,
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Spanned, CurveError> {
    let order = measure.order();
    let up = order.checked_add(1).ok_or(CurveError::Defect)?;
    let bound = side.pcurve.bound(span, meter)?;
    let ([u0, v0], [u1, v1]) = (bound.lo, bound.hi);
    let region = inside(((u0, u1), (v0, v1)), side.surface.domain())?;
    let p = conditioned(side.surface, region, up, Limit::FromRight, meter)?;
    let w = derivative(side.pcurve, span, 1, meter)?;
    let smooth = smooth(side, span, region);
    let Some(w) = w else {
        if order >= 2 {
            return Err(CurveError::Unbounded);
        }
        let zero = Interval::point(0.0)?;
        return Ok((Local::new(&p, [zero; 2], side.turned), None, smooth));
    };
    let direct = Local::new(&p, w, side.turned);
    // A part's derivative along t needs the pcurve's second derivative only through the tangent.
    let w2 = if order >= 2 {
        derivative(side.pcurve, span, 2, meter)?
    } else {
        Some([Interval::point(0.0)?; 2])
    };
    let Some(w2) = w2 else {
        return Ok((direct, None, smooth));
    };
    let mut duals: Vec<V<Dual<Interval, 1>>> = Vec::new();
    for n in 0..=order {
        for j in 0..=n {
            let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
            let value = got(&p, i, j)?;
            let (du, dv) = (
                got(&p, i.checked_add(1).ok_or(CurveError::Defect)?, j)?,
                got(&p, i, j.checked_add(1).ok_or(CurveError::Defect)?)?,
            );
            let [w0, w1] = w;
            let mut d = [Dual::new(Interval::point(0.0)?, [Interval::point(0.0)?]); 3];
            for (k, slot) in d.iter_mut().enumerate() {
                let (Some(&x), Some(&a), Some(&b)) = (value.get(k), du.get(k), dv.get(k)) else {
                    return Err(CurveError::Defect);
                };
                let slope = tight::add(tight::mul(a, w0)?, tight::mul(b, w1)?)?;
                *slot = Dual::new(x, [slope]);
            }
            duals.push(d);
        }
    }
    let [w0, w1] = w;
    let [a0, a1] = w2;
    let dual = Local::new(
        &duals,
        [Dual::new(w0, [a0]), Dual::new(w1, [a1])],
        side.turned,
    );
    Ok((direct, Some(dual), smooth))
}

/// Partial (i, j) of a list by [`partial_index`].
fn got(p: &[V<Interval>], i: usize, j: usize) -> Result<V<Interval>, CurveError> {
    p.get(partial_index(i, j)?)
        .copied()
        .ok_or(CurveError::Defect)
}

/// The box of `pcurve`'s derivative of `order` over `span`, `None` where it is unbounded.
fn derivative(
    pcurve: &CurveKind<2>,
    span: (f64, f64),
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[Interval; 2]>, CurveError> {
    match pcurve.derivative_bound(span, order, meter)? {
        DerivativeBound::Bounded(b) => {
            let ([u0, v0], [u1, v1]) = (b.lo, b.hi);
            Ok(Some([Interval::new(u0, u1)?, Interval::new(v0, v1)?]))
        }
        DerivativeBound::Unbounded => Ok(None),
    }
}

/// Whether a side's parts are smooth over `span`: no pcurve break strictly inside it and no
/// surface break in its box `region`, ends included.
fn smooth(side: Side<'_>, span: (f64, f64), region: ParamBox) -> bool {
    let (lo, hi) = span;
    if side.pcurve.breaks().iter().any(|&b| lo < b && b < hi) {
        return false;
    }
    let (bu, bv) = side.surface.breaks();
    let ((u0, u1), (v0, v1)) = region;
    !(bu.iter().any(|&b| u0 <= b && b <= u1) || bv.iter().any(|&b| v0 <= b && b <= v1))
}

/// `region` cut to `domain`: a valid pcurve lies in its surface's domain, so the cut keeps every
/// point it reaches.
fn inside(region: ParamBox, domain: ParamBox) -> Result<ParamBox, CurveError> {
    let cut = |(lo, hi): (f64, f64), (a, b): (f64, f64)| {
        let (l, h) = (tight::larger(lo, a), tight::smaller(hi, b));
        if l <= h {
            Ok((l, h))
        } else {
            Err(CurveError::SpanOutside)
        }
    };
    Ok((cut(region.0, domain.0)?, cut(region.1, domain.1)?))
}

/// How much of the smooth piece of the domain around a box the box for the higher partials spans
/// at least, in each finite direction: 1/128.
const REACH: f64 = 1.0 / 128.0;

/// The surface's partials to `order` over `region`, well conditioned however small the region: a
/// region with no break of the surface in it, and not a point, is enclosed by the mean value form
/// about its centre, P(c) + `P_u`(W)·Δu + `P_v`(W)·Δv, with P(c) at the centre point and the
/// partials one order higher over W, the region widened to at least [`REACH`] of the smooth piece
/// of the domain around it (but no further than that piece). A box's partials found by cutting a
/// patch down to the box lose a factor of the box's width to rounding for each order, so the
/// partials straight over a box of 2⁻³⁰ are far wider than the values; over W they are not, and
/// the steps Δ keep the result as narrow as the region. Any other region is [`partials`]'s.
fn conditioned(
    surface: &SurfaceKind,
    region: ParamBox,
    order: usize,
    limit: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<V<Interval>>, CurveError> {
    let ((u0, u1), (v0, v1)) = region;
    let (bu, bv) = surface.breaks();
    let clear = |(lo, hi): (f64, f64), breaks: &[f64]| !breaks.iter().any(|&b| lo <= b && b <= hi);
    if (u0 == u1 && v0 == v1) || !(clear(region.0, &bu) && clear(region.1, &bv)) {
        return partials(surface, region, order, limit, meter);
    }
    let (du, dv) = surface.domain();
    let (cu, cv) = (f64::midpoint(u0, u1), f64::midpoint(v0, v1));
    let wide = (widened(region.0, &bu, du), widened(region.1, &bv, dv));
    let up = order.checked_add(1).ok_or(CurveError::Defect)?;
    let at = surface.partials_over(((cu, cu), (cv, cv)), (limit, limit), order, meter)?;
    let over = surface.partials_over(wide, (limit, limit), up, meter)?;
    let steps = (offset_from(region.0, cu)?, offset_from(region.1, cv)?);
    centred(&at, &over, steps, order)
}

/// `side` of a box widened about its centre to at least [`REACH`] of the smooth piece of `domain`
/// around it (between the nearest `breaks` or the domain's ends), and cut to that piece; as it
/// is where that piece is not finite.
fn widened((lo, hi): (f64, f64), breaks: &[f64], (a, b): (f64, f64)) -> (f64, f64) {
    let below = breaks
        .iter()
        .copied()
        .filter(|&x| x < lo)
        .fold(a, tight::larger);
    let above = breaks
        .iter()
        .copied()
        .filter(|&x| x > hi)
        .fold(b, tight::smaller);
    let piece = above - below;
    if !piece.is_finite() {
        return (lo, hi);
    }
    let c = f64::midpoint(lo, hi);
    let half = tight::larger(0.5 * (hi - lo), 0.5 * REACH * piece);
    // The box itself is kept whatever the rounding of c ± half.
    (
        tight::larger(tight::smaller(c - half, lo), below),
        tight::smaller(tight::larger(c + half, hi), above),
    )
}

/// The surface's partials to `order` over `region`, read from `limit` at a break; a side of the
/// box that is one parameter on a break (a pcurve along a break line) is read from both sides,
/// and the two enclosures joined; one on an end of the domain from the side inside it.
fn partials(
    surface: &SurfaceKind,
    region: ParamBox,
    order: usize,
    limit: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<V<Interval>>, CurveError> {
    let (bu, bv) = surface.breaks();
    let (du, dv) = surface.domain();
    let ways = |(lo, hi): (f64, f64), breaks: &[f64], (a, b): (f64, f64)| -> &'static [Limit] {
        if lo != hi {
            &[Limit::FromRight]
        } else if breaks.contains(&lo) {
            &[Limit::FromLeft, Limit::FromRight]
        } else if lo == b && a < b {
            &[Limit::FromLeft]
        } else if lo == a {
            &[Limit::FromRight]
        } else if matches!(limit, Limit::FromLeft) {
            &[Limit::FromLeft]
        } else {
            &[Limit::FromRight]
        }
    };
    let mut out: Option<Vec<V<Interval>>> = None;
    for &su in ways(region.0, &bu, du) {
        for &sv in ways(region.1, &bv, dv) {
            let p = surface.partials_over(region, (su, sv), order, meter)?;
            out = Some(match out {
                None => p,
                Some(q) => join(&q, &p)?,
            });
        }
    }
    out.ok_or(CurveError::Defect)
}

/// The boxes holding both lists' boxes, place by place.
fn join(a: &[V<Interval>], b: &[V<Interval>]) -> Result<Vec<V<Interval>>, CurveError> {
    let mut out = Vec::new();
    for (x, y) in a.iter().zip(b) {
        let mut o = *x;
        for (s, t) in o.iter_mut().zip(y) {
            *s = Interval::new(
                tight::smaller(s.lo(), t.lo()),
                tight::larger(s.hi(), t.hi()),
            )?;
        }
        out.push(o);
    }
    Ok(out)
}
