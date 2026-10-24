//! Spare Kit geometry kernel: surface quality.
//!
//! One job: Surface quality: continuity, curvature, light lines, draft, gap and flush, deviation.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every measure is a certified maximum or interval from interval and Bernstein bounds, never a
//!   sample of points (16.5).
//! - Roots are separated into proven roots and unresolved intervals, and both are reported (16.5).
//! - Thresholds are inputs; the kernel hard-codes none (16.5).
//! - Display stripes and colours never feed back into a result (16.5).
//!
//! Sections: 16.5 and 16.6; open question OQ-16.1, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Query.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G346, G347, G348, G349.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
