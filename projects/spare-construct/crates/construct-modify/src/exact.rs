//! Motions in plan, on the length grid. A point is two whole numbers of grid steps ([`Steps`]); a
//! motion ([`Motion`]) is `p -> (M p + t) / d` for a whole 2 by 2 matrix `M`, a whole vector `t` and
//! a whole `d` above zero, kept in lowest terms, on whole numbers of any size (num-bigint), so
//! nothing overflows and motions compose exactly. Applying one ([`Motion::apply`]) works out where
//! it takes a point as that exact fraction and rounds it once, each coordinate to the nearest grid
//! step (a half away from zero, as construct-dimensions rounds a point): a point the fraction puts
//! on the grid is given exactly, any other is at most half a step from it on each axis.
//!
//! A turn ([`Turn`]) is held as a cosine and a sine over one whole number, `cos/hyp` and
//! `sin/hyp` with `cos^2 + sin^2 = hyp^2` always, so a turn held is a true rotation. The rule (the
//! decision note `notes/CON-A3-F4-P1-modify-tools.md`, main's ruling of 28 Nov):
//! - **Exact where it can be.** A whole number of quarter turns ([`Turn::quarters`], and any angle
//!   that is one), and a turn from one direction to another whose lengths multiply to a whole
//!   number (3:4:5, [`Turn::between`]), are held exactly. By Niven's theorem no other angle in
//!   degrees has a rational cosine and sine, so no other can be.
//! - **Any other angle** ([`Turn::of_angle`]; a turn between two directions whose lengths do not
//!   multiply to a whole number): the turn held is the true rotation whose half-tangent is the
//!   nearest whole number of 2^-160 to the angle's ([`crate::trig`]), within 2^-159 of a radian of
//!   the angle asked. A point an `i64` holds is then within 2^-94 of a step of where the exact turn
//!   puts it before it is rounded, so each coordinate given is within half a step of the exact turn,
//!   and that 2^-94 of a step: under 0.0001 mm, far inside the model's tolerance of 0.01 mm.
//!
//! A mirror ([`Motion::mirror`]) about the line through two grid points always has a rational
//! matrix, `(a^2 - b^2, 2ab; 2ab, b^2 - a^2) / (a^2 + b^2)` for its direction `(a, b)` in lowest
//! terms, so it is exact; along an axis or at 45 degrees to one the matrix is whole and every point
//! lands, and any other point is rounded once.
//!
//! What cannot be built is refused: a point past the model's limit ([`LIMIT`], `2^40 - 1` steps,
//! about 171.8 km from the origin along x or y, the kernel's grid), which [`Motion::apply`] gives as
//! none.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "whole numbers of any size cannot overflow and every divisor here is above zero; each \
              fixed-width step is a checked method all the same"
)]

use crate::error::ModifyError;
use crate::trig;
use construct_params::Angle;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

/// A point or a vector in plan, in grid steps: `[x, y]`.
pub type Steps = [i64; 2];

/// Angle units in a quarter turn: 90 degrees of 9,000,000 units.
pub const QUARTER: i64 = 810_000_000;
/// Angle units in a full turn.
pub const FULL: i64 = 3_240_000_000;
/// The model's limit: no coordinate is further than this from the origin, in grid steps
/// (construct-solver's `COORD_LIMIT`, the kernel's grid).
pub const LIMIT: i64 = construct_solver::COORD_LIMIT;

/// The greatest common divisor of whole numbers, never negative; 0 for all zeros.
fn gcd_all<'a>(values: impl IntoIterator<Item = &'a BigInt>) -> BigInt {
    values.into_iter().fold(BigInt::zero(), |g, v| g.gcd(v))
}

/// `n / d` to the nearest whole number, a half away from zero, for `d` above zero.
fn nearest(n: &BigInt, d: &BigInt) -> BigInt {
    let twice: BigInt = d * 2;
    let up: BigInt = (n.abs() * 2 + d) / &twice;
    if n.is_negative() { -up } else { up }
}

