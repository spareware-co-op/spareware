//! Group 3 of chapter 8.3 on faces that lie exactly on integer planes, whatever their shape and
//! whether their vertices are grid points or the meetings of three planes (G134).
//!
//! What it does: an [`Outline`] is a flat face as this module reads it: its integer plane, and for
//! each fin the edge's two ends and a plane that crosses the face's along the edge. That plane is
//! the other face's across the edge, or, along a seam, a plane the edge's ends both lie on, or for
//! two grid points one stood on them; so a side of the polygon is a pair of planes, as EMBER keeps
//! it, and no test here ever needs the orientation of three implicit points, about 840 bits a term
//! (19.2).
//!
//! - **A face and a line.** Where a face's plane meets another plane, [`Cut`] names each point of
//!   the line by a plane that crosses it there. Moved infinitely little off the other plane, the
//!   line crosses the face's boundary at sorted marks that pair up into stretches; the stretches on
//!   the two sides together are the whole of the closed face on the line, with no special case for
//!   a vertex or an edge lying on it.
//! - **Two crossing faces.** Their stretches on their planes' line may overlap only where the two
//!   faces share edges and vertices.
//! - **Two faces on one plane.** Each side of each face, as a stretch of its own line, against the
//!   other face's stretches on that line, by the same rule; and two that share an edge must lie on
//!   opposite sides of it.
//! - **Two edges of one face.** Each is a stretch of its own line; they may meet only at a vertex
//!   they share.
//! - **A point against a shell.** The parity of the crossings of a ray along the line of two of the
//!   point's planes, moved infinitely little off both, so that no tie is left.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is the sign of a plane at a grid point or at a meeting of three
//!   planes (`geom_num::plane::Plane::side` and `side_of_tpi`, within 384 bits), or the sign of a
//!   3 × 3 determinant of normals (`geom_num::plane::Tpi::weight`). No double is read and nothing
//!   is sampled.
//! - **Any polygon.** A face that is not convex, has holes, or has several outer loops is cut like
//!   any other. A vertex on the line, an edge along it and a touch from one side all fall to the
//!   moved line, which sees each as ordinary crossings at one mark.
//! - **Fails closed.** A step that cannot be decided is [`Meet::Unknown`], which the caller reports,
//!   never a pass. A face without a plane or a plane for each side has no outline, and its pairs are
//!   the caller's to report.
//! - **Runs on bodies that passed group 2.** Every vertex is on every face it bounds, so a shared
//!   edge lies on both faces' planes and the other face's plane crosses along it.
//! - **Charged.** One meter unit per side visited and per pair of sides or stretches compared.
//!
//! Method: the plane-based polygon of EMBER, a supporting plane and one bounding plane per side
//! (Trettner, Nehring-Wirxel and Kobbelt, "EMBER: exact mesh booleans via efficient and robust local
//! arrangements", ACM TOG 41(4) 2022, doi:10.1145/3528223.3530181), on integer plane arithmetic
//! (Nehring-Wirxel, Trettner and Kobbelt, "Fast exact booleans for iterated CSG using octree-embedded
//! BSPs", Computer-Aided Design 135 (2021), doi:10.1016/j.cad.2021.103015); the moved line is
//! Simulation of Simplicity's idea (Edelsbrunner and Mücke 1990, doi:10.1145/77635.77639), applied
//! to the query alone, as `geom_topo::query` applies it.
//!
//! Cost: an outline is linear in its face's fins, a pair of faces linear in their sides plus the
//! sorting of their crossings, a pair of faces on one plane the product of their sides. Each test
//! allocates a short list, reserved fallibly with its logical bytes on the meter. Recursion: none.

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_curve::CurveKind;
use geom_num::plane::{Plane, Tpi};
use geom_num::{P3, Sign};
use geom_op::run::reserve;
use geom_run::{LogicalSize, Stopped};
use geom_topo::entity::{Edge, Face, Sense, Shell, Vertex};
use geom_topo::ix::Arena;
use geom_topo::mass::Corner;
use geom_topo::query::Side as ShellSide;
use geom_topo::{Arenas, Ix, SurfaceKind};

use crate::combinatorial::{edge_fins, face_fins, fin_edge, fin_ends, fin_face, rows};
use crate::curved::Pair;
use crate::geometric::{corner, planes_lie, surface};
use crate::self_intersection::Meet;
use crate::sink::Sink;

