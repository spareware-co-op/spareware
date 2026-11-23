//! How a view says an element looks, as the drawing asks for it.
//!
//! What it does: a [`Looks`] answers, for a category and an edge (cut, or seen in projection), the
//! whole look of it in one view: whether it is drawn, its pen's printed width, colour and pattern,
//! the fill of a cut, halftone and transparency. [`crate::pens::Styles`] carries an optional
//! `&dyn Looks`; with none, the drawing is as it was before views had rules (the project's object
//! styles by category). The drawing never works a look out a second way: the caller of the drawing
//! (the server) answers [`Looks::look`] by asking construct-views' `resolve`, so the one rule
//! stays in one place, and this crate stays below construct-views.
//!
//! Printing: a look's halftone and transparency become one opacity, in whole percent
//! ([`opacity`]): halftone is [`HALFTONE_PERCENT`] of full, and transparency takes its percentage
//! off. A full opacity writes nothing, so a view with no rules writes the bytes it always did.

use std::fmt::Debug;

use construct_categories::CategoryId;
use construct_styles::{Colour, EdgeStyle};

use crate::drawing::LineKind;

/// The share of full strength a halftone line or fill is printed at, in whole percent.
pub const HALFTONE_PERCENT: u8 = 50;

/// A fill a view gives a cut category: a fill pattern by name and the colour its lines print in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillLook {
    /// The pattern's name, as [`crate::hatch::FillPatterns`] holds it.
    pub pattern: String,
    /// The colour of its lines.
    pub colour: Colour,
}

/// The whole look of one edge of one category in one view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Look {
    /// Whether it is drawn at all.
    pub shown: bool,
    /// Its printed width, colour and pattern.
    pub edge: EdgeStyle,
    /// The fill of a cut region; `None` leaves it to the region's material.
    pub fill: Option<FillLook>,
    /// Drawn in a lighter tone.
    pub halftone: bool,
    /// How much of what lies behind shows through, in whole percent (0 is solid).
    pub transparency: u8,
}

/// What answers for a view: the look of a category on an edge.
pub trait Looks: Debug + Send + Sync {
    /// The look of `category` seen as `kind` says (a cut edge, or in projection).
    ///
    /// # Errors
    ///
    /// In words, when the view cannot say (an unknown category, a cut of what cannot be cut).
    fn look(&self, category: CategoryId, kind: LineKind) -> Result<Look, String>;
}

/// The opacity a look prints at, in whole percent: 100 is full strength.
#[must_use]
pub fn opacity(look: &Look) -> u8 {
    let solid = 100u16.saturating_sub(u16::from(look.transparency.min(100)));
    let tone = if look.halftone {
        u16::from(HALFTONE_PERCENT)
    } else {
        100
    };
    // Whole percent: the product over 100, rounded down; at most 100 by construction.
    u8::try_from(solid.saturating_mul(tone).checked_div(100).unwrap_or(0)).unwrap_or(100)
}
