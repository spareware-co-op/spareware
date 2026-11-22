//! The parameter definitions a caller works with, each with an optional default: the last place
//! [`crate::resolve`] looks.

use crate::error::TypeError;
use construct_params::{Definition, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// One definition and its default.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Param {
    definition: Definition,
    default: Option<Value>,
}

/// Parameter definitions by permanent ID, each with an optional default value it takes. Where
/// definitions are stored is not this crate's: the caller reads them and builds this set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parameters(BTreeMap<Id, Param>);

impl Parameters {
    /// No parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a definition, with the value an element reads when neither it nor its type holds one.
    /// Whether the instance or the type holds the value is the definition's [`Definition::holder`].
    ///
    /// # Errors
    /// [`TypeError::DuplicateParameter`] if its ID is already defined; [`TypeError::Param`] if the
    /// default is not a value the definition takes. Either way nothing changes.
    pub fn add(&mut self, definition: Definition, default: Option<Value>) -> Result<(), TypeError> {
        if self.0.contains_key(definition.id()) {
            return Err(TypeError::DuplicateParameter(definition.id().clone()));
        }
        if let Some(value) = &default {
            definition.check(value)?;
        }
        self.0.insert(
            definition.id().clone(),
            Param {
                definition,
                default,
            },
        );
        Ok(())
    }

    /// The definition with this ID.
    #[must_use]
    pub fn definition(&self, id: &Id) -> Option<&Definition> {
        self.0.get(id).map(|param| &param.definition)
    }

    /// The default of the parameter with this ID, if it has one.
    #[must_use]
    pub fn default_of(&self, id: &Id) -> Option<&Value> {
        self.0.get(id).and_then(|param| param.default.as_ref())
    }

    /// Every definition, in ID order.
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.0.values().map(|param| &param.definition)
    }

    /// How many parameters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The definition with this ID, or the refusal naming it.
    pub(crate) fn require(&self, id: &Id) -> Result<&Definition, TypeError> {
        self.definition(id)
            .ok_or_else(|| TypeError::UnknownParameter(id.clone()))
    }
}
