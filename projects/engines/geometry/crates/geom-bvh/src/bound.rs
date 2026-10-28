//! The box tree's boxes: closed boxes of whole grid steps in two or three dimensions, padded by the
//! exactness of what they hold.
//!
//! What it does: an [`Aabb`] holds a lower and an upper corner of whole grid steps. It comes from
//! `geom-math`'s grid boxes exactly ([`Box2`], [`Box3`]), or from a box of doubles in grid units
//! rounded outward ([`Aabb::from_float`], [`FBox2`], [`FBox3`]), and [`Aabb::inflated`] pads it
//! by its holder's class (chapter 23.7): nothing for Exact geometry, the bound ε for Certified(ε),
//! the tolerance for a Tolerant import, each rounded up to whole steps. The tree measures a box by
//! [`Aabb::area`], the surface-area heuristic's measure in whole numbers.
//!
//! Guarantees:
//!
//! - **Never misses what it holds.** Grid boxes convert exactly. A float box's lower corner is
//!   rounded down and its upper corner up, each to a whole step, and a padding is the bound
//!   rounded up, so the result holds every point within the bound of the float box.
//! - **Bounded, so every cost is exact.** Every coordinate lies within ±[`LIMIT`] = 2^43 (the
//!   2^40 frame of chapter 6 with room for padding), so an extent plus one is at most
//!   2^44 + 1 < 2^45, a 3D area (three products of two) is below 3·2^90 < 2^92, and an area times
//!   an entry count (below 2^32) is below 2^124: the tree's costs are exact in `u128`.
//! - **Closed.** Boxes that touch overlap, as `geom-math`'s do.
//!
//! Method: chapter 23.7 of the kernel architecture ("Boxes that never miss", padding by
//! exactness) and section 19.9 ("Boxes are whole grid steps"); the area measure is the surface
//! area of I. Wald, "On fast construction of SAH-based bounding volume hierarchies", IEEE
//! Symposium on Interactive Ray Tracing (2007), <https://doi.org/10.1109/RT.2007.4342588>, counted
//! in grid points so a point's box has a positive measure, and in two dimensions the perimeter,
//! the plane's analogue (the chance a random line meets a convex region is proportional to its
//! perimeter).
//!
//! Cost: constant per box; no allocation.

use geom_math::{Box2, Box3, FBox2, FBox3};
use geom_num::math;
use geom_num::real::Real;

use crate::error::BvhError;

/// The largest coordinate magnitude a box may have: 2^43 grid steps, the 2^40 of a frame
/// (chapter 6) with room for paddings up to seven times the frame.
pub const LIMIT: i64 = 1 << 43;

/// How a box is padded: by the exactness of the geometry it holds (chapter 23.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Inflation {
    /// Exact geometry: no padding.
    Exact,
    /// Certified geometry: the true shape lies within this bound, in grid steps, of the stored one.
    Certified(Real),
    /// A Tolerant import: operations work within this tolerance, in grid steps.
    Tolerant(Real),
}

/// A closed axis-aligned box of whole grid steps, `lo ≤ hi` on every axis, in D = 2 or 3
/// dimensions, every coordinate within ±[`LIMIT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Aabb<const D: usize> {
    lo: [i64; D],
    hi: [i64; D],
}

/// Whole steps for a padding bound: the bound rounded up.
fn pad(bound: Real) -> Result<i64, BvhError> {
    // The bound itself, not its ceiling: −0.5 rounds up to −0, which is not below zero.
    if bound.get() < 0.0 {
        return Err(BvhError::BadBound);
    }
    whole(
        math::ceil(bound.get())
            .map_err(|_| BvhError::BadBound)?
            .get(),
    )
}

