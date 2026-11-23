//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. One
//! about the footprint names the edge or edges by their permanent IDs; one about a layer names it
//! by its index from the top (construct-slabs' words); one that a later job lifts names the job.

use construct_categories::CategoryId;
use construct_materials::MaterialError;
use construct_slabs::SlabError;
use construct_types::TypeError;
use kit_core::Id;
use std::fmt;

/// Why a roof, a roof type, a stored text or a roof's reading was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoofError {
    /// The footprint or the type's layers, refused by construct-slabs' checks in its own words.
    Slab(SlabError),
    /// The footprint has holes: an opening through a roof is cut by job 4.2's openings.
    Holes {
        /// How many it has.
        count: usize,
    },
    /// An edge is given that the footprint does not have.
    UnknownEdge {
        /// The edge.
        edge: Id,
    },
    /// An edge's overhang is less than nothing.
    NegativeOverhang {
        /// The edge.
        edge: Id,
    },
    /// An edge's slope rises by nothing or falls: an edge that does not slope is given no slope.
    NoRise {
        /// The edge.
        edge: Id,
    },
    /// An edge's slope is steeper than [`crate::MOST_SLOPE`] (10 in 1, about 84.3 degrees), or an
    /// angle of 90 degrees or more: a face that steep is a wall.
    TooSteep {
        /// The edge.
        edge: Id,
    },
    /// A flat roof's edge has an overhang while an edge of its footprint runs off the plan's
    /// axes: the eaves outline of such a footprint is solved with job 6.2.
    OverhangOffAxis {
        /// The edge off the axes.
        edge: Id,
    },
    /// The overhangs fold the eaves outline back over itself at this edge, or two edges on one
    /// line have different overhangs.
    OverhangFolds {
        /// The edge.
        edge: Id,
    },
    /// Edges slope on a footprint that is not a rectangle along the plan's axes: other footprints
    /// come with job 6.2.
    NotARectangle {
        /// The sloping edges, in the footprint's order.
        edges: Vec<Id>,
    },
    /// Sloping edges that meet at a corner: hips come with job 6.2.
    Hips {
        /// The sloping edges, in the footprint's order.
        edges: Vec<Id>,
        /// Whether every edge slopes.
        every: bool,
    },
    /// The two sloping edges of a gable slope at different pitches: an uneven gable's ridge is
    /// solved with job 6.2.
    UnevenGable {
        /// The first sloping edge, in the footprint's order.
        first: Id,
        /// The second.
        second: Id,
    },
    /// The roof rising from this edge does not rise a whole grid step to its ridge or its high
    /// side, or is too narrow to have a ridge between its eaves.
    TooShallow {
        /// The edge.
        edge: Id,
    },
    /// A corner or a length is beyond the kernel's grid, or a sum of them does not fit.
    OutOfRange,
    /// An area or a volume falls between two square or cubic steps of the length unit, so it has
    /// no exact value; corners on the hundredth of a millimetre never meet it.
    NotExact,
    /// The type is not of the roofs category.
    NotARoofType {
        /// Its category.
        category: CategoryId,
    },
    /// A roof type of the roofs category with no roof block.
    NoRoofLayers {
        /// The type's ID.
        ty: Id,
    },
    /// A record or recipe that is not a roof's; the text says why.
    NotARoof(String),
    /// A material ID that is not a kit ID, or the library's refusal.
    Material(MaterialError),
    /// Refused by construct-types.
    Type(TypeError),
    /// A stored text or value is not one this crate writes; the text says why.
    Malformed(String),
}

impl From<SlabError> for RoofError {
    fn from(error: SlabError) -> Self {
        match error {
            SlabError::OutOfRange => Self::OutOfRange,
            SlabError::AreaNotExact => Self::NotExact,
            SlabError::Material(error) => Self::Material(error),
            SlabError::Type(error) => Self::Type(error),
            other => Self::Slab(other),
        }
    }
}

impl From<TypeError> for RoofError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<MaterialError> for RoofError {
    fn from(error: MaterialError) -> Self {
        Self::Material(error)
    }
}

/// `a`, `a and b`, `a, b and c`: a list of edges in words.
fn listed(edges: &[Id]) -> String {
    let names: Vec<&str> = edges.iter().map(Id::as_str).collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

impl fmt::Display for RoofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slab(error) => error.fmt(f),
            Self::Holes { count } => write!(
                f,
                "a roof's footprint has no holes, and this one has {count}: an opening through a \
                 roof is cut by job 4.2's openings"
            ),
            Self::UnknownEdge { edge } => {
                write!(f, "edge {edge} is not an edge of the roof's footprint")
            }
            Self::NegativeOverhang { edge } => {
                write!(f, "edge {edge}'s overhang is less than nothing")
            }
            Self::NoRise { edge } => write!(
                f,
                "edge {edge}'s slope does not rise: give no slope for an edge that does not slope"
            ),
            Self::TooSteep { edge } => write!(
                f,
                "edge {edge} slopes steeper than 10 in 1 (about 84.3 degrees): a face that steep is \
                 a wall"
            ),
            Self::OverhangOffAxis { edge } => write!(
                f,
                "edge {edge} runs off the plan's axes, so this footprint's overhangs have no exact \
                 eaves outline yet: they come with job 6.2"
            ),
            Self::OverhangFolds { edge } => write!(
                f,
                "the overhangs fold the eaves outline over itself at edge {edge}"
            ),
            Self::NotARectangle { edges } => write!(
                f,
                "{} {}, and a sloped roof's footprint is a rectangle along the plan's axes for \
                 now: other footprints come with job 6.2",
                if edges.len() == 1 { "edge" } else { "edges" },
                slope_words(edges)
            ),
            Self::Hips { edges, every: true } => write!(
                f,
                "every edge slopes: hips come with job 6.2 (edges {})",
                listed(edges)
            ),
            Self::Hips {
                edges,
                every: false,
            } if edges.len() == 2 => write!(
                f,
                "edges {} slope and meet at a corner: hips come with job 6.2",
                listed(edges)
            ),
            Self::Hips { edges, .. } => write!(
                f,
                "edges {} slope, and two of them meet at a corner: hips come with job 6.2",
                listed(edges)
            ),
            Self::UnevenGable { first, second } => write!(
                f,
                "edges {first} and {second} slope at different pitches: an uneven gable's ridge is \
                 solved with job 6.2"
            ),
            Self::TooShallow { edge } => write!(
                f,
                "the roof rising from edge {edge} does not rise a grid step to its ridge or its \
                 high side"
            ),
            Self::OutOfRange => write!(f, "a corner or length is beyond the kernel's grid"),
            Self::NotExact => write!(
                f,
                "the roof's area or volume falls between two steps of the length unit"
            ),
            Self::NotARoofType { category } => {
                write!(f, "a type of category {} is not a roof type", category.0)
            }
            Self::NoRoofLayers { ty } => write!(
                f,
                "roof type {ty} has no layers: a roof type carries a {:?} system block",
                crate::roof_type::OWNER
            ),
            Self::NotARoof(why) => write!(f, "not a roof: {why}"),
            Self::Material(error) => error.fmt(f),
            Self::Type(error) => error.fmt(f),
            Self::Malformed(why) => write!(f, "not a stored roof: {why}"),
        }
    }
}

/// `a slopes` or `a and b slope`.
fn slope_words(edges: &[Id]) -> String {
    if edges.len() == 1 {
        format!("{} slopes", listed(edges))
    } else {
        format!("{} slope", listed(edges))
    }
}

impl std::error::Error for RoofError {}
