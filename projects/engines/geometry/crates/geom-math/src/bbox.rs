//! Axis-aligned boxes: exact boxes on the grid, and floating-point boxes that round outward.
//!
//! [`Box2`] and [`Box3`] are closed boxes of grid points, `min ≤ max` on every axis: union,
//! intersection, containment and overlap are comparisons of integers, so they are exact. A box
//! moved by a [`GridTransform`] is again a grid box, exactly.
//!
//! [`FBox3`] is a closed box of finite doubles, in grid units, that is certain to contain what it
//! bounds. Building one from points or grid boxes is exact (no arithmetic); transforming one by a
//! [`Transform`] uses interval arithmetic with outward rounding (`geom_num::interval`, each bound
//! moved one floating-point step outward after rounding to nearest), so the result contains the
//! exact image of every point of the box; converting one to the grid rounds its lower bounds down
//! and its upper bounds up.
//!
//! Guarantees:
//!
//! - **Never empty, always ordered.** Every box has `min ≤ max` (`lo ≤ hi`) on every axis;
//!   constructors refuse anything else with [`NumError::EmptyInterval`], and an intersection that
//!   would be empty is `None`.
//! - **Closed.** Boxes contain their boundary: two boxes that share only a face, an edge or a
//!   corner overlap.
//! - **Finite.** `FBox3` bounds are finite, with zero stored as +0; operations that would overflow
//!   return [`NumError::NotFinite`].
//!
//! Method: closed intervals per axis, so a box is the product of three intervals and every
//! operation is per-axis interval logic; outward-rounded interval arithmetic as in R. E. Moore,
//! R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>.
//!
//! Cost: constant time and no allocation for every operation except `from_points`, which is one
//! union per point. Grid boxes compare integers (at most six comparisons for a union, an
//! intersection or a containment test); moving one by a [`GridTransform`] moves its two corners.
//! [`FBox3::transformed`] is 18 interval operations, a product and a sum for each entry of the
//! linear part (an interval sum is two IEEE additions, a product four multiplications and six
//! comparisons, each followed by a one-step widening of both bounds);
//! [`FBox3::to_grid_outward`] is six roundings and six range checks.

use geom_num::grid::{Coord, P2, P3};
use geom_num::interval::Interval;
use geom_num::{NumError, Real, math};

use crate::float::F3;
use crate::grid_transform::GridTransform;
use crate::transform::Transform;
use crate::vector::{V2, V3};

/// A closed axis-aligned box of 2D grid points, `min ≤ max` on both axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Box2 {
    min: P2,
    max: P2,
}

impl Box2 {
    /// The box with corners `min` and `max`.
    ///
    /// # Errors
    ///
    /// [`NumError::EmptyInterval`] if `min` exceeds `max` on an axis.
    pub fn new(min: P2, max: P2) -> Result<Box2, NumError> {
        if min.x > max.x || min.y > max.y {
            return Err(NumError::EmptyInterval);
        }
        Ok(Box2 { min, max })
    }

    /// The box holding one point.
    #[must_use]
    pub const fn from_point(p: P2) -> Box2 {
        Box2 { min: p, max: p }
    }

    /// The smallest box holding all the points, or `None` for no points.
    pub fn from_points(points: impl IntoIterator<Item = P2>) -> Option<Box2> {
        points
            .into_iter()
            .map(Box2::from_point)
            .reduce(|a, b| a.union(&b))
    }

    /// The lower corner.
    #[must_use]
    pub const fn min(&self) -> P2 {
        self.min
    }

    /// The upper corner.
    #[must_use]
    pub const fn max(&self) -> P2 {
        self.max
    }

    /// The smallest box holding both boxes.
    #[must_use]
    pub fn union(&self, other: &Box2) -> Box2 {
        Box2 {
            min: P2 {
                x: self.min.x.min(other.min.x),
                y: self.min.y.min(other.min.y),
            },
            max: P2 {
                x: self.max.x.max(other.max.x),
                y: self.max.y.max(other.max.y),
            },
        }
    }

    /// The points both boxes hold, or `None` if they do not overlap.
    #[must_use]
    pub fn intersection(&self, other: &Box2) -> Option<Box2> {
        let min = P2 {
            x: self.min.x.max(other.min.x),
            y: self.min.y.max(other.min.y),
        };
        let max = P2 {
            x: self.max.x.min(other.max.x),
            y: self.max.y.min(other.max.y),
        };
        Box2::new(min, max).ok()
    }

    /// Whether the box holds the point (boundary included).
    #[must_use]
    pub fn contains(&self, p: P2) -> bool {
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }

    /// Whether the box holds every point of `other`.
    #[must_use]
    pub fn contains_box(&self, other: &Box2) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Whether the boxes share at least one point (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &Box2) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// The extent `max − min`, a grid vector with non-negative components.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked difference failed, which the grid's range
    /// rules out ([`V2::between`]): for every box the result is `Ok`.
    pub fn extent(&self) -> Result<V2, NumError> {
        V2::between(self.min, self.max)
    }
}

/// A closed axis-aligned box of 3D grid points, `min ≤ max` on every axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Box3 {
    min: P3,
    max: P3,
}

