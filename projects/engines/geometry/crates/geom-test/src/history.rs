//! The history laws as a kit every operation's tests run: chapter 20.3's four laws checked on one
//! history and on a chain, and seeded generators of valid histories and chains.
//!
//! What it does: [`check_laws`] checks one operation's history against laws 1 to 3 and 13.14's
//! recording rule. [`check_chain`] checks every step of a chain, composes it, checks the
//! composition against the chain's first inputs (law 4: histories compose and keep laws 1 to 3),
//! and checks that composing left to right and right to left give the same relations; it says
//! which step or which part failed ([`ChainBreak`]). [`step`] and [`chain`] are proptest
//! strategies of valid histories: every input unchanged, modified, split, merged with others,
//! deleted, or deleted while generating, plus outputs generated from nothing, each output under a
//! fresh name.
//!
//! Guarantees:
//!
//! - **Every law is checked through `geom-op`'s own checker**, so the kit and the contract cannot
//!   disagree; the kit adds the chain's composition and its associativity.
//! - **The generators only make histories the laws allow**, so a law break the kit reports on them
//!   is a defect of `geom-op`'s checker or composition; its tests seed one break of each law to
//!   show the checker finds it.
//! - **Seeded and repeatable**: names are made from the step and position, never drawn at random
//!   beyond proptest's own seeded runner.
//!
//! Method: chapter 20.3's laws and the method note's composition rules
//! (`projects/engines/geometry/architecture/20-naming-history.md`,
//! `projects/engines/geometry/methods/occt-history-contract.md`), as property-based tests in
//! proptest's strategies (<https://proptest-rs.github.io/proptest/proptest/tutorial/strategy-basics.html>).
//!
//! Cost: linear in a history's records, times the composition's ordered-set operations; a chain of
//! `k` steps composes twice, `O(k)` compositions.

use core::fmt;
use std::collections::BTreeSet;

use geom_op::{
    History, LawBreak, NameId, NameSet, Origin, PieceKey, Record, Relations, Role, Side,
};
use proptest::prelude::*;

/// Whether `history` keeps laws 1 to 3 and 13.14's recording rule for an operation with these
/// inputs: `geom-op`'s own checker.
///
/// # Errors
///
/// The first [`LawBreak`].
pub fn check_laws(history: &History, inputs: &NameSet) -> Result<(), LawBreak> {
    history.check(inputs)
}

/// Where a chain of histories breaks chapter 20.3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainBreak {
    /// Step `at` (from 0) breaks a law on its own.
    Step {
        /// The step.
        at: usize,
        /// The law it breaks.
        broken: LawBreak,
    },
    /// The composed chain breaks a law for the first step's inputs (law 4).
    Composed(LawBreak),
    /// Composing from the left and from the right give different relations (law 4).
    Groupings,
    /// Law 2: an output generated and modified from one input, at step `at` or, with `None`, in
    /// the composed chain. `geom-op` makes this impossible by construction; the kit checks it.
    GeneratedAndModified {
        /// The step, or `None` for the composed chain.
        at: Option<usize>,
        /// The input.
        input: NameId,
        /// The output.
        output: NameId,
    },
}

impl fmt::Display for ChainBreak {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainBreak::Step { at, broken } => write!(f, "step {at}: {broken}"),
            ChainBreak::Composed(broken) => write!(f, "the composed chain: {broken}"),
            ChainBreak::Groupings => {
                f.write_str("law 4: composing from the left and from the right differ")
            }
            ChainBreak::GeneratedAndModified { at, input, output } => {
                match at {
                    Some(at) => write!(f, "step {at}: ")?,
                    None => f.write_str("the composed chain: ")?,
                }
                write!(
                    f,
                    "law 2: {output} is both generated and modified from {input}"
                )
            }
        }
    }
}

impl core::error::Error for ChainBreak {}

/// Whether every step of `steps` (each history with its inputs, the next step's inputs being the
/// outputs of the one before) keeps the laws, the composed chain keeps laws 1 to 3 for the first
/// step's inputs, and composing from either end agrees (law 4).
///
/// # Errors
///
/// The first [`ChainBreak`]: steps in order, then the groupings, then the composed chain.
pub fn check_chain(steps: &[(History, NameSet)]) -> Result<(), ChainBreak> {
    let relations: Vec<Relations> = steps.iter().map(|(h, _)| h.relations()).collect();
    for (at, ((history, inputs), r)) in steps.iter().zip(&relations).enumerate() {
        history
            .check(inputs)
            .map_err(|broken| ChainBreak::Step { at, broken })?;
        law_two(r, Some(at))?;
    }
    let Some((first, rest)) = relations.split_first() else {
        return Ok(());
    };
    let left = rest.iter().fold(first.clone(), |acc, next| acc.then(next));
    let right = relations
        .iter()
        .rev()
        .cloned()
        .reduce(|later, earlier| earlier.then(&later));
    if right.as_ref() != Some(&left) {
        return Err(ChainBreak::Groupings);
    }
    law_two(&left, None)?;
    match steps.first() {
        Some((_, inputs)) => left.check(inputs).map_err(ChainBreak::Composed),
        None => Ok(()),
    }
}

