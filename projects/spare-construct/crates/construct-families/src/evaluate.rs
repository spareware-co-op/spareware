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
//! - [`symbols()`] and [`symbols_within()`]: the family's 2D symbols drawn at one type's values
//!   ([`DrawnSymbols`], [`DrawnSymbol`], [`Edge`]), each symbol's sketch solved as a step's
//!   sketch is (added by piece A62, so a template's plan and elevation symbols can be read back
//!   and tested at every type).
//! - [`opening()`] and [`opening_within()`]: a hosted family's opening at one type's values: the
//!   host it declares, the void's outline solved in the family's own frame, and how deep it is cut
//!   ([`SolvedOpening`], [`Host`], [`Depth`]; piece A64, job 6.4). No engine is asked.
//! - [`Turn`], [`evaluate_turned()`] and [`Evaluator::evaluate_turned`]: a family built turned
//!   about its z axis by quarter turns, exactly, so it can stand in a host along any axis way
//!   (piece A64).
//! - [`Seat`], [`evaluate_seated()`] and [`Evaluator::evaluate_seated`]: a family seated in a host
//!   that runs at any angle, every corner worked out exactly and rounded once to the grid, an
//!   upright profile made of upright boxes (piece A83; the build module's header says why).
//! - [`extents()`]: the box each of a family's solids lies in at one type's values, worked out
//!   without an engine ([`Extent`]; piece A64), for a builder that draws a family as boxes.
//!
//! Nothing here is interim: the engine behind the trait is the caller's choice (the sketch solver
//! behind `construct_solver::solve` has its own interim note). Not here: cutting the opening in a
//! host and placing evaluated families as elements (construct-regen, job 6.4); the edit log and
//! the page (later jobs).

mod build;
mod cache;
mod error;
mod extent;
mod hosting;
mod limits;
mod seat;
mod sketch;
mod symbols;
mod turn;
mod values;

pub use cache::{CacheKey, Evaluator, TypeResult};
pub use error::{EvalError, Place, Reason};
pub use extent::{Extent, extents};
pub use hosting::{Depth, Host, SolvedOpening, opening, opening_within};
pub use limits::{Limit, Limits, Used};
pub use seat::Seat;
pub use symbols::{DrawnSymbol, DrawnSymbols, Edge, Point2, symbols, symbols_within};
pub use turn::Turn;
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
    build::run(
        engine,
        recipe,
        values,
        Limits::DEFAULT,
        build::Stand::Turned(Turn::None),
    )
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
    build::run(
        engine,
        recipe,
        values,
        limits,
        build::Stand::Turned(Turn::None),
    )
}

/// As [`evaluate_within()`], the family turned by `turn` about its z axis (piece A64): every
/// corner and every extrusion's vector turned before the engine is asked, exactly.
///
/// # Errors
///
/// As [`evaluate()`].
pub fn evaluate_turned<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
    turn: Turn,
) -> Result<Evaluated<E::Solid>, EvalError> {
    build::run(engine, recipe, values, limits, build::Stand::Turned(turn))
}

/// As [`evaluate_within()`], the family seated by `seat` in a host that runs at any angle (piece
/// A83): every corner put through the seat, each rounded once to the grid, and a profile on an
/// upright plane made of upright boxes when the seat is off the quarter turns.
///
/// # Errors
///
/// As [`evaluate()`]; and `UprightNotSquare` for a profile on an upright plane with an edge along
/// neither of its sketch's axes, off the quarter turns.
pub fn evaluate_seated<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
    seat: &Seat,
) -> Result<Evaluated<E::Solid>, EvalError> {
    build::run(engine, recipe, values, limits, build::Stand::Seated(seat))
}
