//! Spare Kit geometry kernel: `LandXML` exchange.
//!
//! One job: `LandXML` 1.2 surfaces, alignments, profiles, COGO points and parcels.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Reads through `geom-exchange`'s hardened XML reader (24.7).
//! - Spiral types with a published definition map to curve kinds; the others are kept as their
//!   `LandXML` data, and an operation that must evaluate one fails with an error naming the missing
//!   kind (17.13).
//! - Waits for a terms check that gives `LandXML` a row in section 29 (owner action G408; 24.11,
//!   29.19).
//!
//! Sections: 17.11, 17.13 and 24.11; open questions OQ-17.3 and OQ-24.6, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G437.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
