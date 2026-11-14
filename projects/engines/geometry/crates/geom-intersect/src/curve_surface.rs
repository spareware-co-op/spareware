//! Where a curve meets a surface (chapter 12.6).
//!
//! What it does: [`curve_surface`] answers for any curve and surface. A line with a plane is a linear
//! equation, a line with a sphere, cylinder or cone a quadratic in the surface's own coordinates, and
//! a line with a torus a quartic, isolated by [`crate::solve`], each with its count from an exact sign
//! or a Sturm chain where the data are whole. A helix on a coaxial cylinder of its radius is an
//! overlap. Every other pair runs [`crate::solve`] on C(t) − S(u, v), with the Interval Projected
//! Polyhedron step narrowing the boxes of NURBS pieces.
//!
//! Guarantees:
//!
//! - **Every root found or flagged.** A box is discarded only on a proof, so a meeting no item reports
//!   does not exist.
//! - **Exact where the data are.** On whole-number data the count, and whether a root is double, come
//!   from exact signs, within the budgets of chapter 6.2. Elsewhere an interval decides, and what it
//!   cannot tell goes to the engine.
//! - **Grazing is read, not guessed.** Where the tangent cannot be excluded from the surface's
//!   tangent plane the box is a candidate, and the second order test compares the curve's normal
//!   curvature with the surface's: [`Unresolved::TangentBand`] with its width where they differ,
//!   [`Unresolved::HigherOrderContact`] where they cannot be told apart, and
//!   [`Unresolved::NearCoincident`] for a long run, a curve lying near the surface.
//! - **Overlap by identity.** A line in a plane or along a ruling, and a helix on a coaxial cylinder of
//!   its radius, are [`Item::Overlap`]s with [`Coincidence::Identical`].
//! - **A join is no boundary.** A root on a closed kind's join (a round kind's u = 0, a torus's
//!   v = 0, a closed curve's start) crosses like any other; only a true end of a domain makes a
//!   [`PointContact::BoundaryTouch`]. Every enclosure is rounded outward, so one whose bound lands
//!   exactly on an end holds a value past it, and misses.
//!
//! Method: chapter 12.6 (`projects/engines/geometry/architecture/12-intersections.md`): the Interval
//! Projected Polyhedron solver ([Sherbrooke and Patrikalakis
//! 1993](https://doi.org/10.1016/0167-8396(93)90019-Y)), projecting a patch's coefficients on each
//! parameter axis against the thin slabs around a curve piece, and a curve piece against the patch's
//! fat plane; Krawczyk's test finishes every box.
//!
//! Cost: a closed form is a few predicates. The engine's boxes are near the logarithm of the kinds'
//! reach for a transversal root, each a few evaluations; a clip is O(p·q). Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: t the curve's parameter, u and v the surface's, o and d a \\
              line, c and n a frame's centre and axis"
)]

use alloc::vec::Vec;

use geom_curve::tight::{self, larger, smaller};
use geom_curve::{CurveError, CurveKind, DerivativeBound, Frame, HelixAxes, Line, ParamCurve};
use geom_num::Sign;
use geom_num::grid::{P3, Steps};
use geom_num::interval::Interval;
use geom_num::isolate::RESOLUTION_FLOOR;
use geom_num::sturm::Roots;
use geom_num::{predicate, quadric};
use geom_nurbs::{BernsteinCurve, BernsteinPatch, Bounds, Direction, Limit, NurbsError};
use geom_run::WorkMeter;
use geom_surface::{
    Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus, partial_index,
};

use crate::closed::{self, Meeting};
use crate::curve::{
    Seam, bound_over, certified, cut, exact_steps, finite, grid_box, grid_line, grid_point, held,
    onto, proven, quadratic, seam_of, sign_of, slope_over, span, turn_of, whole, widest,
};
use crate::error::IntersectError;
use crate::result::{
    Coincidence, Coverage, Degeneracy, Intersection, Item, OverlapRegion, ParamBox, PointContact,
    StepsInterval, Test, Unresolved,
};
use crate::solve::{Field, Reason, Solved, grown, runs, solve};

/// A flagged run longer than this, in steps, is a curve lying near the surface, not a grazing.
const LONG_RUN: f64 = 64.0;

/// The dot product of two enclosed vectors.
pub(crate) fn dot(a: [Interval; 3], b: [Interval; 3]) -> Result<Interval, IntersectError> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    Ok(p.mul(x)?.add(q.mul(y)?)?.add(r.mul(z)?)?)
}

/// The cross product of two enclosed vectors.
pub(crate) fn cross(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], IntersectError> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    Ok([
        q.mul(z)?.sub(r.mul(y)?)?,
        r.mul(x)?.sub(p.mul(z)?)?,
        p.mul(y)?.sub(q.mul(x)?)?,
    ])
}

/// `a − b`.
pub(crate) fn minus(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], IntersectError> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    Ok([p.sub(x)?, q.sub(y)?, r.sub(z)?])
}

/// `v` enclosed.
pub(crate) fn lift(v: [f64; 3]) -> Result<[Interval; 3], IntersectError> {
    let [x, y, z] = v;
    Ok([
        Interval::point(x)?,
        Interval::point(y)?,
        Interval::point(z)?,
    ])
}

/// A box as its three sides.
pub(crate) fn sides(b: &Bounds<3>) -> Result<[Interval; 3], IntersectError> {
    let ([a, c, e], [f, g, h]) = (b.lo, b.hi);
    Ok([
        Interval::new(a, f)?,
        Interval::new(c, g)?,
        Interval::new(e, h)?,
    ])
}

/// The item for one root, with `(u, v)` read from the surface's own coordinates.
fn root(
    line: &Line<3>,
    t: Interval,
    (u, v): (Interval, Interval),
    contact: PointContact,
    meter: &mut WorkMeter<'_>,
) -> Result<Item, IntersectError> {
    let t = held(t, finite_line(line)?)?;
    Ok(Item::Point {
        at: ParamBox::new(&[t, u, v])?,
        xyz: grid_box(&line.bound(span(t), meter)?)?,
        contact,
        class: certified(),
    })
}

