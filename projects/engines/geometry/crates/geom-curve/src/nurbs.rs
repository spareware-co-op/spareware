//! The NURBS curve kind: `geom-nurbs`'s [`Curve`] under the kind contract.
//!
//! What it does: implements [`ParamCurve`] for [`Curve`]. Points and derivatives are A3.1, A3.2 and
//! A4.2 (`geom-nurbs`). Enclosures, bounds and derivative bounds come from the curve's Bernstein
//! forms in interval arithmetic (piece G59): the piece holding the parameter, or each piece
//! meeting the span cut down to it by de Casteljau subdivision at interval parameters, its box
//! from the convex hull; derivatives from hodographs, and for a rational curve by Leibniz's rule on
//! W·C = A, C^(k) = (A^(k) − Σ C(k, i)·W^(i)·C^(k−i)) / W, in interval boxes. Conversion to NURBS is
//! the curve itself, exactly. A split inserts the parameter until its multiplicity is the degree
//! (A5.1) and parts the control points there. Reversal negates and reverses the knots, which is
//! exact. A transform maps the control points (affine invariance) with the weights kept.
//!
//! Guarantees:
//!
//! - **Certified.** Every enclosure and bound holds the exact value; a split's deviation is the
//!   certified rounding of the knot insertion; a transform's is the largest certified error of a
//!   control point's image, which bounds the curve's because a NURBS point is a weighted average
//!   of its control points with weights `N_i`·`w_i` ≥ 0 (The NURBS Book, P4.3 and P4.4).
//! - **Exact where the operation is.** Conversion and reversal change no bit (deviation 0).
//! - **Refused outside the domain.** A parameter or span outside it is an error, never clamped.
//!
//! Method: The NURBS Book, 2nd ed. (<https://doi.org/10.1007/978-3-642-59223-2>): A3.1, A3.2,
//! A4.2, A5.1, de Casteljau subdivision (section 1.3), hodographs (section 3.3) and affine
//! invariance (P4.10), on `geom-nurbs` (pieces G56 and G59).
//!
//! Cost: an enclosure or bound runs Bezier extraction in intervals, O(s·p²·D) for s spans, plus
//! O(p²·D) per piece and derivative order; a split O(n·D); a reversal or transform O(n·D).

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: p for the degree, t and s for parameters, a and b for a domain, w for weights, h for homogeneous points"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_nurbs::{Basis, BernsteinCurve, Bounds, Changed, Curve, Limit};
use geom_run::WorkMeter;

use crate::affine::Entries;
use crate::error::CurveError;
use crate::map::{NurbsForm, ParamMap};
use crate::param::{CurveKind, DerivativeBound, ParamCurve};

/// `x` as a point interval.
fn iv(x: f64) -> Result<Interval, CurveError> {
    Ok(Interval::point(x)?)
}

/// A checked index.
fn at<T: Copy>(s: &[T], i: usize) -> Result<T, CurveError> {
    s.get(i).copied().ok_or(CurveError::Defect)
}

/// A span inside the domain, ordered and finite.
pub(crate) fn check_span(span: (f64, f64), (a, b): (f64, f64)) -> Result<(), CurveError> {
    let (lo, hi) = span;
    if lo.is_finite() && hi.is_finite() && a <= lo && lo <= hi && hi <= b {
        Ok(())
    } else {
        Err(CurveError::SpanOutside)
    }
}

/// The piece cut down to the part of [lo, hi] it covers (the piece itself when it lies inside).
fn restrict<const D: usize>(
    piece: &BernsteinCurve<D>,
    (lo, hi): (f64, f64),
) -> Result<BernsteinCurve<D>, CurveError> {
    let (a, b) = piece.domain();
    let sub = (lo.clamp(a, b), hi.clamp(a, b));
    if sub == (a, b) {
        return Ok(piece.clone());
    }
    Ok(piece.restrict(sub)?)
}

/// Binomial C(k, i) as an interval (exact for the orders used, k ≤ 2·`MAX_DEGREE`).
fn binomial(k: usize, i: usize) -> Result<Interval, CurveError> {
    let mut c = iv(1.0)?;
    for j in 0..i {
        let num = iv(f64::from(
            u32::try_from(k.saturating_sub(j)).map_err(|_| CurveError::Defect)?,
        ))?;
        let den = iv(f64::from(
            u32::try_from(j.saturating_add(1)).map_err(|_| CurveError::Defect)?,
        ))?;
        c = c.mul(num)?.div(den)?;
    }
    Ok(c)
}

/// Leibniz's rule on W·C = A in intervals: C^(k) from A^(k), W^(1..=k) and C^(0..k), W > 0.
fn leibniz<const D: usize>(
    a: &[[Interval; D]],
    w: &[Interval],
) -> Result<Vec<[Interval; D]>, CurveError> {
    let w0 = at(w, 0)?;
    let mut c: Vec<[Interval; D]> = Vec::with_capacity(a.len());
    for (k, ak) in a.iter().enumerate() {
        let mut v = *ak;
        for i in 1..=k {
            let f = binomial(k, i)?.mul(at(w, i)?)?;
            let prev = at(&c, k.saturating_sub(i))?;
            for (x, p) in v.iter_mut().zip(&prev) {
                *x = x.sub(f.mul(*p)?)?;
            }
        }
        for x in &mut v {
            *x = x.div(w0)?;
        }
        c.push(v);
    }
    Ok(c)
}

