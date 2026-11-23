//! One viewport: a view placed on a sheet. Its position on the paper (the centre of the viewport,
//! exact), whether its title shows, and its alignments to other viewports, written as an element
//! record of the viewports category and read back.
//!
//! Its scale is the view's own (the feature text gives a viewport no scale of its own), so a view
//! that changes scale changes its viewport's outline and nothing is stored twice.

use crate::error::SheetError;
use crate::sheet::{length, stored};
use construct_categories::VIEWPORTS;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::Length;
use construct_params::Value;
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every viewport record, and its recipe's operation.
pub const VIEWPORT_KIND: &str = "viewport";
/// The record's category.
pub const CATEGORY_KEY: &str = "@category";
/// The sheet the viewport is on, an ID.
pub const SHEET_KEY: &str = "@viewport.sheet";
/// The view it shows, an ID.
pub const VIEW_KEY: &str = "@viewport.view";
/// The centre's distance across the paper from its left edge, a stored length.
pub const X_KEY: &str = "@viewport.x";
/// The centre's distance up the paper from its bottom edge, a stored length.
pub const Y_KEY: &str = "@viewport.y";
/// Whether the view's title shows, in construct-params' stored yes or no text.
pub const TITLE_KEY: &str = "@viewport.title";
/// The viewport the horizontal position follows, an ID; its anchor is under [`ALIGN_X_ANCHOR_KEY`].
pub const ALIGN_X_KEY: &str = "@viewport.align.x";
/// Which part of the two viewports line up across the paper: `min` (left edge), `centre`, `max`.
pub const ALIGN_X_ANCHOR_KEY: &str = "@viewport.align.x.anchor";
/// The viewport the vertical position follows, an ID.
pub const ALIGN_Y_KEY: &str = "@viewport.align.y";
/// Which part of the two viewports line up up the paper: `min` (bottom edge), `centre`, `max`.
pub const ALIGN_Y_ANCHOR_KEY: &str = "@viewport.align.y.anchor";
/// The name every viewport record carries.
pub const VIEWPORT_NAME: &str = "Viewport";

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a viewport record: Spare Construct's own operation `viewport`, version 1, with no
/// arguments. It builds no shape.
#[must_use]
pub fn viewport_recipe() -> Recipe {
    Recipe::construct(VIEWPORT_KIND, BTreeMap::new())
}

/// Which part of a viewport lines up with the same part of another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Anchor {
    /// The left edge across the paper, the bottom edge up it.
    Min,
    /// The centre.
    Centre,
    /// The right edge across the paper, the top edge up it.
    Max,
}

impl Anchor {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Centre => "centre",
            Self::Max => "max",
        }
    }

    /// The anchor a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "min" => Some(Self::Min),
            "centre" => Some(Self::Centre),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

/// An alignment: this viewport's `anchor` stays level with the other's, whatever their sizes
/// become. It is data, not a position: [`crate::Sheets::layout`] works the position from it
/// every time, so it survives a view's extent changing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alignment {
    /// The viewport followed, on the same sheet.
    pub to: Id,
    /// What lines up.
    pub anchor: Anchor,
}

/// A viewport: a view on a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewport {
    id: Id,
    sheet: Id,
    view: Id,
    x: Length,
    y: Length,
    title: bool,
    align_x: Option<Alignment>,
    align_y: Option<Alignment>,
}