/// `line`'s domain, finite.
fn finite_line(line: &Line<3>) -> Result<(f64, f64), IntersectError> {
    crate::curve::line_span(line)
}

/// Whether `x` touches an end of `domain`, so that a root there is a touch.
fn at_an_end(x: Interval, (lo, hi): (f64, f64)) -> bool {
    x.contains(lo) || x.contains(hi)
}

/// Where `(u, v)` touches a true end of a surface's domain: a closed kind's join is none (12.6).
fn surface_ends(seams: [Seam; 2], u: Interval, v: Interval) -> [bool; 2] {
    let [seam_u, seam_v] = seams;
    [seam_u.ends_at(u), seam_v.ends_at(v)]
}

/// How a root stands: a double root touches, one on a domain's end touches, and others cross.
fn standing(double: bool, ends: [bool; 3]) -> PointContact {
    if double {
        PointContact::IsolatedTangent
    } else if ends.contains(&true) {
        PointContact::BoundaryTouch
    } else {
        PointContact::Crossing
    }
}

/// `x` on `domain`, if any of it can land there. Every `x` here is an outward-rounded result, each
/// bound strictly past the exact value, so a bound on an end of the domain puts the value past it.
fn within(x: Interval, domain: (f64, f64)) -> Result<Option<Interval>, IntersectError> {
    if x.hi() <= domain.0 || x.lo() >= domain.1 {
        return Ok(None);
    }
    held(x, domain).map(Some)
}

/// A line with a plane. Parallel, and lying in it, are exact signs on whole-number data, and
/// otherwise a determinant proven apart from zero. `None` where neither tells.
fn line_plane(
    line: &Line<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (c, a, b) = plane.vectors();
    let (o, d) = (line.origin(), line.direction());
    let exact = plane_signs(line, plane)?;
    let normal = cross(lift(a)?, lift(b)?)?;
    let step = dot(normal, lift(d)?)?;
    let offset = dot(normal, minus(lift(c)?, lift(o)?)?)?;
    let domain = ParamSurface::domain(plane);
    // Parallel, and lying in it, only from the exact signs: every operation rounds outward, so an
    // enclosure of the step or the offset is never the one point zero.
    if let Some((Sign::Zero, side)) = exact {
        if side != Sign::Zero {
            return proven(Vec::new(), meter).map(Some);
        }
        return in_plane(line, plane, meter).map(Some);
    }
    if step.contains(0.0) {
        return Ok(None);
    }
    let t = offset.div(step)?;
    let Some(t) = within(t, ParamCurve::domain(line))? else {
        return proven(Vec::new(), meter).map(Some);
    };
    let (u, v) = plane_parameters(line, plane, t)?;
    let (Some(u), Some(v)) = (within(u, domain.0)?, within(v, domain.1)?) else {
        return proven(Vec::new(), meter).map(Some);
    };
    let ends = [
        at_an_end(t, ParamCurve::domain(line)),
        at_an_end(u, domain.0),
        at_an_end(v, domain.1),
    ];
    let item = root(line, t, (u, v), standing(false, ends), meter)?;
    proven(alloc::vec![item], meter).map(Some)
}

/// `(u, v)` of the point at `t` of a line on a plane.
fn plane_parameters(
    line: &Line<3>,
    plane: &Plane,
    t: Interval,
) -> Result<(Interval, Interval), IntersectError> {
    let (c, a, b) = plane.vectors();
    let (o, d) = (lift(line.origin())?, lift(line.direction())?);
    let [ox, oy, oz] = o;
    let [dx, dy, dz] = d;
    let at = [
        ox.add(t.mul(dx)?)?,
        oy.add(t.mul(dy)?)?,
        oz.add(t.mul(dz)?)?,
    ];
    let w = minus(at, lift(c)?)?;
    let (a, b) = (lift(a)?, lift(b)?);
    let (aa, ab, bb) = (dot(a, a)?, dot(a, b)?, dot(b, b)?);
    let (wa, wb) = (dot(w, a)?, dot(w, b)?);
    let det = aa.mul(bb)?.sub(ab.mul(ab)?)?;
    Ok((
        wa.mul(bb)?.sub(wb.mul(ab)?)?.div(det)?,
        wb.mul(aa)?.sub(wa.mul(ab)?)?.div(det)?,
    ))
}

/// A line's direction against a plane and which side its origin lies on, on whole-number data.
fn plane_signs(line: &Line<3>, plane: &Plane) -> Result<Option<(Sign, Sign)>, IntersectError> {
    let (c, a, b) = plane.vectors();
    let (Some(g), Some(x), Some(y), Some(d), Some(l)) = (
        grid_point(&c),
        grid_point(&a),
        grid_point(&b),
        grid_point(&line.direction()),
        grid_line(line),
    ) else {
        return Ok(None);
    };
    let far = |v: [f64; 3]| {
        let [p, q, r] = c;
        let [e, f, h] = v;
        grid_point(&[p + e, q + f, r + h])
    };
    let (Some(p), Some(q)) = (far(a), far(b)) else {
        return Ok(None);
    };
    let origin = P3::new(0, 0, 0)?;
    let along = predicate::orient3d(origin, x, y, d)?;
    let side = predicate::orient3d(g, p, q, l.a)?;
    Ok(Some((along, side)))
}

/// A line lying in a plane: the part of it over the plane's domain.
fn in_plane(
    line: &Line<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let span_t = finite_line(line)?;
    let (start, end) = (Interval::point(span_t.0)?, Interval::point(span_t.1)?);
    let (u0, v0) = plane_parameters(line, plane, start)?;
    let (u1, v1) = plane_parameters(line, plane, end)?;
    let hull =
        |a: Interval, b: Interval| Interval::new(smaller(a.lo(), b.lo()), larger(a.hi(), b.hi()));
    let domain = ParamSurface::domain(plane);
    let (Some(u), Some(v)) = (
        within(hull(u0, u1)?, domain.0)?,
        within(hull(v0, v1)?, domain.1)?,
    ) else {
        return proven(Vec::new(), meter);
    };
    let at = ParamBox::new(&[Interval::new(span_t.0, span_t.1)?, u, v])?;
    let overlap = Item::Overlap {
        at,
        region: OverlapRegion {
            boundary: Vec::new(),
        },
        proof: Coincidence::Identical,
        class: certified(),
    };
    proven(alloc::vec![overlap], meter)
}

