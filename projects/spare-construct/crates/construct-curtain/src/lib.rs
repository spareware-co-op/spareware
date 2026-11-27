//! Spare Construct: curtain walls, the data and shape half (CON-A4-F6-P1, first part; job 14.1,
//! piece A84).
//!
//! What a curtain wall is, as data: a wall of its own kind on a straight line between levels,
//! divided by a grid of vertical and horizontal lines into cells, each cell holding a panel
//! (glazed, solid, empty, or a door of the family library), with mullions standing on the grid
//! lines and the borders. Its type says the grid (in each direction none, a fixed spacing with a
//! justification, a fixed count or a most spacing) and the parts (the panel, the four kinds of
//! mullion, which mullion runs through where two cross); each wall adds or removes grid lines one
//! by one, leaves segments of lines out and swaps panels cell by cell. From these, exactly, where
//! every line, panel and mullion stands, every solid as a box in the wall's own frame, and what a
//! wall reads back. Map: projects/spare-construct/ARCHITECTURE.md, "Building elements"; the
//! stored form is FORMAT.md beside this crate, the decisions
//! notes/CON-A4-F6-P1-curtain-walls.md.
//!
//! - [`grid`]: [`Direction`], [`Justify`] and [`Layout`], the lines a layout puts on an extent.
//! - [`spec`]: [`CurtainSpec`], a type's grid and parts: [`Panel`] ([`Infill`], [`DoorPanel`],
//!   [`PanelKind`]), [`Mullions`] of [`Profile`]s, and [`Crossing`].
//! - [`edits`]: [`Edits`], a wall's own changes, each by where it stands ([`Segment`]).
//! - [`layout`]: [`Curtain`] and its [`Grid`] ([`GridLine`], [`PanelPlace`], [`MullionPlace`]),
//!   its [`Solid`]s ([`FrameBox`], [`Part`]) and [`DoorPlace`]s, and the pure edits
//!   ([`Curtain::with_line`], [`Curtain::without_line`], [`Curtain::with_segment_omitted`],
//!   [`Curtain::with_segment_restored`], [`Curtain::with_panel`]), each a valid new wall or the
//!   refusal.
//! - [`measure`]: [`quantities`] ([`Quantities`]) and [`read_only`], the parameters a schedule
//!   lists.
//! - [`stored`]: the type's block text under [`OWNER`] ([`CurtainSpec::to_text`],
//!   [`CurtainSpec::from_text`]).
//! - [`curtain_type`]: [`new_curtain_type`], [`with_spec`], [`spec_of`].
//! - [`record`]: the recipe ([`curtain_recipe`], [`embedded_recipe`], [`filled`], [`with_line`],
//!   [`with_edits`], [`edits_of`], [`Filled`]).
//! - [`defaults`]: the default set of types, [`default_curtains`].
//! - [`error`]: [`CurtainError`], every refusal as a value.
//!
//! Promises: every length is construct-params' `Length` in the kernel's grid steps, worked only in
//! the private `length` module; the only roundings are a layout's equal divisions and a line at an
//! angle's length, each the nearest step once; no float is stored or computed; areas are exact;
//! the same data gives the same bytes; reading refuses anything writing would not give; nothing
//! panics. Nothing here builds a shape: construct-regen's builders read [`Grid::solids`] and
//! [`Grid::doors`], put each box on the kernel's grid in plan and make it through the kernel's
//! interface. Depends on construct-types, construct-params, construct-categories,
//! construct-materials, construct-families (a door panel's family and type), construct-model (the
//! recipe), kit-core, serde and `serde_json`. Nothing here is interim.
//!
//! Not here, each with its place: curved curtain walls (job 14.4, with curved walls); curtain
//! systems on faces and sloped glazing (CON-A4-F6-P2); a mullion profile other than a rectangle (a
//! new [`Profile`] shape, a new stored version). The page's curtain wall tool and its drawing are
//! in `app/connect.js` (the second half of piece A84).

pub mod curtain_type;
pub mod defaults;
pub mod edits;
pub mod error;
pub mod grid;
pub mod layout;
mod length;
pub mod measure;
pub mod record;
pub mod spec;
pub mod stored;

pub use curtain_type::{new_curtain_type, spec_of, with_spec};
pub use defaults::{DefaultCurtain, default_curtains};
pub use edits::{Edits, Segment};
pub use error::CurtainError;
pub use grid::{Direction, Justify, Layout, MOST_COUNT, MOST_LINES};
pub use layout::{
    Curtain, DoorPlace, FrameBox, Grid, GridLine, MOST_CELLS, MOST_EXTENT, MullionPlace,
    PanelPlace, Part, SegmentState, Solid,
};
pub use length::TOLERANCE;
pub use measure::{
    AREA, GLAZED_AREA, HEIGHT, LENGTH, MULLION_LENGTH, MULLIONS, PANEL_AREA, PANELS, Quantities,
    quantities, read_only,
};
pub use record::{
    CURTAIN_KIND, END_ARG, Filled, GRID_ARG, HEIGHT_ARG, LENGTH_ARG, OP, SPEC_ARG, START_ARG,
    curtain_recipe, edits_of, embedded_length, embedded_recipe, filled, height_of, is_curtain,
    is_embedded, line_length, reads_type, type_arg, with_edits, with_line,
};
pub use spec::{
    Crossing, CurtainSpec, DoorPanel, Infill, MOST_SIZE, Mullions, Panel, PanelKind, Profile,
};
pub use stored::{FORMAT_VERSION, OWNER};
