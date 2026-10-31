//! The bulge polyline kind: lines and arcs through vertices, each arc given by its bulge, with
//! optional widths, as DXF's LWPOLYLINE stores them.
//!
//! What it does: [`BulgePolyline`] stores 2D vertices, one bulge per segment (b = tan(θ/4) for an
//! arc of sweep θ, positive turning anticlockwise; 0 for a straight segment) and optionally a
//! start and end width per segment. Segment i runs over parameters [s + i, s + i + 1] (s, the
//! start, is 0 for a new polyline). Each segment is exactly the rational quadratic its bulge
//! defines, in homogeneous form with p₀ and p₂ its ends and w = (1 − b²)/(1 + b²):
//! h₁ = ((1 − b²)·m + b·(d.y, −d.x))/(1 + b²), with m the chord's middle and d = p₂ − p₀, so
//! C(v) = (B₀p₀ + B₁h₁ + B₂p₂)/(B₀ + B₁w + B₂) with v the parameter within the segment. Every value
//! is a rational function of the stored doubles: a bulge arc is stored exactly, a semicircle
//! (w = 0) and a major arc (w < 0) included, whose denominator stays at least (1 + w)/2 > 0.
//!
//! Guarantees:
//!
//! - **Exact definition, certified evaluation.** Points, derivatives of every order (Leibniz's rule
//!   on W·C = A) and bounds over spans are interval evaluations of those rational functions.
//! - **An exact NURBS form.** The segments are rational quadratic Bezier pieces on the same
//!   parameter; an arc whose middle weight is low (a semicircle or more) is first halved, or
//!   quartered, by de Casteljau's algorithm at dyadic parameters, which keeps every weight
//!   positive. The deviation bounds only the rounding of the pieces' points and weights.
//! - **Transforms change the kind explicitly.** A similarity keeps a bulge polyline (vertices
//!   mapped, widths scaled, bulges negated by a mirror); any other transform makes its NURBS form.
//! - **Splits are trimmed polylines.** A split leaves the polyline untouched and trims it
//!   (`crate::trimmed`); a reversal reverses the vertices and negates the bulges, exactly.
//!
//! Method: the bulge of DXF's LWPOLYLINE (chapter 14's polyline row,
//! `projects/engines/geometry/architecture/14-2d-drafting.md`), written as the rational quadratic
//! form of a circular arc (L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., section 7.3,
//! <https://doi.org/10.1007/978-3-642-59223-2>) with the middle point carried homogeneously, so
//! the semicircle's point at infinity is a finite h₁.
//!
//! Cost: O(k) per evaluation of order k; a bound O(1) per segment the span meets; conversion O(n)
//! for n segments.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': p, h and w the homogeneous control data, b a bulge, v a parameter, d a chord, m its middle"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::eft::two_sum;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Curve, Limit};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::{check_parameter, count};
use crate::map::{NurbsForm, ParamMap};
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::tight::{self, point};
use crate::trimmed::Trimmed;

/// A bulge polyline (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct BulgePolyline<const D: usize> {
    vertices: Vec<[f64; D]>,
    bulges: Vec<f64>,
    widths: Option<Vec<(f64, f64)>>,
    closed: bool,
    start: f64,
}

/// One segment's homogeneous control data, enclosed: h₀, h₁, h₂ and w₀, w₁, w₂ (w₀ = w₂ = 1).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Segment<const D: usize> {
    pub(crate) h: [[Interval; D]; 3],
    pub(crate) w: [Interval; 3],
}

/// The pieces a segment is cut into for its NURBS form: 1, 2 or 4.
fn pieces_for(b: f64) -> usize {
    // A piece's middle weight is the cosine of a quarter of its sweep; keep it above 1/2.
    let a = b.abs();
    if a <= 0.577 {
        1
    } else if a <= 2.0 {
        2
    } else {
        4
    }
}

