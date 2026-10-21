//! Exactness classes (chapter 9): how exact a result is, carried by every body.
//!
//! - `Exact`: decided entirely by exact predicates on exact data.
//! - `Certified`: within a proven bound of the true shape.
//! - `Approximate`: within a measured, not proven, deviation.
//!
//! A result is never more exact than its least exact input, and bounds add up. An operation that
//! would lower the class without being asked returns an error instead (chapter 9); this type only
//! records the class.

use crate::grid::Steps;

/// How exact a result is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exactness {
    /// Decided entirely by exact predicates on exact data.
    Exact,
    /// Within a proven bound of the true shape.
    Certified {
        /// The proven bound, in grid steps.
        bound: Steps,
    },
    /// Within a measured deviation of the intended shape; not proven.
    Approximate {
        /// The measured deviation, in grid steps.
        deviation: Steps,
    },
}

impl Exactness {
    /// The class of a result made from inputs of these two classes: the less exact class, with the
    /// bounds added (saturating). `Exact` is the identity.
    #[must_use]
    pub const fn combine(self, other: Exactness) -> Exactness {
        match (self, other) {
            (Exactness::Exact, x) | (x, Exactness::Exact) => x,
            (Exactness::Certified { bound: a }, Exactness::Certified { bound: b }) => {
                Exactness::Certified {
                    bound: a.saturating_add(b),
                }
            }
            (Exactness::Certified { bound: a }, Exactness::Approximate { deviation: b })
            | (Exactness::Approximate { deviation: b }, Exactness::Certified { bound: a })
            | (Exactness::Approximate { deviation: a }, Exactness::Approximate { deviation: b }) => {
                Exactness::Approximate {
                    deviation: a.saturating_add(b),
                }
            }
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
mod tests {
    use super::*;

    #[test]
    fn combine_takes_the_least_exact_and_adds_bounds() {
        let c = |b| Exactness::Certified { bound: Steps(b) };
        let a = |d| Exactness::Approximate {
            deviation: Steps(d),
        };
        assert_eq!(Exactness::Exact.combine(Exactness::Exact), Exactness::Exact);
        assert_eq!(Exactness::Exact.combine(c(3)), c(3));
        assert_eq!(c(3).combine(c(4)), c(7));
        assert_eq!(c(3).combine(a(4)), a(7));
        assert_eq!(a(4).combine(c(3)), a(7));
        assert_eq!(c(u64::MAX).combine(c(1)), c(u64::MAX));
        for x in [Exactness::Exact, c(1), a(2)] {
            for y in [Exactness::Exact, c(5), a(6)] {
                assert_eq!(x.combine(y), y.combine(x));
                assert_eq!(x.combine(y).rank(), x.rank().max(y.rank()));
            }
        }
    }
}
