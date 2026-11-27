//! The text a dimension shows: its value in the person's units, and where it is centred.
//!
//! The words are construct-params' own: [`Definition::format`] of a length with the person's
//! [`Units`] (`2400 mm`, `2.4 m`), so a dimension reads as every other length in the program.
//! Drawing the text (its font, its size at the view's scale, avoiding the witness lines) waits for
//! the text of job 7.4 (piece A78, not on main yet): what the drawing needs from it is behind the
//! trait [`TextSize`], which says how big a piece of text is. Interim: until A78's text can answer,
//! [`Unmeasured`] is what a caller passes, and every label's `fits` is `None` (not known). The
//! piece that replaces it implements [`TextSize`] over the text engine and changes no caller.

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

/// The text of one segment and where it is centred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// The segment, from 0.
    pub segment: usize,
    /// The value in the person's units.
    pub text: String,
    /// The middle of the segment on the offset line.
    pub at: Point,
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
        let fits = text.size(&shown).map(|(width, _)| width <= value);
        found.push(Label {
            segment: segment.from,
            text: shown,
            at,
            fits,
        });
    }
    Ok(found)
}