/// How a motion changes an angle in plan, when it changes it by a whole number of angle units:
/// turned, `a -> by + a`; mirrored, `a -> by - a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Angles {
    /// Whether it is a mirror.
    pub mirrored: bool,
    /// Angle units, from 0 up to but not including a full turn.
    pub by: i64,
}

impl Angles {
    /// No change.
    pub const NONE: Angles = Angles {
        mirrored: false,
        by: 0,
    };

    /// Where it takes the angle `angle`, from 0 up to but not including a full turn.
    #[must_use]
    pub fn angle(self, angle: Angle) -> Angle {
        let a = angle.units().rem_euclid(FULL);
        // Both are below a full turn, so neither sum leaves an i64.
        let units = if self.mirrored {
            self.by.saturating_sub(a)
        } else {
            self.by.saturating_add(a)
        };
        Angle::from_units(units.rem_euclid(FULL))
    }

    /// This change and then `next`.
    #[must_use]
    pub fn then(self, next: Angles) -> Angles {
        // next(this(a)): turned after, `next.by + this(a)`; mirrored after, `next.by - this(a)`.
        let by = if next.mirrored {
            next.by.saturating_sub(self.by)
        } else {
            next.by.saturating_add(self.by)
        };
        Angles {
            mirrored: self.mirrored != next.mirrored,
            by: by.rem_euclid(FULL),
        }
    }
}

/// A turn: cosine `cos / hyp`, sine `sin / hyp`, in lowest terms, `hyp` above zero and
/// `cos^2 + sin^2 = hyp^2`; and its angle in whole angle units, when it is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    cos: BigInt,
    sin: BigInt,
    hyp: BigInt,
    units: Option<i64>,
}

impl Turn {
    /// No turn.
    #[must_use]
    pub fn none() -> Turn {
        Turn::quarters(0)
    }

    /// `k` quarter turns anticlockwise (any whole number; four make a full turn), exactly.
    #[must_use]
    pub fn quarters(k: i64) -> Turn {
        let k = k.rem_euclid(4);
        let (cos, sin) = match k {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (0, -1),
        };
        Turn {
            cos: BigInt::from(cos),
            sin: BigInt::from(sin),
            hyp: BigInt::one(),
            // At most 3 quarter turns, well inside an i64.
            units: Some(k.saturating_mul(QUARTER)),
        }
    }

    /// The turn by `angle`, anticlockwise: exact for a whole number of quarter turns, and for any
    /// other angle the true rotation within 2^-159 of a radian of it (the module header).
    #[must_use]
    pub fn of_angle(angle: Angle) -> Turn {
        Turn::of_fraction(i128::from(angle.units()), 1)
    }

    /// The turn by `num / den` angle units, `den` above zero (one is taken for any other), worked
    /// out as [`Turn::of_angle`] does: copy `k` of a radial array of `n` steps over a sweep is
    /// `k x sweep / n`, worked out from the originals.
    #[must_use]
    pub fn of_fraction(num: i128, den: i128) -> Turn {
        let den = den.max(1);
        // den is at most a radial array's copies, and num a count of them times an i64, so these
        // are far inside an i128; a whole turn's units over den are whole.
        let full = den.saturating_mul(i128::from(FULL));
        let quarter = den.saturating_mul(i128::from(QUARTER));
        let units = num.rem_euclid(full);
        // The angle from minus a half turn up to a half turn, so that a turn and its opposite are
        // worked out alike and are each other's way back exactly.
        let signed = if units > full / 2 {
            units.saturating_sub(full)
        } else {
            units
        };
        let size = signed.unsigned_abs();
        let quarter_size = quarter.unsigned_abs();
        // The nearest quarter turn, a half rounding toward zero: what is left is more than minus
        // an eighth of a turn and at most an eighth.
        let mut quarters = size / quarter_size;
        let mut rest = i128::try_from(size % quarter_size).unwrap_or(0);
        if rest > quarter / 2 {
            quarters = quarters.saturating_add(1);
            rest = rest.saturating_sub(quarter);
        }
        let mut quarters = i64::try_from(quarters).unwrap_or(0);
        if signed < 0 {
            quarters = quarters.saturating_neg();
            rest = rest.saturating_neg();
        }
        let whole = (num.rem_euclid(den) == 0)
            .then(|| i64::try_from((num / den).rem_euclid(i128::from(FULL))).ok())
            .flatten();
        let turn = if rest == 0 {
            Turn::quarters(quarters)
        } else {
            let n = trig::half_tangent(rest.unsigned_abs(), den.unsigned_abs());
            let n = if rest < 0 { -n } else { n };
            Turn::of_half_tangent(&n).then(&Turn::quarters(quarters))
        };
        Turn {
            units: whole,
            ..turn
        }
    }

