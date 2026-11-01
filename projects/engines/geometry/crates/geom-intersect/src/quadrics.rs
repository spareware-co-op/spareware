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
//!   and each stretch is kept if its middle lies on both.
//!
//! Method: Goldman and Miller, SMA '91 (<https://doi.org/10.1145/112515.112545>) and GMIP 57(1), 1995
//! (<https://doi.org/10.1006/gmip.1995.1006>); the table of chapter 12.7
//! (`projects/engines/geometry/architecture/12-intersections.md`). Two surfaces of revolution tangent
//! to one sphere S along planes P and Q are S − k·P² and S − l·Q², so their difference factors as
//! (√l·Q − √k·P)(√l·Q + √k·P), two planes.
//!
//! Cost: a few predicates and square roots for a pair, and a curve-surface pair for each edge that a
//! trimmed domain has. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "the names are the chapter's: n a plane's normal, e an axis, c a centre, k a slope"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola};
use geom_curve::{HYPERBOLA_REACH, ParamCurve, tight};
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::interval::Interval;
use geom_num::{Exactness, NumError, Sign, predicate, quadric};
use geom_nurbs::{Bounds, Limit};
use geom_run::WorkMeter;
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

use crate::closed::{self, Natural, Position};
use crate::curve::{exact_steps, grid_box, grid_point, held, sign_of, turn_of, whole};
use crate::curve_surface::{cross, curve_surface, dot, lift, minus, sides};
use crate::error::IntersectError;
use crate::result::{
    Coincidence, Coverage, CurveContact, CurveEnd, Intersection, Item, OverlapRegion, ParamBox,
    PointContact, StepsInterval,
};

/// A vector, enclosed.
type Vector = [Interval; 3];

/// How far a point can stand from a curve and still be read as on it, in steps.
const STEP: f64 = 2.0;

/// How far a shared domain is written, in each parameter of a surface with no end.
const REACH: f64 = 1_099_511_627_776.0;

/// What a pair's closed form finds.
enum Piece {
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

fn scale(v: Vector, k: Interval) -> Result<Vector, IntersectError> {
    let [x, y, z] = v;
    Ok([x.mul(k)?, y.mul(k)?, z.mul(k)?])
}

fn plus(a: Vector, b: Vector) -> Result<Vector, IntersectError> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    Ok([p.add(x)?, q.add(y)?, r.add(z)?])
}

fn unit(v: Vector) -> Result<Vector, IntersectError> {
    let [x, y, z] = v;
    let length = tight::norm(&[x, y, z])?;
    Ok([x.div(length)?, y.div(length)?, z.div(length)?])
}

fn middle(v: Vector) -> [f64; 3] {
    v.map(tight::middle)
}

/// How wide an enclosure of a vector is.
fn wide(v: Vector) -> f64 {
    let [x, y, z] = v.map(|c: Interval| c.hi() - c.lo());
    tight::larger(tight::larger(x, y), z)
}

fn point(v: f64) -> Result<Interval, IntersectError> {
    Ok(Interval::point(v)?)
}

/// The square root of what is known not to be negative, whose enclosure may reach below zero.
fn root(v: Interval) -> Result<Interval, IntersectError> {
    let lo = if v.lo() < 0.0 { 0.0 } else { v.lo() };
    Ok(Interval::new(lo, tight::larger(lo, v.hi()))?.sqrt()?)
}

