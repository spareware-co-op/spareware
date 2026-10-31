//! Persistent names: how an operation names what it makes, and the `NameId` each name hashes to
//! under `NameScheme v1`.
//!
//! What it does: a [`TopoName`] says how an entity was made: created by an operation step in a
//! [`Role`], split from one input by cutting faces ([`PieceKey`]), merged from several, bounded by
//! the faces it lies between and those that end it, or, where entities would still collide, the
//! [`Ordinal`]-th of a [`TopoName::Weak`] group (chapter 20.2). [`TopoName::encode`] writes its
//! canonical bytes and [`TopoName::id`] hashes them to the 128-bit [`NameId`] (32.19): the first
//! 16 bytes of SHA-256 over [`NAME_SCHEME_V1_PREFIX`] and those bytes. Sets of names are
//! [`NameSet`]s, sorted lists [`NameList`]s, both in name order.
//!
//! The canonical encoding, fixed for v1 by 32.19 and written with this crate's deterministic CBOR
//! encoder ([`crate::cbor`]):
//!
//! | Value | Encoding |
//! | --- | --- |
//! | `TopoName` | an array: the variant's number (`Created` 0, `Split` 1, `Merged` 2, `Bounded` 3, `Weak` 4), then its fields in their declared order |
//! | `NameId` | a 16-byte byte string |
//! | `NameSet`, `NameList` | an array of the names in byte order; a set has no repeats |
//! | `OpPath` | an array: the `OpId`'s 16 bytes, then each label, a step as text and a key as its 16 bytes |
//! | `Role` | an array: its name as text, then its arguments, a name as its 16 bytes and a count as an unsigned integer |
//! | `PieceKey::ByTools` | `[0, tools, side]` |
//! | `Side` | `Back` 0, `Front` 1 |
//! | `EdgeOrVertex` | `Edge` 0, `Vertex` 1 |
//! | `Ordinal` | `[n, of]` |
//!
//! 32.19 fixes every row but the numbering of `Side` and `EdgeOrVertex` and the two kinds of role
//! argument, which this piece fixes for v1 and G93's CDDL records.
//!
//! Guarantees:
//!
//! - **One name, one encoding, one `NameId`.** A `TopoName` can only hold sets without repeats,
//!   lists in order, ordinals with `n < of` and role names in Normalization Form C, so its encoding
//!   is a pure function of the name, and so is its `NameId`, on every target.
//! - **Text is NFC under Unicode 18.0.0** (32.19): a role name not in Normalization Form C is
//!   refused when the role is made, never normalised behind the caller's back. Step labels are
//!   printable ASCII ([`crate::id`]), which is NFC under every Unicode version.
//! - **Names never collide with other hashes.** The prefix `SKG NameScheme v1` and a zero byte go
//!   before every name's bytes, so no name hashes like a content ID (32.20) or a later scheme.
//!
//! Method: chapter 20.2's `TopoName` and decision 32.19 (`NameScheme v1`),
//! `projects/engines/geometry/architecture/20-naming-history.md` and
//! `projects/engines/geometry/architecture/32-decisions.md`; FIPS 180-4's SHA-256 truncated to its
//! leftmost 128 bits as NIST SP 800-107 Rev. 1 section 5.1 allows
//! (<https://csrc.nist.gov/pubs/sp/800/107/r1/final>); and Unicode's Normalization Form C, UAX #15
//! (<https://www.unicode.org/reports/tr15/>), checked with the `unicode-normalization` crate at
//! 0.1.26, whose tables are Unicode 18.0.0.
//!
//! Cost: encoding a name allocates its bytes once and one small item per element of each set;
//! hashing is one SHA-256 pass over at most a few hundred bytes. The NFC check is linear in a role
//! name's length. Recursion: none; a name refers to other names by `NameId`, never by nesting.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{EncodeError, Encoder, Item};
use crate::hash::Sha256;
use crate::id::{Label, NameId, OpPath};

/// The bytes hashed before every name's canonical encoding under `NameScheme v1`: the 17 ASCII
/// bytes `SKG NameScheme v1` and one zero byte (32.19).
pub const NAME_SCHEME_V1_PREFIX: &[u8; 18] = b"SKG NameScheme v1\0";

