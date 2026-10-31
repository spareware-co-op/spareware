//! Spare Kit geometry kernel: the operation contract.
//!
//! One job: The operation contract every crate shares: `OpContext`; the shared parts of `OpOutput`
//! (history, names, exactness, diagnostics); the `OpError` base (`ErrorKind`, `Evidence`,
//! `Remedy`); the exactness class, `OpId` and `OpPath`; the `NameId` value and its hashing under
//! `NameScheme`, `TopoName`, `Role` and `PieceKey`; the history records of 20.3; the operation
//! schema types.
//!
//! Layer: 1, Numbers and the contract (chapter 27's proposed layer table).
//!
//! Every operation has chapter 10.1's shape:
//!
//! ```text
//! fn operation(ctx: &OpContext, input: &OpInput) -> Result<OpOutput<R>, OpError>
//! ```
//!
//! An [`OpContext`] says which operation runs ([`OpPath`]), inside which budget, under whose
//! cancellation and at which [`Precision`]. A success is an [`OpOutput`]: the operation's own
//! result with its exactness class and its warnings ([`Diagnostic`]). A failure is an [`OpError`]:
//! a base [`ErrorKind`] from chapter 22's closed set, read from the operation's own typed
//! [`OpErrorDetail`], with the entities involved, the [`Evidence`], plain words and the
//! [`Remedy`]s that would make it succeed. [`OpContext::run`] runs an operation's body inside its
//! context's budget, charging by its schema's [`Charge`] schedule, and publishes nothing once
//! stopped. Every operation is also described as data by an [`OpSchema`].
//!
//! ```
//! use geom_num::NumError;
//! use geom_op::{ErrorKind, NumDetail, OpError, OpId, OpPath, Remedy};
//!
//! let path = OpPath::new(OpId::from_bytes([7; 16])).step("cut").unwrap();
//! // A coordinate past the frame's range, from the numbers layer, wrapped by meaning.
//! let error = OpError::new(path, NumError::OutOfRange { value: 1 << 41 });
//! assert_eq!(error.kind(), ErrorKind::FrameRangeExceeded);
//! assert_eq!(error.remedies(), [Remedy::ChangeFrame]);
//! // An exact computation over its proven bit budget is a defect, never a budget failure.
//! assert_eq!(NumDetail::from(NumError::BudgetExceeded).kind(), ErrorKind::Internal);
//! ```
//!
//! Invariants this crate keeps:
//!
//! - `OpContext` holds no randomness and no clock that can affect a result (10.1).
//! - `ErrorKind` is section 22's closed set of base kinds and is not `#[non_exhaustive]`; every
//!   `OpError` is built from its detail, so its kind always agrees with the detail's base kind
//!   (22).
//! - A `NameId` is the 128-bit hash of the one canonical encoding of a `TopoName` under a versioned
//!   `NameScheme`; nothing is named by position, memory address or a random suffix (20.1, 20.2).
//! - History records keep section 20.3's laws: a deleted input has no modified outputs, an input's
//!   generated and modified sets do not overlap, every output traces to an input or is generated,
//!   and histories compose across operations.
//! - Batch items are keyed by the caller's stable key, never by position: an item's `OpPath` is the
//!   batch's `OpPath` plus that key (10.1).
//! - Evidence carries only `geom-num` values, so this crate needs no geometry crate (27).
//!
//! Sections: 10.1, 13.2, 13.14, 20, 22, 23.10 and 24.3; chapter 27 ("Who owns what"), in the
//! kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`), and `sha2`, the one hash 32.20 approves.
//!
//! Licences of the published crates this crate depends on (DEPEND, chapter 29.1), as each one's
//! manifest at the version in `Cargo.lock` states it:
//!
//! - MIT OR Apache-2.0: `sha2`.
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-normalization`.
//!
//! Status: the context, the error base and its details for the parallel helper and the numbers
//! layer, the exactness class and warnings of an output, operation identity and the schema types
//! built (G82); budgets in the contract, the charge schedule and fallible reservations (G83); the
//! canonical CBOR encoder ([`cbor`]) and SHA-256 ([`hash`]) built (G86); persistent names and their
//! `NameId`s under `NameScheme v1` ([`name`]) built (G87); history records, their laws and their
//! composition ([`history`]) built (G88); the batch runner to come in G104, per-entity bounds in
//! G143, the local-operation driver in G144 and preview precision in G459.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod cbor;
pub mod context;
pub mod detail;
pub mod error;
pub mod evidence;
pub mod hash;
pub mod history;
pub mod id;
pub mod kind;
pub mod name;
pub mod output;
pub mod remedy;
pub mod run;
pub mod schema;

pub use context::{OpContext, Precision};
pub use detail::{NumDetail, OpErrorDetail, RunDetail};
pub use error::OpError;
pub use evidence::{Evidence, Value};
pub use geom_num::Exactness;
pub use history::{
    ChangeClass, History, HistoryError, LawBreak, LoopChange, Origin, Record, Relations,
    TopologyChange, Vanish,
};
pub use id::{Label, NameId, OpId, OpPath};
pub use kind::ErrorKind;
pub use name::{
    EdgeOrVertex, NAME_SCHEME_V1_PREFIX, NameError, NameList, NameSet, Ordinal, PieceKey, Role,
    RoleArg, Side, TopoName,
};
pub use output::{Diagnostic, OpOutput};
pub use remedy::{Bound, Remedy};
pub use run::{Charge, RUN, reserve};
pub use schema::{Class, DiagnosticSpec, ErrorSpec, Family, Field, OpSchema, RoleSpec, Unit};
