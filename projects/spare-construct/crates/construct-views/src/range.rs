//! A plan's view range: four heights, each a level and an offset from it, and how they resolve.
//!
//! A plan is a horizontal section of the building at the cut plane, looking down (a floor plan)
//! or up (a ceiling plan). The range says how much of the building the view reaches. It is held
//! as four ties, as every height of Spare Construct is (construct-levels' [`Tie`]): moving a level
//! moves the range with it and no view record changes. Drawing practice, from ISO 128-30 on
//! views and sections and the usual floor plan, puts the cut plane about 1.2 m above the floor
//! (through windows, below the head of doors); that is a default for a builder of views to choose,
//! not a rule held here.
//!
//! From high to low: **top** (nothing above it is in the view), **cut plane** (what it crosses is
//! cut), **bottom** (below it, what is seen is beyond the main drawing) and **view depth** (the
//! lowest anything is seen).

use crate::error::ViewError;
use construct_levels::{Levels, Tie, height_of};
use construct_params::Length;
use kit_core::Id;

/// A view range: four ties. It is a plain value; whether the levels it names exist, and whether
/// the heights are in order, are asked by [`ViewRange::resolve`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewRange {
    /// Nothing above this is in the view.
    pub top: Tie,
    /// The horizontal plane the plan is cut at.
    pub cut: Tie,
    /// Below this, what is seen is beyond the main drawing.
    pub bottom: Tie,
    /// The lowest anything is seen.
    pub depth: Tie,
}

/// The four heights of a range above the project's zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heights {
    /// The top.
    pub top: Length,
    /// The cut plane.
    pub cut: Length,
    /// The bottom.
    pub bottom: Length,
    /// The view depth.
    pub depth: Length,
}

impl ViewRange {
    /// A range with all four heights tied to one level, each at an offset from it.
    #[must_use]
    pub fn on_level(level: &Id, top: Length, cut: Length, bottom: Length, depth: Length) -> Self {
        let tie = |offset| Tie {
            level: level.clone(),
            offset,
        };
        Self {
            top: tie(top),
            cut: tie(cut),
            bottom: tie(bottom),
            depth: tie(depth),
        }
    }

    /// Every level the range is tied to, top first, with repeats.
    #[must_use]
    pub fn levels(&self) -> [&Id; 4] {
        [
            &self.top.level,
            &self.cut.level,
            &self.bottom.level,
            &self.depth.level,
        ]
    }

    /// The four heights, each through [`height_of`], the one function that says how high a tie sits.
    ///
    /// # Errors
    /// [`ViewError::UnknownLevel`] for a level `levels` cannot resolve, [`ViewError::OutOfRange`]
    /// for a height that does not fit a length, [`ViewError::RangeOrder`] unless top is at or
    /// above the cut plane, the cut plane at or above the bottom and the bottom at or above the
    /// view depth.
    pub fn resolve(&self, levels: &Levels) -> Result<Heights, ViewError> {
        let at = |tie: &Tie| -> Result<Length, ViewError> {
            let level = levels
                .resolve(&tie.level)
                .ok_or_else(|| ViewError::UnknownLevel(tie.level.clone()))?;
            height_of(&level, tie.offset).map_err(|e| ViewError::OutOfRange(e.to_string()))
        };
        let heights = Heights {
            top: at(&self.top)?,
            cut: at(&self.cut)?,
            bottom: at(&self.bottom)?,
            depth: at(&self.depth)?,
        };
        if heights.top >= heights.cut
            && heights.cut >= heights.bottom
            && heights.bottom >= heights.depth
        {
            Ok(heights)
        } else {
            Err(ViewError::RangeOrder {
                top: heights.top.to_string(),
                cut: heights.cut.to_string(),
                bottom: heights.bottom.to_string(),
                depth: heights.depth.to_string(),
            })
        }
    }
}