/// Why a test could not finish.
enum Stuck {
    /// The meter stopped it.
    Stopped(Stopped),
    /// A step had no answer: nothing within the numbers' budgets can cause it, and it is reported
    /// as [`Meet::Unknown`] if it happens.
    Undecided,
}

impl From<Stopped> for Stuck {
    fn from(stop: Stopped) -> Self {
        Stuck::Stopped(stop)
    }
}

/// `found`, with a step that had no answer as `unknown`.
fn settled<T>(found: Result<T, Stuck>, unknown: T) -> Result<T, Stopped> {
    found.or_else(|stuck| match stuck {
        Stuck::Stopped(stop) => Err(stop),
        Stuck::Undecided => Ok(unknown),
    })
}

/// The answer to a step of a test, if it had one.
fn known<T>(step: Option<T>) -> Result<T, Stuck> {
    step.ok_or(Stuck::Undecided)
}

/// A vertex of an outline.
#[derive(Clone)]
struct End {
    vertex: Ix<Vertex>,
    at: Corner,
}

/// One fin of an outline.
#[derive(Clone)]
struct Side {
    edge: Ix<Edge>,
    sense: Sense,
    start: End,
    end: End,
    /// A plane through the edge that crosses the face's plane: the edge's line is where they meet.
    across: Plane,
}

impl LogicalSize for Side {
    /// Two corners with their vertices, a plane, the edge and the sense.
    const LOGICAL_BYTES: u64 = 2 * (Corner::LOGICAL_BYTES + 4) + 64 + 4 + 1;
}

/// A flat face, as this module reads it.
pub(crate) struct Outline {
    face: Ix<Face>,
    plane: Plane,
    sense: Sense,
    sides: Vec<Side>,
    /// Whether a vertex of it is a meeting of planes.
    pub(crate) implicit: bool,
}

/// A place in the list of outlines, one per face.
pub(crate) struct Slot(pub(crate) Option<Outline>);

impl LogicalSize for Slot {
    /// A list's three words, the plane, and the face and sense.
    const LOGICAL_BYTES: u64 = 24 + 64 + 8;
}

/// A point of a line: the plane that crosses the line there.
#[derive(Clone, Copy)]
struct Mark(Plane);

impl LogicalSize for Mark {
    /// Four 16-byte integers.
    const LOGICAL_BYTES: u64 = 64;
}

/// A closed stretch of a line, from an earlier mark to a later or equal one.
#[derive(Clone, Copy)]
struct Stretch {
    from: Mark,
    to: Mark,
}

impl LogicalSize for Stretch {
    /// Two marks.
    const LOGICAL_BYTES: u64 = 2 * Mark::LOGICAL_BYTES;
}

/// A sign as an ordering.
const fn ordering(s: Sign) -> Ordering {
    match s {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    }
}

/// The plane where coordinate `k` of a point equals `p`'s.
fn axis(p: P3, k: usize) -> Option<Plane> {
    let [x, y, z] = [p.x, p.y, p.z].map(|c| i128::from(c.get()));
    let (normal, offset) = match k {
        0 => ([1, 0, 0], x),
        1 => ([0, 1, 0], y),
        _ => ([0, 0, 1], z),
    };
    Plane::new(normal, offset.checked_neg()?).ok()
}

/// The line where two planes meet, its points named by the planes that cross it. Its direction is
/// the cross product of the two normals, whichever two planes name the line, up to sign: a list is
/// sorted against one [`Cut`].
struct Cut {
    first: Plane,
    second: Plane,
}

impl Cut {
    /// The point where `plane` crosses the line, if it does.
    fn at(&self, plane: &Plane) -> Option<Tpi> {
        Tpi::new(self.first, self.second, *plane).ok()
    }

    /// The order along the line of the points where `a` and `b` cross it. `b` rises along the
    /// line when det(first, second, b) is positive, which is its point's weight, so the sign of
    /// `b` at `a`'s point, turned by that, is the order.
    fn order(&self, a: &Mark, b: &Mark) -> Result<Ordering, Stuck> {
        let point = known(self.at(&a.0))?;
        let rising = known(self.at(&b.0))?.weight();
        let side = known(b.0.side_of_tpi(&point).ok())?;
        Ok(ordering(if rising == Sign::Positive {
            side
        } else {
            side.negate()
        }))
    }

