//! Double-double balls: a real number known to lie within a radius of a double-double centre, the
//! arithmetic the root-isolation engine falls back to where enclosures in doubles are too wide.
//!
//! What it does: a [`Ball`] is a centre, a [`DD`], and a radius, a double rounded up; it stands for
//! every real within the radius of the centre. Each operation computes its centre by the [`DD`]
//! operation of the same name, and its radius from the operands' centres and radii
//! (midpoint-radius arithmetic) plus that [`DD`] operation's proven error bound, every step of the
//! radius rounded up. `Ball` implements [`Scalar`], so a residual written once runs in it, and
//! [`crate::dual::Dual`] over it gives enclosures of Jacobians. Near a root, where a residual is
//! the small difference of large terms, an enclosure in doubles is as wide as the rounding of the
//! terms, while a ball is about 2^−53 times narrower; [`crate::isolate`] evaluates a box in balls
//! when the evaluation in doubles is not precise enough (chapter 12.3: "that box is evaluated in
//! double-double arithmetic").
//!
//! Guarantees:
//!
//! - **Enclosure.** Every operation returns a ball holding the exact result of the operation on
//!   every pair of reals in its operands (on every real in its operand, for the functions of one
//!   argument). Each operation's documentation gives the bound its radius adds; with the exact
//!   centres `a`, `b` and radii `r`, `s` of the operands:
//!   - sum and difference: `r + s`;
//!   - product: `|a|·s + |b|·r + r·s`, from `xy − ab = a·δy + b·δx + δx·δy`; the square
//!     `2|a|·r + r²`;
//!   - quotient: `(r·|b| + |a|·s) / (|b|·(|b| − s))`, from
//!     `x/y − a/b = (δx·b − a·δy)/(y·b)` with `|y| ≥ |b| − s > 0`;
//!   - square root: the smaller of `r/√a` and `√r`, from `|√x − √a| = |x − a|/(√x + √a)` and
//!     the concavity of the root.
//!
//!   To that it adds the [`DD`] operation's rounding: an operation proven within `B·|z| + A` of the
//!   exact result `z` that returned `c` is within `2B·|c| + 2A` of it, because
//!   `|z| ≤ (|c| + A)/(1 − B) ≤ 2(|c| + A)` for `B ≤ 1/2` (every bound of [`DD`] is below 10⁻³⁰),
//!   so the error is at most `2B·(|c| + A) + A ≤ 2B·|c| + 2A`.
//!   Every radius step is a double operation rounded to nearest and then moved one double up,
//!   which is at or above the exact result; bounds on a centre's magnitude are read from its two
//!   parts the same way ([`DD`]'s invariant puts the value within half a step of its leading part).
//! - **Where the precision stops.** `exp`, `ln`, `sinpi`, `cospi` and `atan2` have proven versions
//!   in doubles only ([`crate::math`]), so a ball takes them through [`Interval`]: the ball is
//!   enclosed in an interval, the certified interval function applied, and the result made a ball
//!   again. Those results are as wide as the interval ones, still certified. π is the double
//!   nearest to it with radius 2^−52, above its distance from π (about 1.22 · 10⁻¹⁶).
//! - **A ball cannot say a square is never negative.** A square whose ball reaches zero still
//!   reaches below it, so a square root of it is refused (`NegativeSqrt`), where an interval would
//!   take it; over such a box the derivative of the root is refused anyway ([`crate::dual`]), so
//!   the operators lose nothing they could otherwise prove.
//! - **Never NaN, never infinite.** The centre is a [`DD`], finite by construction; a radius that
//!   overflows is [`NumError::NotFinite`]. A divisor ball reaching zero is
//!   [`NumError::DivisorContainsZero`], and the square root of a ball reaching below zero
//!   [`NumError::NegativeSqrt`], the errors [`Interval`] gives.
//! - **Deterministic and commutative.** Only [`DD`]'s operations, double `+`, `−`, `×` and `÷`
//!   rounded to nearest, `next_up` and `next_down`, and [`crate::math`]'s correctly rounded square
//!   root, so every target gives the same bits. Sums and products have the same bits in either
//!   order, as [`DD`]'s do.
//!
//! Method: midpoint-radius interval arithmetic, also called ball arithmetic: S. M. Rump, "Fast and
//! parallel interval arithmetic", *BIT Numerical Mathematics* 39(3) (1999),
//! doi:10.1023/A:1022374804152, and J. van der Hoeven, "Ball arithmetic" (2009),
//! <https://hal.science/hal-00432152>, with the centre in double-double ([`DD`], whose error bounds
//! are proven and machine-checked) and the radius in doubles rounded up with `next_up`, since
//! neither Rust nor WebAssembly can change the rounding mode. The radius formulas are the standard
//! ones those papers give, re-derived above. Written for this kernel: no published Rust crate has a
//! double-double ball with proven bounds.
//!
//! Cost: one [`DD`] operation and a few double operations per operation; the maths functions add
//! an interval enclosure on each side. No allocation, no recursion.