impl<const D: usize> BulgePolyline<D> {
    /// The polyline through `vertices` with one bulge per segment and, optionally, one (start,
    /// end) width per segment; closed polylines have a segment from the last vertex to the first.
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 2; [`CurveError::NotFinite`];
    /// [`CurveError::Degenerate`] for fewer than two vertices, a segment whose ends are the same
    /// point, a negative width, or counts that do not match the segments.
    pub fn new(
        vertices: Vec<[f64; D]>,
        bulges: Vec<f64>,
        widths: Option<Vec<(f64, f64)>>,
        closed: bool,
    ) -> Result<BulgePolyline<D>, CurveError> {
        BulgePolyline::with_start(vertices, bulges, widths, closed, 0.0)
    }

    /// The polyline of [`BulgePolyline::new`] whose parameter starts at `start` rather than 0, as
    /// reversing one leaves it: what a stored polyline is rebuilt from.
    ///
    /// # Errors
    ///
    /// As [`BulgePolyline::new`]; [`CurveError::NotFinite`] for a start that is not finite.
    pub fn starting_at(
        vertices: Vec<[f64; D]>,
        bulges: Vec<f64>,
        widths: Option<Vec<(f64, f64)>>,
        closed: bool,
        start: f64,
    ) -> Result<BulgePolyline<D>, CurveError> {
        if !start.is_finite() {
            return Err(CurveError::NotFinite);
        }
        BulgePolyline::with_start(vertices, bulges, widths, closed, start)
    }

    /// The parameter at the first vertex.
    #[must_use]
    pub fn start(&self) -> f64 {
        self.start
    }

    fn with_start(
        vertices: Vec<[f64; D]>,
        bulges: Vec<f64>,
        widths: Option<Vec<(f64, f64)>>,
        closed: bool,
        start: f64,
    ) -> Result<BulgePolyline<D>, CurveError> {
        if D != 2 {
            return Err(CurveError::Dimension { dimension: D });
        }
        let segments = if closed {
            vertices.len()
        } else {
            vertices.len().saturating_sub(1)
        };
        if vertices.len() < 2 || bulges.len() != segments {
            return Err(CurveError::Degenerate);
        }
        if !vertices
            .iter()
            .flatten()
            .chain(&bulges)
            .all(|x| x.is_finite())
        {
            return Err(CurveError::NotFinite);
        }
        if let Some(w) = &widths {
            if w.len() != segments {
                return Err(CurveError::Degenerate);
            }
            for &(a, b) in w {
                if !(a.is_finite() && b.is_finite()) {
                    return Err(CurveError::NotFinite);
                }
                if a < 0.0 || b < 0.0 {
                    return Err(CurveError::Degenerate);
                }
            }
        }
        let p = BulgePolyline {
            vertices,
            bulges,
            widths,
            closed,
            start,
        };
        for i in 0..segments {
            let (a, b) = p.ends(i)?;
            if a == b {
                return Err(CurveError::Degenerate);
            }
        }
        Ok(p)
    }

    /// The vertices.
    #[must_use]
    pub fn vertices(&self) -> &[[f64; D]] {
        &self.vertices
    }

    /// The bulges, one per segment.
    #[must_use]
    pub fn bulges(&self) -> &[f64] {
        &self.bulges
    }

    /// The (start, end) widths, one pair per segment, if the polyline has widths.
    #[must_use]
    pub fn widths(&self) -> Option<&[(f64, f64)]> {
        self.widths.as_deref()
    }

    /// Whether the last vertex joins the first.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The number of segments.
    #[must_use]
    pub fn segments(&self) -> usize {
        self.bulges.len()
    }

    /// The end vertices of segment i.
    fn ends(&self, i: usize) -> Result<([f64; D], [f64; D]), CurveError> {
        let n = self.vertices.len();
        let j = i.checked_add(1).ok_or(CurveError::Defect)?;
        let j = if j == n { 0 } else { j };
        match (self.vertices.get(i), self.vertices.get(j)) {
            (Some(&a), Some(&b)) => Ok((a, b)),
            _ => Err(CurveError::Defect),
        }
    }

