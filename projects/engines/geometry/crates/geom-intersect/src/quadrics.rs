//! Where two natural quadrics meet, in closed form (chapter 12.7).
//!
//! What it does: [`natural`] answers for a plane, sphere, cylinder or cone with any other, wherever
//! the pair's meeting holds a conic. A plane cuts each quadric in its conic section; two spheres meet
//! in a circle; coaxial quadrics meet in circles at the roots of their meridians; parallel cylinders
//! in lines; cones of one slope on parallel axes in one conic; and two quadrics with a sphere tangent
//! to both about the meeting of their axes in two conics, Goldman and Miller's condition, each conic
//! a plane through the pair cutting one of them. What is left, a pair in general position, is `None`:
//! the general paths take it.
//!
//! Guarantees:
//!
//! - **Exact type decisions on exact data.** Empty, touching or crossing, and which conic, are signs
//!   from `geom_num::quadric` on whole-number data, or from the stored doubles where the question is a
//!   sum of their products. Elsewhere an interval decides, and a pair it cannot tell is `None`.
//! - **Certified curves.** A result's data are the midpoints of enclosures of the exact conic, and its
//!   class carries how far those enclosures reach.
//! - **Tangency named.** A tangent line or circle is a curve of [`CurveContact::Tangent`] with no band,
//!   and a touch at one point a [`PointContact::IsolatedTangent`].
//! - **Trimmed to both domains.** Each curve is cut where the edges of either surface meet the other,
//!   and no edge cuts a stretch between cuts, or a curve with none, so each is decided by its middle:
//!   kept only where that middle is proven inside both domains, its box inside each range or
//!   reaching only a join, and nothing where that box is off one; where the box reaches an edge of
//!   a domain, the pair is `None`, the engine's (decision 32.94). A circle along a cylinder's end
//!   circle that exact signs place on the other surface (a column standing on a slab) is kept as
//!   that end circle, exactly, decided on its turn and the other domain; one a rounding off it
//!   stays the engine's.
//! - **A turn far from zero loses nothing.** A loop's parameter, or a round surface's, may start any
//!   number of turns out. A loop is cut and kept on its domain moved by whole turns, exactly, to
//!   within two turns of zero, and a surface's turn is read on its own domain the same way, so no
//!   cut and no point is lost to how far out a domain starts. An operand whose parameters start so
//!   far out that their doubles stand more than a step apart on it cannot hold its own points, or
//!   a cut, to a step: it is refused with `FrameRange`, as one outside the frame is.
//!
//! Method: Goldman and Miller, SMA '91 (<https://doi.org/10.1145/112515.112545>) and GMIP 57(1), 1995
//! (<https://doi.org/10.1006/gmip.1995.1006>); the table of chapter 12.7
//! (`projects/engines/geometry/architecture/12-intersections.md`). Two surfaces of revolution tangent
//! to one sphere S along planes P and Q are S − k·P² and S − l·Q², so their difference factors as
//! (√l·Q − √k·P)(√l·Q + √k·P), two planes.
//!
//! Cost: a few predicates and square roots for a pair, and a curve-surface pair for each edge that a
//! trimmed domain has, once a call whatever the count of its pieces. An edge that crosses the
//! other surface costs the engine 13 to 2,481 work units in the stored cases. An edge that lies on
//! the other surface is a closed form, a few predicates, where exact signs place it (decision
//! 32.64), and so is each arc of an end circle that a plane's domain holds a part of (decision
//! 32.69: 62 to 105 units a call where the engine took 1,252,098 to 3,046,840). An end circle
//! about a line through a sphere's centre that stands off the sphere meets nothing by one exact
//! sign (decision 32.74: 23 units a call where the engine took 782,683 for a gap of 0.002).
//! An end circle across the axis of another cylinder or cone parallel to its own meets it where
//! it meets the circle that surface cuts in its plane (decision 32.86: 1,377 to 3,984 units a
//! call where the engine took 3,106,700 to 4,193,766 for four such circles). Where the signs
//! cannot place an edge (an end circle whose centre the frame rounds, as on an axis off the coordinate axes; a
//! sphere's latitude; a round domain short of a whole turn; a plane whose vectors run off the
//! coordinate axes, or whose domain holds a lone point of the circle) it costs the
//! engine boxes all along it, about a million units for a circle of radius 200, bounded by the work
//! budget (issue 974). Recursion: none.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "the names are the chapter's: n a plane's normal, e an axis, c a centre, k a slope"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_curve::{
    Axes, Basis, Circle, CurveError, CurveKind, DerivativeBound, Ellipse, Frame, Hyperbola, Line,
    Parabola, Trimmed,
};
use geom_curve::{HYPERBOLA_REACH, ParamCurve, tight};
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::interval::Interval;
use geom_num::{Exactness, NumError, Sign, predicate, quadric};
use geom_nurbs::{Bounds, Direction, Limit};
use geom_run::WorkMeter;
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};

use crate::closed::{self, Natural, Position};
use crate::curve::{
    enclosed_sum, exact_steps, grid_box, grid_point, held, sign_of, turn_of, whole,
};
use crate::curve_surface::{circle_lies_on, cross, curve_surface, dot, lift, minus, sides};
use crate::error::IntersectError;
use crate::reduced;
use crate::result::{
    Coincidence, Coverage, CurveContact, CurveEnd, Intersection, Item, OverlapRegion, ParamBox,
    PointContact, StepsInterval,
};

/// A vector, enclosed.
pub(crate) type Vector = [Interval; 3];

/// How far a point can stand from a curve and still be read as on it, in steps.
const STEP: f64 = 2.0;

/// How far a shared domain is written, in each parameter of a surface with no end.
const REACH: f64 = 1_099_511_627_776.0;

/// The most periods a loop's parameter is moved by to bring it into the loop's domain: a turn's
/// angle lies from −1 to 2 half-turns, and a loop's domain starts within two turns of zero
/// ([`nearer`]), so three periods reach it from any angle and the fourth pass ends the loop.
const WRAPS: usize = 4;

/// What a pair's closed form finds.
pub(crate) enum Piece {
    /// A curve on both.
    Curve {
        curve: Box<CurveKind<3>>,
        tangent: bool,
        reach: f64,
    },
    /// A point on both.
    Point { at: Vector, tangent: bool },
    /// The two are one surface.
    Same,
}

pub(crate) fn scale(v: Vector, k: Interval) -> Result<Vector, IntersectError> {
    let [x, y, z] = v;
    Ok([x.mul(k)?, y.mul(k)?, z.mul(k)?])
}

pub(crate) fn plus(a: Vector, b: Vector) -> Result<Vector, IntersectError> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    Ok([p.add(x)?, q.add(y)?, r.add(z)?])
}

pub(crate) fn unit(v: Vector) -> Result<Vector, IntersectError> {
    let [x, y, z] = v;
    let length = tight::norm(&[x, y, z])?;
    Ok([x.div(length)?, y.div(length)?, z.div(length)?])
}

pub(crate) fn middle(v: Vector) -> [f64; 3] {
    v.map(tight::middle)
}

/// How wide an enclosure of a vector is.
pub(crate) fn wide(v: Vector) -> f64 {
    let [x, y, z] = v.map(|c: Interval| c.hi() - c.lo());
    tight::larger(tight::larger(x, y), z)
}

pub(crate) fn point(v: f64) -> Result<Interval, IntersectError> {
    Ok(Interval::point(v)?)
}

/// The square root of what is known not to be negative, whose enclosure may reach below zero.
pub(crate) fn root(v: Interval) -> Result<Interval, IntersectError> {
    let lo = if v.lo() < 0.0 { 0.0 } else { v.lo() };
    Ok(Interval::new(lo, tight::larger(lo, v.hi()))?.sqrt()?)
}

