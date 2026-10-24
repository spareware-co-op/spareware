//! Spare Kit geometry kernel: exact meshes.
//!
//! One job: Exact meshes, watertight tessellation, the exact constrained Delaunay triangulation,
//! levels of detail and display meshes, mesh booleans and modelling, repair, volume meshes,
//! parameterisation and flattening.
//!
//! Layer: 7, Meshes and spatial (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - An exact mesh is exact data on the grid, decided by the same exact predicates as the rest of
//!   the kernel, with a proven bound on its distance from the shape it stands for (19).
//! - The display copy is a different type that no operation accepts (19, 19.4).
//! - Tessellation is watertight: each edge is sampled once and shared by its faces; where a bound
//!   cannot be reached it returns `BoundNotReached`, never a mesh that crosses itself (16.3,
//!   19.13).
//! - The constrained Delaunay triangulation is unique: every tie breaks by one rule, so an
//!   incremental edit and a full rebuild give the same triangles (17.3, 19.14).
//! - A flat map is committed and checked with exact predicates before it is used (16.4).
//!
//! Sections: 16.4, 16.6, 17.3, 19.2 to 19.8, 19.14, 23.11 and 24.4; open questions OQ-16.1 and
//! OQ-19.2, in the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Family: Mesh.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G146, G147, G148, G149, G150, G151, G152, G153, G288,
//! G289, G290, G291, G298, G299, G300, G301, G302, G303, G304, G305, G306, G319, G341, G350, G411.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
