//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. One
//! about a layer names it by its index from the top (the top layer is index 0); one about a
//! boundary names the loop ([`Ring`]) and the edge or edges by their permanent IDs.

use crate::boundary::Ring;
use construct_categories::CategoryId;
use construct_materials::MaterialError;
use construct_types::TypeError;
use kit_core::Id;
use std::fmt;

/// Why a floor type, a boundary, a stored text or a floor's reading was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlabError {
    /// A floor type needs at least one layer.
    NoLayers,
    /// Only a membrane may be 0 thick.
    ZeroThickness {
        /// The layer's index from the top.
        layer: usize,
    },
    /// A layer's thickness is below 0.
    NegativeThickness {
        /// The layer's index from the top.
        layer: usize,
    },
    /// The layers up to and including this one add up to more than [`crate::MOST_TOTAL`].
    TooThick {
        /// The layer's index from the top.
        layer: usize,
    },
    /// Every layer is 0 thick, so the floor would have no solid.
    NoThickness,
    /// The floor's sketch holds constraints the solver cannot solve; the text is the solver's
    /// reason (over-constrained with the constraints that conflict, not converged, out of work,
    /// refused at rounding).
    NotSolved {
        /// The solver's reason.
        why: String,
    },
    /// A dimension change named a constraint the sketch lacks, or one that holds no length.
    NotADimension {
        /// The constraint's number in the sketch.
        constraint: u32,
    },
    /// A slope arrow's tail and head are one point, so it has no direction.
    SlopeNoDirection,
    /// A slope given as a rise over a run along an arrow that is not a whole number of grid steps
    /// long, so no height along it is exact.
    SlopeArrowNotWhole,
    /// The slope puts this corner of the floor's top face at a height that is not a whole grid
    /// step (156.25 nm), so the sloped face cannot be built exactly on the grid.
    SlopeOffGrid {
        /// The loop.
        ring: Ring,
        /// The edge that starts at the corner.
        segment: Id,
    },
    /// A floor with both a slope and slab edges: the edges' tops would follow the sloped
    /// underside, which is not built yet.
    SlopeWithEdges,
    /// A slab edge names no edge of the floor's boundary.
    EdgeUnknown {
        /// The edge it names.
        edge: Id,
    },
    /// Two slab edges run along one edge of the floor.
    EdgeTwice {
        /// The edge.
        edge: Id,
    },
    /// A slab edge's profile is not one that can be built: the text says why.
    EdgeProfile {
        /// The edge.
        edge: Id,
        /// Why.
        why: String,
    },
    /// A slab edge's inner face does not land on whole grid steps along this edge (its width
    /// across a sloped edge, or its mitre with the next edge, falls between steps).
    EdgeOffGrid {
        /// The edge.
        edge: Id,
    },
    /// Two slab edges of different profiles meet at a corner; their pieces cannot be mitred.
    EdgesMeet {
        /// The edge that ends at the corner.
        first: Id,
        /// The edge that starts there.
        second: Id,
    },
    /// A slab edge ends square at a corner sharper than a right angle, so its end would stand
    /// outside the floor.
    EdgeEndOutside {
        /// The edge.
        edge: Id,
    },
    /// A slab edge is too wide for its floor: its inner face reaches another side of the floor,
    /// or its ends cross.
    EdgeTooWide {
        /// The edge.
        edge: Id,
    },
    /// Two slab edges that do not meet at a corner overlap.
    EdgesOverlap {
        /// The earlier edge.
        first: Id,
        /// The later edge.
        second: Id,
    },
    /// A slab edge's volume is not a whole number of cubic grid steps, so it has no exact value.
    EdgeVolumeNotExact {
        /// The edge.
        edge: Id,
    },
    /// An edge names a line or curve the sketch does not have.
    UnknownEntity {
        /// The loop.
        ring: Ring,
        /// The edge.
        segment: Id,
    },
    /// Two edges are the same line or curve of the sketch.
    UsedTwice {
        /// The loop of the second.
        ring: Ring,
        /// The second edge.
        segment: Id,
    },
    /// A point, line or curve of the sketch that no edge uses.
    Unused {
        /// What it is, such as `line 10`.
        what: String,
    },
    /// Two edges of the floor have one ID.
    SameId {
        /// The ID.
        segment: Id,
    },
    /// An edge is an arc. The engine's extrusion takes a polygon only, so an arc edge cannot be
    /// built exactly yet; arcs are held in the data and refused until it can.
    Arc {
        /// The loop.
        ring: Ring,
        /// The edge.
        segment: Id,
    },
    /// A loop has fewer than three edges.
    TooFewSides {
        /// The loop.
        ring: Ring,
        /// How many it has.
        sides: usize,
    },
    /// An edge does not end where the next one starts.
    NotClosed {
        /// The loop.
        ring: Ring,
        /// The edge whose end is not the next one's start.
        segment: Id,
    },
    /// An edge starts and ends at one place.
    ZeroLength {
        /// The loop.
        ring: Ring,
        /// The edge.
        segment: Id,
    },
    /// Two edges of one loop cross, touch, or run over each other.
    Crossing {
        /// The loop.
        ring: Ring,
        /// The earlier edge.
        first: Id,
        /// The later edge.
        second: Id,
    },
    /// A hole lies outside the outer loop.
    HoleOutside {
        /// The hole's index.
        hole: usize,
    },
    /// A hole's edge crosses or touches the outer loop.
    HoleCrossesOuter {
        /// The hole's index.
        hole: usize,
        /// The hole's edge.
        segment: Id,
        /// The outer loop's edge.
        outer: Id,
    },
    /// Two holes overlap or touch.
    HolesOverlap {
        /// The earlier hole's index.
        first: usize,
        /// The later hole's index.
        second: usize,
        /// The two edges that meet, the earlier hole's first; none when one hole lies inside the
        /// other.
        segment: Option<(Id, Id)>,
    },
    /// A corner or length is beyond the kernel's coordinate range, or a sum of them does not fit.
    OutOfRange,
    /// The area is not a whole number of square grid steps (its corners are an odd number of
    /// steps apart in a way that leaves half a square step), so it has no exact value.
    AreaNotExact,
    /// The type is not of the floors category.
    NotAFloorType {
        /// Its category.
        category: CategoryId,
    },
    /// A floor type of the floors category with no floor block.
    NoFloorLayers {
        /// The type's ID.
        ty: Id,
    },
    /// A record or recipe that is not a floor's; the text says why.
    NotAFloor(String),
    /// The type is not of the ceilings category.
    NotACeilingType {
        /// Its category.
        category: CategoryId,
    },
    /// A ceiling type of the ceilings category with no ceiling block.
    NoCeilingLayers {
        /// The type's ID.
        ty: Id,
    },
    /// A record or recipe that is not a ceiling's; the text says why.
    NotACeiling(String),
    /// A ceiling's grid pattern that cannot be held or counted; the text says why.
    BadGrid(String),
    /// A material ID that is not a kit ID, or the library's refusal.
    Material(MaterialError),
    /// Refused by construct-types.
    Type(TypeError),
    /// A stored text or value is not one this crate writes; the text says why.
    Malformed(String),
}

