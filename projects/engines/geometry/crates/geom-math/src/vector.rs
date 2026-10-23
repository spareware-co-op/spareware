//! Exact grid vectors: differences of grid points, with exact dot, cross and triple products.
//!
//! A grid point has coordinates of magnitude at most M = [`COORD_MAX`] = 2^40 − 1 (chapter 6.1), so
//! the difference of two grid points has components of magnitude at most δ = 2M = 2^41 − 2 < 2^41.
//! [`V2`] and [`V3`] hold exactly such differences, in `i64`, and keep `|component| ≤` [`VEC_MAX`]
//! = δ. Every product is evaluated in `i128` from that bound, and each function's doc comment
//! carries its bit budget, so no product can overflow.
//!
//! Guarantees:
//!
//! - **Exact.** Integer arithmetic only. A dot or cross product is the exact integer, never a
//!   rounded one; its sign is an exact predicate.
//! - **No overflow, no panic, the same result in every build profile.** Every integer step is a
//!   checked method (`checked_add`, `checked_sub`, `checked_mul`, `checked_neg`), never a native
//!   operator, so nothing depends on overflow checks being on. Products are proved to fit `i128`
//!   (|v| < 2^127) with room to spare, and a step the bit budget proves fits can only fail through
//!   a defect in this crate: it is then reported as [`NumError::BudgetExceeded`], never answered,
//!   so a defect can never pass for an exact result. For grid vectors every such result is `Ok`.
//!   Sums and differences of vectors, which can leave the vector range, return
//!   [`NumError::OutOfRange`].
//! - **Deterministic.** The same integers on every target.
//!
//! Method: these products are the building blocks of Shewchuk's determinant forms of the
//! orientation predicates, evaluated exactly in fixed-width integers because the inputs are grid
//! integers below 2^41, as `geom_num::predicate` does: J. R. Shewchuk, "Adaptive precision
//! floating-point arithmetic and fast robust geometric predicates", *Discrete & Computational
//! Geometry* 18 (1997), 305–363, <https://doi.org/10.1007/PL00009321>.
//!
//! Notation in the bit budgets: δ = [`VEC_MAX`] = 2^41 − 2 < 2^41, so δ² < 2^82 and δ³ < 2^123.
//!
//! Cost: constant time, no allocation. A dot product is two or three checked `i128`
//! multiplications, a 3D cross product six and a triple product nine; sums, differences and moved
//! points are one checked `i128` addition and one range check per component.

use geom_num::NumError;
use geom_num::grid::{COORD_MAX, Coord, P2, P3};

use crate::error::budget;

/// The largest component magnitude of a grid vector: 2(2^40 − 1) = 2^41 − 2, the largest difference
/// of two coordinates.
pub const VEC_MAX: i64 = 2 * COORD_MAX;

/// A component from a wider integer, or `OutOfRange` if its magnitude exceeds [`VEC_MAX`].
fn component(v: i128) -> Result<i64, NumError> {
    if v.unsigned_abs() > u128::from(VEC_MAX.unsigned_abs()) {
        Err(NumError::OutOfRange { value: v })
    } else {
        // In range, so it fits i64; the conversion cannot fail.
        i64::try_from(v).map_err(|_| NumError::OutOfRange { value: v })
    }
}

/// The difference `to − from` of two coordinates: at most 2(2^40 − 1) = [`VEC_MAX`] in magnitude,
/// so it fits `i64`.
fn difference(to: Coord, from: Coord) -> Result<i64, NumError> {
    budget(to.get().checked_sub(from.get()))
}

/// A sum or difference of two components, then range-checked: each is at most δ < 2^41, so the
/// exact value is below 2^42 in magnitude and fits `i128`.
fn combined(a: i64, b: i64, negate_b: bool) -> Result<i64, NumError> {
    let (a, b) = (i128::from(a), i128::from(b));
    let exact = if negate_b {
        a.checked_sub(b)
    } else {
        a.checked_add(b)
    };
    component(budget(exact)?)
}