    /// The mark of `c`, a point of the line: a coordinate plane through a grid point, and through a
    /// meeting of three planes one of them, which cannot all hold the line.
    fn mark(&self, c: &Corner) -> Result<Mark, Stuck> {
        let crosses = |plane: &Plane| self.at(plane).is_some();
        let found = match c {
            Corner::Grid(p) => (0..3).filter_map(|k| axis(*p, k)).find(crosses),
            Corner::Tpi(t) => t.planes().iter().copied().find(crosses),
            Corner::Lpi(_) => None,
        };
        known(found).map(Mark)
    }

    /// Whether `c` lies on the line.
    fn holds(&self, c: &Corner) -> bool {
        [&self.first, &self.second]
            .into_iter()
            .all(|plane| c.side(plane) == Ok(Sign::Zero))
    }

    /// The stretch between two marks, in order.
    fn stretch(&self, a: Mark, b: Mark) -> Result<Stretch, Stuck> {
        Ok(if self.order(&a, &b)? == Ordering::Greater {
            Stretch { from: b, to: a }
        } else {
            Stretch { from: a, to: b }
        })
    }

    /// What two stretches have in common.
    fn overlap(&self, s: &Stretch, t: &Stretch) -> Result<Option<Stretch>, Stuck> {
        let from = if self.order(&s.from, &t.from)? == Ordering::Less {
            t.from
        } else {
            s.from
        };
        let to = if self.order(&s.to, &t.to)? == Ordering::Greater {
            t.to
        } else {
            s.to
        };
        Ok((self.order(&from, &to)? != Ordering::Greater).then_some(Stretch { from, to }))
    }

    /// Whether `runs`, joined end to end, hold all of `wanted`: from its start, each step takes the
    /// run that holds the place reached and goes farthest, so the walk ends within one step per run.
    fn covered(&self, runs: &[Stretch], wanted: &Stretch) -> Result<bool, Stuck> {
        let mut at = wanted.from;
        loop {
            let mut farthest: Option<Mark> = None;
            for run in runs {
                let holds = self.order(&run.from, &at)? != Ordering::Greater
                    && self.order(&at, &run.to)? != Ordering::Greater;
                if holds
                    && farthest.as_ref().map_or(Ok(true), |far| {
                        self.order(&run.to, far).map(|o| o == Ordering::Greater)
                    })?
                {
                    farthest = Some(run.to);
                }
            }
            let Some(reached) = farthest else {
                return Ok(false);
            };
            if self.order(&reached, &wanted.to)? != Ordering::Less {
                return Ok(true);
            }
            if self.order(&reached, &at)? == Ordering::Equal {
                return Ok(false);
            }
            at = reached;
        }
    }

    /// `marks` with `mark` put in its place, by binary search.
    fn insert(&self, marks: &mut Vec<Mark>, mark: Mark) -> Result<(), Stuck> {
        let (mut low, mut high) = (0, marks.len());
        while low < high {
            let middle = usize::midpoint(low, high);
            let there = known(marks.get(middle))?;
            if self.order(&mark, there)? == Ordering::Less {
                high = middle;
            } else {
                low = middle.saturating_add(1);
            }
        }
        marks.insert(low, mark);
        Ok(())
    }
}

/// The planes of a meeting of three, and none for a grid point.
fn meeting_planes(c: &Corner) -> &[Plane] {
    match c {
        Corner::Tpi(t) => t.planes(),
        _ => &[],
    }
}

/// The plane through grid points `p` and `q` and the point one step along an axis `plane` is not
/// parallel to, towards the origin so that it stays on the grid. It crosses `plane`.
fn stood_on(plane: &Plane, p: P3, q: P3) -> Option<Plane> {
    let k = plane.n().iter().position(|&c| c != 0)?;
    let step = |c: geom_num::Coord, axis: usize| -> i64 {
        let v = c.get();
        if axis == k {
            v.saturating_sub(if v > 0 { 1 } else { -1 })
        } else {
            v
        }
    };
    let off = P3::new(step(p.x, 0), step(p.y, 1), step(p.z, 2)).ok()?;
    Plane::through(p, q, off).ok()
}