    /// Segment i's homogeneous control data, enclosed (module documentation).
    pub(crate) fn segment(&self, i: usize) -> Result<Segment<D>, CurveError> {
        let (a, c) = self.ends(i)?;
        let b = point(*self.bulges.get(i).ok_or(CurveError::Defect)?)?;
        let one = point(1.0)?;
        let half = point(0.5)?;
        let b2 = tight::square(b)?;
        let s = tight::add(one, b2)?;
        let k = tight::sub(one, b2)?;
        let zero = point(0.0)?;
        let (mut h0, mut h1, mut h2) = ([zero; D], [zero; D], [zero; D]);
        let p0: [Interval; D] = {
            let mut o = [zero; D];
            for (x, &y) in o.iter_mut().zip(&a) {
                *x = point(y)?;
            }
            o
        };
        let p2: [Interval; D] = {
            let mut o = [zero; D];
            for (x, &y) in o.iter_mut().zip(&c) {
                *x = point(y)?;
            }
            o
        };
        // d = p₂ − p₀ and the right-hand perpendicular (d_y, −d_x).
        let [dx, dy] = {
            let mut d = [zero; 2];
            for ((o, &x), &y) in d.iter_mut().zip(&p0).zip(&p2) {
                *o = tight::sub(y, x)?;
            }
            d
        };
        let perp = [dy, dx.neg()];
        for (i, ((o0, o1), o2)) in h0
            .iter_mut()
            .zip(h1.iter_mut())
            .zip(h2.iter_mut())
            .enumerate()
        {
            let (x0, x2) = (
                *p0.get(i).ok_or(CurveError::Defect)?,
                *p2.get(i).ok_or(CurveError::Defect)?,
            );
            let m = tight::mul(tight::add(x0, x2)?, half)?;
            let q = *perp.get(i).ok_or(CurveError::Defect)?;
            let num = tight::add(tight::mul(k, m)?, tight::mul(b, q)?)?;
            *o0 = x0;
            *o1 = tight::div(num, s)?;
            *o2 = x2;
        }
        Ok(Segment {
            h: [h0, h1, h2],
            w: [one, tight::div(k, s)?, one],
        })
    }

    /// The segment of `t`, from `side` at a vertex, and its local parameter v enclosed. The
    /// offset x = t − s is split exactly into a double and its error (`two_sum`), so the segment
    /// is chosen by exact comparisons and v = (x − i) is a point when it is a double.
    fn locate(&self, t: f64, side: Limit) -> Result<(usize, Interval), CurveError> {
        check_parameter(ParamCurve::domain(self), t)?;
        let n = self.segments();
        let (x, e) = two_sum(t, -self.start);
        let mut i = crate::form::floor_index(tight::larger(x, 0.0))?;
        let whole = count(i)? == x;
        // Below the vertex exactly, or at it from the left: the segment before (none before the
        // first).
        if whole && (e < 0.0 || (e == 0.0 && side == Limit::FromLeft)) {
            i = i.saturating_sub(1);
        }
        if i >= n {
            i = n.saturating_sub(1);
        }
        // x − i is exact (Sterbenz: i ≤ x ≤ 2i, or i = 0), and e is the rest.
        let v = tight::add(point(x - count(i)?)?, point(e)?)?;
        let (lo, hi) = (tight::larger(v.lo(), 0.0), tight::smaller(v.hi(), 1.0));
        Ok((i, Interval::new(lo, tight::larger(hi, lo))?))
    }

    /// The local parameters of the span [lo, hi] within segment i, rounded outward and cut to
    /// [0, 1].
    fn local(&self, i: usize, (lo, hi): (f64, f64)) -> Result<Interval, CurveError> {
        let a = point(self.start + count(i)?)?;
        let l = tight::sub(point(lo)?, a)?.lo();
        let h = tight::sub(point(hi)?, a)?.hi();
        let l = l.clamp(0.0, 1.0);
        let h = tight::larger(tight::smaller(h, 1.0), l);
        Ok(Interval::new(l, h)?)
    }
}

