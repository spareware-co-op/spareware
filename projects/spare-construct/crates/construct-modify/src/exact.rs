//! Exact motions in plan, on the length grid. A point is two whole numbers of grid steps
//! ([`Steps`]); a motion ([`Motion`]) is `p -> (M p + t) / d` for a whole 2 by 2 matrix `M`, a
//! whole vector `t` and a whole `d` above zero, kept in lowest terms. Applying one either gives a
//! point on the grid exactly or says it would not ([`Miss`]); nothing is rounded.
//!
//! A turn ([`Turn`]) is held as the cosine and sine of its angle over one whole number, `cos/hyp`
//! and `sin/hyp` with `cos^2 + sin^2 = hyp^2`. The rule for which turns exist (the decision note
//! `notes/CON-A3-F4-P1-modify-tools.md`):
//! - by an angle ([`Turn::of_angle`]): an angle a person types is a whole number of
//!   construct-params' angle units, so a rational number of degrees. A quarter turn always lands.
//!   No other rational number of degrees has both a rational cosine and a rational sine (Niven's
//!   theorem), and a turn that takes one grid point other than its centre to another grid point
//!   has both (its cosine and sine are the dot and cross products of the two whole vectors over
//!   their common length squared). So every other angle in degrees moves every point but the centre
//!   off the grid, and is refused naming the quarter turns either side of it;
//! - from one direction to another ([`Turn::between`]): exact when the product of the two
//!   directions' lengths is whole (a 3:4:5 turn, say), each point then landing or not by plain
//!   division.
//!
//! A mirror ([`Motion::mirror`]) about the line through two grid points always has a rational
//! matrix, `(a^2 - b^2, 2ab; 2ab, b^2 - a^2) / (a^2 + b^2)` for its direction `(a, b)` in lowest
//! terms, so it lands a point when the division is exact: always for a line along an axis or at
//! 45 degrees to one, where the matrix is whole.
//!
//! Numbers: every coordinate is within an `i64`; the matrix, the vector and the products are
//! `i128`, every step checked. A motion whose numbers grow past that (a long chain of turns
//! composed) is refused as too large, never wrapped.

use crate::error::ModifyError;
use construct_params::Angle;
use num_integer::{Integer, Roots};

/// A point or a vector in plan, in grid steps: `[x, y]`.
pub type Steps = [i64; 2];

/// Angle units in a quarter turn: 90 degrees of 9,000,000 units.
pub const QUARTER: i64 = 810_000_000;
/// Angle units in a full turn.
pub const FULL: i64 = 3_240_000_000;

/// Why a point did not land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// It would lie between grid steps.
    OffGrid,
    /// A number past what is held exactly.
    TooLarge,
}

/// The greatest common divisor of whole numbers, never negative; 0 for all zeros.
fn gcd_all(values: &[i128]) -> i128 {
    values.iter().fold(0_i128, |g, v| g.gcd(v))
}

/// `n / d` for a `d` known to divide `n`, or too large (only `i128::MIN / -1` is).
fn div(n: i128, d: i128) -> Result<i128, ModifyError> {
    n.checked_div(d).ok_or(ModifyError::TooLarge)
}

/// The divisor that puts `values` and a denominator `last` in lowest terms with the denominator
/// above zero: their greatest common divisor, negated when the denominator is below zero.
fn lowest(values: &[i128], last: i128) -> Result<i128, ModifyError> {
    let g = gcd_all(values).gcd(&last).max(1);
    if last < 0 {
        g.checked_neg().ok_or(ModifyError::TooLarge)
    } else {
        Ok(g)
    }
}

/// A turn by an exact angle: cosine `cos / hyp`, sine `sin / hyp`, in lowest terms, `hyp` above
/// zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    cos: i128,
    sin: i128,
    hyp: i128,
}

impl Turn {
    /// No turn.
    pub const NONE: Turn = Turn {
        cos: 1,
        sin: 0,
        hyp: 1,
    };

