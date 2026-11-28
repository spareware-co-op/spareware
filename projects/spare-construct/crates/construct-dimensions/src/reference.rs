//! What a dimension is bound to: an element and the part of it that is measured.
//!
//! A reference is held in the dimension's record as construct-model's own [`Pointer`] (kind
//! [`REFERENCE_KIND`], the part's key as its name, wanting the carrier), so that the model's one
//! way of pointing at part of another element is used and nothing is stored twice. The parts are
//! named by what they are, never by a position in a list.

use crate::error::DimensionError;
use construct_model::element::{Reference as Pointer, Want};
use kit_core::Id;

/// The kind of every reference a dimension record holds.
pub const REFERENCE_KIND: &str = "dimension.reference";

/// A side of a wall: its exterior is the side on the right of its line walked from start to end
/// (construct-regen's builders), its interior the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The exterior finish face.
    Exterior,
    /// The interior finish face.
    Interior,
}

/// An edge of a door, a window or an opening in a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edge {
    /// The edge nearer the wall's start.
    Start,
    /// The edge nearer the wall's end.
    End,
}

/// The part of an element a dimension measures to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// A finish face of a wall.
    WallFace(Side),
    /// A wall's reference line: its location line, as drawn.
    WallLine,
    /// A straight grid line.
    GridLine,
    /// A level: a horizontal plane at its elevation.
    Level,
    /// The centre plane of a door, a window or an opening in a wall.
    Centre,
    /// One edge of a door, a window or an opening in a wall.
    Edge(Edge),
}

impl Feature {
    /// Every feature, in the order their keys sort.
    pub const ALL: [Feature; 8] = [
        Feature::Centre,
        Feature::Edge(Edge::End),
        Feature::Edge(Edge::Start),
        Feature::GridLine,
        Feature::Level,
        Feature::WallFace(Side::Exterior),
        Feature::WallFace(Side::Interior),
        Feature::WallLine,
    ];

    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::WallFace(Side::Exterior) => "wall.face.exterior",
            Self::WallFace(Side::Interior) => "wall.face.interior",
            Self::WallLine => "wall.line",
            Self::GridLine => "grid.line",
            Self::Level => "level",
            Self::Centre => "centre",
            Self::Edge(Edge::Start) => "edge.start",
            Self::Edge(Edge::End) => "edge.end",
        }
    }

    /// The feature a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|feature| feature.key() == key)
    }

    /// What kind of element it is part of, in one word: `wall`, `grid`, `level` or `opening`.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::WallFace(_) | Self::WallLine => "wall",
            Self::GridLine => "grid",
            Self::Level => "level",
            Self::Centre | Self::Edge(_) => "opening",
        }
    }

    /// What it is called in a sentence.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::WallFace(Side::Exterior) => "the exterior face of",
            Self::WallFace(Side::Interior) => "the interior face of",
            Self::WallLine => "the reference line of",
            Self::GridLine => "the grid line",
            Self::Level => "the level",
            Self::Centre => "the centre of",
            Self::Edge(Edge::Start) => "the start edge of",
            Self::Edge(Edge::End) => "the end edge of",
        }
    }
}

/// One reference: an element, and the part of it that is measured.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    element: Id,
    feature: Feature,
}

impl Reference {
    /// A reference to `feature` of `element`.
    #[must_use]
    pub fn new(element: Id, feature: Feature) -> Self {
        Self { element, feature }
    }

    /// The element.
    #[must_use]
    pub fn element(&self) -> &Id {
        &self.element
    }

    /// The part of it.
    #[must_use]
    pub fn feature(&self) -> Feature {
        self.feature
    }

    /// It in a sentence: `the exterior face of W-1`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{} {}", self.feature.words(), self.element)
    }

    /// It as the model's own reference, as a dimension record holds it.
    #[must_use]
    pub fn to_model(&self) -> Pointer {
        Pointer {
            doc: None,
            element: self.element.clone(),
            name: self.feature.key().to_owned(),
            kind: REFERENCE_KIND.to_owned(),
            want: Want::Carrier,
            signature: None,
        }
    }

    /// Reads one from the model's reference. Anything [`Reference::to_model`] would not write is
    /// refused.
    ///
    /// # Errors
    /// [`DimensionError::Malformed`] for a reference that is not one a dimension holds.
    pub fn from_model(pointer: &Pointer) -> Result<Self, DimensionError> {
        let feature = Feature::from_key(&pointer.name);
        match feature {
            Some(feature)
                if pointer.doc.is_none()
                    && pointer.kind == REFERENCE_KIND
                    && pointer.want == Want::Carrier
                    && pointer.signature.is_none() =>
            {
                Ok(Self::new(pointer.element.clone(), feature))
            }
            _ => Err(DimensionError::Malformed(format!(
                "A dimension's reference to {} is not one as a dimension writes it.",
                pointer.element
            ))),
        }
    }
}
