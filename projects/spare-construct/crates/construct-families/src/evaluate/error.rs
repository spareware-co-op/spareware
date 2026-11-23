//! Why a recipe was not evaluated, as a value a caller can show a person: the step being built
//! when it stopped (if one was), the place in the recipe, and the reason.

use super::limits::Limit;
use crate::check::Sort;
use crate::model::Key;
use construct_formulas::FormulaError;
use construct_params::ParamError;
use construct_solver::{SketchError, SolveError};
use engine_geometry::value::engine::EngineError;
use engine_geometry::value::grid::GridStep;
use std::fmt;

/// Where in a recipe evaluation stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A type the recipe names (built-in, or a type table row).
    Type(Key),
    /// One value a named type sets, or lacks.
    TypeValue {
        /// The type.
        ty: Key,
        /// The parameter.
        parameter: Key,
    },
    /// A parameter: its definition, its value or its formula.
    Parameter(Key),
    /// A plane: its offset.
    Plane(Key),
    /// A sketch as a whole.
    Sketch(Key),
    /// A point, line, arc or circle of a sketch.
    Item {
        /// The sketch.
        sketch: Key,
        /// The item.
        item: Key,
    },
    /// A constraint of a sketch, by its place in the sketch's list (from 0).
    Constraint {
        /// The sketch.
        sketch: Key,
        /// Its place in the list.
        index: usize,
    },
    /// A step.
    Step(Key),
    /// The engine itself, before any step.
    Engine,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type(key) => write!(f, "type {key}"),
            Self::TypeValue { ty, parameter } => write!(f, "type {ty}, parameter {parameter}"),
            Self::Parameter(key) => write!(f, "parameter {key}"),
            Self::Plane(key) => write!(f, "plane {key}"),
            Self::Sketch(key) => write!(f, "sketch {key}"),
            Self::Item { sketch, item } => write!(f, "sketch {sketch}, {item}"),
            Self::Constraint { sketch, index } => {
                write!(f, "sketch {sketch}, constraint {index}")
            }
            Self::Step(key) => write!(f, "step {key}"),
            Self::Engine => f.write_str("the geometry engine"),
        }
    }
}

