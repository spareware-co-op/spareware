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
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G346): [`continuity`], the certified maximum of G0, G1, G2 or G3 across
//! a join of two faces or a body's edge, with the parameter where it is reached; the measures
//! themselves are `geom-surface`'s Class-A maths (`geom_surface::class_a`), as chapter 16.5
//! places them.
//!
//! Status: continuity across edges built (G346); curvature extremes, light lines, and draft, gap,
//! flush and deviation to come in G347, G348 and G349.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod continuity;
