//! Where a curve meets a surface (chapter 12.6).
//!
//! What it does: [`curve_surface`] answers for any curve and surface. A line with a plane is a linear
//! equation, a line with a sphere, cylinder or cone a quadratic in the surface's own coordinates, and
//! a line with a torus a quartic, isolated by [`crate::solve`], each with its count from an exact sign
//! or a Sturm chain where the data are whole. A helix on a coaxial cylinder of its radius is an
//! overlap, and so is each arc of a circle lying in a plane that the plane's domain holds, and a
//! circle about the axis of a cylinder or cone at its radius there. A circle about a line through
//! a sphere's centre that stands off the sphere meets nothing, by one exact sign (decision
//! 32.74). Every other pair runs [`crate::solve`] on C(t) − S(u, v), with the Interval Projected
//! Polyhedron step narrowing the boxes of NURBS pieces.
//!
//! Guarantees:
//!
//! - **Every root found or flagged.** A box is discarded only on a proof, so a meeting no item reports
//!   does not exist.
//! - **Exact where the data are.** On whole-number data the count, and whether a root is double, come
//!   from exact signs, within the budgets of chapter 6.2. Whether a line runs along a plane, and
//!   whether it lies in it, are exact signs of the stored doubles on any data whose products of
//!   three are exact (decision 32.85). Elsewhere an interval decides, and what it cannot tell goes
//!   to the engine.
//! - **Grazing is read, not guessed.** Where the tangent cannot be excluded from the surface's
//!   tangent plane the box is a candidate, and the second order test compares the curve's normal
//!   curvature with the surface's: [`Unresolved::TangentBand`] with its width where they differ,
//!   [`Unresolved::HigherOrderContact`] where they cannot be told apart, and
//!   [`Unresolved::NearCoincident`] for a long run, a curve lying near the surface.
//! - **Overlap by identity.** A line in a plane or along a ruling, a helix on a coaxial cylinder of
//!   its radius, and a circle lying in a plane or on a coaxial cylinder or cone, are
//!   [`Item::Overlap`]s with [`Coincidence::Identical`]. Whether a circle lies on the surface is
//!   decided by exact signs of sums of products of the stored doubles, never by an enclosure. A
//!   circle the signs cannot place goes to the engine as before: on a cone, or at an end of a
//!   round kind's domain, where the axis is off the coordinate axes; where a round kind's domain
//!   is short of a whole turn; and on a sphere (decision 32.64). A plane's domain that holds a
//!   part of a whole circle holds arcs of it: which arcs is decided by exact signs where the
//!   plane's vectors run along coordinate axes, each arc is an overlap over an enclosure of its
//!   stretch of the circle, and a lone point of the circle in the domain, or a plane on other
//!   vectors, is the engine's (decision 32.69). An arc's end on the circle's own join is placed
//!   by an exact sign, so it is the domain's own end exactly and no stretch stands past the join
//!   unless the arc passes it; where an end's enclosure reaches the join and the frame's x runs
//!   off the plane's axes, or the join is off x's line, the circle is the engine's. So is a
//!   circle whose stretches would meet, as where a side stands within a rounding of touching it
//!   and the enclosures of its two crossings cannot tell them apart (decision 32.80).
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
//! reach for a transversal root, each a few evaluations; a clip is O(p·q). A curve lying on the
//! surface that no closed form takes costs the engine boxes in proportion to its length over the
//! resolution floor, each flagged: a circle of radius 207 lying in a plane cost 1,164,603 work
//! units (46,647 boxes) before its closed form, and one on an axis off the coordinate axes still
//! does, bounded by the work budget (issue 974). A circle of radius 600 in a plane whose domain
//! holds an arc of it cost 1,252,043 units before decision 32.69; now the signs of at most four
//! sides and eight crossings, and one box of the circle for each arc. A circle of radius 1 about
//! a line through the centre of a sphere of radius 256, standing 0.002 off it all the way round,
//! cost 782,639 units before decision 32.74: the engine's boxes under that gap in three
//! variables. A segment lying in a plane whose vectors hold a half ran the engine past 2^20 units,
//! and one along it 0.48 to 8.3 off it took 720,686 to 919,038 to say nothing, before decision
//! 32.85; now two exact sums of 6 and 12 products of three doubles, at most 1 unit, formed only
//! where an enclosure of the same value holds zero. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: t the curve's parameter, u and v the surface's, o and d a \\
              line, c and n a frame's centre and axis"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_curve::tight::{self, larger, smaller};
use geom_curve::{
    Circle, CurveError, CurveKind, DerivativeBound, Frame, HelixAxes, Line, ParamCurve,
};
use geom_num::Sign;
use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::grid::{P3, Steps};
use geom_num::interval::Interval;
use geom_num::isolate::RESOLUTION_FLOOR;
use geom_num::sturm::Roots;
use geom_num::{math, predicate, quadric};
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
use crate::quadrics::{off_line, same_line};
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