/// Why evaluation stopped.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Reason {
    /// The recipe names no type with this key (built-in or a type table row).
    NoSuchType,
    /// A type table row has no value for this column.
    NoValue,
    /// A type table row has more values than the table has columns.
    ExtraValues {
        /// The row's values.
        values: usize,
        /// The table's columns.
        columns: usize,
    },
    /// Nothing of this sort has this key.
    Missing {
        /// What was looked for.
        sort: Sort,
        /// The key.
        key: String,
    },
    /// The key names something of another sort.
    WrongSort {
        /// The key.
        key: String,
        /// What was wanted.
        wanted: Sort,
        /// What it names.
        found: Sort,
    },
    /// Two things in one list, or one sketch, have this key.
    Duplicate {
        /// The key.
        key: String,
    },
    /// A step uses a step that is not earlier than it.
    NotEarlier {
        /// The step it uses.
        key: String,
    },
    /// construct-params refuses the parameter's definition.
    BadDefinition(ParamError),
    /// construct-params refuses the value for its parameter, or it is not in its one spelling.
    BadValue(ParamError),
    /// A type sets a parameter each instance holds.
    HeldByInstance,
    /// A type sets a parameter that is worked out by its formula.
    SetsFormula,
    /// A formula uses, or a parameter with a formula is, a parameter that is not a number a
    /// formula can use (a length, an angle, a number or a whole number).
    NotNumeric {
        /// The parameter.
        key: String,
    },
    /// construct-formulas refuses the formula: it does not read, its units do not agree with the
    /// length wanted, it divides by zero at these values, and so on (`code` says which).
    Formula(FormulaError),
    /// A sketch's coordinate or radius is not a length in its one stored spelling.
    NotALength {
        /// The text.
        text: String,
    },
    /// The sketch solver does not take the item or constraint as written.
    BadSketch(SketchError),
    /// The sketch cannot be solved at these values, from its stored geometry: its constraints
    /// conflict (over-constrained), no position meeting them is found from where it is drawn, the
    /// answer does not stay within the model tolerance on the grid, or the solver cannot tell
    /// (the solver's error says which, and names the constraints).
    NotSolved(SolveError),
    /// An extrusion's profile or hole is not one closed loop of three or more lines.
    NotALoop,
    /// An extrusion starts and ends at the same height.
    NoThickness,
    /// A point or vector lies beyond the range the geometry engine takes.
    OutOfRange,
    /// A step's material is taken from a parameter that is not a material parameter.
    NotAMaterial {
        /// The parameter.
        key: String,
    },
    /// The engine works on a grid step other than construct-params' lengths' (`Q0`).
    WrongGrid {
        /// The engine's grid step.
        step: GridStep,
    },
    /// The engine gave no answer.
    Engine(EngineError),
    /// Going on would pass one of the evaluation's limits ([`crate::Limits`]).
    LimitReached {
        /// Which limit.
        limit: Limit,
        /// What it allows.
        allowed: u64,
    },
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchType => f.write_str("the recipe has no such type"),
            Self::NoValue => f.write_str("the type table row has no value for it"),
            Self::ExtraValues { values, columns } => write!(
                f,
                "the type table row has {values} values for {columns} columns"
            ),
            Self::Missing { sort, key } => write!(f, "there is no {sort} {key}"),
            Self::WrongSort { key, wanted, found } => {
                write!(f, "{key} is a {found}, and a {wanted} is wanted")
            }
            Self::Duplicate { key } => write!(f, "the key {key} is used twice"),
            Self::NotEarlier { key } => write!(f, "the step {key} does not come earlier"),
            Self::BadDefinition(e) | Self::BadValue(e) => write!(f, "{e}"),
            Self::HeldByInstance => {
                f.write_str("it is held by each instance, so a type cannot set it")
            }
            Self::SetsFormula => f.write_str("it is worked out by its formula, so nothing sets it"),
            Self::NotNumeric { key } => write!(f, "{key} is not a number a formula can use"),
            Self::Formula(e) => write!(f, "{e}"),
            Self::NotALength { text } => write!(f, "{text} is not a length in its stored spelling"),
            Self::BadSketch(e) => write!(f, "the sketch solver does not take it: {e}"),
            Self::NotSolved(e) => write!(
                f,
                "the sketch cannot be solved at these values from where it is drawn: {e}"
            ),
            Self::NotALoop => f.write_str("the lines are not one closed loop"),
            Self::NoThickness => f.write_str("it starts and ends at the same height"),
            Self::OutOfRange => f.write_str("it lies beyond the geometry engine's range"),
            Self::NotAMaterial { key } => write!(f, "{key} is not a material parameter"),
            Self::WrongGrid { step } => write!(
                f,
                "the engine works on the grid step {step:?}, and lengths are in Q0 steps"
            ),
            Self::Engine(e) => match e {
                EngineError::Invalid { operation, what } => {
                    write!(f, "the engine's {operation} does not take it: {what}")
                }
                EngineError::Refused { operation, reason } => {
                    write!(f, "the engine refused the {operation}: {reason}")
                }
                EngineError::Unsupported { operation, what } => {
                    write!(f, "the engine's {operation} does not answer {what} yet")
                }
            },
            Self::LimitReached { limit, allowed } => write!(
                f,
                "evaluating it would pass the limit of {allowed} {limit} for one type"
            ),
        }
    }
}

/// Why a recipe was not evaluated: the step being built (if it had got that far), the place, and
/// the reason. Held in one box, so a `Result` carrying it stays small.
#[derive(Clone, Debug, PartialEq)]
pub struct EvalError(Box<Parts>);

#[derive(Clone, Debug, PartialEq)]
struct Parts {
    step: Option<Key>,
    place: Place,
    reason: Reason,
}

impl EvalError {
    pub(crate) fn new(place: Place, reason: Reason) -> Self {
        Self(Box::new(Parts {
            step: None,
            place,
            reason,
        }))
    }

    /// The same error, while building `step`.
    pub(crate) fn in_step(mut self, step: &Key) -> Self {
        self.0.step = Some(step.clone());
        self
    }

    /// The step being built when evaluation stopped, if it had reached the steps.
    #[must_use]
    pub fn step(&self) -> Option<&Key> {
        self.0.step.as_ref()
    }

    /// Where in the recipe.
    #[must_use]
    pub fn place(&self) -> &Place {
        &self.0.place
    }

    /// Why.
    #[must_use]
    pub fn reason(&self) -> &Reason {
        &self.0.reason
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Parts {
            step,
            place,
            reason,
        } = &*self.0;
        match step {
            Some(step) if *place != Place::Step(step.clone()) => {
                write!(f, "step {step}: {place}: {reason}")
            }
            _ => write!(f, "{place}: {reason}"),
        }
    }
}

impl std::error::Error for EvalError {}
