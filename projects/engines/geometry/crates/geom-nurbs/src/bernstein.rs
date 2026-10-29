//! Bernstein forms: the Bezier pieces of curves and surfaces as interval enclosures of their exact
//! Bernstein coefficients, and the bounds the convex hull property gives them: boxes, derivative
//! boxes from hodographs, slabs, fat lines and fat planes, and the parameter range a Bezier
//! clipping step keeps.
//!
//! What it does: [`Curve::bernstein`] and [`Surface::bernstein`] run Bezier extraction (A5.6,
//! A5.7) in `geom-num`'s interval arithmetic, so each piece's coefficients enclose the exact
//! Bernstein coefficients of the exact curve or surface the stored doubles define. On a piece,
//! [`BernsteinCurve::bound`] and [`BernsteinPatch::bound`] give a box holding every point of it;
//! [`BernsteinCurve::hodograph`] and [`BernsteinPatch::partial`] give the derivative as a
//! Bernstein form of one degree less (nonrational); [`BernsteinCurve::derivative_bound`] and
//! [`BernsteinPatch::derivative_bound`] bound the derivative of a rational piece too, through the
//! quotient rule's numerator and the weights' range. [`BernsteinCurve::slab`] and
//! [`BernsteinPatch::slab`] bound the signed distance n·(x − o) over a piece, for any direction n;
//! [`BernsteinCurve::fat_line`] (in the plane) and [`BernsteinPatch::fat_plane`] choose n from the
//! piece's end points or corners, as Sederberg and Nishita's fat line does. [`BernsteinCurve::clip`]
//! is a Bezier clipping step: the part of a curve's domain where it can meet a slab.
//!
//! Guarantees:
//!
//! - **Enclosures of the exact.** Every coefficient is an interval holding the exact Bernstein
//!   coefficient: extraction makes the same operations as in doubles, each rounded outward
//!   (`crate::arith`). Every bound below is computed from these intervals with outward rounding,
//!   so it holds for the exact curve or surface, not only for its doubles.
//! - **The convex hull property.** A Bezier piece lies in the convex hull of its control points
//!   (for a rational piece, of its Euclidean points, since the weights are positive), so a box,
//!   a slab or any linear function's range over the coefficients bounds the piece (The NURBS
//!   Book, properties P3.4 and P4.4; Farouki and Rajan, *Computer Aided Geometric Design* 5(1)
//!   (1988), 1–26). A derivative is bounded from the hodograph, the derivative's own Bernstein
//!   form; a rational derivative C′ = (A′·W − A·W′)/W² from its numerator's coefficients and the
//!   smallest and largest weights, W lying between them.
//! - **Clipping keeps every meeting.** [`BernsteinCurve::clip`] returns a parameter range outside
//!   which the exact curve is outside the slab, or `None` when the whole curve is: the hull of the
//!   control polygon of the distance to each side of the slab, computed with each coefficient at
//!   its upper bound and every crossing rounded outward.
//! - **Typed refusals.** A piece needs a domain of more than one value, coefficients that are
//!   finite, and positive weights ([`NurbsError::Weight`]); hodographs and products of rational
//!   pieces are [`NurbsError::Rational`] (bound their numerators instead); a fat line or plane of a
//!   piece whose end points or corners give no direction is [`NurbsError::Degenerate`].
//! - **Deterministic.** Fixed loops in index order; no fused multiply-add.
//!
//! Method: Bezier extraction (The NURBS Book, 2nd ed., algorithms A5.6 and A5.7,
//! <https://doi.org/10.1007/978-3-642-59223-2>) in interval arithmetic, the convex hull property
//! and hodographs (sections 1.4 and 3.3), and Bezier clipping with fat lines (T. W. Sederberg and
//! T. Nishita, "Curve intersection using Bézier clipping", *Computer-Aided Design* 22(9) (1990),
//! 538–549, <https://doi.org/10.1016/0010-4485(90)90039-F>), its hull taken over every pair of
//! control points so it needs no convex hull routine.
//!
//! Cost: extraction O(s·p²·D) in intervals for s spans (O(`s_u`·`s_v`·(p + q)·p·q·D) for a surface);
//! a box, slab or hodograph is linear in the coefficients; a clip O(p²); each result one
//! allocation, charged first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: p and q for degrees, a and b for a domain, n and o for a slab's normal and origin, w for weights"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::Num;
use crate::bezier::decompose;
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, mul, real, sub};
use crate::ops::{Direction, charge_op, homs, larger, smaller};
use crate::surface::Surface;

