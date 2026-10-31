//! Spare Kit geometry kernel: the validity checker.
//!
//! One job: The B-rep validity checker.
//!
//! Layer: 5, Checker (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Checks run in groups: combinatorial, geometric consistency, self-intersection and the
//!   exactness record (8.3).
//! - Geometric consistency is certified with bounds, never sampled (8.3).
//! - Self-intersection checks use the same certified intersections the operations do
//!   (ARCHITECTURE.md, layers).
//! - The cheap groups run after every operation in every build; the full set runs in debug builds,
//!   in continuous integration, on every import and on request (8.3).
//! - The checker reports what fails with the entities involved and never modifies a body (principle
//!   4).
//!
//! Sections: 8.3, 10.2, 16.1, 18.10, 19.2, 22, 24.7 and 28.4, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (pieces G84, the framework and groups 1 and 4, and G85, groups 2 and 3 for
//! primitive bodies):
//!
//! - [`check`], in `run`: one check of a body at a [`Level`], returning a [`Report`].
//! - [`report`]: the [`Group`]s of 8.3, the [`Level`]s, the [`Entity`] a [`Fault`] is at, its
//!   [`FaultKind`], and the [`Report`].
//! - `combinatorial`: group 1, loops close, fin partners, shells closed and oriented, regions,
//!   Euler's formula.
//! - `geometric`: group 2, vertices on their faces' surfaces and at their edges' curve ends, and
//!   edges on their faces, within the body's bound: exactly on flat faces given by integers,
//!   certified on lines, circles, planes, cylinders, cones, spheres and ring tori; every other
//!   carrier is reported unchecked, never passed.
//! - `curved`: the closed forms group 2 and group 3 certify their bounds with.
//! - `self_intersection`: group 3 for primitive bodies, a face's edges meeting only at shared
//!   vertices and faces only along shared edges; exact on segments and flat convex polygons,
//!   certified on circles; every other pair is reported unchecked, never passed.
//! - `exactness`: group 4, the exactness record against the geometry.
//! - `sink`: the faults found so far, capped at [`MAX_FAULTS`], and the caller's meter.
//!
//! Status: the framework and groups 1 and 4 (G84) and groups 2 and 3 for primitive bodies (G85)
//! built. Group 3 tests primitives with its own exact predicates and closed forms, as
//! `geom-intersect` is not built yet; plane-based bodies, pcurves and general surfaces come in
//! G102, G134, G177 and G322.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod combinatorial;
mod curved;
mod exactness;
mod geometric;
pub mod report;
mod run;
mod self_intersection;
mod sink;

pub use report::{Entity, Fault, FaultKind, Group, Level, Report};
pub use run::check;
pub use sink::MAX_FAULTS;
