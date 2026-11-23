//! The templates a spec fills: the library's door and window recipes (`library/doors-windows`,
//! piece A62), baked in, and what this crate knows of each: the range of overall sizes it is
//! generated across, which spec fields it has, and which parameters it halves.
//!
//! The ranges are ours (note CON-A7-F4-P1-door-window-generator): a single leaf from 600 to
//! 1,200 mm wide (wider is a pair of leaves) and 1,800 to 3,000 mm high; a fixed light from 300 to
//! 3,000 mm each way; a sliding window from 900 to 3,600 mm wide and 600 to 2,400 mm high. Every
//! other rule follows from the template's own shape (crate `rules`).

use crate::refusal::{Refusal, Rule};
use construct_families::Recipe;
use construct_params::Length;

/// A template a spec fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Template {
    /// A side-hung door: frame (jambs and head) and one leaf, hinged left or right.
    SingleSwingDoor,
    /// A fixed light: frame and glass.
    FixedWindow,
    /// Two sashes on two tracks, the left fixed, the right sliding.
    SlidingWindow,
}

/// A range of lengths, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    /// The least.
    pub low: Length,
    /// The most.
    pub high: Length,
}

impl Range {
    const fn mm(low: i32, high: i32) -> Self {
        Self {
            low: Length::from_mm(low),
            high: Length::from_mm(high),
        }
    }

    /// Whether `length` is in the range.
    #[must_use]
    pub fn holds(self, length: Length) -> bool {
        self.low <= length && length <= self.high
    }
}

/// A length a spec may set, by the name it has in a spec and as its template's parameter key
/// (they are the same words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    /// The frame's width on its face.
    Frame,
    /// The frame's depth.
    FrameDepth,
    /// A door leaf's thickness.
    Leaf,
    /// A sash's width on its face.
    Sash,
    /// A sash's thickness.
    SashDepth,
    /// The gap between two tracks.
    TrackGap,
    /// The glass's thickness.
    Glass,
}

impl Part {
    /// The spec field's name, which is also the template parameter's key.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::FrameDepth => "frame_depth",
            Self::Leaf => "leaf",
            Self::Sash => "sash",
            Self::SashDepth => "sash_depth",
            Self::TrackGap => "track_gap",
            Self::Glass => "glass",
        }
    }
}

impl Template {
    /// Every template.
    pub const ALL: [Self; 3] = [
        Self::SingleSwingDoor,
        Self::FixedWindow,
        Self::SlidingWindow,
    ];

    /// The template's file stem in the library (`single-swing-door`).
    #[must_use]
    pub fn stem(self) -> &'static str {
        match self {
            Self::SingleSwingDoor => "single-swing-door",
            Self::FixedWindow => "fixed-window",
            Self::SlidingWindow => "sliding-window",
        }
    }

    /// The template's recipe text, as the library holds it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::SingleSwingDoor => {
                include_str!("../../../library/doors-windows/single-swing-door.json")
            }
            Self::FixedWindow => include_str!("../../../library/doors-windows/fixed-window.json"),
            Self::SlidingWindow => {
                include_str!("../../../library/doors-windows/sliding-window.json")
            }
        }
    }

    /// The template's recipe.
    ///
    /// # Errors
    /// [`Rule::Template`] if the baked text does not read (a fault of this build, never of a
    /// spec; the tests hold every template to reading).
    pub fn recipe(self) -> Result<Recipe, Refusal> {
        Recipe::from_json(self.text())
            .map_err(|e| Refusal::new(Rule::Template, self.stem(), e.to_string()))
    }

    /// The overall widths it is generated across.
    #[must_use]
    pub fn widths(self) -> Range {
        match self {
            Self::SingleSwingDoor => Range::mm(600, 1200),
            Self::FixedWindow => Range::mm(300, 3000),
            Self::SlidingWindow => Range::mm(900, 3600),
        }
    }

    /// The overall heights it is generated across.
    #[must_use]
    pub fn heights(self) -> Range {
        match self {
            Self::SingleSwingDoor => Range::mm(1800, 3000),
            Self::FixedWindow => Range::mm(300, 3000),
            Self::SlidingWindow => Range::mm(600, 2400),
        }
    }

    /// The lengths a spec may set, in the template's parameter order.
    #[must_use]
    pub fn parts(self) -> &'static [Part] {
        match self {
            Self::SingleSwingDoor => &[Part::Frame, Part::FrameDepth, Part::Leaf],
            Self::FixedWindow => &[Part::Frame, Part::FrameDepth, Part::Glass],
            Self::SlidingWindow => &[
                Part::Frame,
                Part::FrameDepth,
                Part::Sash,
                Part::SashDepth,
                Part::TrackGap,
                Part::Glass,
            ],
        }
    }

    /// The template's material parameter a spec's leaf material sets, if it has a leaf or sash.
    #[must_use]
    pub fn leaf_material(self) -> Option<&'static str> {
        match self {
            Self::SingleSwingDoor => Some("leaf_material"),
            Self::FixedWindow => None,
            Self::SlidingWindow => Some("sash_material"),
        }
    }

    /// Whether a placed unit stands on a sill height (a window) or on the floor (a door).
    #[must_use]
    pub fn has_sill(self) -> bool {
        !matches!(self, Self::SingleSwingDoor)
    }

    /// Whether it has glass, so a glass area tag.
    #[must_use]
    pub fn glazed(self) -> bool {
        !matches!(self, Self::SingleSwingDoor)
    }

    /// Whether it has a leaf or sashes, so a leaf area tag.
    #[must_use]
    pub fn has_leaf(self) -> bool {
        !matches!(self, Self::FixedWindow)
    }

    /// The hands it draws: a door either (its instance choice `swing`), a sliding window its
    /// sliding sash on the right only (the other hand is the placed window mirrored), a fixed
    /// light none.
    #[must_use]
    pub fn hands(self) -> &'static [crate::Hand] {
        match self {
            Self::SingleSwingDoor => &[crate::Hand::Left, crate::Hand::Right],
            Self::FixedWindow => &[],
            Self::SlidingWindow => &[crate::Hand::Right],
        }
    }
}
