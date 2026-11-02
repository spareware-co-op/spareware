//! Fuzz target: `geom-name`'s attribute step gives each output exactly the values section 8.4's
//! table gives, and setting a value changes no entity.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two attributes with arbitrary
//! behaviours (one opaque, of an unknown owner; one a point, which a transform may move), values on
//! up to [`MAX_NAMES`] input names, a history of up to [`MAX_RECORDS`] outputs of every origin and
//! some deleted inputs, and the step's transform and copy role. [`check`] runs the step and
//! compares every output's value with 8.4's table, written here from the table; it checks that a
//! note is given exactly for each value a merge or a transform dropped; and it sets a value with
//! `geom_name::set` and checks that applying that operation's own history changes nothing.
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case whose
//!   history `History` accepts.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against section 8.4's table read independently
//! (`projects/engines/geometry/architecture/08-topology.md`), over histories of chapter 20.3's
//! records (`projects/engines/geometry/architecture/20-naming-history.md`).
//!
//! Cost: at most [`MAX_RECORDS`] records, two attributes and [`MAX_NAMES`] values each; at most
//! [`MAX_INPUT_BYTES`] bytes read.

use std::collections::{BTreeMap, BTreeSet};

use geom_name::{Mover, Note, Step, apply, set};
use geom_op::{History, NameId, NameSet, Origin, PieceKey, Record, Role, Side};
use geom_test::{ByteSource, Entropy};
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, TransformKind, ValueSchema,
};
use geom_topo::order::NameKey;

use crate::Finding;

/// The most input names that carry values, and the names records draw inputs from.
pub const MAX_NAMES: u8 = 12;

/// The most records a case's history holds.
pub const MAX_RECORDS: u8 = 12;

/// The most bytes [`decode`] reads from any input: 1 + 2 · 6 + 2 · 12 · 2 + 1 + 12 · 5 + 1 + 1 = 124.
///
/// The transform byte; each attribute's six behaviour bytes; each attribute's value bytes, a name
/// and a value per input name, at most [`MAX_NAMES`] each; the record count; each record's origin
/// byte, output byte and three input bytes; the deleted byte; the copy byte. The test
/// `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = 1 + 2 * 6 + 2 * 12 * 2 + 1 + 12 * 5 + 1 + 1;

const _: () = assert!(MAX_INPUT_BYTES == 124);

/// One case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The two attributes' behaviours: opaque (unknown owner), then point.
    pub behaviours: [Behaviour; 2],
    /// Each attribute's values, as (input name, value byte).
    pub values: [Vec<(u8, u8)>; 2],
    /// The history's records, as (output name, origin).
    pub records: Vec<(u8, Origin)>,
    /// The deleted inputs.
    pub deleted: Vec<u8>,
    /// The transform, if the operation is one.
    pub transform: Option<TransformKind>,
    /// Whether the `instance` role is declared a copy.
    pub copies: bool,
}

/// The name of row `i`.
fn n(i: u8) -> NameId {
    NameId::from_bytes(u128::from(i).to_be_bytes())
}

fn key(i: NameId) -> NameKey {
    NameKey(i.to_bytes())
}

fn instance() -> Option<Role> {
    Role::new("instance", []).ok()
}

fn side() -> Option<Role> {
    Role::new("side", []).ok()
}

/// A behaviour from six bytes; transforms only where `movable`.
fn behaviour(src: &mut ByteSource<'_>, movable: bool) -> Behaviour {
    let mut t = || match src.byte() % 3 {
        0 => TransformAction::Keep,
        1 if movable => TransformAction::Transform,
        _ => TransformAction::Delete,
    };
    let transformed = OnTransform {
        moved: t(),
        rotated: t(),
        mirrored: t(),
        scaled: t(),
    };
    let b = src.byte();
    let m = src.byte();
    Behaviour {
        changed: if b % 2 == 0 {
            ChangeAction::Keep
        } else {
            ChangeAction::Delete
        },
        transformed,
        split: if b & 2 == 0 {
            SplitAction::Propagate
        } else {
            SplitAction::Delete
        },
        merged: match m % 3 {
            0 => MergeAction::KeepIfEqual,
            1 => MergeAction::Combine,
            _ => MergeAction::Delete,
        },
        copied: if b & 4 == 0 {
            CopyAction::Copy
        } else {
            CopyAction::Delete
        },
    }
}

