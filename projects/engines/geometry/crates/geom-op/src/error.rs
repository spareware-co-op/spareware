//! The error every operation returns: its base kind, its own typed detail, where it happened, the
//! entities and values involved, plain words, and what would make it succeed.
//!
//! What it does: an [`OpError`] is chapter 22's structured error, the same for people and for AI
//! agents: `kind`, `detail`, `op`, `entities`, `at` (evidence), `message` and `remedies`. It is
//! built from its detail ([`OpError::new`]), which fixes its kind, its first message and the
//! remedies the detail knows; the operation then adds the entities it names, the values that
//! explain it, a fuller message and the remedies only it knows.
//!
//! Guarantees:
//!
//! - **The kind always agrees with the detail.** [`OpError::kind`] is the detail's base kind, read
//!   from the detail every time; nothing can set one without the other (chapter 22).
//! - **Never empty of meaning.** Every error has a message in plain words, the detail's own until
//!   the operation gives a fuller one, and the remedies its detail knows.
//! - **Additions keep their order.** Entities, evidence and remedies are kept in the order the
//!   operation gave them; a deterministic operation gives them in a deterministic order, so equal
//!   inputs give equal errors (chapters 21 and 28.13).
//! - **Replayable unless stopped.** [`OpError::is_replayable`] is false exactly for `Cancelled`
//!   and `OutOfMemory`, which are never logged, stored or cached.
//!
//! Method: chapter 22's `OpError`, a closed base kind with an open, typed detail that the kind is
//! read from (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: an error owns its path, its message and three short vectors; making one allocates them,
//! once. Errors are made only on failure.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::detail::OpErrorDetail;
use crate::evidence::Evidence;
use crate::id::{NameId, OpPath};
use crate::kind::ErrorKind;
use crate::remedy::Remedy;

/// Why an operation returned no result (chapter 22).
#[derive(Clone, Debug, PartialEq)]
pub struct OpError {
    detail: OpErrorDetail,
    op: OpPath,
    entities: Vec<NameId>,
    at: Vec<Evidence>,
    message: String,
    remedies: Vec<Remedy>,
}

impl OpError {
    /// The error `detail` at the operation or step `op`, with the detail's message and remedies,
    /// and no entities or evidence yet.
    #[must_use]
    pub fn new(op: OpPath, detail: impl Into<OpErrorDetail>) -> Self {
        let detail = detail.into();
        Self {
            message: detail.to_string(),
            remedies: detail.remedies(),
            detail,
            op,
            entities: Vec::new(),
            at: Vec::new(),
        }
    }

    /// The same error naming `entities` too, after those it names already.
    #[must_use]
    pub fn with_entities(mut self, entities: impl IntoIterator<Item = NameId>) -> Self {
        self.entities.extend(entities);
        self
    }

    /// The same error with `evidence` added after the evidence it has.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.at.push(evidence);
        self
    }

    /// The same error with `remedy` added after the remedies it has.
    #[must_use]
    pub fn with_remedy(mut self, remedy: Remedy) -> Self {
        self.remedies.push(remedy);
        self
    }

    /// The same error with `message`, in plain words, in place of the one it has.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// The base kind: the detail's.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.detail.kind()
    }

    /// The operation's own typed detail.
    #[must_use]
    pub const fn detail(&self) -> &OpErrorDetail {
        &self.detail
    }

    /// The operation and step that failed.
    #[must_use]
    pub const fn op(&self) -> &OpPath {
        &self.op
    }

    /// The faces, edges and vertices involved, in the order the operation gave them.
    #[must_use]
    pub fn entities(&self) -> &[NameId] {
        &self.entities
    }

    /// The points, parameters, distances and bounds that explain the error.
    #[must_use]
    pub fn evidence(&self) -> &[Evidence] {
        &self.at
    }

    /// The error in plain words.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// What would make the operation succeed.
    #[must_use]
    pub fn remedies(&self) -> &[Remedy] {
        &self.remedies
    }

    /// Whether the error is a deterministic answer that may be logged, stored and cached: false
    /// exactly for `Cancelled` and `OutOfMemory` (chapter 22).
    #[must_use]
    pub const fn is_replayable(&self) -> bool {
        self.kind().is_replayable()
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.kind(), self.op, self.message)
    }
}

impl core::error::Error for OpError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.detail)
    }
}
