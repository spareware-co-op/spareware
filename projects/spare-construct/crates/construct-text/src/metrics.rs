//! Font metrics: what the layout needs to know about a font and cannot work out itself. The caller
//! supplies them (the page measures with the browser's own fonts; a server with font files reads
//! them), so that no number here is a guess. Widths are in thousandths of the font's size (an em),
//! whole numbers, so the layout stays exact integer arithmetic.
//!
//! The shared text engine (`engine-text`) has no code yet (its own header says so). When it has
//! font loading and shaping, one implementation of [`FontMetrics`] over it replaces whatever a
//! caller supplies today, and nothing else here changes.

use crate::error::TextError;

/// A font as a style names it: the name and the weight and slant asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face<'a> {
    /// The font's name.
    pub font: &'a str,
    /// Whether bold is asked for.
    pub bold: bool,
    /// Whether italic is asked for.
    pub italic: bool,
}

/// What a font measures. Implemented by the caller, over real fonts.
pub trait FontMetrics {
    /// How wide `text` is when set on one line in `face`, in thousandths of the size.
    ///
    /// # Errors
    /// [`TextError::Metrics`] when the font is not known to the caller.
    fn advance(&self, face: &Face<'_>, text: &str) -> Result<u64, TextError>;

    /// The font's own line height (ascent, descent and gap), in thousandths of the size.
    ///
    /// # Errors
    /// [`TextError::Metrics`] when the font is not known to the caller.
    fn line_height(&self, face: &Face<'_>) -> Result<u64, TextError>;
}

/// Metrics for tests: every character is `advance` thousandths of the size wide, and a line is
/// `line_height` thousandths high, in every font. It is not a font and no page or server uses it
/// for anything a person sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedWidth {
    /// Thousandths of the size each character advances.
    pub advance: u64,
    /// Thousandths of the size a line is high.
    pub line_height: u64,
}

impl FontMetrics for FixedWidth {
    fn advance(&self, _: &Face<'_>, text: &str) -> Result<u64, TextError> {
        u64::try_from(text.chars().count())
            .ok()
            .and_then(|count| count.checked_mul(self.advance))
            .ok_or_else(|| TextError::Metrics("text too long to measure".to_owned()))
    }

    fn line_height(&self, _: &Face<'_>) -> Result<u64, TextError> {
        Ok(self.line_height)
    }
}
