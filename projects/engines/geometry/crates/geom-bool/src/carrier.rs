//! What the curved builder stores: where a point or a curve is, and which carriers say so (11.4,
//! G242).
//!
//! What it does: [`PointGeom`] and [`CurveGeom`] hold exact and certified geometry side by side,
//! and every entity says which it is. A point is a committed grid point (exact), the meeting of
//! three planes of the plane table (exact, implicit), or a grid point proven within a bound of the
//! carriers it lies on (certified). A curve is the meeting of two planes (exact), a curve kind
//! stored as itself, or a procedural meeting of two surfaces with an optional cached fit. A
//! **carrier** ([`CarrierIx`]) is the surface a face lies on or the curve an edge lies on, named
//! by its index into the builder's pools; a boolean never moves one. [`SortedVec`] keeps a
//! certified point's carriers in one order, and [`Bound`] is a distance in whole grid steps.
//!
//! Guarantees:
//!
//! - **Every entity says which kind it is.** [`PointGeom::is_exact`] is true for a grid point and a
//!   plane meeting only; a certified point carries the carriers it was proven against and the
//!   bound it was proven within.
//! - **A set of carriers has one order.** [`SortedVec`] holds its items sorted and without
//!   repeats, whatever order they were given in, so two entities with the same carriers compare
//!   equal and a decision never depends on the order carriers were found.
//! - **A bound is a whole number of steps, rounded up.** [`Bound::of`] never rounds a distance
//!   down, and refuses a negative or infinite one.
//!
//! Method: the data model of chapter 11.4 (`projects/engines/geometry/architecture/11-booleans.md`),
//! whose `PointGeom` and `CurveGeom` these are, with the carriers' indices those of the plane table
//! of `geom_poly::plane` and the surface and curve pools of `geom_topo`.
//!
//! Cost: constant per entity; a carrier set costs its length. No recursion.

use alloc::vec::Vec;

use geom_num::{P3, Steps};
use geom_poly::plane::PlaneIx;
use geom_topo::{CurveIx, SurfaceIx};

/// The index of a cached fit of a procedural curve, in the builder's fit pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FitIx(pub u32);

/// A carrier: the surface a face lies on, or the curve an edge lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarrierIx {
    /// A plane of the plane table: exact.
    Plane(PlaneIx),
    /// A surface of the pool.
    Surface(SurfaceIx),
    /// A curve of the pool.
    Curve(CurveIx),
}

/// Items in one order and without repeats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortedVec<T: Ord>(Vec<T>);

impl<T: Ord> Default for SortedVec<T> {
    fn default() -> Self {
        SortedVec(Vec::new())
    }
}

impl<T: Ord + Clone> SortedVec<T> {
    /// The items given, sorted and without repeats.
    #[must_use]
    pub fn from_items(items: impl IntoIterator<Item = T>) -> Self {
        let mut v: Vec<T> = items.into_iter().collect();
        v.sort_unstable();
        v.dedup();
        SortedVec(v)
    }

    /// The items, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// How many items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether `item` is held.
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.binary_search(item).is_ok()
    }

    /// The items of both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self::from_items(self.0.iter().chain(other.0.iter()).cloned())
    }
}

/// A distance in whole grid steps, rounded up: how far a certified entity is proven to lie from
/// its carriers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Bound(pub Steps);

impl Bound {
    /// No distance: an exact entity.
    pub const ZERO: Bound = Bound(Steps(0));

    /// The least bound of whole steps that is at least `distance`, or `None` for a distance that is
    /// negative, not a number, or too long for a count.
    #[must_use]
    pub fn of(distance: f64) -> Option<Bound> {
        if !(0.0..9.0e18).contains(&distance) {
            return None;
        }
        // `distance` is in 0..9e18, which `u64` holds; the truncation is rounded up by hand, since
        // `ceil` is not in `core`.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a number from 0 to 9e18 checked above, compared with its own truncation"
        )]
        let steps = {
            let floor = distance as u64;
            if (floor as f64) < distance {
                floor.saturating_add(1)
            } else {
                floor
            }
        };
        Some(Bound(Steps(steps)))
    }

    /// The bound as a length in steps.
    #[must_use]
    pub const fn steps(self) -> u64 {
        (self.0).0
    }
}

/// Where a point is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointGeom {
    /// A committed grid point: exact.
    Grid(P3),
    /// The meeting of three planes of the plane table: exact, implicit.
    Planes([PlaneIx; 3]),
    /// A grid point proven within `within` of where its carriers meet.
    Certified {
        /// The grid point.
        at: P3,
        /// The carriers it lies on, in one order.
        on: SortedVec<CarrierIx>,
        /// How far it is proven to lie from each of them.
        within: Bound,
    },
}

impl PointGeom {
    /// Whether the point is exact: a grid point, or the meeting of three planes.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        matches!(self, PointGeom::Grid(_) | PointGeom::Planes(_))
    }

    /// The carriers the point is proven against.
    #[must_use]
    pub fn carriers(&self) -> SortedVec<CarrierIx> {
        match self {
            PointGeom::Grid(_) => SortedVec::default(),
            PointGeom::Planes(planes) => {
                SortedVec::from_items(planes.iter().map(|ix| CarrierIx::Plane(*ix)))
            }
            PointGeom::Certified { on, .. } => on.clone(),
        }
    }
}

/// Where a curve is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveGeom {
    /// The meeting of two planes: exact.
    Line([PlaneIx; 2]),
    /// A circle, ellipse or other kind, stored as itself.
    Kind(CurveIx),
    /// A procedural intersection curve, defined by its two carriers.
    Meeting {
        /// The two surfaces.
        of: [SurfaceIx; 2],
        /// A cached fit that only speeds decisions: a decision taken on it carries the fit's bound.
        fit: Option<FitIx>,
    },
}

impl CurveGeom {
    /// Whether the curve is exact: the meeting of two planes.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        matches!(self, CurveGeom::Line(_))
    }
}