/// A box: every point it bounds has coordinate d between `lo[d]` and `hi[d]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds<const D: usize> {
    /// The smallest value of each coordinate.
    pub lo: [f64; D],
    /// The largest value of each coordinate.
    pub hi: [f64; D],
}

/// A slab: the points x with `lo` ≤ `normal`·(x − `origin`) ≤ `hi`. In the plane, a fat line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab<const D: usize> {
    /// A point the distances are measured from.
    pub origin: [f64; D],
    /// The direction the distances are measured along; not necessarily of unit length.
    pub normal: [f64; D],
    /// The smallest signed distance, in units of |`normal`|.
    pub lo: f64,
    /// The largest signed distance, in units of |`normal`|.
    pub hi: f64,
}

/// One Bezier piece of a curve: interval enclosures of its Bernstein coefficients over its
/// domain, the weighted points w·P and the weights w if it is rational.
#[derive(Clone, Debug, PartialEq)]
pub struct BernsteinCurve<const D: usize> {
    domain: (f64, f64),
    points: Vec<[Interval; D]>,
    weights: Option<Vec<Interval>>,
}

/// One Bezier patch of a surface: interval enclosures of its Bernstein coefficients, row by row
/// (u index first), the weighted points w·P and the weights w if it is rational.
#[derive(Clone, Debug, PartialEq)]
pub struct BernsteinPatch<const D: usize> {
    u: (f64, f64),
    v: (f64, f64),
    degrees: (usize, usize),
    points: Vec<[Interval; D]>,
    weights: Option<Vec<Interval>>,
}

/// `x` as an interval.
pub(crate) fn iv(x: f64) -> Result<Interval, NurbsError> {
    Interval::of(x)
}

/// A domain of more than one value, both ends finite.
fn check_domain((a, b): (f64, f64)) -> Result<(), NurbsError> {
    if a.is_finite() && b.is_finite() && a < b {
        Ok(())
    } else {
        Err(NurbsError::EmptyDomain)
    }
}

/// Weights whose lower bounds are all positive.
fn check_weights(weights: Option<&[Interval]>, count: usize) -> Result<(), NurbsError> {
    if let Some(w) = weights {
        if w.len() != count {
            return Err(NurbsError::PointCount {
                points: w.len(),
                expected: count,
            });
        }
        if let Some(index) = w
            .iter()
            .position(|x| x.lo().partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater))
        {
            return Err(NurbsError::Weight { index });
        }
    }
    Ok(())
}

/// The Euclidean points of homogeneous `points` with `weights`: w·P / w, enclosed.
fn euclidean<const D: usize>(
    meter: &mut WorkMeter<'_>,
    points: &[[Interval; D]],
    weights: Option<&[Interval]>,
) -> Result<Vec<[Interval; D]>, NurbsError> {
    let mut out = vec_for(meter, points.len())?;
    match weights {
        None => out.extend_from_slice(points),
        Some(w) => {
            for (p, &wi) in points.iter().zip(w) {
                let mut e = *p;
                for x in &mut e {
                    *x = x.over(wi)?;
                }
                out.push(e);
            }
        }
    }
    Ok(out)
}

