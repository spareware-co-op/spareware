//! Spare Construct: formulas.
//!
//! The one small, safe expression language for parameter values, shared by families and Workshop
//! (map: "Data model", "Formulas use a small, safe expression language"; feature map parts
//! CON-A27-F4-P2 and CON-A7-F1-P1). A value is written as text, such as `width / 2 - 4 mm`,
//! `2 ft 8 in` or `count > 3 ? 12 mm : 8 mm`, and evaluated exactly, with its units checked.
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
//!   over a length is a plain number.
//! - **Exact numbers**: literals and arithmetic are exact rationals (`dashu-ratio`, the crate the
//!   kernel uses), rounded once, to the nearest double, when a value leaves for a geometry step.
//!   `sqrt` and the trigonometric functions aren't here, because they aren't exact: they wait for
//!   the kernel's certified functions.
//!
//! What it holds:
//!
//! - [`parse`]: text to an [`Expr`], or a [`FormulaError`] saying where it stopped.
//! - [`Quantity`] and [`Dim`]: an exact value and its dimension; [`Kind`]: what a parameter or a
//!   field is (a length, an angle or a count).
//! - [`value_of`]: one value, read in the light of the names it can see.
//! - [`evaluate`]: a whole parameter table, in any order, with circular formulas refused.
//!
//! Lengths are held in millimetres and angles in degrees; `in` is 25.4 mm and `ft` 304.8 mm,
//! exactly. A number with no unit standing alone as a whole value is in the value's own unit, so a
//! length parameter of `120` is 120 mm, as Workshop and families have always read it.

mod eval;
mod parse;
mod quantity;
mod table;

pub use eval::value_of;
pub use parse::{Expr, Op, parse};
pub use quantity::{Dim, Kind, Quantity};
pub use table::{Entry, evaluate};

use std::fmt;

/// Why a formula can't be read or worked out. `code` is for programs, `message` for people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulaError {
    /// `Syntax`, `UnknownName`, `Units`, `Circular`, `Depends`, `DivideByZero`, `TooLarge`,
    /// `NotExact`, `Type` or `Name`.
    pub code: &'static str,
    pub message: String,
}

impl FormulaError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
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
