//! Exact non-negative fractions for generators, and their exact decimal form.
//!
//! What it does: [`Ratio`] is a non-negative fraction of two `u128`s, always in lowest terms, with
//! checked multiplication and division. [`Ratio::decimal`] writes its exact decimal expansion when
//! it has a finite one, and [`Ratio::fraction`] writes it as `n/d`.
//!
//! Guarantees:
//!
//! - **Exact or nothing.** Every operation is exact; one whose result does not fit in `u128` returns
//!   `None`, never a rounded or wrapped value. A decimal is written only when it terminates, and
//!   then with every digit.
//! - **Canonical.** Lowest terms and a positive denominator, so equal values compare equal and print
//!   the same.
//!
//! Method: Euclid's algorithm for the greatest common divisor (Knuth, *The Art of Computer
//! Programming*, volume 2, section 4.5.2, Algorithm A;
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>), with cross-cancellation before
//! multiplying (Knuth 4.5.1), so intermediate products stay as small as the result allows. A
//! fraction in lowest terms has a finite decimal expansion exactly when its denominator is
//! 2^a × 5^b, and then it has max(a, b) digits after the point (Hardy and Wright, *An Introduction
//! to the Theory of Numbers*, section 9.2); the unit tests check both facts on every denominator
//! that divides 6,400.
//!
//! Cost: O(log n) steps for each greatest common divisor; a decimal is O(number of digits).

use core::fmt;

/// A non-negative fraction in lowest terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ratio {
    num: u128,
    den: u128,
}

impl Ratio {
    /// num / den in lowest terms, or `None` if `den` is zero.
    #[must_use]
    pub fn new(num: u128, den: u128) -> Option<Ratio> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den);
        // g divides both and g ≥ 1, because den ≠ 0.
        Some(Ratio {
            num: num.checked_div(g)?,
            den: den.checked_div(g)?,
        })
    }

    /// The whole number `n`.
    #[must_use]
    pub const fn whole(n: u128) -> Ratio {
        Ratio { num: n, den: 1 }
    }

    /// The numerator, in lowest terms.
    #[must_use]
    pub const fn numerator(self) -> u128 {
        self.num
    }

    /// The denominator, in lowest terms; never zero.
    #[must_use]
    pub const fn denominator(self) -> u128 {
        self.den
    }

    /// Whether it is a whole number.
    #[must_use]
    pub const fn is_whole(self) -> bool {
        self.den == 1
    }

    /// The exact product, or `None` if it does not fit.
    #[must_use]
    pub fn checked_mul(self, other: Ratio) -> Option<Ratio> {
        // Cancel across before multiplying: (a/b)(c/d) = ((a/g1)(c/g2)) / ((b/g2)(d/g1)).
        let g1 = gcd(self.num, other.den);
        let g2 = gcd(other.num, self.den);
        let num = self
            .num
            .checked_div(g1)?
            .checked_mul(other.num.checked_div(g2)?)?;
        let den = self
            .den
            .checked_div(g2)?
            .checked_mul(other.den.checked_div(g1)?)?;
        Ratio::new(num, den)
    }

    /// The exact quotient, or `None` if `other` is zero or the result does not fit.
    #[must_use]
    pub fn checked_div(self, other: Ratio) -> Option<Ratio> {
        self.checked_mul(Ratio::new(other.den, other.num)?)
    }

    /// `n` or `n/d`.
    #[must_use]
    pub fn fraction(self) -> String {
        if self.den == 1 {
            format!("{}", self.num)
        } else {
            format!("{}/{}", self.num, self.den)
        }
    }

    /// The exact decimal expansion, every digit of it, or `None` if it does not terminate (or, for
    /// denominators beyond 10^38, does not fit the arithmetic).
    #[must_use]
    pub fn decimal(self) -> Option<String> {
        // The denominator is never zero, so both loops end. A u128 below 2^128 has at most 127
        // factors of 2 and at most 55 of 5 (5^56 > 2^128), so neither count saturates.
        let (mut d, mut twos, mut fives) = (self.den, 0u32, 0u32);
        while d % 2 == 0 {
            d /= 2;
            twos = twos.saturating_add(1);
        }
        while d % 5 == 0 {
            d /= 5;
            fives = fives.saturating_add(1);
        }
        if d != 1 {
            return None;
        }
        let digits = twos.max(fives);
        let scale = 10u128.checked_pow(digits)?;
        // den divides 10^digits, so this is a whole number.
        let scaled = self.num.checked_mul(scale.checked_div(self.den)?)?;
        let whole = scaled.checked_div(scale)?;
        let part = scaled.checked_rem(scale)?;
        if digits == 0 {
            return Some(format!("{whole}"));
        }
        let width = usize::try_from(digits).ok()?;
        Some(format!("{whole}.{part:0width$}"))
    }

    /// The decimal if it terminates, otherwise the fraction.
    #[must_use]
    pub fn exact(self) -> String {
        self.decimal().unwrap_or_else(|| self.fraction())
    }
}