/// A plane that holds `start` and `end` and crosses `plane`, for the side of face `f` along edge
/// `e`: another face's, then one of the ends' own, then one stood on two grid points.
fn across(
    a: &Arenas,
    (f, e): (Ix<Face>, Ix<Edge>),
    plane: &Plane,
    ends: [&Corner; 2],
    exact: bool,
) -> Option<Plane> {
    let [start, end] = ends;
    let fits = |p: &Plane| {
        planes_lie(plane, p) == Pair::Crossing && ends.iter().all(|c| c.side(p) == Ok(Sign::Zero))
    };
    let beside = edge_fins(a, e)
        .filter_map(|n| fin_face(a, n))
        .filter(|&g| g != f)
        .filter_map(|g| surface(a, g, exact)?.grid);
    let own = meeting_planes(start)
        .iter()
        .chain(meeting_planes(end))
        .copied();
    beside.chain(own).find(fits).or_else(|| match (start, end) {
        (Corner::Grid(p), Corner::Grid(q)) => stood_on(plane, *p, *q),
        _ => None,
    })
}

/// The plane `f`'s grid corners lie on exactly: `None` if a corner is not a grid point, if they
/// lie on one line, or if they leave the plane of the first three that do not.
fn corners_plane(a: &Arenas, f: Ix<Face>) -> Option<Plane> {
    let grid = |n| match corner(a, fin_ends(a, n)?.0)? {
        Corner::Grid(p) => Some(p),
        _ => None,
    };
    let first = face_fins(a, f).find_map(grid)?;
    let plane = face_fins(a, f).filter_map(grid).find_map(|q| {
        face_fins(a, f)
            .filter_map(grid)
            .find_map(|r| Plane::through(first, q, r).ok())
    })?;
    face_fins(a, f)
        .all(|n| grid(n).is_some_and(|p| plane.side(p) == Ok(Sign::Zero)))
        .then_some(plane)
}

/// The side of face `f`, on `plane`, that fin `n` makes: `None` for a fin on anything but a line
/// between two vertices, or one no plane crosses along.
fn side_of(
    a: &Arenas,
    f: Ix<Face>,
    n: Ix<geom_topo::entity::Fin>,
    plane: &Plane,
    exact: bool,
) -> Option<Side> {
    let edge = fin_edge(a, n)?;
    let curve = a.edges.curve.get(edge).and_then(|c| a.curves.get(*c));
    if !matches!(curve, Some(CurveKind::Line(_))) {
        return None;
    }
    let (s, t) = fin_ends(a, n)?;
    let (start, end) = (corner(a, s)?, corner(a, t)?);
    Some(Side {
        edge,
        sense: *a.fins.sense.get(n)?,
        across: across(a, (f, edge), plane, [&start, &end], exact)?,
        start: End {
            vertex: s,
            at: start,
        },
        end: End { vertex: t, at: end },
    })
}

/// Face `f` as an outline, if it is one: exactly on an integer plane, with a plane for each side.
fn outline(
    a: &Arenas,
    f: Ix<Face>,
    exact: bool,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<Outline>, Stopped> {
    let kind = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s));
    if !matches!(kind, Some(SurfaceKind::Plane(_))) {
        return Ok(None);
    }
    let Some(plane) = surface(a, f, exact)
        .and_then(|on| on.grid)
        .or_else(|| corners_plane(a, f))
    else {
        return Ok(None);
    };
    let fins = face_fins(a, f).count();
    let mut sides: Vec<Side> = reserve(sink.meter(), u64::try_from(fins).unwrap_or(u64::MAX))?;
    for n in face_fins(a, f) {
        sink.charge()?;
        let Some(side) = side_of(a, f, n, &plane, exact) else {
            return Ok(None);
        };
        sides.push(side);
    }
    let implicit = sides.iter().any(|s| matches!(s.start.at, Corner::Tpi(_)));
    Ok(Some(Outline {
        face: f,
        plane,
        sense: a.faces.sense.get(f).copied().unwrap_or(Sense::Forward),
        sides,
        implicit,
    }))
}

/// One slot per face of `a`.
pub(crate) fn outlines(
    a: &Arenas,
    exact: bool,
    sink: &mut Sink<'_, '_>,
) -> Result<Vec<Slot>, Stopped> {
    let mut out: Vec<Slot> = reserve(sink.meter(), u64::from(a.rows(Arena::Face)))?;
    for f in rows::<Face>(a.rows(Arena::Face)) {
        out.push(Slot(outline(a, f, exact, sink)?));
    }
    Ok(out)
}

