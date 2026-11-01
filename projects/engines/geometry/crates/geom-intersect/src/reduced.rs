//! Closed forms by reduction (chapter 12.7): tori in special positions, surfaces of revolution and
//! extrusions.
//!
//! What it does: [`crate::quadrics::natural`] calls these for the rows of 12.7 past the natural
//! quadrics. Two surfaces about one axis meet in circles, one at each crossing of their meridians in
//! the half-plane of the axis, so [`rings`] is curve with curve in two dimensions (12.5). A plane
//! square to the axis is a line of that half-plane. A plane through a torus's axis cuts two tube
//! circles, and a plane through its centre at the bitangent angle two Villarceau circles. A plane
//! through a revolution's axis cuts two copies of the profile, the second mirrored. An extrusion cut
//! by a plane along its direction is lines at the profile's crossings with the plane's trace; by any
//! other plane it is an affine copy of the profile, of the kind the map keeps; and two sweeps of one
//! direction meet in lines at the crossings of their cross-sections. What is left is `None`.
//!
//! Guarantees:
//!
//! - **Exact where the data are.** A plane's being square to an axis is a sign on the stored
//!   doubles; through an axis, and Villarceau's angle, signs from `geom_num::quadric` on whole-number
//!   data. Off the grid an enclosure decides, and a pair it cannot tell is `None`.
//! - **Curve with curve is 12.5.** Each ring comes from a crossing or a tangency that
//!   [`crate::curve::curves`] proved, and is tangent where that was.
//! - **The kind is kept.** Lines stay lines, a circle maps to a circle where the map is a turn and to
//!   an ellipse otherwise, conics stay conics and NURBS stay NURBS.
//! - **Whole turns.** A torus or revolution of a part of a turn is `None`, for the general paths.
//!
//! Method: the rows of chapter 12.7
//! (`projects/engines/geometry/architecture/12-intersections.md`). Villarceau's circles: a plane
//! through the centre of a ring torus at the angle θ to its equator with sin θ = r/R holds two
//! circles of radius R, centred ±r from the centre along the line where the plane meets the equator
//! plane's square. Symmetry reduces the rest to chapter 12.5.
//!
//! Cost: a curve-curve pair for each pair of meridian curves or cross-sections, and a curve-surface
//! pair for each profile cut. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: n a plane's normal, e an axis, d a direction, r a radius"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_curve::{Axes, Basis, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola};
use geom_curve::{ParamCurve, Trimmed, tight};
use geom_num::grid::Steps;
use geom_num::interval::Interval;
use geom_num::{Sign, quadric};
use geom_nurbs::Curve;
use geom_run::WorkMeter;
use geom_surface::{Extrusion, ParamSurface, Plane, Revolution, SurfaceKind, Torus};

use crate::curve::{curves, finite, sign_of, whole};
use crate::curve_surface::{cross, curve_surface, dot, lift, minus};
use crate::error::IntersectError;
use crate::quadrics::{
    Axis, Piece, Vector, axis, beside, circle, decided, funnel, middle, plus, point, same_line,
    scale, sheet, stretch, unit, wide,
};
use crate::result::{Item, PointContact};

/// How far a plane's line runs in the half-plane, in steps: past any surface of the frame.
const OUT: f64 = 4_398_046_511_104.0;

/// A surface's meridian curves, in (ρ, z) about an axis.
type Meridians = Vec<CurveKind<2>>;

/// A place in the half-plane, and whether two surfaces touch there.
type Touch = (Interval, Interval, bool);

/// How far along `on` the point `at` stands, exactly where the data are: an exact height is what
/// lets curve with curve see two surfaces touch.
fn level(at: Vector, on: &Axis) -> Result<Interval, IntersectError> {
    let ([a, b, c], [p, q, r], [x, y, z]) = (at, on.at, on.along);
    let first = tight::mul(tight::sub(a, p)?, x)?;
    let second = tight::mul(tight::sub(b, q)?, y)?;
    let third = tight::mul(tight::sub(c, r)?, z)?;
    Ok(tight::add(tight::add(first, second)?, third)?)
}

/// The ring about `on` at height `z`, its centre exact where the data are.
fn ring(on: &Axis, z: Interval, r: Interval, tangent: bool) -> Result<Piece, IntersectError> {
    let ([a, b, c], [x, y, w]) = (on.at, on.along);
    let centre = [
        tight::add(a, tight::mul(x, z)?)?,
        tight::add(b, tight::mul(y, z)?)?,
        tight::add(c, tight::mul(w, z)?)?,
    ];
    circle(centre, r, on.along, tangent)
}