/// A vector not along `n`, for a frame's reference direction.
pub(crate) fn beside(n: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = n.map(f64::abs);
    if x <= y && x <= z {
        [1.0, 0.0, 0.0]
    } else if y <= z {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// A full circle about `centre` in the plane of `normal`, as a piece.
pub(crate) fn circle(
    centre: Vector,
    radius: Interval,
    normal: Vector,
    tangent: bool,
) -> Result<Piece, IntersectError> {
    let n = middle(normal);
    let frame = Frame::new(beside(n), n)?;
    let curve = Circle::full(middle(centre), tight::middle(radius), frame)?;
    Ok(Piece::Curve {
        curve: Box::new(CurveKind::Circle(curve)),
        tangent,
        reach: tight::larger(wide(centre), radius.hi() - radius.lo()),
    })
}

/// A line through `at` along `along`, as a piece, over all of the grid its surfaces let it reach.
pub(crate) fn line(at: Vector, along: Vector, tangent: bool) -> Result<Piece, IntersectError> {
    let curve = Line::infinite(middle(at), middle(along))?;
    Ok(Piece::Curve {
        curve: Box::new(CurveKind::Line(curve)),
        tangent,
        reach: wide(at),
    })
}

/// How a sign stands: the exact decision where one came, the enclosure's otherwise, or `None`.
pub(crate) fn decided(exact: Option<Result<Sign, NumError>>, enclosure: Interval) -> Option<Sign> {
    if let Some(Ok(sign)) = exact {
        return Some(sign);
    }
    if enclosure.lo() > 0.0 {
        Some(Sign::Positive)
    } else if enclosure.hi() < 0.0 {
        Some(Sign::Negative)
    } else {
        None
    }
}

/// A plane as a point and a unit normal, whatever made it.
#[derive(Clone, Copy)]
pub(crate) struct Sheet {
    pub(crate) at: Vector,
    pub(crate) normal: Vector,
    pub(crate) exact: Option<quadric::Plane>,
}

/// The point `c + v`, if `c` and `v` are both on the grid and their sum is in reach. The sum is
/// made in integers, not doubles (#1067): near 2^39 a double holds only 2^-13, so a stored
/// vector's fraction under half of that rounds away in `c + v`, the sum lands on the grid, and the
/// closed forms would be handed another plane or axis from the stored one.
fn grid_sum(c: &[f64; 3], v: &[f64; 3]) -> Option<P3> {
    let (c, v) = (grid_point(c)?, grid_point(v)?);
    P3::new(
        c.x.get().checked_add(v.x.get())?,
        c.y.get().checked_add(v.y.get())?,
        c.z.get().checked_add(v.z.get())?,
    )
    .ok()
}

pub(crate) fn sheet(plane: &Plane) -> Result<Sheet, IntersectError> {
    let (c, a, b) = plane.vectors();
    let exact = (|| {
        Some(quadric::Plane {
            a: grid_point(&c)?,
            b: grid_sum(&c, &a)?,
            c: grid_sum(&c, &b)?,
        })
    })();
    Ok(Sheet {
        at: lift(c)?,
        normal: unit(cross(lift(a)?, lift(b)?)?)?,
        exact,
    })
}

/// An axis: a point on it, its unit direction, and two grid points on it if the data allow.
#[derive(Clone, Copy)]
pub(crate) struct Axis {
    pub(crate) at: Vector,
    pub(crate) along: Vector,
    pub(crate) exact: Option<(P3, P3)>,
}

pub(crate) fn axis(center: [f64; 3], frame: &Frame<3>, sign: f64) -> Result<Axis, IntersectError> {
    let (_, [_, _, n]) = frame.unit_vectors()?;
    let exact = (|| Some((grid_point(&center)?, grid_sum(&center, &frame.normal())?)))();
    let sign = tight::point(sign)?;
    let [x, y, z] = n;
    Ok(Axis {
        at: lift(center)?,
        along: [
            tight::mul(x, sign)?,
            tight::mul(y, sign)?,
            tight::mul(z, sign)?,
        ],
        exact,
    })
}

/// A cone by its apex, its unit axis, and the tangent of its half-angle.
#[derive(Clone, Copy)]
pub(crate) struct Funnel {
    pub(crate) apex: Vector,
    pub(crate) along: Vector,
    pub(crate) slope: Interval,
    extent: f64,
    pub(crate) exact: Option<quadric::Cone>,
}

pub(crate) fn funnel(cone: &Cone) -> Result<Funnel, IntersectError> {
    // S(u, v) = c + (r₀ + k·v)·e(u) + s·v·n̂: v runs along sign(s)·n̂, |s| of height for each unit
    // of it, so the radius grows by |k/s| per unit of height and the apex, at v = −r₀/k, stands
    // |s|·(−r₀/k) along that unit axis. The axial factor s is ±1 as built, where these are |k| and
    // −r₀/k themselves, but a similarity multiplies it by its scale (and r₀ and k with it).
    let s = cone.axial_sign();
    let line = axis(cone.center(), &cone.frame(), s.signum())?;
    let (r0, k) = cone.radius_and_slope();
    // Exact where the data are: a grid apex is what makes the exact decisions possible. A cone
    // stored by its apex has it as its centre, exactly, whatever its axis; any other is reached
    // along the unit axis, exact only where that is.
    let apex = if r0 == 0.0 {
        line.at
    } else {
        let v = tight::div(tight::point(-r0)?, tight::point(k)?)?;
        let h = tight::mul(v, tight::point(s.abs())?)?;
        let [a, b, c] = line.at;
        let [x, y, z] = line.along;
        [
            tight::add(a, tight::mul(x, h)?)?,
            tight::add(b, tight::mul(y, h)?)?,
            tight::add(c, tight::mul(z, h)?)?,
        ]
    };
    let slope = tight::div(point(k.abs())?, point(s.abs())?)?;
    let exact = (|| {
        if wide(apex) > 0.0 || slope.lo() != slope.hi() {
            return None;
        }
        let centre = middle(apex);
        Some(quadric::Cone {
            apex: grid_point(&centre)?,
            along: grid_sum(&centre, &cone.frame().normal())?,
            slope: quadric::Slope {
                rise: u64::try_from(whole(slope.lo() * 1_048_576.0)?).ok()?,
                run: 1 << 20,
            },
        })
    })();
    Ok(Funnel {
        apex,
        along: line.along,
        slope,
        extent: cone_extent(cone),
        exact,
    })
}

/// Which of the two sides of a sum of products of doubles, exactly: (p − c) × e, for whether `p` lies
/// on the line through `c` along `e`.
pub(crate) fn off_line(p: [f64; 3], c: [f64; 3], e: [f64; 3]) -> bool {
    let [p0, p1, p2] = p;
    let [c0, c1, c2] = c;
    let [e0, e1, e2] = e;
    let parts = [
        sign_of(&[(p1, e2), (-c1, e2), (-p2, e1), (c2, e1)]),
        sign_of(&[(p2, e0), (-c2, e0), (-p0, e2), (c0, e2)]),
        sign_of(&[(p0, e1), (-c0, e1), (-p1, e0), (c1, e0)]),
    ];
    parts.iter().any(|s| *s != Sign::Zero)
}

/// Whether two stored directions are parallel, exactly.
pub(crate) fn parallel(a: [f64; 3], b: [f64; 3]) -> bool {
    let [p, q, r] = a;
    let [x, y, z] = b;
    [
        sign_of(&[(q, z), (-r, y)]),
        sign_of(&[(r, x), (-p, z)]),
        sign_of(&[(p, y), (-q, x)]),
    ]
    .iter()
    .all(|s| *s == Sign::Zero)
}

/// Whether two axes are one line, by their stored doubles.
pub(crate) fn same_line(a: [f64; 3], e: [f64; 3], b: [f64; 3], f: [f64; 3]) -> bool {
    parallel(e, f) && !off_line(b, a, e)
}

/// A plane with a plane.
fn plane_plane(a: &Plane, b: &Plane) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (p, q) = (sheet(a)?, sheet(b)?);
    let d = cross(p.normal, q.normal)?;
    // The second plane's three grid points are its sheet's, its far points summed exactly.
    let exact = match (p.exact, q.exact) {
        (Some(s), Some(t)) => {
            let parallel = quadric::plane_along(&s, t.a, t.b)? == Sign::Zero
                && quadric::plane_along(&s, t.a, t.c)? == Sign::Zero;
            Some((
                parallel,
                predicate::orient3d(s.a, s.b, s.c, t.a)? == Sign::Zero,
            ))
        }
        _ => None,
    };
    // Parallel, and one plane, only from the exact signs: every operation rounds outward, so an
    // enclosure of the normals' cross product is never the one point zero.
    if let Some((true, inside)) = exact {
        return Ok(Some(if inside {
            alloc::vec![Piece::Same]
        } else {
            Vec::new()
        }));
    }
    if d.iter().all(|c| c.contains(0.0)) {
        return Ok(None);
    }
    let (h, k) = (dot(p.normal, p.at)?, dot(q.normal, q.at)?);
    let at = plus(
        scale(cross(q.normal, d)?, h)?,
        scale(cross(d, p.normal)?, k)?,
    )?;
    let at = scale(at, point(1.0)?.div(dot(d, d)?)?)?;
    Ok(Some(alloc::vec![line(at, d, false)?]))
}

/// A plane with a sphere.
fn plane_sphere(a: &Plane, b: &Sphere) -> Result<Option<Vec<Piece>>, IntersectError> {
    let p = sheet(a)?;
    let c = lift(b.center())?;
    let r = point(b.radius())?;
    let h = dot(p.normal, minus(c, p.at)?)?;
    let exact = (|| {
        let plane = p.exact?;
        let centre = grid_point(&b.center())?;
        let radius = Steps(u64::try_from(whole(b.radius())?).ok()?);
        Some(quadric::plane_ball(&plane, centre, radius))
    })();
    let squared = r.square()?.sub(h.square()?)?;
    let Some(sign) = decided(exact, squared) else {
        return Ok(None);
    };
    let foot = minus(c, scale(p.normal, h)?)?;
    Ok(Some(match sign {
        Sign::Negative => Vec::new(),
        Sign::Zero => alloc::vec![Piece::Point {
            at: foot,
            tangent: true
        }],
        Sign::Positive => alloc::vec![circle(foot, root(squared)?, p.normal, false)?],
    }))
}

/// The conic a plane cuts from a cylinder, given how the axis stands to the plane.
fn cut_cylinder(
    sheet: &Sheet,
    tube: &Axis,
    r: Interval,
    along: Option<Sign>,
    across: bool,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (n, e) = (sheet.normal, tube.along);
    let tilt = dot(n, e)?;
    if along == Some(Sign::Zero) {
        return Ok(None);
    }
    // Not along the axis, or not known to be, yet no enclosure of the tilt tells it from zero (an
    // exact plane all but along it): the centre cannot be divided out, so the pair is left
    // undecided.
    if tilt.contains(0.0) {
        return Ok(None);
    }
    let reach = dot(minus(sheet.at, tube.at)?, n)?.div(tilt)?;
    let centre = plus(tube.at, scale(e, reach)?)?;
    // Square to the axis, or within an enclosure of it: a circle, which stands within the piece's
    // reach of an ellipse that close to one.
    let side = cross(n, e)?;
    if across || side.iter().all(|c| c.contains(0.0)) {
        return Ok(Some(alloc::vec![circle(centre, r, n, false)?]));
    }
    let minor = unit(side)?;
    let major = unit(cross(n, minor)?)?;
    let long = r.div(tilt)?;
    let (u, v) = (scale(major, long)?, scale(minor, r)?);
    let axes = Axes::Vectors {
        u: middle(u),
        v: middle(v),
    };
    Ok(Some(alloc::vec![Piece::Curve {
        curve: Box::new(CurveKind::Ellipse(Ellipse::new(
            middle(centre),
            axes,
            (0.0, 2.0)
        )?)),
        tangent: false,
        reach: tight::larger(wide(centre), tight::larger(wide(u), wide(v))),
    }]))
}

/// A plane with a cylinder.
fn plane_cylinder(a: &Plane, b: &Cylinder) -> Result<Option<Vec<Piece>>, IntersectError> {
    let p = sheet(a)?;
    let tube = axis(b.center(), &b.frame(), 1.0)?;
    let r = point(b.radius())?;
    let tilt = dot(p.normal, tube.along)?;
    let along = match (p.exact, tube.exact) {
        (Some(s), Some((g, h))) => decided(Some(quadric::plane_along(&s, g, h)), tilt),
        _ => decided(None, tilt),
    };
    if along == Some(Sign::Zero) {
        // Parallel to the axis: rulings, as far from the axis as the plane stands.
        let h = dot(p.normal, minus(tube.at, p.at)?)?;
        let exact = (|| {
            let plane = p.exact?;
            let centre = tube.exact?.0;
            let radius = Steps(u64::try_from(whole(b.radius())?).ok()?);
            Some(quadric::plane_ball(&plane, centre, radius))
        })();
        let squared = r.square()?.sub(h.square()?)?;
        let Some(sign) = decided(exact, squared) else {
            return Ok(None);
        };
        let foot = minus(tube.at, scale(p.normal, h)?)?;
        return Ok(Some(match sign {
            Sign::Negative => Vec::new(),
            Sign::Zero => alloc::vec![line(foot, tube.along, true)?],
            Sign::Positive => {
                let side = scale(unit(cross(p.normal, tube.along)?)?, root(squared)?)?;
                alloc::vec![
                    line(plus(foot, side)?, tube.along, false)?,
                    line(minus(foot, side)?, tube.along, false)?,
                ]
            }
        }));
    }
    let across = match (p.exact, tube.exact) {
        (Some(s), Some((g, h))) => quadric::plane_across(&s, g, h)?,
        _ => false,
    };
    cut_cylinder(&p, &tube, r, along, across)
}

/// The conic a plane cuts from a cone, by the sign of its section, the apex off the plane.
fn cut_cone(
    sheet: &Sheet,
    cone: &Funnel,
    sign: Sign,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let extent = cone.extent;
    let (n, e) = (sheet.normal, cone.along);
    let beta = dot(n, e)?;
    let lean = minus(e, scale(n, beta)?)?;
    let across = dot(lean, lean)?;
    if across.lo() <= 0.0 {
        // The plane is square to the axis: a circle, its radius the slope times the height.
        let h = dot(n, minus(cone.apex, sheet.at)?)?;
        let foot = minus(cone.apex, scale(n, h)?)?;
        let radius = cone.slope.mul(tight_size(h))?;
        return Ok(Some(alloc::vec![circle(foot, radius, n, false)?]));
    }
    let a = unit(lean)?;
    let b = cross(n, a)?;
    let alpha = dot(a, e)?;
    let k = point(1.0)?.add(cone.slope.square()?)?;
    let h = dot(n, minus(cone.apex, sheet.at)?)?;
    let foot = minus(cone.apex, scale(n, h)?)?;
    let bent = point(1.0)?.sub(k.mul(alpha.square()?)?)?;
    let cross_term = k.mul(h)?.mul(alpha)?.mul(beta)?;
    // An exact sign can stand where no enclosure of what the conic is divided by tells it from
    // zero (a plane all but along a generator, or passing all but through the apex): its axes
    // cannot be divided out, so the pair is left undecided.
    let divisor = if sign == Sign::Zero { cross_term } else { bent };
    if divisor.contains(0.0) {
        return Ok(None);
    }
    let rest = h.square()?.mul(point(1.0)?.sub(k.mul(beta.square()?)?)?)?;
    Ok(Some(match sign {
        Sign::Positive => {
            let s0 = cross_term.neg().div(bent)?;
            let c = bent.mul(s0.square()?)?.sub(rest)?;
            let centre = plus(foot, scale(a, s0)?)?;
            let (u, v) = (scale(a, root(c.div(bent)?)?)?, scale(b, root(c)?)?);
            let axes = Axes::Vectors {
                u: middle(u),
                v: middle(v),
            };
            alloc::vec![Piece::Curve {
                curve: Box::new(CurveKind::Ellipse(Ellipse::new(
                    middle(centre),
                    axes,
                    (0.0, 2.0)
                )?)),
                tangent: false,
                reach: tight::larger(wide(centre), tight::larger(wide(u), wide(v))),
            }]
        }
        Sign::Zero => {
            let twice = cross_term.mul(point(2.0)?)?;
            let shift = rest.neg().div(twice)?;
            let centre = plus(foot, scale(a, shift)?)?;
            let u = scale(a, point(-1.0)?.div(twice)?)?;
            let axes = Axes::Vectors {
                u: middle(u),
                v: middle(b),
            };
            let reach = tight::larger(extent, 1.0);
            let curve = Parabola::new(middle(centre), axes, (-reach, reach))?;
            alloc::vec![Piece::Curve {
                curve: Box::new(CurveKind::Parabola(curve)),
                tangent: false,
                reach: tight::larger(wide(centre), wide(u)),
            }]
        }
        Sign::Negative => {
            let s0 = cross_term.neg().div(bent)?;
            let c = bent.mul(s0.square()?)?.sub(rest)?;
            let centre = plus(foot, scale(a, s0)?)?;
            let wide_bent = bent.neg();
            let (lead, side) = if c.hi() < 0.0 {
                (
                    scale(a, root(c.neg().div(wide_bent)?)?)?,
                    scale(b, root(c.neg())?)?,
                )
            } else if c.lo() > 0.0 {
                (scale(b, root(c)?)?, scale(a, root(c.div(wide_bent)?)?)?)
            } else {
                return Ok(None);
            };
            let span = branch_span(extent, lead, side)?;
            let branch = |sign: f64| -> Result<Piece, IntersectError> {
                let u = scale(lead, point(sign)?)?;
                let axes = Axes::Vectors {
                    u: middle(u),
                    v: middle(side),
                };
                let curve = Hyperbola::new(middle(centre), axes, (-span, span))?;
                Ok(Piece::Curve {
                    curve: Box::new(CurveKind::Hyperbola(curve)),
                    tangent: false,
                    reach: tight::larger(wide(centre), wide(u)),
                })
            };
            alloc::vec![branch(1.0)?, branch(-1.0)?]
        }
    }))
}

/// The t of a hyperbola's branches past which they lie beyond `extent` of its centre.
fn branch_span(extent: f64, lead: Vector, side: Vector) -> Result<f64, IntersectError> {
    let smaller = tight::smaller(tight::norm(&lead)?.lo(), tight::norm(&side)?.lo());
    let ratio = point(2.0 * extent + 2.0)?.div(point(smaller)?)?;
    let t = ratio.add(point(2.0)?)?.ln()?.hi();
    Ok(tight::smaller(t, HYPERBOLA_REACH))
}

/// How far a cone's domain reaches from its apex, along its surface.
fn cone_extent(cone: &Cone) -> f64 {
    let (r0, k) = cone.radius_and_slope();
    let (_, (v0, v1)) = ParamSurface::domain(cone);
    // The axial factor s, which a similarity scales, does not enter: a parabola's parameter is its
    // distance across, at most the radius |r₀ + k·v| this bounds, and a hyperbola's branches run
    // past this by the ratio of its axes (`branch_span` divides by the shorter), which covers the
    // height |s·v| (tests/axial_factor.rs: a nappe 400 high and 20 wide, cut along its axis).
    let far = tight::larger(v0.abs(), v1.abs());
    (far + r0.abs()) * (1.0 + k.abs()) + 1.0
}

/// Whether a plane holds a grid point, exactly where it can say, by enclosure otherwise.
fn on_sheet(sheet: &Sheet, at: Vector) -> Result<Option<bool>, IntersectError> {
    if let (Some(s), Some(g), true) = (sheet.exact, grid_point(&middle(at)), wide(at) == 0.0) {
        return Ok(Some(predicate::orient3d(s.a, s.b, s.c, g)? == Sign::Zero));
    }
    let h = dot(sheet.normal, minus(at, sheet.at)?)?;
    Ok(if h.contains(0.0) { None } else { Some(false) })
}

/// A plane with a cone.
fn plane_cone(a: &Plane, b: &Cone) -> Result<Option<Vec<Piece>>, IntersectError> {
    let p = sheet(a)?;
    let c = funnel(b)?;
    let n = p.normal;
    let tilt = dot(n, c.along)?;
    let k = point(1.0)?.add(c.slope.square()?)?;
    let enclosure = tilt.square()?.mul(k)?.sub(c.slope.square()?)?;
    let exact = (|| Some(quadric::plane_cone(&p.exact?, &c.exact?)))();
    let Some(sign) = decided(exact, enclosure) else {
        return Ok(None);
    };
    let Some(through) = on_sheet(&p, c.apex)? else {
        return Ok(None);
    };
    if !through {
        return cut_cone(&p, &c, sign);
    }
    let lean = minus(c.along, scale(n, tilt)?)?;
    Ok(Some(match sign {
        Sign::Positive => alloc::vec![Piece::Point {
            at: c.apex,
            tangent: true
        }],
        Sign::Zero => alloc::vec![line(c.apex, unit(lean)?, true)?],
        Sign::Negative => {
            let a = unit(lean)?;
            let b = cross(n, a)?;
            let alpha = dot(a, c.along)?;
            let bent = k.mul(alpha.square()?)?.sub(point(1.0)?)?;
            let side = scale(b, root(bent)?)?;
            alloc::vec![
                line(c.apex, plus(a, side)?, false)?,
                line(c.apex, minus(a, side)?, false)?,
            ]
        }
    }))
}

/// A sphere with a sphere, its count from exact signs on the stored doubles.
fn sphere_sphere(a: &Sphere, b: &Sphere) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (c, e) = (a.center(), b.center());
    let (r, q) = (a.radius(), b.radius());
    if c == e {
        return Ok(Some(if r == q {
            alloc::vec![Piece::Same]
        } else {
            Vec::new()
        }));
    }
    let [x0, y0, z0] = c;
    let [x1, y1, z1] = e;
    let compare = |sum: bool| {
        let other = if sum { -q } else { q };
        sign_of(&[
            (x0, x0),
            (x1, x1),
            (y0, y0),
            (y1, y1),
            (z0, z0),
            (z1, z1),
            (-2.0 * x0, x1),
            (-2.0 * y0, y1),
            (-2.0 * z0, z1),
            (-r, r),
            (-q, q),
            (2.0 * r, other),
        ])
    };
    let (outer, inner) = (compare(true), compare(false));
    if outer == Sign::Positive || inner == Sign::Negative {
        return Ok(Some(Vec::new()));
    }
    let run = minus(lift(e)?, lift(c)?)?;
    let [p, u, v] = run;
    let d = tight::norm(&[p, u, v])?;
    let (first, second) = (point(r)?, point(q)?);
    let foot = first
        .square()?
        .add(d.sub(second)?.mul(d.add(second)?)?)?
        .div(d.mul(point(2.0)?)?)?;
    let along = unit(run)?;
    let centre = plus(lift(c)?, scale(along, foot)?)?;
    if outer == Sign::Zero || inner == Sign::Zero {
        return Ok(Some(alloc::vec![Piece::Point {
            at: centre,
            tangent: true
        }]));
    }
    let high = root(first.square()?.sub(foot.square()?)?)?;
    Ok(Some(alloc::vec![circle(centre, high, along, false)?]))
}

