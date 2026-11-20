//! Where a curve meets a surface (chapter 12.6).
//!
//! What it does: [`curve_surface`] answers for any curve and surface. A line with a plane is a linear
//! equation, a line with a sphere, cylinder or cone a quadratic in the surface's own coordinates, and
//! a line with a torus a quartic, isolated by [`crate::solve`], each with its count from an exact sign
//! or a Sturm chain where the data are whole. A helix on a coaxial cylinder of its radius is an
//! overlap, and so is each arc of a circle lying in a plane that the plane's domain holds, and a
//! circle about the axis of a cylinder or cone at its radius there. A circle about a line through
//! a sphere's centre that stands off the sphere meets nothing, by one exact sign (decision
//! 32.74). A circle across the axis of a cylinder or cone, its normal parallel to the axis, meets
//! nothing where its plane is past the domain, and otherwise meets it where it meets the circle the
//! surface cuts in that plane: nothing, two crossings or one touch, told by enclosures and, on a
//! coordinate axis, by exact signs (decision 32.86). Every other pair runs [`crate::solve`] on
//! C(t) − S(u, v), with the Interval Projected Polyhedron step narrowing the boxes of NURBS pieces.
//!
//! Guarantees:
//!
//! - **Every root found or flagged.** A box is discarded only on a proof, so a meeting no item reports
//!   does not exist.
//! - **Exact where the data are.** On whole-number data the count, and whether a root is double, come
//!   from exact signs, within the budgets of chapter 6.2. Whether a line runs along a plane, and
//!   whether it lies in it, are exact signs of the stored doubles on any data whose products of
//!   three are exact (decision 32.85). Where the enclosure of a line's discriminant against a
//!   cylinder, or a cone whose radius at v = 0 is zero, reaches zero, its sign is exact on any
//!   stored doubles whose products of eight are exact, and the discriminant is then enclosed
//!   from its exact sum: a line through the apex or tangent is one point, and one a step either
//!   side two or none (decision 32.87). For a cone of any other start radius the discriminant is
//!   P + Q·√N, and its sign is exact in whole numbers of the stored doubles of at most 768 bits,
//!   with P² against Q²·N in 1,536 where the signs of P and Q differ (decision 32.88). Elsewhere
//!   an interval decides, and what it cannot tell goes to the engine.
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
//! - **Crossings across an axis are proven or left.** A circle across a parallel round axis has
//!   points only where enclosures prove the two circles of its plane crossing, or exact signs on a
//!   coordinate axis prove them crossing or touching, and only with its crossings' boxes apart. A
//!   plane whose enclosure reaches an end of the domain meets nothing where the two circles are
//!   proven apart for every height it holds, and is the engine's where they are not. A touch off
//!   the coordinate axes, a plane through a cone's apex, and a domain short of a whole turn are
//!   the engine's (decision 32.86).
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
//! where an enclosure of the same value holds zero.
//! An end circle of a cone of radius 344 to 856 across another cone on a parallel axis
//! cost the engine 258,258 to 1,759,429 units before decision 32.86, for nothing or two points;
//! now a few predicates, and a box of the circle for each point. A plane's edge through a cone's
//! apex, or tangent to a cylinder off the grid, cost the engine 483,244 to 4,147,759 units before
//! decision 32.87; now 4, and where the enclosure of the discriminant reaches zero, one exact sum
//! of at most 432 products of six or eight doubles, each at most 128 doubles on the stack.
//! A plane's edge beside a cone of start radius 100 off the grid cost the engine 3,824,869
//! units, and a segment touching a start circle 387,503 to 597,337, before decision 32.88; now
//! at most 4, and where the enclosure reaches zero 50 products of 768-bit integers and three
//! of 1,536-bit ones, on the stack. Recursion: none.

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
use geom_num::wide::Wide;
use geom_num::{math, predicate, quadric};
use geom_nurbs::{BernsteinCurve, BernsteinPatch, Bounds, Direction, Limit, NurbsError};
use geom_run::WorkMeter;
use geom_surface::{
    Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus, partial_index,
};

use crate::closed::{self, Meeting};
use crate::curve::{
    Crossing, Seam, bound_over, certified, cut, exact_steps, finite, grid_box, grid_line,
    grid_point, held, onto, proven, quadratic_with, seam_of, sign_of, slope_over, span, turn_of,
    whole, widest,
};
use crate::error::IntersectError;
use crate::quadrics::{off_line, parallel, plus, same_line, scale};
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

