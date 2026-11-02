//! Contacts between two curves of the plane, for the kinds the exact predicates do not cover:
//! splines, spirals and procedural offsets (chapter 14.2 and 14.4; piece G195).
//!
//! What it does: [`contacts`] asks `geom_intersect::curve::curves` where two curves meet. That
//! search is certified, closed form where one exists and otherwise subdivision and Krawczyk
//! (chapter 12.5). It reads each item it gives as a [`Contact`]: where on each curve, where in the
//! plane, and of which [`Kind`]:
//!
//! - **Crossing:** a point the search proves the two cross at, transversally.
//! - **Touching:** a point where they meet without crossing, proven tangent or one's end lying on
//!   the other; or a stretch they share.
//! - **Undecided:** a region the search could not settle, with its reason. That is a band too
//!   tangent to tell, a contact of higher order, a near miss or near coincidence below the floor,
//!   or a singular point whose branches it does not sort.
//!
//! [`Contact::cuts`] and [`Contact::closes`] are the only ways a trim or a boundary walk reads a
//! contact. Neither takes an Undecided one, so no undecided contact ever cuts a trim or closes a
//! boundary (14.4).
//!
//! Guarantees:
//!
//! - **Undecided is never either.** A contact is Crossing or Touching only where the search proved
//!   it so. Anything flagged, and a singular point, is Undecided, and [`Contact::cuts`] and
//!   [`Contact::closes`] refuse it.
//! - **Nothing dropped.** Every item the search gives becomes one contact, in its order (12.15),
//!   so the list accounts for every place the two curves can meet.
//! - **Deterministic.** The search is, and so is this reading.
//!
//! Method: chapter 14.2's rule for spline and spiral crossings and 14.4's for contacts that cannot
//! be ordered (`projects/engines/geometry/architecture/14-2d-drafting.md`), on chapter 12.5's
//! certified curve-curve intersection (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: the search's (12.5), and one pass over its items. Recursion: none.

use alloc::vec::Vec;

use geom_curve::CurveKind;
use geom_intersect::IntersectError;
use geom_intersect::curve::curves;
use geom_intersect::result::{Item, PointContact, Unresolved};
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_run::WorkMeter;

/// Why a contact is undecided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    /// The search flagged the region (12.5).
    Unresolved(Unresolved),
    /// The curves meet at a point where one of them is singular, and the search does not sort
    /// its branches into crossing and touching.
    Singular {
        /// The branches through the point.
        branches: u16,
    },
}

/// How two curves meet at a contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// They cross, transversally.
    Crossing,
    /// They meet and do not cross.
    Touching,
    /// The search could not tell.
    Undecided(Why),
}

/// One place where two curves meet, or may.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// How they meet.
    pub kind: Kind,
    /// Its parameters on the first curve.
    pub first: Interval,
    /// Its parameters on the second.
    pub second: Interval,
    /// Its place in the plane, where the search gives one: a point's or a flagged region's box.
    pub place: Option<Box3>,
}

impl Contact {
    /// Where a trim may cut the first and second curves: a crossing's or a touching's parameters;
    /// `None` for an undecided contact, which never cuts a trim.
    #[must_use]
    pub const fn cuts(&self) -> Option<(Interval, Interval)> {
        match self.kind {
            Kind::Crossing | Kind::Touching => Some((self.first, self.second)),
            Kind::Undecided(_) => None,
        }
    }

    /// Whether a boundary walk may close a loop through it: only at a decided contact.
    #[must_use]
    pub const fn closes(&self) -> bool {
        !matches!(self.kind, Kind::Undecided(_))
    }
}

/// The parameters of a search's item on each curve; `None` for a box that is not two spans.
fn spans(item: &Item) -> Option<(Interval, Interval)> {
    match item.at().spans() {
        [s, t] => Some((*s, *t)),
        _ => None,
    }
}

/// Every place where `a` and `b` meet, or may, in the search's order (module documentation).
///
/// # Errors
///
/// The search's errors: a kind's, and the meter's stop, which an unlimited meter never gives.
pub fn contacts(
    a: &CurveKind<2>,
    b: &CurveKind<2>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Contact>, IntersectError> {
    let found = curves(a, b, meter)?;
    let mut out = Vec::with_capacity(found.items().len());
    for item in found.items() {
        let (first, second) = spans(item).ok_or(IntersectError::InvalidInput)?;
        let (kind, place) = match item {
            Item::Point { contact, xyz, .. } => (
                match contact {
                    PointContact::Crossing => Kind::Crossing,
                    PointContact::IsolatedTangent | PointContact::BoundaryTouch => Kind::Touching,
                    PointContact::Singular { branches } => Kind::Undecided(Why::Singular {
                        branches: *branches,
                    }),
                },
                Some(*xyz),
            ),
            // A stretch the two share: they meet along it and cross nowhere inside it.
            Item::Curve { .. } | Item::Overlap { .. } => (Kind::Touching, None),
            Item::Flagged { xyz, why, .. } => (Kind::Undecided(Why::Unresolved(*why)), Some(*xyz)),
        };
        out.push(Contact {
            kind,
            first,
            second,
            place,
        });
    }
    Ok(out)
}
