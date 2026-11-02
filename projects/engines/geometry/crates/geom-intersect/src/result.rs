//! What an intersection returns: its items, how each pair meets, and how much of the answer is
//! proven (12.2).
//!
//! What it does: an [`Intersection`] is the items of one pair in canonical order, with its
//! [`Coverage`], its diagnostics and the work it took. An [`Item`] is a point, a curve, an overlap
//! or a flagged region, each with its enclosure on the operands' parameters and in space. A
//! [`ParamBox`] holds one interval per unknown of the pair, the canonical first operand's first,
//! so [`Intersection::swapped`] gives the same items with the roles changed.
//!
//! Guarantees:
//!
//! - **One order.** [`Intersection::new`] sorts the items by kind, then by their enclosure on the
//!   parameters: lower corner, then upper corner, coordinate by coordinate, compared exactly
//!   (12.15). Two items of one kind whose enclosures overlap are refused: distinct items are
//!   disjoint, so the order is total.
//! - **Nothing unproven is called certified.** [`Coverage::TopologyCertified`] with a flagged item
//!   is refused: a flagged region is what such a result does not have.
//! - **One shape of box.** Every item of an intersection has as many unknowns as the pair, and a
//!   [`ParamBox`] has two, three or four.
//! - **Flagged items carry no class.** The type gives them none (12.2).
//! - **Swapping twice is nothing.** [`Intersection::swapped`] moves the unknowns and keeps the
//!   items' order, the order of the canonical pair, whichever way round the caller asked.
//!
//! Method: chapter 12.2's result types and 12.15's output order
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: O(n log n) comparisons to order n items, then a sweep to prove them disjoint: for each
//! item, one comparison per later item of its kind that starts before it ends in the first unknown,
//! n²/2 at worst and each charged to the meter. Recursion: none.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_num::{Exactness, Steps};
use geom_run::{Usage, WorkMeter};

use crate::error::IntersectError;

/// The most unknowns a pair has: surface with surface.
const MAX_UNKNOWNS: usize = 4;

/// One interval per unknown of a pair: two for curve with curve (t₁, t₂), three for curve with
/// surface (t, u, v), four for surface with surface (u₁, v₁, u₂, v₂).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamBox {
    spans: [Interval; MAX_UNKNOWNS],
    unknowns: u8,
}

impl ParamBox {
    /// The box of `spans`.
    ///
    /// # Errors
    ///
    /// [`IntersectError::InvalidInput`] unless there are two, three or four spans.
    pub fn new(spans: &[Interval]) -> Result<ParamBox, IntersectError> {
        let filler = Interval::point(0.0).map_err(|_| IntersectError::InvalidInput)?;
        let mut held = [filler; MAX_UNKNOWNS];
        if !(2..=MAX_UNKNOWNS).contains(&spans.len()) {
            return Err(IntersectError::InvalidInput);
        }
        for (slot, span) in held.iter_mut().zip(spans) {
            *slot = *span;
        }
        let unknowns = u8::try_from(spans.len()).map_err(|_| IntersectError::InvalidInput)?;
        Ok(ParamBox {
            spans: held,
            unknowns,
        })
    }

    /// The intervals, one per unknown.
    #[must_use]
    pub fn spans(&self) -> &[Interval] {
        self.spans
            .get(..usize::from(self.unknowns))
            .unwrap_or(&self.spans)
    }

    /// How many unknowns the box has.
    #[must_use]
    pub const fn unknowns(&self) -> u8 {
        self.unknowns
    }

    /// The box with the operands' roles changed: `first` unknowns moved behind the rest.
    fn swapped(&self, first: u8) -> ParamBox {
        let mut spans = self.spans;
        if let Some(moved) = spans.get_mut(..usize::from(self.unknowns)) {
            moved.rotate_left(usize::from(first).min(moved.len()));
        }
        ParamBox {
            spans,
            unknowns: self.unknowns,
        }
    }

    /// The order of 12.15: lower corner, then upper corner, coordinate by coordinate.
    fn order(&self, other: &ParamBox) -> Ordering {
        let corner = |pick: fn(&Interval) -> f64| {
            self.spans()
                .iter()
                .zip(other.spans())
                .map(|(a, b)| pick(a).total_cmp(&pick(b)))
                .find(|o| o.is_ne())
                .unwrap_or(Ordering::Equal)
        };
        corner(|i| i.lo()).then_with(|| corner(|i| i.hi()))
    }

    /// Whether the two boxes share a point: in every unknown their intervals meet.
    fn meets(&self, other: &ParamBox) -> bool {
        self.spans()
            .iter()
            .zip(other.spans())
            .all(|(a, b)| a.lo() <= b.hi() && b.lo() <= a.hi())
    }

