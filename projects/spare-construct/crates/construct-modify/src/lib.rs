//! Spare Construct: the modify tools (CON-A3-F4-P1, job 10.2, first half). Move, copy, rotate,
//! mirror and array elements, as pure functions over a document's records: what a person asks
//! for goes in, the records it writes come out, or a refusal in words naming the element. The
//! server's commands (`modify.*` in construct-api's `cloud::modify`) put them through the one
//! ordered edit log, so each is one entry and one undo step.
//!
//! What it promises:
//! - **Exact.** Every stored coordinate is a construct-params `Length` in grid steps, and every
//!   motion here is held as whole numbers ([`exact`]): a translation, a turn about a point, a mirror
//!   about a line, or any of these in turn. A point a motion would put between grid steps is
//!   refused, naming the element, never rounded. Quarter turns and mirrors about lines at
//!   multiples of 45 degrees always land; why no other angle given in degrees can, and which
//!   turns between two directions do, is the decision note
//!   `projects/spare-construct/notes/CON-A3-F4-P1-modify-tools.md`.
//! - **Each kind its own way** ([`kinds`]): a layered wall's line, a floor's boundary, a roof's
//!   footprint, a room's point, a shaft's or floor opening's outline, a grid's or reference plane's
//!   geometry, a block's box, each read and written by the crate that owns its stored form, so a
//!   moved record reads exactly as one drawn there would. A mirrored wall keeps its exterior where
//!   the mirror puts it (its `wall.flipped` turns over); a mirrored door changes hand.
//! - **Hosted elements follow their host** ([`select`]): a door, a window or an opening moves,
//!   turns and mirrors with its wall, and a floor opening with its floor; one moved alone slides
//!   along its host or is refused by name. Copies get new IDs from the key their command names,
//!   keep their types and parameters, and a copy's hosted elements are hosted on the copy.
//! - **Arrays** ([`array`]): the motions of a linear array (by a spacing or a span) and a radial
//!   one (by an angle or a sweep), each copy a plain element; an associated array, kept together
//!   and edited as one, is a later part of CON-A3-F4.
//!
//! What it does not do: build or check a shape. The server's command checks every record it
//! writes through the regeneration the model already uses (the builders' own readers and their
//! fit rules), so what is written is what is built.
//!
//! Depends on construct-model (records), construct-params (lengths, angles, stored text),
//! construct-regen (the hosting reader, a layered wall's line, a placed family's instance: the
//! modules the builders read them with), construct-families (a family's parameters), and the
//! stored forms of construct-slabs, construct-roofs, construct-rooms, construct-openings,
//! construct-grids and construct-walls. Nothing here is interim: the rule is the grid's, and the
//! exact kernel takes the same points.

pub mod array;
pub mod error;
pub mod exact;
pub mod kinds;
pub mod select;

pub use error::ModifyError;
pub use exact::{Motion, Square, Steps, Turn};
pub use kinds::{Hand, Sense, hands_of};
pub use select::{Copies, MOST_MADE, Plan, Request, plan};
