//! An element's or a type's parameter values, by parameter ID, and how they sit in the element
//! record's `props` (construct-model's FORMAT.md): each key a parameter's permanent ID, each value
//! its stored text. A key that names no defined parameter is not read here and is left alone, so
//! props written before parameters existed still read.

use crate::definition::Definition;
use crate::error::ParamError;
use crate::value::Value;
use kit_core::Id;
use std::collections::BTreeMap;

/// Parameter values by parameter ID, in ID order. Each was checked against its definition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values(BTreeMap<Id, Value>);

impl Values {
    /// No values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `def`'s value, replacing any it had.
    ///
    /// # Errors
    /// As [`Definition::check`], changing nothing.
    pub fn set(&mut self, def: &Definition, value: Value) -> Result<(), ParamError> {
        def.check(&value)?;
        self.0.insert(def.id().clone(), value);
        Ok(())
    }

    /// The value of the parameter with this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Value> {
        self.0.get(id)
    }

    /// Takes the value of the parameter with this ID out.
    pub fn remove(&mut self, id: &Id) -> Option<Value> {
        self.0.remove(id)
    }

    /// How many values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every value with its parameter's ID, in ID order.
    pub fn iter(&self) -> impl Iterator<Item = (&Id, &Value)> {
        self.0.iter()
    }

    /// The values as `props` entries: parameter ID to stored text. Ordered, so the same values
    /// always give the same bytes.
    #[must_use]
    pub fn to_props(&self) -> BTreeMap<String, String> {
        self.0
            .iter()
            .map(|(id, value)| (id.as_str().to_owned(), value.stored()))
            .collect()
    }

    /// Reads the values of `defs` from `props`. A definition with no entry has no value; an entry
    /// that is no definition's ID is left alone.
    ///
    /// # Errors
    /// As [`Definition::read`], naming the parameter whose entry is wrong.
    pub fn from_props<'a>(
        defs: impl IntoIterator<Item = &'a Definition>,
        props: &BTreeMap<String, String>,
    ) -> Result<Self, ParamError> {
        let mut values = Self::new();
        for def in defs {
            if let Some(stored) = props.get(def.id().as_str()) {
                values.0.insert(def.id().clone(), def.read(stored)?);
            }
        }
        Ok(values)
    }
}