    /// The true rotation whose half-tangent is `n / 2^PRECISION`: cosine `(1 - t^2) / (1 + t^2)`
    /// and sine `2t / (1 + t^2)`.
    fn of_half_tangent(n: &BigInt) -> Turn {
        let scale = BigInt::one() << trig::PRECISION;
        let square = &scale * &scale;
        let nn = n * n;
        Turn::reduced(&square - &nn, (n * &scale) * 2, square + nn, None)
    }

    /// The turn that takes the direction `from` to the direction `to`: its cosine is their dot
    /// product and its sine their cross product, each over the product of their lengths. Exact when
    /// that product is a whole number (each direction in lowest terms first: 3 m by 4 m is 3 to 4);
    /// otherwise the true rotation within 2^-159 of a radian of it.
    ///
    /// # Errors
    /// [`ModifyError::NoDirection`] for a direction of no length.
    pub fn between(from: Steps, to: Steps) -> Result<Turn, ModifyError> {
        let reduce = |v: Steps| -> Result<[BigInt; 2], ModifyError> {
            let [x, y] = v.map(BigInt::from);
            let g = x.gcd(&y);
            if g.is_zero() {
                return Err(ModifyError::NoDirection);
            }
            Ok([x / &g, y / &g])
        };
        let ([ux, uy], [vx, vy]) = (reduce(from)?, reduce(to)?);
        let dot = &ux * &vx + &uy * &vy;
        let cross = &ux * &vy - &uy * &vx;
        let (uu, vv) = (&ux * &ux + &uy * &uy, &vx * &vx + &vy * &vy);
        // uu * vv is a square exactly when uu / g and vv / g are (they have no common factor).
        let g = uu.gcd(&vv);
        let (a, b) = (&uu / &g, &vv / &g);
        let (ra, rb) = (a.sqrt(), b.sqrt());
        if &ra * &ra == a && &rb * &rb == b {
            let quarter = |k: i64| Some(k.saturating_mul(QUARTER));
            let units = match (dot.sign(), cross.sign()) {
                (num_bigint::Sign::Plus, num_bigint::Sign::NoSign) => quarter(0),
                (num_bigint::Sign::NoSign, num_bigint::Sign::Plus) => quarter(1),
                (num_bigint::Sign::Minus, num_bigint::Sign::NoSign) => quarter(2),
                (num_bigint::Sign::NoSign, num_bigint::Sign::Minus) => quarter(3),
                _ => None,
            };
            return Ok(Turn::reduced(dot, cross, g * ra * rb, units));
        }
        // Past a quarter turn either way, a half turn and then the turn from the opposite of
        // `from`, whose dot product is not below zero; the half-tangent is then at most one.
        let product = uu * vv;
        if dot.is_negative() {
            // The opposite of `from` crosses `to` the other way: its cross product is `-cross`.
            let n = trig::half_tangent_between(&cross, &-&dot, &product);
            let n = if cross.is_positive() { -n } else { n };
            return Ok(Turn::of_half_tangent(&n).then(&Turn::quarters(2)));
        }
        let n = trig::half_tangent_between(&cross, &dot, &product);
        let n = if cross.is_negative() { -n } else { n };
        Ok(Turn::of_half_tangent(&n))
    }

