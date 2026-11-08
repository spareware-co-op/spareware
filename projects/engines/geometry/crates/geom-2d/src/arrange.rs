//! The arrangement of line segments, circular arcs and ellipses (chapter 14.4): an exact plane sweep,
//! overlap merging, the order of edges at each vertex, the faces and the unbounded face, and where
//! each edge came from.
//!
//! What it does: [`arrange`] takes segments between grid points, and [`arrange_curves`] segments,
//! arcs and whole ellipses ([`Curve`]), and each returns the [`Arrangement`] they divide the plane
//! into: the vertices (every end point, every crossing, and where an arc's circle or an ellipse is
//! leftmost or rightmost, in lexicographic order), the edges between consecutive vertices along each
//! curve (curves that share a stretch share its edges, and an edge names every curve that covers
//! it), what each edge lies on ([`Along`]: a line's smallest carrier, or one half of a listed circle
//! or ellipse), each vertex's
//! edges in counter-clockwise order, and the faces: one unbounded face, and one bounded face for
//! each outer boundary, with the boundaries of the components inside it as holes. A vertex is an
//! [`At`]: a rational point, or a one-root point where arcs or ellipses come in. An ellipse meets a
//! line at one-root points; where it may meet a circle or another ellipse, at a root of a quartic,
//! the sweep refuses ([`ArrangeError::Conic`]).
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is a sign of one of `geom-num`'s exact predicates on carriers,
//!   circles, ellipses, rational points and one-root points ([`Carrier::side`], [`Carrier::meet`],
//!   [`Rational2::cmp_x`], `Crossing2`'s comparisons and crossings, `Ellipse2`'s sides and meetings,
//!   and the tangent and curvature orders of the `piece` module); no coordinate is ever rounded, and
//!   a crossing is the exact point where two carriers meet. A step past the exact budget is
//!   [`ArrangeError::Range`], never a wrong answer, and a meeting of an ellipse with a circle or
//!   another ellipse that the pieces may hold is [`ArrangeError::Conic`], never a missing vertex.
//! - **Permutation-invariant.** Vertices are named by their rank in lexicographic order, edges by
//!   their end vertices and the order they leave the lower one, faces by the rank of their
//!   lowest-leftmost vertex, circles by centre and radius, ellipses by centre and form; the only thing
//!   that depends on the input order is the label of each source, which is the curve's place in the
//!   input. Permuting the input gives the same arrangement with the labels permuted.
//! - **Overlaps merge, touching splits.** A curve through another's end point is split there; two
//!   segments on one line, or two arcs of one circle, share the edges of their common stretch; a
//!   tangency is one vertex where the curves touch.
//! - **Cancellable.** The sweep charges its [`WorkMeter`] once for each event point.
//!
//! Method: Bentley and Ottmann's sweep ([DOI](https://doi.org/10.1109/TC.1979.1675432)) in the form
//! of de Berg, Cheong, van Kreveld and Overmars, *Computational Geometry*, 3rd ed., Springer 2008,
//! section 2.1 (doi:10.1007/978-3-540-77974-2), with the sweep line tilted so that a vertical segment
//! is the steepest upward one, with every comparison an exact sign, and generalised to arcs and
//! ellipses cut into x-monotone pieces as the `piece` module says; the doubly-connected edge list and
//! the placing of holes by a leftward ray after the same book, section 2.2.
//!
//! Cost: O((n + k) log n) comparisons for the sweep, n curves and k crossings, plus the cost of
//! moving the entries of the status list on insertion, which is linear in the pieces the sweep
//! line crosses at once; the faces are linear in the edges, and finding the face around each hole
//! is a second sweep over the finished edges. An arc is at most three pieces and an ellipse two.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "s, t, p, q, u, v, e, h are the geometry's own letters, and the sweep and the face walk are each one procedure of de Berg et al. that reads best whole"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_curve::{CurveKind, Line};
use geom_math::bbox::Box3;
use geom_num::arc::{Arc2, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::Ellipse2;
use geom_num::grid::{P2, P3};
use geom_num::one_root::Crossing2;
use geom_num::{NumError, Sign};
use geom_run::{Stopped, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge as TopoEdge, Face as TopoFace, Fin, Ix, Loop, Region, RegionKind,
    Sense, Shell, TopoError, Vertex,
};

mod piece;

use piece::{
    On, Piece, Shapes, Way, circles_of, cmp_pt, cut, cut_oval, cut_oval_arc, meetings, order_at,
    ordering, ovals_of, side, way,
};

use crate::entity::Param;

/// Why an arrangement was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrangeError {
    /// A step of the exact arithmetic did not fit its budget.
    Range,
    /// More curves, vertices or edges than a 32-bit index names.
    TooMany,
    /// The work meter stopped the sweep.
    Stopped(Stopped),
    /// An ellipse may meet a circle or another ellipse on the pieces the sweep holds: they meet at
    /// roots of a quartic, which the sweep does not yet place as vertices.
    Conic,
}

impl From<NumError> for ArrangeError {
    fn from(_: NumError) -> Self {
        ArrangeError::Range
    }
}

impl From<Stopped> for ArrangeError {
    fn from(s: Stopped) -> Self {
        ArrangeError::Stopped(s)
    }
}

impl fmt::Display for ArrangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArrangeError::Range => f.write_str("a step of the exact arithmetic did not fit"),
            ArrangeError::TooMany => f.write_str("more items than a 32-bit index names"),
            ArrangeError::Stopped(s) => write!(f, "stopped: {s:?}"),
            ArrangeError::Conic => f.write_str(
                "an ellipse may meet a circle or another ellipse, at a root of a quartic the sweep does not place",
            ),
        }
    }
}

impl core::error::Error for ArrangeError {}

/// Where a vertex is: a rational point, or a one-root point.
///
/// Where an arc meets a line or another arc, and where the sweep cuts an arc at the leftmost or
/// rightmost point of its circle, is a one-root point ([`Crossing2`]); every other vertex is
/// rational. A one-root point may happen to be rational as well, so two vertices are one place by
/// [`At::same`], never by their variants.
#[derive(Clone, Debug)]
pub enum At {
    /// A rational point.
    Rational(Rational2),
    /// A one-root point.
    Root(Box<Crossing2>),
}

impl At {
    /// The rational point, when the vertex is stored as one.
    #[must_use]
    pub const fn rational(&self) -> Option<&Rational2> {
        match self {
            At::Rational(r) => Some(r),
            At::Root(_) => None,
        }
    }

    /// The vertex as a one-root point, the form every vertex has.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which no point in the budget can reach.
    pub fn crossing(&self) -> Result<Crossing2, NumError> {
        match self {
            At::Rational(r) => Crossing2::of_point(r),
            At::Root(c) => Ok(**c),
        }
    }

    /// The sign of this point's x less `other`'s (`across`) or of its y (`!across`).
    fn compare(&self, other: &At, across: bool) -> Result<Sign, NumError> {
        let pick = |a: &Crossing2, b: &Crossing2| {
            if across { a.cmp_x(b) } else { a.cmp_y(b) }
        };
        match (self, other) {
            (At::Rational(a), At::Rational(b)) => {
                if across {
                    a.cmp_x(b)
                } else {
                    a.cmp_y(b)
                }
            }
            (At::Root(a), At::Root(b)) => pick(a, b),
            (At::Rational(a), At::Root(b)) => pick(&Crossing2::of_point(a)?, b),
            (At::Root(a), At::Rational(b)) => pick(a, &Crossing2::of_point(b)?),
        }
    }

