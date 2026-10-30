//! Spare Kit geometry kernel: topology.
//!
//! One job: Bodies to vertices in arenas; the builder; attributes and continuity records; regions
//! and cells; the instance record.
//!
//! Layer: 3, Topology (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Entities live in arenas addressed by 32-bit typed indices: no pointers, no reference counting
//!   between entities, no identity by memory address (8.2).
//! - The builder assigns indices in persistent-name order, so a body's arenas depend only on what
//!   it contains, never on how it was built (8.2, proposed).
//! - A body is immutable after building; an edit makes a new version (8.2).
//! - Ordered containers only: maps that can be iterated are ordered by stable keys (8.2, 21).
//! - Indices are never persisted; references and files hold persistent names (8.2, 20.1).
//! - Each edge has one primary geometry; pcurves are derived and checked against it, never kept in
//!   loose agreement by a growing tolerance (8.1).
//! - Attributes are keyed by persistent name and never decide shape (8.4).
//!
//! Sections: 8, 10.2, 13.2, 18.7, 19.12, 22 and 23.9, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G81, in progress):
//!
//! - [`order`]: [`order::NameKey`], a persistent name's bytes, and [`order::Permutation`], the move of
//!   an arena's rows into name order that the builder applies before publishing (8.2, 32.17).
//!
//! Status: scaffold only; built by pieces G80, G81, G90, G102, G111, G112, G143, G145, G247, G398.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod order;