    /// `cos / hyp`, `sin / hyp` in lowest terms, `hyp` above zero.
    fn reduced(cos: BigInt, sin: BigInt, hyp: BigInt, units: Option<i64>) -> Turn {
        let mut g = gcd_all([&cos, &sin, &hyp]);
        if g.is_zero() {
            g = BigInt::one();
        }
        if hyp.is_negative() {
            g = -g;
        }
        Turn {
            cos: cos / &g,
            sin: sin / &g,
            hyp: hyp / &g,
            units,
        }
    }

    /// How many quarter turns it is, when it is a whole number of them (0 to 3).
    #[must_use]
    pub fn quarter_turns(&self) -> Option<u8> {
        if !self.hyp.is_one() {
            return None;
        }
        let unit = |v: &BigInt| i8::try_from(v).ok();
        match (unit(&self.cos)?, unit(&self.sin)?) {
            (1, 0) => Some(0),
            (0, 1) => Some(1),
            (-1, 0) => Some(2),
            (0, -1) => Some(3),
            _ => None,
        }
    }

    /// Its angle in whole angle units, from 0 up to but not including a full turn, when it is one:
    /// a turn by an angle, or a quarter turn; none for a turn between two directions at any other
    /// angle, whose angle no whole number of units is.
    #[must_use]
    pub fn angle(&self) -> Option<Angle> {
        self.units.map(Angle::from_units)
    }

    /// This turn and then `next`, exactly: the product of the two rotations held.
    #[must_use]
    pub fn then(&self, next: &Turn) -> Turn {
        let cos = &self.cos * &next.cos - &self.sin * &next.sin;
        let sin = &self.cos * &next.sin + &self.sin * &next.cos;
        let units = match (self.units, next.units) {
            // Both are below a full turn.
            (Some(a), Some(b)) => Some(a.saturating_add(b).rem_euclid(FULL)),
            _ => None,
        };
        Turn::reduced(cos, sin, &self.hyp * &next.hyp, units)
    }

    /// Its cosine, sine and their common denominator, in lowest terms.
    #[must_use]
    pub fn parts(&self) -> (&BigInt, &BigInt, &BigInt) {
        (&self.cos, &self.sin, &self.hyp)
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

    /// How it changes angles.
    #[must_use]
    pub fn angles(self) -> Angles {
        Angles {
            mirrored: self.mirrored,
            by: i64::from(self.quarters).saturating_mul(QUARTER),
        }
    }

    /// Where it takes the angle `angle`, from 0 up to but not including a full turn.
    #[must_use]
    pub fn angle(self, angle: Angle) -> Angle {
        self.angles().angle(angle)
    }
}

/// A motion of the plan: `p -> (m p + t) / d`, exactly; and how it changes an angle, when it
/// changes it by a whole number of angle units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Motion {
    m: [[BigInt; 2]; 2],
    t: [BigInt; 2],
    d: BigInt,
    angles: Option<Angles>,
}

impl Motion {
    /// No motion.
    #[must_use]
    pub fn identity() -> Motion {
        Motion::by([0, 0])
    }

    /// In lowest terms, `d` above zero; how it changes angles is the square's own when its linear
    /// part is one of the eight motions of a square, else `angles`.
    fn reduced(
        matrix: [[BigInt; 2]; 2],
        shift: [BigInt; 2],
        denominator: BigInt,
        angles: Option<Angles>,
    ) -> Motion {
        let [[m00, m01], [m10, m11]] = matrix;
        let [tx, ty] = shift;
        let mut common = gcd_all([&m00, &m01, &m10, &m11, &tx, &ty, &denominator]);
        if common.is_zero() {
            common = BigInt::one();
        }
        if denominator.is_negative() {
            common = -common;
        }
        let mut motion = Motion {
            m: [
                [m00 / &common, m01 / &common],
                [m10 / &common, m11 / &common],
            ],
            t: [tx / &common, ty / &common],
            d: denominator / &common,
            angles,
        };
        if let Some(square) = motion.square() {
            motion.angles = Some(square.angles());
        }
        motion
    }

