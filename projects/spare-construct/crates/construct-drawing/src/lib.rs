//! Spare Construct: drawings as plain values, the cut of a plan, sections and elevations, and their
//! regions merged by material and hatched (CON-A8-F2-P1; note A12's build order, steps 2 to 5).
//!
//! What it is: the 2D drawing of a view, as exact values a person, a script or an AI can read, the
//! drawings made from a model (the cut of a plan, a section, an elevation), and a drawing made
//! ready to print.
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
//! - [`section`]: [`section::SectionView`], a section or an elevation on a line along a grid axis,
//!   its depth, and the map from its own frame (along the line, and up) to the drawing;
//!   [`section::part`], what one element gives it (its cut, where the view cuts, and the outline
//!   of what of it lies beyond the plane, to the depth); [`section::section`], the drawing of the
//!   parts, each outline less what a cut region or an element wholly nearer hides;
//!   [`section::draw`], both. Interim: outlines only, not the edges inside them (job 8.2), and
//!   elements whose depths overlap do not hide each other (job 9.1).
//! - [`datum`]: [`datum::marks`], the levels and grids a section or an elevation shows, as lines
//!   with their names ([`datum::Label`]) and a grid's bubble, inside the view's frame
//!   ([`datum::frame`]).
//! - [`merge`]: [`merge::merge`], cut regions of one category and material that touch joined into
//!   one [`merge::MergedRegion`], with no line between them; each outline edge keeps its source.
//! - [`hatch`]: [`hatch::hatch`], a region filled with a [`hatch::FillPattern`] (families of
//!   parallel lines at a printed spacing), its lines clipped exactly to the region, holes left
//!   unhatched, counted before they are made; [`hatch::FillPatterns::default_metric`], one pattern
//!   for each cut fill of the default materials.
//! - [`picture`]: [`picture::picture`], a drawing ready to print at one scale for any output:
//!   merged regions hatched in their material's cut pattern and outlined at their category's cut
//!   weight for the scale, a line two regions share drawn once; [`picture::Output`], the writer
//!   of a picture.
//! - [`look`]: [`look::Looks`], what answers for a view how a category looks on each edge (the
//!   server asks construct-views' `resolve`); with none the drawing is as it was.
//! - [`crop`]: [`crop::clip`], a picture cut exactly to a view's crop outline: every stroke and hatch
//!   line keeps the pieces inside it.
//! - [`svg`]: [`svg::Svg`], the SVG output, and [`svg::to_svg`], so a person can see a drawing;
//!   [`svg::Svg::labelled`] writes a section's datum names with it.
//!
//! Promises: everything is exact (whole grid steps of the kernel, `Q0`, and exact fractions of
//! them where a cut crosses a sloped edge, where edges of merged regions cross, and where hatch
//! lines meet a region's edge); no float is stored or compared; the same input gives the same
//! bytes on every machine; every refusal is a [`DrawingError`] value.
//!
//! Depends on: construct-categories, construct-styles, construct-materials, construct-params and
//! construct-model for what a line refers to; construct-regen's kernel builder for the solids and
//! their face names; engine-geometry's engine seam (`Engine::section`) for the cut; num-rational
//! and num-bigint for the overlay's exact fractions. Geometry only through the kernel's interface.
//!
//! Interim: Spare Construct's own drawing code, written to the shapes of the kernel's draw types
//! (chapter 18.2 and 18.8 of the geometry architecture: a view, cut curves keyed by element and
//! source, cut regions keyed by element and material) so that the kernel's own drawing engine,
//! `geom-draw` (pieces G387, G389 and G394 to G402), replaces [`plan`] under the same tests. The
//! cut itself is the engine seam's `section`, which geom-solidcore's engine answers today from the
//! kernel's own pieces (its interim register row names G389). A face is named through its plane
//! (its carrier), the name the seam gives a cut edge, until the seam carries the kernel's
//! persistent names (chapter 20). The union of merged regions and the clipping of hatch lines are
//! the crate's own exact 2D overlay, in one private module (`overlay`), which the kernel's 2D
//! booleans and hatches (`geom-2d`, pieces G196, G199, G200, G202 and G210) replace. What a
//! section or an elevation sees beyond its plane is the shadow of each solid, made in one private
//! module (`sweep`) from the seam's exact cuts between the depths of the solid's corners, since the
//! seam has no projection; the kernel's view predicates, sections and hidden lines (G388, G389 and
//! G394) replace it. The method and
//! the order of the steps are note A12's (`projects/spare-construct/notes/A12-hidden-lines.md`);
//! the decisions of this crate are note CON-A8-F2-P1
//! (`projects/spare-construct/notes/CON-A8-F2-P1-drawing-types.md`).

pub mod crop;
pub mod datum;
pub mod drawing;
pub mod error;
pub mod hatch;
pub mod look;
pub mod merge;
mod overlay;
pub mod pens;
pub mod picture;
pub mod plan;
pub mod section;
pub mod svg;
mod sweep;
pub mod view;

pub use error::DrawingError;
