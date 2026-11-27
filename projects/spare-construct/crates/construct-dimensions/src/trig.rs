//! Angles, arcs and points on circles in whole numbers: the angle between two directions in
//! construct-params' angle units, an arc's length and a point at an angle on a circle, each the
//! nearest whole unit or step to the true value, with a bound proved here and tested against
//! values worked out by other methods (`tests/trig.rs`).
//!
//! **Exact where it can be.** An angle between two directions of whole steps is a whole number of
//! angle units (1/9,000,000 of a degree) only when it is a multiple of 45 degrees: such an angle
//! is a rational multiple of pi with a rational tangent, and by Niven's theorem (I. Niven,
//! "Irrational Numbers", 1956, corollary 3.12) the only such tangents are 0, 1 and -1 (and the
//! right angle's, which has none). Those are found exactly from the whole-number cross and dot
//! products ([`between`]: a zero cross product, a zero dot product, or the two of the same size)
//! and given as exact; every other angle is irrational in degrees, and is given as the nearest
//! whole unit, marked not exact. An arc's length is pi times a whole number, so it is never a
//! whole number of steps and is always given as the nearest step.
//!
//! **The bound.** Everything is done on whole numbers of any size (num-bigint), in fixed point
//! with [`WORK`] bits after the point, every division of numbers not below zero rounding down:
//! - `atan(b / a)` for `0 <= b <= a`, by Euler's series `atan x = sum over n of
//!   (2^2n (n!)^2 / (2n+1)!) x^(2n+1) / (1+x^2)^(n+1)` (L. Euler, 1755). With `x = b/a` its first
//!   term is `ab / (a^2+b^2)` and each next term is the last times `(2n / (2n+1)) b^2 / (a^2+b^2)`,
//!   a ratio under one half, so each term is one floor division of whole numbers: the error of
//!   each is under one plus half the last's, so under 2 of the last bits; there are at most
//!   [`WORK`] terms before one is zero, and the tail then is under 4. So `atan` is low by under
//!   `2 WORK + 4` = 260 of the last bits, under 2^9.
//! - pi by Machin's formula, `16 atan(1/5) - 4 atan(1/239)` (J. Machin, 1706), each arctangent by
//!   the series above (29 and 9 terms): within 16 x 62 + 4 x 22, under 2^11 of the last bits.
//! - the angle of a direction from its octant's arctangent and pi: within 2^12 of the last bits;
//!   the angle in units, `angle x 1,620,000,000 / pi`, is then within 2^-80 of a unit of the true
//!   one before it is rounded once, to the nearest unit, a half up.
//! - sine and cosine by their Taylor series on an eighth of a turn at most (each term from the
//!   last, under 3 of the last bits each over fewer than 40 terms), after taking whole quarter
//!   turns away: within 2^13 of the last bits; a point at a radius of up to 2^41 steps is then
//!   within 2^-70 of a step of the true one before it is rounded once.
//! - an arc's length, its radius times its sweep in radians: within 2^-70 of a step before it is
//!   rounded once.
//!
//! So an angle given is within half a unit and 2^-80 of a unit of the true angle (under 10^-24 of
//! a degree beyond the half unit), and a length or a coordinate within half a step and 2^-70 of a
//! step. Nothing here reads a float, a clock or anything that differs between machines.
//!
//! Why this is written here and not taken from a library: the project's rule is a bound proved in
//! whole numbers (no floating point in anything compared), and no permissively licensed library
//! gives an arctangent as a whole number with such a bound; num-bigint (MIT OR Apache-2.0) does
//! the arithmetic and the series are the textbook ones. Piece A80's `construct-modify` holds a
//! half-tangent by the same rule for turning elements; the two answer different questions (an
//! angle found from two directions here, a rotation made from an angle there).

#![allow(
    clippy::arithmetic_side_effects,
    reason = "whole numbers of any size cannot overflow, and every divisor here is above zero"
)]

use num_bigint::BigInt;
use std::sync::OnceLock;

/// Bits after the point of the working: an angle in radians, or a sine, is held as `n / 2^WORK`.
pub const WORK: u32 = 128;

/// Angle units in half a turn: 180 degrees of construct-params' 9,000,000 units.
pub const HALF_TURN: i64 = 1_620_000_000;

/// Angle units in a full turn.
pub const FULL_TURN: i64 = 3_240_000_000;

fn big(value: i64) -> BigInt {
    BigInt::from(value)
}

fn one() -> BigInt {
    BigInt::from(1) << WORK
}

