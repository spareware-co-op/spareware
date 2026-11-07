//! Survey points (chapter 17.2): a dataset's points imported as one Acorn body, each point a
//! vertex named by the import and its record key, its ID an attribute.
//!
//! What it does: [`import`] takes the path of the dataset's import operation and its [`Record`]s,
//! each a point's ID, its stable [`PointKey`] and its grid position, and returns [`Imported`]: an
//! Acorn (points) body whose vertices are the points, published in name order; the names it made,
//! the outside region `Created { op, role: region.outside }` and each point
//! `Created { op, role: survey.point(key) }`; and each point's ID as the attribute [`survey_id`],
//! keyed by the point's name. [`PointKey::first_import`] makes a point's key from its ID at its
//! first import: the first 16 bytes of SHA-256 over [`KEY_PREFIX`] and the ID as a CBOR text
//! string. The key is frozen from then on; the ID may change, as an attribute does.
//!
//! Guarantees:
//!
//! - **Names hold across imports.** A point's name is a function of the import's path and its key
//!   alone, never of its ID, its position or the order of the records. Run again with the same
//!   path and keys, an updated dataset gives every point it still holds the same name, a renamed
//!   point (a new ID under its old key) included (17.2).
//! - **Two points with one ID are an error naming both** ([`SurveyError::DuplicateId`]: the ID and
//!   both records), never a silent suffix; so are two records with one key
//!   ([`SurveyError::DuplicateKey`]), which would make one name twice. Of several, the pair
//!   reported is the first in byte order of the ID (or key), and within it the two lowest records.
//! - **IDs are text a name can rest on.** An ID is non-empty and in Normalization Form C under
//!   Unicode 18.0.0, decision 32.19's rule for text hashed into a name, checked by the same check
//!   `geom-op` applies to role names ([`Role::new`]); an ID that is not is refused, never
//!   normalised behind the caller's back.
//! - **Arenas follow names.** The body is published by `geom_topo::Builder::publish_named`, so its
//!   vertices are in name order and the body is the same whatever order the records come in.
//! - **Inputs unchanged, failures whole.** Nothing the caller holds is changed, and a failure
//!   returns no body.
//! - **Metered.** One work unit per record, before anything is built, beside what publishing
//!   charges.
//!
//! Method: chapter 17.2's survey points
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`), named as chapter 20.2
//! names what an operation creates (`projects/engines/geometry/architecture/20-naming-history.md`)
//! under `NameScheme v1` (32.19), with the record key in a keyed role's argument as `geom-op`'s
//! keyed roles hold an operand's key; the ID an attribute as chapter 8.4 asks
//! (`projects/engines/geometry/architecture/08-topology.md`), its value the ID's CBOR text, 24.3's
//! canonical encoding.
//!
//! Cost: O(n log n) in the records, for the two sorts that find repeated IDs and keys, the
//! hashing of n + 1 names, and the builder's sort by name. Nothing recurses.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_num::P3;
use geom_op::cbor::Encoder;
use geom_op::hash::Sha256;
use geom_op::{NameError, NameId, NameList, OpPath, Role, RoleArg, TopoName};
use geom_run::WorkMeter;
use geom_topo::attributes::{AttrId, Attributes, Class, MergeAction, SplitAction, Value};
use geom_topo::order::{NameKey, Names};
use geom_topo::{Body, BodyKind, Builder, Region, RegionKind, TopoError, Vertex};

/// The bytes hashed before a point's ID to make its key at first import: the 23 ASCII bytes
/// `SKG survey point key v1` and one zero byte, so a key never hashes like a name or a content ID,
/// and a later rule would take a new prefix.
pub const KEY_PREFIX: &[u8; 24] = b"SKG survey point key v1\0";

/// The role every survey point is created in: `survey.point(key)`, keyed (`geom_op::KEYED_ROLES`).
pub const POINT_ROLE: &str = "survey.point";

/// The role of the body's outside region.
pub const OUTSIDE_ROLE: &str = "region.outside";

/// A point's stable record key: its ID at first import, frozen (17.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointKey([u8; 16]);

impl PointKey {
    /// The key whose sixteen bytes are `bytes`: a key kept from an earlier import.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> PointKey {
        PointKey(bytes)
    }

    /// The key's sixteen bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }

    /// The key of a point first imported with ID `id`: the first 16 bytes of SHA-256 over
    /// [`KEY_PREFIX`] and `id` as a CBOR text string.
    ///
    /// # Errors
    ///
    /// [`IdError::Empty`] for an empty ID; [`IdError::NotNfc`] for one not in Normalization Form C.
    pub fn first_import(id: &str) -> Result<PointKey, IdError> {
        checked(id)?;
        let text = text(id).map_err(|_| IdError::Encode)?;
        let mut hash = Sha256::new();
        hash.write(KEY_PREFIX);
        hash.write(&text);
        Ok(PointKey(hash.finish_128()))
    }
}

/// Why an ID cannot name a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdError {
    /// The ID is empty.
    Empty,
    /// The ID is not in Unicode Normalization Form C (32.19).
    NotNfc,
    /// The encoder refused the ID; only a defect can cause it.
    Encode,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => f.write_str("the point ID is empty"),
            IdError::NotNfc => f.write_str("the point ID is not in Normalization Form C"),
            IdError::Encode => f.write_str("internal error: the point ID could not be encoded"),
        }
    }
}

impl core::error::Error for IdError {}

/// One record of a dataset: a point's ID, its key and its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Record<'a> {
    /// The point's ID as the dataset gives it now.
    pub id: &'a str,
    /// Its key: [`PointKey::first_import`] of its ID at its first import, kept since.
    pub key: PointKey,
    /// Its position on the grid.
    pub at: P3,
}

impl<'a> Record<'a> {
    /// The record of a point imported for the first time: its key from its ID.
    ///
    /// # Errors
    ///
    /// As [`PointKey::first_import`].
    pub fn first(id: &'a str, at: P3) -> Result<Record<'a>, IdError> {
        Ok(Record {
            id,
            key: PointKey::first_import(id)?,
            at,
        })
    }
}

/// Why a dataset could not be imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurveyError {
    /// Two records carry the ID `id`: records `first` and `second`, counting from 0, the lower
    /// first.
    DuplicateId {
        /// The ID both carry.
        id: String,
        /// The lower record.
        first: usize,
        /// The higher record.
        second: usize,
    },
    /// Two records carry the key `key`, which would give two points one name: records `first`
    /// and `second`, the lower first.
    DuplicateKey {
        /// The key both carry.
        key: PointKey,
        /// The lower record.
        first: usize,
        /// The higher record.
        second: usize,
    },
    /// Record `index` has an ID that cannot name a point.
    Id {
        /// The record, counting from 0.
        index: usize,
        /// Why.
        reason: IdError,
    },
    /// The body could not be built: the work meter stopped ([`TopoError::Stopped`]) or memory was
    /// refused.
    Body(TopoError),
    /// A step its own bounds rule out failed: a defect in this module, reported, never a panic.
    Defect,
}

impl fmt::Display for SurveyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SurveyError::DuplicateId { id, first, second } => {
                write!(f, "points {first} and {second} both have the ID {id:?}")
            }
            SurveyError::DuplicateKey { key, first, second } => {
                write!(f, "points {first} and {second} both have the key ")?;
                key.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
            }
            SurveyError::Id { index, reason } => write!(f, "point {index}: {reason}"),
            SurveyError::Body(e) => write!(f, "{e}"),
            SurveyError::Defect => {
                f.write_str("internal error: a survey import step failed (a defect in geom-2d)")
            }
        }
    }
}

impl core::error::Error for SurveyError {}

/// A dataset imported: the body, the names made and each point's ID.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    /// The Acorn body: its outside region and one vertex per point, in name order.
    pub body: Body,
    /// The names made, the outside region's first, then each point's in record order: what a
    /// name graph (`geom_name`) needs to resolve a reference to them.
    pub names: Vec<TopoName>,
    /// Each record's point's name, in record order.
    pub points: Vec<NameId>,
    /// Each point's ID as [`survey_id`], keyed by the point's name.
    pub attributes: Attributes,
}

/// The attribute that holds a survey point's ID (8.4, 17.2): owner `kit.geom`, name `survey.id`,
/// version 1; its value the ID as a CBOR text string.
#[must_use]
pub fn survey_id() -> AttrId {
    AttrId {
        owner: String::from("kit.geom"),
        name: String::from("survey.id"),
        version: 1,
    }
}

/// The class of [`survey_id`]: a point's ID goes with every piece of it, and survives a merge only
/// when every input carries the same ID.
pub const SURVEY_ID_CLASS: Class = Class {
    split: SplitAction::Propagate,
    merge: MergeAction::KeepIfEqual,
};

/// `id` if it can name a point: non-empty and NFC, as a role name must be (32.19).
fn checked(id: &str) -> Result<(), IdError> {
    match Role::new(id, []) {
        Ok(_) => Ok(()),
        Err(NameError::EmptyRole) => Err(IdError::Empty),
        Err(_) => Err(IdError::NotNfc),
    }
}

/// `id` as a CBOR text string, 24.3's canonical encoding.
fn text(id: &str) -> Result<Vec<u8>, geom_op::cbor::EncodeError> {
    let mut e = Encoder::new();
    e.text(id)?;
    Ok(e.finish()?.into_bytes())
}

/// The first pair of records, in the order of `key`'s bytes and then by record, that share a key.
fn repeated<K: Ord + Clone>(keys: &[K]) -> Option<(K, usize, usize)> {
    let mut order: Vec<(K, usize)> = keys.iter().cloned().zip(0..).collect();
    order.sort();
    order.windows(2).find_map(|w| match w {
        [(a, i), (b, j)] if a == b => Some((a.clone(), *i, *j)),
        _ => None,
    })
}

/// `name`'s `NameId` as the builder's key.
fn key_of(name: &TopoName) -> Result<(NameId, NameKey), SurveyError> {
    let id = name.id().map_err(|_| SurveyError::Defect)?;
    Ok((id, NameKey(id.to_bytes())))
}

/// The name of the point keyed `key`, created by `op`.
///
/// # Errors
///
/// [`SurveyError::Defect`] only for a defect.
pub fn point_name(op: &OpPath, key: PointKey) -> Result<TopoName, SurveyError> {
    let role = Role::new(POINT_ROLE, [RoleArg::Name(NameId::from_bytes(key.0))])
        .map_err(|_| SurveyError::Defect)?;
    Ok(TopoName::Created {
        op: op.clone(),
        role,
        from: NameList::new([]),
    })
}

/// Imports a dataset's `records` in the operation `op` (module documentation).
///
/// # Errors
///
/// [`SurveyError::Id`] for the first record whose ID cannot name a point;
/// [`SurveyError::DuplicateId`] and then [`SurveyError::DuplicateKey`] for two records that share
/// one; [`SurveyError::Body`] if the meter stops or memory is refused; [`SurveyError::Defect`]
/// only for a defect.
pub fn import(
    op: &OpPath,
    records: &[Record<'_>],
    meter: &mut WorkMeter<'_>,
) -> Result<Imported, SurveyError> {
    if let Some((index, reason)) = records
        .iter()
        .enumerate()
        .find_map(|(i, r)| checked(r.id).err().map(|e| (i, e)))
    {
        return Err(SurveyError::Id { index, reason });
    }
    let ids: Vec<&[u8]> = records.iter().map(|r| r.id.as_bytes()).collect();
    if let Some((id, first, second)) = repeated(&ids) {
        return Err(SurveyError::DuplicateId {
            id: String::from_utf8_lossy(id).into_owned(),
            first,
            second,
        });
    }
    let keys: Vec<PointKey> = records.iter().map(|r| r.key).collect();
    if let Some((key, first, second)) = repeated(&keys) {
        return Err(SurveyError::DuplicateKey { key, first, second });
    }
    let count = u64::try_from(records.len()).map_err(|_| SurveyError::Defect)?;
    meter
        .charge(count)
        .map_err(|s| SurveyError::Body(TopoError::from(s)))?;

    let outside = TopoName::Created {
        op: op.clone(),
        role: Role::new(OUTSIDE_ROLE, []).map_err(|_| SurveyError::Defect)?,
        from: NameList::new([]),
    };
    let mut names = Vec::with_capacity(records.len().saturating_add(1));
    let mut points = Vec::with_capacity(records.len());
    let mut rows = Names {
        regions: Vec::from([key_of(&outside)?.1]),
        faces: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::with_capacity(records.len()),
    };
    names.push(outside);
    let mut builder = Builder::new();
    let build = SurveyError::Body;
    let outside_row = builder
        .arenas
        .regions
        .push(Region {
            kind: RegionKind::Void,
            first_shell: None,
        })
        .map_err(build)?;
    let mut attributes = Attributes::new()
        .define(survey_id(), SURVEY_ID_CLASS)
        .ok_or(SurveyError::Defect)?;
    let attr = survey_id();
    for r in records {
        let name = point_name(op, r.key)?;
        let (id, key) = key_of(&name)?;
        let point = builder.arenas.points.push(r.at).map_err(build)?;
        builder
            .arenas
            .vertices
            .push(Vertex { point })
            .map_err(build)?;
        let value = text(r.id).map_err(|_| SurveyError::Id {
            index: points.len(),
            reason: IdError::Encode,
        })?;
        attributes = attributes
            .set(&attr, key, Value(value))
            .ok_or(SurveyError::Defect)?;
        rows.vertices.push(key);
        points.push(id);
        names.push(name);
    }
    let body = builder
        .publish_named(&rows, BodyKind::Acorn, outside_row, meter)
        .map_err(SurveyError::Body)?;
    Ok(Imported {
        body,
        names,
        points,
        attributes,
    })
}