    /// Whether this box starts, in the first unknown, at or before `other` ends there.
    fn starts_within(&self, other: &ParamBox) -> bool {
        self.spans().first().map(|i| i.lo()) <= other.spans().first().map(|i| i.hi())
    }
}

/// A range of lengths in grid steps, its lower end at most its upper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepsInterval {
    lo: Steps,
    hi: Steps,
}

impl StepsInterval {
    /// The range from `lo` to `hi`; `None` when `lo` is above `hi`.
    #[must_use]
    pub const fn new(lo: Steps, hi: Steps) -> Option<StepsInterval> {
        if lo.get() <= hi.get() {
            Some(StepsInterval { lo, hi })
        } else {
            None
        }
    }

    /// Its lower end.
    #[must_use]
    pub const fn lo(self) -> Steps {
        self.lo
    }

    /// Its upper end.
    #[must_use]
    pub const fn hi(self) -> Steps {
        self.hi
    }
}

/// How a pair meets at a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointContact {
    /// They cross at an angle.
    Crossing,
    /// They touch at one point without crossing, like a bowl resting on a plane.
    IsolatedTangent,
    /// They meet at one point only because both domains end there.
    BoundaryTouch,
    /// Several branches meet: a saddle cut by its tangent plane has four half-branches.
    Singular {
        /// How many half-branches.
        branches: u16,
    },
}

/// How a pair meets along a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveContact {
    /// They cross at an angle.
    Crossing,
    /// They touch along the curve: inside the band the true intersection is this curve, two
    /// crossing curves, or nothing.
    Tangent {
        /// The width of the band.
        band: Steps,
        /// The gap between the surfaces across it.
        gap: StepsInterval,
    },
}

/// What proves an overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coincidence {
    /// The defining data are one.
    Identical,
    /// The two lie within this distance.
    Within(Steps),
}

/// Which test could not decide at the floor of subdivision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    /// That nothing lies in the box.
    Exclusion,
    /// That no closed loop lies in the box.
    LoopFreedom,
    /// That one solution lies in the box.
    Uniqueness,
}

/// Where a shape is not smooth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Degeneracy {
    /// A crease of a field.
    Crease,
    /// A pole of a parametrisation.
    Pole,
    /// An extraordinary vertex.
    Extraordinary,
}

/// Why a region could not be decided (12.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unresolved {
    /// A tangent contact too wide to resolve.
    TangentBand {
        /// The band's width.
        width: Steps,
    },
    /// The contact is of higher order than the tests reach.
    HigherOrderContact,
    /// The shapes lie close together, neither proven one nor proven apart.
    NearCoincident {
        /// The gap between them.
        gap: StepsInterval,
    },
    /// The shapes pass close, neither proven touching nor proven apart.
    NearMiss {
        /// The gap between them.
        gap: StepsInterval,
    },
    /// The item's bound passes the ceiling.
    BoundExceeded {
        /// The bound it would need.
        needed: Steps,
    },
    /// A curve known by construction could not be proven the only one here.
    KnownCurveUnproven,
    /// A test could not decide at the floor.
    UndecidedAtFloor {
        /// The test.
        test: Test,
    },
    /// The shape is not smooth here.
    NonSmooth {
        /// How.
        at: Degeneracy,
    },
}

/// Why a curve ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveEnd {
    /// At a boundary of a parameter domain.
    Boundary,
    /// At a singular point.
    Singular,
    /// At a flagged region.
    Flagged,
    /// At a seam or knot line, where it continues (12.10).
    Seam,
    /// Nowhere: the curve is a closed loop.
    Closed,
}

/// The overlap of a pair: the boundary curves of the region both shapes share.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlapRegion {
    /// The curves bounding the region, in order.
    pub boundary: Vec<CurveKind<3>>,
}

/// One part of an intersection (12.2).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// The pair meets at a point.
    Point {
        /// The enclosure on the parameters.
        at: ParamBox,
        /// The enclosure in space.
        xyz: Box3,
        /// How they meet.
        contact: PointContact,
        /// The item's class.
        class: Exactness,
    },
    /// The pair meets along a curve. A result with a closed form is its own kind, never wrapped.
    Curve {
        /// The enclosure on the parameters, which orders the item.
        at: ParamBox,
        /// The curve, boxed since a curve is far larger than a point.
        curve: Box<CurveKind<3>>,
        /// How they meet.
        contact: CurveContact,
        /// Why it ends, at its start and at its end.
        ends: [CurveEnd; 2],
        /// The item's class.
        class: Exactness,
    },
    /// The pair shares a region.
    Overlap {
        /// The enclosure on the parameters, which orders the item.
        at: ParamBox,
        /// The region.
        region: OverlapRegion,
        /// What proves it.
        proof: Coincidence,
        /// The item's class.
        class: Exactness,
    },
    /// Everything outside is proven; the answer in here is undecided. It asserts nothing else.
    Flagged {
        /// The enclosure on the parameters.
        at: ParamBox,
        /// The enclosure in space.
        xyz: Box3,
        /// Why.
        why: Unresolved,
    },
}

