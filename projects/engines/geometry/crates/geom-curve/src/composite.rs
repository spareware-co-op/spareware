//! The composite curve kind: trimmed curves joined end to end into one curve, and its conversion to
//! one NURBS curve.
//!
//! What it does: [`Composite`] stores segments (trimmed curves, so a segment is any leaf kind on
//! any interval) and starts s₀ < s₁ < … < sₙ: segment k runs over [sₖ, sₖ₊₁] of the
//! composite's parameter, mapped affinely onto its own interval [aₖ, bₖ], so the joints sit at
//! the starts exactly. A new composite starts at 0 and gives each segment its own length,
//! sₖ₊₁ = sₖ + (bₖ − aₖ), rounded, which the affine map absorbs. Consecutive segments must
//! meet: the end of one and the start of the next must have certified enclosures that overlap
//! (chapter 14: "ends must be the same exact point"; there is no join fuzz), and each joint keeps
//! a certified bound on its gap, 0 when both ends are the same exact point.
//!
//! [`ParamCurve::to_nurbs`] makes one NURBS curve (chapter 14's "separate conversion"): each
//! segment's NURBS form is raised to the highest degree, cut into Bezier pieces (A5.6), and the
//! pieces laid end to end with their knots shifted, the joint control points shared and the
//! weights rescaled to meet; the map takes each segment's map and shift ([`ParamMap::Pieces`]).
//!
//! Guarantees:
//!
//! - **Exact definition.** Evaluation at a parameter whose segment parameter is exact is the
//!   segment's; otherwise the segment parameter is enclosed and the segment bounded over it, so
//!   enclosures stay certified; derivatives carry the affine map's factor.
//! - **Certified conversion.** The deviation adds, per segment, the segment's conversion,
//!   elevation and Bezier extraction, the joint's gap and weight rescaling (the rational bound of
//!   `crate::rational`), and the rounding of shifted knots (a Bezier piece reparametrised by e at
//!   its ends moves by at most |N′|·e).
//! - **Exact operations where they can be.** A split at a joint, a reversal and a split whose
//!   segment parameter is exact change no point; a transform moves each segment with its own
//!   deviation, the joints' gaps recomputed within it.
//!
//! Method: STEP's `composite_curve` (ISO 10303-42) with segments on parameter trims, joined as
//! chapter 14 asks (`projects/engines/geometry/architecture/14-2d-drafting.md`); conversion by
//! degree elevation (A5.9) and Bezier decomposition (A5.6) of L. Piegl and W. Tiller,
//! *The NURBS Book*, 2nd ed. (<https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: evaluation is a binary search over the starts plus the segment's; a bound is the bounds
//! of the segments it meets; the conversion is linear in the Bezier pieces, each O(p²·D).

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': s the starts, a and b a segment's interval, r the affine factor, t and l parameters, k a segment, p a degree"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Curve, Limit};
use geom_run::{Budget, WorkMeter};

use crate::error::CurveError;
use crate::form::check_parameter;
use crate::map::{LocalMap, MapPiece, NurbsForm, ParamMap};
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::tight::{self, point};
use crate::trimmed::Trimmed;

/// A composite curve (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Composite<const D: usize> {
    segments: Vec<Trimmed<D>>,
    starts: Vec<f64>,
    gaps: Vec<f64>,
}

fn unlimited() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// f·x, coordinate by coordinate; x itself when f is exactly 1 (a segment as long as its share of
/// the composite's parameter), so nothing widens.
fn scaled<const D: usize>(f: Interval, x: &[Interval; D]) -> Result<[Interval; D], CurveError> {
    if f.lo() == 1.0 && f.hi() == 1.0 {
        return Ok(*x);
    }
    let mut out = [point(0.0)?; D];
    for (o, &v) in out.iter_mut().zip(x) {
        *o = tight::mul(f, v)?;
    }
    Ok(out)
}

