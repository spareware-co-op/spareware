//! Exact reals of the one form a railing's points take, and the one rule that puts them on the
//! grid: main's any-angle rule (28 Nov 2026), each point worked out exactly and rounded once to the
//! nearest grid step, a half away from zero.
//!
//! A [`Real`] is `(a + c1 sqrt(r1) + c2 sqrt(r2) + ...) / d` in whole numbers of any size
//! (num-bigint), `d` above zero and every `r` at least nothing. Every point of a railing is one:
//! a point along a segment at any angle divides by the segment's length, `sqrt(dx^2 + dy^2)`; a
//! rail's section on a slope is stretched by `sqrt(run^2 + rise^2) / run`; a round profile's
//! corners carry `sqrt(3) / 2`. So at most two square roots meet in one coordinate, and [`Real::sign`]
//! decides the sign of such a sum exactly by squaring with the signs kept (no float, no
//! tolerance); a sum of three or more different roots is refused as none, and no railing makes one.
//!
//! [`Real::nearest`] is the rounding: the whole number nearest the real, a half away from zero, so
//! a point is within half a step of where it truly is along each axis, and a point already on the
//! grid is given exactly. It starts from an estimate (each root's whole part, which integer square
//! roots give exactly) and settles the answer with exact sign tests, so it is the true nearest, not
//! an approximation of it.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "whole numbers of any size cannot overflow, every divisor here is above zero, and every \
              square root is of a number at least nothing; fixed-width steps elsewhere are checked"
)]

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

/// A real number `(a + sum c sqrt(r)) / d`, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Real {
    a: BigInt,
    terms: Vec<(BigInt, BigInt)>,
    d: BigInt,
}

/// The sign of a whole number, as an ordering against nothing.
fn sign_of(v: &BigInt) -> Ordering {
    v.cmp(&BigInt::zero())
}

/// The sign of `p + q sqrt(r)`, `r` at least nothing, exactly.
fn sign1(p: &BigInt, q: &BigInt, r: &BigInt) -> Ordering {
    let sp = sign_of(p);
    let sq = if r.is_zero() {
        Ordering::Equal
    } else {
        sign_of(q)
    };
    if sq == Ordering::Equal {
        return sp;
    }
    if sp == Ordering::Equal || sp == sq {
        return sq;
    }
    match (p * p).cmp(&(q * q * r)) {
        Ordering::Greater => sp,
        Ordering::Less => sq,
        Ordering::Equal => Ordering::Equal,
    }
}

/// The sign of `c1 sqrt(r1) + c2 sqrt(r2)`, exactly.
fn sign_roots(c1: &BigInt, r1: &BigInt, c2: &BigInt, r2: &BigInt) -> Ordering {
    let s1 = if r1.is_zero() {
        Ordering::Equal
    } else {
        sign_of(c1)
    };
    let s2 = if r2.is_zero() {
        Ordering::Equal
    } else {
        sign_of(c2)
    };
    if s2 == Ordering::Equal {
        return s1;
    }
    if s1 == Ordering::Equal || s1 == s2 {
        return s2;
    }
    match (c1 * c1 * r1).cmp(&(c2 * c2 * r2)) {
        Ordering::Greater => s1,
        Ordering::Less => s2,
        Ordering::Equal => Ordering::Equal,
    }
}

/// The sign of `a + c1 sqrt(r1) + c2 sqrt(r2)`, exactly: the two roots' sum is compared with `a`
/// by squaring it, which leaves one root, `sqrt(r1 r2)`.
fn sign2(a: &BigInt, (c1, r1): (&BigInt, &BigInt), (c2, r2): (&BigInt, &BigInt)) -> Ordering {
    let sx = sign_roots(c1, r1, c2, r2);
    let sa = sign_of(a);
    if sa == Ordering::Equal {
        return sx;
    }
    if sx == Ordering::Equal || sx == sa {
        return sa;
    }
    // |x| against |a|: x^2 - a^2 = c1^2 r1 + c2^2 r2 - a^2 + 2 c1 c2 sqrt(r1 r2).
    let p = c1 * c1 * r1 + c2 * c2 * r2 - a * a;
    let q = c1 * c2 * 2;
    match sign1(&p, &q, &(r1 * r2)) {
        Ordering::Greater => sx,
        Ordering::Less => sa,
        Ordering::Equal => Ordering::Equal,
    }
}

/// The product of whole numbers, exactly.
fn product(factors: &[i128]) -> BigInt {
    factors
        .iter()
        .fold(BigInt::from(1), |p, &f| p * BigInt::from(f))
}

/// `a / d`, each the product of its whole numbers; none over nothing.
pub(crate) fn ratio(a: &[i128], d: &[i128]) -> Option<Real> {
    Real::new(product(a), Vec::new(), product(d))
}