/// A height along an axis from a point on it, of a point of space.
pub(crate) fn height(at: Vector, on: &Axis) -> Result<Interval, IntersectError> {
    dot(minus(at, on.at)?, on.along)
}

/// The circle about `on` at height `z` with the given radius.
pub(crate) fn ring(
    on: &Axis,
    z: Interval,
    r: Interval,
    tangent: bool,
) -> Result<Piece, IntersectError> {
    circle(plus(on.at, scale(on.along, z)?)?, r, on.along, tangent)
}

/// A sphere with a cylinder about an axis through its centre.
fn sphere_cylinder(a: &Sphere, b: &Cylinder) -> Result<Option<Vec<Piece>>, IntersectError> {
    let e = b.frame().normal();
    if off_line(a.center(), b.center(), e) {
        return Ok(None);
    }
    let on = axis(a.center(), &b.frame(), 1.0)?;
    let (r, q) = (a.radius(), b.radius());
    let squared = point(r)?.square()?.sub(point(q)?.square()?)?;
    Ok(Some(if r < q {
        Vec::new()
    } else if r == q {
        alloc::vec![ring(&on, point(0.0)?, point(q)?, true)?]
    } else {
        let z = root(squared)?;
        alloc::vec![
            ring(&on, z, point(q)?, false)?,
            ring(&on, z.neg(), point(q)?, false)?,
        ]
    }))
}

/// A sphere with a cone about an axis through its centre: the roots of a quadratic in the height.
fn sphere_cone(a: &Sphere, b: &Cone) -> Result<Option<Vec<Piece>>, IntersectError> {
    if off_line(a.center(), b.center(), b.frame().normal()) {
        return Ok(None);
    }
    let c = funnel(b)?;
    let on = Axis {
        at: c.apex,
        along: c.along,
        exact: None,
    };
    let z0 = height(lift(a.center())?, &on)?;
    let k = point(1.0)?.add(c.slope.square()?)?;
    let r = point(a.radius())?;
    let d = z0.square()?.sub(k.mul(z0.square()?.sub(r.square()?)?)?)?;
    if d.hi() < 0.0 {
        return Ok(Some(Vec::new()));
    }
    if d.lo() <= 0.0 {
        return Ok(None);
    }
    let s = root(d)?;
    let mut out = Vec::new();
    for sign in [1.0, -1.0] {
        let z = z0.add(s.mul(point(sign)?)?)?.div(k)?;
        // A sphere through the apex: no circle there, and no enclosure proves the point.
        if z.contains(0.0) {
            return Ok(None);
        }
        let radius = c.slope.mul(tight_size(z))?;
        out.push(ring(&on, z, radius, false)?);
    }
    Ok(Some(out))
}

/// The size of `x`, which does not straddle zero.
fn tight_size(x: Interval) -> Interval {
    if x.hi() < 0.0 { x.neg() } else { x }
}

/// Two cylinders.
fn cylinder_cylinder(a: &Cylinder, b: &Cylinder) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (e, f) = (a.frame().normal(), b.frame().normal());
    let (c, g) = (a.center(), b.center());
    if parallel(e, f) {
        if same_line(c, e, g, f) {
            return Ok(Some(if a.radius() == b.radius() {
                alloc::vec![Piece::Same]
            } else {
                Vec::new()
            }));
        }
        return parallel_cylinders(a, b).map(Some);
    }
    meeting_axes(&tube(a)?, &tube(b)?)
}

/// Two cylinders on parallel axes: the crossings of their circles, along the axis.
fn parallel_cylinders(a: &Cylinder, b: &Cylinder) -> Result<Vec<Piece>, IntersectError> {
    let first = axis(a.center(), &a.frame(), 1.0)?;
    let second = axis(b.center(), &b.frame(), 1.0)?;
    let (r, q) = (a.radius(), b.radius());
    let exact = |g: f64| -> Option<Sign> {
        let (p, s) = first.exact?;
        let (t, _) = second.exact?;
        let gap = Steps(u64::try_from(whole(g)?).ok()?);
        quadric::axis_gap(p, s, t, gap).ok()
    };
    let w = minus(second.at, first.at)?;
    let run = minus(w, scale(first.along, dot(w, first.along)?)?)?;
    let [p, u, v] = run;
    let d = tight::norm(&[p, u, v])?;
    // On axes a hair apart, rounding on numbers the size of the centres swallows
    // the offset, whose enclosure then holds zero: it is taken from its exact sums instead.
    let lost = d.lo() <= 0.0;
    let (run, d) = if lost {
        let run = exact_run(a, b)?;
        let [p, u, v] = run;
        (run, tight::norm(&[p, u, v])?)
    } else {
        (run, d)
    };
    let (one, two) = (point(r)?, point(q)?);
    let sum = one.add(two)?;
    let gap = one.sub(two)?;
    let outer = exact(r + q).or_else(|| decided(None, d.sub(sum).ok()?));
    let inner = exact((r - q).abs()).or_else(|| decided(None, d.sub(tight_size(gap)).ok()?));
    if outer == Some(Sign::Positive) || inner == Some(Sign::Negative) {
        return Ok(Vec::new());
    }
    let along = unit(run)?;
    let twice = d.mul(point(2.0)?)?;
    // The foot (r² + d² − q²)/2d: over an offset that small, r² − q² rounded at the size of r²
    // would swamp d², so the exact route takes r² − q² exactly too.
    let foot = if lost {
        let squares = enclosed_sum(&[(r, r), (-q, q)])?;
        d.div(point(2.0)?)?.add(squares.div(twice)?)?
    } else {
        one.square()?
            .add(d.sub(two)?.mul(d.add(two)?)?)?
            .div(twice)?
    };
    let at = plus(first.at, scale(along, foot)?)?;
    if outer == Some(Sign::Zero) || inner == Some(Sign::Zero) {
        return Ok(alloc::vec![line(at, first.along, true)?]);
    }
    let area = d
        .add(sum)?
        .mul(sum.sub(d)?)?
        .mul(d.sub(gap)?)?
        .mul(d.add(gap)?)?;
    let high = root(area)?.div(d.mul(point(2.0)?)?)?;
    let side = scale(cross(first.along, along)?, high)?;
    Ok(alloc::vec![
        line(plus(at, side)?, first.along, false)?,
        line(minus(at, side)?, first.along, false)?,
    ])
}

/// The offset, square to the axes, from the axis of `a` to the parallel axis of `b`, from exact
/// sums: n = (b's centre − a's centre) × e is a sum of products of the stored doubles, so each of
/// its parts is enclosed to a step of its exact value however small, and the offset is e × n over
/// e · e.
fn exact_run(a: &Cylinder, b: &Cylinder) -> Result<Vector, IntersectError> {
    let [p0, p1, p2] = b.center();
    let [c0, c1, c2] = a.center();
    let e = a.frame().normal();
    let [e0, e1, e2] = e;
    let n = [
        enclosed_sum(&[(p1, e2), (-c1, e2), (-p2, e1), (c2, e1)])?,
        enclosed_sum(&[(p2, e0), (-c2, e0), (-p0, e2), (c0, e2)])?,
        enclosed_sum(&[(p0, e1), (-c0, e1), (-p1, e0), (c1, e0)])?,
    ];
    let e = lift(e)?;
    let size = dot(e, e)?;
    let [x, y, z] = cross(e, n)?;
    Ok([x.div(size)?, y.div(size)?, z.div(size)?])
}

/// A surface of revolution with a sphere tangent about the meeting of two axes.
#[derive(Clone, Copy)]
enum Round {
    /// A cylinder: its axis and radius, and itself on whole-number data.
    Tube(Axis, f64, Option<quadric::Cylinder>),
    /// A cone.
    Horn(Funnel),
}

impl Round {
    fn line(&self) -> Axis {
        match self {
            Round::Tube(on, ..) => *on,
            Round::Horn(c) => Axis {
                at: c.apex,
                along: c.along,
                exact: None,
            },
        }
    }

    /// The squared radius of the sphere about `at` tangent to the surface, the factor k of
    /// S − k·P², and the height along the axis, from `at`, of the plane of tangency.
    fn tangency(&self, at: Vector) -> Result<(Interval, Interval, Interval), IntersectError> {
        match self {
            Round::Tube(_, r, _) => Ok((point(*r)?.square()?, point(1.0)?, point(0.0)?)),
            Round::Horn(c) => {
                let k = point(1.0)?.add(c.slope.square()?)?;
                // z_a: the apex's height from `at`, along the axis.
                let z = dot(minus(c.apex, at)?, c.along)?;
                let sine = c.slope.square()?.div(k)?;
                Ok((z.square()?.mul(sine)?, k, z.mul(sine)?))
            }
        }
    }

    /// The conic a plane cuts from this surface.
    fn section(&self, plane: &Sheet) -> Result<Option<Vec<Piece>>, IntersectError> {
        match self {
            Round::Tube(on, r, _) => {
                let tilt = dot(plane.normal, on.along)?;
                let along = decided(None, tilt);
                cut_cylinder(plane, on, point(*r)?, along, false)
            }
            Round::Horn(c) => {
                let tilt = dot(plane.normal, c.along)?;
                let k = point(1.0)?.add(c.slope.square()?)?;
                let enclosure = tilt.square()?.mul(k)?.sub(c.slope.square()?)?;
                let Some(sign) = decided(None, enclosure) else {
                    return Ok(None);
                };
                cut_cone(plane, c, sign)
            }
        }
    }

    /// The same surface as `closed` writes it, on whole-number data.
    fn natural(&self) -> Option<Natural> {
        match self {
            Round::Tube(.., exact) => exact.map(Natural::Cylinder),
            Round::Horn(c) => c.exact.map(Natural::Cone),
        }
    }
}

/// A cylinder as a surface of revolution.
fn tube(c: &Cylinder) -> Result<Round, IntersectError> {
    let on = axis(c.center(), &c.frame(), 1.0)?;
    let exact = (|| {
        let (base, along) = on.exact?;
        Some(quadric::Cylinder {
            base,
            along,
            radius: Steps(u64::try_from(whole(c.radius())?).ok()?),
        })
    })();
    Ok(Round::Tube(on, c.radius(), exact))
}