/// The componentwise minimum and maximum of two grid points.
fn min_max(a: P3, b: P3) -> (P3, P3) {
    (
        P3 {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            z: a.z.min(b.z),
        },
        P3 {
            x: a.x.max(b.x),
            y: a.y.max(b.y),
            z: a.z.max(b.z),
        },
    )
}

impl Box3 {
    /// The box with corners `min` and `max`.
    ///
    /// # Errors
    ///
    /// [`NumError::EmptyInterval`] if `min` exceeds `max` on an axis.
    pub fn new(min: P3, max: P3) -> Result<Box3, NumError> {
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(NumError::EmptyInterval);
        }
        Ok(Box3 { min, max })
    }

    /// The box holding one point.
    #[must_use]
    pub const fn from_point(p: P3) -> Box3 {
        Box3 { min: p, max: p }
    }

    /// The smallest box holding all the points, or `None` for no points.
    pub fn from_points(points: impl IntoIterator<Item = P3>) -> Option<Box3> {
        points
            .into_iter()
            .map(Box3::from_point)
            .reduce(|a, b| a.union(&b))
    }

    /// The lower corner.
    #[must_use]
    pub const fn min(&self) -> P3 {
        self.min
    }

    /// The upper corner.
    #[must_use]
    pub const fn max(&self) -> P3 {
        self.max
    }

    /// The smallest box holding both boxes.
    #[must_use]
    pub fn union(&self, other: &Box3) -> Box3 {
        let (min, _) = min_max(self.min, other.min);
        let (_, max) = min_max(self.max, other.max);
        Box3 { min, max }
    }

    /// The points both boxes hold, or `None` if they do not overlap.
    #[must_use]
    pub fn intersection(&self, other: &Box3) -> Option<Box3> {
        let (_, min) = min_max(self.min, other.min);
        let (max, _) = min_max(self.max, other.max);
        Box3::new(min, max).ok()
    }

    /// Whether the box holds the point (boundary included).
    #[must_use]
    pub fn contains(&self, p: P3) -> bool {
        (self.min.x..=self.max.x).contains(&p.x)
            && (self.min.y..=self.max.y).contains(&p.y)
            && (self.min.z..=self.max.z).contains(&p.z)
    }

    /// Whether the box holds every point of `other`.
    #[must_use]
    pub fn contains_box(&self, other: &Box3) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Whether the boxes share at least one point (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &Box3) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
            && self.min.z <= other.max.z
            && other.min.z <= self.max.z
    }

    /// The extent `max − min`, a grid vector with non-negative components.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked difference failed, which the grid's range
    /// rules out ([`V3::between`]): for every box the result is `Ok`.
    pub fn extent(&self) -> Result<V3, NumError> {
        V3::between(self.min, self.max)
    }

    /// The box moved by a grid transform, exactly: a signed permutation maps the box's corners to
    /// the corners of the image box, so the images of `min` and `max`, sorted per axis, are its
    /// corners.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a corner of the image leaves the frame's range.
    pub fn transformed(&self, t: &GridTransform) -> Result<Box3, NumError> {
        let (min, max) = min_max(t.apply(self.min)?, t.apply(self.max)?);
        Ok(Box3 { min, max })
    }
}

/// A closed axis-aligned box of finite doubles, in grid units: `lo ≤ hi` on every axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FBox3 {
    lo: [f64; 3],
    hi: [f64; 3],
}

/// −0 becomes +0; every other finite value is unchanged.
const fn canon(x: f64) -> f64 {
    x + 0.0
}

