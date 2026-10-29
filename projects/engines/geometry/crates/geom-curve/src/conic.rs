//! The conic kinds: circle and arc, ellipse and elliptical arc, hyperbola and parabola, each stored
//! as itself.
//!
//! What it does: [`Circle`] is c + r·cos πt·x̂ + r·sin πt·ŷ on an exact [`Frame`], t in half-turns
//! over at most one turn (a full circle is [0, 2]). [`Ellipse`] is c + cos πt·u + sin πt·v, by its
//! parameter (the eccentric angle), not by polar angle (chapter 7), with u and v principal semi-axes
//! on a frame or any conjugate pair ([`Axes`]). [`Hyperbola`] is c + cosh t·u + sinh t·v, one
//! branch. [`Parabola`] is c + t²·u + t·v; on a frame with u = f·x̂ and v = 2f·ŷ it is the parabola
//! of focal length f with its vertex at c, as ISO 10303-42 defines it. Each implements the kind
//! contract through the shared analytic form (`crate::form`).
//!
//! Guarantees:
//!
//! - **Stored as themselves.** An arc stays an arc: its center, radius and frame are the stored
//!   doubles, exactly (chapter 7).
//! - **Exact NURBS forms.** Circles, ellipses and hyperbolas convert to rational quadratics on the
//!   tangent half-angle parameter (`crate::rational`), parabolas to a quadratic on the same
//!   parameter; each with a certified deviation, 0 when the doubles hold the conversion exactly.
//! - **Transforms change kinds explicitly.** A similarity keeps a circle a circle (radius times its
//!   scale, frame carried); any other transform makes it an [`Ellipse`] with conjugate axes, on the
//!   same parameter; principal axes on a frame stay principal under a similarity.
//! - **Exact split and reversal.** A split changes only the domain; a reversal negates the domain
//!   and v (a frame's normal), exactly.
//!
//! Method: the parametric conics of ISO 10303-42 (`circle`, `ellipse`, `hyperbola`, `parabola`),
//! with angles in half-turns, the kernel's proven angle form (chapter 6); their conversion from
//! L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., chapter 7
//! (<https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: constant per evaluation; conversion O(n) in its pieces (at most 5 for a circle or
//! ellipse, 1024 for a hyperbola).

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::axes::Axes;
use crate::error::CurveError;
use crate::form::{self, Form, Shape};
use crate::frame::Frame;
use crate::map::NurbsForm;
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::rational;
use crate::taylor;

/// The largest |t| of a hyperbola's domain: cosh 512 is below 2^739, far from overflow.
pub const HYPERBOLA_REACH: f64 = 512.0;

/// Finite coordinates.
fn check_point<const D: usize>(p: &[f64; D]) -> Result<(), CurveError> {
    if p.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(CurveError::NotFinite)
    }
}

/// A circle or circular arc (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Circle<const D: usize> {
    center: [f64; D],
    radius: f64,
    frame: Frame<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Circle<D> {
    /// The arc of the circle about `center` of `radius` on `frame` over `domain`, in half-turns
    /// from x̂ towards ŷ, at most one turn.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Degenerate`] unless the radius is positive;
    /// [`CurveError::SpanOutside`] unless the domain is finite, ordered and at most 2 long.
    pub fn new(
        center: [f64; D],
        radius: f64,
        frame: Frame<D>,
        domain: (f64, f64),
    ) -> Result<Circle<D>, CurveError> {
        check_point(&center)?;
        form::check_turn(domain)?;
        let axes = Axes::Frame {
            frame,
            a: radius,
            b: radius,
        };
        axes.check()?;
        let form = axes.form(Shape::Trig, center)?;
        Ok(Circle {
            center,
            radius,
            frame,
            domain,
            form,
        })
    }

    /// The full circle, t in [0, 2].
    ///
    /// # Errors
    ///
    /// As [`Circle::new`].
    pub fn full(center: [f64; D], radius: f64, frame: Frame<D>) -> Result<Circle<D>, CurveError> {
        Circle::new(center, radius, frame, (0.0, 2.0))
    }

    /// The center.
    #[must_use]
    pub fn center(&self) -> [f64; D] {
        self.center
    }

    /// The radius.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The frame.
    #[must_use]
    pub fn frame(&self) -> Frame<D> {
        self.frame
    }
}

/// An ellipse or elliptical arc (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Ellipse<const D: usize> {
    center: [f64; D],
    axes: Axes<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Ellipse<D> {
    /// The arc of the ellipse c + cos πt·u + sin πt·v over `domain`, at most one turn.
    ///
    /// # Errors
    ///
    /// As [`Circle::new`]; [`CurveError::Degenerate`] if the axes' lengths are not positive or
    /// their vectors are parallel.
    pub fn new(
        center: [f64; D],
        axes: Axes<D>,
        domain: (f64, f64),
    ) -> Result<Ellipse<D>, CurveError> {
        check_point(&center)?;
        form::check_turn(domain)?;
        axes.check()?;
        let form = axes.form(Shape::Trig, center)?;
        Ok(Ellipse {
            center,
            axes,
            domain,
            form,
        })
    }

    /// The center.
    #[must_use]
    pub fn center(&self) -> [f64; D] {
        self.center
    }

    /// The axes.
    #[must_use]
    pub fn axes(&self) -> Axes<D> {
        self.axes
    }
}