/// A line in a frame's coordinates: its origin from `center`, and its direction, along x̂, ŷ and n̂.
fn local(
    line: &Line<3>,
    center: [f64; 3],
    frame: &Frame<3>,
) -> Result<([Interval; 3], [Interval; 3]), IntersectError> {
    let (_, [x, y, n]) = frame.unit_vectors()?;
    let w = minus(lift(line.origin())?, lift(center)?)?;
    let d = lift(line.direction())?;
    Ok((
        [dot(w, x)?, dot(w, y)?, dot(w, n)?],
        [dot(d, x)?, dot(d, y)?, dot(d, n)?],
    ))
}

/// The turn of `(x, y)` on the circle of `radius`, whose sign says which side of the axis it is.
fn around(
    x: Interval,
    y: Interval,
    domain: (f64, f64),
) -> Result<Option<Interval>, IntersectError> {
    onto(turn_of(x, y)?, domain)
}

/// A root at `t` on the frame's coordinates, placed by `place`, kept if it lies on both domains.
fn on_domains(
    line: &Line<3>,
    t: Interval,
    double: bool,
    seams: [Seam; 2],
    place: impl Fn(Interval) -> Result<Option<(Interval, Interval)>, IntersectError>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Item>, IntersectError> {
    let Some(t) = within(t, ParamCurve::domain(line))? else {
        return Ok(None);
    };
    let Some((u, v)) = place(t)? else {
        return Ok(None);
    };
    let [on_u, on_v] = surface_ends(seams, u, v);
    let ends = [at_an_end(t, ParamCurve::domain(line)), on_u, on_v];
    root(line, t, (u, v), standing(double, ends), meter).map(Some)
}

/// The roots of the line with A·t² + 2B·t + C = 0 on a quadric, as items.
fn quadric_roots(
    line: &Line<3>,
    terms: [Interval; 3],
    exact: Option<Meeting>,
    seams: [Seam; 2],
    place: impl Fn(Interval) -> Result<Option<(Interval, Interval)>, IntersectError> + Copy,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let Some(roots) = quadratic(terms, exact)? else {
        return Ok(None);
    };
    let double = exact == Some(Meeting::Tangent) || exact == Some(Meeting::Apex);
    let mut items = Vec::new();
    for t in roots {
        if let Some(item) = on_domains(line, t, double, seams, place, meter)? {
            items.push(item);
        }
    }
    proven(items, meter).map(Some)
}

/// `a·x + b`, for a line in one coordinate.
fn along(o: Interval, d: Interval, t: Interval) -> Result<Interval, IntersectError> {
    Ok(o.add(t.mul(d)?)?)
}

/// A line with a sphere.
fn line_sphere(
    line: &Line<3>,
    sphere: &Sphere,
    seams: [Seam; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (o, d) = local(line, sphere.center(), &sphere.frame())?;
    let r = sphere.radius();
    let c = dot(o, o)?.sub(Interval::point(r)?.square()?)?;
    let exact = exact_sphere(line, sphere);
    let domain = ParamSurface::domain(sphere);
    let sign = sphere.axial_sign();
    let [ox, oy, oz] = o;
    let [dx, dy, dz] = d;
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let z = z.mul(Interval::point(sign)?)?;
        let round = tight::norm(&[x, y])?;
        let v = Interval::atan2(z, round)?.div(Interval::PI)?;
        let Some(v) = within(v, domain.1)? else {
            return Ok(None);
        };
        // A pole has every u: the domain's.
        let u = if round.lo() <= 0.0 {
            Some(Interval::new(domain.0.0, domain.0.1)?)
        } else {
            around(x, y, domain.0)?
        };
        Ok(u.map(|u| (u, v)))
    };
    quadric_roots(
        line,
        [dot(d, d)?, dot(o, d)?, c],
        exact,
        seams,
        place,
        meter,
    )
}

/// How a line meets a sphere, exactly, where their data are whole.
fn exact_sphere(line: &Line<3>, sphere: &Sphere) -> Option<Meeting> {
    let s = quadric::Sphere {
        centre: grid_point(&sphere.center())?,
        radius: Steps(u64::try_from(whole(sphere.radius())?).ok()?),
    };
    closed::line_sphere(&grid_line(line)?, &s).ok()
}

/// The point `c + n`, on the grid.
fn on_axis(center: &[f64; 3], frame: &Frame<3>) -> Option<(P3, P3)> {
    let n = frame.normal();
    let [a, b, c] = *center;
    let [p, q, r] = n;
    Some((grid_point(center)?, grid_point(&[a + p, b + q, c + r])?))
}

/// A line with a cylinder.
fn line_cylinder(
    line: &Line<3>,
    cylinder: &Cylinder,
    seams: [Seam; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let ([ox, oy, oz], [dx, dy, dz]) = local(line, cylinder.center(), &cylinder.frame())?;
    let r = Interval::point(cylinder.radius())?;
    let terms = [
        dx.square()?.add(dy.square()?)?,
        ox.mul(dx)?.add(oy.mul(dy)?)?,
        ox.square()?.add(oy.square()?)?.sub(r.square()?)?,
    ];
    let exact = exact_cylinder(line, cylinder);
    let domain = ParamSurface::domain(cylinder);
    if exact == Some(Meeting::Within) {
        return along_ruling(line, cylinder, meter).map(Some);
    }
    let sign = cylinder.axial_sign();
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let v = z.mul(Interval::point(sign)?)?;
        let (Some(u), Some(v)) = (around(x, y, domain.0)?, within(v, domain.1)?) else {
            return Ok(None);
        };
        Ok(Some((u, v)))
    };
    quadric_roots(line, terms, exact, seams, place, meter)
}

/// How a line meets a cylinder, exactly, where their data are whole.
fn exact_cylinder(line: &Line<3>, cylinder: &Cylinder) -> Option<Meeting> {
    let (base, along) = on_axis(&cylinder.center(), &cylinder.frame())?;
    let c = quadric::Cylinder {
        base,
        along,
        radius: Steps(u64::try_from(whole(cylinder.radius())?).ok()?),
    };
    closed::line_cylinder(&grid_line(line)?, &c).ok()
}

/// A line along a cylinder's ruling: the part over the domain in v, at the ruling's u.
fn along_ruling(
    line: &Line<3>,
    cylinder: &Cylinder,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let ([ox, oy, oz], [_, _, dz]) = local(line, cylinder.center(), &cylinder.frame())?;
    let domain = ParamSurface::domain(cylinder);
    let Some(u) = around(ox, oy, domain.0)? else {
        return proven(Vec::new(), meter);
    };
    let sign = Interval::point(cylinder.axial_sign())?;
    let (start, end) = finite_line(line)?;
    let v_of = |t: f64| -> Result<Interval, IntersectError> {
        Ok(along(oz, dz, Interval::point(t)?)?.mul(sign)?)
    };
    let (first, last) = (v_of(start)?, v_of(end)?);
    let (lo, hi) = (
        smaller(first.lo(), last.lo()),
        larger(first.hi(), last.hi()),
    );
    let Some(v) = within(Interval::new(lo, hi)?, domain.1)? else {
        return proven(Vec::new(), meter);
    };
    let overlap = Item::Overlap {
        at: ParamBox::new(&[Interval::new(start, end)?, u, v])?,
        region: OverlapRegion {
            boundary: Vec::new(),
        },
        proof: Coincidence::Identical,
        class: certified(),
    };
    proven(alloc::vec![overlap], meter)
}

/// A line with a cone: ρ = r₀ + k·v on the axis at s·v, ρ² against x² + y².
fn line_cone(
    line: &Line<3>,
    cone: &Cone,
    seams: [Seam; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let ([ox, oy, oz], [dx, dy, dz]) = local(line, cone.center(), &cone.frame())?;
    let (r0, k) = cone.radius_and_slope();
    let sign = Interval::point(cone.axial_sign())?;
    let (r0, k) = (Interval::point(r0)?, Interval::point(k)?);
    let (e, f) = (r0.add(k.mul(sign)?.mul(oz)?)?, k.mul(sign)?.mul(dz)?);
    let terms = [
        dx.square()?.add(dy.square()?)?.sub(f.square()?)?,
        ox.mul(dx)?.add(oy.mul(dy)?)?.sub(e.mul(f)?)?,
        ox.square()?.add(oy.square()?)?.sub(e.square()?)?,
    ];
    let domain = ParamSurface::domain(cone);
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let v = z.mul(sign)?;
        let rho = r0.add(k.mul(v)?)?;
        let Some(v) = within(v, domain.1)? else {
            return Ok(None);
        };
        // The apex has every u; past it, e(u) points the other way.
        if rho.contains(0.0) {
            return Ok(Some((Interval::new(domain.0.0, domain.0.1)?, v)));
        }
        let flip = if rho.hi() < 0.0 { -1.0 } else { 1.0 };
        let flip = Interval::point(flip)?;
        Ok(around(x.mul(flip)?, y.mul(flip)?, domain.0)?.map(|u| (u, v)))
    };
    quadric_roots(line, terms, None, seams, place, meter)
}

/// What an exact chain says of a line and a torus, on whole-number data.
fn exact_torus(line: &Line<3>, torus: &Torus) -> Option<Roots> {
    let (major, minor) = torus.radii();
    let (centre, along) = on_axis(&torus.center(), &torus.frame())?;
    let t = quadric::Torus {
        centre,
        along,
        major: Steps(u64::try_from(whole(major)?).ok()?),
        minor: Steps(u64::try_from(whole(minor)?).ok()?),
    };
    closed::line_torus(&grid_line(line)?, &t).ok()
}

/// (|P|² + R² − r²)² − 4R²(x² + y²) along a line, as a polynomial in its parameter, lowest first.
struct Quartic {
    terms: [Interval; 5],
    speed: f64,
}

impl Quartic {
    /// The polynomial about `m`, by repeated synthetic division: its value there and its derivatives
    /// over their factorials, lowest first. Written so, a box's enclosures hold no dependency on how
    /// far the line's origin stands from it.
    fn about(&self, m: Interval) -> Result<[Interval; 5], IntersectError> {
        let [a0, a1, a2, a3, a4] = self.terms;
        let next = |c: Interval, high: Interval| -> Result<Interval, IntersectError> {
            Ok(c.add(m.mul(high)?)?)
        };
        let b3 = next(a3, a4)?;
        let b2 = next(a2, b3)?;
        let b1 = next(a1, b2)?;
        let b0 = next(a0, b1)?;
        let c3 = next(b3, a4)?;
        let c2 = next(b2, c3)?;
        let c1 = next(b1, c2)?;
        let d3 = next(c3, a4)?;
        let d2 = next(c2, d3)?;
        let e3 = next(d3, a4)?;
        Ok([b0, c1, d2, e3, a4])
    }

    /// The value at every point of `x`.
    fn value(&self, x: Interval) -> Result<Interval, IntersectError> {
        let m = Interval::point(tight::middle(x))?;
        let h = x.sub(m)?;
        let mut out = Interval::point(0.0)?;
        for c in self.about(m)?.iter().rev() {
            out = out.mul(h)?.add(*c)?;
        }
        Ok(out)
    }

    /// The slope at every point of `x`.
    fn slope_at(&self, x: Interval) -> Result<Interval, IntersectError> {
        let m = Interval::point(tight::middle(x))?;
        let h = x.sub(m)?;
        let mut out = Interval::point(0.0)?;
        for (c, k) in self.about(m)?.iter().skip(1).zip(1_u8..=4).rev() {
            out = out.mul(h)?.add(c.mul(Interval::point(f64::from(k))?)?)?;
        }
        Ok(out)
    }
}

impl Field<1> for Quartic {
    fn at(
        &self,
        [t]: [f64; 1],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; 1], IntersectError> {
        Ok([self.value(Interval::point(t)?)?])
    }

    fn over(
        &self,
        [t]: &[Interval; 1],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 1]>, IntersectError> {
        Ok(Some([self.value(*t)?]))
    }

    fn slope(
        &self,
        [t]: &[Interval; 1],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 1]; 1]>, IntersectError> {
        Ok(Some([[self.slope_at(*t)?]]))
    }

    fn reach(
        &self,
        [t]: &[Interval; 1],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<f64, IntersectError> {
        Ok((t.hi() - t.lo()) * self.speed)
    }
}

/// The stretch of a line within `reach` of its nearest point to the origin of its local coordinates.
fn near_origin(
    line: &Line<3>,
    (o, d): ([Interval; 3], [Interval; 3]),
    reach: Interval,
) -> Result<Option<(f64, f64)>, IntersectError> {
    let square = dot(d, d)?;
    let foot = dot(o, d)?.neg().div(square)?;
    let half = reach.div(square.sqrt()?)?;
    let (lo, hi) = (foot.sub(half)?.lo(), foot.add(half)?.hi());
    let (a, b) = ParamCurve::domain(line);
    let (lo, hi) = (larger(lo, a), smaller(hi, b));
    Ok((lo < hi).then_some((lo, hi)))
}

/// A line with a torus: its quartic isolated, each root placed by the torus's own coordinates, and
/// on whole-number data the count from its Sturm chain.
fn line_torus(
    line: &Line<3>,
    torus: &Torus,
    seams: [Seam; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let exact = exact_torus(line, torus);
    if exact.is_some_and(|roots| roots.distinct == 0) {
        return proven(Vec::new(), meter);
    }
    let (o, d) = local(line, torus.center(), &torus.frame())?;
    let (major, minor) = torus.radii();
    let (big, small) = (Interval::point(major)?, Interval::point(minor)?);
    let Some(stretch) = near_origin(line, (o, d), big.add(small)?.add(Interval::point(1.0)?)?)?
    else {
        return proven(Vec::new(), meter);
    };
    let [ox, oy, oz] = o;
    let [dx, dy, dz] = d;
    let two = Interval::point(2.0)?;
    let (a, b) = (dot(d, d)?, dot(o, d)?.mul(two)?);
    let c = dot(o, o)?.add(big.square()?)?.sub(small.square()?)?;
    let (e, f, g) = (
        dx.square()?.add(dy.square()?)?,
        ox.mul(dx)?.add(oy.mul(dy)?)?.mul(two)?,
        ox.square()?.add(oy.square()?)?,
    );
    let k = big.square()?.mul(Interval::point(4.0)?)?;
    let quartic = Quartic {
        terms: [
            c.square()?.sub(k.mul(g)?)?,
            b.mul(c)?.mul(two)?.sub(k.mul(f)?)?,
            b.square()?.add(a.mul(c)?.mul(two)?)?.sub(k.mul(e)?)?,
            a.mul(b)?.mul(two)?,
            a.square()?,
        ],
        speed: a.sqrt()?.hi(),
    };
    let solved = solve(
        &quartic,
        [Interval::new(stretch.0, stretch.1)?],
        |_, _| Ok(true),
        meter,
    )?;
    let domain = ParamSurface::domain(torus);
    let sign = Interval::point(torus.axial_sign())?;
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let round = tight::norm(&[x, y])?;
        let z = z.mul(sign)?;
        // The outer sheet, or on a spindle the far side of the axis.
        if let (Some(u), Some(v)) = (
            around(x, y, domain.0)?,
            around(round.sub(big)?, z, domain.1)?,
        ) {
            return Ok(Some((u, v)));
        }
        let (Some(u), Some(v)) = (
            around(x.neg(), y.neg(), domain.0)?,
            around(round.neg().sub(big)?, z, domain.1)?,
        ) else {
            return Ok(None);
        };
        Ok(Some((u, v)))
    };
    let mut items = Vec::new();
    for z in &solved.zeros {
        let [t] = z.enclosure;
        if let Some(item) = on_domains(line, t, false, seams, place, meter)? {
            items.push(item);
        }
    }
    let regions = runs(solved.flagged)?;
    let settled = exact.is_some_and(|roots| {
        roots.repeated
            && solved.zeros.len().saturating_add(regions.len()) == usize::from(roots.distinct)
    });
    for ([t], _) in regions {
        if settled {
            if let Some(item) = on_domains(line, t, true, seams, place, meter)? {
                items.push(item);
            }
            continue;
        }
        let Some((u, v)) = place(t)? else {
            continue;
        };
        items.push(Item::Flagged {
            at: ParamBox::new(&[t, u, v])?,
            xyz: grid_box(&line.bound(span(t), meter)?)?,
            why: Unresolved::TangentBand {
                width: Steps(exact_steps((t.hi() - t.lo()) * quartic.speed)),
            },
        });
    }
    let flagged = items.iter().any(|i| matches!(i, Item::Flagged { .. }));
    let coverage = if flagged {
        Coverage::CompleteOutsideFlagged
    } else {
        Coverage::TopologyCertified
    };
    Intersection::new(items, coverage, Vec::new(), meter)
}

/// C(t) − S(u, v).
struct Contact<'a> {
    curve: &'a CurveKind<3>,
    surface: &'a SurfaceKind,
    seams: [Seam; 3],
    pieces: Vec<BernsteinCurve<3>>,
    patches: Vec<BernsteinPatch<3>>,
}