/// The box of interval points.
fn hull<const D: usize>(points: &[[Interval; D]]) -> Result<Bounds<D>, NurbsError> {
    let mut lo = [f64::INFINITY; D];
    let mut hi = [f64::NEG_INFINITY; D];
    for p in points {
        for ((l, h), x) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
            *l = smaller(*l, x.lo());
            *h = larger(*h, x.hi());
        }
    }
    if lo.iter().chain(&hi).all(|x| x.is_finite()) {
        Ok(Bounds { lo, hi })
    } else {
        Err(NurbsError::Defect)
    }
}

/// The range [lo, hi] of the interval values `x`, at least one.
fn range(mut x: impl Iterator<Item = Interval>) -> Result<(f64, f64), NurbsError> {
    let first = x.next().ok_or(NurbsError::Defect)?;
    Ok(x.fold((first.lo(), first.hi()), |(lo, hi), v| {
        (smaller(lo, v.lo()), larger(hi, v.hi()))
    }))
}

/// n·(x − o) for an enclosed point x.
fn distance<const D: usize>(
    n: [f64; D],
    o: [f64; D],
    x: [Interval; D],
) -> Result<Interval, NurbsError> {
    let mut s = iv(0.0)?;
    for ((&nd, &od), xd) in n.iter().zip(&o).zip(&x) {
        s = s.plus(iv(nd)?.times(xd.minus(iv(od)?)?)?)?;
    }
    Ok(s)
}

/// p / (b − a): the factor of a derivative of degree p on [a, b].
fn chain_factor(p: usize, (a, b): (f64, f64)) -> Result<Interval, NurbsError> {
    iv(real(p)?)?.over(iv(b)?.minus(iv(a)?)?)
}

/// A derivative bound: the numerator's box divided by W², W in [`w_lo`, `w_hi`] with `w_lo` > 0.
fn quotient_bound<const D: usize>(
    numerator: Bounds<D>,
    (w_lo, w_hi): (f64, f64),
) -> Result<Bounds<D>, NurbsError> {
    let w = Interval::new(w_lo, w_hi).map_err(|_| NurbsError::Defect)?;
    let w2 = w.times(w)?;
    let mut out = numerator;
    for (l, h) in out.lo.iter_mut().zip(out.hi.iter_mut()) {
        let x = Interval::new(*l, *h)
            .map_err(|_| NurbsError::Defect)?
            .over(w2)?;
        *l = x.lo();
        *h = x.hi();
    }
    Ok(out)
}

impl<const D: usize> BernsteinCurve<D> {
    /// The piece on `domain` with these Bernstein coefficients (the weighted points w·P if
    /// `weights` is given), of degree one less than their number.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] for no coefficients, or weights of another number;
    /// [`NurbsError::EmptyDomain`] unless the domain's ends are finite and increase;
    /// [`NurbsError::Weight`] for a weight not certainly positive.
    pub fn new(
        domain: (f64, f64),
        points: Vec<[Interval; D]>,
        weights: Option<Vec<Interval>>,
    ) -> Result<Self, NurbsError> {
        if points.is_empty() {
            return Err(NurbsError::PointCount {
                points: 0,
                expected: 1,
            });
        }
        check_domain(domain)?;
        check_weights(weights.as_deref(), points.len())?;
        Ok(BernsteinCurve {
            domain,
            points,
            weights,
        })
    }

