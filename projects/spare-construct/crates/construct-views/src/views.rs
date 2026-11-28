//! A document's views by ID, with the rule a command checks before it writes one: a name unique in
//! the document without regard to case. Reading a document's records never applies it, so an old
//! document always reads: it binds what is written from now on.

use crate::error::ViewError;
use crate::view::{VIEW_KIND, View};
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// A document's views.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Views {
    by_id: BTreeMap<Id, View>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Views {
    /// No views.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's views from its records: every record of kind view. The same records in any
    /// order give the same views. Two records of one name are both read: the name rule binds
    /// commands, not reading.
    ///
    /// # Errors
    /// [`ViewError::Malformed`] for a view record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, ViewError> {
        let mut views = Self::new();
        for record in records
            .into_iter()
            .filter(|record| record.kind == VIEW_KIND)
        {
            let view = View::from_record(record)?;
            views.by_id.insert(view.id().clone(), view);
        }
        Ok(views)
    }

    /// The view of this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&View> {
        self.by_id.get(id)
    }

    /// How many views there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every view, by name without regard to case, then by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&View> {
        let mut all: Vec<&View> = self.by_id.values().collect();
        all.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    /// The same views each put through `change`: a view template applies to the views tied to it
    /// this way.
    #[must_use]
    pub fn mapped(&self, change: impl Fn(&View) -> View) -> Self {
        Self {
            by_id: self
                .by_id
                .iter()
                .map(|(id, view)| (id.clone(), change(view)))
                .collect(),
        }
    }

    /// Every view as a record of `document`, in ID order.
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        self.by_id
            .values()
            .map(|view| view.to_record(document))
            .collect()
    }

    /// Refuses a name another view has, case aside.
    fn name_free(&self, name: &str, except: &Id) -> Result<(), ViewError> {
        if self
            .by_id
            .values()
            .any(|view| view.id() != except && same_name(view.name(), name))
        {
            return Err(ViewError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }

    /// Adds a view.
    ///
    /// # Errors
    /// [`ViewError::DuplicateView`] for an ID a view has, [`ViewError::DuplicateName`] for a name
    /// one has.
    pub fn add(&mut self, view: View) -> Result<(), ViewError> {
        if self.by_id.contains_key(view.id()) {
            return Err(ViewError::DuplicateView(view.id().clone()));
        }
        self.name_free(view.name(), view.id())?;
        self.by_id.insert(view.id().clone(), view);
        Ok(())
    }

    /// Puts a changed view in the place of the one of its ID: another name, scale, detail level,
    /// kind or phase settings. A view may change the case of its own name.
    ///
    /// # Errors
    /// [`ViewError::UnknownView`] if no view has its ID, [`ViewError::DuplicateName`] for a name
    /// another view has.
    pub fn replace(&mut self, view: View) -> Result<(), ViewError> {
        if !self.by_id.contains_key(view.id()) {
            return Err(ViewError::UnknownView(view.id().clone()));
        }
        self.name_free(view.name(), view.id())?;
        self.by_id.insert(view.id().clone(), view);
        Ok(())
    }

    /// Takes a view away and gives it back.
    ///
    /// # Errors
    /// [`ViewError::UnknownView`] if no view has the ID.
    pub fn remove(&mut self, id: &Id) -> Result<View, ViewError> {
        self.by_id
            .remove(id)
            .ok_or_else(|| ViewError::UnknownView(id.clone()))
    }
}
