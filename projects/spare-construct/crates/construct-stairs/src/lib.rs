//! Spare Construct: stairs laid out by rules, the data and shape half (CON-A4-F5-P1 first half,
//! CON-A4-F5-P2; job 6.7).
//!
//! What a stair is, as data: straight runs with landings between them (straight on, or turning a
//! quarter or a half to make an L or a U), its width and the way it runs, from a base level to a
//! top level; the limits of a country's code it is laid out by, held as data with each clause
//! cited; its layout worked out exactly from the height between its levels; one solid for each
//! run and each landing on the kernel's grid; and what it reads back as parameters. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Building elements" (stairs and railings); the
//! stored form is in FORMAT.md beside this crate, the decisions in
//! notes/CON-A4-F5-P1-straight-stairs.md.
//!
//! - [`rules`]: [`RuleSet`] (the limits of a code, read from JSON), [`Category`],
//!   [`LengthLimit`], [`PitchLimit`]; New Zealand's clause D1 through D1/AS1 is built in
//!   ([`NZ_D1_AS1`], the file `rules/nz-d1-as1.json`).
//! - [`stair`]: [`Stair`] and its [`Settings`]: the rule set and category, start, [`Direction`],
//!   width, going, waist, runs ([`RunSpec`]) and landings ([`LandingSpec`], [`Turn`]); every check
//!   of its own shape, and its stored arguments.
//! - [`layout`]: [`layout`] and [`layout_with`] give a stair's [`Layout`] for a rise: the count of
//!   risers and each one's height, the going, each run ([`RunLayout`]) and landing
//!   ([`LandingLayout`]), the total run; a rise no count lays out within the limits is refused
//!   naming the limits and the nearest heights that work.
//! - [`shape`]: [`shape`] gives a stair's [`Shape`]: each [`Piece`] ([`PieceKind`]) with its plan
//!   outline and its [`Solid`] (a run lofted across its width, a landing extruded up), and the
//!   headroom where one piece passes over another.
//! - [`measure`]: [`read_only`]: the rise, risers, riser height, treads, going, total run, pitch
//!   and width as read-only parameters.
//! - [`record`]: the element record and its recipe ([`stair_record`], [`stair_recipe`],
//!   [`stair_of`], [`rise_of`], [`laid_out`], [`with_stair`], [`filled`]).
//! - [`error`]: [`StairError`], every refusal as a value.
//!
//! Promises: every [`Stair`] has passed every check of its own shape, and a layout every limit of
//! its rule set; every length is a whole number of construct-params' grid steps and every corner
//! a point of the kernel's grid, each rounded at most once by the rule its module names; a rise
//! that does not divide evenly gives risers one grid step apart, never a rounded riser; the same
//! data gives the same bytes; reading refuses anything writing would not give; nothing panics. No
//! float is stored or computed here (a pitch limit in degrees is worked as a slope by
//! construct-roofs' one rule). Depends on construct-categories, construct-levels, construct-model,
//! construct-params, construct-roofs, construct-types and kit-core.
//!
//! Not here, each refused in words or left for its job: runs at angles off the plan's axes;
//! winders and curved or spiral stairs; nosing projections (the going and the tread depth are one
//! length); stringers and other supports; railings (job 14.2) and ramps (job 14.3), which build on
//! this crate; the headroom to other elements above (a floor's edge, a beam), checked when stairs
//! meet the floors they pass through. The commands that make stairs are construct-api's
//! `cloud::stairs`, the regenerator fills and builds them (construct-regen's `stairs`,
//! `stair_plan` and the kernel's `stair`), and the page's stair tool is the next lane's. Nothing
//! here is interim.

pub mod error;
pub mod layout;
pub mod measure;
pub mod record;
pub mod rules;
pub mod shape;
pub mod stair;

pub use error::{Named, NoCount, RiserCount, StairError};
pub use layout::{LandingLayout, Layout, RunLayout, layout, layout_with};
pub use measure::{GOING, PITCH, RISE, RISER_HEIGHT, RISERS, RUN_LENGTH, TREADS, WIDTH, read_only};
pub use record::{
    MATERIAL_ARG, OP_VERSION, PIECES_ARG, RISE_ARG, STAIR_KIND, STAIR_OP, filled, is_stair,
    is_stair_recipe, laid_out, rise_of, stair_of, stair_recipe, stair_record, with_stair,
};
pub use rules::{Category, LengthLimit, NZ_D1_AS1, PitchLimit, RULES_VERSION, RuleSet};
pub use shape::{MATERIAL, Piece, PieceKind, Shape, Solid, shape};
pub use stair::{
    Direction, GRID_LIMIT, LandingSpec, MOST_LENGTH, MOST_RISERS, MOST_RUNS, RunSpec, Settings,
    Stair, Turn,
};
