//! The coordinate grid: where every coordinate the kernel stores lives.
//!
//! Every coordinate is a whole number of grid steps in a local frame, within 2^40 − 1 steps of the
//! frame's origin (chapter 6.1). The step is one of three, each exactly 25 times finer than the one
//! above:
//!
//! | Step | Size | Exact units | Range per frame |
//! | --- | --- | --- | --- |
//! | `Q0` | 156.25 nm | 0.01 mm, 1 mm, 1/256 inch | ±171.8 km |
//! | `Q1` | 6.25 nm | 1 µm, 0.01 mm, 1 mil, 1/256 inch | ±6.87 km |
//! | `Q2` | 0.25 nm | 1 nm and everything above | ±275 m |
//!
//! Sizes are held exactly in quarter-nanometres (625, 25 and 1), never as floating point, and so
//! are the units ([`QUARTER_NM_PER_MM`] and the constants after it): a unit is a whole number of
//! steps of a grid exactly when its size in quarter-nanometres is a multiple of the step's
//! ([`GridStep::steps_per`]). One millimetre is 6,400 steps of `Q0` and one inch 162,560, checked
//! when the crate compiles.
//!
//! Guarantees: refining (coarse to fine) multiplies by 25 per rung and is exact. Coarsening
//! divides by 25 per rung and rounds to the nearest step; because 25 is odd, an exact tie can never
//! happen, so the rounding needs no tie rule and is the same everywhere. Every integer step is a
//! checked method, so nothing here depends on overflow checks being on.
//!
//! Method: the grid ladder of chapter 6.1
//! (`projects/engines/geometry/architecture/06-numbers.md`): exact scaling by 25 per rung, and
//! coarsening by Euclidean division rounded to the nearest step (R. T. Boute, "The Euclidean
//! definition of the functions div and mod", ACM Transactions on Programming Languages and Systems
//! 14(2), 1992, <https://doi.org/10.1145/128861.128862>), as [`coarsen`] derives.
//!
//! Cost: constant and no allocation: each function is a few checked integer operations on `i128`
//! or `u64`.

use crate::error::NumError;

/// Bits of magnitude a coordinate may use. Every exact predicate's bit budget assumes this.
pub const COORD_BITS: u32 = 40;

/// The largest coordinate magnitude, 2^40 − 1 grid steps. Evaluated when the crate compiles, where
/// an overflow is an error.
pub const COORD_MAX: i64 = (1 << COORD_BITS) - 1;

/// [`COORD_MAX`] as an `i128` (a widening, so exact), for [`Coord::from_i128`].
const COORD_MAX_I128: i128 = COORD_MAX as i128;

/// −[`COORD_MAX`] as an `i128`, the smallest coordinate. Evaluated when the crate compiles.
const COORD_MIN_I128: i128 = -COORD_MAX_I128;

/// One millimetre in quarter-nanometres: 4,000,000.
pub const QUARTER_NM_PER_MM: u64 = 4_000_000;

/// One inch (25.4 mm exactly) in quarter-nanometres: 101,600,000.
pub const QUARTER_NM_PER_INCH: u64 = 101_600_000;

/// One micrometre in quarter-nanometres: 4,000.
pub const QUARTER_NM_PER_MICRON: u64 = 4_000;

/// One mil (a thousandth of an inch, 25.4 µm) in quarter-nanometres: 101,600.
pub const QUARTER_NM_PER_MIL: u64 = 101_600;

/// One nanometre in quarter-nanometres: 4.
pub const QUARTER_NM_PER_NM: u64 = 4;

/// The grid step a frame uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridStep {
    /// 156.25 nm (1/6400 mm): buildings, sites, roads, survey.
    Q0,
    /// 6.25 nm: mechanical parts, products, circuit boards.
    Q1,
    /// 0.25 nm: reserved, for example chip layout.
    Q2,
}

impl GridStep {
    /// The step's size in quarter-nanometres, exactly: 625, 25 or 1.
    #[must_use]
    pub const fn quarter_nm(self) -> u32 {
        match self {
            GridStep::Q0 => 625,
            GridStep::Q1 => 25,
            GridStep::Q2 => 1,
        }
    }

    /// How many rungs below the coarsest step this is: 0, 1 or 2.
    #[must_use]
    pub const fn rung(self) -> u32 {
        match self {
            GridStep::Q0 => 0,
            GridStep::Q1 => 1,
            GridStep::Q2 => 2,
        }
    }

