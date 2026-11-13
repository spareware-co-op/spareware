//! How an item's bound adds up: the crate's own error, the operands' bounds and the angle at which
//! the shapes meet (12.2).
//!
//! What it does: [`settle`] gives an item the bound `ε_out` that [`Terms`] prove for it, and flags it
//! `BoundExceeded` if that passes the ceiling. An operand's bound is zero when the crate intersects
//! exact defining data, and a cache's certified error when it intersects a cache.
//!
//! Guarantees:
//!
//! - **Never a wider Certified item.** An item comes back Certified within the ceiling, or flagged
//!   with the bound it needed.
//! - **Rounds up.** `ε_out` is an upper bound of an interval sum, rounded up to whole steps.
//! - **Exact stays exact only on exact operands.** Any operand's bound makes an Exact item Certified.
//! - **Approximate stays Approximate.** Its deviation grows by the same terms, and proves nothing,
//!   so the ceiling does not judge it.
//! - **No angle, no bound.** Where the angle cannot carry the operands' bounds through, as it cannot
//!   near tangency, the item is flagged with a bound of `u64::MAX` steps. 12.9's square root is the
//!   tangent case's bound, and that is [`crate::contact`]'s.
//!
//! Method: chapter 12.2 (`projects/engines/geometry/architecture/12-intersections.md`). Two shapes
//! moved by `ε_A` and `ε_B`, whose sum is e, meeting at an angle whose sine is at least s, move their
//! crossing by at most r = e/s to first order. Over a distance r the normals of shapes of curvature
//! at most κ each turn by at most κ·r, so the angle there is within 2κ·r of θ, and sine being
//! 1-Lipschitz, r ≤ e/(s − 2κ·r). Take r₀ = 2e/s. If s − 2κ·r₀ ≥ s/2, that map takes [0, r₀] into
//! itself, so the crossing lies within e/(s − 2κ·r₀) of where it was. That quotient is both
//! terms of `ε_out` at once: e/s, and the second-order rest. If not, the curvature can undo the angle
//! inside the distance moved, and no bound of this form exists.
//!
//! What this piece leaves: the methods know their `θ_min` and curvature at the end of a march or a
//! solve, and the pipeline that hands an item its [`Terms`] comes with the operations that intersect
//! caches. Exact data, which every method here takes today, needs nothing added.
//!
//! Cost: constant. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.2's: e the operands' sum, s the sine, r the distance moved"
)]

use geom_math::Box3;
use geom_num::grid::{GridStep, Steps};
use geom_num::interval::Interval;
use geom_num::{Exactness, NumError};

use crate::IntersectError;
use crate::curve::exact_steps;
use crate::result::{Item, ParamBox, Unresolved};

/// What a method proved about an item, for its bound.
#[derive(Clone, Debug, PartialEq)]
pub struct Terms {
    /// What each operand may be from the true shape.
    pub operands: [Steps; 2],
    /// A proven lower bound on the sine of the angle at which the shapes meet over the item.
    pub sine: f64,
    /// A proven upper bound on either shape's curvature over the item, in turns of the normal per
    /// step: 1/r for a sphere of radius r steps.
    pub curvature: f64,
    /// The grid step an Exact item's bound will count.
    pub step: GridStep,
    /// The item's enclosure in space, which a flagged item keeps.
    pub xyz: Box3,
}

/// A count of steps as an interval: below 2^53 a point, and past it the doubles either side.
#[expect(
    clippy::cast_precision_loss,
    reason = "the interval's ends hold what the cast rounds"
)]
fn count(steps: u64) -> Result<Interval, NumError> {
    let near = steps as f64;
    Interval::new(near * (1.0 - f64::EPSILON), near * (1.0 + f64::EPSILON))
}

impl Terms {
    /// How far the operands' bounds can move the item, or `None` where the angle cannot carry them.
    fn moved(&self) -> Result<Option<Interval>, NumError> {
        let [a, b] = self.operands;
        let e = count(a.get().saturating_add(b.get()))?;
        if e.hi() <= 0.0 {
            return Ok(Some(e));
        }
        if self.sine <= 0.0 {
            return Ok(None);
        }
        let (s, kappa) = (
            Interval::point(self.sine)?,
            Interval::point(self.curvature)?,
        );
        let two = Interval::point(2.0)?;
        let room = two.mul(kappa)?.mul(two.mul(e)?.div(s)?)?;
        let left = s.sub(room)?;
        if left.lo() * 2.0 <= self.sine {
            return Ok(None);
        }
        Ok(Some(e.div(left)?))
    }
}

/// `item`'s enclosure on the parameters.
fn place(item: &Item) -> ParamBox {
    match item {
        Item::Point { at, .. }
        | Item::Curve { at, .. }
        | Item::Overlap { at, .. }
        | Item::Flagged { at, .. } => *at,
    }
}

/// `item` with its bound added up, or flagged if that passes `ceiling`.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a sine outside 0 to 1, or a curvature that is negative or
/// not finite.
pub fn settle(mut item: Item, terms: &Terms, ceiling: Steps) -> Result<Item, IntersectError> {
    let sane = terms.sine >= 0.0 && terms.sine <= 1.0 && terms.curvature >= 0.0;
    if !sane || !terms.curvature.is_finite() {
        return Err(IntersectError::InvalidInput);
    }
    let at = place(&item);
    let (Item::Point { class, .. } | Item::Curve { class, .. } | Item::Overlap { class, .. }) =
        &mut item
    else {
        return Ok(item);
    };
    let own = match *class {
        Exactness::Exact => Steps::ZERO,
        Exactness::Certified { bound, .. } => bound,
        Exactness::Approximate { deviation, .. } => deviation,
    };
    let needed = match terms.moved()? {
        Some(by) if by.hi() <= 0.0 => own,
        Some(by) => Steps(exact_steps(count(own.get())?.add(by)?.hi())),
        None => Steps(u64::MAX),
    };
    if needed == Steps::ZERO {
        return Ok(item);
    }
    if let Exactness::Approximate { deviation, .. } = class {
        *deviation = needed;
        return Ok(item);
    }
    if needed > ceiling {
        return Ok(Item::Flagged {
            at,
            xyz: terms.xyz,
            why: Unresolved::BoundExceeded { needed },
        });
    }
    let step = match *class {
        Exactness::Certified { step, .. } => step,
        _ => terms.step,
    };
    *class = Exactness::Certified {
        bound: needed,
        step,
    };
    Ok(item)
}
