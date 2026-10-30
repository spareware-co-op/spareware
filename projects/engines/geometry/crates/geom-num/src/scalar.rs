//! The arithmetic a residual is written over once, so that the same code gives a rounded value, a
//! certified enclosure and, through [`crate::dual::Dual`], its derivatives.
//!
//! What it does: [`Scalar`] is the set of operations a residual function may use: the four basic
//! operations, the negation, the square, the fused multiply-add, the square root, π, and the
//! proven maths functions `sinpi`, `cospi`, `exp`, `ln` and `atan2` of [`crate::math`]. It is
//! implemented for [`Real`], rounded to nearest; for [`Interval`], rounded outward; and for
//! [`crate::dual::Dual`] over any scalar. A residual written once as
//! `fn residuals<S: Scalar>(x: [S; N]) -> Result<[S; N], NumError>` then gives its value at a point
//! with `S = Real`, an enclosure of its range over a box with `S = Interval`, its Jacobian at a
//! point with `S = Dual<Real, N>`, and an enclosure of its Jacobian over a box with
//! `S = Dual<Interval, N>`, all from the one text, so a derivative can never drift from the value
//! it belongs to (chapter 15.4: "Derivatives are generated"). The interval Newton operators of
//! [`crate::newton`] take residuals in this form.
//!
//! Guarantees:
//!
//! - **Never NaN, never infinite.** Every operation on a [`Real`] rounds to nearest and checks its
//!   result with [`Real::new`], so a NaN or an infinity anywhere, whether an overflow, a division
//!   by zero or ∞ − ∞, is [`NumError::NotFinite`] at the operation that made it, in every build.
//!   That check is the NaN tripwire of chapters 15.4 and 21: an error, never a debug-only panic
//!   (31.7's G24 row explains why the kernel has no panicking tripwire), and never a NaN carried
//!   on. Operations on an [`Interval`] keep its invariant: finite bounds, or an error.
//! - **The maths is the kernel's.** Every function comes from [`crate::math`] for a `Real` and
//!   from the certified interval versions (`interval.rs`, `math/enclose.rs`) for an `Interval`:
//!   correctly rounded `sinpi`, `cospi`, `exp`, `ln` and `atan2`, and `libm`'s correctly rounded
//!   square root and fused multiply-add. No platform maths, so every target computes the same
//!   bits.
//! - **Enclosures.** On an `Interval`, each operation is the interval operation of the same name,
//!   so it encloses the exact result over its operands, and [`Scalar::pi`] encloses π
//!   ([`Interval::PI`]). By induction over the operations, a residual evaluated in `Interval`
//!   encloses the exact residual at every point of the box: the natural interval extension (Moore,
//!   Kearfott and Cloud).
//! - **Sealed.** Only this crate implements `Scalar`, for `Real`, `Interval` and
//!   [`crate::dual::Dual`], so the operators that certify roots only ever run on arithmetic whose
//!   rounding this crate proves.
//! - **Errors as the operation states them.** A domain error is the underlying function's
//!   (`OutsideDomain` for the square root of a negative `Real` or the logarithm of a value that is
//!   not positive, `NegativeSqrt` for an interval reaching below zero, `DivisorContainsZero` for an
//!   interval divisor holding zero), unchanged.
//!
//! Method: a number type the computation is generic over, as forward-mode automatic
//! differentiation by operator overloading uses it (A. Griewank and A. Walther, *Evaluating
//! Derivatives*, 2nd ed., SIAM, 2008, chapters 2 and 3, doi:10.1137/1.9780898717761), and the
//! natural interval extension of an expression (R. E. Moore, R. B. Kearfott and M. J. Cloud,
//! *Introduction to Interval Analysis*, SIAM, 2009, chapter 5, doi:10.1137/1.9780898717716). The
//! trait is sealed in the usual Rust way, by a supertrait in a private module (Rust API
//! Guidelines, C-SEALED, <https://rust-lang.github.io/api-guidelines/future-proofing.html>).
//! Written for this kernel because the published dual-number crates compute with the platform's
//! maths functions, which chapter 21 forbids, and none has an outward-rounded interval scalar.
//!
//! Cost: one operation of the underlying type per operation, plus one finiteness check on a
//! `Real`. No allocation, no recursion.

