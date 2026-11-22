//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing, and
//! every refusal about one layer names it by its index, counted from the exterior face (the
//! exterior finish layer is index 0).

use crate::assembly::Side;
use crate::offsets::LocationLine;
use construct_categories::CategoryId;
use construct_materials::MaterialError;
use construct_types::TypeError;
use std::fmt;

/// Why a wall type, an edit, a stored text or a wall instance's reading was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WallError {
    /// A wall type needs at least one layer.
    NoLayers,
    /// Only a membrane may be 0 thick.
    ZeroThickness {
        /// The layer's index from the exterior face.
        layer: usize,
    },
    /// A layer's thickness is below 0.
    NegativeThickness {
        /// The layer's index from the exterior face.
        layer: usize,
    },
    /// The layers up to and including this one add up to more than [`crate::MOST_TOTAL`].
    TooThick {
        /// The layer's index from the exterior face.
        layer: usize,
    },
    /// A core boundary lies past the last layer.
    BoundaryOutOfRange {
        /// Which boundary.
        side: Side,
        /// Where it was asked to lie: the count of layers on its exterior side.
        at: usize,
        /// How many layers the type has.
        layers: usize,
    },
    /// The exterior core boundary is not before the interior one, so the core holds no layer.
    EmptyCore {
        /// The exterior boundary.
        exterior: usize,
        /// The interior boundary.
        interior: usize,
    },
    /// Every layer in the core is 0 thick.
    CoreWithoutThickness {
        /// The index of the first core layer.
        first: usize,
        /// The index of the last core layer.
        last: usize,
    },
    /// A core layer is set to wrap; only a shell layer wraps (round the core).
    CoreLayerWraps {
        /// The layer's index from the exterior face.
        layer: usize,
    },
    /// A centreline falls between two steps of the length unit, so it has no exact offset.
    HalfNotExact {
        /// The line.
        line: LocationLine,
    },
    /// An edit named a layer index the type does not have.
    NoSuchLayer {
        /// The index asked for.
        layer: usize,
        /// How many layers the type has.
        layers: usize,
    },
    /// A material ID that is not a kit ID.
    Material(MaterialError),
    /// The type is not of the walls category.
    NotAWallType {
        /// Its category.
        category: CategoryId,
    },
    /// Refused by construct-types.
    Type(TypeError),
    /// A stored text or value is not one this crate writes; the text says why.
    Malformed(String),
}

impl From<TypeError> for WallError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<MaterialError> for WallError {
    fn from(error: MaterialError) -> Self {
        Self::Material(error)
    }
}

impl fmt::Display for WallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLayers => write!(f, "a wall type needs at least one layer"),
            Self::ZeroThickness { layer } => {
                write!(f, "layer {layer} is 0 thick; only a membrane may be 0")
            }
            Self::NegativeThickness { layer } => {
                write!(f, "layer {layer} is less than 0 thick")
            }
            Self::TooThick { layer } => {
                write!(f, "the layers up to layer {layer} add up to more than 10 m")
            }
            Self::BoundaryOutOfRange { side, at, layers } => write!(
                f,
                "the {} core boundary at {at} lies past the last of {layers} layers",
                side.word()
            ),
            Self::EmptyCore { exterior, interior } => write!(
                f,
                "the core holds no layer: its exterior boundary {exterior} is not before its \
                 interior boundary {interior}"
            ),
            Self::CoreWithoutThickness { first, last } => write!(
                f,
                "the core, layers {first} to {last}, has no layer thicker than 0"
            ),
            Self::CoreLayerWraps { layer } => write!(
                f,
                "layer {layer} is in the core and so cannot wrap; only a shell layer wraps"
            ),
            Self::HalfNotExact { line } => write!(
                f,
                "the {} falls between two steps of the length unit",
                line.label().to_lowercase()
            ),
            Self::NoSuchLayer { layer, layers } => {
                write!(f, "there is no layer {layer}: the type has {layers} layers")
            }
            Self::Material(error) => error.fmt(f),
            Self::NotAWallType { category } => {
                write!(f, "a type of category {} is not a wall type", category.0)
            }
            Self::Type(error) => error.fmt(f),
            Self::Malformed(why) => write!(f, "not a stored wall type: {why}"),
        }
    }
}

impl std::error::Error for WallError {}
