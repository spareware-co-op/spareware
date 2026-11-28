//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. Each
//! has a short code (what a failed build calls it) and its words (what a person is told).

use construct_categories::CategoryId;
use construct_grids::GridError;
use construct_levels::LevelError;
use construct_materials::MaterialError;
use construct_params::Length;
use construct_types::TypeError;
use kit_core::Id;
use std::fmt;

/// Why a column type, a column, a stored text or a reading was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnError {
    /// A section's width, depth or diameter is not above zero, is not a whole even number of grid
    /// steps, or is past the largest a column has; the text says which and why.
    BadSize(String),
    /// The column's top is at its base: it would have no height.
    NoHeight,
    /// The column's top is below its base.
    TopBelowBase {
        /// How far below its base the top is.
        by: Length,
    },
    /// The type is not of the columns category.
    NotAColumnType {
        /// Its category.
        category: CategoryId,
    },
    /// A column type of the columns category with no column block.
    NoColumnSection {
        /// The type's ID.
        ty: Id,
    },
    /// A record or recipe that is not a column's; the text says why.
    NotAColumn(String),
    /// The two grid lines a column stands on do not cross.
    NoCrossing {
        /// The first grid's ID.
        first: Id,
        /// The second grid's ID.
        second: Id,
    },
    /// The two grid lines cross fewer times than the column's crossing number asks for.
    NoSuchCrossing {
        /// The first grid's ID.
        first: Id,
        /// The second grid's ID.
        second: Id,
        /// The crossing the column stands on, counted from 0.
        wanted: usize,
        /// How many crossings there are.
        found: usize,
    },
    /// A sum or a product does not fit a length, an area or a volume.
    OutOfRange,
    /// A material ID that is not a kit ID, or the library's refusal.
    Material(MaterialError),
    /// Refused by construct-types.
    Type(TypeError),
    /// Refused by construct-levels.
    Level(LevelError),
    /// Refused by construct-grids.
    Grid(GridError),
    /// A stored text or value is not one this crate writes; the text says why.
    Malformed(String),
}

impl ColumnError {
    /// The short code a failed build is called by.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadSize(_) => "BadSize",
            Self::NoHeight => "NoHeight",
            Self::TopBelowBase { .. } => "TopBelowBase",
            Self::NotAColumnType { .. } | Self::NoColumnSection { .. } | Self::Type(_) => {
                "BadColumnType"
            }
            Self::NotAColumn(_) | Self::Malformed(_) => "NotAColumn",
            Self::NoCrossing { .. } | Self::NoSuchCrossing { .. } | Self::Grid(_) => "BadGrid",
            Self::OutOfRange => "TooBig",
            Self::Material(_) => "BadMaterial",
            Self::Level(_) => "BadLevel",
        }
    }
}

impl From<TypeError> for ColumnError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<MaterialError> for ColumnError {
    fn from(error: MaterialError) -> Self {
        Self::Material(error)
    }
}

impl From<LevelError> for ColumnError {
    fn from(error: LevelError) -> Self {
        Self::Level(error)
    }
}

impl From<GridError> for ColumnError {
    fn from(error: GridError) -> Self {
        Self::Grid(error)
    }
}

impl fmt::Display for ColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadSize(why) => write!(f, "{why}"),
            Self::NoHeight => write!(f, "the column's top is at its base: it has no height"),
            Self::TopBelowBase { by } => write!(
                f,
                "the column's top is {by} below its base: a column stands up from its base"
            ),
            Self::NotAColumnType { category } => write!(
                f,
                "a column type is of the columns category, and this one is of category {}",
                category.0
            ),
            Self::NoColumnSection { ty } => {
                write!(f, "the type {ty} has no column section")
            }
            Self::NotAColumn(why) => write!(f, "not a column: {why}"),
            Self::NoCrossing { first, second } => write!(
                f,
                "grids {first} and {second} do not cross, so a column cannot stand where they do"
            ),
            Self::NoSuchCrossing {
                first,
                second,
                wanted,
                found,
            } => write!(
                f,
                "grids {first} and {second} cross {found} times, and the column stands on crossing {wanted}"
            ),
            Self::OutOfRange => write!(f, "a size, height or sum is past what a length holds"),
            Self::Material(e) => write!(f, "{e}"),
            Self::Type(e) => write!(f, "{e}"),
            Self::Level(e) => write!(f, "{e}"),
            Self::Grid(e) => write!(f, "{e}"),
            Self::Malformed(why) => {
                write!(f, "a stored value is not as this crate writes it: {why}")
            }
        }
    }
}

impl std::error::Error for ColumnError {}
