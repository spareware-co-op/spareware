//! Spare Construct: views as elements, and what each shows (CON-A8-F1-P1, P2 and P3; the part
//! "view scale and detail level").
//!
//! A view is an element of the views category: a floor plan, a ceiling plan, a section, an
//! elevation, a three-dimensional view, a drafting view or a legend. It has a name, a scale (the
//! `n` of 1:n, construct-styles' [`Scale`]), a detail level, the data of its kind, and the phase and
//! design-option fields every element carries from day one. A view has no shape; what it shows is
//! asked of [`list`]. Map: projects/spare-construct/ARCHITECTURE.md, "Views"; the stored form is
//! in FORMAT.md beside this crate and the choices are in notes/CON-A8-F1-P1-views.md. The crate
//! is the home of the later view jobs too (graphics rules, crops, templates): they add to it.
//!
//! - [`View`]: its ID, name, scale, [`DetailLevel`], [`ViewKind`], staging and optional phase
//!   settings; written as an element record of kind [`VIEW_KIND`] in the views category and read
//!   back strictly; [`View::check`] asks whether it stands in a [`Model`]; [`View::pen_width`]
//!   picks a pen's printed width by the view's scale.
//! - [`ViewKind`]: the seven kinds, each holding only its own data: [`PlanSetup`] (a level and a
//!   [`ViewRange`]) for the two plans, a [`CutLine`] for a section or an elevation.
//! - [`ViewRange`], [`Heights`]: a plan's top, cut plane, bottom and view depth, each a level and an
//!   offset (construct-levels' [`Tie`]); [`ViewRange::resolve`] gives the heights, in order.
//! - [`CutLine`], [`Point`], [`Side`]: a section's or elevation's line, direction and depth.
//! - [`Views`]: a document's views by ID, names unique without regard to case.
//! - [`list`], [`Listing`], [`Shown`], [`How`], [`ViewElement`], [`Model`]: which elements a view
//!   shows and, for a plan, which the cut plane crosses and which are seen below it.
//! - [`Graphics`], [`Rule`], [`RuleStyle`], [`Fill`]: a view's graphics rules (CON-A8-F3-P2), the
//!   style of what is cut and of what is seen beyond, per category and subcategory; held by
//!   [`View::graphics`] and stored under [`GRAPHICS_KEY`].
//! - [`resolve`], [`Resolved`], [`Origin`]: the one rule for how an element looks in a view:
//!   the subcategory's rule, else the category's, else the project's object style.
//! - [`Crop`], [`Region`]: a view's crop region (CON-A8-F1-P3): a rectangle or a closed outline in
//!   the view's own coordinates, checked exactly to be simple, with an optional annotation region
//!   and on and shown switches; held by [`View::crop`] and stored under [`CROP_KEY`]. Stored and
//!   served only: the drawing does not clip to it yet.
//! - [`ViewError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log and the server's commands is a later job. The
//! same data gives the same bytes and the same answer on every machine and run (no clock, no
//! random number, no hash-map order); reading refuses anything writing would not give; no length
//! is rounded to whole millimetres or held as a float; nothing panics. Depends on construct-model
//! (the record and the box), construct-categories (the category and its kind), construct-levels
//! (levels and ties), construct-params (lengths, stored), construct-phases (staging and the
//! phase rule), construct-spatial (what is in a box), construct-styles (scale and pens) and
//! kit-core (IDs). Nothing here is interim.
//!
//! Not built here, with their places kept:
//! - Drawing a plan or a section from a view (jobs 7.2, 8.1): this crate says what a view shows,
//!   not how it is drawn. A section's height range is not held; it is every height.
//! - View templates and overrides (job 8.5): more fields on the view record under their own
//!   reserved keys, read in [`View::from_record`]. The plan drawing applying the graphics rules,
//!   and the page showing them, are later jobs: [`resolve`] is what they call. Clipping a drawing
//!   to a crop (job 7.7's next step) is the drawing's.
//! - Commands through the edit log: construct-api's `cloud::views` (a view's entry in
//!   `construct_categories::category_of_kind` and the level-deletion guard for the levels a range
//!   is tied to are done: construct-levels counts [`RANGE_KEYS`]).
//! - Datum extents per view, annotation owned by a view, and three-dimensional camera settings.

mod crop;
mod cut;
mod error;
mod graphics;
mod kind;
mod listing;
mod range;
mod resolve;
mod shape;
mod view;
mod views;

pub use construct_levels::Tie;
pub use construct_params::Length;
pub use construct_styles::Scale;
pub use construct_styles::{Colour, Edge, PatternId};
pub use crop::{CROP_FORMAT_VERSION, CROP_KEY, Crop, Region};
pub use cut::{CutLine, Point, Side};
pub use error::ViewError;
pub use graphics::{
    FILL_PATTERNS, Fill, GRAPHICS_FORMAT_VERSION, GRAPHICS_KEY, Graphics, MOST_TRANSPARENCY, Rule,
    RuleStyle, check_style, edge_from_key, edge_key,
};
pub use kind::{DetailLevel, PlanSetup, ViewKind};
pub use listing::{How, Listing, Model, Shown, ViewElement, list};
pub use range::{Heights, ViewRange};
pub use resolve::{Origin, Resolved, resolve};
pub use view::{
    CATEGORY_KEY, DETAIL_KEY, FAR_KEY, KIND_KEY, LINE_KEYS, LOOK_KEY, PHASING_KEY, PLAN_LEVEL_KEYS,
    RANGE_KEYS, SCALE_KEY, STAGING_KEY, VIEW_KIND, View, view_recipe,
};
pub use views::Views;
