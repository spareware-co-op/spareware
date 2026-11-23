//! Recipe evaluation (CON-A7-F1-P2, with the named types and type tables of CON-A7-F1-P3 and the
//! limits of CON-A7-F1-P6): a family recipe and one type's values made into the family's solids
//! through the kernel's `Engine` interface, kept per type.
//!
//! What it does, in order (FORMAT.md, "Evaluation"; the choices are note
//! CON-A7-F1-P2-recipe-evaluation):
//!
//! 1. **Values** ([`TypeValues`]): a named type's values (a built-in type's, or a type table
//!    row's, column by column) or values given by hand; each is checked as construct-params
//!    checks it; every other parameter reads its default; every formula is worked out with
//!    construct-formulas over the numbers, units checked, and a refusal names the type and the
//!    parameter.
//! 2. **Sketches**: each sketch a step uses becomes construct-solver's sketch, its constraint
//!    lengths the formulas' values, its points and radii starting where they are stored, and is
//!    solved there (`construct_solver::solve`): the least-change answer nearest what was drawn,
//!    on the grid by the solver's own rounding rule, the same on every run. A part no constraint
//!    holds stays as drawn; a sketch the solver refuses is refused naming the sketch and the
//!    constraint.
//! 3. **Steps**, in recipe order: an extrusion between two heights along its plane's normal, a
//!    subtraction (an extrusion used as its tool is a void, and cuts the target named), a union.
//!    A step a later step uses is not drawn; the rest are the family's solids.
//!
//! All of it within [`Limits`]: solver steps, solid steps, formula text and profile corners,
//! counted (never timed), refused at the place a limit is reached.
//!
//! Geometry only through `engine_geometry::value::engine::Engine`: everything here is generic
//! over `E: Engine`, names no engine and links none, so the same tests judge any engine behind
//! the trait. Every length is a `construct_params::Length` (grid steps of `Q0`, the step the
//! engine must work on); nothing is held as millimetres or as a double.
//!
//! - [`evaluate()`] and [`evaluate_within()`]: one evaluation, nothing kept.
//! - [`Evaluator`]: an engine and the evaluated types it keeps, by [`CacheKey`]; a named type
//!   ([`Evaluator::evaluate_type`]) or every type a recipe names, type table rows included
//!   ([`Evaluator::evaluate_types`]).
//! - [`Evaluated`] and [`Built`]: the result, with what it [`Used`]; [`EvalError`], [`Place`] and
//!   [`Reason`]: why there is none, as a value to show a person.
//! - [`Limits`], [`Limit`]: the limits and which one was reached.
//!
//! Nothing here is interim: the engine behind the trait is the caller's choice (the sketch solver
//! behind `construct_solver::solve` has its own interim note). Not here: a hosted family's opening
//! cut in its host (job 6.4); placing evaluated families as elements, the edit log and the page
//! (later jobs).

mod build;
mod cache;
mod error;
mod limits;
mod sketch;
mod values;

pub use cache::{CacheKey, Evaluator, TypeResult};
pub use error::{EvalError, Place, Reason};
pub use limits::{Limit, Limits, Used};
pub use values::TypeValues;

use crate::model::{Key, Recipe, Visibility};
use construct_categories::CategoryId;
use construct_params::Value;
use engine_geometry::value::engine::Engine;
use kit_core::Id;

/// A recipe evaluated for one type.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluated<S> {
    /// The family's solids: every step no later step uses, in recipe order.
    pub solids: Vec<Built<S>>,
    /// Every parameter's value for this type, in recipe order: the type's, a default, or what
    /// its formula gives.
    pub values: Vec<(Key, Value)>,
    /// What the evaluation used, counted as its limits count.
    pub used: Used,
}

/// One of a family's solids.
#[derive(Clone, Debug, PartialEq)]
pub struct Built<S> {
    /// The step that made it.
    pub step: Key,
    /// The engine's solid.
    pub solid: S,
    /// The subcategory it is drawn with.
    pub subcategory: CategoryId,
    /// Its material: the step's fixed material, or its material parameter's value.
    pub material: Id,
    /// Which detail levels show it.
    pub visibility: Visibility,
}

/// Evaluates `recipe` for the type whose values are `values`, asking `engine` for every solid,
/// within the default limits ([`Limits::DEFAULT`]). Nothing is kept: [`Evaluator`] keeps each
/// type it builds.
///
/// # Errors
///
/// An [`EvalError`] naming the step, the place and the [`Reason`]: a value the type may not set
/// or that is not its parameter's, a formula refused, a sketch the solver refuses at these
/// values, a profile that is not a loop, a missing or later step, an engine on another grid step,
/// the engine's own refusal, or a limit reached.
pub fn evaluate<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
) -> Result<Evaluated<E::Solid>, EvalError> {
    build::run(engine, recipe, values, Limits::DEFAULT)
}

/// As [`evaluate()`], within `limits`.
///
/// # Errors
///
/// As [`evaluate()`].
pub fn evaluate_within<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
) -> Result<Evaluated<E::Solid>, EvalError> {
    build::run(engine, recipe, values, limits)
}
