//! The six location lines, and where a wall's faces and lines lie from its reference line.
//!
//! Every face and both core faces are sums of layer thicknesses. Only the two centrelines are a
//! half (of a sum of two depths), and a half is exact only where the length unit holds it: see
//! the length module. A centreline whose half the unit cannot hold is refused, naming the line;
//! nothing is rounded and no float is used. Nothing here is stored.

use construct_params::Length;

/// The line of a wall that lies on its drawn reference line: the instance's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocationLine {
    /// Halfway between the two finish faces.
    WallCentre,
    /// Halfway between the two core faces.
    CoreCentre,
    /// The exterior face of the first layer.
    ExteriorFinishFace,
    /// The interior face of the last layer.
    InteriorFinishFace,
    /// The exterior face of the core.
    ExteriorCoreFace,
    /// The interior face of the core.
    InteriorCoreFace,
}

impl LocationLine {
    /// All six, in the order a list shows them.
    pub const ALL: [Self; 6] = [
        Self::WallCentre,
        Self::CoreCentre,
        Self::ExteriorFinishFace,
        Self::InteriorFinishFace,
        Self::ExteriorCoreFace,
        Self::InteriorCoreFace,
    ];

    /// Its stored key: the choice key of the location line parameter.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::WallCentre => "wall-centre",
            Self::CoreCentre => "core-centre",
            Self::ExteriorFinishFace => "exterior-finish-face",
            Self::InteriorFinishFace => "interior-finish-face",
            Self::ExteriorCoreFace => "exterior-core-face",
            Self::InteriorCoreFace => "interior-core-face",
        }
    }

    /// The name people see.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::WallCentre => "Wall centreline",
            Self::CoreCentre => "Core centreline",
            Self::ExteriorFinishFace => "Exterior finish face",
            Self::InteriorFinishFace => "Interior finish face",
            Self::ExteriorCoreFace => "Exterior core face",
            Self::InteriorCoreFace => "Interior core face",
        }
    }

    /// The line with this stored key.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|line| line.key() == key)
    }

    /// Its place in [`LocationLine::ALL`].
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::WallCentre => 0,
            Self::CoreCentre => 1,
            Self::ExteriorFinishFace => 2,
            Self::InteriorFinishFace => 3,
            Self::ExteriorCoreFace => 4,
            Self::InteriorCoreFace => 5,
        }
    }
}

/// Where a wall's layer faces and location lines lie from its reference line, each positive
/// toward the wall's exterior side (made by [`crate::Assembly::offsets`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offsets {
    faces: Vec<Length>,
    /// By [`LocationLine::index`]; `None` for a centreline whose half the unit cannot hold.
    lines: [Option<Length>; 6],
}

impl Offsets {
    pub(crate) fn new(faces: Vec<Length>, lines: [Option<Length>; 6]) -> Self {
        Self { faces, lines }
    }

    /// Every layer face, from the exterior finish face (face 0) to the interior finish face
    /// (face `n`); face `k` is between layers `k - 1` and `k`.
    #[must_use]
    pub fn faces(&self) -> &[Length] {
        &self.faces
    }

    /// Where a location line lies; `None` only for a centreline whose half the length unit
    /// cannot hold.
    #[must_use]
    pub fn line(&self, line: LocationLine) -> Option<Length> {
        self.lines.get(line.index()).copied().flatten()
    }
}
