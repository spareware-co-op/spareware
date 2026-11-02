//! Which step of the predicate ladder settled a call (chapter 23.2).
//!
//! What it does: a predicate takes the cheapest step that can answer it: identity, a narrow path
//! in `i64`, an axis comparison, the generated filter, then the exact path. A predicate with more
//! than a filter and an exact path to choose from, or a count to give, has a twin ending in
//! `_step`, such as [`crate::predicate::orient2d_step`], that gives the same sign
//! and names the [`Step`] that settled it, which is how chapter 23.2's counts and this ladder's
//! tests see the steps. A twin makes the plain call, then asks each step in turn, so counting
//! costs the plain call nothing.
//!
//! Guarantees:
//!
//! - **The same sign, whatever the step.** Every step returns the exact path's sign or leaves the
//!   call to the next; a step changes speed only. `tests/steps.rs` holds each to a big-integer
//!   oracle, at the edge of the range it takes.
//! - **No shared state.** A count is a tally over what the twins return, kept by whoever asks
//!   (`geom-bench`), so the predicates stay pure functions, with nothing to reset and nothing that
//!   threads or a build profile could change.
//!
//! Method: the predicate step ladder of chapter 23.2
//! (`projects/engines/geometry/architecture/23-performance.md`), which decision 32.22 joins to
//! 6.2; its filter is Meyer and Pion's, as in Geogram's PCK.
//!
//! Cost: none on a plain call; a twin pays for its call and for each step it asks.

/// The step that settled a predicate, in the order of chapter 23.2's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// The data answered with no arithmetic: a vertex on one of its own planes.
    Identity,
    /// A magnitude check kept every intermediate in 63 bits, and `i64` answered exactly.
    Narrow,
    /// The plane is square to a frame axis, and one product answered exactly.
    Axis,
    /// The generated `f64` filter proved the sign.
    Filter,
    /// The fixed-width exact path answered.
    Exact,
}

impl Step {
    /// Every step, in the order of the table.
    pub const ALL: [Step; 5] = [
        Step::Identity,
        Step::Narrow,
        Step::Axis,
        Step::Filter,
        Step::Exact,
    ];

    /// The step that answers a predicate that `filters`, whose filter `filtered` it or not, and
    /// whose narrow path `narrowed` it or not, without which the exact path answers.
    #[must_use]
    pub const fn route(filters: bool, filtered: bool, narrowed: bool) -> Step {
        match (filters, filtered, narrowed) {
            (true, true, _) => Step::Filter,
            (true, false, true) => Step::Narrow,
            _ => Step::Exact,
        }
    }

    /// The step's name in the table of chapter 23.2.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Step::Identity => "identity",
            Step::Narrow => "narrow",
            Step::Axis => "axis",
            Step::Filter => "filter",
            Step::Exact => "exact",
        }
    }
}