/// A box as intervals.
fn intervals<const D: usize>(b: &Bounds<D>) -> Result<[Interval; D], CurveError> {
    let mut out = [point(0.0)?; D];
    for (o, (&l, &h)) in out.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
        *o = Interval::new(l, h)?;
    }
    Ok(out)
}

impl<const D: usize> Composite<D> {
    /// The composite of `segments`, starting at parameter 0.
    ///
    /// # Errors
    ///
    /// [`CurveError::Degenerate`] for no segments or starts that do not increase;
    /// [`CurveError::Disjoint`] if a segment's end and the next one's start do not meet.
    pub fn new(segments: Vec<Trimmed<D>>) -> Result<Composite<D>, CurveError> {
        Composite::starting_at(segments, 0.0)
    }

    /// The composite of `segments`, starting at parameter `start`.
    ///
    /// # Errors
    ///
    /// As [`Composite::new`]; [`CurveError::NotFinite`] for a start that is not finite.
    pub fn starting_at(segments: Vec<Trimmed<D>>, start: f64) -> Result<Composite<D>, CurveError> {
        if !start.is_finite() {
            return Err(CurveError::NotFinite);
        }
        let mut starts = Vec::with_capacity(segments.len().saturating_add(1));
        starts.push(start);
        let mut s = start;
        for seg in &segments {
            let (a, b) = seg.interval();
            let next = s + (b - a);
            if !(next.is_finite() && next > s) {
                return Err(CurveError::Degenerate);
            }
            starts.push(next);
            s = next;
        }
        Composite::assemble(segments, starts, 0.0)
    }

    /// The composite of `segments` over `starts`, whose joints may be apart by at most `allow`
    /// beyond overlapping (a transform's deviation).
    fn assemble(
        segments: Vec<Trimmed<D>>,
        starts: Vec<f64>,
        allow: f64,
    ) -> Result<Composite<D>, CurveError> {
        if segments.is_empty() || starts.len() != segments.len().saturating_add(1) {
            return Err(CurveError::Degenerate);
        }
        for k in 1..starts.len() {
            let a: f64 = *starts.get(k.saturating_sub(1)).ok_or(CurveError::Defect)?;
            let b: f64 = *starts.get(k).ok_or(CurveError::Defect)?;
            if a >= b || a.is_nan() || b.is_nan() {
                return Err(CurveError::Degenerate);
            }
        }
        let m = &mut unlimited();
        let mut gaps = Vec::with_capacity(segments.len().saturating_sub(1));
        for k in 1..segments.len() {
            let l: &Trimmed<D> = segments
                .get(k.saturating_sub(1))
                .ok_or(CurveError::Defect)?;
            let r: &Trimmed<D> = segments.get(k).ok_or(CurveError::Defect)?;
            let end: Vec<[Interval; D]> = l.enclose(l.interval().1, Limit::FromLeft, 0, m)?;
            let start: Vec<[Interval; D]> = r.enclose(r.interval().0, Limit::FromRight, 0, m)?;
            let e: &[Interval; D] = end.first().ok_or(CurveError::Defect)?;
            let s: &[Interval; D] = start.first().ok_or(CurveError::Defect)?;
            let meet = e
                .iter()
                .zip(s)
                .all(|(x, y)| x.lo() <= y.hi() && y.lo() <= x.hi());
            let gap = tight::distance(e, s)?;
            if !meet && gap > allow {
                return Err(CurveError::Disjoint);
            }
            gaps.push(gap);
        }
        Ok(Composite {
            segments,
            starts,
            gaps,
        })
    }

    /// The segments.
    #[must_use]
    pub fn segments(&self) -> &[Trimmed<D>] {
        &self.segments
    }

    /// The starts s₀ … sₙ.
    #[must_use]
    pub fn starts(&self) -> &[f64] {
        &self.starts
    }

    /// Each joint's certified gap: an upper bound on the distance between a segment's end and the
    /// next one's start.
    #[must_use]
    pub fn gaps(&self) -> &[f64] {
        &self.gaps
    }