impl From<TypeError> for SlabError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<MaterialError> for SlabError {
    fn from(error: MaterialError) -> Self {
        Self::Material(error)
    }
}

impl fmt::Display for SlabError {
    #[allow(
        clippy::too_many_lines,
        reason = "one sentence for each refusal, in the order they are declared"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLayers => write!(f, "a floor type needs at least one layer"),
            Self::ZeroThickness { layer } => {
                write!(f, "layer {layer} is 0 thick; only a membrane may be 0")
            }
            Self::NegativeThickness { layer } => write!(f, "layer {layer} is less than 0 thick"),
            Self::TooThick { layer } => {
                write!(f, "the layers up to layer {layer} add up to more than 10 m")
            }
            Self::NoThickness => write!(f, "a floor type needs a layer thicker than 0"),
            Self::NotSolved { why } => write!(f, "the floor's sketch cannot be solved: {why}"),
            Self::NotADimension { constraint } => write!(
                f,
                "constraint {constraint} of the floor's sketch is not a dimension to change"
            ),
            Self::UnknownEntity { ring, segment } => {
                write!(f, "edge {segment} of {ring} names nothing in the sketch")
            }
            Self::UsedTwice { ring, segment } => write!(
                f,
                "edge {segment} of {ring} is a line or curve another edge already is"
            ),
            Self::Unused { what } => write!(f, "{what} of the sketch is on no edge of the floor"),
            Self::SameId { segment } => write!(f, "two edges of the floor are both {segment}"),
            Self::Arc { ring, segment } => write!(
                f,
                "edge {segment} of {ring} is an arc, and a floor with an arc edge cannot be built \
                 exactly yet: draw it with straight edges"
            ),
            Self::TooFewSides { ring, sides } => {
                write!(f, "{ring} has {sides} edges; a loop needs at least three")
            }
            Self::NotClosed { ring, segment } => write!(
                f,
                "{ring} is not closed: edge {segment} does not end where the next edge starts"
            ),
            Self::ZeroLength { ring, segment } => {
                write!(f, "edge {segment} of {ring} has no length")
            }
            Self::Crossing {
                ring,
                first,
                second,
            } => write!(
                f,
                "{ring} crosses or touches itself where edges {first} and {second} meet"
            ),
            Self::HoleOutside { hole } => write!(f, "hole {hole} is outside the outer loop"),
            Self::HoleCrossesOuter {
                hole,
                segment,
                outer,
            } => write!(
                f,
                "hole {hole} crosses or touches the outer loop where its edge {segment} meets \
                 edge {outer}"
            ),
            Self::HolesOverlap {
                first,
                second,
                segment: Some((a, b)),
            } => write!(
                f,
                "holes {first} and {second} overlap or touch where edges {a} and {b} meet"
            ),
            Self::HolesOverlap {
                first,
                second,
                segment: None,
            } => write!(
                f,
                "holes {first} and {second} overlap: one is inside the other"
            ),
            Self::OutOfRange => write!(f, "a corner or length is beyond the kernel's grid"),
            Self::AreaNotExact => write!(
                f,
                "the floor's area falls between two square steps of the length unit"
            ),
            Self::NotAFloorType { category } => {
                write!(f, "a type of category {} is not a floor type", category.0)
            }
            Self::NoFloorLayers { ty } => write!(
                f,
                "floor type {ty} has no layers: a floor type carries a {:?} system block",
                crate::stored::OWNER
            ),
            Self::NotAFloor(why) => write!(f, "not a floor: {why}"),
            Self::NotACeilingType { category } => {
                write!(f, "a type of category {} is not a ceiling type", category.0)
            }
            Self::NoCeilingLayers { ty } => write!(
                f,
                "ceiling type {ty} has no layers: a ceiling type carries a {:?} system block",
                crate::ceiling_type::OWNER
            ),
            Self::NotACeiling(why) => write!(f, "not a ceiling: {why}"),
            Self::BadGrid(why) => write!(f, "the ceiling's grid: {why}"),
            Self::Material(error) => error.fmt(f),
            Self::Type(error) => error.fmt(f),
            Self::Malformed(why) => write!(f, "not a stored floor: {why}"),
            shaped => shaped.shape_text(f),
        }
    }
}

