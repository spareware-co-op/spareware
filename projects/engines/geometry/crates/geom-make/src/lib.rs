//! Spare Kit geometry kernel: primitives and transforms.
//!
//! One job: The Make and Transform families of 10.2: primitives, transforms and patterns.
//!
//! Layer: 9, Shared operation services (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every result passes all four checker groups before it is returned; inputs are never changed
//!   (principle 4, 8.3).
//! - Primitives name their faces by role (20.2).
//! - Transforms are typed so a change of kind is explicit: whole-step translations, quarter turns
//!   and mirrors stay exact; other rotations round to the grid and the result says so (7).
//! - Attribute events (Transformed, Copied) follow the history; a pattern declares its instances as
//!   copies in its schema (8.4).
//!
//! Sections: 7, 8.4, 10.2 and 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Make and Transform.
//!
//! Chapter 30 places primitives here because no crate in chapter 27 owns them, and they cannot live
//! in `geom-topo`: every operation runs the checker before it returns, and `geom-check` sits above
//! `geom-topo`.
//!
//! Its direct dependencies are its row in chapter 30.5's amendment to chapter 27's crate table,
//! checked by the scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Proposed in chapter 30, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G101.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