impl Outline {
    /// Where its boundary crosses `cut`, on its plane where it meets `against`, moved infinitely
    /// little to the side of `against` that `above` names: a vertex on `against` counts as below
    /// the moved line, so one that only touches it gives two crossings or none. Sorted.
    fn crossings(
        &self,
        against: &Plane,
        cut: &Cut,
        above: Sign,
        sink: &mut Sink<'_, '_>,
    ) -> Result<Vec<Mark>, Stuck> {
        let count = u64::try_from(self.sides.len()).unwrap_or(u64::MAX);
        let mut marks: Vec<Mark> = reserve(sink.meter(), count)?;
        for side in &self.sides {
            sink.charge()?;
            let from = known(side.start.at.side(against).ok())?;
            let to = known(side.end.at.side(against).ok())?;
            if (from == above) != (to == above) {
                let mark = if from == Sign::Zero {
                    cut.mark(&side.start.at)?
                } else if to == Sign::Zero {
                    cut.mark(&side.end.at)?
                } else {
                    Mark(side.across)
                };
                cut.insert(&mut marks, mark)?;
            }
        }
        Ok(marks)
    }

    /// The whole of this face on `cut`, its plane's line with `against`: the stretches of the line
    /// moved to each side, whose union is the closed face on the line.
    fn stretches(
        &self,
        against: &Plane,
        cut: &Cut,
        sink: &mut Sink<'_, '_>,
    ) -> Result<Vec<Stretch>, Stuck> {
        let count = u64::try_from(self.sides.len()).unwrap_or(u64::MAX);
        let mut out: Vec<Stretch> = reserve(sink.meter(), count)?;
        for above in [Sign::Positive, Sign::Negative] {
            let marks = self.crossings(against, cut, above, sink)?;
            // A closed boundary crosses a line an even number of times.
            let ends = marks.iter().skip(1).step_by(2);
            out.extend(
                marks
                    .iter()
                    .step_by(2)
                    .zip(ends)
                    .map(|(&from, &to)| Stretch { from, to }),
            );
        }
        Ok(out)
    }
}

/// What `f` and `g` share on `cut`: each edge of both that lies along it, and each vertex of both
/// on it as a stretch of one point.
fn joins(
    f: &Outline,
    g: &Outline,
    cut: &Cut,
    sink: &mut Sink<'_, '_>,
) -> Result<Vec<Stretch>, Stuck> {
    let count = u64::try_from(f.sides.len().saturating_mul(2)).unwrap_or(u64::MAX);
    let mut out: Vec<Stretch> = reserve(sink.meter(), count)?;
    for s in &f.sides {
        let shared = g.sides.iter().any(|t| t.edge == s.edge);
        if shared && cut.holds(&s.start.at) && cut.holds(&s.end.at) {
            let (from, to) = (cut.mark(&s.start.at)?, cut.mark(&s.end.at)?);
            out.push(cut.stretch(from, to)?);
        }
        let met = g.sides.iter().any(|t| t.start.vertex == s.start.vertex);
        if met && cut.holds(&s.start.at) {
            let at = cut.mark(&s.start.at)?;
            out.push(Stretch { from: at, to: at });
        }
        sink.charge()?;
    }
    Ok(out)
}

