//! Forward-mode dual numbers over the kernel's own arithmetic: a value carried together with its
//! derivatives along `N` directions, so that one evaluation of a residual gives its Jacobian.
//!
//! What it does: a [`Dual<T, N>`] holds a value `v` and `N` partial derivatives `∂v/∂xⱼ`, all in a
//! [`Scalar`] `T`. Every operation applies the chain rule to them, with the derivative rule of each
//! function written once below, and is itself a [`Scalar`], so a residual written over `Scalar`
//! (`crate::scalar`) runs on dual numbers unchanged. [`Dual::variables`] seeds `N` unknowns with
//! the unit directions, and [`jacobian`] runs a function on them and reads the values and the
//! Jacobian off the result. Over [`Real`] that is the Jacobian at a point, rounded; over
//! [`Interval`] it is an enclosure of the Jacobian over a box, which the interval Newton operators
//! of [`crate::newton`] need. Chapter 15.4 asks for exactly this: derivatives "from forward-mode
//! automatic differentiation ... over the same code and the kernel's own maths functions".
//!
//! The derivative rules, with `u` the operand's value, `p` one of its partials and `w` the result's
//! value: `(a ± b)′ = a′ ± b′`; `(ab)′ = a·b′ + b·a′`; `(a/b)′ = (a′ − w·b′)/b`;
//! `(ab + c)′ = a·b′ + (b·a′ + c′)`; `(u²)′ = (u + u)·p`; `√u′ = p/(w + w)`; `exp′ = w·p`;
//! `ln′ = p/u`; `sinpi′ = π·cospi(u)·p`; `cospi′ = −(π·sinpi(u))·p`; and
//! `atan2(y, x)′ = (x·y′ − y·x′)/(x² + y²)`.
//!
//! Guarantees:
//!
//! - **Derivatives of the code.** The partials are the derivatives of the function the operations
//!   compute, each operation's rule applied once, in the order the code runs; tests check them
//!   against central differences of the same code at seeded points (`tests/attack_g52.rs`).
//! - **The NaN tripwire.** Over `Real`, every value and every partial is checked as it is made,
//!   so a NaN or an infinity anywhere, in a value or in a derivative, is [`NumError::NotFinite`]
//!   from the operation that made it, in every build (`crate::scalar`). That includes the
//!   derivative's own singularities: the square root at zero, the logarithm's slope at a tiny
//!   argument, and the angle's at the origin, whose rules divide by zero or overflow. A dual number
//!   never holds a NaN.
//! - **Enclosures over a box.** Over `Interval`, the value encloses the function's range over the
//!   box and each partial encloses that partial derivative's range, by induction over the
//!   operations: each rule is evaluated in outward-rounded interval arithmetic on enclosures of
//!   its inputs. That needs the function continuously differentiable on the box, which the rules
//!   themselves enforce: a rule that divides by an interval holding zero (the square root at
//!   zero, the angle at the origin) refuses with [`NumError::DivisorContainsZero`], the logarithm
//!   refuses a box reaching zero, and `atan2` refuses a box crossing its branch cut on the
//!   negative x-axis from below with [`NumError::OutsideDomain`], where [`Interval::atan2`] itself
//!   gives the whole range but the angle jumps by 2π and has no derivative.
//! - **Deterministic.** Only the operations of [`Scalar`], in a fixed order: the same bits on
//!   every target.
//!
//! Method: forward-mode automatic differentiation by operator overloading on dual numbers, as in
//! A. Griewank and A. Walther, *Evaluating Derivatives*, 2nd ed., SIAM, 2008, chapter 3 (the
//! tangent propagation of each elemental function), doi:10.1137/1.9780898717761, with all `N`
//! tangents carried at once (their vector mode, section 3.1); over intervals it is the interval
//! evaluation of derivatives of R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to
//! Interval Analysis*, SIAM, 2009, chapter 8, doi:10.1137/1.9780898717716. Written for this kernel
//! for the reason `crate::scalar` gives.
//!
//! Cost: an operation on a `Dual<T, N>` costs a few operations of `T` on the value and the same
//! again for each of the `N` partials; [`jacobian`] runs the function once. No allocation, no
//! recursion.

use crate::error::NumError;
use crate::real::Real;
use crate::scalar::Scalar;
use crate::scalar::sealed::Sealed;

#[cfg(doc)]
use crate::interval::Interval;

/// A value and its partial derivatives along `N` directions, in the scalar `T`.
///
/// Made by [`Dual::new`], by [`Dual::variables`] or by the operations of [`Scalar`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dual<T, const N: usize> {
    value: T,
    partials: [T; N],
}

impl<T: Scalar, const N: usize> Dual<T, N> {
    /// The dual number with this value and these partial derivatives.
    #[must_use]
    pub const fn new(value: T, partials: [T; N]) -> Dual<T, N> {
        Dual { value, partials }
    }

    /// `N` unknowns at `x`: unknown `i` has value `x[i]`, derivative 1 along direction `i` and 0
    /// along the others, so a function of them carries its Jacobian's rows.
    #[must_use]
    pub fn variables(x: [T; N]) -> [Dual<T, N>; N] {
        let zero = T::constant(Real::ZERO);
        let one = T::constant(Real::ONE);
        let mut out = [Dual::new(zero, [zero; N]); N];
        for (i, (d, v)) in out.iter_mut().zip(x).enumerate() {
            d.value = v;
            for (j, p) in d.partials.iter_mut().enumerate() {
                if i == j {
                    *p = one;
                }
            }
        }
        out
    }

    /// The value.
    #[must_use]
    pub const fn value(self) -> T {
        self.value
    }

    /// The partial derivatives, one per direction.
    #[must_use]
    pub const fn partials(self) -> [T; N] {
        self.partials
    }

