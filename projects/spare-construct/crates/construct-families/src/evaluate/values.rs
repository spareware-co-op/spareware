//! One type's values, and every parameter of a recipe worked out from them: each value a type
//! sets is checked as construct-params checks it (the parameter exists, the type holds it, it is
//! not worked out, the text is its one stored spelling), every other parameter reads its default,
//! and every formula is worked out with construct-formulas over the numbers (lengths, angles,
//! numbers and whole numbers). Length formulas outside the parameters (plane offsets, extrusion
//! heights, constraint lengths) are read against the same numbers.

use super::error::{EvalError, Place, Reason};
use crate::check::{Sort, definition, formula_kind, formula_text, names, stored_value};
use crate::model::{Key, ParameterValue, Recipe};
use construct_formulas::{Entry, FormulaError, evaluate, parse, value_of};
use construct_params::{Definition, Holder, Kind, Length, Value};
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// The values one type sets, by parameter key, in construct-params' stored text. A parameter the
/// type does not set reads its default. Kept in key order, so the same values are the same
/// whatever order they were set in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeValues {
    values: BTreeMap<Key, String>,
}

impl TypeValues {
    /// No values: every parameter reads its default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The values a recipe's built-in type sets.
    ///
    /// # Errors
    ///
    /// [`Reason::NoSuchType`] if the recipe has no type with this key, [`Reason::Duplicate`] if
    /// the type sets one parameter twice.
    pub fn of(recipe: &Recipe, ty: &Key) -> Result<Self, EvalError> {
        let Some(found) = recipe.types.iter().find(|t| t.key == *ty) else {
            return Err(EvalError::new(Place::Type(ty.clone()), Reason::NoSuchType));
        };
        let mut values = Self::new();
        for value in &found.values {
            if values.values.contains_key(&value.parameter) {
                return Err(EvalError::new(
                    Place::Type(ty.clone()),
                    Reason::Duplicate {
                        key: value.parameter.as_str().to_owned(),
                    },
                ));
            }
            values.set(value.parameter.clone(), value.value.clone());
        }
        Ok(values)
    }

    /// Sets a parameter's value, in stored text, in place of any it had.
    pub fn set(&mut self, parameter: Key, stored: impl Into<String>) {
        self.values.insert(parameter, stored.into());
    }

    /// These values with one more set, in place of any it had.
    #[must_use]
    pub fn with(mut self, parameter: Key, stored: impl Into<String>) -> Self {
        self.set(parameter, stored);
        self
    }

    /// The value set for a parameter, if one is.
    #[must_use]
    pub fn get(&self, parameter: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(key, _)| key.as_str() == parameter)
            .map(|(_, text)| text.as_str())
    }

    /// Every value set, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &str)> {
        self.values.iter().map(|(key, text)| (key, text.as_str()))
    }
}

/// Every parameter's value for one type, worked out.
pub(crate) struct Worked {
    /// Every parameter, in recipe order, with its value.
    pub(crate) all: Vec<(Key, Value)>,
    /// The parameters formulas can read (lengths, angles, numbers, whole numbers), by key.
    numbers: BTreeMap<String, Value>,
    /// The kind of every parameter, by key, to tell a formula that reads the wrong kind.
    kinds: BTreeMap<String, Kind>,
}

/// The first refusal in recipe order: one of a parameter's own, before one that only follows
/// from another's.
fn first_refusal(failed: Vec<(Key, FormulaError)>) -> Option<(Key, FormulaError)> {
    let mut later = None;
    for (key, e) in failed {
        if e.code != "Depends" {
            return Some((key, e));
        }
        later.get_or_insert((key, e));
    }
    later
}

/// A refusal at a parameter.
fn refuse<T>(key: &Key, reason: Reason) -> Result<T, EvalError> {
    Err(EvalError::new(Place::Parameter(key.clone()), reason))
}

/// Each parameter's definition, in recipe order, each key once.
fn definitions(recipe: &Recipe) -> Result<Vec<(Key, Definition)>, EvalError> {
    let mut defs: Vec<(Key, Definition)> = Vec::with_capacity(recipe.parameters.len());
    let mut seen = BTreeSet::new();
    for row in &recipe.parameters {
        if !seen.insert(row.key.as_str()) {
            // The checker refuses this too; evaluation does not pick one of the two.
            return refuse(
                &row.key,
                Reason::Duplicate {
                    key: row.key.as_str().to_owned(),
                },
            );
        }
        match definition(row) {
            Ok(def) => defs.push((row.key.clone(), def)),
            Err(e) => return refuse(&row.key, Reason::BadDefinition(e)),
        }
    }
    Ok(defs)
}

/// Checks that every value the type sets is one it may set.
fn check_given(
    recipe: &Recipe,
    defs: &[(Key, Definition)],
    given: &TypeValues,
) -> Result<(), EvalError> {
    for (key, text) in given.iter() {
        let Some((row, def)) = recipe
            .parameters
            .iter()
            .zip(defs)
            .find(|(row, _)| row.key == *key)
            .map(|(row, (_, def))| (row, def))
        else {
            return refuse(
                key,
                Reason::Missing {
                    sort: Sort::Parameter,
                    key: key.as_str().to_owned(),
                },
            );
        };
        if def.holder() == Holder::Instance {
            return refuse(key, Reason::HeldByInstance);
        }
        if matches!(row.value, ParameterValue::Formula(_)) {
            return refuse(key, Reason::SetsFormula);
        }
        if let Err(e) = stored_value(def, text) {
            return refuse(key, Reason::BadValue(e));
        }
    }
    Ok(())
}

