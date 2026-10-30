//! What a successful operation returns besides its own result: the result's exactness class and
//! the warnings it carries.
//!
//! What it does: an [`OpOutput`] is chapter 10.1's `OpOutput`: the operation's own result (bodies,
//! meshes, a solved sketch: each crate's type), with the parts every operation shares. This piece
//! builds two of them: the result's [`Exactness`] and its [`Diagnostic`]s, warnings with the
//! entities they concern ("a short edge", "a near-tangency handled"). The other two shared parts,
//! the new persistent names and the history of chapter 20.3, join it with their types, in pieces
//! G87 and G88.
//!
//! Guarantees:
//!
//! - **A result states its class.** Every output carries the exactness class and bound of its
//!   result (principle 3); an output is made only with one ([`OpOutput::new`]).
//! - **Warnings are data.** A [`Diagnostic`] has a stable code, the entities it concerns, the
//!   values that explain it and plain words; its code is lowercase ASCII letters, digits, `.` and
//!   `_`, 1 to [`CODE_MAX`] bytes, so it hashes and compares the same everywhere.
//! - **Order is the operation's.** Diagnostics keep the order the operation added them in, which a
//!   deterministic operation fixes (chapter 21).
//! - **Room to grow.** `OpOutput` and `Diagnostic` are `#[non_exhaustive]`: made only by their
//!   constructors, so the shared parts still to come join without breaking a caller.
//!
//! Method: chapter 10.1's `OpOutput` and chapter 27's split of it into shared parts and each
//! crate's own result (`projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/27-crates-folders.md`).
//!
//! Cost: an output is its result plus one vector of diagnostics; a diagnostic owns its message and
//! two short vectors.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_num::Exactness;

use crate::evidence::Evidence;
use crate::id::NameId;

/// The longest diagnostic code, in bytes.
pub const CODE_MAX: usize = 64;

/// A warning a successful operation carries (chapter 10.1): what happened, to which entities, with
/// the values that explain it.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Diagnostic {
    /// The stable code, listed in the operation's schema: `bool.sliver`, `sweep.short_edge`.
    pub code: &'static str,
    /// The faces, edges and vertices it concerns.
    pub entities: Vec<NameId>,
    /// The values that explain it.
    pub at: Vec<Evidence>,
    /// The warning in plain words.
    pub message: String,
}

impl Diagnostic {
    /// The warning `code`, with `message`, concerning no entity yet; `None` if `code` is not 1 to
    /// [`CODE_MAX`] bytes of lowercase ASCII letters, digits, `.` and `_`.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Option<Self> {
        let bytes = code.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= CODE_MAX
            && bytes
                .iter()
                .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_');
        valid.then(|| Self {
            code,
            entities: Vec::new(),
            at: Vec::new(),
            message: message.into(),
        })
    }

    /// The same warning concerning `entities` too.
    #[must_use]
    pub fn with_entities(mut self, entities: impl IntoIterator<Item = NameId>) -> Self {
        self.entities.extend(entities);
        self
    }

    /// The same warning with `evidence` added.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.at.push(evidence);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// A successful operation's output (chapter 10.1): its own result `R`, and the parts every
/// operation shares.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct OpOutput<R> {
    /// The operation's own result: bodies, meshes, a solved sketch.
    pub result: R,
    /// The result's exactness class and bound: the least exact of its parts.
    pub exactness: Exactness,
    /// The warnings, in the order the operation added them.
    pub diagnostics: Vec<Diagnostic>,
}

impl<R> OpOutput<R> {
    /// The output `result`, of class `exactness`, with no warnings.
    #[must_use]
    pub const fn new(result: R, exactness: Exactness) -> Self {
        Self {
            result,
            exactness,
            diagnostics: Vec::new(),
        }
    }

    /// The same output with `diagnostic` added after the warnings it has.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }
}
