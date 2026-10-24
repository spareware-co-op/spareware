//! Shared engine: geometry core, the stable public facade of the Spare Kit geometry kernel.
//!
//! One job: The stable facade: schemas, stable value types, re-exports; no kernel logic.
//!
//! Layer: 13, Facade (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - It is the only crate programs depend on for geometry: programs, the C interface and the
//!   WebAssembly and Python bindings all reach the kernel through it, and every `geom-*` crate
//!   stays internal (25).
//! - No kernel logic: it holds the operation schemas, the stable value types, re-exports of kernel
//!   crates' public items and the conversion of `kit-core`'s permanent IDs and units at the
//!   boundary, and no operation, query, dispatch, hashing, cache or trait implementation that
//!   computes anything (27).
//! - Stable tier: semantic versioning, where a breaking change is a new major version that runs
//!   beside the old one, with `cargo-semver-checks` before every release (25).
//! - No unsafe code; the bindings that need it live in crates of their own (27).
//! - It never depends on a development crate, and reaches threads only through `geom-pool`, behind
//!   its `threads` feature (23.4, 27).
//!
//! Sections: 5, 22, 25 and 27 of the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Used by Spare Construct (every discipline, civil ground included), Spare Studio's 3D app and
//! the simulation engine.
//!
//! Its direct dependencies are `kit-core`, `geom-pool` behind the `threads` feature, and every
//! kernel crate it re-exports (none yet: G100 adds them), checked by the scaffold test in
//! `tests/scaffold`, which holds the kernel's crate table.
//!
//! Status: scaffold only; built by pieces G100 and G482, and every operation piece adds its schema
//! here (chapter 30.1, the standard bar).
#![forbid(unsafe_code)]