/// `atan(b / a)` for `0 <= b <= a`, `a > 0`, in the working's fixed point, by Euler's series: low
/// by under 2^9 of the last bits (the module header).
fn atan_ratio(a: &BigInt, b: &BigInt) -> BigInt {
    let bb = b * b;
    let sum = a * a + &bb;
    let mut term = ((a * b) << WORK) / &sum;
    let mut total = BigInt::ZERO;
    let mut n: u64 = 0;
    while term > BigInt::ZERO {
        total += &term;
        n += 1;
        let up = BigInt::from(2 * n) * &bb;
        let down = BigInt::from(2 * n + 1) * &sum;
        term = term * up / down;
    }
    total
}

/// Pi in the working's fixed point, by Machin's formula; worked out once.
fn pi() -> &'static BigInt {
    static PI: OnceLock<BigInt> = OnceLock::new();
    PI.get_or_init(|| atan_ratio(&big(5), &big(1)) * 16 - atan_ratio(&big(239), &big(1)) * 4)
}

/// The angle of the direction `(x, y)`, anticlockwise from `+x`, in `[0, 2 pi)`, in the
/// working's fixed point; `None` for the zero vector.
#[must_use]
pub fn direction(x: &BigInt, y: &BigInt) -> Option<BigInt> {
    if *x == BigInt::ZERO && *y == BigInt::ZERO {
        return None;
    }
    let ax = BigInt::from(x.magnitude().clone());
    let ay = BigInt::from(y.magnitude().clone());
    let pi = pi();
    let first = if ay <= ax {
        atan_ratio(&ax, &ay)
    } else {
        (pi >> 1u32) - atan_ratio(&ay, &ax)
    };
    Some(match (*x >= BigInt::ZERO, *y >= BigInt::ZERO) {
        (true, true) => first,
        (false, true) => pi - first,
        (false, false) => pi + first,
        (true, false) => (pi << 1u32) - first,
    })
}

/// The direction of a vector of whole steps, in radians in the working's fixed point.
#[must_use]
pub fn direction_of(v: [i64; 2]) -> Option<BigInt> {
    let [x, y] = v;
    direction(&big(x), &big(y))
}

/// How an angle between two directions was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Between {
    /// The angle, in angle units, `0` to [`HALF_TURN`]: the nearest whole unit to the true one.
    pub units: i64,
    /// Whether that is the true angle (a multiple of 45 degrees: the module header).
    pub exact: bool,
    /// Whether the second direction is anticlockwise of the first (the cross product above zero).
    pub anticlockwise: bool,
}

/// The angle between the directions `u` and `v` (vectors of whole steps, neither zero), from 0 to
/// a half turn; `None` for a zero vector.
#[must_use]
pub fn between(u: [i64; 2], v: [i64; 2]) -> Option<Between> {
    let [ux, uy] = u.map(big);
    let [vx, vy] = v.map(big);
    if (ux == BigInt::ZERO && uy == BigInt::ZERO) || (vx == BigInt::ZERO && vy == BigInt::ZERO) {
        return None;
    }
    let cross = &ux * &vy - &uy * &vx;
    let dot = &ux * &vx + &uy * &vy;
    let anticlockwise = cross > BigInt::ZERO;
    let exact = |units: i64| {
        Some(Between {
            units,
            exact: true,
            anticlockwise,
        })
    };
    if cross == BigInt::ZERO {
        return exact(if dot > BigInt::ZERO { 0 } else { HALF_TURN });
    }
    if dot == BigInt::ZERO {
        return exact(HALF_TURN / 2);
    }
    if cross.magnitude() == dot.magnitude() {
        return exact(if dot > BigInt::ZERO {
            HALF_TURN / 4
        } else {
            HALF_TURN / 4 * 3
        });
    }
    let across = BigInt::from(cross.magnitude().clone());
    let theta = direction(&dot, &across)?;
    Some(Between {
        units: units_of(&theta)?,
        exact: false,
        anticlockwise,
    })
}

/// The nearest whole number of angle units to an angle not below zero given in radians in the
/// working's fixed point (a half rounding up); `None` below zero or past 64 bits.
#[must_use]
pub fn units_of(theta: &BigInt) -> Option<i64> {
    if *theta < BigInt::ZERO {
        return None;
    }
    let pi = pi();
    // Both sides are above zero, so the division rounds down.
    let units = (theta * big(HALF_TURN) * 2 + pi) / (pi * 2);
    i64::try_from(&units).ok()
}

