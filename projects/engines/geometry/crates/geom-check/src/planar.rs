//! Group 3 of chapter 8.3 on faces that lie exactly on integer planes, whatever their shape and
//! whether their vertices are grid points or the meetings of three planes (G134).
//!
//! What it does: an [`Outline`] is a flat face as this module reads it: its integer plane, and for
//! each fin, ring by ring, the corner the edge starts at (it ends at the next fin's) and a plane
//! that crosses the face's along the edge. That plane is the other face's across the edge, or,
//! along a seam, a plane the edge's ends both lie on, or for two grid points one stood on them; so
//! a side of the polygon is a pair of planes, as EMBER keeps it, and no test here ever needs the
//! orientation of three implicit points, about 840 bits a term (19.2).
//!
//! - **A face and a line.** Where a face's plane meets another plane, [`Cut`] names each point of
//!   the line by a plane that crosses it there. Moved infinitely little off the other plane, the
//!   line crosses the face's boundary at sorted marks that pair up into stretches; the stretches on
//!   the two sides together are the whole of the closed face on the line, with no special case for
//!   a vertex or an edge lying on it.
//! - **A face that keeps to one side of another's plane.** If its corners are all on one side of
//!   that plane or on it, each one on it a vertex of the other face and each side along it an
//!   edge of the other, it holds nothing else of the plane, so the two meet only at what they
//!   share. One sign for each corner settles such a pair, with no line cut: every pair of faces
//!   of a convex body.
//! - **Two crossing faces.** Their stretches on their planes' line may overlap only where the two
//!   faces share edges and vertices.
//! - **Two faces on one plane.** Each side of each face, as a stretch of its own line, against the
//!   other face's stretches on that line, by the same rule; and two that share an edge must lie on
//!   opposite sides of it.
//! - **Two edges of one face.** Each is a stretch of its own line; they may meet only at a vertex
//!   they share.
//! - **A flat face its rounded corners have bent.** A Certified face whose grid corners leave one
//!   plane has no plane to read, so it stands for the star of triangles from the mean of its
//!   corners, one to each edge, which lies within the record's bound of the true face (every point
//!   of it is a mix of rounded corners, each within the bound of its true place), provided every
//!   triangle turns the way the ring does as a whole. Each triangle is an outline, and the face
//!   meets another where any of its triangles does. A bent face no star can stand for has no
//!   outline.
//! - **A point against a shell.** The parity of the crossings of a ray along the line of two of the
//!   point's planes, moved infinitely little off both, so that no tie is left. A shell is its
//!   faces' outlines, a bent face's being the triangles of its star: they close the shell as the
//!   faces do, and group 3's orientation asks them of every body that is not recorded Exact, so
//!   that where a shell lies does not depend on where a ring starts either.
//!
//! Why a star from the mean, and not a fan from a corner: what the checker finds of a body must
//! depend on where the body lies and on nothing else. A fan needs a corner to start from, and a
//! ring's first corner is chosen by the body's names (`geom_topo::Builder::publish_named` starts
//! each loop at its least fin), so a mirror, which turns every ring round, and a pattern's copy,
//! which renames everything, moved the fan, and a valid body's exact copy could be found crossing
//! itself. The mean of a ring's corners, the triangles from it and the test of their turning are
//! the same whichever corner the ring starts at and whichever way it runs, and the mean of a ring
//! moved by any affine map is the moved mean. So a body, its mirror, and every exact move, quarter
//! turn, scale by one factor and renumbered copy of it get one verdict. A scale by unlike factors
//! keeps the triangles and what they cross, but not the angles between them, so it can leave a
//! triangle turned against its ring: such a face waits, and no crossing is found that was not
//! there before.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is the sign of a plane at a grid point or at a meeting of three
//!   planes (`geom_num::plane::Plane::side` and `side_of_tpi`, within 384 bits), or the sign of a
//!   3 × 3 determinant of normals (`geom_num::plane::Tpi::weight`). No double is read and nothing
//!   is sampled. The mean of n corners is such a meeting, of the planes n x = Σx, n y = Σy and
//!   n z = Σz, so a star needs no arithmetic of its own.
//! - **One verdict for a shape.** A bent face's star depends on where its corners lie, in their
//!   order round the ring, and on nothing else (above). Nor does the plane a face of grid corners
//!   is read on depend on how its plane is written: it is the written plane if the corners lie on
//!   it, and otherwise the plane the corners fix, whether or not the written one is given by
//!   integers (a whole scale can make it so).
//! - **Any polygon.** A face that is not convex, has holes, or has several outer loops is cut like
//!   any other. A vertex on the line, an edge along it and a touch from one side all fall to the
//!   moved line, which sees each as ordinary crossings at one mark.
//! - **Fails closed.** A step that cannot be decided is [`Meet::Unknown`], which the caller reports,
//!   never a pass. A face without a plane or a plane for each side has no outline, and its pairs are
//!   the caller's to report. So is a bent face with no star: one with a second loop, one whose
//!   mean does not see every edge from inside, and one of five or more corners so wide that a
//!   triangle's plane passes a plane's budget (its normal is the sum of n − 2 normals of triangles
//!   of grid points, each below 2^82, against the budget's 2^83; a ring of four never does). No
//!   point is given a side of a shell with a face that has neither.
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
//! sorting of their crossings, a pair of faces on one plane the product of their sides. A face is
//! first read without keeping its outline ([`read`]), and a pair of faces whose corners are grid
//! points is tested from their fins as far as their boxes, planes and corners go ([`look`]): a
//! pair one of whose faces keeps to one side of the other's plane costs a sign for each of its
//! corners and reserves nothing, so a convex body's faces keep no outline. Outlines are kept for
//! the tests that need them; they and each other test's short lists are reserved fallibly, with
//! their logical bytes on the meter. Recursion: none.

use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cmp::Ordering;

use geom_curve::CurveKind;
use geom_num::plane::{Plane, Tpi};
use geom_num::wide::I256;
use geom_num::{P3, Sign};
use geom_op::run::reserve;
use geom_run::{LogicalSize, Stopped};
use geom_topo::entity::{Edge, Face, Loop, Shell, Vertex};
use geom_topo::ix::Arena;
use geom_topo::mass::Corner;
use geom_topo::query::Side as ShellSide;
use geom_topo::{Arenas, Ix, SurfaceKind};

use crate::combinatorial::{edge_fins, face_fins, face_loops, fin_edge, fin_ends, fin_face, rows};
use crate::curved::Pair;
use crate::geometric::{corner, planes_lie, surface};
use crate::self_intersection::{Meet, cross, diff, dot};
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

/// A list's length as the meter counts it.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The answer to a step of a test, if it had one.
fn known<T>(step: Option<T>) -> Result<T, Stuck> {
    step.ok_or(Stuck::Undecided)
}

/// A corner of an outline.
#[derive(Clone)]
struct End {
    /// The vertex; `None` for the hub of a star, which no vertex is at and no other face shares.
    vertex: Option<Ix<Vertex>>,
    at: Corner,
}

/// One side of an outline, from the corner it starts at. It ends where the next side round its
/// ring starts ([`Outline::rounds`]), so that corner is kept once.
#[derive(Clone)]
struct Side {
    /// The edge; `None` for a spoke of a star.
    edge: Option<Ix<Edge>>,
    /// The loop it runs round; `None` round a triangle of a star, which is one ring.
    ring: Option<Ix<Loop>>,
    start: End,
    /// A plane through the edge that crosses the face's plane: the edge's line is where they meet.
    across: Plane,
}

