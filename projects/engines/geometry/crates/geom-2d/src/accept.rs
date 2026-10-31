//! What a caller accepts: how exact a result must be, stated on the input.
//!
//! What it does: every operation states the classes it can return, and every input states the
//! least exact result the caller takes (chapter 14.3). [`Accept`] is that statement. A result below
//! it is refused with [`PointError::ExactnessNotAccepted`], carrying the class and bound it would
//! reach, so nothing is lowered without being asked (chapter 9). Interactive drafting passes
//! `Certified` with the ladder's own bounds, on record in the operation log.
//!
//! Guarantees:
//!
//! - **Never assumed.** [`Accept::Exact`] admits only an exact result, and a bound is compared by
//!   exact integers in quarter nanometres, whatever grid step each side counts in.
//! - **Ordered.** A class is admitted only by an acceptance of its class or a looser one, and a
//!   bound only by a larger or equal one.
//!
//! Method: chapter 14.3's acceptance (`projects/engines/geometry/architecture/14-2d-drafting.md`)
//! and chapter 9's rule that an operation never lowers the class unasked
//! (`projects/engines/geometry/architecture/09-representations.md`).
//!
//! Cost: constant and no allocation.

use geom_num::grid::{GridStep, Steps};
use geom_num::ladder::quarter_nm;
use geom_num::real::Real;
use geom_num::Exactness;

use crate::point::PointError;

/// The least exact result a caller takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accept {
    /// Exact results only.
    Exact,
    /// Exact, or certified within these bounds.
    Certified {
        /// The largest position bound.
        pos: Steps,
        /// The grid step `pos` counts.
        step: GridStep,
        /// The largest direction gap at a tangent join, in radians.
        gap: Real,
    },
    /// Anything measured within this deviation.
    Approximate {
        /// The largest deviation.
        pos: Steps,
        /// The grid step `pos` counts.
        step: GridStep,
    },
}

impl Eq for Accept {}

/// Whether `steps` of `step` is at most `limit` of `of`, compared exactly.
fn within(steps: Steps, step: GridStep, limit: Steps, of: GridStep) -> bool {
    quarter_nm(steps, step)
        .zip(quarter_nm(limit, of))
        .is_some_and(|(a, b)| a <= b)
}

impl Accept {
    /// Admits a result of class `reached` with direction gap `gap`, or refuses it.
    ///
    /// # Errors
    ///
    /// [`PointError::ExactnessNotAccepted`], with what the result would reach and what was
    /// accepted.
    pub fn admit(&self, reached: Exactness, gap: Real) -> Result<Exactness, PointError> {
        let admitted = match (*self, reached) {
            (_, Exactness::Exact) => true,
            (
                Accept::Certified { pos, step, gap: most },
                Exactness::Certified { bound, step: of },
            ) => within(bound, of, pos, step) && gap.get() <= most.get(),
            (
                Accept::Approximate { pos, step },
                Exactness::Certified { bound: d, step: of }
                | Exactness::Approximate { deviation: d, step: of },
            ) => within(d, of, pos, step),
            _ => false,
        };
        if admitted {
            Ok(reached)
        } else {
            Err(PointError::ExactnessNotAccepted {
                would_reach: reached,
                accepted: *self,
            })
        }
    }
}