/// An angle of whole angle units, not below zero, in radians in the working's fixed point
/// (rounded down); `None` below zero.
#[must_use]
pub fn radians_of(units: i64) -> Option<BigInt> {
    if units < 0 {
        return None;
    }
    Some(pi() * big(units) / big(HALF_TURN))
}

/// The sine and cosine of `theta`, `0 <= theta`, given and given back in the working's fixed
/// point.
fn sin_cos(theta: &BigInt) -> (BigInt, BigInt) {
    let quarter = pi() >> 1u32;
    // Not below zero, so the division rounds down.
    let turns = theta / &quarter;
    let x = theta - &turns * &quarter;
    // A quarter turn's second half is its complement's first: sin x = cos (pi/2 - x).
    let (sin, cos) = if x > (pi() >> 2u32) {
        let (s, c) = taylor(&(&quarter - &x));
        (c, s)
    } else {
        taylor(&x)
    };
    let quarters = &turns % 4;
    if quarters == big(1) {
        (cos, -sin)
    } else if quarters == big(2) {
        (-sin, -cos)
    } else if quarters == big(3) {
        (-cos, sin)
    } else {
        (sin, cos)
    }
}

/// The sine and cosine of `x`, `0 <= x <= pi/4`, by their Taylor series.
fn taylor(x: &BigInt) -> (BigInt, BigInt) {
    let x2 = (x * x) >> WORK;
    let series = |first: BigInt, start: u64| {
        let mut sum = first.clone();
        let mut term = first;
        let mut k = start;
        let mut add = false;
        loop {
            // The next term is the last times x^2 over the next two whole numbers.
            term = ((&term * &x2) >> WORK) / BigInt::from(k * (k + 1));
            if term == BigInt::ZERO {
                return sum;
            }
            if add {
                sum += &term;
            } else {
                sum -= &term;
            }
            add = !add;
            k += 2;
        }
    };
    (series(x.clone(), 2), series(one(), 1))
}

/// `value / 2^WORK` to the nearest whole number, a half rounding up; `None` past 64 bits.
fn nearest(value: &BigInt) -> Option<i64> {
    // A right shift of a whole number below zero rounds down too, so this is the floor of the
    // value and a half.
    let rounded = (value + (BigInt::from(1) << (WORK - 1))) >> WORK;
    i64::try_from(&rounded).ok()
}

/// The point at `units` (anticlockwise from `+x`) on the circle about `centre` of `radius` steps,
/// each coordinate the nearest step; `None` past 64 bits.
#[must_use]
pub fn point_at(centre: [i64; 2], radius: i64, units: i64) -> Option<[i64; 2]> {
    let turned = units.rem_euclid(FULL_TURN);
    point_at_radians(centre, radius, &radians_of(turned)?)
}

/// The point at an angle given in radians in the working's fixed point; `None` below zero or past
/// 64 bits.
#[must_use]
pub fn point_at_radians(centre: [i64; 2], radius: i64, theta: &BigInt) -> Option<[i64; 2]> {
    if *theta < BigInt::ZERO {
        return None;
    }
    let (sin, cos) = sin_cos(theta);
    let [cx, cy] = centre;
    let x = nearest(&(big(radius) * cos))?;
    let y = nearest(&(big(radius) * sin))?;
    Some([cx.checked_add(x)?, cy.checked_add(y)?])
}

/// The length of an arc of `radius` steps sweeping `units` angle units, both not below zero, the
/// nearest step; `None` below zero or past 64 bits.
#[must_use]
pub fn arc_length(radius: i64, units: i64) -> Option<i64> {
    if radius < 0 || units < 0 {
        return None;
    }
    let whole = pi() * big(radius) * big(units);
    // One rounding: (whole / HALF_TURN) / 2^WORK, a half up; not below zero, so it rounds down.
    let scale = big(HALF_TURN) << WORK;
    let rounded = (whole * 2 + &scale) / (scale * 2);
    i64::try_from(&rounded).ok()
}

/// Half way round from the direction `from` (radians, fixed point, not below zero) by
/// `sweep_units` angle units, anticlockwise or clockwise: the direction of an arc's middle, in
/// `[0, 2 pi)`; `None` for a sweep below zero.
#[must_use]
pub fn middle_of(from: &BigInt, sweep_units: i64, anticlockwise: bool) -> Option<BigInt> {
    let half = radians_of(sweep_units)? >> 1u32;
    let turn = pi() << 1u32;
    let mid = if anticlockwise {
        from + half
    } else {
        from - half + &turn
    };
    Some(((mid % &turn) + &turn) % &turn)
}
