//! Snaps and tracking (chapter 14.8): a typed reference that resolves to an exact point.
//!
//! What it does: a [`SnapRef`] is a [`SnapKind`] on one or two entities, named by section 20.4's
//! [`Reference`]s, with whatever else the kind needs: a part of an entity, a dyadic parameter, a
//! chosen point, a rational direction, and which of several answers. [`SnapRef::resolve`] reads the
//! entities from a [`Drawing`] and gives a [`Pt2`] in one of 14.2's four classes, never a rounded
//! copy. The kinds are the thirteen of DXF's `DIMASSOC` (end point, midpoint, centre, node,
//! quadrant, intersection, insertion, perpendicular, tangent, nearest, apparent intersection,
//! parallel and start) and the tracking kinds of 14.8: a point beyond a line's end, on a line
//! through a chosen point in a chosen rational direction, where two such lines cross, and the
//! centroid of a closed loop.
//!
//! Which entities each kind covers, and how its point is made exact:
//!
//! | Kind | Entities | The point |
//! | --- | --- | --- |
//! | End, Start | segment, ray (start), arc, polyline (a vertex), spline (its first and last control point) | the stored end, as it was stored |
//! | Mid | segment, polyline segment, arc | a segment's midpoint is rational; an arc's is where the circle crosses the ray from its centre through its chord's midpoint (opposite for an arc over a half turn), a one-root point |
//! | Center | circle, arc, ellipse | the stored centre |
//! | Node | point | the point |
//! | Quadrant | circle, ellipse | the ends of the axes: rational for an ellipse and a circle with a whole radius, a one-root point where the radius is not whole |
//! | Intersection, Apparent | any two of line, circle, arc, ellipse and a carrier | the crossing of the two carriers, which of two by `pick`; refused unless it is on both entities' extents; an ellipse with a circle or an ellipse is a conic root |
//! | Insertion | insert, text | the placement of the block's origin if the transform is exact; the anchor of text |
//! | Perpendicular | line, circle | the foot from the chosen point: rational on a line; on a circle where the circle crosses the line through the centre, `pick` choosing the near or far one |
//! | Tangent | circle, ellipse | where the curve meets the chosen point's polar line, a rational line |
//! | Nearest | segment, circle | at the stored dyadic parameter on a segment; on a circle the radial point towards the chosen point |
//! | Extension | line | a point on the carrier at a dyadic parameter, which may lie beyond the ends |
//! | Direction, Parallel | any | on the line through the chosen point in the direction (Parallel: the direction of a line), at a dyadic parameter, or where it crosses an entity or another such line |
//! | Centroid | closed polyline of segments | the polygon's centroid, rational |
//!
//! Guarantees:
//!
//! - **Every snap resolves to an exact point, or is refused with a reason.** [`SnapError`] names the
//!   reason: a reference that did not resolve (with its [`Resolution`]), an end that is itself
//!   implicit where a rational one is needed, a crossing off an entity's extent, a transform that
//!   would round, or an entity kind the snap does not cover. Nothing is guessed or approximated.
//! - **Extents are exact.** Whether a crossing lies within a segment, a ray or an arc is decided by
//!   exact signs on the one-root number and the stored ends.
//! - **A reference that moved is followed by history, not geometry.** [`SnapRef::resolve`] resolves
//!   each [`Reference`] with section 20.4's [`resolve`], so a split or merge is followed, a deleted
//!   entity is [`SnapError::Unresolved`], and the cached point is for display only: it is never
//!   read.
//! - **Deterministic and allocation-light.** Every choice depends on the stored data alone; the
//!   only allocations are the carriers a crossing needs, pushed to the [`Carriers`] table given.
//! - **No recursion, no unsafe code.**
//!
//! Method: chapter 14.8 of the kernel architecture
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`) and 14.2's implicit points; the
//! list of kinds and their codes is the public `DIMASSOC` reference
//! (<https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C0B96256-A911-4B4D-85E6-EB4AF2C91E27.htm>),
//! as stored in `projects/kit/notes/G01-evidence/06-2d-drafting.md`; the tangent point on the
//! polar line is the classical pole-and-polar construction, and a circle's perpendicular foot and
//! nearest point lie on the line through its centre.
//!
//! Cost: constant for every kind but Centroid, which is linear in the loop; at most one crossing is
//! made, and at most two carriers pushed.

use alloc::vec::Vec;

pub use geom_name::resolve::Reference;
use geom_name::resolve::{Lookup, Resolution, Target, resolve};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};
use geom_num::{NumError, Sign};
use geom_op::NameId;

use crate::carrier::{Carriers, Circle2, Curve2};
use crate::entity::{Ends, Entity, Param, Shape};
use crate::point::{Place, PointError, Pt2};

/// The kinds of snap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapKind {
    /// The end of a curve (`DIMASSOC` code 1).
    End,
    /// The midpoint of a curve (2).
    Mid,
    /// The centre of a circle, arc or ellipse (3).
    Center,
    /// A point entity (4).
    Node,
    /// Where a circle or ellipse meets its own axes (5).
    Quadrant,
    /// The crossing of two curves (6).
    Intersection,
    /// An insert's or a text's insertion point (7).
    Insertion,
    /// The foot of a perpendicular from a chosen point (8).
    Perpendicular,
    /// A tangent point from a chosen point (9).
    Tangent,
    /// The nearest point of a curve to a chosen point (10).
    Nearest,
    /// The crossing of two curves as seen in a view (11): of their projections, which the caller
    /// gives as the two entities.
    ApparentIntersection,
    /// A point on the line through a chosen point parallel to a line (12).
    Parallel,
    /// The start of a curve (13).
    Start,
    /// A point on a line's carrier, beyond its end or within it, at a dyadic parameter.
    Extension,
    /// A point on the line through a chosen point in a chosen rational direction.
    Direction,
    /// The centroid of a closed loop.
    Centroid,
}

impl SnapKind {
    /// The public reference's `DIMASSOC` object-snap code, for the thirteen kinds it has.
    #[must_use]
    pub const fn dimassoc(self) -> Option<u8> {
        Some(match self {
            SnapKind::End => 1,
            SnapKind::Mid => 2,
            SnapKind::Center => 3,
            SnapKind::Node => 4,
            SnapKind::Quadrant => 5,
            SnapKind::Intersection => 6,
            SnapKind::Insertion => 7,
            SnapKind::Perpendicular => 8,
            SnapKind::Tangent => 9,
            SnapKind::Nearest => 10,
            SnapKind::ApparentIntersection => 11,
            SnapKind::Parallel => 12,
            SnapKind::Start => 13,
            SnapKind::Extension | SnapKind::Direction | SnapKind::Centroid => return None,
        })
    }

    /// The kind a `DIMASSOC` code names; `None` for 0 and for a code past 13.
    #[must_use]
    pub const fn from_dimassoc(code: u8) -> Option<SnapKind> {
        Some(match code {
            1 => SnapKind::End,
            2 => SnapKind::Mid,
            3 => SnapKind::Center,
            4 => SnapKind::Node,
            5 => SnapKind::Quadrant,
            6 => SnapKind::Intersection,
            7 => SnapKind::Insertion,
            8 => SnapKind::Perpendicular,
            9 => SnapKind::Tangent,
            10 => SnapKind::Nearest,
            11 => SnapKind::ApparentIntersection,
            12 => SnapKind::Parallel,
            13 => SnapKind::Start,
            _ => return None,
        })
    }
}

/// A parameter that is a fraction with a power-of-two denominator: n / 2^k along a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExactParam {
    /// The numerator.
    pub numerator: i64,
    /// The exponent of the denominator, at most [`ExactParam::MOST_SHIFT`].
    pub shift: u8,
}

impl ExactParam {
    /// The most halvings a parameter has.
    pub const MOST_SHIFT: u8 = 40;

    /// The parameter n / 2^`shift`.
    ///
    /// # Errors
    ///
    /// [`SnapError::Param`] for a shift over [`ExactParam::MOST_SHIFT`].
    pub const fn new(numerator: i64, shift: u8) -> Result<ExactParam, SnapError> {
        if shift > ExactParam::MOST_SHIFT {
            return Err(SnapError::Param);
        }
        Ok(ExactParam { numerator, shift })
    }

    /// The denominator, 2^`shift`.
    #[must_use]
    pub const fn denominator(self) -> i128 {
        1_i128 << self.shift
    }
}

/// A line to track along: through a point, in a rational direction (whole numbers, not both zero).
#[derive(Clone, Copy, Debug)]
pub struct Track {
    /// A point on it.
    pub through: Pt2,
    /// Its direction.
    pub direction: [i64; 2],
}

/// A snap: a kind, the references it is made on, and what else it needs.
#[derive(Clone, Debug)]
pub struct SnapRef {
    /// The kind.
    pub kind: SnapKind,
    /// The entity (or its carrier) the snap is on.
    pub on: Option<Reference>,
    /// The second entity, for intersections and for a tracking line's crossing.
    pub with: Option<Reference>,
    /// A part of one entity: a polyline's vertex or segment, counted from 0. The entity model
    /// names no part, so this is a place, not a name.
    pub sub: Option<u32>,
    /// A dyadic parameter: for nearest, extension and tracking.
    pub at: Option<ExactParam>,
    /// Which of two or four answers: a quadrant from 0 (east) counter-clockwise, the first or second
    /// crossing, the near or far foot.
    pub pick: u8,
    /// The chosen point: the point a perpendicular, tangent, nearest or parallel is from, and where
    /// a direction passes.
    pub from: Option<Pt2>,
    /// The line to track along, for a direction (a parallel takes its direction from `on`).
    pub track: Option<Track>,
    /// A second line to track along, for where two meet.
    pub track2: Option<Track>,
    /// The point as last resolved, for display only: resolution never reads it.
    pub cached: Pt2,
}

impl SnapRef {
    /// A snap of `kind` with nothing else set.
    #[must_use]
    pub const fn new(kind: SnapKind, cached: Pt2) -> SnapRef {
        SnapRef {
            kind,
            on: None,
            with: None,
            sub: None,
            at: None,
            pick: 0,
            from: None,
            track: None,
            track2: None,
            cached,
        }
    }
}

/// Why a snap could not be made into a point.
#[derive(Clone, Debug, PartialEq)]
pub enum SnapError {
    /// A reference did not resolve to one entity or carrier: why, as section 20.4 says.
    Unresolved(Resolution),
    /// The name resolved, but the drawing holds no entity of it.
    NoEntity(NameId),
    /// The snap needs a reference, a point, a direction or a parameter that it was not given.
    Missing(&'static str),
    /// The snap's kind does not cover this kind of entity.
    Unsupported(SnapKind, crate::entity::Tag),
    /// An end that this snap needs as a rational point is itself implicit (a crossing).
    ImplicitEnd,
    /// The part, the pick or the parameter is out of range.
    Param,
    /// The crossing is not within the extent of one of the entities.
    OffExtent,
    /// The entity's transform would round the point.
    Rounded,
    /// A direction of zero length, or a tracking line with none.
    NoDirection,
    /// A step of the numbers layer failed.
    Num(NumError),
    /// A point could not be read or made.
    Point(PointError),
}

impl From<NumError> for SnapError {
    fn from(e: NumError) -> Self {
        SnapError::Num(e)
    }
}

impl From<PointError> for SnapError {
    fn from(e: PointError) -> Self {
        SnapError::Point(e)
    }
}

impl core::fmt::Display for SnapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SnapError::Unresolved(r) => write!(f, "the reference did not resolve: {r:?}"),
            SnapError::NoEntity(_) => f.write_str("the drawing holds no entity of that name"),
            SnapError::Missing(what) => write!(f, "the snap needs {what}"),
            SnapError::Unsupported(kind, tag) => {
                write!(f, "a {kind:?} snap does not cover a {tag:?}")
            }
            SnapError::ImplicitEnd => f.write_str("an end of the entity is not a rational point"),
            SnapError::Param => f.write_str("the part, the pick or the parameter is out of range"),
            SnapError::OffExtent => f.write_str("the crossing is off an entity's extent"),
            SnapError::Rounded => f.write_str("the transform would round the point"),
            SnapError::NoDirection => f.write_str("a direction of zero length"),
            SnapError::Num(e) => write!(f, "{e}"),
            SnapError::Point(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for SnapError {}

/// The entities of a drawing by name, and the carriers references of kind carrier name by index.
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    items: Vec<(NameId, Entity)>,
    carriers: Vec<(geom_name::resolve::CarrierIx, Curve2)>,
}

impl Drawing {
    /// A drawing of `items`, which are sorted by name here, and `carriers`, sorted by index.
    ///
    /// # Errors
    ///
    /// [`SnapError::NoEntity`] naming a name that two items share.
    pub fn new(
        mut items: Vec<(NameId, Entity)>,
        mut carriers: Vec<(geom_name::resolve::CarrierIx, Curve2)>,
    ) -> Result<Drawing, SnapError> {
        items.sort_by_key(|(name, _)| *name);
        if let Some(pair) = items.windows(2).find(|w| w[0].0 == w[1].0) {
            return Err(SnapError::NoEntity(pair[0].0));
        }
        carriers.sort_by_key(|(ix, _)| *ix);
        Ok(Drawing { items, carriers })
    }

    /// The entities, in name order.
    #[must_use]
    pub fn items(&self) -> &[(NameId, Entity)] {
        &self.items
    }

    /// The entity named `name`.
    #[must_use]
    pub fn entity(&self, name: NameId) -> Option<&Entity> {
        self.items
            .binary_search_by_key(&name, |(n, _)| *n)
            .ok()
            .and_then(|at| self.items.get(at))
            .map(|(_, e)| e)
    }

    /// The carrier at `ix`.
    #[must_use]
    pub fn carrier(&self, ix: geom_name::resolve::CarrierIx) -> Option<&Curve2> {
        self.carriers
            .binary_search_by_key(&ix, |(i, _)| *i)
            .ok()
            .and_then(|at| self.carriers.get(at))
            .map(|(_, c)| c)
    }
}

/// What a reference came to.
pub(crate) enum Subject<'a> {
    /// An entity.
    Entity(&'a Entity),
    /// A carrier alone: the whole curve.
    Carrier(Curve2),
}

fn subject<'a>(
    r: &Reference,
    drawing: &'a Drawing,
    lookup: &Lookup<'_>,
) -> Result<Subject<'a>, SnapError> {
    match resolve(r, lookup) {
        Resolution::Exact { target, .. } | Resolution::FollowedHistory { target, .. } => {
            match target {
                Target::One(name) => drawing
                    .entity(name)
                    .map(Subject::Entity)
                    .ok_or(SnapError::NoEntity(name)),
                Target::Carrier(ix) => {
                    drawing
                        .carrier(ix)
                        .map(|c| Subject::Carrier(*c))
                        .ok_or(SnapError::Unresolved(Resolution::Unresolved {
                            reason: geom_name::resolve::UnresolvedReason::NoCarrier,
                        }))
                }
                Target::Pieces(_) => Err(SnapError::Unresolved(Resolution::Unresolved {
                    reason: geom_name::resolve::UnresolvedReason::NameUnknown,
                })),
            }
        }
        other => Err(SnapError::Unresolved(other)),
    }
}

// Exact integer arithmetic for building carriers and rational points; every step is checked.

fn mul(a: i128, b: i128) -> Result<i128, SnapError> {
    a.checked_mul(b)
        .ok_or(SnapError::Num(NumError::BudgetExceeded))
}

fn add(a: i128, b: i128) -> Result<i128, SnapError> {
    a.checked_add(b)
        .ok_or(SnapError::Num(NumError::BudgetExceeded))
}

fn sub(a: i128, b: i128) -> Result<i128, SnapError> {
    a.checked_sub(b)
        .ok_or(SnapError::Num(NumError::BudgetExceeded))
}

fn neg(a: i128) -> Result<i128, SnapError> {
    a.checked_neg()
        .ok_or(SnapError::Num(NumError::BudgetExceeded))
}

/// The greatest common divisor of `a` and `b`.
fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    i128::try_from(a).unwrap_or(1)
}

/// The rational point (x, y) / w, reduced.
fn rational(x: i128, y: i128, w: i128) -> Result<Rational2, SnapError> {
    let g = gcd(gcd(x, y), w).max(1);
    let (x, y, w) = (
        x.checked_div(g).unwrap_or(x),
        y.checked_div(g).unwrap_or(y),
        w.checked_div(g).unwrap_or(w),
    );
    Ok(Rational2::new(x, y, w)?)
}

/// A point as a rational point, if it is a grid or rational one.
fn rat(p: &Pt2) -> Result<Rational2, SnapError> {
    match p {
        Pt2::Grid(g) => Ok(Rational2::from(*g)),
        Pt2::Rational(r) => Ok(*r),
        Pt2::Crossing { .. } | Pt2::ConicRoot { .. } => Err(SnapError::ImplicitEnd),
    }
}

/// A rational point as the simplest class that holds it.
fn point_of(r: Rational2) -> Pt2 {
    let ([x, y], w) = r.coordinates();
    if w == 1
        && let (Ok(xi), Ok(yi)) = (i64::try_from(x), i64::try_from(y))
        && let (Ok(cx), Ok(cy)) = (
            geom_num::grid::Coord::new(xi),
            geom_num::grid::Coord::new(yi),
        )
    {
        return Pt2::Grid(P2 { x: cx, y: cy });
    }
    Pt2::Rational(r)
}

/// The line through the rational point `a` in the direction `d`: a = −d_y w, b = d_x w and
/// c = −d_y X + d_x Y, so that it runs along (b, −a) = (d_x w, d_y w).
fn line_through(a: &Rational2, d: [i128; 2]) -> Result<Carrier, SnapError> {
    if d == [0, 0] {
        return Err(SnapError::NoDirection);
    }
    let ([x, y], w) = a.coordinates();
    let na = neg(mul(d[1], w)?)?;
    let nb = mul(d[0], w)?;
    let c = add(neg(mul(d[1], x)?)?, mul(d[0], y)?)?;
    let g = gcd(gcd(na, nb), c).max(1);
    Ok(Carrier::new(
        na.checked_div(g).unwrap_or(na),
        nb.checked_div(g).unwrap_or(nb),
        c.checked_div(g).unwrap_or(c),
    )?)
}

/// The centre of a circle as a rational point.
fn centre_of(circle: &Circle2) -> Rational2 {
    match circle {
        Circle2::Grid(c) => Rational2::from(c.centre()),
        Circle2::Round(r) => r.centre(),
    }
}

/// `p` less `q`, scaled to a common weight: the vector and the weight.
fn vector(p: &Rational2, q: &Rational2) -> Result<([i128; 2], i128), SnapError> {
    let ([px, py], pw) = p.coordinates();
    let ([qx, qy], qw) = q.coordinates();
    Ok((
        [
            sub(mul(px, qw)?, mul(qx, pw)?)?,
            sub(mul(py, qw)?, mul(qy, pw)?)?,
        ],
        mul(pw, qw)?,
    ))
}

/// Pushes `curve` and reads the crossing of `a` and `b` as a point, `pick` choosing the root.
fn crossing_point(
    carriers: &mut Carriers,
    a: &Curve2,
    b: &Curve2,
    pick: u8,
) -> Result<Pt2, SnapError> {
    if pick > 1 {
        return Err(SnapError::Param);
    }
    let ia = carriers.push(*a)?;
    let ib = carriers.push(*b)?;
    let ellipse_a = matches!(a, Curve2::Ellipse(_));
    let ellipse_b = matches!(b, Curve2::Ellipse(_));
    let curved = |c: &Curve2| !matches!(c, Curve2::Line(_));
    let point = if (ellipse_a && curved(b)) || (ellipse_b && curved(a)) {
        // An ellipse with a circle or an ellipse: a root of the conic's quartic, the ellipse first.
        if ellipse_a {
            Pt2::ConicRoot {
                a: ia,
                b: ib,
                k: pick,
            }
        } else {
            Pt2::ConicRoot {
                a: ib,
                b: ia,
                k: pick,
            }
        }
    } else {
        Pt2::Crossing {
            a: ia,
            b: ib,
            root: if pick == 0 { Root::First } else { Root::Second },
        }
    };
    point.place(carriers)?;
    Ok(point)
}

/// The sign of (z − `centre`) · `d` for the one-root point `z`.
fn toward(z: &Crossing2, centre: &Rational2, d: [i128; 2]) -> Result<Sign, SnapError> {
    Ok(z.side(&normal_line(centre, d)?)?)
}

/// The carrier through `centre` with normal `d`.
fn normal_line(centre: &Rational2, d: [i128; 2]) -> Result<Carrier, SnapError> {
    let ([x, y], w) = centre.coordinates();
    let c = add(mul(d[0], x)?, mul(d[1], y)?)?;
    let (a, b) = (mul(d[0], w)?, mul(d[1], w)?);
    let g = gcd(gcd(a, b), c).max(1);
    Ok(Carrier::new(
        a.checked_div(g).unwrap_or(a),
        b.checked_div(g).unwrap_or(b),
        c.checked_div(g).unwrap_or(c),
    )?)
}

/// Where the curve `curve`, closed and centred at `centre`, meets the ray from `centre` towards `d`:
/// the crossing of the curve with the line through the centre that is on the side `d` points to.
fn along_ray(
    carriers: &mut Carriers,
    curve: &Curve2,
    centre: &Rational2,
    d: [i128; 2],
) -> Result<Pt2, SnapError> {
    let ray = Curve2::Line(line_through(centre, d)?);
    for pick in 0..2 {
        let point = crossing_point(carriers, curve, &ray, pick)?;
        if let Place::OneRoot(z) = point.place(carriers)?
            && toward(&z, centre, d)? == Sign::Positive
        {
            return Ok(point);
        }
    }
    Err(SnapError::Num(NumError::Degenerate))
}

/// The curve of an entity, and what it lies on, for the snaps that cross or drop to a carrier.
fn curve_of(e: &Entity) -> Option<Curve2> {
    match e.shape() {
        Shape::Line { carrier, .. } => Some(Curve2::Line(*carrier)),
        Shape::Circle(c) | Shape::Arc { circle: c, .. } => Some(Curve2::Circle(*c)),
        Shape::Ellipse(el) | Shape::EllipticArc { ellipse: el, .. } => Some(Curve2::Ellipse(*el)),
        _ => None,
    }
}

fn curve_of_subject(s: &Subject<'_>, kind: SnapKind) -> Result<Curve2, SnapError> {
    match s {
        Subject::Carrier(c) => Ok(*c),
        Subject::Entity(e) => curve_of(e).ok_or(SnapError::Unsupported(kind, e.tag())),
    }
}

/// Whether the one-root point `z`, which lies on `curve`, is within the extent of `e`.
fn within(z: &Pt2, e: &Entity, carriers: &Carriers) -> Result<bool, SnapError> {
    let Place::OneRoot(zc) = z.place(carriers)? else {
        return Ok(true);
    };
    match e.shape() {
        Shape::Line { carrier, ends } => match ends {
            Ends::Infinite => Ok(true),
            Ends::Ray(start) => {
                let s = place_of(start, carriers)?;
                let d = carrier.direction();
                // The ray runs from `start` the way the carrier runs.
                let along = if d[0] != 0 {
                    (zc.cmp_x(&s)?, d[0])
                } else {
                    (zc.cmp_y(&s)?, d[1])
                };
                Ok(along.0 == Sign::Zero || (along.0 == Sign::Positive) == (along.1 > 0))
            }
            Ends::Segment(a, b) => {
                let (pa, pb) = (place_of(a, carriers)?, place_of(b, carriers)?);
                let d = carrier.direction();
                let (sa, sb) = if d[0] != 0 {
                    (zc.cmp_x(&pa)?, zc.cmp_x(&pb)?)
                } else {
                    (zc.cmp_y(&pa)?, zc.cmp_y(&pb)?)
                };
                Ok(sa == Sign::Zero || sb == Sign::Zero || sa != sb)
            }
        },
        Shape::Arc { circle, start, end } => arc_holds(&zc, circle, start, end),
        Shape::Circle(_) | Shape::Ellipse(_) => Ok(true),
        _ => Ok(true),
    }
}

/// A point as a one-root number.
fn place_of(p: &Pt2, carriers: &Carriers) -> Result<Crossing2, SnapError> {
    match p.place(carriers)? {
        Place::OneRoot(z) => Ok(z),
        Place::Conic { .. } => Err(SnapError::ImplicitEnd),
    }
}

/// Whether `z`, on the circle, is on the counter-clockwise arc from `start` to `end`.
fn arc_holds(z: &Crossing2, circle: &Circle2, start: &Pt2, end: &Pt2) -> Result<bool, SnapError> {
    let (c, s, e) = (centre_of(circle), rat(start)?, rat(end)?);
    let (ds, _) = vector(&s, &c)?;
    let (de, _) = vector(&e, &c)?;
    let turn = sub(mul(ds[0], de[1])?, mul(ds[1], de[0])?)?;
    let dot = add(mul(ds[0], de[0])?, mul(ds[1], de[1])?)?;
    // Left of c to s is the counter-clockwise side of s: the normal (-dy, dx) points there.
    let left_of_start = normal_line(&c, [neg(ds[1])?, ds[0]])?;
    let left_of_end = normal_line(&c, [neg(de[1])?, de[0]])?;
    let ls = z.side(&shift_through(&left_of_start, &c)?)?;
    let le = z.side(&shift_through(&left_of_end, &c)?)?;
    Ok(if turn == 0 && dot > 0 {
        // The ends are one: the whole circle.
        true
    } else if turn == 0 {
        // A half turn: the side of the line from the centre to the start.
        ls != Sign::Negative
    } else if turn > 0 {
        // Under a half turn: left of the start and right of the end.
        ls != Sign::Negative && le != Sign::Positive
    } else {
        // Over a half turn: not in the short way round from the end to the start.
        !(le == Sign::Positive && ls == Sign::Negative)
    })
}

/// `line` as it is (it already passes through `centre`).
fn shift_through(line: &Carrier, _centre: &Rational2) -> Result<Carrier, SnapError> {
    Ok(*line)
}

impl SnapRef {
    /// The point this snap names, exactly.
    ///
    /// `lookup` is the element's body for resolving each [`Reference`] (20.4), and `carriers` the
    /// table to push the carriers of a crossing to.
    ///
    /// # Errors
    ///
    /// A [`SnapError`] naming why there is no exact point.
    #[allow(
        clippy::too_many_lines,
        reason = "one match on the kind: each arm is the construction of 14.8's table for that kind"
    )]
    pub fn resolve(
        &self,
        drawing: &Drawing,
        lookup: &Lookup<'_>,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        match self.kind {
            SnapKind::Direction | SnapKind::Parallel => {
                return self.tracked(drawing, lookup, carriers);
            }
            SnapKind::Intersection | SnapKind::ApparentIntersection => {
                return self.crossing(drawing, lookup, carriers);
            }
            _ => {}
        }
        let on = self.on.as_ref().ok_or(SnapError::Missing("an entity"))?;
        let subject = subject(on, drawing, lookup)?;
        self.on_subject(subject, carriers)
    }

    /// The point of a snap on a single subject, whichever way the subject was found: the part of
    /// [`SnapRef::resolve`] after the references are read. The index uses it for the candidates
    /// near a cursor.
    ///
    /// # Errors
    ///
    /// As [`SnapRef::resolve`], but for a reference.
    #[allow(
        clippy::too_many_lines,
        reason = "one match on the kind: each arm is the construction of 14.8's table for that kind"
    )]
    pub(crate) fn on_subject(
        &self,
        subject: Subject<'_>,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        let kind = self.kind;
        let entity = match &subject {
            Subject::Entity(e) => Some(*e),
            Subject::Carrier(_) => None,
        };
        match kind {
            SnapKind::Node => match entity.map(Entity::shape) {
                Some(Shape::Point(p)) => Ok(Pt2::Grid(*p)),
                _ => Err(unsupported(kind, entity)),
            },
            SnapKind::Start | SnapKind::End => self.end(kind == SnapKind::Start, entity),
            SnapKind::Mid => self.middle(entity, carriers),
            SnapKind::Center => match entity.map(Entity::shape) {
                Some(Shape::Circle(c) | Shape::Arc { circle: c, .. }) => Ok(point_of(centre_of(c))),
                Some(Shape::Ellipse(e) | Shape::EllipticArc { ellipse: e, .. }) => {
                    Ok(Pt2::Grid(e.centre()))
                }
                _ => match subject {
                    Subject::Carrier(Curve2::Circle(c)) => Ok(point_of(centre_of(&c))),
                    Subject::Carrier(Curve2::Ellipse(e)) => Ok(Pt2::Grid(e.centre())),
                    _ => Err(unsupported(kind, entity)),
                },
            },
            SnapKind::Quadrant => {
                let curve = curve_of_subject(&subject, kind)?;
                self.quadrant(&curve, carriers)
            }
            SnapKind::Insertion => match entity.map(Entity::shape) {
                Some(Shape::Insert(i)) => {
                    let image =
                        i.at.apply_2d(P2::new(0, 0).map_err(SnapError::Num)?)
                            .map_err(|_| SnapError::Rounded)?;
                    if image.is_exact() {
                        Ok(Pt2::Grid(image.value()))
                    } else {
                        Err(SnapError::Rounded)
                    }
                }
                Some(Shape::Text(t)) => Ok(Pt2::Grid(t.anchor())),
                _ => Err(unsupported(kind, entity)),
            },
            SnapKind::Perpendicular => {
                let curve = curve_of_subject(&subject, kind)?;
                let from = rat(self.from.as_ref().ok_or(SnapError::Missing("a point"))?)?;
                self.foot(&curve, &from, carriers)
            }
            SnapKind::Tangent => {
                let curve = curve_of_subject(&subject, kind)?;
                let from = rat(self.from.as_ref().ok_or(SnapError::Missing("a point"))?)?;
                self.tangent(&curve, &from, carriers)
            }
            SnapKind::Nearest => self.nearest(&subject, carriers),
            SnapKind::Extension => self.extension(&subject),
            SnapKind::Centroid => match entity.map(Entity::shape) {
                Some(Shape::Polyline(p)) => centroid(p),
                _ => Err(unsupported(kind, entity)),
            },
            SnapKind::Direction
            | SnapKind::Parallel
            | SnapKind::Intersection
            | SnapKind::ApparentIntersection => Err(SnapError::Missing("a handled kind")),
        }
    }

    /// An end or the start of an entity.
    fn end(&self, start: bool, entity: Option<&Entity>) -> Result<Pt2, SnapError> {
        let kind = if start {
            SnapKind::Start
        } else {
            SnapKind::End
        };
        match entity.map(Entity::shape) {
            Some(Shape::Line { ends, .. }) => match (ends, start) {
                (Ends::Segment(a, _) | Ends::Ray(a), true) => Ok(*a),
                (Ends::Segment(_, b), false) => Ok(*b),
                _ => Err(unsupported(kind, entity)),
            },
            Some(Shape::Arc {
                start: s, end: e, ..
            }) => Ok(if start { *s } else { *e }),
            Some(Shape::Polyline(p)) => {
                let vertices = p.vertices();
                let at = match self.sub {
                    Some(n) => usize::try_from(n).map_err(|_| SnapError::Param)?,
                    None if start => 0,
                    None => vertices.len().saturating_sub(1),
                };
                vertices
                    .get(at)
                    .map(|v| Pt2::Grid(*v))
                    .ok_or(SnapError::Param)
            }
            Some(Shape::Spline(s)) => {
                let control = s.control();
                let end = if start {
                    control.first()
                } else {
                    control.last()
                };
                end.map(|v| Pt2::Grid(*v)).ok_or(SnapError::Param)
            }
            Some(Shape::EllipticArc {
                ellipse,
                start: s,
                end: e,
            }) => ellipse_point(ellipse, if start { s } else { e }),
            _ => Err(unsupported(kind, entity)),
        }
    }

    /// The midpoint of a segment, a polyline segment or an arc.
    fn middle(&self, entity: Option<&Entity>, carriers: &mut Carriers) -> Result<Pt2, SnapError> {
        let kind = SnapKind::Mid;
        match entity.map(Entity::shape) {
            Some(Shape::Line {
                ends: Ends::Segment(a, b),
                ..
            }) => Ok(point_of(midpoint(&rat(a)?, &rat(b)?)?)),
            Some(Shape::Polyline(p)) => {
                let n = usize::try_from(self.sub.unwrap_or(0)).map_err(|_| SnapError::Param)?;
                let vs = p.vertices();
                let a = vs.get(n).ok_or(SnapError::Param)?;
                let next = n.checked_add(1).ok_or(SnapError::Param)?;
                let b = if next == vs.len() && p.is_closed() {
                    vs.first()
                } else {
                    vs.get(next)
                }
                .ok_or(SnapError::Param)?;
                if p.bulges().get(n).is_some_and(|f| !f.is_zero()) {
                    return Err(unsupported(kind, entity));
                }
                Ok(point_of(midpoint(
                    &Rational2::from(*a),
                    &Rational2::from(*b),
                )?))
            }
            Some(Shape::Arc { circle, start, end }) => {
                let (c, s, e) = (centre_of(circle), rat(start)?, rat(end)?);
                let (ds, _) = vector(&s, &c)?;
                let (de, _) = vector(&e, &c)?;
                // Chord midpoint less the centre, with the weights of s and e matched.
                let ([sx, sy], sw) = s.coordinates();
                let ([ex, ey], ew) = e.coordinates();
                let ([cx, cy], cw) = c.coordinates();
                let mid_x = add(mul(sx, ew)?, mul(ex, sw)?)?;
                let mid_y = add(mul(sy, ew)?, mul(ey, sw)?)?;
                let mid_w = mul(mul(sw, ew)?, 2)?;
                let d = [
                    sub(mul(mid_x, cw)?, mul(cx, mid_w)?)?,
                    sub(mul(mid_y, cw)?, mul(cy, mid_w)?)?,
                ];
                let turn = sub(mul(ds[0], de[1])?, mul(ds[1], de[0])?)?;
                let dot = add(mul(ds[0], de[0])?, mul(ds[1], de[1])?)?;
                let direction = if turn == 0 {
                    if dot > 0 {
                        // A whole circle: the point opposite the start.
                        [neg(ds[0])?, neg(ds[1])?]
                    } else {
                        // A half turn: at right angles to the chord, on its counter-clockwise side.
                        [neg(ds[1])?, ds[0]]
                    }
                } else if turn > 0 {
                    d
                } else {
                    [neg(d[0])?, neg(d[1])?]
                };
                along_ray(carriers, &Curve2::Circle(*circle), &c, direction)
            }
            Some(Shape::EllipticArc { .. }) => Err(unsupported(kind, entity)),
            _ => Err(unsupported(kind, entity)),
        }
    }

    /// The point where `curve` meets one of its own axes: `pick` 0 east, 1 north, 2 west, 3 south,
    /// the axes of an ellipse being its major and minor.
    fn quadrant(&self, curve: &Curve2, carriers: &mut Carriers) -> Result<Pt2, SnapError> {
        if self.pick > 3 {
            return Err(SnapError::Param);
        }
        match curve {
            Curve2::Circle(Circle2::Grid(c)) => {
                let (cx, cy) = (
                    i128::from(c.centre().x.get()),
                    i128::from(c.centre().y.get()),
                );
                let r = c.radius();
                let (x, y) = match self.pick {
                    0 => (add(cx, r)?, cy),
                    1 => (cx, add(cy, r)?),
                    2 => (sub(cx, r)?, cy),
                    _ => (cx, sub(cy, r)?),
                };
                Ok(point_of(rational(x, y, 1)?))
            }
            Curve2::Circle(circle @ Circle2::Round(r)) => {
                let d = match self.pick {
                    0 => [1, 0],
                    1 => [0, 1],
                    2 => [-1, 0],
                    _ => [0, -1],
                };
                along_ray(carriers, &Curve2::Circle(*circle), &r.centre(), d)
            }
            Curve2::Ellipse(e) => {
                let (p, q) = e.ratio();
                let [mx, my] = e.major();
                let (cx, cy) = (
                    i128::from(e.centre().x.get()),
                    i128::from(e.centre().y.get()),
                );
                let (p, q) = (i128::from(p), i128::from(q));
                let (mx, my) = (i128::from(mx), i128::from(my));
                // The minor semi-axis is the major turned a quarter, scaled by p / q.
                let (x, y) = match self.pick {
                    0 => (
                        add(mul(cx, q)?, mul(mx, q)?)?,
                        add(mul(cy, q)?, mul(my, q)?)?,
                    ),
                    1 => (
                        sub(mul(cx, q)?, mul(my, p)?)?,
                        add(mul(cy, q)?, mul(mx, p)?)?,
                    ),
                    2 => (
                        sub(mul(cx, q)?, mul(mx, q)?)?,
                        sub(mul(cy, q)?, mul(my, q)?)?,
                    ),
                    _ => (
                        add(mul(cx, q)?, mul(my, p)?)?,
                        sub(mul(cy, q)?, mul(mx, p)?)?,
                    ),
                };
                Ok(point_of(rational(x, y, q)?))
            }
            Curve2::Line(_) => Err(SnapError::Unsupported(
                SnapKind::Quadrant,
                crate::entity::Tag::Segment,
            )),
        }
    }

    /// The foot of the perpendicular from `from` to `curve`.
    fn foot(
        &self,
        curve: &Curve2,
        from: &Rational2,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        match curve {
            Curve2::Line(l) => {
                let ([a, b], _) = l.coefficients();
                let perpendicular = line_through(from, [a, b])?;
                Ok(point_of(l.meet(&perpendicular)?))
            }
            Curve2::Circle(circle) => {
                let c = centre_of(circle);
                let (d, _) = vector(from, &c)?;
                if d == [0, 0] {
                    return Err(SnapError::NoDirection);
                }
                // `pick` 0 is the foot on the side of `from`, 1 the one opposite.
                let d = if self.pick == 0 {
                    d
                } else {
                    [neg(d[0])?, neg(d[1])?]
                };
                along_ray(carriers, curve, &c, d)
            }
            Curve2::Ellipse(_) => Err(SnapError::Unsupported(
                SnapKind::Perpendicular,
                crate::entity::Tag::Ellipse,
            )),
        }
    }

    /// The tangent point from `from`: where the curve meets its polar line.
    fn tangent(
        &self,
        curve: &Curve2,
        from: &Rational2,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        let ([px, py], pw) = from.coordinates();
        let polar = match curve {
            Curve2::Circle(circle) => {
                // With the centre C = (cx, cy) / cw and radius squared ρ, the polar of P is
                // (P − C) · (X − C) = ρ; for a Round, ρ is not rational in general, so the polar is
                // built from the centre and a point on the circle: (P − C) · (X − C) = (O − C)².
                let c = centre_of(circle);
                let ([cx, cy], cw) = c.coordinates();
                let (rho_num, rho_den) = radius_squared(circle)?;
                // (px/pw − cx/cw)(X − cx/cw) + (py/pw − cy/cw)(Y − cy/cw) = rho_num / rho_den
                let ex = sub(mul(px, cw)?, mul(cx, pw)?)?;
                let ey = sub(mul(py, cw)?, mul(cy, pw)?)?;
                let scale = mul(pw, cw)?;
                // ex/scale · X + ey/scale · Y = rho + (ex cx + ey cy) / (scale cw)
                let rhs_num = add(
                    mul(rho_num, mul(scale, cw)?)?,
                    mul(rho_den, add(mul(ex, cx)?, mul(ey, cy)?)?)?,
                );
                let rhs_den = mul(rho_den, mul(scale, cw)?)?;
                let (a, b, c0) = (mul(ex, rhs_den)?, mul(ey, rhs_den)?, mul(rhs_num?, scale)?);
                let g = gcd(gcd(a, b), c0).max(1);
                Carrier::new(
                    a.checked_div(g).unwrap_or(a),
                    b.checked_div(g).unwrap_or(b),
                    c0.checked_div(g).unwrap_or(c0),
                )?
            }
            Curve2::Ellipse(e) => {
                // With m the major semi-axis vector, m⊥ = (−m_y, m_x) and the minor n = (p / q) m⊥,
                // a point is C + u m + v n for the unit circle u² + v² = 1, where u = (X − C) · m /
                // |m|² and v = (q / p) (X − C) · m⊥ / |m|². The polar of P is u_P u + v_P v = 1, that
                // is g · (X − C) = p² |m|⁴ for g = p² (D · m) m + q² (D · m⊥) m⊥, with D = P − C
                // taken over the weight of P.
                let [mx, my] = e.major();
                let (p, q) = e.ratio();
                let (mx, my, p, q) = (i128::from(mx), i128::from(my), i128::from(p), i128::from(q));
                let cx = i128::from(e.centre().x.get());
                let cy = i128::from(e.centre().y.get());
                let dx = sub(px, mul(cx, pw)?)?;
                let dy = sub(py, mul(cy, pw)?)?;
                let (pp, qq) = (mul(p, p)?, mul(q, q)?);
                let dm = add(mul(dx, mx)?, mul(dy, my)?)?;
                let dp = add(mul(dx, neg(my)?)?, mul(dy, mx)?)?;
                let gx = add(mul(mul(dm, mx)?, pp)?, mul(mul(dp, neg(my)?)?, qq)?)?;
                let gy = add(mul(mul(dm, my)?, pp)?, mul(mul(dp, mx)?, qq)?)?;
                let mm = add(mul(mx, mx)?, mul(my, my)?)?;
                let rhs = mul(mul(mul(mm, mm)?, pp)?, pw)?;
                let c0 = add(rhs, add(mul(gx, cx)?, mul(gy, cy)?)?)?;
                let g = gcd(gcd(gx, gy), c0).max(1);
                Carrier::new(
                    gx.checked_div(g).unwrap_or(gx),
                    gy.checked_div(g).unwrap_or(gy),
                    c0.checked_div(g).unwrap_or(c0),
                )?
            }
            Curve2::Line(_) => {
                return Err(SnapError::Unsupported(
                    SnapKind::Tangent,
                    crate::entity::Tag::Segment,
                ));
            }
        };
        crossing_point(carriers, curve, &Curve2::Line(polar), self.pick)
    }

    /// The nearest point, from the stored parameter on a segment, radially on a circle.
    fn nearest(&self, subject: &Subject<'_>, carriers: &mut Carriers) -> Result<Pt2, SnapError> {
        let kind = SnapKind::Nearest;
        if let Subject::Entity(e) = subject
            && let Shape::Line {
                ends: Ends::Segment(a, b),
                ..
            } = e.shape()
        {
            let t = self.at.ok_or(SnapError::Missing("a parameter"))?;
            if t.numerator < 0 || i128::from(t.numerator) > t.denominator() {
                return Err(SnapError::Param);
            }
            return along(&rat(a)?, &rat(b)?, t);
        }
        let curve = curve_of_subject(subject, kind)?;
        match curve {
            Curve2::Circle(circle) => {
                let c = centre_of(&circle);
                let from = rat(self.from.as_ref().ok_or(SnapError::Missing("a point"))?)?;
                let (d, _) = vector(&from, &c)?;
                if d == [0, 0] {
                    return Err(SnapError::NoDirection);
                }
                let point = along_ray(carriers, &Curve2::Circle(circle), &c, d)?;
                if let Subject::Entity(e) = subject
                    && let Shape::Arc { circle, start, end } = e.shape()
                    && !arc_holds(&place_of(&point, carriers)?, circle, start, end)?
                {
                    return Err(SnapError::OffExtent);
                }
                Ok(point)
            }
            _ => Err(unsupported(kind, entity_of(subject))),
        }
    }

    /// A point on a line's carrier at a dyadic parameter along its ends or its direction.
    fn extension(&self, subject: &Subject<'_>) -> Result<Pt2, SnapError> {
        let kind = SnapKind::Extension;
        let t = self.at.ok_or(SnapError::Missing("a parameter"))?;
        match subject {
            Subject::Entity(e) => match e.shape() {
                Shape::Line {
                    ends: Ends::Segment(a, b),
                    ..
                } => along(&rat(a)?, &rat(b)?, t),
                _ => Err(unsupported(kind, Some(e))),
            },
            Subject::Carrier(_) => Err(SnapError::Missing("a segment")),
        }
    }

    /// Direction and parallel snaps: a point on a tracking line.
    fn tracked(
        &self,
        drawing: &Drawing,
        lookup: &Lookup<'_>,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        let line = self.tracking_line(drawing, lookup)?;
        let (through, d) = line;
        if let Some(t) = self.at {
            let ([x, y], w) = through.coordinates();
            let n = i128::from(t.numerator);
            let den = t.denominator();
            let scale = mul(w, den)?;
            return Ok(point_of(rational(
                add(mul(x, den)?, mul(mul(d[0], n)?, w)?)?,
                add(mul(y, den)?, mul(mul(d[1], n)?, w)?)?,
                scale,
            )?));
        }
        let tracking = Curve2::Line(line_through(&through, d)?);
        if let Some(second) = &self.track2 {
            let there = rat(&second.through)?;
            let d2 = [
                i128::from(second.direction[0]),
                i128::from(second.direction[1]),
            ];
            let other = Curve2::Line(line_through(&there, d2)?);
            return crossing_point(carriers, &tracking, &other, 0);
        }
        let with = self.with.as_ref().ok_or(SnapError::Missing(
            "a second line, an entity or a parameter",
        ))?;
        let subject = subject(with, drawing, lookup)?;
        let other = curve_of_subject(&subject, self.kind)?;
        let point = crossing_point(carriers, &other, &tracking, self.pick)?;
        if let Subject::Entity(e) = &subject
            && !within(&point, e, carriers)?
        {
            return Err(SnapError::OffExtent);
        }
        Ok(point)
    }

    /// The tracking line a direction or parallel snap runs along: a point and a direction.
    fn tracking_line(
        &self,
        drawing: &Drawing,
        lookup: &Lookup<'_>,
    ) -> Result<(Rational2, [i128; 2]), SnapError> {
        if self.kind == SnapKind::Parallel {
            let from = rat(self.from.as_ref().ok_or(SnapError::Missing("a point"))?)?;
            let on = self.on.as_ref().ok_or(SnapError::Missing("a line"))?;
            let line = match subject(on, drawing, lookup)? {
                Subject::Carrier(Curve2::Line(l)) => l,
                Subject::Entity(e) => match e.shape() {
                    Shape::Line { carrier, .. } => *carrier,
                    _ => return Err(unsupported(SnapKind::Parallel, Some(e))),
                },
                Subject::Carrier(_) => {
                    return Err(SnapError::Unsupported(
                        SnapKind::Parallel,
                        crate::entity::Tag::Segment,
                    ));
                }
            };
            return Ok((from, line.direction()));
        }
        let t = self
            .track
            .ok_or(SnapError::Missing("a line to track along"))?;
        let d = [i128::from(t.direction[0]), i128::from(t.direction[1])];
        if d == [0, 0] {
            return Err(SnapError::NoDirection);
        }
        Ok((rat(&t.through)?, d))
    }

    /// The crossing of two entities' carriers, `pick` choosing which, on both extents.
    fn crossing(
        &self,
        drawing: &Drawing,
        lookup: &Lookup<'_>,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        let on = self.on.as_ref().ok_or(SnapError::Missing("an entity"))?;
        let with = self
            .with
            .as_ref()
            .ok_or(SnapError::Missing("a second entity"))?;
        let first = subject(on, drawing, lookup)?;
        let second = subject(with, drawing, lookup)?;
        self.cross_subjects(&first, &second, carriers)
    }

    /// The crossing of two subjects' carriers, `pick` choosing which, on both extents.
    ///
    /// # Errors
    ///
    /// As [`SnapRef::resolve`], but for a reference.
    pub(crate) fn cross_subjects(
        &self,
        first: &Subject<'_>,
        second: &Subject<'_>,
        carriers: &mut Carriers,
    ) -> Result<Pt2, SnapError> {
        let (a, b) = (
            curve_of_subject(first, self.kind)?,
            curve_of_subject(second, self.kind)?,
        );
        let point = crossing_point(carriers, &a, &b, self.pick)?;
        for s in [first, second] {
            if let Subject::Entity(e) = s
                && !within(&point, e, carriers)?
            {
                return Err(SnapError::OffExtent);
            }
        }
        Ok(point)
    }
}

/// The entity of a subject, if it is one.
fn entity_of<'a>(s: &Subject<'a>) -> Option<&'a Entity> {
    match s {
        Subject::Entity(e) => Some(*e),
        Subject::Carrier(_) => None,
    }
}

fn unsupported(kind: SnapKind, entity: Option<&Entity>) -> SnapError {
    match entity {
        Some(e) => SnapError::Unsupported(kind, e.tag()),
        None => SnapError::Missing("an entity, not only a carrier"),
    }
}

/// The midpoint of two rational points.
fn midpoint(a: &Rational2, b: &Rational2) -> Result<Rational2, SnapError> {
    let ([ax, ay], aw) = a.coordinates();
    let ([bx, by], bw) = b.coordinates();
    rational(
        add(mul(ax, bw)?, mul(bx, aw)?)?,
        add(mul(ay, bw)?, mul(by, aw)?)?,
        mul(mul(aw, bw)?, 2)?,
    )
}

/// A + (B − A) · t, for the dyadic t.
fn along(a: &Rational2, b: &Rational2, t: ExactParam) -> Result<Pt2, SnapError> {
    let ([ax, ay], aw) = a.coordinates();
    let ([bx, by], bw) = b.coordinates();
    let den = t.denominator();
    let n = i128::from(t.numerator);
    let w = mul(mul(aw, bw)?, den)?;
    // A(1 − t) + B t over the weights: (ax bw (den − n) + bx aw n) / (aw bw den).
    let rest = sub(den, n)?;
    Ok(point_of(rational(
        add(mul(mul(ax, bw)?, rest)?, mul(mul(bx, aw)?, n)?)?,
        add(mul(mul(ay, bw)?, rest)?, mul(mul(by, aw)?, n)?)?,
        w,
    )?))
}

/// The squared radius of a circle as a fraction.
fn radius_squared(circle: &Circle2) -> Result<(i128, i128), SnapError> {
    match circle {
        Circle2::Grid(c) => Ok((mul(c.radius(), c.radius())?, 1)),
        Circle2::Round(r) => {
            let on = r.on();
            let c = r.centre();
            let ([cx, cy], cw) = c.coordinates();
            let dx = sub(mul(i128::from(on.x.get()), cw)?, cx)?;
            let dy = sub(mul(i128::from(on.y.get()), cw)?, cy)?;
            Ok((add(mul(dx, dx)?, mul(dy, dy)?)?, mul(cw, cw)?))
        }
    }
}

/// The point of an ellipse at the end `param` of an elliptical arc, which is rational.
fn ellipse_point(ellipse: &geom_num::conic::Ellipse2, param: &Param) -> Result<Pt2, SnapError> {
    let (p, q) = ellipse.ratio();
    let [mx, my] = ellipse.major();
    let (p, q) = (i128::from(p), i128::from(q));
    let (mx, my) = (i128::from(mx), i128::from(my));
    let cx = i128::from(ellipse.centre().x.get());
    let cy = i128::from(ellipse.centre().y.get());
    // x = a (1 − s²) / (1 + s²) along the major axis, y = b · 2s / (1 + s²) along the minor, with
    // s = n / d: (d² − n²) / (d² + n²) and 2 n d / (d² + n²).
    let (u_num, v_num, den) = match param {
        Param::Far => (-1_i128, 0_i128, 1_i128),
        Param::At(s) => {
            let (n, d) = (i128::from(s.numerator()), i128::from(s.denominator()));
            let (nn, dd) = (mul(n, n)?, mul(d, d)?);
            (sub(dd, nn)?, mul(mul(2, n)?, d)?, add(dd, nn)?)
        }
    };
    // C + u M + v N with N = (p/q)(−my, mx): over den · q.
    let w = mul(den, q)?;
    let x = add(
        add(mul(mul(cx, den)?, q)?, mul(mul(u_num, mx)?, q)?)?,
        neg(mul(mul(v_num, my)?, p)?)?,
    )?;
    let y = add(
        add(mul(mul(cy, den)?, q)?, mul(mul(u_num, my)?, q)?)?,
        mul(mul(v_num, mx)?, p)?,
    )?;
    Ok(point_of(rational(x, y, w)?))
}

/// The centroid of a closed polyline of straight segments.
fn centroid(p: &crate::entity::Polyline) -> Result<Pt2, SnapError> {
    if !p.is_closed() || !p.is_straight() {
        return Err(SnapError::Unsupported(
            SnapKind::Centroid,
            crate::entity::Tag::Polyline,
        ));
    }
    let vs = p.vertices();
    let (mut area, mut sx, mut sy) = (0_i128, 0_i128, 0_i128);
    for (i, a) in vs.iter().enumerate() {
        let b = vs
            .get(i.checked_add(1).ok_or(SnapError::Param)?)
            .or_else(|| vs.first())
            .ok_or(SnapError::Param)?;
        let (ax, ay) = (i128::from(a.x.get()), i128::from(a.y.get()));
        let (bx, by) = (i128::from(b.x.get()), i128::from(b.y.get()));
        let cross = sub(mul(ax, by)?, mul(bx, ay)?)?;
        area = add(area, cross)?;
        sx = add(sx, mul(add(ax, bx)?, cross)?)?;
        sy = add(sy, mul(add(ay, by)?, cross)?)?;
    }
    if area == 0 {
        return Err(SnapError::NoDirection);
    }
    // A = area / 2, so C = S / (6 A) = S / (3 · area).
    Ok(point_of(rational(sx, sy, mul(area, 3)?)?))
}
