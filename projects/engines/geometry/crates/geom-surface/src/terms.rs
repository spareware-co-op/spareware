//! The certified evaluation the analytic surface kinds share: each is S(u, v) = c + Σ f(u)·g(v)·W,
//! fixed vectors W times products of scalar functions of one parameter each.
//!
//! What it does: [`Fun`] names the scalar functions (1, t, cos πt, sin πt; angles in half-turns,
//! the kernel's proven angle form), and [`Terms`] holds a kind's c and terms in doubles and as
//! enclosures of the exact values its stored data define. It evaluates every partial derivative
//! ∂^{i+j}S/∂u^i∂v^j in closed form: in doubles at a point, and in intervals over any box, which
//! is both the certified evaluation and the bound. [`Terms::distance`] bounds how far two surfaces
//! of the same terms lie apart over a box, and [`Terms::to_nurbs`] writes them as one
//! tensor-product NURBS surface: each direction's functions are the coordinates of a unit circle
//! (cos, sin and 1, the exact rational quadratic of `geom-curve`) or of a line (t and 1), and a
//! product of two such curves is a tensor product (The NURBS Book, section 8.2, on surfaces of
//! revolution, the same construction).
//!
//! Guarantees:
//!
//! - **Certified.** An enclosure holds the exact value at every point of its box: `geom-num`'s
//!   proven `sinpi` and `cospi` enclosures, π between its neighbouring doubles, and `geom-curve`'s
//!   exact-where-it-can interval arithmetic.
//! - **Derivatives in closed form.** A derivative of cos πt or sin πt shifts it a quarter turn and
//!   gains π; never a finite difference (chapter 7).
//! - **Conversion with its error.** The NURBS surface differs from the terms by at most the
//!   direction curves' own deviations, multiplied out over the terms, plus the rounding of its
//!   control points and weights (the rational bound of `geom-curve`'s conics).
//!
//! Method: direct evaluation of the closed forms in interval arithmetic (R. E. Moore,
//! R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>); tensor-product rational surfaces from rational
//! curves (L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., chapter 8,
//! <https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: O(T) interval operations per evaluation of T terms; a conversion O(m·n·T) for an m × n
//! control net.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the centre, f and g the scalar functions, w a term's vector, u and v the parameters, i and j orders"
)]

use alloc::vec::Vec;

use geom_curve::tight::{self, point};
use geom_curve::{Circle, CurveError, Frame, ParamCurve, ParamMap};
use geom_num::interval::Interval;
use geom_num::math;
use geom_nurbs::{Basis, Changed, Surface};
use geom_run::WorkMeter;

use crate::param::{ParamBox, SurfaceForm};

/// π rounded down.
const PI_LO: f64 = f64::from_bits(0x4009_21FB_5444_2D18);
/// π rounded up.
const PI_HI: f64 = f64::from_bits(0x4009_21FB_5444_2D19);

/// A scalar function of one parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Fun {
    /// 1.
    One,
    /// t.
    Lin,
    /// cos πt.
    Cos,
    /// sin πt.
    Sin,
}

/// One term f(u)·g(v)·W.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Term {
    pub(crate) u: Fun,
    pub(crate) v: Fun,
    pub(crate) w: [f64; 3],
    pub(crate) we: [Interval; 3],
}

/// A kind's c and terms (module documentation).
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::struct_field_names,
    reason = "the form's own words: c and its terms"
)]
pub(crate) struct Terms {
    pub(crate) c: [f64; 3],
    pub(crate) ce: [Interval; 3],
    pub(crate) terms: Vec<Term>,
}

/// The cos/sin pair shifted by k quarter turns: cos(x + kπ/2), sin(x + kπ/2).
fn shifted<T: Copy>(c: T, s: T, k: usize, neg: impl Fn(T) -> T) -> (T, T) {
    match k & 3 {
        0 => (c, s),
        1 => (neg(s), c),
        2 => (neg(c), neg(s)),
        _ => (s, neg(c)),
    }
}