    /// How this point's x compares with `other`'s: the sign of x less `other`'s x.
    ///
    /// # Errors
    ///
    /// As for [`At::crossing`].
    pub fn cmp_x(&self, other: &At) -> Result<Sign, NumError> {
        self.compare(other, true)
    }

    /// How this point's y compares with `other`'s, as [`At::cmp_x`] says of x.
    ///
    /// # Errors
    ///
    /// As for [`At::crossing`].
    pub fn cmp_y(&self, other: &At) -> Result<Sign, NumError> {
        self.compare(other, false)
    }

    /// Whether this point and `other` are one place, however each is stored.
    ///
    /// # Errors
    ///
    /// As for [`At::crossing`].
    pub fn same(&self, other: &At) -> Result<bool, NumError> {
        Ok(self.cmp_x(other)? == Sign::Zero && self.cmp_y(other)? == Sign::Zero)
    }
}

/// What an edge lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    /// A line: its smallest carrier (a and b with no common factor), running from the edge's
    /// `from` to its `to`, so that the same line always has the same carrier.
    Line(Carrier),
    /// An arc of the circle [`Arrangement::circles`] holds at `circle`: on its upper half, which
    /// runs from the circle's leftmost point over the top to its rightmost, or on its lower half.
    Arc {
        /// The circle's place in [`Arrangement::circles`].
        circle: u32,
        /// Whether the edge is on the upper half.
        upper: bool,
    },
    /// A piece of the ellipse [`Arrangement::ellipses`] holds at `ellipse`: on its upper half, from
    /// its leftmost point over the top to its rightmost, or on its lower half; the halves are split
    /// by the diameter through those two points.
    Ellipse {
        /// The ellipse's place in [`Arrangement::ellipses`].
        ellipse: u32,
        /// Whether the edge is on the upper half.
        upper: bool,
    },
}

/// An edge between two vertices, lowest first, with the curves that cover it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The vertex of the lexicographically lower end.
    pub from: u32,
    /// The vertex of the higher end.
    pub to: u32,
    /// The input places of the curves that cover the edge, ascending.
    pub sources: Vec<u32>,
}

/// A face: its outer boundary (none for the unbounded face) and the boundaries of the components
/// that lie inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// The index of the cycle that bounds the face, counter-clockwise; `None` for the unbounded face.
    pub outer: Option<u32>,
    /// The cycles of the components inside, each the outside boundary of its component, ascending.
    pub holes: Vec<u32>,
}

/// The arrangement of a set of segments.
#[derive(Clone, Debug)]
pub struct Arrangement {
    vertices: Vec<At>,
    edges: Vec<Edge>,
    /// What each edge lies on, by edge.
    along: Vec<Along>,
    /// The circles the arcs lie on, each once.
    circles: Vec<Round>,
    /// The ellipses, each once.
    ellipses: Vec<Ellipse2>,
    /// For each vertex, the half-edges that leave it, counter-clockwise from the negative-x
    /// direction. Half-edge `2 e` runs along edge `e` from `from` to `to`; `2 e + 1` runs back.
    around: Vec<Vec<u32>>,
    cycles: Vec<Vec<u32>>,
    faces: Vec<Face>,
}

impl Arrangement {
    /// The vertices, in lexicographic order of (x, y).
    #[must_use]
    pub fn vertices(&self) -> &[At] {
        &self.vertices
    }

    /// The edges, in order of (`from`, `to`); edges between the same two vertices (a chord and its
    /// arc, or two arcs) in the counter-clockwise order they leave `from`.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// What the edge `edge` lies on.
    #[must_use]
    pub fn along(&self, edge: u32) -> Option<Along> {
        usize::try_from(edge)
            .ok()
            .and_then(|i| self.along.get(i))
            .copied()
    }

    /// The circles the arcs lie on, each once, in order of centre (x, then y), then of radius.
    #[must_use]
    pub fn circles(&self) -> &[Round] {
        &self.circles
    }

    /// The ellipses, each once, in order of centre (x, then y), then of the ratios of their forms'
    /// coefficients to their levels. Of the copies of one ellipse, the one whose major-axis vector,
    /// then ratio, has the least integers is the one stored.
    #[must_use]
    pub fn ellipses(&self) -> &[Ellipse2] {
        &self.ellipses
    }

    /// The half-edges leaving `vertex`, counter-clockwise from the negative x direction: one that
    /// leaves exactly westwards first, one that leaves exactly eastwards after those that leave
    /// below it. Half-edge `2 e` runs from edge `e`'s `from` to its `to`, and `2 e + 1` back.
    #[must_use]
    pub fn around(&self, vertex: u32) -> &[u32] {
        usize::try_from(vertex)
            .ok()
            .and_then(|i| self.around.get(i))
            .map_or(&[], Vec::as_slice)
    }

    /// The boundary cycles, each a list of half-edges in order, the face on the left of each.
    #[must_use]
    pub fn cycles(&self) -> &[Vec<u32>] {
        &self.cycles
    }

    /// The faces: the unbounded face first, then one for each counter-clockwise cycle, by the rank
    /// of the cycle's lowest-leftmost vertex.
    #[must_use]
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }
}

/// A piece of linework to arrange.
#[derive(Clone, Copy, Debug)]
pub enum Curve {
    /// A segment between two grid points; one of no length is ignored.
    Segment(P2, P2),
    /// An arc: grid ends and a rational centre on their bisector, running counter-clockwise from its
    /// start to its end.
    Arc(Arc2),
    /// A whole ellipse. One of ratio 1 is a circle through its centre plus its major-axis vector,
    /// and is arranged as one, so that it shares the edges of any arc of that circle.
    Ellipse(Ellipse2),
    /// An arc of an ellipse, counter-clockwise from the point at parameter `start` to the point at
    /// `end` (`entity::Param`: x = a(1 − s²) / (1 + s²), y = b · 2s / (1 + s²) along its axes), the
    /// whole ellipse when they are equal. Its ends are the rational points those parameters give;
    /// one past the budget of a rational point is refused. It is arranged as an ellipse whatever its
    /// ratio, so an arc of ratio 1 beside a circle of the same curve is refused as one that may meet
    /// it ([`ArrangeError::Conic`]).
    EllipticArc {
        /// The ellipse.
        ellipse: Ellipse2,
        /// Where it starts.
        start: Param,
        /// Where it ends.
        end: Param,
    },
}

