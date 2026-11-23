//! One text style: a name, a font by name, a size on paper, bold, italic, underline, a colour and
//! a line spacing, written as an element record of the text notes category and read back. A note
//! holds only the style's ID, so changing the style changes every note that uses it.

use crate::error::TextError;
use construct_categories::TEXT_NOTES;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every text style record, and its recipe's operation.
pub const STYLE_KIND: &str = "text-style";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The font's name, as text.
pub const FONT_KEY: &str = "@text.style.font";
/// The size on paper: the height of the font's em, a stored length above zero.
pub const SIZE_KEY: &str = "@text.style.size";
/// Whether the text is bold, in construct-params' stored yes or no text.
pub const BOLD_KEY: &str = "@text.style.bold";
/// Whether the text is italic.
pub const ITALIC_KEY: &str = "@text.style.italic";
/// Whether the text is underlined.
pub const UNDERLINE_KEY: &str = "@text.style.underline";
/// The colour, as `#rrggbb` in lower case hexadecimal.
pub const COLOUR_KEY: &str = "@text.style.colour";
/// The space from one line's top to the next as a whole percentage of the font's own line height.
pub const SPACING_KEY: &str = "@text.style.spacing";

/// The longest a style name or a font name may be, in characters.
pub const MAX_NAME_LEN: usize = 64;
/// The largest size a style may have: 500 mm on paper. The bound keeps a typo from making text no
/// sheet can hold and keeps every sum far from overflow.
pub const MAX_SIZE: Length = Length::from_mm(500);
/// The smallest and largest line spacing, in percent.
pub const SPACING_RANGE: (u16, u16) = (50, 300);
/// The line spacing of a new style, in percent.
pub const DEFAULT_SPACING: u16 = 100;

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a style record: Spare Construct's own operation `text-style`, version 1, with no
/// arguments. It builds no shape.
#[must_use]
pub fn style_recipe() -> Recipe {
    Recipe::construct(STYLE_KIND, BTreeMap::new())
}

/// A name as kept: trimmed, not empty, short, with no control character.
fn clean(text: &str, empty: &TextError, what: &str) -> Result<String, TextError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(empty.clone());
    }
    if text.chars().count() > MAX_NAME_LEN || text.chars().any(char::is_control) {
        return Err(TextError::BadName(format!(
            "{what} is at most {MAX_NAME_LEN} characters with no control characters."
        )));
    }
    Ok(text.to_owned())
}

/// A text style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextStyle {
    id: Id,
    name: String,
    font: String,
    size: Length,
    bold: bool,
    italic: bool,
    underline: bool,
    colour: [u8; 3],
    spacing: u16,
}

