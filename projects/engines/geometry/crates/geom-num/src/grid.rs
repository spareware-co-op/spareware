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
//! Sizes are held exactly in quarter-nanometres (625, 25 and 1), never as floating point.
//! Refining (coarse to fine) multiplies by 25 per rung and is exact. Coarsening divides by 25 per rung
//! and rounds to the nearest step; because 25 is odd, an exact tie can never happen, so the rounding
//! needs no tie rule and is the same everywhere.

use crate::error::NumError;

/// Bits of magnitude a coordinate may use. Every exact predicate's bit budget assumes this.
pub const COORD_BITS: u32 = 40;

/// The largest coordinate magnitude, 2^40 − 1 grid steps.
pub const COORD_MAX: i64 = (1 << COORD_BITS) - 1;

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
}

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
    pub const fn new(v: i64) -> Result<Coord, NumError> {
        if v > COORD_MAX || v < -COORD_MAX {
            Err(NumError::OutOfRange { value: v as i128 })
        } else {
            Ok(Coord(v))
        }
    }

    /// A coordinate from a wider integer, or `OutOfRange`.
    pub const fn from_i128(v: i128) -> Result<Coord, NumError> {
        if v > COORD_MAX as i128 || v < -(COORD_MAX as i128) {
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
    pub const fn new(x: i64, y: i64, z: i64) -> Result<P3, NumError> {
        match (Coord::new(x), Coord::new(y), Coord::new(z)) {
            (Ok(x), Ok(y), Ok(z)) => Ok(P3 { x, y, z }),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
        }
    }
}

/// A count of grid steps, used for distances and error bounds. Never negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Steps(pub u64);

impl Steps {
    /// Zero steps.
    pub const ZERO: Steps = Steps(0);

    /// The sum, saturating at the largest representable count.
    #[must_use]
    pub const fn saturating_add(self, other: Steps) -> Steps {
        Steps(self.0.saturating_add(other.0))
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A value that may have been rounded, with whether it is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rounded<T> {
    /// The value.
    pub value: T,
    /// Whether no rounding happened.
    pub exact: bool,
}

/// 25 to the power `k`, for `k` in 0..=2.
const fn pow25(k: u32) -> i128 {
    match k {
        0 => 1,
        1 => 25,
        _ => 625,
    }
}

/// The same position on a finer (or the same) step: multiplies by 25 per rung. Exact.
///
/// Errors: `WrongDirection` if `to` is coarser than `from`; `OutOfRange` if the result leaves the
/// frame's 40-bit range (a coordinate far from the origin may not fit on a finer step).
pub const fn refine(c: Coord, from: GridStep, to: GridStep) -> Result<Coord, NumError> {
    if to.rung() < from.rung() {
        return Err(NumError::WrongDirection);
    }
    Coord::from_i128(c.0 as i128 * pow25(to.rung() - from.rung()))
}

/// The nearest position on a coarser (or the same) step: divides by 25 per rung and rounds to the
/// nearest step. Ties cannot occur because 25 is odd. Reports whether the result is exact.
///
/// Errors: `WrongDirection` if `to` is finer than `from`.
pub const fn coarsen(c: Coord, from: GridStep, to: GridStep) -> Result<Rounded<Coord>, NumError> {
    if to.rung() > from.rung() {
        return Err(NumError::WrongDirection);
    }
    let d = pow25(from.rung() - to.rung());
    let v = c.0 as i128;
    let q = v.div_euclid(d);
    let r = v.rem_euclid(d);
    // r is in 0..d. Round up when the remainder is more than half the divisor; 2r == d is impossible.
    let q = if 2 * r > d { q + 1 } else { q };
    // Coarsening never increases a magnitude, so the result is always in range.
    Ok(Rounded { value: Coord(q as i64), exact: r == 0 })
}

#[cfg(test)]
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
        }
        assert_eq!(GridStep::Q0.quarter_nm(), 25 * GridStep::Q1.quarter_nm());
        assert_eq!(GridStep::Q1.quarter_nm(), 25 * GridStep::Q2.quarter_nm());
    }

    #[test]
    fn coordinate_range_is_enforced() {
        assert!(Coord::new(COORD_MAX).is_ok());
        assert!(Coord::new(-COORD_MAX).is_ok());
        assert_eq!(Coord::new(COORD_MAX + 1), Err(NumError::OutOfRange { value: (COORD_MAX + 1) as i128 }));
        assert!(P3::new(0, 0, -COORD_MAX - 1).is_err());
    }

    #[test]
    fn refine_then_coarsen_round_trips() {
        for v in [0i64, 1, -1, 12_345, -987_654_321, COORD_MAX / 625, -(COORD_MAX / 625)] {
            let c = Coord::new(v).unwrap();
            let fine = refine(c, GridStep::Q0, GridStep::Q2).unwrap();
            assert_eq!(fine.get(), v * 625);
            let back = coarsen(fine, GridStep::Q2, GridStep::Q0).unwrap();
            assert_eq!(back, Rounded { value: c, exact: true });
        }
        assert!(refine(Coord::MAX, GridStep::Q0, GridStep::Q1).is_err());
        assert_eq!(refine(Coord::ZERO, GridStep::Q1, GridStep::Q0), Err(NumError::WrongDirection));
        assert_eq!(coarsen(Coord::ZERO, GridStep::Q0, GridStep::Q1), Err(NumError::WrongDirection));
    }

    #[test]
    fn coarsen_rounds_to_nearest() {
        let q1 = |v: i64| coarsen(Coord::new(v).unwrap(), GridStep::Q1, GridStep::Q0).unwrap();
        assert_eq!(q1(12), Rounded { value: Coord(0), exact: false });
        assert_eq!(q1(13), Rounded { value: Coord(1), exact: false });
        assert_eq!(q1(-12), Rounded { value: Coord(0), exact: false });
        assert_eq!(q1(-13), Rounded { value: Coord(-1), exact: false });
        assert_eq!(q1(50), Rounded { value: Coord(2), exact: true });
    }
}
