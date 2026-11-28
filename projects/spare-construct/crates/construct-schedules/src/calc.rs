//! Calculated fields: a column worked out of other columns by a formula, for every row.
//!
//! A formula is written in the project's one formula language (construct-formulas), so units and
//! exactness are that language's: `Width * Height` of two lengths is an area, `Length / 2 + 50 mm`
//! a length, `5 mm + 2` refused for want of a unit, and every answer is landed on the kind the
//! calculated field says it has, a length on the nearest grid step. Nothing here works a number
//! out by itself.
//!
//! A formula names other fields by their name written the way a formula can spell it
//! ([`reference_name`]): every run of characters that is not a letter or a digit becomes one `_`,
//! so `Mass (kg)` is `Mass_kg` and `Sill height` is `Sill_height`. It may name any field of the
//! schedule's category, and the calculated fields before it in the schedule's own list. A formula
//! is checked when the schedule is checked (a field that is not there, kinds that do not fit, a
//! unit that is missing, in construct-formulas' own words); for a row it is worked out from the
//! row's values, and a row that lacks a value the formula needs, or whose values make it divide
//! by zero, has an empty cell.

use crate::error::ScheduleError;
use crate::field::Field;
use crate::rows::Reading;
use construct_formulas::{FormulaError, canonical, value_of};
use construct_params::{
    Angle, Area, Decimal, Definition, Holder, Kind, Length, Slope, Units, Value, Volume,
};
use kit_core::Id;
use std::collections::BTreeMap;

/// The longest a calculated field's name may be, in characters.
pub const MAX_NAME_LEN: usize = 64;

/// What every calculated field's definition is called: it is the schedule's, not the document's.
const DEFINITION_ID: &str = "calculated.value";

/// The kinds a calculated field may have: a measured value or a number.
const RESULT_KINDS: [Kind; 6] = [
    Kind::Length,
    Kind::Area,
    Kind::Volume,
    Kind::Angle,
    Kind::Number,
    Kind::Whole,
];

/// A calculated field: its name, its formula in the formula language's one spelling, and the kind
/// of value it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calculated {
    name: String,
    formula: String,
    kind: Kind,
}

impl Calculated {
    /// A calculated field. The formula is read here and kept in its one spelling.
    ///
    /// # Errors
    /// [`ScheduleError::Calculated`]: a name that is empty, too long or has a control character;
    /// a kind that is not a measured value or a number; a formula that does not read.
    pub fn new(name: &str, formula: &str, kind: Kind) -> Result<Self, ScheduleError> {
        let name = name.trim();
        let refuse = |why: &str| ScheduleError::Calculated {
            name: name.to_owned(),
            why: why.to_owned(),
        };
        if name.is_empty() {
            return Err(refuse("it needs a name."));
        }
        if name.chars().count() > MAX_NAME_LEN || name.chars().any(char::is_control) {
            return Err(refuse(&format!(
                "a name is at most {MAX_NAME_LEN} characters with no control characters."
            )));
        }
        if !RESULT_KINDS.contains(&kind) {
            return Err(refuse(
                "it can be a length, an area, a volume, an angle, a number or a whole number.",
            ));
        }
        let formula = canonical(formula).map_err(|e| refuse(&e.message))?;
        Ok(Self {
            name: name.to_owned(),
            formula,
            kind,
        })
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its formula, in the formula language's one spelling.
    #[must_use]
    pub fn formula(&self) -> &str {
        &self.formula
    }

    /// The kind of value it gives.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The field that shows it.
    #[must_use]
    pub fn field(&self) -> Field {
        Field::Calculated(self.name.clone())
    }

    /// The definition its column is shown, typed and compared through.
    #[must_use]
    pub fn definition(&self) -> Option<Definition> {
        let id = Id::parse(DEFINITION_ID).ok()?;
        Definition::new(id, &self.name, self.kind, Holder::Instance).ok()
    }
}

/// A field's name as a formula spells it: each run of characters that is not an ASCII letter or
/// digit becomes one `_`, none at either end, and a name that would start with a digit is given a
/// leading `_`.
#[must_use]
pub fn reference_name(name: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if pending && !out.is_empty() {
                out.push('_');
            }
            pending = false;
            out.push(c);
        } else {
            pending = true;
        }
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// What a formula may name: the spelling, the field and what it holds.
pub(crate) type Names = Vec<(String, Field, Kind)>;

/// The kinds a formula can take as an input.
const fn usable(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Length
            | Kind::Area
            | Kind::Volume
            | Kind::Angle
            | Kind::Slope
            | Kind::Number
            | Kind::Whole
    )
}

