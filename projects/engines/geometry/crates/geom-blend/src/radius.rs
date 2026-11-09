//! Certified radius bounds and the largest feasible radius (13.10, "Feasibility first", steps 3 and
//! 5, G262).
//!
//! What it does: a rolling ball of radius r between two faces whose normals turn by an angle φ
//! touches each face at a distance r · tan(φ / 2) from the edge. [`half_angle_tangent`] gives that
//! tangent as an enclosure from two integer normals, with no angle taken. A chain's radius is
//! bounded by [`Limit`]s, each an enclosure with the [`Cause`] that makes it, and [`reach_limit`]
//! makes the limit that a face's width sets. [`largest`] reduces limits to the largest radius
//! that certainly builds, with the cause and whether the bound is open, and [`judge`] answers a
//! request: [`Verdict::Feasible`], or [`Verdict::RadiusTooLarge`] naming the radius that fits.
//!
//! Guarantees:
//!
//! - **Every bound is an enclosure.** A limit holds the true largest radius its cause allows;
//!   [`largest`] takes the lowest of their lower ends, so a radius it reports builds as far as that
//!   cause goes, never past it.
//! - **A refusal names the size that fits.** [`Verdict::RadiusTooLarge`] carries the largest
//!   feasible radius and the cause that sets it; the caller shows it and offers it, and never
//!   searches by retrying builds.
//! - **An open bound is reported as open.** Where a radius at the bound would leave a face or an
//!   edge of no extent, [`Largest::open`] is set and the radius reported is the largest number
//!   below the bound, not a multiple of a tolerance.
//! - **Undecided is not feasible.** A request whose radius is inside a limit's enclosure, neither
//!   below its lower end nor above its upper end, is [`Verdict::Undecided`], never accepted.
//! - **No limit, no answer.** With no limit at all [`largest`] is `None`.
//!
//! Method: chapter 13.10 of the kernel architecture, "Feasibility first", steps 3 and 5, and "A
//! refusal names the size that fits" (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`),
//! whose contract is solidcore's (<https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/docs/generic-blend.md>,
//! section 3.5); the tangent of the half angle is tan(φ / 2) = |n₁ × n₂| / (n₁ · n₂ + |n₁||n₂|), a
//! standard half-angle identity, with the norms by interval square roots.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/docs/generic-blend.md>,
//!   for its contract that a blend refusal returns the largest size that builds, with the cause that
//!   sets it (section 3.5). No code copied; the bounds here are re-derived as certified enclosures.
//!
//! Cost: constant per limit. No recursion.

use alloc::vec::Vec;

use geom_num::NumError;
use geom_num::interval::Interval;

/// What sets a limit on the radius (13.10, "A refusal names the size that fits").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// A face is too narrow for the ball's contact to stay on it.
    FaceWidth,
    /// A polygon face bounded by straight edges, every one blended, is taken whole once the balls
    /// reach its deepest point.
    PolygonTakenWhole,
    /// A ball as wide as the face it rolls on collapses that face's offset onto an axis or a
    /// centre.
    CollapsedOffset,
    /// A face does not enclose the ball locally: a principal radius of curvature.
    Fold,
    /// The ball must fit between the faces, as in a slot up to half its width.
    NoRoom,
    /// An open end of an overflow: the blend stays on its faces only up to this size.
    OpenEnd,
    /// Neighbouring setbacks must fit along an edge.
    Setback,
}

/// One bound on a chain's radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    /// What sets it.
    pub cause: Cause,
    /// The largest radius it allows, as an enclosure.
    pub radius: Interval,
    /// Whether a radius at the bound would leave a face or an edge of no extent: the bound itself is
    /// not feasible.
    pub open: bool,
}

/// The largest radius that certainly builds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Largest {
    /// The radius.
    pub radius: f64,
    /// The cause that sets it.
    pub cause: Cause,
    /// Whether the bound is open: the radius is the largest number below it.
    pub open: bool,
}

/// What a request for a radius comes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// The radius is within every limit.
    Feasible,
    /// A limit is passed, and this is the radius that fits.
    RadiusTooLarge(Largest),
    /// A limit's enclosure holds the radius: neither proven within nor proven past.
    Undecided(Cause),
}

