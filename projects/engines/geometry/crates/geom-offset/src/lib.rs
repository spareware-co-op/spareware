//! Spare Kit geometry kernel: offsets, shells and face edits.
//!
//! One job: Offsets, shells, thickening, drafts, face edits, defeaturing.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never returns a body with a hole, a missing face or a face without geometry; never changes its
//!   inputs or any other body; a failed or cancelled operation publishes nothing (13.21, 10.1).
//! - Never ignores a cusp, loop or self-intersection, never grows a tolerance, and never retries
//!   with nudged inputs or stitches a result afterwards so it passes the checker (13.21).
//! - Offsets are certified wherever section 12's certified intersection succeeds; near-tangent
//!   self-contact fails and offers the mesh route explicitly (13.22).
//! - Offsetting by d then by -d returns the original wherever no topology change was reported
//!   (13.22).
//! - Small-feature removal is the explicit defeature operation, never a side effect of healing
//!   (13.13, 24.9).
//!
//! Sections: 13.11, 13.12, 13.13, 13.22 and 24.8; open question OQ-13.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G270, G275, G276, G277, G278, G279.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
