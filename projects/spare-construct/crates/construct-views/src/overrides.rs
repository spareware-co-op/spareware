//! Per-element overrides in one view (CON-A8-F3-P6): how this one element looks in this one view,
//! over what its category's rule, a filter or the project says.
//!
//! An override is held by the view and names an element by its ID and the parts of style it
//! changes ([`ElementStyle`]): line weight, line colour, line pattern, halftone and hiding the
//! element in this view. It changes nothing about the element and nothing in any other view, and
//! it is not a template setting: a view tied to a template keeps its own overrides. An override
//! of an element that no longer exists does no harm and is simply never asked for; clearing the
//! view's overrides takes them all away.
//!
//! Stored form ([`OVERRIDES_KEY`] on the view record): one JSON text written only when the view
//! has an override: `{"format":1,"overrides":[{"element":"<id>","style":{...}}]}`, in ascending
//! element ID, never one with an empty style, no spare space. A reader refuses any other spelling.

use crate::style::ElementStyle;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The record key a view's overrides are stored under.
pub const OVERRIDES_KEY: &str = "@view.overrides";
/// The version of the stored form this crate writes.
pub const OVERRIDES_FORMAT_VERSION: u32 = 1;

/// A view's overrides: the style parts of each element that has any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    by_element: BTreeMap<Id, ElementStyle>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredOverride {
    element: String,
    style: ElementStyle,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    overrides: Vec<StoredOverride>,
}

impl Overrides {
    /// No overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_element.is_empty()
    }

    /// Every override, in ascending element ID.
    pub fn iter(&self) -> impl Iterator<Item = (&Id, &ElementStyle)> {
        self.by_element.iter()
    }

    /// The override of one element, if it has one.
    #[must_use]
    pub fn get(&self, element: &Id) -> Option<&ElementStyle> {
        self.by_element.get(element)
    }

    /// These overrides with `style` for `element`, replacing what it had; an empty style is no
    /// override.
    #[must_use]
    pub fn with(&self, element: &Id, style: ElementStyle) -> Self {
        let mut by_element = self.by_element.clone();
        if style.is_empty() {
            by_element.remove(element);
        } else {
            by_element.insert(element.clone(), style);
        }
        Self { by_element }
    }

    /// These overrides without the one of `element`, or without any when it is `None`.
    #[must_use]
    pub fn without(&self, element: Option<&Id>) -> Self {
        match element {
            None => Self::new(),
            Some(element) => {
                let mut by_element = self.by_element.clone();
                by_element.remove(element);
                Self { by_element }
            }
        }
    }

    /// The stored text, or `None` for no overrides (the record then has no key).
    pub(crate) fn to_text(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let overrides = self
            .by_element
            .iter()
            .map(|(element, style)| StoredOverride {
                element: element.to_string(),
                style: style.clone(),
            })
            .collect();
        // Plain structs of strings and numbers: serialising one cannot fail, and a record that
        // held nothing here would be refused when read.
        Some(
            serde_json::to_string(&Stored {
                format: OVERRIDES_FORMAT_VERSION,
                overrides,
            })
            .unwrap_or_default(),
        )
    }

    /// Reads the stored text. The caller compares the record it makes of the result with the one
    /// it was given, so any other spelling of the same overrides is refused there.
    pub(crate) fn from_text(text: &str) -> Result<Self, String> {
        let stored: Stored = serde_json::from_str(text)
            .map_err(|why| format!("holds overrides that do not read: {why}"))?;
        if stored.format != OVERRIDES_FORMAT_VERSION {
            return Err(format!(
                "holds overrides of version {}, which is not known",
                stored.format
            ));
        }
        let mut by_element = BTreeMap::new();
        let mut last: Option<Id> = None;
        for one in stored.overrides {
            let element = Id::parse(&one.element)
                .map_err(|_| "holds an override of an element with no ID".to_owned())?;
            if last.as_ref().is_some_and(|before| *before >= element) {
                return Err("holds overrides not in ascending element order".to_owned());
            }
            if one.style.is_empty() {
                return Err("holds an override with no style".to_owned());
            }
            last = Some(element.clone());
            by_element.insert(element, one.style);
        }
        if by_element.is_empty() {
            return Err("holds overrides with no override in them".to_owned());
        }
        Ok(Self { by_element })
    }
}