use crate::error::NumError;
use crate::interval::Interval;
use crate::math;
use crate::real::Real;

/// The operations a residual function may use, implemented for [`Real`], [`Interval`] and
/// [`crate::dual::Dual`] over either (module documentation).
///
/// Each method is the operation of the same name on the underlying type, so its rounding, its
/// errors and its proofs are that type's.
pub trait Scalar: Copy + sealed::Sealed {
    /// The constant `c`: the double itself, an interval holding exactly it, or a dual number with
    /// value `c` and zero derivatives.
    fn constant(c: Real) -> Self;

    /// π: rounded to nearest for a `Real` ([`Real::PI`]), enclosed for an `Interval`
    /// ([`Interval::PI`]), with zero derivatives for a dual number.
    fn pi() -> Self;

    /// The negation, which is exact.
    #[must_use]
    fn neg(self) -> Self;

    /// The sum.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn add(self, other: Self) -> Result<Self, NumError>;

    /// The difference.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn sub(self, other: Self) -> Result<Self, NumError>;

    /// The product.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn mul(self, other: Self) -> Result<Self, NumError>;

    /// The quotient.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] for a `Real` divided by zero or an overflow;
    /// [`NumError::DivisorContainsZero`] for an `Interval` divisor holding zero.
    fn div(self, other: Self) -> Result<Self, NumError>;

    /// `self · b + c`, with one rounding for a `Real` (the fused multiply-add).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn mul_add(self, b: Self, c: Self) -> Result<Self, NumError>;

    /// The square, which an `Interval` knows is never negative.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn square(self) -> Result<Self, NumError>;

    /// The square root.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] for a negative `Real`; [`NumError::NegativeSqrt`] for an
    /// `Interval` reaching below zero.
    fn sqrt(self) -> Result<Self, NumError>;

    /// `e` to the power `self`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] on overflow.
    fn exp(self) -> Result<Self, NumError>;

    /// The natural logarithm.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] for a `Real` that is not positive; an `Interval`'s error from
    /// [`Interval::ln`] for one reaching zero or below.
    fn ln(self) -> Result<Self, NumError>;

    /// sin(π · `self`), the sine of an angle in half-turns.
    ///
    /// # Errors
    ///
    /// None for finite values; the `Result` is the shape every operation shares.
    fn sinpi(self) -> Result<Self, NumError>;

    /// cos(π · `self`), the cosine of an angle in half-turns.
    ///
    /// # Errors
    ///
    /// None for finite values; the `Result` is the shape every operation shares.
    fn cospi(self) -> Result<Self, NumError>;

    /// The angle of the point (`x`, `self`) in radians, in (−π, π], IEEE 754's `atan2(self, x)`.
    ///
    /// # Errors
    ///
    /// For an `Interval`, [`Interval::atan2`]'s: [`NumError::OutsideDomain`] for the origin
    /// alone. A dual number also refuses a box on which the angle is not differentiable
    /// ([`crate::dual::Dual`]).
    fn atan2(self, x: Self) -> Result<Self, NumError>;
}

/// The private supertrait that seals [`Scalar`], and the one question a derivative rule needs
/// answered that the operations above do not.
pub(crate) mod sealed {
    use crate::error::NumError;

    /// Implemented only in this crate.
    pub trait Sealed: Sized {
        /// `Ok` when `atan2(y, x)` is continuously differentiable on a neighbourhood, within the
        /// set the arguments stand for, of every pair in it, so that its derivative's formula is
        /// the derivative there and the mean value theorem holds along every segment in the set.
        ///
        /// # Errors
        ///
        /// [`NumError::OutsideDomain`] when the set crosses the branch cut on the negative
        /// x-axis from below, where the angle jumps from near −π to π.
        fn atan2_smooth(y: Self, x: Self) -> Result<(), NumError>;
    }
}

