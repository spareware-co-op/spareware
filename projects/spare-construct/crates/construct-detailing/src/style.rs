//! Line styles for detail items: a pen of construct-styles' model pens (so it prints thinner at a
//! small scale as every model line does), a line pattern and a colour. Six are built in, by name;
//! a person's own are element records of this document (`detail.linestyle`), each with its own ID.
//!
//! The six are our own set, named for what they are for: `fine`, `standard`, `bold`, `centre`,
//! `hidden` and `dotted`. A line, or a region's boundary, names its style by [`StyleRef`]: stored
//! as `builtin:<name>` or as the ID of an own style. A style is never copied into the item that
//! uses it, so changing a style changes every line drawn with it.

use crate::error::DetailError;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_styles::patterns::{DASH_DOT, DOTTED, HIDDEN, SOLID};
use construct_styles::{Colour, LineStyles, Micrometres, PatternId, Scale};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of a person's own line style record, and its recipe's operation.
pub const STYLE_KIND: &str = "detail.linestyle";
/// The pen's number, in plain decimal.
pub const PEN_KEY: &str = "@detail.style.pen";
/// The pattern's ID, in plain decimal.
pub const PATTERN_KEY: &str = "@detail.style.pattern";
/// The colour, `#rrggbb` in lower case.
pub const COLOUR_KEY: &str = "@detail.style.colour";
/// The record's category: detail lines.
pub const CATEGORY_KEY: &str = "@category";
/// The longest name a style has, in characters.
pub const MAX_NAME: usize = 64;

const BUILTIN_PREFIX: &str = "builtin:";
const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// A built-in line style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuiltIn {
    /// Pen 2, solid.
    Fine,
    /// Pen 4, solid: the style a new line has.
    Standard,
    /// Pen 6, solid.
    Bold,
    /// Pen 3, the centre line's dash and dot.
    Centre,
    /// Pen 3, the hidden-edge dashes.
    Hidden,
    /// Pen 2, dots.
    Dotted,
}

impl BuiltIn {
    /// Every built-in style, in the order a person is shown them.
    pub const ALL: [BuiltIn; 6] = [
        BuiltIn::Fine,
        BuiltIn::Standard,
        BuiltIn::Bold,
        BuiltIn::Centre,
        BuiltIn::Hidden,
        BuiltIn::Dotted,
    ];

    /// Its name, which is also its key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Fine => "fine",
            Self::Standard => "standard",
            Self::Bold => "bold",
            Self::Centre => "centre",
            Self::Hidden => "hidden",
            Self::Dotted => "dotted",
        }
    }

    /// The built-in style of this key.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.key() == key)
    }

    /// What it is made of.
    #[must_use]
    pub const fn style(self) -> LineStyle {
        let black = Colour(0, 0, 0);
        match self {
            Self::Fine => LineStyle::new(2, SOLID, black),
            Self::Standard => LineStyle::new(4, SOLID, black),
            Self::Bold => LineStyle::new(6, SOLID, black),
            Self::Centre => LineStyle::new(3, DASH_DOT, black),
            Self::Hidden => LineStyle::new(3, HIDDEN, black),
            Self::Dotted => LineStyle::new(2, DOTTED, black),
        }
    }
}

/// What a line style is made of: a model pen by number, a line pattern, a colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineStyle {
    /// A number of construct-styles' model pens.
    pub pen: u16,
    /// A pattern of construct-styles' line patterns.
    pub pattern: PatternId,
    /// The colour it prints in.
    pub colour: Colour,
}

impl LineStyle {
    /// A style of these parts, not yet checked against a project's pens and patterns.
    #[must_use]
    pub const fn new(pen: u16, pattern: PatternId, colour: Colour) -> Self {
        Self {
            pen,
            pattern,
            colour,
        }
    }

    /// Whether the pen and the pattern are in `styles`.
    ///
    /// # Errors
    /// [`DetailError::BadStyle`] naming what is missing.
    pub fn check(self, styles: &LineStyles) -> Result<(), DetailError> {
        if styles.model_pens.pen(self.pen).is_none() {
            return Err(DetailError::BadStyle(format!(
                "There is no model pen {}.",
                self.pen
            )));
        }
        if !styles
            .patterns
            .patterns()
            .iter()
            .any(|p| p.id == self.pattern)
        {
            return Err(DetailError::BadStyle(format!(
                "There is no line pattern {}.",
                self.pattern.0
            )));
        }
        Ok(())
    }
}

