//! Angles as whole angle units, each 1/2,500 of an arc-second; bearings; and an angle's sine and
//! cosine, and back (17.2).
//!
//! What it does: [`AngleUnits`] holds an angle exactly, as surveyors write angles down. A full
//! turn is [`FULL_TURN`], 3,240,000,000 units: the least common multiple of the turn's
//! 129,600,000 hundredths of an arc-second and its 40,000,000 hundred-thousandths of a gon, so
//! readings in degrees, minutes and seconds to 0.01″, in gon to 0.00001 gon and in decimal degrees
//! to six places are all whole numbers of units ([`crate::angle_text`] reads and writes them). A
//! [`Bearing`] is an angle measured clockwise from frame north, held within one turn.
//! [`AngleUnits::sin_cos`] and [`Bearing::east_north`] enclose an angle's sine and cosine: the
//! angle is reduced to an eighth of a turn exactly, in integers, and only what is left becomes a
//! double, in half-turns, for the proven `sinpi` and `cospi`. [`AngleUnits::nearest_radians`]
//! goes the other way, from an enclosure of an angle in radians, such as `atan2`'s, to the whole
//! number of units nearest it, with a certified bound; [`Bearing::of_vector`] is that for the
//! direction of an integer vector, exact on the axes and the diagonals.
//!
//! Guarantees:
//!
//! - **Exact arithmetic.** Adding, subtracting, negating and reducing angles, turning a bearing by
//!   an angle (a Ground frame's rotation, 17.2), reversing it, and converting to and from IFC's
//!   angles, anticlockwise from east (a bearing β is the IFC angle 90° − β), are exact integer
//!   steps. A sum or difference beyond `i64` is `None`, never wrapped or saturated.
//! - **Reduced in integers.** Before any double is formed, an angle is reduced to `[0, FULL_TURN)`,
//!   then to its quarter turn and to a part of at most an eighth of a turn, all by exact integer
//!   remainders, so an angle of any size loses nothing to its reduction (17.2; tested at the ends
//!   of `i64`).
//! - **Exact on the quarter turns, enclosed elsewhere.** At a multiple of a quarter turn the sine
//!   and cosine are exactly 0 and ±1, as single points. Every other enclosure contains the exact
//!   sine and cosine of the angle: the part left after the reduction becomes an interval in
//!   half-turns that contains it, and `sinpi` and `cospi` of an interval contain the function over
//!   every real in it (`Interval::sinpi`). Tested against an exact oracle on every input class.
//! - **A certified unit back.** [`AngleUnits::nearest_radians`] gives the whole unit nearest the
//!   midpoint of the enclosure in units, ties to even, so the same bits on every target. Its bound,
//!   a whole number of units rounded up, is at least the distance from that unit to every angle in
//!   the enclosure: half a unit plus the maths bound, so 1 for a narrow enclosure, and 0 only for
//!   exactly zero radians, the one double that is a whole number of units. [`Nearest::is_certain`] says whether every angle in the enclosure has that
//!   unit as its nearest, so a caller that must not guess can say the rounding is undecided
//!   (17.2: "the kernel says so rather than guessing").
//! - **Bearings of integer vectors.** [`Bearing::of_vector`] is exact, bound 0, for a vector along
//!   an axis or a diagonal, decided by exact integer comparisons, and otherwise the certified
//!   nearest unit of `atan2`'s enclosure; the zero vector has no bearing and is refused.
//!
//! Method: whole angle units and bearings clockwise from north as chapter 17.2 sets them
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`); argument reduction by an
//! exact integer remainder to the first octant with the symmetries of sine and cosine (J.-M.
//! Muller, *Elementary Functions: Algorithms and Implementation*, 3rd ed., Birkhäuser 2016,
//! chapter 11, doi:10.1007/978-1-4899-7983-4), on the certified enclosures of `sinpi`, `cospi` and
//! `atan2` (`crate::math`).
//!
//! Cost: constant time and no allocation: a few integer remainders, one interval division and one
//! `sinpi` and `cospi` of an interval for [`AngleUnits::sin_cos`]; one interval `atan2`, division
//! and product for [`AngleUnits::nearest_radians`] and [`Bearing::of_vector`].

use crate::error::NumError;
use crate::interval::Interval;
use crate::math;
use crate::real::Real;

/// Angle units in one arc-second.
pub const PER_SECOND: i64 = 2_500;
/// Angle units in one arc-minute.
pub const PER_MINUTE: i64 = 150_000;
/// Angle units in one degree.
pub const PER_DEGREE: i64 = 9_000_000;
/// Angle units in one gon (a four-hundredth of a turn).
pub const PER_GON: i64 = 8_100_000;
/// Angle units in a quarter turn, 90°.
pub const QUARTER_TURN: i64 = 810_000_000;
/// Angle units in a half turn, 180°.
pub const HALF_TURN: i64 = 1_620_000_000;
/// Angle units in a full turn, 360°.
pub const FULL_TURN: i64 = 3_240_000_000;
/// Angle units in an eighth of a turn, 45°.
const EIGHTH_TURN: i64 = 405_000_000;

/// The largest magnitude a double holds every integer up to, 2^53.
const EXACT_IN_F64: i64 = 1 << 53;

// The units, each from the one before, and the turn they make: 60 seconds a minute, 60 minutes a
// degree, 360 degrees and 400 gon a turn; 0.01″ is 25 units, 0.00001 gon 81 and 0.000001° 9.
const _: () = {
    assert!(PER_MINUTE == 60 * PER_SECOND);
    assert!(PER_DEGREE == 60 * PER_MINUTE);
    assert!(FULL_TURN == 360 * PER_DEGREE);
    assert!(FULL_TURN == 400 * PER_GON);
    assert!(FULL_TURN == 4 * QUARTER_TURN);
    assert!(FULL_TURN == 2 * HALF_TURN);
    assert!(QUARTER_TURN == 2 * EIGHTH_TURN);
    assert!(PER_SECOND == 100 * 25);
    assert!(PER_GON == 100_000 * 81);
    assert!(PER_DEGREE == 1_000_000 * 9);
    // A half turn and an eighth are below 2^31, so both are exact in a double.
    assert!(HALF_TURN < (1 << 31));
};

/// An angle, as a whole number of angle units: 1/2,500 of an arc-second each, [`FULL_TURN`] to a
/// turn. Positive is the direction the caller's convention says: clockwise for a bearing,
/// anticlockwise for IFC's angles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AngleUnits(pub i64);

/// `v` as a double, exactly: every caller passes `|v| ≤ 2^53`, which a double holds exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "every caller passes |v| ≤ 2^53 (checked or bounded beside each call), which a double \
              holds exactly"
)]
const fn exactly(v: i64) -> f64 {
    v as f64
}

/// A whole double as an `i64`, or [`NumError::OutsideDomain`] if `i64` does not hold it.
fn whole(v: f64) -> Result<i64, NumError> {
    // A whole double within ±2^127 is an `i128` exactly; one beyond goes to the nearer end of
    // `i128`, which no `i64` holds either, so the conversion refuses it as it should.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "exact for a whole double within ±2^127; beyond, the saturated i128 is refused below"
    )]
    let wide = v as i128;
    i64::try_from(wide).map_err(|_| NumError::OutsideDomain)
}

impl AngleUnits {
    /// No angle.
    pub const ZERO: AngleUnits = AngleUnits(0);

    /// The units.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    /// `self + other`, or `None` beyond `i64`.
    #[must_use]
    pub const fn checked_add(self, other: AngleUnits) -> Option<AngleUnits> {
        match self.0.checked_add(other.0) {
            Some(v) => Some(AngleUnits(v)),
            None => None,
        }
    }

    /// `self − other`, or `None` beyond `i64`.
    #[must_use]
    pub const fn checked_sub(self, other: AngleUnits) -> Option<AngleUnits> {
        match self.0.checked_sub(other.0) {
            Some(v) => Some(AngleUnits(v)),
            None => None,
        }
    }

    /// `−self`, or `None` for `i64::MIN`.
    #[must_use]
    pub const fn checked_neg(self) -> Option<AngleUnits> {
        match self.0.checked_neg() {
            Some(v) => Some(AngleUnits(v)),
            None => None,
        }
    }

    /// The same direction within one turn, in `[0, FULL_TURN)`: the exact remainder.
    #[must_use]
    pub const fn reduced(self) -> AngleUnits {
        AngleUnits(self.0.rem_euclid(FULL_TURN))
    }

    /// Enclosures of the sine and cosine of the angle, in that order: exact points on the quarter
    /// turns, and otherwise intervals that contain the exact values.
    ///
    /// The angle is reduced in integers to its quarter turn `q` and the part `u` within it, then
    /// to `t = u` or `t = QUARTER_TURN − u`, whichever is at most an eighth of a turn; `t` in
    /// half-turns, `t / HALF_TURN`, is enclosed by one interval division of exact doubles, and
    /// `sinpi` and `cospi` of it are swapped and negated as the octant's symmetries say.
    ///
    /// # Errors
    ///
    /// None in practice: the doubles are finite and the divisor positive, so the `NumError`s of
    /// the interval operations cannot occur.
    pub fn sin_cos(self) -> Result<(Interval, Interval), NumError> {
        let turn = self.0.rem_euclid(FULL_TURN);
        let quarter = turn.div_euclid(QUARTER_TURN);
        let within = turn.rem_euclid(QUARTER_TURN);
        let (sine, cosine) = if within == 0 {
            (Interval::point(0.0)?, Interval::point(1.0)?)
        } else {
            let swapped = within > EIGHTH_TURN;
            // `within` is below a quarter turn, so the difference is positive and cannot saturate.
            let part = if swapped {
                QUARTER_TURN.saturating_sub(within)
            } else {
                within
            };
            // The part, at most EIGHTH_TURN, and HALF_TURN are below 2^31, so both doubles are
            // exact.
            let half_turns =
                Interval::point(exactly(part))?.div(Interval::point(exactly(HALF_TURN))?)?;
            let (sine, cosine) = (half_turns.sinpi()?, half_turns.cospi()?);
            if swapped {
                (cosine, sine)
            } else {
                (sine, cosine)
            }
        };
        Ok(match quarter {
            0 => (sine, cosine),
            1 => (cosine, sine.neg()),
            2 => (sine.neg(), cosine.neg()),
            _ => (cosine.neg(), sine),
        })
    }

    /// The whole number of angle units nearest an angle enclosed in radians, with its bound and
    /// whether it is certainly the nearest.
    ///
    /// The enclosure is turned into units by an interval division by π and an exact product by
    /// `HALF_TURN`; the unit is the midpoint of that enclosure rounded to nearest, ties to even.
    /// The result is not reduced: an angle of −π/2 gives −810,000,000 units.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] when `i64` does not hold the unit or `u64` its bound: for an
    /// enclosure whose midpoint is about 2^63 units from zero (about 2.8 × 10^9 turns) or more,
    /// or which is about 2^64 units wide or wider; the interval operations' errors for one
    /// beyond what a double holds.
    pub fn nearest_radians(theta: Interval) -> Result<Nearest<AngleUnits>, NumError> {
        // Zero radians is zero units exactly, the one angle a double holds that is a whole number
        // of units: every other is π times an irrational number of half-turns or not a unit.
        if theta.lo() == 0.0 && theta.hi() == 0.0 {
            return Ok(Nearest {
                value: AngleUnits::ZERO,
                bound: 0,
                certain: true,
            });
        }
        let units = theta
            .div(Interval::PI)?
            .mul(Interval::point(exactly(HALF_TURN))?)?;
        let (lo, hi) = (units.lo(), units.hi());
        // The midpoint of the two ends, each halved first so the sum cannot overflow.
        let k = math::round_ties_even(lo * 0.5 + hi * 0.5)?.get();
        let certain =
            math::round_ties_even(lo)?.get() == k && math::round_ties_even(hi)?.get() == k;
        let value = AngleUnits(whole(k)?);
        // Each distance to an end, rounded to nearest and moved a step up, is at least the exact
        // distance, and the larger is not negative, since the two add up to the enclosure's width.
        let (below, above) = ((k - lo).next_up(), (hi - k).next_up());
        let bound = ceiling(Real::new(above)?.total_max(Real::new(below)?).get())?;
        Ok(Nearest {
            value,
            bound,
            certain,
        })
    }
}

/// The ceiling of a double that is not negative, as a whole number of units, or
/// [`NumError::OutsideDomain`] if `u64` does not hold it.
fn ceiling(v: f64) -> Result<u64, NumError> {
    // The ceiling is whole, so within 2^128 it is a `u128` exactly; beyond, it goes to `u128::MAX`,
    // which no `u64` holds either, so the conversion refuses it as it should.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "exact for a whole double from 0 to 2^128; beyond, the saturated u128 is refused below"
    )]
    let wide = math::ceil(v)?.get() as u128;
    u64::try_from(wide).map_err(|_| NumError::OutsideDomain)
}

/// A whole number of angle units chosen as the nearest to an angle that is only known within an
/// enclosure: the value, a bound on its distance from the true angle in whole units (0 when it is
/// exact), and whether every angle in the enclosure has it as its nearest unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Nearest<T> {
    /// The value.
    value: T,
    /// At least the distance from the value to the true angle, in whole angle units.
    bound: u64,
    /// Whether the true angle's nearest unit is certainly the value.
    certain: bool,
}

impl<T: Copy> Nearest<T> {
    /// The value.
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }

    /// A bound on the distance from the value to the true angle, in whole angle units: 0 when the
    /// value is exact, and otherwise half a unit plus the maths bound, rounded up.
    #[must_use]
    pub const fn bound(&self) -> u64 {
        self.bound
    }

    /// Whether the value is exactly the angle.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.bound == 0
    }

    /// Whether the value is certainly the unit nearest the true angle: every angle in the
    /// enclosure rounds to it. When not, the true angle lies near a half unit, and either
    /// neighbour may be the nearest.
    #[must_use]
    pub const fn is_certain(&self) -> bool {
        self.certain
    }
}

/// A direction measured clockwise from frame north, as an angle in `[0, FULL_TURN)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bearing(AngleUnits);

