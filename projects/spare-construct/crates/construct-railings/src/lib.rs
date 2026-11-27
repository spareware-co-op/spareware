//! Spare Construct: railings by a sketched path or on a stair, the data and shape half
//! (CON-A4-F5-P3 first half; job 14.2).
//!
//! What a railing is, as data: a path (straight segments sketched on a level, or a stair it
//! follows, on one side or both, along each run's pitch and round its landings); a type (a top
//! rail and an optional handrail by profile and height, balusters by spacing, justification and
//! profile, posts at the start, the end and every corner, and a material); the limits of a
//! country's code it is checked by, as data with each clause cited (barrier heights and the
//! openings a sphere must not pass, F4/AS1; handrail heights, D1/AS1); its layout worked out
//! exactly; one solid for every post, baluster and piece of rail on the kernel's grid; what it
//! reads back as parameters; and the warnings a railing that breaks a limit gives. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Building elements" (stairs and railings); the
//! stored form is in FORMAT.md beside this crate, the decisions in
//! notes/CON-A4-F5-P3-railings.md.
//!
//! - [`rules`]: [`RuleSet`] (the limits of a code, read from JSON), [`Category`], [`Openings`],
//!   [`OpeningLimits`], [`LengthLimit`]; New Zealand's clause F4 through F4/AS1 is built in
//!   ([`NZ_F4_AS1`], the file `rules/nz-f4-as1.json`).
//! - [`kind`]: [`RailingType`] and its parts ([`Rail`], [`Handrail`], [`Balusters`],
//!   [`Profile`], [`Justify`]), every check of its own shape, stored and patched from typed text.
//! - [`path`]: [`Path`] (sketched, or [`Side`]s of a stair), and the [`Line`]s that come of it
//!   ([`Vertex`], [`Flight`], [`Hand`]): [`sketched_line`], [`stair_lines`].
//! - [`railing`]: [`Railing`] and its [`Settings`]; [`StairHost`], the stair a railing follows.
//! - [`layout`]: [`lay_out`] gives a railing's [`Layout`]: every baluster ([`Baluster`]), every
//!   piece of line ([`Segment`]) with its rail length and its openings ([`Opening`]), its
//!   handrail, and its counts.
//! - [`shape`]: [`shape`] gives every [`Piece`] ([`Part`]), each a flat profile swept along a
//!   vector.
//! - [`warning`]: [`warnings`], each a [`Warning`] by name ([`LOW`], [`OPENING`],
//!   [`HANDRAIL`]), in construct-warnings' shape behind our own type.
//! - [`measure`]: [`read_only`]: rail and handrail lengths, balusters, posts and height as
//!   read-only parameters.
//! - [`record`]: the element record and its recipe ([`railing_record`], [`railing_recipe`],
//!   [`railing_of`], [`laid_out`], [`filled`], [`with_railing`]).
//! - [`error`]: [`RailingError`], every refusal as a value.
//!
//! Promises: every [`Railing`] has passed every check of its own shape; every length is a whole
//! number of construct-params' grid steps and every point a point of the kernel's grid, worked out
//! exactly and rounded once to the nearest step, a half away from zero (main's any-angle rule, in
//! [`exact`]); a limit broken is a warning, never a refusal; the same data gives the same bytes;
//! reading refuses anything writing would not give; nothing panics. No float is stored or
//! computed here. Depends on construct-categories, construct-levels, construct-materials,
//! construct-model, construct-params, construct-stairs, construct-types, kit-core and num-bigint.
//!
//! Not here, each refused in words or left for its job: arcs in a path (the sketch solver has
//! them, but the engine's extrusion takes polygons: curved railings come after curved walls, job
//! 14.4); handrail extensions past the ends and rails that turn down; infill panels; a railing
//! type stored once as a type record shared by many railings; railings on ramps (job 14.3). The
//! commands that make railings are construct-api's `cloud::railings`, the regenerator fills and
//! builds them (construct-regen's `railings`, `railing_plan` and the kernel's `railing`), and the
//! page's railing tool is the next lane's. Interim: a round profile is a twelve-sided polygon
//! ([`kind`], [`shape`]), until the kernel's own swept circle answers behind the same pieces.

pub mod error;
mod exact;
pub mod kind;
pub mod layout;
pub mod measure;
pub mod path;
pub mod railing;
pub mod record;
pub mod rules;
pub mod shape;
pub mod warning;

pub use error::RailingError;
pub use kind::{
    Balusters, Handrail, Justify, MOST_HEIGHT, MOST_OFFSET, MOST_PROFILE, MOST_SPACING, Profile,
    Rail, RailingType, STANDARD_MATERIAL,
};
pub use layout::{
    Baluster, Between, Layout, LineLayout, MOST_BALUSTERS, Opening, Segment, lay_out,
};
pub use measure::{BALUSTERS, HANDRAIL_LENGTH, HEIGHT, POSTS, RAIL_LENGTH, read_only};
pub use path::{
    Flight, GRID_LIMIT, Hand, Line, MOST_INSET, MOST_POINTS, MOST_SEGMENT, Path, Side, Vertex,
    check_points, sketched_line, stair_lines,
};
pub use railing::{Railing, Settings, StairHost};
pub use record::{
    MATERIAL_ARG, OP_VERSION, PIECES_ARG, RAILING_KIND, RAILING_OP, filled, is_railing,
    is_railing_recipe, laid_out, railing_of, railing_recipe, railing_record, stair_of,
    with_railing,
};
pub use rules::{
    Category, LengthLimit, NZ_F4_AS1, OpeningLimits, Openings, RULES_VERSION, RuleSet,
};
pub use shape::{Part, Piece, shape};
pub use warning::{HANDRAIL, LOW, OPENING, Severity, Warning, warnings};
