//! One element type: its permanent ID, name, category, the parameters its elements have, and the
//! values it holds for the parameters a type holds. Written as an element record and read back.

use crate::error::TypeError;
use crate::parameters::Parameters;
use crate::record::{
    CATEGORY_KEY, PARAMS_KEY, TYPE_KIND, is_reserved, read_category, read_id, read_params,
    type_recipe, write_category, write_params,
};
use construct_categories::{CategoryId, CategoryTable};
use construct_model::element::{Element, Placement};
use construct_params::{Definition, Holder, Value, Values};
use kit_core::Id;
use std::collections::BTreeSet;

/// Where a type record sits: it has no shape, so nowhere.
const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// A type name as kept: trimmed, and never empty.
pub(crate) fn clean_name(name: &str) -> Result<String, TypeError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(TypeError::EmptyName);
    }
    Ok(name.to_owned())
}

/// A named variant its instances share. Its name is kept unique within its category by
/// [`crate::Types`], which alone renames a type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementType {
    id: Id,
    name: String,
    category: CategoryId,
    /// Every parameter the type's elements have, held by the type or by each instance.
    params: BTreeSet<Id>,
    /// Values of parameters the type holds; one it has no value for reads its default.
    values: Values,
}

impl ElementType {
    /// A type with no parameters yet. The name is trimmed.
    ///
    /// # Errors
    /// [`TypeError::EmptyName`] if the name is only spaces; [`TypeError::UnknownCategory`] if
    /// `categories` has no such category.
    pub fn new(
        id: Id,
        name: &str,
        category: CategoryId,
        categories: &CategoryTable,
    ) -> Result<Self, TypeError> {
        let name = clean_name(name)?;
        if categories.get(category).is_none() {
            return Err(TypeError::UnknownCategory(category));
        }
        Ok(Self {
            id,
            name,
            category,
            params: BTreeSet::new(),
            values: Values::new(),
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The name people see.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its category; every instance of it is in this category.
    #[must_use]
    pub fn category(&self) -> CategoryId {
        self.category
    }

    /// The IDs of every parameter its elements have, ascending.
    pub fn params(&self) -> impl Iterator<Item = &Id> {
        self.params.iter()
    }

    /// Whether its elements have the parameter with this ID.
    #[must_use]
    pub fn has(&self, id: &Id) -> bool {
        self.params.contains(id)
    }

    /// The values it holds, by parameter ID.
    #[must_use]
    pub fn values(&self) -> &Values {
        &self.values
    }

    /// Its own value of the parameter with this ID, if it holds one.
    #[must_use]
    pub fn value(&self, id: &Id) -> Option<&Value> {
        self.values.get(id)
    }

    /// Gives its elements the parameter with this ID. Having it already changes nothing.
    ///
    /// # Errors
    /// [`TypeError::UnknownParameter`] if `params` does not define it.
    pub fn add_parameter(&mut self, params: &Parameters, id: &Id) -> Result<(), TypeError> {
        params.require(id)?;
        self.params.insert(id.clone());
        Ok(())
    }

    /// Sets its value of a parameter it holds. Every instance of it reads the new value.
    ///
    /// # Errors
    /// [`TypeError::UnknownParameter`], [`TypeError::NotOnType`] if its elements lack the
    /// parameter, [`TypeError::HeldByInstance`] if each instance holds it, or
    /// [`TypeError::Param`] if the value is not one the parameter takes. Nothing changes.
    pub fn set(&mut self, params: &Parameters, id: &Id, value: Value) -> Result<(), TypeError> {
        let def = self.require_param(params, id)?;
        if def.holder() == Holder::Instance {
            return Err(TypeError::HeldByInstance {
                parameter: def.name().to_owned(),
            });
        }
        self.values.set(def, value)?;
        Ok(())
    }

    /// Takes its value of a parameter out, so its instances read the default; returns the value.
    pub fn clear(&mut self, id: &Id) -> Option<Value> {
        self.values.remove(id)
    }

    /// Its element record in `document`: kind [`TYPE_KIND`], its name, the type recipe, and in
    /// `props` its category, its parameter IDs and its values (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = self.values.to_props();
        props.insert(CATEGORY_KEY.to_owned(), write_category(self.category));
        props.insert(PARAMS_KEY.to_owned(), write_params(&self.params));
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: TYPE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: type_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a type from its element record. Anything [`ElementType::to_record`] would not write
    /// is refused, never repaired, so one type has one byte string.
    ///
    /// # Errors
    /// [`TypeError::Malformed`] for a record that is not a type record as written;
    /// [`TypeError::UnknownCategory`] or [`TypeError::UnknownParameter`] for an ID the caller does
    /// not know; [`TypeError::HeldByInstance`] for a value of a parameter each instance holds;
    /// [`TypeError::Param`] for a value its parameter refuses.
    pub fn from_record(
        record: &Element,
        params: &Parameters,
        categories: &CategoryTable,
    ) -> Result<Self, TypeError> {
        let malformed = |why: &str| Err(TypeError::Malformed(why.to_owned()));
        if record.kind != TYPE_KIND {
            return malformed("the record's kind is not \"type\"");
        }
        if record.recipe != type_recipe() {
            return malformed("a type record's recipe is the type recipe");
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return malformed("a type record has no level, placement or references");
        }
        if clean_name(&record.name).ok().as_deref() != Some(record.name.as_str()) {
            return malformed("a type's name is trimmed and not empty");
        }
        let Some(category) = record.props.get(CATEGORY_KEY) else {
            return malformed("a type record names its category");
        };
        let category = read_category(category)?;
        if categories.get(category).is_none() {
            return Err(TypeError::UnknownCategory(category));
        }
        let Some(listed) = record.props.get(PARAMS_KEY) else {
            return malformed("a type record lists its parameters");
        };
        let listed = read_params(listed)?;
        for id in &listed {
            params.require(id)?;
        }
        let mut values = Values::new();
        for (key, text) in &record.props {
            if key == CATEGORY_KEY || key == PARAMS_KEY {
                continue;
            }
            if is_reserved(key) {
                return malformed("a type record has an unknown reserved key");
            }
            let id = read_id(key, "props key")?;
            if !listed.contains(&id) {
                return malformed("a type record holds a value of a parameter it does not list");
            }
            let def = params.require(&id)?;
            if def.holder() == Holder::Instance {
                return Err(TypeError::HeldByInstance {
                    parameter: def.name().to_owned(),
                });
            }
            values.set(def, def.read(text)?)?;
        }
        Ok(Self {
            id: record.id.clone(),
            name: record.name.clone(),
            category,
            params: listed,
            values,
        })
    }

    /// The definition of a parameter its elements have.
    pub(crate) fn require_param<'a>(
        &self,
        params: &'a Parameters,
        id: &Id,
    ) -> Result<&'a Definition, TypeError> {
        let def = params.require(id)?;
        if !self.has(id) {
            return Err(TypeError::NotOnType {
                type_name: self.name.clone(),
                parameter: def.name().to_owned(),
            });
        }
        Ok(def)
    }

    /// Gives it a name already cleaned and checked by [`crate::Types`].
    pub(crate) fn rename_to(&mut self, name: String) {
        self.name = name;
    }

    /// A copy under a new ID and a name already cleaned and checked by [`crate::Types`].
    pub(crate) fn copy_as(&self, id: Id, name: String) -> Self {
        Self {
            id,
            name,
            ..self.clone()
        }
    }
}
