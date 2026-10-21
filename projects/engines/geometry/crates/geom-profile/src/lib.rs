//! Spare Kit geometry kernel: profiles between sketches and the model.
//!
//! One job: Carrying geometry between sketches and the model: references in, regions out, selection
//! by name, profile checks and names.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Construction geometry never enters a profile (15.14).
//! - Regions are chosen by name; where sketch curves cross, the pieces are named as splits by the
//!   crossing curves and the side (15.14).
//! - Its errors say what is wrong with a profile: an open loop, a self-intersection, a dangling
//!   curve, a region that is not there (15.14).
//! - A sketch that stops solving during regeneration is model state, `SketchUnsolved`, never stale
//!   geometry passed off as current (15.14).
//! - Tangency declared in a sketch travels as a record on the shared vertex, never re-derived from
//!   angles (15.14).
//!
//! Sections: 15.11 and 15.14; open question OQ-15.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G229, G230.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
