//! Object styles: how each category and subcategory draws (CON-A8-F3-P1, second half).
//!
//! An object style says, for one category or subcategory (by [`CategoryId`]): the pen its edges
//! print with where a view sees the element in projection, the pen where a view cuts through it,
//! a line colour and a line pattern. Pens are named by number ([`crate::pens`]) and patterns by
//! [`PatternId`] ([`crate::patterns`]); this module stores neither, so a pen or pattern edit never
//! touches it.
//!
//! **Which pen table a number means.** A category of kind annotation or view is drawn at a fixed
//! printed size, so its pen numbers name [`crate::pens::AnnotationPens`] (one width at every
//! scale). Every other category (model, datum, analytical, import) names [`crate::pens::PenTable`]
//! (a width per view scale). A pen number must exist in the table its category uses.
//!
//! **Cut pen.** A category has a cut pen exactly when it can be cut, by the category's own
//! "can be cut" flag in `construct-categories`. A cut pen on a category that cannot be cut is
//! refused ([`ObjectStyleError::CutPenOnUncuttable`]); a category that can be cut must have one
//! ([`ObjectStyleError::CutPenRequired`]); asking for the cut edge of one that cannot be cut is
//! refused too ([`ObjectStyleError::NotCuttable`]).
//!
//! **Inheritance rule.** A subcategory with no entry of its own takes its parent's style, except
//! that it has a cut pen only if it can itself be cut (a door's swing arc, which cannot be cut,
//! inherits the door's projection pen, colour and pattern and no cut pen). Every built-in
//! top-level category always has an entry, so the rule always finds a value. A project's own
//! subcategory starts with no entry and so looks like its parent until it is given one
//! ([`ObjectStyles::with_style`]); [`ObjectStyles::without_style`] takes the entry away again.
//!
//! **Defaults.** [`ObjectStyles::default_metric`] is our default set, with an entry for every
//! built-in category and subcategory: see [`crate::object_default`] for how it was weighted.
//!
//! **Changes** are pure functions that return a new table and leave the old one as it was.
//! Putting them through the edit log is a later job. The pen, pattern and category tables a
//! change is checked against are passed in, so this module owns no copy of them.
//!
//! **Stored form.** One JSON document: `{"format":1,"entries":[...]}`, entries in ascending
//! category id, each `{"category":n,"style":{"projection_pen":n,"cut_pen":n or null,"colour":
//! [r,g,b],"pattern":n}}`, no spare space: the same table is always the same bytes, and a reader
//! refuses any other spelling of the same data.

use crate::error::StyleError;
use crate::object_default::default_entries;
use crate::patterns::PatternId;
use crate::stored::LineStyles;
use crate::units::{Micrometres, Scale};
use construct_categories::{BUILT_IN, Category, CategoryId, CategoryKind, CategoryTable};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The version of the stored form this crate writes. Only version 1 exists so far; a reader never
/// stops reading an old one.
pub const OBJECT_STYLES_FORMAT_VERSION: u32 = 1;

/// A line colour: red, green and blue, each a whole number from 0 to 255. Stored exactly as
/// `[r,g,b]`; a value above 255 cannot be read.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour(
    /// Red, 0 to 255.
    pub u8,
    /// Green, 0 to 255.
    pub u8,
    /// Blue, 0 to 255.
    pub u8,
);

/// Which edge of an element a drawing is asking about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// An edge seen beyond the cut plane, or in a view that cuts nothing.
    Projected,
    /// An edge where the view's cut plane passes through the element.
    Cut,
}

/// One category's style: the stored values, with pens by number.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectStyle {
    /// The pen for edges seen in projection.
    pub projection_pen: u16,
    /// The pen for cut edges; `None` exactly when the category cannot be cut.
    pub cut_pen: Option<u16>,
    /// The line colour.
    pub colour: Colour,
    /// The line pattern.
    pub pattern: PatternId,
}

/// What the drawing code gets for one edge: the printed width, the colour and the pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeStyle {
    /// The printed width of the edge's pen at the view's scale.
    pub width: Micrometres,
    /// The line colour.
    pub colour: Colour,
    /// The line pattern.
    pub pattern: PatternId,
}

