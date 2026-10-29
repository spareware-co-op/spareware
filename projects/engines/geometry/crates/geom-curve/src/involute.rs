//! The circular involute kind: the path of the end of a taut string unwound from a circle,
//! c + r·(cos θ + θ·sin θ)·x̂ + r·(sin θ − θ·cos θ)·ŷ with θ = πt, t in half-turns.
//!
//! What it does: [`Involute`] stores the base circle's center, radius and frame (the involute
//! leaves the circle at x̂ when t = 0 and unwinds towards ŷ for t > 0, the other way for t < 0), or,
//! as the affine image of one, the vectors u and v themselves ([`Axes`]). The point t = 0 is a cusp:
//! the derivative vanishes there, and the bounds say so. Gear teeth flanks are its common use.
//!
//! Guarantees:
//!
//! - **Exact.** The stored doubles define the involute; evaluation is certified through the shared
//!   analytic form (`crate::form`, shape `Involute`), whose derivatives of every order are closed
//!   forms.
//! - **A certified NURBS form.** The involute is not a rational curve; it converts as the helix
//!   does, by Taylor pieces of degree 9 an eighth of a turn wide on the same parameter
//!   (`crate::taylor`), the remainder bounded by its tenth derivative.
//! - **Transforms change it explicitly.** A similarity keeps it the involute of a circle (radius
//!   times the scale); any other transform gives the `Vectors` form on the same parameter.
//! - **Exact split and reversal.**
//!
//! Method: the involute of a circle in its standard parametrisation by the unwinding angle (for
//! gears, F. L. Litvin and A. Fuentes, *Gear Geometry and Applied Theory*, 2nd ed., Cambridge
//! (2004), <https://doi.org/10.1017/CBO9780511547126>), with angles in half-turns.
//!
//! Cost: constant per evaluation; conversion O(n·p²) for n eighths of a turn.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::axes::Axes;
use crate::error::CurveError;
use crate::form::{self, Form, Shape};
use crate::frame::Frame;
use crate::helix::{TAYLOR_DEGREE, TAYLOR_WIDTH};
use crate::map::NurbsForm;
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::taylor;

/// An involute of a circle, or its affine image (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Involute<const D: usize> {
    center: [f64; D],
    axes: Axes<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Involute<D> {
    /// The involute of the circle about `center` of `radius` on `frame`, over `domain`
    /// (half-turns of unwinding).
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Degenerate`] unless the radius is positive;
    /// [`CurveError::SpanOutside`] unless the domain is finite and ordered.
    pub fn new(
        center: [f64; D],
        radius: f64,
        frame: Frame<D>,
        domain: (f64, f64),
    ) -> Result<Involute<D>, CurveError> {
        Involute::with_axes(
            center,
            Axes::Frame {
                frame,
                a: radius,
                b: radius,
            },
            domain,
        )
    }

    /// The involute c + f₁(t)·u + f₂(t)·v with any axes.
    ///
    /// # Errors
    ///
    /// As [`Involute::new`]; [`CurveError::Degenerate`] for parallel u and v.
    pub fn with_axes(
        center: [f64; D],
        axes: Axes<D>,
        domain: (f64, f64),
    ) -> Result<Involute<D>, CurveError> {
        if !center.iter().all(|x| x.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        form::check_domain(domain)?;
        axes.check()?;
        let form = axes.form(Shape::Involute, center)?;
        Ok(Involute {
            center,
            axes,
            domain,
            form,
        })
    }

    /// The base circle's center.
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

impl<const D: usize> ParamCurve<D> for Involute<D> {
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
            value: CurveKind::Involute(Involute {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        taylor::same_parameter(self, TAYLOR_DEGREE, TAYLOR_WIDTH, meter)
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let i = Involute::with_axes(self.center, self.axes.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Involute(i),
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
        let i = Involute::with_axes(
            m.point(self.center)?,
            self.axes.moved(&self.form, &m)?,
            self.domain,
        )?;
        let deviation = form::distance(Shape::Involute, self.domain, &i.form.exact, &image)?;
        Ok(Changed {
            value: CurveKind::Involute(i),
            deviation,
        })
    }
}