    /// `k` quarter turns anticlockwise (any whole number; four make a full turn).
    #[must_use]
    pub fn quarters(k: i64) -> Turn {
        let (cos, sin) = match k.rem_euclid(4) {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (0, -1),
        };
        Turn { cos, sin, hyp: 1 }
    }

    /// The turn by `angle`, anticlockwise.
    ///
    /// # Errors
    /// [`ModifyError::AngleOffGrid`] for an angle that is not a whole number of quarter turns,
    /// naming the quarter turns either side of it.
    pub fn of_angle(angle: Angle) -> Result<Turn, ModifyError> {
        let units = angle.units();
        if units.rem_euclid(QUARTER) == 0 {
            return Ok(Turn::quarters(units.div_euclid(QUARTER)));
        }
        let below = units.div_euclid(QUARTER).saturating_mul(QUARTER);
        Err(ModifyError::AngleOffGrid {
            angle,
            below: Angle::from_units(below),
            above: Angle::from_units(below.saturating_add(QUARTER)),
        })
    }

    /// The turn that takes the direction `from` to the direction `to`: its cosine is their dot
    /// product and its sine their cross product, each over the product of their lengths.
    ///
    /// # Errors
    /// [`ModifyError::NoDirection`] for a direction of no length; [`ModifyError::TurnNotWhole`]
    /// when the product of their lengths is not a whole number (then neither the cosine nor the
    /// sine is a fraction); [`ModifyError::TooLarge`] past an `i128`.
    pub fn between(from: Steps, to: Steps) -> Result<Turn, ModifyError> {
        let reduce = |v: Steps| -> Result<[i128; 2], ModifyError> {
            let [x, y] = v.map(i128::from);
            let g = x.gcd(&y);
            if g == 0 {
                return Err(ModifyError::NoDirection);
            }
            Ok([div(x, g)?, div(y, g)?])
        };
        let ([ux, uy], [vx, vy]) = (reduce(from)?, reduce(to)?);
        let too_large = || ModifyError::TooLarge;
        let square = |a: i128, b: i128| {
            a.checked_mul(a)
                .and_then(|aa| b.checked_mul(b).and_then(|bb| aa.checked_add(bb)))
                .ok_or_else(too_large)
        };
        let (uu, vv) = (square(ux, uy)?, square(vx, vy)?);
        // uu * vv is a square exactly when uu / g and vv / g are (they have no common factor).
        let g = uu.gcd(&vv);
        let (a, b) = (div(uu, g)?, div(vv, g)?);
        let (ra, rb) = (a.sqrt(), b.sqrt());
        let exact = |r: i128, n: i128| r.checked_mul(r) == Some(n);
        if !exact(ra, a) || !exact(rb, b) {
            return Err(ModifyError::TurnNotWhole);
        }
        let hyp = g
            .checked_mul(ra)
            .and_then(|h| h.checked_mul(rb))
            .ok_or_else(too_large)?;
        let product = |p: i128, q: i128| p.checked_mul(q).ok_or_else(too_large);
        let cos = product(ux, vx)?
            .checked_add(product(uy, vy)?)
            .ok_or_else(too_large)?;
        let sin = product(ux, vy)?
            .checked_sub(product(uy, vx)?)
            .ok_or_else(too_large)?;
        Turn::reduced(cos, sin, hyp)
    }

    /// `cos / hyp`, `sin / hyp` in lowest terms, `hyp` above zero.
    fn reduced(cos: i128, sin: i128, hyp: i128) -> Result<Turn, ModifyError> {
        let g = lowest(&[cos, sin], hyp)?;
        Ok(Turn {
            cos: div(cos, g)?,
            sin: div(sin, g)?,
            hyp: div(hyp, g)?,
        })
    }

    /// How many quarter turns it is, when it is a whole number of them (0 to 3).
    #[must_use]
    pub fn quarter_turns(self) -> Option<u8> {
        match (self.cos, self.sin, self.hyp) {
            (1, 0, 1) => Some(0),
            (0, 1, 1) => Some(1),
            (-1, 0, 1) => Some(2),
            (0, -1, 1) => Some(3),
            _ => None,
        }
    }