/// One branch of a hyperbola (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Hyperbola<const D: usize> {
    center: [f64; D],
    axes: Axes<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Hyperbola<D> {
    /// The arc of the hyperbola branch c + cosh t·u + sinh t·v over `domain`.
    ///
    /// # Errors
    ///
    /// As [`Ellipse::new`], with the domain finite, ordered and inside
    /// [−[`HYPERBOLA_REACH`], [`HYPERBOLA_REACH`]] instead of one turn.
    pub fn new(
        center: [f64; D],
        axes: Axes<D>,
        domain: (f64, f64),
    ) -> Result<Hyperbola<D>, CurveError> {
        check_point(&center)?;
        form::check_domain(domain)?;
        if !(-HYPERBOLA_REACH <= domain.0 && domain.1 <= HYPERBOLA_REACH) {
            return Err(CurveError::SpanOutside);
        }
        axes.check()?;
        let form = axes.form(Shape::Hyperbolic, center)?;
        Ok(Hyperbola {
            center,
            axes,
            domain,
            form,
        })
    }

    /// The center.
    #[must_use]
    pub fn center(&self) -> [f64; D] {
        self.center
    }

    /// The axes.
    #[must_use]
    pub fn axes(&self) -> Axes<D> {
        self.axes
    }
}

/// An arc of a parabola (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Parabola<const D: usize> {
    vertex: [f64; D],
    axes: Axes<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Parabola<D> {
    /// The arc of the parabola c + t²·u + t·v over `domain`.
    ///
    /// # Errors
    ///
    /// As [`Ellipse::new`], with the domain finite and ordered instead of one turn.
    pub fn new(
        vertex: [f64; D],
        axes: Axes<D>,
        domain: (f64, f64),
    ) -> Result<Parabola<D>, CurveError> {
        check_point(&vertex)?;
        form::check_domain(domain)?;
        axes.check()?;
        let form = axes.form(Shape::Parabolic, vertex)?;
        Ok(Parabola {
            vertex,
            axes,
            domain,
            form,
        })
    }

    /// The parabola of focal length `focal` with its vertex at `vertex`, opening along x̂:
    /// c + f·(t²·x̂ + 2t·ŷ).
    ///
    /// # Errors
    ///
    /// As [`Parabola::new`].
    pub fn focal(
        vertex: [f64; D],
        focal: f64,
        frame: Frame<D>,
        domain: (f64, f64),
    ) -> Result<Parabola<D>, CurveError> {
        Parabola::new(
            vertex,
            Axes::Frame {
                frame,
                a: focal,
                b: 2.0 * focal,
            },
            domain,
        )
    }

    /// The vertex.
    #[must_use]
    pub fn vertex(&self) -> [f64; D] {
        self.vertex
    }

    /// The axes.
    #[must_use]
    pub fn axes(&self) -> Axes<D> {
        self.axes
    }
}

impl<const D: usize> ParamCurve<D> for Circle<D> {
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

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Circle(Circle {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        rational::conic(&self.form, self.domain, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let c = Circle::new(self.center, self.radius, self.frame.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Circle(c),
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
        let image = m.form_image(&self.form)?;
        let center = m.point(self.center)?;
        let (value, exact) = if m.similar() {
            let frame = Frame::new(m.vector(self.frame.x())?, m.normal(self.frame.normal())?)?;
            let c = Circle::new(center, m.scale * self.radius, frame, self.domain)?;
            let exact = c.form.exact;
            (CurveKind::Circle(c), exact)
        } else {
            let axes = Axes::Frame {
                frame: self.frame,
                a: self.radius,
                b: self.radius,
            }
            .moved(&self.form, &m)?;
            let e = Ellipse::new(center, axes, self.domain)?;
            let exact = e.form.exact;
            (CurveKind::Ellipse(e), exact)
        };
        let deviation = form::distance(Shape::Trig, self.domain, &exact, &image)?;
        Ok(Changed { value, deviation })
    }
}

impl<const D: usize> ParamCurve<D> for Ellipse<D> {
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

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Ellipse(Ellipse {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        rational::conic(&self.form, self.domain, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let e = Ellipse::new(self.center, self.axes.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Ellipse(e),
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
        let image = m.form_image(&self.form)?;
        let e = Ellipse::new(
            m.point(self.center)?,
            self.axes.moved(&self.form, &m)?,
            self.domain,
        )?;
        let deviation = form::distance(Shape::Trig, self.domain, &e.form.exact, &image)?;
        Ok(Changed {
            value: CurveKind::Ellipse(e),
            deviation,
        })
    }
}

impl<const D: usize> ParamCurve<D> for Hyperbola<D> {
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

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Hyperbola(Hyperbola {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        rational::conic(&self.form, self.domain, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let h = Hyperbola::new(self.center, self.axes.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Hyperbola(h),
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
        let image = m.form_image(&self.form)?;
        let h = Hyperbola::new(
            m.point(self.center)?,
            self.axes.moved(&self.form, &m)?,
            self.domain,
        )?;
        let deviation = form::distance(Shape::Hyperbolic, self.domain, &h.form.exact, &image)?;
        Ok(Changed {
            value: CurveKind::Hyperbola(h),
            deviation,
        })
    }
}

impl<const D: usize> ParamCurve<D> for Parabola<D> {
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

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Parabola(Parabola {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        taylor::same_parameter(self, 2, f64::INFINITY, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let p = Parabola::new(self.vertex, self.axes.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Parabola(p),
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
        let image = m.form_image(&self.form)?;
        let p = Parabola::new(
            m.point(self.vertex)?,
            self.axes.moved(&self.form, &m)?,
            self.domain,
        )?;
        let deviation = form::distance(Shape::Parabolic, self.domain, &p.form.exact, &image)?;
        Ok(Changed {
            value: CurveKind::Parabola(p),
            deviation,
        })
    }
}