/// How an item names its line style.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StyleRef {
    /// One of the six built in.
    Built(BuiltIn),
    /// A person's own style, by its ID.
    Own(Id),
}

impl StyleRef {
    /// The one stored spelling: `builtin:standard`, or the own style's ID.
    #[must_use]
    pub fn stored(&self) -> String {
        match self {
            Self::Built(b) => format!("{BUILTIN_PREFIX}{}", b.key()),
            Self::Own(id) => id.to_string(),
        }
    }

    /// Reads the stored spelling.
    ///
    /// # Errors
    /// [`DetailError::UnknownStyle`] for a built-in name there is not, or text that is no ID.
    pub fn from_stored(text: &str) -> Result<Self, DetailError> {
        if let Some(key) = text.strip_prefix(BUILTIN_PREFIX) {
            return BuiltIn::from_key(key)
                .map(Self::Built)
                .ok_or_else(|| DetailError::UnknownStyle(text.to_owned()));
        }
        Id::parse(text)
            .map(Self::Own)
            .map_err(|_| DetailError::UnknownStyle(text.to_owned()))
    }

    /// In words: `the built-in style standard`, `the style S-1`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Built(b) => format!("the built-in style {}", b.key()),
            Self::Own(id) => format!("the style {id}"),
        }
    }
}

/// How a line is printed at one scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Printed {
    /// The pen's printed width at the scale.
    pub width: Micrometres,
    /// The colour.
    pub colour: Colour,
    /// The pattern.
    pub pattern: PatternId,
}

/// A person's own line style: its ID, its name and what it is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnStyle {
    id: Id,
    name: String,
    style: LineStyle,
}

fn colour_text(c: Colour) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// Reads `#rrggbb`, lower case only (its one spelling).
///
/// # Errors
/// [`DetailError::BadStyle`] for anything else.
pub fn colour_of(text: &str) -> Result<Colour, DetailError> {
    let bad = || DetailError::BadStyle(format!("{text:?} is not a colour written #rrggbb."));
    let digits = text.strip_prefix('#').ok_or_else(bad)?;
    if digits.len() != 6
        || !digits
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(bad());
    }
    let part = |from: usize| -> Result<u8, DetailError> {
        digits
            .get(from..from.saturating_add(2))
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .ok_or_else(bad)
    };
    Ok(Colour(part(0)?, part(2)?, part(4)?))
}

/// A colour in its one spelling.
#[must_use]
pub fn colour_spelled(c: Colour) -> String {
    colour_text(c)
}

impl OwnStyle {
    /// An own style.
    ///
    /// # Errors
    /// [`DetailError::BadStyle`] for an empty or over-long name, or a name a built-in style has.
    pub fn new(id: Id, name: &str, style: LineStyle) -> Result<Self, DetailError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(DetailError::BadStyle(format!(
                "A style's name is 1 to {MAX_NAME} characters."
            )));
        }
        if BuiltIn::from_key(&name.to_lowercase()).is_some() {
            return Err(DetailError::BadStyle(format!(
                "{name:?} is the name of a built-in style."
            )));
        }
        Ok(Self {
            id,
            name: name.to_owned(),
            style,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What it is made of.
    #[must_use]
    pub fn style(&self) -> LineStyle {
        self.style
    }

    /// The style with another name, pen, pattern or colour.
    ///
    /// # Errors
    /// As [`OwnStyle::new`].
    pub fn with(self, name: &str, style: LineStyle) -> Result<Self, DetailError> {
        Self::new(self.id, name, style)
    }

    /// The style as an element record of `document`.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(
            CATEGORY_KEY.to_owned(),
            construct_categories::DETAIL_LINES.0.to_string(),
        );
        props.insert(PEN_KEY.to_owned(), self.style.pen.to_string());
        props.insert(PATTERN_KEY.to_owned(), self.style.pattern.0.to_string());
        props.insert(COLOUR_KEY.to_owned(), colour_text(self.style.colour));
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: STYLE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: Recipe::construct(STYLE_KIND, BTreeMap::new()),
            props,
            references: Vec::new(),
        }
    }

    /// Reads an own style from its record; anything [`OwnStyle::to_record`] would not write is
    /// refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, DetailError> {
        let bad = |why: &str| {
            DetailError::Malformed(format!("The line style record {} {why}.", record.id))
        };
        let number = |key: &str| -> Result<u32, DetailError> {
            record
                .props
                .get(key)
                .and_then(|t| t.parse::<u32>().ok())
                .ok_or_else(|| bad(&format!("holds no {key}")))
        };
        let pen =
            u16::try_from(number(PEN_KEY)?).map_err(|_| bad("holds a pen number too large"))?;
        let colour = colour_of(
            record
                .props
                .get(COLOUR_KEY)
                .ok_or_else(|| bad("holds no colour"))?,
        )
        .map_err(|e| bad(&e.to_string()))?;
        let own = Self::new(
            record.id.clone(),
            &record.name,
            LineStyle::new(pen, PatternId(number(PATTERN_KEY)?), colour),
        )
        .map_err(|e| bad(&e.to_string()))?;
        if own.to_record(&record.document) != *record {
            return Err(bad("is not as a line style record is written"));
        }
        Ok(own)
    }
}