/// Why a name could not be made or encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// A role name that is empty.
    EmptyRole,
    /// A role name not in Unicode Normalization Form C (32.19).
    NotNfc,
    /// A set with the same name twice.
    Repeated,
    /// An ordinal whose `n` is not below its `of`.
    Ordinal,
    /// The encoder refused a value; only a defect in this module can cause it.
    Encode(EncodeError),
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameError::EmptyRole => f.write_str("a role name is empty"),
            NameError::NotNfc => f.write_str("a role name is not in Normalization Form C"),
            NameError::Repeated => f.write_str("a set of names holds one name twice"),
            NameError::Ordinal => f.write_str("an ordinal's n is not below its count"),
            NameError::Encode(e) => write!(f, "the name could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for NameError {}

impl From<EncodeError> for NameError {
    fn from(e: EncodeError) -> Self {
        NameError::Encode(e)
    }
}

/// A set of names: in byte order, each once.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameSet(Vec<NameId>);

impl NameSet {
    /// The set of `names`, sorted.
    ///
    /// # Errors
    ///
    /// [`NameError::Repeated`] if a name is given twice.
    pub fn new(names: impl IntoIterator<Item = NameId>) -> Result<NameSet, NameError> {
        let mut names: Vec<NameId> = names.into_iter().collect();
        names.sort_unstable();
        if names.windows(2).any(|w| matches!(w, [a, b] if a == b)) {
            return Err(NameError::Repeated);
        }
        Ok(NameSet(names))
    }

    /// The names, in byte order.
    #[must_use]
    pub fn names(&self) -> &[NameId] {
        &self.0
    }
}

/// A sorted list of names: in byte order, repeats kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameList(Vec<NameId>);

impl NameList {
    /// The list of `names`, sorted.
    #[must_use]
    pub fn new(names: impl IntoIterator<Item = NameId>) -> NameList {
        let mut names: Vec<NameId> = names.into_iter().collect();
        names.sort_unstable();
        NameList(names)
    }

    /// The names, in byte order.
    #[must_use]
    pub fn names(&self) -> &[NameId] {
        &self.0
    }
}

/// One argument of a role: the name of an entity it is relative to (`side(source edge)`,
/// `roll(edge)`), or a count (the n-th of a division set by count).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoleArg {
    /// A name, encoded as its 16 bytes.
    Name(NameId),
    /// A count, encoded as an unsigned integer.
    Count(u64),
}

/// A role an operation names what it makes by (chapter 20.2): `cap.start`, `side(e)`, `roll(e)`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Role {
    name: String,
    args: Vec<RoleArg>,
}

impl Role {
    /// The role `name` with `args`, in the order given.
    ///
    /// # Errors
    ///
    /// [`NameError::EmptyRole`] for an empty name, and [`NameError::NotNfc`] for one not in
    /// Normalization Form C under Unicode 18.0.0 (32.19).
    pub fn new(name: &str, args: impl IntoIterator<Item = RoleArg>) -> Result<Role, NameError> {
        if name.is_empty() {
            return Err(NameError::EmptyRole);
        }
        if !unicode_normalization::is_nfc(name) {
            return Err(NameError::NotNfc);
        }
        Ok(Role {
            name: String::from(name),
            args: args.into_iter().collect(),
        })
    }

    /// The role's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its arguments, in order.
    #[must_use]
    pub fn args(&self) -> &[RoleArg] {
        &self.args
    }
}

/// Which side of the cutting faces a split piece lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// Against the tools' normals, encoded 0.
    Back,
    /// Along the tools' normals, encoded 1.
    Front,
}

/// What a `Bounded` name names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeOrVertex {
    /// An edge, encoded 0.
    Edge,
    /// A vertex, encoded 1.
    Vertex,
}

/// How a split piece is told from the others (chapter 20.2): an enum, so a later scheme can add
/// other keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PieceKey {
    /// Which cutting faces made the piece, and on which side of them it lies.
    ByTools {
        /// The cutting faces.
        tools: NameSet,
        /// The side.
        side: Side,
    },
}

/// The `n`-th, counting from 0, of the `of` entities that share a `Weak` name's base (chapter
/// 20.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ordinal {
    n: u32,
    of: u32,
}

impl Ordinal {
    /// The `n`-th of `of`.
    ///
    /// # Errors
    ///
    /// [`NameError::Ordinal`] unless `n < of`.
    pub fn new(n: u32, of: u32) -> Result<Ordinal, NameError> {
        if n < of {
            Ok(Ordinal { n, of })
        } else {
            Err(NameError::Ordinal)
        }
    }

    /// Every ordinal of `of` entities, in order: the 0-th to the (`of` − 1)-th.
    pub fn all(of: u32) -> impl Iterator<Item = Ordinal> {
        (0..of).map(move |n| Ordinal { n, of })
    }

    /// Its position, from 0.
    #[must_use]
    pub const fn n(self) -> u32 {
        self.n
    }

    /// How many entities share the base.
    #[must_use]
    pub const fn of(self) -> u32 {
        self.of
    }
}

/// How an entity was made: the persistent name an operation gives it (chapter 20.2, 32.19).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopoName {
    /// Made new by an operation step, in a role, from these inputs.
    Created {
        /// The step.
        op: OpPath,
        /// The role.
        role: Role,
        /// The names it was made from.
        from: NameList,
    },
    /// One piece of an input that became several.
    Split {
        /// The step.
        op: OpPath,
        /// The input split.
        of: NameId,
        /// Which piece.
        piece: PieceKey,
    },
    /// Several inputs that became one.
    Merged {
        /// The step.
        op: OpPath,
        /// The inputs merged.
        of: NameSet,
    },
    /// An edge or vertex named by the faces it lies between and the faces that end it (empty for a
    /// vertex or a closed edge).
    Bounded {
        /// Edge or vertex.
        kind: EdgeOrVertex,
        /// The faces it lies between.
        by: NameSet,
        /// The faces that end it.
        ends: NameSet,
    },
    /// A name that would still collide, told apart by its place in 20.2's ordinal order.
    Weak {
        /// The name the colliding entities share.
        base: NameId,
        /// Which of them.
        ord: Ordinal,
    },
}