/// The roots of the line with A·t² + 2B·t + C = 0 on a quadric, as items: counted by `exact`
/// where it is known, with `known` for the discriminant where an exact sum gave it.
fn quadric_roots(
    line: &Line<3>,
    terms: [Interval; 3],
    (exact, known): (Option<Meeting>, Option<Interval>),
    seams: [Seam; 2],
    place: impl Fn(Interval) -> Result<Option<(Interval, Interval)>, IntersectError> + Copy,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let Some(roots) = quadratic_with(terms, exact, known)? else {
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
        (exact, None),
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

/// The axis's two points, the centre `c` and `c + n`, n the stored normal, if `c` and `n` are both
/// on the grid and `c + n` is in reach. The far point is summed in integers, not doubles (#1063):
/// near 2^39 a double holds only 2^-13, so a normal's fraction under half of that rounds away in
/// `c + n`, the sum lands on the grid, and the closed forms would be handed an upright axis for a
/// tilted surface.
fn on_axis(center: &[f64; 3], frame: &Frame<3>) -> Option<(P3, P3)> {
    let (c, n) = (grid_point(center)?, grid_point(&frame.normal())?);
    let along = P3::new(
        c.x.get().checked_add(n.x.get())?,
        c.y.get().checked_add(n.y.get())?,
        c.z.get().checked_add(n.z.get())?,
    )
    .ok()?;
    Some((c, along))
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
    // Whole-number data too: the exact sum counts as `closed::line_cylinder` would, and holds Δ to
    // a step besides, which a count alone does not, so a near tangent's two roots stand apart.
    let known = if straddles(terms)? {
        let n = lift(cylinder.frame().normal())?;
        stored_count(cylinder_excess(line, cylinder), dot(n, n)?)?
    } else {
        (None, None)
    };
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
    quadric_roots(line, terms, known, seams, place, meter)
}

/// Whether the enclosure of the discriminant B² − A·C reaches zero, so that it cannot count the
/// roots.
fn straddles([a, b, c]: [Interval; 3]) -> Result<bool, IntersectError> {
    let d = b.square()?.sub(a.mul(c)?)?;
    Ok(d.lo() <= 0.0 && d.hi() >= 0.0)
}

/// The count from an exact multiple of the discriminant, `excess` = `by`·Δ with `by` positive,
/// and an enclosure of Δ two steps of `excess` wide. `(None, None)` where no exact sum was taken.
fn stored_count(
    excess: Option<f64>,
    by: Interval,
) -> Result<(Option<Meeting>, Option<Interval>), IntersectError> {
    let Some(e) = excess else {
        return Ok((None, None));
    };
    let meeting = if e > 0.0 {
        Meeting::Twice
    } else if e < 0.0 {
        Meeting::Nowhere
    } else {
        Meeting::Tangent
    };
    let known = Interval::new(e.next_down(), e.next_up())?.div(by)?;
    Ok((Some(meeting), Some(known)))
}

/// Whether products of up to [`MOST_FACTORS`] doubles this size are exact as sums of doubles:
/// zero, or between 10⁻²⁴ and 10²⁴ in size. Every part of such a product is a whole multiple of
/// the product of the factors' last places, each at least 2⁻¹³², so of 2⁻¹⁰⁵⁶, which a double
/// holds, and none is near 2¹⁰²⁴.
fn mild(x: f64) -> bool {
    x == 0.0 || (1e-24..=1e24).contains(&x) || (-1e24..=-1e-24).contains(&x)
}

/// The most stored doubles in one product of an exact sum here.
const MOST_FACTORS: usize = 8;

/// The most doubles such a product is the sum of: 2^(`MOST_FACTORS` − 1).
const MOST_PARTS: usize = 128;

/// Adds the product of `factors`, at most [`MOST_FACTORS`] stored doubles, to `sum` exactly: each
/// factor multiplies every part so far into a double and its error (`two_prod`), so the product
/// is the sum of at most 128 doubles, held on the stack. `None` where a factor is not [`mild`] or
/// there are too many.
fn add_product(sum: &mut ExactSum, factors: &[f64]) -> Option<()> {
    let (&first, rest) = factors.split_first()?;
    if rest.len() >= MOST_FACTORS || !factors.iter().all(|x| mild(*x)) {
        return None;
    }
    let mut parts = [0.0; MOST_PARTS];
    *parts.first_mut()? = first;
    let mut count: usize = 1;
    for &f in rest {
        let mut next = [0.0; MOST_PARTS];
        for (i, &part) in parts.iter().take(count).enumerate() {
            let (p, e) = two_prod(part, f);
            let at = i.checked_mul(2)?;
            *next.get_mut(at)? = p;
            *next.get_mut(at.checked_add(1)?)? = e;
        }
        parts = next;
        count = count.checked_mul(2)?;
    }
    sum.add_slice(parts.get(..count)?);
    Some(())
}

/// Adds `±(Σ mᵢ)² · Π scale` to `sum` exactly, each mᵢ a product of stored doubles: every
/// product mᵢ·mⱼ·Π scale in turn, negated where `negative`.
fn add_square<const K: usize>(
    sum: &mut ExactSum,
    monomials: &[[f64; K]],
    scale: &[f64],
    negative: bool,
) -> Option<()> {
    for p in monomials {
        for q in monomials {
            let mut factors = [0.0; MOST_FACTORS];
            let all = p.iter().chain(q).chain(scale);
            let count = all.clone().count();
            for (slot, x) in factors.iter_mut().zip(all) {
                *slot = *x;
            }
            if negative {
                let first = factors.first_mut()?;
                *first = -*first;
            }
            add_product(sum, factors.get(..count)?)?;
        }
    }
    Some(())
}

/// The twelve products of three stored doubles whose sum is W·(d × n), W = o − c.
fn skew_terms(o: [f64; 3], c: [f64; 3], d: [f64; 3], n: [f64; 3]) -> [[f64; 3]; 12] {
    let [c0, c1, c2] = c;
    let mut terms = [[0.0; 3]; 12];
    let parts = triple_terms(o, d, n);
    let behind = triple_terms([-c0, -c1, -c2], d, n);
    for (slot, term) in terms.iter_mut().zip(parts.iter().chain(&behind)) {
        *slot = *term;
    }
    terms
}

/// The components of a × b, each as two products of two stored doubles.
fn cross_parts(a: [f64; 3], b: [f64; 3]) -> [[[f64; 2]; 2]; 3] {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [
        [[a1, b2], [-a2, b1]],
        [[a2, b0], [-a0, b2]],
        [[a0, b1], [-a1, b0]],
    ]
}

/// N·Δ for a line and a cylinder, exactly, rounded once: Δ is the discriminant of the line's
/// quadratic in the cylinder's own coordinates and N = n·n, n the stored axis. A line meets the
/// cylinder where it comes within r of the axis, and its distance from the axis is
/// |W·(d × n)| / |d × n|, W = o − c, so N·Δ = r²·|d × n|² − (W·(d × n))², a sum of products of
/// six stored doubles. `None` where a product would not be exact.
fn cylinder_excess(line: &Line<3>, cylinder: &Cylinder) -> Option<f64> {
    let (o, d) = (line.origin(), line.direction());
    let (c, n, r) = (
        cylinder.center(),
        cylinder.frame().normal(),
        cylinder.radius(),
    );
    let mut sum = ExactSum::new();
    for part in cross_parts(d, n) {
        add_square(&mut sum, &part, &[r, r], false)?;
    }
    add_square(&mut sum, &skew_terms(o, c, d, n), &[], true)?;
    let e = sum.round_ieee();
    e.is_finite().then_some(e)
}

/// How a line meets a cylinder, exactly, where their data are whole. [`line_cylinder`] reads only
/// whether it lies along a ruling: the count is the exact sum's, which encloses Δ too.
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
    let started = r0 != 0.0;
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
    // Whole-number data too: the exact sums count as `closed::line_cone` would, and hold Δ to a
    // step besides, which a count alone does not, so a near tangent's two roots stand apart.
    let known = if straddles(terms)? {
        if started {
            started_count(line, cone)
        } else {
            let n = lift(cone.frame().normal())?;
            let by = dot(n, n)?.mul(factor.square()?)?;
            stored_count(cone_excess(line, cone), by)?
        }
    } else {
        (None, None)
    };
    quadric_roots(line, terms, known, seams, place, meter)
}

/// The components of (o − c) × d, each as four products of two stored doubles.
fn cross_parts_from(o: [f64; 3], c: [f64; 3], d: [f64; 3]) -> [[[f64; 2]; 4]; 3] {
    let ([o0, o1, o2], [c0, c1, c2], [d0, d1, d2]) = (o, c, d);
    [
        [[o1, d2], [-c1, d2], [-o2, d1], [c2, d1]],
        [[o2, d0], [-c2, d0], [-o0, d2], [c0, d2]],
        [[o0, d1], [-c0, d1], [-o1, d0], [c1, d0]],
    ]
}

/// s²·N·Δ for a line and a cone whose radius at v = 0 is zero, exactly, rounded once: Δ is the
/// discriminant of the line's quadratic in the cone's own coordinates, N = n·n with n the stored
/// axis, s the axial factor and k the slope. The apex is then the stored centre c, and the plane
/// through it and the line, of normal m = W × d with W = o − c, cuts the double cone in two
/// rulings, touches it along one, or holds the apex alone as k²·N·|m|² is above, equal to or
/// below (s² + k²)·(m·n)²: s²·N·Δ is their difference, a sum of products of eight stored
/// doubles. A line through the apex has m = 0, so Δ = 0: it meets the cone there alone, or lies
/// along a ruling. For a cone whose radius at v = 0 is zero only (a cone of another start radius
/// is [`started_count`]'s); `None` where a product would not be exact.
fn cone_excess(line: &Line<3>, cone: &Cone) -> Option<f64> {
    let (_, k) = cone.radius_and_slope();
    let (o, d) = (line.origin(), line.direction());
    let (c, n, s) = (cone.center(), cone.frame().normal(), cone.axial_sign());
    let mut sum = ExactSum::new();
    for part in cross_parts_from(o, c, d) {
        for x in n {
            add_square(&mut sum, &part, &[k, k, x, x], false)?;
        }
    }
    let skew = skew_terms(o, c, d, n);
    add_square(&mut sum, &skew, &[s, s], true)?;
    add_square(&mut sum, &skew, &[k, k], true)?;
    let e = sum.round_ieee();
    e.is_finite().then_some(e)
}

/// The width of the whole numbers of [`started_count`]: 768 bits. Each factor there is below
/// 2^127 ([`whole_numbers`]), and P and Q, sums of products of eight and seven, fit it while each
/// kind's whole numbers stay below about 2^90; past that a step is `None` (the engine's).
type Part = Wide<12>;

/// The width of P² − Q²·N: 1,536 bits, twice [`Part`]'s.
type Square = Wide<24>;

/// The fraction field of a double's bits: the low 52 bits.
const FRACTION: u64 = (1 << 52) - 1;

/// A normal double's hidden bit, above its 52 fraction bits.
const HIDDEN: i64 = 1 << 52;

/// 2^32, the place of the upper half of a 64-bit limb.
const TWO_POW_32: f64 = 4_294_967_296.0;

/// A finite double as m·2^e exactly, m odd and below 2^53 in size, or (0, 0) for a zero: the
/// significand with its hidden bit and the exponent less the bias and the 52 fraction bits (IEEE
/// 754-2019, section 3.3; the subnormals have no hidden bit and the exponent of the least normal
/// one). `None` for an infinity or NaN. (The same as `geom_math`'s `exact::dyadic`, which that
/// crate keeps to itself.)
fn dyadic(x: f64) -> Option<(i64, i32)> {
    if !x.is_finite() {
        return None;
    }
    let bits = x.to_bits();
    let fraction = i64::try_from(bits & FRACTION).ok()?;
    let field = i32::try_from((bits >> 52) & 0x7ff).ok()?;
    // A subnormal, of field 0, has no hidden bit and the exponent of the least normal double.
    let m = if field == 0 {
        fraction
    } else {
        fraction.checked_add(HIDDEN)?
    };
    let e = field.max(1).checked_sub(1075)?;
    if m == 0 {
        return Some((0, 0));
    }
    // m ≠ 0 and below 2^53, so it has at most 52 trailing zeros.
    let zeros = m.trailing_zeros();
    let m = m.checked_shr(zeros)?;
    let m = if x.is_sign_negative() {
        m.checked_neg()?
    } else {
        m
    };
    Some((m, e.checked_add(i32::try_from(zeros).ok()?)?))
}

/// Stored doubles of one kind as whole numbers, exactly: each times 2^−e, with e the exponent of
/// the least last place among them, and e. `None` where one would reach 2^127 (they span more
/// than about 127 bits) or is not finite.
fn whole_numbers<const K: usize>(xs: [f64; K]) -> Option<([Part; K], i32)> {
    let mut parts = [(0, 0); K];
    for (slot, x) in parts.iter_mut().zip(xs) {
        *slot = dyadic(x)?;
    }
    let least = parts
        .iter()
        .filter(|(m, _)| *m != 0)
        .map(|&(_, e)| e)
        .min()
        .unwrap_or(0);
    let mut whole = [Part::ZERO; K];
    for (slot, (m, e)) in whole.iter_mut().zip(parts) {
        if m != 0 {
            let shift = u32::try_from(e.checked_sub(least)?).ok()?;
            *slot = Part::from_i128(i128::from(m).checked_mul(2_i128.checked_pow(shift)?)?);
        }
    }
    Some((whole, least))
}

/// a·b for whole-number vectors, exactly; `None` where it does not fit.
fn wide_dot(a: &[Part; 3], b: &[Part; 3]) -> Option<Part> {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    a0.checked_mul(b0)?
        .checked_add(&a1.checked_mul(b1)?)?
        .checked_add(&a2.checked_mul(b2)?)
}

/// a × b for whole-number vectors, exactly; `None` where it does not fit.
fn wide_cross(a: &[Part; 3], b: &[Part; 3]) -> Option<[Part; 3]> {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    let minus = |p: &Part, q: &Part, r: &Part, s: &Part| -> Option<Part> {
        p.checked_mul(q)?.checked_sub(&r.checked_mul(s)?)
    };
    Some([
        minus(a1, b2, a2, b1)?,
        minus(a2, b0, a0, b2)?,
        minus(a0, b1, a1, b0)?,
    ])
}

/// 2^e as a double, for the normal powers of two only (e from −1022 to 1023), from its bits.
fn pow2(e: i32) -> Option<f64> {
    let field = u64::try_from(e.checked_add(1023)?).ok()?;
    (1..=2046)
        .contains(&field)
        .then(|| f64::from_bits(field << 52))
}

/// A whole number enclosed: its limbs' halves summed exactly and rounded once to nearest, then
/// widened a step either way. `None` where it does not fit a double.
fn enclose<const L: usize>(w: &Wide<L>) -> Option<Interval> {
    let negative = w.is_negative();
    let magnitude = if negative { w.checked_neg()? } else { *w };
    let mut sum = ExactSum::new();
    let mut place = 1.0;
    for limb in magnitude.to_le_limbs() {
        for half in [limb & 0xffff_ffff, limb >> 32] {
            if half != 0 {
                sum.add(f64::from(u32::try_from(half).ok()?) * place);
            }
            place *= TWO_POW_32;
        }
    }
    let r = sum.round_ieee();
    let r = if negative { -r } else { r };
    if !r.is_finite() {
        return None;
    }
    Interval::new(r.next_down(), r.next_up()).ok()
}

/// The count of a line on a cone of any start radius r₀, from the exact sign of its
/// discriminant, and an enclosure of the discriminant (or `None` where its doubles cannot hold
/// it). `(None, None)` where a whole number does not fit.
///
/// With W = o − c, m = W × d, N = n·n, s the axial factor and k the slope, Δ is
/// (P + Q·√N) / (s²·N), where P = k²·N·|m|² − (s² + k²)·(m·n)² + r₀²·s²·|d × n|², a sum of
/// products of eight stored doubles, and Q = 2·r₀·k·s·((d·d)(W·n) − (W·d)(d·n)), of seven. (For
/// r₀ = 0 it is [`cone_excess`]'s.) Where P and Q do not have opposite signs, Δ has the sign of
/// P + Q; where they do, P + Q·√N has P's sign times that of P² − Q²·N, which is then also
/// (P² − Q²·N) / (P − Q·√N) with no cancellation in either part.
///
/// Each stored double is m·2^e exactly, so each of the four kinds of data (o, c and r₀; d; n;
/// k and s) times one power of two is a whole number, and P and Q·√N are homogeneous in each kind
/// (of degrees two, two, two and two), so the scaling keeps the sign, and Δ only gains the power
/// of two of the first two kinds, squared. P, Q and N are then whole numbers of at most 768 bits
/// and P² − Q²·N of at most 1,536 ([`Part`], [`Square`]).
///
/// Cost: four conversions of at most seven doubles, 50 products and 30 sums or differences of
/// 768-bit integers, three products and a difference of 1,536-bit ones where the signs differ, and
/// the enclosure's four or five conversions and a few interval steps. No allocation: every number
/// is on the stack.
fn started_count(line: &Line<3>, cone: &Cone) -> (Option<Meeting>, Option<Interval>) {
    match started_sign(line, cone) {
        Some((Sign::Positive, known)) => (Some(Meeting::Twice), known),
        Some((Sign::Negative, known)) => (Some(Meeting::Nowhere), known),
        Some((Sign::Zero, known)) => (Some(Meeting::Tangent), known),
        None => (None, None),
    }
}

/// The sign of Δ for [`started_count`], and its enclosure where the doubles hold it.
fn started_sign(line: &Line<3>, cone: &Cone) -> Option<(Sign, Option<Interval>)> {
    let ([o0, o1, o2], [c0, c1, c2]) = (line.origin(), cone.center());
    let (r0, k) = cone.radius_and_slope();
    let ([o0, o1, o2, c0, c1, c2, r0], lengths) = whole_numbers([o0, o1, o2, c0, c1, c2, r0])?;
    let (d, along) = whole_numbers(line.direction())?;
    let (n, _) = whole_numbers(cone.frame().normal())?;
    let ([k, s], _) = whole_numbers([k, cone.axial_sign()])?;
    let w = [
        o0.checked_sub(&c0)?,
        o1.checked_sub(&c1)?,
        o2.checked_sub(&c2)?,
    ];
    let (m, dn) = (wide_cross(&w, &d)?, wide_cross(&d, &n)?);
    let (big_n, mn) = (wide_dot(&n, &n)?, wide_dot(&m, &n)?);
    let (kk, ss) = (k.checked_mul(&k)?, s.checked_mul(&s)?);
    let p = kk
        .checked_mul(&big_n)?
        .checked_mul(&wide_dot(&m, &m)?)?
        .checked_sub(&ss.checked_add(&kk)?.checked_mul(&mn.checked_mul(&mn)?)?)?
        .checked_add(
            &r0.checked_mul(&r0)?
                .checked_mul(&ss)?
                .checked_mul(&wide_dot(&dn, &dn)?)?,
        )?;
    let q = wide_dot(&d, &d)?
        .checked_mul(&wide_dot(&w, &n)?)?
        .checked_sub(&wide_dot(&w, &d)?.checked_mul(&wide_dot(&d, &n)?)?)?
        .checked_mul(&r0.checked_mul(&k)?.checked_mul(&s)?)?;
    let q = q.checked_add(&q)?;
    let opposite = matches!(
        (p.signum(), q.signum()),
        (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive)
    );
    let gap = if opposite {
        let (p2, q2) = (p.resize::<24>()?, q.resize::<24>()?);
        let n2: Square = big_n.resize()?;
        Some(
            p2.checked_mul(&p2)?
                .checked_sub(&q2.checked_mul(&q2)?.checked_mul(&n2)?)?,
        )
    } else {
        None
    };
    let sign = match &gap {
        Some(e) => p.signum().mul(e.signum()),
        None => p.checked_add(&q)?.signum(),
    };
    let scale = lengths.checked_add(along)?.checked_mul(2)?;
    Some((
        sign,
        started_delta([&p, &q, &big_n, &ss], gap.as_ref(), scale),
    ))
}

/// Δ = (P + Q·√N) / (s²·N) · 2^`scale` enclosed, through (P² − Q²·N) / (P − Q·√N) where `gap`
/// holds P² − Q²·N; `None` where a step leaves the doubles.
fn started_delta(
    [p, q, big_n, ss]: [&Part; 4],
    gap: Option<&Square>,
    scale: i32,
) -> Option<Interval> {
    let (p, q, big_n, ss) = (enclose(p)?, enclose(q)?, enclose(big_n)?, enclose(ss)?);
    let q_root = q.mul(big_n.sqrt().ok()?).ok()?;
    let top = match gap {
        Some(e) => enclose(e)?.div(p.sub(q_root).ok()?).ok()?,
        None => p.add(q_root).ok()?,
    };
    let unit = Interval::point(pow2(scale)?).ok()?;
    top.div(ss.mul(big_n).ok()?).ok()?.mul(unit).ok()
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
    let regions = runs(solved.flagged, &[Seam::open(stretch)])?;
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
    let [seam_u, seam_v] = seams;
    let seams = [Seam::open(stretch), seam_u, seam_v];
    Intersection::on(items, coverage, Vec::new(), &seams, meter)
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
pub(crate) fn seams_of(surface: &SurfaceKind) -> [Seam; 2] {
    let (u, v) = ParamSurface::domain(surface);
    match surface {
        SurfaceKind::Torus(_) => [Seam::turns(u), Seam::turns(v)],
        SurfaceKind::Sphere(_) | SurfaceKind::Cylinder(_) | SurfaceKind::Cone(_) => {
            [Seam::turns(u), Seam::open(v)]
        }
        _ => [Seam::open(u), Seam::open(v)],
    }
}

/// A stretch of a surface's domain: its range in u and its range in v.
type Stretch = ((f64, f64), (f64, f64));

impl Contact<'_> {
    /// Every stretch of the surface's domain that `u` × `v` covers, `u`'s stretches outer: at
    /// most two in each parameter, so at most four, held in a fixed array with no heap block (the
    /// engine asks for them on every box it bounds). A slot is `None` where a parameter has one
    /// stretch only.
    fn regions(&self, u: Interval, v: Interval) -> [Option<Stretch>; 4] {
        let [_, seam_u, seam_v] = self.seams;
        let [p0, p1] = seam_u.spans(u);
        let [q0, q1] = seam_v.spans(v);
        [p0.zip(q0), p0.zip(q1), p1.zip(q0), p1.zip(q1)]
    }

    /// A box around the surface over `u` × `v`, across its joins.
    fn surface_box(
        &self,
        u: Interval,
        v: Interval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, IntersectError> {
        let mut out: Option<Bounds<3>> = None;
        for region in self.regions(u, v).into_iter().flatten() {
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
        for region in self.regions(u, v).into_iter().flatten() {
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
    // Over a run grown past where rounding swamps second derivatives, and on the domains: on a
    // whole turn a run may run past the window's end by under a turn (decision 32.104), and is
    // read modulo the turn, each stretch either side of the join on its own.
    let [seam_t, seam_u, seam_v] = field.seams;
    let [t, u, v] = grown(
        &[t, u, v],
        &[seam_t.reach()?, seam_u.reach()?, seam_v.reach()?],
    )?;
    // Written back as an answer writes a span: one grown a turn wide or more is the whole turn,
    // read once, not again over the stretch past the end that it overlaps.
    let [t, u, v] = [seam_t.onto(t)?, seam_u.onto(u)?, seam_v.onto(v)?];
    let mut slopes: [Option<Bounds<3>>; 2] = [None, None];
    for part in seam_t.spans(t).into_iter().flatten() {
        for (slot, order) in slopes.iter_mut().zip([1, 2]) {
            let DerivativeBound::Bounded(d) = field.curve.derivative_bound(part, order, meter)?
            else {
                return Ok(not_smooth);
            };
            *slot = Some(match slot.take() {
                Some(a) => a.union(&d).map_err(CurveError::from)?,
                None => d,
            });
        }
    }
    let [Some(d1), Some(d2)] = slopes else {
        return Err(IntersectError::Internal);
    };
    let (d1, d2) = (sides(&d1)?, sides(&d2)?);
    let limit = (Limit::FromRight, Limit::FromRight);
    let mut list: Vec<[Interval; 3]> = Vec::new();
    for region in field.regions(u, v).into_iter().flatten() {
        let Ok(part) = field.surface.partials_over(region, limit, 2, meter) else {
            return Ok(not_smooth);
        };
        if list.is_empty() {
            list = part;
            continue;
        }
        if part.len() != list.len() {
            return Err(IntersectError::Internal);
        }
        for (held, more) in list.iter_mut().zip(&part) {
            for (a, b) in held.iter_mut().zip(more) {
                *a = Interval::new(smaller(a.lo(), b.lo()), larger(a.hi(), b.hi()))?;
            }
        }
    }
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
    let regions = runs(flagged, &field.seams)?;
    let mut items = Vec::new();
    let mut seen: Vec<[Interval; 3]> = Vec::new();
    for z in &solved.zeros {
        let enclosure = held(z.enclosure)?;
        // A zero inside a run is part of what the run could not tell apart, and one on a join is
        // found from both sides of it: on a whole turn, modulo the turn (decision 32.104).
        let meeting = |hull: &[Interval; 3]| -> Result<bool, IntersectError> {
            let mut all = true;
            for ((seam, a), b) in field.seams.iter().zip(hull).zip(&enclosure) {
                all &= seam.meets(*a, *b)?;
            }
            Ok(all)
        };
        let mut known = false;
        for hull in regions.iter().map(|(hull, _)| hull).chain(&seen) {
            known |= meeting(hull)?;
        }
        if known {
            continue;
        }
        seen.push(enclosure);
        let [t, u, v] = enclosure;
        items.push(Item::Point {
            at: ParamBox::new(&enclosure)?,
            xyz: grid_box(&bound_over(field.curve, seam_t, t, meter)?)?,
            contact: boundary(field, [t, u, v]),
            class: certified(),
        });
    }
    for ([t, u, v], why) in regions {
        let image = bound_over(field.curve, seam_t, t, meter)?;
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
    Intersection::on(items, coverage, Vec::new(), &field.seams, meter)
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
    // Raw on purpose: a stretch through the join is two pieces, which meet only modulo the turn.
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
/// The circle's v is placed by [`height_across`].
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
    let v = match height_across(at, (center, frame, factor), dv)? {
        Across::Past => return proven(Vec::new(), meter).map(Some),
        Across::Within(v) | Across::Reaching(v) => v,
        Across::Unplaced(_) => return Ok(None),
    };
    lying(curve, Interval::new(du.0, du.1)?, held(v, dv)?, meter).map(Some)
}

/// Where the plane across a round kind's axis through a point stands against its domain in v.
enum Across {
    /// Past an end of the domain, by an enclosure or an exact sign: the plane meets none of it.
    Past,
    /// The plane's v, an enclosure inside the domain.
    Within(Interval),
    /// The plane's v, an enclosure that reaches an end, held to the domain: an exact sign says
    /// it is not past that end.
    Reaching(Interval),
    /// The plane's v, an enclosure that reaches an end, held to the domain: no exact sign places
    /// it, so the plane may be past that end, on it or inside it.
    Unplaced(Interval),
}

/// Where the plane across the axis of the round kind `(center, frame, factor)` through `at`
/// stands against the domain `dv` in v.
///
/// The point at v stands s·v along the unit axis, so the plane's v is the height of `at` over the
/// centre divided by s, and v against an end `e` of the domain is the sign of (height − s·e) with
/// the sign of s: exact where [`height_terms`] has the height.
fn height_across(
    at: [f64; 3],
    (center, frame, factor): ([f64; 3], Frame<3>, f64),
    dv: (f64, f64),
) -> Result<Across, IntersectError> {
    let (_, [_, _, n]) = frame.unit_vectors()?;
    let height = dot(minus(lift(at)?, lift(center)?)?, n)?;
    let v = height.div(Interval::point(factor)?)?;
    if v.hi() < dv.0 || v.lo() > dv.1 {
        return Ok(Across::Past);
    }
    if v.lo() >= dv.0 && v.hi() <= dv.1 {
        return Ok(Across::Within(v));
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
            return Ok(Across::Unplaced(held(v, dv)?));
        };
        let side = sign_of(&[p, q, (-factor, end)]);
        // `factor` is the kind's axial sign, 1 or -1.
        let side = if positive(factor) {
            side
        } else {
            side.negate()
        };
        if side == outward {
            return Ok(Across::Past);
        }
    }
    Ok(Across::Reaching(held(v, dv)?))
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

/// A cylinder or a cone, as [`circle_across`] reads it: its centre, frame and axial factor, its
/// radius at v = 0 and its slope (a cylinder's is zero), its domain and its seams.
struct RoundKind {
    center: [f64; 3],
    frame: Frame<3>,
    factor: f64,
    radius: f64,
    slope: f64,
    domain: ((f64, f64), (f64, f64)),
    seams: [Seam; 2],
}

impl RoundKind {
    fn cylinder(c: &Cylinder, seams: [Seam; 2]) -> RoundKind {
        RoundKind {
            center: c.center(),
            frame: c.frame(),
            factor: c.axial_sign(),
            radius: c.radius(),
            slope: 0.0,
            domain: ParamSurface::domain(c),
            seams,
        }
    }

    fn cone(c: &Cone, seams: [Seam; 2]) -> RoundKind {
        let (radius, slope) = c.radius_and_slope();
        RoundKind {
            center: c.center(),
            frame: c.frame(),
            factor: c.axial_sign(),
            radius,
            slope,
            domain: ParamSurface::domain(c),
            seams,
        }
    }
}

/// How two circles in one plane stand.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Meet {
    /// Apart, one beside the other or inside it.
    Apart,
    /// Crossing, at two points.
    Cross,
    /// Touching, at one.
    Touch,
}

/// A circle across the axis of a cylinder or cone, its normal parallel to that axis (decision
/// 32.86): nothing where its plane is past the domain, and otherwise the points where it meets
/// the circle the surface cuts in its plane. `None`, the engine's, where either domain is short of
/// a whole turn, the plane holds a cone's apex, neither the enclosures nor exact signs tell how the
/// two circles stand, the plane's enclosure reaches an end of the domain and the circles are not
/// apart, or their two crossings are not apart.
///
/// Theorem. A plane square to the axis of a right circular cylinder or cone cuts it in the circle
/// about the axis of the radius at that height, |r₀ + k·v|, or at a cone's apex in that point. A
/// circle in that plane meets the surface where it meets that circle, and two circles in one plane
/// whose centres are D apart, of radii r and ρ, cross at two points where (r − ρ)² < D² < (r + ρ)²,
/// touch at one where D² is either end and D is not zero, and are apart where D² is past both. The
/// normal parallel to the axis is an exact sign ([`parallel`]); D² and the radii are enclosed, and
/// where the axis runs along a coordinate axis D² − (r ± ρ)², times s², is the sign of a sum of
/// products of stored doubles ([`exactly`]).
fn circle_across(
    curve: &CurveKind<3>,
    circle: &Circle<3>,
    round: &RoundKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    let ((du, dv), (lo, hi)) = (round.domain, curve.domain());
    if du.1 - du.0 < 2.0 || hi - lo < 2.0 {
        return Ok(None);
    }
    if !parallel(round.frame.normal(), circle.frame().normal()) {
        return Ok(None);
    }
    let q = circle.center();
    // A plane whose enclosure reaches an end may be on it, inside or past it. Every point of the
    // surface in the plane is on the cut circle wherever the end lies, so two circles apart for
    // every v of the enclosure held to the domain meet nothing; points there are the engine's.
    let (v, inside) = match height_across(q, (round.center, round.frame, round.factor), dv)? {
        Across::Past => return proven(Vec::new(), meter).map(Some),
        Across::Within(v) => (v, true),
        Across::Reaching(v) | Across::Unplaced(v) => (v, false),
    };
    // The cut's radius, signed: past a cone's apex it is negative, and the point at u is half a
    // turn round.
    let rho = Interval::point(round.radius)?.add(Interval::point(round.slope)?.mul(v)?)?;
    let (big, sense) = match rho.certain_sign() {
        Some(Sign::Positive) => (rho, 1.0),
        Some(Sign::Negative) => (rho.neg(), -1.0),
        _ => return Ok(None),
    };
    // The circle's centre in the frame's x̂ and ŷ, from the axis.
    let (_, [x, y, _]) = round.frame.unit_vectors()?;
    let w = minus(lift(q)?, lift(round.center)?)?;
    let off = [dot(w, x)?, dot(w, y)?];
    let r = Interval::point(circle.radius())?;
    let [ox, oy] = off;
    let far = ox.square()?.add(oy.square()?)?;
    // D² against (r + ρ)² and (r − ρ)², each sign where its enclosure holds it from zero.
    let beyond = far.sub(r.add(big)?.square()?)?.certain_sign();
    let within = far.sub(r.sub(big)?.square()?)?.certain_sign();
    let meet = match (beyond, within) {
        (Some(Sign::Positive), _) | (_, Some(Sign::Negative)) => Meet::Apart,
        (Some(Sign::Negative), Some(Sign::Positive)) => Meet::Cross,
        _ => {
            let Some(meet) = exactly(circle, round) else {
                return Ok(None);
            };
            meet
        }
    };
    if meet == Meet::Apart {
        return proven(Vec::new(), meter).map(Some);
    }
    if !inside {
        return Ok(None);
    }
    let tangent = meet == Meet::Touch;
    // Measured from the axis, across the line to the circle's centre.
    let cut = Crossing::between(off, big, r, tangent)?;
    let heights = [Some(cut.height), (!tangent).then_some(cut.height.neg())];
    let (_, [cx, cy, _]) = circle.frame().unit_vectors()?;
    let sense = Interval::point(sense)?;
    let mut found: [Option<Item>; 2] = [None, None];
    for (slot, h) in found.iter_mut().zip(heights) {
        let Some(h) = h else {
            continue;
        };
        let ([ax, ay], [ex, ey]) = cut.point(h)?;
        let e = plus(scale(x, ex)?, scale(y, ey)?)?;
        let t = onto(turn_of(dot(e, cx)?, dot(e, cy)?)?, (lo, hi))?;
        let u = around(ax.mul(sense)?, ay.mul(sense)?, du)?;
        let (Some(t), Some(u)) = (t, u) else {
            return Ok(None);
        };
        let [on_u, on_v] = surface_ends(round.seams, u, v);
        *slot = Some(Item::Point {
            at: ParamBox::new(&[t, u, v])?,
            // A span across the join is read modulo the turn (decision 32.104).
            xyz: grid_box(&bound_over(curve, Seam::turns((lo, hi)), t, meter)?)?,
            contact: standing(tangent, [false, on_u, on_v]),
            class: certified(),
        });
    }
    // Two crossings whose boxes meet are not told apart: the engine's. On a whole turn they meet
    // modulo the turn (decision 32.104).
    let [seam_u, seam_v] = round.seams;
    let seams = [Seam::turns((lo, hi)), seam_u, seam_v];
    if let [Some(a), Some(b)] = &found
        && a.at().meets_on(b.at(), &seams)?
    {
        return Ok(None);
    }
    proven(found.into_iter().flatten().collect(), meter).map(Some)
}

/// How a circle and the circle `round` cuts in its plane stand, by exact signs, where the axis runs
/// along a coordinate axis: there the height is a difference of two stored doubles
/// ([`height_terms`]) and the centres' distance squared a sum of their products, so s²·(D² − (r ±
/// ρ)²) is a sum of products of doubles, each factor written as a double and its error. `None` on
/// any other axis, where a product would not be exact, and where the centres are one: a touch
/// there is the circle lying on the surface, [`ring_overlap`]'s.
fn exactly(circle: &Circle<3>, round: &RoundKind) -> Option<Meet> {
    let n = round.frame.normal();
    let (q, c) = (circle.center(), round.center);
    let (up, [(_, p), (_, from)]) = height_terms(n, q, c)?;
    let k = axis_of(n)?;
    let (s, r, r0) = (round.factor, circle.radius(), round.radius);
    let rise = if up { round.slope } else { -round.slope };
    // s times each offset across the axis, and s·(r ± ρ) = r·s ± (r₀·s + k·h), as doubles whose sum
    // is each exactly.
    let others = match k {
        0 => [1, 2],
        1 => [2, 0],
        _ => [0, 1],
    };
    let mut offsets = [[0.0; 4]; 2];
    for (out, i) in offsets.iter_mut().zip(others) {
        split(&[(s, pick(q, i)), (-s, pick(c, i))], out);
    }
    let sums = [1.0, -1.0].map(|way| {
        let mut out = [0.0; 8];
        split(
            &[(r, s), (way * r0, s), (way * rise, p), (-way * rise, from)],
            &mut out,
        );
        out
    });
    let factors = [s, r, r0, rise, p, from].into_iter().chain(q).chain(c);
    let parts = offsets.iter().flatten().chain(sums.iter().flatten());
    if !factors.chain(parts.copied()).all(tame) {
        return None;
    }
    let [outer, inner] = sums.map(|sum| squares_less(&offsets, &sum));
    match (outer, inner) {
        (Sign::Zero, _) | (_, Sign::Zero) => {
            (squares_less(&offsets, &[0.0; 8]) == Sign::Positive).then_some(Meet::Touch)
        }
        (a, b) if a == b => Some(Meet::Apart),
        _ => Some(Meet::Cross),
    }
}

/// Each product of `products` as a double and its error, in `out` in turn: their sum is the sum of
/// the products, exactly.
fn split(products: &[(f64, f64)], out: &mut [f64]) {
    let parts = products.iter().flat_map(|&(a, b)| {
        let (p, e) = two_prod(a, b);
        [p, e]
    });
    for (slot, x) in out.iter_mut().zip(parts) {
        *slot = x;
    }
}

/// The sign of Σ (Σ y)² over the two `offsets`, less (Σ x)² over `sum`, exactly: every term a
/// product of two of the doubles, held on the stack.
fn squares_less(offsets: &[[f64; 4]; 2], sum: &[f64; 8]) -> Sign {
    let mut terms = [(0.0, 0.0); 96];
    let pairs = offsets
        .iter()
        .flat_map(|y| pairs_of(y, 1.0))
        .chain(pairs_of(sum, -1.0));
    for (slot, pair) in terms.iter_mut().zip(pairs) {
        *slot = pair;
    }
    sign_of(&terms)
}

/// Every product of two of `v`'s doubles, the first taken with the sign of `way`.
fn pairs_of(v: &[f64], way: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
    v.iter()
        .flat_map(move |&a| v.iter().map(move |&b| (way * a, b)))
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

/// Whether the whole circle about `centre` of radius `r` across the stored direction `normal`
/// lies on `surface`, by exact signs of sums of products of the stored doubles, never an
/// enclosure: in a plane as [`circle_in_plane`] decides it, on a sphere by
/// [`circle_about_sphere`]'s theorem, and on a torus about the circle's own axis. Any other kind,
/// a torus about another axis or one off the coordinate axes, and a sphere or torus whose products
/// would not be exact, are `false` (decision 32.94).
///
/// Theorem (torus). A torus of radii R and r holds the point at distance ρ from its axis and
/// height h along it over its centre if and only if (ρ − R)² + h² = r². Every point of a circle of
/// radius ρ about that axis, in a plane across it at height h, stands at that distance and height,
/// so the circle lies on the torus if and only if its normal is parallel to the axis, its centre is
/// on it, and (ρ − R)² + h² − r² = 0. With the axis along a coordinate axis h is a difference of
/// two stored doubles ([`height_terms`]), and the equation is the sign of a sum of their products.
pub(crate) fn circle_lies_on(
    centre: [f64; 3],
    r: f64,
    normal: [f64; 3],
    surface: &SurfaceKind,
) -> bool {
    let zero = [0.0; 3];
    match surface {
        SurfaceKind::Plane(p) => {
            let (c, a, b) = p.vectors();
            [
                side_of(a, zero, normal),
                side_of(b, zero, normal),
                side_of(centre, c, normal),
            ]
            .iter()
            .all(|s| *s == Sign::Zero)
        }
        SurfaceKind::Sphere(s) => {
            let (q, big) = (s.center(), s.radius());
            // r² − R² + |centre − q|², a product at a time.
            let mut terms = alloc::vec![(r, r), (-big, big)];
            for (x, y) in centre.into_iter().zip(q) {
                terms.extend([(x, x), (y, y), (-x, y), (-y, x)]);
            }
            let exact = [r, big].iter().chain(&centre).chain(&q).all(|x| tame(*x));
            exact && !off_line(q, centre, normal) && sign_of(&terms) == Sign::Zero
        }
        SurfaceKind::Torus(t) => {
            let (q, axis) = (t.center(), t.frame().normal());
            let (big, small) = t.radii();
            // The height is p − c, the centres' coordinates along the axis.
            let Some((_, [(_, p), (_, c)])) = height_terms(axis, centre, q) else {
                return false;
            };
            // (r − R)² + (p − c)² − small², a product at a time.
            let terms = [
                (r, r),
                (-r, big),
                (-r, big),
                (big, big),
                (p, p),
                (-p, c),
                (-c, p),
                (c, c),
                (-small, small),
            ];
            let exact = [r, big, small, p, c].iter().all(|x| tame(*x));
            exact && same_line(q, axis, centre, normal) && sign_of(&terms) == Sign::Zero
        }
        _ => false,
    }
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
        (CurveKind::Circle(k), SurfaceKind::Cylinder(c)) => {
            match circle_on_cylinder(curve, k, c, meter)? {
                None => circle_across(curve, k, &RoundKind::cylinder(c, seams), meter),
                found => Ok(found),
            }
        }
        (CurveKind::Circle(k), SurfaceKind::Cone(c)) => match circle_on_cone(curve, k, c, meter)? {
            None => circle_across(curve, k, &RoundKind::cone(c, seams), meter),
            found => Ok(found),
        },
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
    // A plane may run on without end (the only kind that may), and the engine needs a finite
    // domain. Every point of the curve lies in its box, so the plane held, on each end it lacks, to
    // that box read in its own parameters and a unit past it meets the curve where the plane does,
    // and nowhere on an end it gains (issue 1071: an end circle of a cone or cylinder across an
    // endless plane was refused as invalid input).
    let held;
    let mut surface = surface;
    if let SurfaceKind::Plane(p) = surface {
        let ((u0, u1), (v0, v1)) = ParamSurface::domain(p);
        if ![u0, u1, v0, v1].into_iter().all(f64::is_finite) {
            let (u, v) = in_parameters(curve, p, finite(curve)?, meter)?;
            let mut ends = [(u0, u1), (v0, v1)];
            for ((lo, hi), x) in ends.iter_mut().zip([u, v]) {
                if !lo.is_finite() {
                    *lo = (smaller(x.lo(), *hi) - 1.0).next_down();
                }
                if !hi.is_finite() {
                    *hi = (larger(x.hi(), *lo) + 1.0).next_up();
                }
            }
            let (c, a, b) = p.vectors();
            held = SurfaceKind::Plane(Plane::new(c, a, b, (ends[0], ends[1]))?);
            surface = &held;
        }
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
