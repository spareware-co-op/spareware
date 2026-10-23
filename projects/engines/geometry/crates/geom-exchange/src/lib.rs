//! Spare Kit geometry kernel: what every format crate shares.
//!
//! One job: What every format crate shares: units as exact ratios, numbers to grid, frames from
//! files, the ISO 10303-42 mapping, the hardened XML and zip reader, reports.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never guesses a unit, a frame or a tolerance (24.8, 24.13).
//! - A decimal length is parsed straight into integers and rounded once, half to even, never
//!   through `f64`, and every rounding is recorded (24.8).
//! - The XML and zip reader takes no document type definitions or external entities, holds depth,
//!   attributes, text and decompression to budgets, and refuses duplicate or path-escaping zip
//!   entries (24.7).
//! - The same bytes, options and kernel edition always give a bit-identical model; one failed
//!   element is one failed item, never a failed file (24.8).
//! - The shared Part 42 mapping starts only once licensed copies of the ISO 10303 parts are
//!   obtained (24.11).
//!
//! Sections: 24.7, 24.8, 24.10 and 24.11; open question OQ-24.6; 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its dependencies on `geom-format`, `geom-p21`, `geom-intersect` and `geom-check` are section
//! 30.5's amendment: the ISO 10303-42 mapping parses Part 21 entities and builds certified Tolerant
//! bodies.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G413, G414, G418, G436.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
