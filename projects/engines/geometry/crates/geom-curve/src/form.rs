//! The certified evaluation the analytic kinds share: every one of them is
//! C(t) = c + f₁(t)·u + f₂(t)·v + f₃(t)·w, fixed vectors times scalar functions of the parameter.
//!
//! What it does: [`Shape`] names the scalar functions (f₁, f₂, f₃) of each kind, with angles in
//! half-turns, the kernel's proven angle form (chapter 6):
//!
//! | Shape | f₁(t) | f₂(t) | f₃(t) | Kinds |
//! | --- | --- | --- | --- | --- |
//! | `Linear` | t | 0 | 0 | line, ray, infinite line |
//! | `Trig` | cos πt | sin πt | 0 | circle, arc, ellipse |
//! | `Hyperbolic` | cosh t | sinh t | 0 | hyperbola |
//! | `Parabolic` | t² | t | 0 | parabola |
//! | `Helical` | cos πt | sin πt | t | helix |
//! | `Involute` | cos πt + πt·sin πt | sin πt − πt·cos πt | 0 | involute of a circle |
//!
//! [`Form`] holds a kind's vectors c, u, v, w in doubles and as enclosures of the exact vectors its
//! stored data define, and evaluates C and its derivatives of any order: in doubles at a
//! parameter, and in intervals over any interval of parameters, which is both the certified
//! evaluation (a point interval) and the bound over a span (a wide one). The free functions are
//! the parts of the kind contract every analytic kind implements the same way.
//!
//! Guarantees:
//!
//! - **Certified.** An enclosure holds the exact value of C⁽ᵏ⁾ at every parameter of the interval:
//!   `geom-num`'s proven `sinpi`, `cospi` and `exp` enclosures, π between its two neighbouring
//!   doubles, and interval arithmetic that is exact where it can be (`tight`).
//! - **Derivatives in closed form.** The k-th derivative of each scalar function is its formula,
//!   never a finite difference (chapter 7): cos and sin of πt shift by a quarter turn per
//!   derivative and gain a factor π, cosh and sinh trade places, and the involute's terms follow
//!   d^k(θ·cos θ) = θ·cos⁽ᵏ⁾θ + k·cos⁽ᵏ⁻¹⁾θ.
//! - **Refused outside.** A parameter or span outside the domain, or not finite, is an error.
//!
//! Method: direct evaluation of the closed forms, in interval arithmetic for the enclosures
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>, the natural interval extension); a span is cut into
//! eighths of a turn for the periodic shapes, so the box follows the curve.
//!
//! Cost: a few dozen floating-point operations per evaluation and derivative order; a bound
//! evaluates one interval extension per piece, at most 64 pieces.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c, u, v, w the vectors, f the scalar functions, t a parameter, k an order"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_num::math;
use geom_nurbs::Bounds;
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::param::DerivativeBound;
use crate::tight::{self, point};

/// π rounded down: the double just below π.
pub(crate) const PI_LO: f64 = f64::from_bits(0x4009_21FB_5444_2D18);
/// π rounded up: the double just above π.
pub(crate) const PI_HI: f64 = f64::from_bits(0x4009_21FB_5444_2D19);

/// The most pieces a bound cuts a span into.
const MAX_PIECES: u32 = 64;

/// The scalar functions of a kind (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Shape {
    /// (t, 0, 0).
    Linear,
    /// (cos πt, sin πt, 0).
    Trig,
    /// (cosh t, sinh t, 0).
    Hyperbolic,
    /// (t², t, 0).
    Parabolic,
    /// (cos πt, sin πt, t).
    Helical,
    /// (cos πt + πt·sin πt, sin πt − πt·cos πt, 0).
    Involute,
}

/// A kind's vectors c, u, v, w, in doubles and enclosed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Form<const D: usize> {
    /// The scalar functions.
    pub(crate) shape: Shape,
    /// c, u, v, w in doubles.
    pub(crate) doubles: [[f64; D]; 4],
    /// Enclosures of the exact c, u, v, w the stored data define.
    pub(crate) exact: [[Interval; D]; 4],
}

/// `k` as a double (exact: orders are small).
pub(crate) fn count(k: usize) -> Result<f64, CurveError> {
    u32::try_from(k)
        .map(f64::from)
        .map_err(|_| CurveError::Defect)
}

/// π enclosed.
pub(crate) fn pi() -> Result<Interval, CurveError> {
    Ok(Interval::new(PI_LO, PI_HI)?)
}

/// π^k in doubles.
fn pi_power(k: usize) -> f64 {
    let mut p = 1.0;
    for _ in 0..k {
        p *= PI_LO;
    }
    p
}