    fn seg(&self, k: usize) -> Result<&Trimmed<D>, CurveError> {
        self.segments.get(k).ok_or(CurveError::Defect)
    }

    fn start(&self, k: usize) -> Result<f64, CurveError> {
        self.starts.get(k).copied().ok_or(CurveError::Defect)
    }

    /// The segment of `t`, from `side` at a joint.
    fn locate(&self, t: f64, side: Limit) -> Result<usize, CurveError> {
        let n = self.segments.len();
        let mut k = self.starts.partition_point(|&s| s <= t).saturating_sub(1);
        if k >= n {
            k = n.saturating_sub(1);
        }
        if side == Limit::FromLeft && k > 0 && self.start(k)? == t {
            k = k.saturating_sub(1);
        }
        Ok(k)
    }

    /// The affine factor rₖ = (bₖ − aₖ)/(sₖ₊₁ − sₖ), enclosed.
    fn factor(&self, k: usize) -> Result<Interval, CurveError> {
        let (a, b) = self.seg(k)?.interval();
        let len = tight::sub(point(b)?, point(a)?)?;
        let span = tight::sub(
            point(self.start(k.saturating_add(1))?)?,
            point(self.start(k)?)?,
        )?;
        tight::div(len, span)
    }

    /// The segment parameter aₖ + (t − sₖ)·rₖ of `t`, enclosed and cut to the segment.
    fn local(&self, k: usize, t: f64) -> Result<Interval, CurveError> {
        let (a, b) = self.seg(k)?.interval();
        let l = tight::add(
            point(a)?,
            tight::mul(
                tight::sub(point(t)?, point(self.start(k)?)?)?,
                self.factor(k)?,
            )?,
        )?;
        let lo = if l.lo() < a { a } else { l.lo() };
        let hi = if l.hi() > b { b } else { l.hi() };
        Ok(Interval::new(lo, if hi < lo { lo } else { hi })?)
    }

    /// r^j, enclosed.
    fn power(&self, k: usize, j: usize) -> Result<Interval, CurveError> {
        let r = self.factor(k)?;
        let mut p = point(1.0)?;
        for _ in 0..j {
            p = tight::mul(p, r)?;
        }
        Ok(p)
    }

    /// The derivative of order j of segment k over its parameters `l`, enclosed.
    fn segment_over(
        &self,
        k: usize,
        l: Interval,
        j: usize,
        side: Limit,
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; D], CurveError> {
        let seg = self.seg(k)?;
        let e = if l.lo() == l.hi() {
            *seg.enclose(l.lo(), side, j, meter)?
                .get(j)
                .ok_or(CurveError::Defect)?
        } else if j == 0 {
            intervals(&seg.bound((l.lo(), l.hi()), meter)?)?
        } else {
            match seg.derivative_bound((l.lo(), l.hi()), j, meter)? {
                DerivativeBound::Bounded(b) => intervals(&b)?,
                DerivativeBound::Unbounded => return Err(CurveError::Unbounded),
            }
        };
        scaled(self.power(k, j)?, &e)
    }

    /// The segments a span meets, with each one's part of it in segment parameters.
    fn parts(&self, (lo, hi): (f64, f64)) -> Result<Vec<(usize, Interval)>, CurveError> {
        let first = self.locate(lo, Limit::FromRight)?;
        let mut last = self.locate(hi, Limit::FromLeft)?;
        if last < first {
            last = first;
        }
        let mut out = Vec::with_capacity(last.saturating_sub(first).saturating_add(1));
        for k in first..=last {
            let (s0, s1) = (self.start(k)?, self.start(k.saturating_add(1))?);
            let x = if lo > s0 { lo } else { s0 };
            let y = if hi < s1 { hi } else { s1 };
            let (lx, ly) = (self.local(k, x)?, self.local(k, if y < x { x } else { y })?);
            out.push((
                k,
                Interval::new(lx.lo(), if ly.hi() < lx.lo() { lx.lo() } else { ly.hi() })?,
            ));
        }
        Ok(out)
    }
}

