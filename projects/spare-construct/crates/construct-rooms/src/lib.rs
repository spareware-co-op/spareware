//! Spare Construct: rooms found from a closed loop of walls, the shape half (CON-A4-F7-P1, first
//! half).
//!
//! What a room is, as data and exact plane geometry: a point on a level, a name and a number, an
//! upper limit (the level above, or a given height); and, found from the walls of its level, its
//! boundary (by each wall's finish face toward the room), its holes (island walls, columns), and
//! what it measures exactly: area, perimeter and volume. A room whose loop is broken stays a
//! record and is reported not enclosed; a second room in one loop is reported redundant. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Building elements" (rooms); the decisions are in
//! notes/CON-A4-F7-P1-rooms.md and the stored form in FORMAT.md beside this crate.
//!
//! - [`plane`]: [`Bound`] and [`Outline`] (a wall's outline in plan), [`find`] (the loop round a
//!   point, exactly), [`Found`] and [`Corner`], [`LIMIT`]. The search is our own exact geometry;
//!   its header says why.
//! - [`measure`]: [`Measures`] and [`measure`](measure::measure); [`Outcome`] and [`assess`],
//!   with [`Neighbour`] for the other rooms of a level.
//! - [`room`]: the element record and its recipe ([`room_record`], [`room_recipe`], [`Limit`],
//!   [`at_of`], [`limit_of`], [`number_of`], [`next_number`], [`walls_arg`], [`reads_walls`]) and
//!   the recipe the builders take ([`filled`]).
//! - [`error`]: [`RoomError`], every refusal as a value, with the code a build failure is filed
//!   under.
//!
//! Promises: every result is exact on the kernel's grid (corners are grid points, areas whole
//! square steps, volumes whole cubic steps); nothing is rounded but each perimeter edge, to the
//! nearest step, as a floor's; no float is stored or computed; the same walls and point give the
//! same answer and bytes; nothing panics. Depends on construct-model, construct-levels,
//! construct-params, construct-categories, construct-types, kit-core and dashu (exact rationals).
//! Nothing here builds a shape: construct-regen's `rooms` module reads the walls and fills the
//! recipe, and its builders make the room's volume through the kernel's interface.
//!
//! Interim: curved walls ([`RoomError::CurvedWall`]) and sloped upper limits
//! ([`RoomError::SlopedLimit`]) are refused by name until the kernel's exact arcs and sloped
//! bounds reach rooms; both are rows of the interim register. Not here, with their places kept:
//! the commands that place, rename, number, move and delete rooms (construct-api), room
//! separation lines and rooms bounded by floors and ceilings (job 6.3), and wall joins changing a
//! room's faces: they change only the outlines the regenerator hands in (construct-regen's
//! `rooms` hands each wall's outline as its joints shape it), and the search here takes any
//! polygon, at any angle.

pub mod error;
mod exact;
pub mod measure;
pub mod plane;
pub mod room;

pub use error::RoomError;
pub use measure::{Measures, Neighbour, Outcome, assess, measure};
pub use plane::{Bound, Corner, Found, LIMIT, Outline, find};
pub use room::{
    AT_ARG, HEIGHT_ARG, LOOPS_ARG, Limit, NUMBER_KEY, OP_VERSION, ROOM_KIND, ROOM_OP, WALLS_ARG,
    at_of, filled, is_room, is_room_recipe, limit_of, next_number, number_of, reads_walls,
    room_recipe, room_record, walls_arg,
};
