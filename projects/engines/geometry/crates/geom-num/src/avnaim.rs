//! The sign of a 2 × 2 integer determinant in single-precision arithmetic, by Avnaim, Boissonnat,
//! Devillers, Preparata and Yvinec's method: chapter 23.3's reference for the exact paths of
//! orientation (piece G42).
//!
//! What it does: [`sign_of_det2`] finds the sign of x1·y2 − y1·x2 from the four entries without
//! ever forming a product: it compares, subtracts and divides, keeping every value within the
//! entries' own magnitude, and [`orient2d`] applies it to the three points' translations, as the
//! generated `orient2d` does. A product is never computed, so no value needs more bits than an
//! entry: the method answers in 64-bit arithmetic where the exact paths need 128 bits.
//!
//! Guarantees:
//!
//! - **The exact sign.** For every four entries of magnitude at most [`ENTRY_MAX`], the result is
//!   the sign of x1·y2 − y1·x2 (tested against 128-bit products and against the generated
//!   `orient2d` in `tests/attack_g42.rs`, and against big integers by geom-fuzz's `variants`
//!   target). An entry past the bound is refused with `None`, never answered.
//! - **Checked and bounded.** Every step is a checked method, and the loop runs at most
//!   [`ITERATIONS`] times: each pass either answers or replaces the second row by one whose entries
//!   are below half the first row's, so the entries halve at least every two passes.
//!
//! Method: Avnaim, Boissonnat, Devillers, Preparata and Yvinec, "Evaluating signs of determinants
//! using single-precision arithmetic", Algorithmica 17 (1997), section 2,
//! doi:10.1007/BF02522822 (the authors' report: <https://inria.hal.science/inria-00090613>). Signs
//! first: an entry of zero leaves one product, and entries of mixed signs settle the sign or are
//! made positive by negating rows and columns. Then, with the rows ordered so that x2 ≥ x1 and
//! y2 ≥ y1, the Euclidean step x2 = k·x1 + xr, yr = y2 − k·y1 keeps the determinant
//! (|x1 y1; xr yr|), and the halves of the first row settle it or give a smaller second row: (xr, yr)
//! when both lie in the lower halves, (x1 − xr, y1 − yr) when both lie in the upper, which negates
//! the determinant. The paper writes that last determinant without the change of sign; it is
//! −|x1 y1; xr yr|, as expanding both shows, and this module follows the expansion.
//!
//! Cost: at most [`ITERATIONS`] passes of a few comparisons, subtractions and one Euclidean
//! division each; no allocation, no recursion.

use core::cmp::Ordering;

use crate::grid::P2;
use crate::sign::Sign;

/// The largest entry magnitude taken: 2^62 − 1. The method subtracts entries and doubles none,
/// so 2^62 − 1 keeps every difference of two entries, and the negation of each, within `i64`.
pub const ENTRY_MAX: i64 = (1 << 62) - 1;

/// The most passes the loop runs: each pass halves the second row or answers, and the rows swap
/// at most every other pass, so 2 · 63 passes reach a zero entry, which answers.
pub const ITERATIONS: usize = 128;

/// The sign of x · y for nonzero or zero `x` and `y`, without the product.
fn sign_of_product(x: i64, y: i64) -> Sign {
    match (x.cmp(&0), y.cmp(&0)) {
        (Ordering::Equal, _) | (_, Ordering::Equal) => Sign::Zero,
        (a, b) if a == b => Sign::Positive,
        _ => Sign::Negative,
    }
}

/// `s` if `flip` is false, its negation otherwise.
const fn flipped(s: Sign, flip: bool) -> Sign {
    if flip { s.negate() } else { s }
}