/// `c sqrt(r) / d`, each the product of its whole numbers; none over nothing or for a root of a
/// number below nothing.
pub(crate) fn term(c: &[i128], r: &[i128], d: &[i128]) -> Option<Real> {
    Real::new(BigInt::zero(), vec![(product(c), product(r))], product(d))
}

impl Real {
    /// A whole number of steps.
    pub(crate) fn whole(v: i128) -> Self {
        Self {
            a: BigInt::from(v),
            terms: Vec::new(),
            d: BigInt::from(1),
        }
    }

    /// `(a + sum c sqrt(r)) / d`; none for `d` nothing or a root of a number below nothing.
    pub(crate) fn new(a: BigInt, terms: Vec<(BigInt, BigInt)>, d: BigInt) -> Option<Self> {
        if d.is_zero() || terms.iter().any(|(_, r)| r.is_negative()) {
            return None;
        }
        let real = if d.is_negative() {
            Self {
                a: -a,
                terms: terms.into_iter().map(|(c, r)| (-c, r)).collect(),
                d: -d,
            }
        } else {
            Self { a, terms, d }
        };
        Some(real.merged())
    }

    /// The same real with no term of nothing, every root of a square taken out whole, and equal
    /// roots added together, the roots in order: one value, one spelling.
    fn merged(self) -> Self {
        let mut a = self.a;
        let mut terms: Vec<(BigInt, BigInt)> = Vec::new();
        for (c, r) in self.terms {
            if c.is_zero() || r.is_zero() {
                continue;
            }
            let s = r.sqrt();
            if &s * &s == r {
                a += c * s;
                continue;
            }
            match terms.iter_mut().find(|(_, held)| *held == r) {
                Some((held, _)) => *held += c,
                None => terms.push((c, r)),
            }
        }
        terms.retain(|(c, _)| !c.is_zero());
        terms.sort_by(|x, y| x.1.cmp(&y.1));
        // In lowest terms: no whole number above one divides the whole part, every coefficient
        // and the denominator.
        let g = terms.iter().fold(a.gcd(&self.d), |g, (c, _)| g.gcd(c));
        if g > BigInt::from(1) {
            return Self {
                a: a / &g,
                terms: terms.into_iter().map(|(c, r)| (c / &g, r)).collect(),
                d: self.d / &g,
            };
        }
        Self {
            a,
            terms,
            d: self.d,
        }
    }

    /// `self + other`.
    pub(crate) fn plus(&self, other: &Self) -> Self {
        let mut terms: Vec<(BigInt, BigInt)> = self
            .terms
            .iter()
            .map(|(c, r)| (c * &other.d, r.clone()))
            .collect();
        terms.extend(other.terms.iter().map(|(c, r)| (c * &self.d, r.clone())));
        Self {
            a: &self.a * &other.d + &other.a * &self.d,
            terms,
            d: &self.d * &other.d,
        }
        .merged()
    }

    /// `-self`.
    pub(crate) fn negated(&self) -> Self {
        Self {
            a: -&self.a,
            terms: self.terms.iter().map(|(c, r)| (-c, r.clone())).collect(),
            d: self.d.clone(),
        }
    }

    /// `self - other`.
    pub(crate) fn minus(&self, other: &Self) -> Self {
        self.plus(&other.negated())
    }

    /// Its sign, exactly; none for a sum of more than two different roots.
    pub(crate) fn sign(&self) -> Option<Ordering> {
        match self.terms.as_slice() {
            [] => Some(sign_of(&self.a)),
            [(c, r)] => Some(sign1(&self.a, c, r)),
            [(c1, r1), (c2, r2)] => Some(sign2(&self.a, (c1, r1), (c2, r2))),
            _ => None,
        }
    }

    /// How it compares with `other`, exactly ([`Real::sign`] of the difference).
    pub(crate) fn compare(&self, other: &Self) -> Option<Ordering> {
        self.minus(other).sign()
    }

    /// How it compares with a whole number of steps.
    pub(crate) fn compare_whole(&self, v: i128) -> Option<Ordering> {
        self.compare(&Self::whole(v))
    }

    /// The nearest whole number at least nothing, for a real at least nothing: the largest `n`
    /// with `n - 1/2 <= x`, found from an estimate within two of it by exact tests.
    fn nearest_up(&self) -> Option<BigInt> {
        let mut estimate = self.a.clone();
        for (c, r) in &self.terms {
            let whole = (c * c * r).sqrt();
            if c.is_negative() {
                estimate -= whole;
            } else {
                estimate += whole;
            }
        }
        let mut n = estimate.div_floor(&self.d) + 3;
        for _ in 0..8 {
            // n - 1/2 <= x, that is 2 (a + sum) - (2n - 1) d >= 0.
            let test = Self {
                a: &self.a * 2 - (&n * 2 - 1) * &self.d,
                terms: self.terms.iter().map(|(c, r)| (c * 2, r.clone())).collect(),
                d: BigInt::from(1),
            };
            if test.sign()? != Ordering::Less {
                return Some(n);
            }
            n -= 1;
        }
        None
    }