/// The product of two values held as `i64`, exactly, in `i128`: |a|, |b| ≤ 2^63, so |a·b| ≤ 2^126.
fn product(a: i64, b: i64) -> Option<i128> {
    i128::from(a).checked_mul(i128::from(b))
}

/// a·d − b·c for components, exactly: each product is at most δ² < 2^82, the difference at most
/// 2δ² < 2^83.
fn det2(a: i64, b: i64, c: i64, d: i64) -> Result<i128, NumError> {
    let (ad, bc) = (product(a, d), product(b, c));
    budget(ad.zip(bc).and_then(|(ad, bc)| ad.checked_sub(bc)))
}

/// A grid point moved by a vector, or `OutOfRange` if it leaves the frame.
///
/// Budget: |p| ≤ 2^40 − 1 and |v| ≤ 2^41 − 2, so the sum is below 2^42 in magnitude and fits `i128`.
fn moved(p: Coord, v: i64) -> Result<Coord, NumError> {
    Coord::from_i128(budget(i128::from(p.get()).checked_add(i128::from(v)))?)
}

/// The opposite of a component: |v| ≤ [`VEC_MAX`] < 2^63, so −v fits `i64`.
fn opposite(v: i64) -> Result<i64, NumError> {
    budget(v.checked_neg())
}

/// A 2D grid vector: the difference of two grid points, each component at most [`VEC_MAX`] in
/// magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct V2 {
    x: i64,
    y: i64,
}

// Vector addition, subtraction and negation return `Result` (they can leave the vector range, or
// report a defect), so they are deliberately not the `core::ops` traits, whose callers expect a
// plain value.
impl V2 {
    /// The zero vector.
    pub const ZERO: V2 = V2 { x: 0, y: 0 };

    /// The vector (x, y).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component's magnitude exceeds [`VEC_MAX`].
    pub fn new(x: i64, y: i64) -> Result<V2, NumError> {
        Ok(V2 {
            x: component(i128::from(x))?,
            y: component(i128::from(y))?,
        })
    }

    /// The vector from `from` to `to`, that is `to − from`. Always in range: each component is a
    /// difference of two coordinates of magnitude at most 2^40 − 1, so at most [`VEC_MAX`].
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked difference failed, which the bound above
    /// rules out: for grid points the result is always `Ok`.
    pub fn between(from: P2, to: P2) -> Result<V2, NumError> {
        Ok(V2 {
            x: difference(to.x, from.x)?,
            y: difference(to.y, from.y)?,
        })
    }

    /// The x component, in grid steps.
    #[must_use]
    pub const fn x(self) -> i64 {
        self.x
    }

    /// The y component, in grid steps.
    #[must_use]
    pub const fn y(self) -> i64 {
        self.y
    }

