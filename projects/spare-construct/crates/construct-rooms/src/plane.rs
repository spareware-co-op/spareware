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
//! A separation line (job 6.3) is a [`Bound`] of a different kind: a segment with no thickness
//! ([`Outline::Line`]). It is two-sided: both of its directions are kept as pieces, so the open
//! space on either side of it is walled by it, as a wall's face is for one side. Where it runs
//! inside a wall's outline or along a wall's face the wall stands and the line is dropped; where
//! two lines lie along each other the earlier one stands. A line with one end loose is a spike in
//! a face's walk and is cut out of it, and a line that joins one loop to another (a column to a
//! wall) is cut out and leaves them two loops, so every loop returned is simple. A line that
//! crosses nothing or bounds nothing changes nothing here and is no refusal. Area boundary lines
//! (job 14.6) are lines too: [`find`] over lines alone is the search for an area.
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
    /// A segment of no thickness between two grid points, which bounds the open space on both
    /// sides of it: a room separation line or an area boundary line.
    Line([[i64; 2]; 2]),
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
    /// Whether a corner where a line meets a wall's face (or another line) was off the grid and
    /// was rounded, once, to the nearest grid step: the area's twice-area may then be odd, and the
    /// measures round it once ([`crate::measure_flat`]). Never set for a corner between walls
    /// only, which is refused.
    pub rounded: bool,
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

/// One outline, checked: its wall and its corners anticlockwise (a line's two ends).
struct Ring {
    wall: Id,
    /// Whether it is a line, whose two ends make both of its directions.
    line: bool,
    corners: Vec<Pt>,
    /// The low and high corner of its box, in steps.
    low: [i64; 2],
    high: [i64; 2],
}

