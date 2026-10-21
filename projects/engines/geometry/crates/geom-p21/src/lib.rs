//! Spare Kit geometry kernel: the ISO 10303-21 encoding.
//!
//! One job: The ISO 10303-21 clear-text encoding: tokenizer, streaming parser, canonical writer.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Schema-free: it knows the encoding, not the entities (24.8).
//! - Reads through the `no_std` byte-source traits, checks every length, walks nesting with an
//!   explicit stack, and returns typed errors with the byte offset or line, never a panic (24.7).
//! - The writer is canonical: canonical instance order and each `f64` as its shortest round-trip
//!   decimal, so write, read, write gives identical bytes (24.10, 24.12).
//! - Starts only once a licensed copy of ISO 10303-21 is obtained; parser methods are studied in
//!   ruststep and foxtrot and the code is our own (24.11).
//!
//! Sections: 24.7, 24.8, 24.10 and 24.11; open question OQ-24.6; 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its dependency on `geom-format`, for the byte sources it parses, is section 30.5's amendment.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G415.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
