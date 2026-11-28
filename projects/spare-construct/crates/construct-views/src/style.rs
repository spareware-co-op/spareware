//! A part of a style: what an element override or a view filter says about how something looks
//! in one view (CON-A8-F3-P6, and the filters of CON-A8-F3-P4).
//!
//! A graphics rule ([`crate::RuleStyle`]) is a whole style for a category. An [`ElementStyle`] is
//! not: it names only the parts it changes (the pen, the line colour, the line pattern, halftone,
//! shown), and whatever it leaves out is what the element looks like below it. Parts are held as
//! options, so "not named" and "named as it already is" stay apart, and two parts of style stack
//! with [`ElementStyle::or`] (the first one's parts win) and land on a whole style with
//! [`ElementStyle::over`].
//!
//! Stored form (inside an override or a filter): a JSON object with the parts named, in the order
//! `pen`, `pattern`, `colour`, `halftone`, `shown`, a part never written as `null`.

use crate::error::ViewError;
use crate::graphics::{RuleStyle, pen_exists};
use construct_categories::CategoryKind;
use construct_styles::{Colour, LineStyles, PatternId};
use serde::{Deserialize, Serialize};

/// The parts of a look an override or a filter names.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ElementStyle {
    /// The line weight: a pen by number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pen: Option<u16>,
    /// The line pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternId>,
    /// The line colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    /// Drawn in a lighter tone, or explicitly not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halftone: Option<bool>,
    /// Drawn at all, or hidden in this view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown: Option<bool>,
}

impl ElementStyle {
    /// Whether it names no part at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// This style with the parts it leaves out taken from `later`: where both name a part, this
    /// one wins.
    #[must_use]
    pub fn or(&self, later: &Self) -> Self {
        Self {
            pen: self.pen.or(later.pen),
            pattern: self.pattern.or(later.pattern),
            colour: self.colour.or(later.colour),
            halftone: self.halftone.or(later.halftone),
            shown: self.shown.or(later.shown),
        }
    }

    /// `base` with the parts this style names put over it.
    #[must_use]
    pub fn over(&self, base: RuleStyle) -> RuleStyle {
        let mut style = base;
        if let Some(pen) = self.pen {
            style.pen = pen;
        }
        if let Some(pattern) = self.pattern {
            style.pattern = pattern;
        }
        if let Some(colour) = self.colour {
            style.colour = colour;
        }
        if let Some(halftone) = self.halftone {
            style.halftone = halftone;
        }
        if let Some(shown) = self.shown {
            style.shown = shown;
        }
        style
    }

    /// Asks whether the parts it names exist, for an element of a category of `kind`: the pen in
    /// the pen table that kind uses and the line pattern in the project's.
    ///
    /// # Errors
    /// [`ViewError::NoSuchPen`] or [`ViewError::NoSuchPattern`].
    pub fn check(&self, lines: &LineStyles, kind: CategoryKind) -> Result<(), ViewError> {
        if let Some(pen) = self.pen
            && !pen_exists(lines, kind, pen)
        {
            return Err(ViewError::NoSuchPen { pen });
        }
        if let Some(pattern) = self.pattern
            && lines.patterns.get(pattern).is_none()
        {
            return Err(ViewError::NoSuchPattern { pattern: pattern.0 });
        }
        Ok(())
    }
}
