//! The result of every exact predicate: negative, zero or positive.
//!
//! Guarantees: predicates return a sign, never a number, so no caller can compare a rounded value
//! against a threshold by accident. `Zero` means "exactly on it", decided exactly.
//!
//! Method: the sign of an exactly computed quantity, as chapter 6.2's exact predicates return it
//! (`projects/engines/geometry/architecture/06-numbers.md`): a comparison with zero, and the sign
//! rules of negation and multiplication of the reals.
//!
//! Cost: constant: every function is one comparison or one `match`.

/// The sign of an exactly computed quantity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sign {
    /// Strictly less than zero.
    Negative,
    /// Exactly zero.
    Zero,
    /// Strictly greater than zero.
    Positive,
}

impl Sign {
    /// The sign of an `i128`.
    #[must_use]
    pub const fn of_i128(v: i128) -> Sign {
        if v < 0 {
            Sign::Negative
        } else if v > 0 {
            Sign::Positive
        } else {
            Sign::Zero
        }
    }

    /// The sign of an `i64`.
    #[must_use]
    pub const fn of_i64(v: i64) -> Sign {
        Sign::of_i128(v as i128)
    }

    /// The opposite sign; `Zero` stays `Zero`.
    #[must_use]
    pub const fn negate(self) -> Sign {
        match self {
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::Positive => Sign::Negative,
        }
    }

    /// The sign of a product of two quantities with these signs.
    #[must_use]
    pub const fn mul(self, other: Sign) -> Sign {
        match (self, other) {
            (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
            (Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,
            _ => Sign::Negative,
        }
    }

    /// Whether the quantity is exactly zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        matches!(self, Sign::Zero)
    }

    /// As -1, 0 or 1.
    #[must_use]
    pub const fn as_i8(self) -> i8 {
        match self {
            Sign::Negative => -1,
            Sign::Zero => 0,
            Sign::Positive => 1,
        }
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
    use super::Sign;

    #[test]
    fn of_i128_and_products() {
        assert_eq!(Sign::of_i128(-5), Sign::Negative);
        assert_eq!(Sign::of_i128(0), Sign::Zero);
        assert_eq!(Sign::of_i128(i128::MAX), Sign::Positive);
        assert_eq!(Sign::of_i128(i128::MIN), Sign::Negative);
        let all = [Sign::Negative, Sign::Zero, Sign::Positive];
        for a in all {
            assert_eq!(a.negate().negate(), a);
            for b in all {
                assert_eq!(a.mul(b).as_i8(), a.as_i8() * b.as_i8());
            }
        }
    }
}

/// Bounded proofs (chapter 28.7; piece G41).
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;

    /// `of_i128` gives the sign of every `i128`, and `as_i8`, `negate` and `mul` keep the sign
    /// rules of the integers, for every pair.
    #[kani::proof]
    fn signs_follow_the_integers() {
        let (a, b): (i128, i128) = (kani::any(), kani::any());
        let (s, t) = (Sign::of_i128(a), Sign::of_i128(b));
        assert!(i128::from(s.as_i8()) == a.signum());
        assert!(i128::from(s.negate().as_i8()) == -a.signum());
        assert!(i128::from(s.mul(t).as_i8()) == a.signum() * b.signum());
        assert!(s.is_zero() == (a == 0));
    }
}