/// How `f` and `g` meet on `cut`, given what each holds of it: `Cross` if they share any of it they
/// do not share as an edge or a vertex.
fn along(
    (f, g): (&Outline, &Outline),
    cut: &Cut,
    own: &[Stretch],
    other: &[Stretch],
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stuck> {
    let mut shared: Option<Vec<Stretch>> = None;
    for s in own {
        for t in other {
            sink.charge()?;
            let Some(common) = cut.overlap(s, t)? else {
                continue;
            };
            let runs = match &shared {
                Some(runs) => runs,
                None => shared.insert(joins(f, g, cut, sink)?),
            };
            if !cut.covered(runs, &common)? {
                return Ok(Meet::Cross);
            }
        }
    }
    Ok(Meet::Apart)
}

/// How two faces on crossing planes meet.
fn crossing(f: &Outline, g: &Outline, sink: &mut Sink<'_, '_>) -> Result<Meet, Stuck> {
    let cut = Cut {
        first: f.plane,
        second: g.plane,
    };
    let own = f.stretches(&g.plane, &cut, sink)?;
    let other = g.stretches(&f.plane, &cut, sink)?;
    along((f, g), &cut, &own, &other, sink)
}

/// Whether `o`'s outward side is the way its plane's normal points, measured against `reference`'s
/// normal: `plane` and `reference` are one plane, so a normal is a multiple of the other.
fn outward_with(o: &Outline, reference: &Plane) -> bool {
    let turned = reference
        .n()
        .iter()
        .zip(o.plane.n())
        .find(|&(r, _)| *r != 0)
        .is_some_and(|(r, n)| (*r > 0) != (n > 0));
    turned == (o.sense == Sense::Forward)
}

/// How two faces on one plane meet: each side of each, as a stretch of its own line, against the
/// other; then each edge they share, which must keep them on opposite sides. A fin runs with its
/// face on its left, seen from the outward side, so two faces on one side of a shared edge are two
/// that face the same way with fins running the same way, or opposite ways with fins opposite.
fn coplanar(f: &Outline, g: &Outline, sink: &mut Sink<'_, '_>) -> Result<Meet, Stuck> {
    for (one, other) in [(f, g), (g, f)] {
        for side in &one.sides {
            let cut = Cut {
                first: one.plane,
                second: side.across,
            };
            let from = cut.mark(&side.start.at)?;
            let to = cut.mark(&side.end.at)?;
            let own = [cut.stretch(from, to)?];
            let cuts = other.stretches(&side.across, &cut, sink)?;
            if along((one, other), &cut, &own, &cuts, sink)? == Meet::Cross {
                return Ok(Meet::Cross);
            }
        }
    }
    let facing = outward_with(f, &f.plane) == outward_with(g, &f.plane);
    for s in &f.sides {
        sink.charge()?;
        let folded = g
            .sides
            .iter()
            .any(|t| t.edge == s.edge && (t.sense == s.sense) == facing);
        if folded {
            return Ok(Meet::Cross);
        }
    }
    Ok(Meet::Apart)
}

/// How faces `f` and `g` meet, exactly. Stored boxes that do not overlap settle it: group 2 has
/// held each face's vertices to its box.
pub(crate) fn faces_meet(
    a: &Arenas,
    f: &Outline,
    g: &Outline,
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stopped> {
    let boxes = a.faces.bound.get(f.face).zip(a.faces.bound.get(g.face));
    if boxes.is_some_and(|(p, q)| !p.overlaps(q)) {
        return Ok(Meet::Apart);
    }
    let found = match planes_lie(&f.plane, &g.plane) {
        Pair::Crossing => crossing(f, g, sink),
        Pair::Apart => Ok(Meet::Apart),
        _ => coplanar(f, g, sink),
    };
    settled(found, Meet::Unknown)
}

/// Where two sides meet on the line of the first: `None` if the second keeps to one side of it.
fn contact(cut: &Cut, s: &Side, t: &Side) -> Result<Option<Stretch>, Stuck> {
    let from = known(t.start.at.side(&s.across).ok())?;
    let to = known(t.end.at.side(&s.across).ok())?;
    let point = |at: Mark| Stretch { from: at, to: at };
    Ok(match (from, to) {
        (Sign::Zero, Sign::Zero) => {
            let (p, q) = (cut.mark(&t.start.at)?, cut.mark(&t.end.at)?);
            Some(cut.stretch(p, q)?)
        }
        (Sign::Zero, _) => Some(point(cut.mark(&t.start.at)?)),
        (_, Sign::Zero) => Some(point(cut.mark(&t.end.at)?)),
        (p, q) if p == q => None,
        _ => Some(point(Mark(t.across))),
    })
}

/// How two distinct edges of the face `o` meet, exactly: on the first's line, they may meet only at
/// a vertex they share.
pub(crate) fn edges_meet(
    o: &Outline,
    first: Ix<Edge>,
    second: Ix<Edge>,
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stopped> {
    let found = (|| -> Result<Meet, Stuck> {
        sink.charge()?;
        let side = |e| known(o.sides.iter().find(|s| s.edge == e));
        let (s, t) = (side(first)?, side(second)?);
        let cut = Cut {
            first: o.plane,
            second: s.across,
        };
        let (from, to) = (cut.mark(&s.start.at)?, cut.mark(&s.end.at)?);
        let own = cut.stretch(from, to)?;
        let Some(common) = contact(&cut, s, t)?
            .map(|c| cut.overlap(&own, &c))
            .transpose()?
            .flatten()
        else {
            return Ok(Meet::Apart);
        };
        let mut runs: Vec<Stretch> = reserve(sink.meter(), 2)?;
        for end in [&s.start, &s.end] {
            if [&t.start, &t.end].iter().any(|e| e.vertex == end.vertex) {
                let at = cut.mark(&end.at)?;
                runs.push(Stretch { from: at, to: at });
            }
        }
        Ok(if cut.covered(&runs, &common)? {
            Meet::Apart
        } else {
            Meet::Cross
        })
    })();
    settled(found, Meet::Unknown)
}

/// Three planes that meet exactly at `p`, the first two the ray's line.
fn frame(p: &Corner) -> Option<[Plane; 3]> {
    match p {
        Corner::Grid(q) => Some([axis(*q, 0)?, axis(*q, 1)?, axis(*q, 2)?]),
        Corner::Tpi(t) => Some(*t.planes()),
        Corner::Lpi(_) => None,
    }
}

/// Which side of the closed shell `shell` the point `p` lies on, or `None` if a corner of the shell
/// is at `p`.
///
/// The ray runs along the line of `x` and `y`, two of three planes that meet at `p`, from `p` moved
/// along it, off `x` towards its normal by much less than off `y`, so that the ray meets no edge and
/// runs along no face. A face it pierces is one whose plane it crosses ahead of `p`, at a place on
/// the face: the face's line with `x`, moved as the ray is, in the sorted crossings of
/// [`Outline::crossings`], and the place `y`'s crossing of that line, moved the way `y` rises. The
/// parity of the faces pierced is the side.
fn point_side(
    p: &Corner,
    shell: &[Outline],
    sink: &mut Sink<'_, '_>,
) -> Result<Option<ShellSide>, Stuck> {
    let [x, y, z] = known(frame(p))?;
    let on = |c: &Corner| [&x, &y, &z].iter().all(|q| c.side(q) == Ok(Sign::Zero));
    if shell
        .iter()
        .any(|o| o.sides.iter().any(|s| on(&s.start.at)))
    {
        return Ok(None);
    }
    let ray = Cut {
        first: x,
        second: y,
    };
    let mut inside = false;
    for o in shell {
        sink.charge()?;
        // A face parallel to the ray cannot be crossed by it, even one that holds it unmoved.
        if ray.at(&o.plane).is_none() || ray.order(&Mark(z), &Mark(o.plane))? != Ordering::Less {
            continue;
        }
        let line = Cut {
            first: o.plane,
            second: x,
        };
        let rising = known(line.at(&y))?.weight() == Sign::Positive;
        let mut pierced = false;
        for mark in &o.crossings(&x, &line, Sign::Positive, sink)? {
            let passed = match line.order(mark, &Mark(y))? {
                Ordering::Less => true,
                Ordering::Equal => rising,
                Ordering::Greater => false,
            };
            pierced ^= passed;
        }
        inside ^= pierced;
    }
    Ok(Some(if inside {
        ShellSide::Inside
    } else {
        ShellSide::Outside
    }))
}

/// Which side of closed shell `t` shell `s` lies on: the side of the first of `s`'s corners that is
/// not at a corner of `t`, or `None` if there is none, or a face of `t` has no outline.
pub(crate) fn shell_side(
    a: &Arenas,
    s: Ix<Shell>,
    t: Ix<Shell>,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<ShellSide>, Stopped> {
    let of = |shell: Ix<Shell>| {
        rows::<Face>(a.rows(Arena::Face)).filter(move |&f| a.faces.shell.get(f) == Some(&shell))
    };
    let faces = u64::try_from(of(t).count()).unwrap_or(u64::MAX);
    let mut shell: Vec<Slot> = reserve(sink.meter(), faces)?;
    for f in of(t) {
        shell.push(Slot(outline(a, f, false, sink)?));
    }
    let Some(shell) = shell
        .into_iter()
        .map(|slot| slot.0)
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    for n in of(s).flat_map(|f| face_fins(a, f)) {
        let Some(p) = fin_ends(a, n).and_then(|(v, _)| corner(a, v)) else {
            continue;
        };
        let side = settled(point_side(&p, &shell, sink), None)?;
        if side.is_some() {
            return Ok(side);
        }
    }
    Ok(None)
}