use crate::dd::{self, DD};
use crate::error::NumError;
use crate::interval::{Interval, min2};
use crate::math;
use crate::real::Real;
use crate::scalar::{Scalar, sealed};

/// A radius above π's distance from [`Real::PI`], the double nearest to it: 2^−52 ≈ 2.2 · 10⁻¹⁶,
/// against about 1.2246 · 10⁻¹⁶ (`tests/attack_g53.rs` checks it against π's digits).
const PI_RADIUS: f64 = f64::from_bits(0x3CB0_0000_0000_0000);

/// A real number within `rad` of `mid` (module documentation).
///
/// Made only by [`Ball::from_interval`], [`Scalar::constant`], [`Scalar::pi`] and the operations,
/// so its radius is always finite and not negative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    mid: DD,
    rad: f64,
}

/// A double at or above `a + b`: the sum rounded to nearest, then one double up. Infinite on
/// overflow, which [`Ball::made`] refuses.
fn add_up(a: f64, b: f64) -> f64 {
    (a + b).next_up()
}

/// A double at or above `a · b`, as [`add_up`].
fn mul_up(a: f64, b: f64) -> f64 {
    (a * b).next_up()
}

/// A double at or above `a / b`, for `b > 0`, as [`add_up`].
fn div_up(a: f64, b: f64) -> f64 {
    (a / b).next_up()
}

/// A double at or above `a − b`, as [`add_up`].
fn sub_up(a: f64, b: f64) -> f64 {
    (a - b).next_up()
}

/// A double at or below `a − b`: the difference rounded to nearest, then one double down.
fn sub_down(a: f64, b: f64) -> f64 {
    (a - b).next_down()
}

/// A double at or below the value of `d`: its leading part, or the double below it when the
/// trailing part is negative. The leading part is the value rounded to nearest, so the value lies
/// within half the gap to the next double on either side.
fn below(d: DD) -> f64 {
    if d.lo() < 0.0 {
        d.hi().next_down()
    } else {
        d.hi()
    }
}

/// A double at or above the value of `d`, as [`below`].
fn above(d: DD) -> f64 {
    if d.lo() > 0.0 {
        d.hi().next_up()
    } else {
        d.hi()
    }
}

/// A double at or above `|d|`.
fn mag(d: DD) -> f64 {
    above(d.abs())
}

/// A bound on how far `c`, returned by a [`DD`] operation proven within `b·|z| + a` of its exact
/// result `z`, lies from `z`: `2b·|c| + 2a` (module documentation). Both doublings are exact:
/// `b` is below 10⁻³⁰ and `a` is 0 or 2^−1074.
fn rounding(c: DD, b: f64, a: f64) -> f64 {
    add_up(mul_up(2.0 * b, mag(c)), 2.0 * a)
}