/// A quadratic in v from its Bernstein coefficients: (value, first, second derivative) at `v`.
fn quadratic(c: [Interval; 3], v: Interval) -> Result<[Interval; 3], CurveError> {
    let [c0, c1, c2] = c;
    let two = point(2.0)?;
    // c(v) = c₀ + 2v(c₁ − c₀) + v²(c₀ − 2c₁ + c₂).
    let a1 = tight::mul(two, tight::sub(c1, c0)?)?;
    let a2 = tight::add(tight::sub(c0, tight::mul(two, c1)?)?, c2)?;
    let value = tight::add(c0, tight::mul(v, tight::add(a1, tight::mul(v, a2)?)?)?)?;
    let first = tight::add(a1, tight::mul(tight::mul(two, a2)?, v)?)?;
    let second = tight::mul(two, a2)?;
    Ok([value, first, second])
}

/// The rational quadratic's point and derivatives up to `order` over the local parameters `v`,
/// enclosed: C^(k) = (A^(k) − Σᵢ C(k, i)·W^(i)·C^(k−i)) / W, A and W of degree 2. W is at least
/// (1 + w₁)/2 on [0, 1] (its minimum, at v = 1/2), which bounds it away from zero.
pub(crate) fn rational_derivatives<const D: usize>(
    seg: &Segment<D>,
    v: Interval,
    order: usize,
) -> Result<Vec<[Interval; D]>, CurveError> {
    let zero = point(0.0)?;
    let wq = quadratic(seg.w, v)?;
    let [_, w1, _] = seg.w;
    let floor = tight::mul(tight::add(point(1.0)?, w1)?, point(0.5)?)?;
    let w = *wq.first().ok_or(CurveError::Defect)?;
    let w0 = Interval::new(
        tight::larger(w.lo(), floor.lo()),
        tight::larger(w.hi(), floor.lo()),
    )?;
    let mut a: [[Interval; D]; 3] = [[zero; D]; 3];
    for d in 0..D {
        let c = [
            *seg.h[0].get(d).ok_or(CurveError::Defect)?,
            *seg.h[1].get(d).ok_or(CurveError::Defect)?,
            *seg.h[2].get(d).ok_or(CurveError::Defect)?,
        ];
        let q = quadratic(c, v)?;
        for (k, row) in a.iter_mut().enumerate() {
            if let (Some(slot), Some(&x)) = (row.get_mut(d), q.get(k)) {
                *slot = x;
            }
        }
    }
    let at = |s: &[[Interval; D]; 3], k: usize| -> [Interval; D] {
        s.get(k).copied().unwrap_or([zero; D])
    };
    let wk = |k: usize| -> Interval {
        if k == 0 {
            w0
        } else {
            wq.get(k).copied().unwrap_or(zero)
        }
    };
    let mut out: Vec<[Interval; D]> = Vec::with_capacity(order.saturating_add(1));
    for k in 0..=order {
        let mut num = at(&a, k);
        let mut binom = 1.0_f64;
        for i in 1..=k {
            // C(k, i), built step by step: exact for the orders used.
            binom = binom
                * count(
                    k.checked_sub(i)
                        .ok_or(CurveError::Defect)?
                        .checked_add(1)
                        .ok_or(CurveError::Defect)?,
                )?
                / count(i)?;
            let wi = wk(i);
            // An exactly 0 derivative of the weight (a straight segment) adds nothing.
            if wi == zero {
                continue;
            }
            let prev = out
                .get(k.checked_sub(i).ok_or(CurveError::Defect)?)
                .copied()
                .ok_or(CurveError::Defect)?;
            let f = tight::mul(point(binom)?, wi)?;
            for (n, &c) in num.iter_mut().zip(&prev) {
                *n = tight::sub(*n, tight::mul(f, c)?)?;
            }
        }
        let mut c = [zero; D];
        for (o, &n) in c.iter_mut().zip(&num) {
            *o = tight::div(n, w0)?;
        }
        out.push(c);
    }
    Ok(out)
}

