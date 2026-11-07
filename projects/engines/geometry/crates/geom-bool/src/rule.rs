//! The contact rule at `ε_fit`, and the two errors that stand for a decision it cannot take (11.4,
//! 11.11, G242).
//!
//! What it does: a certified decision computes an **enclosure**, an interval proven to hold the
//! true value, and [`judge`] reads it. It excludes zero: the question is decided, with the sign.
//! It holds zero and its largest magnitude is strictly below `ε_fit` ([`EPS_FIT`]): the entities
//! **touch** and become one. Otherwise the decision is **near** and the operation fails.
//! [`decide`] refines an enclosure up to a fixed limit ([`REFINEMENTS`]) before it reads the last:
//! an entity that can be proven apart stays apart, however close. [`NearContact`] carries the
//! enclosure as evidence and [`CoincidenceChain`] the width a merge needed, each with its remedies
//! ([`Remedy`]).
//!
//! Guarantees:
//!
//! - **Touch only when proven close and not proven apart.** [`Verdict::Touch`] needs an enclosure
//!   that holds zero at the finest refinement and whose largest magnitude is strictly below `ε_fit`;
//!   at exactly `ε_fit` it is [`Verdict::Near`].
//! - **Apart wins whenever it can be proven.** [`decide`] always runs every refinement until one
//!   excludes zero, so no touch is declared that a finer enclosure would have refuted.
//! - **A fixed amount of work.** [`decide`] calls its measure at most [`REFINEMENTS`] plus one
//!   times, whatever the numbers, so a budget runs out at the same stage on every machine.
//! - **Never a guess.** A decision neither decided nor touching is an error with evidence, not a
//!   rounded answer; `ε_fit` is the ladder's fixed value, never stored per entity and never grown.
//!
//! Method: chapter 11.4 of the kernel architecture
//! (`projects/engines/geometry/architecture/11-booleans.md`), "Certified decisions and the one
//! contact radius", and 11.11 for the errors and their remedies. The contact radius is
//! `geom_num::ladder::FIT`.
//!
//! Cost: constant outside the measure. No allocation. Recursion: none.

use core::fmt;

use geom_intersect::IntersectError;
use geom_num::interval::Interval;
use geom_num::sign::Sign;
use geom_num::{NumError, Steps, ladder};

use crate::carrier::{Bound, CarrierIx};

/// The lesser of two numbers that are not NaN.
pub(crate) fn lesser(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// The greater of two numbers that are not NaN.
pub(crate) fn greater(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// The contact radius, in steps: the ladder's fit tolerance.
pub const EPS_FIT: Steps = ladder::FIT;

/// How many times a decision is refined after its first enclosure, before it is read.
pub const REFINEMENTS: u32 = 8;

/// What an enclosure says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The enclosure excludes zero: decided, with the sign of the value.
    Decided(Sign),
    /// It holds zero and every value in it is strictly below `ε_fit` in size: the entities touch.
    Touch,
    /// It holds zero and is not within `ε_fit`: undecided.
    Near,
}

/// Reads one enclosure of a signed value against the contact radius.
#[must_use]
pub fn judge(enclosure: Interval) -> Verdict {
    if enclosure.lo() > 0.0 {
        return Verdict::Decided(Sign::Positive);
    }
    if enclosure.hi() < 0.0 {
        return Verdict::Decided(Sign::Negative);
    }
    let largest = greater(enclosure.lo().abs(), enclosure.hi().abs());
    #[allow(
        clippy::cast_precision_loss,
        reason = "`ε_fit` is a count of 64 steps, which a double holds exactly"
    )]
    let radius = EPS_FIT.0 as f64;
    if largest < radius {
        Verdict::Touch
    } else {
        Verdict::Near
    }
}

/// What a decision came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    /// Proven apart, or proven to lie on one side, with the sign.
    Apart(Sign),
    /// They touch: the finest enclosure held zero and lay within `ε_fit`.
    Touch(Interval),
}