impl Ball {
    /// The ball with centre `mid` and radius `rad`, or `NotFinite` when the radius overflowed.
    fn made(mid: DD, rad: f64) -> Result<Ball, NumError> {
        if rad.is_finite() {
            Ok(Ball { mid, rad })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// A ball holding the interval `x`: centre `(lo + hi)/2` in double-double, radius half the
    /// width plus that centre's rounding. A point interval gives the exact ball of radius 0.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] when `lo + hi` overflows.
    pub fn from_interval(x: Interval) -> Result<Ball, NumError> {
        if x.lo() == x.hi() {
            return Ok(Ball {
                mid: DD::from_f64(x.lo())?,
                rad: 0.0,
            });
        }
        let mid = DD::from_sum(x.lo(), x.hi())?.mul_f64(0.5)?;
        let half = mul_up(sub_up(x.hi(), x.lo()), 0.5);
        Ball::made(
            mid,
            add_up(half, rounding(mid, dd::MUL_F64_BOUND, dd::UNDERFLOW_BOUND)),
        )
    }

    /// An interval holding the ball: `[centre − radius, centre + radius]`, each bound rounded
    /// outward.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] when a bound overflows.
    pub fn to_interval(self) -> Result<Interval, NumError> {
        Interval::new(
            sub_down(below(self.mid), self.rad),
            add_up(above(self.mid), self.rad),
        )
    }

    /// The centre.
    #[must_use]
    pub const fn mid(self) -> DD {
        self.mid
    }

    /// The radius: finite and not negative.
    #[must_use]
    pub const fn rad(self) -> f64 {
        self.rad
    }

    /// The result of an interval function on the ball (module documentation, "Where the precision
    /// stops").
    fn through(
        self,
        f: impl FnOnce(Interval) -> Result<Interval, NumError>,
    ) -> Result<Ball, NumError> {
        Ball::from_interval(f(self.to_interval()?)?)
    }
}

impl sealed::Sealed for Ball {
    /// Refused where the interval of the ball is: the angle's cut, from below.
    fn atan2_smooth(y: Ball, x: Ball) -> Result<(), NumError> {
        <Interval as sealed::Sealed>::atan2_smooth(y.to_interval()?, x.to_interval()?)
    }
}

impl Scalar for Ball {
    fn constant(c: Real) -> Ball {
        Ball {
            mid: DD::from_real(c),
            rad: 0.0,
        }
    }

    fn pi() -> Ball {
        Ball {
            mid: DD::from_real(Real::PI),
            rad: PI_RADIUS,
        }
    }

    fn neg(self) -> Ball {
        Ball {
            mid: self.mid.negate(),
            rad: self.rad,
        }
    }

    fn add(self, other: Ball) -> Result<Ball, NumError> {
        let mid = self.mid.add(other.mid)?;
        Ball::made(
            mid,
            add_up(
                add_up(self.rad, other.rad),
                rounding(mid, dd::ADD_BOUND, 0.0),
            ),
        )
    }

    fn sub(self, other: Ball) -> Result<Ball, NumError> {
        let mid = self.mid.sub(other.mid)?;
        Ball::made(
            mid,
            add_up(
                add_up(self.rad, other.rad),
                rounding(mid, dd::ADD_BOUND, 0.0),
            ),
        )
    }

    fn mul(self, other: Ball) -> Result<Ball, NumError> {
        let mid = self.mid.mul(other.mid)?;
        let cross = add_up(
            mul_up(mag(self.mid), other.rad),
            mul_up(mag(other.mid), self.rad),
        );
        let spread = add_up(cross, mul_up(self.rad, other.rad));
        Ball::made(
            mid,
            add_up(spread, rounding(mid, dd::MUL_BOUND, dd::UNDERFLOW_BOUND)),
        )
    }

    fn div(self, other: Ball) -> Result<Ball, NumError> {
        let size = below(other.mid.abs());
        let gap = sub_down(size, other.rad);
        if gap <= 0.0 {
            return Err(NumError::DivisorContainsZero);
        }
        let mid = self.mid.div(other.mid)?;
        let cross = add_up(
            mul_up(self.rad, mag(other.mid)),
            mul_up(mag(self.mid), other.rad),
        );
        // `size ≥ gap > 0`, so neither division is by zero; dividing twice cannot underflow to a
        // zero denominator as the product `size · gap` could.
        let spread = div_up(div_up(cross, size), gap);
        Ball::made(
            mid,
            add_up(spread, rounding(mid, dd::DIV_BOUND, dd::UNDERFLOW_BOUND)),
        )
    }

    fn mul_add(self, b: Ball, c: Ball) -> Result<Ball, NumError> {
        self.mul(b)?.add(c)
    }

    fn square(self) -> Result<Ball, NumError> {
        let mid = self.mid.mul(self.mid)?;
        let cross = mul_up(mag(self.mid), self.rad);
        let spread = add_up(add_up(cross, cross), mul_up(self.rad, self.rad));
        Ball::made(
            mid,
            add_up(spread, rounding(mid, dd::MUL_BOUND, dd::UNDERFLOW_BOUND)),
        )
    }

    fn sqrt(self) -> Result<Ball, NumError> {
        // The ball's lowest real, centre − radius, is at least `floor − radius`: refused unless
        // that is not negative, by an exact comparison.
        let floor = below(self.mid);
        if floor < self.rad {
            return Err(NumError::NegativeSqrt);
        }
        let mid = self.mid.sqrt()?;
        let by_root = math::sqrt(self.rad)?.get().next_up();
        // A double at or below √a. It is positive whenever `floor` is (√floor ≥ 2^−537), which
        // a nonzero radius ensures (`floor ≥ radius`); only the exact ball at zero has none, and
        // its spread is zero, which `by_root` bounds.
        let root_floor = math::sqrt(floor)?.get().next_down();
        let spread = if root_floor > 0.0 {
            min2(div_up(self.rad, root_floor), by_root)
        } else {
            by_root
        };
        Ball::made(mid, add_up(spread, rounding(mid, dd::SQRT_BOUND, 0.0)))
    }

    fn exp(self) -> Result<Ball, NumError> {
        self.through(Interval::exp)
    }

    fn ln(self) -> Result<Ball, NumError> {
        self.through(Interval::ln)
    }

    fn sinpi(self) -> Result<Ball, NumError> {
        self.through(Interval::sinpi)
    }

    fn cospi(self) -> Result<Ball, NumError> {
        self.through(Interval::cospi)
    }

    fn atan2(self, x: Ball) -> Result<Ball, NumError> {
        let x = x.to_interval()?;
        self.through(|y| y.atan2(x))
    }
}