impl LogicalSize for Side {
    /// A corner of 200 bytes with its vertex, a plane of 64, the edge and the loop.
    const LOGICAL_BYTES: u64 = 280;
}

/// A flat face, as this module reads it.
pub(crate) struct Outline {
    face: Ix<Face>,
    plane: Plane,
    sides: Vec<Side>,
    /// Whether a vertex of it is a meeting of planes.
    pub(crate) implicit: bool,
}

/// What a face is made of, as outlines: one, a star's, or none for a face that has none.
pub(crate) struct Slot(pub(crate) Vec<Outline>);

impl LogicalSize for Slot {
    /// A list's three words.
    const LOGICAL_BYTES: u64 = 24;
}

impl LogicalSize for Outline {
    /// A list's three words, the plane, the face and the flag.
    const LOGICAL_BYTES: u64 = 96;
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
    const LOGICAL_BYTES: u64 = 128;
}

/// A sign as an ordering.
const fn ordering(s: Sign) -> Ordering {
    match s {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    }
}

/// The plane where coordinate `which` of a point equals `point`'s.
fn axis(point: P3, which: usize) -> Option<Plane> {
    let [x, y, z] = [point.x, point.y, point.z].map(|c| i128::from(c.get()));
    let (normal, offset) = match which {
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
    /// The point where `plane` crosses the line, if it does. Made by [`Tpi::meeting`], which finds
    /// only the sign of the point's weight: every caller reads a sign from it, and [`Tpi::new`],
    /// which also works out where the point is, cost a box three times its build (23.13).
    fn at(&self, plane: &Plane) -> Option<Tpi> {
        Tpi::meeting(self.first, self.second, *plane).ok()
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

    /// The farthest end of the runs that hold `at`.
    fn farthest(&self, runs: &[Stretch], at: &Mark) -> Result<Option<Mark>, Stuck> {
        let mut far: Option<Mark> = None;
        for run in runs {
            let holds = self.order(&run.from, at)? != Ordering::Greater
                && self.order(at, &run.to)? != Ordering::Greater;
            let beyond = match &far {
                Some(mark) => self.order(&run.to, mark)? == Ordering::Greater,
                None => true,
            };
            if holds && beyond {
                far = Some(run.to);
            }
        }
        Ok(far)
    }

    /// Whether `runs`, joined end to end, hold all of `wanted`: from its start, each step takes the
    /// run that holds the place reached and goes farthest, so one step per run reaches all they
    /// join up to.
    fn covered(&self, runs: &[Stretch], wanted: &Stretch) -> Result<bool, Stuck> {
        let mut at = wanted.from;
        for _ in runs {
            let Some(reached) = self.farthest(runs, &at)? else {
                return Ok(false);
            };
            if self.order(&reached, &wanted.to)? != Ordering::Less {
                return Ok(true);
            }
            at = reached;
        }
        Ok(false)
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

/// A plane that holds `start` and `end` and crosses `plane`, for the side of face `face` along
/// edge `edge`: another face's, then one of the ends' own, then one stood on two grid points.
fn across(
    a: &Arenas,
    (face, edge): (Ix<Face>, Ix<Edge>),
    plane: &Plane,
    ends: [&Corner; 2],
    exact: bool,
) -> Option<Plane> {
    let [start, end] = ends;
    let fits = |candidate: &Plane| {
        planes_lie(plane, candidate) == Pair::Crossing
            && ends
                .iter()
                .all(|corner| corner.side(candidate) == Ok(Sign::Zero))
    };
    let beside = edge_fins(a, edge)
        .filter_map(|fin| fin_face(a, fin))
        .filter(|&other| other != face)
        .filter_map(|other| surface(a, other, exact)?.grid);
    let own = meeting_planes(start)
        .iter()
        .chain(meeting_planes(end))
        .copied();
    beside.chain(own).find(fits).or_else(|| match (start, end) {
        (Corner::Grid(from), Corner::Grid(to)) => stood_on(plane, *from, *to),
        _ => None,
    })
}

/// The plane `f`'s grid corners lie on exactly: `None` if a corner is not a grid point, if they
/// lie on one line, or if they leave the plane of the first three that do not.
fn corners_plane(a: &Arenas, f: Ix<Face>) -> Option<Plane> {
    let grid = |n| {
        let (v, _) = fin_ends(a, n)?;
        a.vertices
            .point
            .get(v)
            .and_then(|p| a.points.get(*p))?
            .grid()
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

/// The side of face `face`, on `plane`, that fin `fin` makes: `None` for a fin on anything but a
/// line between two vertices, or one no plane crosses along.
fn side_of(
    a: &Arenas,
    face: Ix<Face>,
    fin: Ix<geom_topo::entity::Fin>,
    plane: &Plane,
    exact: bool,
) -> Option<Side> {
    let edge = fin_edge(a, fin)?;
    let curve = a.edges.curve.get(edge).and_then(|c| a.curves.get(*c));
    if !matches!(curve, Some(CurveKind::Line(_))) {
        return None;
    }
    let (first, last) = fin_ends(a, fin)?;
    let (start, end) = (corner(a, first)?, corner(a, last)?);
    Some(Side {
        edge: Some(edge),
        ring: a.fins.loop_.get(fin).copied(),
        across: across(a, (face, edge), plane, [&start, &end], exact)?,
        start: End {
            vertex: Some(first),
            at: start,
        },
    })
}

/// Face `f` as an outline, if it is one: exactly on an integer plane, with a plane for each side.
///
/// The plane is the one written for the face, if that is given by integers and the face's
/// corners lie on it, and otherwise the one its grid corners fix. So which plane a face of grid
/// corners is read on depends on where the corners lie and not on how its plane is written: a
/// plane written half a step off its corners is no integer plane, and the same plane scaled by
/// two is one, a step off them.
///
/// Why that keeps what a Certified body promises: group 2 alone holds each corner within the
/// body's bound of the surface written for the face, whichever plane is read here, and a body
/// reaches group 3 only if it passed. Group 3 has always judged the corners: a face is the polygon
/// its corners make, never a piece of its written plane. So a face read on the plane its corners
/// fix is the same polygon, every point of it a mix of corners that lie within the bound of the
/// written surface; a written plane the corners are farther from is group 2's fault, found before
/// any face is read.
///
/// `reading`: how it is read, charged or not and its sides kept or not ([`Reading`]).
fn outline(
    a: &Arenas,
    f: Ix<Face>,
    exact: bool,
    reading: Reading,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<Outline>, Stopped> {
    let kind = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s));
    if !matches!(kind, Some(SurfaceKind::Plane(_))) {
        return Ok(None);
    }
    if let Some(written) = surface(a, f, exact).and_then(|on| on.grid)
        && let Some(one) = outline_on(a, f, written, exact, reading, sink)?
    {
        return Ok(Some(one));
    }
    match corners_plane(a, f) {
        Some(fixed) => outline_on(a, f, fixed, exact, reading, sink),
        None => Ok(None),
    }
}

/// Face `f` as an outline on `plane`, if its corners lie on it and each side has a plane. Read by
/// [`Reading::Look`], it keeps no side, so its list is empty and reserves nothing.
fn outline_on(
    a: &Arenas,
    f: Ix<Face>,
    plane: Plane,
    exact: bool,
    reading: Reading,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<Outline>, Stopped> {
    let keeps = reading != Reading::Look;
    let mut sides: Vec<Side> = if keeps {
        reserve(sink.meter(), count(face_fins(a, f).count()))?
    } else {
        Vec::new()
    };
    let (mut on, mut implicit) = (true, false);
    for n in face_fins(a, f) {
        if reading != Reading::Again {
            sink.charge()?;
        }
        let Some(side) = side_of(a, f, n, &plane, exact) else {
            return Ok(None);
        };
        // A Certified face may lie within its bound of an integer plane, not on it.
        on = on && side.start.at.side(&plane) == Ok(Sign::Zero);
        implicit = implicit || matches!(side.start.at, Corner::Tpi(_));
        if keeps {
            sides.push(side);
        }
    }
    Ok(on.then_some(Outline {
        face: f,
        plane,
        sides,
        implicit,
    }))
}

/// Whether `first` and `second`, two normals, point the same way: the sign of their dot product.
fn same_way(first: [i128; 3], second: [i128; 3]) -> bool {
    let mut sum = Some(I256::from_i64(0));
    for (p, q) in first.into_iter().zip(second) {
        let term = I256::from_i128(p).checked_mul(&I256::from_i128(q));
        sum = sum.zip(term).and_then(|(s, t)| s.checked_add(&t));
    }
    sum.is_some_and(|s| s.signum() == Sign::Positive)
}

/// A corner of a face a star stands for, where a spoke ends: where it lies, and the edge from it
/// to the next.
#[derive(Clone, Copy)]
struct Spoke {
    at: P3,
    vertex: Ix<Vertex>,
    edge: Ix<Edge>,
}

impl LogicalSize for Spoke {
    /// A grid point, and the vertex's and edge's rows.
    const LOGICAL_BYTES: u64 = 32;
}

/// Whether flat face `f` is one ring of spokes: one loop, every fin a [`spoke`]. Only such a face
/// has a star.
fn rings(a: &Arenas, f: Ix<Face>) -> bool {
    let kind = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s));
    matches!(kind, Some(SurfaceKind::Plane(_)))
        && face_loops(a, f).nth(1).is_none()
        && face_fins(a, f).all(|n| spoke(a, n).is_some())
}

/// Fin `n` as a corner of a ring, if its edge is straight and starts at a grid point.
fn spoke(a: &Arenas, n: Ix<geom_topo::entity::Fin>) -> Option<Spoke> {
    let edge = fin_edge(a, n)?;
    let curve = a.edges.curve.get(edge).and_then(|c| a.curves.get(*c));
    let (vertex, _) = fin_ends(a, n)?;
    let place = a
        .vertices
        .point
        .get(vertex)
        .and_then(|p| a.points.get(*p))?;
    matches!(curve, Some(CurveKind::Line(_))).then_some(Spoke {
        at: place.grid()?,
        vertex,
        edge,
    })
}

/// The corners of face `f`, one ring of spokes ([`rings`]), in its order.
fn ring(a: &Arenas, f: Ix<Face>, sink: &mut Sink<'_, '_>) -> Result<Vec<Spoke>, Stopped> {
    let mut out: Vec<Spoke> = reserve(sink.meter(), count(face_fins(a, f).count()))?;
    out.extend(face_fins(a, f).filter_map(|n| spoke(a, n)));
    Ok(out)
}

/// `p`'s coordinates.
fn wide(p: P3) -> [i128; 3] {
    [p.x, p.y, p.z].map(|c| i128::from(c.get()))
}

/// `u` + `v`: `None` past an `i128`, which no sum over a ring of an arena reaches.
fn added(u: [i128; 3], v: [i128; 3]) -> Option<[i128; 3]> {
    let mut sum = u;
    for (total, term) in sum.iter_mut().zip(v) {
        *total = total.checked_add(term)?;
    }
    Some(sum)
}

/// The plane with normal `normal` through the point `through`: `None` for a zero normal, or a
/// normal or an offset past a plane's budget.
fn plane_at(normal: [i128; 3], through: [i128; 3]) -> Option<Plane> {
    Plane::new(normal, dot(normal, through)?.checked_neg()?).ok()
}

/// The hub of a star: the mean of a ring's corners, kept exactly as their sum Σ and their number n.
struct Hub {
    sum: [i128; 3],
    count: i128,
    /// The mean as the meeting of the planes n x = Σx, n y = Σy and n z = Σz.
    at: Corner,
}

/// One triangle of a star, as planes: its own, turning as the ring runs, and one along each of its
/// three sides that crosses it.
struct Wedge {
    plane: Plane,
    /// Along the spoke from the hub out to the edge's start.
    out: Plane,
    /// Along the ring's edge, from its start to its end.
    along: Plane,
    /// Along the spoke from the edge's end back to the hub.
    back: Plane,
}

impl Hub {
    /// The mean of `ring`'s corners: `None` for a ring of none.
    ///
    /// Bit budget: n coordinates of at most 2^40 sum to at most n · 2^40, far inside a plane's
    /// offset (2^125), and n is far inside its normal (2^83), so the three planes are planes; they
    /// lie along the three axes, so they meet.
    fn of(ring: &[Spoke]) -> Option<Hub> {
        let count = i128::from(count(ring.len()));
        let mut corners = ring.iter().map(|spoke| wide(spoke.at));
        let sum = corners.try_fold([0_i128; 3], added)?;
        let [x, y, z] = sum;
        let at = Tpi::new(
            Plane::new([count, 0, 0], x.checked_neg()?).ok()?,
            Plane::new([0, count, 0], y.checked_neg()?).ok()?,
            Plane::new([0, 0, count], z.checked_neg()?).ok()?,
        );
        Some(Hub {
            sum,
            count,
            at: Corner::Tpi(at.ok()?),
        })
    }

    /// n times the vector from the mean to `p`, n p − Σ: whole numbers, where the vector itself
    /// has n in its denominators.
    fn arm(&self, p: P3) -> Option<[i128; 3]> {
        let mut out = wide(p);
        for (slot, total) in out.iter_mut().zip(self.sum) {
            *slot = self.count.checked_mul(*slot)?.checked_sub(total)?;
        }
        Some(out)
    }

    /// The triangle from the mean to `p` and on to `q`, as planes. Its own has the normal
    /// (n p − Σ) × (q − p) and holds `p`, so it holds `q` and the mean. A spoke's is the plane
    /// through the spoke and along an axis the triangle's plane is not parallel to, which
    /// therefore crosses it; the edge's is one stood on its two corners.
    ///
    /// `None` for a triangle of no area, and for one whose plane is past a plane's budget. The
    /// normal is the sum, over the ring's other n − 2 corners r, of (p − r) × (q − r), the normal
    /// of a triangle of grid points, below 2^82: it stays inside 2^83 for every ring of four
    /// corners, and for a ring of five or more unless the ring spans most of the frame.
    fn wedge(&self, p: P3, q: P3) -> Option<Wedge> {
        let (from, to) = (self.arm(p)?, self.arm(q)?);
        let plane = plane_at(cross(from, diff(q, p)?)?, wide(p))?;
        let k = plane.n().iter().position(|&c| c != 0)?;
        let mut unit = [0_i128; 3];
        *unit.get_mut(k)? = 1;
        let spoke = |arm: [i128; 3], end: P3| plane_at(cross(arm, unit)?, wide(end));
        Some(Wedge {
            plane,
            out: spoke(from, p)?,
            along: stood_on(&plane, p, q)?,
            back: spoke(to, q)?,
        })
    }
}

/// One triangle of a star as an outline: from the hub out to `p`, along `p`'s edge to `q`, and
/// back. The hub is a meeting of planes but no vertex, so the outline is not marked `implicit`:
/// the face's own edges all run between grid points, and are tested as the segments they are.
fn triangle(
    face: Ix<Face>,
    hub: &Hub,
    wedge: &Wedge,
    (p, q): (&Spoke, &Spoke),
    sink: &mut Sink<'_, '_>,
) -> Result<Outline, Stopped> {
    let middle = End {
        vertex: None,
        at: hub.at.clone(),
    };
    let rim = |s: &Spoke| End {
        vertex: Some(s.vertex),
        at: Corner::Grid(s.at),
    };
    let mut sides: Vec<Side> = reserve(sink.meter(), 3)?;
    sides.push(Side {
        edge: None,
        ring: None,
        start: middle,
        across: wedge.out,
    });
    sides.push(Side {
        edge: Some(p.edge),
        ring: None,
        start: rim(p),
        across: wedge.along,
    });
    sides.push(Side {
        edge: None,
        ring: None,
        start: rim(q),
        across: wedge.back,
    });
    Ok(Outline {
        face,
        plane: wedge.plane,
        sides,
        implicit: false,
    })
}

/// Whether every one of `normals`, those of a star's triangles, points the way their sum does.
///
/// The sum is n times the ring's area vector, whatever point the star is drawn from, so each
/// triangle is asked to turn the way the ring does as a whole. No triangle is the one the others
/// are held to, so the answer does not depend on where the ring starts.
fn one_way(normals: impl Iterator<Item = [i128; 3]> + Clone) -> bool {
    let mut each = normals.clone();
    let mut all = normals;
    each.try_fold([0_i128; 3], added)
        .is_some_and(|whole| all.all(|one| same_way(one, whole)))
}

/// Flat face `f`, one ring of spokes ([`rings`]), as the star of triangles from the mean of its
/// corners, one to each edge, if every triangle turns the way the ring does as a whole; none
/// otherwise.
///
/// The mean, the triangles and the test of their turning are the same whichever corner the ring
/// starts at and whichever way it runs, and the mean of a ring moved by an affine map is the moved
/// mean. So a face's star, and what the checker finds of it, depend on where its corners lie and on
/// nothing else: not on a name, nor on the order a body was built in. A fan from the ring's first
/// corner did depend on them, and found a valid body's mirror crossing itself.
fn star(a: &Arenas, f: Ix<Face>, sink: &mut Sink<'_, '_>) -> Result<Vec<Outline>, Stopped> {
    let ring = ring(a, f, sink)?;
    // Only a ring of no corners has no mean, and it has no triangle to ask for one.
    let hub = Hub::of(&ring);
    let mut out: Vec<Outline> = reserve(sink.meter(), count(ring.len()))?;
    for (p, q) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        let wedge = hub
            .as_ref()
            .and_then(|hub| Some((hub, hub.wedge(p.at, q.at)?)));
        let Some((hub, wedge)) = wedge else {
            return Ok(Vec::new());
        };
        out.push(triangle(f, hub, &wedge, (p, q), sink)?);
    }
    let turns = one_way(out.iter().map(|one| one.plane.n()));
    Ok(if turns { out } else { Vec::new() })
}

/// What face `f`, read as `read`, is made of, as outlines: its own, a star's triangles, or none.
/// Read again, not charged: [`read`] charged the reading.
fn slot(
    a: &Arenas,
    f: Ix<Face>,
    read: Read,
    exact: bool,
    sink: &mut Sink<'_, '_>,
) -> Result<Slot, Stopped> {
    if matches!(read, Read::Bare) {
        return Ok(Slot(Vec::new()));
    }
    Ok(Slot(match outline(a, f, exact, Reading::Again, sink)? {
        Some(one) => {
            let mut list: Vec<Outline> = reserve(sink.meter(), 1)?;
            list.push(one);
            list
        }
        None => star(a, f, sink)?,
    }))
}

/// One slot per face of `a`, whose faces [`read`] read as `reads`.
pub(crate) fn outlines(
    a: &Arenas,
    reads: &[Read],
    exact: bool,
    sink: &mut Sink<'_, '_>,
) -> Result<Vec<Slot>, Stopped> {
    let mut out: Vec<Slot> = reserve(sink.meter(), count(reads.len()))?;
    for (f, &read) in rows::<Face>(a.rows(Arena::Face)).zip(reads) {
        out.push(slot(a, f, read, exact, sink)?);
    }
    Ok(out)
}

/// How a face's outline is read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// Charged, and nothing kept or reserved: [`read`].
    Look,
    /// Its sides kept and not charged: an outline kept after [`read`] has charged the same
    /// reading.
    Again,
}

/// What group 3 reads of a face before it keeps any outline. Most pairs of faces are settled
/// from this alone ([`look`]), so a body whose pairs all are, a box or a prism, keeps no outline
/// and reserves nothing for one.
#[derive(Clone, Copy)]
pub(crate) enum Read {
    /// A face with neither an outline nor a star: one not on a plane, or a flat one with no
    /// outline that is not one ring of spokes. Its pairs are the caller's to test.
    Bare,
    /// A face with an outline on this plane, every corner of it a grid point.
    Plain(Plane),
    /// Any other flat face: one with a corner where three planes meet, whose edges are tested on
    /// its outline, or one with no outline that is one ring of spokes, which may have a star. Its
    /// pairs are tested on the outlines kept.
    Kept,
}

impl LogicalSize for Read {
    /// A plane and its tag.
    const LOGICAL_BYTES: u64 = 80;
}

/// Face `f` as group 3 first reads it, charged as reading its outline is.
pub(crate) fn read(
    a: &Arenas,
    f: Ix<Face>,
    exact: bool,
    sink: &mut Sink<'_, '_>,
) -> Result<Read, Stopped> {
    Ok(match outline(a, f, exact, Reading::Look, sink)? {
        Some(one) if !one.implicit => Read::Plain(one.plane),
        Some(_) => Read::Kept,
        None if rings(a, f) => Read::Kept,
        None => Read::Bare,
    })
}

impl Outline {
    /// Each side with the corner it ends at: where the next side round its ring starts, and the
    /// ring's first side after its last. The sides are kept ring by ring, in the order each ring
    /// runs, and group 1 has held every loop closed, each fin ending at the vertex the next
    /// starts from; a star's triangle is one ring of three. So the next side round the ring is
    /// the one kept after it, if that is of its ring, and otherwise the first kept of its ring.
    fn rounds(&self) -> impl Iterator<Item = (&Side, &End)> {
        self.sides.iter().enumerate().map(|(at, side)| {
            let of_ring = |other: &&Side| other.ring == side.ring;
            let after = self.sides.get(at.saturating_add(1)).filter(of_ring);
            let next = after.or_else(|| self.sides.iter().find(of_ring));
            (side, next.map_or(&side.start, |to| &to.start))
        })
    }

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
        let mut marks: Vec<Mark> = reserve(sink.meter(), count(self.sides.len()))?;
        for (side, end) in self.rounds() {
            sink.charge()?;
            let from = known(side.start.at.side(against).ok())?;
            let to = known(end.at.side(against).ok())?;
            if (from == above) != (to == above) {
                let mark = if from == Sign::Zero {
                    cut.mark(&side.start.at)?
                } else if to == Sign::Zero {
                    cut.mark(&end.at)?
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
        let above = self.moved(against, cut, Sign::Positive, sink)?;
        let below = self.moved(against, cut, Sign::Negative, sink)?;
        let both = count(above.len().saturating_add(below.len()));
        let mut out: Vec<Stretch> = reserve(sink.meter(), both)?;
        out.extend(above);
        out.extend(below);
        Ok(out)
    }

    /// The stretches of this face on `cut` moved to the side of `against` that `above` names.
    fn moved(
        &self,
        against: &Plane,
        cut: &Cut,
        above: Sign,
        sink: &mut Sink<'_, '_>,
    ) -> Result<Vec<Stretch>, Stuck> {
        let marks = self.crossings(against, cut, above, sink)?;
        let mut out: Vec<Stretch> = reserve(sink.meter(), count(marks.len()))?;
        // A closed boundary crosses a line an even number of times.
        let ends = marks.iter().skip(1).step_by(2);
        out.extend(
            marks
                .iter()
                .step_by(2)
                .zip(ends)
                .map(|(&from, &to)| Stretch { from, to }),
        );
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
    let mut out: Vec<Stretch> = reserve(sink.meter(), count(f.sides.len().saturating_mul(2)))?;
    for (s, end) in f.rounds() {
        if s.runs_on(g) && cut.holds(&s.start.at) && cut.holds(&end.at) {
            let (from, to) = (cut.mark(&s.start.at)?, cut.mark(&end.at)?);
            out.push(cut.stretch(from, to)?);
        }
        if s.starts_on(g) && cut.holds(&s.start.at) {
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

impl Side {
    /// Whether the corner this side starts at is a vertex of `other` too: the one vertex, not
    /// another at its place. A star's hub is no vertex, so it is shared with nothing, not with
    /// another star's hub at the same place either.
    fn starts_on(&self, other: &Outline) -> bool {
        self.start.vertex.is_some_and(|v| other.has_vertex(v))
    }

    /// Whether this side's edge is an edge of `other` too. A star's spoke is no edge.
    fn runs_on(&self, other: &Outline) -> bool {
        self.edge.is_some_and(|e| other.has_edge(e))
    }
}

impl Outline {
    /// Whether vertex `v` is a corner of this outline.
    fn has_vertex(&self, v: Ix<Vertex>) -> bool {
        self.sides.iter().any(|t| t.start.vertex == Some(v))
    }

    /// Whether edge `e` is a side of this outline.
    fn has_edge(&self, e: Ix<Edge>) -> bool {
        self.sides.iter().any(|t| t.edge == Some(e))
    }
    /// Whether this face reaches the plane of `other`, which crosses its own, only where the two
    /// faces share a vertex or an edge: its corners on one side of that plane or on it, and not
    /// all on it; each corner on the plane a vertex of `other`; and each side along the plane an
    /// edge of `other`. Two such faces meet nowhere else, since they can meet only on `other`'s
    /// plane.
    ///
    /// Why the corners decide it. A face is a closed, bounded piece of its plane whose boundary is
    /// its sides, so it lies inside the convex hull of its corners, and the other plane's
    /// equation, which is zero or of one sign at every corner, is so over the whole face. The
    /// planes cross, so on the face's own plane the equation is zero along one line and of both
    /// signs about every point of that line. So it is zero at no point with a neighbourhood in
    /// the face, only on a side. Along a side it runs evenly from one end's value to the other's,
    /// neither of the other sign, so it is zero at an end that is on the plane, along the whole
    /// side if both ends are, and nowhere else. What the face holds of the other plane is
    /// therefore its corners on it and its sides along it and nothing more, whatever its shape:
    /// not convex, with holes, or one triangle of a star.
    ///
    /// Every pair of faces of a convex body is settled here, by one sign for each corner. The
    /// answer is the one [`crossing`]'s cut gives, which finds the same corners and sides on the
    /// planes' line and excuses them by the same two tests of what is shared
    /// ([`Side::starts_on`], [`Side::runs_on`]). A face with corners on both sides of the plane,
    /// or with a corner or a side on it that the other does not share, is left to the cut; so is
    /// one with every corner on it, a face folded onto the planes' line.
    fn rests_on(&self, other: &Outline, sink: &mut Sink<'_, '_>) -> Result<bool, Stuck> {
        let rounds = self.rounds().map(|(side, end)| {
            Ok(Round {
                start: &side.start.at,
                end: &end.at,
                vertex: side.start.vertex,
                edge: side.edge,
            })
        });
        let held = (|v| other.has_vertex(v), |e| other.has_edge(e));
        rests(rounds, &other.plane, held, sink)
    }
}

/// A side as [`rests`] reads it: the corners it runs from and to, the vertex it starts at and its
/// edge, each `None` at a star's hub or along its spoke.
struct Round<C> {
    start: C,
    end: C,
    vertex: Option<Ix<Vertex>>,
    edge: Option<Ix<Edge>>,
}

/// Whether a face whose sides are `rounds` reaches `plane`, the other face's, which crosses its
/// own, only where the two share a vertex or an edge: [`Outline::rests_on`], whose documentation
/// says why that keeps them apart, asked of a face's outline or of its fins ([`fin_rounds`]).
/// `held` says whether the other face has a vertex, and whether it has an edge.
fn rests<C: Borrow<Corner>>(
    rounds: impl Iterator<Item = Result<Round<C>, Stuck>>,
    plane: &Plane,
    held: (impl Fn(Ix<Vertex>) -> bool, impl Fn(Ix<Edge>) -> bool),
    sink: &mut Sink<'_, '_>,
) -> Result<bool, Stuck> {
    let (has_vertex, has_edge) = held;
    let (mut above, mut below) = (false, false);
    for round in rounds {
        sink.charge()?;
        let round = round?;
        match known(round.start.borrow().side(plane).ok())? {
            Sign::Positive => above = true,
            Sign::Negative => below = true,
            Sign::Zero => {
                let along = known(round.end.borrow().side(plane).ok())? == Sign::Zero;
                let shared = round.vertex.is_some_and(&has_vertex)
                    && (!along || round.edge.is_some_and(&has_edge));
                if !shared {
                    return Ok(false);
                }
            }
        }
    }
    Ok(above != below)
}

/// The sides of plain face `f` ([`Read::Plain`]) as [`Outline::rounds`] gives them of its
/// outline, read from its fins: the outline keeps a side for each fin, in the fins' order,
/// starting at the fin's first vertex, and group 1 has held every loop closed, so each side ends
/// at its fin's last vertex, where the next side round the loop starts.
fn fin_rounds(a: &Arenas, f: Ix<Face>) -> impl Iterator<Item = Result<Round<Corner>, Stuck>> + '_ {
    face_fins(a, f).map(move |n| -> Result<Round<Corner>, Stuck> {
        let (v, w) = known(fin_ends(a, n))?;
        Ok(Round {
            start: known(corner(a, v))?,
            end: known(corner(a, w))?,
            vertex: Some(v),
            edge: fin_edge(a, n),
        })
    })
}

/// Whether face `f` has a vertex, and whether it has an edge: as its outline's sides would say,
/// one side for each fin, at the fin's first vertex and along its edge.
fn has(
    a: &Arenas,
    f: Ix<Face>,
) -> (
    impl Fn(Ix<Vertex>) -> bool + '_,
    impl Fn(Ix<Edge>) -> bool + '_,
) {
    let vertex = move |v| face_fins(a, f).any(|n| fin_ends(a, n).is_some_and(|(s, _)| s == v));
    let edge = move |e| face_fins(a, f).any(|n| fin_edge(a, n) == Some(e));
    (vertex, edge)
}

/// How two faces on crossing planes meet: apart if either reaches the other's plane only at what
/// they share ([`Outline::rests_on`]), with no line cut; otherwise as their stretches of the
/// planes' line lie.
fn crossing(f: &Outline, g: &Outline, sink: &mut Sink<'_, '_>) -> Result<Meet, Stuck> {
    if f.rests_on(g, sink)? || g.rests_on(f, sink)? {
        return Ok(Meet::Apart);
    }
    cut(f, g, sink)
}

/// How two faces on crossing planes meet on their planes' line, cut: what they share of it must
/// be what they share as edges and vertices.
fn cut(f: &Outline, g: &Outline, sink: &mut Sink<'_, '_>) -> Result<Meet, Stuck> {
    let cut = Cut {
        first: f.plane,
        second: g.plane,
    };
    let own = f.stretches(&g.plane, &cut, sink)?;
    let other = g.stretches(&f.plane, &cut, sink)?;
    along((f, g), &cut, &own, &other, sink)
}

/// Whether `f` and `g`, on one plane, both reach `side`'s line from one side of it for any length,
/// which is what two faces that share the edge must not do: the line moved each way, against what
/// each face holds of it.
fn same_side(
    f: &Outline,
    g: &Outline,
    side: &Side,
    sink: &mut Sink<'_, '_>,
) -> Result<bool, Stuck> {
    let cut = Cut {
        first: f.plane,
        second: side.across,
    };
    for above in [Sign::Positive, Sign::Negative] {
        let own = f.moved(&side.across, &cut, above, sink)?;
        let other = g.moved(&side.across, &cut, above, sink)?;
        for s in &own {
            for t in &other {
                sink.charge()?;
                if let Some(common) = cut.overlap(s, t)?
                    && cut.order(&common.from, &common.to)? == Ordering::Less
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// How two faces on one plane meet: each side of each, as a stretch of its own line, against the
/// other; then each side, which must keep them on opposite sides of it if they both reach it.
fn coplanar(f: &Outline, g: &Outline, sink: &mut Sink<'_, '_>) -> Result<Meet, Stuck> {
    for (one, other) in [(f, g), (g, f)] {
        for (side, end) in one.rounds() {
            let cut = Cut {
                first: one.plane,
                second: side.across,
            };
            let from = cut.mark(&side.start.at)?;
            let to = cut.mark(&end.at)?;
            let own = [cut.stretch(from, to)?];
            let cuts = other.stretches(&side.across, &cut, sink)?;
            if along((one, other), &cut, &own, &cuts, sink)? == Meet::Cross {
                return Ok(Meet::Cross);
            }
        }
    }
    for side in &f.sides {
        if same_side(f, g, side, sink)? {
            return Ok(Meet::Cross);
        }
    }
    Ok(Meet::Apart)
}

/// How the faces of `first` and `second` meet: what any two of their outlines found.
pub(crate) fn slots_meet(
    a: &Arenas,
    first: &[Outline],
    second: &[Outline],
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stopped> {
    let mut found = Meet::Apart;
    for f in first {
        for g in second {
            found = found.or(faces_meet(a, f, g, sink)?);
        }
    }
    Ok(found)
}

/// How faces `f` and `g` meet, exactly. Stored boxes that do not overlap settle it: group 2 has
/// held each face's vertices to its box.
fn faces_meet(
    a: &Arenas,
    f: &Outline,
    g: &Outline,
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stopped> {
    if boxes_apart(a, f.face, g.face) {
        return Ok(Meet::Apart);
    }
    let found = match planes_lie(&f.plane, &g.plane) {
        Pair::Crossing => crossing(f, g, sink),
        Pair::Apart => Ok(Meet::Apart),
        _ => coplanar(f, g, sink),
    };
    settled(found, Meet::Unknown)
}

/// Whether the stored boxes of faces `f` and `g` do not overlap.
fn boxes_apart(a: &Arenas, f: Ix<Face>, g: Ix<Face>) -> bool {
    let boxes = a.faces.bound.get(f).zip(a.faces.bound.get(g));
    boxes.is_some_and(|(p, q)| !p.overlaps(q))
}

/// What [`look`] finds of two plain faces.
pub(crate) enum Look {
    /// How they meet.
    Found(Meet),
    /// What is left to test, on their outlines ([`finish`]).
    Left(Left),
}

/// The test [`look`] leaves to two plain faces' outlines.
#[derive(Clone, Copy)]
pub(crate) enum Left {
    /// Their planes cross, and neither face rests on the other's: the line is cut.
    Cut,
    /// They lie on one plane.
    Coplanar,
}

/// How two plain faces ([`Read::Plain`]), each given with its plane, meet as far as their boxes,
/// their planes and their corners say. These are [`faces_meet`]'s tests and [`crossing`]'s first,
/// in the same order, charged alike, and asked of the faces' fins rather than their outlines,
/// which give the same corners, vertices and edges: so a pair they settle needs no outline kept,
/// and what they leave is the rest of those tests ([`finish`]).
pub(crate) fn look(
    a: &Arenas,
    first: (Ix<Face>, &Plane),
    second: (Ix<Face>, &Plane),
    sink: &mut Sink<'_, '_>,
) -> Result<Look, Stopped> {
    let ((f, plane), (g, other)) = (first, second);
    if boxes_apart(a, f, g) {
        return Ok(Look::Found(Meet::Apart));
    }
    let found = match planes_lie(plane, other) {
        Pair::Crossing => (|| -> Result<Look, Stuck> {
            let rested = rests(fin_rounds(a, f), other, has(a, g), sink)?
                || rests(fin_rounds(a, g), plane, has(a, f), sink)?;
            Ok(if rested {
                Look::Found(Meet::Apart)
            } else {
                Look::Left(Left::Cut)
            })
        })(),
        Pair::Apart => Ok(Look::Found(Meet::Apart)),
        _ => Ok(Look::Left(Left::Coplanar)),
    };
    settled(found, Look::Found(Meet::Unknown))
}

/// How two plain faces meet that [`look`] left with `left`, tested on their outlines, `first`
/// and `second`: each its face's one outline.
pub(crate) fn finish(
    left: Left,
    first: &[Outline],
    second: &[Outline],
    sink: &mut Sink<'_, '_>,
) -> Result<Meet, Stopped> {
    let mut found = Meet::Apart;
    for f in first {
        for g in second {
            let one = match left {
                Left::Cut => cut(f, g, sink),
                Left::Coplanar => coplanar(f, g, sink),
            };
            found = found.or(settled(one, Meet::Unknown)?);
        }
    }
    Ok(found)
}

/// Where two sides meet on the line of the first: `None` if the second, which ends at `end`,
/// keeps to one side of it.
fn contact(cut: &Cut, s: &Side, (t, end): (&Side, &End)) -> Result<Option<Stretch>, Stuck> {
    let from = known(t.start.at.side(&s.across).ok())?;
    let to = known(end.at.side(&s.across).ok())?;
    let point = |at: Mark| Stretch { from: at, to: at };
    Ok(match (from, to) {
        (Sign::Zero, Sign::Zero) => {
            let (p, q) = (cut.mark(&t.start.at)?, cut.mark(&end.at)?);
            Some(cut.stretch(p, q)?)
        }
        (p, q) if p == q => None,
        // An end of `t` on the line is where `t`'s own plane crosses it.
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
        let side = |e| known(o.rounds().find(|(s, _)| s.edge == Some(e)));
        let ((s, s_end), (t, t_end)) = (side(first)?, side(second)?);
        let cut = Cut {
            first: o.plane,
            second: s.across,
        };
        let (from, to) = (cut.mark(&s.start.at)?, cut.mark(&s_end.at)?);
        let own = cut.stretch(from, to)?;
        let Some(touch) = contact(&cut, s, (t, t_end))? else {
            return Ok(Meet::Apart);
        };
        let Some(common) = cut.overlap(&own, &touch)? else {
            return Ok(Meet::Apart);
        };
        let mut runs: Vec<Stretch> = reserve(sink.meter(), 2)?;
        // Both sides are edges, so each of their ends is a vertex, never a star's hub.
        for end in [&s.start, s_end] {
            if [&t.start, t_end].iter().any(|e| e.vertex == end.vertex) {
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
    shell: &[Slot],
    sink: &mut Sink<'_, '_>,
) -> Result<Option<ShellSide>, Stuck> {
    let [x, y, z] = known(frame(p))?;
    let on = |c: &Corner| [&x, &y, &z].iter().all(|q| c.side(q) == Ok(Sign::Zero));
    let outlines = || shell.iter().flat_map(|slot| slot.0.iter());
    if outlines().any(|o| o.sides.iter().any(|s| on(&s.start.at))) {
        return Ok(None);
    }
    let ray = Cut {
        first: x,
        second: y,
    };
    let mut inside = false;
    for o in outlines() {
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
/// not at a corner of `t`, or `None` if there is none, or a face of `t` has neither an outline nor
/// a star.
///
/// Each face of `t` is read as group 3 read it against every other face: its outline, or for a
/// bent face the triangles of its star, which close the shell as the faces do (two neighbours
/// share their edge, two triangles of a star their spoke). So the side does not depend on where a
/// ring starts or which way it runs, as the count of a ray's crossings with a fan from each loop's
/// first corner does on a bent shell (`geom_topo::query::side_of_shell`). Two shells that do not
/// cross lie wholly on one side of each other, so whichever corner of `s` comes first gives the
/// one answer, and a corner at a star's hub is passed over like one at a vertex.
pub(crate) fn shell_side(
    a: &Arenas,
    s: Ix<Shell>,
    t: Ix<Shell>,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<ShellSide>, Stopped> {
    let of = |shell: Ix<Shell>| {
        rows::<Face>(a.rows(Arena::Face)).filter(move |&f| a.faces.shell.get(f) == Some(&shell))
    };
    let mut shell: Vec<Slot> = reserve(sink.meter(), count(of(t).count()))?;
    for f in of(t) {
        // Read as group 3 reads a face, then kept: charged once.
        let read = read(a, f, false, sink)?;
        shell.push(slot(a, f, read, false, sink)?);
    }
    // A face nothing stands for would leave the shell open to the ray, so against such a shell no
    // corner is asked. (No body brings one this far: such a face shares a corner with its
    // neighbours, so their planes are within twice the bound of each other or cross, and each of
    // those pairs was reported unchecked before any shell was compared.)
    let closed = shell.iter().all(|one| !one.0.is_empty());
    let corners = of(s)
        .flat_map(|f| face_fins(a, f))
        .filter_map(|n| fin_ends(a, n).and_then(|(v, _)| corner(a, v)))
        .filter(|_| closed);
    for p in corners {
        let side = settled(point_side(&p, &shell, sink), None)?;
        if side.is_some() {
            return Ok(side);
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::unwrap_used,
        reason = "test code: a failure should stop loudly"
    )]

    use geom_num::plane::Lpi;

    use super::*;

    /// The plane x = `at`.
    fn wall(at: i128) -> Plane {
        Plane::new([1, 0, 0], -at).unwrap()
    }

    /// A point where a line meets a plane, of the one kind no vertex is.
    fn crossing_point() -> Corner {
        let (p, q) = (P3::new(0, 0, 0).unwrap(), P3::new(10, 0, 0).unwrap());
        Corner::Lpi(Lpi::new(p, q, wall(5)).unwrap())
    }

    /// The grid point (`x`, `y`, `z`).
    fn grid(x: i64, y: i64, z: i64) -> P3 {
        P3::new(x, y, z).unwrap()
    }

    /// A ring's corners as a star reads them; no test here asks for their vertices or edges.
    fn spokes(corners: &[[i64; 3]]) -> Vec<Spoke> {
        corners
            .iter()
            .map(|&[x, y, z]| Spoke {
                at: grid(x, y, z),
                vertex: Ix::new(0).unwrap(),
                edge: Ix::new(0).unwrap(),
            })
            .collect()
    }

    /// Whether `corner` lies on `plane`.
    fn on(corner: &Corner, plane: &Plane) -> bool {
        corner.side(plane) == Ok(Sign::Zero)
    }

    #[test]
    fn a_sum_or_a_plane_that_does_not_fit_is_refused() {
        assert_eq!(added([1, 2, 3], [10, 20, 30]), Some([11, 22, 33]));
        let most = i128::MAX;
        for past in [[most, 0, 0], [0, most, 0], [0, 0, most]] {
            assert_eq!(added(past, [1; 3]), None);
        }
        let plane = plane_at([0, 0, 2], [5, 6, 7]).unwrap();
        assert_eq!((plane.n(), plane.d()), ([0, 0, 2], -14));
        // No normal, a normal past its budget, an offset past its own, and one past an `i128`.
        assert!(plane_at([0; 3], [5, 6, 7]).is_none());
        assert!(plane_at([1 << 83, 0, 0], [0; 3]).is_none());
        assert!(plane_at([1, 0, 0], [1 << 125, 0, 0]).is_none());
        assert!(plane_at([i128::MAX, 0, 0], [2, 0, 0]).is_none());
    }

    #[test]
    fn a_hub_is_the_mean_of_its_ring_exactly() {
        // The mean, (5/4, 3/4, 1/2), is no grid point.
        let ring = spokes(&[[0, 0, 0], [4, 0, 0], [1, 3, 0], [0, 0, 2]]);
        let hub = Hub::of(&ring).unwrap();
        assert_eq!((hub.sum, hub.count), ([5, 3, 2], 4));
        for (normal, offset) in [
            ([4, 0, 0], -5),
            ([0, 4, 0], -3),
            ([0, 0, 2], -1),
            ([2, 2, 2], -5),
        ] {
            assert!(on(&hub.at, &Plane::new(normal, offset).unwrap()));
        }
        assert_eq!(hub.at.side(&wall(1)), Ok(Sign::Positive));
        assert_eq!(hub.at.side(&wall(2)), Ok(Sign::Negative));
        // Four times the way from the mean to a corner.
        assert_eq!(hub.arm(grid(4, 0, 0)), Some([11, -3, -2]));
        assert!(Hub::of(&[]).is_none());
    }

    #[test]
    fn a_wedge_is_a_triangle_from_the_hub_with_a_plane_along_each_side() {
        let ring = spokes(&[[0, 0, 0], [10, 0, 0], [10, 10, 1], [0, 10, 0]]);
        let hub = Hub::of(&ring).unwrap();
        let (p, q) = (grid(10, 0, 0), grid(10, 10, 1));
        let wedge = hub.wedge(p, q).unwrap();
        // (4 p − Σ) × (q − p) = (20, −20, −1) × (0, 10, 1).
        assert_eq!(wedge.plane.n(), [-10, -20, 200]);
        let (start, end) = (Corner::Grid(p), Corner::Grid(q));
        for corner in [&hub.at, &start, &end] {
            assert!(on(corner, &wedge.plane));
        }
        // Each side's plane holds the side's two ends and not the third corner, and crosses.
        for (side, ends, off) in [
            (&wedge.out, [&hub.at, &start], &end),
            (&wedge.along, [&start, &end], &hub.at),
            (&wedge.back, [&end, &hub.at], &start),
        ] {
            assert!(ends.iter().all(|corner| on(corner, side)));
            assert!(!on(off, side));
            assert_eq!(planes_lie(&wedge.plane, side), Pair::Crossing);
        }
        // Run the other way, the triangle turns the other way.
        assert_eq!(hub.wedge(q, p).unwrap().plane.n(), [10, 20, -200]);
    }

    #[test]
    fn a_triangle_of_no_area_or_past_a_planes_budget_has_no_wedge() {
        // The second corner is the mean of the five.
        let ring = spokes(&[[-4, -2, 0], [0, 3, 1], [4, -2, 2], [6, 8, 0], [-6, 8, 2]]);
        let hub = Hub::of(&ring).unwrap();
        assert!(hub.wedge(grid(-4, -2, 0), grid(0, 3, 1)).is_none());
        assert!(hub.wedge(grid(4, -2, 2), grid(6, 8, 0)).is_some());
        // Across the whole frame: five corners pass the budget, four do not.
        let m = (1_i64 << 40) - 1;
        let wide = [[-m, -m, 0], [m, -m, 0], [m, m, 1], [0, m, 0], [-m, 0, 0]];
        let (p, q) = (grid(-m, -m, 0), grid(m, -m, 0));
        assert!(Hub::of(&spokes(&wide)).unwrap().wedge(p, q).is_none());
        let four = [wide[0], wide[1], wide[2], wide[4]];
        assert!(Hub::of(&spokes(&four)).unwrap().wedge(p, q).is_some());
    }

    #[test]
    fn a_star_turns_one_way_when_every_normal_agrees_with_their_sum() {
        let up = [0, 0, 5];
        assert!(one_way([up, [1, 0, 2], [-1, 0, 2]].into_iter()));
        assert!(!one_way([up, up, [0, 0, -1]].into_iter()));
        // Two that cancel agree with nothing.
        assert!(!one_way([up, [0, 0, -5]].into_iter()));
        // No one of them is the one the rest are held to. Held to the first, these would pass
        // started from the second and fail started from the first, which the third points against.
        let mixed = [[4, 0, 1], [0, 0, 3], [-4, 0, 1]];
        for start in 0..3 {
            let mut turned = mixed;
            turned.rotate_left(start);
            assert!(one_way(turned.into_iter()), "{start}");
        }
        // A sum past an `i128` is no answer, so no star.
        assert!(!one_way([[i128::MAX, 0, 0], [i128::MAX, 0, 0]].into_iter()));
    }

    #[test]
    fn a_step_with_no_answer_is_unknown_never_a_pass() {
        assert!(matches!(known::<u8>(None), Err(Stuck::Undecided)));
        assert!(matches!(known(Some(3)), Ok(3)));
        assert_eq!(
            settled(Err(Stuck::Undecided), Meet::Unknown).ok(),
            Some(Meet::Unknown)
        );
    }

    #[test]
    fn a_point_of_no_vertex_kind_has_no_mark_and_no_frame() {
        let cut = Cut {
            first: wall(5),
            second: Plane::new([0, 1, 0], 0).unwrap(),
        };
        assert!(matches!(cut.mark(&crossing_point()), Err(Stuck::Undecided)));
        assert!(frame(&crossing_point()).is_none());
    }

    /// The line along x where z = 0 meets y = 0, and the mark of each place along it.
    fn x_axis() -> (Cut, impl Fn(i128) -> Mark) {
        let cut = Cut {
            first: Plane::new([0, 0, 1], 0).unwrap(),
            second: Plane::new([0, 1, 0], 0).unwrap(),
        };
        (cut, |at| Mark(wall(at)))
    }

    #[test]
    fn a_line_runs_the_way_its_two_planes_turn() {
        // (0, 0, 1) × (0, 1, 0) is (−1, 0, 0): the line runs down x.
        let (cut, at) = x_axis();
        assert_eq!(cut.order(&at(5), &at(2)).ok(), Some(Ordering::Less));
        assert_eq!(cut.order(&at(2), &at(5)).ok(), Some(Ordering::Greater));
        assert_eq!(cut.order(&at(2), &at(2)).ok(), Some(Ordering::Equal));
    }

    #[test]
    fn a_line_holds_only_what_lies_on_both_planes() {
        let (cut, _) = x_axis();
        assert!(cut.holds(&Corner::Grid(P3::new(7, 0, 0).unwrap())));
        assert!(!cut.holds(&Corner::Grid(P3::new(7, 1, 0).unwrap())));
        assert!(!cut.holds(&Corner::Grid(P3::new(7, 0, 1).unwrap())));
    }

    #[test]
    fn stretches_cover_what_their_runs_join_up_to() {
        let (cut, at) = x_axis();
        let run = |a: i128, b: i128| Stretch {
            from: at(a),
            to: at(b),
        };
        let covered = |runs: &[Stretch], wanted| cut.covered(runs, &wanted).ok();
        // Joined end to end, in either order.
        let two = [run(9, 5), run(5, 1)];
        assert_eq!(covered(&two, run(8, 2)), Some(true));
        assert_eq!(covered(&[two[1], two[0]], run(8, 2)), Some(true));
        // A place no run holds, one reached short of the end, and a run that only touches.
        assert_eq!(covered(&two, run(10, 2)), Some(false));
        assert_eq!(covered(&two, run(8, 0)), Some(false));
        assert_eq!(covered(&[run(9, 7), run(5, 1)], run(8, 2)), Some(false));
        assert_eq!(covered(&[run(5, 5)], run(5, 5)), Some(true));
        assert_eq!(covered(&[run(5, 5)], run(6, 4)), Some(false));
        assert_eq!(covered(&[], run(5, 5)), Some(false));
        // What two stretches have in common.
        let common = cut.overlap(&run(9, 3), &run(6, 1)).ok().flatten().unwrap();
        assert_eq!(common.from.0, at(6).0);
        assert_eq!(common.to.0, at(3).0);
        assert!(cut.overlap(&run(9, 7), &run(5, 1)).ok().flatten().is_none());
    }

    #[test]
    fn found_together_a_crossing_outranks_an_unknown_and_an_unknown_apart() {
        for (first, second, both) in [
            (Meet::Apart, Meet::Apart, Meet::Apart),
            (Meet::Apart, Meet::Unknown, Meet::Unknown),
            (Meet::Unknown, Meet::Apart, Meet::Unknown),
            (Meet::Unknown, Meet::Cross, Meet::Cross),
            (Meet::Cross, Meet::Unknown, Meet::Cross),
            (Meet::Cross, Meet::Apart, Meet::Cross),
        ] {
            assert_eq!(first.or(second), both);
        }
    }
}