/// π^k enclosed.
fn pi_power_enclosed(k: usize) -> Result<Interval, CurveError> {
    let pi = pi()?;
    let mut p = point(1.0)?;
    for _ in 0..k {
        p = tight::mul(p, pi)?;
    }
    Ok(p)
}

/// cos(x + k·π/2) and sin(x + k·π/2) from c = cos x and s = sin x.
fn shifted<T: Copy>(c: T, s: T, k: usize, neg: impl Fn(T) -> T) -> (T, T) {
    match k & 3 {
        0 => (c, s),
        1 => (neg(s), c),
        2 => (neg(c), neg(s)),
        _ => (s, neg(c)),
    }
}

/// The k-th derivatives of the scalar functions at `t`, in doubles.
pub(crate) fn scalars(shape: Shape, t: f64, k: usize) -> Result<[f64; 3], CurveError> {
    let f = match shape {
        Shape::Linear => match k {
            0 => [t, 0.0, 0.0],
            1 => [1.0, 0.0, 0.0],
            _ => [0.0; 3],
        },
        Shape::Parabolic => match k {
            0 => [t * t, t, 0.0],
            1 => [2.0 * t, 1.0, 0.0],
            2 => [2.0, 0.0, 0.0],
            _ => [0.0; 3],
        },
        Shape::Trig | Shape::Helical => {
            let (c, s) = (math::cospi(t)?.get(), math::sinpi(t)?.get());
            let (ck, sk) = shifted(c, s, k, |x: f64| -x);
            let p = pi_power(k);
            let lift = match (shape, k) {
                (Shape::Helical, 0) => t,
                (Shape::Helical, 1) => 1.0,
                _ => 0.0,
            };
            [p * ck, p * sk, lift]
        }
        Shape::Hyperbolic => {
            let (e, ei) = (math::exp(t)?.get(), math::exp(-t)?.get());
            let (ch, sh) = (f64::midpoint(e, ei), (e - ei) * 0.5);
            if k & 1 == 0 {
                [ch, sh, 0.0]
            } else {
                [sh, ch, 0.0]
            }
        }
        Shape::Involute => {
            let (c, s) = (math::cospi(t)?.get(), math::sinpi(t)?.get());
            let theta = PI_LO * t;
            match k.checked_sub(1) {
                None => [c + theta * s, s - theta * c, 0.0],
                Some(j) => {
                    let (c1, s1) = shifted(c, s, j, |x: f64| -x);
                    let (c2, s2) = match j.checked_sub(1) {
                        Some(i) => shifted(c, s, i, |x: f64| -x),
                        None => (0.0, 0.0),
                    };
                    let m = count(j)?;
                    let p = pi_power(k);
                    [p * (theta * c1 + m * c2), p * (theta * s1 + m * s2), 0.0]
                }
            }
        }
    };
    if f.iter().all(|x| x.is_finite()) {
        Ok(f)
    } else {
        Err(CurveError::NotFinite)
    }
}

/// The k-th derivatives of the scalar functions over `t`, enclosed.
pub(crate) fn scalars_enclosed(
    shape: Shape,
    t: Interval,
    k: usize,
) -> Result<[Interval; 3], CurveError> {
    let zero = point(0.0)?;
    let one = point(1.0)?;
    let half = point(0.5)?;
    let neg = |x: Interval| x.neg();
    Ok(match shape {
        Shape::Linear => match k {
            0 => [t, zero, zero],
            1 => [one, zero, zero],
            _ => [zero; 3],
        },
        Shape::Parabolic => match k {
            0 => [tight::square(t)?, t, zero],
            1 => [tight::mul(point(2.0)?, t)?, one, zero],
            2 => [point(2.0)?, zero, zero],
            _ => [zero; 3],
        },
        Shape::Trig | Shape::Helical => {
            let (ck, sk) = shifted(t.cospi()?, t.sinpi()?, k, neg);
            let p = pi_power_enclosed(k)?;
            let lift = match (shape, k) {
                (Shape::Helical, 0) => t,
                (Shape::Helical, 1) => one,
                _ => zero,
            };
            [tight::mul(p, ck)?, tight::mul(p, sk)?, lift]
        }
        Shape::Hyperbolic => {
            let (e, ei) = (t.exp()?, t.neg().exp()?);
            let ch = tight::mul(tight::add(e, ei)?, half)?;
            let sh = tight::mul(tight::sub(e, ei)?, half)?;
            if k & 1 == 0 {
                [ch, sh, zero]
            } else {
                [sh, ch, zero]
            }
        }
        Shape::Involute => {
            let (c, s) = (t.cospi()?, t.sinpi()?);
            let theta = tight::mul(pi()?, t)?;
            match k.checked_sub(1) {
                None => [
                    tight::add(c, tight::mul(theta, s)?)?,
                    tight::sub(s, tight::mul(theta, c)?)?,
                    zero,
                ],
                Some(j) => {
                    let (c1, s1) = shifted(c, s, j, neg);
                    let (c2, s2) = match j.checked_sub(1) {
                        Some(i) => shifted(c, s, i, neg),
                        None => (zero, zero),
                    };
                    let m = point(count(j)?)?;
                    let p = pi_power_enclosed(k)?;
                    let f1 = tight::add(tight::mul(theta, c1)?, tight::mul(m, c2)?)?;
                    let f2 = tight::add(tight::mul(theta, s1)?, tight::mul(m, s2)?)?;
                    [tight::mul(p, f1)?, tight::mul(p, f2)?, zero]
                }
            }
        }
    })
}

