//! Error bounds per entity: what each face and edge has spent of the fit bound, along its own
//! history (chapter 13.2; decision 32.21; piece G143).
//!
//! What it does: [`Bounds`] holds each named entity's error bound as the terms that make it up:
//! which step added how much ([`Spent`]). [`Bounds::after`] gives the bounds of an operation's
//! outputs from those of its inputs, its [`History`] and its own terms. An output's bound is the sum
//! of its own operands' bounds and the operation's own term for it. [`Bounds::largest`] is a body's
//! bound, and [`Bounds::class`] the class it makes. [`Bounds::join`] puts the bounds of an
//! operation's operands together.
//!
//! Guarantees:
//!
//! - **Along an entity's own history, and nowhere else.** A term reaches an output only through
//!   that output's origin. A face an operation leaves unchanged keeps its bound and takes on nothing,
//!   so a part cut 200 times keeps its measured bound; an edge filleted after a loft carries both.
//! - **Only commits and fits add.** A term of zero is no term: an entity with no term is exact, is
//!   not stored, and a step that spends nothing leaves no trace.
//! - **Refused, with the steps named.** An output past [`Limits::total`] is
//!   [`BoundDetail::OverCeiling`], `ExactnessNotAccepted`, whose message names the earlier steps
//!   that used the budget and whose remedy offers the larger bound. An operation's own term past
//!   [`Limits::own`] is [`BoundDetail::OverStep`] the same way.
//! - **No term goes astray.** An own term for an entity that is not an output, or for one that
//!   passed through unchanged, is a defect of the operation ([`BoundDetail::OwnTermAstray`]), and an
//!   entity named twice in the own terms likewise: it is never summed or dropped.
//! - **In name order.** The same outputs and terms give the same bounds in any order of reporting,
//!   and the first entity in name order is the one a refusal names.
//!
//! Method: the one rule of chapter 13.2, adopted by owner action G120
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`, 32.21).
//!
//! Cost: `after` is `O(n log n)` in the outputs and own terms, plus the terms copied; a lookup is
//! `O(log n)` by bisection, on sorted vectors, never hashed. An entity holds at most one term per
//! whole step of its bound, since every term is at least one. Recursion: none.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use geom_num::ladder::{FIT, PER_OPERATION};
use geom_num::{Exactness, GridStep, Steps};

use crate::detail::BoundDetail;
use crate::error::OpError;
use crate::history::{History, Origin, Record};
use crate::id::{NameId, OpPath};

/// One step's part of an entity's bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spent {
    /// The step that added it.
    pub step: OpPath,
    /// How much.
    pub bound: Steps,
}

/// What an operation may spend: one step's term, and an entity's total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most the operation's own commits and fits may add to one entity.
    pub own: Steps,
    /// The most any entity's bound may reach.
    pub total: Steps,
}

impl Limits {
    /// The ladder's limits: `ε_step` a step, and `ε_fit` in all (6.5).
    pub const DEFAULT: Limits = Limits {
        own: PER_OPERATION,
        total: FIT,
    };
}

/// One entity's terms, which are never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    entity: NameId,
    total: Steps,
    terms: Vec<Spent>,
}

/// The error bounds of a body's named entities, in steps of one grid step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bounds {
    step: GridStep,
    rows: Vec<Row>,
}

/// The sum of `terms`, saturating: a sum that saturates is past any limit.
fn total_of(terms: &[Spent]) -> Steps {
    Steps(
        terms
            .iter()
            .fold(0_u64, |sum, term| sum.saturating_add(term.bound.0)),
    )
}

/// What `record`'s output carries on from: its own operands.
fn operands(record: &Record) -> &[NameId] {
    match &record.origin {
        Origin::Unchanged => core::slice::from_ref(&record.output),
        Origin::Modified { from } | Origin::Generated { from, .. } => from.names(),
        Origin::Split { of, .. } => core::slice::from_ref(of),
        Origin::Merged { of } => of.names(),
    }
}

/// `terms` in words, for a refusal.
fn named(terms: &[Spent]) -> String {
    let words: Vec<String> = terms
        .iter()
        .map(|term| format!("{} spent {}", term.step, term.bound.0))
        .collect();
    words.join(", ")
}

impl Bounds {
    /// No entity has a bound yet: everything is exact, on grid step `step`.
    #[must_use]
    pub const fn new(step: GridStep) -> Bounds {
        Bounds {
            step,
            rows: Vec::new(),
        }
    }

    /// The grid step the bounds count.
    #[must_use]
    pub const fn step(&self) -> GridStep {
        self.step
    }

    /// The row of `entity`.
    fn row(&self, entity: NameId) -> Option<&Row> {
        self.rows
            .binary_search_by_key(&entity, |row| row.entity)
            .ok()
            .and_then(|at| self.rows.get(at))
    }

    /// `entity`'s bound, zero if it has none.
    #[must_use]
    pub fn of(&self, entity: NameId) -> Steps {
        self.row(entity).map_or(Steps(0), |row| row.total)
    }

    /// What makes up `entity`'s bound, oldest step first; empty if it has none.
    #[must_use]
    pub fn terms(&self, entity: NameId) -> &[Spent] {
        self.row(entity).map_or(&[], |row| &row.terms)
    }

    /// The entities with a bound, in name order.
    pub fn entities(&self) -> impl Iterator<Item = NameId> + '_ {
        self.rows.iter().map(|row| row.entity)
    }

    /// The body's bound: the largest of its entities'.
    #[must_use]
    pub fn largest(&self) -> Steps {
        self.rows
            .iter()
            .map(|row| row.total)
            .max()
            .unwrap_or(Steps(0))
    }

    /// The class these bounds make: Exact with no bound, otherwise Certified with the largest.
    #[must_use]
    pub fn class(&self) -> Exactness {
        match self.largest() {
            Steps(0) => Exactness::Exact,
            bound => Exactness::Certified {
                bound,
                step: self.step,
            },
        }
    }

    /// The bounds of two operands of `op` together. An entity both carry is one entity, copied, and
    /// keeps its larger bound.
    ///
    /// # Errors
    ///
    /// [`BoundDetail::MixedSteps`] for bounds on different grid steps.
    pub fn join(&self, op: &OpPath, other: &Bounds) -> Result<Bounds, OpError> {
        if self.step != other.step {
            return Err(OpError::new(op.clone(), BoundDetail::MixedSteps));
        }
        let mut rows = Vec::with_capacity(self.rows.len().saturating_add(other.rows.len()));
        rows.extend(self.rows.iter().cloned());
        for row in &other.rows {
            match self.of(row.entity) {
                Steps(0) => rows.push(row.clone()),
                held if held < row.total => {
                    rows.retain(|r: &Row| r.entity != row.entity);
                    rows.push(row.clone());
                }
                _ => {}
            }
        }
        rows.sort_unstable_by_key(|row| row.entity);
        Ok(Bounds {
            step: self.step,
            rows,
        })
    }

    /// The bounds of `op`'s outputs: each is the sum of its operands' bounds, from `self`, and its
    /// term in `own`, which names only outputs with a term. Inputs with no output go.
    ///
    /// # Errors
    ///
    /// For the first entity in name order: [`BoundDetail::OverStep`] for an own term past
    /// `limits.own`, [`BoundDetail::OverCeiling`] for a sum past `limits.total`, and
    /// [`BoundDetail::OwnTermAstray`] for an own term that names no output, an unchanged output, or
    /// one output twice.
    pub fn after(
        &self,
        op: &OpPath,
        history: &History,
        own: &[(NameId, Steps)],
        limits: Limits,
    ) -> Result<Bounds, OpError> {
        let mut own: Vec<(NameId, Steps)> = own.iter().copied().filter(|t| t.1.0 > 0).collect();
        own.sort_unstable();
        let astray = |entity: NameId| {
            OpError::new(op.clone(), BoundDetail::OwnTermAstray).with_entities([entity])
        };
        let repeat = own.iter().zip(own.iter().skip(1)).find(|(a, b)| a.0 == b.0);
        if let Some(((entity, _), _)) = repeat {
            return Err(astray(*entity));
        }
        let mut rows = Vec::with_capacity(self.rows.len().saturating_add(own.len()));
        for record in history.records() {
            let at = own.binary_search_by_key(&record.output, |t| t.0);
            let added = at.ok().and_then(|at| own.get(at)).map(|t| t.1);
            let mut terms: Vec<Spent> = Vec::new();
            for operand in operands(record) {
                terms.extend_from_slice(self.terms(*operand));
            }
            if let Some(bound) = added {
                if record.origin == Origin::Unchanged {
                    return Err(astray(record.output));
                }
                if bound > limits.own {
                    let detail = BoundDetail::OverStep {
                        own: bound,
                        limit: limits.own,
                        step: self.step,
                    };
                    return Err(OpError::new(op.clone(), detail).with_entities([record.output]));
                }
                terms.push(Spent {
                    step: op.clone(),
                    bound,
                });
            }
            let total = total_of(&terms);
            if total > limits.total {
                let detail = BoundDetail::OverCeiling {
                    needed: total,
                    ceiling: limits.total,
                    step: self.step,
                };
                let words = format!("{detail}: {}", named(&terms));
                return Err(OpError::new(op.clone(), detail)
                    .with_entities([record.output])
                    .with_message(words));
            }
            if !terms.is_empty() {
                rows.push(Row {
                    entity: record.output,
                    total,
                    terms,
                });
            }
        }
        if let Some((entity, _)) = own
            .iter()
            .find(|(entity, _)| history.origin(*entity).is_none())
        {
            return Err(astray(*entity));
        }
        Ok(Bounds {
            step: self.step,
            rows,
        })
    }
}