    /// The degree.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.points.len().saturating_sub(1)
    }

    /// The domain [a, b].
    #[must_use]
    pub fn domain(&self) -> (f64, f64) {
        self.domain
    }

    /// The Bernstein coefficients: the weighted points w·P of a rational piece.
    #[must_use]
    pub fn points(&self) -> &[[Interval; D]] {
        &self.points
    }

    /// The weights, if the piece is rational.
    #[must_use]
    pub fn weights(&self) -> Option<&[Interval]> {
        self.weights.as_deref()
    }

    /// Whether the piece is rational.
    #[must_use]
    pub fn is_rational(&self) -> bool {
        self.weights.is_some()
    }

    /// The weighted points as a nonrational piece: the numerator A of C = A / W.
    #[must_use]
    pub fn numerator(&self) -> BernsteinCurve<D> {
        BernsteinCurve {
            domain: self.domain,
            points: self.points.clone(),
            weights: None,
        }
    }

    /// The weight function W as a nonrational scalar piece; `None` for a nonrational piece.
    #[must_use]
    pub fn weight_function(&self) -> Option<BernsteinCurve<1>> {
        self.weights.as_ref().map(|w| BernsteinCurve {
            domain: self.domain,
            points: w.iter().map(|&x| [x]).collect(),
            weights: None,
        })
    }

    /// A box holding every point of the piece (the hull of its Euclidean control points).
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`] if an enclosure overflows; [`NurbsError::Stopped`].
    pub fn bound(&self, meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, NurbsError> {
        hull(&euclidean(meter, &self.points, self.weights.as_deref())?)
    }

    /// The derivative of a nonrational piece, of degree p − 1 (degree 0 and zero for p = 0):
    /// p / (b − a) times the differences of neighbouring coefficients.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Rational`] for a rational piece; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn hodograph(&self, meter: &mut WorkMeter<'_>) -> Result<BernsteinCurve<D>, NurbsError> {
        if self.is_rational() {
            return Err(NurbsError::Rational);
        }
        let p = self.degree();
        let zero = iv(0.0)?;
        let mut out = vec_for(meter, p.max(1))?;
        if p == 0 {
            out.push([zero; D]);
        } else {
            let f = chain_factor(p, self.domain)?;
            for pair in self.points.windows(2) {
                let [a, b] = pair else {
                    return Err(NurbsError::Defect);
                };
                let mut d = [zero; D];
                for (x, (&bx, &ax)) in d.iter_mut().zip(b.iter().zip(a)) {
                    *x = f.times(bx.minus(ax)?)?;
                }
                out.push(d);
            }
        }
        Ok(BernsteinCurve {
            domain: self.domain,
            points: out,
            weights: None,
        })
    }

    /// A box holding the first derivative at every parameter of the piece: the hodograph's hull,
    /// or for a rational piece the hull of the numerator A′·W − A·W′ divided by W², W between the
    /// smallest and largest weight.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn derivative_bound(&self, meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, NurbsError> {
        let Some(w) = self.weight_function() else {
            return self.hodograph(meter)?.bound(meter);
        };
        let a = self.numerator();
        let da = a.hodograph(meter)?;
        let dw = w.hodograph(meter)?;
        let first = da.scaled_by(&w, meter)?;
        let second = a.scaled_by(&dw, meter)?;
        let numerator = first.minus(&second, meter)?;
        let weights = range(self.weights.iter().flatten().copied())?;
        quotient_bound(numerator.bound(meter)?, weights)
    }

    /// The range of n·(x − o) over every point x of the piece.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`] if a direction or point is not finite or a distance overflows;
    /// [`NurbsError::Stopped`].
    pub fn slab(
        &self,
        normal: [f64; D],
        origin: [f64; D],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Slab<D>, NurbsError> {
        let pts = euclidean(meter, &self.points, self.weights.as_deref())?;
        let mut ds = vec_for(meter, pts.len())?;
        for p in &pts {
            ds.push(distance(normal, origin, *p)?);
        }
        let (lo, hi) = range(ds.into_iter())?;
        Ok(Slab {
            origin,
            normal,
            lo,
            hi,
        })
    }

    /// The parameter range of the piece's domain outside which the piece lies outside `slab`, or
    /// `None` if it lies outside the slab everywhere (a Bezier clipping step).
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn clip(
        &self,
        slab: &Slab<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<(f64, f64)>, NurbsError> {
        let p = self.degree();
        // Distances to each side, weighted: w·(n·(x − o) − lo) = n·(w·x − w·o) − w·lo ≥ 0 and
        // w·hi − n·(w·x − w·o) ≥ 0, each a Bernstein polynomial with these coefficients.
        let mut above = vec_for(meter, add(p, 1)?)?;
        let mut below = vec_for(meter, add(p, 1)?)?;
        for (k, x) in self.points.iter().enumerate() {
            let w = match &self.weights {
                Some(ws) => get(ws, k)?,
                None => iv(1.0)?,
            };
            let mut s = iv(0.0)?;
            for ((&nd, &od), xd) in slab.normal.iter().zip(&slab.origin).zip(x) {
                s = s.plus(iv(nd)?.times(xd.minus(w.times(iv(od)?)?)?)?)?;
            }
            above.push(s.minus(w.times(iv(slab.lo)?)?)?.hi());
            below.push(w.times(iv(slab.hi)?)?.minus(s)?.hi());
        }
        let (Some(a), Some(b)) = (nonnegative_range(&above)?, nonnegative_range(&below)?) else {
            return Ok(None);
        };
        let (t0, t1) = (larger(a.0, b.0), smaller(a.1, b.1));
        if t0 > t1 {
            return Ok(None);
        }
        let (lo, hi) = self.domain;
        let width = iv(hi)?.minus(iv(lo)?)?;
        let start = iv(lo)?.plus(iv(t0)?.times(width)?)?.lo();
        let end = iv(lo)?.plus(iv(t1)?.times(width)?)?.hi();
        // Outward rounding may step past the domain; the clip never does.
        Ok(Some((start.clamp(lo, hi), end.clamp(lo, hi))))
    }

    /// `self` − `other`, two nonrational pieces of one degree and domain.
    pub(crate) fn minus(
        &self,
        other: &BernsteinCurve<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinCurve<D>, NurbsError> {
        if self.degree() != other.degree() || self.domain != other.domain {
            return Err(NurbsError::Defect);
        }
        let mut out = vec_for(meter, self.points.len())?;
        for (a, b) in self.points.iter().zip(&other.points) {
            let mut d = *a;
            for (x, y) in d.iter_mut().zip(b) {
                *x = x.minus(*y)?;
            }
            out.push(d);
        }
        Ok(BernsteinCurve {
            domain: self.domain,
            points: out,
            weights: None,
        })
    }
}

/// The range [t0, t1] ⊂ [0, 1] outside which the Bernstein polynomial with coefficients at most
/// `c` (at t = i/p) is negative: the projection onto t of the part of its control polygon's hull
/// with value ≥ 0, from every coefficient ≥ 0 and every crossing of zero between two of them,
/// rounded outward. `None` if every coefficient is negative. A constant (degree 0) has one
/// coefficient, standing for all of [0, 1].
fn nonnegative_range(c: &[f64]) -> Result<Option<(f64, f64)>, NurbsError> {
    let p = sub(c.len(), 1)?;
    let at = |i: usize| -> Result<Interval, NurbsError> {
        if p == 0 {
            Ok(Interval::new(0.0, 1.0)?)
        } else {
            iv(real(i)?)?.over(iv(real(p)?)?)
        }
    };
    let mut found: Option<(f64, f64)> = None;
    let mut take = |t: Interval| {
        found = Some(match found {
            None => (t.lo(), t.hi()),
            Some((t0, t1)) => (smaller(t0, t.lo()), larger(t1, t.hi())),
        });
    };
    for (i, &ci) in c.iter().enumerate() {
        if ci >= 0.0 {
            take(at(i)?);
        }
        for (j, &cj) in c.iter().enumerate() {
            if ci < 0.0 && cj >= 0.0 {
                // Where the segment from (t_i, c_i) to (t_j, c_j) crosses zero.
                let (ti, tj) = (at(i)?, at(j)?);
                let frac = iv(-ci)?.over(iv(cj)?.minus(iv(ci)?)?)?;
                take(ti.plus(frac.times(tj.minus(ti)?)?)?);
            }
        }
    }
    // Outward rounding may step past [0, 1]; the polynomial lives on it.
    Ok(found.map(|(t0, t1)| (t0.clamp(0.0, 1.0), t1.clamp(0.0, 1.0))))
}

impl BernsteinCurve<2> {
    /// The fat line of a plane piece (Sederberg and Nishita): the slab whose direction is
    /// perpendicular to the chord from its first to its last control point (from its first two if
    /// they coincide), measured from its first point.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degenerate`] if every control point coincides with the first;
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn fat_line(&self, meter: &mut WorkMeter<'_>) -> Result<Slab<2>, NurbsError> {
        let pts = euclidean(meter, &self.points, self.weights.as_deref())?;
        let mid = |x: &[Interval; 2]| x.map(|c| c.lo() * 0.5 + c.hi() * 0.5);
        let first = mid(pts.first().ok_or(NurbsError::Defect)?);
        let mut chord = None;
        for x in pts.iter().rev() {
            let m = mid(x);
            let d = [m[0] - first[0], m[1] - first[1]];
            if d != [0.0, 0.0] {
                chord = Some(d);
                break;
            }
        }
        let [dx, dy] = chord.ok_or(NurbsError::Degenerate)?;
        self.slab([-dy, dx], first, meter)
    }
}

impl<const D: usize> Curve<D> {
    /// The Bezier pieces of a clamped curve as Bernstein forms enclosing their exact coefficients
    /// (A5.6 in interval arithmetic), in order along the domain.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`] for an unclamped curve; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn bernstein(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<BernsteinCurve<D>>, NurbsError> {
        let basis = self.basis();
        if !basis.is_clamped() {
            return Err(NurbsError::NotClamped);
        }
        charge_op(meter, self.homogeneous().len(), D, basis.knots().len())?;
        let pw = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let pieces = decompose(meter, basis, &pw)?;
        let rational = self.is_rational();
        let mut out = vec_for(meter, pieces.len())?;
        for (a, b, pts) in pieces {
            let weights = if rational {
                Some(pts.iter().map(|h| h.w).collect())
            } else {
                None
            };
            out.push(BernsteinCurve::new(
                (a, b),
                pts.iter().map(|h| h.c).collect(),
                weights,
            )?);
        }
        Ok(out)
    }
}

impl<const D: usize> BernsteinPatch<D> {
    /// The patch on [u0, u1] × [v0, v1] of degrees (p, q) with these Bernstein coefficients, row
    /// by row ((p + 1)·(q + 1) of them, u index first), the weighted points w·P if `weights` is
    /// given.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless there are (p + 1)·(q + 1) coefficients (and weights);
    /// [`NurbsError::EmptyDomain`]; [`NurbsError::Weight`].
    pub fn new(
        (u, v): ((f64, f64), (f64, f64)),
        degrees: (usize, usize),
        points: Vec<[Interval; D]>,
        weights: Option<Vec<Interval>>,
    ) -> Result<Self, NurbsError> {
        let expected = mul(add(degrees.0, 1)?, add(degrees.1, 1)?)?;
        if points.len() != expected {
            return Err(NurbsError::PointCount {
                points: points.len(),
                expected,
            });
        }
        check_domain(u)?;
        check_domain(v)?;
        check_weights(weights.as_deref(), expected)?;
        Ok(BernsteinPatch {
            u,
            v,
            degrees,
            points,
            weights,
        })
    }

    /// The degrees (p, q).
    #[must_use]
    pub fn degrees(&self) -> (usize, usize) {
        self.degrees
    }

    /// The domain ([u0, u1], [v0, v1]).
    #[must_use]
    pub fn domain(&self) -> ((f64, f64), (f64, f64)) {
        (self.u, self.v)
    }

    /// The Bernstein coefficients, row by row: the weighted points of a rational patch.
    #[must_use]
    pub fn points(&self) -> &[[Interval; D]] {
        &self.points
    }

    /// The weights, if the patch is rational.
    #[must_use]
    pub fn weights(&self) -> Option<&[Interval]> {
        self.weights.as_deref()
    }

    /// Whether the patch is rational.
    #[must_use]
    pub fn is_rational(&self) -> bool {
        self.weights.is_some()
    }

    /// The weighted points as a nonrational patch: the numerator A of S = A / W.
    #[must_use]
    pub fn numerator(&self) -> BernsteinPatch<D> {
        BernsteinPatch {
            u: self.u,
            v: self.v,
            degrees: self.degrees,
            points: self.points.clone(),
            weights: None,
        }
    }

    /// The weight function W as a nonrational scalar patch; `None` for a nonrational patch.
    #[must_use]
    pub fn weight_function(&self) -> Option<BernsteinPatch<1>> {
        self.weights.as_ref().map(|w| BernsteinPatch {
            u: self.u,
            v: self.v,
            degrees: self.degrees,
            points: w.iter().map(|&x| [x]).collect(),
            weights: None,
        })
    }

    /// Coefficient (i, j).
    pub(crate) fn at(&self, i: usize, j: usize) -> Result<[Interval; D], NurbsError> {
        get(&self.points, add(mul(i, add(self.degrees.1, 1)?)?, j)?)
    }

    /// A box holding every point of the patch.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn bound(&self, meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, NurbsError> {
        hull(&euclidean(meter, &self.points, self.weights.as_deref())?)
    }

    /// The partial derivative of a nonrational patch in `dir`, of one degree less there (0 and
    /// zero for degree 0): p / (u1 − u0) times the differences of neighbouring rows (u), or
    /// q / (v1 − v0) times those of neighbouring columns (v).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Rational`] for a rational patch; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn partial(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<D>, NurbsError> {
        if self.is_rational() {
            return Err(NurbsError::Rational);
        }
        let (p, q) = self.degrees;
        let zero = iv(0.0)?;
        // A direction of degree 0: the patch is constant along it, and the partial is zero.
        let (degrees, factor, flat) = match dir {
            Direction::U => ((p.saturating_sub(1), q), chain_factor(p, self.u)?, p == 0),
            Direction::V => ((p, q.saturating_sub(1)), chain_factor(q, self.v)?, q == 0),
        };
        let count = mul(add(degrees.0, 1)?, add(degrees.1, 1)?)?;
        let mut out = vec_for(meter, count)?;
        for i in 0..=degrees.0 {
            for j in 0..=degrees.1 {
                if flat {
                    out.push([zero; D]);
                    continue;
                }
                let (a, b) = match dir {
                    Direction::U => (self.at(i, j)?, self.at(add(i, 1)?, j)?),
                    Direction::V => (self.at(i, j)?, self.at(i, add(j, 1)?)?),
                };
                let mut d = [zero; D];
                for (x, (&bx, &ax)) in d.iter_mut().zip(b.iter().zip(&a)) {
                    *x = factor.times(bx.minus(ax)?)?;
                }
                out.push(d);
            }
        }
        Ok(BernsteinPatch {
            u: self.u,
            v: self.v,
            degrees,
            points: out,
            weights: None,
        })
    }

    /// A box holding the partial derivative in `dir` at every point of the patch; for a rational
    /// patch from the numerator `A_d`·W − A·`W_d` and the weights' range.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn derivative_bound(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<D>, NurbsError> {
        let Some(w) = self.weight_function() else {
            return self.partial(dir, meter)?.bound(meter);
        };
        let a = self.numerator();
        let da = a.partial(dir, meter)?;
        let dw = w.partial(dir, meter)?;
        let first = da.scaled_by(&w, meter)?;
        let second = a.scaled_by(&dw, meter)?;
        let numerator = first.minus(&second, meter)?;
        let weights = range(self.weights.iter().flatten().copied())?;
        quotient_bound(numerator.bound(meter)?, weights)
    }

    /// The range of n·(x − o) over every point x of the patch.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`]; [`NurbsError::Stopped`].
    pub fn slab(
        &self,
        normal: [f64; D],
        origin: [f64; D],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Slab<D>, NurbsError> {
        let pts = euclidean(meter, &self.points, self.weights.as_deref())?;
        let mut ds = vec_for(meter, pts.len())?;
        for p in &pts {
            ds.push(distance(normal, origin, *p)?);
        }
        let (lo, hi) = range(ds.into_iter())?;
        Ok(Slab {
            origin,
            normal,
            lo,
            hi,
        })
    }

    /// `self` − `other`, two nonrational patches of one degree and domain.
    pub(crate) fn minus(
        &self,
        other: &BernsteinPatch<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BernsteinPatch<D>, NurbsError> {
        if self.degrees != other.degrees || self.u != other.u || self.v != other.v {
            return Err(NurbsError::Defect);
        }
        let mut out = vec_for(meter, self.points.len())?;
        for (a, b) in self.points.iter().zip(&other.points) {
            let mut d = *a;
            for (x, y) in d.iter_mut().zip(b) {
                *x = x.minus(*y)?;
            }
            out.push(d);
        }
        Ok(BernsteinPatch {
            u: self.u,
            v: self.v,
            degrees: self.degrees,
            points: out,
            weights: None,
        })
    }

    /// A patch of the same domain with other coefficients.
    pub(crate) fn with<const E: usize>(
        &self,
        degrees: (usize, usize),
        points: Vec<[Interval; E]>,
    ) -> BernsteinPatch<E> {
        BernsteinPatch {
            u: self.u,
            v: self.v,
            degrees,
            points,
            weights: None,
        }
    }
}

impl BernsteinPatch<3> {
    /// The fat plane of a patch: the slab whose direction is the cross product of its two
    /// diagonals (of two edges from the first corner if those are parallel), measured from its
    /// first corner.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degenerate`] if neither choice gives a direction; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn fat_plane(&self, meter: &mut WorkMeter<'_>) -> Result<Slab<3>, NurbsError> {
        let (p, q) = self.degrees;
        let pts = euclidean(meter, &self.points, self.weights.as_deref())?;
        let mid = |i: usize, j: usize| -> Result<[f64; 3], NurbsError> {
            let x = get(&pts, add(mul(i, add(q, 1)?)?, j)?)?;
            Ok(x.map(|c| c.lo() * 0.5 + c.hi() * 0.5))
        };
        let sub3 = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let (c00, cp0, c0q, cpq) = (mid(0, 0)?, mid(p, 0)?, mid(0, q)?, mid(p, q)?);
        let mut n = cross(sub3(cpq, c00), sub3(c0q, cp0));
        if n == [0.0; 3] {
            n = cross(sub3(cp0, c00), sub3(c0q, c00));
        }
        if n == [0.0; 3] || !n.iter().all(|x| x.is_finite()) {
            return Err(NurbsError::Degenerate);
        }
        self.slab(n, c00, meter)
    }
}

impl<const D: usize> Surface<D> {
    /// The Bezier patches of a clamped surface as Bernstein forms enclosing their exact
    /// coefficients (A5.7 in interval arithmetic), u spans outer, v spans inner.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`] unless clamped in both directions; [`NurbsError::NotFinite`];
    /// [`NurbsError::Stopped`].
    pub fn bernstein(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<BernsteinPatch<D>>, NurbsError> {
        let degrees = (self.basis_u().degree(), self.basis_v().degree());
        let rational = self.is_rational();
        let patches = self.interval_patches(meter)?;
        let mut out = vec_for(meter, patches.len())?;
        for (u, v, pts) in patches {
            let weights = if rational {
                Some(pts.iter().map(|h| h.w).collect())
            } else {
                None
            };
            out.push(BernsteinPatch::new(
                (u, v),
                degrees,
                pts.iter().map(|h| h.c).collect(),
                weights,
            )?);
        }
        Ok(out)
    }
}
