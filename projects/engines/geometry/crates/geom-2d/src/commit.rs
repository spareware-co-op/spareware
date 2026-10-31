//! Commits: an implicit point to the grid, only where that changes nothing.
//!
//! What it does: a constructed point stays its definition until it must become explicit (chapter
//! 6.3). [`commit`] rounds each coordinate to the nearest step, ties to even, decided exactly, and
//! then asks the rounded point every question the implicit one answered: its own carriers, and each
//! [`Decision`] the caller names. If every answer stands the point is [`Commit::Grid`]. If any would
//! change, [`Commit::Kept`] says which, and the definition stays. So a committed point is exactly on
//! its carriers, which is what makes a join one shared point.
//!
//! For output with no implicit form, [`explicit`] gives the rounded point anyway, as a Certified
//! result within the commit rung, and only if the caller's [`Accept`] takes that.
//!
//! Guarantees:
//!
//! - **Exactly on its carriers.** [`Commit::Grid`] is returned only after an exact test puts the grid
//!   point on both of the point's carriers; there is no tolerance.
//! - **Flips nothing.** Each [`Decision`] has the same sign on the committed point as was asked.
//! - **Rounds as 6.3 does.** Each coordinate goes to the nearest step, ties to even, by exact
//!   comparison with half steps, never by a floating-point round. A coordinate that rounds outside
//!   the frame is [`NumError::OutOfRange`], which is `FrameRangeExceeded`.
//! - **The definition survives.** Nothing is changed by a commit: [`Commit::Kept`] hands back the
//!   point it was given. A root of a quartic is always kept, its place needing no grid (14.2).
//!
//! Method: the commit of chapter 6.3 (`projects/engines/geometry/architecture/06-numbers.md`) and
//! 14.3's budgets and commits (`projects/engines/geometry/architecture/14-2d-drafting.md`).
//!
//! Cost: about 41 exact comparisons per coordinate, by bisection of the frame, most settled by a
//! `Crossing2` filter, then one exact sign per carrier and per decision. No allocation.

use geom_num::carrier::Rational2;
use geom_num::grid::{COORD_MAX, GridStep, P2};
use geom_num::ladder::COMMIT_DISTANCE_2D;
use geom_num::one_root::Crossing2;
use geom_num::real::Real;
use geom_num::{Exactness, NumError, Sign};

use crate::accept::Accept;
use crate::carrier::{Carriers, CurveIx};
use crate::point::{Place, PointError, Pt2};

/// A decision a commit must not change: the sign of the point against a curve, as
/// [`crate::carrier::Curve2::side`] gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decision {
    /// The curve.
    pub curve: CurveIx,
    /// The sign the point has.
    pub sign: Sign,
}

/// Why a point stays implicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kept {
    /// The rounded point would be off this carrier of the point's.
    OffCarrier(CurveIx),
    /// The rounded point would change this decision.
    Flips(Decision),
    /// A root of a quartic has no rounded form that could be on both its carriers by construction.
    ConicRoot,
}

/// What a commit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Commit {
    /// The point is this grid point, exactly on its carriers, and no decision moved.
    Grid(P2),
    /// The point stays as it was.
    Kept {
        /// The point.
        point: Pt2,
        /// Why.
        why: Kept,
    },
}

/// Half a step past `n`, as a point on the axis `axis`.
fn half_past(n: i64, axis: usize) -> Result<Crossing2, NumError> {
    let twice = i128::from(n).saturating_mul(2).saturating_add(1);
    let z = if axis == 0 {
        Rational2::new(twice, 0, 2)?
    } else {
        Rational2::new(0, twice, 2)?
    };
    Crossing2::of_point(&z)
}

/// How the coordinate `axis` of `z` compares with that of `other`.
fn compared(z: &Crossing2, other: &Crossing2, axis: usize) -> Result<Sign, NumError> {
    if axis == 0 {
        z.cmp_x(other)
    } else {
        z.cmp_y(other)
    }
}

