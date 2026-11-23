//! Recipe evaluation, first half (CON-A7-F1-P2, job 4.4): a family recipe and one type's values
//! made into the family's solids through the kernel's `Engine` interface, kept per type.
//!
//! What it does, in order (FORMAT.md, "Evaluation"; the choices are note
//! CON-A7-F1-P2-recipe-evaluation):
//!
//! 1. **Values** ([`TypeValues`]): each value the type sets is checked as construct-params checks
//!    it; every other parameter reads its default; every formula is worked out with
//!    construct-formulas over the numbers, units checked, and a refusal names the parameter.
//! 2. **Sketches**: each sketch a step uses becomes construct-solver's sketch, its constraint
//!    lengths the formulas' values, and is checked as stored: its stored points and radii must
//!    already meet every constraint at these values within the model tolerance. This half does
//!    not solve: a type whose values would move a sketch is refused, saying so (solving inside a
//!    recipe is job 5.5's).
//! 3. **Steps**, in recipe order: an extrusion between two heights along its plane's normal, a
//!    subtraction (an extrusion used as its tool is a void, and cuts the target named), a union.
//!    A step a later step uses is not drawn; the rest are the family's solids.
//!
//! Geometry only through `engine_geometry::value::engine::Engine`: everything here is generic
//! over `E: Engine`, names no engine and links none, so the same tests judge any engine behind
//! the trait. Every length is a `construct_params::Length` (grid steps of `Q0`, the step the
//! engine must work on); nothing is held as millimetres or as a double.
//!
//! - [`evaluate()`]: one evaluation, nothing kept.
//! - [`Evaluator`]: an engine and the evaluated types it keeps, by [`CacheKey`].
//! - [`Evaluated`] and [`Built`]: the result; [`EvalError`], [`Place`] and [`Reason`]: why there
//!   is none, as a value to show a person.
//! - [`Limits`]: the one place job 5.5's time and size limits go.
//!
//! Nothing here is interim: the engine behind the trait is the caller's choice. Not here yet:
//! solving sketches, type tables and limits (job 5.5); a hosted family's opening cut in its host
//! (job 6.4); placing evaluated families as elements, the edit log and the page (later jobs).

mod build;
mod cache;
mod error;
mod limits;
mod sketch;
mod values;

pub use cache::{CacheKey, Evaluator};
pub use error::{EvalError, Place, Reason};
pub use limits::Limits;
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

/// Evaluates `recipe` for the type whose values are `values`, asking `engine` for every solid.
/// Nothing is kept: [`Evaluator`] keeps each type it builds.
///
/// # Errors
///
/// An [`EvalError`] naming the step, the place and the [`Reason`]: a value the type may not set
/// or that is not its parameter's, a formula refused, a sketch not met as stored at these values,
/// a profile that is not a loop, a missing or later step, an engine on another grid step, or the
/// engine's own refusal.
pub fn evaluate<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
) -> Result<Evaluated<E::Solid>, EvalError> {
    build::run(engine, recipe, values)
}