impl Bearing {
    /// Frame north, bearing 0.
    pub const NORTH: Bearing = Bearing(AngleUnits(0));

    /// The bearing of `angle` clockwise from north, reduced to one turn.
    #[must_use]
    pub const fn new(angle: AngleUnits) -> Bearing {
        Bearing(angle.reduced())
    }

    /// The bearing's angle clockwise from north, in `[0, FULL_TURN)`.
    #[must_use]
    pub const fn units(self) -> AngleUnits {
        self.0
    }

    /// The bearing turned clockwise by `angle`: on a Ground frame, a bearing plus the frame's
    /// rotation is the map grid bearing (17.2). Exact.
    #[must_use]
    pub const fn rotated(self, angle: AngleUnits) -> Bearing {
        // Both are in [0, FULL_TURN), so the sum is below 2^33 and cannot saturate.
        Bearing(AngleUnits(self.0.0.saturating_add(angle.reduced().0)).reduced())
    }

    /// The opposite bearing, a half turn round: the back bearing of a line.
    #[must_use]
    pub const fn reversed(self) -> Bearing {
        // Below FULL_TURN + HALF_TURN, so the sum cannot saturate.
        Bearing(AngleUnits(self.0.0.saturating_add(HALF_TURN)).reduced())
    }

    /// The bearing of an IFC angle, measured anticlockwise from the x axis (east): 90° − α.
    #[must_use]
    pub const fn from_ifc(alpha: AngleUnits) -> Bearing {
        // QUARTER_TURN − a value in [0, FULL_TURN) lies in (−FULL_TURN, QUARTER_TURN].
        Bearing(AngleUnits(QUARTER_TURN.saturating_sub(alpha.reduced().0)).reduced())
    }

