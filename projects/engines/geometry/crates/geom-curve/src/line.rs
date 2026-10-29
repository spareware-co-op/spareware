//! The line kind: a segment, a ray or an infinite line, P(t) = o + t·d.
//!
//! What it does: [`Line`] stores an origin o, a direction d (any nonzero vector) and a domain whose
//! ends may be infinite: finite for a segment, one infinite end for a ray, both for an infinite
//! line ([`Extent`]). It implements the kind contract through the shared analytic form
//! (`crate::form`, shape `Linear`). A ray or infinite line has no finite NURBS form and no bound
//! over an infinite span; both are refused, never clamped.
//!
//! Guarantees:
//!
//! - **Exact storage.** The doubles o and d define the line; [`Line::segment`] from two points says
//!   how far its stored direction is from the exact difference (0 when it is exact, as it is for
//!   grid points).
//! - **Exact operations.** Split and reversal (o kept, d negated, the domain negated) change no
//!   bit; conversion of a segment is the degree-1 NURBS curve through its ends.
//! - **Bounded or refused.** A transform of a ray or infinite line whose direction's image is not
//!   exact would drift without bound along it, so it is refused with
//!   [`CurveError::Unbounded`]; trim the line to a segment first.
//!
//! Method: the parametric line of ISO 10303-42 (`line`: a point and a vector), with the segment,
//! ray and infinite line distinguished by the domain, as DXF's LINE, RAY and XLINE are; the kinds
//! chapter 7 lists (`projects/engines/geometry/architecture/07-geometry-types.md`).
//!
//! Cost: constant per evaluation; conversion and transforms constant.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::{self, Form, Shape};
use crate::map::NurbsForm;
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::taylor;
use crate::tight::{self, point};

/// Which ends of a line's domain are infinite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extent {
    /// Both ends finite.
    Segment,
    /// One end infinite.
    Ray,
    /// Both ends infinite.
    Infinite,
}

/// A segment, ray or infinite line, P(t) = o + t·d (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Line<const D: usize> {
    origin: [f64; D],
    direction: [f64; D],
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Line<D> {
    /// The line through `origin` along `direction` over `domain`, whose ends may be infinite.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if the origin or direction is not finite;
    /// [`CurveError::Degenerate`] if the direction is zero; [`CurveError::SpanOutside`] unless
    /// lo < hi, neither is NaN, lo is not +∞ and hi is not −∞.
    pub fn new(
        origin: [f64; D],
        direction: [f64; D],
        domain: (f64, f64),
    ) -> Result<Line<D>, CurveError> {
        let form = Form::exact_vectors(Shape::Linear, [origin, direction, [0.0; D], [0.0; D]])?;
        if direction.iter().all(|&x| x == 0.0) {
            return Err(CurveError::Degenerate);
        }
        let (lo, hi) = domain;
        if !(lo < hi && lo < f64::INFINITY && hi > f64::NEG_INFINITY) {
            return Err(CurveError::SpanOutside);
        }
        Ok(Line {
            origin,
            direction,
            domain,
            form,
        })
    }

    /// The segment from `a` (t = 0) to `b` (t = 1), with the direction b − a rounded, and the
    /// certified distance from the exact segment (0 when b − a is exact).
    ///
    /// # Errors
    ///
    /// As [`Line::new`].
    pub fn segment(a: [f64; D], b: [f64; D]) -> Result<Changed<Line<D>>, CurveError> {
        let mut d = [0.0; D];
        let mut exact = Vec::with_capacity(D);
        for ((o, &x), &y) in d.iter_mut().zip(&a).zip(&b) {
            *o = y - x;
            exact.push(tight::sub(point(y)?, point(x)?)?);
        }
        let line = Line::new(a, d, (0.0, 1.0))?;
        // |o + t·d − (a + t·(b − a))| = t·|d − (b − a)|, largest at t = 1.
        let deviation = tight::distance_to(&d, &exact)?;
        Ok(Changed {
            value: line,
            deviation,
        })
    }

    /// The ray from `origin` along `direction`: t ≥ 0.
    ///
    /// # Errors
    ///
    /// As [`Line::new`].
    pub fn ray(origin: [f64; D], direction: [f64; D]) -> Result<Line<D>, CurveError> {
        Line::new(origin, direction, (0.0, f64::INFINITY))
    }

    /// The infinite line through `origin` along `direction`.
    ///
    /// # Errors
    ///
    /// As [`Line::new`].
    pub fn infinite(origin: [f64; D], direction: [f64; D]) -> Result<Line<D>, CurveError> {
        Line::new(origin, direction, (f64::NEG_INFINITY, f64::INFINITY))
    }

    /// The origin o.
    #[must_use]
    pub fn origin(&self) -> [f64; D] {
        self.origin
    }

    /// The direction d.
    #[must_use]
    pub fn direction(&self) -> [f64; D] {
        self.direction
    }

    /// Which ends are infinite.
    #[must_use]
    pub fn extent(&self) -> Extent {
        match (self.domain.0.is_finite(), self.domain.1.is_finite()) {
            (true, true) => Extent::Segment,
            (false, false) => Extent::Infinite,
            _ => Extent::Ray,
        }
    }
}

impl<const D: usize> ParamCurve<D> for Line<D> {
    fn domain(&self) -> (f64, f64) {
        self.domain
    }

    fn point(&self, t: f64, _side: Limit) -> Result<[f64; D], CurveError> {
        form::point_at(&self.form, self.domain, t)
    }

    fn derivatives(&self, t: f64, _side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        form::derivatives_at(&self.form, self.domain, t, out)
    }

    fn enclose(
        &self,
        t: f64,
        _side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        form::enclose_at(&self.form, self.domain, t, order, meter)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        form::bound(&self.form, self.domain, span, meter)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        form::derivative_bound(&self.form, self.domain, span, order, meter)
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        taylor::same_parameter(self, 1, f64::INFINITY, meter)
    }

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Line(Line {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let line = Line::new(self.origin, self.direction.map(|x| -x), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Line(line),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let m = Motion::of::<D>(transform)?;
        meter.charge(1)?;
        let line = Line::new(
            m.point(self.origin)?,
            m.vector(self.direction)?,
            self.domain,
        )?;
        let image = m.form_image(&self.form)?;
        let deviation = if self.extent() == Extent::Segment {
            form::distance(Shape::Linear, self.domain, &line.form.exact, &image)?
        } else {
            let [c, u, _, _] = &line.form.exact;
            let [ic, iu, _, _] = &image;
            if tight::distance(u, iu)? != 0.0 {
                return Err(CurveError::Unbounded);
            }
            tight::distance(c, ic)?
        };
        Ok(Changed {
            value: CurveKind::Line(line),
            deviation,
        })
    }
}
