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
    /// A view of this kind draws no model through a window of its own, so it has no crop.
    NoCropOnKind {
        /// The kind, in words.
        kind: String,
    },
    /// A crop outline with fewer than three corners.
    CropFewCorners {
        /// The corners given.
        count: usize,
    },
    /// A crop rectangle whose lower left corner is not below and left of its upper right.
    CropEmptyRectangle,
    /// A crop outline that gives a corner twice in a row (counting the last then the first).
    CropRepeatedCorner {
        /// The corner, counted from 1, that equals the next.
        corner: usize,
    },
    /// A crop outline with two edges that cross, touch or lie along each other.
    CropSelfCrossing {
        /// The first edge, counted from 1 (edge n runs from corner n to the next).
        first: usize,
        /// The second edge.
        second: usize,
    },
    /// A crop outline that encloses no area.
    CropNoArea,
    /// A view with no crop to show, hide or switch.
    NoCrop(Id),
    /// A view template needs a name with something in it besides spaces.
    EmptyTemplateName,
    /// The document already has a view template of this name (case does not matter).
    DuplicateTemplateName(String),
    /// A view template already has this ID.
    DuplicateTemplate(Id),
    /// No view template has this ID.
    UnknownTemplate(Id),
    /// A view of this kind shows no model, so no template applies to it.
    NoTemplateOnKind {
        /// The kind, in words.
        kind: String,
    },
    /// A tied view's change to a setting its template includes.
    TiedSetting {
        /// The view's name.
        view: String,
        /// The template's name.
        template: String,
        /// The setting, in words.
        setting: String,
    },
    /// A template that views are tied to cannot be deleted.
    TemplateInUse {
        /// The template's name.
        template: String,
        /// How many views are tied to it.
        views: usize,
    },
    /// A filter needs a name with something in it besides spaces.
    EmptyFilterName,
    /// The list already has a filter of this name (case does not matter).
    DuplicateFilterName(String),
    /// No filter has this number in the list.
    UnknownFilter(u32),
    /// A place in the list of filters that it does not have.
    FilterPosition {
        /// The place asked for (0 is the first).
        to: usize,
        /// How many filters there are.
        len: usize,
    },
    /// An operator a parameter of this kind cannot be given.
    OperatorKind {
        /// The operator's key.
        op: String,
        /// The parameter's name.
        parameter: String,
    },
    /// A view of this kind shows no model, so no override or filter applies to it.
    NoOverridesOnKind {
        /// The kind, in words.
        kind: String,
    },
}

impl fmt::Display for ViewError {
    #[allow(
        clippy::too_many_lines,
        reason = "one arm of words for each refusal; splitting it would scatter them"
    )]
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
            Self::NoCropOnKind { kind } => write!(
                f,
                "A {kind} view has no crop: only plans, sections and elevations are cropped."
            ),
            Self::CropFewCorners { count } => write!(
                f,
                "A crop outline needs at least three corners, not {count}."
            ),
            Self::CropEmptyRectangle => write!(
                f,
                "A crop rectangle needs its lower left corner below and left of its upper right."
            ),
            Self::CropRepeatedCorner { corner } => {
                write!(f, "A crop outline gives corner {corner} twice in a row.")
            }
            Self::CropSelfCrossing { first, second } => write!(
                f,
                "A crop outline must not cross itself: edges {first} and {second} meet."
            ),
            Self::CropNoArea => write!(f, "A crop outline must enclose an area above zero."),
            Self::NoCrop(id) => write!(f, "The view {id} has no crop."),
            Self::EmptyTemplateName => write!(f, "A view template needs a name."),
            Self::DuplicateTemplateName(name) => write!(
                f,
                "There is already a view template named {name:?} in this model."
            ),
            Self::DuplicateTemplate(id) => write!(f, "There is already a view template {id}."),
            Self::UnknownTemplate(id) => {
                write!(f, "There is no view template {id} in this model.")
            }
            Self::NoTemplateOnKind { kind } => write!(
                f,
                "A {kind} view shows no model, so a view template does not apply to it."
            ),
            Self::TiedSetting {
                view,
                template,
                setting,
            } => write!(
                f,
                "{view} follows the view template {template}, which sets its {setting}: \
                 change it in the template, or untie the view first."
            ),
            Self::TemplateInUse { template, views } => write!(
                f,
                "The view template {template} is in use by {views} view{}: untie {} first.",
                if *views == 1 { "" } else { "s" },
                if *views == 1 { "it" } else { "them" }
            ),
            Self::EmptyFilterName => write!(f, "A filter needs a name."),
            Self::DuplicateFilterName(name) => {
                write!(f, "There is already a filter named {name:?} here.")
            }
            Self::UnknownFilter(id) => write!(f, "There is no filter {id} here."),
            Self::FilterPosition { to, len } => write!(
                f,
                "There is no place {to} in a list of {len} filter{}.",
                if *len == 1 { "" } else { "s" }
            ),
            Self::OperatorKind { op, parameter } => write!(
                f,
                "A filter cannot compare the parameter {parameter} with {op}: \
                 contains is for text, choices and links, greater_than for lengths, areas, \
                 volumes, angles and numbers."
            ),
            Self::NoOverridesOnKind { kind } => write!(
                f,
                "A {kind} view shows no model, so it has no overrides or filters."
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