    /// This turn and then `next`.
    ///
    /// # Errors
    /// [`ModifyError::TooLarge`] past an `i128`.
    pub fn then(self, next: Turn) -> Result<Turn, ModifyError> {
        let too_large = || ModifyError::TooLarge;
        let mul = |p: i128, q: i128| p.checked_mul(q).ok_or_else(too_large);
        let cos = mul(self.cos, next.cos)?
            .checked_sub(mul(self.sin, next.sin)?)
            .ok_or_else(too_large)?;
        let sin = mul(self.cos, next.sin)?
            .checked_add(mul(self.sin, next.cos)?)
            .ok_or_else(too_large)?;
        Turn::reduced(cos, sin, mul(self.hyp, next.hyp)?)
    }

    /// Its cosine, sine and their common denominator, in lowest terms.
    #[must_use]
    pub fn parts(self) -> (i128, i128, i128) {
        (self.cos, self.sin, self.hyp)
    }
}

/// One of the eight motions of a square about its centre: the linear part of a motion whose matrix
/// is whole and of entries -1, 0 and 1. How it changes an angle `a`: turned, `a + quarters x 90`
/// degrees; mirrored, `quarters x 90 - a` (a mirror about the line at `quarters x 45` degrees).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square {
    /// Quarter turns, 0 to 3.
    pub quarters: u8,
    /// Whether it is a mirror.
    pub mirrored: bool,
}

impl Square {
    /// Whether it takes the plan's x axis to its y axis (a quarter turn either way, or a mirror
    /// about a line at 45 degrees).
    #[must_use]
    pub fn swaps_axes(self) -> bool {
        self.quarters & 1 == 1
    }

    /// Where it takes the angle `angle`, from 0 up to but not including a full turn.
    #[must_use]
    pub fn angle(self, angle: Angle) -> Angle {
        let turn = i64::from(self.quarters).saturating_mul(QUARTER);
        let units = if self.mirrored {
            turn.saturating_sub(angle.units())
        } else {
            turn.saturating_add(angle.units())
        };
        Angle::from_units(units.rem_euclid(FULL))
    }
}

/// An exact motion of the plan: `p -> (m p + t) / d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    m: [[i128; 2]; 2],
    t: [i128; 2],
    d: i128,
}

impl Motion {
    /// No motion.
    pub const IDENTITY: Motion = Motion {
        m: [[1, 0], [0, 1]],
        t: [0, 0],
        d: 1,
    };

    /// In lowest terms, `d` above zero.
    fn reduced(
        matrix: [[i128; 2]; 2],
        shift: [i128; 2],
        denominator: i128,
    ) -> Result<Motion, ModifyError> {
        let [[m00, m01], [m10, m11]] = matrix;
        let [tx, ty] = shift;
        let common = lowest(&[m00, m01, m10, m11, tx, ty], denominator)?;
        Ok(Motion {
            m: [
                [div(m00, common)?, div(m01, common)?],
                [div(m10, common)?, div(m11, common)?],
            ],
            t: [div(tx, common)?, div(ty, common)?],
            d: div(denominator, common)?,
        })
    }

    /// `matrix x vector` for a whole vector.
    fn times(matrix: [[i128; 2]; 2], vector: [i128; 2]) -> Result<[i128; 2], ModifyError> {
        let [first, second] = matrix;
        let [vx, vy] = vector;
        let row = |[along_x, along_y]: [i128; 2]| {
            along_x
                .checked_mul(vx)
                .and_then(|px| along_y.checked_mul(vy).and_then(|py| px.checked_add(py)))
                .ok_or(ModifyError::TooLarge)
        };
        Ok([row(first)?, row(second)?])
    }

