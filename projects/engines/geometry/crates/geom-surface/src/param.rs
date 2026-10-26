//! The behaviour every surface kind has (chapter 7), and the closed enum that stores every kind.
//!
//! What it does: [`ParamSurface`] is the trait chapter 12.1 names: evaluation, first partial
//! derivatives, certified enclosures, conservative bounds on the value and the partials over any
//! parameter box, a bound on the normals' directions (the Gauss map), conversion to NURBS with an
//! error, split, reverse and transform. [`SurfaceKind`] is the storage enum, one variant per kind,
//! which dispatches to it. New surfaces come back as [`SurfaceKind`] inside `geom-nurbs`'s
//! [`Changed`], with a certified bound on their distance from the exact result. Errors are
//! `geom-curve`'s [`CurveError`], shared by every kind (a span there is a box here).
//!
//! Guarantees:
//!
//! - **Certified, or refused.** Every bound and enclosure holds the exact value for the surface the
//!   stored data define; a derivative that cannot be bounded over a box is
//!   [`DerivativeBound::Unbounded`], and the normal bound then still bounds the directions.
//! - **Closed and exhaustive.** Matching on [`SurfaceKind`] is exhaustive; a new kind is a new
//!   variant (the playbook in `projects/engines/geometry/AGENTS.md`).
//! - **Trimming is not here.** A surface kind is a carrier; trimming belongs to the topology's
//!   loops (chapter 7).
//!
//! Method: the operations of chapter 7 and the trait names of chapter 12.1
//! (`projects/engines/geometry/architecture/12-intersections.md`), with the normal bound in place
//! of an unbounded derivative as chapter 12.1 asks.
//!
//! Cost: none here; each kind's implementation states its own.

use geom_curve::{CurveError, DerivativeBound};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::WorkMeter;

/// A parameter box ([u0, u1], [v0, v1]).
pub type ParamBox = ((f64, f64), (f64, f64));

/// The behaviour of a surface kind.
pub trait ParamSurface {
    /// The parameter domain.
    fn domain(&self) -> ParamBox;

    /// The point S(u, v), in doubles, from `sides` at a break.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`]; [`CurveError::NotFinite`].
    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError>;

    /// S, `S_u` and `S_v` at (u, v), in doubles.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::point`].
    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError>;

    /// Enclosures of the exact S, `S_u` and `S_v` at (u, v).
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::point`]; [`CurveError::Stopped`].
    fn enclose(
        &self,
        u: f64,
        v: f64,
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError>;

    /// A box holding the surface over `region`, inside the domain.
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`]; [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError>;

    /// A bound on the partial derivative in `dir` over `region`.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::bound`].
    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError>;

    /// A box holding a positive multiple of the normal `S_u` × `S_v` at every point of `region` (at a
    /// pole, its limit): the signs it decides are the normals' signs.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::bound`].
    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError>;

    /// The surface as NURBS with the same parameters, and a certified bound on their distance.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::bound`].
    fn to_nurbs(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<geom_nurbs::Surface<3>>, CurveError>;

    /// The surface split at `t` in `dir`, strictly inside the domain: the part before and the part
    /// after, with the same parameters.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`]; as [`ParamSurface::bound`].
    fn split(
        &self,
        dir: Direction,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError>;

    /// The surface with `dir` reversed: on [−t1, −t0] in that direction, the reversed surface at −t
    /// is the surface at t.
    ///
    /// # Errors
    ///
    /// As [`ParamSurface::bound`].
    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError>;

    /// The image of the surface under `transform`, with the same parameters.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError>;
}

/// Every surface kind, stored as itself (chapter 7): one variant per kind built. Later pieces add
/// the analytic and swept kinds (G65), the offset surface (G66) and the subdivision limit surface.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceKind {
    /// A NURBS surface, rational or not.
    Nurbs(geom_nurbs::Surface<3>),
}

impl ParamSurface for SurfaceKind {
    fn domain(&self) -> ParamBox {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::domain(s),
        }
    }

    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::point(s, u, v, sides),
        }
    }

    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.partials(u, v, sides),
        }
    }

    fn enclose(
        &self,
        u: f64,
        v: f64,
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.enclose(u, v, sides, meter),
        }
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::bound(s, region, meter),
        }
    }

    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::derivative_bound(s, region, dir, meter),
        }
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.normal_bound(region, meter),
        }
    }

    fn to_nurbs(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<geom_nurbs::Surface<3>>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.to_nurbs(meter),
        }
    }

    fn split(
        &self,
        dir: Direction,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::split(s, dir, t, meter),
        }
    }

    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.reversed(dir, meter),
        }
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.transformed(transform, meter),
        }
    }
}
