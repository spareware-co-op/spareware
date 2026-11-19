//! The drafting entity model: every entity of chapter 14.1 as a closed enum with a stable tag.
//!
//! What it does: [`Shape`] is the closed list of what a drawing holds: points, lines, rays and
//! infinite lines, circles and arcs, ellipses and elliptical arcs, splines, polylines with bulges,
//! regions, hatches, text, dimensions and leaders, blocks and inserts, and viewports. An [`Entity`]
//! is a [`Shape`] with an [`Attr`], the program's opaque attribute key (layer, colour, weight and
//! style are program data), which every operation passes through untouched. Each shape has a
//! [`Tag`], a number that is written into files and never reused. A region is a planar Sheet body of
//! `geom-topo`. [`Shape::under`] says what a shape becomes under a transform (chapter 14.10): a
//! circle under a non-uniform scale is an ellipse, a mirror reverses an arc and negates a bulge.
//!
//! Guarantees:
//!
//! - **Closed.** [`Tag`] and [`Shape`] list every entity of 14.1 and nothing else; a tag read from a
//!   file that names none is refused ([`Tag::from_u8`]), never guessed.
//! - **Stable.** A tag's number is part of the native format. A new entity takes a new number.
//! - **Exact.** A stored number is a whole number of grid steps or a [`Frac`], a fraction of
//!   bounded size ([`FRAC_LIMIT`]), reduced, with a positive denominator. Nothing is a double; a
//!   bulge is the fraction tan(sweep / 4) and an ellipse's end is a fraction, not a polar angle.
//! - **Attribute pass-through.** [`Entity::under`] returns the same [`Attr`] it was given, whatever
//!   the shape becomes.
//! - **Valid by construction.** The constructors refuse a polyline with the wrong number of bulges, a
//!   spline whose knots do not fit its control points, a region that is not a Sheet, a zero text
//!   direction and the like, with a typed [`EntityError`].
//! - **Kind changes are written down.** [`Shape::under`] never loses a shape silently: a polyline
//!   with a bulge under a non-uniform scale becomes a run of lines and elliptical arcs
//!   ([`Becomes::Run`]), and a degenerate transform is refused.
//!
//! Method: the entity table of chapter 14.1 and the transform rules of 14.10
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`), in our own words from the public
//! DXF reference (`projects/kit/notes/G01-evidence/06-2d-drafting.md`, section 1).
//!
//! Cost: constructors are linear in what they check; [`Shape::tag`] and [`Shape::under`] are
//! constant time.

use alloc::string::String;
use alloc::vec::Vec;

use geom_math::{Box2, Classification, TransformKind, TypedTransform};
use geom_num::conic::Ellipse2;
use geom_num::grid::{P2, Steps};
use geom_topo::{Body, BodyKind};

use crate::carrier::{Circle2, Line2};
use crate::point::Pt2;

/// A stored fraction has numerator and denominator below this: 2^32.
pub const FRAC_LIMIT: i64 = 1 << 32;

/// Why an entity could not be made or changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityError {
    /// A fraction with a zero denominator, or one of a size past [`FRAC_LIMIT`].
    Fraction,
    /// A polyline with too few vertices, or with a bulge or width count that is not its segment
    /// count.
    Polyline,
    /// A spline whose degree, control points, weights or knots do not fit each other.
    Spline,
    /// A region whose body is not a Sheet.
    NotASheet,
    /// A text height that is not positive, or a direction that is the zero vector.
    Text,
    /// A hatch with no boundary.
    Hatch,
    /// A dimension with no reference, or a leader with fewer than two vertices.
    Annotation,
    /// An array with no rows or no columns.
    Array,
    /// A viewport scale that is not positive.
    Viewport,
    /// A transform that collapses space: no entity has an image.
    Degenerate,
}

/// A fraction of bounded size, reduced, with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frac {
    n: i64,
    d: i64,
}

/// The greatest common divisor, by Euclid's algorithm.
const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let Some(r) = a.checked_rem(b) else {
            return 0;
        };
        a = b;
        b = r;
    }
    a
}

impl Frac {
    /// The fraction `n / d`, reduced.
    ///
    /// # Errors
    ///
    /// [`EntityError::Fraction`] if `d` is zero or the reduced numerator or denominator is past
    /// [`FRAC_LIMIT`] in size.
    pub fn new(n: i64, d: i64) -> Result<Frac, EntityError> {
        if d == 0 {
            return Err(EntityError::Fraction);
        }
        let negative = (n < 0) != (d < 0);
        let (an, ad) = (n.unsigned_abs(), d.unsigned_abs());
        let g = gcd(an, ad);
        let (rn, rd) = (an.checked_div(g), ad.checked_div(g));
        let (Some(rn), Some(rd)) = (rn, rd) else {
            return Err(EntityError::Fraction);
        };
        let limit = FRAC_LIMIT.unsigned_abs();
        if rn >= limit || rd >= limit {
            return Err(EntityError::Fraction);
        }
        let n = i64::try_from(rn).map_err(|_| EntityError::Fraction)?;
        let d = i64::try_from(rd).map_err(|_| EntityError::Fraction)?;
        Ok(Frac {
            n: if negative { n.saturating_neg() } else { n },
            d,
        })
    }

    /// The whole number `n`.
    ///
    /// # Errors
    ///
    /// As for [`Frac::new`].
    pub fn whole(n: i64) -> Result<Frac, EntityError> {
        Frac::new(n, 1)
    }

    /// The numerator.
    #[must_use]
    pub const fn numerator(self) -> i64 {
        self.n
    }

    /// The denominator, positive.
    #[must_use]
    pub const fn denominator(self) -> i64 {
        self.d
    }

    /// Whether it is below zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.n < 0
    }

    /// Whether it is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.n == 0
    }

    /// Its negation, which always fits.
    #[must_use]
    pub const fn negated(self) -> Frac {
        Frac {
            n: self.n.saturating_neg(),
            d: self.d,
        }
    }

    /// Whether `self` is at most `other`, by exact products.
    #[must_use]
    pub fn at_most(self, other: Frac) -> bool {
        let left = i128::from(self.n).checked_mul(i128::from(other.d));
        let right = i128::from(other.n).checked_mul(i128::from(self.d));
        // Both products are below 2^64, so neither fails.
        matches!((left, right), (Some(l), Some(r)) if l <= r)
    }
}

/// The program's opaque attribute key: layer, colour, line weight and style live behind it. Every
/// edit passes it through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attr(pub u64);

/// An opaque key to program data: a style, a pattern, a gradient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub u64);

/// A place in a drawing's list of entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityIx(pub u32);

/// A place in a drawing's list of block definitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockIx(pub u32);

/// A place in a drawing's list of views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewIx(pub u32);

/// The number each entity is written under. Never reused, never renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Tag {
    /// A grid point.
    Point = 1,
    /// A line segment.
    Segment = 2,
    /// A ray.
    Ray = 3,
    /// An infinite line.
    Infinite = 4,
    /// A circle.
    Circle = 5,
    /// A circular arc.
    Arc = 6,
    /// An ellipse.
    Ellipse = 7,
    /// An elliptical arc.
    EllipticArc = 8,
    /// A spline.
    Spline = 9,
    /// A polyline with bulges.
    Polyline = 10,
    /// A region.
    Region = 11,
    /// A hatch.
    Hatch = 12,
    /// Text.
    Text = 13,
    /// A dimension.
    Dimension = 14,
    /// A leader.
    Leader = 15,
    /// A block definition.
    Block = 16,
    /// A block insert.
    Insert = 17,
    /// A viewport.
    Viewport = 18,
}

impl Tag {
    /// Every tag, in number order.
    pub const ALL: [Tag; 18] = [
        Tag::Point,
        Tag::Segment,
        Tag::Ray,
        Tag::Infinite,
        Tag::Circle,
        Tag::Arc,
        Tag::Ellipse,
        Tag::EllipticArc,
        Tag::Spline,
        Tag::Polyline,
        Tag::Region,
        Tag::Hatch,
        Tag::Text,
        Tag::Dimension,
        Tag::Leader,
        Tag::Block,
        Tag::Insert,
        Tag::Viewport,
    ];

    /// The number it is written under.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// The tag written as `number`, or `None` if no entity has that number.
    #[must_use]
    pub const fn from_u8(number: u8) -> Option<Tag> {
        Some(match number {
            1 => Tag::Point,
            2 => Tag::Segment,
            3 => Tag::Ray,
            4 => Tag::Infinite,
            5 => Tag::Circle,
            6 => Tag::Arc,
            7 => Tag::Ellipse,
            8 => Tag::EllipticArc,
            9 => Tag::Spline,
            10 => Tag::Polyline,
            11 => Tag::Region,
            12 => Tag::Hatch,
            13 => Tag::Text,
            14 => Tag::Dimension,
            15 => Tag::Leader,
            16 => Tag::Block,
            17 => Tag::Insert,
            18 => Tag::Viewport,
            _ => return None,
        })
    }
}

/// Where an elliptical arc starts or stops: a parameter of the ellipse, never a polar angle. The
/// point is x = a (1 − s²) / (1 + s²), y = b · 2s / (1 + s²) along the axes, for the fraction s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// The parameter s.
    At(Frac),
    /// The one point no fraction reaches, the far end of the major axis.
    Far,
}

/// A polyline segment's bulge: tan(sweep / 4), positive when the segment turns counter-clockwise.
pub type Bulge = Frac;

/// A line's end points: none for an infinite line, one for a ray, two for a segment.
#[derive(Clone, Copy, Debug)]
pub enum Ends {
    /// Unbounded both ways.
    Infinite,
    /// Starts here and runs on for ever.
    Ray(Pt2),
    /// Between these two.
    Segment(Pt2, Pt2),
}

/// What a spline was made from, kept as creation intent and refitted only by a versioned algorithm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitIntent {
    /// The fit points.
    pub points: Vec<P2>,
    /// The tangent at the start, if given.
    pub start_tangent: Option<[i64; 2]>,
    /// The tangent at the end, if given.
    pub end_tangent: Option<[i64; 2]>,
    /// The fit tolerance.
    pub tolerance: Steps,
    /// The version of the fitting algorithm that made the control points.
    pub version: u16,
}

/// A NURBS spline: control points on the grid, weights and knots as fractions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spline {
    degree: u8,
    control: Vec<P2>,
    weights: Option<Vec<Frac>>,
    knots: Vec<Frac>,
    intent: Option<FitIntent>,
}

impl Spline {
    /// A spline of `degree` over `control` points with the given knots, weights (all ones if none)
    /// and creation intent.
    ///
    /// # Errors
    ///
    /// [`EntityError::Spline`] if the degree is zero, the control points do not outnumber the degree,
    /// the knots are not control points plus degree plus one and in order, or a weight is missing or
    /// not positive.
    pub fn new(
        degree: u8,
        control: Vec<P2>,
        weights: Option<Vec<Frac>>,
        knots: Vec<Frac>,
        intent: Option<FitIntent>,
    ) -> Result<Spline, EntityError> {
        let p = usize::from(degree);
        let wanted = control
            .len()
            .checked_add(p)
            .and_then(|n| n.checked_add(1))
            .ok_or(EntityError::Spline)?;
        if degree == 0 || control.len() <= p || knots.len() != wanted {
            return Err(EntityError::Spline);
        }
        if !knots.windows(2).all(|w| match w {
            [a, b] => a.at_most(*b),
            _ => true,
        }) {
            return Err(EntityError::Spline);
        }
        if weights
            .as_ref()
            .is_some_and(|w| w.len() != control.len() || w.iter().any(|f| f.n <= 0))
        {
            return Err(EntityError::Spline);
        }
        Ok(Spline {
            degree,
            control,
            weights,
            knots,
            intent,
        })
    }

    /// The degree.
    #[must_use]
    pub const fn degree(&self) -> u8 {
        self.degree
    }

    /// The control points.
    #[must_use]
    pub fn control(&self) -> &[P2] {
        &self.control
    }

    /// The weights, if it is rational.
    #[must_use]
    pub fn weights(&self) -> Option<&[Frac]> {
        self.weights.as_deref()
    }

    /// The knots.
    #[must_use]
    pub fn knots(&self) -> &[Frac] {
        &self.knots
    }

    /// The creation intent, if kept.
    #[must_use]
    pub const fn intent(&self) -> Option<&FitIntent> {
        self.intent.as_ref()
    }
}

/// A polyline: vertices, a bulge for each segment, optional widths for each segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polyline {
    vertices: Vec<P2>,
    bulges: Vec<Bulge>,
    widths: Option<Vec<(Steps, Steps)>>,
    closed: bool,
}

impl Polyline {
    /// A polyline through `vertices`. A closed one has a segment from the last vertex back to the
    /// first.
    ///
    /// # Errors
    ///
    /// [`EntityError::Polyline`] if there are fewer than two vertices (three when closed), or the
    /// bulges, or the widths when given, are not one for each segment.
    pub fn new(
        vertices: Vec<P2>,
        bulges: Vec<Bulge>,
        widths: Option<Vec<(Steps, Steps)>>,
        closed: bool,
    ) -> Result<Polyline, EntityError> {
        let n = vertices.len();
        let least = if closed { 3 } else { 2 };
        let segments = if closed { Some(n) } else { n.checked_sub(1) };
        if n < least
            || segments != Some(bulges.len())
            || widths.as_ref().is_some_and(|w| Some(w.len()) != segments)
        {
            return Err(EntityError::Polyline);
        }
        Ok(Polyline {
            vertices,
            bulges,
            widths,
            closed,
        })
    }

    /// The vertices.
    #[must_use]
    pub fn vertices(&self) -> &[P2] {
        &self.vertices
    }

    /// The bulges, one for each segment.
    #[must_use]
    pub fn bulges(&self) -> &[Bulge] {
        &self.bulges
    }

    /// The start and end width of each segment, if any.
    #[must_use]
    pub fn widths(&self) -> Option<&[(Steps, Steps)]> {
        self.widths.as_deref()
    }

    /// Whether the last vertex joins the first.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Whether every segment is straight.
    #[must_use]
    pub fn is_straight(&self) -> bool {
        self.bulges.iter().all(|b| b.is_zero())
    }
}

/// A region: outer loops with nested holes of curve edges, stored as a planar Sheet body.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    body: Body,
}

impl Region {
    /// A region from a body.
    ///
    /// # Errors
    ///
    /// [`EntityError::NotASheet`] if the body is not a Sheet.
    pub fn new(body: Body) -> Result<Region, EntityError> {
        if body.kind() == BodyKind::Sheet {
            Ok(Region { body })
        } else {
            Err(EntityError::NotASheet)
        }
    }

    /// The body.
    #[must_use]
    pub const fn body(&self) -> &Body {
        &self.body
    }
}

/// How a hatch decides what lies inside its boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Island {
    /// Every other nesting level is filled.
    Normal,
    /// Only the outermost level is filled.
    Outer,
    /// Everything inside the outer boundary is filled.
    Ignore,
}

/// What a hatch fills with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fill {
    /// A solid fill.
    Solid,
    /// A pattern family, by key.
    Pattern(Key),
    /// A gradient, by key.
    Gradient(Key),
}

/// A hatch: a boundary, an island rule and a fill. The lines are derived, never stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hatch {
    boundary: Vec<EntityIx>,
    island: Island,
    fill: Fill,
}

impl Hatch {
    /// A hatch over the boundary entities.
    ///
    /// # Errors
    ///
    /// [`EntityError::Hatch`] if the boundary is empty.
    pub fn new(boundary: Vec<EntityIx>, island: Island, fill: Fill) -> Result<Hatch, EntityError> {
        if boundary.is_empty() {
            return Err(EntityError::Hatch);
        }
        Ok(Hatch {
            boundary,
            island,
            fill,
        })
    }

    /// The boundary entities.
    #[must_use]
    pub fn boundary(&self) -> &[EntityIx] {
        &self.boundary
    }

    /// The island rule.
    #[must_use]
    pub const fn island(&self) -> Island {
        self.island
    }

    /// The fill.
    #[must_use]
    pub const fn fill(&self) -> Fill {
        self.fill
    }
}

/// Which corner or edge of its box a text's anchor is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attach {
    /// Top left.
    TopLeft,
    /// Top centre.
    TopCentre,
    /// Top right.
    TopRight,
    /// Middle left.
    MiddleLeft,
    /// Middle centre.
    MiddleCentre,
    /// Middle right.
    MiddleRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom centre.
    BottomCentre,
    /// Bottom right.
    BottomRight,
}

/// Text: anchor, attachment, direction, height and reference width. Its extents come from
/// `engine-text`, not from here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    anchor: P2,
    attach: Attach,
    direction: [i64; 2],
    height: Steps,
    width: Option<Steps>,
    content: String,
}

impl Text {
    /// A text.
    ///
    /// # Errors
    ///
    /// [`EntityError::Text`] if the height is zero or the direction is the zero vector.
    pub fn new(
        anchor: P2,
        attach: Attach,
        direction: [i64; 2],
        height: Steps,
        width: Option<Steps>,
        content: String,
    ) -> Result<Text, EntityError> {
        if height.0 == 0 || direction == [0, 0] {
            return Err(EntityError::Text);
        }
        Ok(Text {
            anchor,
            attach,
            direction,
            height,
            width,
            content,
        })
    }

    /// The anchor.
    #[must_use]
    pub const fn anchor(&self) -> P2 {
        self.anchor
    }

    /// The attachment.
    #[must_use]
    pub const fn attach(&self) -> Attach {
        self.attach
    }

    /// The direction vector.
    #[must_use]
    pub const fn direction(&self) -> [i64; 2] {
        self.direction
    }

    /// The height.
    #[must_use]
    pub const fn height(&self) -> Steps {
        self.height
    }

    /// The reference width, if any.
    #[must_use]
    pub const fn width(&self) -> Option<Steps> {
        self.width
    }

    /// The string.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// What a dimension or leader refers to.
#[derive(Clone, Copy, Debug)]
pub enum Ref {
    /// An entity.
    Entity(EntityIx),
    /// A point.
    Point(Pt2),
}

/// What a dimension measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DimKind {
    /// Along an axis.
    Linear,
    /// Along the line between two points.
    Aligned,
    /// An angle.
    Angular,
    /// A radius.
    Radial,
    /// A diameter.
    Diameter,
    /// A coordinate from an origin.
    Ordinate,
}

/// A dimension: typed references and a style. The picture is derived.
#[derive(Clone, Debug)]
pub struct Dimension {
    kind: DimKind,
    refs: Vec<Ref>,
    style: Key,
}

impl Dimension {
    /// A dimension.
    ///
    /// # Errors
    ///
    /// [`EntityError::Annotation`] if there is no reference.
    pub fn new(kind: DimKind, refs: Vec<Ref>, style: Key) -> Result<Dimension, EntityError> {
        if refs.is_empty() {
            return Err(EntityError::Annotation);
        }
        Ok(Dimension { kind, refs, style })
    }

    /// What it measures.
    #[must_use]
    pub const fn kind(&self) -> DimKind {
        self.kind
    }

    /// Its references.
    #[must_use]
    pub fn refs(&self) -> &[Ref] {
        &self.refs
    }

    /// Its style.
    #[must_use]
    pub const fn style(&self) -> Key {
        self.style
    }
}

/// A leader: a path of vertices to a target, and a style.
#[derive(Clone, Debug)]
pub struct Leader {
    vertices: Vec<P2>,
    target: Option<Ref>,
    style: Key,
}

impl Leader {
    /// A leader.
    ///
    /// # Errors
    ///
    /// [`EntityError::Annotation`] if there are fewer than two vertices.
    pub fn new(vertices: Vec<P2>, target: Option<Ref>, style: Key) -> Result<Leader, EntityError> {
        if vertices.len() < 2 {
            return Err(EntityError::Annotation);
        }
        Ok(Leader {
            vertices,
            target,
            style,
        })
    }

    /// The path.
    #[must_use]
    pub fn vertices(&self) -> &[P2] {
        &self.vertices
    }

    /// What it points at.
    #[must_use]
    pub const fn target(&self) -> Option<&Ref> {
        self.target.as_ref()
    }

    /// Its style.
    #[must_use]
    pub const fn style(&self) -> Key {
        self.style
    }
}

/// A block definition, in its own frame: the point that sits at an insert's placement, and the
/// entities of the drawing that belong to it. The members are places in the drawing's list, not
/// copies, so an entity model has no cycle (a block cannot hold itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDef {
    /// The point of the block that sits at the insert's placement.
    pub origin: P2,
    /// The entities that belong to the block.
    pub members: Vec<EntityIx>,
}

/// A rectangular array of copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Array {
    columns: u32,
    rows: u32,
    column_step: [i64; 2],
    row_step: [i64; 2],
}

impl Array {
    /// An array of `columns` by `rows` copies, each step a vector in grid steps.
    ///
    /// # Errors
    ///
    /// [`EntityError::Array`] if there are no columns or no rows.
    pub fn new(
        columns: u32,
        rows: u32,
        column_step: [i64; 2],
        row_step: [i64; 2],
    ) -> Result<Array, EntityError> {
        if columns == 0 || rows == 0 {
            return Err(EntityError::Array);
        }
        Ok(Array {
            columns,
            rows,
            column_step,
            row_step,
        })
    }

    /// The columns.
    #[must_use]
    pub const fn columns(&self) -> u32 {
        self.columns
    }

    /// The rows.
    #[must_use]
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// The step from one column to the next.
    #[must_use]
    pub const fn column_step(&self) -> [i64; 2] {
        self.column_step
    }

    /// The step from one row to the next.
    #[must_use]
    pub const fn row_step(&self) -> [i64; 2] {
        self.row_step
    }
}

/// A block insert: a typed transform, an optional array, and the values of the block's attributes.
#[derive(Clone, Debug)]
pub struct Insert {
    /// The block.
    pub block: BlockIx,
    /// Where and how it is placed.
    pub at: TypedTransform,
    /// The array of copies, if any.
    pub array: Option<Array>,
    /// The values of the block's attribute definitions, by key.
    pub values: Vec<(Key, String)>,
}

/// A viewport: a sheet window onto a view, with a scale, a twist and a clip region.
#[derive(Clone, Debug)]
pub struct Viewport {
    window: Box2,
    view: ViewIx,
    scale: Frac,
    twist: Frac,
    clip: Option<Region>,
}

impl Viewport {
    /// A viewport. The twist is in half-turns.
    ///
    /// # Errors
    ///
    /// [`EntityError::Viewport`] if the scale is not positive.
    pub fn new(
        window: Box2,
        view: ViewIx,
        scale: Frac,
        twist: Frac,
        clip: Option<Region>,
    ) -> Result<Viewport, EntityError> {
        if scale.n <= 0 {
            return Err(EntityError::Viewport);
        }
        Ok(Viewport {
            window,
            view,
            scale,
            twist,
            clip,
        })
    }

    /// The window on the sheet.
    #[must_use]
    pub const fn window(&self) -> Box2 {
        self.window
    }

    /// The view it shows.
    #[must_use]
    pub const fn view(&self) -> ViewIx {
        self.view
    }

    /// The scale.
    #[must_use]
    pub const fn scale(&self) -> Frac {
        self.scale
    }

    /// The twist, in half-turns.
    #[must_use]
    pub const fn twist(&self) -> Frac {
        self.twist
    }

    /// The clip region, if any.
    #[must_use]
    pub const fn clip(&self) -> Option<&Region> {
        self.clip.as_ref()
    }
}

/// What a drawing holds: the closed list of 14.1's entities.
#[derive(Clone, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "an entity is stored in a list, not moved around, and boxing each variant would add an allocation to every point"
)]
pub enum Shape {
    /// A grid point.
    Point(P2),
    /// A line on its carrier, with zero, one or two end points.
    Line {
        /// The carrier.
        carrier: Line2,
        /// The end points.
        ends: Ends,
    },
    /// A whole circle.
    Circle(Circle2),
    /// A circular arc, counter-clockwise from `start` to `end`.
    Arc {
        /// The circle.
        circle: Circle2,
        /// Where it starts.
        start: Pt2,
        /// Where it ends.
        end: Pt2,
    },
    /// A whole ellipse.
    Ellipse(Ellipse2),
    /// An elliptical arc, counter-clockwise from `start` to `end`, by parameter.
    EllipticArc {
        /// The ellipse.
        ellipse: Ellipse2,
        /// Where it starts.
        start: Param,
        /// Where it ends.
        end: Param,
    },
    /// A spline.
    Spline(Spline),
    /// A polyline.
    Polyline(Polyline),
    /// A region.
    Region(Region),
    /// A hatch.
    Hatch(Hatch),
    /// Text.
    Text(Text),
    /// A dimension.
    Dimension(Dimension),
    /// A leader.
    Leader(Leader),
    /// A block definition.
    Block(BlockDef),
    /// A block insert.
    Insert(Insert),
    /// A viewport.
    Viewport(Viewport),
}

/// What a shape becomes under a transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Becomes {
    /// One shape of this tag.
    One(Tag),
    /// A run of lines and elliptical arcs, one for each segment of a polyline that had a bulge.
    Run,
}

/// What a transform does to a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    /// What the shape becomes.
    pub becomes: Becomes,
    /// Whether the transform reverses orientation: arcs run the other way, so their ends swap, and
    /// a bulge changes sign.
    pub mirror: bool,
}

impl Shape {
    /// Its tag.
    #[must_use]
    pub const fn tag(&self) -> Tag {
        match self {
            Shape::Point(_) => Tag::Point,
            Shape::Line { ends, .. } => match ends {
                Ends::Infinite => Tag::Infinite,
                Ends::Ray(_) => Tag::Ray,
                Ends::Segment(..) => Tag::Segment,
            },
            Shape::Circle(_) => Tag::Circle,
            Shape::Arc { .. } => Tag::Arc,
            Shape::Ellipse(_) => Tag::Ellipse,
            Shape::EllipticArc { .. } => Tag::EllipticArc,
            Shape::Spline(_) => Tag::Spline,
            Shape::Polyline(_) => Tag::Polyline,
            Shape::Region(_) => Tag::Region,
            Shape::Hatch(_) => Tag::Hatch,
            Shape::Text(_) => Tag::Text,
            Shape::Dimension(_) => Tag::Dimension,
            Shape::Leader(_) => Tag::Leader,
            Shape::Block(_) => Tag::Block,
            Shape::Insert(_) => Tag::Insert,
            Shape::Viewport(_) => Tag::Viewport,
        }
    }

    /// What a transform of this classification does to the shape (chapter 14.10).
    ///
    /// A transform that keeps circles keeps every shape's tag. Any other invertible one turns a
    /// circle into an ellipse and an arc into an elliptical arc, and leaves lines, ellipses,
    /// splines, regions and the rest as they are in kind; a polyline with a bulge becomes a run of
    /// lines and elliptical arcs, since a bulge is a circular arc. A mirror reverses arcs and
    /// negates bulges.
    ///
    /// # Errors
    ///
    /// [`EntityError::Degenerate`] if the transform collapses space.
    pub fn under(&self, class: Classification) -> Result<Change, EntityError> {
        if class.kind == TransformKind::Degenerate {
            return Err(EntityError::Degenerate);
        }
        let round = class.keeps_circles();
        let tag = self.tag();
        let becomes = match self {
            Shape::Circle(_) if !round => Becomes::One(Tag::Ellipse),
            Shape::Arc { .. } if !round => Becomes::One(Tag::EllipticArc),
            Shape::Polyline(p) if !round && !p.is_straight() => Becomes::Run,
            _ => Becomes::One(tag),
        };
        Ok(Change {
            becomes,
            mirror: class.mirror,
        })
    }
}

/// A shape with its attribute key.
#[derive(Clone, Debug)]
pub struct Entity {
    attr: Attr,
    shape: Shape,
}

/// An entity after a transform: its attribute key, untouched, and what it became.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Under {
    /// The attribute key, exactly as it was.
    pub attr: Attr,
    /// What the shape did.
    pub change: Change,
}

impl Entity {
    /// A shape with an attribute key.
    #[must_use]
    pub const fn new(attr: Attr, shape: Shape) -> Entity {
        Entity { attr, shape }
    }

    /// The attribute key.
    #[must_use]
    pub const fn attr(&self) -> Attr {
        self.attr
    }

    /// The shape.
    #[must_use]
    pub const fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Its tag.
    #[must_use]
    pub const fn tag(&self) -> Tag {
        self.shape.tag()
    }

    /// What a transform does to it, with the attribute key passed through.
    ///
    /// # Errors
    ///
    /// As for [`Shape::under`].
    pub fn under(&self, class: Classification) -> Result<Under, EntityError> {
        Ok(Under {
            attr: self.attr,
            change: self.shape.under(class)?,
        })
    }
}