/// The patch of `patches` that holds `u` × `v` whole, cut to it.
fn cut_patch(patches: &[BernsteinPatch<3>], u: Interval, v: Interval) -> Option<BernsteinPatch<3>> {
    let patch = patches.iter().find(|p| {
        let ((a, b), (c, d)) = p.domain();
        a <= u.lo() && u.hi() <= b && c <= v.lo() && v.hi() <= d
    })?;
    patch.restrict(span(u), span(v)).ok()
}

/// A patch's slab, or what a degenerate one leaves undecided.
fn normals(piece: &BernsteinCurve<3>) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    let (lo, hi) = piece.domain();
    let end = |t: f64| -> Option<[f64; 3]> {
        let (n, w) = piece.value_at(t).ok()?;
        let w = w.lo() * 0.5 + w.hi() * 0.5;
        (w != 0.0).then(|| n.map(|c| (c.lo() * 0.5 + c.hi() * 0.5) / w))
    };
    let (first, last) = (end(lo)?, end(hi)?);
    let [a, b, c] = first;
    let [p, q, r] = last;
    let chord = [p - a, q - b, r - c];
    let [x, y, z] = chord.map(f64::abs);
    if x == 0.0 && y == 0.0 && z == 0.0 {
        return None;
    }
    let axis = if x <= y && x <= z {
        [1.0, 0.0, 0.0]
    } else if y <= z {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let side = plain_cross(chord, axis);
    Some((first, side, plain_cross(chord, side)))
}

