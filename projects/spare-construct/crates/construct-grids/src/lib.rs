//! Spare Construct: grids and reference planes (CON-A4-F1-P2, CON-A4-F1-P3).
//!
//! A grid line and a reference plane are elements of the datum categories: each has a permanent
//! ID, a name, and its geometry in plan, and is stored as an element record like any other (the
//! records in the database are the truth). Map: projects/spare-construct/FEATURES.md, parts
//! CON-A4-F1-P2 and P3; the stored forms are in FORMAT.md beside this crate.
//!
//! - [`Grid`]: a grid line. Its name is unique among the document's grids without regard to case
//!   ([`same_name`]); the next name is suggested by a stated rule ([`next_name`]: after `1`
//!   comes `2`, after `A` comes `B`, after `Z` comes `AA`). Its geometry is a [`Shape`]: a
//!   straight segment, or an arc (centre, radius, start and end angle, anticlockwise), all in
//!   exact grid steps ([`construct_params::Length`], [`construct_params::Angle`]). A grid is
//!   vertical and has no elevation of its own.
//! - [`ReferencePlane`]: a reference plane, with an optional name, unique when given: a
//!   [`PlaneShape::Vertical`] line in plan standing vertical, or a [`PlaneShape::Horizontal`]
//!   plane at an elevation. [`Datums::plane_named`] finds one by name, which families and
//!   sketches will snap to.
//! - [`Datums`]: a document's grids and planes, with the rules that need to see them all (names,
//!   IDs), the lookups by name, [`Datums::crossings_of`] and [`Datums::nearest_grid`].
//! - Pure functions on shapes ([`Shape`]): [`Shape::length`], [`Shape::moved`],
//!   [`Shape::with_end`] and [`Shape::extended`] (a line's ends), [`Shape::with_angle`] and
//!   [`Shape::swept`] (an arc's), [`Shape::polyline`] (the outline a view draws); [`crossings`]
//!   (where two grid lines cross: exact where both are straight and the answer falls on a step,
//!   else the nearest step with a stated bound; none if they are parallel or apart) and
//!   [`nearest_on`].
//! - [`GridError`]: every refusal, as a value: a name that is empty, a duplicate name, a line of
//!   no length, an arc of no radius.
//!
//! Promises: the same data gives the same bytes on every machine (no clock, no random number, no
//! hash-map order; sine and cosine are the pure-Rust `libm`); every stored length and angle is
//! whole units, and a length is never held as a float or as millimetres; reading refuses what
//! writing would not give; nothing panics. Nothing here touches the edit log: construct-api's
//! `cloud::grids` is the commands, and construct-regen's `datum` is the display shape.
//! Depends on construct-model (the record), construct-categories (the two categories),
//! construct-params (`Length`, `Angle`, stored values), kit-core (IDs), `serde_json` and `libm`.
//! Nothing here is interim.
//!
//! Not built here, with their places kept:
//! - Which levels a grid or a plane shows on, and its extent per view (the length it is drawn
//!   to in each view): views and levels are not elements yet (jobs 3.1 and 5.x). Nothing is
//!   stored for either; a grid's record has no level, and a view will hold its own extents.
//! - A grid's 3D extent by level.
//! - Multi-segment grids: a grid is one segment or one arc; a chain would be a new recipe
//!   operation beside the two.
//! - Dimensions to grids (annotation jobs), which will read [`crossings`] and [`nearest_on`].

mod cross;
mod datum;
mod error;
mod name;
mod record;
mod set;
mod shape;

pub use cross::{Crossing, Foot, crossings, nearest_on};
pub use datum::{Grid, PlaneShape, ReferencePlane};
pub use error::GridError;
pub use name::{MAX_NAME_CHARS, clean_name, next_name, same_name, suggest};
pub use record::{
    CATEGORY_KEY, Datum, GRID_KIND, OP_GRID_ARC, OP_GRID_LINE, OP_PLANE_HORIZONTAL,
    OP_PLANE_VERTICAL, PLANE_KIND, datum_of_recipe, is_datum_kind, plane_recipe, shape_recipe,
};
pub use set::{Datums, Nearest};
pub use shape::{End, FULL_TURN, LIMIT, Point, Shape};
