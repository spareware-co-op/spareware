//! The text a dimension shows: its value in the person's units, and where it is centred.
//!
//! The words are construct-params' own: [`Definition::format`] of a length with the person's
//! [`Units`] (`2400 mm`, `2.4 m`), so a dimension reads as every other length in the program.
//! Drawing the text (its font, its size at the view's scale, avoiding the witness lines) waits for
//! the text of job 7.4 (piece A78, not on main yet): what the drawing needs from it is behind the
//! trait [`TextSize`], which says how big a piece of text is. Interim: until A78's text can answer,
//! [`PlainText`] is what the plan drawing passes: one default height and a plain width for each
//! character; [`Unmeasured`] says nothing is known (every label's `fits` is then `None`). The piece
//! that replaces them implements [`TextSize`] over the text engine and changes no caller.

use crate::error::DimensionError;
use crate::measure::Measured;
use construct_grids::Point;
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use kit_core::Id;

/// A length as a person reads it, in their units: `2400 mm`.
///
/// # Errors
/// [`DimensionError::Unshowable`] for a length the units cannot show.
pub fn display(value: Length, units: &Units) -> Result<String, DimensionError> {
    let id = Id::parse("dimension.value").map_err(|e| DimensionError::Unshowable(e.to_string()))?;
    let definition = Definition::new(id, "Dimension value", Kind::Length, Holder::Instance)
        .map_err(|e| DimensionError::Unshowable(e.to_string()))?;
    definition
        .format(&Value::Length(value), units)
        .map_err(|e| DimensionError::Unshowable(e.to_string()))
}

/// How big a piece of text is when drawn: what the page's text (job 7.4) will answer.
pub trait TextSize {
    /// The width and height of `text` on paper's scale in the view, or `None` when not known.
    fn size(&self, text: &str) -> Option<(Length, Length)>;
}

/// Interim: no text engine yet (piece A78 replaces it): every size is unknown.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unmeasured;

impl TextSize for Unmeasured {
    fn size(&self, _text: &str) -> Option<(Length, Length)> {
        None
    }
}

/// Interim: the plan's plain default text, until the text of job 7.4 (piece A78) answers in its place
/// behind [`TextSize`]: every piece of text is one fixed height (`DEFAULT_HEIGHT_MM`, as drawn at
/// 1:100) and each character is three fifths of that wide. It is a size to lay a label out by, not
/// a measure of any font.
#[derive(Clone, Copy, Debug)]
pub struct PlainText {
    height: Length,
}

impl PlainText {
    /// The default height of dimension text, in millimetres, in a view at 1:100.
    pub const DEFAULT_HEIGHT_MM: i32 = 250;

    /// The default text.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            height: Length::from_mm(Self::DEFAULT_HEIGHT_MM),
        }
    }
}

impl Default for PlainText {
    fn default() -> Self {
        Self::standard()
    }
}

impl TextSize for PlainText {
    fn size(&self, text: &str) -> Option<(Length, Length)> {
        let characters = i64::try_from(text.chars().count()).ok()?;
        let width = self
            .height
            .steps()
            .checked_mul(3)?
            .checked_mul(characters)?
            .checked_div(5)?;
        Some((Length::from_steps(width), self.height))
    }
}

/// The text of one segment and where it is centred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// The segment, from 0.
    pub segment: usize,
    /// The value in the person's units.
    pub text: String,
    /// The middle of the segment on the offset line.
    pub at: Point,
    /// How wide and how high the text is, when its size is known.
    pub size: Option<(Length, Length)>,
    /// Whether the text is no wider than the segment, when its size is known.
    pub fits: Option<bool>,
}

/// A label for each segment that has a value, in order.
///
/// # Errors
/// [`DimensionError::Unshowable`] for a value the units cannot show.
pub fn labels(
    measured: &Measured,
    units: &Units,
    text: &dyn TextSize,
) -> Result<Vec<Label>, DimensionError> {
    let mut found = Vec::new();
    for segment in &measured.segments {
        let (Some(value), Some(at)) = (segment.value, segment.middle) else {
            continue;
        };
        let shown = display(value, units)?;
        let size = text.size(&shown);
        let fits = size.map(|(width, _)| width <= value);
        found.push(Label {
            segment: segment.from,
            text: shown,
            at,
            size,
            fits,
        });
    }
    Ok(found)
}