    /// The dot product x₁x₂ + y₁y₂, exactly.
    ///
    /// Budget: each product is at most δ² < 2^82, the sum at most 2δ² < 2^83.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn dot(self, other: V2) -> Result<i128, NumError> {
        let xx = product(self.x, other.x);
        let yy = product(self.y, other.y);
        budget(xx.zip(yy).and_then(|(a, b)| a.checked_add(b)))
    }

    /// The cross product (the 2 × 2 determinant) x₁y₂ − y₁x₂, exactly: positive when `other` lies
    /// counter-clockwise of `self`, zero exactly when they are parallel.
    ///
    /// Budget: each product is at most δ² < 2^82, the difference at most 2δ² < 2^83.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn cross(self, other: V2) -> Result<i128, NumError> {
        det2(self.x, self.y, other.x, other.y)
    }

    /// The squared length x² + y², exactly. Budget: at most 2δ² < 2^83.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn norm2(self) -> Result<i128, NumError> {
        self.dot(self)
    }

    /// The sum `self + other`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component of the sum exceeds [`VEC_MAX`] in magnitude.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it can leave the vector range), so it cannot be core::ops::Add"
    )]
    pub fn add(self, other: V2) -> Result<V2, NumError> {
        Ok(V2 {
            x: combined(self.x, other.x, false)?,
            y: combined(self.y, other.y, false)?,
        })
    }

    /// The difference `self − other`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component of the difference exceeds [`VEC_MAX`] in magnitude.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it can leave the vector range), so it cannot be core::ops::Sub"
    )]
    pub fn sub(self, other: V2) -> Result<V2, NumError> {
        Ok(V2 {
            x: combined(self.x, other.x, true)?,
            y: combined(self.y, other.y, true)?,
        })
    }

    /// The opposite vector. Exact and always in range: the range is symmetric about zero.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked negation failed, which the range rules out
    /// (|component| ≤ [`VEC_MAX`] < 2^63): for every vector the result is `Ok`.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (every integer step is checked), so it cannot be core::ops::Neg"
    )]
    pub fn neg(self) -> Result<V2, NumError> {
        Ok(V2 {
            x: opposite(self.x)?,
            y: opposite(self.y)?,
        })
    }

    /// The point `p` moved by this vector.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if the moved point leaves the frame's ±(2^40 − 1) range.
    pub fn offset(self, p: P2) -> Result<P2, NumError> {
        Ok(P2 {
            x: moved(p.x, self.x)?,
            y: moved(p.y, self.y)?,
        })
    }
}

/// A 3D grid vector: the difference of two grid points, each component at most [`VEC_MAX`] in
/// magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct V3 {
    x: i64,
    y: i64,
    z: i64,
}

impl V3 {
    /// The zero vector.
    pub const ZERO: V3 = V3 { x: 0, y: 0, z: 0 };

    /// The vector (x, y, z).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component's magnitude exceeds [`VEC_MAX`].
    pub fn new(x: i64, y: i64, z: i64) -> Result<V3, NumError> {
        Ok(V3 {
            x: component(i128::from(x))?,
            y: component(i128::from(y))?,
            z: component(i128::from(z))?,
        })
    }

    /// The vector from `from` to `to`, that is `to − from`. Always in range: each component is a
    /// difference of two coordinates of magnitude at most 2^40 − 1, so at most [`VEC_MAX`].
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked difference failed, which the bound above
    /// rules out: for grid points the result is always `Ok`.
    pub fn between(from: P3, to: P3) -> Result<V3, NumError> {
        Ok(V3 {
            x: difference(to.x, from.x)?,
            y: difference(to.y, from.y)?,
            z: difference(to.z, from.z)?,
        })
    }

    /// The x component, in grid steps.
    #[must_use]
    pub const fn x(self) -> i64 {
        self.x
    }

    /// The y component, in grid steps.
    #[must_use]
    pub const fn y(self) -> i64 {
        self.y
    }

    /// The z component, in grid steps.
    #[must_use]
    pub const fn z(self) -> i64 {
        self.z
    }

    /// The components as an array `[x, y, z]`.
    #[must_use]
    pub const fn to_array(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }

