//! A document's dimensions, and the rules that need to see them all.

use crate::dimension::{DIMENSION_KIND, Dimension};
use crate::error::DimensionError;
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// A document's dimensions, by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dimensions {
    all: BTreeMap<Id, Dimension>,
}

impl Dimensions {
    /// No dimensions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The dimensions among `records` (the document's records of every kind: others are left).
    ///
    /// # Errors
    /// [`DimensionError::Malformed`] for a dimension record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, DimensionError> {
        let mut found = Self::new();
        for record in records {
            if record.kind == DIMENSION_KIND {
                let dimension = Dimension::from_record(record)?;
                found.all.insert(dimension.id().clone(), dimension);
            }
        }
        Ok(found)
    }

    /// The dimension with this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Dimension> {
        self.all.get(id)
    }

    /// Every dimension, by ID.
    #[must_use]
    pub fn all(&self) -> Vec<&Dimension> {
        self.all.values().collect()
    }

    /// The dimensions in a view, by ID.
    #[must_use]
    pub fn of_view(&self, view: &Id) -> Vec<&Dimension> {
        self.all.values().filter(|d| d.view() == view).collect()
    }

    /// The dimensions that name `element` among their references, by ID.
    #[must_use]
    pub fn referencing(&self, element: &Id) -> Vec<&Dimension> {
        self.all
            .values()
            .filter(|d| d.references().iter().any(|r| r.element() == element))
            .collect()
    }

    /// Adds a dimension.
    ///
    /// # Errors
    /// [`DimensionError::Duplicate`] when its ID is taken.
    pub fn add(&mut self, dimension: Dimension) -> Result<(), DimensionError> {
        if self.all.contains_key(dimension.id()) {
            return Err(DimensionError::Duplicate(dimension.id().clone()));
        }
        self.all.insert(dimension.id().clone(), dimension);
        Ok(())
    }

    /// Replaces a dimension by one of the same ID.
    ///
    /// # Errors
    /// [`DimensionError::Unknown`] when there is none of that ID.
    pub fn replace(&mut self, dimension: Dimension) -> Result<(), DimensionError> {
        match self.all.get_mut(dimension.id()) {
            Some(slot) => {
                *slot = dimension;
                Ok(())
            }
            None => Err(DimensionError::Unknown(dimension.id().clone())),
        }
    }

    /// Takes a dimension away and gives it back.
    ///
    /// # Errors
    /// [`DimensionError::Unknown`] when there is none of that ID.
    pub fn remove(&mut self, id: &Id) -> Result<Dimension, DimensionError> {
        self.all
            .remove(id)
            .ok_or_else(|| DimensionError::Unknown(id.clone()))
    }
}