impl sealed::Sealed for Real {
    /// A point: the derivative's formula is the one-sided derivative on the cut, and the origin is
    /// refused by the formula's division.
    fn atan2_smooth(_: Real, _: Real) -> Result<(), NumError> {
        Ok(())
    }
}

impl Scalar for Real {
    fn constant(c: Real) -> Real {
        c
    }

    fn pi() -> Real {
        Real::PI
    }

    fn neg(self) -> Real {
        // A negation never rounds and a finite double's negation is finite: `Real::new` cannot
        // fail, and keeping the value on failure keeps this exact path branch-free.
        Real::new(-self.get()).unwrap_or(self)
    }

    fn add(self, other: Real) -> Result<Real, NumError> {
        Real::new(self.get() + other.get())
    }

    fn sub(self, other: Real) -> Result<Real, NumError> {
        Real::new(self.get() - other.get())
    }

    fn mul(self, other: Real) -> Result<Real, NumError> {
        Real::new(self.get() * other.get())
    }

    fn div(self, other: Real) -> Result<Real, NumError> {
        Real::new(self.get() / other.get())
    }

    fn mul_add(self, b: Real, c: Real) -> Result<Real, NumError> {
        math::fma(self.get(), b.get(), c.get())
    }

    fn square(self) -> Result<Real, NumError> {
        Real::new(self.get() * self.get())
    }

    fn sqrt(self) -> Result<Real, NumError> {
        math::sqrt(self.get())
    }

    fn exp(self) -> Result<Real, NumError> {
        math::exp(self.get())
    }

    fn ln(self) -> Result<Real, NumError> {
        math::ln(self.get())
    }

    fn sinpi(self) -> Result<Real, NumError> {
        math::sinpi(self.get())
    }

    fn cospi(self) -> Result<Real, NumError> {
        math::cospi(self.get())
    }

    fn atan2(self, x: Real) -> Result<Real, NumError> {
        math::atan2(self.get(), x.get())
    }
}

impl sealed::Sealed for Interval {
    /// Refused exactly where [`Interval::atan2`] gives the whole range: the box holds points with
    /// x < 0 and y < 0, and reaches y ≥ 0. Every other box lies in the closed upper half-plane, in
    /// the open lower half-plane, or in the half-plane x ≥ 0, and on each of those the angle is the
    /// restriction of a branch that is smooth away from the origin; a box holding the origin is
    /// refused by the derivative's division by x² + y².
    fn atan2_smooth(y: Interval, x: Interval) -> Result<(), NumError> {
        if x.lo() < 0.0 && y.lo() < 0.0 && y.hi() >= 0.0 {
            return Err(NumError::OutsideDomain);
        }
        Ok(())
    }
}

impl Scalar for Interval {
    fn constant(c: Real) -> Interval {
        Interval::from(c)
    }

    fn pi() -> Interval {
        Interval::PI
    }

    fn neg(self) -> Interval {
        Interval::neg(self)
    }

    fn add(self, other: Interval) -> Result<Interval, NumError> {
        Interval::add(self, other)
    }

    fn sub(self, other: Interval) -> Result<Interval, NumError> {
        Interval::sub(self, other)
    }

    fn mul(self, other: Interval) -> Result<Interval, NumError> {
        Interval::mul(self, other)
    }

    fn div(self, other: Interval) -> Result<Interval, NumError> {
        Interval::div(self, other)
    }

    fn mul_add(self, b: Interval, c: Interval) -> Result<Interval, NumError> {
        Interval::mul_add(self, b, c)
    }

    fn square(self) -> Result<Interval, NumError> {
        Interval::square(self)
    }

    fn sqrt(self) -> Result<Interval, NumError> {
        Interval::sqrt(self)
    }

    fn exp(self) -> Result<Interval, NumError> {
        Interval::exp(self)
    }

    fn ln(self) -> Result<Interval, NumError> {
        Interval::ln(self)
    }

    fn sinpi(self) -> Result<Interval, NumError> {
        Interval::sinpi(self)
    }

    fn cospi(self) -> Result<Interval, NumError> {
        Interval::cospi(self)
    }

    fn atan2(self, x: Interval) -> Result<Interval, NumError> {
        Interval::atan2(self, x)
    }
}
