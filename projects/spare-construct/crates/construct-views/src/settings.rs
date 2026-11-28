//! The settings of a view that a view template can hold (CON-A8-F3-P4): scale, detail level,
//! graphics rules by category, which categories show, and filters.
//!
//! [`Settings`] is those five together as a plain value. A view holds them as its own fields and a
//! template holds one [`Settings`] too, so one function can change either. A [`Setting`] names one
//! of the five, which is what a template's "included" flag is for: only the included settings of a
//! template reach a view tied to it or a view it is applied to.
//!
//! "Which categories show" is a view's list of hidden categories (a category's number, or a
//! subcategory's), stored under [`HIDDEN_KEY`] as the numbers in ascending order separated by
//! commas, written only when there is one. It is the view-wide switch above the graphics rules'
//! `shown` of one edge: a hidden category is not drawn in either edge.

use crate::filters::Filters;
use crate::graphics::Graphics;
use crate::kind::{DetailLevel, ViewKind};
use construct_categories::CategoryId;
use construct_styles::Scale;
use std::collections::BTreeSet;

/// The record key the hidden categories of a view or a template are stored under.
pub const HIDDEN_KEY: &str = "@view.hidden";

/// One of the settings a template can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Setting {
    /// The scale, 1:n.
    Scale,
    /// The detail level.
    Detail,
    /// The graphics rules by category and subcategory.
    Graphics,
    /// Which categories show.
    Categories,
    /// The filters.
    Filters,
}

impl Setting {
    /// Every setting, in the order they are stored and listed.
    pub const ALL: [Self; 5] = [
        Self::Scale,
        Self::Detail,
        Self::Graphics,
        Self::Categories,
        Self::Filters,
    ];

    /// The key a setting is spelt with, in commands, answers and records.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Scale => "scale",
            Self::Detail => "detail",
            Self::Graphics => "graphics",
            Self::Categories => "categories",
            Self::Filters => "filters",
        }
    }

    /// The setting a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|setting| setting.key() == key)
    }

    /// The setting in words, as a refusal names it.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Scale => "scale",
            Self::Detail => "detail level",
            Self::Graphics => "graphics rules",
            Self::Categories => "categories shown",
            Self::Filters => "filters",
        }
    }
}

/// The five settings of a view or a template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The scale.
    pub scale: Scale,
    /// The detail level.
    pub detail: DetailLevel,
    /// The graphics rules.
    pub graphics: Graphics,
    /// The categories (and subcategories) not drawn.
    pub hidden: BTreeSet<CategoryId>,
    /// The filters.
    pub filters: Filters,
}

impl Settings {
    /// These settings as a view of `kind` can hold them: graphics rules a view of that kind cannot
    /// hold are left out (none at all for a drafting view or a legend, projection rules only for a
    /// three-dimensional view).
    #[must_use]
    pub fn fit_to(&self, kind: &ViewKind) -> Self {
        let mut fitted = self.clone();
        fitted.graphics = self.graphics.for_kind(kind);
        fitted
    }

    /// These settings with the ones in `which` taken from `from`.
    #[must_use]
    pub fn taking(&self, from: &Self, which: &BTreeSet<Setting>) -> Self {
        let mut taken = self.clone();
        for setting in which {
            match setting {
                Setting::Scale => taken.scale = from.scale,
                Setting::Detail => taken.detail = from.detail,
                Setting::Graphics => taken.graphics = from.graphics.clone(),
                Setting::Categories => taken.hidden.clone_from(&from.hidden),
                Setting::Filters => taken.filters = from.filters.clone(),
            }
        }
        taken
    }
}

/// The stored text of hidden categories, or `None` for none.
pub(crate) fn hidden_to_text(hidden: &BTreeSet<CategoryId>) -> Option<String> {
    if hidden.is_empty() {
        return None;
    }
    Some(
        hidden
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// Reads stored hidden categories; the caller compares the record it makes of them with the one it
/// was given, so any other spelling is refused there.
pub(crate) fn hidden_from_text(text: &str) -> Result<BTreeSet<CategoryId>, String> {
    let mut hidden = BTreeSet::new();
    for part in text.split(',') {
        let n: u32 = part
            .parse()
            .map_err(|_| "holds hidden categories that are not numbers".to_owned())?;
        hidden.insert(CategoryId(n));
    }
    Ok(hidden)
}