/// The smaller of two finite doubles; either zero when given −0 and +0, which `canon` then fixes.
const fn min2(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two finite doubles; either zero when given −0 and +0, which `canon` then fixes.
const fn max2(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Applies `f` to matching components of two arrays.
fn zip3(a: [f64; 3], b: [f64; 3], f: impl Fn(f64, f64) -> f64) -> [f64; 3] {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [f(a0, b0), f(a1, b1), f(a2, b2)]
}

impl FBox3 {
    /// The box with corners `lo` and `hi`.
    ///
    /// # Errors
    ///
    /// [`NumError::EmptyInterval`] if `lo` exceeds `hi` on an axis.
    pub fn new(lo: F3, hi: F3) -> Result<FBox3, NumError> {
        let (lo, hi) = (lo.to_array(), hi.to_array());
        if lo.iter().zip(hi.iter()).any(|(l, h)| l > h) {
            return Err(NumError::EmptyInterval);
        }
        Ok(FBox3 {
            lo: lo.map(canon),
            hi: hi.map(canon),
        })
    }

    /// The box holding one point.
    #[must_use]
    pub fn from_point(p: F3) -> FBox3 {
        let a = p.to_array().map(canon);
        FBox3 { lo: a, hi: a }
    }

    /// The box whose axes are the three intervals.
    #[must_use]
    pub fn from_intervals(axes: [Interval; 3]) -> FBox3 {
        FBox3 {
            lo: axes.map(Interval::lo),
            hi: axes.map(Interval::hi),
        }
    }

    /// A grid box as doubles, exactly (coordinates below 2^40 in magnitude).
    #[must_use]
    pub fn from_box3(b: &Box3) -> FBox3 {
        let (lo, hi) = (F3::from(b.min).to_array(), F3::from(b.max).to_array());
        FBox3 {
            lo: lo.map(canon),
            hi: hi.map(canon),
        }
    }

    /// The lower corner.
    #[must_use]
    pub const fn lo(&self) -> F3 {
        // The bounds are finite by the type's invariant: every constructor takes them from an F3,
        // a grid point or an interval (each finite by its own type), or as the smaller or larger
        // of two such bounds, so they are a finite F3 as they stand.
        F3::from_finite(self.lo)
    }

    /// The upper corner.
    #[must_use]
    pub const fn hi(&self) -> F3 {
        // Finite by the type's invariant, as for `lo`.
        F3::from_finite(self.hi)
    }

    /// The axes as intervals, x, y, z.
    ///
    /// # Errors
    ///
    /// Never in practice: the bounds are finite and ordered by the invariant.
    pub fn axes(&self) -> Result<[Interval; 3], NumError> {
        let ([l0, l1, l2], [h0, h1, h2]) = (self.lo, self.hi);
        Ok([
            Interval::new(l0, h0)?,
            Interval::new(l1, h1)?,
            Interval::new(l2, h2)?,
        ])
    }

    /// The smallest box holding both boxes. Exact: no arithmetic.
    #[must_use]
    pub fn union(&self, other: &FBox3) -> FBox3 {
        FBox3 {
            lo: zip3(self.lo, other.lo, |a, b| canon(min2(a, b))),
            hi: zip3(self.hi, other.hi, |a, b| canon(max2(a, b))),
        }
    }

    /// The points both boxes hold, or `None` if they do not overlap. Exact: no arithmetic.
    #[must_use]
    pub fn intersection(&self, other: &FBox3) -> Option<FBox3> {
        let lo = zip3(self.lo, other.lo, |a, b| canon(max2(a, b)));
        let hi = zip3(self.hi, other.hi, |a, b| canon(min2(a, b)));
        if lo.iter().zip(hi.iter()).any(|(l, h)| l > h) {
            None
        } else {
            Some(FBox3 { lo, hi })
        }
    }

    /// Whether the box holds the point (boundary included).
    #[must_use]
    pub fn contains(&self, p: F3) -> bool {
        let p = p.to_array();
        p.iter()
            .zip(self.lo.iter().zip(self.hi.iter()))
            .all(|(v, (l, h))| (*l..=*h).contains(v))
    }

    /// Whether the boxes share at least one point (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &FBox3) -> bool {
        self.intersection(other).is_some()
    }

    /// A box certain to contain the image of every point of this box under `t`.
    ///
    /// Each output coordinate is enclosed as t + l₀·X + l₁·Y + l₂·Z in interval arithmetic, X, Y
    /// and Z being this box's axes. Interval arithmetic contains the exact real result for every
    /// point of its operands, so the box holds the exact image (of the transform with these exact
    /// `f64` entries), whatever rounding happens on the way.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a bound overflows.
    pub fn transformed(&self, t: &Transform) -> Result<FBox3, NumError> {
        let [ax, ay, az] = self.axes()?;
        let row = |r: [f64; 4]| -> Result<Interval, NumError> {
            let [l0, l1, l2, tr] = r;
            Interval::point(tr)?
                .add(Interval::point(l0)?.mul(ax)?)?
                .add(Interval::point(l1)?.mul(ay)?)?
                .add(Interval::point(l2)?.mul(az)?)
        };
        let [r0, r1, r2] = t.rows();
        Ok(FBox3::from_intervals([row(r0)?, row(r1)?, row(r2)?]))
    }

    /// The smallest grid box holding this box: lower bounds rounded down, upper bounds up.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a rounded bound lies outside the frame's ±(2^40 − 1) range,
    /// carrying that rounded bound exactly, or saturated with its sign when it is beyond `i128`, as
    /// the numbers layer documents the variant. The bounds are finite (the type's invariant), so
    /// `math::floor` and `math::ceil` never refuse one; their error would be returned if they did.
    pub fn to_grid_outward(self) -> Result<Box3, NumError> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "v is a whole number (a floor or a ceiling): below 2^127 in magnitude it \
                      converts exactly, and beyond that the conversion saturates at the ends of \
                      i128, which is how NumError::OutOfRange reports a coordinate beyond i128; \
                      Coord::from_i128 refuses every value outside the frame with that value"
        )]
        let coord = |v: f64| Coord::from_i128(v as i128);
        let down = |v: f64| math::floor(v).map(Real::get);
        let up = |v: f64| math::ceil(v).map(Real::get);
        let ([l0, l1, l2], [h0, h1, h2]) = (self.lo, self.hi);
        Box3::new(
            P3 {
                x: coord(down(l0)?)?,
                y: coord(down(l1)?)?,
                z: coord(down(l2)?)?,
            },
            P3 {
                x: coord(up(h0)?)?,
                y: coord(up(h1)?)?,
                z: coord(up(h2)?)?,
            },
        )
    }
}
