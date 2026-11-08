//! A query region as it moves through frames and placements: a box whose corners are rationals
//! over one denominator, so a region moved into a coarser frame stays exact (19.12; piece G295).
//!
//! What it does: a [`Region`] is the box of reals from `lo / den` to `hi / den` on each axis, in
//! some frame's steps. A region given in grid steps has `den = 1`. Moved into a frame `refine`
//! times coarser, its corners divide by `refine`, which the denominator keeps exactly instead of
//! rounding; moved into a finer frame, they multiply, and the denominator reduces again. So a
//! region is never rounded on its way down an assembly, and [`Region::meets`] decides exactly
//! whether an integer box shares a point with it.
//!
//! Guarantees:
//!
//! - **Exact.** Every operation here is integer arithmetic on the numerators and the denominator;
//!   [`Region::meets`], [`Region::clip`] and [`Region::unplace`] give exactly what the real box
//!   gives.
//! - **Small denominators.** Frames' refinements are 1, 25 and 625, so the denominator is a power
//!   of 5. After each step the region is reduced by every factor 5 that divides the denominator and
//!   all six numerators, so the denominator is 25 to the power of how many rungs the frame is
//!   coarser than the frame the region was given in: at most 625 (chapter 6.1's three steps).
//!   Numerators then stay below 2^62 in magnitude, since every region is kept within a
//!   definition's box (below 2^43 steps) before it moves on; a step that would leave `i128`
//!   anyway is refused, never wrapped.
//!
//! Method: exact rational arithmetic with one common denominator, as chapter 6.1 converts between
//! frames (`projects/engines/geometry/architecture/06-numbers.md`): a finer step divides every
//! coarser one exactly, so a region converts into a coarser frame by a division the denominator
//! records, and back by a multiplication.
//!
//! Cost: a few `i128` operations per axis for each step; the reduction at most 8 divisions by 5.
//! Nothing allocates; no recursion.

use geom_bvh::Aabb;
use geom_math::{Box3, GridTransform};
use geom_num::NumError;
use geom_num::grid::{Coord, P3};

use super::error::InstanceError;

/// A box of reals with corners `lo / den` and `hi / den` (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    lo: [i128; 3],
    hi: [i128; 3],
    den: i128,
}

/// A step that would leave `i128`, which the denominator's bound rules out for every region the
/// instance tree makes: refused as out of range, never wrapped.
pub(crate) fn wide(v: Option<i128>) -> Result<i128, InstanceError> {
    v.ok_or(InstanceError::Num(NumError::OutOfRange {
        value: i128::MAX,
    }))
}

/// The three coordinates of `p`, widened.
fn coords(p: P3) -> [i128; 3] {
    [p.x.get(), p.y.get(), p.z.get()].map(i128::from)
}

impl Region {
    /// The region of a grid box.
    #[must_use]
    pub fn from_box(b: &Box3) -> Region {
        Region {
            lo: coords(b.min()),
            hi: coords(b.max()),
            den: 1,
        }
    }

    /// A region from its numerators and denominator, reduced, or `None` if it holds no point.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Defect`] for a denominator that is not positive.
    pub(crate) fn new(
        lo: [i128; 3],
        hi: [i128; 3],
        den: i128,
    ) -> Result<Option<Region>, InstanceError> {
        if den < 1 {
            return Err(InstanceError::Defect);
        }
        if lo.iter().zip(&hi).any(|(l, h)| l > h) {
            return Ok(None);
        }
        let mut r = Region { lo, hi, den };
        // A power of 5 at most 625 at most 8 times over: the loop ends.
        while r.den.checked_rem(5) == Some(0)
            && r.lo
                .iter()
                .chain(&r.hi)
                .all(|v| v.checked_rem(5) == Some(0))
        {
            for v in r.lo.iter_mut().chain(r.hi.iter_mut()) {
                *v = wide(v.checked_div(5))?;
            }
            r.den = wide(r.den.checked_div(5))?;
        }
        Ok(Some(r))
    }

    /// The numerators of the lower corner.
    #[must_use]
    pub const fn lo(&self) -> [i128; 3] {
        self.lo
    }

    /// The numerators of the upper corner.
    #[must_use]
    pub const fn hi(&self) -> [i128; 3] {
        self.hi
    }