/// The box of interval values.
fn hull_of<const D: usize>(v: &[Interval; D]) -> Bounds<D> {
    Bounds {
        lo: v.map(Interval::lo),
        hi: v.map(Interval::hi),
    }
}

/// The union of two boxes, the first absent before any.
fn union<const D: usize>(a: Option<Bounds<D>>, b: Bounds<D>) -> Result<Bounds<D>, CurveError> {
    match a {
        None => Ok(b),
        Some(a) => Ok(a.union(&b)?),
    }
}

/// A box as intervals.
fn box_iv<const D: usize>(b: &Bounds<D>) -> Result<[Interval; D], CurveError> {
    let mut out = [iv(0.0)?; D];
    for (o, (&l, &h)) in out.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
        *o = Interval::new(l, h)?;
    }
    Ok(out)
}

/// The index of the Bezier piece holding `t`, from `side` at a break.
fn piece_at<const D: usize>(
    pieces: &[BernsteinCurve<D>],
    t: f64,
    side: Limit,
) -> Result<usize, CurveError> {
    let last = pieces.len().checked_sub(1).ok_or(CurveError::Defect)?;
    for (i, p) in pieces.iter().enumerate() {
        let (a, b) = p.domain();
        let inside = match side {
            Limit::FromRight => a <= t && (t < b || i == last),
            Limit::FromLeft => (a < t || i == 0) && t <= b,
        };
        if inside && a <= t && t <= b {
            return Ok(i);
        }
    }
    Err(CurveError::ParameterOutside)
}