impl TextStyle {
    /// A style with a name, a font and a size on paper: not bold, italic or underlined, black,
    /// with the font's own line spacing.
    ///
    /// # Errors
    /// [`TextError::EmptyName`] or [`TextError::BadName`] for a name that is empty, long or holds
    /// a control character; [`TextError::BadFont`] likewise for the font; [`TextError::BadSize`]
    /// for a size that is not above zero or is over [`MAX_SIZE`].
    pub fn new(id: Id, name: &str, font: &str, size: Length) -> Result<Self, TextError> {
        let style = Self {
            id,
            name: clean(name, &TextError::EmptyName, "A text style name")?,
            font: String::new(),
            size: Length::from_steps(0),
            bold: false,
            italic: false,
            underline: false,
            colour: [0, 0, 0],
            spacing: DEFAULT_SPACING,
        };
        style.with_font(font)?.with_size(size)
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name, unique in the document without regard to case.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The font, by name.
    #[must_use]
    pub fn font(&self) -> &str {
        &self.font
    }

    /// The size on paper.
    #[must_use]
    pub fn size(&self) -> Length {
        self.size
    }

    /// Whether the text is bold.
    #[must_use]
    pub fn bold(&self) -> bool {
        self.bold
    }

    /// Whether the text is italic.
    #[must_use]
    pub fn italic(&self) -> bool {
        self.italic
    }

    /// Whether the text is underlined.
    #[must_use]
    pub fn underline(&self) -> bool {
        self.underline
    }

    /// The colour: red, green, blue.
    #[must_use]
    pub fn colour(&self) -> [u8; 3] {
        self.colour
    }

    /// The line spacing in percent of the font's own line height.
    #[must_use]
    pub fn spacing(&self) -> u16 {
        self.spacing
    }

    /// The style under another name.
    ///
    /// # Errors
    /// [`TextError::EmptyName`] or [`TextError::BadName`].
    pub fn renamed(mut self, name: &str) -> Result<Self, TextError> {
        self.name = clean(name, &TextError::EmptyName, "A text style name")?;
        Ok(self)
    }

    /// The style in another font.
    ///
    /// # Errors
    /// [`TextError::BadFont`] for a font name that is empty, long or holds a control character.
    pub fn with_font(mut self, font: &str) -> Result<Self, TextError> {
        let font = font.trim();
        if font.is_empty()
            || font.chars().count() > MAX_NAME_LEN
            || font.chars().any(char::is_control)
        {
            return Err(TextError::BadFont(format!(
                "A font name is 1 to {MAX_NAME_LEN} characters with no control characters."
            )));
        }
        font.clone_into(&mut self.font);
        Ok(self)
    }

    /// The style at another size on paper.
    ///
    /// # Errors
    /// [`TextError::BadSize`] for a size that is not above zero, or over [`MAX_SIZE`].
    pub fn with_size(mut self, size: Length) -> Result<Self, TextError> {
        if size <= Length::from_steps(0) || size > MAX_SIZE {
            return Err(TextError::BadSize(format!(
                "A text size on paper is above zero and at most {MAX_SIZE}; this is {size}."
            )));
        }
        self.size = size;
        Ok(self)
    }

    /// The style bold or not.
    #[must_use]
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// The style italic or not.
    #[must_use]
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// The style underlined or not.
    #[must_use]
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// The style in another colour.
    #[must_use]
    pub fn with_colour(mut self, colour: [u8; 3]) -> Self {
        self.colour = colour;
        self
    }

    /// The style with another line spacing, in percent.
    ///
    /// # Errors
    /// [`TextError::BadSize`] for a spacing outside [`SPACING_RANGE`].
    pub fn with_spacing(mut self, percent: u16) -> Result<Self, TextError> {
        let (low, high) = SPACING_RANGE;
        if percent < low || percent > high {
            return Err(TextError::BadSize(format!(
                "A line spacing is a whole percentage from {low} to {high}; this is {percent}."
            )));
        }
        self.spacing = percent;
        Ok(self)
    }

    /// The style as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let [red, green, blue] = self.colour;
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), TEXT_NOTES.0.to_string());
        props.insert(FONT_KEY.to_owned(), self.font.clone());
        props.insert(SIZE_KEY.to_owned(), Value::Length(self.size).stored());
        props.insert(BOLD_KEY.to_owned(), Value::YesNo(self.bold).stored());
        props.insert(ITALIC_KEY.to_owned(), Value::YesNo(self.italic).stored());
        props.insert(
            UNDERLINE_KEY.to_owned(),
            Value::YesNo(self.underline).stored(),
        );
        props.insert(
            COLOUR_KEY.to_owned(),
            format!("#{red:02x}{green:02x}{blue:02x}"),
        );
        props.insert(SPACING_KEY.to_owned(), self.spacing.to_string());
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: STYLE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: style_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a style from its element record. Anything [`TextStyle::to_record`] would not write is
    /// refused, never repaired.
    ///
    /// # Errors
    /// [`TextError::Malformed`], naming the record, for a record that is not a style record as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, TextError> {
        let malformed =
            |why: &str| TextError::Malformed(format!("The text style record {} {why}.", record.id));
        if record.kind != STYLE_KIND || record.recipe != style_recipe() {
            return Err(malformed(
                "is not of kind text-style, with the text style recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let text = |key: &str| {
            record
                .props
                .get(key)
                .ok_or_else(|| malformed(&format!("holds nothing under {key}")))
        };
        let size = crate::stored::length(record, SIZE_KEY)
            .ok_or_else(|| malformed("holds no size in its one spelling"))?;
        let flag = |key: &str| match crate::stored::value(record, key) {
            Some(Value::YesNo(on)) => Ok(on),
            _ => Err(malformed(&format!("holds no yes or no under {key}"))),
        };
        let colour = parse_colour(text(COLOUR_KEY)?)
            .ok_or_else(|| malformed("holds a colour that is not #rrggbb"))?;
        let spacing: u16 = text(SPACING_KEY)?
            .parse()
            .map_err(|_| malformed("holds a line spacing that is not a whole percentage"))?;
        let style = Self::new(record.id.clone(), &record.name, text(FONT_KEY)?, size)
            .and_then(|style| style.with_spacing(spacing))
            .map_err(|e| malformed(&e.to_string()))?
            .with_bold(flag(BOLD_KEY)?)
            .with_italic(flag(ITALIC_KEY)?)
            .with_underline(flag(UNDERLINE_KEY)?)
            .with_colour(colour);
        if style.to_record(&record.document) != *record {
            return Err(malformed("is not as a text style record is written"));
        }
        Ok(style)
    }
}

/// A colour from `#rrggbb`.
fn parse_colour(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let part = |from: usize| {
        digits
            .get(from..from.checked_add(2)?)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
    };
    Some([part(0)?, part(2)?, part(4)?])
}