/// The field a spelling names, or why it names none.
fn named<'a>(
    names: &'a [(String, Field, Kind)],
    spelling: &str,
) -> Result<&'a (String, Field, Kind), FormulaError> {
    let mut found = names.iter().filter(|(spelt, _, _)| spelt == spelling);
    let Some(first) = found.next() else {
        let known: Vec<&str> = names
            .iter()
            .filter(|(_, _, kind)| usable(*kind))
            .map(|(spelt, _, _)| spelt.as_str())
            .collect();
        return Err(FormulaError::new(
            "UnknownName",
            format!(
                "There is no field called {spelling} to use here. The fields a formula can use are: {}.",
                known.join(", ")
            ),
        ));
    };
    if found.next().is_some() {
        return Err(FormulaError::new(
            "Name",
            format!("Two fields are called {spelling} in a formula: rename one of them."),
        ));
    }
    if !usable(first.2) {
        return Err(FormulaError::new(
            "Type",
            format!(
                "{spelling} holds {}, which a formula cannot use: a formula works with lengths, areas, volumes, angles and numbers.",
                first.2
            ),
        ));
    }
    Ok(first)
}

/// A value of each kind a formula takes, for the type check that needs only kinds.
fn probe(kind: Kind) -> Option<Value> {
    Some(match kind {
        Kind::Length => Value::Length(Length::from_steps(1)),
        Kind::Area => Value::Area(Area::from_square_steps(1)),
        Kind::Volume => Value::Volume(Volume::from_cubic_steps(1)),
        Kind::Angle => Value::Angle(Angle::from_units(1)),
        Kind::Slope => Value::Slope(Slope::new(1, 1)?),
        Kind::Number => Value::number(Decimal::ONE),
        Kind::Whole => Value::Whole(1),
        _ => return None,
    })
}

/// Whether a refusal depends on the values a formula is worked out over, and not on its units or
/// the fields it names.
fn depends_on_values(code: &str) -> bool {
    matches!(
        code,
        "DivideByZero" | "TooLarge" | "NotExact" | "NotWhole" | "Missing"
    )
}

/// Checks a calculated field's formula against what it may name: it reads, names fields that are
/// there, and its units fit the kind the field gives. This is done over a value of each input's
/// kind, so what is found is what the kinds say and not what any element holds.
///
/// # Errors
/// [`ScheduleError::Calculated`], in construct-formulas' words.
pub(crate) fn check(
    calc: &Calculated,
    names: &[(String, Field, Kind)],
) -> Result<(), ScheduleError> {
    let mut look = |spelling: &str| {
        let (_, _, kind) = named(names, spelling)?;
        probe(*kind).ok_or_else(|| FormulaError::new("Type", format!("{spelling} cannot be used.")))
    };
    match value_of(&calc.formula, calc.kind, &mut look) {
        Ok(_) => Ok(()),
        Err(e) if depends_on_values(e.code) => Ok(()),
        Err(e) => Err(ScheduleError::Calculated {
            name: calc.name.clone(),
            why: e.message,
        }),
    }
}

/// One row's value of a calculated field, from the row's other readings; `None` when the row has
/// no value the formula needs or the values make it fail, exact and landed on the field's kind.
pub(crate) fn work(
    calc: &Calculated,
    names: &[(String, Field, Kind)],
    readings: &BTreeMap<String, Reading>,
    units: &Units,
) -> Option<Reading> {
    let mut look = |spelling: &str| {
        let (_, field, _) = named(names, spelling)?;
        readings
            .get(&field.key())
            .map(|reading| reading.value.clone())
            .ok_or_else(|| {
                FormulaError::new(
                    "Missing",
                    format!("This element has no value for {spelling}."),
                )
            })
    };
    let value = value_of(&calc.formula, calc.kind, &mut look).ok()?;
    let shown = calc.definition()?.format(&value, units).ok()?;
    Some(Reading { value, shown })
}