    /// The dot product x₁x₂ + y₁y₂ + z₁z₂, exactly.
    ///
    /// Budget: each product is at most δ² < 2^82, the sum at most 3δ² < 2^84.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn dot(self, other: V3) -> Result<i128, NumError> {
        let sum = || {
            product(self.x, other.x)?
                .checked_add(product(self.y, other.y)?)?
                .checked_add(product(self.z, other.z)?)
        };
        budget(sum())
    }

    /// The cross product `self × other`, exactly, as a [`W3`].
    ///
    /// Budget: each component is a difference of two products of components, at most
    /// 2δ² < 2^83 in magnitude.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn cross(self, other: V3) -> Result<W3, NumError> {
        let (a, b) = (self, other);
        Ok(W3 {
            x: det2(a.y, a.z, b.y, b.z)?,
            y: det2(a.z, a.x, b.z, b.x)?,
            z: det2(a.x, a.y, b.x, b.y)?,
        })
    }

    /// The scalar triple product `self · (b × c)`, the 3 × 3 determinant with rows `self`, `b`,
    /// `c`, exactly: positive when the three vectors form a right-handed set, zero exactly when
    /// they are coplanar.
    ///
    /// Budget: each component of b × c is at most 2δ² < 2^83 ([`V3::cross`]); each of the three
    /// terms of the dot product is at most δ · 2δ² = 2δ³ < 2^124, and their sum at most
    /// 6δ³ < 2^126.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn triple(self, b: V3, c: V3) -> Result<i128, NumError> {
        b.cross(c)?.dot(self)
    }

    /// The squared length x² + y² + z², exactly. Budget: at most 3δ² < 2^84.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn norm2(self) -> Result<i128, NumError> {
        self.dot(self)
    }

    /// The sum `self + other`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component of the sum exceeds [`VEC_MAX`] in magnitude.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it can leave the vector range), so it cannot be core::ops::Add"
    )]
    pub fn add(self, other: V3) -> Result<V3, NumError> {
        Ok(V3 {
            x: combined(self.x, other.x, false)?,
            y: combined(self.y, other.y, false)?,
            z: combined(self.z, other.z, false)?,
        })
    }

    /// The difference `self − other`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component of the difference exceeds [`VEC_MAX`] in magnitude.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it can leave the vector range), so it cannot be core::ops::Sub"
    )]
    pub fn sub(self, other: V3) -> Result<V3, NumError> {
        Ok(V3 {
            x: combined(self.x, other.x, true)?,
            y: combined(self.y, other.y, true)?,
            z: combined(self.z, other.z, true)?,
        })
    }

    /// The opposite vector. Exact and always in range: the range is symmetric about zero.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked negation failed, which the range rules out
    /// (|component| ≤ [`VEC_MAX`] < 2^63): for every vector the result is `Ok`.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (every integer step is checked), so it cannot be core::ops::Neg"
    )]
    pub fn neg(self) -> Result<V3, NumError> {
        Ok(V3 {
            x: opposite(self.x)?,
            y: opposite(self.y)?,
            z: opposite(self.z)?,
        })
    }

    /// The point `p` moved by this vector.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if the moved point leaves the frame's ±(2^40 − 1) range.
    pub fn offset(self, p: P3) -> Result<P3, NumError> {
        Ok(P3 {
            x: moved(p.x, self.x)?,
            y: moved(p.y, self.y)?,
            z: moved(p.z, self.z)?,
        })
    }
}

/// A 3D vector of wide integers: the exact cross product of two grid vectors.
///
/// Made only by [`V3::cross`], so every component has magnitude at most 2δ² < 2^83, which the
/// budget of [`W3::dot`] relies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct W3 {
    x: i128,
    y: i128,
    z: i128,
}

impl W3 {
    /// The x component.
    #[must_use]
    pub const fn x(self) -> i128 {
        self.x
    }

    /// The y component.
    #[must_use]
    pub const fn y(self) -> i128 {
        self.y
    }

    /// The z component.
    #[must_use]
    pub const fn z(self) -> i128 {
        self.z
    }

    /// Whether every component is zero: for a cross product, exactly when the two grid vectors are
    /// parallel (or one is zero).
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.x == 0 && self.y == 0 && self.z == 0
    }

    /// The dot product with a grid vector, exactly.
    ///
    /// Budget: each term is at most 2δ² · δ = 2δ³ < 2^124, the sum at most 6δ³ < 2^126.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked step failed, which the budget rules out.
    pub fn dot(self, v: V3) -> Result<i128, NumError> {
        let sum = || {
            self.x
                .checked_mul(i128::from(v.x))?
                .checked_add(self.y.checked_mul(i128::from(v.y))?)?
                .checked_add(self.z.checked_mul(i128::from(v.z))?)
        };
        budget(sum())
    }
}