/// Where a line meets a circle in the half-plane, as (ρ, z) and whether it touches: the quadratic in
/// the line's parameter, exact where the data are. `None` for a discriminant no enclosure tells
/// from zero.
fn line_and_circle(
    line: &Line<2>,
    round: &Circle<2>,
) -> Result<Option<Vec<Touch>>, IntersectError> {
    let (o, d, m) = (line.origin(), line.direction(), round.center());
    let (span, r) = (ParamCurve::domain(line), tight::point(round.radius())?);
    let [ox, oy] = o;
    let [dx, dy] = d;
    let [mx, my] = m;
    let (px, py) = (tight::point(ox)?, tight::point(oy)?);
    let (vx, vy) = (tight::point(dx)?, tight::point(dy)?);
    let wx = tight::sub(px, tight::point(mx)?)?;
    let wy = tight::sub(py, tight::point(my)?)?;
    let a = tight::add(tight::square(vx)?, tight::square(vy)?)?;
    let b = tight::add(tight::mul(vx, wx)?, tight::mul(vy, wy)?)?;
    let e = tight::sub(
        tight::add(tight::square(wx)?, tight::square(wy)?)?,
        tight::square(r)?,
    )?;
    let disc = tight::sub(tight::square(b)?, tight::mul(a, e)?)?;
    if disc.hi() < 0.0 {
        return Ok(Some(Vec::new()));
    }
    let touching = disc.lo() == 0.0 && disc.hi() == 0.0;
    if disc.contains(0.0) && !touching {
        return Ok(None);
    }
    let root = tight::sqrt(disc)?;
    let mut out = Vec::new();
    let signs: &[f64] = if touching { &[1.0] } else { &[1.0, -1.0] };
    for sign in signs {
        let t = tight::div(tight::sub(tight::mul(root, tight::point(*sign)?)?, b)?, a)?;
        if t.hi() < span.0 || t.lo() > span.1 {
            continue;
        }
        let rho = tight::add(px, tight::mul(vx, t)?)?;
        let z = tight::add(py, tight::mul(vy, t)?)?;
        out.push((rho, z, touching));
    }
    Ok(Some(out))
}

/// A straight meridian curve.
fn straight(
    at: [f64; 2],
    along: [f64; 2],
    span: (f64, f64),
) -> Result<CurveKind<2>, IntersectError> {
    Ok(CurveKind::Line(Line::new(at, along, span)?))
}

/// The sign of a · b, exactly.
fn sign_along(a: [f64; 3], b: [f64; 3]) -> Sign {
    let [x, y, z] = a;
    let [p, q, r] = b;
    sign_of(&[(x, p), (y, q), (z, r)])
}

/// Whether the plane is square to `e`, exactly.
fn square_to(plane: &Plane, e: [f64; 3]) -> bool {
    let (_, u, v) = plane.vectors();
    sign_along(u, e) == Sign::Zero && sign_along(v, e) == Sign::Zero
}

/// Whether the surface turns all the way round its own axis.
fn whole_turn(s: &SurfaceKind) -> bool {
    let ((u0, u1), _) = ParamSurface::domain(s);
    u1 - u0 >= 2.0
}

/// A torus's whole tube.
fn torus_meridian(t: &Torus, on: &Axis) -> Result<Meridians, IntersectError> {
    let (major, minor) = t.radii();
    let z = tight::middle(level(lift(t.center())?, on)?);
    let frame = Frame::<2>::standard()?;
    let tube = Circle::new([major, z], minor, frame, (0.0, 2.0))?;
    Ok(alloc::vec![CurveKind::Circle(tube)])
}

/// `kind` as a curve on its own, cut to `span`.
fn on_span(kind: CurveKind<3>, span: (f64, f64)) -> Result<Option<CurveKind<3>>, IntersectError> {
    if finite(&kind)? == span {
        return Ok(Some(kind));
    }
    stretch(&kind, span.0, span.1)
}

/// The curve inside a trimmed one.
fn untrimmed(profile: &Trimmed<3>) -> Result<Option<CurveKind<3>>, IntersectError> {
    on_span(CurveKind::from(profile.basis().clone()), profile.interval())
}

/// The map from space to the half-plane of `on` that holds `at`, or `None` if `at` is on the axis.
fn to_half_plane(on: &Axis, at: Vector) -> Result<Option<Affine<2>>, IntersectError> {
    let w = minus(at, on.at)?;
    let lean = minus(w, scale(on.along, dot(w, on.along)?)?)?;
    if lean.iter().all(|c| c.contains(0.0)) {
        return Ok(None);
    }
    let out = unit(lean)?;
    Ok(Some(Affine::onto(on.at, [out, on.along])?))
}

/// A surface of revolution's axis.
fn revolution_axis(r: &Revolution) -> Result<Axis, IntersectError> {
    let (_, a, frame) = r.shape()?;
    axis(a, &frame, 1.0)
}

/// The profile of `r` in the half-plane of its axis, if it lies in one.
fn profile_meridian(r: &Revolution) -> Result<Option<Meridians>, IntersectError> {
    let (profile, ..) = r.shape()?;
    let on = revolution_axis(r)?;
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    let (lo, hi) = finite(&kind)?;
    let middle_of = lift(kind.point(lo * 0.5 + hi * 0.5, geom_nurbs::Limit::FromRight)?)?;
    let Some(map) = to_half_plane(&on, middle_of)? else {
        return Ok(None);
    };
    // Every point of the profile must stay on the half-plane: its ends and middle are read, and
    // the kind's own data by the map's image, whose reach counts what leaves the plane.
    for t in [lo, hi] {
        let at = lift(kind.point(t, geom_nurbs::Limit::FromRight)?)?;
        let [_, _, off] = map.leaves(at)?;
        if !off.contains(0.0) {
            return Ok(None);
        }
    }
    Ok(map.image(&kind)?.map(|(curve, _)| alloc::vec![curve]))
}

