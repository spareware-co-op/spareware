//! Every refusal, as a value the caller can read. Its words are what a person sees.

use construct_categories::CategoryId;
use construct_phases::PhaseError;
use construct_spatial::SpatialError;
use kit_core::Id;
use std::fmt;

/// Why a view, a record of one, or a question about what a view shows was refused. A refusal
/// changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// A view needs a name with something in it besides spaces.
    EmptyName,
    /// The document already has a view of this name (case does not matter).
    DuplicateName(String),
    /// A view already has this ID.
    DuplicateView(Id),
    /// No view has this ID.
    UnknownView(Id),
    /// No level has this ID: a plan's level or a level a view range is tied to.
    UnknownLevel(Id),
    /// A view range whose heights are not top at or above cut plane, cut plane at or above bottom,
    /// bottom at or above view depth. Each is the height as the levels give it, in millimetres.
    RangeOrder {
        /// The top, as text.
        top: String,
        /// The cut plane, as text.
        cut: String,
        /// The bottom, as text.
        bottom: String,
        /// The view depth, as text.
        depth: String,
    },
    /// A section or elevation line that starts where it ends.
    EmptyLine,
    /// A section or elevation that looks no distance.
    NoDepth,
    /// A height or a distance that does not fit, or a view too large to test exactly.
    OutOfRange(String),
    /// Two elements given to one question share an ID.
    DuplicateElement(Id),
    /// An element names a category the table does not have.
    UnknownCategory(CategoryId),
    /// A refusal of the phases and options rule.
    Phase(PhaseError),
    /// A refusal of the spatial index: an element's box is not one it can hold.
    Spatial(SpatialError),
    /// A refusal of the pens: no such pen.
    Style(String),
    /// A record that is not a view as written.
    Malformed(String),
    /// A view of this kind shows no model, so it holds no graphics rules.
    NoGraphicsOnKind {
        /// The kind, in words.
        kind: String,
    },
    /// A view of this kind cuts nothing, so it holds no cut rule.
    NoCutInKind {
        /// The kind, in words.
        kind: String,
    },
    /// A cut rule for a category that cannot be cut.
    CutOnUncuttable(CategoryId),
    /// A category a view does not draw (the views category itself).
    NotStyled(CategoryId),
    /// A category given where a top-level category is needed.
    NotTopLevel(CategoryId),
    /// A subcategory given with a category that is not its parent.
    NotASubcategory {
        /// The subcategory.
        subcategory: CategoryId,
        /// The category it was given with.
        category: CategoryId,
    },
    /// A pen the pen table of the category's kind does not have.
    NoSuchPen {
        /// The pen number given.
        pen: u16,
    },
    /// A line pattern the project does not have.
    NoSuchPattern {
        /// The pattern number given.
        pattern: u32,
    },
    /// A fill pattern that is not one we have.
    NoSuchFill(String),
    /// A fill on a projection rule: only what is cut has one.
    FillOnProjection,
    /// A transparency above 100 percent.
    TransparencyAbove(u8),
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A view needs a name."),
            Self::DuplicateName(name) => {
                write!(f, "There is already a view named {name:?} in this model.")
            }
            Self::DuplicateView(id) => write!(f, "There is already a view {id}."),
            Self::UnknownView(id) => write!(f, "There is no view {id} in this model."),
            Self::UnknownLevel(id) => write!(f, "There is no level {id} in this model."),
            Self::RangeOrder {
                top,
                cut,
                bottom,
                depth,
            } => write!(
                f,
                "A view range must run top, cut plane, bottom, view depth from high to low, \
                 not {top}, {cut}, {bottom}, {depth}."
            ),
            Self::EmptyLine => write!(f, "A section or elevation line needs two different ends."),
            Self::NoDepth => write!(f, "A section or elevation needs a depth above zero."),
            Self::OutOfRange(why) => write!(f, "Out of range: {why}."),
            Self::DuplicateElement(id) => write!(f, "The element {id} was given twice."),
            Self::UnknownCategory(id) => write!(f, "There is no category {}.", id.0),
            Self::Phase(error) => write!(f, "{error}"),
            Self::Spatial(error) => write!(f, "{error}"),
            Self::Style(why) | Self::Malformed(why) => f.write_str(why),
            Self::NoGraphicsOnKind { kind } => write!(
                f,
                "A {kind} view shows no model, so it has no graphics rules."
            ),
            Self::NoCutInKind { kind } => {
                write!(f, "A {kind} view cuts nothing, so it has no cut rules.")
            }
            Self::CutOnUncuttable(id) => {
                write!(f, "Category {} cannot be cut, so it has no cut rule.", id.0)
            }
            Self::NotStyled(id) => write!(f, "Category {} is not drawn in a view.", id.0),
            Self::NotTopLevel(id) => write!(
                f,
                "Category {} is a subcategory; name its parent as the category.",
                id.0
            ),
            Self::NotASubcategory {
                subcategory,
                category,
            } => write!(
                f,
                "Category {} is not a subcategory of category {}.",
                subcategory.0, category.0
            ),
            Self::NoSuchPen { pen } => write!(f, "There is no pen {pen} for this category."),
            Self::NoSuchPattern { pattern } => write!(f, "There is no line pattern {pattern}."),
            Self::NoSuchFill(name) => write!(
                f,
                "There is no fill pattern {name:?}. The fill patterns are {}.",
                crate::graphics::FILL_PATTERNS.join(", ")
            ),
            Self::FillOnProjection => {
                write!(f, "Only a cut has a fill: a projection rule has none.")
            }
            Self::TransparencyAbove(n) => write!(
                f,
                "Transparency is a percentage from 0 to {}, not {n}.",
                crate::graphics::MOST_TRANSPARENCY
            ),
        }
    }
}

impl std::error::Error for ViewError {}

impl From<PhaseError> for ViewError {
    fn from(error: PhaseError) -> Self {
        Self::Phase(error)
    }
}

impl From<SpatialError> for ViewError {
    fn from(error: SpatialError) -> Self {
        Self::Spatial(error)
    }
}
