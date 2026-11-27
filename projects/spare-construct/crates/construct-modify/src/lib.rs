//! Spare Construct: the modify tools (CON-A3-F4-P1, job 10.2, first half). Move, copy, rotate,
//! mirror and array elements, as pure functions over a document's records: what a person asks
//! for goes in, the records it writes come out, or a refusal in words naming the element. The
//! server's commands (`modify.*` in construct-api's `cloud::modify`) put them through the one
//! ordered edit log, so each is one entry and one undo step.
//!
//! What it promises:
//! - **Any angle, rounded once.** Every stored coordinate is a construct-params `Length` in grid
//!   steps, and every motion here is held as whole numbers of any size ([`exact`]): a translation,
//!   a turn about a point, a mirror about a line, or any of these in turn. A turn may be by any
//!   angle (15, 30, 37.5 degrees) and a radial array of any count. Quarter turns, turns between two
//!   directions whose lengths multiply to a whole number (3:4:5) and every mirror are exact; any
//!   other turn is held as the true rotation within 2^-159 of a radian of it ([`trig`]). Each point
//!   is then rounded once to the nearest grid step: at most half a step from the exact motion on
//!   each axis, and 2^-94 of a step (under 0.0001 mm, against the model's tolerance of 0.01 mm). No
//!   floating point is used. The rule and its bound are the decision note
//!   `projects/spare-construct/notes/CON-A3-F4-P1-modify-tools.md` (main's ruling of 28 Nov).
//! - **Consistent through the rounding.** Each point is rounded from where it is, by the one
//!   motion: a point two elements share (joined walls' common end) lands on one point; what a wall
//!   hosts sits at its offsets along it, so it stays exactly on its host; a radial array's copy is
//!   the turn of the originals by its own angle, never a turn of the copy before it, so nothing
//!   builds up. A turn and the turn back are exactly no turn, but the points between were rounded,
//!   so turning back by the same angle may differ from where they began by a step.
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
//!   one (by an angle or a sweep, any count), each copy a plain element worked out from the
//!   originals; an associated array, kept together and edited as one, is a later part of CON-A3-F4.
//! - **Refused by name only what cannot be built**: a point past the model's limit (2^40 - 1 steps,
//!   171.8 km), a block or an older box off the axes or off whole millimetres, an arc by a motion
//!   whose angles are no whole number of angle units, a hosted element moved off its host.
//!
//! What it does not do: build or check a shape. The server's command checks every record it
//! writes through the regeneration the model already uses (the builders' own readers and their
//! fit rules), so what is written is what is built.
//!
//! Depends on construct-model (records), construct-params (lengths, angles, stored text),
//! construct-regen (the hosting reader, a layered wall's line, a placed family's instance: the
//! modules the builders read them with), construct-families (a family's parameters), and the
//! stored forms of construct-slabs, construct-roofs, construct-rooms, construct-openings,
//! construct-grids and construct-walls; num-bigint for whole numbers of any size. Nothing here is
//! interim: the rule is the grid's, and the exact kernel takes the same points.

pub mod array;
pub mod error;
pub mod exact;
pub mod kinds;
pub mod select;
pub mod trig;

pub use error::ModifyError;
pub use exact::{Angles, Motion, Square, Steps, Turn};
pub use kinds::{Hand, Sense, hands_of};
pub use select::{Copies, MOST_MADE, Plan, Request, plan};
