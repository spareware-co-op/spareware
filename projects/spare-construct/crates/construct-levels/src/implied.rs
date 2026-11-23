//! The levels an old record's integer `level` names.
//!
//! Before levels were elements, every element record carried a plain integer, and nothing said
//! what it meant beyond "the plan it is drawn on". So a document written then reads with a level
//! for each integer it uses, made by this one rule, the same on every machine and every replay:
//!
//! - `0` is no level. It is what a type, the parameter table, a Workshop part and an element made
//!   without a level always carried, and what every record written since carries.
//! - Any other `n` is the level whose ID is `n` in decimal (`1`, `12`), or `m` and the digits for
//!   one below zero (`m1` for -1), named `Level n`, a storey, at `(n - 1) x 3000 mm` above the
//!   project's zero for `n > 0` and `n x 3000 mm` for `n < 0`: levels -1, 1 and 2 sit at -3000, 0
//!   and 3000 mm, so the 3000 mm walls of the old pages stack.
//!
//! Such a level is implied: it has no record, and is listed while something is tied to it
//! ([`crate::Levels::from_records`]). A command that changes one writes its record, and from then
//! on the record is the level; take the record away (an undo) and the implied level returns.

use crate::level::Level;
use construct_params::Length;
use kit_core::Id;

/// How far apart the implied levels of neighbouring integers are.
pub const STOREY_STEP: Length = Length::from_mm(3000);

/// What an implied level's ID below zero starts with.
const BELOW: char = 'm';

/// The ID of the level an old integer names, or `None` for `0`, which is no level.
#[must_use]
pub fn implied_id(n: i32) -> Option<Id> {
    let text = match n {
        0 => return None,
        n if n > 0 => n.to_string(),
        n => format!("{BELOW}{}", n.unsigned_abs()),
    };
    Id::parse(&text).ok()
}

/// The integer whose implied level has this ID, if the ID is one in its one spelling.
#[must_use]
pub fn implied_number(id: &Id) -> Option<i32> {
    let text = id.as_str();
    let (digits, below) = match text.strip_prefix(BELOW) {
        Some(digits) => (digits, true),
        None => (text, false),
    };
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude: i64 = digits.parse().ok()?;
    let n = if below {
        magnitude.checked_neg()?
    } else {
        magnitude
    };
    i32::try_from(n).ok()
}

/// The level an old integer names, as the rule above makes it; `None` for `0`.
#[must_use]
pub fn implied(n: i32) -> Option<Level> {
    let id = implied_id(n)?;
    let steps_up = if n > 0 {
        i64::from(n).checked_sub(1)?
    } else {
        i64::from(n)
    };
    let elevation = Length::from_steps(STOREY_STEP.steps().checked_mul(steps_up)?);
    Level::new(id, &format!("Level {n}"), elevation, true).ok()
}
