//! Exact decisions in closed form: how a line meets a quadric or a torus, where two natural
//! quadrics stand, and whether a patch's edge collapses (12.6, 12.7, 12.10).
//!
//! What it does: on the exact data of 12.7, [`line_sphere`], [`line_cylinder`] and [`line_cone`] say
//! how a line meets a quadric as a [`Meeting`], with no square root anywhere, and [`line_torus`]
//! counts its real points on a torus. [`between`] counts them on a stretch of the line. [`position`]
//! decides which of Goldman and Miller's special positions two natural quadrics stand in.
//! [`collapsed`] decides whether the control points along a patch's edge are one point.
//!
//! Guarantees:
//!
//! - **Exact on exact data.** Every decision here is a sign from `geom-num`'s `quadric` and `sturm`
//!   modules, each with its budget written there: no double decides anything.
//! - **Every degenerate case named.** A line along a cylinder or a cone's ruling is [`Meeting::Within`],
//!   one through a cone's apex and nothing else is [`Meeting::Apex`], and a line parallel to a
//!   ruling that crosses once is [`Meeting::Once`]. A shape with no size is
//!   [`IntersectError::InvalidInput`], never an answer.
//! - **Positions in order of how much they share.** [`position`] reports the strongest position the
//!   pair holds: one axis, then a shared apex, parallel axes, axes meeting with a tangent sphere, and
//!   axes meeting.
//!
//! Method: the closed forms of 12.6 and 12.7, on Goldman and Miller's geometric conditions (SMA '91,
//! doi:10.1145/112515.112545; GMIP 57(1), 1995, doi:10.1006/gmip.1995.1006), whose planar sections
//! are the two quadrics' rotational symmetry and a sphere tangent to both about the meeting of their
//! axes. `projects/engines/geometry/architecture/12-intersections.md` holds the table.
//!
//! Cost: a few predicates per call, each one filter and at most one exact path. [`line_torus`] and
//! [`between`] add at most one Sturm chain. [`collapsed`] is linear. No allocation. Recursion: none.

use geom_num::Sign;
use geom_num::grid::{P3, Steps};
use geom_num::predicate::orient3d;
use geom_num::quadric::{self, Cone, Cylinder, Line, Sphere, Surface, Torus};
use geom_num::sturm::{Point, Roots};

use crate::error::IntersectError;

/// How a line meets a quadric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meeting {
    /// It has no real point on the surface.
    Nowhere,
    /// It crosses once: a line parallel to a cone's ruling.
    Once,
    /// It crosses at two points.
    Twice,
    /// It touches at one point, a double root.
    Tangent,
    /// It passes through a cone's apex and meets the cone nowhere else.
    Apex,
    /// It lies in the surface.
    Within,
}

/// A natural quadric that is not a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Natural {
    /// A sphere.
    Sphere(Sphere),
    /// A cylinder.
    Cylinder(Cylinder),
    /// A cone.
    Cone(Cone),
}

/// Where two natural quadrics stand, the strongest that holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// They share an axis, so they meet in circles.
    Coaxial,
    /// Two cones share an apex, so they meet in rulings through it.
    SharedApex,
    /// Their axes are parallel: lines for two cylinders, a conic for cones of one slope.
    ParallelAxes,
    /// Their axes meet, and one sphere about that point touches both: two conics.
    TangentSphere,
    /// Their axes meet, with no such sphere.
    AxesMeet,
    /// None of these: the general paths decide.
    General,
}

fn line_is_real(line: &Line) -> Result<(), IntersectError> {
    if line.a == line.b {
        return Err(IntersectError::InvalidInput);
    }
    Ok(())
}

/// How `line` meets `sphere`.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a line of one point, a sphere of no size, or a radius past
/// the budget.
pub fn line_sphere(line: &Line, sphere: &Sphere) -> Result<Meeting, IntersectError> {
    line_is_real(line)?;
    if sphere.radius == Steps::ZERO {
        return Err(IntersectError::InvalidInput);
    }
    Ok(match quadric::line_sphere(line, sphere)? {
        Sign::Positive => Meeting::Twice,
        Sign::Zero => Meeting::Tangent,
        Sign::Negative => Meeting::Nowhere,
    })
}

/// How `line` meets `cylinder`.
///
/// # Errors
///
/// As [`line_sphere`], and for an axis of one point.
pub fn line_cylinder(line: &Line, cylinder: &Cylinder) -> Result<Meeting, IntersectError> {
    line_is_real(line)?;
    if cylinder.radius == Steps::ZERO || cylinder.base == cylinder.along {
        return Err(IntersectError::InvalidInput);
    }
    if quadric::parallel(line.a, line.b, cylinder.base, cylinder.along)? {
        let gap = quadric::axis_gap(cylinder.base, cylinder.along, line.a, cylinder.radius)?;
        return Ok(if gap == Sign::Zero {
            Meeting::Within
        } else {
            Meeting::Nowhere
        });
    }
    Ok(match quadric::line_cylinder(line, cylinder)? {
        Sign::Positive => Meeting::Twice,
        Sign::Zero => Meeting::Tangent,
        Sign::Negative => Meeting::Nowhere,
    })
}

