//! The behaviour every surface kind has (chapter 7), and the closed enum that stores every kind.
//!
//! What it does: [`ParamSurface`] is the trait chapter 12.1 names: evaluation, first partial
//! derivatives, certified enclosures, conservative bounds on the value and the partials over any
//! parameter box, a bound on the normals' directions (the Gauss map), conversion to NURBS with an
//! error, split, reverse and transform; with every partial derivative of any order enclosed at a
//! point or over a box ([`ParamSurface::partials_over`], listed by [`partial_index`]) and the
//! parameters where a kind may not be smooth ([`ParamSurface::breaks`]), which chapter 13's
//! offsets and Class-A blends need (piece G66). [`SurfaceKind`] is the storage enum, one variant
//! per kind, which dispatches to it. New surfaces come back as [`SurfaceKind`] inside `geom-nurbs`'s
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

use alloc::vec::Vec;

use geom_curve::tight;
use geom_curve::{CurveError, DerivativeBound, ParamMap};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::WorkMeter;

use crate::analytic::{AffineSurface, Cone, Cylinder, Plane, Sphere, Torus};
use crate::offset::Offset;
use crate::swept::{Extrusion, Revolution, Ruled};

/// A surface kind as NURBS: the surface, and the maps from the kind's parameters to its own in
/// each direction (the same parameter, or a conic's tangent half-angle map).
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceForm {
    /// The NURBS surface.
    pub surface: geom_nurbs::Surface<3>,
    /// The map of u.
    pub u: ParamMap,
    /// The map of v.
    pub v: ParamMap,
}

/// A parameter box ([u0, u1], [v0, v1]).
pub type ParamBox = ((f64, f64), (f64, f64));

/// Where ∂^{i+j}S/∂u^i∂v^j stands in a list of partials: by total order n = i + j, then by j, so
/// at n(n + 1)/2 + j (S, `S_u`, `S_v`, `S_uu`, `S_uv`, `S_vv`, …).
///
/// # Errors
///
/// [`CurveError::Defect`] if the index overflows.
pub fn partial_index(i: usize, j: usize) -> Result<usize, CurveError> {
    let n = i.checked_add(j).ok_or(CurveError::Defect)?;
    n.checked_mul(n.checked_add(1).ok_or(CurveError::Defect)?)
        .and_then(|x| x.checked_div(2))
        .and_then(|x| x.checked_add(j))
        .ok_or(CurveError::Defect)
}

/// The partials to `order` over a box with centre c, each the intersection of its enclosure over
/// the box in `wide` (which holds the partials to order + 1 there) with its centred form, the mean
/// value theorem's P(c) + `P_u`(B)·Δu + `P_v`(B)·Δv, P(c) from `at` and Δu, Δv the box's extent
/// from c. Both hold every value, so their intersection does; the centred form's width shrinks
/// with the square of the box's, the direct one's only with its width (R. E. Moore,
/// R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009), section 6.4,
/// <https://doi.org/10.1137/1.9780898717716>). The box must hold no break of the surface inside.
#[allow(
    clippy::many_single_char_names,
    reason = "the mean value theorem's names: P and its partials, i and j orders, c the centre's value, a and b the slopes"
)]
pub(crate) fn centred(
    at: &[[Interval; 3]],
    wide: &[[Interval; 3]],
    (du, dv): (Interval, Interval),
    order: usize,
) -> Result<Vec<[Interval; 3]>, CurveError> {
    let get = |x: &[[Interval; 3]], i: usize, j: usize| -> Result<[Interval; 3], CurveError> {
        x.get(partial_index(i, j)?)
            .copied()
            .ok_or(CurveError::Defect)
    };
    let mut out = Vec::with_capacity(at.len());
    for n in 0..=order {
        for j in 0..=n {
            let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
            let (p, w) = (get(at, i, j)?, get(wide, i, j)?);
            let (pu, pv) = (
                get(wide, i.saturating_add(1), j)?,
                get(wide, i, j.saturating_add(1))?,
            );
            let mut o = w;
            for d in 0..3 {
                let (Some(&c), Some(&a), Some(&b), Some(slot)) =
                    (p.get(d), pu.get(d), pv.get(d), o.get_mut(d))
                else {
                    return Err(CurveError::Defect);
                };
                let step = tight::add(tight::mul(a, du)?, tight::mul(b, dv)?)?;
                *slot = meet(*slot, tight::add(c, step)?)?;
            }
            out.push(o);
        }
    }
    Ok(out)
}

/// The interval [lo − c, hi − c], rounded outward.
pub(crate) fn offset_from((lo, hi): (f64, f64), c: f64) -> Result<Interval, CurveError> {
    let l = tight::sub(tight::point(lo)?, tight::point(c)?)?.lo();
    let h = tight::sub(tight::point(hi)?, tight::point(c)?)?.hi();
    Ok(Interval::new(l, h)?)
}

