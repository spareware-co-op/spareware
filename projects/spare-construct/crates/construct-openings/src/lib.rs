//! Spare Construct: openings and shafts, the data half (CON-A4-F3-P2, job 4.2).
//!
//! What an opening is, as data: a rectangular hole through a wall that is not a door or a window
//! (its width and height; where it sits along the wall and its sill height are its hosting's, as
//! a door's are), an opening through a floor given by a closed outline inside the floor's
//! boundary, and a shaft: a closed outline between a base level and a top level, each with an
//! offset, that cuts every floor it passes through between them. Each cuts every layer of what it
//! goes through. Map: projects/spare-construct/ARCHITECTURE.md, "Building elements"; the stored
//! form is FORMAT.md beside this crate; the decisions are in
//! `projects/spare-construct/notes/CON-A4-F3-P2-openings-and-shafts.md`.
//!
//! - [`record`]: the kinds, operations and records ([`wall_opening_record`],
//!   [`floor_opening_record`], [`shaft_record`]) and the readers of their recipes
//!   ([`WallOpening::of`], [`outline_of`], [`Shaft::of`]).
//! - [`outline`]: [`Outline`], a closed loop of straight edges in grid steps, checked by the
//!   floor's own loop checks.
//! - [`fit`]: [`fit_in_floor`] (which outlines cut a floor, and every refusal) and [`reach`]
//!   (whether a shaft passes wholly through a floor's thickness).
//! - [`measure`]: what a wall layer and a floor layer hold after the cut ([`wall_layer`],
//!   [`floor_layer`]), exactly.
//! - [`error`]: [`OpeningError`], every refusal as a value naming the element and why.
//!
//! Promises: every length is construct-params' [`construct_params::Length`] in grid steps and no
//! float is stored or computed; the same data gives the same bytes; nothing panics. The checks
//! are construct-slabs' boundary checks, so an outline a floor refuses as a hole is refused here
//! in the same terms. Building is construct-regen's: the kernel's builder cuts a wall opening on
//! the door's own path and a floor's openings and shafts as holes through every layer.
//!
//! Interim: a curved outline is refused by name ([`OpeningError::Curved`]) until the engine
//! seam's extrusion takes curves; a shaft that ends inside a floor's thickness is refused
//! ([`OpeningError::EndsInFloor`]) rather than cut part way; openings in roofs and sloped hosts
//! are not here. Each has its row in the interim register.

pub mod error;
pub mod fit;
pub mod measure;
pub mod outline;
pub mod record;

pub use error::{End, Named, OpeningError, What};
pub use fit::{Cutter, Reach, fit_in_floor, reach};
pub use measure::{LayerAfter, floor_layer, wall_layer, with_cuts};
pub use outline::Outline;
pub use record::{
    AXIS_ARG, FLOOR_OP, HEIGHT_ARG, OP_VERSION, OPENING_KIND, OUTLINE_ARG, SHAFT_KIND, SHAFT_OP,
    Shaft, WALL_OP, WIDTH_ARG, WallOpening, floor_opening_recipe, floor_opening_record,
    is_floor_opening, is_shaft, is_shaft_record, is_void, is_wall_opening, named, outline_of,
    shaft_recipe, shaft_record, top_arg, wall_opening_record,
};
