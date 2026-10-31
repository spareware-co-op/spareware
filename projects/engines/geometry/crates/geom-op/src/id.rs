//! Operation identity: an operation's permanent ID, the path to one of its steps or batch items,
//! and the value of a persistent name.
//!
//! What it does: an [`OpId`] is the permanent 128-bit ID of one operation, set when the operation
//! is authored and kept in the operation log forever (architecture chapter 20.1). An [`OpPath`]
//! adds sub-step labels to it: a [`Label::Step`] named by the operation, or a [`Label::Key`], the
//! stable key the caller gives a batch item (chapter 10.1), so an item's path is the batch's path
//! plus that key and never its position. A [`NameId`] is the 128-bit value of a persistent name
//! (chapter 20.1), held here as its sixteen bytes so that errors and diagnostics can name the
//! entities they concern; computing one from a `TopoName` under `NameScheme v1` is piece G87's.
//!
//! Guarantees:
//!
//! - **Nothing is named by position or by chance.** An [`OpPath`] holds only the [`OpId`] and the
//!   labels given to it, in the order given; a batch item's label is the caller's key, so adding an
//!   item to a batch changes no other item's path.
//! - **Every label hashes the same everywhere.** A step label is 1 to [`STEP_MAX`] bytes of
//!   printable ASCII or spaces, not starting or ending with a space. ASCII text is in Unicode
//!   Normalization Form C under every Unicode version, so a step label meets section 32.19's rule
//!   for text hashed into a name without a normalisation table.
//! - **Order is byte order.** IDs, names and paths compare by their bytes and labels (a step before
//!   a key at the same depth, then by bytes), the order section 32.19 sorts names in: the order of
//!   their canonical encodings, which for sixteen-byte strings is the order of the bytes.
//! - **Shown as their value.** `Debug` and `Display` print an ID or a name as 32 lowercase
//!   hexadecimal digits, the most significant byte first, and a path as its ID followed by its
//!   labels; nothing prints an address.
//!
//! Method: chapter 20.1's three kinds of identity and section 32.19's canonical form of an `OpPath`
//! ("an array of the `OpId`'s 16 bytes followed by its labels"), in
//! `projects/engines/geometry/architecture/20-naming-history.md` and
//! `projects/engines/geometry/architecture/32-decisions.md`, with chapter 10.1's rule that a batch item is keyed by the caller's key
//! (`projects/engines/geometry/architecture/10-operations.md`).
//!
//! Cost: IDs and names are sixteen bytes, copied by value. A path holds its labels in one vector;
//! a child path copies its parent's labels once and adds one, so it allocates once.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// The longest step label, in bytes.
pub const STEP_MAX: usize = 64;

/// Writes sixteen bytes as 32 lowercase hexadecimal digits, the first byte first.
fn hex(bytes: &[u8; 16], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}

/// The permanent ID of one operation (chapter 20.1): set when the operation is authored, stored in
/// the operation log, and the same in every copy of the document.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OpId([u8; 16]);

impl OpId {
    /// The ID whose sixteen bytes are `bytes`, the most significant first.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The ID's sixteen bytes, the most significant first.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for OpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OpId(")?;
        hex(&self.0, f)?;
        f.write_str(")")
    }
}

impl fmt::Display for OpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex(&self.0, f)
    }
}

/// The value of a persistent name (chapter 20.1): 128 bits that say how an entity was made.
///
/// This is the value only. A `NameId` is computed from the canonical encoding of a `TopoName`
/// under a versioned `NameScheme` (section 32.19), which piece G87 adds; until then a name comes
/// from its bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameId([u8; 16]);

impl NameId {
    /// The name whose sixteen bytes are `bytes`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The name's sixteen bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for NameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NameId(")?;
        hex(&self.0, f)?;
        f.write_str(")")
    }
}

impl fmt::Display for NameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex(&self.0, f)
    }
}

/// One label of an [`OpPath`].
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Label {
    /// A sub-step the operation names, such as `cut` or `blend faces`: printable ASCII and spaces.
    Step(String),
    /// A batch item's stable key, given by the caller: a permanent ID such as the item's element.
    Key([u8; 16]),
}

impl Label {
    /// The step label `name`, or `None` if `name` is not 1 to [`STEP_MAX`] bytes of printable
    /// ASCII (`!` to `~`) and spaces, or starts or ends with a space.
    #[must_use]
    pub fn step(name: &str) -> Option<Label> {
        let bytes = name.as_bytes();
        let printable = bytes.iter().all(|&b| matches!(b, b' '..=b'~'));
        let trimmed = bytes.first() != Some(&b' ') && bytes.last() != Some(&b' ');
        (!bytes.is_empty() && bytes.len() <= STEP_MAX && printable && trimmed)
            .then(|| Label::Step(String::from(name)))
    }

    /// The key label of a batch item whose permanent ID is `key`.
    #[must_use]
    pub const fn key(key: [u8; 16]) -> Label {
        Label::Key(key)
    }
}

impl fmt::Debug for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Label::Step(name) => write!(f, "{name:?}"),
            Label::Key(key) => {
                f.write_str("#")?;
                hex(key, f)
            }
        }
    }
}

/// An operation and the step or batch item inside it (chapter 20.1): the operation's [`OpId`]
/// followed by labels.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OpPath {
    op: OpId,
    labels: Vec<Label>,
}

impl OpPath {
    /// The path of the operation `op` itself, with no labels.
    #[must_use]
    pub const fn new(op: OpId) -> Self {
        Self {
            op,
            labels: Vec::new(),
        }
    }

    /// The operation.
    #[must_use]
    pub const fn op(&self) -> OpId {
        self.op
    }

    /// The labels after the operation's ID, outermost first.
    #[must_use]
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// This path with `label` added at the end.
    #[must_use]
    pub fn child(&self, label: Label) -> OpPath {
        let mut labels = Vec::with_capacity(self.labels.len().saturating_add(1));
        labels.extend_from_slice(&self.labels);
        labels.push(label);
        OpPath {
            op: self.op,
            labels,
        }
    }

    /// The path of this operation's step `name` (see [`Label::step`]), or `None` if `name` is not a
    /// step label.
    #[must_use]
    pub fn step(&self, name: &str) -> Option<OpPath> {
        Label::step(name).map(|label| self.child(label))
    }

    /// The path of the batch item the caller keys `key` (chapter 10.1): this path plus that key.
    #[must_use]
    pub fn item(&self, key: [u8; 16]) -> OpPath {
        self.child(Label::Key(key))
    }

    /// Whether `other` is this path or lies inside it: the same operation, and this path's labels
    /// are the first labels of `other`'s.
    #[must_use]
    pub fn contains(&self, other: &OpPath) -> bool {
        self.op == other.op && other.labels.starts_with(&self.labels)
    }
}

impl fmt::Debug for OpPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpPath({self})")
    }
}

impl fmt::Display for OpPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex(&self.op.0, f)?;
        for label in &self.labels {
            write!(f, "/{label}")?;
        }
        Ok(())
    }
}
