//! Spare Kit geometry kernel: entry points across representations.
//!
//! One job: Entry points across representations: the one boolean, dispatch, the validity query.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Section 9's rule: the B-rep boolean for B-rep operands, the exact mesh boolean if any operand
//!   is a mesh, the field boolean if any is a field; point clouds are refused with `InvalidOperand`
//!   (11.8).
//! - Nothing falls back silently: a failed B-rep boolean offers the mesh or field path as a remedy,
//!   and the caller chooses (11.8).
//! - A result's class is the least exact of its inputs, with bounds added (9).
//! - A preview carries its real class, is marked as a preview, and is never stored or logged as
//!   model geometry (11.8).
//! - The validity query runs each representation's own checker group (27).
//!
//! Sections: 9, 11.8, 13.18, 19.2 and 19.5; open question OQ-11.2, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Combine.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G336, G337.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