/// The same in doubles at one parameter.
fn rational_doubles<const D: usize>(
    seg: &Segment<D>,
    v: f64,
    out: &mut [[f64; D]],
) -> Result<(), CurveError> {
    let e = rational_derivatives(seg, point(v)?, out.len().saturating_sub(1))?;
    for (o, x) in out.iter_mut().zip(&e) {
        *o = x.map(tight::middle);
    }
    Ok(())
}

impl<const D: usize> ParamCurve<D> for BulgePolyline<D> {
    fn domain(&self) -> (f64, f64) {
        // Whole numbers below 2^53, so exact.
        (
            self.start,
            self.start + count(self.segments()).unwrap_or(0.0),
        )
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        let mut out = [[0.0; D]; 1];
        self.derivatives(t, side, &mut out)?;
        let [p] = out;
        Ok(p)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        let (i, v) = self.locate(t, side)?;
        rational_doubles(&self.segment(i)?, tight::middle(v), out)
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        meter.charge(
            u64::try_from(order)
                .map_err(|_| CurveError::Defect)?
                .saturating_add(1),
        )?;
        let (i, v) = self.locate(t, side)?;
        rational_derivatives(&self.segment(i)?, v, order)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        hull(self, span, 0, meter)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        Ok(DerivativeBound::Bounded(hull(self, span, order, meter)?))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        let n = self.segments();
        meter.charge(u64::try_from(n).map_err(|_| CurveError::Defect)?)?;
        let mut homs: Vec<[f64; D]> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();
        let mut knots: Vec<f64> = Vec::new();
        let mut deviation = point(0.0)?;
        let (a, _) = ParamCurve::domain(self);
        knots.extend([a, a, a]);
        for i in 0..n {
            let seg = self.segment(i)?;
            let b = *self.bulges.get(i).ok_or(CurveError::Defect)?;
            let parts = pieces_for(b);
            for piece in subdivide(&seg, parts)? {
                let dev = piece_deviation(&piece)?;
                deviation = point(tight::larger(deviation.hi(), dev))?;
                let [h0, h1, h2] = piece.h;
                let [w0, w1, w2] = piece.w;
                if homs.is_empty() {
                    homs.push(h0.map(tight::middle));
                    weights.push(tight::middle(w0));
                }
                homs.push(h1.map(tight::middle));
                weights.push(tight::middle(w1));
                homs.push(h2.map(tight::middle));
                weights.push(tight::middle(w2));
            }
            for k in 1..=parts {
                let u = a + count(i)? + count(k)? / count(parts)?;
                if i.saturating_add(1) == n && k == parts {
                    knots.extend([u, u, u]);
                } else {
                    knots.extend([u, u]);
                }
            }
        }
        let curve = Curve::from_homogeneous(Basis::new(2, knots)?, homs, Some(weights))?;
        Ok(Changed {
            value: NurbsForm {
                curve,
                map: ParamMap::Same,
            },
            deviation: deviation.hi(),
        })
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        Trimmed::new(
            crate::basis::Basis::Bulge(self.clone()),
            ParamCurve::domain(self),
        )?
        .split(t, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = ParamCurve::domain(self);
        let mut vertices = self.vertices.clone();
        let mut bulges: Vec<f64> = self.bulges.iter().map(|x| -x).collect();
        let mut widths = self
            .widths
            .clone()
            .map(|w| w.into_iter().map(|(s, e)| (e, s)).collect::<Vec<_>>());
        if self.closed {
            // Segment i runs v_i → v_(i+1); reversed, v_0 stays first and the segments run back.
            vertices.get_mut(1..).ok_or(CurveError::Defect)?.reverse();
        } else {
            vertices.reverse();
        }
        bulges.reverse();
        if let Some(w) = widths.as_mut() {
            w.reverse();
        }
        let _ = a;
        let p = BulgePolyline::with_start(vertices, bulges, widths, self.closed, -b)?;
        Ok(Changed {
            value: CurveKind::Bulge(p),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let m = Motion::of::<D>(transform)?;
        meter.charge(u64::try_from(self.segments()).map_err(|_| CurveError::Defect)?)?;
        if !m.similar() {
            // Arcs become elliptical arcs: the NURBS form, moved.
            let n = self.to_nurbs(meter)?;
            let moved = ParamCurve::transformed(&n.value.curve, transform, meter)?;
            return Ok(Changed {
                value: moved.value,
                deviation: tight::add(point(n.deviation)?, point(moved.deviation)?)?.hi(),
            });
        }
        let mut vertices = Vec::with_capacity(self.vertices.len());
        for v in &self.vertices {
            vertices.push(m.point(*v)?);
        }
        let bulges: Vec<f64> = if m.mirror() {
            self.bulges.iter().map(|x| -x).collect()
        } else {
            self.bulges.clone()
        };
        let widths = self
            .widths
            .as_ref()
            .map(|w| w.iter().map(|&(a, b)| (m.scale * a, m.scale * b)).collect());
        let p = BulgePolyline::with_start(vertices, bulges, widths, self.closed, self.start)?;
        // Each segment of the image is a rational quadratic with the same weights as the image of
        // the old one; the curves differ by at most max |Δh| / min W, with W ≥ (1 + w₁)/2.
        let mut deviation = point(0.0)?;
        for i in 0..self.segments() {
            let (old, new) = (self.segment(i)?, p.segment(i)?);
            let mut worst = 0.0;
            for k in 0..3 {
                let (h, w) = (
                    *old.h.get(k).ok_or(CurveError::Defect)?,
                    *old.w.get(k).ok_or(CurveError::Defect)?,
                );
                let image = m.homogeneous_image(&h, w)?;
                let d = tight::distance(new.h.get(k).ok_or(CurveError::Defect)?, &image)?;
                worst = tight::larger(worst, d);
            }
            if worst != 0.0 {
                let [_, w1, _] = old.w;
                let floor = tight::mul(tight::add(point(1.0)?, w1)?, point(0.5)?)?;
                let dev = tight::div(point(worst)?, Interval::new(floor.lo(), floor.lo())?)?.hi();
                deviation = point(tight::larger(deviation.hi(), dev))?;
            }
        }
        Ok(Changed {
            value: CurveKind::Bulge(p),
            deviation: deviation.hi(),
        })
    }
}

/// The hull of the derivative of `order` over `span`, segment by segment.
fn hull<const D: usize>(
    p: &BulgePolyline<D>,
    span: (f64, f64),
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, CurveError> {
    crate::nurbs::check_span(span, ParamCurve::domain(p))?;
    let mut out = Bounds {
        lo: [f64::INFINITY; D],
        hi: [f64::NEG_INFINITY; D],
    };
    let (first, last) = (
        p.locate(span.0, Limit::FromRight)?.0,
        p.locate(span.1, Limit::FromLeft)?.0,
    );
    for i in first..=last {
        meter.charge(1)?;
        let v = p.local(i, span)?;
        let e = rational_derivatives(&p.segment(i)?, v, order)?;
        let x = e.last().ok_or(CurveError::Defect)?;
        for ((lo, hi), v) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(x) {
            *lo = tight::smaller(*lo, v.lo());
            *hi = tight::larger(*hi, v.hi());
        }
    }
    Ok(out)
}

/// A segment cut into `parts` (1, 2 or 4) equal pieces by de Casteljau's algorithm on homogeneous
/// points at v = 1/2 (twice for 4): halving and averaging only, so exact but for the additions.
fn subdivide<const D: usize>(
    seg: &Segment<D>,
    parts: usize,
) -> Result<Vec<Segment<D>>, CurveError> {
    let mut out = alloc::vec![*seg];
    let mut done = 1;
    while done < parts {
        let mut next = Vec::with_capacity(out.len().saturating_mul(2));
        for s in &out {
            let (l, r) = halve(s)?;
            next.push(l);
            next.push(r);
        }
        out = next;
        done = done.saturating_mul(2);
    }
    Ok(out)
}

/// The two halves of a homogeneous quadratic at v = 1/2.
fn halve<const D: usize>(s: &Segment<D>) -> Result<(Segment<D>, Segment<D>), CurveError> {
    let half = point(0.5)?;
    let mid = |a: Interval, b: Interval| -> Result<Interval, CurveError> {
        tight::mul(tight::add(a, b)?, half)
    };
    let [w0, w1, w2] = s.w;
    let (a1, b1) = (mid(w0, w1)?, mid(w1, w2)?);
    let c = mid(a1, b1)?;
    let zero = point(0.0)?;
    let (mut ha1, mut hb1, mut hc) = ([zero; D], [zero; D], [zero; D]);
    for d in 0..D {
        let g = |k: usize| -> Result<Interval, CurveError> {
            s.h.get(k)
                .and_then(|h| h.get(d))
                .copied()
                .ok_or(CurveError::Defect)
        };
        let (x0, x1, x2) = (g(0)?, g(1)?, g(2)?);
        let (p, q) = (mid(x0, x1)?, mid(x1, x2)?);
        if let (Some(o1), Some(o2), Some(o3)) = (ha1.get_mut(d), hb1.get_mut(d), hc.get_mut(d)) {
            *o1 = p;
            *o2 = q;
            *o3 = mid(p, q)?;
        }
    }
    let [h0, _, h2] = s.h;
    Ok((
        Segment {
            h: [h0, ha1, hc],
            w: [w0, a1, c],
        },
        Segment {
            h: [hc, hb1, h2],
            w: [c, b1, w2],
        },
    ))
}

/// The deviation of a piece stored as doubles from its exact data: max |Pᵢ − Qᵢ| plus
/// 2ε/(1 − ε)·max |Qᵢ − Q₀|, with Pᵢ the stored Euclidean points, Qᵢ the exact ones and ε the
/// largest relative error of a stored weight (`crate::rational`'s bound).
fn piece_deviation<const D: usize>(s: &Segment<D>) -> Result<f64, CurveError> {
    let zero = point(0.0)?;
    let one = point(1.0)?;
    let mut exact = [[zero; D]; 3];
    let mut stored = [[zero; D]; 3];
    let mut eps = 0.0;
    for k in 0..3 {
        let h: [Interval; D] = *s.h.get(k).ok_or(CurveError::Defect)?;
        let w: Interval = *s.w.get(k).ok_or(CurveError::Defect)?;
        let sw = point(tight::middle(w))?;
        let r = tight::magnitude(tight::sub(tight::div(sw, w)?, one)?);
        eps = tight::larger(eps, r);
        let erow: &mut [Interval; D] = exact.get_mut(k).ok_or(CurveError::Defect)?;
        let srow: &mut [Interval; D] = stored.get_mut(k).ok_or(CurveError::Defect)?;
        for ((e, st), &x) in erow.iter_mut().zip(srow.iter_mut()).zip(&h) {
            *e = tight::div(x, w)?;
            *st = tight::div(point(tight::middle(x))?, sw)?;
        }
    }
    if eps.is_nan() || eps >= 0.5 {
        return Err(CurveError::NotFinite);
    }
    let mut moved = 0.0;
    let mut reach = 0.0;
    for k in 0..3 {
        let (e, st) = (
            exact.get(k).ok_or(CurveError::Defect)?,
            stored.get(k).ok_or(CurveError::Defect)?,
        );
        let d = tight::distance(st, e)?;
        moved = tight::larger(moved, d);
        let r = tight::distance(e, exact.first().ok_or(CurveError::Defect)?)?;
        reach = tight::larger(reach, r);
    }
    let mut total = point(moved)?;
    if eps > 0.0 {
        let e = point(eps)?;
        let factor = tight::div(tight::mul(point(2.0)?, e)?, tight::sub(one, e)?)?;
        total = tight::add(total, tight::mul(factor, point(reach)?)?)?;
    }
    Ok(total.hi())
}