/// `s`'s meridian curves about `on`, if it stands square to the axis or about it.
fn meridians(
    s: &SurfaceKind,
    on: &Axis,
    e: [f64; 3],
    c: [f64; 3],
) -> Result<Option<Meridians>, IntersectError> {
    let about = |center: [f64; 3], along: [f64; 3]| same_line(center, along, c, e);
    Ok(Some(match s {
        SurfaceKind::Plane(p) if square_to(p, e) => {
            let (at, ..) = p.vectors();
            let z = tight::middle(level(lift(at)?, on)?);
            alloc::vec![straight([0.0, z], [1.0, 0.0], (0.0, OUT))?]
        }
        SurfaceKind::Sphere(x) if about(x.center(), e) => {
            let z = tight::middle(level(lift(x.center())?, on)?);
            let frame = Frame::<2>::standard()?;
            alloc::vec![CurveKind::Circle(Circle::new(
                [0.0, z],
                x.radius(),
                frame,
                (-0.5, 0.5)
            )?)]
        }
        SurfaceKind::Cylinder(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            let z = tight::middle(level(lift(x.center())?, on)?);
            let (_, (v0, v1)) = ParamSurface::domain(x);
            let sense = tight::middle(dot(lift(x.frame().normal())?, on.along)?);
            let (lo, hi) = if sense > 0.0 { (v0, v1) } else { (-v1, -v0) };
            alloc::vec![straight([x.radius(), z], [0.0, 1.0], (lo, hi))?]
        }
        SurfaceKind::Cone(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            let f = funnel(x)?;
            let z = tight::middle(level(f.apex, on)?);
            let k = tight::middle(f.slope);
            alloc::vec![
                straight([0.0, z], [k, 1.0], (0.0, OUT))?,
                straight([0.0, z], [k, -1.0], (0.0, OUT))?,
            ]
        }
        SurfaceKind::Torus(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            torus_meridian(x, on)?
        }
        SurfaceKind::Revolution(x) if whole_turn(s) => {
            let (_, a, frame) = x.shape()?;
            if !about(a, frame.normal()) {
                return Ok(None);
            }
            let Some(curves) = profile_meridian(x)? else {
                return Ok(None);
            };
            curves
        }
        _ => return Ok(None),
    }))
}

/// Whether two rings or points stand at one place of the axis.
fn same_place(a: (Interval, Interval), b: (Interval, Interval)) -> bool {
    let meet = |x: Interval, y: Interval| x.lo() <= y.hi() && y.lo() <= x.hi();
    meet(a.0, b.0) && meet(a.1, b.1)
}

/// The rings at the crossings of two sets of meridian curves. `None` where curve with curve could
/// not decide.
fn rings(
    on: &Axis,
    first: &[CurveKind<2>],
    second: &[CurveKind<2>],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let mut out = Vec::new();
    let mut seen: Vec<(Interval, Interval)> = Vec::new();
    let mut place = |r: Interval, z: Interval, tangent: bool| -> Result<(), IntersectError> {
        if r.hi() < 0.0 || seen.iter().any(|at| same_place(*at, (r, z))) {
            return Ok(());
        }
        seen.push((r, z));
        out.push(if r.contains(0.0) {
            Piece::Point {
                at: plus(on.at, scale(on.along, z)?)?,
                tangent: true,
            }
        } else {
            ring(on, z, r, tangent)?
        });
        Ok(())
    };
    for x in first {
        for y in second {
            let closed = match (x, y) {
                (CurveKind::Line(l), CurveKind::Circle(c))
                | (CurveKind::Circle(c), CurveKind::Line(l)) => Some(line_and_circle(l, c)?),
                _ => None,
            };
            if let Some(found) = closed {
                let Some(found) = found else {
                    return Ok(None);
                };
                for (r, z, tangent) in found {
                    place(r, z, tangent)?;
                }
                continue;
            }
            let found = curves(x, y, meter)?;
            for item in found.items() {
                match item {
                    Item::Point { at, contact, .. } => {
                        let [t, ..] = at.spans() else {
                            return Err(IntersectError::Internal);
                        };
                        let b = x.bound((t.lo(), t.hi()), meter)?;
                        let ([r0, z0], [r1, z1]) = (b.lo, b.hi);
                        let (r, z) = (Interval::new(r0, r1)?, Interval::new(z0, z1)?);
                        place(r, z, !matches!(contact, PointContact::Crossing))?;
                    }
                    Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
                    Item::Flagged { .. } => return Ok(None),
                    Item::Curve { .. } => {}
                }
            }
        }
    }
    Ok(Some(out))
}

