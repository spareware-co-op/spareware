//! What a door or window is asked for: a spec, one row of a schedule or a supplier's catalogue.
//!
//! A spec names its template and type mark, its size (overall, the outside of the frame, or clear,
//! the opening a person passes through or sees through), and, where its template has them, the
//! frame, leaf, sash and glass sizes, its hand, its sill height, its frame and leaf materials and
//! the thickness of the wall it is for. Anything left unset reads the template's own default.
//! Every length is a `construct_params::Length` (grid steps); nothing is millimetres or a double.

use crate::template::Template;
use construct_params::Length;
use kit_core::Id;

/// How a spec gives its size: width first, then height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// The outside of the frame: the coordinating size the wall opening is set out to.
    Overall {
        /// The overall width.
        width: Length,
        /// The overall height.
        height: Length,
    },
    /// The clear opening: for a swing door, from the face of the leaf open at a right angle to
    /// the far jamb, and from the floor to the head; for a fixed window, the glass seen inside
    /// the frame; for a sliding window, what the sliding sash leaves open when slid fully across.
    Clear {
        /// The clear width.
        width: Length,
        /// The clear height.
        height: Length,
    },
}

/// The hand of a unit, seen in the front elevation (from the side a door's leaf opens away
/// from): a door's hinge side, a sliding window's sliding sash side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hand {
    /// On the left.
    Left,
    /// On the right.
    Right,
}

impl Hand {
    /// The hand's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// One door or window asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The type mark a tag shows and a schedule sorts by (`D01`, `W-12`).
    pub mark: String,
    /// The template it is made from.
    pub template: Template,
    /// Its size.
    pub size: Size,
    /// The frame's width on its face (jambs and head; a window's sill too).
    pub frame: Option<Length>,
    /// The frame's depth, across the wall.
    pub frame_depth: Option<Length>,
    /// A door leaf's thickness.
    pub leaf: Option<Length>,
    /// A sliding sash's width on its face.
    pub sash: Option<Length>,
    /// A sliding sash's thickness.
    pub sash_depth: Option<Length>,
    /// The gap between a sliding window's two tracks.
    pub track_gap: Option<Length>,
    /// The glass's thickness.
    pub glass: Option<Length>,
    /// The hand: a door's hinge side, a sliding window's sliding sash side.
    pub hand: Option<Hand>,
    /// A window's sill height above the floor, where a placed window starts.
    pub sill_height: Option<Length>,
    /// The frame's material.
    pub frame_material: Option<Id>,
    /// A door leaf's or a sash's material.
    pub leaf_material: Option<Id>,
    /// The thickness of the wall it is for; its depth must fit.
    pub wall: Option<Length>,
}

impl Spec {
    /// A spec of this mark, template and size, everything else the template's default.
    #[must_use]
    pub fn new(mark: impl Into<String>, template: Template, size: Size) -> Self {
        Self {
            mark: mark.into(),
            template,
            size,
            frame: None,
            frame_depth: None,
            leaf: None,
            sash: None,
            sash_depth: None,
            track_gap: None,
            glass: None,
            hand: None,
            sill_height: None,
            frame_material: None,
            leaf_material: None,
            wall: None,
        }
    }
}
