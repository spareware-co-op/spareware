//! The one shared attribute step: an operation's output attributes from its input attributes, the
//! definitions and its history (8.4); and setting a value as an operation of its own.
//!
//! What it does: [`apply`] reads each output's [`Record`] in an operation's [`History`] as one of
//! section 8.4's events and gives the output the values the definitions' behaviours say:
//!
//! - **Unchanged** (the input passed through): every value stays.
//! - **Changed** (modified from one input, keeping its name): kept or deleted, by
//!   [`ChangeAction`]; or, when the operation is a transform, **Transformed**: kept, moved with the
//!   entity by the caller's [`Mover`] (only for a schema the kernel knows how to move), or
//!   deleted, by the [`TransformAction`] for that kind of transform.
//! - **Split** from one input: every piece gets the input's value, or none.
//! - **Merged** from several inputs (a `Merged` record, or a `Modified` record from more than one
//!   input): the value is kept only when every input carries an equal one (otherwise dropped, with
//!   a [`Note`]), each input's value is carried under its name, or none.
//! - **Copied** (generated from one input in a role the operation's schema declares a copy): the
//!   copy carries the input's value, or none.
//! - **Generated** otherwise, and **Deleted**: no value; an operation that sets a value on a new
//!   entity (a fillet's continuity record) sets it afterwards, as its schema says.
//!
//! [`set`] sets, changes or removes one value as an operation of its own: the new attributes, and a
//! history in which every named entity of the body is unchanged (8.4, "setting one is an
//! operation"), so every replica agrees.
//!
//! Guarantees:
//!
//! - **Pure, and one step for every operation.** The output depends only on the definitions, the
//!   input attributes, the history and the [`Step`] (the transform and the copy roles the
//!   operation's schema gives); no operation writes attribute code of its own, and nothing calls
//!   back into the operation (8.4, "no callbacks"): a [`Mover`] only moves values the kernel knows,
//!   by section 7's rules.
//! - **Values of an unknown owner are carried.** A value whose schema this program does not read
//!   ([`ValueSchema::Opaque`]) follows its definition's behaviour like any other: kept, split,
//!   merged, copied or deleted, never moved and never dropped for being unknown.
//! - **Every value has a definition.** An input value whose attribute has no definition is
//!   [`AttrError::Undefined`], never carried without a behaviour.
//! - **Ordered.** The output's values are in attribute, then name order, and the notes in record
//!   order, the same on every machine whatever order the history was reported in (8.2, 21).
//!
//! Method: section 8.4's events read from section 20.3's history records
//! (`projects/engines/geometry/architecture/08-topology.md`,
//! `projects/engines/geometry/architecture/20-naming-history.md`), after Parasolid's attribute
//! classes, taken as a concept only (`projects/kit/notes/G01-evidence/07-persistent-naming.md`,
//! section 5).
//!
//! Cost: `O(r · a · log v)` for `r` records, `a` attributes and `v` values per attribute; a merge
//! compares its inputs' values' bytes. Nothing recurses.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_op::{History, HistoryError, NameError, NameId, NameSet, Origin, Record, Role};
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    ChangeAction, CopyAction, Definition, Definitions, EntityKind, TransformAction, TransformKind,
    ValueSchema,
};
use geom_topo::order::NameKey;

/// Moves a value with its entity under a transform, by section 7's rules: what a transform
/// operation gives the attribute step, for the schemas the kernel knows how to move.
pub trait Mover {
    /// The value of `schema` moved; `None` when it cannot be moved (its bytes are not a value of
    /// that schema), which drops it with a [`Note::NotMoved`].
    fn moved(&self, schema: &ValueSchema, value: &Value) -> Option<Value>;
}

/// What the operation's schema tells the step: the transform it applies, if it is one, and the
/// roles it declares copies (8.4's Transformed and Copied events).
#[derive(Clone, Copy, Default)]
pub struct Step<'a> {
    /// The kind of transform and its mover, for a transform operation.
    pub transform: Option<(TransformKind, &'a dyn Mover)>,
    /// The roles of outputs that are copies of their one input.
    pub copies: &'a [Role],
}

impl fmt::Debug for Step<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Step")
            .field("transform", &self.transform.map(|(kind, _)| kind))
            .field("copies", &self.copies)
            .finish()
    }
}

/// Something the step reports without failing: a value dropped by its class.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Note {
    /// A merge's inputs did not all carry equal values under [`MergeAction::KeepIfEqual`].
    Dropped {
        /// The attribute.
        attr: AttrId,
        /// The merged output.
        into: NameId,
    },
    /// A transform could not move the value; it was dropped.
    NotMoved {
        /// The attribute.
        attr: AttrId,
        /// The output.
        entity: NameId,
    },
}