impl fmt::Display for Ratio {
    /// The same as [`Ratio::exact`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.exact())
    }
}

/// The greatest common divisor, by Euclid's algorithm; gcd(0, 0) = 0.
#[must_use]
pub fn gcd(mut a: u128, mut b: u128) -> u128 {
    while let Some(r) = a.checked_rem(b) {
        a = b;
        b = r;
    }
    a
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

    fn r(n: u128, d: u128) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    #[test]
    fn lowest_terms_and_equality() {
        assert_eq!(r(625, 4_000_000), r(1, 6_400));
        assert_eq!(r(0, 7), Ratio::whole(0));
        assert_eq!(r(12, 4), Ratio::whole(3));
        assert!(Ratio::new(1, 0).is_none());
        assert_eq!(gcd(0, 0), 0);
        assert_eq!(gcd(0, 9), 9);
        assert_eq!(gcd(9, 0), 9);
        assert_eq!(gcd(1_071, 462), 21);
    }

    #[test]
    fn arithmetic_is_exact_and_checked() {
        assert_eq!(r(1, 6_400).checked_mul(r(6_400, 1)), Some(Ratio::whole(1)));
        assert_eq!(
            r(127, 1_280).checked_div(r(1, 6_400)),
            Some(Ratio::whole(635))
        );
        assert_eq!(r(3, 4).checked_div(Ratio::whole(0)), None);
        // Cross-cancellation keeps a product that fits from overflowing on the way.
        let big = Ratio::whole(u128::MAX);
        assert_eq!(big.checked_mul(r(1, u128::MAX)), Some(Ratio::whole(1)));
        assert_eq!(big.checked_mul(Ratio::whole(2)), None);
    }

    #[test]
    fn decimals_are_exact_or_absent() {
        assert_eq!(r(1, 6_400).decimal().as_deref(), Some("0.00015625"));
        assert_eq!(r(625, 4).decimal().as_deref(), Some("156.25"));
        assert_eq!(Ratio::whole(6_400).decimal().as_deref(), Some("6400"));
        assert_eq!(r(1, 2).decimal().as_deref(), Some("0.5"));
        assert_eq!(r(1, 3).decimal(), None);
        assert_eq!(r(1_200_000, 3_937).decimal(), None);
        assert_eq!(r(1_200_000, 3_937).exact(), "1200000/3937");
        assert_eq!(r(7, 1).fraction(), "7");
        assert_eq!(
            r(1, 1 << 34).decimal().as_deref(),
            Some("0.0000000000582076609134674072265625")
        );
        // The last digit of a terminating expansion in lowest terms is never zero.
        for d in [2u128, 4, 5, 8, 10, 16, 20, 25, 40, 64, 125, 6_400] {
            for n in 1..d {
                let s = r(n, d).decimal().unwrap();
                assert!(!s.ends_with('0'), "{n}/{d} = {s}");
            }
        }
    }

    /// The two facts the module header cites, on every denominator that divides 6,400 = 2^8 × 5^2
    /// and every numerator up to it: the decimal terminates, with max(a, b) digits after the point
    /// for a denominator 2^a × 5^b in lowest terms, and reads back as exactly the same fraction.
    /// Denominators with another prime factor (3, 7, 3,937) never terminate.
    #[test]
    fn the_terminating_decimal_facts_hold() {
        let divisors: Vec<u128> = (1..=6_400u128).filter(|d| 6_400 % d == 0).collect();
        assert_eq!(divisors.len(), 27);
        for &d in &divisors {
            for n in 0..=d {
                let x = r(n, d);
                let s = x.decimal().unwrap();
                let (mut den, mut twos, mut fives) = (x.denominator(), 0u32, 0u32);
                while den % 2 == 0 {
                    den /= 2;
                    twos += 1;
                }
                while den % 5 == 0 {
                    den /= 5;
                    fives += 1;
                }
                assert_eq!(den, 1);
                let digits = s.split_once('.').map_or(0, |(_, f)| f.len());
                assert_eq!(digits, twos.max(fives) as usize, "{n}/{d} = {s}");
                let back = s.replace('.', "").parse::<u128>().unwrap();
                assert_eq!(r(back, 10u128.pow(digits as u32)), x, "{n}/{d} = {s}");
            }
        }
        for d in [3u128, 7, 3_937, 6_400 * 3] {
            assert_eq!(r(1, d).decimal(), None, "1/{d}");
        }
    }
}