/// The sign of the determinant |x1 y1; x2 y2| = x1·y2 − y1·x2, or `None` for an entry of
/// magnitude above [`ENTRY_MAX`].
#[must_use]
pub fn sign_of_det2(x1: i64, y1: i64, x2: i64, y2: i64) -> Option<Sign> {
    if [x1, y1, x2, y2]
        .iter()
        .any(|v| v.unsigned_abs() > ENTRY_MAX.unsigned_abs())
    {
        return None;
    }
    // A zero entry leaves one of the two products.
    if x1 == 0 || y2 == 0 {
        return Some(sign_of_product(y1, x2).negate());
    }
    if y1 == 0 || x2 == 0 {
        return Some(sign_of_product(x1, y2));
    }
    // Make the first column positive by negating rows, each a change of sign.
    let mut flip = false;
    let (mut x1, mut y1, mut x2, mut y2) = (x1, y1, x2, y2);
    if x1 < 0 {
        (x1, y1) = (x1.checked_neg()?, y1.checked_neg()?);
        flip = !flip;
    }
    if x2 < 0 {
        (x2, y2) = (x2.checked_neg()?, y2.checked_neg()?);
        flip = !flip;
    }
    // Mixed signs in the second column settle it: with x1, x2 > 0, D = x1·y2 − y1·x2 has the sign
    // of y2 when y1 and y2 differ in sign.
    match (y1 < 0, y2 < 0) {
        (true, false) => return Some(flipped(Sign::Positive, flip)),
        (false, true) => return Some(flipped(Sign::Negative, flip)),
        (true, true) => {
            (y1, y2) = (y1.checked_neg()?, y2.checked_neg()?);
            flip = !flip;
        }
        (false, false) => {}
    }
    for _ in 0..ITERATIONS {
        // Every entry is positive here, except a second row that has reached zero.
        if x2 == 0 || y2 == 0 {
            return Some(flipped(
                if x2 == 0 && y2 == 0 {
                    Sign::Zero
                } else if x2 == 0 {
                    Sign::Positive
                } else {
                    Sign::Negative
                },
                flip,
            ));
        }
        // Order the rows so that x2 ≥ x1 and y2 ≥ y1, or settle the sign.
        if x2 < x1 && y2 < y1 {
            (x1, y1, x2, y2) = (x2, y2, x1, y1);
            flip = !flip;
        } else if x1 <= x2 && y2 < y1 {
            // x1·y2 < x2·y1.
            return Some(flipped(Sign::Negative, flip));
        } else if x1 > x2 && y2 >= y1 {
            // x1·y2 > x2·y1.
            return Some(flipped(Sign::Positive, flip));
        }
        // x2 = k·x1 + xr, 0 ≤ xr < x1, and yr = y2 − k·y1, without forming k·y1 when it would
        // pass the range: then k·y1 > y2 and yr < 0.
        let k = x2.checked_div(x1)?;
        let xr = x2.checked_rem(x1)?;
        let yr = match k.checked_mul(y1) {
            Some(ky) if ky <= y2 => y2.checked_sub(ky)?,
            // k·y1 > y2, so yr < 0 and D = x1·yr − y1·xr < 0.
            _ => return Some(flipped(Sign::Negative, flip)),
        };
        // D = |x1 y1; xr yr| with x1, y1 > 0 and 0 ≤ xr < x1.
        if yr > y1 {
            // x1·yr > x1·y1 > xr·y1.
            return Some(flipped(Sign::Positive, flip));
        }
        if xr == 0 {
            return Some(flipped(
                if yr == 0 { Sign::Zero } else { Sign::Positive },
                flip,
            ));
        }
        // Where xr and yr lie against the halves of x1 and y1: 2·xr against x1 as xr against
        // x1 − xr, which cannot overflow.
        let cx = xr.cmp(&x1.checked_sub(xr)?);
        let cy = yr.cmp(&y1.checked_sub(yr)?);
        match (cx, cy) {
            // D = x1·yr − y1·xr with xr < x1/2 < … : the halves settle it.
            (Ordering::Less, Ordering::Greater) => return Some(flipped(Sign::Positive, flip)),
            (Ordering::Greater, Ordering::Less) => return Some(flipped(Sign::Negative, flip)),
            // 2·xr = x1: D = x1·(yr − y1/2), the sign of 2·yr − y1.
            (Ordering::Equal, c) => return Some(flipped(sign_of_ordering(c), flip)),
            // 2·yr = y1: D = y1·(x1/2 − xr), the sign of x1 − 2·xr.
            (c, Ordering::Equal) => return Some(flipped(sign_of_ordering(c).negate(), flip)),
            // Both in the lower halves: iterate on (x1, y1), (xr, yr).
            (Ordering::Less, Ordering::Less) => (x2, y2) = (xr, yr),
            // Both in the upper halves: iterate on (x1, y1), (x1 − xr, y1 − yr), which negates
            // the determinant.
            (Ordering::Greater, Ordering::Greater) => {
                (x2, y2) = (x1.checked_sub(xr)?, y1.checked_sub(yr)?);
                flip = !flip;
            }
        }
    }
    // Unreachable within ITERATIONS passes (module header); refused rather than guessed.
    None
}