/// Why the attribute step or a set failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrError {
    /// A value's attribute has no definition.
    Undefined(AttrId),
    /// The attribute may not be carried by this kind of entity.
    WrongKind {
        /// The attribute.
        attr: AttrId,
        /// The entity's kind.
        kind: EntityKind,
    },
    /// The attributes hold a split or merge class for the attribute other than its definition's.
    ClassMismatch(AttrId),
    /// The entity set is not among the body's names.
    NotInBody(NameId),
    /// The history could not be made (a name list with a repeated name).
    History(HistoryError),
    /// The body's names could not be made into a set.
    Names(NameError),
}

impl fmt::Display for AttrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttrError::Undefined(a) => write!(
                f,
                "{}/{} version {} has no definition",
                a.owner, a.name, a.version
            ),
            AttrError::WrongKind { attr, kind } => {
                write!(f, "a {kind:?} may not carry {}/{}", attr.owner, attr.name)
            }
            AttrError::ClassMismatch(a) => write!(
                f,
                "{}/{} version {} is held with a class its definition does not give",
                a.owner, a.name, a.version
            ),
            AttrError::NotInBody(n) => write!(f, "{n} is not a named entity of the body"),
            AttrError::History(e) => write!(f, "{e}"),
            AttrError::Names(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for AttrError {}

/// A name as the attribute store keys it.
fn key(name: NameId) -> NameKey {
    NameKey(name.to_bytes())
}

/// The event a record is, for one attribute's behaviour.
enum Event<'r> {
    /// The value of `of` stays.
    Stays(NameId),
    /// One input, keeping its name: changed or transformed.
    Changed(NameId),
    /// One input split.
    Split(NameId),
    /// Several inputs merged.
    Merged(&'r [NameId]),
    /// One input copied.
    Copied(NameId),
    /// No value.
    Nothing,
}

/// The event `record` is, given the operation's copy roles.
fn event<'r>(record: &'r Record, copies: &[Role]) -> Event<'r> {
    match &record.origin {
        Origin::Unchanged => Event::Stays(record.output),
        Origin::Modified { from } => match from.names() {
            [one] => Event::Changed(*one),
            many => Event::Merged(many),
        },
        Origin::Split { of, .. } => Event::Split(*of),
        Origin::Merged { of } => Event::Merged(of.names()),
        Origin::Generated { from, role } => match from.names() {
            [one] if copies.contains(role) => Event::Copied(*one),
            _ => Event::Nothing,
        },
    }
}

/// What one output carries for one attribute, by its definition and the event.
fn carried(
    def: &Definition,
    values: &BTreeMap<NameKey, Carried>,
    output: NameId,
    event: &Event<'_>,
    step: &Step<'_>,
    notes: &mut Vec<Note>,
) -> Option<Carried> {
    let b = def.behaviour();
    let of = |n: NameId| values.get(&key(n)).cloned();
    match *event {
        Event::Stays(n) => of(n),
        Event::Changed(n) => match step.transform {
            None => match b.changed {
                ChangeAction::Keep => of(n),
                ChangeAction::Delete => None,
            },
            Some((kind, mover)) => match b.transformed.get(kind) {
                TransformAction::Keep => of(n),
                TransformAction::Delete => None,
                TransformAction::Transform => {
                    let value = of(n)?;
                    let result = moved(def.schema(), &value, mover);
                    if result.is_none() {
                        notes.push(Note::NotMoved {
                            attr: def.attr().clone(),
                            entity: output,
                        });
                    }
                    result
                }
            },
        },
        Event::Split(n) => match b.split {
            SplitAction::Propagate => of(n),
            SplitAction::Delete => None,
        },
        Event::Copied(n) => match b.copied {
            CopyAction::Copy => of(n),
            CopyAction::Delete => None,
        },
        Event::Merged(inputs) => merged(def, values, inputs, output, notes),
        Event::Nothing => None,
    }
}

/// A value moved by `mover`, every value of a combined one moved; `None` if any cannot be.
fn moved(schema: &ValueSchema, value: &Carried, mover: &dyn Mover) -> Option<Carried> {
    match value {
        Carried::One(v) => mover.moved(schema, v).map(Carried::One),
        Carried::Combined(each) => {
            let mut out = BTreeMap::new();
            for (n, v) in each {
                out.insert(*n, mover.moved(schema, v)?);
            }
            Some(Carried::Combined(out))
        }
    }
}