/// A vector not along `n`, for a frame's reference direction.
fn beside(n: [f64; 3]) -> [f64; 3] {
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
fn circle(
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
fn line(at: Vector, along: Vector, tangent: bool) -> Result<Piece, IntersectError> {
    let curve = Line::infinite(middle(at), middle(along))?;
    Ok(Piece::Curve {
        curve: Box::new(CurveKind::Line(curve)),
        tangent,
        reach: wide(at),
    })
}

/// How a sign stands: the exact decision where one came, the enclosure's otherwise, or `None`.
fn decided(exact: Option<Result<Sign, NumError>>, enclosure: Interval) -> Option<Sign> {
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
struct Sheet {
    at: Vector,
    normal: Vector,
    exact: Option<quadric::Plane>,
}

fn sheet(plane: &Plane) -> Result<Sheet, IntersectError> {
    let (c, a, b) = plane.vectors();
    let far = |v: [f64; 3]| {
        let [p, q, r] = c;
        let [x, y, z] = v;
        grid_point(&[p + x, q + y, r + z])
    };
    let exact = (|| {
        Some(quadric::Plane {
            a: grid_point(&c)?,
            b: far(a)?,
            c: far(b)?,
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
    let stored = frame.normal();
    let [a, b, c] = center;
    let [p, q, r] = stored;
    let exact = (|| Some((grid_point(&center)?, grid_point(&[a + p, b + q, c + r])?)))();
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
    let line = axis(cone.center(), &cone.frame(), cone.axial_sign())?;
    let (r0, k) = cone.radius_and_slope();
    // Exact where the data are: a grid apex is what makes the exact decisions possible. A cone
    // stored by its apex has it as its centre, exactly, whatever its axis; any other is reached
    // along the unit axis, exact only where that is.
    let apex = if r0 == 0.0 {
        line.at
    } else {
        let v = tight::div(tight::point(-r0)?, tight::point(k)?)?;
        let [a, b, c] = line.at;
        let [x, y, z] = line.along;
        [
            tight::add(a, tight::mul(x, v)?)?,
            tight::add(b, tight::mul(y, v)?)?,
            tight::add(c, tight::mul(z, v)?)?,
        ]
    };
    let slope = point(k.abs())?;
    let exact = (|| {
        if wide(apex) > 0.0 {
            return None;
        }
        let centre = middle(apex);
        let [a, b, c] = centre;
        let [p, q, r] = cone.frame().normal();
        Some(quadric::Cone {
            apex: grid_point(&centre)?,
            along: grid_point(&[a + p, b + q, c + r])?,
            slope: quadric::Slope {
                rise: u64::try_from(whole(k.abs() * 1_048_576.0)?).ok()?,
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
fn off_line(p: [f64; 3], c: [f64; 3], e: [f64; 3]) -> bool {
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
fn parallel(a: [f64; 3], b: [f64; 3]) -> bool {
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
fn same_line(a: [f64; 3], e: [f64; 3], b: [f64; 3], f: [f64; 3]) -> bool {
    parallel(e, f) && !off_line(b, a, e)
}

/// A plane with a plane.
fn plane_plane(a: &Plane, b: &Plane) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (p, q) = (sheet(a)?, sheet(b)?);
    let d = cross(p.normal, q.normal)?;
    let (o, u, v) = b.vectors();
    let far = |w: [f64; 3]| {
        let [x, y, z] = o;
        let [e, f, g] = w;
        grid_point(&[x + e, y + f, z + g])
    };
    let exact = match (p.exact, grid_point(&o), far(u), far(v)) {
        (Some(s), Some(g), Some(h), Some(k)) => {
            let parallel = quadric::plane_along(&s, g, h)? == Sign::Zero
                && quadric::plane_along(&s, g, k)? == Sign::Zero;
            Some((
                parallel,
                predicate::orient3d(s.a, s.b, s.c, g)? == Sign::Zero,
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
    if along.is_none() && tilt.contains(0.0) {
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
fn height(at: Vector, on: &Axis) -> Result<Interval, IntersectError> {
    dot(minus(at, on.at)?, on.along)
}

/// The circle about `on` at height `z` with the given radius.
fn ring(on: &Axis, z: Interval, r: Interval, tangent: bool) -> Result<Piece, IntersectError> {
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
    let (one, two) = (point(r)?, point(q)?);
    let sum = one.add(two)?;
    let gap = one.sub(two)?;
    let outer = exact(r + q).or_else(|| decided(None, d.sub(sum).ok()?));
    let inner = exact((r - q).abs()).or_else(|| decided(None, d.sub(tight_size(gap)).ok()?));
    if outer == Some(Sign::Positive) || inner == Some(Sign::Negative) {
        return Ok(Vec::new());
    }
    let along = unit(run)?;
    let foot = one
        .square()?
        .add(d.sub(two)?.mul(d.add(two)?)?)?
        .div(d.mul(point(2.0)?)?)?;
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
            let stored =
                e == f && a.center() == b.center() && a.radius_and_slope() == b.radius_and_slope();
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
fn pieces(a: &SurfaceKind, b: &SurfaceKind) -> Result<Option<Vec<Piece>>, IntersectError> {
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
        _ => None,
    }
}

/// The parameters of a surface's points in `at`, enclosed; a turn a box runs around is the domain's.
fn parameters(surface: &SurfaceKind, at: Vector) -> Result<Option<[Interval; 2]>, IntersectError> {
    let (u, _) = ParamSurface::domain(surface);
    let whole_turn = |x: Interval, y: Interval| -> Result<Interval, IntersectError> {
        if x.contains(0.0) && y.contains(0.0) {
            Ok(Interval::new(u.0, u.1)?)
        } else {
            let t = turn_of(x, y)?;
            let two = point(2.0)?;
            let mut moved = t;
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
        SurfaceKind::Cylinder(c) => {
            let [x, y, z] = local(&c.frame(), c.center())?;
            [whole_turn(x, y)?, z.mul(point(c.axial_sign())?)?]
        }
        SurfaceKind::Cone(c) => {
            let [x, y, z] = local(&c.frame(), c.center())?;
            let v = z.mul(point(c.axial_sign())?)?;
            let (r0, k) = c.radius_and_slope();
            let rho = point(r0)?.add(point(k)?.mul(v)?)?;
            let flip = if rho.hi() < 0.0 { -1.0 } else { 1.0 };
            let flip = point(flip)?;
            [whole_turn(x.mul(flip)?, y.mul(flip)?)?, v]
        }
        SurfaceKind::Sphere(s) => {
            let [x, y, z] = local(&s.frame(), s.center())?;
            let z = z.mul(point(s.axial_sign())?)?;
            let round = tight::norm(&[x, y])?;
            [
                whole_turn(x, y)?,
                Interval::atan2(z, round)?.div(Interval::PI)?,
            ]
        }
        _ => return Ok(None),
    }))
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
    let Some([u, v]) = parameters(surface, at)? else {
        return Ok(None);
    };
    let (du, dv) = ParamSurface::domain(surface);
    let (Some(u), Some(v)) = (on(u, du)?, on(v, dv)?) else {
        return Ok(None);
    };
    Ok(Some([u, v]))
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
            for v in [v0, v1] {
                let at = plus(on.at, scale(on.along, point(v)?)?)?;
                let curve = Circle::new(middle(at), c.radius(), c.frame(), (u0, u1))?;
                out.push(CurveKind::Circle(curve));
            }
            if u1 - u0 < 2.0 {
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
            if u1 - u0 < 2.0 {
                rulings(surface, (u0, u1), (v0, v1), &mut out)?;
            }
        }
        SurfaceKind::Sphere(s) => {
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(s);
            let on = axis(s.center(), &s.frame(), s.axial_sign())?;
            let r = point(s.radius())?;
            for v in [v0, v1] {
                if v.abs() < 0.5 {
                    let turn = Interval::point(v)?;
                    let at = plus(on.at, scale(on.along, r.mul(turn.sinpi()?)?)?)?;
                    let radius = tight::middle(r.mul(turn.cospi()?)?);
                    let curve = Circle::new(middle(at), radius, s.frame(), (u0, u1))?;
                    out.push(CurveKind::Circle(curve));
                }
            }
            if u1 - u0 < 2.0 {
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
            let term = a.add(b)?;
            // cosh t + sinh t is positive; on the other branch this is no point of the curve.
            if term.lo() <= 0.0 {
                return Ok(None);
            }
            term.ln()?
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

/// Whether a kind's domain is a closed loop.
fn closed(curve: &CurveKind<3>) -> bool {
    matches!(curve, CurveKind::Circle(_) | CurveKind::Ellipse(_))
}

/// `curve` over `(lo, hi)`.
fn stretch(curve: &CurveKind<3>, lo: f64, hi: f64) -> Result<Option<CurveKind<3>>, IntersectError> {
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
        _ => None,
    })
}

/// The point of `curve` at `t`, enclosed.
fn at_parameter(
    curve: &CurveKind<3>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Vector, IntersectError> {
    let list = curve.enclose(t, Limit::FromRight, 0, meter)?;
    list.first().copied().ok_or(IntersectError::Internal)
}

/// Whether the point at `t` lies on both surfaces.
fn on_both(
    curve: &CurveKind<3>,
    t: f64,
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    let at = at_parameter(curve, t, meter)?;
    Ok(on_surface(a, at)?.is_some() && on_surface(b, at)?.is_some())
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
    let mut t = tight::middle(s);
    // On a loop the turns wrap.
    if closed(curve) {
        let period = hi - lo;
        while t < lo {
            t += period;
        }
        while t >= hi {
            t -= period;
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

/// Where the curve's stretches on both domains end: the parameters at which an edge of either surface
/// meets the other, sorted.
fn cuts(
    curve: &CurveKind<3>,
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<f64>, IntersectError> {
    let mut out = Vec::new();
    for (surface, other) in [(a, b), (b, a)] {
        for edge in edges(surface)? {
            let found = curve_surface(&edge, other, meter)?;
            for item in found.items() {
                let [t, ..] = item.at().spans() else {
                    continue;
                };
                let box_of = edge.bound((t.lo(), t.hi()), meter)?;
                if let Some(t) = cut_at(curve, sides(&box_of)?, meter)? {
                    out.push(t);
                }
            }
        }
        // A curve through an apex the domain ends at leaves the domain there.
        for at in apex_ends(surface)? {
            if let Some(t) = cut_at(curve, at, meter)? {
                out.push(t);
            }
        }
    }
    out.sort_unstable_by(f64::total_cmp);
    Ok(out)
}

/// The stretches of `curve` on both domains, and how each ends.
fn trimmed(
    curve: &CurveKind<3>,
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(CurveKind<3>, [CurveEnd; 2])>, IntersectError> {
    let (lo, hi) = crate::curve::finite(curve)?;
    let cut = cuts(curve, a, b, meter)?;
    let loops = closed(curve);
    if cut.is_empty() {
        let inside = on_both(curve, lo * 0.5 + hi * 0.5, a, b, meter)?;
        let kept = stretch(curve, lo, hi)?;
        return Ok(match kept {
            Some(c) if inside => {
                let end = if loops {
                    CurveEnd::Closed
                } else {
                    CurveEnd::Boundary
                };
                alloc::vec![(c, [end; 2])]
            }
            _ => Vec::new(),
        });
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
    let mut out = Vec::new();
    for end in ends {
        let middle_of = start * 0.5 + end * 0.5;
        let on_loop = if loops && middle_of > hi {
            middle_of - period
        } else {
            middle_of
        };
        // Flagged cuts around one point stand a little apart: what lies between them is no stretch.
        let part = stretch(curve, start, end)?;
        let long = match &part {
            Some(c) => crate::curve::widest(&c.bound((start, end), meter)?) > STEP,
            None => false,
        };
        let kept = if long && on_both(curve, on_loop, a, b, meter)? {
            part
        } else {
            None
        };
        if let Some(c) = kept {
            out.push((c, [CurveEnd::Boundary; 2]));
        }
        start = end;
    }
    Ok(out)
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

/// The items of a pair's pieces.
fn items_of(
    found: Vec<Piece>,
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Item>, IntersectError> {
    let mut items = Vec::new();
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
                let (Some([p, q]), Some([r, s])) = (on_surface(a, at)?, on_surface(b, at)?) else {
                    continue;
                };
                let [x, y, z] = at;
                let box_of = Bounds {
                    lo: [x.lo(), y.lo(), z.lo()],
                    hi: [x.hi(), y.hi(), z.hi()],
                };
                items.push(Item::Point {
                    at: ParamBox::new(&[p, q, r, s])?,
                    xyz: grid_box(&box_of)?,
                    contact: if tangent {
                        PointContact::IsolatedTangent
                    } else {
                        PointContact::Crossing
                    },
                    class: class_of(wide(at)),
                });
            }
            Piece::Curve {
                curve,
                tangent,
                reach,
            } => {
                for (part, ends) in trimmed(&curve, a, b, meter)? {
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
    Ok(items)
}

/// Where two natural quadrics meet in closed form (12.7); `None` for a pair that is not natural, or
/// that stands in general position, or that no enclosure can decide.
///
/// # Errors
///
/// A kind's errors, and the meter's stop.
pub fn natural(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Intersection>, IntersectError> {
    meter.poll()?;
    let (Some(first), Some(second)) = (rank(a), rank(b)) else {
        return Ok(None);
    };
    let turned = first > second;
    let (x, y) = if turned { (b, a) } else { (a, b) };
    let Some(found) = pieces(x, y)? else {
        return Ok(None);
    };
    let items = items_of(found, x, y, meter)?;
    let answer = Intersection::new(items, Coverage::TopologyCertified, Vec::new(), meter)?;
    Ok(Some(if turned { answer.swapped(2) } else { answer }))
}