/// `a × b` in doubles, for choosing a direction.
fn plain_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [p, q, r] = a;
    let [x, y, z] = b;
    [q * z - r * y, r * x - p * z, p * y - q * x]
}

/// What one step of the projected polyhedron says of a box.
enum Projected {
    /// A degenerate piece refused a step: the box stands as it was.
    Refused,
    /// It holds no root.
    Empty,
    /// Every root lies in these ranges of t, u and v.
    Within([(f64, f64); 3]),
}

/// One step of the projected polyhedron: the curve piece against the patch's fat plane, then the
/// patch against the two thin slabs around the piece.
fn projected(
    piece: &BernsteinCurve<3>,
    patch: &BernsteinPatch<3>,
    meter: &mut WorkMeter<'_>,
) -> Result<Projected, NurbsError> {
    let (mut u, mut v) = patch.domain();
    let Ok(plane) = patch.fat_plane(meter) else {
        return Ok(Projected::Refused);
    };
    let Some(t) = piece.clip(&plane, meter)? else {
        return Ok(Projected::Empty);
    };
    let Some((origin, first, second)) = normals(piece) else {
        return Ok(Projected::Refused);
    };
    for n in [first, second] {
        let slab = piece.slab(n, origin, meter)?;
        let Some((p, q)) = patch.clip(&slab, meter)? else {
            return Ok(Projected::Empty);
        };
        u = (larger(u.0, p.0), smaller(u.1, p.1));
        v = (larger(v.0, q.0), smaller(v.1, q.1));
    }
    Ok(Projected::Within([t, u, v]))
}