/// Where the lines through `a` along `e` and through `b` along `f` meet, those lines not parallel.
fn crossing(a: Vector, e: Vector, b: Vector, f: Vector) -> Result<Vector, IntersectError> {
    let n = cross(e, f)?;
    let t = dot(cross(minus(b, a)?, f)?, n)?.div(dot(n, n)?)?;
    plus(a, scale(e, t)?)
}

/// Two surfaces whose axes meet, with one sphere about that point tangent to both: two conics.
fn meeting_axes(a: &Round, b: &Round) -> Result<Option<Vec<Piece>>, IntersectError> {
    // That the axes meet, and that one sphere touches both, are equalities: only exact data can say.
    let (Some(first), Some(second)) = (a.natural(), b.natural()) else {
        return Ok(None);
    };
    if closed::position(&first, &second)? != Position::TangentSphere {
        return Ok(None);
    }
    let (p, q) = (a.line(), b.line());
    let at = crossing(p.at, p.along, q.at, q.along)?;
    let (_, k, z) = a.tangency(at)?;
    let (_, l, w) = b.tangency(at)?;
    let (root_k, root_l) = (root(k)?, root(l)?);
    let mut out = Vec::new();
    for sign in [1.0, -1.0] {
        let normal = plus(
            scale(q.along, root_l)?,
            scale(p.along, root_k.mul(point(sign)?)?)?,
        )?;
        let [x, y, c] = normal;
        let size = tight::norm(&[x, y, c])?;
        if size.lo() <= 0.0 {
            continue;
        }
        let offset = root_l
            .mul(w)?
            .add(root_k.mul(z)?.mul(point(sign)?)?)?
            .div(size)?;
        let unit_normal = unit(normal)?;
        let plane = Sheet {
            at: plus(at, scale(unit_normal, offset)?)?,
            normal: unit_normal,
            exact: None,
        };
        let Some(pieces) = a.section(&plane)? else {
            return Ok(None);
        };
        out.extend(pieces);
    }
    Ok(Some(out))
}

/// A cylinder with a cone.
fn cylinder_cone(a: &Cylinder, b: &Cone) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (e, f) = (a.frame().normal(), b.frame().normal());
    let c = funnel(b)?;
    if parallel(e, f) {
        if !same_line(a.center(), e, b.center(), f) {
            return Ok(None);
        }
        // Coaxial: where the cone's radius is the cylinder's, on each nappe.
        let on = Axis {
            at: c.apex,
            along: c.along,
            exact: None,
        };
        let r = point(a.radius())?;
        let z = r.div(c.slope)?;
        return Ok(Some(alloc::vec![
            ring(&on, z, r, false)?,
            ring(&on, z.neg(), r, false)?,
        ]));
    }
    meeting_axes(&tube(a)?, &Round::Horn(c))
}

/// Two cones.
fn cone_cone(a: &Cone, b: &Cone) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (e, f) = (a.frame().normal(), b.frame().normal());
    let (p, q) = (funnel(a)?, funnel(b)?);
    if parallel(e, f) {
        let same = same_line(a.center(), e, b.center(), f);
        let equal = p.slope.lo() == q.slope.lo();
        if same {
            // One cone only if every stored datum is equal, the axial factor too: with it negated
            // the cone is mirrored, and with it scaled its slope per unit of height is k/s.
            let stored = e == f
                && a.center() == b.center()
                && a.radius_and_slope() == b.radius_and_slope()
                && a.axial_sign() == b.axial_sign();
            if stored {
                return Ok(Some(alloc::vec![Piece::Same]));
            }
            return coaxial_cones(&p, &q, equal);
        }
        if equal {
            return slopes_alike(&p, &q);
        }
        return Ok(None);
    }
    let shared = wide(p.apex) == 0.0 && wide(q.apex) == 0.0 && middle(p.apex) == middle(q.apex);
    if shared {
        return shared_apex(&p, &q);
    }
    meeting_axes(&Round::Horn(p), &Round::Horn(q))
}

/// Two cones on one axis.
fn coaxial_cones(
    p: &Funnel,
    q: &Funnel,
    equal: bool,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let on = Axis {
        at: p.apex,
        along: p.along,
        exact: None,
    };
    let z = dot(minus(q.apex, p.apex)?, p.along)?;
    if z.contains(0.0) {
        // One apex, if it is proven: the same cone, or that one point.
        let one = wide(p.apex) == 0.0 && wide(q.apex) == 0.0 && middle(p.apex) == middle(q.apex);
        return Ok(one.then(|| {
            alloc::vec![if equal {
                Piece::Same
            } else {
                Piece::Point {
                    at: p.apex,
                    tangent: true,
                }
            }]
        }));
    }
    let mut out = Vec::new();
    // k·h = ±l·(h − z).
    for sign in [1.0, -1.0] {
        let l = q.slope.mul(point(sign)?)?;
        let under = p.slope.sub(l)?;
        if under.contains(0.0) {
            continue;
        }
        let h = l.neg().mul(z)?.div(under)?;
        let radius = p.slope.mul(tight_size(h))?;
        out.push(ring(&on, h, radius, false)?);
    }
    Ok(Some(out))
}

/// Two cones of one slope on parallel axes: their difference is a plane.
fn slopes_alike(p: &Funnel, q: &Funnel) -> Result<Option<Vec<Piece>>, IntersectError> {
    let k = point(1.0)?.add(p.slope.square()?)?;
    let w = minus(p.apex, q.apex)?;
    let e = p.along;
    let normal = minus(w, scale(e, k.mul(dot(w, e)?)?)?)?;
    let [x, y, z] = normal;
    let size = tight::norm(&[x, y, z])?;
    if size.lo() <= 0.0 {
        return Ok(None);
    }
    let (a, b) = (dot(p.apex, p.apex)?, dot(q.apex, q.apex)?);
    let (s, t) = (dot(p.apex, e)?, dot(q.apex, e)?);
    let offset = a
        .sub(b)?
        .sub(k.mul(s.square()?.sub(t.square()?)?)?)?
        .div(point(2.0)?)?
        .div(size)?;
    let unit_normal = unit(normal)?;
    let plane = Sheet {
        at: scale(unit_normal, offset)?,
        normal: unit_normal,
        exact: None,
    };
    let tilt = dot(unit_normal, e)?;
    let enclosure = tilt.square()?.mul(k)?.sub(p.slope.square()?)?;
    let Some(sign) = decided(None, enclosure) else {
        return Ok(None);
    };
    cut_cone(&plane, p, sign)
}

/// Two cones through one apex: rulings, a direction at each half-angle from each axis. `sigma` says
/// which nappe of the second holds the ruling, and the sign of what is left once two of the three
/// coordinates are spent counts them: two, one where the cones touch along it, or none.
fn shared_apex(p: &Funnel, q: &Funnel) -> Result<Option<Vec<Piece>>, IntersectError> {
    let one = point(1.0)?;
    let cosine = |k: Interval| -> Result<Interval, IntersectError> {
        Ok(one.div(root(one.add(k.square()?)?)?)?)
    };
    let (c, d) = (cosine(p.slope)?, cosine(q.slope)?);
    let g = dot(p.along, q.along)?;
    let under = one.sub(g.square()?)?;
    let normal = cross(p.along, q.along)?;
    let mut out = Vec::new();
    for sigma in [true, false] {
        let second = d.mul(point(if sigma { 1.0 } else { -1.0 })?)?;
        let a = c.sub(second.mul(g)?)?.div(under)?;
        let b = second.sub(c.mul(g)?)?.div(under)?;
        let rest = one
            .sub(a.square()?)?
            .sub(b.square()?)?
            .sub(a.mul(b)?.mul(g)?.mul(point(2.0)?)?)?;
        let height = rest.div(dot(normal, normal)?)?;
        let exact = (|| Some(quadric::shared_apex(&p.exact?, &q.exact?, sigma)))();
        let Some(sign) = decided(exact, height) else {
            return Ok(None);
        };
        let flat = plus(scale(p.along, a)?, scale(q.along, b)?)?;
        match sign {
            Sign::Negative => {}
            Sign::Zero => out.push(line(p.apex, flat, true)?),
            Sign::Positive => {
                let up = scale(normal, root(height)?)?;
                out.push(line(p.apex, plus(flat, up)?, false)?);
                out.push(line(p.apex, minus(flat, up)?, false)?);
            }
        }
    }
    Ok(Some(out))
}

/// The closed form of a pair in the order plane, sphere, cylinder, cone; `None` for any other.
fn pieces(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    match (a, b) {
        (SurfaceKind::Plane(x), SurfaceKind::Plane(y)) => plane_plane(x, y),
        (SurfaceKind::Plane(x), SurfaceKind::Sphere(y)) => plane_sphere(x, y),
        (SurfaceKind::Plane(x), SurfaceKind::Cylinder(y)) => plane_cylinder(x, y),
        (SurfaceKind::Plane(x), SurfaceKind::Cone(y)) => plane_cone(x, y),
        (SurfaceKind::Sphere(x), SurfaceKind::Sphere(y)) => sphere_sphere(x, y),
        (SurfaceKind::Sphere(x), SurfaceKind::Cylinder(y)) => sphere_cylinder(x, y),
        (SurfaceKind::Sphere(x), SurfaceKind::Cone(y)) => sphere_cone(x, y),
        (SurfaceKind::Cylinder(x), SurfaceKind::Cylinder(y)) => cylinder_cylinder(x, y),
        (SurfaceKind::Cylinder(x), SurfaceKind::Cone(y)) => cylinder_cone(x, y),
        (SurfaceKind::Cone(x), SurfaceKind::Cone(y)) => cone_cone(x, y),
        (SurfaceKind::Plane(x), SurfaceKind::Torus(y)) => reduced::plane_torus(x, y, meter),
        (_, SurfaceKind::Torus(y)) => reduced::about_torus(a, y, meter),
        (SurfaceKind::Plane(x), SurfaceKind::Revolution(y)) => {
            reduced::plane_revolution(x, y, meter)
        }
        (_, SurfaceKind::Revolution(y)) => reduced::about_revolution(a, y, meter),
        (SurfaceKind::Plane(x), SurfaceKind::Extrusion(y)) => reduced::plane_extrusion(x, y, meter),
        (SurfaceKind::Cylinder(_) | SurfaceKind::Extrusion(_), SurfaceKind::Extrusion(_)) => {
            reduced::sweeps(a, b, meter)
        }
        _ => Ok(None),
    }
}

/// A kind's place in the order of [`pieces`].
const fn rank(s: &SurfaceKind) -> Option<u8> {
    match s {
        SurfaceKind::Plane(_) => Some(0),
        SurfaceKind::Sphere(_) => Some(1),
        SurfaceKind::Cylinder(_) => Some(2),
        SurfaceKind::Cone(_) => Some(3),
        SurfaceKind::Torus(_) => Some(4),
        SurfaceKind::Revolution(_) => Some(5),
        SurfaceKind::Extrusion(_) => Some(6),
        _ => None,
    }
}

/// The parameters of a surface's points in `at`, enclosed; a turn a box runs around is the domain's.
/// Only the angles `read` names are worked out: one it leaves is the box of its whole range, which
/// holds it, for a reader that needs nothing of it but that (an angle is a box's four corners'
/// correctly rounded angles, the dearest part of the reading). `None` for a kind with no parameters
/// here, or a torus's tube turn that meets its domain at no turn.
fn parameters(
    surface: &SurfaceKind,
    at: Vector,
    read: [bool; 2],
) -> Result<Option<[Interval; 2]>, IntersectError> {
    let (u, v_range) = ParamSurface::domain(surface);
    let [read_u, read_v] = read;
    let whole_turn = |x: Interval, y: Interval| -> Result<Interval, IntersectError> {
        if x.contains(0.0) && y.contains(0.0) {
            Ok(Interval::new(u.0, u.1)?)
        } else {
            let t = turn_of(x, y)?;
            let two = point(2.0)?;
            // Whole turns to the domain's start first: `%` is exact, so this is a multiple of two
            // half-turns within a turn of the start, and the angle lands within two periods of the
            // domain however many turns out it starts. A domain within a turn of zero needs none,
            // and its angle is not widened by adding it.
            let shift = u.0 - u.0 % 2.0;
            let mut moved = if shift == 0.0 {
                t
            } else {
                t.add(point(shift)?)?
            };
            for _ in 0..8 {
                if moved.hi() >= u.0 {
                    break;
                }
                moved = moved.add(two)?;
            }
            for _ in 0..8 {
                if moved.lo() <= u.1 {
                    break;
                }
                moved = moved.sub(two)?;
            }
            Ok(moved)
        }
    };
    let whole_turn = |x: Interval, y: Interval| -> Result<Interval, IntersectError> {
        if read_u {
            whole_turn(x, y)
        } else {
            Ok(Interval::new(u.0, u.1)?)
        }
    };
    let all_v = || -> Result<Interval, IntersectError> { Ok(Interval::new(v_range.0, v_range.1)?) };
    let local = |frame: &Frame<3>, center: [f64; 3]| -> Result<Vector, IntersectError> {
        let (_, [x, y, n]) = frame.unit_vectors()?;
        let w = minus(at, lift(center)?)?;
        Ok([dot(w, x)?, dot(w, y)?, dot(w, n)?])
    };
    Ok(Some(match surface {
        SurfaceKind::Plane(p) => {
            let (c, a, b) = p.vectors();
            let (a, b) = (lift(a)?, lift(b)?);
            let w = minus(at, lift(c)?)?;
            let (aa, ab, bb) = (dot(a, a)?, dot(a, b)?, dot(b, b)?);
            let (wa, wb) = (dot(w, a)?, dot(w, b)?);
            let det = aa.mul(bb)?.sub(ab.square()?)?;
            [
                wa.mul(bb)?.sub(wb.mul(ab)?)?.div(det)?,
                wb.mul(aa)?.sub(wa.mul(ab)?)?.div(det)?,
            ]
        }
        // A cylinder's or cone's point at v stands s·v along n̂, so the height z is v = z/s: the
        // axial factor s is ±1 as built, where z/s is z·s, but a similarity scales it.
        SurfaceKind::Cylinder(c) => {
            let [x, y, z] = local(&c.frame(), c.center())?;
            [whole_turn(x, y)?, z.div(point(c.axial_sign())?)?]
        }
        SurfaceKind::Cone(c) => {
            let [x, y, z] = local(&c.frame(), c.center())?;
            let v = z.div(point(c.axial_sign())?)?;
            let (r0, k) = c.radius_and_slope();
            let rho = point(r0)?.add(point(k)?.mul(v)?)?;
            let flip = if rho.hi() < 0.0 { -1.0 } else { 1.0 };
            let flip = point(flip)?;
            [whole_turn(x.mul(flip)?, y.mul(flip)?)?, v]
        }
        SurfaceKind::Sphere(s) => {
            let [x, y, z] = local(&s.frame(), s.center())?;
            let z = z.mul(point(s.axial_sign())?)?;
            let latitude = if read_v {
                Interval::atan2(z, tight::norm(&[x, y])?)?.div(Interval::PI)?
            } else {
                all_v()?
            };
            [whole_turn(x, y)?, latitude]
        }
        SurfaceKind::Torus(t) => {
            let [x, y, z] = local(&t.frame(), t.center())?;
            let v = if read_v {
                let Some(v) = tube_turn(t, [x, y, z], v_range)? else {
                    return Ok(None);
                };
                v
            } else {
                all_v()?
            };
            [whole_turn(x, y)?, v]
        }
        // Each piece of these is built on them: a box of the domain is true, and all they need.
        SurfaceKind::Revolution(_) | SurfaceKind::Extrusion(_) => {
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(surface);
            [Interval::new(u0, u1)?, Interval::new(v0, v1)?]
        }
        _ => return Ok(None),
    }))
}