/// The point of `e` at the parameter `s`: for s = n / d, K + (M(d² − n²) + (p / q) M⊥ · 2nd) /
/// (d² + n²), M⊥ = (−My, Mx), over q(d² + n²); K − M at the far end of the major axis.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit, or the point is past a rational point's budget.
fn param_point(e: &Ellipse2, s: &Param) -> Result<Rational2, ArrangeError> {
    let k = e.centre();
    let [mx, my] = e.major().map(i128::from);
    let (kx, ky) = (i128::from(k.x.get()), i128::from(k.y.get()));
    let fit = |v: Option<i128>| v.ok_or(ArrangeError::Range);
    let (n, d) = match s {
        Param::Far => {
            let far = Rational2::new(fit(kx.checked_sub(mx))?, fit(ky.checked_sub(my))?, 1);
            return Ok(far?);
        }
        Param::At(f) => (i128::from(f.numerator()), i128::from(f.denominator())),
    };
    let (p, q) = e.ratio();
    let (p, q) = (i128::from(p), i128::from(q));
    let (dd, nn) = (fit(d.checked_mul(d))?, fit(n.checked_mul(n))?);
    let (sum, difference) = (fit(dd.checked_add(nn))?, fit(dd.checked_sub(nn))?);
    let twice = fit(n.checked_mul(d).and_then(|v| v.checked_mul(2)))?;
    let w = fit(q.checked_mul(sum))?;
    let coordinate = |c: i128, along: i128, across: i128| -> Result<i128, ArrangeError> {
        let centre = fit(c.checked_mul(w))?;
        let on = fit(q.checked_mul(along).and_then(|v| v.checked_mul(difference)))?;
        let turned = fit(p.checked_mul(across).and_then(|v| v.checked_mul(twice)))?;
        fit(centre.checked_add(on).and_then(|v| v.checked_add(turned)))
    };
    let neg = fit(my.checked_neg())?;
    Ok(Rational2::new(
        coordinate(kx, mx, neg)?,
        coordinate(ky, my, mx)?,
        w,
    )?)
}

/// One event: a point, and the piece that starts there if it is a start.
struct Event {
    at: At,
    start: Option<u32>,
}

/// A binary min-heap of events by lexicographic point, with comparisons that can fail.
struct Heap {
    items: Vec<Event>,
}

impl Heap {
    /// Whether the event at place `a` comes before the one at place `b`.
    fn before(&self, a: usize, b: usize) -> Result<bool, ArrangeError> {
        let at = |k: usize| {
            self.items
                .get(k)
                .map(|e| &e.at)
                .ok_or(ArrangeError::TooMany)
        };
        Ok(cmp_pt(at(a)?, at(b)?)? == Ordering::Less)
    }

    fn push(&mut self, e: Event) -> Result<(), ArrangeError> {
        self.items.push(e);
        let mut i = self.items.len().saturating_sub(1);
        while let Some(parent) = i.checked_sub(1).map(|j| j / 2) {
            if !self.before(i, parent)? {
                break;
            }
            self.items.swap(i, parent);
            i = parent;
        }
        Ok(())
    }

    fn peek(&self) -> Option<&Event> {
        self.items.first()
    }

    fn pop(&mut self) -> Result<Option<Event>, ArrangeError> {
        let Some(last) = self.items.len().checked_sub(1) else {
            return Ok(None);
        };
        self.items.swap(0, last);
        let out = self.items.pop();
        let n = self.items.len();
        let mut i = 0_usize;
        // Each step goes a level down, so fewer than n steps end it.
        for _ in 0..n {
            let l = i.saturating_mul(2).saturating_add(1);
            let r = l.saturating_add(1);
            let mut least = i;
            for c in [l, r] {
                if c < n && self.before(c, least)? {
                    least = c;
                }
            }
            if least == i {
                break;
            }
            self.items.swap(i, least);
            i = least;
        }
        Ok(out)
    }
}

fn index(i: usize) -> Result<u32, ArrangeError> {
    u32::try_from(i).map_err(|_| ArrangeError::TooMany)
}

/// Keeps the first refusal of a comparison made inside a sort, which cannot return one, and lets
/// the sort go on as if the two were equal; the caller returns the refusal once the sort ends.
fn failed(slot: &mut Result<(), ArrangeError>, e: ArrangeError) -> Ordering {
    if slot.is_ok() {
        *slot = Err(e);
    }
    Ordering::Equal
}

/// Puts `v` on the list at `at` of `lists`.
fn mark(lists: &mut [Vec<u32>], at: u32, v: u32) -> Result<(), ArrangeError> {
    let i = usize::try_from(at).map_err(|_| ArrangeError::TooMany)?;
    lists.get_mut(i).ok_or(ArrangeError::TooMany)?.push(v);
    Ok(())
}

/// Sets the entry at `at` of `slots` to `value`.
fn set<T>(slots: &mut [T], at: usize, value: T) -> Result<(), ArrangeError> {
    *slots.get_mut(at).ok_or(ArrangeError::TooMany)? = value;
    Ok(())
}

/// The greatest common divisor, by Euclid's algorithm: two lines of arithmetic, which the kernel's
/// crates take from no outside crate (their one vetted dependency is `libm`).
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while let Some(r) = a.checked_rem(b) {
        a = b;
        b = r;
    }
    a
}

/// The carrier with a and b divided by their greatest common divisor, which divides c too (c is
/// a · x + b · y at a grid point), so one line always has one carrier, and its direction unturned.
fn smallest(line: Carrier) -> Result<Carrier, ArrangeError> {
    let ([a, b], c) = line.coefficients();
    let part = |v: i128| u64::try_from(v.unsigned_abs()).map_err(|_| ArrangeError::Range);
    let g = i128::from(gcd(part(a)?, part(b)?));
    let cut = |v: i128| v.checked_div(g).ok_or(ArrangeError::Range);
    Ok(Carrier::new(cut(a)?, cut(b)?, cut(c)?)?)
}

/// The piece of the segment from `a` to `b`, lower end first, or none for a segment of no length.
fn segment(a: P2, b: P2) -> Result<Option<Piece>, ArrangeError> {
    let (ra, rb) = (Rational2::from(a), Rational2::from(b));
    let (lo, hi, p, q) = match ordering(ra.cmp_x(&rb)?).then(ordering(ra.cmp_y(&rb)?)) {
        Ordering::Equal => return Ok(None),
        Ordering::Less => (ra, rb, a, b),
        Ordering::Greater => (rb, ra, b, a),
    };
    let line = smallest(Carrier::through(p, q)?)?;
    Ok(Some(Piece {
        lo: At::Rational(lo),
        hi: At::Rational(hi),
        on: On::Line {
            line,
            dir: line.direction(),
        },
    }))
}

fn piece_of(pieces: &[Piece], id: u32) -> Result<&Piece, ArrangeError> {
    usize::try_from(id)
        .ok()
        .and_then(|i| pieces.get(i))
        .ok_or(ArrangeError::TooMany)
}

/// Pushes as events the points after `p` where pieces `s` and `t` meet.
fn new_events(
    heap: &mut Heap,
    pieces: &[Piece],
    shapes: &Shapes,
    s: u32,
    t: u32,
    p: &At,
) -> Result<(), ArrangeError> {
    let (a, b) = (piece_of(pieces, s)?, piece_of(pieces, t)?);
    for q in meetings(a, b, shapes, p)?.into_iter().flatten() {
        heap.push(Event { at: q, start: None })?;
    }
    Ok(())
}

/// The arrangement of `segments`, each between two grid points; segments of no length are ignored.
///
/// # Errors
///
/// [`ArrangeError::Range`] past the exact budget, [`ArrangeError::TooMany`] past 2^32 items and
/// [`ArrangeError::Stopped`] if the meter stops.
pub fn arrange(
    segments: &[(P2, P2)],
    meter: &mut WorkMeter<'_>,
) -> Result<Arrangement, ArrangeError> {
    let mut pieces: Vec<Piece> = Vec::with_capacity(segments.len());
    let mut ids: Vec<u32> = Vec::with_capacity(segments.len());
    for (place, (a, b)) in segments.iter().enumerate() {
        if let Some(piece) = segment(*a, *b)? {
            pieces.push(piece);
            ids.push(index(place)?);
        }
    }
    sweep(&pieces, &ids, Shapes::default(), meter)
}