    /// The motion with linear part `m / d` that keeps `fixed` where it is.
    fn about(m: [[i128; 2]; 2], d: i128, fixed: Steps) -> Result<Motion, ModifyError> {
        let p = fixed.map(i128::from);
        let moved = Motion::times(m, p)?;
        let too_large = || ModifyError::TooLarge;
        let mut t = [0_i128; 2];
        for ((slot, q), r) in t.iter_mut().zip(p).zip(moved) {
            *slot = d
                .checked_mul(q)
                .and_then(|dq| dq.checked_sub(r))
                .ok_or_else(too_large)?;
        }
        Motion::reduced(m, t, d)
    }

    /// A move by `by`.
    #[must_use]
    pub fn by(by: Steps) -> Motion {
        Motion {
            t: by.map(i128::from),
            ..Motion::IDENTITY
        }
    }

    /// A turn by `turn` about `centre`.
    ///
    /// # Errors
    /// [`ModifyError::TooLarge`] past an `i128`.
    pub fn turn_about(centre: Steps, turn: Turn) -> Result<Motion, ModifyError> {
        let (c, s, h) = turn.parts();
        let minus_s = s.checked_neg().ok_or(ModifyError::TooLarge)?;
        Motion::about([[c, minus_s], [s, c]], h, centre)
    }

    /// A mirror about the line through `a` and `b`.
    ///
    /// # Errors
    /// [`ModifyError::LineOnePoint`] when they are the same point; [`ModifyError::TooLarge`]
    /// past an `i128`.
    pub fn mirror(start: Steps, end: Steps) -> Result<Motion, ModifyError> {
        let too_large = || ModifyError::TooLarge;
        let [sx, sy] = start.map(i128::from);
        let [ex, ey] = end.map(i128::from);
        let (run, rise) = (
            ex.checked_sub(sx).ok_or_else(too_large)?,
            ey.checked_sub(sy).ok_or_else(too_large)?,
        );
        let common = run.gcd(&rise);
        if common == 0 {
            return Err(ModifyError::LineOnePoint);
        }
        let (run, rise) = (div(run, common)?, div(rise, common)?);
        let mul = |x: i128, y: i128| x.checked_mul(y).ok_or_else(too_large);
        let (run_run, rise_rise) = (mul(run, run)?, mul(rise, rise)?);
        let diagonal = run_run.checked_sub(rise_rise).ok_or_else(too_large)?;
        let across = mul(2, mul(run, rise)?)?;
        let denominator = run_run.checked_add(rise_rise).ok_or_else(too_large)?;
        let minus = diagonal.checked_neg().ok_or_else(too_large)?;
        Motion::about([[diagonal, across], [across, minus]], denominator, start)
    }

    /// This motion and then `next`.
    ///
    /// # Errors
    /// [`ModifyError::TooLarge`] past an `i128`.
    pub fn then(&self, next: &Motion) -> Result<Motion, ModifyError> {
        let too_large = || ModifyError::TooLarge;
        let [[s00, s01], [s10, s11]] = self.m;
        let [m00, m10] = Motion::times(next.m, [s00, s10])?;
        let [m01, m11] = Motion::times(next.m, [s01, s11])?;
        let moved = Motion::times(next.m, self.t)?;
        let mut t = [0_i128; 2];
        for ((slot, r), n) in t.iter_mut().zip(moved).zip(next.t) {
            *slot = self
                .d
                .checked_mul(n)
                .and_then(|dn| r.checked_add(dn))
                .ok_or_else(too_large)?;
        }
        let d = self.d.checked_mul(next.d).ok_or_else(too_large)?;
        Motion::reduced([[m00, m01], [m10, m11]], t, d)
    }

    /// This motion `k` times over (none for 0).
    ///
    /// # Errors
    /// [`ModifyError::TooLarge`] past an `i128`.
    pub fn times_over(&self, k: u32) -> Result<Motion, ModifyError> {
        let mut out = Motion::IDENTITY;
        for _ in 0..k {
            out = out.then(self)?;
        }
        Ok(out)
    }

