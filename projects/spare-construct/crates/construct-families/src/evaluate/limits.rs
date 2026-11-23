//! The one place limits on evaluating a recipe go (CON-A7-F1-P6, job 5.5): how long one
//! evaluation may work, how much it may hold, and how many evaluated types the cache keeps.
//!
//! This half sets none. Every formula is already bounded by construct-formulas' own caps (its
//! length, nesting, conditionals and number sizes), a sketch is only checked here, never solved,
//! and the engine is asked once per step, so an evaluation's work is bounded by the recipe's size.
//! The cache keeps every type it has built until [`crate::Evaluator::forget`] is called. Job 5.5
//! adds its limits as fields of [`Limits`], read by the evaluator where it starts each step and
//! where it stores a type; no caller changes.

/// Limits on evaluating a recipe. Today there are none; see the module's header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {}

impl Limits {
    /// No limits beyond those every part already keeps.
    #[must_use]
    pub const fn none() -> Self {
        Self {}
    }
}
