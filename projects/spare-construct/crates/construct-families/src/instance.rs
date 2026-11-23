//! The values a placed instance of a family holds (CON-A7-F3-P2, piece A64, job 7.1): the
//! parameters a recipe marks as held by each instance (a window's sill height, a door's swing),
//! offered to a person with their defaults, and a typed value read into its stored text, refusing
//! in words a person can follow what an instance may not set.
//!
//! What a type holds is the type's: an instance is given a value only for a parameter each
//! instance holds, never one the type holds nor one that is worked out by a formula. The values
//! then go into evaluation through [`crate::TypeValues::set_instance`].
//!
//! Nothing here is interim. Depends on construct-params for the typed text and its words.

use crate::check::{definition, stored_value};
use crate::model::{Key, ParameterValue, Recipe};
use construct_params::{Holder, Kind, Units, Value};

/// A parameter each instance of a family holds, as a placing command offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceParameter {
    /// Its key in the recipe.
    pub key: Key,
    /// Its name, as people see it.
    pub name: String,
    /// The kind of value it takes.
    pub kind: Kind,
    /// Its default as a person reads it (`900 mm`, `Hinged on the left`).
    pub shown: String,
    /// Its default in stored text.
    pub stored: String,
    /// A choice parameter's choices, each its key and its label; empty for any other kind.
    pub choices: Vec<(String, String)>,
}

impl Recipe {
    /// The parameters each instance holds, in recipe order, each with its default. One that is
    /// worked out by a formula, or whose definition construct-params refuses, is left out (the
    /// checker refuses such a recipe).
    #[must_use]
    pub fn instance_parameters(&self, units: &Units) -> Vec<InstanceParameter> {
        self.parameters
            .iter()
            .filter(|p| p.definition.holder == Holder::Instance)
            .filter_map(|p| {
                let ParameterValue::Default(text) = &p.value else {
                    return None;
                };
                let def = definition(p).ok()?;
                let value = stored_value(&def, text).ok()?;
                Some(InstanceParameter {
                    key: p.key.clone(),
                    name: p.definition.name.clone(),
                    kind: p.definition.kind,
                    shown: def.format(&value, units).ok()?,
                    stored: value.stored(),
                    choices: p
                        .definition
                        .choices
                        .iter()
                        .map(|c| (c.key.clone(), c.label.clone()))
                        .collect(),
                })
            })
            .collect()
    }

    /// Whether `key` is a parameter an instance of this family may be given a value for.
    ///
    /// # Errors
    ///
    /// The words that say why not: the recipe has no such parameter, the type holds it, or it is
    /// worked out.
    pub fn check_instance_key(&self, key: &Key) -> Result<(), String> {
        let Some(parameter) = self.parameters.iter().find(|p| p.key == *key) else {
            return Err(format!(
                "{} has no value called {key} to set on a placed one.",
                self.name
            ));
        };
        let name = &parameter.definition.name;
        if matches!(parameter.value, ParameterValue::Formula(_)) {
            return Err(format!("{name} is worked out, so nothing sets it."));
        }
        if parameter.definition.holder != Holder::Instance {
            return Err(format!(
                "{name} belongs to the type of {}: change its type to change it.",
                self.name
            ));
        }
        Ok(())
    }

    /// What a person typed for the instance parameter `key`, as stored text, read as the
    /// parameter's definition reads it (`900`, `0.9 m` or `900mm` for a length: a number with no
    /// unit is in the unit `units` shows the kind in).
    ///
    /// # Errors
    ///
    /// The words that say why not: the recipe has no such parameter, the type holds it, it is
    /// worked out, or the text is not a value of it (construct-params' own words).
    pub fn instance_value(&self, key: &Key, typed: &str, units: &Units) -> Result<String, String> {
        self.check_instance_key(key)?;
        let Some(parameter) = self.parameters.iter().find(|p| p.key == *key) else {
            return Err(format!("{} has no value called {key}.", self.name));
        };
        let def = definition(parameter).map_err(|e| e.to_string())?;
        let value: Value = def.parse(typed, units).map_err(|e| e.to_string())?;
        Ok(value.stored())
    }
}