/// The sign an ordering of a value against zero gives.
const fn sign_of_ordering(o: Ordering) -> Sign {
    match o {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// `orient2d(a, b, c)` by the method: the sign of |b − a; c − a|, positive when a, b, c turn
/// counterclockwise, as `crate::predicate::orient2d`. Grid translations are at most 2^41 − 2 in
/// magnitude, so every entry is in range and the result is always `Some`.
#[must_use]
pub fn orient2d(a: P2, b: P2, c: P2) -> Option<Sign> {
    let x1 = b.x.get().checked_sub(a.x.get())?;
    let y1 = b.y.get().checked_sub(a.y.get())?;
    let x2 = c.x.get().checked_sub(a.x.get())?;
    let y2 = c.y.get().checked_sub(a.y.get())?;
    sign_of_det2(x1, y1, x2, y2)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    reason = "tests: oracles and generators"
)]
mod tests {
    use super::*;

    /// The exact sign by 128-bit products.
    fn exact(x1: i64, y1: i64, x2: i64, y2: i64) -> Sign {
        Sign::of_i128(i128::from(x1) * i128::from(y2) - i128::from(y1) * i128::from(x2))
    }

    /// `SplitMix64` (Steele, Lea and Flood, 2014).
    struct Mix(u64);

    impl Mix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// An entry of a random bit length and sign, or an extreme, zero or a small value.
        fn entry(&mut self) -> i64 {
            let v = match self.next() % 8 {
                0 => ENTRY_MAX,
                1 => 0,
                2 => (self.next() % 5) as i64,
                _ => (self.next() >> (2 + self.next() % 62)) as i64,
            };
            if self.next() % 2 == 0 { v } else { -v }
        }
    }

    #[test]
    fn agrees_with_128_bit_products() {
        let mut mix = Mix(0x0642_a7a1_0000_0001);
        for _ in 0..1_000_000 {
            let [x1, y1, x2, y2] = core::array::from_fn(|_| mix.entry());
            assert_eq!(
                sign_of_det2(x1, y1, x2, y2),
                Some(exact(x1, y1, x2, y2)),
                "{x1} {y1} {x2} {y2}"
            );
        }
    }

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "the rows' entries, as the paper names them"
    )]
    fn agrees_on_near_degenerate_entries() {
        // Rows nearly proportional: (x, y) and (k·x + e, k·y + f) for small e and f.
        let mut mix = Mix(0x0642_a7a1_0000_0002);
        for _ in 0..200_000 {
            let x = (mix.next() >> 34) as i64 + 1;
            let y = (mix.next() >> 34) as i64 + 1;
            let k = (mix.next() % 1_000) as i64 + 1;
            let (e, f) = ((mix.next() % 3) as i64 - 1, (mix.next() % 3) as i64 - 1);
            let (x2, y2) = (k * x + e, k * y + f);
            assert_eq!(sign_of_det2(x, y, x2, y2), Some(exact(x, y, x2, y2)));
            assert_eq!(sign_of_det2(-x, y, x2, -y2), Some(exact(-x, y, x2, -y2)));
        }
    }

    #[test]
    fn every_sign_pattern_and_the_limits() {
        let m = ENTRY_MAX;
        for x1 in [-m, -1, 0, 1, m] {
            for y1 in [-m, -1, 0, 1, m] {
                for x2 in [-m, -1, 0, 1, m] {
                    for y2 in [-m, -1, 0, 1, m] {
                        assert_eq!(sign_of_det2(x1, y1, x2, y2), Some(exact(x1, y1, x2, y2)));
                    }
                }
            }
        }
        assert_eq!(sign_of_det2(m + 1, 1, 1, 1), None);
        assert_eq!(sign_of_det2(1, i64::MIN, 1, 1), None);
    }
}