    /// The next finer step, if there is one.
    #[must_use]
    pub const fn finer(self) -> Option<GridStep> {
        match self {
            GridStep::Q0 => Some(GridStep::Q1),
            GridStep::Q1 => Some(GridStep::Q2),
            GridStep::Q2 => None,
        }
    }

    /// The next coarser step, if there is one.
    #[must_use]
    pub const fn coarser(self) -> Option<GridStep> {
        match self {
            GridStep::Q0 => None,
            GridStep::Q1 => Some(GridStep::Q0),
            GridStep::Q2 => Some(GridStep::Q1),
        }
    }

    /// How many of these steps make a unit of `unit_quarter_nm` quarter-nanometres (such as
    /// [`QUARTER_NM_PER_MM`]), or `None` when the unit is not a whole number of steps: a micrometre
    /// is 6.4 steps of `Q0`, so `Q0.steps_per(QUARTER_NM_PER_MICRON)` is `None`, and a frame that
    /// needs exact micrometres uses `Q1`.
    #[must_use]
    pub const fn steps_per(self, unit_quarter_nm: u64) -> Option<u64> {
        // The step is 625, 25 or 1, never zero, so neither checked operation returns `None`.
        let step = self.quarter_nm() as u64;
        match unit_quarter_nm.checked_rem(step) {
            Some(0) => unit_quarter_nm.checked_div(step),
            _ => None,
        }
    }
}

/// Checks, at compile time, the unit ratios chapter 6.1 states: 1 mm is 6,400 steps of `Q0` and
/// 1 inch 162,560; 0.01 mm is 64 and 1/256 inch 635; 1 µm and 1 mil are whole numbers of `Q1`
/// steps but not of `Q0` steps; and each step is 25 of the next finer one.
const _: () = {
    assert!(matches!(
        GridStep::Q0.steps_per(QUARTER_NM_PER_MM),
        Some(6_400)
    ));
    assert!(matches!(
        GridStep::Q0.steps_per(QUARTER_NM_PER_INCH),
        Some(162_560)
    ));
    assert!(matches!(
        GridStep::Q0.steps_per(QUARTER_NM_PER_MM / 100),
        Some(64)
    ));
    assert!(matches!(
        GridStep::Q0.steps_per(QUARTER_NM_PER_INCH / 256),
        Some(635)
    ));
    assert!(QUARTER_NM_PER_INCH % 256 == 0);
    assert!(GridStep::Q0.steps_per(QUARTER_NM_PER_MICRON).is_none());
    assert!(GridStep::Q0.steps_per(QUARTER_NM_PER_MIL).is_none());
    assert!(matches!(
        GridStep::Q1.steps_per(QUARTER_NM_PER_MICRON),
        Some(160)
    ));
    assert!(matches!(
        GridStep::Q1.steps_per(QUARTER_NM_PER_MIL),
        Some(4_064)
    ));
    assert!(matches!(GridStep::Q2.steps_per(QUARTER_NM_PER_NM), Some(4)));
    assert!(QUARTER_NM_PER_INCH == 254 * QUARTER_NM_PER_MM / 10);
    assert!(QUARTER_NM_PER_MIL * 1_000 == QUARTER_NM_PER_INCH);
    assert!(GridStep::Q0.quarter_nm() == 25 * GridStep::Q1.quarter_nm());
    assert!(GridStep::Q1.quarter_nm() == 25 * GridStep::Q2.quarter_nm());
};

/// One coordinate: a whole number of grid steps, with magnitude at most [`COORD_MAX`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Coord(i64);

impl Coord {
    /// Zero steps.
    pub const ZERO: Coord = Coord(0);
    /// The largest coordinate, 2^40 − 1 steps.
    pub const MAX: Coord = Coord(COORD_MAX);
    /// The smallest coordinate, −(2^40 − 1) steps.
    pub const MIN: Coord = Coord(-COORD_MAX);

    /// A coordinate of `v` steps, or `OutOfRange` if `|v| > 2^40 − 1`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] with `v` itself, if `|v| > 2^40 − 1`.
    pub const fn new(v: i64) -> Result<Coord, NumError> {
        if v > COORD_MAX || v < -COORD_MAX {
            Err(NumError::OutOfRange { value: v as i128 })
        } else {
            Ok(Coord(v))
        }
    }

