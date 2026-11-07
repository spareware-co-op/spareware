//! Operation schemas: every operation described as data, from which the interfaces, the MCP tools
//! and the documentation are generated.
//!
//! What it does: an [`OpSchema`] is chapter 22's description of one operation: its name, version
//! and family; its inputs and outputs, each with a type and a unit; the roles it names; the errors
//! it can return, each detail with its base kind and its data; the warnings it can give; the
//! exactness classes it can return; its charge schedule; whether it has a batch entry; and an
//! example. The types here
//! are the vocabulary; the facade, `engine-geometry`, holds every operation's schema as data and
//! generates from them (piece G100).
//!
//! Guarantees:
//!
//! - **Constant data.** Every schema type is made of `&'static` strings and slices, so a schema is
//!   a constant, compiled in, with no allocation and nothing to initialise.
//! - **Errors name their base kind.** An [`ErrorSpec`] gives each detail its [`ErrorKind`], as
//!   chapter 22 asks every error table and schema to.
//! - **Classes without bounds.** A schema states which [`Class`]es its operation can return; the
//!   bound is a property of each result, and [`Class::of`] reads the class of one.
//!
//! Method: chapter 22's list of what each operation's schema states, chapter 10.2's families and
//! chapter 9's exactness classes, written as types
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`,
//! `projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/09-representations.md`).
//!
//! Cost: none at run time: schemas are constants.

use geom_num::Exactness;

use crate::kind::ErrorKind;
use crate::run::Charge;

/// The families of chapter 10.2's catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    /// Points, curves, surfaces and primitives, and bodies from faces.
    Make,
    /// Drafting entities, regions, hatches and 2D operations (chapter 14).
    TwoD,
    /// Constrained profiles (chapter 15).
    Sketch,
    /// Extrude, revolve, sweep, loft and their kin (chapter 13).
    Sweep,
    /// Unite, subtract, intersect, fuse, imprint and split (chapter 11).
    Combine,
    /// Fillets, chamfers and blends (chapter 13).
    Blend,
    /// Offsets, shells, thickening, drafts and face edits (chapter 13).
    Offset,
    /// Move, rotate, mirror, scale and patterns.
    Transform,
    /// Import healing, sewing and small-feature removal.
    Repair,
    /// Classification, measurement, distances, mass properties and surface quality.
    Query,
    /// Sections, projections, hidden lines and patterns (chapter 18).
    Draw,
    /// Tessellation, mesh booleans and repair, volume meshes (chapter 19).
    Mesh,
    /// Swept volumes of tools, slicing and parting lines.
    Manufacture,
    /// Subdivision, fields, conversions, panels and unrolling (chapter 16).
    FreeForm,
    /// Terrain, contours, cut and fill, alignments and skeletons (chapters 13 and 17).
    Ground,
}

/// An exactness class without its bound (chapter 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    /// Decided entirely by exact predicates on exact data.
    Exact,
    /// Within a proven bound of the true shape.
    Certified,
    /// Within a measured, not proven, deviation.
    Approximate,
}

impl Class {
    /// The class of `exactness`, without its bound.
    #[must_use]
    pub const fn of(exactness: Exactness) -> Class {
        match exactness {
            Exactness::Exact => Class::Exact,
            Exactness::Certified { .. } => Class::Certified,
            Exactness::Approximate { .. } => Class::Approximate,
        }
    }
}

/// The quantity a value measures; the facade converts lengths and angles to and from the
/// program's units at the boundary (chapter 27).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// No unit: a flag, a choice, a name, an entity or a body.
    None,
    /// A length, in grid steps of the frame.
    Length,
    /// An angle.
    Angle,
    /// An area.
    Area,
    /// A volume.
    Volume,
    /// A curve or surface parameter.
    Parameter,
    /// A count.
    Count,
    /// A ratio, such as a rise over a run.
    Ratio,
}

/// One input or output of an operation, or one field of an error's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Field {
    /// Its name, as the interfaces write it.
    pub name: &'static str,
    /// Its type, by the name the facade gives it: `Body`, `Length`, `[NameId]`.
    pub ty: &'static str,
    /// The quantity it measures.
    pub unit: Unit,
    /// Whether a caller may leave it out.
    pub optional: bool,
    /// What it is, in plain words.
    pub doc: &'static str,
}

/// A role an operation names what it makes by (chapter 20.2): `cap.start`, `side`, `roll`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoleSpec {
    /// The role's name.
    pub name: &'static str,
    /// What it names, in plain words.
    pub doc: &'static str,
}

/// An error an operation can return: one detail, its base kind, and its data (chapter 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ErrorSpec {
    /// The detail's name, such as `RadiusTooLarge`.
    pub detail: &'static str,
    /// Its base kind.
    pub kind: ErrorKind,
    /// The data it carries.
    pub data: &'static [Field],
    /// When it happens, in plain words.
    pub doc: &'static str,
}

/// A warning an operation can give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiagnosticSpec {
    /// Its code, as [`Diagnostic`](crate::Diagnostic) carries it.
    pub code: &'static str,
    /// When it is given, in plain words.
    pub doc: &'static str,
}

/// One operation, described as data (chapter 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OpSchema {
    /// Its name: `combine.subtract`.
    pub name: &'static str,
    /// Its version, which fixes its results, its charge schedule and its details (chapter 26.2).
    pub version: u32,
    /// Its family in chapter 10.2's catalogue.
    pub family: Family,
    /// What it does, in plain words.
    pub doc: &'static str,
    /// Its inputs.
    pub inputs: &'static [Field],
    /// Its outputs.
    pub outputs: &'static [Field],
    /// The roles it names what it makes by.
    pub roles: &'static [RoleSpec],
    /// The errors it can return, besides `Cancelled` and `OutOfMemory`, which every operation can.
    pub errors: &'static [ErrorSpec],
    /// The warnings it can give.
    pub diagnostics: &'static [DiagnosticSpec],
    /// The exactness classes it can return.
    pub classes: &'static [Class],
    /// Its charge schedule: what each phase charges, fixed by its version (chapter 23.10).
    pub charges: &'static [Charge],
    /// Whether it has a batch entry with one result per item (chapter 10.1).
    pub batch: bool,
    /// Whether its geometry does not depend on names, so that calls on equal geometry may share a
    /// cache entry and have their names filled in afterwards (23.8).
    pub name_free: bool,
    /// An example call and its result, in plain words and values.
    pub example: &'static str,
}
