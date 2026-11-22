//! A project's types by ID, and what is done to them as a set: names unique within a category,
//! rename, duplicate, delete.

use crate::element_type::{ElementType, clean_name};
use crate::error::TypeError;
use crate::instance::Instance;
use crate::parameters::Parameters;
use construct_categories::{CategoryId, CategoryTable};
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// Whether two type names are the same name, case aside.
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A project's types, by permanent ID. This is the caller's view of the type records, which are
/// the truth; every change here is pure, and the caller writes the changed record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Types(BTreeMap<Id, ElementType>);

impl Types {
    /// No types.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a type.
    ///
    /// # Errors
    /// [`TypeError::DuplicateType`] if its ID is taken; [`TypeError::DuplicateName`] if its
    /// category has a type of that name, case aside. Nothing changes.
    pub fn add(&mut self, ty: ElementType) -> Result<(), TypeError> {
        if self.0.contains_key(ty.id()) {
            return Err(TypeError::DuplicateType(ty.id().clone()));
        }
        self.free_name(ty.category(), ty.name(), None)?;
        self.0.insert(ty.id().clone(), ty);
        Ok(())
    }

    /// The type with this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&ElementType> {
        self.0.get(id)
    }

    /// The type with this ID, to set its values or give it parameters. Its name is changed only
    /// by [`Types::rename`], so names stay unique.
    pub fn get_mut(&mut self, id: &Id) -> Option<&mut ElementType> {
        self.0.get_mut(id)
    }

    /// Every type, in ID order.
    pub fn iter(&self) -> impl Iterator<Item = &ElementType> {
        self.0.values()
    }

    /// How many types.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The types of a category, in ID order.
    #[must_use]
    pub fn in_category(&self, category: CategoryId) -> Vec<&ElementType> {
        self.iter().filter(|ty| ty.category() == category).collect()
    }

    /// The type of a category with this name, case and surrounding spaces aside.
    #[must_use]
    pub fn named(&self, category: CategoryId, name: &str) -> Option<&ElementType> {
        let name = name.trim();
        self.iter()
            .find(|ty| ty.category() == category && same_name(ty.name(), name))
    }

    /// Renames a type. The name is trimmed; a type may change the case of its own name.
    ///
    /// # Errors
    /// [`TypeError::EmptyName`], [`TypeError::UnknownType`], or [`TypeError::DuplicateName`] if
    /// another type of its category has the name. Nothing changes.
    pub fn rename(&mut self, id: &Id, name: &str) -> Result<(), TypeError> {
        let name = clean_name(name)?;
        let category = self.require(id)?.category();
        self.free_name(category, &name, Some(id))?;
        if let Some(ty) = self.0.get_mut(id) {
            ty.rename_to(name);
        }
        Ok(())
    }

    /// Adds a copy of type `id` under `new_id` and `new_name`: the same category, parameters and
    /// values, standing alone from then on.
    ///
    /// # Errors
    /// [`TypeError::UnknownType`] for `id`, [`TypeError::DuplicateType`] for `new_id`,
    /// [`TypeError::EmptyName`] or [`TypeError::DuplicateName`] for `new_name`. Nothing changes.
    pub fn duplicate(&mut self, id: &Id, new_id: Id, new_name: &str) -> Result<(), TypeError> {
        let source = self.require(id)?;
        if self.0.contains_key(&new_id) {
            return Err(TypeError::DuplicateType(new_id));
        }
        let name = clean_name(new_name)?;
        self.free_name(source.category(), &name, None)?;
        let copy = source.copy_as(new_id.clone(), name);
        self.0.insert(new_id, copy);
        Ok(())
    }

    /// Takes type `id` out and returns it, if none of `instances` use it. The caller passes every
    /// instance in the model; this crate keeps no index of them.
    ///
    /// # Errors
    /// [`TypeError::UnknownType`]; [`TypeError::InUse`] naming how many of `instances` use it.
    /// Nothing changes.
    pub fn delete<'a, I>(&mut self, id: &Id, instances: I) -> Result<ElementType, TypeError>
    where
        I: IntoIterator<Item = &'a Instance>,
    {
        let ty = self.require(id)?;
        let count = instances
            .into_iter()
            .filter(|instance| instance.type_id() == id)
            .count();
        if count > 0 {
            return Err(TypeError::InUse {
                type_name: ty.name().to_owned(),
                count,
            });
        }
        self.0
            .remove(id)
            .ok_or_else(|| TypeError::UnknownType(id.clone()))
    }

    /// Every type's element record in `document`, in ID order.
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        self.iter().map(|ty| ty.to_record(document)).collect()
    }

    /// Reads a project's types from their records.
    ///
    /// # Errors
    /// As [`ElementType::from_record`] for each record, and as [`Types::add`] for two records
    /// whose IDs or names clash.
    pub fn from_records(
        records: &[Element],
        params: &Parameters,
        categories: &CategoryTable,
    ) -> Result<Self, TypeError> {
        let mut types = Self::new();
        for record in records {
            types.add(ElementType::from_record(record, params, categories)?)?;
        }
        Ok(types)
    }

    fn require(&self, id: &Id) -> Result<&ElementType, TypeError> {
        self.0
            .get(id)
            .ok_or_else(|| TypeError::UnknownType(id.clone()))
    }

    /// Refuses a name another type of the category has, `except` aside.
    fn free_name(
        &self,
        category: CategoryId,
        name: &str,
        except: Option<&Id>,
    ) -> Result<(), TypeError> {
        let taken = self.iter().any(|ty| {
            ty.category() == category && Some(ty.id()) != except && same_name(ty.name(), name)
        });
        if taken {
            return Err(TypeError::DuplicateName {
                category,
                name: name.to_owned(),
            });
        }
        Ok(())
    }
}