    /// This number's partials, each mapped by `f`, in order; the first error is returned.
    fn map(self, mut f: impl FnMut(T) -> Result<T, NumError>) -> Result<[T; N], NumError> {
        let mut out = self.partials;
        for p in &mut out {
            *p = f(*p)?;
        }
        Ok(out)
    }

    /// This number's and `other`'s partials, paired and mapped by `f`, in order.
    fn zip(
        self,
        other: Dual<T, N>,
        mut f: impl FnMut(T, T) -> Result<T, NumError>,
    ) -> Result<[T; N], NumError> {
        let mut out = self.partials;
        for (p, q) in out.iter_mut().zip(other.partials) {
            *p = f(*p, q)?;
        }
        Ok(out)
    }
}

/// The values and the Jacobian of `f` at `x`: runs `f` once on [`Dual::variables`]`(x)` and
/// returns its `M` values and the `M × N` matrix whose row `i` holds the partials of output `i`.
///
/// Over [`Real`] these are the rounded value and Jacobian at the point; over [`Interval`],
/// enclosures of the function's range and of its Jacobian over the box (module documentation).
///
/// # Errors
///
/// Whatever `f` returns, including the tripwire's [`NumError::NotFinite`] and the refusals of a
/// derivative over a box where the function is not differentiable.
#[allow(
    clippy::type_complexity,
    reason = "the values and the Jacobian's rows are the natural return of this function"
)]
pub fn jacobian<T, F, const N: usize, const M: usize>(
    f: F,
    x: [T; N],
) -> Result<([T; M], [[T; N]; M]), NumError>
where
    T: Scalar,
    F: FnOnce([Dual<T, N>; N]) -> Result<[Dual<T, N>; M], NumError>,
{
    let out = f(Dual::variables(x))?;
    Ok((out.map(Dual::value), out.map(Dual::partials)))
}

impl<T: Scalar, const N: usize> Sealed for Dual<T, N> {
    fn atan2_smooth(y: Dual<T, N>, x: Dual<T, N>) -> Result<(), NumError> {
        T::atan2_smooth(y.value, x.value)
    }
}

impl<T: Scalar, const N: usize> Scalar for Dual<T, N> {
    fn constant(c: Real) -> Dual<T, N> {
        Dual::new(T::constant(c), [T::constant(Real::ZERO); N])
    }

    fn pi() -> Dual<T, N> {
        Dual::new(T::pi(), [T::constant(Real::ZERO); N])
    }

    fn neg(self) -> Dual<T, N> {
        Dual::new(self.value.neg(), self.partials.map(Scalar::neg))
    }

    fn add(self, other: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        let value = self.value.add(other.value)?;
        Ok(Dual::new(value, self.zip(other, T::add)?))
    }

    fn sub(self, other: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        let value = self.value.sub(other.value)?;
        Ok(Dual::new(value, self.zip(other, T::sub)?))
    }

    fn mul(self, other: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        let (a, b) = (self.value, other.value);
        let value = a.mul(b)?;
        Ok(Dual::new(
            value,
            self.zip(other, |p, q| a.mul_add(q, b.mul(p)?))?,
        ))
    }

    fn div(self, other: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        let b = other.value;
        let w = self.value.div(b)?;
        let partials = self.zip(other, |p, q| w.neg().mul_add(q, p)?.div(b))?;
        Ok(Dual::new(w, partials))
    }

    fn mul_add(self, b: Dual<T, N>, c: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        let (av, bv) = (self.value, b.value);
        let value = av.mul_add(bv, c.value)?;
        let mut partials = self.partials;
        for ((p, q), r) in partials.iter_mut().zip(b.partials).zip(c.partials) {
            *p = av.mul_add(q, bv.mul_add(*p, r)?)?;
        }
        Ok(Dual::new(value, partials))
    }

    fn square(self) -> Result<Dual<T, N>, NumError> {
        let u = self.value;
        let twice = u.add(u)?;
        Ok(Dual::new(u.square()?, self.map(|p| twice.mul(p))?))
    }

    fn sqrt(self) -> Result<Dual<T, N>, NumError> {
        let w = self.value.sqrt()?;
        let twice = w.add(w)?;
        Ok(Dual::new(w, self.map(|p| p.div(twice))?))
    }

    fn exp(self) -> Result<Dual<T, N>, NumError> {
        let w = self.value.exp()?;
        Ok(Dual::new(w, self.map(|p| w.mul(p))?))
    }

    fn ln(self) -> Result<Dual<T, N>, NumError> {
        let u = self.value;
        Ok(Dual::new(u.ln()?, self.map(|p| p.div(u))?))
    }

    fn sinpi(self) -> Result<Dual<T, N>, NumError> {
        let u = self.value;
        let slope = T::pi().mul(u.cospi()?)?;
        Ok(Dual::new(u.sinpi()?, self.map(|p| slope.mul(p))?))
    }

    fn cospi(self) -> Result<Dual<T, N>, NumError> {
        let u = self.value;
        let slope = T::pi().mul(u.sinpi()?)?.neg();
        Ok(Dual::new(u.cospi()?, self.map(|p| slope.mul(p))?))
    }

    fn atan2(self, x: Dual<T, N>) -> Result<Dual<T, N>, NumError> {
        T::atan2_smooth(self.value, x.value)?;
        let (yv, xv) = (self.value, x.value);
        let value = yv.atan2(xv)?;
        let r2 = xv.square()?.add(yv.square()?)?;
        let minus_y = yv.neg();
        let partials = self.zip(x, |p, q| xv.mul_add(p, minus_y.mul(q)?)?.div(r2))?;
        Ok(Dual::new(value, partials))
    }
}
