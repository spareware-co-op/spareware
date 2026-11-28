//! A room's loop, found in plan from the walls around a point: exact plane geometry on the walls'
//! outlines, our own code.
//!
//! Why our own: the answer must be exact on the kernel's grid, the same bytes on every machine,
//! and every corner where two wall faces cross is a rational point that a float library would
//! round. The search below is a few hundred lines of plain exact geometry (segment meetings,
//! a point against a loop, the turn between two directions) on exact rationals from dashu-ratio,
//! in the one module [`crate::exact`]; a general polygon-union crate would bring floats or its own
//! number type and still leave the room's own rules (which face bounds, what a hole is) to us.
//!
//! What it does. Each wall is given as its outline in plan ([`Bound`]): a straight wall's is the
//! four corners of its outer faces, exactly as the builders lay it out on the grid. The walls'
//! outlines together are solid; everything else is open. A room is the open region around its
//! point, and it is enclosed when that region is bounded:
//!
//! 1. Every edge of every outline is cut wherever another edge meets it (crossing, touching or
//!    overlapping), exactly.
//! 2. A piece of an edge is kept where it is the edge of the walls taken together: not inside
//!    another outline, not on another outline's edge running the other way (two walls face to
//!    face), and once where two outlines' edges run along each other the same way.
//! 3. The kept pieces, each with the walls on its left, are joined into closed loops, turning at
//!    each corner to the first kept piece anticlockwise from the way back. Walls that touch at
//!    one corner only therefore close a loop there: their faces meet, and no gap is left.
//! 4. A loop running clockwise has open space inside it; the smallest such loop round the point
//!    is the room's outer boundary. A loop running anticlockwise has walls inside it (an island
//!    wall, a column, a wall cluster); each one whose smallest clockwise loop round it is the
//!    room's is a hole in the room.
//!
//! So the face that bounds a room is always the wall's finish face toward the room, whichever of
//! its two faces that is (the decision note). A wall's opening (a door, a window) does not break
//! the loop: the wall's outline is read whole. The answer is given anticlockwise for the outer
//! loop and clockwise for each hole, each corner a grid point with the wall whose face runs from
//! it; a corner off the grid is refused, never rounded ([`RoomError::OffGrid`]).
//!
//! Interim: a curved wall is refused by name ([`RoomError::CurvedWall`]) until the kernel's exact
//! arcs reach the walls (the interim register's row); nothing else here is interim.

use crate::error::RoomError;
use crate::exact::{Pt, Side, cross, dot, meeting, side, sign, sooner, twice_area};
use dashu_ratio::RBig;
use kit_core::Id;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// The farthest a corner may be from the origin, in grid steps: the kernel's grid, `2^40 - 1`
/// steps (171.8 km on `Q0`).
pub const LIMIT: i64 = 1_099_511_627_775;

/// A wall's outline in plan, as the room search reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outline {
    /// A closed polygon of grid points, either way round, with an area: a straight wall's four
    /// outer corners, or a column's outline.
    Polygon(Vec<[i64; 2]>),
    /// A curved wall, which is refused until the kernel's exact arcs reach the walls.
    Curved,
}

/// One wall (or column) around a room, by its element's ID, with its outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    /// The element's permanent ID.
    pub wall: Id,
    /// Its outline in plan, in grid steps of the model.
    pub outline: Outline,
}

/// A corner of a room's loop: where it is, and the wall whose face runs from it to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corner {
    /// The grid point, in steps.
    pub at: [i64; 2],
    /// The wall whose face runs from here to the next corner.
    pub wall: Id,
}

/// A room's loop as found: its outer boundary anticlockwise, each hole clockwise, and every wall
/// whose face is on any of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The outer boundary, anticlockwise, starting at its least corner (by x, then y).
    pub outer: Vec<Corner>,
    /// The holes (island walls, columns), each clockwise from its least corner, in that order.
    pub holes: Vec<Vec<Corner>>,
    /// Every wall that bounds the room, by ID, ascending.
    pub walls: BTreeSet<Id>,
}

impl Found {
    /// Whether the grid point `at` is in the room: inside its outer boundary and in no hole, and
    /// on no boundary.
    #[must_use]
    pub fn holds(&self, at: [i64; 2]) -> bool {
        let ring =
            |corners: &[Corner]| -> Vec<Pt> { corners.iter().map(|c| Pt::grid(c.at)).collect() };
        let p = Pt::grid(at);
        side(&p, &ring(&self.outer)) == Side::Inside
            && self
                .holes
                .iter()
                .all(|hole| side(&p, &ring(hole)) == Side::Outside)
    }
}