impl<const D: usize> Form<D> {
    /// The form with vectors c, u, v, w given as doubles, each exactly itself.
    pub(crate) fn exact_vectors(
        shape: Shape,
        doubles: [[f64; D]; 4],
    ) -> Result<Form<D>, CurveError> {
        if !doubles.iter().flatten().all(|x| x.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        let mut exact = [[point(0.0)?; D]; 4];
        for (e, d) in exact.iter_mut().zip(&doubles) {
            for (x, &y) in e.iter_mut().zip(d) {
                *x = point(y)?;
            }
        }
        Ok(Form {
            shape,
            doubles,
            exact,
        })
    }

    /// C⁽ᵏ⁾(t) in doubles.
    pub(crate) fn value(&self, t: f64, k: usize) -> Result<[f64; D], CurveError> {
        let [f1, f2, f3] = scalars(self.shape, t, k)?;
        let [c, u, v, w] = &self.doubles;
        let mut out = [0.0; D];
        for (o, (((&ci, &ui), &vi), &wi)) in out.iter_mut().zip(c.iter().zip(u).zip(v).zip(w)) {
            let base = if k == 0 { ci } else { 0.0 };
            *o = base + f1 * ui + f2 * vi + f3 * wi;
        }
        if out.iter().all(|x| x.is_finite()) {
            Ok(out)
        } else {
            Err(CurveError::NotFinite)
        }
    }

    /// C⁽ᵏ⁾ at every parameter of `t`, enclosed.
    pub(crate) fn enclose(&self, t: Interval, k: usize) -> Result<[Interval; D], CurveError> {
        let [f1, f2, f3] = scalars_enclosed(self.shape, t, k)?;
        let [c, u, v, w] = &self.exact;
        let zero = point(0.0)?;
        let mut out = [zero; D];
        for (o, (((&ci, &ui), &vi), &wi)) in out.iter_mut().zip(c.iter().zip(u).zip(v).zip(w)) {
            let base = if k == 0 { ci } else { zero };
            let s = tight::add(base, tight::mul(f1, ui)?)?;
            let s = tight::add(s, tight::mul(f2, vi)?)?;
            *o = tight::add(s, tight::mul(f3, wi)?)?;
        }
        Ok(out)
    }
}

/// Whether `t` is a finite parameter inside `domain`.
pub(crate) fn check_parameter((a, b): (f64, f64), t: f64) -> Result<(), CurveError> {
    if t.is_finite() && a <= t && t <= b {
        Ok(())
    } else {
        Err(CurveError::ParameterOutside)
    }
}

/// A finite, ordered domain with room inside it.
pub(crate) fn check_domain((a, b): (f64, f64)) -> Result<(), CurveError> {
    if a.is_finite() && b.is_finite() && a < b {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

/// A finite, ordered domain of at most one turn (2 half-turns), decided exactly.
pub(crate) fn check_turn((a, b): (f64, f64)) -> Result<(), CurveError> {
    check_domain((a, b))?;
    let (s, e) = geom_num::eft::two_sum(b, -a);
    if s < 2.0 || (s == 2.0 && e <= 0.0) {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

/// Two domains: the halves of a split.
pub(crate) type Halves = ((f64, f64), (f64, f64));

/// The two halves of `domain` at `t`, strictly inside it.
pub(crate) fn split_domain((a, b): (f64, f64), t: f64) -> Result<Halves, CurveError> {
    if a < t && t < b {
        Ok(((a, t), (t, b)))
    } else {
        Err(CurveError::ParameterOutside)
    }
}

/// The point at `t`.
pub(crate) fn point_at<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    t: f64,
) -> Result<[f64; D], CurveError> {
    check_parameter(domain, t)?;
    form.value(t, 0)
}

/// The point and derivatives at `t` into `out`.
pub(crate) fn derivatives_at<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    t: f64,
    out: &mut [[f64; D]],
) -> Result<(), CurveError> {
    check_parameter(domain, t)?;
    for (k, o) in out.iter_mut().enumerate() {
        *o = form.value(t, k)?;
    }
    Ok(())
}

/// The point and derivatives up to `order` at `t`, enclosed.
pub(crate) fn enclose_at<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    t: f64,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<[Interval; D]>, CurveError> {
    check_parameter(domain, t)?;
    meter.charge(
        u64::try_from(order)
            .map_err(|_| CurveError::Defect)?
            .saturating_add(1),
    )?;
    let at = point(t)?;
    let mut out = Vec::with_capacity(order.saturating_add(1));
    for k in 0..=order {
        out.push(form.enclose(at, k)?);
    }
    Ok(out)
}

/// The hull of C⁽ᵏ⁾ over `span`, cut into pieces for the periodic shapes.
fn hull<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    (lo, hi): (f64, f64),
    k: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, CurveError> {
    crate::nurbs::check_span((lo, hi), domain)?;
    let pieces = match form.shape {
        Shape::Trig | Shape::Helical | Shape::Involute => {
            // Eighths of a turn: a quarter of a half-turn each.
            let n = math::ceil((hi - lo) * 4.0)?.get();
            if n >= f64::from(MAX_PIECES) {
                MAX_PIECES
            } else if n >= 1.0 {
                // Below MAX_PIECES and whole, so exact in u32.
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "n is a whole number in [1, 64)"
                )]
                let m = n as u32;
                m
            } else {
                1
            }
        }
        _ => 1,
    };
    meter.charge(u64::from(pieces))?;
    let width = hi - lo;
    let n = f64::from(pieces);
    let mut out = Bounds {
        lo: [f64::INFINITY; D],
        hi: [f64::NEG_INFINITY; D],
    };
    let mut start = lo;
    for i in 1..=pieces {
        let end = if i == pieces {
            hi
        } else {
            lo + width * (f64::from(i) / n)
        };
        // Rounding may step past either end; the piece stays in [start, hi].
        let end = tight::larger(tight::smaller(end, hi), start);
        let e = form.enclose(Interval::new(start, end)?, k)?;
        for ((l, h), x) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(&e) {
            *l = tight::smaller(*l, x.lo());
            *h = tight::larger(*h, x.hi());
        }
        start = end;
    }
    Ok(out)
}

/// A box holding the curve over `span`.
pub(crate) fn bound<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, CurveError> {
    hull(form, domain, span, 0, meter)
}

/// A bound on the derivative of `order` over `span`.
pub(crate) fn derivative_bound<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    span: (f64, f64),
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<DerivativeBound<D>, CurveError> {
    Ok(DerivativeBound::Bounded(hull(
        form, domain, span, order, meter,
    )?))
}

/// Upper bounds on |f₁|, |f₂| and |f₃| over the finite `domain`.
pub(crate) fn sup_scalars(shape: Shape, (a, b): (f64, f64)) -> Result<[f64; 3], CurveError> {
    let f = scalars_enclosed(shape, Interval::new(a, b)?, 0)?;
    Ok(f.map(tight::magnitude))
}

/// An upper bound on the distance between two curves of one shape over the finite `domain`, from
/// enclosures of their vectors: |Δc| + sup|f₁|·|Δu| + sup|f₂|·|Δv| + sup|f₃|·|Δw|. Zero when the
/// enclosures are the same exact points.
pub(crate) fn distance<const D: usize>(
    shape: Shape,
    domain: (f64, f64),
    a: &[[Interval; D]; 4],
    b: &[[Interval; D]; 4],
) -> Result<f64, CurveError> {
    let [s1, s2, s3] = sup_scalars(shape, domain)?;
    let [ac, au, av, aw] = a;
    let [bc, bu, bv, bw] = b;
    let mut total = point(tight::distance(ac, bc)?)?;
    for (s, (x, y)) in [s1, s2, s3].iter().zip([(au, bu), (av, bv), (aw, bw)]) {
        let d = tight::distance(x, y)?;
        if d != 0.0 {
            total = tight::add(total, tight::mul(point(*s)?, point(d)?)?)?;
        }
    }
    Ok(total.hi())
}