    /// The denominator: 1 for a region on the grid.
    #[must_use]
    pub const fn den(&self) -> i128 {
        self.den
    }

    /// Whether the integer box from `lo` to `hi` shares a point with the region.
    #[must_use]
    pub fn meets(&self, lo: [i64; 3], hi: [i64; 3]) -> bool {
        let scaled = |v: i64| i128::from(v).saturating_mul(self.den);
        self.lo
            .iter()
            .zip(&self.hi)
            .zip(lo.iter().zip(&hi))
            .all(|((rl, rh), (bl, bh))| scaled(*bl) <= *rh && *rl <= scaled(*bh))
    }

    /// Whether the box tree's box `b` shares a point with the region.
    #[must_use]
    pub fn meets_aabb(&self, b: &Aabb<3>) -> bool {
        self.meets(b.lo(), b.hi())
    }

    /// Whether the grid box `b` shares a point with the region.
    #[must_use]
    pub fn meets_box(&self, b: &Box3) -> bool {
        let p = |q: P3| [q.x.get(), q.y.get(), q.z.get()];
        self.meets(p(b.min()), p(b.max()))
    }

    /// The part of the region inside the grid box `b`, or `None` if they share no point.
    ///
    /// # Errors
    ///
    /// As [`wide`], which the bounds rule out.
    pub fn clip(&self, b: &Box3) -> Result<Option<Region>, InstanceError> {
        let (bl, bh) = (coords(b.min()), coords(b.max()));
        let mut lo = self.lo;
        let mut hi = self.hi;
        for (((l, h), cl), ch) in lo.iter_mut().zip(&mut hi).zip(bl).zip(bh) {
            let (cl, ch) = (
                wide(cl.checked_mul(self.den))?,
                wide(ch.checked_mul(self.den))?,
            );
            *l = (*l).max(cl);
            *h = (*h).min(ch);
        }
        Region::new(lo, hi, self.den)
    }

    /// The region moved by the inverse of the grid transform `t`: where it was before `t` moved
    /// it, exactly.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Num`] if `t` has no inverse within the grid transforms' range, and as
    /// [`wide`].
    pub fn unplace(&self, t: &GridTransform) -> Result<Region, InstanceError> {
        let u = t.inverse()?;
        let (source, negate, shift) = (u.source(), u.negate(), u.translation_part());
        let mut lo = [0i128; 3];
        let mut hi = [0i128; 3];
        for ((((l, h), s), n), c) in lo
            .iter_mut()
            .zip(&mut hi)
            .zip(source)
            .zip(negate)
            .zip(shift)
        {
            let at = usize::from(s.index());
            let (from_lo, from_hi) = (
                *self.lo.get(at).ok_or(InstanceError::Defect)?,
                *self.hi.get(at).ok_or(InstanceError::Defect)?,
            );
            let (a, b) = if n {
                (wide(from_hi.checked_neg())?, wide(from_lo.checked_neg())?)
            } else {
                (from_lo, from_hi)
            };
            let moved = wide(i128::from(c).checked_mul(self.den))?;
            *l = wide(a.checked_add(moved))?;
            *h = wide(b.checked_add(moved))?;
        }
        Region::new(lo, hi, self.den)?.ok_or(InstanceError::Defect)
    }

    /// The smallest grid box holding the region: lower corners rounded down, upper ones up.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Num`] for a corner beyond the grid's range.
    pub fn outward(&self) -> Result<Box3, InstanceError> {
        let mut lo = [0i128; 3];
        let mut hi = [0i128; 3];
        for (((l, h), rl), rh) in lo.iter_mut().zip(&mut hi).zip(self.lo).zip(self.hi) {
            let down = wide(rl.checked_div_euclid(self.den))?;
            let up = wide(
                wide(rh.checked_neg())?
                    .checked_div_euclid(self.den)
                    .and_then(i128::checked_neg),
            )?;
            *l = down;
            *h = up;
        }
        let point = |v: [i128; 3]| -> Result<P3, InstanceError> {
            let [x, y, z] = v;
            Ok(P3 {
                x: Coord::from_i128(x)?,
                y: Coord::from_i128(y)?,
                z: Coord::from_i128(z)?,
            })
        };
        Ok(Box3::new(point(lo)?, point(hi)?)?)
    }
}
