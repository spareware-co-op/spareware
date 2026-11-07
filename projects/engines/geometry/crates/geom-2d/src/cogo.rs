//! Core COGO on the grid: forward and inverse, the nearest whole distance, and a traverse's
//! misclosure (17.2).
//!
//! What it does: [`forward`] puts the point a bearing and a distance from another, committed to the
//! grid. [`inverse`] gives the squared distance between two grid points exactly, the distance to
//! the nearest step by [`nearest_root`], and the bearing to the nearest angle unit. [`misclosure`]
//! runs a traverse's legs from a start point and encloses how far it ends from where it should.
//!
//! Guarantees:
//!
//! - **Forward is committed and certified.** The new point is the nearest grid point to the
//!   midpoint of an enclosure of the exact one, ties to even, so the same on every target. Its
//!   bound is at least its distance from the exact point on each axis: half a step plus the maths
//!   bound, rounded up to whole steps (17.2). On the quarter turns it is exact, computed in
//!   integers. [`Forward::is_certain`] says whether every point of the enclosure rounds to it.
//! - **Inverse is exact where it can be.** The squared distance is an exact integer, so "which is
//!   longer" is exact. The distance is its nearest whole square root, decided exactly in integers,
//!   never through a double: past about 14.8 m the squared distance in steps passes 2^53, where a
//!   double stops holding every integer. No tie is possible, since a square root half way between
//!   two integers would square to a quarter. The bearing is [`Bearing::of_vector`]'s: exact on the
//!   axes and diagonals, and otherwise the certified nearest unit. The direction kept as the vector
//!   between the points is exact.
//! - **The misclosure is a certified vector.** Each leg's run is enclosed from the bearing's
//!   certified sine and cosine, the legs are added in the order given, and the box contains the
//!   exact misclosure; its length encloses the exact length. Nothing is distributed: adjusting a
//!   traverse belongs to the survey crates (17.2).
//! - **Charged and stoppable.** A traverse charges the meter one unit per leg, so a long one stops
//!   cleanly when the budget does.
//!
//! Method: forward and inverse COGO and traverse misclosure as chapter 17.2 sets them
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`): bearings clockwise from
//! north, so a leg of length d on bearing β runs d · sin β east and d · cos β north (C. D. Ghilani
//! and P. R. Wolf, *Elementary Surveying: An Introduction to Geomatics*, chapters 10 and 11,
//! <https://www.pearson.com/en-us/subject-catalog/p/elementary-surveying/P200000003266>); on interval
//! arithmetic (Moore, Kearfott and Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! <https://doi.org/10.1137/1.9780898717716>); the integer square root is `u128::isqrt` from Rust's
//! core library, rounded to nearest by the exact comparison 17.2 gives.
//!
//! Cost: constant time for [`forward`], [`inverse`] and [`nearest_root`], no allocation;
//! [`misclosure`] is linear in its legs, with no allocation.

use geom_num::angle::{Bearing, Nearest, QUARTER_TURN};
use geom_num::interval::Interval;
use geom_num::math;
use geom_num::{Exactness, GridStep, NumError, P2, Steps};
use geom_run::{Stopped, WorkMeter};

/// The longest distance [`forward`] takes, 2^42 steps: longer than any line inside a frame, whose
/// corners are less than 2^41.5 steps apart, and exact in a double.
pub const LONGEST: Steps = Steps(1 << 42);

/// Why a COGO construction gave no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CogoError {
    /// The numbers layer refused: a point outside the frame ([`NumError::OutOfRange`]), or an
    /// enclosure it could not form.
    Num(NumError),
    /// A distance longer than [`LONGEST`].
    TooLong,
    /// The meter stopped the traverse.
    Stopped(Stopped),
}

impl From<NumError> for CogoError {
    fn from(e: NumError) -> Self {
        CogoError::Num(e)
    }
}

impl From<Stopped> for CogoError {
    fn from(e: Stopped) -> Self {
        CogoError::Stopped(e)
    }
}

impl core::fmt::Display for CogoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CogoError::Num(e) => write!(f, "{e}"),
            CogoError::TooLong => write!(f, "a distance longer than {} steps", LONGEST.0),
            CogoError::Stopped(e) => write!(f, "{e}"),
        }
    }
}

/// `v` as a double, exactly: every caller passes `|v| ≤ 2^53`, which a double holds exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "every caller passes |v| ≤ 2^53 (a grid coordinate, a difference of two, or a distance \
              of at most LONGEST), which a double holds exactly"
)]
const fn exactly(v: i64) -> f64 {
    v as f64
}

/// A whole double of magnitude at most 2^53 as an `i64`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "every caller passes a whole number of magnitude at most 2^53, checked beside each call"
)]
const fn whole(v: f64) -> i64 {
    v as i64
}

/// One coordinate of a committed point: the nearest whole step to the midpoint of `x`, ties to
/// even; a bound in whole steps on its distance from every value in `x`; and whether every value
/// in `x` rounds to it.
fn committed(x: Interval) -> Result<(i64, u64, bool), CogoError> {
    let (lo, hi) = (x.lo(), x.hi());
    let most = exactly(1 << 53);
    if lo < -most || hi > most {
        return Err(NumError::OutOfRange {
            value: if hi > most { i128::MAX } else { i128::MIN },
        }
        .into());
    }
    let k = math::round_ties_even(lo * 0.5 + hi * 0.5)?.get();
    let certain = math::round_ties_even(lo)?.get() == k && math::round_ties_even(hi)?.get() == k;
    // Each distance to an end, rounded to nearest and moved a step up, is at least the exact
    // distance; both are below 2^54 + 1.
    let (below, above) = ((k - lo).next_up(), (hi - k).next_up());
    let far = math::ceil(if above > below { above } else { below })?.get();
    // |k| ≤ 2^53 and 0 < far ≤ 2^54 + 2.
    Ok((whole(k), whole(far).unsigned_abs(), certain))
}

/// A point put on the grid by a bearing and a distance, with its bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Forward {
    /// The point.
    point: P2,
    /// A bound on its distance from the exact point on each axis, in whole steps.
    bound: Steps,
    /// Whether the exact point certainly rounds to it.
    certain: bool,
}

impl Forward {
    /// The point.
    #[must_use]
    pub const fn point(&self) -> P2 {
        self.point
    }

    /// A bound on the point's distance from the exact point on each axis, in whole steps: 0 when
    /// it is exact.
    #[must_use]
    pub const fn bound(&self) -> Steps {
        self.bound
    }

    /// Whether the point is exactly the one the bearing and distance give.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.bound.0 == 0
    }

    /// Whether the exact point certainly rounds to this one on both axes; when not, it lies near a
    /// half step, and either neighbour may be the nearest grid point.
    #[must_use]
    pub const fn is_certain(&self) -> bool {
        self.certain
    }

    /// The point's exactness class on a grid of `step`: Exact, or Certified within its bound.
    #[must_use]
    pub const fn exactness(&self, step: GridStep) -> Exactness {
        if self.bound.0 == 0 {
            Exactness::Exact
        } else {
            Exactness::Certified {
                bound: self.bound,
                step,
            }
        }
    }
}

/// The point `distance` steps from `from` on `bearing`, committed to the grid: east by
/// `distance · sin β` and north by `distance · cos β`.
///
/// # Errors
///
/// [`CogoError::TooLong`] for a distance over [`LONGEST`]; [`CogoError::Num`] with
/// [`NumError::OutOfRange`] for a point outside the frame.
pub fn forward(from: P2, bearing: Bearing, distance: Steps) -> Result<Forward, CogoError> {
    if distance.0 > LONGEST.0 {
        return Err(CogoError::TooLong);
    }
    // At most 2^42, so it is an i64 and a double exactly.
    let d = i64::try_from(distance.0).map_err(|_| CogoError::TooLong)?;
    let (x, y) = (from.x.get(), from.y.get());
    let units = bearing.units().get();
    if units % QUARTER_TURN == 0 {
        // North, east, south or west: an exact step along one axis. Each sum is within ±2^43.
        let (dx, dy) = match units / QUARTER_TURN {
            0 => (0, d),
            1 => (d, 0),
            2 => (0, d.saturating_neg()),
            _ => (d.saturating_neg(), 0),
        };
        return Ok(Forward {
            point: P2::new(x.saturating_add(dx), y.saturating_add(dy))?,
            bound: Steps(0),
            certain: true,
        });
    }
    let (east, north) = bearing.east_north()?;
    let length = Interval::point(exactly(d))?;
    let at_x = length.mul_add(east, Interval::point(exactly(x))?)?;
    let at_y = length.mul_add(north, Interval::point(exactly(y))?)?;
    let (px, bx, cx) = committed(at_x)?;
    let (py, by, cy) = committed(at_y)?;
    Ok(Forward {
        point: P2::new(px, py)?,
        bound: Steps(if bx > by { bx } else { by }),
        certain: cx && cy,
    })
}

/// A whole square root rounded to the nearest integer, with whether it is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Root {
    /// The nearest integer to the square root.
    value: u128,
    /// Whether it squares to the number exactly.
    exact: bool,
}

impl Root {
    /// The integer nearest the square root.
    #[must_use]
    pub const fn value(&self) -> u128 {
        self.value
    }

    /// Whether the number is a perfect square, so the value is its square root exactly.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.exact
    }
}

/// The integer nearest `√n`, decided exactly: `k = ⌊√n⌋`, and `k + 1` when `n − k² > k`.
///
/// 17.2 compares `(2k + 1)²` with `4n`: `√n` is nearer `k + 1` exactly when
/// `n > (k + ½)² = k² + k + ¼`, which for integers is `n − k² > k`, the same comparison without
/// the products that would overflow for `n` near `u128::MAX`. They are never equal, since
/// `(2k + 1)²` is odd and `4n` even, so there is no tie.
#[must_use]
pub const fn nearest_root(n: u128) -> Root {
    let k = n.isqrt();
    // k² ≤ n, so neither the square nor the difference can overflow; k ≤ 2^64 − 1, so k + 1 fits.
    let rest = n.saturating_sub(k.saturating_mul(k));
    Root {
        value: if rest > k { k.saturating_add(1) } else { k },
        exact: rest == 0,
    }
}

/// The distance and bearing between two grid points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Inverse {
    /// The difference of the points, east then north: the direction, exactly.
    delta: [i64; 2],
    /// The squared distance, exactly.
    squared: u128,
    /// The distance to the nearest step.
    distance: Root,
    /// The bearing from the first point to the second, or `None` when they are one point.
    bearing: Option<Nearest<Bearing>>,
}

impl Inverse {
    /// The vector from the first point to the second, east then north, exactly.
    #[must_use]
    pub const fn delta(&self) -> [i64; 2] {
        self.delta
    }

    /// The squared distance in steps, exactly.
    #[must_use]
    pub const fn squared(&self) -> u128 {
        self.squared
    }

    /// The distance to the nearest step, and whether it is exact.
    #[must_use]
    pub const fn distance(&self) -> Root {
        self.distance
    }

    /// The bearing from the first point to the second, to the nearest angle unit, with its bound;
    /// `None` when the points are the same and there is no direction.
    #[must_use]
    pub const fn bearing(&self) -> Option<Nearest<Bearing>> {
        self.bearing
    }
}

/// The squared distance, the distance and the bearing from `from` to `to`.
///
/// # Errors
///
/// None in practice: two grid points differ by at most 2^41 on each axis, well within what the
/// bearing's enclosure takes; a refusal would be a [`CogoError::Num`].
pub fn inverse(from: P2, to: P2) -> Result<Inverse, CogoError> {
    // Each coordinate is within ±(2^40 − 1), so each difference is within ±2^41 and cannot
    // saturate, and each square is below 2^82.
    let dx = to.x.get().saturating_sub(from.x.get());
    let dy = to.y.get().saturating_sub(from.y.get());
    let square = |v: i64| u128::from(v.unsigned_abs()).saturating_mul(u128::from(v.unsigned_abs()));
    let squared = square(dx).saturating_add(square(dy));
    let bearing = if dx == 0 && dy == 0 {
        None
    } else {
        Some(Bearing::of_vector(dx, dy)?)
    };
    Ok(Inverse {
        delta: [dx, dy],
        squared,
        distance: nearest_root(squared),
        bearing,
    })
}

/// One leg of a traverse: a bearing and a distance in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Leg {
    /// The leg's bearing, clockwise from frame north.
    pub bearing: Bearing,
    /// The leg's length, in steps.
    pub distance: Steps,
}

/// How far a traverse ends from where it should: a certified vector and its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Misclosure {
    /// An enclosure of the exact misclosure east, in steps: where the traverse ends less where it
    /// should.
    east: Interval,
    /// An enclosure of the exact misclosure north, in steps.
    north: Interval,
    /// An enclosure of the misclosure's length, in steps.
    length: Interval,
    /// The legs' total length, in steps, exactly.
    travelled: u128,
}

impl Misclosure {
    /// An enclosure of the misclosure east, in steps.
    #[must_use]
    pub const fn east(&self) -> Interval {
        self.east
    }

    /// An enclosure of the misclosure north, in steps.
    #[must_use]
    pub const fn north(&self) -> Interval {
        self.north
    }

    /// An enclosure of the misclosure's length, in steps.
    #[must_use]
    pub const fn length(&self) -> Interval {
        self.length
    }

    /// The legs' total length, in steps, exactly: with the length, a traverse's precision.
    #[must_use]
    pub const fn travelled(&self) -> u128 {
        self.travelled
    }
}

/// The misclosure of the traverse that leaves `start` along `legs`, in order, and should end at
/// `end`: the end it reaches less `end`, enclosed. A closed loop has `end` equal to `start`.
///
/// # Errors
///
/// [`CogoError::TooLong`] for a leg over [`LONGEST`]; [`CogoError::Stopped`] when the meter stops,
/// charged one unit per leg; [`CogoError::Num`] for sums the doubles cannot hold, which takes more
/// than 2^960 legs' worth of steps and so never happens.
pub fn misclosure(
    start: P2,
    legs: &[Leg],
    end: P2,
    meter: &mut WorkMeter<'_>,
) -> Result<Misclosure, CogoError> {
    // Each difference is within ±2^41, so it cannot saturate and a double holds it exactly.
    let mut east = Interval::point(exactly(start.x.get().saturating_sub(end.x.get())))?;
    let mut north = Interval::point(exactly(start.y.get().saturating_sub(end.y.get())))?;
    let mut travelled = 0u128;
    for leg in legs {
        meter.charge(1)?;
        if leg.distance.0 > LONGEST.0 {
            return Err(CogoError::TooLong);
        }
        // At most 2^42 each, so the total is below 2^106 for any slice a machine can hold.
        travelled = travelled.saturating_add(u128::from(leg.distance.0));
        let d = i64::try_from(leg.distance.0).map_err(|_| CogoError::TooLong)?;
        let length = Interval::point(exactly(d))?;
        let (sine, cosine) = leg.bearing.east_north()?;
        east = length.mul_add(sine, east)?;
        north = length.mul_add(cosine, north)?;
    }
    Ok(Misclosure {
        east,
        north,
        length: Interval::norm(&[east, north])?,
        travelled,
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    #[test]
    fn a_commit_rounds_ties_to_even_and_bounds_both_ends() {
        let (k, bound, certain) = committed(Interval::new(2.5, 2.5).unwrap()).unwrap();
        assert_eq!((k, bound, certain), (2, 1, true));
        let (k, bound, certain) = committed(Interval::new(3.4, 3.6).unwrap()).unwrap();
        assert_eq!((k, certain), (4, false));
        assert_eq!(bound, 1);
        let (k, bound, certain) = committed(Interval::new(-7.1, -6.9).unwrap()).unwrap();
        assert_eq!((k, bound, certain), (-7, 1, true));
        let wide = committed(Interval::new(0.0, 10.0).unwrap()).unwrap();
        assert_eq!(wide, (5, 6, false));
        assert!(committed(Interval::new(0.0, 18_014_398_509_481_984.0).unwrap()).is_err());
        assert!(committed(Interval::new(-18_014_398_509_481_984.0, 0.0).unwrap()).is_err());
    }

    #[test]
    fn conversions_keep_their_values() {
        assert_eq!(exactly(-(1 << 53)), -9_007_199_254_740_992.0);
        assert_eq!(whole(-9_007_199_254_740_992.0), -(1 << 53));
    }
}