/// The seams of `surface` over its domain: a round kind closes about its axis, and a torus about its
/// tube too.
fn seams_of(surface: &SurfaceKind) -> [Seam; 2] {
    let (u, v) = ParamSurface::domain(surface);
    match surface {
        SurfaceKind::Torus(_) => [Seam::turns(u), Seam::turns(v)],
        SurfaceKind::Sphere(_) | SurfaceKind::Cylinder(_) | SurfaceKind::Cone(_) => {
            [Seam::turns(u), Seam::open(v)]
        }
        _ => [Seam::open(u), Seam::open(v)],
    }
}

impl Contact<'_> {
    /// Every stretch of the surface's domain that `u` × `v` covers.
    fn regions(&self, u: Interval, v: Interval) -> Vec<((f64, f64), (f64, f64))> {
        let [_, seam_u, seam_v] = self.seams;
        let mut out = Vec::new();
        for p in seam_u.spans(u).into_iter().flatten() {
            for q in seam_v.spans(v).into_iter().flatten() {
                out.push((p, q));
            }
        }
        out
    }

    /// A box around the surface over `u` × `v`, across its joins.
    fn surface_box(
        &self,
        u: Interval,
        v: Interval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, IntersectError> {
        let mut out: Option<Bounds<3>> = None;
        for region in self.regions(u, v) {
            let b = self.surface.bound(region, meter)?;
            out = Some(match out {
                Some(a) => a.union(&b).map_err(CurveError::from)?,
                None => b,
            });
        }
        out.ok_or(IntersectError::Internal)
    }

    /// A box around the surface's partial in `dir` over `u` × `v`, across its joins.
    fn surface_slope(
        &self,
        u: Interval,
        v: Interval,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Bounds<3>>, IntersectError> {
        let mut out: Option<Bounds<3>> = None;
        for region in self.regions(u, v) {
            let DerivativeBound::Bounded(b) = self.surface.derivative_bound(region, dir, meter)?
            else {
                return Ok(None);
            };
            out = Some(match out {
                Some(a) => a.union(&b).map_err(CurveError::from)?,
                None => b,
            });
        }
        Ok(out)
    }
}

impl Field<3> for Contact<'_> {
    fn at(
        &self,
        [t, u, v]: [f64; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<[Interval; 3], IntersectError> {
        let [seam_t, seam_u, seam_v] = self.seams;
        let (t, u, v) = (seam_t.wrapped(t), seam_u.wrapped(u), seam_v.wrapped(v));
        let c = self.curve.enclose(t, Limit::FromRight, 0, meter)?;
        let [c] = c.as_slice() else {
            return Err(IntersectError::Internal);
        };
        let sides = (Limit::FromRight, Limit::FromRight);
        let [s, ..] = self.surface.enclose(u, v, sides, meter)?;
        minus(*c, s)
    }

    fn over(
        &self,
        [t, u, v]: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 3]>, IntersectError> {
        let [seam, ..] = self.seams;
        let c = sides(&bound_over(self.curve, seam, *t, meter)?)?;
        let s = sides(&self.surface_box(*u, *v, meter)?)?;
        minus(c, s).map(Some)
    }

    fn slope(
        &self,
        [t, u, v]: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 3]; 3]>, IntersectError> {
        let [seam, ..] = self.seams;
        let (Some(dc), Some(du), Some(dv)) = (
            slope_over(self.curve, seam, *t, meter)?,
            self.surface_slope(*u, *v, Direction::U, meter)?,
            self.surface_slope(*u, *v, Direction::V, meter)?,
        ) else {
            return Ok(None);
        };
        let [a, b, c] = sides(&dc)?;
        let [p, q, r] = sides(&du)?;
        let [x, y, z] = sides(&dv)?;
        Ok(Some([
            [a, p.neg(), x.neg()],
            [b, q.neg(), y.neg()],
            [c, r.neg(), z.neg()],
        ]))
    }

    fn reach(
        &self,
        [t, u, v]: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<f64, IntersectError> {
        let [seam, ..] = self.seams;
        let c = widest(&bound_over(self.curve, seam, *t, meter)?);
        let s = widest(&self.surface_box(*u, *v, meter)?);
        Ok(if c > s { c } else { s })
    }

    fn narrow(
        &self,
        x: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 3]>, IntersectError> {
        let [t, u, v] = *x;
        let (Some(piece), Some(patch)) = (cut(&self.pieces, t), cut_patch(&self.patches, u, v))
        else {
            return Ok(Some(*x));
        };
        match projected(&piece, &patch, meter).map_err(CurveError::from)? {
            Projected::Refused => Ok(Some(*x)),
            Projected::Empty => Ok(None),
            Projected::Within(found) => {
                let mut out = *x;
                for (slot, (lo, hi)) in out.iter_mut().zip(found) {
                    if lo > hi {
                        return Ok(None);
                    }
                    *slot = Interval::new(lo, hi)?;
                }
                Ok(Some(out))
            }
        }
    }
}

/// The second order test on a run: how the curve's normal acceleration stands against the surface's
/// second fundamental form along the same tangent. Where they differ the contact is isolated, its
/// band √(2δ/Δκ).
fn second_order(
    field: &Contact<'_>,
    [t, u, v]: [Interval; 3],
    run: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Unresolved, IntersectError> {
    let not_smooth = Unresolved::NonSmooth {
        at: Degeneracy::Crease,
    };
    // Over a run grown past where rounding swamps second derivatives, and on the domains.
    let [seam_t, seam_u, seam_v] = field.seams;
    let [t, u, v] = grown(
        &[t, u, v],
        &[seam_t.whole()?, seam_u.whole()?, seam_v.whole()?],
    )?;
    let (DerivativeBound::Bounded(d1), DerivativeBound::Bounded(d2)) = (
        field.curve.derivative_bound(span(t), 1, meter)?,
        field.curve.derivative_bound(span(t), 2, meter)?,
    ) else {
        return Ok(not_smooth);
    };
    let (d1, d2) = (sides(&d1)?, sides(&d2)?);
    let limit = (Limit::FromRight, Limit::FromRight);
    let Ok(list) = field
        .surface
        .partials_over((span(u), span(v)), limit, 2, meter)
    else {
        return Ok(not_smooth);
    };
    let partial = |i: usize, j: usize| -> Result<[Interval; 3], IntersectError> {
        list.get(partial_index(i, j)?)
            .copied()
            .ok_or(IntersectError::Internal)
    };
    let (su, sv) = (partial(1, 0)?, partial(0, 1)?);
    let (suu, suv, svv) = (partial(2, 0)?, partial(1, 1)?, partial(0, 2)?);
    let n = cross(su, sv)?;
    // |Su × Sv|² is the first fundamental form's determinant E·G − F² (Lagrange's identity),
    // enclosed without the cancellation that subtracting F² would round.
    let (nn, e, f, g) = (dot(n, n)?, dot(su, su)?, dot(su, sv)?, dot(sv, sv)?);
    let speed = dot(d1, d1)?;
    if nn.lo() <= 0.0 || speed.lo() <= 0.0 {
        return Ok(Unresolved::HigherOrderContact);
    }
    // The tangent in (u, v): the projection of C' on the tangent plane.
    let (a, b) = (dot(d1, su)?, dot(d1, sv)?);
    let du = a.mul(g)?.sub(b.mul(f)?)?.div(nn)?;
    let dv = b.mul(e)?.sub(a.mul(f)?)?.div(nn)?;
    let second = dot(n, suu)?
        .mul(du.square()?)?
        .add(dot(n, suv)?.mul(du.mul(dv)?)?.mul(Interval::point(2.0)?)?)?
        .add(dot(n, svv)?.mul(dv.square()?)?)?;
    let bent = dot(n, d2)?;
    let difference = bent.sub(second)?.div(nn.sqrt()?.mul(speed)?)?;
    if difference.contains(0.0) {
        if run > LONG_RUN {
            let gap = StepsInterval::new(Steps(0), Steps(1)).ok_or(IntersectError::Internal)?;
            return Ok(Unresolved::NearCoincident { gap });
        }
        return Ok(Unresolved::HigherOrderContact);
    }
    let least = if difference.lo() > 0.0 {
        difference.lo()
    } else {
        -difference.hi()
    };
    let band = Interval::point(2.0 * RESOLUTION_FLOOR.get())?
        .div(Interval::point(least)?)?
        .sqrt()?;
    let width = exact_steps(if band.hi() > run { band.hi() } else { run });
    Ok(Unresolved::TangentBand {
        width: Steps(width),
    })
}

/// The answer from what the engine found.
fn finished(
    field: &Contact<'_>,
    solved: Solved<3>,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let [seam_t, seam_u, seam_v] = field.seams;
    let held = |[t, u, v]: [Interval; 3]| -> Result<[Interval; 3], IntersectError> {
        Ok([seam_t.onto(t)?, seam_u.onto(u)?, seam_v.onto(v)?])
    };
    // Written on the domains, where two runs either side of a join are one.
    let mut flagged = solved.flagged;
    for f in &mut flagged {
        f.region = held(f.region)?;
    }
    let regions = runs(flagged)?;
    let mut items = Vec::new();
    let mut seen: Vec<[Interval; 3]> = Vec::new();
    for z in &solved.zeros {
        let enclosure = held(z.enclosure)?;
        // A zero inside a run is part of what the run could not tell apart, and one on a join is
        // found from both sides of it.
        let meeting = |hull: &[Interval; 3]| {
            hull.iter()
                .zip(&enclosure)
                .all(|(a, b)| a.lo() <= b.hi() && b.lo() <= a.hi())
        };
        if regions.iter().any(|(hull, _)| meeting(hull)) || seen.iter().any(meeting) {
            continue;
        }
        seen.push(enclosure);
        let [t, u, v] = enclosure;
        items.push(Item::Point {
            at: ParamBox::new(&enclosure)?,
            xyz: grid_box(&field.curve.bound(span(t), meter)?)?,
            contact: boundary(field, [t, u, v]),
            class: certified(),
        });
    }
    for ([t, u, v], why) in regions {
        let image = field.curve.bound(span(t), meter)?;
        let at = ParamBox::new(&[t, u, v])?;
        let xyz = grid_box(&image)?;
        let why = match why {
            Reason::Singular => second_order(field, [t, u, v], widest(&image), meter)?,
            Reason::Unproven => Unresolved::UndecidedAtFloor {
                test: Test::Uniqueness,
            },
            Reason::NotSmooth => Unresolved::NonSmooth {
                at: Degeneracy::Crease,
            },
        };
        items.push(Item::Flagged { at, xyz, why });
    }
    let flagged = items.iter().any(|i| matches!(i, Item::Flagged { .. }));
    let coverage = if flagged {
        Coverage::CompleteOutsideFlagged
    } else {
        Coverage::TopologyCertified
    };
    Intersection::new(items, coverage, Vec::new(), meter)
}

/// How a proven root stands against the two domains.
fn boundary(field: &Contact<'_>, [t, u, v]: [Interval; 3]) -> PointContact {
    let [seam_t, seam_u, seam_v] = field.seams;
    let [on_u, on_v] = surface_ends([seam_u, seam_v], u, v);
    standing(false, [seam_t.ends_at(t), on_u, on_v])
}

/// A helix on a coaxial cylinder of its radius, by exact signs on its stored data: the same axis, the
/// same radius. The part of it over the cylinder's domain in v.
fn helix_on_cylinder(
    curve: &CurveKind<3>,
    cylinder: &Cylinder,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let CurveKind::Helix(helix) = curve else {
        return Ok(None);
    };
    let HelixAxes::Frame {
        frame,
        radius,
        pitch,
    } = helix.axes()
    else {
        return Ok(None);
    };
    if radius != cylinder.radius() {
        return Ok(None);
    }
    let [a, b, c] = frame.normal();
    let [x, y, z] = cylinder.frame().normal();
    let parallel = [
        sign_of(&[(b, z), (-c, y)]),
        sign_of(&[(c, x), (-a, z)]),
        sign_of(&[(a, y), (-b, x)]),
    ];
    let [p, q, r] = helix.center();
    let [e, f, g] = cylinder.center();
    let on_axis = [
        sign_of(&[(q, c), (-f, c), (-r, b), (g, b)]),
        sign_of(&[(r, a), (-g, a), (-p, c), (e, c)]),
        sign_of(&[(p, b), (-e, b), (-q, a), (f, a)]),
    ];
    if parallel.iter().chain(&on_axis).any(|s| *s != Sign::Zero) {
        return Ok(None);
    }
    let (_, [_, _, n]) = cylinder.frame().unit_vectors()?;
    let (_, [_, _, m]) = frame.unit_vectors()?;
    let start = dot(minus(lift([p, q, r])?, lift([e, f, g])?)?, n)?;
    let rise = dot(m, n)?.mul(Interval::point(pitch * 0.5)?)?;
    let sign = Interval::point(cylinder.axial_sign())?;
    let (t0, t1) = curve.domain();
    let v_at = |t: f64| -> Result<Interval, IntersectError> {
        Ok(along(start, rise, Interval::point(t)?)?.mul(sign)?)
    };
    let (first, last) = (v_at(t0)?, v_at(t1)?);
    let (lo, hi) = (
        smaller(first.lo(), last.lo()),
        larger(first.hi(), last.hi()),
    );
    let (u, w) = ParamSurface::domain(cylinder);
    let Some(v) = within(Interval::new(lo, hi)?, w)? else {
        return proven(Vec::new(), meter).map(Some);
    };
    let overlap = Item::Overlap {
        at: ParamBox::new(&[Interval::new(t0, t1)?, Interval::new(u.0, u.1)?, v])?,
        region: OverlapRegion {
            boundary: Vec::new(),
        },
        proof: Coincidence::Identical,
        class: certified(),
    };
    proven(alloc::vec![overlap], meter).map(Some)
}

/// The closed form for the pair, if one decides.
fn closed_form(
    curve: &CurveKind<3>,
    surface: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let seams = seams_of(surface);
    match (curve, surface) {
        (CurveKind::Line(l), SurfaceKind::Plane(p)) => line_plane(l, p, meter),
        (CurveKind::Line(l), SurfaceKind::Sphere(s)) => line_sphere(l, s, seams, meter),
        (CurveKind::Line(l), SurfaceKind::Cylinder(c)) => line_cylinder(l, c, seams, meter),
        (CurveKind::Line(l), SurfaceKind::Cone(c)) => line_cone(l, c, seams, meter),
        (CurveKind::Line(l), SurfaceKind::Torus(t)) => line_torus(l, t, seams, meter).map(Some),
        (_, SurfaceKind::Cylinder(c)) => helix_on_cylinder(curve, c, meter),
        _ => Ok(None),
    }
}

/// `surface`'s Bézier patches, where it has them. Clipping only speeds the engine, so a form it
/// cannot take goes without.
fn patches(
    surface: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<BernsteinPatch<3>>, IntersectError> {
    match surface {
        SurfaceKind::Nurbs(s) => match s.bernstein(meter) {
            Ok(list) => Ok(list),
            Err(NurbsError::NotClamped) => Ok(Vec::new()),
            Err(e) => Err(CurveError::from(e).into()),
        },
        _ => Ok(Vec::new()),
    }
}

/// Where `curve` meets `surface` (12.6).
///
/// # Errors
///
/// A kind's errors, and the meter's stop: every pair is answered, so
/// [`IntersectError::BudgetExceeded`] means the budget ran out, not that the pair is unsupported.
pub fn curve_surface(
    curve: &CurveKind<3>,
    surface: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    meter.poll()?;
    if let Some(answer) = closed_form(curve, surface, meter)? {
        return Ok(answer);
    }
    let [seam_u, seam_v] = seams_of(surface);
    let seam_t = seam_of(curve, finite(curve)?);
    let field = Contact {
        curve,
        surface,
        seams: [seam_t, seam_u, seam_v],
        pieces: crate::curve::pieces(curve, meter)?,
        patches: patches(surface, meter)?,
    };
    let domain = [seam_t.side()?, seam_u.side()?, seam_v.side()?];
    let solved = solve(&field, domain, |_, _| Ok(true), meter)?;
    finished(&field, solved, meter)
}