/// f⁽ᵏ⁾(t) in doubles.
fn value(f: Fun, t: f64, k: usize) -> Result<f64, CurveError> {
    Ok(match (f, k) {
        (Fun::One, 0) | (Fun::Lin, 1) => 1.0,
        (Fun::Lin, 0) => t,
        (Fun::One | Fun::Lin, _) => 0.0,
        (Fun::Cos | Fun::Sin, _) => {
            let (c, s) = (math::cospi(t)?.get(), math::sinpi(t)?.get());
            let (ck, sk) = shifted(c, s, k, |x: f64| -x);
            let mut p = 1.0;
            for _ in 0..k {
                p *= PI_LO;
            }
            p * if f == Fun::Cos { ck } else { sk }
        }
    })
}

/// f⁽ᵏ⁾ over t, enclosed.
fn enclosed(f: Fun, t: Interval, k: usize) -> Result<Interval, CurveError> {
    Ok(match (f, k) {
        (Fun::One, 0) | (Fun::Lin, 1) => point(1.0)?,
        (Fun::Lin, 0) => t,
        (Fun::One | Fun::Lin, _) => point(0.0)?,
        (Fun::Cos | Fun::Sin, _) => {
            let (ck, sk) = shifted(t.cospi()?, t.sinpi()?, k, |x: Interval| x.neg());
            let pi = Interval::new(PI_LO, PI_HI)?;
            let mut p = point(1.0)?;
            for _ in 0..k {
                p = tight::mul(p, pi)?;
            }
            tight::mul(p, if f == Fun::Cos { ck } else { sk })?
        }
    })
}

impl Term {
    /// The term f(u)·g(v)·W, its W given as doubles that are its exact value.
    pub(crate) fn exact(u: Fun, v: Fun, w: [f64; 3]) -> Result<Term, CurveError> {
        Ok(Term {
            u,
            v,
            w,
            we: [point(w[0])?, point(w[1])?, point(w[2])?],
        })
    }

    /// The term with W = s·x for a length s and an enclosed unit vector x (doubles xd).
    pub(crate) fn scaled(
        u: Fun,
        v: Fun,
        s: f64,
        xd: [f64; 3],
        xe: [Interval; 3],
    ) -> Result<Term, CurveError> {
        let l = point(s)?;
        let [a, b, c] = xe;
        Ok(Term {
            u,
            v,
            w: xd.map(|x| s * x),
            we: [tight::mul(l, a)?, tight::mul(l, b)?, tight::mul(l, c)?],
        })
    }
}

impl Terms {
    /// ∂^{i+j}S/∂u^i∂v^j at (u, v), in doubles.
    pub(crate) fn value(
        &self,
        u: f64,
        v: f64,
        (i, j): (usize, usize),
    ) -> Result<[f64; 3], CurveError> {
        let mut out = if i == 0 && j == 0 { self.c } else { [0.0; 3] };
        for t in &self.terms {
            let f = value(t.u, u, i)? * value(t.v, v, j)?;
            for (o, &w) in out.iter_mut().zip(&t.w) {
                *o += f * w;
            }
        }
        if out.iter().all(|x| x.is_finite()) {
            Ok(out)
        } else {
            Err(CurveError::NotFinite)
        }
    }

    /// ∂^{i+j}S/∂u^i∂v^j at every point of the box (u, v), enclosed.
    pub(crate) fn enclose(
        &self,
        u: Interval,
        v: Interval,
        (i, j): (usize, usize),
    ) -> Result<[Interval; 3], CurveError> {
        let zero = point(0.0)?;
        let mut out = if i == 0 && j == 0 { self.ce } else { [zero; 3] };
        for t in &self.terms {
            let f = tight::mul(enclosed(t.u, u, i)?, enclosed(t.v, v, j)?)?;
            if f.lo() == 0.0 && f.hi() == 0.0 {
                continue;
            }
            for (o, &w) in out.iter_mut().zip(&t.we) {
                *o = tight::add(*o, tight::mul(f, w)?)?;
            }
        }
        Ok(out)
    }

