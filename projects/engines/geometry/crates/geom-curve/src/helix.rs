//! The helix kind, exactly: c + r·cos πt·x̂ + r·sin πt·ŷ + t·(pitch/2)·n̂, t in half-turns.
//!
//! What it does: [`Helix`] stores a center on its axis, a frame whose normal is the axis, a radius
//! and a pitch (the rise per full turn, positive for a right-handed helix), or, as the affine image
//! of one, the vectors u, v and w themselves ([`HelixAxes`]). It is a 3D kind; any length of
//! domain, many turns included. Neither STEP nor IFC has a helix (chapter 7), so it is stored as
//! itself here and exchanged as a NURBS approximation with its certified error.
//!
//! Guarantees:
//!
//! - **Exact.** The stored doubles define the helix; evaluation is certified through the shared
//!   analytic form (`crate::form`, shape `Helical`).
//! - **A certified NURBS form.** A helix is not a rational curve, so it converts by Taylor pieces
//!   of degree 9, each an eighth of a turn, on the same parameter, with the remainder bounded by
//!   its tenth derivative (`crate::taylor`): about 2.4·10⁻¹⁴ times (|u| + |v|) per piece, plus
//!   rounding.
//! - **Transforms change it explicitly.** A similarity keeps it a circular helix (radius and pitch
//!   times the scale, the pitch negated by a mirror, which turns a right-handed helix into a
//!   left-handed one); any other transform gives the `Vectors` form on the same parameter.
//! - **Exact split and reversal.** Reversal negates v and w (the frame's normal) and the domain,
//!   and keeps the pitch: a helix traversed backwards keeps its hand.
//!
//! Method: the circular helix in its standard parametrisation, with angles in half-turns, stored
//! exactly as chapter 7 asks (`projects/engines/geometry/architecture/07-geometry-types.md`: "Helix (exact; neither
//! STEP nor IFC has one)").
//!
//! Cost: constant per evaluation; conversion O(n·p²) for n eighths of a turn.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::axes::HelixAxes;
use crate::error::CurveError;
use crate::form::{self, Form, Shape};
use crate::map::NurbsForm;
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::taylor;

/// The degree of a helix's or involute's NURBS form.
pub(crate) const TAYLOR_DEGREE: usize = 9;
/// The width of each piece of that form, in half-turns: an eighth of a turn.
pub(crate) const TAYLOR_WIDTH: f64 = 0.125;

/// A helix (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Helix<const D: usize> {
    center: [f64; D],
    axes: HelixAxes<D>,
    domain: (f64, f64),
    form: Form<D>,
}

impl<const D: usize> Helix<D> {
    /// The helix about `center` with `axes` over `domain` (half-turns).
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 3; [`CurveError::NotFinite`];
    /// [`CurveError::Degenerate`] for a radius that is not positive, a zero pitch, or parallel
    /// u and v; [`CurveError::SpanOutside`] unless the domain is finite and ordered.
    pub fn new(
        center: [f64; D],
        axes: HelixAxes<D>,
        domain: (f64, f64),
    ) -> Result<Helix<D>, CurveError> {
        if D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        if !center.iter().all(|x| x.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        form::check_domain(domain)?;
        axes.check()?;
        let form = axes.form(center)?;
        Ok(Helix {
            center,
            axes,
            domain,
            form,
        })
    }

    /// The center (the point of the axis at t = 0).
    #[must_use]
    pub fn center(&self) -> [f64; D] {
        self.center
    }

    /// The axes.
    #[must_use]
    pub fn axes(&self) -> HelixAxes<D> {
        self.axes
    }
}

impl<const D: usize> ParamCurve<D> for Helix<D> {
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
            value: CurveKind::Helix(Helix {
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
        let h = Helix::new(self.center, self.axes.reversed(), (-b, -a))?;
        Ok(Changed {
            value: CurveKind::Helix(h),
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
        let h = Helix::new(
            m.point(self.center)?,
            self.axes.moved(&self.form, &m)?,
            self.domain,
        )?;
        let deviation = form::distance(Shape::Helical, self.domain, &h.form.exact, &image)?;
        Ok(Changed {
            value: CurveKind::Helix(h),
            deviation,
        })
    }
}