impl SlabError {
    /// The words of a refusal about a floor's slope or slab edges (job 5.2); nothing for any
    /// other, which [`fmt::Display`] words itself.
    fn shape_text(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlopeNoDirection => write!(
                f,
                "the slope arrow's tail and head are one point, so it points nowhere"
            ),
            Self::SlopeArrowNotWhole => write!(
                f,
                "a slope given as a rise over a run needs an arrow a whole number of grid steps \
                 long (along x or y, or a whole length such as 3, 4, 5): give the heights at its \
                 tail and head instead"
            ),
            Self::SlopeOffGrid { ring, segment } => write!(
                f,
                "the slope puts the start of edge {segment} of {ring} at a height between two grid \
                 steps: move the arrow or change a height"
            ),
            Self::SlopeWithEdges => write!(
                f,
                "a floor cannot have both a slope and slab edges yet: slab edges under a sloped \
                 floor are not built"
            ),
            Self::EdgeUnknown { edge } => {
                write!(f, "slab edge {edge} names no edge of the floor")
            }
            Self::EdgeTwice { edge } => write!(f, "edge {edge} has two slab edges"),
            Self::EdgeProfile { edge, why } => write!(f, "slab edge {edge}: {why}"),
            Self::EdgeOffGrid { edge } => write!(
                f,
                "slab edge {edge}'s inner face falls between grid steps: give it a width that \
                 lands on the grid across its edge"
            ),
            Self::EdgesMeet { first, second } => write!(
                f,
                "slab edges {first} and {second} meet at a corner with different profiles, and \
                 cannot be mitred"
            ),
            Self::EdgeEndOutside { edge } => write!(
                f,
                "slab edge {edge} ends square at a corner sharper than a right angle, outside the \
                 floor: give the next edge the same slab edge so they mitre"
            ),
            Self::EdgeTooWide { edge } => write!(
                f,
                "slab edge {edge} is too wide for the floor: it reaches another side, or its ends \
                 cross"
            ),
            Self::EdgesOverlap { first, second } => {
                write!(f, "slab edges {first} and {second} overlap")
            }
            Self::EdgeVolumeNotExact { edge } => write!(
                f,
                "slab edge {edge}'s volume falls between two cubic steps of the length unit"
            ),
            _ => Ok(()),
        }
    }
}

impl std::error::Error for SlabError {}