/// A piece of an outline's edge, the walls on its left.
#[derive(Clone, Debug)]
struct Piece {
    a: Pt,
    b: Pt,
    /// The outline it is from, by its place in the list.
    of: usize,
}

/// One outline, checked: its wall and its corners anticlockwise.
struct Ring {
    wall: Id,
    corners: Vec<Pt>,
    /// The low and high corner of its box, in steps.
    low: [i64; 2],
    high: [i64; 2],
}

fn checked(bound: &Bound) -> Result<Ring, RoomError> {
    let corners = match &bound.outline {
        Outline::Curved => {
            return Err(RoomError::CurvedWall {
                wall: bound.wall.clone(),
            });
        }
        Outline::Polygon(corners) => corners,
    };
    if corners.len() < 3 {
        return Err(RoomError::NoOutline {
            wall: bound.wall.clone(),
        });
    }
    if corners
        .iter()
        .flatten()
        .any(|v| v.unsigned_abs() > LIMIT.unsigned_abs())
    {
        return Err(RoomError::TooBig);
    }
    let mut ring: Vec<Pt> = corners.iter().copied().map(Pt::grid).collect();
    match sign(&twice_area(&ring)) {
        Ordering::Greater => {}
        Ordering::Less => ring.reverse(),
        Ordering::Equal => {
            return Err(RoomError::NoOutline {
                wall: bound.wall.clone(),
            });
        }
    }
    let mut low = [i64::MAX; 2];
    let mut high = [i64::MIN; 2];
    for c in corners {
        for axis in 0..2 {
            if let (Some(l), Some(h), Some(&v)) =
                (low.get_mut(axis), high.get_mut(axis), c.get(axis))
            {
                *l = (*l).min(v);
                *h = (*h).max(v);
            }
        }
    }
    Ok(Ring {
        wall: bound.wall.clone(),
        corners: ring,
        low,
        high,
    })
}

/// Whether [`find`] takes a wall's outline: a polygon of three corners or more, within the grid,
/// with an area. The error is the one [`find`] gives for it, so a reader that holds outlines
/// checked can tell beforehand whether a search among them would fail on one.
///
/// # Errors
/// [`RoomError::CurvedWall`], [`RoomError::NoOutline`] or [`RoomError::TooBig`], as [`find`].
pub fn check(bound: &Bound) -> Result<(), RoomError> {
    checked(bound).map(|_| ())
}

/// The edges of a ring, each from a corner to the next.
fn edges(ring: &[Pt]) -> impl Iterator<Item = (&Pt, &Pt)> {
    ring.iter()
        .enumerate()
        .filter_map(|(k, a)| Some((a, ring.get(k.saturating_add(1)).or_else(|| ring.first())?)))
}

/// Whether two boxes meet (edges included).
fn boxes_meet(a: &Ring, b: &Ring) -> bool {
    a.low[0] <= b.high[0] && b.low[0] <= a.high[0] && a.low[1] <= b.high[1] && b.low[1] <= a.high[1]
}

/// Every piece of every edge that is an edge of the walls taken together (step 2 of the module
/// header).
fn pieces(rings: &[Ring]) -> Vec<Piece> {
    let mut kept = Vec::new();
    for (i, ring) in rings.iter().enumerate() {
        let near: Vec<usize> = (0..rings.len())
            .filter(|&j| j != i && rings.get(j).is_some_and(|other| boxes_meet(ring, other)))
            .collect();
        for (a, b) in edges(&ring.corners) {
            let mut cuts = vec![RBig::ZERO, RBig::ONE];
            for &j in &near {
                let Some(other) = rings.get(j) else { continue };
                for (c, d) in edges(&other.corners) {
                    cuts.extend(meeting(a, b, c, d));
                }
            }
            cuts.sort();
            cuts.dedup();
            for pair in cuts.windows(2) {
                let [t0, t1] = pair else { continue };
                let (p, q) = (a.toward(b, t0), a.toward(b, t1));
                if keeps(rings, &near, i, &p, &q) {
                    kept.push(Piece { a: p, b: q, of: i });
                }
            }
        }
    }
    kept
}

