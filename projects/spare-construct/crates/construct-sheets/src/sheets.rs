//! A document's sheets and viewports, with the rules a command checks before it writes one: a
//! sheet number unique in the document without regard to case, a view on one sheet only, a
//! viewport's centre on its paper, alignments to viewports of the same sheet and never in a loop.
//! Reading a document's records never applies them, so an old document always reads: they bind
//! what is written from now on.

use crate::error::SheetError;
use crate::sheet::{SHEET_KIND, Sheet};
use crate::viewport::{Alignment, VIEWPORT_KIND, Viewport};
use construct_model::element::Element;
use construct_params::Length;
use kit_core::Id;
use std::collections::BTreeMap;

/// A document's sheets and the viewports on them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheets {
    sheets: BTreeMap<Id, Sheet>,
    viewports: BTreeMap<Id, Viewport>,
}

fn same_number(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Sheets {
    /// No sheets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's sheets and viewports from its records: every record of kind sheet or
    /// viewport. The same records in any order give the same sheets. The rules bind commands, not
    /// reading, so two sheets of one number are both read.
    ///
    /// # Errors
    /// [`SheetError::Malformed`] for a record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, SheetError> {
        let mut found = Self::new();
        for record in records {
            if record.kind == SHEET_KIND {
                let sheet = Sheet::from_record(record)?;
                found.sheets.insert(sheet.id().clone(), sheet);
            } else if record.kind == VIEWPORT_KIND {
                let viewport = Viewport::from_record(record)?;
                found.viewports.insert(viewport.id().clone(), viewport);
            }
        }
        Ok(found)
    }

    /// The sheet of this ID.
    #[must_use]
    pub fn sheet(&self, id: &Id) -> Option<&Sheet> {
        self.sheets.get(id)
    }

    /// The viewport of this ID.
    #[must_use]
    pub fn viewport(&self, id: &Id) -> Option<&Viewport> {
        self.viewports.get(id)
    }

    /// How many sheets there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    /// Whether there are no sheets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }

    /// Every sheet, by number without regard to case, then by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&Sheet> {
        let mut all: Vec<&Sheet> = self.sheets.values().collect();
        all.sort_by(|a, b| {
            a.number()
                .to_lowercase()
                .cmp(&b.number().to_lowercase())
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    /// The viewports on a sheet, by ID.
    #[must_use]
    pub fn viewports_of(&self, sheet: &Id) -> Vec<&Viewport> {
        self.viewports
            .values()
            .filter(|viewport| viewport.sheet() == sheet)
            .collect()
    }

    /// Every viewport, by ID.
    #[must_use]
    pub fn viewports(&self) -> Vec<&Viewport> {
        self.viewports.values().collect()
    }

    /// The viewport that shows a view, and so the sheet the view is on.
    #[must_use]
    pub fn placement_of(&self, view: &Id) -> Option<(&Sheet, &Viewport)> {
        let viewport = self.viewports.values().find(|v| v.view() == view)?;
        self.sheets
            .get(viewport.sheet())
            .map(|sheet| (sheet, viewport))
    }

    /// Every sheet as a record of `document`, then every viewport, each in ID order. Parameter
    /// values are not records of their own: they are in each sheet's `props`, which a caller
    /// that holds them writes with [`Sheet::over`].
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        self.sheets
            .values()
            .map(|sheet| sheet.to_record(document))
            .chain(self.viewports.values().map(|v| v.to_record(document)))
            .collect()
    }

    fn number_free(&self, number: &str, except: &Id) -> Result<(), SheetError> {
        if self
            .sheets
            .values()
            .any(|sheet| sheet.id() != except && same_number(sheet.number(), number))
        {
            return Err(SheetError::DuplicateNumber(number.to_owned()));
        }
        Ok(())
    }

    /// Adds a sheet.
    ///
    /// # Errors
    /// [`SheetError::DuplicateSheet`] for an ID a sheet has, [`SheetError::DuplicateNumber`] for a
    /// number one has.
    pub fn add_sheet(&mut self, sheet: Sheet) -> Result<(), SheetError> {
        if self.sheets.contains_key(sheet.id()) {
            return Err(SheetError::DuplicateSheet(sheet.id().clone()));
        }
        self.number_free(sheet.number(), sheet.id())?;
        self.sheets.insert(sheet.id().clone(), sheet);
        Ok(())
    }

    /// Puts a changed sheet in the place of the one of its ID: another number, name or paper.
    /// Paper that would leave a viewport's centre off it is refused.
    ///
    /// # Errors
    /// [`SheetError::UnknownSheet`], [`SheetError::DuplicateNumber`] or [`SheetError::OffPaper`].
    pub fn replace_sheet(&mut self, sheet: Sheet) -> Result<(), SheetError> {
        if !self.sheets.contains_key(sheet.id()) {
            return Err(SheetError::UnknownSheet(sheet.id().clone()));
        }
        self.number_free(sheet.number(), sheet.id())?;
        for viewport in self.viewports_of(sheet.id()) {
            on_paper(&sheet, viewport)?;
        }
        self.sheets.insert(sheet.id().clone(), sheet);
        Ok(())
    }

    /// Takes a sheet and its viewports away and gives them back.
    ///
    /// # Errors
    /// [`SheetError::UnknownSheet`].
    pub fn remove_sheet(&mut self, id: &Id) -> Result<(Sheet, Vec<Viewport>), SheetError> {
        let sheet = self
            .sheets
            .remove(id)
            .ok_or_else(|| SheetError::UnknownSheet(id.clone()))?;
        let gone: Vec<Id> = self
            .viewports_of(id)
            .into_iter()
            .map(|viewport| viewport.id().clone())
            .collect();
        let viewports = gone
            .iter()
            .filter_map(|viewport| self.viewports.remove(viewport))
            .collect();
        Ok((sheet, viewports))
    }

    /// Adds a viewport.
    ///
    /// # Errors
    /// [`SheetError::DuplicateViewport`], [`SheetError::UnknownSheet`],
    /// [`SheetError::ViewPlaced`] for a view that is on a sheet already, [`SheetError::OffPaper`]
    /// or [`SheetError::BadAlignment`].
    pub fn add_viewport(&mut self, viewport: Viewport) -> Result<(), SheetError> {
        if self.viewports.contains_key(viewport.id()) {
            return Err(SheetError::DuplicateViewport(viewport.id().clone()));
        }
        let sheet = self
            .sheets
            .get(viewport.sheet())
            .ok_or_else(|| SheetError::UnknownSheet(viewport.sheet().clone()))?;
        if let Some((on, _)) = self.placement_of(viewport.view()) {
            return Err(SheetError::ViewPlaced {
                view: viewport.view().clone(),
                sheet_number: on.number().to_owned(),
            });
        }
        on_paper(sheet, &viewport)?;
        let id = viewport.id().clone();
        self.viewports.insert(id.clone(), viewport);
        if let Err(refusal) = self.check_alignment(&id) {
            self.viewports.remove(&id);
            return Err(refusal);
        }
        Ok(())
    }

    /// Puts a changed viewport in the place of the one of its ID: its position, title or
    /// alignments. Its sheet and view do not change (a viewport is taken away and made again to
    /// move a view to another sheet). A position on an axis the viewport stays aligned on does not
    /// change either: it follows the viewport it is aligned to.
    ///
    /// # Errors
    /// [`SheetError::UnknownViewport`], [`SheetError::BadAlignment`], [`SheetError::Aligned`] or
    /// [`SheetError::OffPaper`].
    pub fn replace_viewport(&mut self, viewport: Viewport) -> Result<(), SheetError> {
        let old = self
            .viewports
            .get(viewport.id())
            .ok_or_else(|| SheetError::UnknownViewport(viewport.id().clone()))?
            .clone();
        if old.sheet() != viewport.sheet() || old.view() != viewport.view() {
            return Err(SheetError::BadAlignment(
                "a viewport keeps its sheet and its view".to_owned(),
            ));
        }
        if old.align_x().is_some() && old.align_x() == viewport.align_x() && old.x() != viewport.x()
        {
            return Err(SheetError::Aligned("its horizontal position".to_owned()));
        }
        if old.align_y().is_some() && old.align_y() == viewport.align_y() && old.y() != viewport.y()
        {
            return Err(SheetError::Aligned("its vertical position".to_owned()));
        }
        let sheet = self
            .sheets
            .get(viewport.sheet())
            .ok_or_else(|| SheetError::UnknownSheet(viewport.sheet().clone()))?;
        on_paper(sheet, &viewport)?;
        let id = viewport.id().clone();
        self.viewports.insert(id.clone(), viewport);
        if let Err(refusal) = self.check_alignment(&id) {
            self.viewports.insert(id, old);
            return Err(refusal);
        }
        Ok(())
    }

    /// Takes a viewport away and gives it back.
    ///
    /// # Errors
    /// [`SheetError::UnknownViewport`], or [`SheetError::Followed`] while another viewport is
    /// aligned to it.
    pub fn remove_viewport(&mut self, id: &Id) -> Result<Viewport, SheetError> {
        if !self.viewports.contains_key(id) {
            return Err(SheetError::UnknownViewport(id.clone()));
        }
        let follower = self
            .viewports
            .values()
            .find(|other| {
                other.id() != id
                    && [other.align_x(), other.align_y()]
                        .into_iter()
                        .flatten()
                        .any(|align| &align.to == id)
            })
            .map(|other| other.id().clone());
        if let Some(follower) = follower {
            return Err(SheetError::Followed {
                viewport: id.clone(),
                follower,
            });
        }
        self.viewports
            .remove(id)
            .ok_or_else(|| SheetError::UnknownViewport(id.clone()))
    }

    /// The alignments of a viewport name viewports of its own sheet, not itself, and lead round no
    /// loop. The viewport is already in the collection.
    fn check_alignment(&self, id: &Id) -> Result<(), SheetError> {
        let Some(viewport) = self.viewports.get(id) else {
            return Ok(());
        };
        let axes: [(&str, Axis); 2] = [
            ("across the paper", Viewport::align_x),
            ("up the paper", Viewport::align_y),
        ];
        for (name, axis) in axes {
            let Some(first) = axis(viewport) else {
                continue;
            };
            let mut next = Some(first.to.clone());
            let mut steps = 0_usize;
            while let Some(id) = next {
                if &id == viewport.id() {
                    return Err(SheetError::BadAlignment(format!(
                        "{name}, a viewport cannot be aligned to itself or round a loop"
                    )));
                }
                let Some(target) = self.viewports.get(&id) else {
                    return Err(SheetError::UnknownViewport(id));
                };
                if target.sheet() != viewport.sheet() {
                    return Err(SheetError::BadAlignment(format!(
                        "{name}, the viewport {id} is on another sheet"
                    )));
                }
                steps = steps.saturating_add(1);
                if steps > self.viewports.len() {
                    return Err(SheetError::BadAlignment(format!(
                        "{name}, the alignments run in a loop"
                    )));
                }
                next = axis(target).map(|align| align.to.clone());
            }
        }
        Ok(())
    }
}

/// One of a viewport's two alignments.
type Axis = fn(&Viewport) -> Option<&Alignment>;

/// Whether a viewport's centre is on its sheet's paper, edges included.
fn on_paper(sheet: &Sheet, viewport: &Viewport) -> Result<(), SheetError> {
    let paper = sheet.paper();
    let zero = Length::from_steps(0);
    let inside = |at: Length, size: Length| at >= zero && at <= size;
    if inside(viewport.x(), paper.width()) && inside(viewport.y(), paper.height()) {
        return Ok(());
    }
    Err(SheetError::OffPaper {
        sheet_number: sheet.number().to_owned(),
        position: format!("{}, {}", viewport.x(), viewport.y()),
        paper: paper.describe(),
    })
}
