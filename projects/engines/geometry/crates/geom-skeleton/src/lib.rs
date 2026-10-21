//! Spare Kit geometry kernel: straight skeletons and roofs.
//!
//! One job: Straight skeletons and roof generation.
//!
//! Layer: 9, Shared operation services (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Exact once the planes are set: every event point is an implicit three-plane point, and events
//!   are ordered by an exact predicate (13.15).
//! - Simultaneous events are found exactly and handled together, never perturbed (13.15).
//! - Plane commits keep the footprint's exact grid symmetries, so symmetric roofs stay symmetric
//!   (13.15).
//! - A result that fails the checker's self-intersection group, or an event the chain rule cannot
//!   order, returns `SkeletonUndefined`; curved eaves without a certified method return
//!   `CurvedSkeletonUnsupported` (13.15).
//!
//! Sections: 13.11, 13.15 and 13.22; open question OQ-13.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G271, G272, G274, G471.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