/// A whole ellipse of ratio 1 as the two halves of its circle, counter-clockwise from its centre
/// plus its major-axis vector round to its centre less it and back; `None` for any other ellipse,
/// or for one whose points there are off the grid.
fn as_circle(e: &Ellipse2) -> Option<[Arc2; 2]> {
    if e.ratio() != (1, 1) {
        return None;
    }
    let (k, [mx, my]) = (e.centre(), e.major());
    let at =
        |sx: i64, sy: i64| P2::new(k.x.get().checked_add(sx)?, k.y.get().checked_add(sy)?).ok();
    let (start, end) = (at(mx, my)?, at(mx.checked_neg()?, my.checked_neg()?)?);
    let centre = Rational2::from(k);
    Some([
        Arc2::new(start, end, centre).ok()?,
        Arc2::new(end, start, centre).ok()?,
    ])
}

/// The arrangement of `curves`: segments between grid points, arcs, whole ellipses and elliptic
/// arcs. Each arc or ellipse is cut where its circle or ellipse is leftmost or rightmost, so its
/// pieces run one way in x, and those cuts are vertices; an edge on an arc or an ellipse names its
/// place as its source, as a segment's edges name the segment's.
///
/// # Errors
///
/// As for [`arrange`], and [`ArrangeError::Conic`] where an ellipse may meet a circle or another
/// ellipse.
pub fn arrange_curves(
    curves: &[Curve],
    meter: &mut WorkMeter<'_>,
) -> Result<Arrangement, ArrangeError> {
    let mut arcs: Vec<Arc2> = Vec::new();
    let mut ellipses: Vec<Ellipse2> = Vec::new();
    for c in curves {
        match c {
            Curve::Arc(a) => arcs.push(*a),
            Curve::Ellipse(e) => match as_circle(e) {
                Some(halves) => arcs.extend(halves),
                None => ellipses.push(*e),
            },
            Curve::EllipticArc { ellipse, .. } => ellipses.push(*ellipse),
            Curve::Segment(..) => {}
        }
    }
    let (circles, circle_of_arc) = circles_of(&arcs)?;
    let (ovals, oval_of_ellipse) = ovals_of(&ellipses)?;
    let mut pieces: Vec<Piece> = Vec::with_capacity(curves.len());
    let mut ids: Vec<u32> = Vec::with_capacity(curves.len());
    let (mut nth_arc, mut nth_ellipse) = (0_usize, 0_usize);
    let mut next_arc = |arc: &Arc2, pieces: &mut Vec<Piece>| -> Result<(), ArrangeError> {
        let circle = circle_of_arc
            .get(nth_arc)
            .copied()
            .ok_or(ArrangeError::TooMany)?;
        nth_arc = nth_arc.saturating_add(1);
        cut(arc, circle, &circles, pieces)
    };
    for (place, curve) in curves.iter().enumerate() {
        let before = pieces.len();
        match curve {
            Curve::Segment(a, b) => {
                if let Some(piece) = segment(*a, *b)? {
                    pieces.push(piece);
                }
            }
            Curve::Arc(arc) => next_arc(arc, &mut pieces)?,
            Curve::Ellipse(e) => {
                if let Some(halves) = as_circle(e) {
                    for arc in &halves {
                        next_arc(arc, &mut pieces)?;
                    }
                } else {
                    let ellipse = oval_of_ellipse
                        .get(nth_ellipse)
                        .copied()
                        .ok_or(ArrangeError::TooMany)?;
                    nth_ellipse = nth_ellipse.saturating_add(1);
                    cut_oval(ellipse, &ovals, &mut pieces)?;
                }
            }
            Curve::EllipticArc {
                ellipse: e,
                start,
                end,
            } => {
                let ellipse = oval_of_ellipse
                    .get(nth_ellipse)
                    .copied()
                    .ok_or(ArrangeError::TooMany)?;
                nth_ellipse = nth_ellipse.saturating_add(1);
                let (from, to) = (param_point(e, start)?, param_point(e, end)?);
                cut_oval_arc(ellipse, &ovals, &from, &to, &mut pieces)?;
            }
        }
        let place = index(place)?;
        ids.extend(core::iter::repeat_n(
            place,
            pieces.len().saturating_sub(before),
        ));
    }
    sweep(&pieces, &ids, Shapes { circles, ovals }, meter)
}

