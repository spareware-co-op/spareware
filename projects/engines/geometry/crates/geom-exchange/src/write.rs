//! Grid values written as decimals in a file's unit: exactly where the value terminates, and
//! otherwise the shortest decimal that reads back to the same grid count, with the difference.
//!
//! What it does: a count of n steps is n/(u·S) units exactly (a unit of u = p/q metres, S steps per
//! metre). [`exact`] writes it when that rational is a terminating decimal of at most 38 digits:
//! every metric value is (one q0 step is 0.00015625 mm). [`shortest`] writes the decimal with the
//! fewest significant digits that the grid conversion ([`crate::grid::length`]) reads back to n,
//! and among those the closest to n/(u·S): in inches or feet a grid value generally does not
//! terminate, and this is what chapter 24.10 writes. [`length`] is the export rule: exact when it
//! can be, shortest otherwise. Each says what it wrote, whether it was exact, and its exact
//! difference from the grid value in steps ([`Written::deviation`]), which is under half a step.
//!
//! **The search.** The decimals that read back to n are those in [L, H] with L = (n − ½)/(u·S)
//! and H = (n + ½)/(u·S), an end included exactly when n is even (the conversion rounds a tie to
//! the even count). Writing b = 2·p·S, the three values L, n/(u·S) and H are (2n − 1)·q/b, 2n·q/b
//! and (2n + 1)·q/b. Their whole parts and remainders by b come from one division each, and their
//! decimal digits from long division, one digit per place: the remainder stays below b < 2^97, so
//! ten times it fits 128 bits. Places are tried from the coarsest power of ten down; at the first
//! one where [L, H] holds a multiple, it holds at most nine (ten would include a multiple of the
//! next coarser power), and the one nearest n/(u·S) is written, a tie going to the even last
//! digit. The interval is a whole step wide in steps, so a place 10^j with 10^j·(H − L) ≥ 1 always
//! holds one: the search ends within the digits of 2·u·S + 2 below the units place.
//!
//! Bit budgets: |n| ≤ 2^60 (a frame origin at most); p, q < 2^64 and S < 2^32; so the numerators
//! (2n ± 1)·q < 2^126 and b = 2·p·S < 2^98 fit `u128`. At a coarse place a candidate times its
//! place times b is at most H·b < 2^126, so its difference from the middle fits `i128`. At a
//! fractional place 10^−j the interval is narrower than ten place units (a wider one holds a
//! multiple of ten, found one place coarser), so a candidate is within ten of the middle, its
//! difference times b is below 11·2^98 < 2^102, the whole parts stay below (2n + 1)·50 < 2^68, and
//! 2·q·10^j < 20·p·S < 2^101 when the search ends. Every step is a checked method; a failed one
//! would be a defect, reported as [`ExchangeError::Numeric`], never a wrong digit.
//!
//! Guarantees:
//!
//! - **Reads back.** [`crate::grid::length`] of what [`shortest`] or [`length`] writes, in the same
//!   unit and step, is n, exactly.
//! - **Shortest, then closest.** No decimal with a coarser last place reads back to n; among those
//!   with the same last place, the one written is the closest to n/(u·S), ties to the even digit.
//! - **Exact when it says so.** [`exact`] writes n/(u·S) itself, or nothing; [`Written::is_exact`]
//!   is true exactly when the deviation is zero.
//! - **The difference is exact.** [`Written::deviation`] is the written decimal's value in steps
//!   less n, as a reduced fraction, of magnitude at most one half.
//!
//! Method: the shortest decimal in a rounding interval, by digit generation of its ends (G. L.
//! Steele Jr. and J. L. White, "How to print floating-point numbers accurately", PLDI 1990,
//! <https://doi.org/10.1145/93542.93559>, free-format printing), here on the exact rational interval
//! of a grid count rather than of a double; long division one digit at a time (D. E. Knuth, *The
//! Art of Computer Programming*, vol. 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>); chapter 24.10's "Commit numbers"
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`).
//!
//! Cost: a few 128-bit divisions, then one step of long division per decimal place, at most 40
//! places; a greatest common divisor for the deviation. No allocation.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_math::{Decimal, LinearUnit};
use geom_num::NumError;
use geom_num::grid::GridStep;

use geom_math::frame::{ORIGIN_MAX, steps_per_metre};

use crate::error::ExchangeError;
use crate::unit::gcd;

/// The most decimal places below the units place the search tries; it always ends sooner (see the
/// module documentation).
const PLACES: u32 = 40;

/// A defect: a step the bit budgets rule out failed.
fn defect() -> ExchangeError {
    ExchangeError::Numeric(NumError::BudgetExceeded)
}

/// A difference in grid steps, `num / den` exactly, reduced, with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Deviation {
    num: i128,
    den: u128,
}

impl Deviation {
    /// No difference.
    pub const ZERO: Deviation = Deviation { num: 0, den: 1 };

    /// The fraction `num / den`, reduced.
    fn new(num: i128, den: u128) -> Result<Deviation, ExchangeError> {
        if den == 0 {
            return Err(defect());
        }
        let g = gcd(num.unsigned_abs(), den);
        let g_signed = i128::try_from(g).map_err(|_| defect())?;
        Ok(Deviation {
            num: num.checked_div(g_signed).ok_or_else(defect)?,
            den: den.checked_div(g).ok_or_else(defect)?,
        })
    }

    /// The numerator.
    #[must_use]
    pub const fn num(self) -> i128 {
        self.num
    }

    /// The denominator, positive.
    #[must_use]
    pub const fn den(self) -> u128 {
        self.den
    }

    /// Whether there is no difference.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }
}

/// A grid value written as a decimal, with whether it is exact and its difference in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Written {
    decimal: Decimal,
    deviation: Deviation,
}

impl Written {
    /// The decimal written, in the unit asked for.
    #[must_use]
    pub const fn decimal(self) -> Decimal {
        self.decimal
    }

    /// Whether the decimal is the grid value exactly.
    #[must_use]
    pub const fn is_exact(self) -> bool {
        self.deviation.is_zero()
    }

    /// The decimal's value in steps less the grid count: at most one half in magnitude.
    #[must_use]
    pub const fn deviation(self) -> Deviation {
        self.deviation
    }
}

/// The parts of a count's value: p and q of the unit, S, the count's magnitude and sign.
struct Parts {
    p: u128,
    q: u128,
    s: u128,
    n: u128,
    negative: bool,
}

fn parts(steps: i64, unit: LinearUnit, step: GridStep) -> Result<Parts, ExchangeError> {
    if steps.unsigned_abs() > ORIGIN_MAX.unsigned_abs() {
        return Err(ExchangeError::OutOfRange {
            count: i128::from(steps),
        });
    }
    Ok(Parts {
        p: u128::from(unit.num()),
        q: u128::from(unit.den()),
        s: u128::from(steps_per_metre(step)),
        n: u128::from(steps.unsigned_abs()),
        negative: steps < 0,
    })
}

/// A decimal of magnitude `digits`·10^`exponent` with the count's sign.
fn signed(digits: u128, exponent: i32, negative: bool) -> Result<Decimal, ExchangeError> {
    let m = i128::try_from(digits).map_err(|_| defect())?;
    let m = if negative {
        m.checked_neg().ok_or_else(defect)?
    } else {
        m
    };
    Ok(Decimal::new(m, exponent)?)
}

/// The count `steps` in `unit` as its exact decimal, when n/(u·S) terminates within 38 significant
/// digits; `None` otherwise.
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] for a count beyond ±2^60.
pub fn exact(
    steps: i64,
    unit: LinearUnit,
    step: GridStep,
) -> Result<Option<Written>, ExchangeError> {
    let Parts {
        p,
        q,
        s,
        n,
        negative,
    } = parts(steps, unit, step)?;
    // n·q < 2^124 and p·S < 2^96.
    let num = n.checked_mul(q).ok_or_else(defect)?;
    let den = p.checked_mul(s).ok_or_else(defect)?;
    let g = gcd(num, den);
    let (mut num, mut den) = (
        num.checked_div(g).ok_or_else(defect)?,
        den.checked_div(g).ok_or_else(defect)?,
    );
    // A reduced fraction terminates exactly when its denominator is 2^a·5^b; then num/den =
    // num·2^(k−a)·5^(k−b)/10^k with k = max(a, b).
    let (mut twos, mut fives) = (0_u32, 0_u32);
    while den.is_multiple_of(2) {
        den /= 2;
        twos = twos.checked_add(1).ok_or_else(defect)?;
    }
    while den.is_multiple_of(5) {
        den /= 5;
        fives = fives.checked_add(1).ok_or_else(defect)?;
    }
    if den != 1 {
        return Ok(None);
    }
    let k = twos.max(fives);
    let scale = 2_u128
        .checked_pow(k.checked_sub(twos).ok_or_else(defect)?)
        .zip(5_u128.checked_pow(k.checked_sub(fives).ok_or_else(defect)?))
        .and_then(|(a, b)| a.checked_mul(b));
    let Some(digits) = scale.and_then(|f| num.checked_mul(f)) else {
        return Ok(None);
    };
    num = digits;
    let exponent = i32::try_from(k).map_err(|_| defect())?;
    match signed(num, exponent.checked_neg().ok_or_else(defect)?, negative) {
        Ok(decimal) => Ok(Some(Written {
            decimal,
            deviation: Deviation::ZERO,
        })),
        Err(ExchangeError::Decimal(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// One end or the middle of the interval, as its whole part and remainder over b, advanced one
/// decimal place at a time.
#[derive(Clone, Copy)]
struct Digits {
    whole: u128,
    rest: u128,
}

impl Digits {
    fn of(num: u128, b: u128) -> Result<Digits, ExchangeError> {
        Ok(Digits {
            whole: num.checked_div(b).ok_or_else(defect)?,
            rest: num.checked_rem(b).ok_or_else(defect)?,
        })
    }

    /// The same value times ten.
    fn next(self, b: u128) -> Result<Digits, ExchangeError> {
        // rest < b < 2^97, so ten times it fits.
        let ten_rest = self.rest.checked_mul(10).ok_or_else(defect)?;
        Ok(Digits {
            whole: self
                .whole
                .checked_mul(10)
                .and_then(|w| w.checked_add(ten_rest.checked_div(b)?))
                .ok_or_else(defect)?,
            rest: ten_rest.checked_rem(b).ok_or_else(defect)?,
        })
    }
}

/// The candidates at one place: the smallest and largest multiples of `unit` (as counts of that
/// unit) inside the interval [lo, hi] (ends `closed` or open), if any. `lo` and `hi` are scaled to
/// the place already for fractional places (`unit` 1), or are whole parts for coarser places.
fn inside(lo: Digits, hi: Digits, unit: u128, closed: bool) -> Option<(u128, u128)> {
    // Smallest multiple of `unit` at or past lo (past it when lo is that multiple and open).
    let lo_on = lo.rest == 0 && lo.whole.is_multiple_of(unit);
    let first = if lo_on && closed {
        lo.whole.checked_div(unit)?
    } else {
        lo.whole.checked_div(unit)?.checked_add(1)?
    };
    // Largest multiple at or before hi (before it when hi is that multiple and open).
    let hi_on = hi.rest == 0 && hi.whole.is_multiple_of(unit);
    let last = if hi_on && !closed {
        hi.whole.checked_div(unit)?.checked_sub(1)?
    } else {
        hi.whole.checked_div(unit)?
    };
    (first <= last).then_some((first, last))
}

/// The candidate in `first..=last` (counts of the place) whose `distance` from the middle is least
/// in magnitude, a tie to the even one.
fn nearest_candidate(
    first: u128,
    last: u128,
    distance: impl Fn(u128) -> Result<i128, ExchangeError>,
) -> Result<u128, ExchangeError> {
    let mut best = first;
    let mut best_d = distance(first)?.unsigned_abs();
    let mut c = first;
    while c < last {
        c = c.checked_add(1).ok_or_else(defect)?;
        let d = distance(c)?.unsigned_abs();
        if d < best_d || (d == best_d && c.is_multiple_of(2)) {
            best = c;
            best_d = d;
        }
    }
    Ok(best)
}

/// The interval of decimals that read back to a count, at the units place: its ends and middle
/// over b, and what turns a candidate's offset into steps.
struct Search {
    b: u128,
    a_mid: u128,
    two_q: u128,
    closed: bool,
    negative: bool,
    lo: Digits,
    mid: Digits,
    hi: Digits,
}

impl Search {
    /// The written decimal `c`·10^`exponent`, with its offset from the middle `distance` (in place
    /// units times b) turned into steps by dividing by `per`.
    fn written(
        &self,
        c: u128,
        exponent: i32,
        distance: i128,
        per: u128,
    ) -> Result<Written, ExchangeError> {
        let decimal = signed(c, exponent, self.negative)?;
        let mut off = Deviation::new(distance, per)?;
        if self.negative {
            off.num = off.num.checked_neg().ok_or_else(defect)?;
        }
        Ok(Written {
            decimal,
            deviation: off,
        })
    }

    /// The places 10^k, k ≥ 0, from the digits of the upper end's whole part down to the units.
    fn coarse(&self) -> Result<Option<Written>, ExchangeError> {
        let mut k = 0_u32;
        let mut power = 1_u128;
        while let Some(next) = power.checked_mul(10) {
            if next > self.hi.whole {
                break;
            }
            power = next;
            k = k.checked_add(1).ok_or_else(defect)?;
        }
        loop {
            if let Some((first, last)) = inside(self.lo, self.hi, power, self.closed) {
                // In units times b: c·10^k·b − 2n·q, below 2^127 in magnitude.
                let distance = |c: u128| -> Result<i128, ExchangeError> {
                    let at = c
                        .checked_mul(power)
                        .and_then(|v| v.checked_mul(self.b))
                        .and_then(|v| i128::try_from(v).ok())
                        .ok_or_else(defect)?;
                    let middle = i128::try_from(self.a_mid).map_err(|_| defect())?;
                    at.checked_sub(middle).ok_or_else(defect)
                };
                let c = nearest_candidate(first, last, distance)?;
                let exponent = i32::try_from(k).map_err(|_| defect())?;
                // (c·10^k·b − 2n·q)/b units, times p·S/q = b/(2·q) steps per unit.
                return Ok(Some(self.written(c, exponent, distance(c)?, self.two_q)?));
            }
            if k == 0 {
                return Ok(None);
            }
            k = k.checked_sub(1).ok_or_else(defect)?;
            power = power.checked_div(10).ok_or_else(defect)?;
        }
    }

    /// The places 10^−j, j ≥ 1, one more digit of each end and the middle at a time.
    fn fine(&self) -> Result<Written, ExchangeError> {
        let (mut lo, mut mid, mut hi) = (self.lo, self.mid, self.hi);
        let mut per = self.two_q;
        for j in 1..=PLACES {
            lo = lo.next(self.b)?;
            mid = mid.next(self.b)?;
            hi = hi.next(self.b)?;
            per = per.checked_mul(10).ok_or_else(defect)?;
            if let Some((first, last)) = inside(lo, hi, 1, self.closed) {
                // In place units times b: (c − mid.whole)·b − mid.rest, with |c − mid| < 10.
                let distance = |c: u128| -> Result<i128, ExchangeError> {
                    let c = i128::try_from(c).map_err(|_| defect())?;
                    let w = i128::try_from(mid.whole).map_err(|_| defect())?;
                    let bb = i128::try_from(self.b).map_err(|_| defect())?;
                    let r = i128::try_from(mid.rest).map_err(|_| defect())?;
                    c.checked_sub(w)
                        .and_then(|d| d.checked_mul(bb))
                        .and_then(|d| d.checked_sub(r))
                        .ok_or_else(defect)
                };
                let c = nearest_candidate(first, last, distance)?;
                let exponent = i32::try_from(j)
                    .ok()
                    .and_then(i32::checked_neg)
                    .ok_or_else(defect)?;
                // (distance/b) place units, times b/(2·q·10^j) steps per place unit.
                return self.written(c, exponent, distance(c)?, per);
            }
        }
        Err(defect())
    }
}

/// The count `steps` in `unit` as the shortest decimal that the grid conversion reads back to it,
/// the closest to its exact value among the shortest.
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] for a count beyond ±2^60; [`ExchangeError::Numeric`] only for a
/// defect the bit budgets rule out.
pub fn shortest(steps: i64, unit: LinearUnit, step: GridStep) -> Result<Written, ExchangeError> {
    let Parts {
        p,
        q,
        s,
        n,
        negative,
    } = parts(steps, unit, step)?;
    if n == 0 {
        return Ok(Written {
            decimal: Decimal::ZERO,
            deviation: Deviation::ZERO,
        });
    }
    let b = p
        .checked_mul(s)
        .and_then(|ps| ps.checked_mul(2))
        .ok_or_else(defect)?;
    let two_n = n.checked_mul(2).ok_or_else(defect)?;
    let a_lo = two_n
        .checked_sub(1)
        .and_then(|v| v.checked_mul(q))
        .ok_or_else(defect)?;
    let a_mid = two_n.checked_mul(q).ok_or_else(defect)?;
    let a_hi = two_n
        .checked_add(1)
        .and_then(|v| v.checked_mul(q))
        .ok_or_else(defect)?;
    let search = Search {
        b,
        a_mid,
        // A decimal d units is d·p·S/q steps, so an offset of x/b units is x/(2·q) steps.
        two_q: q.checked_mul(2).ok_or_else(defect)?,
        // A tie reads back to the even count, so the ends belong to n exactly when n is even.
        closed: n.is_multiple_of(2),
        negative,
        lo: Digits::of(a_lo, b)?,
        mid: Digits::of(a_mid, b)?,
        hi: Digits::of(a_hi, b)?,
    };
    match search.coarse()? {
        Some(w) => Ok(w),
        None => search.fine(),
    }
}

/// The export rule of chapter 24.10: the exact decimal of the count when it terminates within 38
/// digits (every metric value), and otherwise the shortest decimal that reads back to it.
///
/// # Errors
///
/// As [`shortest`].
pub fn length(steps: i64, unit: LinearUnit, step: GridStep) -> Result<Written, ExchangeError> {
    match exact(steps, unit, step)? {
        Some(w) => Ok(w),
        None => shortest(steps, unit, step),
    }
}