/// How `line` meets `cone`, the double cone of both nappes.
///
/// # Errors
///
/// As [`line_sphere`], and for an axis of one point or a slope with a zero part.
pub fn line_cone(line: &Line, cone: &Cone) -> Result<Meeting, IntersectError> {
    line_is_real(line)?;
    if cone.apex == cone.along || cone.slope.rise == 0 || cone.slope.run == 0 {
        return Err(IntersectError::InvalidInput);
    }
    // The apex on the line is where the discriminant's zero stops meaning a tangent.
    let through = quadric::parallel(line.a, line.b, line.a, cone.apex)?;
    let delta = quadric::line_cone(line, cone)?;
    let steep = quadric::line_cone_steep(line, cone)?;
    Ok(match (steep, delta) {
        (Sign::Zero, Sign::Zero) if through => Meeting::Within,
        (Sign::Zero, Sign::Zero) | (_, Sign::Negative) => Meeting::Nowhere,
        (Sign::Zero, Sign::Positive) => Meeting::Once,
        (_, Sign::Positive) => Meeting::Twice,
        (_, Sign::Zero) if through => Meeting::Apex,
        (_, Sign::Zero) => Meeting::Tangent,
    })
}

/// The real points of `line` on `torus`, each counted once, and whether any root is repeated: a
/// tangent, or a root the line shares with the complex part of the quartic.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a line of one point, an axis of one point, or a tube that
/// does not fit: a minor radius of zero.
pub fn line_torus(line: &Line, torus: &Torus) -> Result<Roots, IntersectError> {
    line_is_real(line)?;
    if torus.minor == Steps::ZERO || torus.centre == torus.along {
        return Err(IntersectError::InvalidInput);
    }
    Ok(quadric::roots(line, &Surface::Torus(*torus))?)
}

/// How many real points of `line` on `surface` lie strictly between its parameters `lo` and `hi`,
/// each counted once.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] unless `lo` is below `hi`, or for a line whose equation with
/// `surface` is a constant, which [`line_cylinder`] and [`line_cone`] name.
pub fn between(line: &Line, surface: &Surface, lo: Point, hi: Point) -> Result<u8, IntersectError> {
    Ok(quadric::roots_between(line, surface, lo, hi)?)
}

/// A quadric's axis, for a cylinder or a cone.
const fn axis(q: &Natural) -> Option<(P3, P3)> {
    match q {
        Natural::Sphere(_) => None,
        Natural::Cylinder(c) => Some((c.base, c.along)),
        Natural::Cone(c) => Some((c.apex, c.along)),
    }
}

/// Which position two quadrics whose axes meet at one point stand in.
fn meeting_axes(a: &Natural, b: &Natural) -> Result<Position, IntersectError> {
    let touching = match (a, b) {
        (Natural::Cylinder(x), Natural::Cylinder(y)) => x.radius == y.radius,
        (Natural::Cylinder(x), Natural::Cone(y)) | (Natural::Cone(y), Natural::Cylinder(x)) => {
            quadric::tangent_cylinder_cone(x, y)? == Sign::Zero
        }
        (Natural::Cone(x), Natural::Cone(y)) => quadric::tangent_cone_cone(x, y)? == Sign::Zero,
        _ => false,
    };
    Ok(if touching {
        Position::TangentSphere
    } else {
        Position::AxesMeet
    })
}

/// Where `a` and `b` stand (12.7's table, Goldman and Miller's conditions).
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for an axis of one point or a size past the budget.
#[allow(
    clippy::many_single_char_names,
    reason = "two axes and their points, as in 12.7's table"
)]
pub fn position(a: &Natural, b: &Natural) -> Result<Position, IntersectError> {
    let (first, second) = (axis(a), axis(b));
    if [first, second].iter().flatten().any(|(p, q)| p == q) {
        return Err(IntersectError::InvalidInput);
    }
    let centre = |q: &Natural| match q {
        Natural::Sphere(s) => Some(s.centre),
        _ => None,
    };
    match (first, second) {
        // Any two centres lie on one line, which is an axis of both spheres.
        (None, None) => Ok(Position::Coaxial),
        (Some((p, q)), None) | (None, Some((p, q))) => {
            let c = centre(a).or(centre(b)).ok_or(IntersectError::Internal)?;
            Ok(if quadric::parallel(p, q, p, c)? {
                Position::Coaxial
            } else {
                Position::General
            })
        }
        (Some((p, q)), Some((r, s))) => {
            if quadric::parallel(p, q, r, s)? {
                return Ok(if quadric::parallel(p, q, p, r)? {
                    Position::Coaxial
                } else {
                    Position::ParallelAxes
                });
            }
            if let (Natural::Cone(x), Natural::Cone(y)) = (a, b)
                && x.apex == y.apex
            {
                return Ok(Position::SharedApex);
            }
            if orient3d(p, q, r, s)? == Sign::Zero {
                return meeting_axes(a, b);
            }
            Ok(Position::General)
        }
    }
}

/// Whether the control points along an edge are one point (12.10): an exact test on the grid.
/// Comparing whole numbers is already exact and cheaper than any filter, so it has none.
#[must_use]
pub fn collapsed(edge: &[P3]) -> bool {
    edge.windows(2).all(|w| matches!(w, [a, b] if a == b))
}