    /// An upper bound on the distance between these terms and `other`'s (the same functions,
    /// term by term) over the box: |Δc| + Σ sup|f|·sup|g|·|ΔW|.
    pub(crate) fn distance(
        &self,
        other: &Terms,
        ((a, b), (c, d)): ParamBox,
    ) -> Result<f64, CurveError> {
        if self.terms.len() != other.terms.len() {
            return Err(CurveError::Defect);
        }
        let (u, v) = (Interval::new(a, b)?, Interval::new(c, d)?);
        let mut total = point(tight::distance(&self.ce, &other.ce)?)?;
        for (s, o) in self.terms.iter().zip(&other.terms) {
            if s.u != o.u || s.v != o.v {
                return Err(CurveError::Defect);
            }
            let dw = tight::distance(&s.we, &o.we)?;
            if dw == 0.0 {
                continue;
            }
            let f = tight::magnitude(enclosed(s.u, u, 0)?) * tight::magnitude(enclosed(s.v, v, 0)?);
            total = tight::add(total, tight::mul(point(f)?, point(dw)?)?)?;
        }
        Ok(total.hi())
    }

    /// The terms as one tensor-product NURBS surface over the finite box, with the maps from the
    /// kind's parameters to its own (module documentation).
    pub(crate) fn to_nurbs(
        &self,
        ((a, b), (c, d)): ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceForm>, CurveError> {
        let du = Direction1::of(self.terms.iter().map(|t| t.u), (a, b), meter)?;
        let dv = Direction1::of(self.terms.iter().map(|t| t.v), (c, d), meter)?;
        let (m, n) = (du.count(), dv.count());
        let mut homs: Vec<[f64; 3]> = Vec::with_capacity(m.saturating_mul(n));
        let mut weights: Vec<f64> = Vec::with_capacity(m.saturating_mul(n));
        let mut moved = 0.0;
        let mut eps = 0.0;
        let mut reach = 0.0;
        let one = point(1.0)?;
        let mut origin: Option<[Interval; 3]> = None;
        // geom-nurbs stores the net v-fastest: index i·n + j for u-index i, v-index j.
        for i in 0..m {
            for j in 0..n {
                let mut p = self.ce;
                for t in &self.terms {
                    let f = tight::mul(du.coefficient(t.u, i)?, dv.coefficient(t.v, j)?)?;
                    for (o, &w) in p.iter_mut().zip(&t.we) {
                        *o = tight::add(*o, tight::mul(f, w)?)?;
                    }
                }
                let w = tight::mul(du.weight(i)?, dv.weight(j)?)?;
                let (pd, wd) = (p.map(tight::middle), tight::middle(w));
                let d = tight::distance_to(&pd, &p)?;
                if d > moved {
                    moved = d;
                }
                let r = tight::magnitude(tight::sub(tight::div(point(wd)?, w)?, one)?);
                if r > eps {
                    eps = r;
                }
                let o = *origin.get_or_insert(p);
                let rr = tight::distance(&p, &o)?;
                if rr > reach {
                    reach = rr;
                }
                homs.push(pd.map(|x| x * wd));
                weights.push(wd);
            }
        }
        // The stored net against the exact one (`geom-curve`'s rational bound), plus the direction
        // curves' own deviations multiplied out over the terms.
        let mut dev = point(moved)?;
        if eps > 0.0 {
            if eps.is_nan() || eps >= 0.5 {
                return Err(CurveError::NotFinite);
            }
            let e = point(eps)?;
            let factor = tight::div(tight::mul(point(2.0)?, e)?, tight::sub(one, e)?)?;
            dev = tight::add(dev, tight::mul(factor, point(reach)?)?)?;
        }
        if du.deviation != 0.0 || dv.deviation != 0.0 {
            let (uu, vv) = (Interval::new(a, b)?, Interval::new(c, d)?);
            for t in &self.terms {
                let (fu, fv) = (
                    tight::magnitude(enclosed(t.u, uu, 0)?),
                    tight::magnitude(enclosed(t.v, vv, 0)?),
                );
                let (eu, ev) = (point(du.deviation)?, point(dv.deviation)?);
                let e = tight::add(
                    tight::add(tight::mul(eu, point(fv)?)?, tight::mul(point(fu)?, ev)?)?,
                    tight::mul(eu, ev)?,
                )?;
                let w = tight::norm(&t.we)?;
                dev = tight::add(dev, tight::mul(e, w)?)?;
            }
        }
        // Homogeneous points were stored as w·P; geom-nurbs takes them so with the weights.
        let surface =
            Surface::from_homogeneous(du.basis.clone(), dv.basis.clone(), homs, Some(weights))?;
        Ok(Changed {
            value: SurfaceForm {
                surface,
                u: du.map,
                v: dv.map,
            },
            deviation: dev.hi(),
        })
    }
}

/// One direction's curve for a conversion: a unit circle (cos, sin) when the direction has a
/// trigonometric function, else the line t; its basis, control coefficients, weights, map and
/// deviation.
struct Direction1 {
    basis: Basis,
    /// Per control point: (cos, sin) or (t), and its weight.
    coeffs: Vec<([f64; 2], f64)>,
    trig: bool,
    map: ParamMap,
    deviation: f64,
}

impl Direction1 {
    fn of(
        funs: impl Iterator<Item = Fun>,
        (a, b): (f64, f64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Direction1, CurveError> {
        let mut trig = false;
        for f in funs {
            if matches!(f, Fun::Cos | Fun::Sin) {
                trig = true;
            }
        }
        if !trig {
            return Ok(Direction1 {
                basis: Basis::new(1, alloc::vec![a, a, b, b])?,
                coeffs: alloc::vec![([a, 0.0], 1.0), ([b, 0.0], 1.0)],
                trig,
                map: ParamMap::Same,
                deviation: 0.0,
            });
        }
        let circle = Circle::<2>::new([0.0, 0.0], 1.0, Frame::standard()?, (a, b))?;
        let n = circle.to_nurbs(meter)?;
        let c = &n.value.curve;
        let w: Vec<f64> = c
            .weights()
            .map_or_else(|| alloc::vec![1.0; c.homogeneous().len()], <[f64]>::to_vec);
        let mut coeffs = Vec::with_capacity(w.len());
        for (h, &wi) in c.homogeneous().iter().zip(&w) {
            let [x, y] = *h;
            coeffs.push(([x / wi, y / wi], wi));
        }
        Ok(Direction1 {
            basis: c.basis().clone(),
            coeffs,
            trig,
            map: n.value.map,
            deviation: n.deviation,
        })
    }

    fn count(&self) -> usize {
        self.coeffs.len()
    }

    /// The control coefficient of `f` at control point i, enclosed: the stored Euclidean
    /// coordinates are exact values of the stored curve.
    fn coefficient(&self, f: Fun, i: usize) -> Result<Interval, CurveError> {
        let (xy, _) = self.coeffs.get(i).ok_or(CurveError::Defect)?;
        let [x, y] = *xy;
        match (f, self.trig) {
            (Fun::One, _) => point(1.0),
            (Fun::Cos, true) | (Fun::Lin, false) => point(x),
            (Fun::Sin, true) => point(y),
            _ => Err(CurveError::Defect),
        }
    }

    fn weight(&self, i: usize) -> Result<Interval, CurveError> {
        point(self.coeffs.get(i).ok_or(CurveError::Defect)?.1)
    }
}