    /// A coordinate from a wider integer, or `OutOfRange`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] with `v` itself, if `|v| > 2^40 − 1`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the range check above it keeps |v| at most 2^40 − 1, which fits i64"
    )]
    pub const fn from_i128(v: i128) -> Result<Coord, NumError> {
        if v > COORD_MAX_I128 || v < COORD_MIN_I128 {
            Err(NumError::OutOfRange { value: v })
        } else {
            Ok(Coord(v as i64))
        }
    }

    /// The number of steps.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// A point on the grid in 2D.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct P2 {
    /// x, in grid steps.
    pub x: Coord,
    /// y, in grid steps.
    pub y: Coord,
}

impl P2 {
    /// A point from step counts, or `OutOfRange`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] for the first coordinate, in the order x, y, with magnitude above
    /// 2^40 − 1.
    pub const fn new(x: i64, y: i64) -> Result<P2, NumError> {
        match (Coord::new(x), Coord::new(y)) {
            (Ok(x), Ok(y)) => Ok(P2 { x, y }),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }
}

/// A point on the grid in 3D.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct P3 {
    /// x, in grid steps.
    pub x: Coord,
    /// y, in grid steps.
    pub y: Coord,
    /// z, in grid steps.
    pub z: Coord,
}

impl P3 {
    /// A point from step counts, or `OutOfRange`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] for the first coordinate, in the order x, y, z, with magnitude
    /// above 2^40 − 1.
    pub const fn new(x: i64, y: i64, z: i64) -> Result<P3, NumError> {
        match (Coord::new(x), Coord::new(y), Coord::new(z)) {
            (Ok(x), Ok(y), Ok(z)) => Ok(P3 { x, y, z }),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
        }
    }
}

/// A count of grid steps, used for distances and error bounds. Never negative.
///
/// A count means nothing without its step: 64 steps of `Q0` are 10 µm and 64 steps of `Q1` are
/// 0.4 µm. So whatever carries a bound carries its step too
/// ([`Exactness`](crate::exactness::Exactness)), and the ladder's rungs are counts of the frame's
/// own step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Steps(pub u64);

impl Steps {
    /// Zero steps.
    pub const ZERO: Steps = Steps(0);

    /// The sum, or `None` if it does not fit `u64`. Never saturating: a bound that saturated would
    /// be smaller than the true sum, so it would no longer be a bound.
    #[must_use]
    pub const fn checked_add(self, other: Steps) -> Option<Steps> {
        match self.0.checked_add(other.0) {
            Some(sum) => Some(Steps(sum)),
            None => None,
        }
    }

    /// The same length in steps of `to`, a step at least as fine as `from`: multiplied by 25 per
    /// rung, exactly. `None` if `to` is coarser than `from` (a coarser count would round) or the
    /// count does not fit `u64`.
    #[must_use]
    pub const fn refine(self, from: GridStep, to: GridStep) -> Option<Steps> {
        // The ratio of the sizes is 1, 25 or 625: both are 625, 25 or 1, and the finer divides the
        // coarser.
        let (coarse, fine) = (from.quarter_nm() as u64, to.quarter_nm() as u64);
        match (coarse.checked_rem(fine), coarse.checked_div(fine)) {
            (Some(0), Some(ratio)) => match self.0.checked_mul(ratio) {
                Some(count) => Some(Steps(count)),
                None => None,
            },
            _ => None,
        }
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A value that may have been rounded, with whether it is exact.
///
/// The report chapter 6.1 requires ("converting fine to coarse rounds, and the result says so").
/// Only this crate makes one ([`coarsen`] and [`Tpi::to_grid`](crate::plane::Tpi::to_grid)), so
/// `is_exact` is always the truth about the rounding that made the value: the fields are private,
/// and a caller cannot build one that claims exactness for a rounded value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rounded<T> {
    /// The value.
    value: T,
    /// Whether no rounding happened.
    exact: bool,
}

impl<T: Copy> Rounded<T> {
    /// A value and whether it is exact, for this crate's rounding functions only.
    pub(crate) const fn new(value: T, exact: bool) -> Rounded<T> {
        Rounded { value, exact }
    }

    /// The value.
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }

    /// Whether no rounding happened: the value is exactly what was converted.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.exact
    }
}

/// 25 to the power `k`, for `k` in 0..=2, the rung differences of the three steps; `None` for any
/// other `k`, so that a fourth step could not silently get a wrong ratio.
const fn pow25(k: u32) -> Option<i128> {
    match k {
        0 => Some(1),
        1 => Some(25),
        2 => Some(625),
        _ => None,
    }
}

