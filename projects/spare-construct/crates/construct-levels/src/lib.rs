//! Spare Construct: levels, the datum everything in a building hangs from (CON-A4-F1-P1).
//!
//! A level is a named horizontal datum at an elevation above the project's zero: a floor, the top
//! of a slab, a ceiling line, the site. A level that is a building storey is a slice of the
//! building (ISO 16739-1's building storey), and one that is not is a reference datum. Elements
//! are tied to a level, not placed at a height: an element's base is its level's elevation plus
//! its own offset, so moving a level moves everything tied to it and no element record changes.
//! Map: projects/spare-construct/ARCHITECTURE.md, "Levels and grids"; the stored form is in
//! FORMAT.md beside this crate.
//!
//! - [`Level`]: a level's permanent ID, name, elevation (a construct-params [`Length`], negative
//!   allowed) and storey flag; written as an element record of kind [`LEVEL_KIND`] in the
//!   levels category and read back strictly.
//! - [`Levels`]: a document's levels by ID: names unique without regard to case, ordered by
//!   elevation, then name, then ID; two levels at one elevation allowed, at most one of them a
//!   storey; add, rename, move, flag and remove (refused while anything is tied to it).
//! - [`Tie`], [`base_of`], [`top_of`], [`write_base`], [`write_top`]: how an element names the
//!   level it stands on and, for a wall, the level its top is tied to, each with an offset, in
//!   its record's `props`.
//! - [`height_of`]: the one function that says how high something tied to a level sits;
//!   [`span`]: the height between two ties, through it.
//! - [`implied_id`], [`implied`]: the level an old record's integer `level` names. A document
//!   written before levels were elements reads with levels made by this rule, the same on every
//!   replay; nothing is written for them until a command changes one.
//! - [`parse_length`], [`show`]: an elevation or offset typed and shown in the project's units.
//! - [`LevelError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log is construct-api's `cloud::levels`, and reading
//! levels while building is construct-regen's `levels`. The same data gives the same bytes;
//! reading refuses anything writing would not give; nothing panics. Depends on construct-model
//! (the record), construct-categories (the levels category), construct-params (lengths, stored
//! and typed) and kit-core (permanent IDs). Nothing here is interim.
//!
//! Not built here, with their places kept:
//! - Building ownership (the part's "building" clause): a level of one building of a site. Its
//!   place is one more reserved `props` key on the level record naming the building element,
//!   read in [`Level::from_record`]; a site's levels are then grouped by it. No building element
//!   exists yet.
//! - Grids and reference planes (job 3.2): datums of their own categories in their own crate
//!   (construct-grids), with records shaped as a level's: a reserved-key block in `props`, no
//!   shape, read strictly. [`Tie`] is the form a datum tie takes there too.
//! - Datum extents and heads per view (CON-A4-F1, views job 4.5): view data, not the level's.

mod error;
mod implied;
mod level;
mod levels;
mod tie;

pub use construct_params::Length;
pub use error::LevelError;
pub use implied::{STOREY_STEP, implied, implied_id, implied_number};
pub use level::{
    CATEGORY_KEY, ELEVATION_KEY, LEVEL_KIND, Level, STOREY_KEY, level_recipe, parse_length, show,
};
pub use levels::{Levels, tied_to};
pub use tie::{
    LEVEL_KEY, LEVEL_OFFSET_KEY, TIE_KEYS, TOP_KEY, TOP_OFFSET_KEY, Tie, base_of, height_of,
    is_tie_key, span, top_of, write_base, write_top,
};
