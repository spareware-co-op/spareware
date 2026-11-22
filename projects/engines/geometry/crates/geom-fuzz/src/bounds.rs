//! Fuzz target: `geom-op`'s error bounds per entity (G143), on any bytes, give each output the sum
//! of its own operands' terms and its own, keep an unchanged entity's terms, agree on the largest
//! bound and its class, keep the larger on a join, refuse a sum past the ceiling without wrapping,
//! and answer the same in any order of reporting.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a grid step, up to [`MAX_OPS`]
//! operations on up to [`NAMES`] entities, another grid step and the state to join. Each operation
//! has a path (one of four, so steps repeat), a record for each entity it keeps (passed through
//! unchanged, modified, generated, split or merged, from any of the entities, or none), up to
//! [`MAX_TERMS`] own terms (zero, small, exactly the step's limit, one past it, any word, or within
//! four of the largest word, on any entity, an output or not, once or twice) and its limits (the
//! ladder's, small ones, the largest own term with a ceiling near the largest word, or any). Its
//! records and terms come in an order of the bytes' choosing. [`check`] runs the operations from
//! [`Bounds::new`] through [`Bounds::after`], each again with its records and terms reported in
//! the other order, then joins the last state with an earlier one ([`Bounds::join`]) and with no
//! bounds on the other grid step, and holds each to the laws below. [`harness`] is `decode` then
//! `check`, the function every fuzzing engine drives (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, and [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case:
//!   - an output whose bound is not the sum of its own operands' terms and its own term, summed
//!     here from scratch in big integers, or whose terms are not those terms, oldest first; an
//!     entity with a bound that is not an output, or a stored entity with no term;
//!   - an unchanged entity whose terms are not the ones it had;
//!   - a largest bound that is not the largest entity's, or a class that is not Exact for none and
//!     Certified with the largest otherwise, or bounds on another grid step;
//!   - a refusal of an operation with no fault, or no refusal of one with a fault: an own term
//!     past the step's limit, a sum past the ceiling (in big integers, so a sum that would wrap is
//!     past it), an own term on an unchanged output, on no output, or twice on one entity;
//!   - a refusal that names an entity with no fault, or a fault it does not have, or not the first
//!     at fault in name order when every fault is of one kind (named twice, an output at fault, a
//!     term on no output); a needed bound that is not the true sum, or the largest word for a sum
//!     past it; a message that does not name each step that spent; a kind or remedy not its
//!     fault's (`ExactnessNotAccepted` offering the larger bound, `Internal` asking for a report);
//!   - an answer that changes with the order the records and terms are reported in;
//!   - a join whose entity has not the larger of its two bounds and that operand's terms (the
//!     first operand's on a tie), whose two orders differ in a bound, or that is not refused
//!     `MixedSteps` (`InvalidInput`) on two grid steps; a join with nothing that changes anything.
//!
//! Not drawn: a ceiling of the largest word (`u64::MAX`). There a sum past the largest word
//! saturates to it and is not past the ceiling, so it is accepted with a bound below its true sum
//! (found by this target; the module's own words say a sum that saturates is past any limit).
//!
//! Method: chapter 13.2's one rule and decision 32.21
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`), held to a model written from scratch:
//! each entity's terms in a sorted map, an output's terms its operands' (20.3's origins read again
//! here) followed by its own, and every sum in `num-bigint`'s integers, sharing no code with the
//! module.
//!
//! Cost: at most [`MAX_OPS`] operations twice over, on [`NAMES`] entities, plus two joins each way;
//! an entity holds at most [`NAMES`] to the power [`MAX_OPS`] terms, when every output merges every
//! entity in every operation. At most [`MAX_INPUT_BYTES`] bytes read.

use std::collections::BTreeMap;

use geom_num::{Exactness, GridStep, Steps};
use geom_op::bounds::{Bounds, Limits, Spent};
use geom_op::detail::{BoundDetail, OpErrorDetail};
use geom_op::{
    ErrorKind, History, NameId, NameSet, OpError, OpId, OpPath, Origin, PieceKey, Record, Remedy,
    Role, Side,
};
use geom_test::{ByteSource, Entropy};
use num_bigint::BigUint;

use crate::Finding;

/// The entities a case names.
pub const NAMES: usize = 5;

/// The most operations a case runs.
pub const MAX_OPS: usize = 5;

/// The most own terms an operation gives.
pub const MAX_TERMS: usize = 6;

/// The bytes one word reads.
const WORD_BYTES: usize = 8;

/// The bytes one operation reads: its path, its order, a rule and a set for each entity, its
/// limits' rule and two words, its count of terms, and each term's entity, rule and word.
const OP_BYTES: usize = 1 + 1 + 2 * NAMES + 1 + 2 * WORD_BYTES + 1 + MAX_TERMS * (2 + WORD_BYTES);

/// The most bytes [`decode`] reads: the two grid steps, the partner, the count, and the operations.
pub const MAX_INPUT_BYTES: usize = 4 + MAX_OPS * OP_BYTES;

/// The path of every join.
const JOIN: [u8; 16] = [0xaa; 16];

/// One operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    /// Its path.
    pub path: OpPath,
    /// What it did, in the order reported.
    pub records: Vec<Record>,
    /// Its own terms, in the order reported.
    pub own: Vec<(NameId, Steps)>,
    /// What it may spend.
    pub limits: Limits,
}

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The grid step the bounds count.
    pub step: GridStep,
    /// The operations, in order.
    pub ops: Vec<Op>,
    /// Which state the last one is joined with, counted modulo the states.
    pub partner: u8,
    /// The grid step of the empty bounds the last state is joined with.
    pub other: GridStep,
}

/// Entity `i` of [`NAMES`].
#[must_use]
pub fn name(i: usize) -> NameId {
    let byte = u8::try_from(i).unwrap_or(u8::MAX).saturating_add(1);
    NameId::from_bytes([byte; 16])
}

/// `x` modulo `n`, zero for no `n`.
fn modulo(x: usize, n: usize) -> usize {
    x.checked_rem(n).unwrap_or(0)
}

/// A grid step through its byte.
fn grid_step(byte: u8) -> GridStep {
    match byte % 3 {
        0 => GridStep::Q0,
        1 => GridStep::Q1,
        _ => GridStep::Q2,
    }
}

/// The entities whose bits `bits` sets, in name order.
fn subset(bits: u8) -> Vec<NameId> {
    (0..NAMES)
        .filter(|i| {
            u32::try_from(*i)
                .ok()
                .and_then(|i| bits.checked_shr(i))
                .is_some_and(|b| b & 1 != 0)
        })
        .map(name)
        .collect()
}

/// The set of `names`, which hold no repeat.
fn set(names: Vec<NameId>) -> NameSet {
    NameSet::new(names).unwrap_or_default()
}

/// Entity `i`'s origin through its rule and set bytes, or none if the operation does not keep it.
fn origin(i: usize, rule: u8, bits: u8) -> Option<Origin> {
    let mut from = subset(bits);
    match rule % 8 {
        0 | 1 => None,
        2 | 3 => Some(Origin::Unchanged),
        4 => {
            if from.is_empty() {
                from.push(name(i));
            }
            Some(Origin::Modified { from: set(from) })
        }
        5 => Some(Origin::Generated {
            from: set(from),
            role: Role::new("face", []).ok()?,
        }),
        6 => Some(Origin::Split {
            of: name(modulo(usize::from(bits), NAMES)),
            piece: PieceKey::ByTools {
                tools: NameSet::default(),
                side: if bits & 0x80 == 0 {
                    Side::Back
                } else {
                    Side::Front
                },
            },
        }),
        _ => {
            for extra in [i, modulo(i.saturating_add(1), NAMES)] {
                if from.len() < 2 && !from.contains(&name(extra)) {
                    from.push(name(extra));
                }
            }
            from.sort_unstable();
            Some(Origin::Merged { of: set(from) })
        }
    }
}

/// An operation's limits through their rule byte: the ladder's, small, the largest own term with
/// a ceiling near the largest word, or any; never a ceiling of the largest word (the module
/// header says why).
fn limits(src: &mut ByteSource<'_>) -> Limits {
    let rule = src.byte();
    let (a, b) = (src.word(), src.word());
    let below_max = |x: u64| Steps(x.min(u64::MAX.saturating_sub(1)));
    match rule % 6 {
        0..=2 => Limits::DEFAULT,
        3 => Limits {
            own: Steps(a % 32),
            total: Steps(b % 128),
        },
        4 => Limits {
            own: Steps(u64::MAX),
            total: below_max(u64::MAX.saturating_sub(b % 4)),
        },
        _ => Limits {
            own: Steps(a),
            total: below_max(b),
        },
    }
}

/// An own term through its bytes: an entity, and a bound that is zero, the step's limit, one past
/// it, within four of the largest word, any word, or small.
fn term(src: &mut ByteSource<'_>, limits: Limits) -> (NameId, Steps) {
    let entity = name(modulo(usize::from(src.byte()), NAMES));
    let rule = src.byte();
    let word = src.word();
    let bound = match rule % 8 {
        0 => Steps(0),
        1 => limits.own,
        2 => Steps(limits.own.0.saturating_add(1)),
        3 => Steps(u64::MAX.saturating_sub(word % 4)),
        4 => Steps(word),
        _ => Steps(word % 17),
    };
    (entity, bound)
}

/// One operation.
fn op(src: &mut ByteSource<'_>) -> Op {
    let id = src.byte() % 4;
    let path = OpPath::new(OpId::from_bytes([0x10 | id; 16]));
    let order = usize::from(src.byte());
    let mut records = Vec::with_capacity(NAMES);
    for i in 0..NAMES {
        let (rule, bits) = (src.byte(), src.byte());
        if let Some(origin) = origin(i, rule, bits) {
            records.push(Record {
                output: name(i),
                origin,
            });
        }
    }
    let limits = limits(src);
    let count = modulo(usize::from(src.byte()), MAX_TERMS.saturating_add(1));
    let mut own = Vec::with_capacity(MAX_TERMS);
    for i in 0..MAX_TERMS {
        let t = term(src, limits);
        if i < count {
            own.push(t);
        }
    }
    let turn = modulo(order, records.len());
    records.rotate_left(turn);
    let turn = modulo(order >> 3, own.len());
    own.rotate_left(turn);
    if order & 0x80 != 0 {
        records.reverse();
    }
    Op {
        path,
        records,
        own,
        limits,
    }
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let step = grid_step(src.byte());
    let other = grid_step(src.byte());
    let partner = src.byte();
    let count = modulo(usize::from(src.byte()), MAX_OPS.saturating_add(1));
    let mut ops = Vec::with_capacity(MAX_OPS);
    for i in 0..MAX_OPS {
        let o = op(&mut src);
        if i < count {
            ops.push(o);
        }
    }
    let read = data.len().saturating_sub(src.remaining());
    let case = Case {
        step,
        ops,
        partner,
        other,
    };
    (case, read)
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

// The model: each entity's terms in a sorted map, every sum in big integers.

/// Each entity's terms, oldest first; an entity with none is not in it.
type Model = BTreeMap<NameId, Vec<Spent>>;

/// Which kind of fault an entity has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// It has two own terms or more.
    Twice,
    /// It is an output, at fault.
    Output,
    /// It has an own term and is no output.
    Stray,
}

/// What an operation should answer.
#[derive(Debug)]
enum Want {
    /// These bounds.
    Held(Model),
    /// A refusal, for one of these entities, with one of its faults; and every output's terms.
    Refused(BTreeMap<NameId, (Kind, Vec<BoundDetail>)>, Model),
}

/// The entities `record`'s output carries on from (chapter 20.3), written out here again.
fn carried(record: &Record) -> Vec<NameId> {
    match &record.origin {
        Origin::Unchanged => vec![record.output],
        Origin::Modified { from } | Origin::Generated { from, .. } => from.names().to_vec(),
        Origin::Split { of, .. } => vec![*of],
        Origin::Merged { of } => of.names().to_vec(),
    }
}

/// The sum of `terms`, exactly.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "a big integer's sum is exact"
)]
fn sum(terms: &[Spent]) -> BigUint {
    terms
        .iter()
        .fold(BigUint::default(), |s, t| s + BigUint::from(t.bound.0))
}

/// What `op` should answer from bounds `before` on grid step `step`.
fn want(before: &Model, op: &Op, step: GridStep) -> Want {
    let mut own: BTreeMap<NameId, Vec<Steps>> = BTreeMap::new();
    for (entity, bound) in &op.own {
        if bound.0 > 0 {
            own.entry(*entity).or_default().push(*bound);
        }
    }
    let mut faults: BTreeMap<NameId, (Kind, Vec<BoundDetail>)> = BTreeMap::new();
    for (entity, bounds) in &own {
        if bounds.len() > 1 {
            faults.insert(*entity, (Kind::Twice, vec![BoundDetail::OwnTermAstray]));
        }
    }
    let mut after = Model::new();
    for record in &op.records {
        let output = record.output;
        let mut terms: Vec<Spent> = Vec::new();
        for operand in carried(record) {
            terms.extend(before.get(&operand).into_iter().flatten().cloned());
        }
        let mut found = Vec::new();
        if let Some([bound]) = own.get(&output).map(Vec::as_slice) {
            if record.origin == Origin::Unchanged {
                found.push(BoundDetail::OwnTermAstray);
            }
            if *bound > op.limits.own {
                found.push(BoundDetail::OverStep {
                    own: *bound,
                    limit: op.limits.own,
                    step,
                });
            }
            terms.push(Spent {
                step: op.path.clone(),
                bound: *bound,
            });
        }
        let total = sum(&terms);
        if total > BigUint::from(op.limits.total.0) {
            found.push(BoundDetail::OverCeiling {
                needed: Steps(u64::try_from(&total).unwrap_or(u64::MAX)),
                ceiling: op.limits.total,
                step,
            });
        }
        if !found.is_empty() && !faults.contains_key(&output) {
            faults.insert(output, (Kind::Output, found));
        }
        if !terms.is_empty() {
            after.insert(output, terms);
        }
    }
    for entity in own.keys() {
        if !op.records.iter().any(|r| r.output == *entity) {
            faults
                .entry(*entity)
                .or_insert((Kind::Stray, Vec::new()))
                .1
                .push(BoundDetail::OwnTermAstray);
        }
    }
    if faults.is_empty() {
        Want::Held(after)
    } else {
        Want::Refused(faults, after)
    }
}

/// Every entity a case names, in name order.
fn every() -> impl Iterator<Item = NameId> {
    (0..NAMES).map(name)
}

/// `got` holds the model's bounds.
fn held(case: &Case, at: usize, got: &Bounds, model: &Model) -> Result<(), Finding> {
    let entities: Vec<NameId> = got.entities().collect();
    let modelled: Vec<NameId> = model.keys().copied().collect();
    if entities != modelled {
        return Err(Finding::new(
            "the entities with a bound are the outputs with a term",
            || format!("operation {at}: got {entities:?}, model {modelled:?}; {case:?}"),
        ));
    }
    for entity in every() {
        let terms = model.get(&entity).map_or(&[][..], Vec::as_slice);
        if BigUint::from(got.of(entity).0) != sum(terms) || got.terms(entity) != terms {
            return Err(Finding::new(
                "an output's bound is the sum of its own operands' terms and its own term",
                || {
                    format!(
                        "operation {at}, entity {entity}: got {:?} of {:?}, model {} of {terms:?}; \
                         {case:?}",
                        got.of(entity),
                        got.terms(entity),
                        sum(terms)
                    )
                },
            ));
        }
    }
    Ok(())
}

/// An unchanged entity with no own term keeps the terms it had.
fn kept(case: &Case, at: usize, before: &Bounds, got: &Bounds) -> Result<(), Finding> {
    let op = case.ops.get(at);
    for record in op.into_iter().flat_map(|op| &op.records) {
        let e = record.output;
        let own = op.is_some_and(|op| op.own.iter().any(|t| t.0 == e && t.1.0 > 0));
        if record.origin == Origin::Unchanged && !own && got.terms(e) != before.terms(e) {
            return Err(Finding::new(
                "an unchanged entity keeps its terms as they are",
                || {
                    format!(
                        "operation {at}, entity {e}: had {:?}, got {:?}; {case:?}",
                        before.terms(e),
                        got.terms(e)
                    )
                },
            ));
        }
    }
    Ok(())
}

/// The largest bound is the largest entity's, its class says so, and every stored entity has a
/// term.
fn largest(case: &Case, got: &Bounds) -> Result<(), Finding> {
    let mut most = Steps(0);
    for entity in got.entities() {
        if got.of(entity) == Steps(0) || got.terms(entity).is_empty() {
            return Err(Finding::new(
                "an entity with no term is exact and not stored",
                || format!("entity {entity}: {:?}; {case:?}", got.terms(entity)),
            ));
        }
        most = most.max(got.of(entity));
    }
    let class = if most == Steps(0) {
        Exactness::Exact
    } else {
        Exactness::Certified {
            bound: most,
            step: case.step,
        }
    };
    if got.largest() != most || got.class() != class || got.step() != case.step {
        return Err(Finding::new(
            "the body's bound is the largest entity's, and its class says so",
            || {
                format!(
                    "largest {:?} and class {:?} on {:?}, want {most:?} and {class:?} on {:?}; \
                     {case:?}",
                    got.largest(),
                    got.class(),
                    got.step(),
                    case.step
                )
            },
        ));
    }
    Ok(())
}

/// The kind and remedies `detail` asks for.
fn typed(detail: &BoundDetail) -> (ErrorKind, Vec<Remedy>) {
    match *detail {
        BoundDetail::OverStep {
            own: bound, step, ..
        }
        | BoundDetail::OverCeiling {
            needed: bound,
            step,
            ..
        } => (
            ErrorKind::ExactnessNotAccepted,
            vec![Remedy::Accept {
                class: Exactness::Certified { bound, step },
            }],
        ),
        BoundDetail::OwnTermAstray => (ErrorKind::Internal, vec![Remedy::Report]),
        // The model names no other fault, and a refusal's detail is one of the model's.
        _ => (ErrorKind::InvalidInput, Vec::new()),
    }
}

/// `error` is a refusal the faults allow.
fn refused(
    case: &Case,
    at: usize,
    error: &OpError,
    faults: &BTreeMap<NameId, (Kind, Vec<BoundDetail>)>,
    terms: &Model,
) -> Result<(), Finding> {
    let op = case.ops.get(at).map(|op| &op.path);
    let entity = match error.entities() {
        [entity] => *entity,
        _ => {
            return Err(Finding::new("a refusal names one entity", || {
                format!("operation {at}: {error:?}; {case:?}")
            }));
        }
    };
    let OpErrorDetail::Bound(detail) = error.detail() else {
        return Err(Finding::new("a refusal is a bound's", || {
            format!("operation {at}: {error:?}; {case:?}")
        }));
    };
    let allowed = faults.get(&entity).is_some_and(|(_, d)| d.contains(detail));
    if !allowed || op != Some(error.op()) {
        return Err(Finding::new(
            "a refusal names an entity at fault, with a fault it has, and the operation",
            || format!("operation {at}: {error:?}, faults {faults:?}; {case:?}"),
        ));
    }
    let kinds: Vec<Kind> = faults.values().map(|f| f.0).collect();
    let first = faults.keys().next().copied();
    if kinds.windows(2).all(|w| w.first() == w.last()) && first != Some(entity) {
        return Err(Finding::new(
            "the first entity at fault in name order is the one a refusal names",
            || format!("operation {at}: {error:?}, faults {faults:?}; {case:?}"),
        ));
    }
    let (kind, remedies) = typed(detail);
    if error.kind() != kind || error.remedies() != remedies.as_slice() {
        return Err(Finding::new(
            "a refusal's kind and remedy are its fault's",
            || format!("operation {at}: {error:?}; {case:?}"),
        ));
    }
    if let BoundDetail::OverCeiling { .. } = detail {
        let spent = terms.get(&entity).map_or(&[][..], Vec::as_slice);
        if let Some(term) = spent
            .iter()
            .find(|t| !error.message().contains(&format!("{} spent", t.step)))
        {
            return Err(Finding::new(
                "a sum past the ceiling is refused naming each step that spent",
                || {
                    format!(
                        "operation {at}: {term:?} not in {:?}; {case:?}",
                        error.message()
                    )
                },
            ));
        }
    }
    Ok(())
}

/// Runs the operations and holds each answer to the model and the laws; the states that held.
fn run(case: &Case) -> Result<Vec<Bounds>, Finding> {
    let mut states = vec![Bounds::new(case.step)];
    let mut model = Model::new();
    for (at, op) in case.ops.iter().enumerate() {
        let Some(before) = states.last() else {
            break;
        };
        let history = History::new(op.records.iter().cloned(), NameSet::default(), []);
        let other = History::new(op.records.iter().rev().cloned(), NameSet::default(), []);
        let (Ok(history), Ok(other)) = (history, other) else {
            return Err(Finding::new("every case's records make a history", || {
                format!("operation {at}: {case:?}")
            }));
        };
        let got = before.after(&op.path, &history, &op.own, op.limits);
        let reversed: Vec<(NameId, Steps)> = op.own.iter().rev().copied().collect();
        if before.after(&op.path, &other, &reversed, op.limits) != got {
            return Err(Finding::new(
                "the order of reporting changes nothing",
                || format!("operation {at}: {got:?}; {case:?}"),
            ));
        }
        match (want(&model, op, case.step), got) {
            (Want::Held(next), Ok(got)) => {
                held(case, at, &got, &next)?;
                kept(case, at, before, &got)?;
                largest(case, &got)?;
                model = next;
                states.push(got);
            }
            (Want::Refused(faults, terms), Err(error)) => {
                refused(case, at, &error, &faults, &terms)?;
            }
            (Want::Held(_), Err(error)) => {
                return Err(Finding::new(
                    "an operation with no fault is never refused",
                    || format!("operation {at}: {error:?}; {case:?}"),
                ));
            }
            (Want::Refused(faults, _), Ok(got)) => {
                return Err(Finding::new(
                    "an operation with a fault is refused, and a sum past the ceiling never wraps",
                    || format!("operation {at}: faults {faults:?}, got {got:?}; {case:?}"),
                ));
            }
        }
    }
    Ok(states)
}

/// `joined` is `a` and `b` together: each entity with the larger bound and its operand's terms,
/// the first operand's on a tie.
fn together(case: &Case, a: &Bounds, b: &Bounds, joined: &Bounds) -> Result<(), Finding> {
    for entity in every() {
        let (x, y) = (a.of(entity), b.of(entity));
        let terms = if y > x {
            b.terms(entity)
        } else {
            a.terms(entity)
        };
        if joined.of(entity) != x.max(y) || joined.terms(entity) != terms {
            return Err(Finding::new(
                "a join keeps the larger bound, with its terms",
                || {
                    format!(
                        "entity {entity}: {x:?} and {y:?}, got {:?} of {:?}; {case:?}",
                        joined.of(entity),
                        joined.terms(entity)
                    )
                },
            ));
        }
    }
    let union: Vec<NameId> = every()
        .filter(|e| a.of(*e) > Steps(0) || b.of(*e) > Steps(0))
        .collect();
    if joined.entities().collect::<Vec<_>>() != union
        || joined.largest() != a.largest().max(b.largest())
    {
        return Err(Finding::new(
            "a join holds every entity of both, and the larger largest bound",
            || format!("{joined:?}; {case:?}"),
        ));
    }
    largest(case, joined)
}

/// The last state joined with the partner both ways, and with no bounds on the other grid step.
fn joins(case: &Case, states: &[Bounds]) -> Result<(), Finding> {
    let op = OpPath::new(OpId::from_bytes(JOIN));
    let (Some(last), Some(partner)) = (
        states.last(),
        states.get(modulo(usize::from(case.partner), states.len())),
    ) else {
        return Ok(());
    };
    match (last.join(&op, partner), partner.join(&op, last)) {
        (Ok(one), Ok(two)) => {
            together(case, last, partner, &one)?;
            together(case, partner, last, &two)?;
        }
        (one, two) => {
            return Err(Finding::new("bounds on one grid step join", || {
                format!("{one:?}, {two:?}; {case:?}")
            }));
        }
    }
    let empty = Bounds::new(case.other);
    for got in [last.join(&op, &empty), empty.join(&op, last)] {
        let fine = match &got {
            Ok(joined) => case.other == case.step && joined == last,
            Err(error) => {
                case.other != case.step
                    && *error.detail() == OpErrorDetail::Bound(BoundDetail::MixedSteps)
                    && error.kind() == ErrorKind::InvalidInput
                    && error.op() == &op
            }
        };
        if !fine {
            return Err(Finding::new(
                "a join with nothing changes nothing, and two grid steps are refused MixedSteps",
                || format!("{got:?}; {case:?}"),
            ));
        }
    }
    Ok(())
}

/// Holds the case to the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let states = run(case)?;
    joins(case, &states)
}