/// One stored entry: a category and its own style.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub(crate) category: CategoryId,
    pub(crate) style: ObjectStyle,
}

/// Why an object-style request or a stored table was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectStyleError {
    /// No category has this id in the category table given.
    UnknownCategory(CategoryId),
    /// The pen number does not exist in the pen table the category uses.
    NoSuchPen {
        /// The number given.
        pen: u16,
    },
    /// No pattern has this id.
    NoSuchPattern {
        /// The id given.
        id: u32,
    },
    /// A cut pen was given for a category that cannot be cut.
    CutPenOnUncuttable(CategoryId),
    /// No cut pen was given for a category that can be cut.
    CutPenRequired(CategoryId),
    /// The cut edge of a category that cannot be cut was asked for.
    NotCuttable(CategoryId),
    /// A built-in top-level category always keeps its own entry, so it cannot be removed.
    TopLevelNeedsStyle(CategoryId),
    /// A stored table has no entry for this built-in top-level category.
    MissingTopLevel(CategoryId),
    /// Stored entries are not in strictly ascending category order.
    EntriesNotAscending,
    /// The width of a pen could not be found.
    Lines(StyleError),
    /// The stored form is not valid: the reason as the JSON reader gave it.
    Format(String),
    /// The stored form is of a version this reader does not know.
    Version {
        /// The version found.
        found: u32,
    },
    /// The stored form is valid but is not spelt the one way this crate writes it.
    NotCanonical,
}

impl fmt::Display for ObjectStyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCategory(id) => write!(f, "no category has id {}", id.0),
            Self::NoSuchPen { pen } => write!(f, "there is no pen {pen} for this category"),
            Self::NoSuchPattern { id } => write!(f, "there is no pattern {id}"),
            Self::CutPenOnUncuttable(id) => {
                write!(f, "category {} cannot be cut, so it has no cut pen", id.0)
            }
            Self::CutPenRequired(id) => {
                write!(f, "category {} can be cut, so it needs a cut pen", id.0)
            }
            Self::NotCuttable(id) => write!(f, "category {} cannot be cut", id.0),
            Self::TopLevelNeedsStyle(id) => {
                write!(f, "top-level category {} must keep its own style", id.0)
            }
            Self::MissingTopLevel(id) => {
                write!(f, "top-level category {} has no style", id.0)
            }
            Self::EntriesNotAscending => {
                f.write_str("entries must be in strictly ascending category order")
            }
            Self::Lines(why) => write!(f, "{why}"),
            Self::Format(why) => write!(f, "not a valid stored form: {why}"),
            Self::Version { found } => write!(f, "stored form version {found} is not known"),
            Self::NotCanonical => f.write_str("the stored form is not spelt the way it is written"),
        }
    }
}

impl std::error::Error for ObjectStyleError {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    entries: Vec<Entry>,
}

/// The object styles of a project: an own style for every built-in top-level category and for
/// any subcategory that has been given one. See the module's documentation for the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectStyles {
    /// Ascending by category id.
    entries: Vec<Entry>,
}

/// Whether a category's pen numbers name the annotation pens (true) or the model pens (false).
fn uses_annotation_pens(kind: CategoryKind) -> bool {
    matches!(kind, CategoryKind::Annotation | CategoryKind::View)
}

fn pen_exists(lines: &LineStyles, kind: CategoryKind, pen: u16) -> bool {
    if uses_annotation_pens(kind) {
        lines.annotation_pens.pens().iter().any(|p| p.number == pen)
    } else {
        lines.model_pens.pen(pen).is_some()
    }
}

/// Every rule an own style must meet for `category`.
fn check_style(
    lines: &LineStyles,
    category: &Category,
    style: &ObjectStyle,
) -> Result<(), ObjectStyleError> {
    if !pen_exists(lines, category.kind, style.projection_pen) {
        return Err(ObjectStyleError::NoSuchPen {
            pen: style.projection_pen,
        });
    }
    match (category.cuttable, style.cut_pen) {
        (false, Some(_)) => return Err(ObjectStyleError::CutPenOnUncuttable(category.id)),
        (true, None) => return Err(ObjectStyleError::CutPenRequired(category.id)),
        (true, Some(pen)) if !pen_exists(lines, category.kind, pen) => {
            return Err(ObjectStyleError::NoSuchPen { pen });
        }
        _ => {}
    }
    if lines.patterns.get(style.pattern).is_none() {
        return Err(ObjectStyleError::NoSuchPattern {
            id: style.pattern.0,
        });
    }
    Ok(())
}