/// The tangent of half the angle between two faces' normals, as an enclosure:
/// |n₁ × n₂| / (n₁ · n₂ + |n₁||n₂|).
///
/// # Errors
///
/// [`NumError::Degenerate`] when the normals are opposite, where the half angle is a right angle
/// and the tangent has no value (a knife edge); [`NumError::BudgetExceeded`] for components of
/// 2^83 or more in size.
pub fn half_angle_tangent(first: [i128; 3], second: [i128; 3]) -> Result<Interval, NumError> {
    let limit = 1_i128 << 83;
    if first.iter().chain(second.iter()).any(|c| c.abs() >= limit) {
        return Err(NumError::BudgetExceeded);
    }
    let enclose = |v: i128| -> Result<Interval, NumError> {
        // A component below 2^83 in size: the nearest double and one step either way holds it.
        #[allow(
            clippy::cast_precision_loss,
            reason = "the enclosure is widened by a double's step each way, which holds the value"
        )]
        let near = v as f64;
        if v == 0 {
            Interval::point(0.0)
        } else {
            Interval::new(near.next_down(), near.next_up())
        }
    };
    let [a0, a1, a2] = first;
    let [b0, b1, b2] = second;
    // The cross product's components, each below 2^167: held as intervals of integer products.
    let prod = |p: i128, q: i128, r: i128, s: i128| -> Result<Interval, NumError> {
        enclose(p)?
            .mul(enclose(q)?)?
            .sub(enclose(r)?.mul(enclose(s)?)?)
    };
    let cross = [
        prod(a1, b2, a2, b1)?,
        prod(a2, b0, a0, b2)?,
        prod(a0, b1, a1, b0)?,
    ];
    let norm_cross = Interval::norm(&cross)?;
    let na = Interval::norm(&[enclose(a0)?, enclose(a1)?, enclose(a2)?])?;
    let nb = Interval::norm(&[enclose(b0)?, enclose(b1)?, enclose(b2)?])?;
    let dot = enclose(a0)?
        .mul(enclose(b0)?)?
        .add(enclose(a1)?.mul(enclose(b1)?)?)?
        .add(enclose(a2)?.mul(enclose(b2)?)?)?;
    let bottom = dot.add(na.mul(nb)?)?;
    if bottom.lo() <= 0.0 {
        return Err(NumError::Degenerate);
    }
    norm_cross.div(bottom)
}

/// The limit a face's available width sets on the radius: the ball's contact stays on the face
/// while r · tan(φ / 2) is at most the width, so r is at most width / tan(φ / 2).
///
/// # Errors
///
/// [`NumError::Degenerate`] for a half-angle tangent that holds zero (a tangent edge needs no
/// blend, and has no such limit).
pub fn reach_limit(width: Interval, tan_half: Interval, open: bool) -> Result<Limit, NumError> {
    if tan_half.lo() <= 0.0 {
        return Err(NumError::Degenerate);
    }
    Ok(Limit {
        cause: Cause::FaceWidth,
        radius: width.div(tan_half)?,
        open,
    })
}

/// The limit a principal radius of curvature sets on the concave side: the ball cannot be larger
/// than the smallest radius of curvature of a support across the edge, and a radius at that bound
/// folds the face, so the bound is open.
#[must_use]
pub const fn fold_limit(principal_radius: Interval) -> Limit {
    Limit {
        cause: Cause::Fold,
        radius: principal_radius,
        open: true,
    }
}

/// The limit a slot sets: a ball fits between two faces up to half the width between them.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a width that is not positive.
pub fn room_limit(width: Interval) -> Result<Limit, NumError> {
    if width.lo() <= 0.0 {
        return Err(NumError::Degenerate);
    }
    let half = Interval::point(0.5)?.mul(width)?;
    Ok(Limit {
        cause: Cause::NoRoom,
        radius: half,
        open: false,
    })
}

/// The limit a collapsing offset sets: a ball as wide as the cylinder or sphere it rolls on, with
/// the ball inside, collapses that face's offset onto its axis or centre, so the support's radius is
/// an open bound.
#[must_use]
pub const fn collapse_limit(support_radius: Interval) -> Limit {
    Limit {
        cause: Cause::CollapsedOffset,
        radius: support_radius,
        open: true,
    }
}

/// The largest radius that certainly builds, from the lowest lower end among the limits; `None`
/// with no limits.
#[must_use]
pub fn largest(limits: &[Limit]) -> Option<Largest> {
    let mut best: Option<Largest> = None;
    for limit in limits {
        let radius = if limit.open {
            limit.radius.lo().next_down()
        } else {
            limit.radius.lo()
        };
        if best.is_none_or(|b| radius < b.radius) {
            best = Some(Largest {
                radius,
                cause: limit.cause,
                open: limit.open,
            });
        }
    }
    best
}

/// Answers a request for `radius`: feasible if no limit can be passed, too large with the radius
/// that fits if one is proven passed, undecided if one holds it.
#[must_use]
pub fn judge(limits: &[Limit], radius: f64) -> Verdict {
    let mut undecided: Option<Cause> = None;
    let mut passed = false;
    for limit in limits {
        let hi_ok = if limit.open {
            radius < limit.radius.lo()
        } else {
            radius <= limit.radius.lo()
        };
        if hi_ok {
            continue;
        }
        let proven_past = if limit.open {
            radius >= limit.radius.hi()
        } else {
            radius > limit.radius.hi()
        };
        if proven_past {
            passed = true;
        } else if undecided.is_none() {
            undecided = Some(limit.cause);
        }
    }
    if passed && let Some(fit) = largest(limits) {
        return Verdict::RadiusTooLarge(fit);
    }
    match undecided {
        Some(cause) => Verdict::Undecided(cause),
        None => Verdict::Feasible,
    }
}

/// The limits of several edges as one chain's: all of them, in order.
#[must_use]
pub fn chain_limits(edges: &[Vec<Limit>]) -> Vec<Limit> {
    edges.iter().flatten().copied().collect()
}