/// A torus's tube turn at the point `local` in its frame, on the tube's domain: `turn_of` reads it
/// from −1/2 to 3/2 (a point inside the tube's centre circle from 1/2 to 3/2, as its opposite's turn
/// and a half-turn on), and a box across that circle down to −1, and [`to_domain`] moves it there.
fn tube_turn(
    t: &Torus,
    local: Vector,
    domain: (f64, f64),
) -> Result<Option<Interval>, IntersectError> {
    let [x, y, z] = local;
    let (major, _) = t.radii();
    let out = tight::norm(&[x, y])?.sub(point(major)?)?;
    to_domain(turn_of(out, z.mul(point(t.axial_sign())?)?)?, domain)
}

/// `turn`, as [`turn_of`] reads it (from −1 to 3/2), moved by whole turns to the domain `(lo, hi)`
/// of at most a turn, which may start any number of turns out: the box of every turn of it that
/// meets the domain, or `None` where none does. The domain is decided here alone: a box handed on
/// off it could be read by its caller as meeting it at an end.
///
/// First the turns to the domain's window: `m − m % 2` is a multiple of two, exactly, for every
/// double m (`%` is exact), and for m the sum lo + 1/2, however it rounds, it leaves the start
/// strictly between −5/2 and 3/2 past it. From there a turn back, none, one or two on reach every
/// turn of the box that can meet the domain: two back each of its turns is under 3/2 − 4, before
/// the start (its box, rounded and widened by the move, may end on the start, and is not tried),
/// and three on it starts from −1 + 6, over the end, which is under 3/2 + 2.
///
/// A box wider than the gap from the domain's end to its start a turn on can meet it at two turns
/// (a box across a torus's centre circle, or across an arc's gap), one at each end, and the point
/// may stand at either: the answer is the hull of both, which holds each. Both callers carry it.
/// [`tube_turn`]'s read a box as an enclosure ([`on`] holds it to the domain, all of which it then
/// covers), so the span and the standing read from it are true. [`cut_at`] needs one parameter,
/// and takes the hull's middle, half a turn from both meetings: on an arc of radius over 2 the
/// point there is farther from the mark than [`STEP`], so the mark is no cut, and on a smaller one
/// a cut where the curve does not cross only splits a stretch, which [`runs`] joins again. Each
/// meeting lies within the box's width of an end, where a cut cuts nothing while that width is
/// under the room [`trimmed`] leaves at an end (10^−9 of a half-turn and more).
///
/// On a whole turn the end is the start's join, so a box that starts on the end meets the domain
/// only at its start, where the box a turn back, always among the four, meets it too: it is no
/// second meeting, and a box from 0 at the start of (0, 2) is kept, not widened to the whole turn.
///
/// A far domain costs one widening, as one move did before: the turns are summed exactly, in a
/// double (even numbers under 2^53, as a domain of a turn or less starts only where its doubles
/// stand under two apart), and added to the box once. A domain that starts from about −5/2 to 3/2
/// has no turns to its window and is not widened by adding none, so its box is the one of before:
/// the boundary tests' domains start there, and their boxes are the ones they were.
fn to_domain(turn: Interval, (lo, hi): (f64, f64)) -> Result<Option<Interval>, IntersectError> {
    let by = |turns: f64| -> Result<Interval, IntersectError> {
        if turns == 0.0 {
            Ok(turn)
        } else {
            Ok(turn.add(point(turns)?)?)
        }
    };
    let above = lo + 0.5;
    let window = above - above % 2.0;
    let joined = !part_turn(lo, hi);
    let mut met: Option<Interval> = None;
    for turns in [window - 2.0, window, window + 2.0, window + 4.0] {
        let moved = by(turns)?;
        if moved.hi() < lo || moved.lo() > hi || (joined && moved.lo() == hi) {
            continue;
        }
        met = Some(match met {
            Some(first) => Interval::new(first.lo(), moved.hi())?,
            None => moved,
        });
    }
    Ok(met)
}

/// `x`, held to `domain` when it meets it.
fn on(x: Interval, domain: (f64, f64)) -> Result<Option<Interval>, IntersectError> {
    if x.hi() < domain.0 || x.lo() > domain.1 {
        return Ok(None);
    }
    held(x, domain).map(Some)
}

/// The box a piece stands in on one surface, inside its domain; `None` if it lies off it.
fn on_surface(surface: &SurfaceKind, at: Vector) -> Result<Option<[Interval; 2]>, IntersectError> {
    let Some([u, v]) = parameters(surface, at, [true; 2])? else {
        return Ok(None);
    };
    let (du, dv) = ParamSurface::domain(surface);
    let (Some(u), Some(v)) = (on(u, du)?, on(v, dv)?) else {
        return Ok(None);
    };
    Ok(Some([u, v]))
}

/// Where a point's box stands on one surface's domain.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Inside the domain: each parameter strictly inside its range, or reaching only an end that is
    /// a join.
    Inside,
    /// Off the domain: a parameter wholly past an end, or a kind with no parameters here.
    Off,
    /// Reaching an end that is an edge of the domain, inside it or not: the box cannot tell.
    Reaching,
}

/// Whether a turn from `lo` to `hi` stops short of a whole one, so that its ends are edges of the
/// domain, not its join: [`edges`] and [`rims`] read it alike.
fn part_turn(lo: f64, hi: f64) -> bool {
    hi - lo < 2.0
}

/// Whether a sphere's latitude `v` stops short of a pole, so that a domain ending there has an edge
/// circle: [`edges`] and [`rims`] read it alike.
fn short_of_pole(v: f64) -> bool {
    v.abs() < 0.5
}

/// Which ends of each parameter's range are edges of the domain, as [`edges`] and [`apex_ends`]
/// list them: every end of a plane's; a round kind's turn short of a whole one; a cylinder's or
/// cone's height either way (an end circle, or a cone's apex); a sphere's latitude short of a
/// pole; a torus's tube turn short of a whole one. A whole turn's ends are its join and a pole is a
/// point of the sphere, so neither is an edge. A revolution's or extrusion's box is its whole
/// domain by construction ([`parameters`]), so it names none.
fn rims(surface: &SurfaceKind) -> [[bool; 2]; 2] {
    let ((u0, u1), (v0, v1)) = ParamSurface::domain(surface);
    let turn = [part_turn(u0, u1); 2];
    match surface {
        SurfaceKind::Plane(_) => [[true; 2]; 2],
        SurfaceKind::Sphere(_) => [turn, [short_of_pole(v0), short_of_pole(v1)]],
        SurfaceKind::Torus(_) => [turn, [part_turn(v0, v1); 2]],
        SurfaceKind::Revolution(_) | SurfaceKind::Extrusion(_) => [[false; 2]; 2],
        _ => [turn, [true; 2]],
    }
}

/// Where the point in `at` stands on a plane's domain, each end of it an edge. Each parameter is
/// read from the corner of the domain at its ends, as the point's offset from that corner: a
/// plane's origin may stand far from its points, and its parameters with it, so far that a double
/// holds them no closer than the whole domain (a domain of one unit 2^52 out, where the spacing is
/// 1), and then the box of the parameter itself reaches both ends; the offset is as close as the
/// point. The corner c + p·a + q·b is a sum of products of doubles, held to a step of its exact
/// value (an enclosure of each product would be a step of the product wide, a whole unit there).
/// An end without end is never reached, and its corner is read at 0.
fn on_plane(plane: &Plane, at: Vector) -> Result<Standing, IntersectError> {
    let (c, a, b) = plane.vectors();
    let ([c0, c1, c2], [a0, a1, a2], [b0, b1, b2]) = (c, a, b);
    let (a, b) = (lift(a)?, lift(b)?);
    let (aa, ab, bb) = (dot(a, a)?, dot(a, b)?, dot(b, b)?);
    let det = aa.mul(bb)?.sub(ab.square()?)?;
    let ((u0, u1), (v0, v1)) = ParamSurface::domain(plane);
    let mut out = Standing::Inside;
    for (upper, ends) in [(false, [u0, v0]), (true, [u1, v1])] {
        let [p, q] = ends.map(|e| if e.is_finite() { e } else { 0.0 });
        let sum = |x: f64, y: f64, z: f64| enclosed_sum(&[(x, 1.0), (p, y), (q, z)]);
        let corner = [sum(c0, a0, b0)?, sum(c1, a1, b1)?, sum(c2, a2, b2)?];
        let w = minus(at, corner)?;
        let (wa, wb) = (dot(w, a)?, dot(w, b)?);
        let offsets = [
            wa.mul(bb)?.sub(wb.mul(ab)?)?.div(det)?,
            wb.mul(aa)?.sub(wa.mul(ab)?)?.div(det)?,
        ];
        for (d, end) in offsets.iter().zip(ends) {
            if !end.is_finite() {
                continue;
            }
            // An offset whose box holds zero may stand on the end's edge, so it reaches it, an
            // exact zero at either bound included. One that does not lies wholly to one side,
            // whose sign either bound tells: past the end beyond the upper corner or before the
            // lower one, else inside.
            if d.contains(0.0) {
                out = Standing::Reaching;
            } else if d.lo().is_sign_positive() == upper {
                return Ok(Standing::Off);
            }
        }
    }
    Ok(out)
}

/// Where the point in `at` stands on `surface`'s domain, by the box of its parameters. A plane's
/// box that reaches an end is read again from the domain's corners ([`on_plane`]), whose offsets
/// can be closer than the box; its other answers stand as they are, since both read the one
/// parameter and each holds it, so a box inside the range or past an end is proven so either way.
/// The corners cost six exact sums, and most pieces are far from every end of a plane's domain, so
/// they are read only where the plain box cannot tell (decision 32.94). A parameter with no edge at
/// either end (a whole turn, a latitude from pole to pole) is read as its whole range: its box,
/// wrapped onto the domain or inside it by construction, can neither pass an end nor reach an edge.
fn standing(surface: &SurfaceKind, at: Vector) -> Result<Standing, IntersectError> {
    let [ru, rv] = rims(surface);
    let Some([u, v]) = parameters(surface, at, [ru.contains(&true), rv.contains(&true)])? else {
        return Ok(Standing::Off);
    };
    let (du, dv) = ParamSurface::domain(surface);
    let out = both(placed(u, du, ru), placed(v, dv, rv));
    if out == Standing::Reaching
        && let SurfaceKind::Plane(p) = surface
    {
        return on_plane(p, at);
    }
    Ok(out)
}

/// Where one parameter's box `x` stands on its range, whose ends `rim` names edges or not
/// ([`rims`]).
fn placed(x: Interval, (lo, hi): (f64, f64), [at_lo, at_hi]: [bool; 2]) -> Standing {
    if x.hi() < lo || x.lo() > hi {
        Standing::Off
    } else if (at_lo && x.lo() <= lo) || (at_hi && x.hi() >= hi) {
        Standing::Reaching
    } else {
        Standing::Inside
    }
}

/// Two standings together: off if either is, inside if both are, else reaching an edge.
fn both(p: Standing, q: Standing) -> Standing {
    if p == Standing::Off || q == Standing::Off {
        Standing::Off
    } else if p == Standing::Reaching || q == Standing::Reaching {
        Standing::Reaching
    } else {
        Standing::Inside
    }
}