    /// `matrix x vector`.
    fn times(matrix: &[[BigInt; 2]; 2], vector: &[BigInt; 2]) -> [BigInt; 2] {
        let [first, second] = matrix;
        let [vx, vy] = vector;
        let row = |[along_x, along_y]: &[BigInt; 2]| along_x * vx + along_y * vy;
        [row(first), row(second)]
    }

    /// The motion with linear part `m / d` that keeps `fixed` where it is.
    fn about(m: [[BigInt; 2]; 2], d: BigInt, fixed: Steps, angles: Option<Angles>) -> Motion {
        let p = fixed.map(BigInt::from);
        let [mx, my] = Motion::times(&m, &p);
        let [px, py] = p;
        let t = [&d * px - mx, &d * py - my];
        Motion::reduced(m, t, d, angles)
    }

    /// A move by `by`.
    #[must_use]
    pub fn by(by: Steps) -> Motion {
        Motion {
            m: [
                [BigInt::one(), BigInt::zero()],
                [BigInt::zero(), BigInt::one()],
            ],
            t: by.map(BigInt::from),
            d: BigInt::one(),
            angles: Some(Angles::NONE),
        }
    }

    /// A turn by `turn` about `centre`.
    #[must_use]
    pub fn turn_about(centre: Steps, turn: &Turn) -> Motion {
        let (c, s, h) = turn.parts();
        let angles = turn.units.map(|by| Angles {
            mirrored: false,
            by,
        });
        Motion::about(
            [[c.clone(), -s], [s.clone(), c.clone()]],
            h.clone(),
            centre,
            angles,
        )
    }

    /// A mirror about the line through `start` and `end`.
    ///
    /// # Errors
    /// [`ModifyError::LineOnePoint`] when they are the same point.
    pub fn mirror(start: Steps, end: Steps) -> Result<Motion, ModifyError> {
        let [sx, sy] = start.map(BigInt::from);
        let [ex, ey] = end.map(BigInt::from);
        let (run, rise) = (ex - sx, ey - sy);
        let common = run.gcd(&rise);
        if common.is_zero() {
            return Err(ModifyError::LineOnePoint);
        }
        let (run, rise) = (run / &common, rise / &common);
        let (run_run, rise_rise) = (&run * &run, &rise * &rise);
        let diagonal = &run_run - &rise_rise;
        let across: BigInt = run * rise * 2;
        let minus = -&diagonal;
        Ok(Motion::about(
            [[diagonal, across.clone()], [across, minus]],
            run_run + rise_rise,
            start,
            None,
        ))
    }

    /// This motion and then `next`, exactly.
    #[must_use]
    pub fn then(&self, next: &Motion) -> Motion {
        let [[s00, s01], [s10, s11]] = &self.m;
        let [m00, m10] = Motion::times(&next.m, &[s00.clone(), s10.clone()]);
        let [m01, m11] = Motion::times(&next.m, &[s01.clone(), s11.clone()]);
        let [rx, ry] = Motion::times(&next.m, &self.t);
        let [nx, ny] = &next.t;
        let t = [rx + &self.d * nx, ry + &self.d * ny];
        let angles = match (self.angles, next.angles) {
            (Some(first), Some(second)) => Some(first.then(second)),
            _ => None,
        };
        Motion::reduced([[m00, m01], [m10, m11]], t, &self.d * &next.d, angles)
    }