/// A set of up to three input names from three bytes (fewer when they repeat).
fn inputs(src: &mut ByteSource<'_>) -> BTreeSet<u8> {
    (0..3).map(|_| src.byte() % MAX_NAMES).collect()
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let t = src.byte();
    let transform = TransformKind::ALL.get(usize::from(t % 6)).copied();
    let behaviours = [behaviour(&mut src, false), behaviour(&mut src, true)];
    let mut values: [Vec<(u8, u8)>; 2] = [Vec::new(), Vec::new()];
    for list in &mut values {
        for name in 0..MAX_NAMES {
            let present = src.byte();
            if present % 2 == 0 {
                list.push((name, src.byte() % 4));
            }
        }
    }
    let count = src.byte() % (MAX_RECORDS + 1);
    let mut records = Vec::new();
    let mut outputs = BTreeSet::new();
    for _ in 0..count {
        let kind = src.byte();
        let output = src.byte() % (2 * MAX_NAMES);
        let ins = inputs(&mut src);
        let first = ins.iter().next().copied().unwrap_or(0);
        let set = NameSet::new(ins.iter().map(|&i| n(i))).ok();
        let one = NameSet::new([n(first)]).ok();
        let origin = match (kind % 7, set, one) {
            (0, _, _) => Some(Origin::Unchanged),
            (1, _, Some(one)) => Some(Origin::Modified { from: one }),
            (2, Some(set), _) => Some(Origin::Modified { from: set }),
            (3, Some(tools), _) => Some(Origin::Split {
                of: n(first),
                piece: PieceKey::ByTools {
                    tools,
                    side: if kind & 8 == 0 {
                        Side::Front
                    } else {
                        Side::Back
                    },
                },
            }),
            (4, Some(set), _) if set.names().len() >= 2 => Some(Origin::Merged { of: set }),
            (5, _, Some(one)) => instance().map(|role| Origin::Generated { from: one, role }),
            (_, Some(set), _) => side().map(|role| Origin::Generated { from: set, role }),
            _ => None,
        };
        if let Some(origin) = origin
            && outputs.insert(output)
        {
            records.push((output, origin));
        }
    }
    let d = src.byte();
    let bits = [1u8, 2, 4, 8, 16, 32, 64, 128];
    let deleted = (0..8u8)
        .zip(bits)
        .filter(|&(_, bit)| d & bit != 0)
        .map(|(i, _)| i)
        .collect();
    let copies = src.byte() % 2 == 0;
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            behaviours,
            values,
            records,
            deleted,
            transform: transform.filter(|_| t % 6 < 4),
            copies,
        },
        read,
    )
}

/// The harness every fuzzing engine drives: decode `data` into a case, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// Moves a value by adding one to every byte, refusing a value of byte 3: so moves, and refusals,
/// both come up.
struct AddOne;

impl Mover for AddOne {
    fn moved(&self, _: &ValueSchema, value: &Value) -> Option<Value> {
        if value.0.contains(&3) {
            None
        } else {
            Some(Value(value.0.iter().map(|b| b.wrapping_add(1)).collect()))
        }
    }
}

/// The step as `geom-name` gives it.
pub type Applied = Result<(Attributes, Vec<Note>), geom_name::AttrError>;

/// Runs the step and checks the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, |defs, before, history, step| {
        apply(defs, before, history, step)
    })
}