/// The intersection of two enclosures of one value (they overlap; if rounding ever made them
/// miss, the first is kept).
fn meet(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    let lo = tight::larger(a.lo(), b.lo());
    let hi = tight::smaller(a.hi(), b.hi());
    if lo <= hi {
        Ok(Interval::new(lo, hi)?)
    } else {
        Ok(a)
    }
}

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

    /// Enclosures of every partial derivative ∂^{i+j}S/∂u^i∂v^j with i + j ≤ `order`, listed by
    /// [`partial_index`], each holding the partial at every point of `region`. A region of one
    /// point is that point, from `sides` at a break; a region wider in a direction holds the values
    /// from both sides of a break inside it.
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`]; [`CurveError::Unbounded`] where a partial cannot be bounded
    /// over the region (at a pole of a rational form, or a point where an offset's base normal
    /// vanishes); [`geom_nurbs::NurbsError::DerivativeOrder`] for an order above 18 where the
    /// partials pass through interval jets (a rational NURBS surface, an offset), whose
    /// factorials are exact only that far; [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn partials_over(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError>;

    /// The parameters strictly inside the domain where the surface may not be smooth (a partial of
    /// some order may jump there): in u, then in v, each increasing and without repeats.
    fn breaks(&self) -> (Vec<f64>, Vec<f64>);

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

    /// The surface as NURBS, with the map from each of the kind's parameters to the surface's, and
    /// a certified bound on |S(u, v) − N(map(u), map(v))| over the domain (0 when exact).
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] for an unbounded plane; as [`ParamSurface::bound`].
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError>;

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
/// the subdivision limit surface and the procedural sweeps and blends.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceKind {
    /// A NURBS surface, rational or not.
    Nurbs(geom_nurbs::Surface<3>),
    /// A plane.
    Plane(Plane),
    /// A cylinder.
    Cylinder(Cylinder),
    /// A cone.
    Cone(Cone),
    /// A sphere.
    Sphere(Sphere),
    /// A torus, ring, horn or spindle.
    Torus(Torus),
    /// The image of an analytic kind under an affine map that is not a similarity.
    Affine(AffineSurface),
    /// A surface of revolution.
    Revolution(Revolution),
    /// A linear extrusion.
    Extrusion(Extrusion),
    /// A ruled surface.
    Ruled(Ruled),
    /// An offset surface: a base kind moved along its normal (G66).
    Offset(Offset),
}

impl ParamSurface for SurfaceKind {
    fn domain(&self) -> ParamBox {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::domain(s),
            SurfaceKind::Plane(s) => ParamSurface::domain(s),
            SurfaceKind::Cylinder(s) => ParamSurface::domain(s),
            SurfaceKind::Cone(s) => ParamSurface::domain(s),
            SurfaceKind::Sphere(s) => ParamSurface::domain(s),
            SurfaceKind::Torus(s) => ParamSurface::domain(s),
            SurfaceKind::Affine(s) => ParamSurface::domain(s),
            SurfaceKind::Revolution(s) => ParamSurface::domain(s),
            SurfaceKind::Extrusion(s) => ParamSurface::domain(s),
            SurfaceKind::Ruled(s) => ParamSurface::domain(s),
            SurfaceKind::Offset(s) => ParamSurface::domain(s),
        }
    }

    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Plane(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Cylinder(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Cone(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Sphere(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Torus(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Affine(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Revolution(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Extrusion(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Ruled(s) => ParamSurface::point(s, u, v, sides),
            SurfaceKind::Offset(s) => ParamSurface::point(s, u, v, sides),
        }
    }

    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.partials(u, v, sides),
            SurfaceKind::Plane(s) => s.partials(u, v, sides),
            SurfaceKind::Cylinder(s) => s.partials(u, v, sides),
            SurfaceKind::Cone(s) => s.partials(u, v, sides),
            SurfaceKind::Sphere(s) => s.partials(u, v, sides),
            SurfaceKind::Torus(s) => s.partials(u, v, sides),
            SurfaceKind::Affine(s) => s.partials(u, v, sides),
            SurfaceKind::Revolution(s) => s.partials(u, v, sides),
            SurfaceKind::Extrusion(s) => s.partials(u, v, sides),
            SurfaceKind::Ruled(s) => s.partials(u, v, sides),
            SurfaceKind::Offset(s) => s.partials(u, v, sides),
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
            SurfaceKind::Plane(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Cylinder(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Cone(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Sphere(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Torus(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Affine(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Revolution(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Extrusion(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Ruled(s) => s.enclose(u, v, sides, meter),
            SurfaceKind::Offset(s) => s.enclose(u, v, sides, meter),
        }
    }

    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Plane(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Cylinder(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Cone(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Sphere(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Torus(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Affine(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Revolution(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Extrusion(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Ruled(s) => ParamSurface::bound(s, region, meter),
            SurfaceKind::Offset(s) => ParamSurface::bound(s, region, meter),
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
            SurfaceKind::Plane(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Cylinder(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Cone(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Sphere(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Torus(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Affine(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Revolution(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Extrusion(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Ruled(s) => ParamSurface::derivative_bound(s, region, dir, meter),
            SurfaceKind::Offset(s) => ParamSurface::derivative_bound(s, region, dir, meter),
        }
    }

    fn partials_over(
        &self,
        region: ParamBox,
        sides: (Limit, Limit),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Plane(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Cylinder(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Cone(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Sphere(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Torus(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Affine(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Revolution(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Extrusion(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Ruled(s) => s.partials_over(region, sides, order, meter),
            SurfaceKind::Offset(s) => s.partials_over(region, sides, order, meter),
        }
    }

    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        match self {
            SurfaceKind::Nurbs(s) => s.breaks(),
            SurfaceKind::Plane(s) => s.breaks(),
            SurfaceKind::Cylinder(s) => s.breaks(),
            SurfaceKind::Cone(s) => s.breaks(),
            SurfaceKind::Sphere(s) => s.breaks(),
            SurfaceKind::Torus(s) => s.breaks(),
            SurfaceKind::Affine(s) => s.breaks(),
            SurfaceKind::Revolution(s) => s.breaks(),
            SurfaceKind::Extrusion(s) => s.breaks(),
            SurfaceKind::Ruled(s) => s.breaks(),
            SurfaceKind::Offset(s) => s.breaks(),
        }
    }

    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.normal_bound(region, meter),
            SurfaceKind::Plane(s) => s.normal_bound(region, meter),
            SurfaceKind::Cylinder(s) => s.normal_bound(region, meter),
            SurfaceKind::Cone(s) => s.normal_bound(region, meter),
            SurfaceKind::Sphere(s) => s.normal_bound(region, meter),
            SurfaceKind::Torus(s) => s.normal_bound(region, meter),
            SurfaceKind::Affine(s) => s.normal_bound(region, meter),
            SurfaceKind::Revolution(s) => s.normal_bound(region, meter),
            SurfaceKind::Extrusion(s) => s.normal_bound(region, meter),
            SurfaceKind::Ruled(s) => s.normal_bound(region, meter),
            SurfaceKind::Offset(s) => s.normal_bound(region, meter),
        }
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.to_nurbs(meter),
            SurfaceKind::Plane(s) => s.to_nurbs(meter),
            SurfaceKind::Cylinder(s) => s.to_nurbs(meter),
            SurfaceKind::Cone(s) => s.to_nurbs(meter),
            SurfaceKind::Sphere(s) => s.to_nurbs(meter),
            SurfaceKind::Torus(s) => s.to_nurbs(meter),
            SurfaceKind::Affine(s) => s.to_nurbs(meter),
            SurfaceKind::Revolution(s) => s.to_nurbs(meter),
            SurfaceKind::Extrusion(s) => s.to_nurbs(meter),
            SurfaceKind::Ruled(s) => s.to_nurbs(meter),
            SurfaceKind::Offset(s) => s.to_nurbs(meter),
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
            SurfaceKind::Plane(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Cylinder(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Cone(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Sphere(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Torus(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Affine(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Revolution(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Extrusion(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Ruled(s) => ParamSurface::split(s, dir, t, meter),
            SurfaceKind::Offset(s) => ParamSurface::split(s, dir, t, meter),
        }
    }

    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.reversed(dir, meter),
            SurfaceKind::Plane(s) => s.reversed(dir, meter),
            SurfaceKind::Cylinder(s) => s.reversed(dir, meter),
            SurfaceKind::Cone(s) => s.reversed(dir, meter),
            SurfaceKind::Sphere(s) => s.reversed(dir, meter),
            SurfaceKind::Torus(s) => s.reversed(dir, meter),
            SurfaceKind::Affine(s) => s.reversed(dir, meter),
            SurfaceKind::Revolution(s) => s.reversed(dir, meter),
            SurfaceKind::Extrusion(s) => s.reversed(dir, meter),
            SurfaceKind::Ruled(s) => s.reversed(dir, meter),
            SurfaceKind::Offset(s) => s.reversed(dir, meter),
        }
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        match self {
            SurfaceKind::Nurbs(s) => s.transformed(transform, meter),
            SurfaceKind::Plane(s) => s.transformed(transform, meter),
            SurfaceKind::Cylinder(s) => s.transformed(transform, meter),
            SurfaceKind::Cone(s) => s.transformed(transform, meter),
            SurfaceKind::Sphere(s) => s.transformed(transform, meter),
            SurfaceKind::Torus(s) => s.transformed(transform, meter),
            SurfaceKind::Affine(s) => s.transformed(transform, meter),
            SurfaceKind::Revolution(s) => s.transformed(transform, meter),
            SurfaceKind::Extrusion(s) => s.transformed(transform, meter),
            SurfaceKind::Ruled(s) => s.transformed(transform, meter),
            SurfaceKind::Offset(s) => s.transformed(transform, meter),
        }
    }
}
