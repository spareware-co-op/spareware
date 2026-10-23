//! Exactness classes (chapter 9): how exact a result is, carried by every body.
//!
//! - `Exact`: decided entirely by exact predicates on exact data.
//! - `Certified`: within a proven bound of the true shape.
//! - `Approximate`: within a measured, not proven, deviation.
//!
//! A result is never more exact than its least exact input, and bounds add up. An operation that
//! would lower the class without being asked returns an error instead (chapter 9); this type only
//! records the class.
//!
//! A bound is a count of grid steps together with its step, because a count alone means nothing:
//! 64 steps of `Q0` are 10 µm, 64 steps of `Q1` are 0.4 µm. Combining two bounds first converts
//! both to the finer of their steps (25 per rung, exactly; chapter 6.1), then adds the counts, so
//! a body certified to 64 steps of `Q0` with one certified to 64 steps of `Q1` is certified to
//! 64 · 25 + 64 = 1,664 steps of `Q1`, never to 128 of anything. Every step of that is checked:
//! a sum that does not fit is `None`, never saturated, because a saturated sum is smaller than the
//! true one and so would no longer be a bound.
//!
//! Chapter 9 also caps a certified bound: ε ≤ `ε_fit` unless the caller asked for more, and chapter
//! 6.5 makes exceeding an operation's ceiling an error that rolls back. So operations combine with
//! [`Exactness::combine_within`] against their ceiling (the frame's [`crate::ladder::FIT`] unless
//! the caller asked for more), which reports [`NumError::ToleranceExceeded`];
//! [`Exactness::combine`] alone records a class without judging it.

use crate::error::NumError;
use crate::grid::{GridStep, Steps};

/// How exact a result is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exactness {
    /// Decided entirely by exact predicates on exact data.
    Exact,
    /// Within a proven bound of the true shape.
    Certified {
        /// The proven bound, in steps of `step`.
        bound: Steps,
        /// The grid step `bound` counts.
        step: GridStep,
    },
    /// Within a measured deviation of the intended shape; not proven.
    Approximate {
        /// The measured deviation, in steps of `step`.
        deviation: Steps,
        /// The grid step `deviation` counts.
        step: GridStep,
    },
}

/// Two counts in the finer of their two steps, with that step: exact conversions (25 per rung),
/// `None` only if a converted count does not fit `u64`.
const fn in_finer_step(
    a: Steps,
    a_step: GridStep,
    b: Steps,
    b_step: GridStep,
) -> Option<(Steps, Steps, GridStep)> {
    let finer = if a_step.rung() >= b_step.rung() {
        a_step
    } else {
        b_step
    };
    match (a.refine(a_step, finer), b.refine(b_step, finer)) {
        (Some(a), Some(b)) => Some((a, b, finer)),
        _ => None,
    }
}

/// A count of `from` steps as a count of `to` steps, exactly, in `u128`, for `to` at least as fine
/// as `from`: the factor is 1, 25 or 625, so the product is below 2^64 · 2^10 = 2^74 and always
/// fits. `None` only if `to` is coarser than `from`.
const fn in_step_u128(count: Steps, from: GridStep, to: GridStep) -> Option<u128> {
    // Widening casts of u32 and u64 into u128: lossless (`u128::from` is not const).
    let (coarse, fine) = (from.quarter_nm() as u128, to.quarter_nm() as u128);
    match (coarse.checked_rem(fine), coarse.checked_div(fine)) {
        (Some(0), Some(factor)) => (count.0 as u128).checked_mul(factor),
        _ => None,
    }
}

/// The sum of two bounds, in the finer of their steps; `None` if it does not fit.
const fn sum(a: Steps, a_step: GridStep, b: Steps, b_step: GridStep) -> Option<(Steps, GridStep)> {
    match in_finer_step(a, a_step, b, b_step) {
        Some((a, b, step)) => match a.checked_add(b) {
            Some(total) => Some((total, step)),
            None => None,
        },
        None => None,
    }
}

impl Exactness {
    /// The class of a result made from inputs of these two classes: the less exact class, with the
    /// bounds added in the finer of their two steps. `Exact` is the identity.
    ///
    /// `None` if the combined bound does not fit a `u64` count of the finer step (more than about
    /// 2.9 · 10^12 m at `Q0`): reported, never saturated. This records a class and does not judge
    /// it; an operation uses [`Exactness::combine_within`].
    #[must_use]
    pub const fn combine(self, other: Exactness) -> Option<Exactness> {
        match (self, other) {
            (Exactness::Exact, x) | (x, Exactness::Exact) => Some(x),
            (
                Exactness::Certified {
                    bound: a,
                    step: a_step,
                },
                Exactness::Certified {
                    bound: b,
                    step: b_step,
                },
            ) => match sum(a, a_step, b, b_step) {
                Some((bound, step)) => Some(Exactness::Certified { bound, step }),
                None => None,
            },
            (
                Exactness::Certified {
                    bound: a,
                    step: a_step,
                }
                | Exactness::Approximate {
                    deviation: a,
                    step: a_step,
                },
                Exactness::Certified {
                    bound: b,
                    step: b_step,
                }
                | Exactness::Approximate {
                    deviation: b,
                    step: b_step,
                },
            ) => match sum(a, a_step, b, b_step) {
                Some((deviation, step)) => Some(Exactness::Approximate { deviation, step }),
                None => None,
            },
        }
    }