impl ObjectStyles {
    /// The default metric set: an entry for every built-in category and subcategory, designed in
    /// [`crate::object_default`]. It meets every rule against [`LineStyles::default_metric`].
    #[must_use]
    pub fn default_metric() -> Self {
        Self {
            entries: default_entries(),
        }
    }

    /// The categories that have an own style, in ascending id, with that style.
    #[must_use]
    pub fn entries(&self) -> Vec<(CategoryId, ObjectStyle)> {
        self.entries.iter().map(|e| (e.category, e.style)).collect()
    }

    /// The style a category was given itself, if it has one.
    #[must_use]
    pub fn own_style(&self, id: CategoryId) -> Option<ObjectStyle> {
        self.entries
            .binary_search_by_key(&id, |e| e.category)
            .ok()
            .and_then(|at| self.entries.get(at))
            .map(|e| e.style)
    }

    /// The style a category draws with: its own, or else its parent's by the inheritance rule in
    /// the module's documentation.
    ///
    /// # Errors
    /// [`ObjectStyleError::UnknownCategory`] when `categories` has no such category, and
    /// [`ObjectStyleError::MissingTopLevel`] when the table lacks a built-in top-level entry,
    /// which a table made here or read without error never does.
    pub fn style_of(
        &self,
        categories: &CategoryTable,
        id: CategoryId,
    ) -> Result<ObjectStyle, ObjectStyleError> {
        let category = categories
            .get(id)
            .ok_or(ObjectStyleError::UnknownCategory(id))?;
        if let Some(own) = self.own_style(id) {
            return Ok(own);
        }
        let parent = category
            .parent
            .ok_or(ObjectStyleError::MissingTopLevel(id))?;
        let inherited = self
            .own_style(parent)
            .ok_or(ObjectStyleError::MissingTopLevel(parent))?;
        Ok(ObjectStyle {
            cut_pen: if category.cuttable {
                inherited.cut_pen
            } else {
                None
            },
            ..inherited
        })
    }

    /// What a drawing needs for one edge of one category: the printed width at `scale`, the
    /// colour and the pattern.
    ///
    /// # Errors
    /// [`ObjectStyleError::UnknownCategory`] for an unknown category,
    /// [`ObjectStyleError::NotCuttable`] when `edge` is [`Edge::Cut`] and the category cannot be
    /// cut, and [`ObjectStyleError::Lines`] when the pen is not in `lines` (a table checked with
    /// the same `lines` never hits this).
    pub fn lookup(
        &self,
        categories: &CategoryTable,
        lines: &LineStyles,
        id: CategoryId,
        edge: Edge,
        scale: Scale,
    ) -> Result<EdgeStyle, ObjectStyleError> {
        let category = categories
            .get(id)
            .ok_or(ObjectStyleError::UnknownCategory(id))?;
        let style = self.style_of(categories, id)?;
        let pen = match edge {
            Edge::Projected => style.projection_pen,
            Edge::Cut => style.cut_pen.ok_or(ObjectStyleError::NotCuttable(id))?,
        };
        let width = if uses_annotation_pens(category.kind) {
            lines.annotation_pens.width(pen)
        } else {
            lines.model_pens.width(pen, scale)
        }
        .map_err(ObjectStyleError::Lines)?;
        Ok(EdgeStyle {
            width,
            colour: style.colour,
            pattern: style.pattern,
        })
    }

