//! Spare Kit geometry kernel: implicit fields.
//!
//! One job: Implicit fields: expression graph, evaluation, interval bounds, gradients, field
//! bodies, certified meshing and slicing, lattices, the signed heat method; implements
//! `ImplicitBound`.
//!
//! Layer: 8, Other representations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Positions are grid points, lengths whole grid steps and directions integer vectors; only
//!   dimensionless coefficients are `f64`, and they are finite (16.2).
//! - The expression graph is hash-consed with canonical bytecode, so the same field always gives
//!   the same bytes; equal hashes over different bytes are a typed error, never a merge (16.2).
//! - Only bit-exact rewrites: commutative operands may be sorted, never reassociated (16.2).
//! - Opcodes are versioned and every old version is read forever (16.2).
//! - Meshing is Certified by interval bounds, and Approximate only when asked (9, 16.3).
//!
//! Sections: 12.1, 13.17, 16.2, 16.6, 19.7 and 19.14; open questions OQ-12.1 and OQ-16.1, in the
//! kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Family: Free-form.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G326, G327, G328, G329, G330, G331, G332, G338, G339,
//! G352, G469.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
