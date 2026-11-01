//! Spare Kit geometry kernel: topology.
//!
//! One job: Bodies to vertices in arenas; the builder; attributes and continuity records; regions
//! and cells; the instance record.
//!
//! Layer: 3, Topology (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Entities live in arenas addressed by 32-bit typed indices: no pointers, no reference counting
//!   between entities, no identity by memory address (8.2).
//! - The builder assigns indices in persistent-name order, so a body's arenas depend only on what
//!   it contains, never on how it was built (8.2, proposed).
//! - A body is immutable after building; an edit makes a new version (8.2).
//! - Ordered containers only: maps that can be iterated are ordered by stable keys (8.2, 21).
//! - Indices are never persisted; references and files hold persistent names (8.2, 20.1).
//! - Each edge has one primary geometry; pcurves are derived and checked against it, never kept in
//!   loose agreement by a growing tolerance (8.1).
//! - Attributes are keyed by persistent name and never decide shape (8.4).
//!
//! Sections: 8, 10.2, 13.2, 18.7, 19.12, 22 and 23.9, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (pieces G80, the arenas, and G81, in progress):
//!
//! - [`ix`]: [`Ix`], the typed 32-bit index of one row of one arena, the [`Kind`] of row each arena
//!   holds and the [`Arena`]s' names.
//! - [`column`]: [`Column`], one field of one arena, cut into chunks of [`CHUNK_ROWS`] rows that
//!   body versions share until written to, and [`Memory`], what chunks hold, for measurement.
//! - [`entity`]: the rows of chapter 8.1, [`Region`] to [`Vertex`], each with its arena of columns
//!   ([`Regions`] to [`Vertices`]), and [`Link`], every column that holds an index.
//! - [`arenas`]: [`Arenas`], every arena of one body with its four geometry [`Pool`]s.
//! - [`body`]: [`Body`], one shape, immutable once published, and its [`BodyKind`].
//! - [`builder`]: [`Builder`], the one way to make a body: arenas filled or edited, checked and
//!   published.
//! - [`TopoError`], in [`error`]: why an arena could not change or a body could not be published.
//! - [`order`]: [`order::NameKey`], a persistent name's bytes, and [`order::Permutation`], the move of
//!   an arena's rows into name order that the builder applies before publishing (8.2, 32.17).
//! - [`attributes`]: [`attributes::Attributes`], values keyed by persistent name, and the split and
//!   merge classes that say what they do when their entity is split or merged (8.4).
//! - [`definition`]: [`definition::Definition`], an attribute's owner, the entities that may carry
//!   it, its value schema and its behaviour at every event of section 8.4, and
//!   [`definition::Definitions`], a model's definitions, none ever changed (G90).
//!
//! Status: the arenas, their typed indices and the publishing builder (G80) built; the Euler
//! operators, name order, attributes, regions and cells, queries, proofs and instances to come in
//! G81, G90, G102, G111, G112, G143, G145, G247 and G398.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod arenas;
pub mod attributes;
pub mod body;
pub mod builder;
pub mod column;
pub mod definition;
pub mod entity;
pub mod error;
pub mod ix;
pub mod order;

pub use arenas::{Arenas, Pool};
pub use body::{Body, BodyKind};
pub use builder::{Builder, Euler, NewEdge, NewFace, NewVertex, Wire};
pub use column::{CHUNK_ROWS, Column, Memory};
// The geometry kinds a body's pools hold, for the crates above this one that read them (such as
// geom-check's exactness record) without a direct dependency on geom-curve and geom-surface,
// which chapter 27's crate table does not give them.
pub use entity::{
    CurveIx, Edge, EdgeIx, Edges, Face, FaceIx, Faces, Fin, FinIx, Fins, Link, Loop, LoopIx, Loops,
    PcurveIx, PointIx, Region, RegionIx, RegionKind, Regions, Sense, Shell, ShellIx, Shells,
    SurfaceIx, Vertex, VertexIx, Vertices,
};
pub use error::{Refusal, TopoError};
pub use geom_curve::CurveKind;
pub use geom_surface::SurfaceKind;
pub use ix::{Arena, Ix, Kind};