impl Item {
    /// The item's kind, in the order of 12.15.
    const fn rank(&self) -> u8 {
        match self {
            Item::Point { .. } => 0,
            Item::Curve { .. } => 1,
            Item::Overlap { .. } => 2,
            Item::Flagged { .. } => 3,
        }
    }

    /// The enclosure on the parameters.
    #[must_use]
    pub const fn at(&self) -> &ParamBox {
        match self {
            Item::Point { at, .. }
            | Item::Curve { at, .. }
            | Item::Overlap { at, .. }
            | Item::Flagged { at, .. } => at,
        }
    }

    /// The same item with the operands' roles changed.
    fn swapped(mut self, first: u8) -> Item {
        match &mut self {
            Item::Point { at, .. }
            | Item::Curve { at, .. }
            | Item::Overlap { at, .. }
            | Item::Flagged { at, .. } => *at = at.swapped(first),
        }
        self
    }

    /// The order of 12.15: by kind, then by enclosure.
    fn order(&self, other: &Item) -> Ordering {
        self.rank()
            .cmp(&other.rank())
            .then_with(|| self.at().order(other.at()))
    }
}

/// How much of an answer is proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    /// Every component outside the flagged regions is found.
    CompleteOutsideFlagged,
    /// The topology of the whole answer is proven: nothing is flagged.
    TopologyCertified,
}

/// What a method reports about how it got its answer: a route change, a tie broken, an escalation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// Its code.
    pub code: String,
}

/// The answer for one pair (12.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Intersection {
    items: Vec<Item>,
    coverage: Coverage,
    diagnostics: Vec<Diag>,
    work: Usage,
}

impl Intersection {
    /// The intersection of `items`, put in canonical order. Its work is what `meter` has counted
    /// when the order is proven, so a method makes this last.
    ///
    /// # Errors
    ///
    /// [`IntersectError::Internal`] for a broken invariant, which is a bug in the method that made
    /// them: two items of one kind whose enclosures meet, items with different numbers of unknowns,
    /// or a certified topology with a flagged region. The meter's stop, as
    /// [`IntersectError::Cancelled`] or [`IntersectError::BudgetExceeded`].
    pub fn new(
        mut items: Vec<Item>,
        coverage: Coverage,
        diagnostics: Vec<Diag>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Intersection, IntersectError> {
        meter.charge(u64::try_from(items.len()).unwrap_or(u64::MAX))?;
        items.sort_unstable_by(Item::order);
        let unknowns = items.first().map(|i| i.at().unknowns());
        let uneven = items.iter().any(|i| Some(i.at().unknowns()) != unknowns);
        let flagged = items.iter().any(|i| matches!(i, Item::Flagged { .. }));
        if uneven || (flagged && coverage == Coverage::TopologyCertified) {
            return Err(IntersectError::Internal);
        }
        // Neighbours are not enough: two boxes can overlap with a third sorted between them. So
        // each item meets every later one of its kind that starts before it ends in the first
        // unknown, which the sort makes a run.
        for (i, item) in items.iter().enumerate() {
            let later = items.get(i.saturating_add(1)..).unwrap_or_default();
            for other in later
                .iter()
                .take_while(|o| o.rank() == item.rank() && o.at().starts_within(item.at()))
            {
                meter.charge(1)?;
                if item.at().meets(other.at()) {
                    return Err(IntersectError::Internal);
                }
            }
        }
        Ok(Intersection {
            items,
            coverage,
            diagnostics,
            work: meter.used(),
        })
    }

    /// The items, in canonical order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// How much of the answer is proven.
    #[must_use]
    pub const fn coverage(&self) -> Coverage {
        self.coverage
    }

    /// Route changes, ties broken and escalations.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diag] {
        &self.diagnostics
    }

    /// The deterministic count of the work done.
    #[must_use]
    pub const fn work(&self) -> Usage {
        self.work
    }

    /// The same answer with the operands' roles changed: `first` is how many unknowns the first
    /// operand has. The items keep their order, which is the canonical pair's.
    #[must_use]
    pub fn swapped(mut self, first: u8) -> Intersection {
        self.items = self.items.into_iter().map(|i| i.swapped(first)).collect();
        self
    }
}