/// What to do about an error (11.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remedy {
    /// Move one operand by at least this many steps.
    MoveApart(Bound),
    /// Close the gap first with `close_near_contacts`, to this width.
    CloseNearContacts(Bound),
    /// Take the mesh path, Certified within its tessellation bound.
    MeshPath,
}

/// A curved decision still undecided at the refinement limit and wider than the contact radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearContact {
    /// The finest enclosure reached.
    pub enclosure: Interval,
    /// The refinement it was reached at.
    pub refinement: u32,
}

impl NearContact {
    /// The remedies offered: move apart by the enclosure's width, close the gap to it, or take the
    /// mesh path.
    #[must_use]
    pub fn remedies(&self) -> [Remedy; 3] {
        let width = Bound::of(greater(
            self.enclosure.hi().abs(),
            self.enclosure.lo().abs(),
        ))
        .unwrap_or(Bound(EPS_FIT));
        [
            Remedy::MoveApart(width),
            Remedy::CloseNearContacts(width),
            Remedy::MeshPath,
        ]
    }
}

/// A merge whose representative cannot be proven within the remaining bound of all its carriers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoincidenceChain {
    /// The carrier the representative is too far from.
    pub carrier: CarrierIx,
    /// The enclosure of its distance from that carrier.
    pub distance: Interval,
    /// The bound it had to stay within.
    pub remaining: Bound,
}

impl CoincidenceChain {
    /// The remedies offered: `close_near_contacts` with the reported width, or move apart.
    #[must_use]
    pub fn remedies(&self) -> [Remedy; 2] {
        let width = Bound::of(self.distance.hi()).unwrap_or(self.remaining);
        [Remedy::CloseNearContacts(width), Remedy::MoveApart(width)]
    }
}

/// Why a curved decision or merge could not be taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactError {
    /// A decision stayed undecided and wider than the contact radius.
    NearContact(NearContact),
    /// A merge's representative is not within the remaining bound of all its carriers.
    CoincidenceChain(CoincidenceChain),
    /// A certified intersection or projection stopped.
    Intersect(IntersectError),
    /// The numbers layer refused.
    Num(NumError),
}

impl fmt::Display for ContactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContactError::NearContact(_) => f.write_str(
                "a decision is undecided at the refinement limit and wider than the contact radius",
            ),
            ContactError::CoincidenceChain(_) => f.write_str(
                "a merge's representative is not within the remaining bound of all its carriers",
            ),
            ContactError::Intersect(e) => e.fmt(f),
            ContactError::Num(e) => e.fmt(f),
        }
    }
}

impl core::error::Error for ContactError {}

impl From<IntersectError> for ContactError {
    fn from(e: IntersectError) -> Self {
        ContactError::Intersect(e)
    }
}

impl From<NumError> for ContactError {
    fn from(e: NumError) -> Self {
        ContactError::Num(e)
    }
}

/// Takes a certified decision: calls `measure` at refinement 0, 1, ... up to [`REFINEMENTS`], and
/// stops at the first enclosure that excludes zero. If none does, the finest one is read: touching
/// if it lies within `ε_fit`, an error otherwise.
///
/// # Errors
///
/// [`ContactError::NearContact`] with the finest enclosure when it holds zero and is not within
/// `ε_fit`; whatever `measure` returns.
pub fn decide(
    mut measure: impl FnMut(u32) -> Result<Interval, ContactError>,
) -> Result<Contact, ContactError> {
    let mut finest = measure(0)?;
    for refinement in 0..=REFINEMENTS {
        if refinement > 0 {
            finest = measure(refinement)?;
        }
        if let Verdict::Decided(sign) = judge(finest) {
            return Ok(Contact::Apart(sign));
        }
    }
    match judge(finest) {
        Verdict::Touch => Ok(Contact::Touch(finest)),
        Verdict::Near | Verdict::Decided(_) => Err(ContactError::NearContact(NearContact {
            enclosure: finest,
            refinement: REFINEMENTS,
        })),
    }
}
