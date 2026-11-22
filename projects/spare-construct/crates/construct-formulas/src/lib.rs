//! Spare Construct: formulas.
//!
//! The one small, safe expression language for parameter values, shared by families and Workshop
//! (map: "Data model", "Formulas use a small, safe expression language"; feature map parts
//! CON-A2-F2-P2, CON-A27-F4-P2 and CON-A7-F1-P1). A value is written as text, such as
//! `width / 2 - 4 mm`, `2 ft 8 in` or `count > 3 ? 12 mm : 8 mm`, type-checked, and evaluated
//! exactly.
//!
//! One units system. This crate owns the language only: reading a formula, checking its types and
//! working it out. Quantities, kinds, units, typed text and display are `construct-params`':
//! a formula's inputs and its answer are `construct_params::Value`s of a `construct_params::Kind`,
//! a unit after a number is a `construct_params::Unit` with its exact factor, and a length is the
//! one `Length`, in the geometry kernel's grid steps. This crate depends on `construct-params`,
//! never the reverse. The decision is note CON-A2-F3-P1; the stored form of a formula is
//! FORMAT.md beside this crate.
//!
//! Why our own language and not a library: note A07 looked for one and recommends this. The
//! well-known expression crates work in doubles and have no units (CEL and its Rust implementation,
//! for one), and a value here goes to the kernel's exact grid, so units and exact numbers have to
//! be the language's own. What is not ours is the parser: it is written on `winnow`, a well-known
//! parser crate, as Zoo's KCL is.
//!
//! The methods followed, as note A07 names them:
//!
//! - **CEL's guarantees** (the Common Expression Language's definition): no side effects, no loops
//!   and no user functions, so every formula ends. Sizes are capped too: a formula's length, its
//!   nesting, and the size of the numbers it makes, so a shared recipe can't hang the app.
//! - **KCL's units in the types**: every number carries a dimension. Lengths add only to lengths,
//!   `820 mm` and `2 ft 8 in` are both lengths, a length times a length is an area, and a length
//!   over a length is a plain number. The type check runs before any value is worked out, from
//!   the kinds of the parameters a formula uses.
//! - **Exact numbers**: literals and arithmetic are exact rationals (`dashu-ratio`, the crate the
//!   kernel uses), in `construct-params`' stored units, and they never leave: an answer is landed
//!   on its kind by the rule that kind has for typed text. A length, area or volume must be a
//!   whole number of grid steps or is refused naming the nearest values either side; an angle
//!   rounds half to even to the kernel's angle unit; a number is an exact decimal. `sqrt` and the
//!   trigonometric functions aren't here, because they aren't exact: they wait for the kernel's
//!   certified functions.
//!
//! What it holds:
//!
//! - [`parse`]: text to an [`Expr`], or a [`FormulaError`] saying where it stopped; [`canonical`]:
//!   a formula's one spelling, the form it is stored in.
//! - [`value_of`]: one value of a kind, read in the light of the values it can see.
//! - [`evaluate`]: a whole table of texts ([`Entry`]), in any order, with loops refused.
//! - [`define`], [`check`] and [`work_out`]: a parameter definition that carries a formula, the
//!   type check of a set of them, and their values worked out over the other parameters of the
//!   same element or type (CON-A2-F2-P2). Pure functions: storing the results goes through the
//!   edit log elsewhere.
//!
//! A number with no unit standing alone as a whole value is in its kind's own unit, fixed and
//! never the display setting: millimetres for a length (a length of `120` is 120 mm, as Workshop
//! and families have always read it), square and cubic millimetres, degrees, or a plain number.
//! Nothing here is interim. Depends on `construct-params`, `kit-core` (parameter IDs), `winnow`
//! and `dashu`.

mod eval;
mod parse;
mod table;
mod types;

pub use eval::value_of;
pub use parse::{Expr, Op, canonical, parse};
pub use table::{Entry, check, define, evaluate, work_out};

use construct_params::Value;
use std::fmt;

/// Why a formula can't be read or worked out. `code` is for programs, `message` for people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulaError {
    /// `Syntax`, `UnknownName`, `Units`, `Type`, `Name`, `Circular`, `Depends`, `Holder`,
    /// `Missing`, `DivideByZero`, `TooLarge`, `NotExact`, `NotWhole`, `OffGrid`, `Spelling` or
    /// `Definition`.
    pub code: &'static str,
    /// What went wrong, in words.
    pub message: String,
    /// For `OffGrid`: the nearest values below and above, each a whole number of grid steps.
    pub nearest: Option<(Value, Value)>,
}

impl FormulaError {
    /// A refusal with its code and message.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            nearest: None,
        }
    }
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FormulaError {}

/// The longest formula, in bytes.
pub const MAX_LEN: usize = 1000;
/// The deepest nesting of brackets and calls.
pub const MAX_DEPTH: usize = 32;
/// The most conditionals (`?:`) in one formula.
pub const MAX_CONDITIONALS: usize = 16;
/// The most digits in one number.
pub const MAX_DIGITS: usize = 30;
/// The largest number a step may make: its numerator and denominator together, in bits.
pub const MAX_BITS: usize = 2048;