/// The measure of the box from `lo` to `hi` ([`Aabb::area`]), for corners of boxes within the
/// limits with `lo ≤ hi`. Each count of grid points h − l + 1 is at most 2^44 + 1 < 2^45, so it fits
/// a `u64` and the products and sums below (at most three products, each below 2^90) fit a `u128`:
/// the wrapping operations never wrap. Kept to plain products for speed: the build measures dozens
/// of candidate boxes per node.
pub(crate) fn measure<const D: usize>(lo: &[i64; D], hi: &[i64; D]) -> u128 {
    let mut e = [0u128; D];
    for ((slot, l), h) in e.iter_mut().zip(lo).zip(hi) {
        *slot = u128::from(u64::try_from(h.wrapping_sub(*l).wrapping_add(1)).unwrap_or(0));
    }
    match *e.as_slice() {
        [x, y] => x.wrapping_add(y),
        [x, y, z] => x
            .wrapping_mul(y)
            .wrapping_add(y.wrapping_mul(z))
            .wrapping_add(z.wrapping_mul(x)),
        _ => 0,
    }
}

/// A whole number within ±[`LIMIT`] as an `i64`, or `OutOfRange`.
fn whole(x: f64) -> Result<i64, BvhError> {
    // LIMIT = 2^43 is exact as a double, and every double of magnitude at most 2^43 that is a
    // whole number converts to i64 exactly.
    #[allow(
        clippy::cast_precision_loss,
        reason = "LIMIT = 2^43 is a power of two, exact as a double"
    )]
    let limit = LIMIT as f64;
    if !(-limit..=limit).contains(&x) {
        return Err(BvhError::OutOfRange);
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "x is a whole number of magnitude at most 2^43 (checked above), so the cast is exact"
    )]
    Ok(x as i64)
}

impl<const D: usize> Aabb<D> {
    /// The box with corners `lo` and `hi`.
    ///
    /// # Errors
    ///
    /// [`BvhError::Dimension`] unless D is 2 or 3; [`BvhError::OutOfRange`] for a coordinate
    /// beyond ±[`LIMIT`]; [`BvhError::Empty`] if `lo` is above `hi` on an axis.
    pub fn new(lo: [i64; D], hi: [i64; D]) -> Result<Aabb<D>, BvhError> {
        if D != 2 && D != 3 {
            return Err(BvhError::Dimension);
        }
        for (l, h) in lo.iter().zip(&hi) {
            if l.unsigned_abs() > LIMIT.unsigned_abs() || h.unsigned_abs() > LIMIT.unsigned_abs() {
                return Err(BvhError::OutOfRange);
            }
            if l > h {
                return Err(BvhError::Empty);
            }
        }
        Ok(Aabb { lo, hi })
    }

    /// The box of doubles in grid units from `lo` to `hi`, rounded outward to whole steps and
    /// padded by `by`.
    ///
    /// # Errors
    ///
    /// [`BvhError::BadBound`] for a coordinate that is not finite or a negative bound;
    /// [`BvhError::OutOfRange`] and [`BvhError::Empty`] as for [`Aabb::new`];
    /// [`BvhError::Dimension`] unless D is 2 or 3.
    pub fn from_float(lo: [f64; D], hi: [f64; D], by: Inflation) -> Result<Aabb<D>, BvhError> {
        let mut l = [0i64; D];
        let mut h = [0i64; D];
        for ((a, b), (x, y)) in l.iter_mut().zip(h.iter_mut()).zip(lo.iter().zip(&hi)) {
            if x > y {
                return Err(BvhError::Empty);
            }
            *a = whole(math::floor(*x).map_err(|_| BvhError::BadBound)?.get())?;
            *b = whole(math::ceil(*y).map_err(|_| BvhError::BadBound)?.get())?;
        }
        Aabb::new(l, h)?.inflated(by)
    }

    /// The lower corner.
    #[must_use]
    pub const fn lo(&self) -> [i64; D] {
        self.lo
    }

    /// The upper corner.
    #[must_use]
    pub const fn hi(&self) -> [i64; D] {
        self.hi
    }

    /// The box padded by `by` on every side, each padding the bound rounded up to whole steps.
    ///
    /// # Errors
    ///
    /// [`BvhError::BadBound`] for a negative bound; [`BvhError::OutOfRange`] if a padded
    /// coordinate passes ±[`LIMIT`].
    pub fn inflated(&self, by: Inflation) -> Result<Aabb<D>, BvhError> {
        let steps = match by {
            Inflation::Exact => return Ok(*self),
            Inflation::Certified(bound) | Inflation::Tolerant(bound) => pad(bound)?,
        };
        let mut lo = self.lo;
        let mut hi = self.hi;
        for (l, h) in lo.iter_mut().zip(hi.iter_mut()) {
            *l = l.checked_sub(steps).ok_or(BvhError::OutOfRange)?;
            *h = h.checked_add(steps).ok_or(BvhError::OutOfRange)?;
        }
        Aabb::new(lo, hi)
    }

