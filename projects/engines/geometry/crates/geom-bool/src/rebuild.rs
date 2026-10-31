//! The canonical rebuild: the pieces a selection keeps, made into a body (11.3, step 5).
//!
//! What it does: [`select`] reads a test over classified pieces and turns each kept piece to face
//! out of the result. [`rebuild`] merges those pieces into maximal faces, finds the edges between
//! the faces as maximal runs on one line, makes loops, joins faces into shells by their order
//! around each edge, nests shells into regions, and publishes a [`Body`].
//!
//! How it runs: every corner of every piece is sorted by its exact coordinates, so one point has
//! one number and numbers rise along any line that runs up the sort. Every side of every piece is
//! sorted by its line's Plücker coordinates, so one line is one group. Along a line, pieces of one
//! sheet on both sides of a stretch cancel, and what is left are the stretches of boundary: runs
//! with the same fins merge, so the cuts of the subdivision and of the convex split disappear.
//!
//! Guarantees:
//!
//! - **The same bytes however the boxes fell.** Faces, edges and vertices are numbered by exact
//!   keys that depend on the shape alone: not on the boxes, the leaf size, the threads, or the order
//!   the pieces came in (11.3; tested).
//! - **Exact.** Points are compared by cross-multiplying their homogeneous coordinates, lines by
//!   their Plücker coordinates, in at most 576 bits, and nothing is rounded but the shadows below.
//! - **Plane-based.** A vertex on the grid is a grid point. Any other is `Meet`, its lowest valid
//!   triple of the planes around it. Its edges' `Line`s are shadows between rounded ends, as 8.3
//!   allows: the fins decide. An edge's two planes are its two faces' planes, read through its fins,
//!   so the body stores them once.
//! - **Refused, never guessed.** Pieces that overlap on one sheet, and a stretch with a fin that has
//!   no partner across it, mean the pieces are no closed boundary: [`RebuildError`].
//!
//! Method: chapter 11.3's rebuild (`projects/engines/geometry/architecture/11-booleans.md`), which
//! goes further than EMBER's section 6, whose output keeps T-junctions (Trettner, Nehring-Wirxel and
//! Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>); a line's coordinates are Plücker's,
//! <https://doi.org/10.1098/rstl.1865.0017>; radial order at an edge is Weiler's radial edge
//! structure, 1985, <https://doi.org/10.1109/MCG.1985.276271>.
//!
//! Cost: O(n log n) exact comparisons for n sides of pieces, and a sweep of each line. Charges the
//! meter one unit per side of a piece, per fin and per shell.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::Cell as Once;
use core::cmp::Ordering;
use core::fmt;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi};
use geom_num::{I256, I320, NumError, P3, Sign, Wide};
use geom_poly::convex::Piece;
use geom_poly::order::{Flap, Line, Ray, about, around, normals_det};
use geom_poly::plane::canonical;
use geom_poly::subdivide::around as box_around;
use geom_poly::trace::{Classified, Polygon, TraceError, classify};
use geom_poly::winding::{Kept, Winding, WindingTest, kept};
use geom_run::{Stopped, WorkMeter};
use geom_topo::Line as Shadow;
use geom_topo::{
    Body, BodyKind, Box3, Builder, CurveKind, Edge, Face, Fin, Ix, Loop, PointKind, Region,
    RegionKind, Sense, Shell, SurfaceKind, TopoError, Vertex,
};

/// What merges (11.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    /// Everything on one plane facing one way, whatever it came from.
    RemoveRedundant,
    /// Only pieces of one face of one operand with the same vectors on each side.
    KeepSeams,
}

/// The surface an input face lies on, and which way its polygon's normal runs on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Carrier {
    /// The surface.
    pub surface: SurfaceKind,
    /// [`Sense::Forward`] when the polygon's normal is the surface's.
    pub sense: Sense,
}

/// A kept piece on its way to a face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outward {
    /// The piece, its normal pointing out of the result.
    pub piece: Piece,
    /// The input polygon it came from, by index.
    pub polygon: usize,
    /// Whether it faces the other way from that polygon.
    pub turned: bool,
    /// The vector in front of it, out of the result.
    pub front: Winding,
    /// The vector behind it, in the result.
    pub back: Winding,
}

/// Why pieces could not be made into a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildError {
    /// The numbers layer refused.
    Num(NumError),
    /// A trace failed.
    Trace(TraceError),
    /// The topology refused.
    Topo(TopoError),
    /// The meter stopped the run.
    Stopped(Stopped),
    /// Two pieces of one sheet overlap.
    Overlap,
    /// A stretch of boundary has no partner across it: the pieces do not close.
    Open,
    /// A shell faces inward with no shell around it.
    Inside,
}

impl fmt::Display for RebuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebuildError::Num(e) => write!(f, "pieces that cannot be rebuilt: {e:?}"),
            RebuildError::Trace(e) => write!(f, "{e}"),
            RebuildError::Topo(e) => write!(f, "a rebuilt body refused: {e:?}"),
            RebuildError::Stopped(_) => f.write_str("the meter stopped the rebuild"),
            RebuildError::Overlap => f.write_str("two pieces of one face overlap"),
            RebuildError::Open => f.write_str("kept pieces do not close"),
            RebuildError::Inside => f.write_str("a shell faces inward with no shell around it"),
        }
    }
}

impl core::error::Error for RebuildError {}

impl From<NumError> for RebuildError {
    fn from(e: NumError) -> Self {
        RebuildError::Num(e)
    }
}

impl From<TraceError> for RebuildError {
    fn from(e: TraceError) -> Self {
        RebuildError::Trace(e)
    }
}

