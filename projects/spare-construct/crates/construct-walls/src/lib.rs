//! Spare Construct: wall types and their layers, the data half (CON-A4-F2-P1).
//!
//! What a wall type is, as data: an ordered list of layers from the exterior face to the interior
//! face, each with a function, a thickness in whole millimetres, a material and whether it wraps;
//! a core set by two boundaries; and from these the total thickness and where every layer face
//! and every location line lies from the wall's reference line. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Building elements" (walls); the decisions are in
//! notes/CON-A4-F2-P1-wall-layers.md and the stored form in FORMAT.md beside this crate.
//!
//! - [`layer`]: [`Layer`], [`Function`] (what each function means for joins and for drawing),
//!   [`Join`] and [`join`], the join rule from the function's priority.
//! - [`assembly`]: [`Assembly`], the checked layers and core ([`Side`], [`Zone`]); its depths,
//!   [`Assembly::offsets`], and the pure edits [`Assembly::with_inserted`],
//!   [`Assembly::without_layer`], [`Assembly::with_moved`], [`Assembly::with_changed`] and
//!   [`Assembly::with_boundary`], each giving a valid new one or the refusal.
//! - [`offsets`]: [`LocationLine`] and [`Offsets`]. Every length is construct-params'
//!   `Length`, worked only in the private `length` module (sums, differences, the half of a
//!   centreline, the stored spelling), so a change to `Length`'s unit changes that one file.
//! - [`stored`]: [`Assembly::to_text`] and [`Assembly::from_text`], the one spelling kept in the
//!   type's system block under [`OWNER`].
//! - [`wall_type`]: a wall type as a construct-types [`construct_types::ElementType`] of the
//!   walls category ([`new_wall_type`], [`with_assembly`], [`assembly_of`]); the instance
//!   parameters [`LOCATION_LINE`] and [`FLIPPED`] ([`parameter_definitions`],
//!   [`add_parameters`], [`placement`], [`offsets_of`]).
//! - [`joins`]: wall joins in plan, L and T, by layer priority (job 5.1): [`PlanWall`] and
//!   [`PlanLayer`] as the rule reads a wall, [`joins::join`] giving each layer's [`Joined`]
//!   outlines (or why a wall meeting it is left unjoined), and [`lines_meet`], how the walls that
//!   meet are found. Pure and exact; note CON-A4-F2-P2-wall-joins.
//! - [`defaults`]: a small default metric set ([`default_walls`], [`default_wall_types`]).
//! - [`error`]: [`WallError`], every refusal as a value, naming the layer where there is one.
//!
//! Promises: every [`Assembly`] passes every check, and every edit returns one that does or a
//! refusal; every length is exact in construct-params' unit, a centreline the unit cannot hold
//! is refused (never rounded), and no float is stored or computed; the same type gives the same
//! bytes; reading refuses anything writing would not give; nothing panics. Depends on
//! construct-types (the type, its system block, parameters and instances), construct-params (the
//! parameter definitions), construct-categories (the walls category), construct-materials
//! (`MaterialId`, and the library a type's materials are checked against) and kit-core
//! (permanent IDs). Nothing here is interim, and nothing here builds a shape.
//!
//! Elsewhere, with what they take from here:
//! - The wall's shape (job 4.1) is built by construct-regen, not here: its `walls` module fills a
//!   `wall.layered` recipe with a wall instance's layers and their faces from [`offsets_of`], its
//!   `wall_plan` module lays them out on the kernel's grid, and its kernel builder makes one solid
//!   per layer through the kernel's interface only. This crate stays data, so it never depends on
//!   geometry.
//! - The commands that make and change wall types and draw walls through the edit log are
//!   construct-api's (`cloud::walls`): they check every edit with the functions here.
//! - Joins: L and T joins are [`joins`] (job 5.1), each pair of meeting layers settled by the
//!   priority [`join`] gives; construct-regen finds the walls that meet, fills each layered
//!   wall's recipe with its joined outlines, and builds them. X joins, mitres at any angle and
//!   joins of more than two walls at a point are job 6.1's.
//! - Wrapping geometry (job 6.1): which layers wrap is [`Layer::wraps`], with the rule written on
//!   [`Layer`]; separate answers for ends and for openings would be a new stored version.

pub mod assembly;
pub mod defaults;
pub mod error;
pub mod joins;
pub mod layer;
mod length;
pub mod offsets;
pub mod stored;
pub mod wall_type;

pub use assembly::{Assembly, MOST_TOTAL, Side, Zone};
pub use defaults::{DefaultWall, default_wall_types, default_walls};
pub use error::WallError;
pub use joins::{Joined, Outline, PlanLayer, PlanWall, TOLERANCE, lines_meet};
pub use layer::{Function, Join, Layer, join};
pub use offsets::{LocationLine, Offsets};
pub use stored::{FORMAT_VERSION, OWNER};
pub use wall_type::{
    FLIPPED, LOCATION_LINE, add_parameters, assembly_of, location_line_value, new_wall_type,
    offsets_of, parameter_definitions, placement, with_assembly,
};