impl<const D: usize> ParamCurve<D> for Composite<D> {
    fn domain(&self) -> (f64, f64) {
        (
            self.starts.first().copied().unwrap_or(0.0),
            self.starts.last().copied().unwrap_or(0.0),
        )
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        let mut out = [[0.0; D]; 1];
        self.derivatives(t, side, &mut out)?;
        let [p] = out;
        Ok(p)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        check_parameter(self.domain(), t)?;
        let k = self.locate(t, side)?;
        let l = tight::middle(self.local(k, t)?);
        self.seg(k)?.derivatives(l, side, out)?;
        let r = tight::middle(self.factor(k)?);
        let mut f = 1.0;
        for o in out.iter_mut() {
            *o = o.map(|x| x * f);
            f *= r;
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
        let k = self.locate(t, side)?;
        let l = self.local(k, t)?;
        let mut out = Vec::with_capacity(order.saturating_add(1));
        if l.lo() == l.hi() {
            let e = self.seg(k)?.enclose(l.lo(), side, order, meter)?;
            for (j, x) in e.iter().enumerate() {
                out.push(scaled(self.power(k, j)?, x)?);
            }
        } else {
            for j in 0..=order {
                out.push(self.segment_over(k, l, j, side, meter)?);
            }
        }
        Ok(out)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        crate::nurbs::check_span(span, self.domain())?;
        let mut out = Bounds {
            lo: [f64::INFINITY; D],
            hi: [f64::NEG_INFINITY; D],
        };
        for (k, l) in self.parts(span)? {
            let b = self.seg(k)?.bound((l.lo(), l.hi()), meter)?;
            for ((lo, hi), (&x, &y)) in out
                .lo
                .iter_mut()
                .zip(out.hi.iter_mut())
                .zip(b.lo.iter().zip(&b.hi))
            {
                if x < *lo {
                    *lo = x;
                }
                if y > *hi {
                    *hi = y;
                }
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
        for (k, l) in self.parts(span)? {
            let e = match self.segment_over(k, l, order, Limit::FromRight, meter) {
                Ok(e) => e,
                Err(CurveError::Unbounded) => return Ok(DerivativeBound::Unbounded),
                Err(e) => return Err(e),
            };
            for ((lo, hi), x) in out.lo.iter_mut().zip(out.hi.iter_mut()).zip(&e) {
                if x.lo() < *lo {
                    *lo = x.lo();
                }
                if x.hi() > *hi {
                    *hi = x.hi();
                }
            }
        }
        Ok(DerivativeBound::Bounded(out))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        join(self, meter)
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (a, b) = self.domain();
        if !(a < t && t < b) {
            return Err(CurveError::ParameterOutside);
        }
        let k = self.locate(t, Limit::FromRight)?;
        let starts_left = |upto: usize, end: f64| -> Result<Vec<f64>, CurveError> {
            let mut s: Vec<f64> = self.starts.get(..=upto).ok_or(CurveError::Defect)?.to_vec();
            if let Some(x) = s.last_mut() {
                *x = end;
            }
            Ok(s)
        };
        if self.start(k)? == t {
            // At a joint: no segment is cut.
            let left = Composite::assemble(
                self.segments.get(..k).ok_or(CurveError::Defect)?.to_vec(),
                self.starts.get(..=k).ok_or(CurveError::Defect)?.to_vec(),
                f64::INFINITY,
            )?;
            let right = Composite::assemble(
                self.segments.get(k..).ok_or(CurveError::Defect)?.to_vec(),
                self.starts.get(k..).ok_or(CurveError::Defect)?.to_vec(),
                f64::INFINITY,
            )?;
            return Ok((
                Changed {
                    value: CurveKind::Composite(left),
                    deviation: 0.0,
                },
                Changed {
                    value: CurveKind::Composite(right),
                    deviation: 0.0,
                },
            ));
        }
        let seg = self.seg(k)?;
        let (sa, sb) = seg.interval();
        let l = self.local(k, t)?;
        let cut = tight::middle(l);
        if !(sa < cut && cut < sb) {
            return Err(CurveError::Degenerate);
        }
        let (h1, h2) = seg.halves(cut)?;
        // The cut moved by at most |cut − l| in the segment's parameter.
        let moved = {
            let off = if cut - l.lo() > l.hi() - cut {
                cut - l.lo()
            } else {
                l.hi() - cut
            };
            if off == 0.0 {
                0.0
            } else {
                let speed = match seg.derivative_bound((sa, sb), 1, meter)? {
                    DerivativeBound::Bounded(b) => {
                        let mut m = Vec::with_capacity(D);
                        for (&x, &y) in b.lo.iter().zip(&b.hi) {
                            m.push(point(if x.abs() > y.abs() { x.abs() } else { y.abs() })?);
                        }
                        tight::norm(&m)?
                    }
                    DerivativeBound::Unbounded => return Err(CurveError::Unbounded),
                };
                tight::mul(speed, point(off)?)?.hi()
            }
        };
        let mut left_segments: Vec<Trimmed<D>> =
            self.segments.get(..k).ok_or(CurveError::Defect)?.to_vec();
        left_segments.push(h1);
        let mut left_starts = starts_left(k, self.start(k)?)?;
        left_starts.push(t);
        let mut right_segments = alloc::vec![h2];
        right_segments.extend(
            self.segments
                .get(k.saturating_add(1)..)
                .ok_or(CurveError::Defect)?
                .iter()
                .cloned(),
        );
        let mut right_starts = alloc::vec![t];
        right_starts.extend(
            self.starts
                .get(k.saturating_add(1)..)
                .ok_or(CurveError::Defect)?
                .iter()
                .copied(),
        );
        let left = Composite::assemble(left_segments, left_starts, f64::INFINITY)?;
        let right = Composite::assemble(right_segments, right_starts, f64::INFINITY)?;
        Ok((
            Changed {
                value: CurveKind::Composite(left),
                deviation: moved,
            },
            Changed {
                value: CurveKind::Composite(right),
                deviation: moved,
            },
        ))
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let mut segments = Vec::with_capacity(self.segments.len());
        let mut deviation = 0.0;
        for s in self.segments.iter().rev() {
            let r = s.reverse(meter)?;
            if r.deviation > deviation {
                deviation = r.deviation;
            }
            segments.push(r.value);
        }
        let starts: Vec<f64> = self.starts.iter().rev().map(|s| -s).collect();
        let c = Composite::assemble(segments, starts, f64::INFINITY)?;
        Ok(Changed {
            value: CurveKind::Composite(c),
            deviation,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let mut segments = Vec::with_capacity(self.segments.len());
        let mut deviation = 0.0;
        for s in &self.segments {
            let r = s.moved(transform, meter)?;
            if r.deviation > deviation {
                deviation = r.deviation;
            }
            segments.push(r.value);
        }
        let c = Composite::assemble(segments, self.starts.clone(), f64::INFINITY)?;
        Ok(Changed {
            value: CurveKind::Composite(c),
            deviation,
        })
    }
}

/// The local map of a segment's NURBS form.
fn local_map(m: ParamMap) -> Result<LocalMap, CurveError> {
    match m {
        ParamMap::Same => Ok(LocalMap::Same),
        ParamMap::Circular(b) => Ok(LocalMap::Circular(b)),
        ParamMap::Hyperbolic(b) => Ok(LocalMap::Hyperbolic(b)),
        ParamMap::Pieces(_) => Err(CurveError::Defect),
    }
}

/// An upper bound on the norm of a curve's first derivative over its whole domain.
fn speed<const D: usize>(c: &Curve<D>, meter: &mut WorkMeter<'_>) -> Result<Interval, CurveError> {
    match ParamCurve::derivative_bound(c, ParamCurve::domain(c), 1, meter)? {
        DerivativeBound::Bounded(b) => {
            let mut m = Vec::with_capacity(D);
            for (&x, &y) in b.lo.iter().zip(&b.hi) {
                m.push(point(if x.abs() > y.abs() { x.abs() } else { y.abs() })?);
            }
            tight::norm(&m)
        }
        DerivativeBound::Unbounded => Err(CurveError::Unbounded),
    }
}

/// The Euclidean point of homogeneous data, enclosed.
fn euclid<const D: usize>(h: &[f64; D], w: f64) -> Result<[Interval; D], CurveError> {
    let mut out = [point(0.0)?; D];
    for (o, &x) in out.iter_mut().zip(h) {
        *o = tight::div(point(x)?, point(w)?)?;
    }
    Ok(out)
}

/// One NURBS curve from a composite's segments (module documentation).
#[allow(
    clippy::too_many_lines,
    reason = "one construction: convert, elevate and extract each segment, then lay the pieces end to end"
)]
fn join<const D: usize>(
    c: &Composite<D>,
    meter: &mut WorkMeter<'_>,
) -> Result<Changed<NurbsForm<D>>, CurveError> {
    // Each segment's NURBS form, its local map and its deviation so far.
    let mut forms: Vec<(Curve<D>, LocalMap, f64)> = Vec::with_capacity(c.segments.len());
    for s in &c.segments {
        let n = s.to_nurbs(meter)?;
        forms.push((n.value.curve, local_map(n.value.map)?, n.deviation));
    }
    let p = forms
        .iter()
        .map(|(f, _, _)| f.degree())
        .fold(1, |a, b| if b > a { b } else { a });
    let rational = forms.iter().any(|(f, _, _)| f.is_rational());
    let mut homs: Vec<[f64; D]> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();
    let mut knots: Vec<f64> = Vec::new();
    let mut pieces_map: Vec<MapPiece> = Vec::with_capacity(forms.len());
    let mut deviation = 0.0;
    let mut previous_end: Option<f64> = None;
    for (k, (form, map, dev0)) in forms.into_iter().enumerate() {
        let mut dev = point(dev0)?;
        let curve = if form.degree() < p {
            let e = form.elevate(
                p.checked_sub(form.degree()).ok_or(CurveError::Defect)?,
                meter,
            )?;
            dev = tight::add(dev, point(e.deviation)?)?;
            e.value
        } else {
            form
        };
        // A form with no interior knot is one Bezier piece already: no extraction, no rounding.
        let knots_of = curve.basis().knots();
        let single = knots_of
            .windows(2)
            .filter(|w| w.first() != w.get(1))
            .count()
            == 1;
        let pieces = if single {
            Changed {
                value: alloc::vec![curve.clone()],
                deviation: 0.0,
            }
        } else {
            let b = curve.to_bezier(meter)?;
            Changed {
                value: b.value,
                deviation: b.deviation,
            }
        };
        dev = tight::add(dev, point(pieces.deviation)?)?;
        let first_knot = ParamCurve::domain(&curve).0;
        let start_u = previous_end.unwrap_or(first_knot);
        let shift = start_u - first_knot;
        let (a, b) = c.seg(k)?.interval();
        pieces_map.push(MapPiece {
            start: c.start(k)?,
            end: c.start(k.saturating_add(1))?,
            local: (a, b),
            shift,
            map,
        });
        // Rescale this segment's weights so its first meets the joint's.
        // A non-rational piece's weights are all 1.
        let first = pieces.value.first().map_or(1.0, |q| {
            q.weights().and_then(|w| w.first().copied()).unwrap_or(1.0)
        });
        let (scale, scaled) = match weights.last() {
            Some(&prev) if prev != first => (prev / first, true),
            _ => (1.0, false),
        };
        let mut g_start = start_u;
        let mut reach = 0.0;
        let mut eps = 0.0;
        let mut moved = 0.0;
        let mut reparam = 0.0;
        let mut origin: Option<[Interval; D]> = None;
        for (j, piece) in pieces.value.iter().enumerate() {
            let (alpha, beta) = ParamCurve::domain(piece);
            let g_end = beta + shift;
            if g_end <= g_start || g_end.is_nan() {
                return Err(CurveError::Degenerate);
            }
            // The rounding of the piece's shifted ends reparametrises it: |N′|·(e₁ + e₂).
            let e1 = tight::magnitude(tight::sub(
                point(g_start)?,
                tight::add(point(alpha)?, point(shift)?)?,
            )?);
            let e2 = tight::magnitude(tight::sub(
                point(g_end)?,
                tight::add(point(beta)?, point(shift)?)?,
            )?);
            if e1 != 0.0 || e2 != 0.0 {
                let v = speed(piece, meter)?;
                let term = tight::mul(v, tight::add(point(e1)?, point(e2)?)?)?.hi();
                if term > reparam {
                    reparam = term;
                }
            }
            let ph = piece.homogeneous();
            let pw: Vec<f64> = match piece.weights() {
                Some(w) => w.to_vec(),
                None => alloc::vec![1.0; ph.len()],
            };
            for (i, (h, &w)) in ph.iter().zip(&pw).enumerate() {
                // A piece's first point is the previous piece's last (already stored); a
                // segment's first point is replaced by the previous segment's last.
                let shared = i == 0 && j > 0;
                let joint = i == 0 && j == 0 && !homs.is_empty();
                if shared {
                    continue;
                }
                let (hs, ws) = if joint {
                    match (homs.last(), weights.last()) {
                        (Some(h), Some(&w)) => (*h, w),
                        _ => return Err(CurveError::Defect),
                    }
                } else if scaled {
                    (h.map(|x| x * scale), w * scale)
                } else {
                    (*h, w)
                };
                // The stored point against this segment's exact one: the joint's gap, and the
                // rounding of the rescaled points.
                let exact = euclid(h, w)?;
                let d = tight::distance(&euclid(&hs, ws)?, &exact)?;
                if d > moved {
                    moved = d;
                }
                if rational {
                    let r = tight::magnitude(tight::sub(
                        tight::div(point(ws)?, tight::mul(point(scale)?, point(w)?)?)?,
                        point(1.0)?,
                    )?);
                    if r > eps {
                        eps = r;
                    }
                    let o = *origin.get_or_insert(exact);
                    let rr = tight::distance(&exact, &o)?;
                    if rr > reach {
                        reach = rr;
                    }
                }
                if !joint {
                    homs.push(hs);
                    weights.push(ws);
                }
            }
            if knots.is_empty() {
                knots.extend(core::iter::repeat_n(g_start, p.saturating_add(1)));
            }
            knots.extend(core::iter::repeat_n(g_end, p));
            g_start = g_end;
        }
        if eps > 0.0 {
            if eps.is_nan() || eps >= 0.5 {
                return Err(CurveError::NotFinite);
            }
            let e = point(eps)?;
            let factor = tight::div(tight::mul(point(2.0)?, e)?, tight::sub(point(1.0)?, e)?)?;
            moved = tight::add(point(moved)?, tight::mul(factor, point(reach)?)?)?.hi();
        }
        dev = tight::add(dev, point(moved)?)?;
        dev = tight::add(dev, point(reparam)?)?;
        if dev.hi() > deviation {
            deviation = dev.hi();
        }
        previous_end = Some(g_start);
    }
    if let Some(&last) = knots.last() {
        knots.push(last);
    }
    let curve = Curve::from_homogeneous(
        Basis::new(p, knots)?,
        homs,
        if rational { Some(weights) } else { None },
    )?;
    Ok(Changed {
        value: NurbsForm {
            curve,
            map: ParamMap::Pieces(pieces_map),
        },
        deviation,
    })
}