impl TopoName {
    /// The name's canonical encoding under `NameScheme v1` (32.19).
    ///
    /// # Errors
    ///
    /// [`NameError::Encode`], which only a defect in this module can cause.
    pub fn encode(&self) -> Result<Item, NameError> {
        let mut e = Encoder::new();
        match self {
            TopoName::Created { op, role, from } => {
                e.array(4)?;
                e.uint(0)?;
                op_path(&mut e, op)?;
                role_item(&mut e, role)?;
                e.sorted(items(from.names())?)?;
            }
            TopoName::Split { op, of, piece } => {
                e.array(4)?;
                e.uint(1)?;
                op_path(&mut e, op)?;
                e.bytes(&of.to_bytes())?;
                match piece {
                    PieceKey::ByTools { tools, side } => {
                        e.array(3)?;
                        e.uint(0)?;
                        e.set(items(tools.names())?)?;
                        e.uint(side_number(*side))?;
                    }
                }
            }
            TopoName::Merged { op, of } => {
                e.array(3)?;
                e.uint(2)?;
                op_path(&mut e, op)?;
                e.set(items(of.names())?)?;
            }
            TopoName::Bounded { kind, by, ends } => {
                e.array(4)?;
                e.uint(3)?;
                e.uint(kind_number(*kind))?;
                e.set(items(by.names())?)?;
                e.set(items(ends.names())?)?;
            }
            TopoName::Weak { base, ord } => {
                e.array(3)?;
                e.uint(4)?;
                e.bytes(&base.to_bytes())?;
                e.array(2)?;
                e.uint(u64::from(ord.n))?;
                e.uint(u64::from(ord.of))?;
            }
        }
        Ok(e.finish()?)
    }

    /// The name's `NameId` under `NameScheme v1`: the first 16 bytes of SHA-256 over
    /// [`NAME_SCHEME_V1_PREFIX`] and the canonical encoding (32.19).
    ///
    /// # Errors
    ///
    /// As [`TopoName::encode`].
    pub fn id(&self) -> Result<NameId, NameError> {
        Ok(name_id(self.encode()?.as_bytes()))
    }
}

/// The canonical encoding of an `OpPath` (32.19): an array of the `OpId`'s 16 bytes, then each
/// label, a step as text and a key as its 16 bytes.
///
/// # Errors
///
/// [`NameError::Encode`], which only a defect in this module can cause.
pub fn encode_op_path(op: &OpPath) -> Result<Item, NameError> {
    let mut e = Encoder::new();
    op_path(&mut e, op)?;
    Ok(e.finish()?)
}

/// The `NameId` of a name whose canonical encoding is `canonical`, under `NameScheme v1`.
#[must_use]
pub fn name_id(canonical: &[u8]) -> NameId {
    let mut hash = Sha256::new();
    hash.write(NAME_SCHEME_V1_PREFIX);
    hash.write(canonical);
    NameId::from_bytes(hash.finish_128())
}

/// `Side`'s number for v1.
pub(crate) const fn side_number(side: Side) -> u64 {
    match side {
        Side::Back => 0,
        Side::Front => 1,
    }
}

/// `EdgeOrVertex`'s number for v1.
const fn kind_number(kind: EdgeOrVertex) -> u64 {
    match kind {
        EdgeOrVertex::Edge => 0,
        EdgeOrVertex::Vertex => 1,
    }
}

/// Each name as a finished 16-byte byte-string item.
pub(crate) fn items(names: &[NameId]) -> Result<Vec<Item>, NameError> {
    let mut out: Vec<Item> = Vec::with_capacity(names.len());
    for name in names {
        let mut e = Encoder::new();
        e.bytes(&name.to_bytes()).map_err(NameError::Encode)?;
        out.push(e.finish().map_err(NameError::Encode)?);
    }
    Ok(out)
}

/// The length of an array of `n` items after a first one.
fn one_more(n: usize) -> Result<u64, NameError> {
    u64::try_from(n)
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or(NameError::Encode(EncodeError::TooLong))
}

/// Writes an `OpPath`: its `OpId`'s 16 bytes, then each label, a step as text and a key as its 16
/// bytes.
pub(crate) fn op_path(e: &mut Encoder, op: &OpPath) -> Result<(), NameError> {
    let labels = op.labels();
    e.array(one_more(labels.len())?)?;
    e.bytes(&op.op().to_bytes())?;
    for label in labels {
        match label {
            Label::Step(step) => e.text(step)?,
            Label::Key(key) => e.bytes(key)?,
        }
    }
    Ok(())
}

/// Writes a role: its name, then its arguments.
pub(crate) fn role_item(e: &mut Encoder, role: &Role) -> Result<(), NameError> {
    e.array(one_more(role.args.len())?)?;
    e.text(&role.name)?;
    for arg in &role.args {
        match arg {
            RoleArg::Name(name) => e.bytes(&name.to_bytes())?,
            RoleArg::Count(count) => e.uint(*count)?,
        }
    }
    Ok(())
}
