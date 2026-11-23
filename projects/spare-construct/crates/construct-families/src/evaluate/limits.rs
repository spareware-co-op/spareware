//! The limits on evaluating a recipe (CON-A7-F1-P6): how much work one evaluation may do and how
//! large a result it may make, so a recipe can never make evaluation run without end or grow
//! without bound.
//!
//! Every limit is a count, never a clock: the same recipe and values reach a limit at the same
//! place on every machine and every run, so a limit decides an answer as deterministically as the
//! rest of evaluation does. Four are kept, all for one evaluation (one type), each counted where
//! the work is about to be done, and refused before it is done:
//!
//! - **Solver steps** ([`Limits::solver_steps`]): construct-solver's own work count (one
//!   evaluation of a sketch's constraints with one factorisation, or one trial move), for all the
//!   sketches of the evaluation together. Each sketch is solved with what is left as its budget.
//! - **Solid steps** ([`Limits::solid_steps`]): the recipe's steps run, each one engine call
//!   (an extrusion, a subtraction or a union).
//! - **Formula work** ([`Limits::formula_bytes`]): the text of every formula worked out, in
//!   bytes: each parameter's formula and value as the formula table reads it, and every length
//!   formula read for a plane, a constraint or an extrusion. construct-formulas already caps one
//!   formula (its length, nesting, conditionals and number sizes); this caps how many.
//! - **Result size** ([`Limits::corners`]): the profile corners handed to the engine, holes
//!   included. A solid is made only from its profiles' corners, swept and combined, so its size
//!   grows with them.
//!
//! Hitting one is [`crate::Reason::LimitReached`], at the place where it was reached, never a
//! panic and never a partial result. The defaults ([`Limits::DEFAULT`]) are far above what the
//! example families use (the single-swing door uses well under a hundredth of each) and below what
//! would hold a server for long. How much an evaluation used is in [`crate::Evaluated::used`].
//! The cache keeps every type it builds until [`crate::Evaluator::forget`]; it holds one entry
//! per type a caller asks for, so a caller bounds it by what it asks.

use super::error::Reason;
use construct_solver::Budget;
use std::fmt;

/// The limits on one evaluation. Its fields can be read and changed; a new value starts from
/// [`Limits::DEFAULT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// The sketch solver's steps, all sketches together.
    pub solver_steps: u32,
    /// The solid steps run (engine calls).
    pub solid_steps: u32,
    /// The formula text worked out, in bytes.
    pub formula_bytes: u64,
    /// The profile corners handed to the engine.
    pub corners: u64,
}

impl Limits {
    /// The defaults: 4,000 solver steps (twenty sketches at the solver's own default budget),
    /// 1,000 solid steps, 256 KiB of formula text and 100,000 profile corners.
    pub const DEFAULT: Self = Self {
        solver_steps: 4_000,
        solid_steps: 1_000,
        formula_bytes: 262_144,
        corners: 100_000,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Which limit was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Limit {
    /// [`Limits::solver_steps`].
    SolverSteps,
    /// [`Limits::solid_steps`].
    SolidSteps,
    /// [`Limits::formula_bytes`].
    FormulaWork,
    /// [`Limits::corners`].
    ResultSize,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SolverSteps => "sketch solver steps",
            Self::SolidSteps => "solid steps",
            Self::FormulaWork => "bytes of formula text",
            Self::ResultSize => "profile corners",
        })
    }
}

/// What one evaluation used, counted as its limits count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Used {
    /// Sketch solver steps.
    pub solver_steps: u32,
    /// Solid steps run.
    pub solid_steps: u32,
    /// Formula text worked out, in bytes.
    pub formula_bytes: u64,
    /// Profile corners handed to the engine.
    pub corners: u64,
}

/// The count of one evaluation against its limits.
pub(crate) struct Meter {
    limits: Limits,
    used: Used,
}

impl Meter {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            limits,
            used: Used::default(),
        }
    }

    pub(crate) fn used(&self) -> Used {
        self.used
    }

    fn reached(limit: Limit, allowed: u64) -> Reason {
        Reason::LimitReached { limit, allowed }
    }

    /// The solver's budget for the next sketch: the steps left.
    pub(crate) fn solver_budget(&self) -> Budget {
        Budget::steps(
            self.limits
                .solver_steps
                .saturating_sub(self.used.solver_steps),
        )
    }

    /// Counts a sketch's solver steps.
    pub(crate) fn solved(&mut self, steps: u32) {
        self.used.solver_steps = self.used.solver_steps.saturating_add(steps);
    }

    /// The refusal for a sketch that ran out of solver steps.
    pub(crate) fn out_of_solver_steps(&self) -> Reason {
        Self::reached(Limit::SolverSteps, u64::from(self.limits.solver_steps))
    }

    /// Counts one solid step, refusing it if it would pass the limit.
    pub(crate) fn solid_step(&mut self) -> Result<(), Reason> {
        let next = self.used.solid_steps.saturating_add(1);
        if next > self.limits.solid_steps {
            return Err(Self::reached(
                Limit::SolidSteps,
                u64::from(self.limits.solid_steps),
            ));
        }
        self.used.solid_steps = next;
        Ok(())
    }

    /// Counts one formula's text, refusing it if it would pass the limit.
    pub(crate) fn formula(&mut self, text: &str) -> Result<(), Reason> {
        let bytes = u64::try_from(text.len()).unwrap_or(u64::MAX);
        let next = self.used.formula_bytes.saturating_add(bytes);
        if next > self.limits.formula_bytes {
            return Err(Self::reached(Limit::FormulaWork, self.limits.formula_bytes));
        }
        self.used.formula_bytes = next;
        Ok(())
    }

    /// Counts the corners of one engine call's profile, refusing them if they would pass the
    /// limit.
    pub(crate) fn corners(&mut self, corners: usize) -> Result<(), Reason> {
        let count = u64::try_from(corners).unwrap_or(u64::MAX);
        let next = self.used.corners.saturating_add(count);
        if next > self.limits.corners {
            return Err(Self::reached(Limit::ResultSize, self.limits.corners));
        }
        self.used.corners = next;
        Ok(())
    }
}
