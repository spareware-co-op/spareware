//! Conversion between map coordinates and a frame's grid, each way rounded once and correctly: map
//! values in the frame's linear unit, given as doubles or as exact decimal text, go to the nearest
//! grid step, ties to even; grid points go to the nearest doubles, ties to even.
//!
//! The frame record and the formula that ties a frame's grid to its map coordinates are in
//! [`crate::frame`]. For a map value v in CRS units of u = p/q metres, its image in steps is
//! W = v·u·S (S steps per metre), and a frame coordinate is the exact value
//!
//! ```text
//! X = k_d·(α·(W_E − O_E) + β·(W_N − O_N)) / (k_n·ρ)      z = W_H − O_H
//! ```
//!
//! with (α, β) = (c, s) for x and (−s, c) for y, and ρ = r = √(c² + s²).
//!
//! **Map to frame** ([`FrameRecord::to_frame`] for doubles, [`FrameRecord::to_frame_decimal`] for
//! [`Decimal`]s) rounds each coordinate once, to the nearest step, ties to even, and says whether it
//! was exact.
//!
//! - *The fast exact path.* In metres, for doubles within 2^62 m, a height, and the horizontal
//!   coordinates of a Map frame or one turned by quarter turns at scale one, are G03's: v·S is
//!   exact as a double-double (`two_prod`), the origin is subtracted inside the exact rounding of
//!   `DD::round_shifted`, and a quarter turn only swaps and negates.
//! - *The exact path, for everything else.* Each value is read exactly, as m·2^a·5^b (a double:
//!   m·2^a; a decimal m·10^e: a = b = e). The values of one point get a common denominator
//!   d = q·2^A·5^C, so each offset `W_i` − `O_i` is an exact integer `u_i` over d, and each coordinate
//!   is X = P/(Q·ρ) with integers P = `k_d·(α·u_E` + `β·u_N`) (or `u_H`) and Q = `k_n·d` (or d), all
//!   formed exactly in 1536-bit integers. A double-double estimate of P/(Q·ρ), within 2^−101·|X|
//!   (derived on `RATIO_BOUND` and `IRRATIONAL_BOUND`), gives the candidate step; a value
//!   farther than 2^−40 from every whole step and midpoint rounds as its estimate does, and any
//!   other is settled exactly, by the sign of 2P − t·Q·ρ for t = 2x and 2x ± 1: a product and a
//!   subtraction when ρ = r is an integer, and a comparison of squares in 3072-bit integers when
//!   it is irrational (then no tie can occur, and a coordinate is exact only when it is zero).
//! - *Out of range.* A coordinate beyond ±(2^40 − 1) steps is refused with
//!   [`NumError::OutOfRange`] carrying the exact rounded coordinate, found by bisection with exact
//!   signs over the estimate's proven bracket, and saturated at the ends of `i128` as the numbers
//!   layer documents (`i128::MAX` for 2^127 − 1 or more). An input whose image is beyond 2^200
//!   steps is outside every exact budget; then some coordinate is beyond 2^135 steps, and the error
//!   reports the larger horizontal one (or the height) saturated, with the sign of the exact
//!   formula applied to the inputs cut to whole units, which the truncation cannot change.
//!
//! Metres given to 0.01 mm convert exactly when their binary value is exact (one millimetre is
//! 6,400 steps); decimal text given to 0.01 mm always does, and so does every international foot
//! to 0.1 ft. A US survey foot is a whole number of steps only in multiples of 3,937 feet (chapter
//! 24), so most values in survey feet round, once.
//!
//! **Frame to map** ([`FrameRecord::to_map`]) returns the nearest doubles to the exact map
//! coordinates in the frame's linear unit, ties to even, with whether they are exact: by exact
//! integer division when the horizontal transform is rational (in 1536-bit integers when the
//! values outgrow `i128`), and for an irrational direction by a double-double value certified
//! against its proven error, falling back, when the certificate fails, to a bisection over the
//! doubles decided by exact signs of A − B·√(c² + s²). An irrational map coordinate is never exact
//! and never a tie. At q0 in metres a Map-frame round trip frame → map → frame is the identity for
//! every coordinate below 2^29 m, because an ulp of the map value is then smaller than a step.
//!
//! Because each frame rounds its own coordinates to even, a map value exactly halfway between two
//! steps can round to different map positions in two frames whose origins differ by an odd number
//! of steps (chapter 17.1); lattice frames, 2^39 steps apart, agree.
//!
//! Bit budgets. Inputs: a double's m below 2^53 with a from −1074 to 971; a decimal's m below
//! 10^38 < 2^127 with a = b from −340 to 340; images within 2^200 steps; S < 2^32, p and q below
//! 2^64; |c|, |s| ≤ 2^31; `k_n`, `k_d` < 2^64; r < 2^32, c² + s² < 2^63; |O| ≤ 2^60. Then
//! d ≤ 2^64·2^1074 < 2^1139 for doubles and d ≤ 2^64·10^340 < 2^1194 for decimals, so
//! |`u_i`| ≤ 2^200·d + 2^60·d < 2^1395, |P| < 2^64·2^31·2^1396 = 2^1491, and |Q·r| < 2^64·2^1194·2^32
//! = 2^1290. A settling sign forms t·Q·ρ with |t| ≤ 2^128 + 1 (2^42 + 1 when settling a candidate,
//! 2^128 in the bisection for out-of-range values), below 2^1419, and 2P − t·Q·r below 2^1493,
//! inside the 1536-bit ±2^1535; for an irrational ρ the squares are below 2^2984 and
//! 2^2838·2^63 = 2^2901, inside the 3072-bit ±2^3071. The out-of-range report for inputs beyond 2^200
//! steps forms integers below 2^1450. The budgets of frame to map are on [`FrameRecord::to_map`].
//! Every integer step is a checked method, and one its budget proves fits can fail only through a
//! defect, reported as [`NumError::BudgetExceeded`], never answered: no exact sign that fails is
//! replaced by a guess, and no conversion returns a value it could not certify.
//!
//! Guarantees:
//!
//! - **Map to frame rounds once.** [`FrameRecord::to_frame`] and [`FrameRecord::to_frame_decimal`]
//!   return the exact value of every coordinate rounded to the nearest step, ties to even, and say
//!   whether all three were exact, for every frame and every linear unit; never through a
//!   rounded intermediate, and never through `f64` for decimal text.
//! - **Out of range, exactly.** A coordinate beyond ±(2^40 − 1) steps is refused with
//!   [`NumError::OutOfRange`] whose value is that coordinate, exactly rounded, and saturated
//!   only when it is beyond `i128` (and so, for inputs beyond 2^200 steps, a coordinate beyond
//!   2^135 steps with its true sign). NaN and infinities are [`NumError::NotFinite`].
//! - **Frame to map rounds once.** [`FrameRecord::to_map`] gives the nearest doubles to the exact
//!   map coordinates in the frame's linear unit, ties to even, with whether they are exact, for
//!   every frame.
//! - **Certified or refused.** No conversion returns a value it could not certify, and no exact
//!   sign that fails is replaced by a guess: a defect is reported as [`NumError::BudgetExceeded`].
//!
//! Method: the conversion rules of chapter 17.1
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`) and chapter 24's numbers
//! to grid (`projects/engines/geometry/architecture/24-file-format-exchange.md`); double-double
//! arithmetic with `geom-num`'s proven bounds (`geom_num::dd`, piece G25); the exact settling as
//! the filter-then-exact pattern of the kernel's predicates (chapter 6.2; J. R. Shewchuk,
//! *Discrete & Computational Geometry* 18 (1997), <https://doi.org/10.1007/PL00009321>), with
//! exact values of doubles read as dyadic rationals and of decimals as rationals over powers of
//! ten (IEEE 754-2019, <https://doi.org/10.1109/IEEESTD.2019.8766229>), and numbers a + b√n
//! compared exactly by their squares (C. K. Yap, *Computational Geometry* 7 (1997),
//! <https://doi.org/10.1016/0925-7721(95)00040-2>).
//!
//! Cost: no allocation, constant time.
//!
//! - [`FrameRecord::to_frame`] on the fast path: three exact products (`two_prod`) and three exact
//!   roundings. On the exact path, per point: a few products of a 1536-bit integer by integers of
//!   one or two limbs (each linear in the 24 limbs; a decimal's powers of five add one such
//!   product per 27 of them), two double-double estimates of under ten operations each, and,
//!   for a coordinate within 2^−40 of a whole step or a midpoint (rare for measured data, usual
//!   for exact inputs), up to three exact signs: each one product and a subtraction, or, for an
//!   irrational direction, three schoolbook products of 3072-bit integers (48 limbs).
//! - An out-of-range coordinate adds a bisection of at most 130 exact signs.
//! - [`FrameRecord::to_map`]: in a Map frame or at quarter turns at scale one, `i128` arithmetic and
//!   three correctly rounded quotients (at most 181 shift-and-subtract steps each); in a rational
//!   Ground frame, two correctly rounded quotients of 1536-bit integers (58 shifts and 56
//!   subtractions each). For an irrational direction, a ratio and six double-double operations per
//!   horizontal coordinate; when the certificate fails, a bisection of at most 70 halvings and
//!   three more exact signs, each four products of 1536-bit integers.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_num::dd::{self, DD};
use geom_num::grid::{Coord, P3};
use geom_num::sign::Sign;
use geom_num::{NumError, math};

use crate::decimal::Decimal;
use crate::error::budget;
use crate::exact::{
    Big, Square, const_pow2, divrem_u128, dyadic, leading_bits, pow2, pow2_big, pow5_big,
    quotient_to_f64, rational_to_f64, sign_minus_sqrt,
};
use crate::frame::{FrameRecord, steps_per_metre};
use crate::rounded::Rounded;

/// The largest map coordinate the fast path takes, 2^62 m: then v·S is below 2^94, a double-double
/// exactly, and inside `DD::round_shifted`'s 2^100. Larger values take the exact path.
const MAP_MAX: f64 = const_pow2!(62);

/// Estimates beyond 2^41 steps in magnitude are out of range: their exact rounding is found by
/// bisection instead of by settling.
const RANGE_FILTER: f64 = const_pow2!(41);

/// The settling margin, 2^−40 steps: an estimate farther than this from every whole step and every
/// midpoint rounds the same way as the exact value, because its error is below 2^−59.
const FILTER: f64 = const_pow2!(-40);

/// An input whose image is beyond 2^200 steps is outside the exact budgets (see the module
/// documentation).
const INPUT_BITS: u32 = 200;

/// The relative half-width of the bracket around an out-of-range estimate, 2^−50: more than the
/// estimate's proven 2^−100 plus the rounding of the `f64` subtractions that form the bracket's
/// ends (half an ulp, 2^−53 relative).
const FAR_MARGIN: f64 = const_pow2!(-50);

/// The integer below `i128::MAX`: X > MAX − 1/2 = (MAX − 1) + 1/2 saturates.
const BELOW_MAX: i128 = i128::MAX - 1;

/// The bits each operand of [`ratio_dd`] keeps: 106, the most a double-double holds exactly.
const RATIO_BITS: u32 = 106;

/// A ratio whose two operands' leading bits differ by a power below 2^−1006 is below 2^−901 in
/// magnitude, and [`ratio_dd`] returns zero for it.
const RATIO_FLUSH: i32 = -1006;

/// u² = 2^−106, the unit of double-double error bounds.
const U2: f64 = const_pow2!(-106);

/// The truncation of an operand to [`RATIO_BITS`] bits: relative error below 2^−105 = 2u².
const TRUNCATION: f64 = const_pow2!(-105);

/// The factor that covers both the higher-order terms of a product of factors (1 + εᵢ), each
/// |εᵢ| ≤ 2^−100, and the rounding of the `f64` arithmetic that sums the bounds: Π(1 + εᵢ) − 1 ≤
/// Σεᵢ·(1 + 2^−98), and a few roundings add at most 2^−50 relative.
const SLACK: f64 = 1.0 + const_pow2!(-40);

/// The relative error bound of [`ratio_dd`]: the two truncations (each 2^−105, the divisor's as
/// 1/(1 − θ), covered by [`SLACK`]), the double-double division and the two exact scalings by
/// powers of two, taken at the general bound of `mul_f64`: about 17.8u². Absolute: 2^−900 more,
/// for results it flushes to zero or that fall below 2^−969, where a double-double cannot hold 106
/// bits (`geom_num::dd`'s `UNDERFLOW_BOUND` per operation).
const RATIO_BOUND: f64 = (2.0 * TRUNCATION + dd::DIV_BOUND + 2.0 * dd::MUL_F64_BOUND) * SLACK;

/// The relative error bound of an irrational coordinate's estimate, the ratio divided by the
/// double-double √n (with its own relative error, as a divisor): about 30.7u².
const IRRATIONAL_BOUND: f64 = (RATIO_BOUND + dd::DIV_BOUND + dd::SQRT_BOUND) * SLACK;

// Both estimates are within 32u² = 2^−101 of the exact value, relatively, so within 2^−59 steps for
// every coordinate the settling sees (|X| < 2^41 + 1), absolute terms included: the settling's
// premise.
const _: () = assert!(RATIO_BOUND < 32.0 * U2 && IRRATIONAL_BOUND < 32.0 * U2);

/// The smallest magnitude an irrational map coordinate can have, 2^−300 units (see
/// [`FrameRecord::to_map`]): every bisection stays above it.
const IRRATIONAL_MIN: f64 = const_pow2!(-300);

/// The largest magnitude any map coordinate can have, 2^148 units (see [`FrameRecord::to_map`]).
const MAP_RESULT_MAX: f64 = const_pow2!(148);

/// The certificate's relative margin in [`FrameRecord::to_map`], 2^−98 = 256u².
const CERTIFICATE_MARGIN: f64 = const_pow2!(-98);

/// The relative error bound of an irrational map coordinate's double-double value: the estimate
/// of `k_n·a/(k_d·√n)` (the ratio, a division by √n), then the addition of the origin, the product
/// by q and the division by S·p, about 47.5u², below half the certificate's margin.
const MAP_BOUND: f64 = (IRRATIONAL_BOUND + dd::ADD_BOUND + dd::MUL_BOUND + dd::DIV_BOUND) * SLACK;

const _: () = assert!(2.0 * MAP_BOUND < CERTIFICATE_MARGIN);

/// The widening of the bisection's bracket in [`FrameRecord::to_map`], 2^−50.
const BRACKET_WIDENING: f64 = const_pow2!(-50);

/// Map coordinates in the frame's linear unit (metres unless the record says otherwise): easting,
/// northing and height above the vertical datum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPoint {
    /// Easting, in the frame's linear unit.
    pub easting: f64,
    /// Northing, in the frame's linear unit.
    pub northing: f64,
    /// Height, in the frame's linear unit.
    pub height: f64,
}

/// Map coordinates in the frame's linear unit as exact decimals, as a survey file or a coordinate
/// list gives them: easting, northing and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecimalPoint {
    /// Easting, in the frame's linear unit.
    pub easting: Decimal,
    /// Northing, in the frame's linear unit.
    pub northing: Decimal,
    /// Height, in the frame's linear unit.
    pub height: Decimal,
}

/// A map coordinate in metres times the steps per metre, exactly, as a double-double; or the
/// knowledge that it is a nonzero value below 2^−969 in magnitude (`tiny`), where `two_prod`'s
/// error part would fall below 2^−1074 and `dd` is zero.
#[derive(Clone, Copy)]
struct Scaled {
    dd: DD,
    tiny: bool,
}

impl Scaled {
    /// `v · s`, for a map coordinate |v| ≤ 2^62 m and 2^22 < s < 2^32 steps per metre: the product
    /// cannot overflow, so `exact_product` refuses only a product below 2^−969, the tiny case.
    fn new(v: f64, s: f64) -> Scaled {
        match DD::exact_product(v, s) {
            Some(dd) => Scaled { dd, tiny: false },
            None => Scaled {
                dd: DD::ZERO,
                tiny: true,
            },
        }
    }
}

/// `±(scaled − origin)` rounded to the nearest step, ties to even, exactly.
///
/// A tiny scaled value is a nonzero number below 2^−969 in magnitude, so ±(tiny − origin) rounds
/// to ∓origin, never a tie, and is not exact.
fn round_axis(v: Scaled, origin: i64, negate: bool) -> Result<Rounded<Coord>, NumError> {
    let (dd, offset) = if negate {
        (v.dd.negate(), budget(i128::from(origin).checked_neg())?)
    } else {
        (v.dd, i128::from(origin))
    };
    // |dd| < 2^94, far inside round_shifted's 2^100, and |offset| ≤ 2^60, so it always answers.
    let r = budget(dd.round_shifted(offset))?;
    Ok(Rounded::new(
        Coord::from_i128(r.value())?,
        r.is_exact() && !v.tiny,
    ))
}

/// The nearest even of two neighbouring integers.
const fn even(a: i128, b: i128) -> i128 {
    if a.rem_euclid(2) == 0 { a } else { b }
}

/// The exact rounding of a coordinate from its candidate `x` (the estimate `d` rounded) and the
/// exact sign of X − t/2.
///
/// The estimate lies within 2^−59 of X, so X lies within 1/2 + 2^−59 of x and its rounding is
/// x − 1, x or x + 1. If `d` is farther than [`FILTER`] from x and from both midpoints x ± 1/2, X
/// is neither a whole step nor a midpoint and rounds to x, inexactly. Otherwise exact signs decide:
/// X = x is exact; X > x + 1/2 rounds up, X = x + 1/2 is a tie; and likewise below. A failed exact
/// sign (ruled out by the budget) is reported as [`NumError::BudgetExceeded`], never replaced by
/// the candidate; so are the steps x ± 1 and 2x ± 1, which fit (|x| ≤ 2^41), and the magnitude of
/// the finite fraction.
fn settle(
    cand: Rounded<i128>,
    d: DD,
    sign: impl Fn(i128) -> Option<Sign>,
) -> Result<Rounded<i128>, NumError> {
    let x = cand.value();
    let inexact = |value: i128| Rounded::new(value, false);
    // |x| ≤ 2^41, so x is a double exactly and hi − x is exact (Sterbenz, or x = 0).
    #[allow(
        clippy::cast_precision_loss,
        reason = "|x| ≤ 2^41 < 2^53: exact in f64"
    )]
    let xf = x as f64;
    let frac = math::abs((d.hi() - xf) + d.lo())?.get();
    if frac > FILTER && frac < 0.5 - FILTER {
        return Ok(inexact(x));
    }
    let two_x = budget(x.checked_mul(2))?;
    let (above, below) = (budget(two_x.checked_add(1))?, budget(two_x.checked_sub(1))?);
    let (up, down) = (budget(x.checked_add(1))?, budget(x.checked_sub(1))?);
    if budget(sign(two_x))? == Sign::Zero {
        return Ok(Rounded::new(x, true));
    }
    match budget(sign(above))? {
        Sign::Positive => return Ok(inexact(up)),
        Sign::Zero => return Ok(inexact(even(x, up))),
        Sign::Negative => {}
    }
    Ok(match budget(sign(below))? {
        Sign::Negative => inexact(down),
        Sign::Zero => inexact(even(down, x)),
        Sign::Positive => inexact(x),
    })
}

/// A rounded `i128` coordinate as a range-checked [`Coord`]: an out-of-range one is refused with
/// its value.
fn to_coord(r: Rounded<i128>) -> Result<Rounded<Coord>, NumError> {
    Ok(Rounded::new(Coord::from_i128(r.value())?, r.is_exact()))
}

/// The nearest double to `num / den` as a rounded value; the divisions here always have
/// 0 < den < 2^127, so it always answers, and a refusal is reported as
/// [`NumError::BudgetExceeded`].
fn quotient(num: i128, den: u128) -> Result<Rounded<f64>, NumError> {
    budget(rational_to_f64(num, den))
}

/// `a · b`, exactly, or [`NumError::BudgetExceeded`]: every product here has a bound written where
/// it is formed. The first factor is the one with fewer limbs, which the product loops over.
fn product(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_mul(b))
}

/// An integer as a [`Big`].
fn big(v: i128) -> Big {
    Big::from_i128(v)
}

/// p/q as a double-double, for q > 0: within [`RATIO_BOUND`]·|p/q| + 2^−900.
///
/// Each operand keeps its leading 106 bits, truncated toward zero (exact below 2^106, and within
/// 2^−105 relatively otherwise); the double-double quotient of the two kept parts is scaled by the
/// power of two the truncations dropped, in two steps that stay inside the normal exponents. A
/// quotient whose scaling is below 2^−1006 is below 2^−901 and is returned as zero.
fn ratio_dd(p: &Big, q: &Big) -> Result<DD, NumError> {
    let (pv, ps) = budget(leading_bits(p, RATIO_BITS))?;
    let (qv, qs) = budget(leading_bits(q, RATIO_BITS))?;
    // Both shifts are below 1536, so their difference fits i32, and so does half of it.
    let e = budget(
        i32::try_from(ps)
            .ok()
            .zip(i32::try_from(qs).ok())
            .and_then(|(a, b)| a.checked_sub(b)),
    )?;
    if pv == 0 || e < RATIO_FLUSH {
        return Ok(DD::ZERO);
    }
    let r = budget(DD::from_i128(pv))?.div(budget(DD::from_i128(qv))?)?;
    let half = budget(e.checked_div(2))?;
    let rest = budget(e.checked_sub(half))?;
    r.mul_f64(budget(pow2(half))?)?.mul_f64(budget(pow2(rest))?)
}

/// A map value exactly: m·2^two·5^five CRS units. A double is m·2^two with five = 0; a decimal
/// m·10^e has two = five = e; zero is (0, 0, 0).
#[derive(Clone, Copy)]
struct Value {
    m: i128,
    two: i32,
    five: i32,
}

impl Value {
    /// A double's exact value, or [`NumError::NotFinite`] for NaN and the infinities.
    fn of_f64(v: f64) -> Result<Value, NumError> {
        let (m, k) = dyadic(v).ok_or(NumError::NotFinite)?;
        Ok(Value {
            m: i128::from(m),
            two: k,
            five: 0,
        })
    }

    /// A decimal's exact value.
    const fn of_decimal(d: Decimal) -> Value {
        Value {
            m: d.significand(),
            two: d.exponent(),
            five: d.exponent(),
        }
    }

    /// The value cut toward zero to whole units, exactly.
    ///
    /// m·2^two·5^five with both exponents at least zero is a whole number (below 2^127·2^971 for a
    /// double, 2^127·10^340 < 2^1257 for a decimal); with both negative it is m over
    /// 2^−two·5^−five, divided exactly when that fits below 2^127, and otherwise below one unit,
    /// since |m| < 2^127.
    fn whole(self) -> Result<Big, NumError> {
        let up = product(
            &product(
                &big(self.m),
                &budget(pow2_big(self.two.max(0).unsigned_abs()))?,
            )?,
            &budget(pow5_big(self.five.max(0).unsigned_abs()))?,
        )?;
        let down = 2u128
            .checked_pow(self.two.min(0).unsigned_abs())
            .zip(5u128.checked_pow(self.five.min(0).unsigned_abs()))
            .and_then(|(a, b)| a.checked_mul(b))
            .filter(|&d| d < 1 << 127);
        match down {
            Some(d) => Ok(budget(divrem_u128(&up, d))?.0),
            None => Ok(Big::ZERO),
        }
    }
}

/// What a frame's exact conversion needs of its record: S·p, with S steps per metre and the unit
/// p/q metres (below 2^96), and q (below 2^64).
#[derive(Clone, Copy)]
struct Scaling {
    sp: i128,
    q: i128,
}

impl Scaling {
    /// The record's scaling: S < 2^32 and p < 2^64, so S·p < 2^96 fits.
    fn of(frame: &FrameRecord) -> Result<Scaling, NumError> {
        let unit = frame.linear_unit();
        let s = i128::from(steps_per_metre(frame.step()));
        Ok(Scaling {
            sp: budget(s.checked_mul(i128::from(unit.num())))?,
            q: i128::from(unit.den()),
        })
    }

    /// Whether the value's image, |v·S·p/q| steps, exceeds 2^200, decided exactly:
    /// |m|·S·p·2^two⁺·5^five⁺ against q·2^(200 + two⁻)·5^five⁻, both below 2^1394 (the module
    /// documentation's input bounds).
    fn beyond(self, v: Value) -> Result<bool, NumError> {
        let pow = |two: u32, five: u32| -> Result<Big, NumError> {
            product(&budget(pow2_big(two))?, &budget(pow5_big(five))?)
        };
        let m = big(budget(i128::try_from(v.m.unsigned_abs()).ok())?);
        let left = product(
            &product(&big(self.sp), &m)?,
            &pow(v.two.max(0).unsigned_abs(), v.five.max(0).unsigned_abs())?,
        )?;
        let down = budget(INPUT_BITS.checked_add(v.two.min(0).unsigned_abs()))?;
        let right = product(&big(self.q), &pow(down, v.five.min(0).unsigned_abs())?)?;
        Ok(left > right)
    }

    /// The exact offsets of K map values from their origins in steps, over a common denominator:
    /// `u_i`/d = `v_i`·S·p/q − `O_i`, with d = q·2^A·5^C for the largest A and C any value's
    /// denominator needs. Every value's image is within 2^200 steps, so |`u_i`| < 2^1395 (the
    /// module documentation's budget).
    fn offsets<const K: usize>(
        self,
        values: [Value; K],
        origins: [i64; K],
    ) -> Result<([Big; K], Big), NumError> {
        let (mut a, mut c) = (0u32, 0u32);
        for v in values {
            a = a.max(v.two.min(0).unsigned_abs());
            c = c.max(v.five.min(0).unsigned_abs());
        }
        let d = product(
            &product(&big(self.q), &budget(pow2_big(a))?)?,
            &budget(pow5_big(c))?,
        )?;
        let mut u = [Big::ZERO; K];
        for ((slot, v), o) in u.iter_mut().zip(values).zip(origins) {
            // two + a ≥ 0 and five + c ≥ 0 by the choice of a and c.
            let lift = |k: i32, by: u32| -> Result<u32, NumError> {
                budget(
                    i32::try_from(by)
                        .ok()
                        .and_then(|by| k.checked_add(by))
                        .and_then(|e| u32::try_from(e).ok()),
                )
            };
            let n = product(
                &product(
                    &budget(pow2_big(lift(v.two, a)?))?,
                    &product(&big(self.sp), &big(v.m))?,
                )?,
                &budget(pow5_big(lift(v.five, c)?))?,
            )?;
            *slot = budget(n.checked_sub(&product(&big(i128::from(o)), &d)?))?;
        }
        Ok((u, d))
    }
}

/// One frame coordinate as an exact value X = p/(q·ρ) steps, with q > 0 and ρ = 1, or √n for the
/// `root` n, not a square.
struct Axis {
    p: Big,
    q: Big,
    root: Option<u64>,
}

impl Axis {
    /// The exact sign of X − t/2, that is of 2p − t·q·ρ (q·ρ > 0); `None` only if a step does not
    /// fit, which the module's budget rules out.
    fn sign_half(&self, t: &Big) -> Option<Sign> {
        let two_p = self.p.checked_add(&self.p)?;
        let tq = t.checked_mul(&self.q)?;
        match self.root {
            None => Some(two_p.checked_sub(&tq)?.signum()),
            Some(n) => {
                let (a, b): (Square, Square) = (two_p.resize()?, tq.resize()?);
                sign_minus_sqrt(&a, &b, n)
            }
        }
    }

    /// The exact sign of X − (m + h/2), for h = 0 or 1.
    fn sign_at(&self, m: i128, half: bool) -> Result<Sign, NumError> {
        // |2m + 1| ≤ 2^128 fits a Big.
        let t = product(&big(2), &big(m))?.checked_add(&big(i128::from(half)));
        budget(t.and_then(|t| self.sign_half(&t)))
    }

    /// A double-double within [`RATIO_BOUND`]·|X| (rational) or [`IRRATIONAL_BOUND`]·|X|
    /// (irrational) of X, plus 2^−900.
    fn estimate(&self) -> Result<DD, NumError> {
        let ratio = ratio_dd(&self.p, &self.q)?;
        match self.root {
            None => Ok(ratio),
            // n < 2^63 fits i64.
            Some(n) => ratio.div(DD::from_i64(budget(i64::try_from(n).ok())?).sqrt()?),
        }
    }

    /// X rounded to the nearest integer, ties to even, with whether it was exact; saturated at the
    /// ends of `i128` (see [`Axis::far`]).
    fn round(&self) -> Result<Rounded<i128>, NumError> {
        let d = self.estimate()?;
        if d.hi() > RANGE_FILTER || d.hi() < -RANGE_FILTER {
            return self.far(d);
        }
        // |d| ≤ 2^41, inside round_shifted's range, so it answers.
        let r = budget(d.round_shifted(0))?;
        settle(Rounded::new(r.value(), r.is_exact()), d, |t| {
            self.sign_half(&big(t))
        })
    }

    /// The exact rounding of an X whose estimate `d` is beyond 2^41: `i128::MAX` if X > MAX − 1/2
    /// and `i128::MIN` if X ≤ MIN + 1/2 (the numbers layer's saturated values; MAX − 1/2 itself
    /// rounds to the even MAX − 1), otherwise by bisection for ⌊X⌋ over the integers of the
    /// estimate's bracket, d ± (2^−50·|d.hi| + |d.lo| + 2), which holds X because the estimate is
    /// within 2^−101·|X| + 2^−900 of it and the bracket's ends are rounded by half an ulp at most;
    /// the ends are checked exactly, and a bracket that failed would be a defect, reported. At
    /// most 128 halvings close a bracket within `i128`.
    fn far(&self, d: DD) -> Result<Rounded<i128>, NumError> {
        if self.sign_at(BELOW_MAX, true)? == Sign::Positive {
            return Ok(Rounded::new(i128::MAX, false));
        }
        if self.sign_at(i128::MIN, true)? != Sign::Positive {
            return Ok(Rounded::new(i128::MIN, false));
        }
        let size = |w: f64| math::abs(w).map(geom_num::Real::get);
        let width = size(d.hi())? * FAR_MARGIN + size(d.lo())? + 2.0;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a saturating conversion on purpose: X lies strictly inside i128 here, so a \
                      bracket end beyond it is replaced by the end of i128"
        )]
        let (mut lo, mut hi) = (
            math::floor(d.hi() - width)?.get() as i128,
            (math::ceil(d.hi() + width)?.get() as i128).saturating_add(1),
        );
        if self.sign_at(lo, false)? == Sign::Negative || self.sign_at(hi, false)? != Sign::Negative
        {
            return Err(NumError::BudgetExceeded);
        }
        // X ≥ lo and X < hi throughout; the gap is below 2^128 and halves every step.
        for _ in 0..130 {
            let gap = hi.abs_diff(lo);
            if gap <= 1 {
                break;
            }
            let mid = budget(lo.checked_add_unsigned(gap / 2))?;
            if self.sign_at(mid, false)? == Sign::Negative {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        if hi.abs_diff(lo) != 1 {
            return Err(NumError::BudgetExceeded);
        }
        if self.sign_at(lo, false)? == Sign::Zero {
            return Ok(Rounded::new(lo, true));
        }
        // lo = ⌊X⌋ < MAX here, so lo + 1 fits.
        let up = budget(lo.checked_add(1))?;
        Ok(Rounded::new(
            match self.sign_at(lo, true)? {
                Sign::Positive => up,
                Sign::Zero => even(lo, up),
                Sign::Negative => lo,
            },
            false,
        ))
    }
}

/// The saturated out-of-range report for a coordinate of the given sign.
const fn saturated(negative: bool) -> NumError {
    NumError::OutOfRange {
        value: if negative { i128::MIN } else { i128::MAX },
    }
}

/// The magnitude of a [`Big`] below its range's ends.
fn magnitude(w: &Big) -> Result<Big, NumError> {
    if w.is_negative() {
        budget(w.checked_neg())
    } else {
        Ok(*w)
    }
}

impl FrameRecord {
    /// Map coordinates in the frame's linear unit converted to frame grid coordinates, rounded
    /// once to the nearest step, ties to even, with whether no rounding happened (see the module
    /// documentation).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a coordinate is NaN or infinite; [`NumError::OutOfRange`] if a
    /// result lies beyond ±(2^40 − 1) steps, with that coordinate exactly rounded (the height
    /// first, then x, then y), saturated at the ends of `i128`. [`NumError::BudgetExceeded`] only
    /// if a checked step or an exact sign failed, which the bit budgets rule out.
    pub fn to_frame(self, map: MapPoint) -> Result<Rounded<P3>, NumError> {
        let e = Value::of_f64(map.easting)?;
        let n = Value::of_f64(map.northing)?;
        let h = Value::of_f64(map.height)?;
        let metres = self.linear_unit().is_metre();
        let near = |v: f64| (-MAP_MAX..=MAP_MAX).contains(&v);
        #[allow(
            clippy::cast_precision_loss,
            reason = "the steps per metre are below 2^32 < 2^53: exact in f64"
        )]
        let s = steps_per_metre(self.step()) as f64;
        let [oe, on, oh] = self.origin();
        let z = if metres && near(map.height) {
            round_axis(Scaled::new(map.height, s), oh, false)?
        } else {
            self.height_exact(h)?
        };
        let turns = self.rotation().quarter_turn_count();
        let fast = metres && near(map.easting) && near(map.northing) && self.scale().is_one();
        let (x, y) = match turns.filter(|_| fast) {
            Some(turns) => {
                let (e, n) = (Scaled::new(map.easting, s), Scaled::new(map.northing, s));
                match turns {
                    0 => (round_axis(e, oe, false)?, round_axis(n, on, false)?),
                    1 => (round_axis(n, on, false)?, round_axis(e, oe, true)?),
                    2 => (round_axis(e, oe, true)?, round_axis(n, on, true)?),
                    _ => (round_axis(n, on, true)?, round_axis(e, oe, false)?),
                }
            }
            None => self.horizontal_exact(e, n)?,
        };
        Ok(point(x, y, z))
    }

    /// Map coordinates in the frame's linear unit, given as exact decimals, converted to frame grid
    /// coordinates: rounded once, to the nearest step, ties to even, never through `f64`, with
    /// whether no rounding happened (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a result lies beyond ±(2^40 − 1) steps, with that coordinate
    /// exactly rounded (the height first, then x, then y), saturated at the ends of `i128`.
    /// [`NumError::BudgetExceeded`] only if a checked step or an exact sign failed, which the bit
    /// budgets rule out.
    pub fn to_frame_decimal(self, map: DecimalPoint) -> Result<Rounded<P3>, NumError> {
        let z = self.height_exact(Value::of_decimal(map.height))?;
        let (x, y) = self.horizontal_exact(
            Value::of_decimal(map.easting),
            Value::of_decimal(map.northing),
        )?;
        Ok(point(x, y, z))
    }

    /// A height on the exact path: z = W − `O_H` over its own denominator.
    fn height_exact(&self, h: Value) -> Result<Rounded<Coord>, NumError> {
        let scaling = Scaling::of(self)?;
        if scaling.beyond(h)? {
            // |W| > 2^200 and |O| ≤ 2^60: z has W's sign, the sign of m.
            return Err(saturated(h.m < 0));
        }
        let [_, _, oh] = self.origin();
        let ([u], d) = scaling.offsets([h], [oh])?;
        to_coord(
            Axis {
                p: u,
                q: d,
                root: None,
            }
            .round()?,
        )
    }

    /// The horizontal coordinates on the exact path (see the module documentation): x then y.
    fn horizontal_exact(
        &self,
        e: Value,
        n: Value,
    ) -> Result<(Rounded<Coord>, Rounded<Coord>), NumError> {
        let scaling = Scaling::of(self)?;
        let rotation = self.rotation();
        let scale = self.scale();
        let (c, s) = (
            big(i128::from(rotation.cos())),
            big(i128::from(rotation.sin())),
        );
        let (kd, kn) = (big(i128::from(scale.den())), big(i128::from(scale.num())));
        let [oe, on, _] = self.origin();
        if scaling.beyond(e)? || scaling.beyond(n)? {
            return Err(Self::horizontal_beyond(
                e,
                n,
                scaling,
                [oe, on],
                [&c, &s, &kd],
            )?);
        }
        let ([ue, un], d) = scaling.offsets([e, n], [oe, on])?;
        // k_d·(c·u_E + s·u_N) and k_d·(c·u_N − s·u_E), below 2^1491.
        let combine = |a: &Big, u: &Big, b: &Big, v: &Big, negate: bool| -> Result<Big, NumError> {
            let (au, bv) = (product(a, u)?, product(b, v)?);
            let sum = if negate {
                au.checked_sub(&bv)
            } else {
                au.checked_add(&bv)
            };
            product(&kd, &budget(sum)?)
        };
        let px = combine(&c, &ue, &s, &un, false)?;
        let py = combine(&c, &un, &s, &ue, true)?;
        let (q, root) = match rotation.exact_norm() {
            Some(r) => (product(&big(i128::from(r)), &product(&kn, &d)?)?, None),
            None => (product(&kn, &d)?, Some(rotation.norm2())),
        };
        let x = Axis { p: px, q, root };
        let y = Axis {
            p: py,
            q: x.q,
            root,
        };
        Ok((to_coord(x.round()?)?, to_coord(y.round()?)?))
    }

    /// The report for horizontal inputs whose image is beyond 2^200 steps: the larger of x and y,
    /// saturated, with its sign.
    ///
    /// Cut to whole units, each input moves by less than one unit, so its image by less than
    /// S·u ≤ 2^64 steps, and (x, y), which is `k_d/k_n` times a rotation of the offsets, by less
    /// than `k_d/k_n`·2^64.5 steps; while |(x, y)| > `k_d/k_n`·(2^200 − 2^61) ≥ 2^135.9, since
    /// `k_d/k_n` ≥ 2^−64. So the larger coordinate of the cut inputs' (x', y') has at least
    /// |(x', y')|/√2, the true coordinate has the same sign, and its magnitude is beyond 2^127:
    /// saturated, exactly as the numbers layer documents. The cut values are formed exactly over
    /// the denominator q: integers below 2^1450.
    fn horizontal_beyond(
        e: Value,
        n: Value,
        scaling: Scaling,
        origin: [i64; 2],
        [c, s, kd]: [&Big; 3],
    ) -> Result<NumError, NumError> {
        let [oe, on] = origin;
        let cut = |v: Value, o: i64| -> Result<Big, NumError> {
            budget(
                product(&big(scaling.sp), &v.whole()?)?
                    .checked_sub(&product(&big(i128::from(o)), &big(scaling.q))?),
            )
        };
        let (ue, un) = (cut(e, oe)?, cut(n, on)?);
        let px = product(
            kd,
            &budget(product(c, &ue)?.checked_add(&product(s, &un)?))?,
        )?;
        let py = product(
            kd,
            &budget(product(c, &un)?.checked_sub(&product(s, &ue)?))?,
        )?;
        let larger = if magnitude(&px)? >= magnitude(&py)? {
            px
        } else {
            py
        };
        Ok(saturated(larger.is_negative()))
    }

    /// Frame grid coordinates converted to map coordinates in the frame's linear unit: the nearest
    /// doubles to the exact values, ties to even, with whether they are exact (see the module
    /// documentation).
    ///
    /// Each coordinate is V = (O + Δ)·q/(S·p) for the unit p/q, with Δ = z for the height. Heights,
    /// Map frames and quarter turns at scale one divide (O + Δ)·q (below 2^61·2^64 = 2^125) by S·p
    /// (below 2^96) in `i128`; a rational Ground frame divides
    /// (`O·k_d·r` + `k_n·a`)·q (below (2^156 + 2^136)·2^64 < 2^221) by `k_d·r·S·p` (below 2^192), with
    /// a = c·x − s·y or s·x + c·y below 2^72, as 1536-bit integers.
    ///
    /// For an irrational direction each horizontal coordinate is V = (O + `κ`)·q/(S·p) with
    /// `κ` = `k_n·a/(k_d·√n)`, a ≠ 0 and n = c² + s² not a square (a = 0 gives the rational
    /// O·q/(S·p)). Bounds: |`κ`| < 2^136, so |V| < 2^137/2^22·2^32 < 2^148 (the unit is at least
    /// 2^−32 m); and |V| > 2^−300, because V = (A − `B√n)/(k_d·S·u·√n`) with A = `k_n·a` and
    /// B = −`O·k_d`, A² − B²n is a nonzero integer, so |A − B√n| ≥ 1/(|A| + |B|√n) ≥ 2^−138,
    /// while `k_d·S·u·√n` < 2^64·2^32·2^32·2^31.5 = 2^159.5. The double-double value (the ratio
    /// `k_n·a/k_d` and a division by √n, then an addition, a product and a division) is within
    /// `MAP_BOUND`·(|V| + |`κ`|·q/(S·p)) of V, about 47.5u². With a margin of 2^−98 = 256u² times that
    /// sum, doubled, the nearest double to the double-double value is certified when the whole
    /// uncertainty interval lies inside its rounding interval. Otherwise the nearest double is
    /// found by bisection with exact signs (`IrrationalMap::nearest`). V is irrational, so there is
    /// never a tie, and the coordinate is never exact.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] only if a double-double step overflows, and
    /// [`NumError::BudgetExceeded`] only if a checked step or an exact sign failed: the budgets
    /// rule out both.
    pub fn to_map(self, p: P3) -> Result<Rounded<MapPoint>, NumError> {
        let s = steps_per_metre(self.step());
        let unit = self.linear_unit();
        let qu = i128::from(unit.den());
        // S < 2^32 and p < 2^64: S·p < 2^96.
        let sp = budget(u128::from(s).checked_mul(u128::from(unit.num())))?;
        let [oe, on, oh] = self.origin().map(i128::from);
        let (x, y, z) = (
            i128::from(p.x.get()),
            i128::from(p.y.get()),
            i128::from(p.z.get()),
        );
        let linear = |o: i128, a: i128| -> Result<Rounded<f64>, NumError> {
            quotient(
                budget(o.checked_add(a).and_then(|v| v.checked_mul(qu)))?,
                sp,
            )
        };
        let height = linear(oh, z)?;
        let rotation = self.rotation();
        let scale = self.scale();
        let (c, sn) = (i128::from(rotation.cos()), i128::from(rotation.sin()));
        // a·u + b·v exactly, for the budgets below.
        let combine = |a: i128, u: i128, b: i128, v: i128| -> Result<i128, NumError> {
            budget(
                a.checked_mul(u)
                    .zip(b.checked_mul(v))
                    .and_then(|(au, bv)| au.checked_add(bv)),
            )
        };
        // Rotated, unscaled offsets: (c·x − s·y, s·x + c·y), each below 2^72 in magnitude.
        let ns = budget(sn.checked_neg())?;
        let (ae, an) = (combine(c, x, ns, y)?, combine(sn, x, c, y)?);
        let (easting, northing) = match (rotation.quarter_turn_count(), scale.is_one()) {
            // A quarter turn has r = 1, so the offsets are already in steps.
            (Some(_), true) => (linear(oe, ae)?, linear(on, an)?),
            _ => match rotation.exact_norm() {
                Some(r) => {
                    let kdr = product(&big(i128::from(r)), &big(i128::from(scale.den())))?;
                    let den = product(&kdr, &big(budget(i128::try_from(sp).ok())?))?;
                    let kn = big(i128::from(scale.num()));
                    let rational = |o: i128, a: i128| -> Result<Rounded<f64>, NumError> {
                        let num =
                            budget(product(&big(o), &kdr)?.checked_add(&product(&kn, &big(a))?))?;
                        budget(quotient_to_f64(&product(&big(qu), &num)?, &den))
                    };
                    (rational(oe, ae)?, rational(on, an)?)
                }
                None => (
                    self.irrational_to_map(oe, ae, sp)?,
                    self.irrational_to_map(on, an, sp)?,
                ),
            },
        };
        Ok(Rounded::new(
            MapPoint {
                easting: easting.value(),
                northing: northing.value(),
                height: height.value(),
            },
            easting.is_exact() && northing.is_exact() && height.is_exact(),
        ))
    }

    /// One horizontal map coordinate for an irrational direction: the nearest double to
    /// (O + `k_n·a/(k_d·√n))·q/(S·p)` (see [`FrameRecord::to_map`]).
    fn irrational_to_map(&self, origin: i128, a: i128, sp: u128) -> Result<Rounded<f64>, NumError> {
        let qu = i128::from(self.linear_unit().den());
        if a == 0 {
            // |O·q| < 2^124.
            return quotient(budget(origin.checked_mul(qu))?, sp);
        }
        let scale = self.scale();
        let n2 = self.rotation().norm2();
        let kn_a = product(&big(i128::from(scale.num())), &big(a))?;
        let kd = big(i128::from(scale.den()));
        // c² + s² < 2^63 fits i64; |O| ≤ 2^60, q < 2^64 and S·p < 2^96 are double-doubles exactly.
        let n2i = budget(i64::try_from(n2).ok())?;
        let kappa = ratio_dd(&kn_a, &kd)?.div(DD::from_i64(n2i).sqrt()?)?;
        let o = budget(DD::from_i128(origin))?;
        let spi = budget(i128::try_from(sp).ok())?;
        let units = o
            .add(kappa)?
            .mul(budget(DD::from_i128(qu))?)?
            .div(budget(DD::from_i128(spi))?)?;
        let (h, l) = (units.hi(), units.lo());
        // The parts are finite (a double-double's invariant), so `math::abs` refuses none.
        let (h_size, k_size, l_size) = (
            math::abs(h)?.get(),
            math::abs(kappa.hi())?.get(),
            math::abs(l)?.get(),
        );
        #[allow(
            clippy::cast_precision_loss,
            reason = "q and S·p only scale the certificate's margin, which has room for their \
                      rounding (a relative 2^−52 each against a factor of two)"
        )]
        let (qf, spf) = (qu as f64, sp as f64);
        // The certificate: |V − (h + l)| ≤ err, and h + l ± 2·err inside h's rounding interval.
        let err = (h_size + k_size * qf / spf) * CERTIFICATE_MARGIN;
        let up = h.next_up() - h;
        let down = h - h.next_down();
        if h != 0.0 && l + 2.0 * err < 0.5 * up && l - 2.0 * err > -0.5 * down {
            return Ok(Rounded::new(h, false));
        }
        let exact = IrrationalMap {
            origin,
            kn_a,
            kd,
            sp: big(spi),
            q: big(qu),
            n2,
        };
        // A bracket of |V| from the double-double value, checked exactly inside `nearest`.
        let spread = l_size + 2.0 * err;
        let lo = (h_size - spread) * (1.0 - BRACKET_WIDENING);
        let lo = if lo > IRRATIONAL_MIN {
            lo
        } else {
            IRRATIONAL_MIN
        };
        let hi = (h_size + spread) * (1.0 + BRACKET_WIDENING);
        let hi = if hi < MAP_RESULT_MAX {
            hi
        } else {
            MAP_RESULT_MAX
        };
        // `None` would mean an exact sign overflowed its budget, which the bounds rule out: it is
        // reported, never replaced by the double-double's value.
        let value = budget(exact.nearest(lo, hi))?;
        Ok(Rounded::new(value, false))
    }
}

/// A point from its three rounded coordinates: exact when all three are.
fn point(x: Rounded<Coord>, y: Rounded<Coord>, z: Rounded<Coord>) -> Rounded<P3> {
    Rounded::new(
        P3 {
            x: x.value(),
            y: y.value(),
            z: z.value(),
        },
        x.is_exact() && y.is_exact() && z.is_exact(),
    )
}

/// One horizontal map coordinate of a frame with an irrational direction, as exact integers:
/// V = (O + `k_n·a/(k_d·√n))·q/(S·p)` units, with a ≠ 0 the rotated integer offset, n = c² + s² not
/// a square.
struct IrrationalMap {
    origin: i128,
    kn_a: Big,
    kd: Big,
    sp: Big,
    q: Big,
    n2: u64,
}

impl IrrationalMap {
    /// The exact sign of V − w for w = p·2^f.
    ///
    /// V − w = ((O + κ)·q − w·S·p)/(S·p) with κ = `k_n·a/(k_d·√n)`, so its sign is the sign of
    /// `q·k_n·a` − (w·S·p − `O·q)·k_d·√n` (multiplying by `S·p·k_d·√n` > 0). For f < 0 both sides
    /// are multiplied by 2^(−f): A = `q·k_n·a·2^(−f)` and B = (p·S·p − `O·q·2^(−f))·k_d`; for f ≥ 0,
    /// A = `q·k_n·a` and B = (p·S·p·2^f − `O·q)·k_d`. The sign of A − B·√n is decided exactly by
    /// comparing squares.
    ///
    /// Budget, for the w the conversion uses (zero, the two ends of the bracket and the midpoints
    /// between neighbouring doubles inside it, so 2^−301 ≤ |w| ≤ 2^148 otherwise, with |p| < 2^55
    /// and 2^−356 ≤ 2^f): for f < 0, |A| < 2^64·2^136·2^356 = 2^556 and
    /// |B| < (2^55·2^96 + 2^60·2^64·2^356)·2^64 < 2^545; for f ≥ 0, |A| < 2^200 and, since
    /// |p·2^f| = |w| ≤ 2^148, |B| < (2^148·2^96 + 2^124)·2^64 < 2^309. So A² < 2^1112 and
    /// B²·n < 2^1090·2^63 = 2^1153, inside 1536 bits.
    fn sign_minus(&self, p: i128, f: i32) -> Option<Sign> {
        let qka = self.q.checked_mul(&self.kn_a)?;
        let qo = Big::from_i128(self.origin).checked_mul(&self.q)?;
        let ps = Big::from_i128(p).checked_mul(&self.sp)?;
        let k = f.unsigned_abs();
        let (a, b) = if f < 0 {
            let scale = pow2_big(k)?;
            let a = scale.checked_mul(&qka)?;
            (a, ps.checked_sub(&scale.checked_mul(&qo)?)?)
        } else {
            (qka, pow2_big(k)?.checked_mul(&ps)?.checked_sub(&qo)?)
        };
        sign_minus_sqrt::<24>(&a, &self.kd.checked_mul(&b)?, self.n2)
    }

    /// The exact sign of |V| − w for a positive dyadic w = p·2^f, given whether V is negative.
    fn abs_minus(&self, negative: bool, p: i128, f: i32) -> Option<Sign> {
        if negative {
            self.sign_minus(p.checked_neg()?, f).map(Sign::negate)
        } else {
            self.sign_minus(p, f)
        }
    }

    /// The nearest double to V, by bisection over the positive doubles in [lo, hi] (a bracket of
    /// |V|, checked exactly, with the proven bounds as the fallback) decided by exact signs at the
    /// midpoints between neighbouring doubles. Positive doubles are ordered like their bit
    /// patterns, so at most 63 halvings find the double whose rounding interval holds |V|; V is
    /// irrational, so it is never a midpoint. `None` only if a checked step failed, which the
    /// budgets of [`IrrationalMap::sign_minus`] rule out.
    fn nearest(&self, lo: f64, hi: f64) -> Option<f64> {
        let negative = self.sign_minus(0, 0)? == Sign::Negative;
        let holds = |w: f64, want: Sign| -> Option<bool> {
            let (m, e) = dyadic(w)?;
            Some(self.abs_minus(negative, i128::from(m), e)? == want)
        };
        let lo = if holds(lo, Sign::Positive)? {
            lo
        } else {
            IRRATIONAL_MIN
        };
        let hi = if holds(hi, Sign::Negative)? {
            hi
        } else {
            MAP_RESULT_MAX
        };
        let (mut a, mut b) = (lo.to_bits(), hi.to_bits());
        for _ in 0..70 {
            if a >= b {
                break;
            }
            // a < b, both bit patterns of positive finite doubles: every step below fits.
            let mid = a.checked_add(b.checked_sub(a)?.checked_div(2)?)?;
            let next = mid.checked_add(1)?;
            let (m1, e1) = dyadic(f64::from_bits(mid))?;
            let (m2, e2) = dyadic(f64::from_bits(next))?;
            let e0 = e1.min(e2);
            // The midpoint (d₁ + d₂)/2 = p·2^(e0 − 1), |p| < 2^55: each shift is below 128.
            let shift = |m: i64, e: i32| -> Option<i128> {
                i128::from(m).checked_shl(u32::try_from(e.checked_sub(e0)?).ok()?)
            };
            let p = shift(m1, e1)?.checked_add(shift(m2, e2)?)?;
            if self.abs_minus(negative, p, e0.checked_sub(1)?)? == Sign::Positive {
                a = next;
            } else {
                b = mid;
            }
        }
        let magnitude = f64::from_bits(a);
        Some(if negative { -magnitude } else { magnitude })
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic and casts; the rules are for kernel code"
)]
mod tests {
    use super::*;
    use crate::frame::{CrsId, Rotation, Scale};
    use geom_num::grid::GridStep;

    fn dd(hi: f64, lo: f64) -> DD {
        DD::from_sum(hi, lo).unwrap()
    }

    /// The exact sign of X − t/2 for an exact value X = x2/2.
    fn exactly(x2: i128) -> impl Fn(i128) -> Option<Sign> {
        move |t| Some(Sign::of_i128(x2 - t))
    }

    /// The settled rounding of a double-double value with the given exact signs.
    fn run(d: DD, sign: impl Fn(i128) -> Option<Sign>) -> Result<Rounded<i128>, NumError> {
        let r = d.round_shifted(0).unwrap();
        settle(Rounded::new(r.value(), r.is_exact()), d, sign)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the same Result the functions under test return, for assert_eq"
    )]
    fn inexact(value: i128) -> Result<Rounded<i128>, NumError> {
        Ok(Rounded::new(value, false))
    }

    #[test]
    fn settle_trusts_exact_signs_over_the_double_double() {
        let tiny = pow2(-50).unwrap();
        // The exact value is the midpoint 5.5; the double-double value fell just below it.
        let d = dd(5.5, -tiny);
        assert_eq!(d.round_shifted(0).unwrap().value(), 5);
        assert_eq!(run(d, exactly(11)), inexact(6));
        // The exact midpoint 6.5 approached from above goes to the even neighbour too.
        assert_eq!(run(dd(6.5, tiny), exactly(13)), inexact(6));
        // The exact value is a whole step although the double-double value is not.
        assert_eq!(
            run(dd(5.0, pow2(-45).unwrap()), exactly(10)),
            Ok(Rounded::new(5, true))
        );
        // Just above the midpoint 6.5, with the double-double value just below it: up.
        let above = |t: i128| {
            Some(if t <= 13 {
                Sign::Positive
            } else {
                Sign::Negative
            })
        };
        assert_eq!(run(dd(6.5, -tiny), above), inexact(7));
        // Just below the midpoint 6.5, with the double-double value just above it: down.
        let below = |t: i128| {
            Some(if t >= 13 {
                Sign::Negative
            } else {
                Sign::Positive
            })
        };
        assert_eq!(run(dd(6.5, tiny), below), inexact(6));
        // Far from whole steps and midpoints no exact sign is needed.
        let never = |_: i128| -> Option<Sign> { panic!("no exact sign is needed here") };
        assert_eq!(run(dd(5.3, 0.0), never), inexact(5));
        // A failed exact sign is a defect: reported, never replaced by the candidate.
        assert_eq!(run(dd(5.0, 0.0), |_| None), Err(NumError::BudgetExceeded));
    }

    #[test]
    fn the_exact_path_agrees_with_the_fast_path() {
        // A Map frame and a quarter-turned one in metres: the exact path must give the fast
        // path's bits, ties and exactness included.
        let values = [
            0.0,
            1.0 / 4096.0,
            1.0 / 12_800_000.0,
            3.0 / 12_800_000.0,
            5_921_000.123_456_7,
            -171_798.691_84,
            f64::from_bits(1),
            1.0e-300,
        ];
        for turns in 0..4 {
            let f = FrameRecord::new(
                CrsId::Epsg(2193),
                GridStep::Q0,
                [17, -3, 5],
                Rotation::quarter_turns(turns),
                Scale::ONE,
            )
            .unwrap();
            for &e in &values {
                for &n in &values {
                    let fast = f.to_frame(MapPoint {
                        easting: e,
                        northing: n,
                        height: n,
                    });
                    // The height first, as `to_frame` reports it.
                    let exact = f.height_exact(Value::of_f64(n).unwrap()).and_then(|z| {
                        f.horizontal_exact(Value::of_f64(e).unwrap(), Value::of_f64(n).unwrap())
                            .map(|(x, y)| point(x, y, z))
                    });
                    assert_eq!(fast, exact, "turns {turns}, e {e}, n {n}");
                }
            }
        }
    }

    #[test]
    fn far_coordinates_saturate_at_the_ends_of_i128() {
        // X = p/q with q = 1: the bisection and both saturations, exactly at their edges.
        let axis = |p: Big| Axis {
            p,
            q: Big::from_i128(2),
            root: None,
        };
        let max = Big::from_i128(i128::MAX);
        // X = MAX − 1/2 exactly (p = 2·MAX − 1, q = 2): a tie, to the even MAX − 1.
        let tie = max
            .checked_add(&max)
            .unwrap()
            .checked_sub(&Big::one())
            .unwrap();
        let r = axis(tie).round().unwrap();
        assert_eq!((r.value(), r.is_exact()), (i128::MAX - 1, false));
        // Just above it: saturated.
        let over = max.checked_add(&max).unwrap();
        assert_eq!(axis(over).round().unwrap().value(), i128::MAX);
        // X = MIN + 1/2 rounds to the even MIN, the saturated value.
        let min = Big::from_i128(i128::MIN);
        let low = min
            .checked_add(&min)
            .unwrap()
            .checked_add(&Big::one())
            .unwrap();
        assert_eq!(axis(low).round().unwrap().value(), i128::MIN);
        // A whole value far out of range is exact.
        let whole = axis(Big::from_i128(1 << 100)).round().unwrap();
        assert_eq!((whole.value(), whole.is_exact()), (1 << 99, true));
    }
}
