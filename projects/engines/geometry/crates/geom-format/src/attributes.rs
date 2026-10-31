//! The attributes record: every attribute definition a model holds, and every value keyed by the
//! persistent name of the entity that carries it, readable by a program that does not know the
//! attribute's owner.
//!
//! What it does: [`attributes_record`] writes a model's [`Definitions`] and its [`Attributes`] as
//! one required record (tag [`ATTRIBUTES_TAG`]): each definition with its owner, name and
//! version, the kinds of entity that may carry it, its value schema and its behaviour at section
//! 8.4's events; then each attribute's values by name, a value as the owner's opaque bytes, or the
//! values a merge combined. [`read_attributes`] reads them back: definitions, and attributes whose
//! classes come from their definitions, so `geom-name`'s attribute step can carry the values
//! through later edits without ever decoding them.
//!
//! Guarantees:
//!
//! - **No owner needed** (24.4; piece G95): a value is the owner's bytes, kept as they are; what a
//!   value does at each event is in its definition, which every reader reads.
//! - **Every value has its definition**: a value whose attribute the record does not define is
//!   refused, and a definition is checked as `Definition::new` and `Definitions::define` check it.
//! - **One encoding**: the record is read and encoded again, and the bytes must be the stored ones.
//!
//! Method: chapter 24.4's attributes record and section 8.4's events and actions
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`,
//! `projects/engines/geometry/architecture/08-topology.md`), after Parasolid's published attribute
//! classes, as a concept only (`projects/kit/notes/G01-evidence/07-persistent-naming.md`).
//!
//! Cost: linear in the record; values are copied once.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder};
use geom_run::WorkMeter;
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, DefinitionError, Definitions, EntityKind,
    OnTransform, TransformAction, ValueSchema,
};
use geom_topo::order::NameKey;

use crate::payload::{PayloadError, Reader};
use crate::record::{REQUIRED, Record};

/// The attributes record's tag (REGISTRY.md).
pub const ATTRIBUTES_TAG: u64 = 19;

/// The attributes record's schema.
pub const ATTRIBUTES_SCHEMA: u64 = 1;

/// Why an attributes record cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributesError {
    /// The payload's shape or encoding is wrong.
    Payload(PayloadError),
    /// A definition the definition rules refuse, or one defined twice otherwise.
    Definition(DefinitionError),
    /// A value of an attribute the record does not define.
    Undefined(AttrId),
    /// The payload is not the attributes' one canonical encoding.
    NotCanonical,
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
}

impl fmt::Display for AttributesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributesError::Payload(e) => write!(f, "not an attributes record: {e}"),
            AttributesError::Definition(e) => write!(f, "not a definition: {e}"),
            AttributesError::Undefined(attr) => write!(
                f,
                "a value of {}/{} version {}, which the record does not define",
                attr.owner, attr.name, attr.version
            ),
            AttributesError::NotCanonical => {
                f.write_str("the attributes are not in their canonical encoding")
            }
            AttributesError::Encode(e) => write!(f, "the attributes could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for AttributesError {}

impl From<PayloadError> for AttributesError {
    fn from(e: PayloadError) -> Self {
        AttributesError::Payload(e)
    }
}

impl From<EncodeError> for AttributesError {
    fn from(e: EncodeError) -> Self {
        AttributesError::Encode(e)
    }
}

impl From<DefinitionError> for AttributesError {
    fn from(e: DefinitionError) -> Self {
        AttributesError::Definition(e)
    }
}

/// A length as CBOR counts it.
fn count(n: usize) -> Result<u64, EncodeError> {
    u64::try_from(n).map_err(|_| EncodeError::TooLong)
}

const fn kind_code(kind: EntityKind) -> u64 {
    match kind {
        EntityKind::Body => 0,
        EntityKind::Region => 1,
        EntityKind::Face => 2,
        EntityKind::Edge => 3,
        EntityKind::Vertex => 4,
    }
}

const fn transform_code(action: TransformAction) -> u64 {
    match action {
        TransformAction::Keep => 0,
        TransformAction::Transform => 1,
        TransformAction::Delete => 2,
    }
}

fn attr_id(e: &mut Encoder, attr: &AttrId) -> Result<(), EncodeError> {
    e.array(3)?;
    e.text(&attr.owner)?;
    e.text(&attr.name)?;
    e.uint(u64::from(attr.version))
}

fn definition(e: &mut Encoder, d: &Definition) -> Result<(), EncodeError> {
    e.array(4)?;
    attr_id(e, d.attr())?;
    // In `EntityKind`'s order, as `Definition::kinds` gives them.
    let kinds: Vec<EntityKind> = [
        EntityKind::Body,
        EntityKind::Region,
        EntityKind::Face,
        EntityKind::Edge,
        EntityKind::Vertex,
    ]
    .into_iter()
    .filter(|&kind| d.allows(kind))
    .collect();
    e.array(count(kinds.len())?)?;
    for kind in kinds {
        e.uint(kind_code(kind))?;
    }
    match d.schema() {
        ValueSchema::Opaque { id } => {
            e.array(2)?;
            e.uint(0)?;
            e.text(id)?;
        }
        other => {
            e.array(1)?;
            e.uint(match other {
                ValueSchema::Point => 1,
                ValueSchema::Direction => 2,
                ValueSchema::Frame => 3,
                ValueSchema::Length => 4,
                _ => 5,
            })?;
        }
    }
    let b = d.behaviour();
    e.array(5)?;
    e.uint(match b.changed {
        ChangeAction::Keep => 0,
        ChangeAction::Delete => 1,
    })?;
    e.array(4)?;
    for action in [
        b.transformed.moved,
        b.transformed.rotated,
        b.transformed.mirrored,
        b.transformed.scaled,
    ] {
        e.uint(transform_code(action))?;
    }
    e.uint(match b.split {
        SplitAction::Propagate => 0,
        SplitAction::Delete => 1,
    })?;
    e.uint(match b.merged {
        MergeAction::KeepIfEqual => 0,
        MergeAction::Combine => 1,
        MergeAction::Delete => 2,
    })?;
    e.uint(match b.copied {
        CopyAction::Copy => 0,
        CopyAction::Delete => 1,
    })
}

fn carried(e: &mut Encoder, c: &Carried) -> Result<(), EncodeError> {
    e.array(2)?;
    match c {
        Carried::One(value) => {
            e.uint(0)?;
            e.bytes(&value.0)?;
        }
        Carried::Combined(values) => {
            e.uint(1)?;
            e.array(count(values.len())?)?;
            for (name, value) in values {
                e.array(2)?;
                e.bytes(&name.0)?;
                e.bytes(&value.0)?;
            }
        }
    }
    Ok(())
}

/// The canonical encoding of `definitions` and `values`: `[definitions, values]`, definitions in
/// attribute order, values grouped by attribute in order and by name in order.
///
/// # Errors
///
/// [`AttributesError::Encode`], which only a defect can cause.
pub fn encode_attributes(
    definitions: &Definitions,
    values: &Attributes,
) -> Result<Vec<u8>, AttributesError> {
    let defs: Vec<&Definition> = definitions.iter().collect();
    let mut groups: Vec<(&AttrId, Vec<(&NameKey, &Carried)>)> = Vec::new();
    for (attr, name, c) in values.iter() {
        match groups.last_mut() {
            Some((last, entries)) if *last == attr => entries.push((name, c)),
            _ => groups.push((attr, alloc::vec![(name, c)])),
        }
    }
    let mut e = Encoder::new();
    e.array(2)?;
    e.array(count(defs.len())?)?;
    for d in defs {
        definition(&mut e, d)?;
    }
    e.array(count(groups.len())?)?;
    for (attr, entries) in groups {
        e.array(2)?;
        attr_id(&mut e, attr)?;
        e.array(count(entries.len())?)?;
        for (name, c) in entries {
            e.array(2)?;
            e.bytes(&name.0)?;
            carried(&mut e, c)?;
        }
    }
    Ok(e.finish()?.into_bytes())
}

/// The attributes record of `definitions` and `values`.
///
/// # Errors
///
/// As [`encode_attributes`].
pub fn attributes_record(
    definitions: &Definitions,
    values: &Attributes,
) -> Result<Record, AttributesError> {
    Ok(Record {
        tag: ATTRIBUTES_TAG,
        flags: REQUIRED,
        schema: ATTRIBUTES_SCHEMA,
        depends: Vec::new(),
        payload: encode_attributes(definitions, values)?,
    })
}

/// A shape failure at `offset`.
fn shape(offset: usize, expected: &'static str) -> AttributesError {
    AttributesError::Payload(PayloadError::Shape { offset, expected })
}

/// Reads one small code at most `max`.
fn code(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
    max: u64,
    expected: &'static str,
) -> Result<u64, AttributesError> {
    let offset = r.offset();
    let value = r.uint(meter, expected)?;
    if value > max {
        return Err(shape(offset, expected));
    }
    Ok(value)
}

fn read_attr(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<AttrId, AttributesError> {
    let offset = r.offset();
    if r.array(meter, "an attribute [owner, name, version]")? != 3 {
        return Err(shape(offset, "an attribute of three items"));
    }
    let owner = String::from(r.text(meter, "the attribute's owner")?);
    let name = String::from(r.text(meter, "the attribute's name")?);
    let at = r.offset();
    let version = u32::try_from(r.uint(meter, "the attribute's version")?)
        .map_err(|_| shape(at, "a version below 2^32"))?;
    Ok(AttrId {
        owner,
        name,
        version,
    })
}

fn read_definition(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Definition, AttributesError> {
    let offset = r.offset();
    if r.array(meter, "a definition")? != 4 {
        return Err(shape(offset, "a definition of four items"));
    }
    let attr = read_attr(r, meter)?;
    let n = r.array(meter, "the kinds that may carry it")?;
    let mut kinds = Vec::new();
    for _ in 0..n {
        kinds.push(match code(r, meter, 4, "an entity kind 0 to 4")? {
            0 => EntityKind::Body,
            1 => EntityKind::Region,
            2 => EntityKind::Face,
            3 => EntityKind::Edge,
            _ => EntityKind::Vertex,
        });
    }
    let at = r.offset();
    let len = r.array(meter, "a value schema")?;
    let schema = match (code(r, meter, 5, "a value schema 0 to 5")?, len) {
        (0, 2) => ValueSchema::Opaque {
            id: String::from(r.text(meter, "an opaque schema's ID")?),
        },
        (1, 1) => ValueSchema::Point,
        (2, 1) => ValueSchema::Direction,
        (3, 1) => ValueSchema::Frame,
        (4, 1) => ValueSchema::Length,
        (5, 1) => ValueSchema::Field,
        _ => return Err(shape(at, "a value schema with its fields")),
    };
    let at = r.offset();
    if r.array(meter, "a behaviour")? != 5 {
        return Err(shape(at, "a behaviour of five items"));
    }
    let changed = match code(r, meter, 1, "keep 0 or delete 1")? {
        0 => ChangeAction::Keep,
        _ => ChangeAction::Delete,
    };
    let at = r.offset();
    if r.array(meter, "the actions on transform")? != 4 {
        return Err(shape(at, "four actions on transform"));
    }
    let mut actions = [TransformAction::Keep; 4];
    for action in &mut actions {
        *action = match code(r, meter, 2, "keep 0, transform 1 or delete 2")? {
            0 => TransformAction::Keep,
            1 => TransformAction::Transform,
            _ => TransformAction::Delete,
        };
    }
    let [moved, rotated, mirrored, scaled] = actions;
    let split = match code(r, meter, 1, "propagate 0 or delete 1")? {
        0 => SplitAction::Propagate,
        _ => SplitAction::Delete,
    };
    let merged = match code(r, meter, 2, "keep if equal 0, combine 1 or delete 2")? {
        0 => MergeAction::KeepIfEqual,
        1 => MergeAction::Combine,
        _ => MergeAction::Delete,
    };
    let copied = match code(r, meter, 1, "copy 0 or delete 1")? {
        0 => CopyAction::Copy,
        _ => CopyAction::Delete,
    };
    let behaviour = Behaviour {
        changed,
        transformed: OnTransform {
            moved,
            rotated,
            mirrored,
            scaled,
        },
        split,
        merged,
        copied,
    };
    Ok(Definition::new(attr, kinds, schema, behaviour)?)
}

fn read_key(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<NameKey, AttributesError> {
    Ok(NameKey(r.name(meter, "an entity's NameId")?.to_bytes()))
}

fn read_carried(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<Carried, AttributesError> {
    let offset = r.offset();
    if r.array(meter, "a carried value")? != 2 {
        return Err(shape(offset, "a carried value [0, value] or [1, values]"));
    }
    Ok(if code(r, meter, 1, "one 0 or combined 1")? == 0 {
        Carried::One(Value(r.bytes(meter, "a value")?.to_vec()))
    } else {
        {
            let n = r.array(meter, "the combined values")?;
            let mut values = alloc::collections::BTreeMap::new();
            for _ in 0..n {
                let at = r.offset();
                if r.array(meter, "a combined value [name, value]")? != 2 {
                    return Err(shape(at, "a combined value of two items"));
                }
                let name = read_key(r, meter)?;
                values.insert(name, Value(r.bytes(meter, "a value")?.to_vec()));
            }
            Carried::Combined(values)
        }
    })
}

/// Reads an attributes record's payload: the definitions, and the values with their classes from
/// their definitions. No owner is needed.
///
/// # Errors
///
/// [`AttributesError`] for a broken shape, a definition the rules refuse, a value without its
/// definition, or bytes that are not the canonical encoding.
pub fn read_attributes(
    payload: &[u8],
    meter: &mut WorkMeter<'_>,
) -> Result<(Definitions, Attributes), AttributesError> {
    let mut r = Reader::new(payload);
    let offset = r.offset();
    if r.array(meter, "an attributes array")? != 2 {
        return Err(shape(offset, "definitions and values"));
    }
    let n = r.array(meter, "the definitions")?;
    let mut definitions = Definitions::default();
    let mut attributes = Attributes::new();
    for _ in 0..n {
        let d = read_definition(&mut r, meter)?;
        attributes = attributes.defined_as(d.attr().clone(), d.behaviour().class());
        definitions = definitions.define(d)?;
    }
    let groups = r.array(meter, "the values")?;
    let mut values = Vec::new();
    for _ in 0..groups {
        let at = r.offset();
        if r.array(meter, "an attribute's values [attribute, values]")? != 2 {
            return Err(shape(at, "an attribute's values of two items"));
        }
        let attr = read_attr(&mut r, meter)?;
        if definitions.get(&attr).is_none() {
            return Err(AttributesError::Undefined(attr));
        }
        let entries = r.array(meter, "the values by name")?;
        for _ in 0..entries {
            let at = r.offset();
            if r.array(meter, "a value [name, carried]")? != 2 {
                return Err(shape(at, "a value of two items"));
            }
            let name = read_key(&mut r, meter)?;
            values.push((attr.clone(), name, read_carried(&mut r, meter)?));
        }
    }
    r.end()?;
    let attributes = attributes.rebuilt(values);
    if encode_attributes(&definitions, &attributes)? != payload {
        return Err(AttributesError::NotCanonical);
    }
    Ok((definitions, attributes))
}