/// The nearest step to coordinate `axis` of `z`, ties to even. Bisects the frame for the least n
/// with the coordinate at most n + 1/2; at exactly n + 1/2 the even one of n and n + 1 is nearest.
fn nearest(z: &Crossing2, axis: usize) -> Result<i64, NumError> {
    let (mut low, mut high) = (COORD_MAX.saturating_neg().saturating_sub(1), COORD_MAX);
    if compared(z, &half_past(low, axis)?, axis)? == Sign::Negative
        || compared(z, &half_past(high, axis)?, axis)? == Sign::Positive
    {
        return Err(NumError::OutOfRange {
            value: i128::from(high).saturating_add(1),
        });
    }
    // low + 1/2 is at or above the coordinate only at `low` itself, below which nothing is asked.
    while low < high {
        let middle = low.saturating_add(high.saturating_sub(low).div_euclid(2));
        if compared(z, &half_past(middle, axis)?, axis)? == Sign::Positive {
            low = middle.saturating_add(1);
        } else {
            high = middle;
        }
    }
    let tie = compared(z, &half_past(low, axis)?, axis)? == Sign::Zero;
    Ok(if tie && low.rem_euclid(2) != 0 {
        low.saturating_add(1)
    } else {
        low
    })
}

/// `z` rounded to the grid.
fn rounded(z: &Crossing2) -> Result<P2, NumError> {
    P2::new(nearest(z, 0)?, nearest(z, 1)?)
}

/// The carriers of `point`.
const fn carriers_of(point: &Pt2) -> Option<[CurveIx; 2]> {
    match point {
        Pt2::Crossing { a, b, .. } => Some([*a, *b]),
        _ => None,
    }
}

/// Commits `point` to the grid if that keeps it on its carriers and changes no decision.
///
/// # Errors
///
/// As for [`Pt2::place`], and [`NumError::OutOfRange`] for a coordinate that rounds outside the
/// frame.
pub fn commit(
    point: &Pt2,
    carriers: &Carriers,
    decisions: &[Decision],
) -> Result<Commit, PointError> {
    let z = match point.place(carriers)? {
        Place::OneRoot(z) => z,
        Place::Conic { .. } => {
            return Ok(Commit::Kept {
                point: *point,
                why: Kept::ConicRoot,
            });
        }
    };
    let at = rounded(&z)?;
    let grid = Pt2::Grid(at);
    for on in carriers_of(point).into_iter().flatten() {
        if !grid.on(carriers, on)? {
            return Ok(Commit::Kept {
                point: *point,
                why: Kept::OffCarrier(on),
            });
        }
    }
    let here: Rational2 = at.into();
    for decision in decisions {
        let curve = carriers
            .get(decision.curve)
            .ok_or(PointError::NoCarrier(decision.curve))?;
        if curve.side(&here)? != decision.sign {
            return Ok(Commit::Kept {
                point: *point,
                why: Kept::Flips(*decision),
            });
        }
    }
    Ok(Commit::Grid(at))
}

/// `point` on the grid whether or not that keeps it on its carriers, for output that has no implicit
/// form: Exact if it already is a grid point, otherwise Certified within the commit rung of the
/// frame's `step`, if `accept` takes that.
///
/// # Errors
///
/// [`PointError::ExactnessNotAccepted`] if `accept` does not; as for [`commit`] otherwise, and
/// [`PointError::WrongClass`] for a root of a quartic, whose place has no exact form to round.
pub fn explicit(
    point: &Pt2,
    carriers: &Carriers,
    step: GridStep,
    accept: &Accept,
) -> Result<(P2, Exactness), PointError> {
    let Place::OneRoot(z) = point.place(carriers)? else {
        return Err(PointError::WrongClass);
    };
    let at = rounded(&z)?;
    let reached = if z.same(&Crossing2::of_point(&at.into())?)? {
        Exactness::Exact
    } else {
        Exactness::Certified {
            bound: COMMIT_DISTANCE_2D,
            step,
        }
    };
    Ok((at, accept.admit(reached, Real::ZERO)?))
}