/// [`check`] with `step` in place of `geom_name::apply`: how a test shows that a broken step is
/// caught.
///
/// # Errors
///
/// As for [`check`].
pub fn check_with<S>(case: &Case, step: S) -> Result<(), Finding>
where
    S: Fn(&Definitions, &Attributes, &History, &Step<'_>) -> Applied,
{
    let attrs = attributes();
    let (defs, before, history) = setup(case, &attrs)?;
    let copies: Vec<Role> = if case.copies {
        instance().into_iter().collect()
    } else {
        Vec::new()
    };
    let s = Step {
        transform: case.transform.map(|k| (k, &AddOne as &dyn Mover)),
        copies: &copies,
    };
    let (after, notes) = step(&defs, &before, &history, &s).map_err(|e| {
        Finding::new("every value has its definition, so the step runs", || {
            format!("{e}; case {case:?}")
        })
    })?;
    let mut want_notes = Vec::new();
    for ((a, schema), b) in attrs.iter().zip(case.behaviours) {
        let was: BTreeMap<NameKey, Carried> = before
            .iter()
            .filter(|(x, _, _)| *x == a)
            .map(|(_, k, c)| (*k, c.clone()))
            .collect();
        let got: BTreeMap<NameKey, Carried> = after
            .iter()
            .filter(|(x, _, _)| *x == a)
            .map(|(_, k, c)| (*k, c.clone()))
            .collect();
        let mut want = BTreeMap::new();
        for record in history.records() {
            if let Some(c) = table(a, schema, b, &was, record, &s, &mut want_notes) {
                want.insert(key(record.output), c);
            }
        }
        if got != want {
            return Err(Finding::new(
                "each output carries exactly what section 8.4's table gives",
                || format!("{}: got {got:?}, want {want:?}; case {case:?}", a.name),
            ));
        }
    }
    want_notes.sort();
    if notes != want_notes {
        return Err(Finding::new(
            "a note is given exactly for each value a merge or a transform dropped",
            || format!("got {notes:?}, want {want_notes:?}; case {case:?}"),
        ));
    }
    // Setting a value is an operation whose own history changes nothing. The body is every name a
    // record can output, so it holds every value the step gave.
    let body = NameSet::new((0..2 * MAX_NAMES).map(n))
        .map_err(|e| Finding::new("the body's names are a set", || format!("{e}")))?;
    let Some((a, _)) = attrs.first() else {
        return Ok(());
    };
    let (with, own) = set(
        &defs,
        &after,
        &body,
        n(0),
        EntityKind::Face,
        a,
        Some(Value(vec![9])),
    )
    .map_err(|e| {
        Finding::new(
            "setting a defined value on a face of the body works",
            || format!("{e}"),
        )
    })?;
    let again = step(&defs, &with, &own, &Step::default());
    if again != Ok((with.clone(), Vec::new())) {
        return Err(Finding::new(
            "setting a value is an operation that changes no entity",
            || format!("case {case:?}"),
        ));
    }
    Ok(())
}

/// The case's two attributes: opaque, of an unknown owner; and a point.
fn attributes() -> [(AttrId, ValueSchema); 2] {
    [
        (
            AttrId {
                owner: "org.unknown".into(),
                name: "data".into(),
                version: 1,
            },
            ValueSchema::Opaque { id: "?".into() },
        ),
        (
            AttrId {
                owner: "kit.geom".into(),
                name: "datum".into(),
                version: 1,
            },
            ValueSchema::Point,
        ),
    ]
}

/// The case's definitions, input attributes and history.
fn setup(
    case: &Case,
    attrs: &[(AttrId, ValueSchema); 2],
) -> Result<(Definitions, Attributes, History), Finding> {
    let mut defs = Definitions::new();
    let mut before = Attributes::new();
    for ((a, schema), (b, values)) in attrs.iter().zip(case.behaviours.iter().zip(&case.values)) {
        let def = Definition::new(a.clone(), [EntityKind::Face], schema.clone(), *b)
            .map_err(|e| Finding::new("the harness's definitions are valid", || format!("{e}")))?;
        defs = defs
            .define(def)
            .map_err(|e| Finding::new("the harness's definitions are new", || format!("{e}")))?;
        before = before.defined_as(a.clone(), b.class());
        let held: Vec<(AttrId, NameKey, Carried)> = values
            .iter()
            .map(|&(name, v)| (a.clone(), key(n(name)), Carried::One(Value(vec![v, name]))))
            .collect();
        let mut all: Vec<(AttrId, NameKey, Carried)> = before
            .iter()
            .map(|(x, k, c)| (x.clone(), *k, c.clone()))
            .collect();
        all.extend(held);
        before = before.rebuilt(all);
    }
    let records = case.records.iter().map(|(o, origin)| Record {
        output: n(*o),
        origin: origin.clone(),
    });
    let deleted = NameSet::new(case.deleted.iter().map(|&i| n(i)))
        .map_err(|e| Finding::new("the harness's deleted names are a set", || format!("{e}")))?;
    let history = History::new(records, deleted, []).map_err(|e| {
        Finding::new("the harness makes histories History accepts", || {
            format!("{e}")
        })
    })?;
    Ok((defs, before, history))
}

/// What one output carries for one attribute by section 8.4's table, with the notes it gives.
fn table(
    attr: &AttrId,
    schema: &ValueSchema,
    b: Behaviour,
    before: &BTreeMap<NameKey, Carried>,
    record: &Record,
    step: &Step<'_>,
    notes: &mut Vec<Note>,
) -> Option<Carried> {
    let of = |x: &NameId| before.get(&key(*x)).cloned();
    let merge = |inputs: &[NameId], notes: &mut Vec<Note>| -> Option<Carried> {
        let carried: Vec<(NameKey, Carried)> = inputs
            .iter()
            .filter_map(|x| of(x).map(|c| (key(*x), c)))
            .collect();
        let first = carried.first()?.1.clone();
        match b.merged {
            MergeAction::Delete => None,
            MergeAction::KeepIfEqual => {
                if carried.len() == inputs.len() && carried.iter().all(|(_, c)| *c == first) {
                    Some(first)
                } else {
                    notes.push(Note::Dropped {
                        attr: attr.clone(),
                        into: record.output,
                    });
                    None
                }
            }
            MergeAction::Combine => {
                let mut each = BTreeMap::new();
                for (x, c) in carried {
                    match c {
                        Carried::One(v) => {
                            each.insert(x, v);
                        }
                        Carried::Combined(inner) => each.extend(inner),
                    }
                }
                Some(Carried::Combined(each))
            }
        }
    };
    match &record.origin {
        Origin::Unchanged => of(&record.output),
        Origin::Modified { from } if from.names().len() == 1 => {
            let input = from.names().first()?;
            match step.transform {
                None => match b.changed {
                    ChangeAction::Keep => of(input),
                    ChangeAction::Delete => None,
                },
                Some((kind, mover)) => match b.transformed.get(kind) {
                    TransformAction::Keep => of(input),
                    TransformAction::Delete => None,
                    TransformAction::Transform => {
                        let result = match of(input)? {
                            Carried::One(v) => mover.moved(schema, &v).map(Carried::One),
                            Carried::Combined(each) => each
                                .into_iter()
                                .map(|(x, v)| mover.moved(schema, &v).map(|m| (x, m)))
                                .collect::<Option<BTreeMap<_, _>>>()
                                .map(Carried::Combined),
                        };
                        if result.is_none() {
                            notes.push(Note::NotMoved {
                                attr: attr.clone(),
                                entity: record.output,
                            });
                        }
                        result
                    }
                },
            }
        }
        Origin::Modified { from } => merge(from.names(), notes),
        Origin::Merged { of: inputs } => merge(inputs.names(), notes),
        Origin::Split { of: input, .. } => match b.split {
            SplitAction::Propagate => of(input),
            SplitAction::Delete => None,
        },
        Origin::Generated { from, role } => match from.names() {
            [one] if step.copies.contains(role) => match b.copied {
                CopyAction::Copy => of(one),
                CopyAction::Delete => None,
            },
            _ => None,
        },
    }
}