    /// The bearing as an IFC angle, anticlockwise from the x axis (east): 90° − β, in
    /// `[0, FULL_TURN)`.
    #[must_use]
    pub const fn to_ifc(self) -> AngleUnits {
        AngleUnits(QUARTER_TURN.saturating_sub(self.0.0)).reduced()
    }

    /// Enclosures of the bearing's direction as a unit vector, east then north: the sine and the
    /// cosine of the bearing ([`AngleUnits::sin_cos`]).
    ///
    /// # Errors
    ///
    /// As [`AngleUnits::sin_cos`].
    pub fn east_north(self) -> Result<(Interval, Interval), NumError> {
        self.0.sin_cos()
    }

    /// The bearing of the vector `(east, north)`, to the nearest whole unit: exact along an axis
    /// or a diagonal, decided in integers, and otherwise the certified nearest unit of `atan2`'s
    /// enclosure, reduced to one turn.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for the zero vector, which has no direction;
    /// [`NumError::OutOfRange`] for a component beyond ±2^53, which a double cannot hold exactly
    /// (the difference of two grid points is within ±2^41).
    pub fn of_vector(east: i64, north: i64) -> Result<Nearest<Bearing>, NumError> {
        for v in [east, north] {
            if v.unsigned_abs() > EXACT_IN_F64.unsigned_abs() {
                return Err(NumError::OutOfRange {
                    value: i128::from(v),
                });
            }
        }
        let exact = |turns_of_eighth: i64| Nearest {
            value: Bearing(AngleUnits(turns_of_eighth.saturating_mul(EIGHTH_TURN))),
            bound: 0,
            certain: true,
        };
        let (e, n) = (east.signum(), north.signum());
        if east == 0 || north == 0 || east.unsigned_abs() == north.unsigned_abs() {
            return match (e, n) {
                (0, 0) => Err(NumError::Degenerate),
                (0, 1) => Ok(exact(0)),
                (1, 1) => Ok(exact(1)),
                (1, 0) => Ok(exact(2)),
                (1, _) => Ok(exact(3)),
                (0, _) => Ok(exact(4)),
                (_, -1) => Ok(exact(5)),
                (_, 0) => Ok(exact(6)),
                _ => Ok(exact(7)),
            };
        }
        let theta = Interval::point(exactly(east))?.atan2(Interval::point(exactly(north))?)?;
        let near = AngleUnits::nearest_radians(theta)?;
        Ok(Nearest {
            value: Bearing::new(near.value),
            bound: near.bound,
            certain: near.certain,
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn the_turn_is_the_least_common_multiple_of_the_two_readings() {
        // 0.01″ is 1/129,600,000 of a turn and 0.00001 gon 1/40,000,000: the finest unit both are
        // whole numbers of divides the turn into their least common multiple.
        let gcd = |mut a: i64, mut b: i64| {
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        };
        let (s, g) = (129_600_000i64, 40_000_000i64);
        assert_eq!(s / gcd(s, g) * g, FULL_TURN);
    }

    #[test]
    fn whole_and_exact_conversions_keep_their_values() {
        assert_eq!(exactly(1 << 53), 9_007_199_254_740_992.0);
        assert_eq!(exactly(-(1 << 53)), -9_007_199_254_740_992.0);
        assert_eq!(whole(-4.0), Ok(-4));
        assert_eq!(whole(4_611_686_018_427_387_904.0), Ok(1 << 62));
        // i64 holds −2^63 but not 2^63; nor anything beyond 2^127, which the cast saturates.
        assert_eq!(whole(-9_223_372_036_854_775_808.0), Ok(i64::MIN));
        assert_eq!(
            whole(9_223_372_036_854_775_808.0),
            Err(NumError::OutsideDomain)
        );
        assert_eq!(whole(-1e300), Err(NumError::OutsideDomain));
        assert_eq!(ceiling(0.25), Ok(1));
        assert_eq!(ceiling(1.0), Ok(1));
        assert_eq!(ceiling(1.5), Ok(2));
        // u64 holds 2^64 − 2048, the double below 2^64, but not 2^64.
        assert_eq!(ceiling(18_446_744_073_709_549_568.0), Ok(u64::MAX - 2047));
        assert_eq!(
            ceiling(18_446_744_073_709_551_616.0),
            Err(NumError::OutsideDomain)
        );
        assert_eq!(ceiling(1e300), Err(NumError::OutsideDomain));
    }
}
