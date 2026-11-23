//! Spare Construct: drawings as plain values, and the cut of a plan (CON-A8-F2-P1, first steps;
//! note A12's build order, steps 2 and 3).
//!
//! What it is: the 2D drawing of a view, as exact values a person, a script or an AI can read, and
//! the first drawing made from a model, the cut of a plan.
//!
//! - [`view`]: [`view::View`], a view along a grid axis (a plan looking down, sections and
//!   elevations looking north, south, east or west), the plane that cuts it, and the exact map from
//!   a model point to a view point.
//! - [`drawing`]: [`drawing::Drawing`], line segments and arcs in view coordinates, each
//!   [`drawing::Line`] knowing where it came from (the element, the face or edge by its persistent
//!   name), whether it is cut or seen in projection, and its category; closed cut
//!   [`drawing::Region`]s with their material; one canonical order, and the stored form
//!   ([`drawing::Drawing::to_bytes`]): the same model and view give the same bytes.
//! - [`pens`]: the printed width of every line at a view scale, looked up in construct-styles'
//!   object styles and pens ([`pens::widths`]).
//! - [`plan`]: [`plan::plan`], a plan from built elements: what the view's plane cuts, each
//!   element's cut loops as cut lines named by face and its cut region with its material. Nothing
//!   seen below the plane yet, and no hidden lines.
//! - [`svg`]: [`svg::to_svg`], one small exporter, so a person can see a drawing.
//!
//! Promises: everything is exact (whole grid steps of the kernel, `Q0`, and exact fractions of
//! them where a cut crosses a sloped edge); no float is stored or compared; the same input gives
//! the same bytes on every machine; every refusal is a [`DrawingError`] value.
//!
//! Depends on: construct-categories, construct-styles, construct-materials, construct-params and
//! construct-model for what a line refers to; construct-regen's kernel builder for the solids and
//! their face names; engine-geometry's engine seam (`Engine::section`) for the cut. Geometry only
//! through the kernel's interface.
//!
//! Interim: Spare Construct's own drawing code, written to the shapes of the kernel's draw types
//! (chapter 18.2 and 18.8 of the geometry architecture: a view, cut curves keyed by element and
//! source, cut regions keyed by element and material) so that the kernel's own drawing engine,
//! `geom-draw` (pieces G387, G389 and G394 to G402), replaces [`plan`] under the same tests. The
//! cut itself is the engine seam's `section`, which geom-solidcore's engine answers today from the
//! kernel's own pieces (its interim register row names G389). A face is named through its plane
//! (its carrier), the name the seam gives a cut edge, until the seam carries the kernel's
//! persistent names (chapter 20). The method and the order of the steps are note A12's
//! (`projects/spare-construct/notes/A12-hidden-lines.md`); the decisions of this crate are note
//! CON-A8-F2-P1 (`projects/spare-construct/notes/CON-A8-F2-P1-drawing-types.md`).

pub mod drawing;
pub mod error;
pub mod pens;
pub mod plan;
pub mod svg;
pub mod view;

pub use error::DrawingError;