/// A document's own line styles, by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnStyles {
    all: BTreeMap<Id, OwnStyle>,
}

impl OwnStyles {
    /// None.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The own styles among `records`.
    ///
    /// # Errors
    /// [`DetailError::Malformed`] for a style record that is not as written, or two styles with
    /// one name (case aside).
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, DetailError> {
        let mut found = Self::new();
        for record in records {
            if record.kind == STYLE_KIND {
                found.add(OwnStyle::from_record(record)?)?;
            }
        }
        Ok(found)
    }

    /// The style with this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&OwnStyle> {
        self.all.get(id)
    }

    /// Every style, by ID.
    #[must_use]
    pub fn all(&self) -> Vec<&OwnStyle> {
        self.all.values().collect()
    }

    /// Whether another style already has this name, case aside.
    #[must_use]
    pub fn name_taken(&self, name: &str, except: Option<&Id>) -> bool {
        let name = name.trim().to_lowercase();
        self.all
            .values()
            .any(|s| Some(&s.id) != except && s.name.to_lowercase() == name)
    }

    /// Adds a style.
    ///
    /// # Errors
    /// [`DetailError::Duplicate`] for a taken ID, [`DetailError::BadStyle`] for a taken name.
    pub fn add(&mut self, style: OwnStyle) -> Result<(), DetailError> {
        if self.all.contains_key(&style.id) {
            return Err(DetailError::Duplicate(style.id));
        }
        if self.name_taken(&style.name, None) {
            return Err(DetailError::BadStyle(format!(
                "There is already a line style named {:?}.",
                style.name
            )));
        }
        self.all.insert(style.id.clone(), style);
        Ok(())
    }

    /// The style a reference names, as what it is made of.
    ///
    /// # Errors
    /// [`DetailError::UnknownStyle`] for an own style that is not there.
    pub fn resolve(&self, reference: &StyleRef) -> Result<LineStyle, DetailError> {
        match reference {
            StyleRef::Built(b) => Ok(b.style()),
            StyleRef::Own(id) => self
                .all
                .get(id)
                .map(OwnStyle::style)
                .ok_or_else(|| DetailError::UnknownStyle(id.to_string())),
        }
    }

    /// How the style a reference names prints at `scale`.
    ///
    /// # Errors
    /// [`DetailError::UnknownStyle`], or [`DetailError::BadStyle`] when `styles` has no such pen.
    pub fn printed(
        &self,
        reference: &StyleRef,
        styles: &LineStyles,
        scale: Scale,
    ) -> Result<Printed, DetailError> {
        let made = self.resolve(reference)?;
        let width = styles
            .model_pens
            .width(made.pen, scale)
            .map_err(|e| DetailError::BadStyle(format!("{e:?}")))?;
        Ok(Printed {
            width,
            colour: made.colour,
            pattern: made.pattern,
        })
    }
}
