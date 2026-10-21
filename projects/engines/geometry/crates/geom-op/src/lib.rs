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
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G82, G83, G86, G87, G88, G104, G143, G144, G459.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