/// What a merge of `inputs` into `output` carries for one attribute.
fn merged(
    def: &Definition,
    values: &BTreeMap<NameKey, Carried>,
    inputs: &[NameId],
    output: NameId,
    notes: &mut Vec<Note>,
) -> Option<Carried> {
    let present: Vec<(NameKey, &Carried)> = inputs
        .iter()
        .filter_map(|&n| values.get(&key(n)).map(|c| (key(n), c)))
        .collect();
    let (_, first) = *present.first()?;
    match def.behaviour().merged {
        MergeAction::Delete => None,
        MergeAction::KeepIfEqual => {
            if present.len() == inputs.len() && present.iter().all(|(_, c)| *c == first) {
                Some(first.clone())
            } else {
                notes.push(Note::Dropped {
                    attr: def.attr().clone(),
                    into: output,
                });
                None
            }
        }
        MergeAction::Combine => {
            let mut each = BTreeMap::new();
            for (n, c) in present {
                match c {
                    Carried::One(v) => {
                        each.insert(n, v.clone());
                    }
                    Carried::Combined(inner) => {
                        each.extend(inner.iter().map(|(k, v)| (*k, v.clone())));
                    }
                }
            }
            Some(Carried::Combined(each))
        }
    }
}

/// The operation's output attributes from its input attributes `before`, the definitions `defs`
/// and its `history`, by the module's events, with the notes of values dropped.
///
/// # Errors
///
/// [`AttrError::Undefined`] for the first attribute, in order, that `before` carries values of and
/// `defs` does not define; [`AttrError::ClassMismatch`] for the first whose class in `before` is not
/// its definition's.
pub fn apply(
    defs: &Definitions,
    before: &Attributes,
    history: &History,
    step: &Step<'_>,
) -> Result<(Attributes, Vec<Note>), AttrError> {
    let mut by_attr: BTreeMap<&AttrId, BTreeMap<NameKey, Carried>> = BTreeMap::new();
    for (attr, name, value) in before.iter() {
        by_attr
            .entry(attr)
            .or_default()
            .insert(*name, value.clone());
    }
    let mut notes = Vec::new();
    let mut out = Vec::new();
    for (attr, values) in &by_attr {
        let def = defined(defs, before, attr)?;
        for record in history.records() {
            let ev = event(record, step.copies);
            if let Some(c) = carried(def, values, record.output, &ev, step, &mut notes) {
                out.push(((*attr).clone(), key(record.output), c));
            }
        }
    }
    notes.sort();
    Ok((before.rebuilt(out), notes))
}

/// The definition of `attr`, which `before` must hold with the definition's class.
fn defined<'d>(
    defs: &'d Definitions,
    before: &Attributes,
    attr: &AttrId,
) -> Result<&'d Definition, AttrError> {
    let def = defs
        .get(attr)
        .ok_or_else(|| AttrError::Undefined(attr.clone()))?;
    match before.class(attr) {
        Some(class) if class != def.behaviour().class() => {
            Err(AttrError::ClassMismatch(attr.clone()))
        }
        _ => Ok(def),
    }
}

/// Sets `entity`'s value for `attr` to `value`, or removes it for `None`, as an operation of its
/// own: the new attributes, and the operation's history, in which every name in `names` (every named
/// entity of the body, and the body) is unchanged.
///
/// # Errors
///
/// [`AttrError::Undefined`] if `attr` has no definition; [`AttrError::ClassMismatch`] if `before`
/// holds it with another class; [`AttrError::WrongKind`] if `kind` may not carry it; [`AttrError::NotInBody`] if `entity` is not in `names`; [`AttrError::History`] if the
/// names repeat.
pub fn set(
    defs: &Definitions,
    before: &Attributes,
    names: &NameSet,
    entity: NameId,
    kind: EntityKind,
    attr: &AttrId,
    value: Option<Value>,
) -> Result<(Attributes, History), AttrError> {
    let def = defined(defs, before, attr)?;
    if !def.allows(kind) {
        return Err(AttrError::WrongKind {
            attr: attr.clone(),
            kind,
        });
    }
    if names.names().binary_search(&entity).is_err() {
        return Err(AttrError::NotInBody(entity));
    }
    let after = match value {
        Some(v) => {
            let mut values: Vec<(AttrId, NameKey, Carried)> = before
                .iter()
                .filter(|(a, n, _)| !(*a == attr && **n == key(entity)))
                .map(|(a, n, c)| (a.clone(), *n, c.clone()))
                .collect();
            values.push((attr.clone(), key(entity), Carried::One(v)));
            before
                .defined_as(attr.clone(), def.behaviour().class())
                .rebuilt(values)
        }
        None => before.remove(attr, key(entity)),
    };
    let records = names.names().iter().map(|&output| Record {
        output,
        origin: Origin::Unchanged,
    });
    let empty = NameSet::new([]).map_err(AttrError::Names)?;
    let history = History::new(records, empty, []).map_err(AttrError::History)?;
    Ok((after, history))
}