/// The same position on a finer (or the same) step: multiplies by 25 per rung. Exact.
///
/// Bit budget: |c| ≤ 2^40 − 1 and the factor is at most 625 < 2^10, so the product is below 2^50,
/// far inside `i128`; it is checked all the same, and the result is then checked against the
/// frame's range.
///
/// # Errors
///
/// `WrongDirection` if `to` is coarser than `from`; `OutOfRange` if the result leaves the frame's
/// 40-bit range (a coordinate far from the origin may not fit on a finer step). `BudgetExceeded`
/// only if a checked step failed, which the budget rules out.
pub fn refine(c: Coord, from: GridStep, to: GridStep) -> Result<Coord, NumError> {
    let Some(rungs) = to.rung().checked_sub(from.rung()) else {
        return Err(NumError::WrongDirection);
    };
    let fine = pow25(rungs).and_then(|factor| i128::from(c.0).checked_mul(factor));
    match fine {
        Some(v) => Coord::from_i128(v),
        // Unreachable for the three steps (the budget above): reported, not answered.
        None => Err(NumError::BudgetExceeded),
    }
}

/// The nearest position on a coarser (or the same) step: divides by 25 per rung and rounds to the
/// nearest step. Ties cannot occur because 25 is odd. Reports whether the result is exact.
///
/// Method: Euclidean division, q = floor(v / d) with remainder r in 0..d, then q + 1 when r is
/// more than half of d. With d odd, r > (d − 1) / 2 is the same as 2r > d, and 2r = d is
/// impossible. Every step is checked; none fails for the three steps, since d is 1, 25 or 625 and
/// |v| < 2^40.
///
/// # Errors
///
/// `WrongDirection` if `to` is finer than `from`. `BudgetExceeded` only if a checked step failed,
/// which the bounds above rule out.
pub fn coarsen(c: Coord, from: GridStep, to: GridStep) -> Result<Rounded<Coord>, NumError> {
    let Some(rungs) = from.rung().checked_sub(to.rung()) else {
        return Err(NumError::WrongDirection);
    };
    let v = i128::from(c.0);
    let rounded = pow25(rungs).and_then(|d| {
        let (q, r) = (v.checked_div_euclid(d)?, v.checked_rem_euclid(d)?);
        let q = if r > d.checked_div(2)? {
            q.checked_add(1)?
        } else {
            q
        };
        Some((q, r == 0))
    });
    match rounded {
        // Coarsening never increases a magnitude, so `from_i128` always accepts the result.
        Some((q, exact)) => Coord::from_i128(q).map(|value| Rounded::new(value, exact)),
        // Unreachable for the three steps (see above): reported, not answered.
        None => Err(NumError::BudgetExceeded),
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
    fn steps_divide_exactly() {
        // 0.01 mm = 10,000 nm = 40,000 quarter-nm; 1 mil = 25,400 nm; 1/256 inch = 99,218.75 nm.
        for (quarter_nm, step, steps) in [
            (40_000u32, GridStep::Q0, 64u32),
            (40_000, GridStep::Q1, 1_600),
            (4_000, GridStep::Q1, 160),
            (101_600, GridStep::Q1, 4_064),
            (396_875, GridStep::Q0, 635),
            (396_875, GridStep::Q1, 15_875),
            (4, GridStep::Q2, 4),
        ] {
            assert_eq!(quarter_nm % step.quarter_nm(), 0);
            assert_eq!(quarter_nm / step.quarter_nm(), steps);
            let per = step.steps_per(u64::from(quarter_nm));
            assert_eq!(per, Some(u64::from(steps)));
        }
        assert_eq!(GridStep::Q0.quarter_nm(), 25 * GridStep::Q1.quarter_nm());
        assert_eq!(GridStep::Q1.quarter_nm(), 25 * GridStep::Q2.quarter_nm());
    }

    /// Every named unit on every step: a whole number of steps exactly when the step divides it.
    #[test]
    fn units_are_whole_numbers_of_steps_where_chapter_6_says() {
        let units = [
            QUARTER_NM_PER_MM,
            QUARTER_NM_PER_INCH,
            QUARTER_NM_PER_MICRON,
            QUARTER_NM_PER_MIL,
            QUARTER_NM_PER_NM,
        ];
        for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
            for unit in units {
                let size = u64::from(step.quarter_nm());
                let want = (unit % size == 0).then_some(unit / size);
                assert_eq!(step.steps_per(unit), want, "{unit} on {step:?}");
            }
        }
        assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_MM), Some(6_400));
        assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_INCH), Some(162_560));
        assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_MICRON), None);
        assert_eq!(GridStep::Q0.steps_per(QUARTER_NM_PER_MIL), None);
        assert_eq!(GridStep::Q1.steps_per(QUARTER_NM_PER_NM), None);
        assert_eq!(GridStep::Q2.steps_per(QUARTER_NM_PER_NM), Some(4));
    }

    #[test]
    fn step_counts_add_and_refine_without_saturating() {
        assert_eq!(Steps(3).checked_add(Steps(4)), Some(Steps(7)));
        assert_eq!(Steps(u64::MAX).checked_add(Steps(1)), None);
        assert_eq!(
            Steps(64).refine(GridStep::Q0, GridStep::Q1),
            Some(Steps(1_600))
        );
        assert_eq!(
            Steps(64).refine(GridStep::Q0, GridStep::Q2),
            Some(Steps(40_000))
        );
        assert_eq!(
            Steps(64).refine(GridStep::Q1, GridStep::Q1),
            Some(Steps(64))
        );
        assert_eq!(Steps(64).refine(GridStep::Q1, GridStep::Q0), None);
        assert_eq!(Steps(u64::MAX).refine(GridStep::Q0, GridStep::Q1), None);
    }

    #[test]
    fn coordinate_range_is_enforced() {
        assert!(Coord::new(COORD_MAX).is_ok());
        assert!(Coord::new(-COORD_MAX).is_ok());
        assert_eq!(
            Coord::new(COORD_MAX + 1),
            Err(NumError::OutOfRange {
                value: i128::from(COORD_MAX + 1)
            })
        );
        assert!(P3::new(0, 0, -COORD_MAX - 1).is_err());
        assert_eq!(
            Coord::from_i128(i128::MIN),
            Err(NumError::OutOfRange { value: i128::MIN })
        );
    }

    #[test]
    fn refine_then_coarsen_round_trips() {
        for v in [
            0i64,
            1,
            -1,
            12_345,
            -987_654_321,
            COORD_MAX / 625,
            -(COORD_MAX / 625),
        ] {
            let c = Coord::new(v).unwrap();
            let fine = refine(c, GridStep::Q0, GridStep::Q2).unwrap();
            assert_eq!(fine.get(), v * 625);
            let back = coarsen(fine, GridStep::Q2, GridStep::Q0).unwrap();
            assert_eq!((back.value(), back.is_exact()), (c, true));
        }
        assert!(refine(Coord::MAX, GridStep::Q0, GridStep::Q1).is_err());
        assert_eq!(
            refine(Coord::ZERO, GridStep::Q1, GridStep::Q0),
            Err(NumError::WrongDirection)
        );
        assert_eq!(
            coarsen(Coord::ZERO, GridStep::Q0, GridStep::Q1),
            Err(NumError::WrongDirection)
        );
    }

    /// Every rung difference has its power of 25, and no other difference has one.
    #[test]
    fn pow25_covers_exactly_the_three_steps() {
        assert_eq!([0, 1, 2].map(pow25), [Some(1), Some(25), Some(625)]);
        for k in [3, 4, u32::MAX] {
            assert_eq!(pow25(k), None);
        }
    }

    #[test]
    fn coarsen_rounds_to_nearest() {
        let q1 = |v: i64| {
            let r = coarsen(Coord::new(v).unwrap(), GridStep::Q1, GridStep::Q0).unwrap();
            (r.value().get(), r.is_exact())
        };
        assert_eq!(q1(12), (0, false));
        assert_eq!(q1(13), (1, false));
        assert_eq!(q1(-12), (0, false));
        assert_eq!(q1(-13), (-1, false));
        assert_eq!(q1(50), (2, true));
        assert_eq!(q1(-50), (-2, true));
        // Q2 to Q0 divides by 625: 312 rounds down, 313 up, on both sides.
        let q2 = |v: i64| {
            let r = coarsen(Coord::new(v).unwrap(), GridStep::Q2, GridStep::Q0).unwrap();
            (r.value().get(), r.is_exact())
        };
        assert_eq!(q2(312), (0, false));
        assert_eq!(q2(313), (1, false));
        assert_eq!(q2(-312), (0, false));
        assert_eq!(q2(-313), (-1, false));
        assert_eq!(q2(COORD_MAX), (1_759_218_604, false));
    }
}
