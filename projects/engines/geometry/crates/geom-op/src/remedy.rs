//! Remedies: what would make a failed operation succeed, as data a person or an AI agent can act
//! on.
//!
//! What it does: a [`Remedy`] is one of the things chapter 22's table says a caller can do about
//! an error, with the values that make it concrete: change a named input, to at most or at least a
//! value where one is known ("radius at most 4.2 mm here"); choose among candidates; take another
//! route ("use the mesh boolean"); accept a less exact class; heal first; work in another frame or
//! at a coarser step; raise a budget, split the call or ask for a preview; wait for the build piece
//! that adds a route; or report a bug.
//!
//! Guarantees:
//!
//! - **Every action chapter 22 names has a form.** Each kind's "what the caller can do" in chapter
//!   22's table is one or more of these variants; `Cancelled` and `OutOfMemory` have none, because
//!   there is nothing to do.
//! - **Concrete where the operation knows.** A value, a class, a bound or a budget is carried, never
//!   left for the reader to work out from the message.
//! - **Plain words.** `Display` writes each remedy as an instruction.
//!
//! Method: none; chapter 22's column "What the caller can do", written as data
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: a few machine words per remedy, plus the candidates' vector for [`Remedy::Choose`].

use alloc::vec::Vec;
use core::fmt;

use geom_num::{Exactness, GridStep, Steps};
use geom_run::BudgetKind;

use crate::evidence::Value;
use crate::id::NameId;

/// How a value that works relates to the one given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bound {
    /// The input works at this value or below.
    AtMost,
    /// The input works at this value or above.
    AtLeast,
    /// The input works at exactly this value.
    Exactly,
}

/// What a caller can do to make a failed operation succeed (chapter 22).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Remedy {
    /// Change the input named `input`; with `to`, the nearest value that works.
    ChangeInput {
        /// The input, by its name in the operation's schema.
        input: &'static str,
        /// The nearest value that works, and on which side, where the operation knows it.
        to: Option<(Bound, Value)>,
    },
    /// Choose one of these entities, or add what decides between them.
    Choose {
        /// The candidates, in name order.
        candidates: Vec<NameId>,
    },
    /// Take another route that exists, such as the mesh boolean.
    UseRoute {
        /// The route, by its name in the schemas.
        route: &'static str,
    },
    /// Accept explicitly the class and bound the result would reach.
    Accept {
        /// The class and bound the result would have.
        class: Exactness,
    },
    /// Heal the input first, closing gaps of up to this length.
    Heal {
        /// The widest gap healing must close, in steps of `step`.
        gap: Steps,
        /// The grid step `gap` counts.
        step: GridStep,
    },
    /// Work in a frame placed where the data overlaps, or on section 17.1's lattice, or split the
    /// data across frames.
    ChangeFrame,
    /// Ask for a coarser grid step, which lowers the class.
    CoarserStep {
        /// The step that would fit.
        step: GridStep,
    },
    /// Raise a budget to at least this much.
    RaiseBudget {
        /// Which limit.
        kind: BudgetKind,
        /// The least limit that would have let the step that failed pass.
        at_least: u128,
    },
    /// Split the call into smaller ones.
    SplitCall,
    /// Ask for a preview precision.
    Preview,
    /// Wait for the build piece that adds a route for this input.
    AwaitPiece {
        /// The piece, by its board number, such as `G227`.
        piece: &'static str,
    },
    /// Report the defect; the error carries what reproduces it.
    Report,
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Bound::AtMost => "at most",
            Bound::AtLeast => "at least",
            Bound::Exactly => "exactly",
        })
    }
}

impl fmt::Display for Remedy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Remedy::ChangeInput { input, to: None } => write!(f, "change `{input}`"),
            Remedy::ChangeInput {
                input,
                to: Some((bound, value)),
            } => write!(f, "set `{input}` to {bound} {value}"),
            Remedy::Choose { candidates } => {
                f.write_str("choose one of")?;
                for name in candidates {
                    write!(f, " {name}")?;
                }
                f.write_str(", or add what decides")
            }
            Remedy::UseRoute { route } => write!(f, "use `{route}`"),
            Remedy::Accept { class } => write!(f, "accept {class:?} explicitly"),
            Remedy::Heal { gap, step } => {
                write!(
                    f,
                    "heal first, closing gaps up to {} steps of {step:?}",
                    gap.0
                )
            }
            Remedy::ChangeFrame => f.write_str(
                "work in a frame placed where the data overlaps, or split it across frames",
            ),
            Remedy::CoarserStep { step } => write!(f, "use the coarser grid step {step:?}"),
            Remedy::RaiseBudget { kind, at_least } => {
                write!(f, "raise the {kind} budget to at least {at_least}")
            }
            Remedy::SplitCall => f.write_str("split the call into smaller ones"),
            Remedy::Preview => f.write_str("ask for a preview precision"),
            Remedy::AwaitPiece { piece } => write!(f, "wait for piece {piece}, which adds a route"),
            Remedy::Report => f.write_str("report it: this is a defect in the kernel"),
        }
    }
}