/// The derivative bound of one nonrational or rational piece, order `order`.
fn piece_derivative_bound<const D: usize>(
    piece: &BernsteinCurve<D>,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, CurveError> {
    let Some(w) = piece.weight_function() else {
        let mut h = piece.clone();
        for _ in 0..order {
            h = h.hodograph(meter)?;
        }
        return Ok(h.bound(meter)?);
    };
    if order == 0 {
        return Ok(piece.bound(meter)?);
    }
    if order == 1 {
        return Ok(piece.derivative_bound(meter)?);
    }
    // Leibniz's rule on boxes: A^(k) and W^(i) from hodograph chains, W ≥ its least weight.
    let mut a = Vec::with_capacity(order.saturating_add(1));
    let mut ws = Vec::with_capacity(order.saturating_add(1));
    let mut ha = piece.numerator();
    let mut hw = w;
    for k in 0..=order {
        if k > 0 {
            ha = ha.hodograph(meter)?;
            hw = hw.hodograph(meter)?;
        }
        a.push(box_iv(&ha.bound(meter)?)?);
        let [wb] = box_iv(&hw.bound(meter)?)?;
        ws.push(wb);
    }
    // W itself lies between the least and the largest weight.
    let mut range = None;
    for x in piece.weights().ok_or(CurveError::Defect)? {
        range = Some(union(
            range,
            Bounds {
                lo: [x.lo()],
                hi: [x.hi()],
            },
        )?);
    }
    let range = range.ok_or(CurveError::Defect)?;
    if let Some(w0) = ws.first_mut() {
        *w0 = Interval::new(range.lo[0], range.hi[0])?;
    }
    let c = leibniz(&a, &ws)?;
    Ok(hull_of(c.last().ok_or(CurveError::Defect)?))
}

impl<const D: usize> ParamCurve<D> for Curve<D> {
    fn domain(&self) -> (f64, f64) {
        self.basis().domain()
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        Ok(Curve::point(self, t, side)?)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        Ok(self.derivs(t, side, out)?)
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        let (a, b) = ParamCurve::domain(self);
        if !(a <= t && t <= b) {
            return Err(CurveError::ParameterOutside);
        }
        let pieces = self.bernstein(meter)?;
        let piece = pieces
            .get(piece_at(&pieces, t, side)?)
            .ok_or(CurveError::Defect)?;
        let mut values = Vec::with_capacity(order.saturating_add(1));
        let mut weights = Vec::with_capacity(order.saturating_add(1));
        let mut h = piece.numerator();
        let mut hw = piece.weight_function();
        for k in 0..=order {
            if k > 0 {
                h = h.hodograph(meter)?;
                hw = hw.map(|w| w.hodograph(meter)).transpose()?;
            }
            values.push(h.value_at(t)?.0);
            if let Some(w) = &hw {
                let [x] = w.value_at(t)?.0;
                weights.push(x);
            }
        }
        if piece.is_rational() {
            leibniz(&values, &weights)
        } else {
            Ok(values)
        }
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        let dom = ParamCurve::domain(self);
        check_span(span, dom)?;
        let (lo, hi) = span;
        if lo == hi {
            let p = self.enclose(lo, Limit::FromRight, 0, meter)?;
            return Ok(hull_of(p.first().ok_or(CurveError::Defect)?));
        }
        let mut out = None;
        for piece in self.bernstein(meter)? {
            let (a, b) = piece.domain();
            if b <= lo || a >= hi {
                continue;
            }
            out = Some(union(out, restrict(&piece, span)?.bound(meter)?)?);
        }
        out.ok_or(CurveError::Defect)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        check_span(span, ParamCurve::domain(self))?;
        let (lo, hi) = span;
        let mut out = None;
        for piece in self.bernstein(meter)? {
            let (a, b) = piece.domain();
            // A degenerate span bounds both one-sided limits: every piece that touches it.
            if b < lo || a > hi || (lo < hi && (b == lo || a == hi)) {
                continue;
            }
            let part = if lo < hi {
                restrict(&piece, span)?
            } else {
                piece
            };
            out = Some(union(out, piece_derivative_bound(&part, order, meter)?)?);
        }
        out.map(DerivativeBound::Bounded).ok_or(CurveError::Defect)
    }

    fn to_nurbs(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        Ok(Changed {
            value: NurbsForm {
                curve: self.clone(),
                map: ParamMap::Same,
            },
            deviation: 0.0,
        })
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (a, b) = ParamCurve::domain(self);
        if !(a < t && t < b) {
            return Err(CurveError::ParameterOutside);
        }
        let p = self.degree();
        let s = self.basis().multiplicity(t);
        let inserted = self.insert_knot(t, p.saturating_sub(s), meter)?;
        let c = &inserted.value;
        let knots = c.basis().knots();
        // The last copy of t, at index k: points 0..=k−p go left, k−p.. go right.
        let k = knots
            .iter()
            .rposition(|&x| x == t)
            .ok_or(CurveError::Defect)?;
        let cut = k.checked_sub(p).ok_or(CurveError::Defect)?;
        let mut left_knots: Vec<f64> = knots.get(..=k).ok_or(CurveError::Defect)?.to_vec();
        left_knots.push(t);
        let mut right_knots = Vec::with_capacity(knots.len());
        right_knots.push(t);
        right_knots.extend_from_slice(
            knots
                .get(cut.checked_add(1).ok_or(CurveError::Defect)?..)
                .ok_or(CurveError::Defect)?,
        );
        let pts = c.homogeneous();
        let left_pts = pts.get(..=cut).ok_or(CurveError::Defect)?.to_vec();
        let right_pts = pts.get(cut..).ok_or(CurveError::Defect)?.to_vec();
        let (lw, rw) = match c.weights() {
            Some(w) => (
                Some(w.get(..=cut).ok_or(CurveError::Defect)?.to_vec()),
                Some(w.get(cut..).ok_or(CurveError::Defect)?.to_vec()),
            ),
            None => (None, None),
        };
        let left = Curve::from_homogeneous(Basis::new(p, left_knots)?, left_pts, lw)?;
        let right = Curve::from_homogeneous(Basis::new(p, right_knots)?, right_pts, rw)?;
        let d = inserted.deviation;
        Ok((
            Changed {
                value: CurveKind::Nurbs(left),
                deviation: d,
            },
            Changed {
                value: CurveKind::Nurbs(right),
                deviation: d,
            },
        ))
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let knots: Vec<f64> = self.basis().knots().iter().rev().map(|k| -k).collect();
        let pts: Vec<[f64; D]> = self.homogeneous().iter().rev().copied().collect();
        let w = self.weights().map(|w| w.iter().rev().copied().collect());
        let c = Curve::from_homogeneous(Basis::new(self.degree(), knots)?, pts, w)?;
        Ok(Changed {
            value: CurveKind::Nurbs(c),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        if D != 2 && D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        let e = Entries::of(transform)?;
        if D == 2 && !e.acts_in_plane() {
            return Err(CurveError::NotInPlane);
        }
        let mut pts = Vec::with_capacity(self.homogeneous().len());
        let mut deviation = 0.0_f64;
        for (i, h) in self.homogeneous().iter().enumerate() {
            let w = self.weights().map_or(Ok(1.0), |ws| at(ws, i))?;
            let mut h3 = [0.0; 3];
            for (x, y) in h3.iter_mut().zip(h) {
                *x = *y;
            }
            let (img, err) = e.image(h3, w)?;
            let mut out = [0.0; D];
            for (x, y) in out.iter_mut().zip(&img) {
                *x = *y;
            }
            pts.push(out);
            deviation = Real::new(deviation)?.total_max(Real::new(err)?).get();
        }
        let c = Curve::from_homogeneous(
            self.basis().clone(),
            pts,
            self.weights().map(<[f64]>::to_vec),
        )?;
        Ok(Changed {
            value: CurveKind::Nurbs(c),
            deviation,
        })
    }
}