    /// This table with `style` as the own style of category `id`, replacing any it had. A
    /// project's own subcategory is given its style this way.
    ///
    /// # Errors
    /// [`ObjectStyleError::UnknownCategory`] for an unknown category;
    /// [`ObjectStyleError::NoSuchPen`] or [`ObjectStyleError::NoSuchPattern`] for a pen or pattern
    /// `lines` does not have; [`ObjectStyleError::CutPenOnUncuttable`] or
    /// [`ObjectStyleError::CutPenRequired`] for a cut pen that disagrees with the category's "can
    /// be cut" flag. This table is unchanged either way.
    pub fn with_style(
        &self,
        categories: &CategoryTable,
        lines: &LineStyles,
        id: CategoryId,
        style: ObjectStyle,
    ) -> Result<Self, ObjectStyleError> {
        let category = categories
            .get(id)
            .ok_or(ObjectStyleError::UnknownCategory(id))?;
        check_style(lines, &category, &style)?;
        let mut entries = self.entries.clone();
        let entry = Entry {
            category: id,
            style,
        };
        match entries.binary_search_by_key(&id, |e| e.category) {
            Ok(at) => {
                if let Some(slot) = entries.get_mut(at) {
                    *slot = entry;
                }
            }
            Err(at) => entries.insert(at, entry),
        }
        Ok(Self { entries })
    }

    /// This table with the own style of category `id` taken away, so that it inherits its
    /// parent's again. A category with no own style is left as it is.
    ///
    /// # Errors
    /// [`ObjectStyleError::UnknownCategory`] for an unknown category and
    /// [`ObjectStyleError::TopLevelNeedsStyle`] for a top-level category.
    pub fn without_style(
        &self,
        categories: &CategoryTable,
        id: CategoryId,
    ) -> Result<Self, ObjectStyleError> {
        let category = categories
            .get(id)
            .ok_or(ObjectStyleError::UnknownCategory(id))?;
        if category.parent.is_none() {
            return Err(ObjectStyleError::TopLevelNeedsStyle(id));
        }
        let entries = self
            .entries
            .iter()
            .filter(|e| e.category != id)
            .copied()
            .collect();
        Ok(Self { entries })
    }

    /// The stored bytes: the same table always gives the same bytes.
    ///
    /// # Errors
    /// [`ObjectStyleError::Format`] if the writer fails, which it does not for these types.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ObjectStyleError> {
        serde_json::to_vec(&Stored {
            format: OBJECT_STYLES_FORMAT_VERSION,
            entries: self.entries.clone(),
        })
        .map_err(|why| ObjectStyleError::Format(why.to_string()))
    }

    /// Reads stored bytes, checking every rule of [`ObjectStyles::with_style`] on each entry
    /// against `categories` and `lines`, that every built-in top-level category has an entry, and
    /// that the entries are in ascending category order.
    ///
    /// # Errors
    /// [`ObjectStyleError::Format`] for anything that is not valid JSON of this shape (a colour
    /// part above 255 is one), [`ObjectStyleError::Version`] for a version other than 1,
    /// [`ObjectStyleError::EntriesNotAscending`], [`ObjectStyleError::MissingTopLevel`], the
    /// refusal of any entry's own style, and [`ObjectStyleError::NotCanonical`] for valid data
    /// spelt another way.
    pub fn from_bytes(
        bytes: &[u8],
        categories: &CategoryTable,
        lines: &LineStyles,
    ) -> Result<Self, ObjectStyleError> {
        let stored: Stored = serde_json::from_slice(bytes)
            .map_err(|why| ObjectStyleError::Format(why.to_string()))?;
        if stored.format != OBJECT_STYLES_FORMAT_VERSION {
            return Err(ObjectStyleError::Version {
                found: stored.format,
            });
        }
        if !stored
            .entries
            .windows(2)
            .all(|pair| matches!(pair, [a, b] if a.category < b.category))
        {
            return Err(ObjectStyleError::EntriesNotAscending);
        }
        for entry in &stored.entries {
            let category = categories
                .get(entry.category)
                .ok_or(ObjectStyleError::UnknownCategory(entry.category))?;
            check_style(lines, &category, &entry.style)?;
        }
        let table = Self {
            entries: stored.entries,
        };
        for built in BUILT_IN.iter().filter(|b| b.parent.is_none()) {
            if table.own_style(built.id).is_none() {
                return Err(ObjectStyleError::MissingTopLevel(built.id));
            }
        }
        if table.to_bytes()?.as_slice() != bytes {
            return Err(ObjectStyleError::NotCanonical);
        }
        Ok(table)
    }
}