/// Where the point in `at`, a point of an end circle of a cylinder's height ([`along_edge`]),
/// stands on the cylinder's domain: its height is that end's exactly, on the domain's edge and so
/// on the domain, and only its turn is read, as [`standing`] reads it.
fn on_turn(surface: &SurfaceKind, at: Vector) -> Result<Standing, IntersectError> {
    let [ru, _] = rims(surface);
    let Some([u, _]) = parameters(surface, at, [ru.contains(&true), false])? else {
        return Ok(Standing::Off);
    };
    let (du, _) = ParamSurface::domain(surface);
    Ok(placed(u, du, ru))
}

/// The edges of a surface's domain that are not joins.
fn edges(surface: &SurfaceKind) -> Result<Vec<CurveKind<3>>, IntersectError> {
    let mut out = Vec::new();
    match surface {
        SurfaceKind::Plane(p) => {
            // Each finite side of the domain is an edge, along the other vector over the other
            // parameter's range: a segment, a ray, or a whole line where the plane runs on.
            let (c, a, b) = p.vectors();
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(p);
            let shifted = |t: f64, w: [f64; 3]| {
                let [x, y, z] = c;
                let [e, f, g] = w;
                [x + t * e, y + t * f, z + t * g]
            };
            for u in [u0, u1] {
                if u.is_finite() {
                    out.push(CurveKind::Line(Line::new(shifted(u, a), b, (v0, v1))?));
                }
            }
            for v in [v0, v1] {
                if v.is_finite() {
                    out.push(CurveKind::Line(Line::new(shifted(v, b), a, (u0, u1))?));
                }
            }
        }
        SurfaceKind::Cylinder(c) => {
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(c);
            let on = axis(c.center(), &c.frame(), c.axial_sign())?;
            // Its two end circles, and its two rulings on a part of a turn, reserved exactly: grown
            // from empty, the vector would hold four.
            out.reserve_exact(if part_turn(u0, u1) { 4 } else { 2 });
            for v in [v0, v1] {
                let at = plus(on.at, scale(on.along, point(v)?)?)?;
                let curve = Circle::new(middle(at), c.radius(), c.frame(), (u0, u1))?;
                out.push(CurveKind::Circle(curve));
            }
            if part_turn(u0, u1) {
                rulings(surface, (u0, u1), (v0, v1), &mut out)?;
            }
        }
        SurfaceKind::Cone(c) => {
            let (r0, k) = c.radius_and_slope();
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(c);
            let on = axis(c.center(), &c.frame(), c.axial_sign())?;
            for v in [v0, v1] {
                let signed = r0 + k * v;
                let radius = signed.abs();
                if radius > 0.0 {
                    // Past the apex the radius is negative: the point at u is half a turn round.
                    let turn = if signed < 0.0 { 1.0 } else { 0.0 };
                    let at = plus(on.at, scale(on.along, point(v)?)?)?;
                    let curve = Circle::new(middle(at), radius, c.frame(), (u0 + turn, u1 + turn))?;
                    out.push(CurveKind::Circle(curve));
                }
            }
            if part_turn(u0, u1) {
                rulings(surface, (u0, u1), (v0, v1), &mut out)?;
            }
        }
        SurfaceKind::Sphere(s) => {
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(s);
            let on = axis(s.center(), &s.frame(), s.axial_sign())?;
            let r = point(s.radius())?;
            for v in [v0, v1] {
                if short_of_pole(v) {
                    let turn = Interval::point(v)?;
                    let at = plus(on.at, scale(on.along, r.mul(turn.sinpi()?)?)?)?;
                    let radius = tight::middle(r.mul(turn.cospi()?)?);
                    let curve = Circle::new(middle(at), radius, s.frame(), (u0, u1))?;
                    out.push(CurveKind::Circle(curve));
                }
            }
            if part_turn(u0, u1) {
                let (_, [x, y, n]) = s.frame().unit_vectors()?;
                for u in [u0, u1] {
                    let turn = Interval::point(u)?;
                    let e = plus(scale(x, turn.cospi()?)?, scale(y, turn.sinpi()?)?)?;
                    let up = scale(n, point(s.axial_sign())?)?;
                    let frame = Frame::new(middle(e), middle(cross(e, up)?))?;
                    let curve = Circle::new(s.center(), s.radius(), frame, (v0, v1))?;
                    out.push(CurveKind::Circle(curve));
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

/// The two straight edges of a round kind's domain that does not close.
fn rulings(
    surface: &SurfaceKind,
    (u0, u1): (f64, f64),
    (v0, v1): (f64, f64),
    out: &mut Vec<CurveKind<3>>,
) -> Result<(), IntersectError> {
    let sides = (Limit::FromRight, Limit::FromRight);
    for u in [u0, u1] {
        let (a, b) = (surface.point(u, v0, sides)?, surface.point(u, v1, sides)?);
        out.push(CurveKind::Line(Line::segment(a, b)?.value));
    }
    Ok(())
}

/// Where along `curve` the point in `at` lies, enclosed.
fn along_curve(curve: &CurveKind<3>, at: Vector) -> Result<Option<Interval>, IntersectError> {
    let solved = |c: [f64; 3], u: [f64; 3], v: [f64; 3]| -> Result<[Interval; 2], IntersectError> {
        let (u, v) = (lift(u)?, lift(v)?);
        let w = minus(at, lift(c)?)?;
        let (aa, ab, bb) = (dot(u, u)?, dot(u, v)?, dot(v, v)?);
        let (wa, wb) = (dot(w, u)?, dot(w, v)?);
        let det = aa.mul(bb)?.sub(ab.square()?)?;
        Ok([
            wa.mul(bb)?.sub(wb.mul(ab)?)?.div(det)?,
            wb.mul(aa)?.sub(wa.mul(ab)?)?.div(det)?,
        ])
    };
    Ok(Some(match curve {
        CurveKind::Line(l) => {
            let d = lift(l.direction())?;
            dot(minus(at, lift(l.origin())?)?, d)?.div(dot(d, d)?)?
        }
        CurveKind::Circle(c) => {
            let (_, [x, y, _]) = c.frame().unit_vectors()?;
            let w = minus(at, lift(c.center())?)?;
            turn_of(dot(w, x)?, dot(w, y)?)?
        }
        CurveKind::Ellipse(e) => {
            let Axes::Vectors { u, v } = e.axes() else {
                return Ok(None);
            };
            let [a, b] = solved(e.center(), u, v)?;
            turn_of(a, b)?
        }
        CurveKind::Hyperbola(h) => {
            let Axes::Vectors { u, v } = h.axes() else {
                return Ok(None);
            };
            let [a, b] = solved(h.center(), u, v)?;
            // cosh t + sinh t is e^t and cosh t - sinh t is e^-t, both positive; on the other
            // branch neither is, and this is no point of the curve. Each is read where it is the
            // larger, by the sign of sinh t: far down the branch the other subtracts two near-equal
            // numbers, and a box a few hundredths wide (a cut found on an edge) left it straddling
            // zero, so the cut on the branch's far end was lost and the piece ran on past both
            // domains.
            let ahead = b.lo() + b.hi() >= 0.0;
            let term = if ahead { a.add(b)? } else { a.sub(b)? };
            if term.lo() <= 0.0 {
                return Ok(None);
            }
            if ahead { term.ln()? } else { term.ln()?.neg() }
        }
        CurveKind::Parabola(p) => {
            let Axes::Vectors { u, v } = p.axes() else {
                return Ok(None);
            };
            let [_, b] = solved(p.vertex(), u, v)?;
            b
        }
        _ => return Ok(None),
    }))
}

/// Whether a kind's domain is a closed loop: a circle or an ellipse over a whole turn. An arc, as a
/// plane across an extrusion cuts from a profile that is one, is open.
fn closed(curve: &CurveKind<3>) -> bool {
    let (lo, hi) = curve.domain();
    matches!(curve, CurveKind::Circle(_) | CurveKind::Ellipse(_)) && hi - lo >= 2.0
}

/// `curve`, and a loop on its domain moved by whole turns to within two turns of zero: the same
/// points at the same angles. A domain may start any number of turns out, and there a double holds
/// an angle no better than its spacing, which far enough out is wider than a step, while
/// [`cut_at`]'s count of periods and the room [`trimmed`] leaves between cuts are in half-turns
/// near zero: a loop 2^20 turns out lost every cut, and one 2^29 turns out had room of a
/// half-turn.
///
/// The move is exact, so the moved domain is as long as the loop's own to the last bit. `lo % 4` is
/// exact, so the shift `lo − lo % 4` is a multiple of four half-turns that leaves `lo % 4`, between
/// −4 and 4. Under four half-turns out the shift is 0. Further out, `hi` less the shift is a
/// multiple of `hi`'s own spacing (the shift is a multiple of four, and of that spacing), and lies
/// between −2 and 6: under 4 in size where `hi` is at least 2 in size and its spacing at least
/// 2^−51, and from 4 to 6 only where `hi` is past 4 and its spacing at least 2^−50. A double holds
/// every such multiple.
fn nearer(curve: CurveKind<3>) -> Result<CurveKind<3>, IntersectError> {
    if !closed(&curve) {
        return Ok(curve);
    }
    let (lo, hi) = curve.domain();
    let shift = lo - lo % 4.0;
    Ok(stretch(&curve, lo - shift, hi - shift)?.unwrap_or(curve))
}

/// `curve` over `(lo, hi)`.
pub(crate) fn stretch(
    curve: &CurveKind<3>,
    lo: f64,
    hi: f64,
) -> Result<Option<CurveKind<3>>, IntersectError> {
    Ok(match curve {
        CurveKind::Line(l) => Some(CurveKind::Line(Line::new(
            l.origin(),
            l.direction(),
            (lo, hi),
        )?)),
        CurveKind::Circle(c) => Some(CurveKind::Circle(Circle::new(
            c.center(),
            c.radius(),
            c.frame(),
            (lo, hi),
        )?)),
        CurveKind::Ellipse(e) => Some(CurveKind::Ellipse(Ellipse::new(
            e.center(),
            e.axes(),
            (lo, hi),
        )?)),
        CurveKind::Hyperbola(h) => Some(CurveKind::Hyperbola(Hyperbola::new(
            h.center(),
            h.axes(),
            (lo, hi),
        )?)),
        CurveKind::Parabola(p) => Some(CurveKind::Parabola(Parabola::new(
            p.vertex(),
            p.axes(),
            (lo, hi),
        )?)),
        CurveKind::Nurbs(n) => Some(CurveKind::Trimmed(Trimmed::new(
            Basis::Nurbs(n.clone()),
            (lo, hi),
        )?)),
        CurveKind::Trimmed(t) => Some(CurveKind::Trimmed(Trimmed::new(
            t.basis().clone(),
            (lo, hi),
        )?)),
        _ => None,
    })
}

/// The point of `curve` at `t`, enclosed.
fn at_parameter(
    curve: &CurveKind<3>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Vector, IntersectError> {
    curve
        .enclose_point(t, Limit::FromRight, meter)?
        .ok_or(IntersectError::Internal)
}

/// Where the point at `t` stands on both domains ([`standing`]): off them if off either, inside
/// them if inside both, else reaching an edge. The box read is the meeting's, the stored curve's
/// point grown by `reach`, how far the curve may stand from the meeting (32.94). On a curve that is
/// an end circle of one of them (`along`), that one reads only the turn ([`on_turn`]).
fn on_both(
    curve: &CurveKind<3>,
    t: f64,
    reach: f64,
    (a, b): (&SurfaceKind, &SurfaceKind),
    along: Option<Along>,
    meter: &mut WorkMeter<'_>,
) -> Result<Standing, IntersectError> {
    let room = Interval::new(-reach, reach)?;
    let at = plus(at_parameter(curve, t, meter)?, [room; 3])?;
    let place = |surface: &SurfaceKind, second: bool| match along {
        Some(Along { second: edge }) if edge == second => on_turn(surface, at),
        _ => standing(surface, at),
    };
    Ok(both(place(a, false)?, place(b, true)?))
}

/// That a curve is an end circle of a cylinder's domain ([`along_edge`]): of the pair's second
/// surface if `second`, else of its first.
#[derive(Clone, Copy)]
struct Along {
    second: bool,
}

/// The end circle of a cylinder's height that the circle `curve` lies along, where that end circle
/// lies exactly on the other surface: the end circle itself, on `curve`'s frame and domain, whose
/// it is, and whether `curve` is that circle to the bit, its centre's and radius's doubles the end
/// circle's (decision 32.94). `None` for any other piece, which stays the engine's. It reads
/// exact signs and the stored doubles alone, and no enclosure of the pair's, so it costs the
/// meter nothing.
///
/// Theorem. The end circle at v of a cylinder of radius ρ is the circle of radius ρ about the point
/// at v on its axis, across the axis ([`end_centre`]). Where a double holds that point, the end
/// circle is the circle of radius ρ about it across the stored axis, which [`circle_lies_on`]
/// places on the other surface, or not, by exact signs. A circle on `curve`'s frame, whose normal
/// is exactly parallel to the axis, about the same centre with the same radius, is that circle
/// point for point. Where it lies on the other surface it lies on both whole surfaces, so it is a
/// curve of their meeting, and the closed form's circles are that meeting, each within its reach
/// of its own: `curve`, whose centre stands within its reach of the end circle's, is that end
/// circle's. Each of its points is on the cylinder's domain at its height exactly, so only the
/// turn is left to read there, and the other domain is read as before. An end circle whose point
/// no double holds, or that misses the other surface by any rounding, is left alone: the piece
/// stays the engine's.
fn along_edge(
    curve: &CurveKind<3>,
    reach: f64,
    (a, b): (&SurfaceKind, &SurfaceKind),
) -> Result<Option<(CurveKind<3>, Along, bool)>, IntersectError> {
    let CurveKind::Circle(found) = curve else {
        return Ok(None);
    };
    for (second, surface, other) in [(false, a, b), (true, b, a)] {
        let SurfaceKind::Cylinder(c) = surface else {
            continue;
        };
        let (n, rho) = (c.frame().normal(), c.radius());
        if !parallel(found.frame().normal(), n) {
            continue;
        }
        let (_, (v0, v1)) = ParamSurface::domain(c);
        for v in [v0, v1] {
            let Some(centre) = end_centre(c, v) else {
                continue;
            };
            let mut near = true;
            for (e, f) in centre.into_iter().zip(found.center()) {
                near = near && within(e, f, reach)?;
            }
            if near && circle_lies_on(centre, rho, n, other) {
                let edge = Circle::new(centre, rho, found.frame(), found.domain())?;
                let itself = found.center().map(f64::to_bits) == centre.map(f64::to_bits)
                    && found.radius().to_bits() == rho.to_bits();
                return Ok(Some((CurveKind::Circle(edge), Along { second }, itself)));
            }
        }
    }
    Ok(None)
}

/// The centre of a cylinder's end circle at `v`, where a double holds it exactly. The point at v
/// on the axis stands s·v along the unit vector of the stored axis from the centre (s the axial
/// factor, [`parameters`]); on an axis along a coordinate axis that unit vector is the axis,
/// signed, so the point is the centre with one coordinate moved by ±s·v, a product and a sum of
/// doubles, each exact where its rounding error is zero. `None` on any other axis, whose unit
/// vector is no vector of doubles, or where the product or the sum rounds.
fn end_centre(c: &Cylinder, v: f64) -> Option<[f64; 3]> {
    let n = c.frame().normal();
    if n.iter().filter(|x| **x != 0.0).count() != 1 {
        return None;
    }
    let (rise, error) = geom_num::eft::two_prod(c.axial_sign(), v);
    let mut exact = error == 0.0;
    let mut centre = c.center();
    for (p, d) in centre.iter_mut().zip(n) {
        if d != 0.0 {
            let step = if d.is_sign_positive() { rise } else { -rise };
            let (sum, error) = geom_num::eft::two_sum(*p, step);
            exact = exact && error == 0.0;
            *p = sum;
        }
    }
    exact.then_some(centre)
}

/// Whether `x` stands within `reach` of `y`, enclosed outward.
fn within(x: f64, y: f64, reach: f64) -> Result<bool, IntersectError> {
    Ok(point(y)?.add(Interval::new(-reach, reach)?)?.contains(x))
}

/// Where `curve` passes through `at`, a point of an edge: its parameter there, or `None` if `at`
/// lies off it.
fn cut_at(
    curve: &CurveKind<3>,
    at: Vector,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<f64>, IntersectError> {
    // A point of another piece of this pair projects onto this curve too: only one that lies on
    // the curve cuts it.
    let Some(s) = along_curve(curve, at)? else {
        return Ok(None);
    };
    let (lo, hi) = crate::curve::finite(curve)?;
    // An arc's turn is read as a loop's is, from −1 to 3/2, and its domain may start any number of
    // turns from there: the box is moved by whole turns to it, as a tube's turn is. A box across
    // the arc's gap meets it at both ends, and the hull of the two is no cut ([`to_domain`]).
    let arc = matches!(curve, CurveKind::Circle(_) | CurveKind::Ellipse(_)) && !closed(curve);
    let s = if arc {
        let Some(s) = to_domain(s, (lo, hi))? else {
            return Ok(None);
        };
        s
    } else {
        s
    };
    let mut t = tight::middle(s);
    // On a loop the turns wrap. The count, not t, ends the loop, and [`WRAPS`] periods bring the
    // angle into every loop's domain, which [`nearer`] has moved to within two turns of zero. Both
    // tests ask whether t has reached an end, so the loop's own start is on the loop and is never
    // moved.
    if closed(curve) {
        let period = hi - lo;
        for _ in 0..WRAPS {
            if t >= hi {
                t -= period;
            } else if t >= lo {
                break;
            } else {
                t += period;
            }
        }
    }
    if t < lo || t > hi {
        return Ok(None);
    }
    let gap = minus(at_parameter(curve, t, meter)?, at)?;
    Ok(gap
        .iter()
        .all(|c| c.lo() <= STEP && c.hi() >= -STEP)
        .then_some(t))
}

/// The apex of a cone whose domain ends there: an edge of one point, where the edge circles of
/// [`edges`] have no radius.
fn apex_ends(surface: &SurfaceKind) -> Result<Vec<Vector>, IntersectError> {
    let SurfaceKind::Cone(c) = surface else {
        return Ok(Vec::new());
    };
    let (r0, k) = c.radius_and_slope();
    let (_, (v0, v1)) = ParamSurface::domain(c);
    let ends = [v0, v1].iter().filter(|v| r0 + k * **v == 0.0).count();
    Ok(if ends > 0 {
        alloc::vec![funnel(c)?.apex]
    } else {
        Vec::new()
    })
}

/// What can cut a piece of the pair: the box of each edge of either surface where it meets the
/// other, and each apex a domain ends at (a curve through one leaves the domain there). They hang
/// on the two surfaces alone, so a call finds them once, whatever the count of its pieces: before,
/// every piece ran every edge against the other surface again.
fn marks(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vector>, IntersectError> {
    let mut out = Vec::new();
    for (surface, other) in [(a, b), (b, a)] {
        for edge in edges(surface)? {
            let found = curve_surface(&edge, other, meter)?;
            for item in found.items() {
                let [t, ..] = item.at().spans() else {
                    continue;
                };
                out.push(sides(&edge.bound((t.lo(), t.hi()), meter)?)?);
            }
        }
        out.extend(apex_ends(surface)?);
    }
    Ok(out)
}

/// Where the curve's stretches on both domains end: the parameters at which a mark of the pair
/// (an edge of either surface meeting the other, or an apex) lies on the curve, sorted.
fn cuts(
    curve: &CurveKind<3>,
    marks: &[Vector],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<f64>, IntersectError> {
    let mut out = Vec::new();
    for at in marks {
        if let Some(t) = cut_at(curve, *at, meter)? {
            out.push(t);
        }
    }
    out.sort_unstable_by(f64::total_cmp);
    Ok(out)
}

/// What trimming keeps of a piece.
enum Part {
    /// A stretch of it on both domains, and how it ends.
    Curve(Box<CurveKind<3>>, [CurveEnd; 2]),
    /// A point for a run of it on both domains no wider than [`STEP`]: a meeting the domains' ends
    /// make that small is a point at this resolution, never nothing. `width` is how far the run
    /// reaches from it.
    Point { at: Vector, width: f64 },
}

/// The stretches of `curve` on both domains, and how each ends; a run of them on both too short to
/// be a stretch, a point. `None` where the box of the meeting's middle, of the curve if no edge cuts
/// it or of a stretch between cuts, reaches an edge of a domain, off neither (decision 32.94): one
/// box cannot say whether that piece lies on both, and the pair is the engine's. `reach` is how far
/// the stored curve may stand from the meeting it stands for, the bound its item certifies. A curve
/// that is an end circle of one domain (`along`) reads only its turn on that one ([`on_both`]).
fn trimmed(
    curve: &CurveKind<3>,
    (a, b): (&SurfaceKind, &SurfaceKind),
    marks: &[Vector],
    along: Option<Along>,
    reach: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Part>>, IntersectError> {
    let (lo, hi) = crate::curve::finite(curve)?;
    let cut = cuts(curve, marks, meter)?;
    let loops = closed(curve);
    if cut.is_empty() {
        // No edge cuts the curve, so it lies on both domains all along or nowhere, and its middle
        // tells which only where its box is inside both, or off one. The box is the meeting's: the
        // stored curve's middle grown by its reach. An edge the exact signs place past the meeting
        // can still cross the stored curve, a rounding off it (issue 1018 with 32.87's exact line
        // against a cone), and the stored middle's own box would call it inside.
        let Some(kept) = stretch(curve, lo, hi)? else {
            return Ok(Some(Vec::new()));
        };
        match on_both(curve, lo * 0.5 + hi * 0.5, reach, (a, b), along, meter)? {
            Standing::Off => return Ok(Some(Vec::new())),
            Standing::Reaching => return Ok(None),
            Standing::Inside => {}
        }
        let end = if loops {
            CurveEnd::Closed
        } else {
            CurveEnd::Boundary
        };
        return Ok(Some(alloc::vec![Part::Curve(Box::new(kept), [end; 2])]));
    }
    // A loop is written from its first cut on.
    let period = hi - lo;
    let mut ends: Vec<f64> = Vec::new();
    let (mut start, stop) = if loops {
        let base = *cut.first().ok_or(IntersectError::Internal)?;
        // A whole turn stays one, however the sum rounds: its length is decided exactly, as a loop's
        // kind checks it, and the sum is off by at most a step, so a step or two back is enough.
        let mut stop = base + period;
        for _ in 0..4 {
            let (length, error) = geom_num::eft::two_sum(stop, -base);
            if length < period || (length == period && error <= 0.0) {
                break;
            }
            stop = stop.next_down();
        }
        (base, stop)
    } else {
        (lo, hi)
    };
    // Two edges can meet at one point of the curve, and a cut on an end cuts nothing.
    for t in &cut {
        let room = 1e-9 * (1.0 + t.abs());
        let last = ends.last().copied().unwrap_or(start);
        if *t > last + room && *t < stop - room {
            ends.push(*t);
        }
    }
    ends.push(stop);
    // Each stretch between cuts, and whether it lies on both domains. No edge cuts it between its
    // ends, so it is decided as a curve no edge cuts is, by its middle (32.94).
    let mut stretches = Vec::new();
    for end in ends {
        let part = stretch(curve, start, end)?;
        let middle_of = start * 0.5 + end * 0.5;
        let on_loop = if loops && middle_of > hi {
            middle_of - period
        } else {
            middle_of
        };
        let on = part.is_some()
            && match on_both(curve, on_loop, reach, (a, b), along, meter)? {
                Standing::Inside => true,
                Standing::Off => false,
                Standing::Reaching => return Ok(None),
            };
        stretches.push(Stretch {
            from: start,
            to: end,
            on,
        });
        start = end;
    }
    let mut out = Vec::new();
    for run in runs(&stretches, loops) {
        out.extend(kept(curve, &stretches, &run, (lo, hi, loops), meter)?);
    }
    Ok(Some(out))
}

/// What trimming keeps of the piece `curve` ([`trimmed`]), or, where no box decides it, of the end
/// circle it lies along ([`along_edge`]), decided on that cylinder's turn: a piece no box decides
/// may lie along an end circle that exact signs place on the other surface, and then it is that
/// circle (32.94). `None` where neither decides, and the pair is the engine's.
///
/// Theorem. Where `curve` is the end circle to the bit, trimming it on its turn alone keeps what
/// trimming it on both whole domains and then, where that stops, on its turn keeps, at the work of
/// the second alone. Every point of that circle is at the end's height exactly, which the box of
/// its height holds, so that box reaches the edge: the cylinder's whole reading is off where its
/// turn's is and reaches an edge everywhere else. The two trims read the same points of the same
/// curve against the same marks, in the same order, and their readings of both domains differ only
/// where neither the turn's nor the other domain's is off: there the trim on both whole domains
/// reads an edge reached and stops. So where it keeps a piece, every reading agreed and the trim on
/// the turn keeps the same; where it stops, the trim on the turn is the answer. The end circle
/// built for `curve` is then `curve` itself, made from the same doubles. Any other piece is trimmed
/// on both whole domains first, and on its end circle's turn only where that stops.
fn trimmed_or_along(
    curve: &CurveKind<3>,
    reach: f64,
    (a, b): (&SurfaceKind, &SurfaceKind),
    marks: &[Vector],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Part>>, IntersectError> {
    let along = along_edge(curve, reach, (a, b))?;
    if let Some((_, on, true)) = along {
        return trimmed(curve, (a, b), marks, Some(on), reach, meter);
    }
    if let Some(parts) = trimmed(curve, (a, b), marks, None, reach, meter)? {
        return Ok(Some(parts));
    }
    let Some((edge, on, _)) = along else {
        return Ok(None);
    };
    trimmed(&edge, (a, b), marks, Some(on), reach, meter)
}

/// A stretch of a piece between cuts, and whether it lies on both domains.
struct Stretch {
    from: f64,
    to: f64,
    on: bool,
}

/// The runs of stretches on both domains, end to end; on a loop the last runs on into the first.
fn runs(stretches: &[Stretch], loops: bool) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut previous_on = false;
    for (k, s) in stretches.iter().enumerate() {
        match out.last_mut() {
            Some(run) if s.on && previous_on => run.push(k),
            _ if s.on => out.push(alloc::vec![k]),
            _ => {}
        }
        previous_on = s.on;
    }
    let ends_on = stretches.first().is_some_and(|s| s.on) && stretches.last().is_some_and(|s| s.on);
    if loops && ends_on && out.len() > 1 {
        let first = out.remove(0);
        if let Some(last) = out.last_mut() {
            last.extend(first);
        }
    }
    out
}

/// What a run keeps. A run is one piece on both domains, whatever cuts lie inside it: there the
/// curve only touches an edge, or two edges' cuts at one point stand a little apart. A loop on both
/// all round is whole; a run wider than [`STEP`] is one stretch; and a run no wider is a meeting the
/// domains' ends make that small, a point at its middle at this resolution, never nothing.
fn kept(
    curve: &CurveKind<3>,
    stretches: &[Stretch],
    run: &[usize],
    (lo, hi, loops): (f64, f64, bool),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Part>, IntersectError> {
    let period = hi - lo;
    if loops && run.len() == stretches.len() {
        return Ok(stretch(curve, lo, hi)?.map(|c| Part::Curve(Box::new(c), [CurveEnd::Closed; 2])));
    }
    let (Some(first), Some(last)) = (
        run.first().and_then(|k| stretches.get(*k)),
        run.last().and_then(|k| stretches.get(*k)),
    ) else {
        return Err(IntersectError::Internal);
    };
    // A run that wraps a loop's end ends a turn on.
    let to = if last.to < first.from {
        last.to + period
    } else {
        last.to
    };
    let Some(part) = stretch(curve, first.from, to)? else {
        return Ok(None);
    };
    let width = crate::curve::widest(&part.bound((first.from, to), meter)?);
    if width > STEP {
        return Ok(Some(Part::Curve(Box::new(part), [CurveEnd::Boundary; 2])));
    }
    let middle_of = first.from * 0.5 + to * 0.5;
    let t = if loops && middle_of > hi {
        middle_of - period
    } else {
        middle_of
    };
    Ok(Some(Part::Point {
        at: at_parameter(curve, t, meter)?,
        width,
    }))
}

/// The item of a point on both surfaces, or none if it lies off a domain.
fn point_item(
    at: Vector,
    contact: PointContact,
    reach: f64,
    a: &SurfaceKind,
    b: &SurfaceKind,
) -> Result<Option<Item>, IntersectError> {
    let (Some([p, q]), Some([r, s])) = (on_surface(a, at)?, on_surface(b, at)?) else {
        return Ok(None);
    };
    let [x, y, z] = at;
    let box_of = Bounds {
        lo: [x.lo(), y.lo(), z.lo()],
        hi: [x.hi(), y.hi(), z.hi()],
    };
    Ok(Some(Item::Point {
        at: ParamBox::new(&[p, q, r, s])?,
        xyz: grid_box(&box_of)?,
        contact,
        class: class_of(reach),
    }))
}

/// The class of a result whose data reach `reach` from the exact conic.
fn class_of(reach: f64) -> Exactness {
    Exactness::Certified {
        bound: Steps(exact_steps(reach).saturating_add(1)),
        step: GridStep::Q0,
    }
}

/// The four spans of a curve on two surfaces, from the box around it.
fn spans_of(
    box_of: &Bounds<3>,
    a: &SurfaceKind,
    b: &SurfaceKind,
) -> Result<Option<ParamBox>, IntersectError> {
    let at = sides(box_of)?;
    let (Some([p, q]), Some([r, s])) = (on_surface(a, at)?, on_surface(b, at)?) else {
        return Ok(None);
    };
    Ok(Some(ParamBox::new(&[p, q, r, s])?))
}

/// The items of a pair's pieces; `None` where [`trimmed`] leaves a piece to the engine.
fn items_of(
    found: Vec<Piece>,
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Item>>, IntersectError> {
    let mut items = Vec::new();
    // The pair's marks, found on the first piece that is a curve and kept for the rest.
    let mut found_marks: Option<Vec<Vector>> = None;
    for piece in found {
        match piece {
            Piece::Same => {
                let side = |(lo, hi): (f64, f64)| -> Result<Interval, IntersectError> {
                    Ok(Interval::new(
                        tight::larger(lo, -REACH),
                        tight::smaller(hi, REACH),
                    )?)
                };
                let ((p, q), (r, t)) = (ParamSurface::domain(a), ParamSurface::domain(b));
                let at = ParamBox::new(&[side(p)?, side(q)?, side(r)?, side(t)?])?;
                items.push(Item::Overlap {
                    at,
                    region: OverlapRegion {
                        boundary: Vec::new(),
                    },
                    proof: Coincidence::Identical,
                    class: Exactness::Exact,
                });
            }
            Piece::Point { at, tangent } => {
                let contact = if tangent {
                    PointContact::IsolatedTangent
                } else {
                    PointContact::Crossing
                };
                items.extend(point_item(at, contact, wide(at), a, b)?);
            }
            Piece::Curve {
                curve,
                tangent,
                reach,
            } => {
                let marks = match found_marks.take() {
                    Some(list) => list,
                    None => marks(a, b, meter)?,
                };
                let curve = nearer(*curve)?;
                let Some(parts) = trimmed_or_along(&curve, reach, (a, b), &marks, meter)? else {
                    return Ok(None);
                };
                found_marks = Some(marks);
                for kept in parts {
                    let (part, ends) = match kept {
                        Part::Curve(part, ends) => (*part, ends),
                        Part::Point { at, width } => {
                            // The point stands within the run's width of every point of it.
                            let reach = tight::larger(reach, width + wide(at));
                            items.extend(point_item(at, PointContact::BoundaryTouch, reach, a, b)?);
                            continue;
                        }
                    };
                    let span = crate::curve::finite(&part)?;
                    let box_of = part.bound(span, meter)?;
                    let Some(at) = spans_of(&box_of, a, b)? else {
                        continue;
                    };
                    let contact = if tangent {
                        let gap = StepsInterval::new(Steps(0), Steps(0))
                            .ok_or(IntersectError::Internal)?;
                        CurveContact::Tangent {
                            band: Steps(0),
                            gap,
                        }
                    } else {
                        CurveContact::Crossing
                    };
                    items.push(Item::Curve {
                        at,
                        curve: Box::new(part),
                        contact,
                        ends,
                        class: class_of(reach),
                    });
                }
            }
        }
    }
    Ok(Some(items))
}

/// Whether an operand fits the frame's range (12.1): its box over its domain within 2^40 steps of
/// the origin, or, for a plane that runs on without end, the point it is stored by; and its
/// parameters hold its own points to a step.
///
/// A parameter far from zero is held no closer than the doubles' spacing there, and the surface
/// moves that spacing times its speed between two of them: a turn starts any number of turns out,
/// and 2^42 half-turns out a circle of a unit's radius has its representable points more than two
/// steps apart, so no cut on it, and none of its points, can be placed to a step. The spacing is
/// that of the domain's end further from zero, and the speed the largest the partials reach over
/// the domain, so the bound holds everywhere on it.
fn fits(s: &SurfaceKind, meter: &mut WorkMeter<'_>) -> Result<bool, IntersectError> {
    let inside = |x: f64| x.abs() <= REACH;
    let ((a, b), (c, d)) = ParamSurface::domain(s);
    if let SurfaceKind::Plane(p) = s
        && [a, b, c, d].iter().any(|x| !x.is_finite())
    {
        let (o, ..) = p.vectors();
        return Ok(o.iter().all(|x| inside(*x)));
    }
    let region = ((a, b), (c, d));
    let bounds = s.bound(region, meter)?;
    let spacing = |x: f64, y: f64| {
        let far = tight::larger(x.abs(), y.abs());
        far.next_up() - far
    };
    // The first partials alone, along u and then along v: the box above is the point's own, so
    // reading it again through all the partials to order 1 would only repeat its work. Each kind
    // bounds a first partial over a region by the same closed form or hull as its partials to
    // order 1 there, and one it cannot bound is refused as those would be.
    //
    // A kind written as fixed terms, or an extrusion, is screened first by its speed's ceiling
    // from its forms and vectors alone, which takes no extent. That ceiling is at least the length of the extent's box
    // up to the box's rounding, so twice it is more than that length: where twice the ceiling
    // holds the doubles within a step, the extent's bound does too, and the answer is the
    // extent's. Only a parameter far from zero, where the spacing is coarse, falls to the extent.
    let mut apart: f64 = 0.0;
    for (gap, dir) in [(spacing(a, b), Direction::U), (spacing(c, d), Direction::V)] {
        if let Some(ceiling) = s.speed_ceiling(region, dir)?
            && gap * (2.0 * ceiling) <= STEP
        {
            continue;
        }
        let DerivativeBound::Bounded(along) = s.derivative_bound(region, dir, meter)? else {
            return Err(CurveError::Unbounded.into());
        };
        let ([l0, l1, l2], [h0, h1, h2]) = (along.lo, along.hi);
        let speed = [
            Interval::new(l0, h0)?,
            Interval::new(l1, h1)?,
            Interval::new(l2, h2)?,
        ];
        apart = tight::larger(apart, gap * Interval::norm(&speed)?.hi());
    }
    Ok(bounds.lo.iter().chain(&bounds.hi).all(|x| inside(*x)) && apart <= STEP)
}

/// Where two natural quadrics meet in closed form (12.7); `None` for a pair that is not natural, or
/// that stands in general position, or that no enclosure can decide.
///
/// # Errors
///
/// [`IntersectError::FrameRange`] if an operand does not fit the frame's range (12.1), or its
/// parameters' doubles stand more than a step apart on it, checked before any closed form; a kind's
/// errors, and the meter's stop.
pub fn natural(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    meter.poll()?;
    let (Some(first), Some(second)) = (rank(a), rank(b)) else {
        return Ok(None);
    };
    if !fits(a, meter)? || !fits(b, meter)? {
        return Err(IntersectError::FrameRange);
    }
    let turned = first > second;
    let (x, y) = if turned { (b, a) } else { (a, b) };
    let Some(found) = pieces(x, y, meter)? else {
        return Ok(None);
    };
    let Some(items) = items_of(found, x, y, meter)? else {
        return Ok(None);
    };
    let answer = Intersection::new(items, Coverage::TopologyCertified, Vec::new(), meter)?;
    Ok(Some(if turned { answer.swapped(2) } else { answer }))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        reason = "test code: a failure should stop loudly"
    )]

    use super::*;

    /// The frame of an axis along (0.6, 0.8, 0): neither number is a double, so a centre 300
    /// along it in decimals, (180, 240, 0), stands a hair (about 1.3e-14) off the stored axis.
    fn slanted() -> Frame<3> {
        Frame::new([0.0, 0.0, 1.0], [0.6, 0.8, 0.0]).unwrap()
    }

    fn tube_at(centre: [f64; 3], radius: f64) -> Cylinder {
        Cylinder::new(centre, radius, slanted(), ((0.0, 2.0), (-300.0, 300.0))).unwrap()
    }

    /// How far `p` stands from the axis of `c`, in doubles.
    fn off_axis(p: [f64; 3], c: &Cylinder) -> f64 {
        let [x, y, z] = c.frame().normal();
        let size = (x * x + y * y + z * z).sqrt();
        let [a, b, e] = c.center();
        let w = [p[0] - a, p[1] - b, p[2] - e];
        let n = [
            (w[1] * z - w[2] * y) / size,
            (w[2] * x - w[0] * z) / size,
            (w[0] * y - w[1] * x) / size,
        ];
        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
    }

    /// The lines `pieces` holds, each checked to lie along the axes and on both cylinders.
    fn lines_on_both(pieces: &[Piece], a: &Cylinder, b: &Cylinder) -> Vec<[f64; 3]> {
        let mut at = Vec::new();
        for piece in pieces {
            let Piece::Curve {
                curve,
                tangent,
                reach,
            } = piece
            else {
                panic!("not a curve");
            };
            let CurveKind::Line(l) = curve.as_ref() else {
                panic!("not a line");
            };
            assert!(!tangent);
            assert!(*reach < 1e-12, "an enclosure {reach} wide");
            let [p, q, r] = l.direction();
            let [x, y, z] = a.frame().normal();
            let skew = (q * z - r * y).abs() + (r * x - p * z).abs() + (p * y - q * x).abs();
            assert!(skew < 1e-12, "{skew}");
            for c in [a, b] {
                let off = off_axis(l.origin(), c) - c.radius();
                assert!(
                    off.abs() < 1e-9,
                    "{off} off a cylinder of radius {}",
                    c.radius()
                );
            }
            at.push(l.origin());
        }
        at
    }

    /// The search's input `022d3c0009030002000000090305` (natural target): two cylinders of radius
    /// 10 on parallel axes a hair apart meet in two lines, one each side of the plane of the axes.
    #[test]
    fn equal_cylinders_on_parallel_axes_a_hair_apart_meet_in_two_lines() {
        let (a, b) = (tube_at([180.0, 240.0, 0.0], 10.0), tube_at([0.0; 3], 10.0));
        assert!(parallel(a.frame().normal(), b.frame().normal()));
        assert!(!same_line(
            a.center(),
            a.frame().normal(),
            b.center(),
            b.frame().normal()
        ));
        for (p, q) in [(&a, &b), (&b, &a)] {
            let found = parallel_cylinders(p, q).unwrap();
            assert_eq!(found.len(), 2);
            let at = lines_on_both(&found, p, q);
            // About 20 apart: one each side.
            let [x, y, z] = [0, 1, 2].map(|i| at[0][i] - at[1][i]);
            let gap = (x * x + y * y + z * z).sqrt();
            assert!(gap > 19.0 && gap < 21.0, "{gap}");
        }
    }

    /// The same axes, with radii two steps apart at 10: less than the hair between the axes, so
    /// neither cylinder lies inside the other and they still cross in two lines.
    #[test]
    fn cylinders_whose_radii_differ_by_less_than_the_hair_between_their_axes_meet_in_two_lines() {
        let smaller = 10.0_f64.next_down().next_down();
        let (a, b) = (
            tube_at([180.0, 240.0, 0.0], 10.0),
            tube_at([0.0; 3], smaller),
        );
        for (p, q) in [(&a, &b), (&b, &a)] {
            let found = parallel_cylinders(p, q).unwrap();
            assert_eq!(found.len(), 2);
            lines_on_both(&found, p, q);
        }
    }

    /// The same axes, with radii 10 and 5: one inside the other, so they meet nowhere.
    #[test]
    fn unequal_cylinders_on_parallel_axes_a_hair_apart_meet_nowhere() {
        let (a, b) = (tube_at([180.0, 240.0, 0.0], 10.0), tube_at([0.0; 3], 5.0));
        for (p, q) in [(&a, &b), (&b, &a)] {
            assert!(parallel_cylinders(p, q).unwrap().is_empty());
        }
    }
}
