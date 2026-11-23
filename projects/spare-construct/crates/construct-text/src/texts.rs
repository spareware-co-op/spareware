//! A document's text styles and notes, with the rules a command checks before it writes one: a
//! style name unique without regard to case, a note in a view that is there and reading a style
//! that is there, a style in use never taken away. Reading a document's records never applies the
//! rules, so an old document always reads: they bind what is written from now on.

use crate::error::TextError;
use crate::note::{NOTE_KIND, TextNote};
use crate::style::{STYLE_KIND, TextStyle};
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// A document's text styles and notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texts {
    styles: BTreeMap<Id, TextStyle>,
    notes: BTreeMap<Id, TextNote>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Texts {
    /// No styles and no notes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's styles and notes from its records: every record of kind text-style or
    /// text-note. The same records in any order give the same texts. The rules bind commands, not
    /// reading, so two styles of one name are both read.
    ///
    /// # Errors
    /// [`TextError::Malformed`] for a record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, TextError> {
        let mut found = Self::new();
        for record in records {
            if record.kind == STYLE_KIND {
                let style = TextStyle::from_record(record)?;
                found.styles.insert(style.id().clone(), style);
            } else if record.kind == NOTE_KIND {
                let note = TextNote::from_record(record)?;
                found.notes.insert(note.id().clone(), note);
            }
        }
        Ok(found)
    }

    /// The style of this ID.
    #[must_use]
    pub fn style(&self, id: &Id) -> Option<&TextStyle> {
        self.styles.get(id)
    }

    /// The note of this ID.
    #[must_use]
    pub fn note(&self, id: &Id) -> Option<&TextNote> {
        self.notes.get(id)
    }

    /// Every style, by name without regard to case, then by ID.
    #[must_use]
    pub fn styles(&self) -> Vec<&TextStyle> {
        let mut all: Vec<&TextStyle> = self.styles.values().collect();
        all.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    /// Every note, by ID.
    #[must_use]
    pub fn notes(&self) -> Vec<&TextNote> {
        self.notes.values().collect()
    }

    /// The notes of a view, by ID.
    #[must_use]
    pub fn notes_of(&self, view: &Id) -> Vec<&TextNote> {
        self.notes
            .values()
            .filter(|note| note.view() == view)
            .collect()
    }

    /// How many notes read a style.
    #[must_use]
    pub fn uses_of(&self, style: &Id) -> usize {
        self.notes
            .values()
            .filter(|note| note.style() == style)
            .count()
    }

    /// Every style as a record of `document`, then every note, each in ID order.
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        self.styles
            .values()
            .map(|style| style.to_record(document))
            .chain(self.notes.values().map(|note| note.to_record(document)))
            .collect()
    }

    fn name_free(&self, name: &str, except: &Id) -> Result<(), TextError> {
        if self
            .styles
            .values()
            .any(|style| style.id() != except && same_name(style.name(), name))
        {
            return Err(TextError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }

    /// Adds a style.
    ///
    /// # Errors
    /// [`TextError::DuplicateStyle`] for an ID a style has, [`TextError::DuplicateName`] for a
    /// name one has.
    pub fn add_style(&mut self, style: TextStyle) -> Result<(), TextError> {
        if self.styles.contains_key(style.id()) {
            return Err(TextError::DuplicateStyle(style.id().clone()));
        }
        self.name_free(style.name(), style.id())?;
        self.styles.insert(style.id().clone(), style);
        Ok(())
    }

    /// Puts a changed style in the place of the one of its ID. Every note that reads it reads the
    /// change.
    ///
    /// # Errors
    /// [`TextError::UnknownStyle`] or [`TextError::DuplicateName`].
    pub fn replace_style(&mut self, style: TextStyle) -> Result<(), TextError> {
        if !self.styles.contains_key(style.id()) {
            return Err(TextError::UnknownStyle(style.id().clone()));
        }
        self.name_free(style.name(), style.id())?;
        self.styles.insert(style.id().clone(), style);
        Ok(())
    }

    /// Takes a style away and gives it back.
    ///
    /// # Errors
    /// [`TextError::UnknownStyle`], or [`TextError::StyleInUse`] while notes read it.
    pub fn remove_style(&mut self, id: &Id) -> Result<TextStyle, TextError> {
        if !self.styles.contains_key(id) {
            return Err(TextError::UnknownStyle(id.clone()));
        }
        let notes = self.uses_of(id);
        if notes > 0 {
            return Err(TextError::StyleInUse {
                style: id.clone(),
                notes,
            });
        }
        self.styles
            .remove(id)
            .ok_or_else(|| TextError::UnknownStyle(id.clone()))
    }

    /// Adds a note. `view_exists` says whether a view of that ID is in the document.
    ///
    /// # Errors
    /// [`TextError::DuplicateNote`], [`TextError::UnknownView`] or [`TextError::UnknownStyle`].
    pub fn add_note(
        &mut self,
        note: TextNote,
        view_exists: impl Fn(&Id) -> bool,
    ) -> Result<(), TextError> {
        if self.notes.contains_key(note.id()) {
            return Err(TextError::DuplicateNote(note.id().clone()));
        }
        if !view_exists(note.view()) {
            return Err(TextError::UnknownView(note.view().clone()));
        }
        if !self.styles.contains_key(note.style()) {
            return Err(TextError::UnknownStyle(note.style().clone()));
        }
        self.notes.insert(note.id().clone(), note);
        Ok(())
    }

    /// Puts a changed note in the place of the one of its ID. A note keeps its view.
    ///
    /// # Errors
    /// [`TextError::UnknownNote`], [`TextError::UnknownStyle`], or [`TextError::Malformed`] for a
    /// note that would change its view.
    pub fn replace_note(&mut self, note: TextNote) -> Result<(), TextError> {
        let old = self
            .notes
            .get(note.id())
            .ok_or_else(|| TextError::UnknownNote(note.id().clone()))?;
        if old.view() != note.view() {
            return Err(TextError::Malformed(
                "A text note keeps its view: delete it and make another in the other view."
                    .to_owned(),
            ));
        }
        if !self.styles.contains_key(note.style()) {
            return Err(TextError::UnknownStyle(note.style().clone()));
        }
        self.notes.insert(note.id().clone(), note);
        Ok(())
    }

    /// Takes a note away and gives it back.
    ///
    /// # Errors
    /// [`TextError::UnknownNote`].
    pub fn remove_note(&mut self, id: &Id) -> Result<TextNote, TextError> {
        self.notes
            .remove(id)
            .ok_or_else(|| TextError::UnknownNote(id.clone()))
    }

    /// Whether a view may be deleted: it may not while it holds notes.
    ///
    /// # Errors
    /// [`TextError::ViewHasNotes`].
    pub fn view_free(&self, view: &Id) -> Result<(), TextError> {
        let notes = self.notes_of(view).len();
        if notes > 0 {
            return Err(TextError::ViewHasNotes {
                view: view.clone(),
                notes,
            });
        }
        Ok(())
    }
}