/// Works out every parameter of `recipe` for the type whose values are `given`.
pub(crate) fn work_out(recipe: &Recipe, given: &TypeValues) -> Result<Worked, EvalError> {
    let defs = definitions(recipe)?;
    check_given(recipe, &defs, given)?;
    // Every parameter that is not worked out: the type's value, or its default.
    let mut stored: BTreeMap<&str, Value> = BTreeMap::new();
    let mut entries: Vec<(Key, Entry)> = Vec::new();
    let mut kinds = BTreeMap::new();
    for (row, (key, def)) in recipe.parameters.iter().zip(&defs) {
        kinds.insert(key.as_str().to_owned(), def.kind());
        match &row.value {
            ParameterValue::Default(default) => {
                let text = given.get(key.as_str()).unwrap_or(default);
                let value =
                    stored_value(def, text).or_else(|e| refuse(key, Reason::BadValue(e)))?;
                if let Some(kind) = formula_kind(def.kind()) {
                    let Some(text) = formula_text(def, &value) else {
                        return refuse(
                            key,
                            Reason::NotNumeric {
                                key: key.as_str().to_owned(),
                            },
                        );
                    };
                    entries.push((
                        key.clone(),
                        Entry {
                            name: key.as_str().to_owned(),
                            text,
                            kind,
                        },
                    ));
                }
                stored.insert(key.as_str(), value);
            }
            ParameterValue::Formula(text) => {
                let Some(kind) = formula_kind(def.kind()) else {
                    return refuse(
                        key,
                        Reason::NotNumeric {
                            key: key.as_str().to_owned(),
                        },
                    );
                };
                entries.push((
                    key.clone(),
                    Entry {
                        name: key.as_str().to_owned(),
                        text: text.clone(),
                        kind,
                    },
                ));
            }
        }
    }
    // A formula naming a parameter of another kind is told so, before the formulas are worked.
    for row in &recipe.parameters {
        if let ParameterValue::Formula(text) = &row.value
            && let Some(key) = not_numeric(text, &kinds)
        {
            return refuse(&row.key, Reason::NotNumeric { key });
        }
    }
    let table: Vec<Entry> = entries.iter().map(|(_, entry)| entry.clone()).collect();
    let results = evaluate(&table);
    let mut numbers = BTreeMap::new();
    let mut failed = Vec::new();
    for ((key, _), result) in entries.iter().zip(results) {
        match result {
            Ok(value) => {
                numbers.insert(key.as_str().to_owned(), value);
            }
            Err(e) => failed.push((key.clone(), e)),
        }
    }
    // `failed` is in recipe order, as the table is, so the refusal reported is the same on every
    // run.
    if let Some((key, e)) = first_refusal(failed) {
        return refuse(&key, Reason::Formula(e));
    }
    let mut all = Vec::with_capacity(recipe.parameters.len());
    for row in &recipe.parameters {
        let key = row.key.as_str();
        let value = numbers.get(key).or_else(|| stored.get(key)).cloned();
        if let Some(value) = value {
            all.push((row.key.clone(), value));
        }
    }
    Ok(Worked {
        all,
        numbers,
        kinds,
    })
}

/// The first parameter a formula reads that a formula cannot use, if any.
fn not_numeric(text: &str, kinds: &BTreeMap<String, Kind>) -> Option<String> {
    let expr = parse(text).ok()?;
    let mut read = Vec::new();
    names(&expr, &mut read);
    read.into_iter().find(|name| {
        kinds
            .get(name.as_str())
            .is_some_and(|kind| formula_kind(*kind).is_none())
    })
}

impl Worked {
    /// A length formula worked out over these values.
    pub(crate) fn length(&self, text: &str) -> Result<Length, Reason> {
        if let Some(key) = not_numeric(text, &self.kinds) {
            return Err(Reason::NotNumeric { key });
        }
        let numbers = &self.numbers;
        let mut lookup = |name: &str| {
            numbers.get(name).cloned().ok_or_else(|| {
                FormulaError::new(
                    "UnknownName",
                    format!("There is no parameter called {name}."),
                )
            })
        };
        match value_of(text, Kind::Length, &mut lookup) {
            Ok(Value::Length(length)) => Ok(length),
            Ok(other) => Err(Reason::Formula(FormulaError::new(
                "Type",
                format!("It gives {}, and a length is wanted.", other.stored()),
            ))),
            Err(e) => Err(Reason::Formula(e)),
        }
    }

    /// The material a material parameter has.
    pub(crate) fn material(&self, key: &Key) -> Result<Id, Reason> {
        match self.all.iter().find(|(k, _)| k == key) {
            Some((_, Value::Material(id))) => Ok(id.clone()),
            Some(_) => Err(Reason::NotAMaterial {
                key: key.as_str().to_owned(),
            }),
            None => Err(Reason::Missing {
                sort: Sort::Parameter,
                key: key.as_str().to_owned(),
            }),
        }
    }
}