    /// The bound or deviation and its step; `None` for `Exact`.
    #[must_use]
    pub const fn bound(self) -> Option<(Steps, GridStep)> {
        match self {
            Exactness::Exact => None,
            Exactness::Certified { bound, step }
            | Exactness::Approximate {
                deviation: bound,
                step,
            } => Some((bound, step)),
        }
    }

    /// The class itself if its bound is at most `ceiling` steps of `step`, compared exactly in the
    /// finer of the two steps; `Exact` always passes.
    ///
    /// The comparison is in `u128`, where neither count can overflow: a `u64` count times the
    /// largest conversion factor, 625, is below 2^74. So a small bound on a fine step is compared
    /// correctly with a large ceiling on a coarse one (one step of `Q2` against 2^60 steps of `Q0`
    /// passes), which a comparison in `u64` would get wrong.
    ///
    /// # Errors
    ///
    /// [`NumError::ToleranceExceeded`] if the bound is above the ceiling.
    pub const fn within(self, ceiling: Steps, step: GridStep) -> Result<Exactness, NumError> {
        let Some((bound, bound_step)) = self.bound() else {
            return Ok(self);
        };
        let finer = if bound_step.rung() >= step.rung() {
            bound_step
        } else {
            step
        };
        // Both conversions are to a step at least as fine as their own, so neither is `None`;
        // were one ever `None`, the ceiling is not shown to hold, and that is reported.
        match (
            in_step_u128(bound, bound_step, finer),
            in_step_u128(ceiling, step, finer),
        ) {
            (Some(bound), Some(ceiling)) if bound <= ceiling => Ok(self),
            _ => Err(NumError::ToleranceExceeded),
        }
    }

    /// [`Exactness::combine`], then [`Exactness::within`] the ceiling: the class of a result an
    /// operation may return, or the error that makes it roll back (chapter 6.5: exceeding a
    /// ceiling is an error, never silent growth).
    ///
    /// # Errors
    ///
    /// [`NumError::ToleranceExceeded`] if the combined bound is above the ceiling or does not fit.
    pub const fn combine_within(
        self,
        other: Exactness,
        ceiling: Steps,
        step: GridStep,
    ) -> Result<Exactness, NumError> {
        match self.combine(other) {
            Some(combined) => combined.within(ceiling, step),
            None => Err(NumError::ToleranceExceeded),
        }
    }

    /// 0 for exact, 1 for certified, 2 for approximate: higher is less exact.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Exactness::Exact => 0,
            Exactness::Certified { .. } => 1,
            Exactness::Approximate { .. } => 2,
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
    use super::*;
    use crate::ladder::FIT;

    fn c(b: u64, step: GridStep) -> Exactness {
        Exactness::Certified {
            bound: Steps(b),
            step,
        }
    }

    fn a(d: u64, step: GridStep) -> Exactness {
        Exactness::Approximate {
            deviation: Steps(d),
            step,
        }
    }

    #[test]
    fn combine_takes_the_least_exact_and_adds_bounds() {
        use GridStep::{Q0, Q1, Q2};
        assert_eq!(
            Exactness::Exact.combine(Exactness::Exact),
            Some(Exactness::Exact)
        );
        assert_eq!(Exactness::Exact.combine(c(3, Q0)), Some(c(3, Q0)));
        assert_eq!(c(3, Q0).combine(c(4, Q0)), Some(c(7, Q0)));
        assert_eq!(c(3, Q1).combine(a(4, Q1)), Some(a(7, Q1)));
        assert_eq!(a(4, Q2).combine(c(3, Q2)), Some(a(7, Q2)));
        for x in [Exactness::Exact, c(1, Q0), a(2, Q1)] {
            for y in [Exactness::Exact, c(5, Q2), a(6, Q0)] {
                assert_eq!(x.combine(y), y.combine(x));
                let rank = x.combine(y).unwrap().rank();
                assert_eq!(rank, x.rank().max(y.rank()));
            }
        }
    }

