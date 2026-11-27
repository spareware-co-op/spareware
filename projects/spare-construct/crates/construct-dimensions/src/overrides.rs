//! A dimension's own words in place of, or around, the value it shows: a text that replaces it, a
//! prefix before it, a suffix after it. The true value is never touched: it is worked out every
//! time as before, kept beside the shown text, and shown when asked ([`Overrides::apply`] is the
//! one rule for what is shown).
//!
//! An override is the dimension's: its prefix and suffix are put on every value it shows (each
//! segment of a string, each value of an ordinate dimension), and a replacing text is shown in
//! place of every value. A segment of a string worded on its own is a dimension of its own.

use crate::error::DimensionError;

/// The most characters an override's text, prefix or suffix holds.
pub const MAX_OVERRIDE: usize = 120;

/// A dimension's overrides of its shown text. Each part is non-empty, without a space or a line
/// break at either end and without a control character, at most [`MAX_OVERRIDE`] characters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Overrides {
    text: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
}

fn checked(what: &str, part: Option<String>) -> Result<Option<String>, DimensionError> {
    let Some(part) = part else {
        return Ok(None);
    };
    let bad = |why: &str| {
        Err(DimensionError::BadOverride(format!(
            "A dimension's {what} {why}."
        )))
    };
    if part.is_empty() {
        return bad("is not empty: leave it out to have none");
    }
    if part.trim() != part {
        return bad("has no space at either end");
    }
    if part.chars().any(char::is_control) {
        return bad("is one line of text, with no control characters");
    }
    if part.chars().count() > MAX_OVERRIDE {
        return bad(&format!("is at most {MAX_OVERRIDE} characters"));
    }
    Ok(Some(part))
}

impl Overrides {
    /// No overrides: the value is shown as it is.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Overrides: a text in place of the value, a prefix and a suffix, each optional.
    ///
    /// # Errors
    /// [`DimensionError::BadOverride`] for a part that is empty, has a space at an end, holds a
    /// control character or is too long.
    pub fn new(
        text: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
    ) -> Result<Self, DimensionError> {
        Ok(Self {
            text: checked("text", text)?,
            prefix: checked("prefix", prefix)?,
            suffix: checked("suffix", suffix)?,
        })
    }

    /// The text shown in place of the value, if any.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The prefix, if any.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }

    /// The suffix, if any.
    #[must_use]
    pub fn suffix(&self) -> Option<&str> {
        self.suffix.as_deref()
    }

    /// Whether there is none.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.text.is_none() && self.prefix.is_none() && self.suffix.is_none()
    }

    /// What is shown for a value whose own text is `value`: the replacing text or the value, with
    /// the prefix before it and the suffix after it, a space between each.
    #[must_use]
    pub fn apply(&self, value: &str) -> String {
        let middle = self.text.as_deref().unwrap_or(value);
        [self.prefix.as_deref(), Some(middle), self.suffix.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }
}
