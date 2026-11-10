//! The names record: every name node a file keeps, each `NameId` checked against its bytes under
//! the file's `NameScheme` as it is read.
//!
//! What it does: [`names_record`] writes a name graph's nodes as one required record (tag
//! [`NAMES_TAG`]): an array of `[id, name]` pairs in `NameId` order, each name in its canonical
//! encoding (32.19). [`read_names`] reads one back into a fresh name graph. [`read_topo_name`]
//! reads one canonical `TopoName` item.
//!
//! Guarantees:
//!
//! - **Every `NameId` is verified under the recorded scheme** (24.4; piece G95): a file whose header
//!   names a scheme other than `NameScheme v1` is refused, and every node's id must be the hash of
//!   its own stored bytes, or the record is refused with the node's offset. A name is never trusted
//!   because a file says so.
//! - **One encoding per name**: a node is read into a `TopoName` and encoded again, and the bytes
//!   must be the stored ones, so a set out of order, a repeated name, a non-NFC role or any other
//!   second spelling of a name is refused; the graph refuses a second node under one id.
//! - **Order is name order**: nodes are in strictly increasing `NameId` order, or the record is
//!   refused.
//!
//! Method: chapter 24.4's names record and decision 32.19's canonical encoding and hash
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`), read by re-encoding, the way
//! RFC 8949 section 4.2's deterministic encoding is checked: decode, encode, compare
//! (<https://www.rfc-editor.org/rfc/rfc8949#section-4.2>).
//!
//! Cost: linear in the record; each node is decoded, encoded and hashed once.

use alloc::vec::Vec;
use core::fmt;

use geom_name::{GraphError, NameGraph};
use geom_op::cbor::{EncodeError, Encoder};
use geom_op::{
    EdgeOrVertex, Label, NameError, NameId, NameList, NameSet, OpId, OpPath, Ordinal, PieceKey,
    Role, RoleArg, Side, TopoName, name::name_id,
};
use geom_run::WorkMeter;

use crate::cbor::Item;
use crate::header::NAME_SCHEME;
use crate::payload::{PayloadError, Reader};
use crate::record::{REQUIRED, Record};

/// The names record's tag (REGISTRY.md).
pub const NAMES_TAG: u64 = 16;

/// The names record's schema.
pub const NAMES_SCHEMA: u64 = 1;

/// Why a names record, or a name in it, cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum NamesError {
    /// The payload's shape or encoding is wrong.
    Payload(PayloadError),
    /// The file's header names a `NameScheme` this reader does not know.
    Scheme(u64),
    /// A node out of `NameId` order, or a second node with one id, at this offset.
    Order {
        /// The node's offset in the payload.
        offset: usize,
    },
    /// A name stored in an encoding other than its one canonical encoding, at this offset.
    NotCanonical {
        /// The name's offset in the payload.
        offset: usize,
    },
    /// A node whose id is not the hash of its bytes under the recorded scheme.
    WrongId {
        /// The node's offset in the payload.
        offset: usize,
        /// The id the file gives.
        stored: NameId,
        /// The id its bytes hash to.
        computed: NameId,
    },
    /// A name the name types refuse: an empty or non-NFC role, a repeated member of a set, an
    /// ordinal out of range.
    Name(NameError),
    /// The name graph refused a node.
    Graph(GraphError),
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
}

impl fmt::Display for NamesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamesError::Payload(e) => write!(f, "not a names record: {e}"),
            NamesError::Scheme(n) => write!(f, "NameScheme {n} is not readable here"),
            NamesError::Order { offset } => {
                write!(f, "a name node out of NameId order at byte {offset}")
            }
            NamesError::NotCanonical { offset } => {
                write!(f, "a name not in its canonical encoding at byte {offset}")
            }
            NamesError::WrongId {
                offset,
                stored,
                computed,
            } => write!(
                f,
                "the name at byte {offset} is stored as {stored} but hashes to {computed}"
            ),
            NamesError::Name(e) => write!(f, "not a name: {e}"),
            NamesError::Graph(e) => write!(f, "the name graph refused a node: {e}"),
            NamesError::Encode(e) => write!(f, "the names could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for NamesError {}

impl From<PayloadError> for NamesError {
    fn from(e: PayloadError) -> Self {
        NamesError::Payload(e)
    }
}

impl From<NameError> for NamesError {
    fn from(e: NameError) -> Self {
        NamesError::Name(e)
    }
}

impl From<EncodeError> for NamesError {
    fn from(e: EncodeError) -> Self {
        NamesError::Encode(e)
    }
}

/// The names record of `graph`: every node, in `NameId` order.
///
/// # Errors
///
/// [`NamesError::Encode`] or [`NamesError::Name`], which only a defect can cause.
pub fn names_record(graph: &NameGraph) -> Result<Record, NamesError> {
    let ids: Vec<NameId> = graph.ids().collect();
    let mut e = Encoder::new();
    e.array(u64::try_from(ids.len()).map_err(|_| EncodeError::TooLong)?)?;
    for id in ids {
        e.array(2)?;
        e.bytes(&id.to_bytes())?;
        if let Some(name) = graph.name(id) {
            e.item(&name.encode()?)?;
        }
    }
    Ok(Record {
        tag: NAMES_TAG,
        flags: REQUIRED,
        schema: NAMES_SCHEMA,
        depends: Vec::new(),
        payload: e.finish()?.into_bytes(),
    })
}

/// Reads a names record's payload, written under `name_scheme` (the header's), into a name graph.
///
/// # Errors
///
/// [`NamesError`]: an unknown scheme, a broken shape, a node out of order, a name in another
/// encoding, an id that is not its name's hash, or a node the graph refuses.
pub fn read_names(
    payload: &[u8],
    name_scheme: u64,
    meter: &mut WorkMeter<'_>,
) -> Result<NameGraph, NamesError> {
    if name_scheme != NAME_SCHEME {
        return Err(NamesError::Scheme(name_scheme));
    }
    let mut r = Reader::new(payload);
    let nodes = r.array(meter, "an array of name nodes")?;
    let mut graph = NameGraph::new();
    let mut last: Option<NameId> = None;
    for _ in 0..nodes {
        let offset = r.offset();
        if r.array(meter, "a name node [id, name]")? != 2 {
            return Err(PayloadError::Shape {
                offset,
                expected: "a name node of two items",
            }
            .into());
        }
        let stored = r.name(meter, "a NameId")?;
        if last.is_some_and(|last| stored <= last) {
            return Err(NamesError::Order { offset });
        }
        last = Some(stored);
        let at = r.offset();
        let bytes = r.item(meter)?;
        let name = read_topo_name(bytes, meter).map_err(|e| shift(e, at))?;
        let computed = name_id(bytes);
        if computed != stored {
            return Err(NamesError::WrongId {
                offset,
                stored,
                computed,
            });
        }
        graph.intern(name).map_err(NamesError::Graph)?;
    }
    r.end()?;
    Ok(graph)
}

/// An error at an offset inside a name moved to the payload's offsets.
fn shift(e: NamesError, by: usize) -> NamesError {
    match e {
        NamesError::Payload(PayloadError::Shape { offset, expected }) => {
            NamesError::Payload(PayloadError::Shape {
                offset: offset.saturating_add(by),
                expected,
            })
        }
        NamesError::NotCanonical { offset } => NamesError::NotCanonical {
            offset: offset.saturating_add(by),
        },
        other => other,
    }
}

/// Reads one `TopoName` from its canonical encoding, `bytes` exactly.
///
/// # Errors
///
/// [`NamesError`] for a broken shape, a name the name types refuse, or bytes that are not the
/// name's one canonical encoding.
pub fn read_topo_name(bytes: &[u8], meter: &mut WorkMeter<'_>) -> Result<TopoName, NamesError> {
    let mut r = Reader::new(bytes);
    let len = r.array(meter, "a TopoName array")?;
    let variant_at = r.offset();
    let name = match (r.uint(meter, "a TopoName variant")?, len) {
        (0, 4) => TopoName::Created {
            op: read_op_path(&mut r, meter)?,
            role: read_role(&mut r, meter)?,
            from: NameList::new(read_names_list(&mut r, meter)?),
        },
        (1, 4) => TopoName::Split {
            op: read_op_path(&mut r, meter)?,
            of: r.name(meter, "the name split")?,
            piece: read_piece(&mut r, meter)?,
        },
        (2, 3) => TopoName::Merged {
            op: read_op_path(&mut r, meter)?,
            of: NameSet::new(read_names_list(&mut r, meter)?)?,
        },
        (3, 4) => {
            let at = r.offset();
            let kind = match r.uint(meter, "edge 0 or vertex 1")? {
                0 => EdgeOrVertex::Edge,
                1 => EdgeOrVertex::Vertex,
                _ => {
                    return Err(PayloadError::Shape {
                        offset: at,
                        expected: "edge 0 or vertex 1",
                    }
                    .into());
                }
            };
            TopoName::Bounded {
                kind,
                by: NameSet::new(read_names_list(&mut r, meter)?)?,
                ends: NameSet::new(read_names_list(&mut r, meter)?)?,
            }
        }
        (4, 3) => {
            let base = r.name(meter, "the weak name's base")?;
            let at = r.offset();
            if r.array(meter, "an ordinal [n, of]")? != 2 {
                return Err(PayloadError::Shape {
                    offset: at,
                    expected: "an ordinal of two items",
                }
                .into());
            }
            let n = small(&mut r, meter, "the ordinal's n")?;
            let of = small(&mut r, meter, "the ordinal's count")?;
            TopoName::Weak {
                base,
                ord: Ordinal::new(n, of)?,
            }
        }
        _ => {
            return Err(PayloadError::Shape {
                offset: variant_at,
                expected: "a TopoName variant 0 to 4 with its fields",
            }
            .into());
        }
    };
    r.end()?;
    if name.encode()?.as_bytes() != bytes {
        return Err(NamesError::NotCanonical { offset: 0 });
    }
    Ok(name)
}

/// A `u32` count.
fn small(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
    expected: &'static str,
) -> Result<u32, NamesError> {
    let offset = r.offset();
    u32::try_from(r.uint(meter, expected)?)
        .map_err(|_| PayloadError::Shape { offset, expected }.into())
}

/// Reads an `OpPath`: an array of the `OpId`'s 16 bytes, then labels, a step as text and a key as
/// 16 bytes.
///
/// # Errors
///
/// [`NamesError::Payload`] for any other shape, or a step label [`Label::step`] refuses.
pub fn read_op_path(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<OpPath, NamesError> {
    let offset = r.offset();
    let len = r.array(meter, "an OpPath array")?;
    if len == 0 {
        return Err(PayloadError::Shape {
            offset,
            expected: "an OpPath with its OpId",
        }
        .into());
    }
    let op = r.bytes(meter, "the OpId's 16 bytes")?;
    let op = <[u8; 16]>::try_from(op).map_err(|_| PayloadError::Shape {
        offset,
        expected: "the OpId's 16 bytes",
    })?;
    let mut path = OpPath::new(OpId::from_bytes(op));
    for _ in 1..len {
        let at = r.offset();
        let label = match r.next(meter)? {
            Item::Text(step) => Label::step(step),
            Item::Bytes(key) => <[u8; 16]>::try_from(key).ok().map(Label::Key),
            _ => None,
        };
        let label = label.ok_or(PayloadError::Shape {
            offset: at,
            expected: "a step label of printable ASCII or a 16-byte key",
        })?;
        path = path.child(label);
    }
    Ok(path)
}

/// Reads a role: an array of its name as text, then arguments, a name as 16 bytes and a count as
/// an unsigned integer.
///
/// # Errors
///
/// [`NamesError`] for any other shape, or a name [`Role::new`] refuses.
pub fn read_role(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<Role, NamesError> {
    let offset = r.offset();
    let len = r.array(meter, "a role array")?;
    if len == 0 {
        return Err(PayloadError::Shape {
            offset,
            expected: "a role with its name",
        }
        .into());
    }
    let name = r.text(meter, "the role's name")?;
    let mut args = Vec::new();
    for _ in 1..len {
        let at = r.offset();
        args.push(match r.next(meter)? {
            Item::Uint(n) => RoleArg::Count(n),
            Item::Bytes(b) => <[u8; 16]>::try_from(b)
                .map(|b| RoleArg::Name(NameId::from_bytes(b)))
                .map_err(|_| PayloadError::Shape {
                    offset: at,
                    expected: "a role argument: 16 bytes or a count",
                })?,
            _ => {
                return Err(PayloadError::Shape {
                    offset: at,
                    expected: "a role argument: 16 bytes or a count",
                }
                .into());
            }
        });
    }
    Ok(Role::new(name, args)?)
}

/// Reads an array of names (a set or a sorted list; the caller's re-encoding checks which).
///
/// # Errors
///
/// [`NamesError::Payload`] for any other shape.
pub fn read_names_list(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<NameId>, NamesError> {
    let len = r.array(meter, "an array of names")?;
    let mut names = Vec::new();
    for _ in 0..len {
        names.push(r.name(meter, "a NameId's 16 bytes")?);
    }
    Ok(names)
}

/// Reads a piece key: `[0, tools, side]`.
///
/// # Errors
///
/// [`NamesError`] for any other shape, or tools that repeat a name.
pub fn read_piece(r: &mut Reader<'_>, meter: &mut WorkMeter<'_>) -> Result<PieceKey, NamesError> {
    let offset = r.offset();
    let shape = PayloadError::Shape {
        offset,
        expected: "a piece key [0, tools, side]",
    };
    if r.array(meter, "a piece key")? != 3 || r.uint(meter, "a piece key's kind")? != 0 {
        return Err(shape.into());
    }
    let tools = NameSet::new(read_names_list(r, meter)?)?;
    let side = match r.uint(meter, "back 0 or front 1")? {
        0 => Side::Back,
        1 => Side::Front,
        _ => return Err(shape.into()),
    };
    Ok(PieceKey::ByTools { tools, side })
}
