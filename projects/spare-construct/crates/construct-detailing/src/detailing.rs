//! A document's detail items, and the lookups a command needs.
//!
//! Every item belongs to one view ([`Detailing::of_view`] is the only way to ask for what a view
//! shows); nothing here is a model element, so no other view, and no schedule or quantity, ever
//! sees one.

use crate::component::Library;
use crate::error::DetailError;
use crate::line::{DetailLine, LINE_KIND};
use crate::placed::{COMPONENT_KIND, DetailComponent, DetailRepeat, REPEAT_KIND};
use crate::region::{MASKING_KIND, REGION_KIND, Region};
use crate::slope::{SLOPE_KIND, SpotSlope};
use crate::style::{OwnStyles, STYLE_KIND, StyleRef};
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// Whether an element kind is one of a detail item's (including a person's own line style).
#[must_use]
pub fn is_detail_kind(kind: &str) -> bool {
    kind.starts_with(KIND_PREFIX)
}

/// Every detail kind begins with this: how a generic reader of records knows to leave them out of
/// anything with a shape.
pub const KIND_PREFIX: &str = "detail.";

/// What kind of item an ID is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    /// A detail line.
    Line,
    /// A filled region.
    Region,
    /// A masking region.
    Masking,
    /// A placed component.
    Component,
    /// A repeating component.
    Repeat,
    /// A spot slope.
    Slope,
}

impl ItemKind {
    /// The record kind.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Line => LINE_KIND,
            Self::Region => REGION_KIND,
            Self::Masking => MASKING_KIND,
            Self::Component => COMPONENT_KIND,
            Self::Repeat => REPEAT_KIND,
            Self::Slope => SLOPE_KIND,
        }
    }
}

/// A document's detail items, by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detailing {
    styles: OwnStyles,
    lines: BTreeMap<Id, DetailLine>,
    regions: BTreeMap<Id, Region>,
    components: BTreeMap<Id, DetailComponent>,
    repeats: BTreeMap<Id, DetailRepeat>,
    slopes: BTreeMap<Id, SpotSlope>,
}

/// What one view shows of the detail items, each kind by ID.
#[derive(Clone, Debug, Default)]
pub struct ViewItems<'a> {
    /// Its detail lines.
    pub lines: Vec<&'a DetailLine>,
    /// Its filled and masking regions.
    pub regions: Vec<&'a Region>,
    /// Its placed components.
    pub components: Vec<&'a DetailComponent>,
    /// Its repeating components.
    pub repeats: Vec<&'a DetailRepeat>,
    /// Its spot slopes.
    pub slopes: Vec<&'a SpotSlope>,
}

impl ViewItems<'_> {
    /// How many items in all.
    #[must_use]
    pub fn len(&self) -> usize {
        [
            self.lines.len(),
            self.regions.len(),
            self.components.len(),
            self.repeats.len(),
            self.slopes.len(),
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }

    /// Whether the view has none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Detailing {
    /// No items.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The detail items among `records` (the document's records of every kind: others are left).
    ///
    /// # Errors
    /// [`DetailError::Malformed`] for a detail record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element> + Clone,
        library: &Library,
    ) -> Result<Self, DetailError> {
        let mut found = Self {
            styles: OwnStyles::from_records(records.clone())?,
            ..Self::default()
        };
        for record in records {
            let id = record.id.clone();
            match record.kind.as_str() {
                LINE_KIND => {
                    found.lines.insert(id, DetailLine::from_record(record)?);
                }
                REGION_KIND | MASKING_KIND => {
                    found.regions.insert(id, Region::from_record(record)?);
                }
                COMPONENT_KIND => {
                    found
                        .components
                        .insert(id, DetailComponent::from_record(record, library)?);
                }
                REPEAT_KIND => {
                    found
                        .repeats
                        .insert(id, DetailRepeat::from_record(record, library)?);
                }
                SLOPE_KIND => {
                    found.slopes.insert(id, SpotSlope::from_record(record)?);
                }
                STYLE_KIND => {}
                other if is_detail_kind(other) => {
                    return Err(DetailError::Malformed(format!(
                        "The record {} is of kind {other}, which this version does not know.",
                        record.id
                    )));
                }
                _ => {}
            }
        }
        Ok(found)
    }

    /// The document's own line styles.
    #[must_use]
    pub fn styles(&self) -> &OwnStyles {
        &self.styles
    }

    /// What kind of item this ID is, if it is one.
    #[must_use]
    pub fn kind_of(&self, id: &Id) -> Option<ItemKind> {
        if self.lines.contains_key(id) {
            Some(ItemKind::Line)
        } else if let Some(region) = self.regions.get(id) {
            Some(if region.is_masking() {
                ItemKind::Masking
            } else {
                ItemKind::Region
            })
        } else if self.components.contains_key(id) {
            Some(ItemKind::Component)
        } else if self.repeats.contains_key(id) {
            Some(ItemKind::Repeat)
        } else if self.slopes.contains_key(id) {
            Some(ItemKind::Slope)
        } else {
            None
        }
    }

    /// The detail line with this ID.
    #[must_use]
    pub fn line(&self, id: &Id) -> Option<&DetailLine> {
        self.lines.get(id)
    }

    /// The region with this ID.
    #[must_use]
    pub fn region(&self, id: &Id) -> Option<&Region> {
        self.regions.get(id)
    }

    /// The placed component with this ID.
    #[must_use]
    pub fn component(&self, id: &Id) -> Option<&DetailComponent> {
        self.components.get(id)
    }

    /// The repeating component with this ID.
    #[must_use]
    pub fn repeat(&self, id: &Id) -> Option<&DetailRepeat> {
        self.repeats.get(id)
    }

    /// The spot slope with this ID.
    #[must_use]
    pub fn slope(&self, id: &Id) -> Option<&SpotSlope> {
        self.slopes.get(id)
    }

    /// What `view` shows, each kind in order of ID.
    #[must_use]
    pub fn of_view(&self, view: &Id) -> ViewItems<'_> {
        ViewItems {
            lines: self.lines.values().filter(|i| i.view() == view).collect(),
            regions: self.regions.values().filter(|i| i.view() == view).collect(),
            components: self
                .components
                .values()
                .filter(|i| i.view() == view)
                .collect(),
            repeats: self.repeats.values().filter(|i| i.view() == view).collect(),
            slopes: self.slopes.values().filter(|i| i.view() == view).collect(),
        }
    }

    /// The items that draw with the line style `style`: lines, and regions by their boundary.
    #[must_use]
    pub fn using_style(&self, style: &StyleRef) -> Vec<Id> {
        let mut users: Vec<Id> = self
            .lines
            .values()
            .filter(|l| l.style() == style)
            .map(|l| l.id().clone())
            .collect();
        users.extend(
            self.regions
                .values()
                .filter(|r| r.boundary() == Some(style))
                .map(|r| r.id().clone()),
        );
        users.sort();
        users
    }

    /// The items that read a face of `element`.
    #[must_use]
    pub fn reading(&self, element: &Id) -> Vec<&SpotSlope> {
        self.slopes
            .values()
            .filter(|s| s.face() == element)
            .collect()
    }
}
