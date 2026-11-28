//! Spare Construct: views as elements, and what each shows (CON-A8-F1-P1, CON-A8-F1-P2; the part
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
//! - [`ViewTemplate`], [`ViewTemplates`], [`Setting`], [`Settings`]: a saved set of view settings
//!   (scale, detail level, graphics rules, which categories show, filters, each with an "included"
//!   flag) that many views follow (CON-A8-F3-P4); a view is tied to one with [`View::template`],
//!   [`ViewTemplates::effective`] gives the views as they show, [`ViewTemplates::tied_refusal`] is
//!   the refusal of a tied view's own change to an included setting, and
//!   [`ViewTemplate::copy_into`] applies one once.
//! - [`Overrides`], [`ElementStyle`]: one element's look in one view, over its category's rule and
//!   its filters (CON-A8-F3-P6); [`adjust`] puts it over a [`Resolved`].
//! - [`Filters`], [`Filter`], [`Operator`]: rules on a category and a parameter with a look of
//!   their own, in the order that decides which wins ([`Filters::style_for`]).
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
//! - Crop regions (job 7.7, piece A79) and a plan's view range as a template setting: more fields
//!   on the view record under their own reserved keys, read in [`View::from_record`]; a template
//!   holds the five settings above until they land. [`resolve`] and [`adjust`] are what a drawing
//!   calls for how an element looks.
//! - Commands through the edit log: construct-api's `cloud::views` (a view's entry in
//!   `construct_categories::category_of_kind` and the level-deletion guard for the levels a range
//!   is tied to are done: construct-levels counts [`RANGE_KEYS`]).
//! - Datum extents per view, annotation owned by a view, and three-dimensional camera settings.

mod cut;
mod error;
mod filters;
mod graphics;
mod kind;
mod listing;
mod overrides;
mod range;
mod resolve;
mod settings;
mod shape;
mod style;
mod template;
mod view;
mod views;

pub use construct_levels::Tie;
pub use construct_params::Length;
pub use construct_styles::Scale;
pub use construct_styles::{Colour, Edge, PatternId};
pub use cut::{CutLine, Point, Side};
pub use error::ViewError;
pub use filters::{FILTERS_FORMAT_VERSION, FILTERS_KEY, Filter, Filters, Operator};
pub use graphics::{
    FILL_PATTERNS, Fill, GRAPHICS_FORMAT_VERSION, GRAPHICS_KEY, Graphics, MOST_TRANSPARENCY, Rule,
    RuleStyle, check_style, edge_from_key, edge_key,
};
pub use kind::{DetailLevel, PlanSetup, ViewKind};
pub use listing::{How, Listing, Model, Shown, ViewElement, list};
pub use overrides::{OVERRIDES_FORMAT_VERSION, OVERRIDES_KEY, Overrides};
pub use range::{Heights, ViewRange};
pub use resolve::{Origin, Resolved, adjust, resolve, resolve_in};
pub use settings::{HIDDEN_KEY, Setting, Settings};
pub use style::ElementStyle;
pub use template::{
    TEMPLATE_DETAIL_KEY, TEMPLATE_INCLUDED_KEY, TEMPLATE_KIND, TEMPLATE_SCALE_KEY, ViewTemplate,
    ViewTemplates, template_recipe,
};
pub use view::{
    CATEGORY_KEY, DETAIL_KEY, FAR_KEY, KIND_KEY, LINE_KEYS, LOOK_KEY, PHASING_KEY, PLAN_LEVEL_KEYS,
    RANGE_KEYS, SCALE_KEY, STAGING_KEY, TEMPLATE_TIE_KEY, VIEW_KIND, View, view_recipe,
};
pub use views::Views;
