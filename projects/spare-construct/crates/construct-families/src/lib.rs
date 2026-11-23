//! Spare Construct: family recipes (CON-A7-F1-P1).
//!
//! A family is the template a door, window, piece of furniture or fitting is made from: its
//! parameters, the sketches and solid steps that build its shape from them, its 2D symbols for
//! plans and elevations, what shows at which detail level, its materials, what it is hosted on
//! and its built-in types and type table. A recipe is one JSON document with a published schema. This crate is
//! its reader, checker and writer. Map: projects/spare-construct/ARCHITECTURE.md, "Families";
//! the stored form is FORMAT.md beside this crate, the schema `schema/family-recipe-1.schema.json`
//! ([`SCHEMA`]), and the decisions behind it note CON-A7-F1-P1-recipe-format.
//!
//! - [`model`]: what a recipe holds ([`Recipe`] and its parts), in the format's own terms.
//! - [`Recipe::from_json`] and [`Recipe::from_value`]: the reader. It takes exactly the shape
//!   the schema takes, so the two agree (the tests hold them to it), and refuses anything else
//!   with a [`ReadError`] naming the place as a JSON Pointer.
//! - [`Recipe::check`]: the checker, for what the schema cannot see, against the project's
//!   [`Library`] (categories and materials). Every [`Problem`] names its place.
//! - [`Recipe::to_json`] and [`Recipe::content_id`]: the one canonical written form, and the
//!   [`ContentId`] (SHA-256 of those bytes) that names a recipe version.
//! - [`Recipe::named_types`] and [`Recipe::named_type`]: every type a recipe names, its built-in
//!   types and its type table's rows ([`TypeTable`], [`NamedType`]; CON-A7-F1-P3).
//! - [`family_type()`], [`parameters`], [`family_of`] and [`FamilyRef`]: a named type as a
//!   construct-types type carrying a `@system.family` block that names the family and version.
//! - [`evaluate`](mod@evaluate): a recipe and one type's values made into the family's solids
//!   through the kernel's `Engine` interface, its sketches solved at the type's values
//!   ([`evaluate()`], [`evaluate_within()`], [`Evaluator`] keeping each type it builds by
//!   [`CacheKey`] and evaluating a recipe's named types one by one), within [`Limits`], with
//!   refusals as [`EvalError`] values (CON-A7-F1-P2, P3 and P6; FORMAT.md, "Evaluation").
//!
//! What it builds on, by name: a parameter is a construct-params definition (the same five
//! fields as its own JSON, made by its own reader) with a default in its stored text, or a
//! construct-formulas formula; a family type is a construct-types type with a system block, as
//! wall types are; a sketch holds construct-solver's points, lines, arcs, circles and
//! constraints (its `Side` and `Touch`) with a formula wherever the solver takes a length; steps
//! name only the operations the kernel's `Engine` interface names; materials are
//! construct-materials ids; categories and subcategories are construct-categories ids, which
//! construct-styles' object styles key on, so a symbol's line style is its subcategory's.
//!
//! Promises: the same recipe always gives the same bytes, and reading what was written gives the
//! same recipe; reading takes exactly what the schema takes; no length is held as a number
//! (lengths are construct-params' stored text, checked through it, or formulas); no clock, no
//! random number, no hash-map order; nothing panics. Depends on kit-core, construct-params,
//! construct-types, construct-categories, construct-materials, construct-formulas,
//! construct-solver, engine-geometry (the kernel's interface, for evaluation only), `serde_json`,
//! sha2 and hex. Nothing here is interim. Putting a recipe through the edit log and loading it
//! into a project are later jobs (7.6).
//!
//! Not built here, with its place kept: a hosted family's opening cut in its host (job 6.4).

pub mod check;
pub mod evaluate;
pub mod family_type;
pub mod model;
pub mod read;
pub mod write;

pub use check::{Library, Problem, ProblemKind, Sort};
pub use evaluate::{
    Built, CacheKey, EvalError, Evaluated, Evaluator, Limit, Limits, Place, Reason, TypeResult,
    TypeValues, Used, evaluate, evaluate_within,
};
pub use family_type::{FamilyError, FamilyRef, OWNER, family_of, family_type, parameters};
pub use model::{
    Base, BuiltInType, Condition, Constraint, DefinitionRow, FORMAT_VERSION, Hosting, Key,
    MaterialRef, NamedType, Opening, Operation, Parameter, ParameterValue, Plane, Recipe, Sketch,
    SketchArc, SketchCircle, SketchLine, SketchPoint, Step, Symbol, TableRow, TypeTable, TypeValue,
    View, Visibility,
};
pub use read::ReadError;
pub use write::ContentId;

/// The published JSON Schema (draft 2020-12) of format 1, `schema/family-recipe-1.schema.json`.
pub const SCHEMA: &str = include_str!("../schema/family-recipe-1.schema.json");

impl Recipe {
    /// Reads a recipe from JSON text.
    ///
    /// # Errors
    /// [`ReadError`], naming the place, if the text is not JSON or not in the format's shape.
    pub fn from_json(text: &str) -> Result<Self, ReadError> {
        read::from_json(text)
    }

    /// Reads a recipe from a JSON value.
    ///
    /// # Errors
    /// [`ReadError`], naming the place, if the value is not in the format's shape.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, ReadError> {
        read::from_value(value)
    }

    /// The recipe in its one canonical written form.
    #[must_use]
    pub fn to_json(&self) -> String {
        write::to_json(self)
    }

    /// The content id of this recipe version: the SHA-256 of [`Recipe::to_json`]'s bytes.
    #[must_use]
    pub fn content_id(&self) -> ContentId {
        ContentId::of(self.to_json().as_bytes())
    }

    /// Checks what the recipe means against a project's categories and materials.
    ///
    /// # Errors
    /// Every [`Problem`] found, in document order.
    pub fn check(&self, library: &Library<'_>) -> Result<(), Vec<Problem>> {
        check::check(self, library)
    }
}
