//! Spare Kit geometry kernel: STEP exchange.
//!
//! One job: Geometry mapping to and from STEP: reads AP242, AP214 and AP203; writes AP242 and
//! AP214.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every Part 42 curve and surface maps to its own kind; an intersection curve's 3D curve is the
//!   sender's approximation and is re-certified (24.9, 24.11).
//! - Every written file passes a validator generated from the EXPRESS schemas (24.11).
//! - Write, read, write gives identical bytes (24.12).
//! - Starts only once licensed copies of ISO 10303-21, ISO 10303-42 and the application protocols
//!   are obtained; their text is never copied (24.11, 29.14).
//!
//! Sections: 24.8 to 24.12 and 29.14, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! The kinds it maps, from section 24.11: every Part 42 curve and surface, B-rep and faceted
//! bodies, tessellated geometry and placed instances. Generated EXPRESS validator tables (G416) are
//! committed here by `geom-gen`.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G427, G428, G429, G476.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