/// The plane sweep over `pieces`, the input place of each in `ids`.
fn sweep(
    pieces: &[Piece],
    ids: &[u32],
    shapes: Shapes,
    meter: &mut WorkMeter<'_>,
) -> Result<Arrangement, ArrangeError> {
    let mut heap = Heap {
        items: Vec::with_capacity(pieces.len().saturating_mul(2)),
    };
    for (i, piece) in pieces.iter().enumerate() {
        let first = Event {
            at: piece.lo.clone(),
            start: Some(index(i)?),
        };
        heap.push(first)?;
        let last = Event {
            at: piece.hi.clone(),
            start: None,
        };
        heap.push(last)?;
    }
    // The status: the pieces the sweep line crosses, lowest first.
    let mut status: Vec<u32> = Vec::new();
    // The vertices, and for each piece the vertices on it, in order.
    let mut vertices: Vec<At> = Vec::new();
    let mut on: Vec<Vec<u32>> = alloc::vec![Vec::new(); pieces.len()];
    while let Some(first) = heap.pop()? {
        meter.charge(1)?;
        let mut p = first.at;
        let mut starting: Vec<u32> = first.start.into_iter().collect();
        while let Some(next) = heap.peek() {
            if cmp_pt(&next.at, &p)? != Ordering::Equal {
                break;
            }
            let e = heap.pop()?.ok_or(ArrangeError::TooMany)?;
            starting.extend(e.start);
            // One place may come as a crossing and as a rational point: keep the rational.
            if let (None, Some(_)) = (p.rational(), e.at.rational()) {
                p = e.at;
            }
        }
        // The status pieces through p are contiguous: those with p above them come first.
        // lo: first entry with p not above; hi: first entry with p below.
        let (mut a, mut b) = (0_usize, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let s = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if side(piece_of(pieces, s)?, &shapes, &p)? == Sign::Positive {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let lo = a;
        let (mut a, mut b) = (lo, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let s = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if side(piece_of(pieces, s)?, &shapes, &p)? == Sign::Zero {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let hi = a;
        // Every event lies on a piece the status holds or one that starts there: a piece's own
        // ends, and where two pieces of the status meet, which is on both. So every event is a
        // vertex.
        let through: Vec<u32> = status.get(lo..hi).map(<[u32]>::to_vec).unwrap_or_default();
        let v = index(vertices.len())?;
        let mut keep: Vec<u32> = Vec::new();
        for &s in &through {
            mark(&mut on, s, v)?;
            if cmp_pt(&piece_of(pieces, s)?.hi, &p)? != Ordering::Equal {
                keep.push(s);
            }
        }
        for &s in &starting {
            mark(&mut on, s, v)?;
        }
        // Those that go on, in the order just after p: by tangent, then curvature, lowest first.
        keep.extend(starting.iter().copied());
        let mut fail = Ok(());
        keep.sort_by(|&x, &y| {
            let order = leaving(pieces, &shapes, &p, x, y).map(|o| o.then(x.cmp(&y)));
            order.unwrap_or_else(|e| failed(&mut fail, e))
        });
        fail?;
        let count = keep.len();
        let tail = status.split_off(hi);
        status.truncate(lo);
        status.extend(keep);
        status.extend(tail);
        // The new neighbours: each side of the pieces that go on, and, unlike segments, which
        // meet once, each two of those pieces next to each other, which an arc can meet again.
        let end = lo.saturating_add(count);
        for at in lo..=end {
            if let (Some(l), Some(r)) = (
                at.checked_sub(1).and_then(|i| status.get(i)).copied(),
                status.get(at).copied(),
            ) {
                let inside = at != lo && at != end;
                let lines = matches!(piece_of(pieces, l)?.on, On::Line { .. })
                    && matches!(piece_of(pieces, r)?.on, On::Line { .. });
                if !(inside && lines) {
                    new_events(&mut heap, pieces, &shapes, l, r, &p)?;
                }
            }
        }
        vertices.push(p);
    }
    build(pieces, ids, shapes, vertices, &on)
}

/// Where the keys of [`class`] for ellipses start: past every circle's, as a circle's place is below
/// 2^32.
const ELLIPSE_CLASSES: u64 = 0x100_0000_0000;

/// The order in which pieces `x` and `y` leave `p`: by tangent, then curvature, lowest first.
fn leaving(
    pieces: &[Piece],
    shapes: &Shapes,
    p: &At,
    x: u32,
    y: u32,
) -> Result<Ordering, ArrangeError> {
    let wx = way(piece_of(pieces, x)?, shapes, false)?;
    let wy = way(piece_of(pieces, y)?, shapes, false)?;
    order_at(p, &wx, &wy)
}

/// What a piece's edges lie on, as a key: lines first, then each half of each circle, then each half
/// of each ellipse.
fn class(piece: &Piece) -> u64 {
    let half = |at: u32, upper: bool| {
        u64::from(at)
            .saturating_mul(2)
            .saturating_add(u64::from(upper))
            .saturating_add(1)
    };
    match piece.on {
        On::Line { .. } => 0,
        On::Arc { circle, upper } => half(circle, upper),
        On::Ellipse { ellipse, upper } => half(ellipse, upper).saturating_add(ELLIPSE_CLASSES),
    }
}

/// The edges, the order around each vertex and the faces, from the vertices on each piece.
fn build(
    pieces: &[Piece],
    ids: &[u32],
    shapes: Shapes,
    vertices: Vec<At>,
    on: &[Vec<u32>],
) -> Result<Arrangement, ArrangeError> {
    let vertex = |v: u32| -> Result<&At, ArrangeError> {
        usize::try_from(v)
            .ok()
            .and_then(|i| vertices.get(i))
            .ok_or(ArrangeError::TooMany)
    };
    // Edges: consecutive vertices along each piece, with the piece's input as a source. Two pieces
    // between the same two vertices share an edge when they lie on one line, or on one half of one
    // circle.
    let mut raw: Vec<(u32, u32, u64, u32, u32)> = Vec::new();
    for (si, list) in on.iter().enumerate() {
        let place = ids.get(si).copied().ok_or(ArrangeError::TooMany)?;
        let id = index(si)?;
        let kind = class(piece_of(pieces, id)?);
        // A piece gets each vertex once, in order along it, so neighbours on its list differ.
        for (&u, &v) in list.iter().zip(list.iter().skip(1)) {
            raw.push((u, v, kind, place, id));
        }
    }
    raw.sort_unstable();
    let mut grouped: Vec<(Edge, u32)> = Vec::new();
    let mut last: Option<(u32, u32, u64)> = None;
    for &(u, v, kind, place, id) in &raw {
        match grouped.last_mut() {
            Some((e, _)) if last == Some((u, v, kind)) => e.sources.push(place),
            _ => {
                grouped.push((
                    Edge {
                        from: u,
                        to: v,
                        sources: alloc::vec![place],
                    },
                    id,
                ));
                last = Some((u, v, kind));
            }
        }
    }
    // Edges between the same two vertices go in the order they leave the lower one.
    let mut fail = Ok(());
    grouped.sort_by(|(e, s), (f, t)| {
        (e.from, e.to).cmp(&(f.from, f.to)).then_with(|| {
            let order = vertex(e.from).and_then(|at| leaving(pieces, &shapes, at, *s, *t));
            order.unwrap_or_else(|err| failed(&mut fail, err))
        })
    });
    fail?;
    let mut edges: Vec<Edge> = Vec::with_capacity(grouped.len());
    let mut rep: Vec<u32> = Vec::with_capacity(grouped.len());
    for (e, s) in grouped {
        edges.push(e);
        rep.push(s);
    }
    let rep_of = |e: usize| -> Result<&Piece, ArrangeError> {
        piece_of(pieces, rep.get(e).copied().ok_or(ArrangeError::TooMany)?)
    };
    let way_of = |h: u32| -> Result<Way<'_>, ArrangeError> {
        let e = usize::try_from(h / 2).map_err(|_| ArrangeError::TooMany)?;
        way(rep_of(e)?, &shapes, !h.is_multiple_of(2))
    };
    // Around each vertex: the half-edges that leave it, counter-clockwise from the negative x axis.
    let mut around: Vec<Vec<u32>> = alloc::vec![Vec::new(); vertices.len()];
    for (ei, e) in edges.iter().enumerate() {
        let h = index(ei.checked_mul(2).ok_or(ArrangeError::TooMany)?)?;
        mark(&mut around, e.from, h)?;
        mark(&mut around, e.to, h.saturating_add(1))?;
    }
    let mut fail = Ok(());
    for (list, at) in around.iter_mut().zip(&vertices) {
        list.sort_by(|&x, &y| {
            let order = way_of(x).and_then(|wx| way_of(y).and_then(|wy| order_at(at, &wx, &wy)));
            order.unwrap_or_else(|e| failed(&mut fail, e))
        });
    }
    fail?;
    // The place of each half-edge in the list around its origin.
    let origin = |h: u32| -> Result<usize, ArrangeError> {
        let e = usize::try_from(h / 2).map_err(|_| ArrangeError::TooMany)?;
        let edge = edges.get(e).ok_or(ArrangeError::TooMany)?;
        usize::try_from(if h.is_multiple_of(2) {
            edge.from
        } else {
            edge.to
        })
        .map_err(|_| ArrangeError::TooMany)
    };
    let mut place_in: Vec<usize> = alloc::vec![0; edges.len().saturating_mul(2)];
    for list in &around {
        for (i, &h) in list.iter().enumerate() {
            let k = usize::try_from(h).map_err(|_| ArrangeError::TooMany)?;
            set(&mut place_in, k, i)?;
        }
    }
    // next(h): at the end of h, the half-edge clockwise from its twin.
    let next = |h: u32| -> Result<u32, ArrangeError> {
        let twin = h ^ 1;
        let at = origin(twin)?;
        let list = around.get(at).ok_or(ArrangeError::TooMany)?;
        let i = place_in
            .get(usize::try_from(twin).map_err(|_| ArrangeError::TooMany)?)
            .copied()
            .ok_or(ArrangeError::TooMany)?;
        let k = if i == 0 {
            list.len().saturating_sub(1)
        } else {
            i.saturating_sub(1)
        };
        list.get(k).copied().ok_or(ArrangeError::TooMany)
    };
    let total = edges.len().saturating_mul(2);
    let mut seen: Vec<bool> = alloc::vec![false; total];
    let mut cycles: Vec<Vec<u32>> = Vec::new();
    for start in 0..total {
        if seen.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut cycle: Vec<u32> = Vec::new();
        let mut h = index(start)?;
        // A cycle holds each half-edge once at most, so the walk is never longer than there are.
        for _ in 0..total {
            let k = usize::try_from(h).map_err(|_| ArrangeError::TooMany)?;
            if seen.get(k).copied().unwrap_or(true) {
                break;
            }
            set(&mut seen, k, true)?;
            cycle.push(h);
            h = next(h)?;
        }
        cycles.push(cycle);
    }
    // Orientation. A component's outside boundary is the cycle that has the wedge toward the
    // negative x direction at the component's lowest-leftmost vertex, where no half-edge leaves
    // westwards; every other cycle of the component bounds a face from inside. The wedge from the
    // last half-edge of a vertex's list round to its first holds the negative x direction, and is
    // the last half-edge's cycle's. So at a cycle's own lowest-leftmost vertex, the cycle is the
    // outside kind exactly when it holds the last half-edge there. A half-edge that leaves a vertex
    // westwards runs to a lower vertex, along a line or an arc cut where it turns back in x, so
    // where the list starts with one the last half-edge's cycle holds that lower vertex: the cycle
    // whose lowest-leftmost vertex this is cannot be it, and is not the outside kind.
    let mut cycle_of_half: Vec<usize> = alloc::vec![0; total];
    for (c, cycle) in cycles.iter().enumerate() {
        for &h in cycle {
            let k = usize::try_from(h).map_err(|_| ArrangeError::TooMany)?;
            set(&mut cycle_of_half, k, c)?;
        }
    }
    let mut counter: Vec<bool> = Vec::new();
    let mut low: Vec<usize> = Vec::new();
    for (c, cycle) in cycles.iter().enumerate() {
        // A cycle has half-edges, so its lowest vertex is one of theirs, never the starting bound.
        let m = cycle
            .iter()
            .try_fold(usize::MAX, |m, &h| origin(h).map(|o| m.min(o)))?;
        let list = around.get(m).ok_or(ArrangeError::TooMany)?;
        let out = list.last().copied().ok_or(ArrangeError::TooMany)?;
        let outside = usize::try_from(out)
            .ok()
            .and_then(|k| cycle_of_half.get(k))
            .copied()
            == Some(c);
        counter.push(!outside);
        low.push(m);
    }
    let found = Found {
        edges: &edges,
        vertices: &vertices,
        pieces,
        shapes: &shapes,
        rep: &rep,
    };
    let (faces, _) = faces_of(&found, &cycles, &counter, &low)?;
    let mut along: Vec<Along> = Vec::with_capacity(edges.len());
    for e in 0..edges.len() {
        along.push(match rep_of(e)?.on {
            On::Line { line, .. } => Along::Line(line),
            On::Arc { circle, upper } => Along::Arc { circle, upper },
            On::Ellipse { ellipse, upper } => Along::Ellipse { ellipse, upper },
        });
    }
    let Shapes { circles, ovals } = shapes;
    let circles = circles.into_iter().map(|c| c.round).collect();
    let ellipses = ovals.into_iter().map(|o| o.ellipse).collect();
    Ok(Arrangement {
        vertices,
        edges,
        along,
        circles,
        ellipses,
        around,
        cycles,
        faces,
    })
}

/// The finished parts of an arrangement the face placing reads.
struct Found<'a> {
    edges: &'a [Edge],
    vertices: &'a [At],
    pieces: &'a [Piece],
    shapes: &'a Shapes,
    /// The piece that stands for each edge.
    rep: &'a [u32],
}

/// The faces: the unbounded one, then one for each counter-clockwise cycle, with the holes placed.
fn faces_of(
    found: &Found<'_>,
    cycles: &[Vec<u32>],
    counter: &[bool],
    low: &[usize],
) -> Result<(Vec<Face>, Vec<u32>), ArrangeError> {
    let Found {
        edges,
        vertices,
        pieces,
        shapes,
        rep,
    } = *found;
    // Face numbers: by the rank of the lowest vertex of each counter-clockwise cycle.
    let mut order: Vec<usize> = (0..cycles.len())
        .filter(|&c| counter.get(c).copied().unwrap_or(false))
        .collect();
    order.sort_by_key(|&c| (low.get(c).copied().unwrap_or(0), c));
    let mut face_of_cycle: Vec<Option<u32>> = alloc::vec![None; cycles.len()];
    let mut faces = alloc::vec![Face {
        outer: None,
        holes: Vec::new(),
    }];
    for &c in &order {
        let fi = index(faces.len())?;
        faces.push(Face {
            outer: Some(index(c)?),
            holes: Vec::new(),
        });
        set(&mut face_of_cycle, c, Some(fi))?;
    }
    // The cycle each half-edge is on.
    let mut cycle_of: Vec<u32> = alloc::vec![0; edges.len().saturating_mul(2)];
    for (c, cycle) in cycles.iter().enumerate() {
        for &h in cycle {
            let k = usize::try_from(h).map_err(|_| ArrangeError::TooMany)?;
            set(&mut cycle_of, k, index(c)?)?;
        }
    }
    // The parent cycle of each clockwise cycle: the cycle of the edge nearest below its lowest-
    // leftmost vertex. A second sweep over the finished edges finds it: the vertices are in
    // lexicographic order already, the status holds the edges the sweep line crosses, lowest first,
    // and no two cross, so a vertex is placed among them by exact side tests. At a hole's vertex
    // the edge just below it in the status is the edge below the hole; the face above that edge
    // is on the left of its half-edge that runs rightwards, along a line or round either half of a
    // circle.
    let piece_of_edge = |e: usize| -> Result<&Piece, ArrangeError> {
        piece_of(pieces, rep.get(e).copied().ok_or(ArrangeError::TooMany)?)
    };
    let mut hole_at: Vec<Option<usize>> = alloc::vec![None; vertices.len()];
    for (c, m) in low.iter().enumerate() {
        if !counter.get(c).copied().unwrap_or(true) {
            set(&mut hole_at, *m, Some(c))?;
        }
    }
    let mut parent: Vec<Option<u32>> = alloc::vec![None; cycles.len()];
    let mut status: Vec<usize> = Vec::new();
    let mut next_edge = 0_usize;
    for (v, at) in vertices.iter().enumerate() {
        // The edges that end here are those the vertex lies on.
        let (mut a, mut b) = (0_usize, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let e = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if side(piece_of_edge(e)?, shapes, at)? == Sign::Positive {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let lo = a;
        let (mut a, mut b) = (lo, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let e = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if side(piece_of_edge(e)?, shapes, at)? == Sign::Zero {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let hi = a;
        let tail = status.split_off(hi);
        status.truncate(lo);
        status.extend(tail);
        if let Some(Some(c)) = hole_at.get(v).copied()
            && let Some(below) = lo.checked_sub(1).and_then(|k| status.get(k)).copied()
        {
            let h = index(below.checked_mul(2).ok_or(ArrangeError::TooMany)?)?;
            let on_cycle = cycle_of
                .get(usize::try_from(h).map_err(|_| ArrangeError::TooMany)?)
                .copied()
                .ok_or(ArrangeError::TooMany)?;
            set(&mut parent, c, Some(on_cycle))?;
        }
        // The edges that start here, in the order just after the vertex: by tangent, then
        // curvature.
        let mut starting: Vec<usize> = Vec::new();
        while let Some(e) = edges.get(next_edge) {
            if usize::try_from(e.from).ok() != Some(v) {
                break;
            }
            starting.push(next_edge);
            next_edge = next_edge.saturating_add(1);
        }
        let mut fail = Ok(());
        starting.sort_by(|&x, &y| {
            let order = piece_of_edge(x).and_then(|px| {
                let wx = way(px, shapes, false)?;
                order_at(at, &wx, &way(piece_of_edge(y)?, shapes, false)?)
            });
            order.unwrap_or_else(|e| failed(&mut fail, e))
        });
        fail?;
        let tail = status.split_off(lo);
        status.extend(starting);
        status.extend(tail);
    }
    // Resolve each cycle to a face: a counter-clockwise cycle is its own; a clockwise one is in the
    // face of its parent, or the unbounded face.
    // Parents lead outwards, each to a cycle round the one before, so a chain ends within as many
    // steps as there are cycles: at a counter-clockwise cycle, or with no parent, in the unbounded
    // face.
    let resolve = |start: usize| -> u32 {
        let up = |c: &usize| {
            let p = parent.get(*c).copied().flatten()?;
            usize::try_from(p).ok()
        };
        core::iter::successors(Some(start), up)
            .take(cycles.len().saturating_add(1))
            .find_map(|c| face_of_cycle.get(c).copied().flatten())
            .unwrap_or(0)
    };
    for c in 0..cycles.len() {
        if counter.get(c).copied().unwrap_or(true) {
            continue;
        }
        let f = usize::try_from(resolve(c)).map_err(|_| ArrangeError::TooMany)?;
        let hole = index(c)?;
        faces
            .get_mut(f)
            .ok_or(ArrangeError::TooMany)?
            .holes
            .push(hole);
    }
    Ok((faces, cycle_of))
}

/// Why an arrangement could not be stored as a body.
#[derive(Clone, Debug)]
pub enum BodyError {
    /// A vertex is not a grid point, and the body's points are: the first such vertex, by rank
    /// (the vertices are in lexicographic order), and where it is. The exact [`Arrangement`] still
    /// holds it.
    OffGrid {
        /// The vertex's rank.
        vertex: u32,
        /// Where it is.
        at: At,
    },
    /// An edge is an arc or a piece of an ellipse, and the body's curves are stored in doubles,
    /// which hold no circle about a rational centre exactly: the first such edge. The exact
    /// [`Arrangement`] still holds it.
    Curved {
        /// The edge's place in [`Arrangement::edges`].
        edge: u32,
    },
    /// More items than a 32-bit index names.
    TooMany,
    /// The geometry of a plane or a line could not be made.
    Geometry,
    /// The topology refused a row, or the meter stopped publishing.
    Topo(TopoError),
}

impl From<TopoError> for BodyError {
    fn from(e: TopoError) -> Self {
        BodyError::Topo(e)
    }
}

/// The index of row `n`.
fn row<K: geom_topo::Kind>(n: usize) -> Result<Ix<K>, BodyError> {
    Ix::new(u32::try_from(n).map_err(|_| BodyError::TooMany)?).ok_or(BodyError::TooMany)
}

impl Arrangement {
    /// The arrangement as a `General` body, in the plane z = 0: a vertex for each vertex, an edge on
    /// a line segment for each edge, one face on the plane for each bounded face, its outer loop
    /// first and the loops of the components inside it after, one fin for each side of an edge that
    /// a bounded face lies on (the unbounded face has none), and one shell in the outside region.
    ///
    /// The body's curves are stored in doubles, so the arrangement must have no arc or ellipse (the
    /// first is the error), and its points are grid points, so it must have no crossing off the grid (the
    /// options for storing exact points are in the pull request that added this); the first that
    /// is off the grid is the error. A one-root vertex lies on an arc, so it is refused as the arc.
    ///
    /// # Errors
    ///
    /// [`BodyError::Curved`], [`BodyError::OffGrid`], [`BodyError::TooMany`],
    /// [`BodyError::Geometry`] and [`BodyError::Topo`].
    pub fn to_body(&self, meter: &mut WorkMeter<'_>) -> Result<Body, BodyError> {
        if let Some(edge) = self.along.iter().position(|a| !matches!(a, Along::Line(_))) {
            return Err(BodyError::Curved {
                edge: index(edge).map_err(|_| BodyError::TooMany)?,
            });
        }
        // The vertices' points.
        let mut points: Vec<P3> = Vec::with_capacity(self.vertices.len());
        for (rank, v) in self.vertices.iter().enumerate() {
            let off = || -> Result<BodyError, BodyError> {
                Ok(BodyError::OffGrid {
                    vertex: index(rank).map_err(|_| BodyError::TooMany)?,
                    at: v.clone(),
                })
            };
            // A grid point is rational with X and Y whole multiples of W; a one-root vertex is none.
            let point = v.rational().and_then(|r| {
                let ([x, y], w) = r.coordinates();
                let whole = x.checked_rem(w)? == 0 && y.checked_rem(w)? == 0;
                let (gx, gy) = (x.checked_div(w)?, y.checked_div(w)?);
                let p = P3::new(i64::try_from(gx).ok()?, i64::try_from(gy).ok()?, 0).ok()?;
                whole.then_some(p)
            });
            match point {
                Some(p) => points.push(p),
                None => return Err(off()?),
            }
        }
        // The cycle each half-edge is on.
        let total = self.edges.len().saturating_mul(2);
        let mut cycle_of_half: Vec<usize> = alloc::vec![usize::MAX; total];
        for (c, cycle) in self.cycles.iter().enumerate() {
            for &h in cycle {
                let k = usize::try_from(h).map_err(|_| BodyError::TooMany)?;
                *cycle_of_half.get_mut(k).ok_or(BodyError::TooMany)? = c;
            }
        }
        // Which bounded face (numbered from 0, the unbounded face left out) each cycle lies in, and
        // the order of the loops: for each bounded face its outer cycle, then its holes.
        let mut face_of_cycle: Vec<Option<usize>> = alloc::vec![None; self.cycles.len()];
        let mut loop_of_cycle: Vec<Option<usize>> = alloc::vec![None; self.cycles.len()];
        let mut loops: Vec<(usize, usize)> = Vec::new();
        for (fi, face) in self.faces.iter().enumerate().skip(1) {
            for c in face.outer.iter().chain(face.holes.iter()) {
                let k = usize::try_from(*c).map_err(|_| BodyError::TooMany)?;
                *face_of_cycle.get_mut(k).ok_or(BodyError::TooMany)? = Some(fi.saturating_sub(1));
                *loop_of_cycle.get_mut(k).ok_or(BodyError::TooMany)? = Some(loops.len());
                loops.push((fi.saturating_sub(1), k));
            }
        }
        // One fin for each half-edge on a cycle of a bounded face, in half-edge order.
        let mut fin_of_half: Vec<Option<usize>> = alloc::vec![None; total];
        let mut fins: Vec<usize> = Vec::new();
        for (h, slot) in fin_of_half.iter_mut().enumerate() {
            let c = cycle_of_half.get(h).copied().unwrap_or(usize::MAX);
            if face_of_cycle.get(c).copied().flatten().is_some() {
                *slot = Some(fins.len());
                fins.push(h);
            }
        }
        let mut b = Builder::new();
        let a = &mut b.arenas;
        // The outside region, holding the one shell.
        let outside = Region {
            kind: RegionKind::Void,
            first_shell: Some(row(0)?),
        };
        a.regions.push(outside)?;
        // The plane z = 0.
        let whole = (f64::NEG_INFINITY, f64::INFINITY);
        let plane = Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (whole, whole));
        let plane = plane.map_err(|_| BodyError::Geometry)?;
        let surface = a.surfaces.push(SurfaceKind::Plane(plane))?;
        // Vertices.
        for p in &points {
            let at = a.points.push(*p)?;
            a.vertices.push(Vertex { point: at })?;
        }
        // Edges, each on the segment between its end points.
        let flat = |v: u32| -> Result<[f64; 3], BodyError> {
            let p = usize::try_from(v)
                .ok()
                .and_then(|k| points.get(k))
                .ok_or(BodyError::TooMany)?;
            #[allow(
                clippy::cast_precision_loss,
                reason = "grid coordinates are below 2^40"
            )]
            Ok([p.x.get() as f64, p.y.get() as f64, 0.0])
        };
        for (e, edge) in self.edges.iter().enumerate() {
            let line = Line::<3>::segment(flat(edge.from)?, flat(edge.to)?);
            let line = line.map_err(|_| BodyError::Geometry)?.value;
            let curve = a.curves.push(CurveKind::Line(line))?;
            let own = e.checked_mul(2).ok_or(BodyError::TooMany)?;
            let first_fin = [own, own.saturating_add(1)]
                .iter()
                .find_map(|h| fin_of_half.get(*h).copied().flatten());
            let end = |v: u32| {
                usize::try_from(v)
                    .map_err(|_| BodyError::TooMany)
                    .and_then(row)
            };
            let topo = TopoEdge {
                first_fin: first_fin.map(row).transpose()?,
                start: Some(end(edge.from)?),
                end: Some(end(edge.to)?),
                curve,
            };
            a.edges.push(topo)?;
        }
        // Fins, each linked to the next and previous of its cycle and to the other side.
        for &h in &fins {
            let c = cycle_of_half.get(h).copied().unwrap_or(usize::MAX);
            let cycle = self.cycles.get(c).ok_or(BodyError::TooMany)?;
            let at = cycle
                .iter()
                .position(|x| usize::try_from(*x).ok() == Some(h))
                .ok_or(BodyError::TooMany)?;
            let n = cycle.len();
            let neighbour = |k: usize| -> Result<Ix<geom_topo::Fin>, BodyError> {
                let half = cycle
                    .get(k)
                    .and_then(|x| usize::try_from(*x).ok())
                    .ok_or(BodyError::TooMany)?;
                let fin = fin_of_half
                    .get(half)
                    .copied()
                    .flatten()
                    .ok_or(BodyError::TooMany)?;
                row(fin)
            };
            let next = neighbour(at.saturating_add(1).checked_rem(n).unwrap_or(0))?;
            let prev = neighbour(at.checked_sub(1).unwrap_or(n.saturating_sub(1)))?;
            let mine = fin_of_half.get(h).copied().flatten();
            let own = row(mine.ok_or(BodyError::TooMany)?)?;
            let twin = fin_of_half.get(h ^ 1).copied().flatten();
            let on_loop = loop_of_cycle.get(c).copied().flatten();
            let radial = twin.map(row).transpose()?.unwrap_or(own);
            let fin = Fin {
                loop_: row(on_loop.ok_or(BodyError::TooMany)?)?,
                next,
                prev,
                edge: row(h / 2)?,
                sense: if h % 2 == 0 {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
                radial,
                pcurve: None,
            };
            a.fins.push(fin)?;
        }
        // Loops, in the order of their faces.
        for (k, &(face, c)) in loops.iter().enumerate() {
            let cycle = self.cycles.get(c).ok_or(BodyError::TooMany)?;
            let first = cycle
                .first()
                .and_then(|x| usize::try_from(*x).ok())
                .and_then(|h| fin_of_half.get(h).copied().flatten())
                .ok_or(BodyError::TooMany)?;
            let following = k.saturating_add(1);
            let same_face = loops.get(following).filter(|(f, _)| *f == face);
            let next = same_face.map(|_| row(following)).transpose()?;
            let one = Loop {
                face: row(face)?,
                next,
                first_fin: Some(row(first)?),
            };
            a.loops.push(one)?;
        }
        // Faces: on the plane, bounded by the box of their outer cycle's vertices.
        let bounded = self.faces.len().saturating_sub(1);
        for (fi, face) in self.faces.iter().enumerate().skip(1) {
            let outer = face
                .outer
                .and_then(|c| usize::try_from(c).ok())
                .ok_or(BodyError::TooMany)?;
            let cycle = self.cycles.get(outer).ok_or(BodyError::TooMany)?;
            // A cycle's vertices are the ends of its half-edges' edges, either end.
            let corners = cycle
                .iter()
                .filter_map(|&h| self.edges.get(usize::try_from(h / 2).ok()?))
                .flat_map(|e| [e.from, e.to])
                .filter_map(|v| points.get(usize::try_from(v).ok()?).copied());
            let bound = Box3::from_points(corners).ok_or(BodyError::TooMany)?;
            let first_loop = loop_of_cycle.get(outer).copied().flatten();
            let topo = TopoFace {
                shell: row(0)?,
                next: if fi < bounded { Some(row(fi)?) } else { None },
                surface,
                sense: Sense::Forward,
                first_loop: first_loop.map(row).transpose()?,
                bound,
            };
            a.faces.push(topo)?;
        }
        let shell = Shell {
            region: row(0)?,
            next: None,
            first_face: if bounded > 0 { Some(row(0)?) } else { None },
        };
        a.shells.push(shell)?;
        Ok(b.publish(BodyKind::General, row(0)?, meter)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{ArrangeError, BodyError, failed};
    use core::cmp::Ordering;
    use geom_num::NumError;
    use geom_topo::{Arena, TopoError};

    #[test]
    fn an_arithmetic_refusal_is_out_of_range() {
        assert_eq!(
            ArrangeError::from(NumError::BudgetExceeded),
            ArrangeError::Range
        );
    }

    #[test]
    fn a_sort_keeps_the_first_refusal_and_goes_on() {
        let mut slot = Ok(());
        assert_eq!(failed(&mut slot, ArrangeError::Range), Ordering::Equal);
        assert_eq!(failed(&mut slot, ArrangeError::TooMany), Ordering::Equal);
        assert_eq!(slot, Err(ArrangeError::Range));
    }

    #[test]
    fn a_topology_refusal_is_a_body_error() {
        let e = BodyError::from(TopoError::Full(Arena::Face));
        assert!(matches!(e, BodyError::Topo(TopoError::Full(Arena::Face))));
    }
}
