//! Floating-point points and vectors, finite by construction, with deterministic arithmetic.
//!
//! Curved geometry is evaluated in `f64` in grid units (chapter 6.4). [`F2`] and [`F3`] are the
//! points and vectors of that arithmetic. They serve as both: a point is its position vector from
//! the frame's origin.
//!
//! Guarantees:
//!
//! - **Finite.** Every component is finite: made only by constructors that check, and by
//!   operations that check their results and return [`NumError::NotFinite`] when a result would
//!   overflow. No value here is ever NaN or infinite (chapter 21: NaN's bits are not
//!   deterministic).
//! - **Deterministic.** Only IEEE 754 addition, subtraction, multiplication and division, each
//!   correctly rounded to nearest on every target, evaluated in the fixed order written in each
//!   function. Rust never contracts `a * b + c` into a fused multiply-add on its own, and nothing
//!   here asks for one. Square roots come from [`geom_num::math::sqrt`], correctly rounded. No
//!   `f64::max`, `f64::min` or platform maths.
//! - **Exact conversions from the grid.** A grid point or grid vector has components below 2^42
//!   in magnitude, so it converts to `f64` with no rounding.
//! - **Lengths without spurious overflow or underflow.** `length` and `normalized` first scale the
//!   vector by the power of two that brings its largest component into [1, 2) (exact, except that
//!   a component more than 2^1022 times smaller than the largest may round, which changes nothing
//!   visible), so the squares neither overflow nor underflow. Where the plain formula would not
//!   overflow or underflow, the result has the same bits as the plain formula, because scaling by
//!   a power of two commutes with every rounding in the normal range. With u = 2^−53, the squared
//!   length has a relative error below 2u + u² for [`F2`] (two squares, one sum) and 3u + 3u² for
//!   [`F3`]; the square root halves it and adds its own rounding, so the length is within a
//!   relative 2.01u ([`F2`]) or 2.51u ([`F3`]) of the exact length, plus 2^−1070 relative for
//!   components that round when scaled. Only the zero vector cannot be normalized.
//! - **Defects reported, never answered.** The one integer step of the scaling (an exponent's
//!   negation) is checked, and the maths functions' errors are passed on. None of them can occur
//!   for finite components; one that did would be [`NumError::BudgetExceeded`] or the maths
//!   function's own error, never a guessed length.
//!
//! Method: IEEE 754-2019 binary64 basic operations, round to nearest, ties to even, and its
//! scaleB (<https://doi.org/10.1109/IEEESTD.2019.8766229>), under Rust's floating-point semantics,
//! which guarantee those results on every supported target and forbid implicit contraction (Rust
//! RFC 3514, <https://rust-lang.github.io/rfcs/3514-float-semantics.html>); the
//! scaling before squaring is the classic remedy for overflow and underflow in Euclidean norms
//! (J. L. Blue, "A portable Fortran program to find the Euclidean norm of a vector", *ACM
//! Transactions on Mathematical Software* 4(1) (1978), 15–23,
//! <https://doi.org/10.1145/355769.355771>), here with an exact power-of-two scale.
//!
//! Cost: constant time, no loops beyond the two or three components, no allocation. Arithmetic,
//! dot and cross products are at most nine IEEE operations plus a finiteness check per result;
//! `length` and `normalized` add a pass over the components for the largest, one exact
//! power-of-two scaling per component (`scale_b`), the squared sum and one correctly rounded square
//! root, then one more scaling (`length`) or one division per component (`normalized`).

use core::ops::Neg;

use geom_num::grid::{P2, P3};
use geom_num::{NumError, math};

use crate::error::budget;
use crate::exact::{exponent, scale_b};
use crate::vector::{V2, V3};

/// `v` if it is finite, else `NotFinite`.
fn finite(v: f64) -> Result<f64, NumError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(NumError::NotFinite)
    }
}

/// The exponent k that brings the largest magnitude among `v` into [1, 2) when the components are
/// multiplied by 2^−k; `None` when every component is zero.
///
/// The components are finite (the types' invariant), so `math::abs` never refuses one and the
/// largest nonzero magnitude always has an exponent; were either to fail, its error is returned.
fn scale_exponent<const N: usize>(v: [f64; N]) -> Result<Option<i32>, NumError> {
    let mut largest = 0.0;
    for x in v {
        let a = math::abs(x)?.get();
        if a > largest {
            largest = a;
        }
    }
    if largest == 0.0 {
        Ok(None)
    } else {
        budget(exponent(largest)).map(Some)
    }
}

