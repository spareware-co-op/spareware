//! The kinds of view, each holding the data only it has, and the detail level every view has.
//!
//! A kind carries its own data so that a view cannot be half one thing and half another: a plan
//! has a level and a range, a section or an elevation has a line, and the rest have neither.

use crate::cut::CutLine;
use crate::range::ViewRange;
use kit_core::Id;

/// How much of a family's detail a view draws. A family's recipe may show or hide a part by the
/// detail level (ARCHITECTURE.md, families); with the view's scale it also picks the line weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetailLevel {
    /// The simplest drawing: outlines, few parts.
    Coarse,
    /// The usual working drawing.
    Medium,
    /// Every part a family has to show.
    Fine,
}

impl DetailLevel {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Coarse => "coarse",
            Self::Medium => "medium",
            Self::Fine => "fine",
        }
    }

    /// The level a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "coarse" => Some(Self::Coarse),
            "medium" => Some(Self::Medium),
            "fine" => Some(Self::Fine),
            _ => None,
        }
    }
}

/// What a plan holds: the level it belongs to and its view range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanSetup {
    /// The level the plan is of. A view is tied to it as an element is, so the level is not
    /// deleted while a plan is of it.
    pub level: Id,
    /// How much of the building the plan reaches.
    pub range: ViewRange,
}

/// The kind of a view, with the data of that kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// A plan cut by a horizontal plane, looking down.
    FloorPlan(PlanSetup),
    /// A plan looking up at what is above the cut plane (a reflected ceiling plan).
    CeilingPlan(PlanSetup),
    /// A vertical cut along a line.
    Section(CutLine),
    /// A view of the faces of the building from a plane in front of them.
    Elevation(CutLine),
    /// A view of the whole model in three dimensions.
    ThreeD,
    /// A view of drawn lines and text only, with no model in it.
    Drafting,
    /// A view that lists the symbols and styles a project uses, with no model in it.
    Legend,
}

impl ViewKind {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::FloorPlan(_) => "floor_plan",
            Self::CeilingPlan(_) => "ceiling_plan",
            Self::Section(_) => "section",
            Self::Elevation(_) => "elevation",
            Self::ThreeD => "three_d",
            Self::Drafting => "drafting",
            Self::Legend => "legend",
        }
    }

    /// Every key a kind is stored under.
    pub const KEYS: [&'static str; 7] = [
        "floor_plan",
        "ceiling_plan",
        "section",
        "elevation",
        "three_d",
        "drafting",
        "legend",
    ];

    /// The plan's level and range, for a floor plan or a ceiling plan.
    #[must_use]
    pub fn plan(&self) -> Option<&PlanSetup> {
        match self {
            Self::FloorPlan(plan) | Self::CeilingPlan(plan) => Some(plan),
            _ => None,
        }
    }

    /// The line, for a section or an elevation.
    #[must_use]
    pub fn cut_line(&self) -> Option<&CutLine> {
        match self {
            Self::Section(line) | Self::Elevation(line) => Some(line),
            _ => None,
        }
    }

    /// Whether the view draws the model's elements at all: every kind but drafting and legend.
    #[must_use]
    pub fn shows_model(&self) -> bool {
        !matches!(self, Self::Drafting | Self::Legend)
    }
}