/// Whether the piece `from`-`to` of an edge of outline `i` is an edge of the walls taken together.
fn keeps(rings: &[Ring], near: &[usize], i: usize, from: &Pt, to: &Pt) -> bool {
    let mid = from.mid(to);
    let along = to.less(from);
    for &j in near {
        let Some(other) = rings.get(j) else { continue };
        match side(&mid, &other.corners) {
            Side::Outside => {}
            Side::Inside => return false,
            Side::On(k) => {
                let (Some(c), Some(d)) = (
                    other.corners.get(k),
                    other
                        .corners
                        .get(k.saturating_add(1))
                        .or_else(|| other.corners.first()),
                ) else {
                    continue;
                };
                // Along each other the same way: kept once, from the first outline. Face to face:
                // walls on both sides, so not an edge at all.
                if sign(&dot(&along, &d.less(c))) == Ordering::Less || j < i {
                    return false;
                }
            }
        }
    }
    true
}

/// The kept pieces joined into closed loops (step 3 of the module header), each a list of
/// pieces by index.
fn loops(pieces: &[Piece]) -> Result<Vec<Vec<usize>>, RoomError> {
    let mut from: BTreeMap<&Pt, Vec<usize>> = BTreeMap::new();
    for (k, piece) in pieces.iter().enumerate() {
        from.entry(&piece.a).or_default().push(k);
    }
    let mut used = vec![false; pieces.len()];
    let mut found = Vec::new();
    for start in 0..pieces.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut cycle = Vec::new();
        let mut at = start;
        loop {
            if let Some(slot) = used.get_mut(at) {
                *slot = true;
            }
            cycle.push(at);
            if cycle.len() > pieces.len() {
                return Err(RoomError::Inconsistent);
            }
            let piece = pieces.get(at).ok_or(RoomError::Inconsistent)?;
            let back = piece.a.less(&piece.b);
            let mut best: Option<(usize, Pt)> = None;
            for &next in from.get(&piece.b).map_or(&[][..], Vec::as_slice) {
                let Some(out) = pieces.get(next) else {
                    continue;
                };
                let way = out.b.less(&out.a);
                if best.as_ref().is_none_or(|(_, b)| sooner(&back, &way, b)) {
                    best = Some((next, way));
                }
            }
            let (next, _) = best.ok_or(RoomError::Inconsistent)?;
            if next == start {
                break;
            }
            if used.get(next).copied().unwrap_or(true) {
                return Err(RoomError::Inconsistent);
            }
            at = next;
        }
        found.push(cycle);
    }
    Ok(found)
}

/// A loop's corners, as points.
fn ring_of(pieces: &[Piece], cycle: &[usize]) -> Vec<Pt> {
    cycle
        .iter()
        .filter_map(|&k| pieces.get(k).map(|p| p.a.clone()))
        .collect()
}

/// The smallest clockwise loop strictly round the point `p`, by its index in `cycles`.
fn innermost(p: &Pt, clockwise: &[(usize, Vec<Pt>, RBig)]) -> Option<usize> {
    clockwise
        .iter()
        .filter(|(_, ring, _)| side(p, ring) == Side::Inside)
        .min_by(|a, b| b.2.cmp(&a.2))
        .map(|(k, _, _)| *k)
}

/// The place `by` after `k` round a loop of `n`.
fn wrap(k: usize, by: usize, n: usize) -> usize {
    k.saturating_add(by).checked_rem(n).unwrap_or(0)
}

/// A loop turned the other way round, as corners each with the wall whose face runs from it,
/// with corners in a straight run merged, starting at its least corner.
fn turned(rings: &[Ring], pieces: &[Piece], cycle: &[usize]) -> Result<Vec<(Pt, Id)>, RoomError> {
    let mut corners: Vec<(Pt, Id)> = Vec::with_capacity(cycle.len());
    for &k in cycle.iter().rev() {
        let piece = pieces.get(k).ok_or(RoomError::Inconsistent)?;
        let wall = rings
            .get(piece.of)
            .ok_or(RoomError::Inconsistent)?
            .wall
            .clone();
        corners.push((piece.b.clone(), wall));
    }
    // A corner between two pieces in one straight line, the same way, is no corner.
    let mut changed = true;
    while changed && corners.len() > 3 {
        changed = false;
        let n = corners.len();
        for k in 0..n {
            let (Some(a), Some(b), Some(c)) = (
                corners.get(k),
                corners.get(wrap(k, 1, n)),
                corners.get(wrap(k, 2, n)),
            ) else {
                continue;
            };
            let (ab, bc) = (b.0.less(&a.0), c.0.less(&b.0));
            if sign(&cross(&ab, &bc)) == Ordering::Equal
                && sign(&dot(&ab, &bc)) == Ordering::Greater
            {
                corners.remove(wrap(k, 1, n));
                changed = true;
                break;
            }
        }
    }
    let least = corners
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.0.cmp(&b.1.0))
        .map_or(0, |(k, _)| k);
    corners.rotate_left(least);
    Ok(corners)
}