/// A line with a plane. Parallel, and lying in it, are exact signs, on whole-number data and on
/// any stored doubles whose products of three are exact (decision 32.85); a line that is neither
/// meets it where a determinant proven apart from zero puts it. `None` where neither tells.
fn line_plane(
    line: &Line<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (step, offset) = plane_enclosures(line, plane)?;
    let domain = ParamSurface::domain(plane);
    // A step proven apart from zero is a line across the plane: its enclosure's sign is the true
    // one, so the exact signs are asked only where the enclosure holds zero.
    if apart(step).is_none() {
        // Parallel, and lying in it, only from the exact signs: every operation rounds outward,
        // so an enclosure of the step or the offset is never the one point zero.
        if let Some((Sign::Zero, side)) = plane_signs(line, plane, step, offset)? {
            if side != Sign::Zero {
                return proven(Vec::new(), meter).map(Some);
            }
            return in_plane(line, plane, meter).map(Some);
        }
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

/// Enclosures of a line's step against a plane, n·d with n = a × b, and of the plane's offset from
/// the line's origin, n·(c − o), from the stored doubles.
fn plane_enclosures(line: &Line<3>, plane: &Plane) -> Result<(Interval, Interval), IntersectError> {
    let (c, a, b) = plane.vectors();
    let (o, d) = (line.origin(), line.direction());
    let normal = cross(lift(a)?, lift(b)?)?;
    let step = dot(normal, lift(d)?)?;
    let offset = dot(normal, minus(lift(c)?, lift(o)?)?)?;
    Ok((step, offset))
}

/// The sign of the value `x` encloses, where `x` excludes zero: every real in it has the sign, the
/// true value among them. `None` where `x` holds zero, and only an exact sign can tell.
fn apart(x: Interval) -> Option<Sign> {
    if x.contains(0.0) {
        None
    } else {
        x.certain_sign()
    }
}

/// A line's direction against a plane and which side its origin lies on, exactly: on whole-number
/// data by `orient3d` on the stored points, and otherwise by the signs of the stored doubles
/// (decision 32.85), where `step` and `offset`, their enclosures, cannot tell. `None` where a
/// product of three of the doubles would not be exact.
fn plane_signs(
    line: &Line<3>,
    plane: &Plane,
    step: Interval,
    offset: Interval,
) -> Result<Option<(Sign, Sign)>, IntersectError> {
    if let Some(signs) = grid_signs(line, plane)? {
        return Ok(Some(signs));
    }
    Ok(stored_signs(line, plane, step, offset))
}

/// The six products of three whose sum is `w · (a × b)`, each a triple of stored doubles.
fn triple_terms(w: [f64; 3], a: [f64; 3], b: [f64; 3]) -> [[f64; 3]; 6] {
    let ([w0, w1, w2], [a0, a1, a2], [b0, b1, b2]) = (w, a, b);
    [
        [w0, a1, b2],
        [-w0, a2, b1],
        [w1, a2, b0],
        [-w1, a0, b2],
        [w2, a0, b1],
        [-w2, a1, b0],
    ]
}

/// The sign of Σ aᵢ·bᵢ·cᵢ, which `enclosure` holds: the enclosure's where it excludes zero, and
/// otherwise the exact one. `None` where a factor is far enough toward either end of the doubles'
/// range that a product of three would not be exact.
fn sign_of_triples(terms: &[[f64; 3]], enclosure: Interval) -> Option<Sign> {
    if !terms.iter().flatten().all(|x| tame(*x)) {
        return None;
    }
    Some(apart(enclosure).unwrap_or_else(|| exact_sign_of_triples(terms)))
}

/// The sign of Σ aᵢ·bᵢ·cᵢ, exactly, for tame factors: a·b is a double and its error, each times c
/// a double and its error, and the sum of the four is exact.
fn exact_sign_of_triples(terms: &[[f64; 3]]) -> Sign {
    let mut sum = ExactSum::new();
    for &[a, b, c] in terms {
        let (p, e) = two_prod(a, b);
        let (pp, pe) = two_prod(p, c);
        let (ep, ee) = two_prod(e, c);
        sum.add_slice(&[pp, pe, ep, ee]);
    }
    let v = sum.round_ieee();
    if v > 0.0 {
        Sign::Positive
    } else if v < 0.0 {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

/// A line's direction against a plane, n·d with n = a × b, and the side its origin lies on,
/// n·(c − o), each the sign of a sum of products of three stored doubles. The plane is
/// c + u·a + v·b and the line o + t·d, so these doubles are the two kinds exactly, whether or
/// not they are whole numbers. Where `step` or `offset`, the enclosures of the two, excludes zero,
/// its sign is the true one and no exact sum is formed.
fn stored_signs(
    line: &Line<3>,
    plane: &Plane,
    step: Interval,
    offset: Interval,
) -> Option<(Sign, Sign)> {
    let (c, a, b) = plane.vectors();
    let (o, d) = (line.origin(), line.direction());
    let along = sign_of_triples(&triple_terms(d, a, b), step)?;
    let side = sign_of_triples(&side_terms(c, o, a, b), offset)?;
    Some((along, side))
}

/// The twelve products of three whose sum is `(c − o) · (a × b)`: `c`'s six and `−o`'s.
fn side_terms(c: [f64; 3], o: [f64; 3], a: [f64; 3], b: [f64; 3]) -> [[f64; 3]; 12] {
    let [o0, o1, o2] = o;
    let [ahead, behind] = [triple_terms(c, a, b), triple_terms([-o0, -o1, -o2], a, b)];
    let mut terms = [[0.0; 3]; 12];
    for (slot, term) in terms.iter_mut().zip(ahead.iter().chain(&behind)) {
        *slot = *term;
    }
    terms
}

/// A line's direction against a plane and which side its origin lies on, on whole-number data.
fn grid_signs(line: &Line<3>, plane: &Plane) -> Result<Option<(Sign, Sign)>, IntersectError> {
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
    // The point at v stands s·v along n̂, so the height z is at v = z/s (z·s only for s = ±1, as
    // built; a similarity scales the axial factor).
    let factor = Interval::point(cylinder.axial_sign())?;
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let v = z.div(factor)?;
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
    // The height z is at v = z/s.
    let factor = Interval::point(cylinder.axial_sign())?;
    let (start, end) = finite_line(line)?;
    let v_of = |t: f64| -> Result<Interval, IntersectError> {
        Ok(along(oz, dz, Interval::point(t)?)?.div(factor)?)
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

/// A line with a cone: ρ = r₀ + k·v on the axis at z = s·v, so ρ = r₀ + (k/s)·z, ρ² against
/// x² + y².
fn line_cone(
    line: &Line<3>,
    cone: &Cone,
    seams: [Seam; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let ([ox, oy, oz], [dx, dy, dz]) = local(line, cone.center(), &cone.frame())?;
    let (r0, k) = cone.radius_and_slope();
    // k/s and z/s, not k·s and z·s: the two agree only for s = ±1, as built, and a similarity
    // scales the axial factor.
    let factor = Interval::point(cone.axial_sign())?;
    let (r0, k) = (Interval::point(r0)?, Interval::point(k)?);
    let sigma = k.div(factor)?;
    let (e, f) = (r0.add(sigma.mul(oz)?)?, sigma.mul(dz)?);
    let terms = [
        dx.square()?.add(dy.square()?)?.sub(f.square()?)?,
        ox.mul(dx)?.add(oy.mul(dy)?)?.sub(e.mul(f)?)?,
        ox.square()?.add(oy.square()?)?.sub(e.square()?)?,
    ];
    let domain = ParamSurface::domain(cone);
    let place = |t: Interval| -> Result<Option<(Interval, Interval)>, IntersectError> {
        let (x, y, z) = (along(ox, dx, t)?, along(oy, dy, t)?, along(oz, dz, t)?);
        let v = z.div(factor)?;
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
        let Some(c) = self.curve.enclose_point(t, Limit::FromRight, meter)? else {
            return Err(IntersectError::Internal);
        };
        let sides = (Limit::FromRight, Limit::FromRight);
        let [s, ..] = self.surface.enclose(u, v, sides, meter)?;
        minus(c, s)
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
    // The cylinder's point at v stands s·v along n̂: the height is at v = height/s.
    let factor = Interval::point(cylinder.axial_sign())?;
    let (t0, t1) = curve.domain();
    let v_at = |t: f64| -> Result<Interval, IntersectError> {
        Ok(along(start, rise, Interval::point(t)?)?.div(factor)?)
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

/// The height of `p` over `c` along the unit vector of `n`, as two exact terms, where `n` runs along
/// one coordinate axis: there the unit vector is that axis, signed, and the height is a difference
/// of two stored doubles. `None` for any other direction: its unit vector is no vector of doubles
/// (the only ones of length exactly one are the signed axes), so the height is no sum of products
/// of stored doubles. With the terms, whether that axis is taken positive (`true`) or negative.
fn height_terms(n: [f64; 3], p: [f64; 3], c: [f64; 3]) -> Option<(bool, [(f64, f64); 2])> {
    let [a, b, e] = n;
    let [p0, p1, p2] = p;
    let [c0, c1, c2] = c;
    let (along, at, from) = match (a != 0.0, b != 0.0, e != 0.0) {
        (true, false, false) => (a, p0, c0),
        (false, true, false) => (b, p1, c1),
        (false, false, true) => (e, p2, c2),
        _ => return None,
    };
    // The match picked the one component that is not zero.
    let up = positive(along);
    let sigma = if up { 1.0 } else { -1.0 };
    Some((up, [(sigma, at), (-sigma, from)]))
}

/// Whether `x` is positive, where its caller knows it is neither zero nor NaN: read from its sign
/// bit, so no comparison with a zero that cannot occur is written.
fn positive(x: f64) -> bool {
    x.is_sign_positive()
}

/// The sign of `(p − c) · n`, exactly: a sum of six products of stored doubles.
fn side_of(p: [f64; 3], c: [f64; 3], n: [f64; 3]) -> Sign {
    let [p0, p1, p2] = p;
    let [c0, c1, c2] = c;
    let [n0, n1, n2] = n;
    sign_of(&[
        (p0, n0),
        (-c0, n0),
        (p1, n1),
        (-c1, n1),
        (p2, n2),
        (-c2, n2),
    ])
}

/// The whole of `curve`, a circle lying on the surface, as an overlap over `u` and `v`.
fn lying(
    curve: &CurveKind<3>,
    u: Interval,
    v: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let (t0, t1) = curve.domain();
    let overlap = Item::Overlap {
        at: ParamBox::new(&[Interval::new(t0, t1)?, u, v])?,
        region: OverlapRegion {
            boundary: Vec::new(),
        },
        proof: Coincidence::Identical,
        class: certified(),
    };
    proven(alloc::vec![overlap], meter)
}

/// The plane's parameters over the stretch `t` of `curve`, enclosed: its box there, read in the
/// plane's own vectors.
fn in_parameters(
    curve: &CurveKind<3>,
    plane: &Plane,
    t: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<(Interval, Interval), IntersectError> {
    let (c, a, b) = plane.vectors();
    let w = minus(sides(&curve.bound(t, meter)?)?, lift(c)?)?;
    let (a, b) = (lift(a)?, lift(b)?);
    let (aa, ab, bb) = (dot(a, a)?, dot(a, b)?, dot(b, b)?);
    let (wa, wb) = (dot(w, a)?, dot(w, b)?);
    let det = aa.mul(bb)?.sub(ab.mul(ab)?)?;
    let u = wa.mul(bb)?.sub(wb.mul(ab)?)?.div(det)?;
    let v = wb.mul(aa)?.sub(wa.mul(ab)?)?.div(det)?;
    Ok((u, v))
}

/// A circle lying in a plane: an overlap for each part of it the plane's domain holds, nothing
/// where it holds none of it, and `None`, the engine's, where the circle stands off the plane or
/// [`arcs_in_plane`] cannot decide.
///
/// Theorem. A circle of positive radius (a circle kind has no other) holds three points not on one
/// line, so exactly one plane holds it: the plane through its centre across its frame's stored
/// normal n, which defines the frame exactly. It lies in the plane c + u·a + v·b if and only if
/// that is the same plane: n·a = 0, n·b = 0 (a and b are not parallel, so n is then the plane's
/// normal) and (centre − c)·n = 0. Each is the sign of a sum of products of stored doubles, exact.
fn circle_in_plane(
    curve: &CurveKind<3>,
    circle: &Circle<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (c, a, b) = plane.vectors();
    let n = circle.frame().normal();
    let zero = [0.0; 3];
    let signs = [
        side_of(a, zero, n),
        side_of(b, zero, n),
        side_of(circle.center(), c, n),
    ];
    if signs.iter().any(|s| *s != Sign::Zero) {
        return Ok(None);
    }
    // The circle's box, read in the plane's parameters, holds every (u, v) of the circle.
    let (u, v) = in_parameters(curve, plane, curve.domain(), meter)?;
    let ((u0, u1), (v0, v1)) = ParamSurface::domain(plane);
    if u.hi() < u0 || u.lo() > u1 || v.hi() < v0 || v.lo() > v1 {
        return proven(Vec::new(), meter).map(Some);
    }
    if u.lo() < u0 || u.hi() > u1 || v.lo() < v0 || v.hi() > v1 {
        return arcs_in_plane(curve, circle, plane, meter);
    }
    lying(curve, u, v, meter).map(Some)
}

/// A coordinate of a point of a circle, measured from the circle's centre: its sign, its square as
/// a sum of products of doubles (exact), and an enclosure of it.
#[derive(Clone)]
struct Coord {
    sign: Sign,
    square: Vec<(f64, f64)>,
    near: Interval,
}

impl Coord {
    /// What [`order`] reads of it: its sign and its square.
    fn exact(&self) -> Exact<'_> {
        (self.sign, &self.square)
    }
}

/// A coordinate's sign and its square as a sum of products of doubles: all that [`order`] reads.
type Exact<'a> = (Sign, &'a [(f64, f64)]);

/// The sign of `a − b`, exactly: the signs where they differ, the squares where they agree.
fn order(a: Exact<'_>, b: Exact<'_>) -> Sign {
    let squares = |p: Exact<'_>, q: Exact<'_>| {
        let mut terms = p.1.to_vec();
        terms.extend(q.1.iter().map(|&(x, y)| (-x, y)));
        sign_of(&terms)
    };
    match (a.0, b.0) {
        (Sign::Positive, Sign::Positive) => squares(a, b),
        (Sign::Negative, Sign::Negative) => squares(b, a),
        (Sign::Zero, Sign::Zero) => Sign::Zero,
        (Sign::Positive, _) | (Sign::Zero, Sign::Negative) => Sign::Positive,
        (Sign::Negative, _) | (Sign::Zero, Sign::Positive) => Sign::Negative,
    }
}

/// Whether products of pairs of doubles this size are exact: zero, or far from both ends of the
/// doubles' range.
fn tame(x: f64) -> bool {
    x == 0.0 || (1e-75..=1e75).contains(&x) || (-1e75..=-1e-75).contains(&x)
}

/// The coordinate `from + end·along − at` as a [`Coord`]: four doubles whose sum is it exactly
/// (the product is a double and its error), so its sign is an exact sum and its square sixteen
/// products. `None` where a product of the doubles would not be exact.
fn offset(from: f64, end: f64, along: f64, at: f64) -> Result<Option<Coord>, IntersectError> {
    let (p, e) = two_prod(end, along);
    let sum = [from, p, e, -at];
    if !(tame(end) && tame(along) && sum.iter().all(|x| tame(*x))) {
        return Ok(None);
    }
    let sign = sign_of(&sum.map(|x| (1.0, x)));
    let mut square = Vec::new();
    let mut near = Interval::point(0.0)?;
    for x in sum {
        square.extend(sum.map(|y| (x, y)));
        near = near.add(Interval::point(x)?)?;
    }
    Ok(Some(Coord { sign, square, near }))
}

/// The other coordinate of the circle's point whose one coordinate is `b`, on the side `sign`:
/// its square is r² − b².
fn across(b: &Coord, r: f64, sign: Sign) -> Result<Coord, IntersectError> {
    let mut square = alloc::vec![(r, r)];
    square.extend(b.square.iter().map(|&(x, y)| (-x, y)));
    let r = Interval::point(r)?;
    let d = r.mul(r)?.sub(b.near.mul(b.near)?)?;
    // The square is positive where this is called; its enclosure may reach below zero.
    let root = Interval::new(larger(d.lo(), 0.0), larger(d.hi(), 0.0))?.sqrt()?;
    let near = if sign == Sign::Negative {
        root.neg()
    } else {
        root
    };
    Ok(Coord { sign, square, near })
}

/// A point of the circle, from its centre, in the two coordinates of its plane.
#[derive(Clone)]
struct Spot {
    x: Coord,
    y: Coord,
}

/// The order of two points of the circle going round it from (r, 0) towards (0, r): the upper
/// half (y ≥ 0) first, by falling x, then the lower half by rising x. Exact, and equal only for
/// one point.
fn round(p: &Spot, q: &Spot) -> Ordering {
    let upper = |s: &Spot| s.y.sign != Sign::Negative;
    let by = |s: Sign| match s {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    };
    match (upper(p), upper(q)) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (true, true) => by(order(q.x.exact(), p.x.exact())),
        (false, false) => by(order(p.x.exact(), q.x.exact())),
    }
}

/// The angle of `s` from (r, 0) towards (0, r), in half-turns on [0, 2), enclosed. `turn_of`
/// writes a point of the upper half in place; a point of the lower half is the mirror of one of
/// the upper half in the first axis, so its angle is two half-turns less the mirror's. No
/// threshold on an enclosure decides it (decision 32.80). Its cost: where x is surely negative,
/// the enclosure is up to two units in the last place wider at its lower end, and one at its
/// upper, than `turn_of`'s own moved a turn on; it holds the true angle everywhere.
fn angle(s: &Spot) -> Result<Interval, IntersectError> {
    if s.y.sign == Sign::Negative {
        let mirror = turn_of(s.x.near, s.y.near.neg())?;
        Ok(Interval::point(2.0)?.sub(mirror)?)
    } else {
        Ok(turn_of(s.x.near, s.y.near)?)
    }
}

/// One side of a rectangle: the half-plane where the coordinate `axis` (0 or 1) is at least `at`
/// (`keep` positive) or at most `at` (`keep` negative).
struct Half {
    axis: usize,
    at: Coord,
    keep: Sign,
}

impl Half {
    /// Whether `s` is in the half-plane, its line included.
    fn holds(&self, s: &Spot) -> bool {
        let side = order(
            if self.axis == 0 { &s.x } else { &s.y }.exact(),
            self.at.exact(),
        );
        side == Sign::Zero || side == self.keep
    }
}

/// What a rectangle holds of a circle.
enum Held {
    /// None of it.
    Nothing,
    /// All of it.
    Whole,
    /// These arcs of it, each from its first point round to its second in [`round`]'s direction.
    Arcs(Vec<(Spot, Spot)>),
}

/// What the rectangle of `sides` holds of the circle of radius `r` about the origin of their
/// coordinates, by exact signs. `None` where it holds a lone point: a touch from outside, at a
/// side or a corner.
///
/// Theorem. A half-plane whose line crosses the circle holds one closed arc of it, from the
/// crossing where the circle enters to the one where it leaves, and both crossings are
/// transversal. One whose line misses or touches the circle holds all of it, none of it, or the
/// one point of a touch from outside. The rectangle is the intersection of its half-planes, so the
/// points where the circle meets the rectangle's edge are among the crossings that every other
/// half-plane holds, and between two such points next to each other round the circle the open arc
/// meets no edge: it is all inside or all outside. It is inside exactly when no half-plane's
/// leaving crossing lies on it or at its start, since both its ends are in every half-plane and
/// each half-plane's part is one arc. A touch from inside is no crossing and cuts nothing.
fn held_of(r: f64, sides: &[Half]) -> Result<Option<Held>, IntersectError> {
    match crossed_by(r, sides) {
        None => Ok(None),
        Some(Err(all)) => Ok(Some(if all { Held::Whole } else { Held::Nothing })),
        Some(Ok(crossed)) => between(r, &crossed),
    }
}

/// The sides whose lines the circle crosses, where every other side holds all of it; `Err` where
/// that settles the answer, all of the circle (`true`) or none of it; `None` for a touch from
/// outside.
#[allow(
    clippy::type_complexity,
    reason = "three outcomes of one private step, read at once by its one caller"
)]
fn crossed_by(r: f64, sides: &[Half]) -> Option<Result<Vec<&Half>, bool>> {
    // The circle's coordinate runs over [−r, r]: its ends' signs, and their square r².
    let square = [(r, r)];
    let (far, back) = ((Sign::Positive, &square[..]), (Sign::Negative, &square[..]));
    let mut crossed = Vec::new();
    let mut touched = false;
    for half in sides {
        let (low, high) = (order(half.at.exact(), back), order(half.at.exact(), far));
        let (whole, none, touch) = if half.keep == Sign::Positive {
            (
                low != Sign::Positive,
                high == Sign::Positive,
                high == Sign::Zero,
            )
        } else {
            (
                high != Sign::Negative,
                low == Sign::Negative,
                low == Sign::Zero,
            )
        };
        if none {
            return Some(Err(false));
        }
        touched |= touch;
        if !whole && !touch {
            crossed.push(half);
        }
    }
    if touched {
        return None;
    }
    Some(if crossed.is_empty() {
        Err(true)
    } else {
        Ok(crossed)
    })
}

/// The arcs of the circle inside every half-plane of `crossed`, each crossed by it: the second
/// half of [`held_of`].
fn between(r: f64, crossed: &[&Half]) -> Result<Option<Held>, IntersectError> {
    let mut spots = Vec::new();
    let mut leaving = Vec::new();
    for (k, half) in crossed.iter().enumerate() {
        for sign in [Sign::Positive, Sign::Negative] {
            let other = across(&half.at, r, sign)?;
            let (spot, leaves) = if half.axis == 0 {
                (
                    Spot {
                        x: half.at.clone(),
                        y: other,
                    },
                    sign == half.keep,
                )
            } else {
                (
                    Spot {
                        x: other,
                        y: half.at.clone(),
                    },
                    sign != half.keep,
                )
            };
            if leaves {
                leaving.push(spot.clone());
            }
            if crossed
                .iter()
                .enumerate()
                .all(|(j, g)| j == k || g.holds(&spot))
            {
                spots.push(spot);
            }
        }
    }
    spots.sort_by(round);
    spots.dedup_by(|p, q| round(p, q) == Ordering::Equal);
    let count = spots.len();
    if count == 0 {
        return Ok(Some(Held::Nothing));
    }
    let mut arcs = Vec::new();
    let mut inside = Vec::new();
    for (from, to) in spots.iter().zip(spots.iter().cycle().skip(1)) {
        let wraps = round(from, to) != Ordering::Less;
        let leaves = leaving.iter().any(|e| {
            let (after, before) = (
                round(from, e) != Ordering::Greater,
                round(e, to) == Ordering::Less,
            );
            if wraps {
                after || before
            } else {
                after && before
            }
        });
        inside.push(!leaves);
        if !leaves {
            arcs.push((from.clone(), to.clone()));
        }
    }
    // A point with no arc at either hand is a lone point of the circle in the rectangle.
    let lone = inside
        .iter()
        .zip(inside.iter().cycle().skip(count.saturating_sub(1)))
        .any(|(next, last)| !next && !last);
    Ok(if lone { None } else { Some(Held::Arcs(arcs)) })
}

/// The coordinate axis `v` runs along, if it runs along one.
fn axis_of(v: [f64; 3]) -> Option<usize> {
    match v.map(|x| x != 0.0) {
        [true, false, false] => Some(0),
        [false, true, false] => Some(1),
        [false, false, true] => Some(2),
        _ => None,
    }
}

/// Entry `k` of `v`; the last for any `k` past it.
fn pick<T: Copy>(v: [T; 3], k: usize) -> T {
    let [a, b, c] = v;
    match k {
        0 => a,
        1 => b,
        _ => c,
    }
}

/// Which side of the circle's own join a point of the circle near the join is on: positive past
/// it (its parameter just over the domain's start `lo`), negative before it (just under the
/// domain's end), zero on it. `None` where no exact sign says so.
///
/// The join is the point at parameter `lo`. Where `lo` is a whole number it lies along the
/// frame's x̂ (even) or against it (odd), and a point near it is past it exactly when it stands on
/// the ŷ side of x̂'s line (even) or the other side (odd). `ahead` holds the frame's stored x
/// along the plane's two axes (the frame's normal runs along the third, so these two are all of
/// x̂'s direction). Where one of them is zero, x̂ is the other axis, signed, and ŷ = n̂ × x̂ is the
/// first one, signed: the side is the sign of one coordinate of the point, which a [`Spot`] holds
/// exactly. For any other `lo` or x the side is no sign of a sum of products of stored doubles.
fn past_join(
    spot: &Spot,
    ahead: (f64, f64),
    same: bool,
    lo: f64,
) -> Result<Option<Sign>, IntersectError> {
    if lo != math::floor(lo)?.get() {
        return Ok(None);
    }
    let half = lo / 2.0;
    let even = half == math::floor(half)?.get();
    let up = match (ahead.0 != 0.0, ahead.1 != 0.0) {
        (true, false) if positive(ahead.0) == same => spot.y.sign,
        (true, false) => spot.y.sign.negate(),
        (false, true) if positive(ahead.1) == same => spot.x.sign.negate(),
        (false, true) => spot.x.sign,
        _ => return Ok(None),
    };
    Ok(Some(if even { up } else { up.negate() }))
}

/// The stretches of the circle's domain `(lo, hi)`, one turn long, that an arc covers: one, or
/// one each side of the join where the arc passes it. `first` and `last` enclose the parameters
/// of the arc's ends `enter` and `leave`, written on any turn, `last` over `first` by the arc's
/// length; `ahead` and `same` are [`past_join`]'s. `None` where an end's enclosure reaches the
/// join and no exact sign places the end against it.
///
/// Whole turns bring the upper end of the start's enclosure onto the domain, short of its last
/// end, so the enclosure reaches the join only by its lower end, at the domain's first end. There
/// it cannot say which end of the domain the start is at, and the exact side of the join does: a
/// start before the join is a turn on, at the domain's last end; one on the join or past it is
/// at the first end. The arc has a stretch past the join only where its end is truly past it: an
/// end on the join is the domain's last end, and no enclosure's overhang makes a stretch.
#[allow(
    clippy::type_complexity,
    reason = "one or two stretches, or none placed: read at once by its one caller"
)]
fn stretches_of(
    (first, last): (Interval, Interval),
    (enter, leave): (&Spot, &Spot),
    (ahead, same): ((f64, f64), bool),
    (lo, hi): (f64, f64),
) -> Result<Option<[Option<(f64, f64)>; 2]>, IntersectError> {
    let turn = Interval::point(2.0)?;
    let back = Interval::point(2.0 * math::floor((first.hi() - lo) / 2.0)?.get())?;
    let (mut first, mut last) = (first.sub(back)?, last.sub(back)?);
    if first.lo() <= lo {
        let Some(side) = past_join(enter, ahead, same, lo)? else {
            return Ok(None);
        };
        if side == Sign::Negative {
            (first, last) = (first.add(turn)?, last.add(turn)?);
        }
    }
    let t0 = larger(first.lo(), lo);
    let through = if last.hi() < hi {
        false
    } else if last.lo() > hi {
        true
    } else {
        let Some(side) = past_join(leave, ahead, same, lo)? else {
            return Ok(None);
        };
        side == Sign::Positive
    };
    Ok(Some(if through {
        [
            Some((t0, hi)),
            Some((lo, smaller(last.sub(turn)?.hi(), hi))),
        ]
    } else {
        [Some((t0, smaller(last.hi(), hi))), None]
    }))
}

/// The parts of a whole circle lying in `plane` that the plane's domain holds, each an overlap
/// over its own stretch of the circle; `None`, the engine's, where the plane's vectors run off the
/// coordinate axes, the circle is not a whole turn, a product of the stored doubles would not be
/// exact, the domain holds a lone point of the circle ([`held_of`]), or an end of an arc has an
/// enclosure that reaches the circle's own join and no exact sign places it ([`past_join`]).
///
/// With the plane's vectors a and b along two coordinate axes, an end `e` of the domain in u is
/// the line where that coordinate is c + e·a, and its distance from the circle's centre is
/// c + e·a − centre: a sum of four doubles, exact ([`offset`]). Every question [`held_of`] asks is
/// then the sign of a sum of products of doubles. The parameters of the ends of each arc are
/// enclosures; which arcs there are is exact, and so is each arc's place against the join: an arc
/// that starts on the join starts at the domain's first end exactly, one that ends on it ends at
/// the domain's last end exactly, and there is a stretch past the join only where the arc passes
/// it. (A circle kind's domain is at most one turn long, so a whole one is exactly one turn.)
fn arcs_in_plane(
    curve: &CurveKind<3>,
    circle: &Circle<3>,
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (c, a, b) = plane.vectors();
    let (Some(i), Some(j)) = (axis_of(a), axis_of(b)) else {
        return Ok(None);
    };
    let (lo, hi) = curve.domain();
    let (q, r) = (circle.center(), circle.radius());
    if hi - lo < 2.0 || !tame(r) {
        return Ok(None);
    }
    let (du, dv) = ParamSurface::domain(plane);
    let mut sides = Vec::new();
    for (axis, k, along, end, first) in [
        (0, i, pick(a, i), du.0, true),
        (0, i, pick(a, i), du.1, false),
        (1, j, pick(b, j), dv.0, true),
        (1, j, pick(b, j), dv.1, false),
    ] {
        if !end.is_finite() {
            continue;
        }
        let Some(at) = offset(pick(c, k), end, along, pick(q, k))? else {
            return Ok(None);
        };
        // `along` is the one component of the plane's vector that is not zero.
        let keep = if positive(along) == first {
            Sign::Positive
        } else {
            Sign::Negative
        };
        sides.push(Half { axis, at, keep });
    }
    let arcs = match held_of(r, &sides)? {
        None => return Ok(None),
        Some(Held::Nothing) => return proven(Vec::new(), meter).map(Some),
        Some(Held::Whole) => {
            let (u, v) = in_parameters(curve, plane, (lo, hi), meter)?;
            return lying(curve, held(u, du)?, held(v, dv)?, meter).map(Some);
        }
        Some(Held::Arcs(arcs)) => arcs,
    };
    // The circle's parameter is the angle from its frame's x̂ towards n̂ × x̂, in half-turns: the
    // angle of [`round`] less x̂'s own where the plane's axes turn the same way as the frame,
    // and x̂'s less it where they turn the other way.
    let (_, [x, _, _]) = circle.frame().unit_vectors()?;
    let zero = turn_of(pick(x, i), pick(x, j))?;
    let (third, cyclic) = match (i, j) {
        (0, 1) => (2, true),
        (1, 2) => (0, true),
        (2, 0) => (1, true),
        (1, 0) => (2, false),
        (2, 1) => (0, false),
        (0, 2) => (1, false),
        _ => return Ok(None),
    };
    // The circle lies in the plane, so its normal runs along the third axis: that component is
    // the one that is not zero.
    let same = cyclic == positive(pick(circle.frame().normal(), third));
    let stored = circle.frame().x();
    let ahead = (pick(stored, i), pick(stored, j));
    let turn = Interval::point(2.0)?;
    let mut items = Vec::new();
    for (from, to) in &arcs {
        let start = angle(from)?;
        let end = if round(from, to) == Ordering::Less {
            angle(to)?
        } else {
            angle(to)?.add(turn)?
        };
        // In the circle's own parameter the arc runs from `enter` to `leave`.
        let (first, last, enter, leave) = if same {
            (start.sub(zero)?, end.sub(zero)?, from, to)
        } else {
            (zero.sub(end)?, zero.sub(start)?, to, from)
        };
        let Some(stretches) = stretches_of((first, last), (enter, leave), (ahead, same), (lo, hi))?
        else {
            return Ok(None);
        };
        for (t0, t1) in stretches.into_iter().flatten() {
            let (u, v) = in_parameters(curve, plane, (t0, t1), meter)?;
            items.push(Item::Overlap {
                at: ParamBox::new(&[Interval::new(t0, t1)?, held(u, du)?, held(v, dv)?])?,
                region: OverlapRegion {
                    boundary: Vec::new(),
                },
                proof: Coincidence::Identical,
                class: certified(),
            });
        }
    }
    // Two stretches whose enclosures meet hold crossings the enclosures cannot tell apart, as
    // where a side stands within a rounding of touching the circle: the engine's (32.80).
    let apart = items.iter().enumerate().all(|(k, a)| {
        let later = items.get(k.saturating_add(1)..).unwrap_or_default();
        later.iter().all(|b| !a.at().meets(b.at()))
    });
    if !apart {
        return Ok(None);
    }
    proven(items, meter).map(Some)
}

/// A circle about a round kind's axis and of its radius there: an overlap at the v of its height,
/// over every u, where the domain holds that v; nothing where it does not; and `None`, the
/// engine's, where the domain is short of a whole turn (it may hold a part of the circle) or an
/// enclosure cannot place v against an end and the axis is off the coordinate axes.
///
/// The point at v stands s·v along the unit axis, so the circle's v is its height over the centre
/// divided by s, and v against an end `e` of the domain is the sign of (height − s·e) with the
/// sign of s: exact where [`height_terms`] has the height.
fn ring_overlap(
    curve: &CurveKind<3>,
    at: [f64; 3],
    (center, frame, factor): ([f64; 3], Frame<3>, f64),
    (du, dv): ((f64, f64), (f64, f64)),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    if du.1 - du.0 < 2.0 {
        return Ok(None);
    }
    let (_, [_, _, n]) = frame.unit_vectors()?;
    let height = dot(minus(lift(at)?, lift(center)?)?, n)?;
    let v = height.div(Interval::point(factor)?)?;
    if v.hi() < dv.0 || v.lo() > dv.1 {
        return proven(Vec::new(), meter).map(Some);
    }
    let exact = height_terms(frame.normal(), at, center);
    for (end, inside, outward) in [
        (dv.0, v.lo() >= dv.0, Sign::Negative),
        (dv.1, v.hi() <= dv.1, Sign::Positive),
    ] {
        if inside {
            continue;
        }
        let Some((_, [p, q])) = exact else {
            return Ok(None);
        };
        let side = sign_of(&[p, q, (-factor, end)]);
        // `factor` is the kind's axial sign, 1 or -1.
        let side = if positive(factor) {
            side
        } else {
            side.negate()
        };
        if side == outward {
            return proven(Vec::new(), meter).map(Some);
        }
    }
    lying(curve, Interval::new(du.0, du.1)?, held(v, dv)?, meter).map(Some)
}

/// A circle lying on a cylinder.
///
/// Theorem. A plane across a right circular cylinder's axis cuts it in the circle of its radius
/// about the axis; a plane tilted from that cuts it in an ellipse whose half-axes differ; and a
/// plane along the axis in at most two lines. A circle of positive radius lies in one plane only
/// and shares at most four points with any other conic, so it lies on the cylinder if and only if
/// its normal is parallel to the axis, its centre is on the axis, and its radius is the
/// cylinder's: the first two exact signs ([`same_line`]), the third two stored doubles compared.
fn circle_on_cylinder(
    curve: &CurveKind<3>,
    circle: &Circle<3>,
    cylinder: &Cylinder,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (center, frame) = (cylinder.center(), cylinder.frame());
    let coaxial = same_line(
        center,
        frame.normal(),
        circle.center(),
        circle.frame().normal(),
    );
    if !coaxial || circle.radius() != cylinder.radius() {
        return Ok(None);
    }
    ring_overlap(
        curve,
        circle.center(),
        (center, frame, cylinder.axial_sign()),
        ParamSurface::domain(cylinder),
        meter,
    )
}

/// A circle lying on a cone.
///
/// Theorem. A plane across a right circular cone's axis at height h over its centre cuts it in
/// the circle of radius |r₀ + k·h/s| about the axis; a plane tilted from that cuts it in a conic of
/// eccentricity above zero, or through the apex in a point or lines. So, as for a cylinder, a
/// circle of positive radius r lies on the cone if and only if its normal is parallel to the
/// axis, its centre is on the axis, and r₀·s + k·h = r·s or −r·s. With the axis along a coordinate
/// axis h is a difference of two stored doubles and each equation the sign of a sum of four
/// products, exact; on any other axis h is no such sum, and the circle is left to the engine.
fn circle_on_cone(
    curve: &CurveKind<3>,
    circle: &Circle<3>,
    cone: &Cone,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (center, frame) = (cone.center(), cone.frame());
    let at = circle.center();
    if !same_line(center, frame.normal(), at, circle.frame().normal()) {
        return Ok(None);
    }
    let Some((up, [(_, p), (_, c)])) = height_terms(frame.normal(), at, center) else {
        return Ok(None);
    };
    let (r0, k) = cone.radius_and_slope();
    let (s, r) = (cone.axial_sign(), circle.radius());
    // The slope along the axis as the height is taken: k, or −k where the axis runs negative.
    let rise = if up { k } else { -k };
    // The two equations r₀·s + k·h = r·s and = −r·s, as r₀·s + k·h + q·s = 0 for q = −r and r.
    let on_it = [r, -r]
        .iter()
        .any(|&q| sign_of(&[(r0, s), (rise, p), (-rise, c), (q, s)]) == Sign::Zero);
    if !on_it {
        return Ok(None);
    }
    ring_overlap(
        curve,
        at,
        (center, frame, s),
        ParamSurface::domain(cone),
        meter,
    )
}

/// A circle about a line through a sphere's centre that stands off the sphere: nothing (decision
/// 32.74).
///
/// Theorem. Every point p of a circle of centre q and radius r is the same distance from any
/// point s of its axis, the line through q along its normal: |p − s|² = r² + |q − s|². So with
/// the sphere's centre s on that axis the whole circle lies on the sphere of radius R where
/// r² + |q − s|² = R², and no point of it is on the sphere otherwise, whatever the two domains.
/// Whether s is on the axis, and whether the two sides are equal, are each the sign of a sum of
/// products of stored doubles, exact. A circle that lies on the sphere, one whose axis misses
/// the centre, and one with a number whose products would not be exact stay the engine's.
fn circle_about_sphere(
    circle: &Circle<3>,
    sphere: &Sphere,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let (q, r) = (circle.center(), circle.radius());
    let (s, big) = (sphere.center(), sphere.radius());
    let exact = [r, big].iter().chain(&q).chain(&s).all(|x| tame(*x));
    if !exact || off_line(s, q, circle.frame().normal()) {
        return Ok(None);
    }
    // r² − R² + |q − s|², a product at a time.
    let mut terms = alloc::vec![(r, r), (-big, big)];
    for (a, b) in q.into_iter().zip(s) {
        terms.extend([(a, a), (b, b), (-a, b), (-b, a)]);
    }
    if sign_of(&terms) == Sign::Zero {
        return Ok(None);
    }
    proven(Vec::new(), meter).map(Some)
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
        (CurveKind::Circle(k), SurfaceKind::Plane(p)) => circle_in_plane(curve, k, p, meter),
        (CurveKind::Circle(k), SurfaceKind::Cylinder(c)) => circle_on_cylinder(curve, k, c, meter),
        (CurveKind::Circle(k), SurfaceKind::Cone(c)) => circle_on_cone(curve, k, c, meter),
        (CurveKind::Circle(k), SurfaceKind::Sphere(s)) => circle_about_sphere(k, s, meter),
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::float_cmp,
        reason = "test code: a failure should stop loudly, and the doubles compared are exact"
    )]

    use super::*;

    /// A point of the circle whose coordinates have the exact signs `sx`, `sy` and the enclosures
    /// `x`, `y`.
    fn spot(sx: Sign, x: Interval, sy: Sign, y: Interval) -> Spot {
        let coord = |sign, near| Coord {
            sign,
            square: Vec::new(),
            near,
        };
        Spot {
            x: coord(sx, x),
            y: coord(sy, y),
        }
    }

    fn exactly(x: f64) -> Interval {
        Interval::point(x).unwrap()
    }

    #[test]
    fn a_lower_half_point_whose_x_is_not_surely_negative_is_its_mirror_a_turn_on() {
        // Lower right: two half-turns less its mirror's 0.25, about 1.75.
        let right = angle(&spot(
            Sign::Positive,
            exactly(1.0),
            Sign::Negative,
            exactly(-1.0),
        ))
        .unwrap();
        assert!(right.lo() > 1.74 && right.hi() < 1.76, "{right:?}");
        // Straight down, x's enclosure ending exactly at zero: 1.5.
        let down = angle(&spot(
            Sign::Zero,
            exactly(0.0),
            Sign::Negative,
            exactly(-1.0),
        ))
        .unwrap();
        assert!(down.lo() > 1.49 && down.hi() < 1.51, "{down:?}");
    }

    #[test]
    fn a_lower_half_point_whose_x_is_surely_negative_is_its_mirror_a_turn_on() {
        // Lower left: two half-turns less its mirror's 0.75, about 1.25.
        let left = angle(&spot(
            Sign::Negative,
            exactly(-1.0),
            Sign::Negative,
            exactly(-1.0),
        ))
        .unwrap();
        assert!(left.lo() > 1.24 && left.hi() < 1.26, "{left:?}");
        // x surely negative by one step below zero: about 1.5.
        let edge = Interval::new(-1e-300, 0.0_f64.next_down()).unwrap();
        let near = angle(&spot(Sign::Negative, edge, Sign::Negative, exactly(-1.0))).unwrap();
        assert!(near.lo() > 1.49 && near.hi() < 1.51, "{near:?}");
    }

    #[test]
    fn an_upper_half_point_is_kept_on_either_side() {
        let left = angle(&spot(
            Sign::Negative,
            exactly(-1.0),
            Sign::Positive,
            exactly(1.0),
        ))
        .unwrap();
        assert!(left.lo() > 0.74 && left.hi() < 0.76, "{left:?}");
        let right = angle(&spot(
            Sign::Positive,
            exactly(1.0),
            Sign::Positive,
            exactly(1.0),
        ))
        .unwrap();
        assert!(right.lo() > 0.24 && right.hi() < 0.26, "{right:?}");
    }

    /// A double off the whole numbers as often as on them: a binary fraction, or a rounded decimal.
    fn double(rng: &mut geom_test::SplitMix64) -> f64 {
        let m = f64::from(i32::try_from(rng.range_i64(-4096, 4096).unwrap()).unwrap());
        match rng.range_i64(0, 3).unwrap() {
            0 => m,
            1 => m / 64.0,
            2 => m * 0.1,
            _ => m / 3.0,
        }
    }

    fn vector(rng: &mut geom_test::SplitMix64) -> [f64; 3] {
        [double(rng), double(rng), double(rng)]
    }

    /// `p + s·q + r·w`, rounded.
    fn along(p: [f64; 3], s: f64, q: [f64; 3], r: f64, w: [f64; 3]) -> [f64; 3] {
        let mut out = p;
        for ((x, e), f) in out.iter_mut().zip(q).zip(w) {
            *x += s * e + r * f;
        }
        out
    }

    #[test]
    fn the_enclosures_signs_are_the_exact_signs_wherever_they_exclude_zero() {
        // Random planes and lines, a third of the lines parallel to the plane by construction
        // (a direction that is a vector of it times a power of two) and a third of the origins
        // rounded from points of it, so that both enclosures straddle zero on many pairs.
        let mut rng = geom_test::SplitMix64::new(geom_test::seed::fixed(
            "line_plane filter",
            geom_test::Seed::DEFAULT,
        ));
        let count: u32 = if cfg!(miri) { 200 } else { 20_000 };
        let (mut decided, mut straddled) = ([0_u32; 2], [0_u32; 2]);
        for _ in 0..count {
            let (c, a, b) = (vector(&mut rng), vector(&mut rng), vector(&mut rng));
            let Ok(plane) = Plane::new(c, a, b, ((-1e3, 1e3), (-1e3, 1e3))) else {
                continue;
            };
            let d = match rng.range_i64(0, 2).unwrap() {
                0 => vector(&mut rng),
                1 => along([0.0; 3], 0.25, a, 0.0, b),
                _ => along([0.0; 3], 1.0, a, double(&mut rng), b),
            };
            let o = if rng.coin() {
                vector(&mut rng)
            } else {
                along(c, double(&mut rng), a, double(&mut rng), b)
            };
            let Ok(line) = Line::new(o, d, (0.0, 1.0)) else {
                continue;
            };
            let (step, offset) = plane_enclosures(&line, &plane).unwrap();
            let terms: [&[[f64; 3]]; 2] = [&triple_terms(d, a, b), &side_terms(c, o, a, b)];
            for (k, (terms, enclosure)) in terms.into_iter().zip([step, offset]).enumerate() {
                let exact = exact_sign_of_triples(terms);
                match apart(enclosure) {
                    Some(sign) => {
                        assert_eq!(sign, exact, "{k}: {o:?} {d:?} {c:?} {a:?} {b:?}");
                        decided[k] += 1;
                    }
                    None => straddled[k] += 1,
                }
                assert_eq!(sign_of_triples(terms, enclosure), Some(exact));
            }
            let signs = stored_signs(&line, &plane, step, offset).unwrap();
            assert_eq!(
                signs,
                (
                    exact_sign_of_triples(terms[0]),
                    exact_sign_of_triples(terms[1])
                )
            );
        }
        // Both ways taken often for both signs (natively about a third and two thirds of the step's,
        // and half each of the offset's).
        for k in 0..2 {
            let often = |n: u32| n > count / 10;
            assert!(
                often(decided[k]) && often(straddled[k]),
                "{decided:?} {straddled:?}"
            );
        }
    }

    #[test]
    fn the_sign_of_a_number_known_not_to_be_zero_is_its_sign_bit() {
        assert!(positive(1.0));
        assert!(positive(f64::MIN_POSITIVE));
        assert!(!positive(-1.0));
        assert!(!positive(-f64::MIN_POSITIVE));
    }
}