/// The components scaled by 2^−k and their scaled length √(Σ(x·2^−k)²), in [1, 2√3): the squares
/// can neither overflow nor underflow harmfully.
///
/// k is the exponent of a finite double, from −1074 to 1023, so −k fits `i32`; each scaling is of
/// a finite component; and the sum of squares is finite and not negative, so the square root never
/// refuses it. A failure of any of these is returned, never a length.
fn scaled_length<const N: usize>(v: [f64; N], k: i32) -> Result<([f64; N], f64), NumError> {
    let down = budget(k.checked_neg())?;
    let mut s = [0.0; N];
    for (scaled, x) in s.iter_mut().zip(v) {
        *scaled = budget(scale_b(x, down))?;
    }
    let mut sum = 0.0;
    for x in s {
        sum += x * x;
    }
    Ok((s, math::sqrt(sum)?.get()))
}

/// A 2D point or vector of finite doubles, in grid units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct F2 {
    x: f64,
    y: f64,
}

// The arithmetic methods return `Result` (a result may overflow), so they are deliberately not the
// `core::ops` traits, whose callers expect a plain value.
impl F2 {
    /// The origin, or the zero vector.
    pub const ZERO: F2 = F2 { x: 0.0, y: 0.0 };

    /// The point or vector (x, y).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component is NaN or infinite.
    pub fn new(x: f64, y: f64) -> Result<F2, NumError> {
        Ok(F2 {
            x: finite(x)?,
            y: finite(y)?,
        })
    }

    /// The x component.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// The y component.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// The components as an array `[x, y]`.
    #[must_use]
    pub const fn to_array(self) -> [f64; 2] {
        [self.x, self.y]
    }

    /// The point from components that are finite by the invariant of the type they come from (an
    /// [`FBox2`](crate::FBox2)'s bounds), so no check is made and there is no error path that could
    /// never be taken. Crate-private: each caller states why its components are finite.
    pub(crate) const fn from_finite([x, y]: [f64; 2]) -> F2 {
        F2 { x, y }
    }

    /// The sum `self + other`, componentwise.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component overflows.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Add"
    )]
    pub fn add(self, other: F2) -> Result<F2, NumError> {
        F2::new(self.x + other.x, self.y + other.y)
    }

    /// The difference `self − other`, componentwise.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component overflows.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Sub"
    )]
    pub fn sub(self, other: F2) -> Result<F2, NumError> {
        F2::new(self.x - other.x, self.y - other.y)
    }

    /// The vector scaled by `k`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `k` is not finite or a component overflows.
    pub fn scale(self, k: f64) -> Result<F2, NumError> {
        F2::new(self.x * finite(k)?, self.y * k)
    }

    /// The dot product, evaluated as `x₁·x₂ + y₁·y₂`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    pub fn dot(self, other: F2) -> Result<f64, NumError> {
        finite(self.x * other.x + self.y * other.y)
    }

    /// The cross product (2 × 2 determinant), evaluated as `x₁·y₂ − y₁·x₂`. Rounded: its sign is
    /// not a predicate; use the exact grid predicates for decisions.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    pub fn cross(self, other: F2) -> Result<f64, NumError> {
        finite(self.x * other.y - self.y * other.x)
    }

    /// The squared length.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    pub fn norm2(self) -> Result<f64, NumError> {
        self.dot(self)
    }

    /// The length: the correctly rounded square root of the rounded squared length, computed on the
    /// vector scaled by a power of two (see the module documentation), then scaled back.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the length itself overflows. [`NumError::BudgetExceeded`] only
    /// if a step the module's bounds rule out failed.
    pub fn length(self) -> Result<f64, NumError> {
        let Some(k) = scale_exponent([self.x, self.y])? else {
            return Ok(0.0);
        };
        let (_, len) = scaled_length([self.x, self.y], k)?;
        finite(budget(scale_b(len, k))?)
    }

    /// The vector divided by its length, both scaled by the same power of two first (see the
    /// module documentation), so any nonzero vector, however long or short, has a unit result.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for the zero vector. [`NumError::BudgetExceeded`] only if a step
    /// the module's bounds rule out failed.
    pub fn normalized(self) -> Result<F2, NumError> {
        let Some(k) = scale_exponent([self.x, self.y])? else {
            return Err(NumError::Degenerate);
        };
        let ([x, y], len) = scaled_length([self.x, self.y], k)?;
        F2::new(x / len, y / len)
    }
}

impl Neg for F2 {
    type Output = F2;

    /// The opposite vector. Exact: negation never rounds, and a finite value stays finite.
    fn neg(self) -> F2 {
        F2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl From<V2> for F2 {
    /// A grid vector as doubles, exactly (components below 2^41 in magnitude).
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid vector's components are below 2^41 < 2^53 in magnitude: exact in f64"
    )]
    fn from(v: V2) -> F2 {
        F2 {
            x: v.x() as f64,
            y: v.y() as f64,
        }
    }
}