fn checked(bound: &Bound) -> Result<Ring, RoomError> {
    let line = matches!(bound.outline, Outline::Line(_));
    let ends;
    let corners = match &bound.outline {
        Outline::Curved => {
            return Err(RoomError::CurvedWall {
                wall: bound.wall.clone(),
            });
        }
        Outline::Polygon(corners) => corners,
        Outline::Line(at) => {
            ends = at.to_vec();
            &ends
        }
    };
    if corners.len() < if line { 2 } else { 3 } || (line && corners.first() == corners.get(1)) {
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
    if !line {
        match sign(&twice_area(&ring)) {
            Ordering::Greater => {}
            Ordering::Less => ring.reverse(),
            Ordering::Equal => {
                return Err(RoomError::NoOutline {
                    wall: bound.wall.clone(),
                });
            }
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
        line,
        corners: ring,
        low,
        high,
    })
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
    let mine = rings.get(i).is_some_and(|ring| ring.line);
    for &j in near {
        let Some(other) = rings.get(j) else { continue };
        match (mine, other.line) {
            // A wall's edge stands whatever lines run across or along it.
            (false, true) => continue,
            // A line is dropped inside a wall and along its face: the wall bounds there.
            (true, false) => {
                if side(&mid, &other.corners) != Side::Outside {
                    return false;
                }
                continue;
            }
            // Two lines along each other: the earlier stands.
            (true, true) => {
                if matches!(side(&mid, &other.corners), Side::On(_)) && j < i {
                    return false;
                }
                continue;
            }
            (false, false) => {}
        }
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

/// A cycle with every line walked both ways cut out: where it holds a piece and, later, the piece
/// back along the same way (a line with one end loose, or one that ties a loop to another), both
/// go, and the walk between them is a loop of its own. Every loop returned is simple, and
/// cycles of nothing but a line there and back are no loops at all.
fn simple(pieces: &[Piece], cycle: Vec<usize>) -> Vec<Vec<usize>> {
    let mut by_ends: BTreeMap<(&Pt, &Pt), usize> = BTreeMap::new();
    for (at, &k) in cycle.iter().enumerate() {
        if let Some(piece) = pieces.get(k) {
            by_ends.insert((&piece.a, &piece.b), at);
        }
    }
    for (at, &k) in cycle.iter().enumerate() {
        let Some(piece) = pieces.get(k) else { continue };
        let Some(&back) = by_ends.get(&(&piece.b, &piece.a)) else {
            continue;
        };
        if back <= at {
            continue;
        }
        let inside = cycle.get(at.saturating_add(1)..back).unwrap_or_default();
        let mut outside = cycle
            .get(back.saturating_add(1)..)
            .unwrap_or_default()
            .to_vec();
        outside.extend(cycle.get(..at).unwrap_or_default());
        let mut found = simple(pieces, inside.to_vec());
        found.extend(simple(pieces, outside));
        return found;
    }
    if cycle.is_empty() {
        Vec::new()
    } else {
        vec![cycle]
    }
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

/// The loop's corners on the grid, or the refusal naming the walls at the first one off it. A
/// corner off the grid where one of the two faces is a line in `lines` is rounded once to the
/// nearest grid step instead (the second answer is whether any was), for a line may run at any
/// angle; one between walls only is refused.
fn on_grid(corners: &[(Pt, Id)], lines: &BTreeSet<Id>) -> Result<(Vec<Corner>, bool), RoomError> {
    let n = corners.len();
    let mut out = Vec::with_capacity(n);
    let mut rounded = false;
    for (k, (at, wall)) in corners.iter().enumerate() {
        let before = corners
            .get(k.checked_sub(1).unwrap_or(n.saturating_sub(1)))
            .map(|c| c.1.clone());
        let at = if let Some(at) = at.on_grid() {
            at
        } else if lines.contains(wall) || before.as_ref().is_some_and(|b| lines.contains(b)) {
            rounded = true;
            at.nearest_grid().ok_or(RoomError::TooBig)?
        } else {
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
    Ok((out, rounded))
}

/// The room around the grid point `at` among the walls `bounds`, as the module header says:
/// `None` when the open region around it is not enclosed.
///
/// # Errors
/// [`RoomError::CurvedWall`] for a curved wall among them; [`RoomError::NoOutline`] for an
/// outline of fewer than three corners or no area; [`RoomError::TooBig`] beyond the grid;
/// [`RoomError::PointInWall`] for a point inside or on a wall's outline, [`RoomError::PointOnLine`]
/// for one on a line; [`RoomError::OffGrid`]
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
            return Err(if ring.line {
                RoomError::PointOnLine {
                    line: ring.wall.clone(),
                }
            } else {
                RoomError::PointInWall {
                    wall: ring.wall.clone(),
                }
            });
        }
    }
    let pieces = pieces(&rings);
    let cycles: Vec<Vec<usize>> = loops(&pieces)?
        .into_iter()
        .flat_map(|cycle| simple(&pieces, cycle))
        .collect();
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
    let lines: BTreeSet<Id> = rings
        .iter()
        .filter(|ring| ring.line)
        .map(|ring| ring.wall.clone())
        .collect();
    let mut rounded = false;
    let mut holes = Vec::new();
    for (k, ring) in &anticlockwise {
        let (Some(a), Some(b)) = (ring.first(), ring.get(1).or_else(|| ring.first())) else {
            continue;
        };
        if innermost(&a.mid(b), &clockwise) == Some(room) {
            let cycle = cycles.get(*k).ok_or(RoomError::Inconsistent)?;
            take(cycle);
            let (hole, was) = on_grid(&turned(&rings, &pieces, cycle)?, &lines)?;
            rounded |= was;
            holes.push(hole);
        }
    }
    holes.sort_by(|a, b| a.first().map(|c| c.at).cmp(&b.first().map(|c| c.at)));
    let (outer, was) = on_grid(&turned(&rings, &pieces, outer_cycle)?, &lines)?;
    rounded |= was;
    Ok(Some(Found {
        outer,
        holes,
        walls,
        rounded,
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