impl Viewport {
    /// A viewport with its centre at (`x`, `y`) on the paper, its title showing, aligned to
    /// nothing.
    #[must_use]
    pub fn new(id: Id, sheet: Id, view: Id, x: Length, y: Length) -> Self {
        Self {
            id,
            sheet,
            view,
            x,
            y,
            title: true,
            align_x: None,
            align_y: None,
        }
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The sheet it is on.
    #[must_use]
    pub fn sheet(&self) -> &Id {
        &self.sheet
    }

    /// The view it shows.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// Its centre across the paper, as last set by hand (the position used when it is not aligned
    /// on this axis).
    #[must_use]
    pub fn x(&self) -> Length {
        self.x
    }

    /// Its centre up the paper, as last set by hand.
    #[must_use]
    pub fn y(&self) -> Length {
        self.y
    }

    /// Whether its title shows.
    #[must_use]
    pub fn title(&self) -> bool {
        self.title
    }

    /// What it follows across the paper.
    #[must_use]
    pub fn align_x(&self) -> Option<&Alignment> {
        self.align_x.as_ref()
    }

    /// What it follows up the paper.
    #[must_use]
    pub fn align_y(&self) -> Option<&Alignment> {
        self.align_y.as_ref()
    }

    /// The viewport with its centre somewhere else.
    #[must_use]
    pub fn at(mut self, x: Length, y: Length) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// The viewport with its title on or off.
    #[must_use]
    pub fn with_title(mut self, title: bool) -> Self {
        self.title = title;
        self
    }

    /// The viewport aligned across the paper, or with that alignment taken away.
    #[must_use]
    pub fn with_align_x(mut self, align: Option<Alignment>) -> Self {
        self.align_x = align;
        self
    }

    /// The viewport aligned up the paper, or with that alignment taken away.
    #[must_use]
    pub fn with_align_y(mut self, align: Option<Alignment>) -> Self {
        self.align_y = align;
        self
    }

    /// The viewport as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), VIEWPORTS.0.to_string());
        props.insert(SHEET_KEY.to_owned(), self.sheet.to_string());
        props.insert(VIEW_KEY.to_owned(), self.view.to_string());
        props.insert(X_KEY.to_owned(), Value::Length(self.x).stored());
        props.insert(Y_KEY.to_owned(), Value::Length(self.y).stored());
        props.insert(TITLE_KEY.to_owned(), Value::YesNo(self.title).stored());
        for (align, key, anchor_key) in [
            (&self.align_x, ALIGN_X_KEY, ALIGN_X_ANCHOR_KEY),
            (&self.align_y, ALIGN_Y_KEY, ALIGN_Y_ANCHOR_KEY),
        ] {
            if let Some(align) = align {
                props.insert(key.to_owned(), align.to.to_string());
                props.insert(
                    anchor_key.to_owned(),
                    Value::Choice(align.anchor.key().to_owned()).stored(),
                );
            }
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: VIEWPORT_KIND.to_owned(),
            name: VIEWPORT_NAME.to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: viewport_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a viewport from its element record. Anything [`Viewport::to_record`] would not write
    /// is refused, never repaired.
    ///
    /// # Errors
    /// [`SheetError::Malformed`], naming the record, for a record that is not a viewport record
    /// as written.
    pub fn from_record(record: &Element) -> Result<Self, SheetError> {
        let malformed =
            |why: &str| SheetError::Malformed(format!("The viewport record {} {why}.", record.id));
        if record.kind != VIEWPORT_KIND || record.recipe != viewport_recipe() {
            return Err(malformed(
                "is not of kind viewport, with the viewport recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let id_at = |key: &str| {
            record
                .props
                .get(key)
                .and_then(|text| Id::parse(text).ok())
                .ok_or_else(|| malformed(&format!("holds no ID under {key}")))
        };
        let (Some(x), Some(y)) = (length(record, X_KEY), length(record, Y_KEY)) else {
            return Err(malformed("holds no position in its one spelling"));
        };
        let Some(Value::YesNo(title)) = stored(record, TITLE_KEY) else {
            return Err(malformed("holds no title setting in its one spelling"));
        };
        let align = |key: &str, anchor_key: &str| -> Result<Option<Alignment>, SheetError> {
            if !record.props.contains_key(key) {
                return Ok(None);
            }
            let Some(Value::Choice(anchor)) = stored(record, anchor_key) else {
                return Err(malformed("holds an alignment with no anchor"));
            };
            let anchor = Anchor::from_key(&anchor)
                .ok_or_else(|| malformed("holds an anchor that is not min, centre or max"))?;
            Ok(Some(Alignment {
                to: id_at(key)?,
                anchor,
            }))
        };
        let viewport = Self::new(record.id.clone(), id_at(SHEET_KEY)?, id_at(VIEW_KEY)?, x, y)
            .with_title(title)
            .with_align_x(align(ALIGN_X_KEY, ALIGN_X_ANCHOR_KEY)?)
            .with_align_y(align(ALIGN_Y_KEY, ALIGN_Y_ANCHOR_KEY)?);
        if viewport.to_record(&record.document) != *record {
            return Err(malformed("is not as a viewport record is written"));
        }
        Ok(viewport)
    }
}