    /// This motion `k` times over (none for 0).
    #[must_use]
    pub fn times_over(&self, k: u32) -> Motion {
        let mut out = Motion::identity();
        for _ in 0..k {
            out = out.then(self);
        }
        out
    }

    /// Where it takes `p`: the exact fraction, each coordinate rounded once to the nearest grid
    /// step, a half away from zero. None past the model's limit ([`LIMIT`]).
    #[must_use]
    pub fn apply(&self, p: Steps) -> Option<Steps> {
        let moved = Motion::times(&self.m, &p.map(BigInt::from));
        let mut out = [0_i64; 2];
        for ((slot, r), t) in out.iter_mut().zip(moved).zip(&self.t) {
            let at = i64::try_from(nearest(&(r + t), &self.d)).ok()?;
            if at.unsigned_abs() > LIMIT.unsigned_abs() {
                return None;
            }
            *slot = at;
        }
        Some(out)
    }

    /// Whether it is a mirror: it turns the plan over, so a loop drawn anticlockwise comes out
    /// clockwise (its matrix's determinant is below zero).
    #[must_use]
    pub fn is_mirror(&self) -> bool {
        let [[m00, m01], [m10, m11]] = &self.m;
        let determinant: BigInt = m00 * m11 - m01 * m10;
        determinant.is_negative()
    }

    /// Its linear part as one of the eight motions of a square, when it is one.
    #[must_use]
    pub fn square(&self) -> Option<Square> {
        let unit = |v: &BigInt| -> Option<i8> {
            if v.is_zero() {
                Some(0)
            } else if *v == self.d {
                Some(1)
            } else if -v == self.d {
                Some(-1)
            } else {
                None
            }
        };
        let [[m00, m01], [m10, m11]] = &self.m;
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

    /// How it changes an angle, when it changes it by a whole number of angle units: a move, a turn
    /// by an angle, a radial array's copy, one of the eight motions of a square, and any of these
    /// in turn. None for a turn between two directions at another angle, or a mirror about a line
    /// other than along x or y or at 45 degrees to them, whose angles no whole number of units is.
    #[must_use]
    pub fn angles(&self) -> Option<Angles> {
        self.angles
    }

    /// The displacement, when it is only a move.
    #[must_use]
    pub fn translation(&self) -> Option<Steps> {
        let [[m00, m01], [m10, m11]] = &self.m;
        if *m00 != self.d || *m11 != self.d || !m01.is_zero() || !m10.is_zero() {
            return None;
        }
        let mut out = [0_i64; 2];
        for (slot, t) in out.iter_mut().zip(&self.t) {
            let (q, r) = t.div_rem(&self.d);
            if !r.is_zero() {
                return None;
            }
            *slot = i64::try_from(q).ok()?;
        }
        Some(out)
    }

    /// Whether it moves nothing.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.translation() == Some([0, 0])
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    #[test]
    fn a_quarter_turn_about_a_point_lands_every_point() {
        let turn = Motion::turn_about([10, 0], &Turn::quarters(1));
        assert_eq!(turn.apply([20, 0]), Some([10, 10]));
        assert_eq!(turn.apply([10, 0]), Some([10, 0]));
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
        assert_eq!(mirror.apply([3, 1]), Some([1, 3]));
        assert_eq!(
            mirror.square(),
            Some(Square {
                quarters: 1,
                mirrored: true
            })
        );
        assert!(mirror.is_mirror());
    }

    #[test]
    fn a_half_rounds_away_from_zero() {
        let two = BigInt::from(2);
        assert_eq!(nearest(&BigInt::from(5), &two), BigInt::from(3));
        assert_eq!(nearest(&BigInt::from(-5), &two), BigInt::from(-3));
        assert_eq!(
            nearest(&BigInt::from(-4), &BigInt::from(3)),
            BigInt::from(-1)
        );
        assert_eq!(nearest(&BigInt::from(4), &BigInt::from(3)), BigInt::from(1));
    }
}