    /// The whole number nearest it, a half away from zero: the module header's one rounding. None
    /// for a sum of more than two different roots or a number past an `i64`.
    pub(crate) fn nearest(&self) -> Option<i64> {
        let n = if self.sign()? == Ordering::Less {
            -self.negated().nearest_up()?
        } else {
            self.nearest_up()?
        };
        n.to_i64()
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test code; the rules are for the crate's own code"
    )]

    use super::*;

    fn big(v: i128) -> BigInt {
        BigInt::from(v)
    }

    /// `(a + c sqrt(r)) / d`.
    fn one(a: i128, c: i128, r: i128, d: i128) -> Real {
        Real::new(big(a), vec![(big(c), big(r))], big(d)).unwrap()
    }

    #[test]
    fn a_root_is_rounded_to_its_nearest_whole_number() {
        // sqrt(2) = 1.414..., 1000 sqrt(2) = 1414.21..., sqrt(277) x 2,048,000 = 34,085,513.17...
        assert_eq!(one(0, 1, 2, 1).nearest(), Some(1));
        assert_eq!(one(0, 1000, 2, 1).nearest(), Some(1414));
        assert_eq!(one(0, 2_048_000, 277, 1).nearest(), Some(34_085_513));
        // A square's root is whole and given exactly; a half goes away from zero.
        assert_eq!(one(0, 1, 49, 1).nearest(), Some(7));
        assert_eq!(
            Real::new(big(5), vec![], big(2)).unwrap().nearest(),
            Some(3)
        );
        assert_eq!(
            Real::new(big(-5), vec![], big(2)).unwrap().nearest(),
            Some(-3)
        );
        assert_eq!(one(0, -1000, 2, 1).nearest(), Some(-1414));
    }

    #[test]
    fn two_roots_are_compared_and_rounded_exactly() {
        // sqrt(3) - sqrt(2) = 0.3178..., times 1000 is 317.83...; 3 sqrt(2) - 2 sqrt(3) + 1 =
        // 1.7787... rounds to 2; sqrt(8) - 2 sqrt(2) is nothing, by its spelling.
        let x = Real::new(
            big(0),
            vec![(big(1000), big(3)), (big(-1000), big(2))],
            big(1),
        )
        .unwrap();
        assert_eq!(x.nearest(), Some(318));
        let y = Real::new(big(1), vec![(big(3), big(2)), (big(-2), big(3))], big(1)).unwrap();
        assert_eq!(y.nearest(), Some(2));
        assert_eq!(y.sign(), Some(Ordering::Greater));
        // 1 - 3 sqrt(2) + 2 sqrt(3) = 0.2215..., positive; 2 sqrt(2) - sqrt(8): both whole roots
        // differ, the sign test squares them exactly to nothing.
        let z = Real::new(big(1), vec![(big(-3), big(2)), (big(2), big(3))], big(1)).unwrap();
        assert_eq!(z.sign(), Some(Ordering::Greater));
        let nil = Real::new(big(0), vec![(big(2), big(2)), (big(-1), big(8))], big(1)).unwrap();
        assert_eq!(nil.sign(), Some(Ordering::Equal));
        // Three different roots are refused.
        let three = Real::new(
            big(0),
            vec![(big(1), big(2)), (big(1), big(3)), (big(1), big(5))],
            big(1),
        )
        .unwrap();
        assert_eq!(three.sign(), None);
    }

    #[test]
    fn sums_differences_and_comparisons_are_exact() {
        let a = one(1, 1, 2, 2); // (1 + sqrt 2) / 2
        let b = one(3, -1, 2, 4); // (3 - sqrt 2) / 4
        let sum = a.plus(&b); // (5 + sqrt 2) / 4
        assert_eq!(sum, one(5, 1, 2, 4));
        assert_eq!(a.compare(&b), Some(Ordering::Greater));
        assert_eq!(a.minus(&a).sign(), Some(Ordering::Equal));
        assert_eq!(one(0, 1, 2, 1).compare_whole(1), Some(Ordering::Greater));
        assert_eq!(one(0, 2, 2, 1).compare_whole(3), Some(Ordering::Less));
        assert_eq!(Real::new(big(1), vec![], big(0)), None);
    }
}
