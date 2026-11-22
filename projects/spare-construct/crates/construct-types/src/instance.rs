//! An instance's side of its type: the type it names and the values it holds itself. Its other
//! fields (placement, recipe, level) are the element record's and not this crate's.

use crate::element_type::ElementType;
use crate::error::TypeError;
use crate::parameters::Parameters;
use crate::record::{TYPE_KEY, is_reserved, read_id};
use crate::types::Types;
use construct_params::{Holder, Value, Values};
use kit_core::Id;
use std::collections::BTreeMap;

/// The type an element names and its own values: only parameters each instance holds, and only
/// those its type's elements have. Nothing of the type is copied here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    type_id: Id,
    values: Values,
}

impl Instance {
    /// An instance of `ty` with no values of its own.
    #[must_use]
    pub fn new(ty: &ElementType) -> Self {
        Self {
            type_id: ty.id().clone(),
            values: Values::new(),
        }
    }

    /// The permanent ID of its type.
    #[must_use]
    pub fn type_id(&self) -> &Id {
        &self.type_id
    }

    /// Its own values, by parameter ID.
    #[must_use]
    pub fn values(&self) -> &Values {
        &self.values
    }

    /// Sets its own value of a parameter each instance holds; `ty` is its type.
    ///
    /// # Errors
    /// [`TypeError::NotItsType`] if `ty` is not its type, [`TypeError::UnknownParameter`],
    /// [`TypeError::NotOnType`] if the type's elements lack the parameter,
    /// [`TypeError::HeldByType`] if the type holds it, or [`TypeError::Param`] if the value is
    /// not one the parameter takes. Nothing changes.
    pub fn set(
        &mut self,
        params: &Parameters,
        ty: &ElementType,
        id: &Id,
        value: Value,
    ) -> Result<(), TypeError> {
        self.its_type(ty)?;
        let def = ty.require_param(params, id)?;
        if def.holder() == Holder::Type {
            return Err(TypeError::HeldByType {
                parameter: def.name().to_owned(),
            });
        }
        self.values.set(def, value)?;
        Ok(())
    }

    /// Takes its own value of a parameter out, so it reads its default; returns the value.
    pub fn clear(&mut self, id: &Id) -> Option<Value> {
        self.values.remove(id)
    }

    /// Moves it from its type `from` to `to`, of the same category. It keeps each of its values
    /// whose parameter `to`'s elements also have, and returns the rest, which it no longer holds.
    ///
    /// # Errors
    /// [`TypeError::NotItsType`] if `from` is not its type; [`TypeError::OtherCategory`] if `to`
    /// is of another category. Nothing changes.
    pub fn change_type(
        &mut self,
        from: &ElementType,
        to: &ElementType,
    ) -> Result<BTreeMap<Id, Value>, TypeError> {
        self.its_type(from)?;
        if from.category() != to.category() {
            return Err(TypeError::OtherCategory {
                from: from.category(),
                to: to.category(),
            });
        }
        let gone: Vec<Id> = self
            .values
            .iter()
            .filter(|(id, _)| !to.has(id))
            .map(|(id, _)| id.clone())
            .collect();
        let mut dropped = BTreeMap::new();
        for id in gone {
            if let Some(value) = self.values.remove(&id) {
                dropped.insert(id, value);
            }
        }
        self.type_id = to.id().clone();
        Ok(dropped)
    }

    /// Its entries in its element record's `props`: [`TYPE_KEY`] naming its type, and its own
    /// values under their parameter IDs. Ordered, so the same instance gives the same bytes.
    #[must_use]
    pub fn to_props(&self) -> BTreeMap<String, String> {
        let mut props = self.values.to_props();
        props.insert(TYPE_KEY.to_owned(), self.type_id.as_str().to_owned());
        props
    }

    /// Reads an instance from its element record's `props`. A key that is neither reserved nor a
    /// parameter in `params` is not this crate's and is left alone.
    ///
    /// # Errors
    /// [`TypeError::Malformed`] if [`TYPE_KEY`] is missing or not an ID, or another reserved key
    /// is present; [`TypeError::UnknownType`] if `types` lacks its type;
    /// [`TypeError::NotOnType`] or [`TypeError::HeldByType`] for a value it may not hold;
    /// [`TypeError::Param`] for a value its parameter refuses.
    pub fn from_props(
        props: &BTreeMap<String, String>,
        params: &Parameters,
        types: &Types,
    ) -> Result<Self, TypeError> {
        let Some(type_id) = props.get(TYPE_KEY) else {
            return Err(TypeError::Malformed(format!(
                "an instance's props name its type under {TYPE_KEY}"
            )));
        };
        let type_id = read_id(type_id, TYPE_KEY)?;
        let ty = types
            .get(&type_id)
            .ok_or_else(|| TypeError::UnknownType(type_id.clone()))?;
        if props.keys().any(|key| is_reserved(key) && key != TYPE_KEY) {
            return Err(TypeError::Malformed(
                "an instance's props hold an unknown reserved key".to_owned(),
            ));
        }
        let mut values = Values::new();
        for def in params.definitions() {
            let Some(text) = props.get(def.id().as_str()) else {
                continue;
            };
            ty.require_param(params, def.id())?;
            if def.holder() == Holder::Type {
                return Err(TypeError::HeldByType {
                    parameter: def.name().to_owned(),
                });
            }
            values.set(def, def.read(text)?)?;
        }
        Ok(Self { type_id, values })
    }

    /// Refuses a type that is not its own.
    pub(crate) fn its_type(&self, ty: &ElementType) -> Result<(), TypeError> {
        if &self.type_id == ty.id() {
            Ok(())
        } else {
            Err(TypeError::NotItsType {
                instance_type: self.type_id.clone(),
                given: ty.id().clone(),
            })
        }
    }
}

/// The element IDs, ascending, of those `instances` that are of the type `type_id`. This crate
/// keeps no index of which instances use a type: the caller, which holds the elements, passes
/// them, each with its element ID.
pub fn users<'a, I>(type_id: &Id, instances: I) -> Vec<&'a Id>
where
    I: IntoIterator<Item = (&'a Id, &'a Instance)>,
{
    let mut found: Vec<&Id> = instances
        .into_iter()
        .filter(|(_, instance)| instance.type_id() == type_id)
        .map(|(element, _)| element)
        .collect();
    found.sort();
    found.dedup();
    found
}