/// A surface with a surface about one axis, or a plane square to it, the second giving the axis.
fn about_one_axis(
    a: &SurfaceKind,
    b: &SurfaceKind,
    on: &Axis,
    e: [f64; 3],
    c: [f64; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (Some(first), Some(second)) = (meridians(a, on, e, c)?, meridians(b, on, e, c)?) else {
        return Ok(None);
    };
    rings(on, &first, &second, meter)
}

/// Whether the torus's angle to the plane is Villarceau's, and its circles if it is.
fn villarceau(
    p: &crate::quadrics::Sheet,
    on: &Axis,
    t: &Torus,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (major, minor) = t.radii();
    let exact = (|| {
        let plane = p.exact?;
        let (g, h) = on.exact?;
        Some(quadric::villarceau(
            &plane,
            g,
            h,
            Steps(u64::try_from(whole(major)?).ok()?),
            Steps(u64::try_from(whole(minor)?).ok()?),
        ))
    })();
    let Some(Ok(Sign::Zero)) = exact else {
        return Ok(None);
    };
    if major <= minor {
        return Ok(None);
    }
    let along = unit(cross(on.along, p.normal)?)?;
    let shift = scale(along, point(minor)?)?;
    let radius = point(major)?;
    Ok(Some(alloc::vec![
        circle(plus(on.at, shift)?, radius, p.normal, false)?,
        circle(minus(on.at, shift)?, radius, p.normal, false)?,
    ]))
}

/// A plane with a torus: square to its axis, through it, or at Villarceau's angle through its centre.
pub(crate) fn plane_torus(
    a: &Plane,
    b: &Torus,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Torus(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = axis(b.center(), &b.frame(), 1.0)?;
    let e = b.frame().normal();
    if square_to(a, e) {
        return about_one_axis(
            &SurfaceKind::Plane(a.clone()),
            &surface,
            &on,
            e,
            b.center(),
            meter,
        );
    }
    let p = sheet(a)?;
    let tilt = dot(p.normal, on.along)?;
    let side = dot(p.normal, minus(on.at, p.at)?)?;
    let (exact_tilt, exact_side) = match (p.exact, on.exact) {
        (Some(s), Some((g, h))) => (
            Some(quadric::plane_along(&s, g, h)),
            Some(quadric::plane_along(&s, s.a, g)),
        ),
        _ => (None, None),
    };
    let (Some(lean), Some(through)) = (decided(exact_tilt, tilt), decided(exact_side, side)) else {
        return Ok(None);
    };
    if through != Sign::Zero {
        return Ok(None);
    }
    if lean == Sign::Zero {
        let out = unit(cross(p.normal, on.along)?)?;
        let (major, minor) = t_radii(b)?;
        let shift = scale(out, major)?;
        return Ok(Some(alloc::vec![
            circle(plus(on.at, shift)?, minor, p.normal, false)?,
            circle(minus(on.at, shift)?, minor, p.normal, false)?,
        ]));
    }
    villarceau(&p, &on, b)
}

/// A torus's radii, enclosed.
fn t_radii(t: &Torus) -> Result<(Interval, Interval), IntersectError> {
    let (major, minor) = t.radii();
    Ok((point(major)?, point(minor)?))
}

/// A sphere, cylinder, cone or torus with a torus about one axis.
pub(crate) fn about_torus(
    a: &SurfaceKind,
    b: &Torus,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Torus(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = axis(b.center(), &b.frame(), 1.0)?;
    about_one_axis(a, &surface, &on, b.frame().normal(), b.center(), meter)
}

/// A plane with a surface of revolution: square to its axis, or through it.
pub(crate) fn plane_revolution(
    a: &Plane,
    b: &Revolution,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Revolution(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = revolution_axis(b)?;
    let (profile, at, frame) = b.shape()?;
    let e = frame.normal();
    if square_to(a, e) {
        return about_one_axis(&SurfaceKind::Plane(a.clone()), &surface, &on, e, at, meter);
    }
    let p = sheet(a)?;
    let tilt = dot(p.normal, on.along)?;
    let side = dot(p.normal, minus(on.at, p.at)?)?;
    let (exact_tilt, exact_side) = match (p.exact, on.exact) {
        (Some(s), Some((g, h))) => (
            Some(quadric::plane_along(&s, g, h)),
            Some(quadric::plane_along(&s, s.a, g)),
        ),
        _ => (None, None),
    };
    let (Some(Sign::Zero), Some(Sign::Zero)) =
        (decided(exact_tilt, tilt), decided(exact_side, side))
    else {
        return Ok(None);
    };
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    let (lo, hi) = finite(&kind)?;
    let middle_of = lift(kind.point(lo * 0.5 + hi * 0.5, geom_nurbs::Limit::FromRight)?)?;
    let Some(flat) = to_half_plane(&on, middle_of)? else {
        return Ok(None);
    };
    // Turn the half-plane of the profile onto each half of the plane.
    let out = unit(cross(p.normal, on.along)?)?;
    let mut pieces = Vec::new();
    for sense in [1.0, -1.0] {
        let toward = scale(out, point(sense)?)?;
        let map = flat.then_into(on.at, toward, on.along)?;
        let Some((curve, reach)) = map.image(&kind)? else {
            return Ok(None);
        };
        pieces.push(Piece::Curve {
            curve: Box::new(curve),
            tangent: false,
            reach,
        });
    }
    Ok(Some(pieces))
}

/// A quadric or a torus with a surface of revolution about one axis.
pub(crate) fn about_revolution(
    a: &SurfaceKind,
    b: &Revolution,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Revolution(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = revolution_axis(b)?;
    let (_, at, frame) = b.shape()?;
    about_one_axis(a, &surface, &on, frame.normal(), at, meter)
}

/// The plane through `at` of `p`'s directions, far past any surface.
fn endless(p: &Plane, at: [f64; 3]) -> Result<SurfaceKind, IntersectError> {
    let (_, u, v) = p.vectors();
    let span = (-OUT, OUT);
    Ok(SurfaceKind::Plane(Plane::new(at, u, v, (span, span))?))
}

/// `from + d·s` for s over `span`, as a line.
fn along(
    from: Vector,
    d: [f64; 3],
    span: (f64, f64),
    tangent: bool,
) -> Result<Piece, IntersectError> {
    Ok(Piece::Curve {
        curve: Box::new(CurveKind::Line(Line::new(middle(from), d, span)?)),
        tangent,
        reach: wide(from),
    })
}

/// A plane along an extrusion's direction: a line at each crossing of the profile.
fn plane_along(
    a: &Plane,
    kind: &CurveKind<3>,
    d: [f64; 3],
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (at, ..) = a.vectors();
    let found = curve_surface(kind, &endless(a, at)?, meter)?;
    let mut out = Vec::new();
    for item in found.items() {
        match item {
            Item::Point {
                at: place, contact, ..
            } => {
                let [t, ..] = place.spans() else {
                    return Err(IntersectError::Internal);
                };
                let b = kind.bound((t.lo(), t.hi()), meter)?;
                let [x0, y0, z0] = b.lo;
                let [x1, y1, z1] = b.hi;
                let from = [
                    Interval::new(x0, x1)?,
                    Interval::new(y0, y1)?,
                    Interval::new(z0, z1)?,
                ];
                let tangent = !matches!(contact, PointContact::Crossing);
                out.push(along(from, d, span, tangent)?);
            }
            Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
            Item::Flagged { .. } => return Ok(None),
            Item::Curve { .. } => {}
        }
    }
    Ok(Some(out))
}

/// How far along `d` from the plane through `c` of normal `n` the point `x` stands.
fn height_on(x: Vector, c: Vector, n: Vector, d: Vector) -> Result<Interval, IntersectError> {
    Ok(dot(minus(c, x)?, n)?.div(dot(d, n)?)?)
}

/// A plane across an extrusion: the profile moved along `d` into the plane, on the stretches of it
/// the extrusion reaches.
fn plane_across(
    a: &Plane,
    kind: &CurveKind<3>,
    d: [f64; 3],
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let p = sheet(a)?;
    let direction = lift(d)?;
    let Some((image, reach)) = Affine::along(p.at, p.normal, direction)?.image(kind)? else {
        return Ok(None);
    };
    let (at, ..) = a.vectors();
    let [x, y, z] = at;
    let [e, f, g] = d;
    // Where the profile passes the planes the extrusion's ends reach, whose images are its ends.
    let (lo, hi) = finite(kind)?;
    let mut cuts = Vec::new();
    for v in [span.0, span.1] {
        let moved = endless(a, [x - v * e, y - v * f, z - v * g])?;
        for item in curve_surface(kind, &moved, meter)?.items() {
            let [t, ..] = item.at().spans() else {
                return Err(IntersectError::Internal);
            };
            let t = tight::middle(*t);
            if t > lo && t < hi {
                cuts.push(t);
            }
        }
    }
    cuts.sort_unstable_by(f64::total_cmp);
    let mut out = Vec::new();
    let mut start = lo;
    cuts.push(hi);
    for end in cuts {
        let middle_of = start * 0.5 + end * 0.5;
        let there = lift(kind.point(middle_of, geom_nurbs::Limit::FromRight)?)?;
        let v = height_on(there, p.at, p.normal, direction)?;
        if end > start && v.hi() >= span.0 && v.lo() <= span.1 {
            let Some(part) = stretch(&image, start, end)? else {
                return Ok(None);
            };
            out.push(Piece::Curve {
                curve: Box::new(part),
                tangent: false,
                reach,
            });
        }
        start = end;
    }
    Ok(Some(out))
}

/// A plane with an extrusion.
pub(crate) fn plane_extrusion(
    a: &Plane,
    b: &Extrusion,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (profile, d) = b.shape()?;
    let (_, span) = ParamSurface::domain(b);
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    let p = sheet(a)?;
    let tilt = dot(p.normal, lift(d)?)?;
    let (_, u, v) = a.vectors();
    let flat = sign_of_triple(u, v, d);
    let Some(lean) = decided(flat, tilt) else {
        return Ok(None);
    };
    if lean == Sign::Zero {
        plane_along(a, &kind, d, span, meter)
    } else {
        plane_across(a, &kind, d, span, meter)
    }
}

/// The sign of (u × v) · d where all are whole: the triple product of three grid directions.
fn sign_of_triple(
    u: [f64; 3],
    v: [f64; 3],
    d: [f64; 3],
) -> Option<Result<Sign, geom_num::NumError>> {
    let o = crate::curve::grid_point(&[0.0; 3])?;
    let plane = quadric::Plane {
        a: o,
        b: crate::curve::grid_point(&u)?,
        c: crate::curve::grid_point(&v)?,
    };
    Some(quadric::plane_along(
        &plane,
        o,
        crate::curve::grid_point(&d)?,
    ))
}

/// A sweep along one direction: its cross-section, and how far it runs along the unit direction.
struct Sweep {
    section: CurveKind<2>,
    reach: (f64, f64),
}

/// The cross-section of `s` on `map`, if it sweeps along `direction`.
fn sweep(
    s: &SurfaceKind,
    direction: [f64; 3],
    map: &Affine<2>,
    unit_d: Vector,
) -> Result<Option<Sweep>, IntersectError> {
    match s {
        SurfaceKind::Extrusion(x) => {
            let (profile, d) = x.shape()?;
            if !crate::quadrics::parallel(d, direction) {
                return Ok(None);
            }
            let Some(kind) = untrimmed(profile)? else {
                return Ok(None);
            };
            let Some((section, _)) = map.image(&kind)? else {
                return Ok(None);
            };
            let (_, (v0, v1)) = ParamSurface::domain(x);
            let scale_d = tight::middle(dot(lift(d)?, unit_d)?);
            let (lo, hi) = finite(&kind)?;
            let from = tight::middle(dot(
                lift(kind.point(lo * 0.5 + hi * 0.5, geom_nurbs::Limit::FromRight)?)?,
                unit_d,
            )?);
            let (a, b) = (from + v0 * scale_d, from + v1 * scale_d);
            Ok(Some(Sweep {
                section,
                reach: (tight::smaller(a, b), tight::larger(a, b)),
            }))
        }
        SurfaceKind::Cylinder(x) => {
            let e = x.frame().normal();
            if !crate::quadrics::parallel(e, direction) || !whole_turn(s) {
                return Ok(None);
            }
            let c = lift(x.center())?;
            let [cx, cy] = map.at_point(c)?;
            let frame = Frame::<2>::standard()?;
            let circle = Circle::new(
                [tight::middle(cx), tight::middle(cy)],
                x.radius(),
                frame,
                (0.0, 2.0),
            )?;
            let (_, (v0, v1)) = ParamSurface::domain(x);
            let from = tight::middle(dot(c, unit_d)?);
            let sense = tight::middle(dot(lift(e)?, unit_d)?);
            let (a, b) = (from + v0 * sense, from + v1 * sense);
            Ok(Some(Sweep {
                section: CurveKind::Circle(circle),
                reach: (tight::smaller(a, b), tight::larger(a, b)),
            }))
        }
        _ => Ok(None),
    }
}

/// Two sweeps of one direction: lines along it at the crossings of their cross-sections.
pub(crate) fn sweeps(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let direction = match (a, b) {
        (_, SurfaceKind::Extrusion(x)) | (SurfaceKind::Extrusion(x), _) => x.shape()?.1,
        _ => return Ok(None),
    };
    let d = unit(lift(direction)?)?;
    let side = unit(cross(d, lift(beside(direction))?)?)?;
    let other = cross(d, side)?;
    let origin = [point(0.0)?; 3];
    let map = Affine::onto(origin, [side, other])?;
    let (Some(first), Some(second)) =
        (sweep(a, direction, &map, d)?, sweep(b, direction, &map, d)?)
    else {
        return Ok(None);
    };
    let lo = tight::larger(first.reach.0, second.reach.0);
    let hi = tight::smaller(first.reach.1, second.reach.1);
    if lo >= hi {
        return Ok(Some(Vec::new()));
    }
    let mut out = Vec::new();
    for item in curves(&first.section, &second.section, meter)?.items() {
        match item {
            Item::Point { at, contact, .. } => {
                let [t, ..] = at.spans() else {
                    return Err(IntersectError::Internal);
                };
                let box2 = first.section.bound((t.lo(), t.hi()), meter)?;
                let ([x0, y0], [x1, y1]) = (box2.lo, box2.hi);
                let from = plus(
                    scale(side, Interval::new(x0, x1)?)?,
                    scale(other, Interval::new(y0, y1)?)?,
                )?;
                let tangent = !matches!(contact, PointContact::Crossing);
                out.push(along(from, middle(d), (lo, hi), tangent)?);
            }
            Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
            Item::Flagged { .. } => return Ok(None),
            Item::Curve { .. } => {}
        }
    }
    Ok(Some(out))
}

/// x ↦ `at` + Σ xᵢ·`columns`[i], from space to `D` dimensions, enclosed.
#[derive(Clone, Copy)]
struct Affine<const D: usize> {
    at: [Interval; D],
    columns: [[Interval; D]; 3],
}

/// The vector with the entries of `v` on the rows `k` of each column.
impl Affine<2> {
    /// The coordinates of x − `origin` along the two axes.
    fn onto(origin: Vector, [p, q]: [Vector; 2]) -> Result<Affine<2>, IntersectError> {
        let [a, b, c] = p;
        let [e, f, g] = q;
        let at = [dot(origin, p)?.neg(), dot(origin, q)?.neg()];
        Ok(Affine {
            at,
            columns: [[a, e], [b, f], [c, g]],
        })
    }

    /// The point `x` maps to.
    fn at_point(&self, x: Vector) -> Result<[Interval; 2], IntersectError> {
        self.point(x)
    }

    /// How far `x` stands from the half-plane this map reads, as the third coordinate.
    fn leaves(&self, x: Vector) -> Result<[Interval; 3], IntersectError> {
        let [r, z] = self.point(x)?;
        let [[a, e], [b, f], [c, g]] = self.columns;
        let [s, t] = self.at;
        let back = [
            a.mul(r.sub(s)?)?.add(e.mul(z.sub(t)?)?)?,
            b.mul(r.sub(s)?)?.add(f.mul(z.sub(t)?)?)?,
            c.mul(r.sub(s)?)?.add(g.mul(z.sub(t)?)?)?,
        ];
        let [u, v, w] = minus(x, back)?;
        Ok([r, z, u.add(v)?.add(w)?])
    }

    /// This map, then (ρ, z) ↦ `to` + ρ·`out` + z·`up`: a turn about the axis.
    fn then_into(&self, to: Vector, out: Vector, up: Vector) -> Result<Affine<3>, IntersectError> {
        let [r, z] = self.at;
        let [[a, e], [b, f], [c, g]] = self.columns;
        let image = |p: Interval, q: Interval| -> Result<Vector, IntersectError> {
            plus(scale(out, p)?, scale(up, q)?)
        };
        Ok(Affine {
            at: plus(to, image(r, z)?)?,
            columns: [image(a, e)?, image(b, f)?, image(c, g)?],
        })
    }
}

impl Affine<3> {
    /// The projection along `d` onto the plane through `c` of normal `n`.
    fn along(c: Vector, n: Vector, d: Vector) -> Result<Affine<3>, IntersectError> {
        let tilt = dot(d, n)?;
        let k = dot(c, n)?.div(tilt)?;
        let at = scale(d, k)?;
        let one = point(1.0)?;
        let zero = point(0.0)?;
        let mut columns = [[zero; 3]; 3];
        for (i, column) in columns.iter_mut().enumerate() {
            let ni = n
                .get(i)
                .copied()
                .ok_or(IntersectError::Internal)?
                .div(tilt)?;
            for (j, slot) in column.iter_mut().enumerate() {
                let dj = d.get(j).copied().ok_or(IntersectError::Internal)?;
                let identity = if i == j { one } else { zero };
                *slot = identity.sub(dj.mul(ni)?)?;
            }
        }
        Ok(Affine { at, columns })
    }
}

impl<const D: usize> Affine<D> {
    /// The direction `x` maps to.
    fn vector(&self, x: Vector) -> Result<[Interval; D], IntersectError> {
        let mut out = [point(0.0)?; D];
        let [a, b, c] = x;
        let [p, q, r] = &self.columns;
        for (k, slot) in out.iter_mut().enumerate() {
            let cell = |w: &[Interval; D]| w.get(k).copied().ok_or(IntersectError::Internal);
            *slot = a
                .mul(cell(p)?)?
                .add(b.mul(cell(q)?)?)?
                .add(c.mul(cell(r)?)?)?;
        }
        Ok(out)
    }

    /// The point `x` maps to.
    fn point(&self, x: Vector) -> Result<[Interval; D], IntersectError> {
        let mut out = self.vector(x)?;
        for (slot, at) in out.iter_mut().zip(&self.at) {
            *slot = slot.add(*at)?;
        }
        Ok(out)
    }

    /// The conic's axes.
    fn axes(&self, axes: &Axes<3>) -> Result<Option<(Axes<D>, f64)>, IntersectError> {
        let (u, v) = match axes {
            Axes::Vectors { u, v } => (lift(*u)?, lift(*v)?),
            Axes::Frame { frame, a, b } => {
                let (_, [x, y, _]) = frame.unit_vectors()?;
                (scale(x, point(*a)?)?, scale(y, point(*b)?)?)
            }
        };
        let (p, q) = (self.vector(u)?, self.vector(v)?);
        let reach = tight::larger(wide_of(&p), wide_of(&q));
        Ok(Some((
            Axes::Vectors {
                u: middle_of(&p),
                v: middle_of(&q),
            },
            reach,
        )))
    }

    /// The curve `curve` maps to, of the kind the map keeps, and how far its data reach. `None` for
    /// a kind no map here takes.
    fn image(&self, curve: &CurveKind<3>) -> Result<Option<(CurveKind<D>, f64)>, IntersectError> {
        Ok(Some(match curve {
            CurveKind::Line(l) => {
                let at = self.point(lift(l.origin())?)?;
                let d = self.vector(lift(l.direction())?)?;
                let line = Line::new(middle_of(&at), middle_of(&d), finite(curve)?)?;
                (
                    CurveKind::Line(line),
                    tight::larger(wide_of(&at), wide_of(&d)),
                )
            }
            CurveKind::Circle(c) => {
                let (_, [x, y, _]) = c.frame().unit_vectors()?;
                let r = point(c.radius())?;
                let axes = Axes::Vectors {
                    u: middle(scale(x, r)?),
                    v: middle(scale(y, r)?),
                };
                let center = self.point(lift(c.center())?)?;
                let Some((mapped, reach)) = self.axes(&axes)? else {
                    return Ok(None);
                };
                let reach = tight::larger(wide_of(&center), reach);
                if let Axes::Vectors { u, v } = &mapped
                    && let Some(frame) = turned(u, v, c.radius())?
                {
                    let image = Circle::new(middle_of(&center), c.radius(), frame, c.domain())?;
                    return Ok(Some((CurveKind::Circle(image), reach)));
                }
                (
                    CurveKind::Ellipse(Ellipse::new(middle_of(&center), mapped, c.domain())?),
                    reach,
                )
            }
            CurveKind::Ellipse(e) => {
                let center = self.point(lift(e.center())?)?;
                let Some((mapped, reach)) = self.axes(&e.axes())? else {
                    return Ok(None);
                };
                let image = Ellipse::new(middle_of(&center), mapped, e.domain())?;
                (
                    CurveKind::Ellipse(image),
                    tight::larger(wide_of(&center), reach),
                )
            }
            CurveKind::Hyperbola(h) => {
                let center = self.point(lift(h.center())?)?;
                let Some((mapped, reach)) = self.axes(&h.axes())? else {
                    return Ok(None);
                };
                let image = Hyperbola::new(middle_of(&center), mapped, h.domain())?;
                (
                    CurveKind::Hyperbola(image),
                    tight::larger(wide_of(&center), reach),
                )
            }
            CurveKind::Parabola(p) => {
                let vertex = self.point(lift(p.vertex())?)?;
                let Some((mapped, reach)) = self.axes(&p.axes())? else {
                    return Ok(None);
                };
                let image = Parabola::new(middle_of(&vertex), mapped, p.domain())?;
                (
                    CurveKind::Parabola(image),
                    tight::larger(wide_of(&vertex), reach),
                )
            }
            CurveKind::Nurbs(n) => {
                let weights = n.weights();
                let mut points = Vec::with_capacity(n.homogeneous().len());
                let mut reach = 0.0_f64;
                for (i, h) in n.homogeneous().iter().enumerate() {
                    let w = weights.and_then(|all| all.get(i)).copied().unwrap_or(1.0);
                    let mut image = self.vector(lift(*h)?)?;
                    for (slot, at) in image.iter_mut().zip(&self.at) {
                        *slot = slot.add(at.mul(point(w)?)?)?;
                    }
                    reach = tight::larger(reach, wide_of(&image));
                    points.push(middle_of(&image));
                }
                let kept = weights.map(<[f64]>::to_vec);
                let Ok(image) = Curve::from_homogeneous(n.basis().clone(), points, kept) else {
                    return Ok(None);
                };
                (CurveKind::Nurbs(image), reach)
            }
            CurveKind::Trimmed(t) => {
                let kind = CurveKind::from(t.basis().clone());
                let Some((whole_image, reach)) = self.image(&kind)? else {
                    return Ok(None);
                };
                let span = t.interval();
                let Ok(basis) = Basis::try_from(whole_image) else {
                    return Ok(None);
                };
                (CurveKind::Trimmed(Trimmed::new(basis, span)?), reach)
            }
            _ => return Ok(None),
        }))
    }
}

/// The frame of two image axes that are square and of length `r`, as a turn leaves them.
fn turned<const D: usize>(
    u: &[f64; D],
    v: &[f64; D],
    r: f64,
) -> Result<Option<Frame<D>>, IntersectError> {
    let (mut uu, mut vv, mut uv) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (a, b) in u.iter().zip(v) {
        uu += a * a;
        vv += b * b;
        uv += a * b;
    }
    let size = r * r;
    let close = |x: f64, y: f64| (x - y).abs() <= size * 1e-12;
    if !close(uu, size) || !close(vv, size) || !close(uv, 0.0) {
        return Ok(None);
    }
    let at = |w: &[f64; D], k: usize| w.get(k).copied().unwrap_or(0.0);
    let (a, b) = (
        [at(u, 0), at(u, 1), at(u, 2)],
        [at(v, 0), at(v, 1), at(v, 2)],
    );
    let [p, q, s] = a;
    let [x, y, z] = b;
    let normal = [q * z - s * y, s * x - p * z, p * y - q * x];
    Ok(Some(Frame::new(*u, normal)?))
}

/// The midpoints of `v`.
fn middle_of<const D: usize>(v: &[Interval; D]) -> [f64; D] {
    v.map(tight::middle)
}

/// The widest entry of `v`.
fn wide_of<const D: usize>(v: &[Interval; D]) -> f64 {
    let mut out = 0.0_f64;
    for x in v {
        out = tight::larger(out, x.hi() - x.lo());
    }
    out
}