/// The loop's corners on the grid, or the refusal naming the walls at the first one off it.
fn on_grid(corners: &[(Pt, Id)]) -> Result<Vec<Corner>, RoomError> {
    let n = corners.len();
    let mut out = Vec::with_capacity(n);
    for (k, (at, wall)) in corners.iter().enumerate() {
        let Some(at) = at.on_grid() else {
            let before = corners
                .get(k.checked_sub(1).unwrap_or(n.saturating_sub(1)))
                .map(|c| c.1.clone());
            let mut walls: Vec<Id> = before.into_iter().chain([wall.clone()]).collect();
            walls.sort();
            walls.dedup();
            return Err(RoomError::OffGrid { walls });
        };
        out.push(Corner {
            at,
            wall: wall.clone(),
        });
    }
    Ok(out)
}

/// The room around the grid point `at` among the walls `bounds`, as the module header says:
/// `None` when the open region around it is not enclosed.
///
/// # Errors
/// [`RoomError::CurvedWall`] for a curved wall among them; [`RoomError::NoOutline`] for an
/// outline of fewer than three corners or no area; [`RoomError::TooBig`] beyond the grid;
/// [`RoomError::PointInWall`] for a point inside or on a wall's outline; [`RoomError::OffGrid`]
/// for a corner of the room off the grid; [`RoomError::Inconsistent`] only if the search's own
/// loops do not close, which they always do.
pub fn find(bounds: &[Bound], at: [i64; 2]) -> Result<Option<Found>, RoomError> {
    if at.iter().any(|v| v.unsigned_abs() > LIMIT.unsigned_abs()) {
        return Err(RoomError::TooBig);
    }
    let rings = bounds.iter().map(checked).collect::<Result<Vec<_>, _>>()?;
    let p = Pt::grid(at);
    for ring in &rings {
        if side(&p, &ring.corners) != Side::Outside {
            return Err(RoomError::PointInWall {
                wall: ring.wall.clone(),
            });
        }
    }
    let pieces = pieces(&rings);
    let cycles = loops(&pieces)?;
    let mut clockwise = Vec::new();
    let mut anticlockwise = Vec::new();
    for (k, cycle) in cycles.iter().enumerate() {
        let ring = ring_of(&pieces, cycle);
        let area = twice_area(&ring);
        if sign(&area) == Ordering::Less {
            clockwise.push((k, ring, area));
        } else {
            anticlockwise.push((k, ring));
        }
    }
    let Some(room) = innermost(&p, &clockwise) else {
        return Ok(None);
    };
    let mut walls = BTreeSet::new();
    let mut take = |cycle: &[usize]| {
        for &k in cycle {
            if let Some(ring) = pieces.get(k).and_then(|piece| rings.get(piece.of)) {
                walls.insert(ring.wall.clone());
            }
        }
    };
    let outer_cycle = cycles.get(room).ok_or(RoomError::Inconsistent)?;
    take(outer_cycle);
    let mut holes = Vec::new();
    for (k, ring) in &anticlockwise {
        let (Some(a), Some(b)) = (ring.first(), ring.get(1).or_else(|| ring.first())) else {
            continue;
        };
        if innermost(&a.mid(b), &clockwise) == Some(room) {
            let cycle = cycles.get(*k).ok_or(RoomError::Inconsistent)?;
            take(cycle);
            holes.push(on_grid(&turned(&rings, &pieces, cycle)?)?);
        }
    }
    holes.sort_by(|a, b| a.first().map(|c| c.at).cmp(&b.first().map(|c| c.at)));
    let outer = on_grid(&turned(&rings, &pieces, outer_cycle)?)?;
    Ok(Some(Found {
        outer,
        holes,
        walls,
    }))
}

/// Twice a grid loop's signed area, in square steps: above zero anticlockwise.
pub(crate) fn twice_area_of(corners: &[Corner]) -> Result<i128, RoomError> {
    let mut total: i128 = 0;
    for (k, a) in corners.iter().enumerate() {
        let Some(b) = corners.get(k.saturating_add(1)).or_else(|| corners.first()) else {
            continue;
        };
        let [ax, ay] = a.at.map(i128::from);
        let [bx, by] = b.at.map(i128::from);
        // Every coordinate is within 2^40, so each product is under 2^80.
        let term = ax
            .checked_mul(by)
            .and_then(|l| ay.checked_mul(bx).and_then(|r| l.checked_sub(r)))
            .ok_or(RoomError::TooBig)?;
        total = total.checked_add(term).ok_or(RoomError::TooBig)?;
    }
    Ok(total)
}
