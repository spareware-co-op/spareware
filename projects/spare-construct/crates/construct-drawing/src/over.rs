//! What is drawn over a view's model: the view's own detail items.
//!
//! A [`Picture`](crate::picture::Picture) is the model as the view cuts and sees it. [`Over`] is
//! laid on top of it, in the same view coordinates and at the same scale, and holds what belongs to
//! the view alone: solid fills (a masking region hides what is under it; a region with a colour
//! and no pattern), hatches of regions that are not a model material's cut, and lines. They are
//! drawn in that order, after everything else, so a masking region hides the model and a detail
//! line shows over it.
//!
//! This crate only draws them: which items there are, their styles and their places are the
//! caller's (construct-detailing works them out and hands over [`Over`]).

use crate::drawing::ViewPoint;
use crate::picture::Stroke;
use construct_styles::{Colour, Micrometres};
use kit_core::Id;

/// A solid fill: the closed outline's inside in one colour, opaque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverFill {
    /// The corners, anticlockwise, the last joined to the first.
    pub outline: Vec<ViewPoint>,
    /// The colour it fills with.
    pub colour: Colour,
    /// The item it belongs to.
    pub element: Id,
}

/// The pattern lines of a filled region, already clipped to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverHatch {
    /// The lines, each from its first point to its second.
    pub lines: Vec<[ViewPoint; 2]>,
    /// The printed width of every line.
    pub width: Micrometres,
    /// Their colour.
    pub colour: Colour,
    /// The item it belongs to.
    pub element: Id,
}

/// What is drawn over a picture: fills, then hatches, then strokes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Over {
    /// Solid fills, drawn first.
    pub fills: Vec<OverFill>,
    /// Pattern lines, drawn over the fills.
    pub hatches: Vec<OverHatch>,
    /// Lines, drawn over the hatches.
    pub strokes: Vec<Stroke>,
}

impl Over {
    /// Nothing over the picture.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether there is nothing in it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fills.is_empty() && self.hatches.is_empty() && self.strokes.is_empty()
    }
}