    /// Where it takes `p`, exactly.
    ///
    /// # Errors
    /// [`Miss::OffGrid`] for a point between grid steps; [`Miss::TooLarge`] past an `i64`.
    pub fn apply(&self, p: Steps) -> Result<Steps, Miss> {
        let moved = Motion::times(self.m, p.map(i128::from)).map_err(|_| Miss::TooLarge)?;
        let mut out = [0_i64; 2];
        for ((slot, r), t) in out.iter_mut().zip(moved).zip(self.t) {
            let n = r.checked_add(t).ok_or(Miss::TooLarge)?;
            if n.checked_rem(self.d) != Some(0) {
                return Err(Miss::OffGrid);
            }
            let q = n.checked_div(self.d).ok_or(Miss::TooLarge)?;
            *slot = i64::try_from(q).map_err(|_| Miss::TooLarge)?;
        }
        Ok(out)
    }

    /// Whether it is a mirror: it turns the plan over, so a loop drawn anticlockwise comes out
    /// clockwise.
    #[must_use]
    pub fn is_mirror(&self) -> bool {
        let [[m00, m01], [m10, m11]] = self.m;
        // A rotation's matrix is (c, -s; s, c) and a mirror's (c, s; s, -c), over d, so the
        // diagonal tells them apart without a product: equal for a turn, opposite for a mirror,
        // and the two off the diagonal opposite for a turn (when the diagonal is zero).
        if m00 == m11 && m00 != 0 {
            return false;
        }
        if m00 != m11 {
            return true;
        }
        m01 == m10
    }

    /// Its linear part as one of the eight motions of a square, when it is one.
    #[must_use]
    pub fn square(&self) -> Option<Square> {
        let unit = |v: i128| -> Option<i8> {
            if v == 0 {
                Some(0)
            } else if v == self.d {
                Some(1)
            } else if Some(v) == self.d.checked_neg() {
                Some(-1)
            } else {
                None
            }
        };
        let [[m00, m01], [m10, m11]] = self.m;
        let entries = [unit(m00)?, unit(m01)?, unit(m10)?, unit(m11)?];
        let (quarters, mirrored) = match entries {
            [1, 0, 0, 1] => (0, false),
            [0, -1, 1, 0] => (1, false),
            [-1, 0, 0, -1] => (2, false),
            [0, 1, -1, 0] => (3, false),
            [1, 0, 0, -1] => (0, true),
            [0, 1, 1, 0] => (1, true),
            [-1, 0, 0, 1] => (2, true),
            [0, -1, -1, 0] => (3, true),
            _ => return None,
        };
        Some(Square { quarters, mirrored })
    }

    /// The displacement, when it is only a move.
    #[must_use]
    pub fn translation(&self) -> Option<Steps> {
        if self.m != [[self.d, 0], [0, self.d]] {
            return None;
        }
        let mut out = [0_i64; 2];
        for (slot, t) in out.iter_mut().zip(self.t) {
            if t.checked_rem(self.d) != Some(0) {
                return None;
            }
            *slot = i64::try_from(t.checked_div(self.d)?).ok()?;
        }
        Some(out)
    }

    /// Whether it moves nothing.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == Motion::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    #[test]
    fn a_quarter_turn_about_a_point_lands_every_point() {
        let turn = Motion::turn_about([10, 0], Turn::quarters(1)).unwrap();
        assert_eq!(turn.apply([20, 0]), Ok([10, 10]));
        assert_eq!(turn.apply([10, 0]), Ok([10, 0]));
        assert_eq!(
            turn.square(),
            Some(Square {
                quarters: 1,
                mirrored: false
            })
        );
        assert!(!turn.is_mirror());
    }

    #[test]
    fn a_mirror_at_45_degrees_is_whole() {
        let mirror = Motion::mirror([0, 0], [5, 5]).unwrap();
        assert_eq!(mirror.apply([3, 1]), Ok([1, 3]));
        assert_eq!(
            mirror.square(),
            Some(Square {
                quarters: 1,
                mirrored: true
            })
        );
        assert!(mirror.is_mirror());
    }
}
