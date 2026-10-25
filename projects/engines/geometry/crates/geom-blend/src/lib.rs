//! Spare Kit geometry kernel: fillets, chamfers and blends.
//!
//! One job: Fillets, chamfers, vertex blends.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never returns a body with a hole, a missing face or a face without geometry; never changes its
//!   inputs or any other body; a failed or cancelled operation publishes nothing (13.21, 10.1).
//! - Never grows a tolerance, retries with nudged inputs, or stitches a result afterwards so it
//!   passes the checker (13.21).
//! - Every blend face is tangent to its supports within its certified angular bound, and every
//!   derivative comes from a kind's derivative trait, never from finite differences (13.21, 13.22).
//! - Continuity is chosen (G1, G2, G3) and measured on every result: G2 where proven, otherwise a
//!   typed error offering G1 (13.22).
//! - A fillet names its faces `roll(edge)` (20.2).
//!
//! Sections: 13.10, 13.21 and 13.22; open question OQ-13.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Blend.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G262, G263, G264, G265, G266, G267, G268, G269.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