impl From<TopoError> for RebuildError {
    fn from(e: TopoError) -> Self {
        RebuildError::Topo(e)
    }
}

impl From<Stopped> for RebuildError {
    fn from(e: Stopped) -> Self {
        RebuildError::Stopped(e)
    }
}

/// The pieces `test` keeps of `classified`, each facing out of the result, an overlap once.
#[must_use]
pub fn select(test: &WindingTest, classified: &[Classified]) -> Vec<Outward> {
    let mut out = Vec::new();
    for c in classified.iter().filter(|c| c.first) {
        match kept(test, &c.front, &c.back) {
            Kept::No => {}
            Kept::AsIs => out.push(Outward {
                piece: c.piece.clone(),
                polygon: c.polygon,
                turned: false,
                front: c.front.clone(),
                back: c.back.clone(),
            }),
            Kept::Flipped => {
                let [x, y, z] = c.piece.face.n();
                let negate = |v: i128| v.saturating_neg();
                if let Ok(face) =
                    Plane::new([negate(x), negate(y), negate(z)], negate(c.piece.face.d()))
                {
                    let mut bounds = c.piece.bounds.clone();
                    bounds.reverse();
                    out.push(Outward {
                        piece: Piece { face, bounds },
                        polygon: c.polygon,
                        turned: true,
                        front: c.back.clone(),
                        back: c.front.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Pieces that merge: one oriented plane, and under [`Merge::KeepSeams`] one source and one pair of
/// vectors.
struct Sheet {
    /// The plane, facing out.
    face: Plane,
    /// The lowest polygon among its pieces, whose surface the face takes.
    polygon: usize,
    /// Whether that polygon faces the other way.
    turned: bool,
}

/// A corner of a piece, its coordinates X / w with w positive.
struct Corner {
    point: Tpi,
    x: [I320; 3],
    w: I256,
}

/// The sign of a / b − c / d for b and d positive, in `Wide<M>`.
fn ratio_order<const A: usize, const B: usize, const M: usize>(
    a: &Wide<A>,
    b: &Wide<B>,
    c: &Wide<A>,
    d: &Wide<B>,
) -> Option<Ordering> {
    let left = a.resize::<M>()?.checked_mul(&d.resize::<M>()?)?;
    let right = c.resize::<M>()?.checked_mul(&b.resize::<M>()?)?;
    Some(match left.checked_sub(&right)?.signum() {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    })
}

/// Sorts points lexicographically: at most 2^294 · 2^252 in 576 bits.
fn point_order(a: &Corner, b: &Corner) -> Option<Ordering> {
    for (p, q) in a.x.iter().zip(&b.x) {
        let order = ratio_order::<5, 4, 9>(p, &a.w, q, &b.w)?;
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

/// A corner of `face` where `p` and `q` meet.
#[allow(
    clippy::many_single_char_names,
    reason = "p and q are planes, and x and w the point, as in Tpi::homogeneous"
)]
fn corner(face: Plane, p: Plane, q: Plane) -> Result<Corner, NumError> {
    let point = Tpi::new(face, p, q)?;
    let (x, w) = point.homogeneous()?;
    if w.is_negative() {
        let turn = |v: &I320| v.checked_neg().ok_or(NumError::BudgetExceeded);
        let [a, b, c] = &x;
        Ok(Corner {
            point,
            x: [turn(a)?, turn(b)?, turn(c)?],
            w: w.checked_neg().ok_or(NumError::BudgetExceeded)?,
        })
    } else {
        Ok(Corner { point, x, w })
    }
}

/// The Plücker coordinates of a line: its direction, then its moment, the direction's first
/// non-zero component positive. Below 2^209, in [`I256`].
#[derive(Clone, Copy, Debug)]
struct LineKey {
    coordinates: [I256; 6],
    first: usize,
}

/// The line where `p` and `q` meet: direction `n_p` × `n_q`, moment `d_p` `n_q` − `d_q` `n_p`, which
/// is the same up to scale for any two planes through the line.
fn line_key(p: &Plane, q: &Plane) -> Result<LineKey, NumError> {
    let wide = I256::from_i128;
    let product = |a: i128, b: i128| wide(a).checked_mul(&wide(b));
    let difference =
        |a: i128, b: i128, c: i128, d: i128| product(a, b)?.checked_sub(&product(c, d)?);
    let ([px, py, pz], pd) = (p.n(), p.d());
    let ([qx, qy, qz], qd) = (q.n(), q.d());
    let all = [
        difference(py, qz, pz, qy),
        difference(pz, qx, px, qz),
        difference(px, qy, py, qx),
        difference(pd, qx, qd, px),
        difference(pd, qy, qd, py),
        difference(pd, qz, qd, pz),
    ];
    let mut coordinates = [I256::from_i128(0); 6];
    for (slot, value) in coordinates.iter_mut().zip(all) {
        *slot = value.ok_or(NumError::BudgetExceeded)?;
    }
    let first = coordinates
        .iter()
        .take(3)
        .position(|v| !v.is_zero())
        .ok_or(NumError::Degenerate)?;
    if coordinates.get(first).is_some_and(I256::is_negative) {
        for slot in &mut coordinates {
            *slot = slot.checked_neg().ok_or(NumError::BudgetExceeded)?;
        }
    }
    Ok(LineKey { coordinates, first })
}

/// The order of two lines, equal when they are one line: by which component of the direction comes
/// first, then every coordinate over that component, below 2^418 in 512 bits.
fn line_order(a: &LineKey, b: &LineKey) -> Option<Ordering> {
    if a.first != b.first {
        return Some(a.first.cmp(&b.first));
    }
    let (over_a, over_b) = (a.coordinates.get(a.first)?, b.coordinates.get(b.first)?);
    for (p, q) in a.coordinates.iter().zip(&b.coordinates) {
        let order = ratio_order::<4, 4, 8>(p, over_a, q, over_b)?;
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

/// A side of a piece.
struct Side {
    key: LineKey,
    /// Its ends' numbers, the lower first.
    low: usize,
    high: usize,
    /// Whether the piece's turn runs from `low` to `high`.
    rising: bool,
    sheet: usize,
    /// The piece, numbered across all sheets.
    piece: usize,
    /// The bound the side lies on, positive inside the piece.
    bound: Plane,
}

/// One fin of a run: a sheet's boundary, the piece that holds it, and its bound.
#[derive(Clone, Copy)]
struct RunFin {
    sheet: usize,
    rising: bool,
    piece: usize,
    bound: Plane,
}

/// A stretch of one line with the same fins all along.
struct Run {
    low: usize,
    high: usize,
    fins: Vec<RunFin>,
    /// The line's first planes: a point lies on the run's line when it lies on both.
    on: [Plane; 2],
}

/// The root of `at` in a disjoint-set forest, halving the path.
fn root(parents: &mut [usize], mut at: usize) -> usize {
    loop {
        let up = parents.get(at).copied().unwrap_or(at);
        if up == at {
            return at;
        }
        let top = parents.get(up).copied().unwrap_or(up);
        if let Some(slot) = parents.get_mut(at) {
            *slot = top;
        }
        at = top;
    }
}

/// Joins the sets of `a` and `b`, the lower root on top.
fn join(parents: &mut [usize], a: usize, b: usize) {
    let (a, b) = (root(parents, a), root(parents, b));
    if let Some(slot) = parents.get_mut(a.max(b)) {
        *slot = a.min(b);
    }
}

/// Sweeps the sides of one line: pieces of one sheet on both sides of a stretch join, and the rest
/// are runs.
fn sweep(
    sides: &[Side],
    faces: &[Sheet],
    parents: &mut [usize],
    runs: &mut Vec<Run>,
) -> Result<(), RebuildError> {
    let Some(first) = sides.first() else {
        return Ok(());
    };
    let on = [
        faces.get(first.sheet).ok_or(RebuildError::Open)?.face,
        first.bound,
    ];
    let mut marks: Vec<usize> = sides.iter().flat_map(|s| [s.low, s.high]).collect();
    marks.sort_unstable();
    marks.dedup();
    let mut waiting: Vec<&Side> = sides.iter().collect();
    waiting.sort_by_key(|s| core::cmp::Reverse(s.low));
    let mut active: Vec<&Side> = Vec::new();
    let opening = runs.len();
    for pair in marks.windows(2) {
        let (Some(&low), Some(&high)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        while waiting.last().is_some_and(|s| s.low <= low) {
            active.extend(waiting.pop());
        }
        active.retain(|s| s.high >= high);
        let mut sheets: BTreeMap<usize, (i32, Option<&Side>, Option<&Side>)> = BTreeMap::new();
        for side in &active {
            let slot = sheets.entry(side.sheet).or_insert((0, None, None));
            if side.rising {
                slot.0 = slot.0.saturating_add(1);
                slot.1 = Some(*side);
            } else {
                slot.0 = slot.0.saturating_sub(1);
                slot.2 = Some(*side);
            }
        }
        let mut fins = Vec::new();
        for (sheet, (net, up, down)) in sheets {
            if let (Some(a), Some(b)) = (up, down) {
                join(parents, a.piece, b.piece);
            }
            match (net, up, down) {
                (0, _, _) => {}
                (1, Some(side), _) | (-1, _, Some(side)) => fins.push(RunFin {
                    sheet,
                    rising: net == 1,
                    piece: side.piece,
                    bound: side.bound,
                }),
                _ => return Err(RebuildError::Overlap),
            }
        }
        if fins.is_empty() {
            continue;
        }
        let same = |a: &RunFin, b: &RunFin| a.sheet == b.sheet && a.rising == b.rising;
        match runs.get_mut(opening..).and_then(<[Run]>::last_mut) {
            Some(run)
                if run.high == low
                    && run.fins.len() == fins.len()
                    && run.fins.iter().zip(&fins).all(|(a, b)| same(a, b)) =>
            {
                run.high = high;
            }
            _ => runs.push(Run {
                low,
                high,
                fins,
                on,
            }),
        }
    }
    Ok(())
}

/// A fin of the result.
#[derive(Clone, Copy)]
struct OutFin {
    edge: usize,
    face: usize,
    /// Whether it runs from the edge's low end to its high end.
    rising: bool,
    bound: Plane,
}

/// The vertex a fin leaves.
fn fin_from(f: &OutFin, edges: &[Run]) -> usize {
    edges
        .get(f.edge)
        .map_or(0, |e| if f.rising { e.low } else { e.high })
}

/// The vertex a fin reaches.
fn fin_to(f: &OutFin, edges: &[Run]) -> usize {
    edges
        .get(f.edge)
        .map_or(0, |e| if f.rising { e.high } else { e.low })
}

/// The loops of a face's fins, each a ring of fin numbers, in turn, starting at its lowest fin.
fn rings(
    fins: &[OutFin],
    of_face: &[usize],
    plane: &Plane,
    edges: &[Run],
) -> Result<Vec<Vec<usize>>, RebuildError> {
    let mut leaving: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for at in of_face {
        let fin = fins.get(*at).ok_or(RebuildError::Open)?;
        leaving.entry(fin_from(fin, edges)).or_default().push(*at);
    }
    let mut used: BTreeMap<usize, bool> = BTreeMap::new();
    let mut out = Vec::new();
    for start in of_face {
        if used.contains_key(start) {
            continue;
        }
        let mut ring = Vec::new();
        let mut at = *start;
        loop {
            used.insert(at, true);
            ring.push(at);
            let fin = fins.get(at).ok_or(RebuildError::Open)?;
            let options: Vec<usize> = leaving
                .get(&fin_to(fin, edges))
                .ok_or(RebuildError::Open)?
                .iter()
                .copied()
                .filter(|o| !used.contains_key(o))
                .collect();
            let Some((&first, rest)) = options.split_first() else {
                break;
            };
            let mut best = first;
            // Past a vertex where the boundary touches itself, keep the inside on the left: the
            // first way out clockwise from the way in.
            let back = Ray {
                with: fin.bound,
                reversed: false,
            };
            let ray = |o: usize| -> Result<Ray, RebuildError> {
                Ok(Ray {
                    with: fins.get(o).ok_or(RebuildError::Open)?.bound,
                    reversed: true,
                })
            };
            for option in rest {
                if around(plane, &back, &ray(*option)?, &ray(best)?)? == Ordering::Greater {
                    best = *option;
                }
            }
            at = best;
        }
        out.push(ring);
    }
    Ok(out)
}

/// The fins of `edge`, in radial order about it, with the faces each solid wedge joins.
fn radial(
    edge: &[usize],
    fins: &[OutFin],
    planes: &[Plane],
    parents: &mut [usize],
) -> Result<Vec<usize>, RebuildError> {
    let plane_of = |at: usize| -> Result<(OutFin, Plane), RebuildError> {
        let fin = *fins.get(at).ok_or(RebuildError::Open)?;
        Ok((fin, *planes.get(fin.face).ok_or(RebuildError::Open)?))
    };
    let Some(&first) = edge.first() else {
        return Ok(Vec::new());
    };
    let (start, start_plane) = plane_of(first)?;
    let line = Line::new(start_plane, start.bound)?;
    // A fin runs along −`n` × `n_bound`, so the first fin runs against the line's direction, and a
    // fin that runs its way lies on the far side of `n` × v.
    let flap = |at: usize| -> Result<Flap, RebuildError> {
        let (fin, plane) = plane_of(at)?;
        Ok(Flap {
            plane,
            reversed: fin.rising == start.rising,
        })
    };
    let reference = flap(first)?;
    let mut order = alloc::vec![first];
    for at in edge.iter().skip(1) {
        let this = flap(*at)?;
        let mut place = order.len();
        for (k, held) in order.iter().enumerate() {
            if about(&line, &reference, &this, &flap(*held)?)? == Ordering::Less {
                place = k;
                break;
            }
        }
        order.insert(place, *at);
    }
    // Turning about the line, past a fin that runs against it lies the fin's material.
    let count = order.len();
    for (k, at) in order.iter().enumerate() {
        let next = order
            .get(k.saturating_add(1).checked_rem(count).unwrap_or(0))
            .ok_or(RebuildError::Open)?;
        let (this, other) = (plane_of(*at)?.0, plane_of(*next)?.0);
        if this.rising == start.rising {
            if other.rising == this.rising {
                return Err(RebuildError::Open);
            }
            join(parents, this.face, other.face);
        }
    }
    Ok(order)
}

/// A shell's piece to trace from, and the shells around each shell.
struct Nest {
    /// Whether each shell faces outward.
    outer: Vec<bool>,
    /// For each cavity, its outer shell.
    parent: Vec<Option<usize>>,
}

/// Which shells face out, and which holds each cavity, by the winding of a point in front of each
/// shell, counted over the shells.
fn nest(
    sheets_pieces: &[(usize, Piece)],
    shell_of_piece: &[usize],
    shells: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Nest, RebuildError> {
    let polygons: Vec<Polygon> = sheets_pieces
        .iter()
        .zip(shell_of_piece)
        .map(|((_, piece), shell)| Polygon {
            piece: piece.clone(),
            entry: u32::try_from(*shell).ok(),
        })
        .collect();
    let cell = box_around(&polygons)?;
    let mut fronts = Vec::with_capacity(shells);
    for shell in 0..shells {
        meter.charge(1)?;
        let chosen = shell_of_piece.iter().position(|s| *s == shell);
        let lists: Vec<Vec<Piece>> = polygons
            .iter()
            .enumerate()
            .map(|(at, p)| {
                if Some(at) == chosen {
                    alloc::vec![p.piece.clone()]
                } else {
                    Vec::new()
                }
            })
            .collect();
        let found = classify(&cell, &Winding::new(), &polygons, &lists, meter)?;
        fronts.push(found.into_iter().next().map_or(Winding::new(), |c| c.front));
    }
    let number = |at: usize| u32::try_from(at).unwrap_or(u32::MAX);
    let outer: Vec<bool> = fronts
        .iter()
        .enumerate()
        .map(|(at, front)| front.count(number(at)) == 0)
        .collect();
    let holders = |at: usize| -> Vec<usize> {
        (0..shells)
            .filter(|t| {
                *t != at
                    && outer.get(*t) == Some(&true)
                    && fronts.get(at).is_some_and(|f| f.count(number(*t)) > 0)
            })
            .collect()
    };
    let mut parent = Vec::with_capacity(shells);
    for at in 0..shells {
        if outer.get(at) == Some(&true) {
            parent.push(None);
            continue;
        }
        let around_it = holders(at);
        let innermost = around_it
            .iter()
            .copied()
            .max_by_key(|t| holders(*t).len())
            .ok_or(RebuildError::Inside)?;
        parent.push(Some(innermost));
    }
    Ok(Nest { outer, parent })
}

/// The next row of an arena, as a typed index.
fn row<K>(at: usize) -> Result<Ix<K>, RebuildError> {
    u32::try_from(at)
        .ok()
        .and_then(Ix::new)
        .ok_or(RebuildError::Num(NumError::BudgetExceeded))
}

/// The lowest three of `planes` that meet in one point, or `corner`'s own.
fn lowest_triple(planes: &[usize], pool: &[Plane], own: &Tpi) -> Result<[Plane; 3], NumError> {
    for (i, a) in planes.iter().enumerate() {
        for (j, b) in planes.iter().enumerate().skip(i.saturating_add(1)) {
            for c in planes.iter().skip(j.saturating_add(1)) {
                let triple = [pool.get(*a), pool.get(*b), pool.get(*c)];
                if let [Some(p), Some(q), Some(r)] = triple
                    && normals_det(p, q, r)? != Sign::Zero
                {
                    return Ok([*p, *q, *r]);
                }
            }
        }
    }
    Ok(*own.planes())
}

/// The grid point nearest a corner, and the box a stored bound must take around it.
fn rounded(point: &Tpi) -> Result<(P3, bool), NumError> {
    let near = point.to_grid()?;
    Ok((near.value(), near.is_exact()))
}

/// The grid box around `points`, a step wider where a point is off the grid.
fn bound_of(points: &[(P3, bool)]) -> Result<Box3, NumError> {
    let mut low = [COORD_MAX; 3];
    let mut high = [-COORD_MAX; 3];
    for (p, exact) in points {
        let margin = i64::from(!*exact);
        for ((l, h), v) in low
            .iter_mut()
            .zip(&mut high)
            .zip([p.x.get(), p.y.get(), p.z.get()])
        {
            *l = (*l).min(v.saturating_sub(margin)).max(-COORD_MAX);
            *h = (*h).max(v.saturating_add(margin)).min(COORD_MAX);
        }
    }
    let [ax, ay, az] = low;
    let [bx, by, bz] = high;
    Box3::new(P3::new(ax, ay, az)?, P3::new(bx, by, bz)?)
}

/// The body `pieces` bound, every surface from `carriers`, by polygon index.
///
/// # Errors
///
/// A [`RebuildError`]: pieces that overlap on one sheet or do not close, a shell facing inward with
/// nothing around it, what the arenas refuse, or a stopped meter.
#[allow(
    clippy::too_many_lines,
    reason = "the steps of 11.3's rebuild in their order, each short, sharing the arenas they fill"
)]
pub fn rebuild(
    pieces: &[Outward],
    carriers: &[Carrier],
    merge: Merge,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    // Sheets: a sort by exact keys, so the input's order counts for nothing.
    type Key = (
        (i128, [i128; 3]),
        bool,
        usize,
        Vec<(u32, i32)>,
        Vec<(u32, i32)>,
    );
    let mut keyed: Vec<(Key, usize, &Outward)> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let face = canonical(&piece.piece.face)?;
        let plane = (face.plane.d(), face.plane.n());
        let seams = merge == Merge::KeepSeams;
        let key = (
            plane,
            face.flipped,
            if seams { piece.polygon } else { 0 },
            if seams {
                piece.front.pairs().to_vec()
            } else {
                Vec::new()
            },
            if seams {
                piece.back.pairs().to_vec()
            } else {
                Vec::new()
            },
        );
        keyed.push((key, piece.polygon, piece));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut held: Vec<(usize, Piece)> = Vec::with_capacity(keyed.len());
    let mut last: Option<&Key> = None;
    for (key, polygon, piece) in &keyed {
        if last != Some(key) {
            sheets.push(Sheet {
                face: piece.piece.face,
                polygon: *polygon,
                turned: piece.turned,
            });
            last = Some(key);
        }
        held.push((sheets.len().saturating_sub(1), piece.piece.clone()));
    }

    // Every corner, numbered by its place in the sort.
    let mut corners = Vec::new();
    let mut owners = Vec::new();
    for (at, (_, piece)) in held.iter().enumerate() {
        let after = piece.bounds.iter().cycle().skip(1);
        for (i, (bound, next)) in piece.bounds.iter().zip(after).enumerate() {
            meter.charge(1)?;
            corners.push(corner(piece.face, *bound, *next)?);
            owners.push((at, i));
        }
    }
    let mut order: Vec<usize> = (0..corners.len()).collect();
    let failed = Once::new(false);
    order.sort_by(|a, b| {
        corners
            .get(*a)
            .zip(corners.get(*b))
            .and_then(|(p, q)| point_order(p, q))
            .unwrap_or_else(|| {
                failed.set(true);
                Ordering::Equal
            })
    });
    let mut number = alloc::vec![0usize; corners.len()];
    let mut places: Vec<usize> = Vec::new();
    for at in order {
        let same = places.last().and_then(|held| {
            let (p, q) = (corners.get(*held)?, corners.get(at)?);
            point_order(p, q)
        });
        if same != Some(Ordering::Equal) {
            places.push(at);
        }
        if let Some(slot) = number.get_mut(at) {
            *slot = places.len().saturating_sub(1);
        }
    }
    if failed.get() {
        return Err(NumError::BudgetExceeded.into());
    }

    // Every side of every piece, grouped by line.
    let mut sides: Vec<Side> = Vec::new();
    let mut first_of: Vec<usize> = Vec::with_capacity(held.len());
    let mut count = 0usize;
    for (_, piece) in &held {
        first_of.push(count);
        count = count.saturating_add(piece.bounds.len());
    }
    for (at, (sheet, piece)) in held.iter().enumerate() {
        let base = first_of.get(at).copied().unwrap_or(0);
        let k = piece.bounds.len();
        for (i, bound) in piece.bounds.iter().enumerate() {
            meter.charge(1)?;
            // Bound i runs from corner i − 1 to corner i.
            let before = base.saturating_add(i.checked_sub(1).unwrap_or(k.saturating_sub(1)));
            let from = number.get(before).copied().unwrap_or(0);
            let to = number.get(base.saturating_add(i)).copied().unwrap_or(0);
            sides.push(Side {
                key: line_key(&piece.face, bound)?,
                low: from.min(to),
                high: from.max(to),
                rising: from < to,
                sheet: *sheet,
                piece: at,
                bound: *bound,
            });
        }
    }
    let failed = Once::new(false);
    sides.sort_by(|a, b| {
        line_order(&a.key, &b.key).unwrap_or_else(|| {
            failed.set(true);
            Ordering::Equal
        })
    });
    if failed.get() {
        return Err(NumError::BudgetExceeded.into());
    }
    let mut same_piece: Vec<usize> = (0..held.len()).collect();
    let mut runs: Vec<Run> = Vec::new();
    let mut start = 0usize;
    for end in 1..=sides.len() {
        let breaks = match (sides.get(start), sides.get(end)) {
            (Some(a), Some(b)) => line_order(&a.key, &b.key) != Some(Ordering::Equal),
            _ => true,
        };
        if breaks {
            let group = sides.get(start..end).ok_or(RebuildError::Open)?;
            sweep(group, &sheets, &mut same_piece, &mut runs)?;
            start = end;
        }
    }

    // A run ends wherever another run does, on its line.
    let mut ends: Vec<usize> = runs.iter().flat_map(|r| [r.low, r.high]).collect();
    ends.sort_unstable();
    ends.dedup();
    let mut edges: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        let [p, q] = run.on;
        let mut low = run.low;
        for mark in ends.iter().filter(|e| **e > run.low && **e < run.high) {
            let place = places.get(*mark).and_then(|c| corners.get(*c));
            let place = place.ok_or(RebuildError::Open)?;
            let zero = |plane: &Plane| plane.side_of_tpi(&place.point);
            if zero(&p)? == Sign::Zero && zero(&q)? == Sign::Zero {
                edges.push(Run {
                    low,
                    high: *mark,
                    fins: run.fins.clone(),
                    on: run.on,
                });
                low = *mark;
            }
        }
        edges.push(Run {
            low,
            high: run.high,
            fins: run.fins,
            on: run.on,
        });
    }

    // Faces: the connected pieces of a sheet, numbered by the sheet and their lowest vertex.
    let mut lowest: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for edge in &edges {
        for fin in &edge.fins {
            let top = root(&mut same_piece, fin.piece);
            let slot = lowest.entry(top).or_insert((fin.sheet, edge.low));
            slot.1 = slot.1.min(edge.low);
        }
    }
    let mut face_list: Vec<((usize, usize), usize)> =
        lowest.iter().map(|(top, key)| (*key, *top)).collect();
    face_list.sort_unstable();
    let numbered: BTreeMap<usize, usize> = face_list
        .iter()
        .enumerate()
        .map(|(at, (_, top))| (*top, at))
        .collect();
    let face_of = |piece: usize, forest: &[usize]| numbered.get(&root_of(forest, piece)).copied();
    let mut planes_of: Vec<Plane> = Vec::with_capacity(face_list.len());
    for ((sheet, _), _) in &face_list {
        planes_of.push(sheets.get(*sheet).ok_or(RebuildError::Open)?.face);
    }

    // Fins, and the rings they make.
    let mut fins: Vec<OutFin> = Vec::new();
    let mut around_edge: Vec<Vec<usize>> = Vec::with_capacity(edges.len());
    for (e, edge) in edges.iter().enumerate() {
        let mut here = Vec::with_capacity(edge.fins.len());
        for fin in &edge.fins {
            meter.charge(1)?;
            let face = face_of(fin.piece, &same_piece).ok_or(RebuildError::Open)?;
            here.push(fins.len());
            fins.push(OutFin {
                edge: e,
                face,
                rising: fin.rising,
                bound: fin.bound,
            });
        }
        if here.len() % 2 != 0 {
            return Err(RebuildError::Open);
        }
        around_edge.push(here);
    }
    let mut shell_of: Vec<usize> = (0..face_list.len()).collect();
    let mut radial_rings = Vec::with_capacity(around_edge.len());
    for here in &around_edge {
        radial_rings.push(radial(here, &fins, &planes_of, &mut shell_of)?);
    }

    // Shells by connected faces, numbered by their lowest face.
    let mut tops: Vec<usize> = (0..face_list.len())
        .map(|f| root(&mut shell_of, f))
        .collect();
    tops.sort_unstable();
    tops.dedup();
    let shell_number =
        |face: usize, forest: &[usize]| tops.binary_search(&root_of(forest, face)).ok();
    let mut shell_of_piece = Vec::with_capacity(held.len());
    for at in 0..held.len() {
        let face = face_of(at, &same_piece).ok_or(RebuildError::Open)?;
        shell_of_piece.push(shell_number(face, &shell_of).ok_or(RebuildError::Open)?);
    }
    let nested = if held.is_empty() {
        Nest {
            outer: Vec::new(),
            parent: Vec::new(),
        }
    } else {
        nest(&held, &shell_of_piece, tops.len(), meter)?
    };

    // The arenas.
    let mut builder = Builder::new();
    let a = &mut builder.arenas;
    // Solid regions, one per outer shell, then the outside.
    let region_of_shell: Vec<usize> = {
        let outers: Vec<usize> = (0..tops.len())
            .filter(|s| nested.outer.get(*s) == Some(&true))
            .collect();
        (0..tops.len())
            .map(|s| {
                let holder = nested.parent.get(s).copied().flatten().unwrap_or(s);
                outers.iter().position(|o| *o == holder).unwrap_or(0)
            })
            .collect()
    };
    let solids = nested.outer.iter().filter(|o| **o).count();
    for region in 0..solids {
        let first_shell = region_of_shell.iter().position(|r| *r == region);
        a.regions.push(Region {
            kind: RegionKind::Solid,
            first_shell: first_shell.map(row).transpose()?,
        })?;
    }
    let outside = a.regions.push(Region {
        kind: RegionKind::Void,
        first_shell: None,
    })?;
    for (s, region) in region_of_shell.iter().enumerate() {
        let next =
            (s.saturating_add(1)..tops.len()).find(|t| region_of_shell.get(*t) == Some(region));
        let first_face = (0..face_list.len()).find(|f| shell_number(*f, &shell_of) == Some(s));
        a.shells.push(Shell {
            region: row(*region)?,
            next: next.map(row).transpose()?,
            first_face: first_face.map(row).transpose()?,
        })?;
    }

    // Vertices that edges end at, in order, each a grid point or its lowest meeting.
    let mut used: Vec<usize> = edges.iter().flat_map(|e| [e.low, e.high]).collect();
    used.sort_unstable();
    used.dedup();
    let vertex_of = |id: usize| used.binary_search(&id).ok();
    let mut planes_at: Vec<Vec<usize>> = alloc::vec![Vec::new(); used.len()];
    for fin in &fins {
        let edge = edges.get(fin.edge).ok_or(RebuildError::Open)?;
        for id in [edge.low, edge.high] {
            let at = vertex_of(id).ok_or(RebuildError::Open)?;
            if let Some(list) = planes_at.get_mut(at) {
                list.push(fin.face);
            }
        }
    }
    let mut pool: Vec<([i128; 4], Plane)> = Vec::new();
    let mut triples: Vec<Option<[Plane; 3]>> = Vec::with_capacity(used.len());
    let mut rounded_at = Vec::with_capacity(used.len());
    let coefficients = |plane: &Plane| {
        let [x, y, z] = plane.n();
        [x, y, z, plane.d()]
    };
    for (at, id) in used.iter().enumerate() {
        let place = places.get(*id).and_then(|c| corners.get(*c));
        let place = place.ok_or(RebuildError::Open)?;
        let (near, exact) = rounded(&place.point)?;
        rounded_at.push((near, exact));
        if exact {
            triples.push(None);
            continue;
        }
        let mut around_it = planes_at.get(at).cloned().unwrap_or_default();
        around_it.sort_unstable();
        around_it.dedup();
        let triple = lowest_triple(&around_it, &planes_of, &place.point)?;
        pool.extend(triple.iter().map(|plane| (coefficients(plane), *plane)));
        triples.push(Some(triple));
    }
    pool.sort_unstable_by_key(|(key, _)| *key);
    pool.dedup_by_key(|(key, _)| *key);
    for (_, plane) in &pool {
        a.planes.push(*plane)?;
    }
    let pool_row = |plane: &Plane| {
        pool.binary_search_by_key(&coefficients(plane), |(key, _)| *key)
            .map_err(|_| RebuildError::Open)
            .and_then(row)
    };
    for (triple, (near, _)) in triples.iter().zip(&rounded_at) {
        let point = match triple {
            Some([p, q, r]) => PointKind::Meet([pool_row(p)?, pool_row(q)?, pool_row(r)?]),
            None => PointKind::Grid(*near),
        };
        let point = a.points.push(point)?;
        a.vertices.push(Vertex { point })?;
    }

    // Faces, loops and fins, ring by ring.
    let mut surfaces: BTreeMap<usize, usize> = BTreeMap::new();
    let mut ring_rows: Vec<Vec<usize>> = Vec::new();
    let mut fin_loop = alloc::vec![0usize; fins.len()];
    let mut fin_rows = alloc::vec![0usize; fins.len()];
    let mut next_fin_row = 0usize;
    let mut of_each_face: Vec<Vec<usize>> = alloc::vec![Vec::new(); face_list.len()];
    for (at, fin) in fins.iter().enumerate() {
        if let Some(list) = of_each_face.get_mut(fin.face) {
            list.push(at);
        }
    }
    for (f, ((sheet, _), _)) in face_list.iter().enumerate() {
        let sheet = sheets.get(*sheet).ok_or(RebuildError::Open)?;
        let carrier = carriers.get(sheet.polygon).ok_or(RebuildError::Open)?;
        let surface = if let Some(held) = surfaces.get(&sheet.polygon) {
            *held
        } else {
            let new = usize::try_from(a.surfaces.len()).unwrap_or(usize::MAX);
            a.surfaces.push(carrier.surface.clone())?;
            surfaces.insert(sheet.polygon, new);
            new
        };
        let own_fins = of_each_face.get(f).ok_or(RebuildError::Open)?;
        let plane = planes_of.get(f).ok_or(RebuildError::Open)?;
        let loops = rings(&fins, own_fins, plane, &edges)?;
        let first_loop = ring_rows.len();
        let mut boxes = Vec::new();
        for (l, ring) in loops.iter().enumerate() {
            let at_row = first_loop.saturating_add(l);
            for (k, fin) in ring.iter().enumerate() {
                if let Some(slot) = fin_loop.get_mut(*fin) {
                    *slot = at_row;
                }
                if let Some(slot) = fin_rows.get_mut(*fin) {
                    *slot = next_fin_row.saturating_add(k);
                }
                let of_fin = fins.get(*fin).ok_or(RebuildError::Open)?;
                let id = fin_from(of_fin, &edges);
                let v = vertex_of(id).ok_or(RebuildError::Open)?;
                boxes.push(*rounded_at.get(v).ok_or(RebuildError::Open)?);
            }
            next_fin_row = next_fin_row.saturating_add(ring.len());
        }
        let turned = sheet.turned != (carrier.sense == Sense::Reversed);
        let later = (f.saturating_add(1)..face_list.len())
            .find(|g| shell_number(*g, &shell_of) == shell_number(f, &shell_of));
        a.faces.push(Face {
            shell: row(shell_number(f, &shell_of).ok_or(RebuildError::Open)?)?,
            next: later.map(row).transpose()?,
            surface: row(surface)?,
            sense: if turned {
                Sense::Reversed
            } else {
                Sense::Forward
            },
            first_loop: Some(row(first_loop)?),
            bound: bound_of(&boxes)?,
        })?;
        for (l, ring) in loops.iter().enumerate() {
            let at_row = first_loop.saturating_add(l);
            a.loops.push(Loop {
                face: row(f)?,
                next: (l.saturating_add(1) < loops.len())
                    .then(|| row(at_row.saturating_add(1)))
                    .transpose()?,
                first_fin: ring
                    .first()
                    .and_then(|at| fin_rows.get(*at))
                    .copied()
                    .map(row)
                    .transpose()?,
            })?;
        }
        ring_rows.extend(loops);
    }

    // Fins in loop order, each with its neighbours in its ring and around its edge.
    let mut radial_next = alloc::vec![0usize; fins.len()];
    for order in &radial_rings {
        for (k, at) in order.iter().enumerate() {
            let next = order
                .get(k.saturating_add(1).checked_rem(order.len()).unwrap_or(0))
                .copied()
                .ok_or(RebuildError::Open)?;
            if let Some(slot) = radial_next.get_mut(*at) {
                *slot = next;
            }
        }
    }
    for (l, ring) in ring_rows.iter().enumerate() {
        let k = ring.len();
        for (i, at) in ring.iter().enumerate() {
            let fin = fins.get(*at).ok_or(RebuildError::Open)?;
            let neighbour = |by: usize| -> Result<usize, RebuildError> {
                let other = ring
                    .get(i.saturating_add(by).checked_rem(k).unwrap_or(0))
                    .ok_or(RebuildError::Open)?;
                fin_rows.get(*other).copied().ok_or(RebuildError::Open)
            };
            let radial = radial_next.get(*at).and_then(|r| fin_rows.get(*r));
            a.fins.push(Fin {
                loop_: row(l)?,
                next: row(neighbour(1)?)?,
                prev: row(neighbour(k.saturating_sub(1))?)?,
                edge: row(fin.edge)?,
                sense: if fin.rising {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
                radial: row(radial.copied().ok_or(RebuildError::Open)?)?,
                pcurve: None,
            })?;
        }
    }
    // Edges, with shadows between their rounded ends.
    for (e, edge) in edges.iter().enumerate() {
        let (from, to) = (
            vertex_of(edge.low).ok_or(RebuildError::Open)?,
            vertex_of(edge.high).ok_or(RebuildError::Open)?,
        );
        let end = |at: usize| {
            rounded_at
                .get(at)
                .map(|(p, _)| *p)
                .ok_or(RebuildError::Open)
        };
        let (p, q) = (end(from)?, end(to)?);
        let f = |v: geom_num::Coord| {
            #[allow(
                clippy::cast_precision_loss,
                reason = "a coordinate is below 2^40, which a double holds"
            )]
            let x = v.get() as f64;
            x
        };
        let origin = [f(p.x), f(p.y), f(p.z)];
        let direction = [f(q.x) - f(p.x), f(q.y) - f(p.y), f(q.z) - f(p.z)];
        let shadow = if direction == [0.0; 3] {
            Shadow::new(origin, [1.0, 0.0, 0.0], (0.0, 0.5))
        } else {
            Shadow::new(origin, direction, (0.0, 1.0))
        };
        let curve = a
            .curves
            .push(CurveKind::Line(shadow.map_err(|_| NumError::Degenerate)?))?;
        a.edges.push(Edge {
            first_fin: around_edge
                .get(e)
                .and_then(|h| h.first())
                .and_then(|f| fin_rows.get(*f))
                .copied()
                .map(row)
                .transpose()?,
            start: Some(row(from)?),
            end: Some(row(to)?),
            curve,
        })?;
    }

    let kind = if around_edge.iter().all(|h| h.len() == 2) {
        BodyKind::Solid
    } else {
        BodyKind::General
    };
    Ok(builder.publish(kind, outside, meter)?)
}

/// The root of `at` without halving, for a forest behind a shared borrow.
fn root_of(parents: &[usize], mut at: usize) -> usize {
    loop {
        let up = parents.get(at).copied().unwrap_or(at);
        if up == at {
            return at;
        }
        at = up;
    }
}
