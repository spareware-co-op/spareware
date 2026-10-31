//! The stable value types: what a program holds, passes to an operation and reads back. Each is a
//! kernel crate's own type, re-exported here, so a program names it from this crate and never
//! from a `geom-*` crate (25).
//!
//! What it does: re-exports, grouped as chapter 25 lists the Stable tier's value types: grid points
//! and steps ([`grid`]), exactness ([`Exactness`]), curves ([`curve`]), surfaces ([`surface`]),
//! bodies ([`body`]), names and history ([`name`]), errors ([`error`]), and an operation's context
//! and output ([`op`]). Anything not re-exported here is internal.
//!
//! Guarantees:
//!
//! - **Only re-exports.** Every item is a `pub use` of a kernel crate's public item, so the types
//!   are the kernel's own and a value passes between the facade and the kernel unchanged.
//! - **One name each.** No type is re-exported under two paths, and the two kinds named `Offset`
//!   keep their own modules ([`curve::Offset`], [`surface::Offset`]).
//!
//! Method: none; re-exports only, as chapter 25 sets the Stable tier's value types
//! (`projects/engines/geometry/architecture/25-public-interface.md`).
//!
//! Cost: none at run time.

pub use geom_num::Exactness;

/// Grid points, coordinates and steps (chapter 6).
pub mod grid {
    pub use geom_num::{Coord, GridStep, P2, P3, Steps};
}

/// Curves: the kinds of chapter 7, in two and three dimensions, and the frames they are placed on.
pub mod curve {
    pub use geom_curve::{
        BulgePolyline, Circle, Composite, CurveKind, Ellipse, Frame, Helix, Hyperbola, Involute,
        Line, Offset, OnSurface, Parabola, Polynomial, Transition, Trimmed,
    };
}

/// Surfaces: the kinds of chapter 7.
pub mod surface {
    pub use geom_surface::{
        AffineSurface, Cone, Cylinder, Extrusion, Offset, Plane, Revolution, Ruled, Sphere,
        SurfaceKind, Torus,
    };
}

/// Bodies: one shape, immutable once published (chapter 8).
pub mod body {
    pub use geom_topo::{Body, BodyKind};
}

/// Persistent names and the history that carries them (chapter 20).
pub mod name {
    pub use geom_op::{
        ChangeClass, History, NameId, Ordinal, PieceKey, Record, Role, Side, TopoName,
    };
}

/// Errors: the closed set of base kinds, the typed details, and what the caller can do
/// (chapter 22).
pub mod error {
    pub use geom_op::{
        ErrorKind, Evidence, NumDetail, OpError, OpErrorDetail, Remedy, RunDetail, Value,
    };
}

/// An operation's context and output (chapter 10.1).
pub mod op {
    pub use geom_op::{Diagnostic, Label, OpContext, OpId, OpOutput, OpPath, Precision};
}