    /// The smallest box holding both.
    #[must_use]
    pub fn union(&self, other: &Aabb<D>) -> Aabb<D> {
        let mut out = *self;
        for (((l, h), ol), oh) in out
            .lo
            .iter_mut()
            .zip(out.hi.iter_mut())
            .zip(&other.lo)
            .zip(&other.hi)
        {
            if ol < l {
                *l = *ol;
            }
            if oh > h {
                *h = *oh;
            }
        }
        out
    }

    /// Whether the boxes share a point (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &Aabb<D>) -> bool {
        self.lo
            .iter()
            .zip(&self.hi)
            .zip(other.lo.iter().zip(&other.hi))
            .all(|((l, h), (ol, oh))| l <= oh && ol <= h)
    }

    /// Whether this box holds every point of `other`.
    #[must_use]
    pub fn contains_box(&self, other: &Aabb<D>) -> bool {
        self.lo
            .iter()
            .zip(&self.hi)
            .zip(other.lo.iter().zip(&other.hi))
            .all(|((l, h), (ol, oh))| l <= ol && oh <= h)
    }

    /// The box's measure in the surface-area heuristic, in grid points: with e the number of grid
    /// points along each axis (extent plus one), e₀ + e₁ in 2D (half the perimeter) and
    /// e₀·e₁ + e₁·e₂ + e₂·e₀ in 3D (half the surface area). Exact (module documentation).
    #[must_use]
    pub fn area(&self) -> u128 {
        measure(&self.lo, &self.hi)
    }

    /// Twice the centre on `axis`: `lo + hi`, a whole number (|lo + hi| ≤ 2^44).
    pub(crate) fn centre2(&self, axis: usize) -> i64 {
        match (self.lo.get(axis), self.hi.get(axis)) {
            (Some(l), Some(h)) => l.saturating_add(*h),
            _ => 0,
        }
    }

    /// The box with these corners, which the caller has from boxes of this type (a union of
    /// them), so they are already within the limits and ordered.
    pub(crate) const fn from_corners(lo: [i64; D], hi: [i64; D]) -> Aabb<D> {
        Aabb { lo, hi }
    }
}

impl From<Box2> for Aabb<2> {
    fn from(b: Box2) -> Aabb<2> {
        let (lo, hi) = (b.min(), b.max());
        Aabb {
            lo: [lo.x.get(), lo.y.get()],
            hi: [hi.x.get(), hi.y.get()],
        }
    }
}

impl From<Box3> for Aabb<3> {
    fn from(b: Box3) -> Aabb<3> {
        let (lo, hi) = (b.min(), b.max());
        Aabb {
            lo: [lo.x.get(), lo.y.get(), lo.z.get()],
            hi: [hi.x.get(), hi.y.get(), hi.z.get()],
        }
    }
}

impl Aabb<2> {
    /// A float box of `geom-math`, rounded outward and padded by `by`.
    ///
    /// # Errors
    ///
    /// As [`Aabb::from_float`].
    pub fn from_fbox2(b: &FBox2, by: Inflation) -> Result<Aabb<2>, BvhError> {
        let (lo, hi) = (b.lo(), b.hi());
        Aabb::from_float([lo.x(), lo.y()], [hi.x(), hi.y()], by)
    }
}

impl Aabb<3> {
    /// A float box of `geom-math`, rounded outward and padded by `by`.
    ///
    /// # Errors
    ///
    /// As [`Aabb::from_float`].
    pub fn from_fbox3(b: &FBox3, by: Inflation) -> Result<Aabb<3>, BvhError> {
        let (lo, hi) = (b.lo(), b.hi());
        Aabb::from_float([lo.x(), lo.y(), lo.z()], [hi.x(), hi.y(), hi.z()], by)
    }
}