    /// The review's example: 64 steps of `Q0` (10 µm) with 64 steps of `Q1` (0.4 µm) is
    /// 64 · 25 + 64 = 1,664 steps of `Q1` (10.4 µm), not 128 steps of either.
    #[test]
    fn bounds_on_different_steps_add_in_the_finer_step() {
        use GridStep::{Q0, Q1, Q2};
        assert_eq!(c(64, Q0).combine(c(64, Q1)), Some(c(1_664, Q1)));
        assert_eq!(c(64, Q1).combine(c(64, Q0)), Some(c(1_664, Q1)));
        assert_eq!(c(1, Q0).combine(a(1, Q2)), Some(a(626, Q2)));
        assert_eq!(a(2, Q1).combine(c(1, Q0)), Some(a(27, Q1)));
    }

    /// A sum or a conversion that does not fit is `None`, never a saturated, smaller "bound".
    #[test]
    fn overflow_is_reported_not_saturated() {
        use GridStep::{Q0, Q1};
        assert_eq!(c(u64::MAX, Q0).combine(c(1, Q0)), None);
        assert_eq!(c(u64::MAX / 2, Q0).combine(c(1, Q1)), None);
        assert_eq!(
            c(u64::MAX, Q1).combine(Exactness::Exact),
            Some(c(u64::MAX, Q1))
        );
    }

    /// The ceiling is compared exactly in the finer step: 1,600 steps of `Q1` is the fit bound of a
    /// `Q0` frame, and one more is over it.
    #[test]
    fn ceilings_are_enforced() {
        use GridStep::{Q0, Q1};
        assert_eq!(Exactness::Exact.within(Steps(0), Q0), Ok(Exactness::Exact));
        assert_eq!(c(64, Q0).within(FIT, Q0), Ok(c(64, Q0)));
        assert_eq!(c(65, Q0).within(FIT, Q0), Err(NumError::ToleranceExceeded));
        assert_eq!(c(1_600, Q1).within(FIT, Q0), Ok(c(1_600, Q1)));
        assert_eq!(
            c(1_601, Q1).within(FIT, Q0),
            Err(NumError::ToleranceExceeded)
        );
        assert_eq!(
            a(3, Q0).within(Steps(74), Q1),
            Err(NumError::ToleranceExceeded)
        );
        assert_eq!(a(3, Q0).within(Steps(75), Q1), Ok(a(3, Q0)));
        let combined = c(32, Q0).combine_within(c(32, Q0), FIT, Q0);
        assert_eq!(combined, Ok(c(64, Q0)));
        let over = c(32, Q0).combine_within(c(33, Q0), FIT, Q0);
        assert_eq!(over, Err(NumError::ToleranceExceeded));
        let huge = c(u64::MAX, Q0).combine_within(c(1, Q0), Steps(u64::MAX), Q0);
        assert_eq!(huge, Err(NumError::ToleranceExceeded));
        assert_eq!(
            c(u64::MAX, Q0).within(Steps(1), Q1),
            Err(NumError::ToleranceExceeded)
        );
    }

    /// A ceiling whose count in the finer step does not fit `u64` is still compared exactly: one
    /// step of `Q2` is far inside 2^60 steps of `Q0` (found by G06's review, where the comparison
    /// in `u64` overflowed and reported the bound as exceeding the ceiling).
    #[test]
    fn large_ceilings_on_coarse_steps_are_compared_exactly() {
        use GridStep::{Q0, Q1, Q2};
        assert_eq!(c(1, Q2).within(Steps(1 << 60), Q0), Ok(c(1, Q2)));
        assert_eq!(a(1, Q2).within(Steps(u64::MAX), Q0), Ok(a(1, Q2)));
        assert_eq!(
            c(u64::MAX, Q2).within(Steps(u64::MAX), Q0),
            Ok(c(u64::MAX, Q2))
        );
        assert_eq!(
            c(u64::MAX, Q1).within(Steps(u64::MAX), Q0),
            Ok(c(u64::MAX, Q1))
        );
        // The boundary, both ways round: u64::MAX steps of Q0 are u64::MAX · 625 steps of Q2.
        assert_eq!(
            c(u64::MAX, Q0).within(Steps(u64::MAX), Q0),
            Ok(c(u64::MAX, Q0))
        );
        assert_eq!(c(25, Q1).within(Steps(1), Q0), Ok(c(25, Q1)));
        assert_eq!(
            c(26, Q1).within(Steps(1), Q0),
            Err(NumError::ToleranceExceeded)
        );
        assert_eq!(c(625, Q2).within(Steps(1), Q0), Ok(c(625, Q2)));
        assert_eq!(
            c(626, Q2).within(Steps(1), Q0),
            Err(NumError::ToleranceExceeded)
        );
        // A bound on a coarse step against a ceiling on a finer one.
        assert_eq!(c(1, Q0).within(Steps(625), Q2), Ok(c(1, Q0)));
        assert_eq!(
            c(1, Q0).within(Steps(624), Q2),
            Err(NumError::ToleranceExceeded)
        );
        assert_eq!(
            c(u64::MAX, Q0).within(Steps(u64::MAX), Q2),
            Err(NumError::ToleranceExceeded)
        );
    }
}