/// Law 2 on one set of relations: no pair is both generated and modified.
fn law_two(relations: &Relations, at: Option<usize>) -> Result<(), ChainBreak> {
    let modified: BTreeSet<(NameId, NameId)> = relations.modified().collect();
    match relations.generated().find(|pair| modified.contains(pair)) {
        Some((input, output)) => Err(ChainBreak::GeneratedAndModified { at, input, output }),
        None => Ok(()),
    }
}

/// A fresh name: step `step`, position `at`, tagged so it never equals another's.
fn fresh(step: u8, at: u16, tag: u8) -> NameId {
    let [hi, lo] = at.to_be_bytes();
    let mut bytes = [0xA5; 16];
    bytes[0] = step;
    bytes[1] = tag;
    bytes[2] = hi;
    bytes[3] = lo;
    NameId::from_bytes(bytes)
}

/// What becomes of one input in a generated step.
#[derive(Clone, Copy, Debug)]
enum Fate {
    Unchanged,
    Modified,
    Split,
    Merged,
    Deleted,
    DeletedGenerating,
}

fn fate() -> impl Strategy<Value = Fate> {
    prop_oneof![
        3 => Just(Fate::Unchanged),
        2 => Just(Fate::Modified),
        1 => Just(Fate::Split),
        2 => Just(Fate::Merged),
        1 => Just(Fate::Deleted),
        1 => Just(Fate::DeletedGenerating),
    ]
}

/// The history of step `step` over `inputs` with the given fates and `new` outputs made from
/// nothing, with its outputs; `None` only if the generator broke `History`'s forms itself.
fn build(step: u8, inputs: &[NameId], fates: &[Fate], new: u8) -> Option<(History, NameSet)> {
    let mut records = Vec::new();
    let mut deleted = Vec::new();
    let mut merging = Vec::new();
    let mut at: u16 = 0;
    let mut next = |tag: u8| {
        at = at.wrapping_add(1);
        fresh(step, at, tag)
    };
    let roll = Role::new("roll", []).ok()?;
    let cap = Role::new("cap", []).ok()?;
    for (&input, &fate) in inputs.iter().zip(fates) {
        match fate {
            Fate::Unchanged => records.push(Record {
                output: input,
                origin: Origin::Unchanged,
            }),
            Fate::Modified => records.push(Record {
                output: next(1),
                origin: Origin::Modified {
                    from: NameSet::new([input]).ok()?,
                },
            }),
            Fate::Split => {
                let tools = NameSet::new([next(2)]).ok()?;
                for side in [Side::Back, Side::Front] {
                    records.push(Record {
                        output: next(3),
                        origin: Origin::Split {
                            of: input,
                            piece: PieceKey::ByTools {
                                tools: tools.clone(),
                                side,
                            },
                        },
                    });
                }
            }
            Fate::Merged => merging.push(input),
            Fate::Deleted => deleted.push(input),
            Fate::DeletedGenerating => {
                deleted.push(input);
                records.push(Record {
                    output: next(4),
                    origin: Origin::Generated {
                        from: NameSet::new([input]).ok()?,
                        role: roll.clone(),
                    },
                });
            }
        }
    }
    match merging.len() {
        0 => {}
        1 => records.push(Record {
            output: next(5),
            origin: Origin::Modified {
                from: NameSet::new(merging).ok()?,
            },
        }),
        _ => records.push(Record {
            output: next(6),
            origin: Origin::Merged {
                of: NameSet::new(merging).ok()?,
            },
        }),
    }
    for _ in 0..new {
        records.push(Record {
            output: next(7),
            origin: Origin::Generated {
                from: NameSet::default(),
                role: cap.clone(),
            },
        });
    }
    let outputs = NameSet::new(records.iter().map(|r| r.output)).ok()?;
    let history = History::new(records, NameSet::new(deleted).ok()?, []).ok()?;
    Some((history, outputs))
}

/// A strategy of one valid history of step `step` over `inputs`, with its outputs.
pub fn step(step: u8, inputs: Vec<NameId>) -> impl Strategy<Value = (History, NameSet)> {
    let n = inputs.len();
    (prop::collection::vec(fate(), n), 0_u8..3).prop_filter_map(
        "the generator makes only histories History accepts",
        move |(fates, new)| build(step, &inputs, &fates, new),
    )
}

/// A strategy of a chain of `len` valid histories over `inputs`: each with its inputs, the next
/// step's inputs being the outputs of the one before (at most 64 of them).
pub fn chain(inputs: Vec<NameId>, len: u8) -> impl Strategy<Value = Vec<(History, NameSet)>> {
    (
        prop::collection::vec(prop::collection::vec(fate(), 64), usize::from(len)),
        prop::collection::vec(0_u8..3, usize::from(len)),
    )
        .prop_filter_map(
            "the generator makes only histories History accepts",
            move |(fates, news)| {
                let mut steps = Vec::new();
                let mut current = inputs.clone();
                for (k, (fates, new)) in (0_u8..).zip(fates.iter().zip(news)) {
                    let (history, outputs) = build(k, &current, fates, new)?;
                    let ins = NameSet::new(current).ok()?;
                    current = outputs.names().iter().take(64).copied().collect();
                    steps.push((history, ins));
                }
                Some(steps)
            },
        )
}
