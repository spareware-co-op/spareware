//! Spare Construct: floors from a boundary, the data half (CON-A4-F4-P1, first half).
//!
//! What a floor is, as data: a floor type's layers from the top face down (function, thickness,
//! material), with its total thickness and the rule for where it sits on its level (its top face
//! at the level's elevation plus its offset, the layers below); a floor's boundary, one closed
//! outer loop and any number of holes of straight edges in plan, held as the sketch solver's
//! geometry and checked; and what a floor reads back exactly: area, perimeter, volume and mass.
//! Map: projects/spare-construct/ARCHITECTURE.md, "Building elements" (floors); the stored form
//! is in FORMAT.md beside this crate (written with the handover's next step).
//!
//! - [`layers`]: [`Function`] (a wall layer's five, kept equal by a test), [`FloorLayer`],
//!   [`Layers`] ([`Layers::face_heights`] is the level rule), [`MOST_TOTAL`].
//! - [`stored`]: [`Layers::to_text`] and [`Layers::from_text`], the one spelling kept in the
//!   type's system block under [`OWNER`].
//! - [`floor_type`]: [`new_floor_type`], [`with_layers`], [`layers_of`].
//! - [`boundary`]: [`Boundary`], [`Segment`], [`Edge`], [`Ring`], [`Corner`]; every check and
//!   refusal; [`Boundary::profile`] for the builder; the stored and the plain (command) forms.
//! - [`measure`]: [`area`], [`perimeter`], [`quantities`], [`read_only`], [`layer_read_only`].
//! - [`shape`]: a sloped floor and slab edges (job 5.2): [`SlopeArrow`], [`Rise`],
//!   [`EdgeProfile`], [`SlabEdge`], [`Shape`] (a boundary with both, checked), [`EdgePiece`],
//!   [`TopArea`].
//! - [`floor`]: the element record and its recipe ([`floor_record`], [`floor_recipe`],
//!   [`shaped_recipe`], [`shape_of`], [`boundary_of`], [`with_boundary`], [`with_slope`],
//!   [`with_edges`], [`filled`]).
//! - [`defaults`]: [`default_floors`], [`default_floor_types`].
//! - [`error`]: [`SlabError`], every refusal as a value.
//!
//! Promises: every [`Layers`] and [`Boundary`] passes every check; every length is exact in
//! construct-params' unit and no float is stored or computed; the same data gives the same bytes;
//! reading refuses anything writing would not give; nothing panics. Depends on construct-solver,
//! construct-types, construct-params, construct-materials, construct-levels, construct-model,
//! construct-categories and kit-core.
//!
//! A floor's sketch may hold constraints, solved by construct-solver as a family recipe's are
//! ([`boundary`]); a floor may slope and carry slab edges ([`shape`]); the decisions are in the
//! note CON-A4-F4-P1-floor-slope-and-edges.
//!
//! Interim: an arc edge is held in the data and refused ([`SlabError::Arc`]) because the engine
//! seam's extrusion takes polygons only; it is built once the kernel's exact sweep takes arcs
//! (geom-sweep, chapter 13.4). Not here, with their places kept: slab edges under a sloped floor
//! ([`SlabError::SlopeWithEdges`]), floors by picking walls, openings by shafts (job 4.2),
//! structural decks.

pub mod boundary;
pub mod defaults;
pub mod error;
pub mod floor;
pub mod floor_type;
pub mod layers;
pub mod measure;
pub mod shape;
pub mod stored;

pub use boundary::{BOUNDARY_VERSION, Boundary, Corner, Edge, Ring, Segment};
pub use construct_solver::{ConstraintId, Point};
pub use defaults::{DefaultFloor, default_floor_types, default_floors};
pub use error::SlabError;
pub use floor::{
    BOUNDARY_ARG, EDGES_ARG, FLOOR_KIND, FLOOR_OP, LAYERS_ARG, LOOPS_ARG, OP_VERSION,
    SHAPED_VERSION, SLOPE_ARG, TOPS_ARG, boundary_of, filled, floor_recipe, floor_record, is_floor,
    layers_arg, reads_layers, shape_of, shaped_recipe, with_boundary, with_edges, with_slope,
};
pub use floor_type::{layers_of, new_floor_type, with_layers};
pub use layers::{FloorLayer, Function, Layers, MOST_TOTAL};
pub use measure::{
    AREA, EdgeQuantity, HIGHEST, LOWEST, LayerQuantity, MASS, PERIMETER, Quantities,
    ShapeQuantities, THICKNESS, TOP_AREA, VOLUME, area, layer_read_only, perimeter, quantities,
    read_only, shape_quantities, shape_read_only,
};
pub use shape::{EdgePiece, EdgeProfile, Rise, Shape, SlabEdge, SlopeArrow, TopArea};
pub use stored::{FORMAT_VERSION, OWNER};