impl From<P2> for F2 {
    /// A grid point as doubles, exactly (components below 2^40 in magnitude).
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40 < 2^53 in magnitude: exact in f64"
    )]
    fn from(p: P2) -> F2 {
        F2 {
            x: p.x.get() as f64,
            y: p.y.get() as f64,
        }
    }
}

/// A 3D point or vector of finite doubles, in grid units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct F3 {
    x: f64,
    y: f64,
    z: f64,
}

impl F3 {
    /// The origin, or the zero vector.
    pub const ZERO: F3 = F3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// The point or vector (x, y, z).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component is NaN or infinite.
    pub fn new(x: f64, y: f64, z: f64) -> Result<F3, NumError> {
        Ok(F3 {
            x: finite(x)?,
            y: finite(y)?,
            z: finite(z)?,
        })
    }

    /// The point from components that are finite by the invariant of the type they come from (an
    /// [`FBox3`](crate::FBox3)'s bounds), so no check is made and there is no error path that could
    /// never be taken. Crate-private: each caller states why its components are finite.
    pub(crate) const fn from_finite([x, y, z]: [f64; 3]) -> F3 {
        F3 { x, y, z }
    }

    /// The x component.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// The y component.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// The z component.
    #[must_use]
    pub const fn z(self) -> f64 {
        self.z
    }

    /// The components as an array `[x, y, z]`.
    #[must_use]
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// The sum `self + other`, componentwise.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component overflows.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Add"
    )]
    pub fn add(self, other: F3) -> Result<F3, NumError> {
        F3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// The difference `self − other`, componentwise.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component overflows.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Sub"
    )]
    pub fn sub(self, other: F3) -> Result<F3, NumError> {
        F3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// The vector scaled by `k`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `k` is not finite or a component overflows.
    pub fn scale(self, k: f64) -> Result<F3, NumError> {
        let k = finite(k)?;
        F3::new(self.x * k, self.y * k, self.z * k)
    }

    /// The dot product, evaluated as `(x₁·x₂ + y₁·y₂) + z₁·z₂`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    pub fn dot(self, other: F3) -> Result<f64, NumError> {
        finite(self.x * other.x + self.y * other.y + self.z * other.z)
    }

    /// The cross product `self × other`, each component evaluated as a difference of two products.
    /// Rounded: use the exact grid predicates for decisions.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a component overflows.
    pub fn cross(self, other: F3) -> Result<F3, NumError> {
        F3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// The squared length.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    pub fn norm2(self) -> Result<f64, NumError> {
        self.dot(self)
    }

    /// The length: the correctly rounded square root of the rounded squared length (summed as
    /// `(x² + y²) + z²`), computed on the vector scaled by a power of two (see the module
    /// documentation), then scaled back.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the length itself overflows. [`NumError::BudgetExceeded`] only
    /// if a step the module's bounds rule out failed.
    pub fn length(self) -> Result<f64, NumError> {
        let Some(k) = scale_exponent(self.to_array())? else {
            return Ok(0.0);
        };
        let (_, len) = scaled_length(self.to_array(), k)?;
        finite(budget(scale_b(len, k))?)
    }

    /// The vector divided by its length, both scaled by the same power of two first (see the
    /// module documentation), so any nonzero vector, however long or short, has a unit result.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for the zero vector. [`NumError::BudgetExceeded`] only if a step
    /// the module's bounds rule out failed.
    pub fn normalized(self) -> Result<F3, NumError> {
        let Some(k) = scale_exponent(self.to_array())? else {
            return Err(NumError::Degenerate);
        };
        let ([x, y, z], len) = scaled_length(self.to_array(), k)?;
        F3::new(x / len, y / len, z / len)
    }
}

impl Neg for F3 {
    type Output = F3;

    /// The opposite vector. Exact: negation never rounds, and a finite value stays finite.
    fn neg(self) -> F3 {
        F3 {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl From<V3> for F3 {
    /// A grid vector as doubles, exactly (components below 2^41 in magnitude).
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid vector's components are below 2^41 < 2^53 in magnitude: exact in f64"
    )]
    fn from(v: V3) -> F3 {
        F3 {
            x: v.x() as f64,
            y: v.y() as f64,
            z: v.z() as f64,
        }
    }
}

impl From<P3> for F3 {
    /// A grid point as doubles, exactly (components below 2^40 in magnitude).
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40 < 2^53 in magnitude: exact in f64"
    )]
    fn from(p: P3) -> F3 {
        F3 {
            x: p.x.get() as f64,
            y: p.y.get() as f64,
            z: p.z.get() as f64,
        }
    }
}
