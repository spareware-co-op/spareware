//! The maximal-face merge: the pieces a selection keeps, as faces, edges, rings and shells (11.3,
//! step 5), shared by `geom-bool` and `geom-mesh` (19.5, 27).
//!
//! What it does: [`select`] reads a test over classified pieces and turns each kept piece to face
//! out of the result. [`merge`] merges those pieces into maximal faces, finds the edges between the
//! faces as maximal runs on one line, makes rings, orders the fins around each edge, joins faces
//! into shells, and says which shells are cavities of which.
//!
//! How it runs: every corner of every piece is sorted by its exact coordinates, so one point has
//! one number and numbers rise along any line that runs up the sort. Every side of every piece is
//! sorted by its line's Plücker coordinates, so one line is one group. Along a line, pieces of one
//! sheet on both sides of a stretch cancel, and what is left are the stretches of boundary: runs
//! with the same fins merge, so the cuts of the subdivision and of the convex split disappear.
//!
//! Guarantees:
//!
//! - **The same numbers however the boxes fell.** Faces, edges and vertices are numbered by exact
//!   keys that depend on the shape alone: not on the boxes, the leaf size, the threads, or the order
//!   the pieces came in (11.3; tested).
//! - **Exact.** Points are compared by cross-multiplying their homogeneous coordinates, lines by
//!   their Plücker coordinates, in at most 576 bits, and nothing is rounded.
//! - **Refused, never guessed.** Pieces that overlap on one sheet, and a stretch with a fin that has
//!   no partner across it, mean the pieces are no closed boundary: [`MergeError`].
//! - **Each fin knows its piece.** [`Fin::piece`] is an index into the pieces given, whose
//!   [`Outward::polygon`] names where it came from, for a caller's history.
//! - **Told where to keep a seam.** [`merge_with`] takes pieces it does not keep, whose sides hold a
//!   seam open where they lie along a face: an imprint's outline, and the walls of a split. A cut
//!   that ends inside a face leaves it whole, with no slit.
//! - **No recursion.** Disjoint sets and rings are walked in loops (principle 13).
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
use core::cmp::{Ordering, Reverse};
use core::fmt;

use geom_num::plane::{Plane, Tpi};
use geom_num::{I256, I320, NumError, Sign, Wide};
use geom_run::{Stopped, WorkMeter};

use crate::convex::{Piece, opposite};
use crate::order::{Flap, Line, Ray, about, around};
use crate::plane::canonical;
use crate::subdivide::around as box_around;
use crate::trace::{Classified, Polygon, TraceError, classify};
use crate::winding::{Kept, Winding, WindingTest, kept};

/// What merges (11.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    /// Everything on one plane facing one way, whatever it came from.
    RemoveRedundant,
    /// Only pieces of one face of one operand with the same vectors on each side.
    KeepSeams,
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
    /// The input face it came from, which [`Merge::KeepSeams`] keeps apart. [`select`] gives each
    /// polygon its own; a caller whose polygons are parts of faces says which.
    pub source: usize,
    /// The vector in front of it, out of the result.
    pub front: Winding,
    /// The vector behind it, in the result.
    pub back: Winding,
}

/// What a merge is told beyond its pieces.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rules<'p> {
    /// Pieces not kept, whose sides hold a seam open wherever one lies along a stretch of a face.
    pub cuts: &'p [Piece],
    /// Whether the pieces are a sheet, whose edges may have one fin and whose shells bound nothing.
    pub open: bool,
}

/// Why pieces could not be merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeError {
    /// The numbers layer refused.
    Num(NumError),
    /// A trace failed.
    Trace(TraceError),
    /// The meter stopped the run.
    Stopped(Stopped),
    /// Two pieces of one sheet overlap.
    Overlap,
    /// A stretch of boundary has no partner across it: the pieces do not close.
    Open,
    /// A shell faces inward with no shell around it.
    Inside,
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::Num(e) => write!(f, "pieces that cannot be merged: {e:?}"),
            MergeError::Trace(e) => write!(f, "{e}"),
            MergeError::Stopped(_) => f.write_str("the meter stopped the merge"),
            MergeError::Overlap => f.write_str("two pieces of one face overlap"),
            MergeError::Open => f.write_str("kept pieces do not close"),
            MergeError::Inside => f.write_str("a shell faces inward with no shell around it"),
        }
    }
}

impl core::error::Error for MergeError {}

impl From<NumError> for MergeError {
    fn from(e: NumError) -> Self {
        MergeError::Num(e)
    }
}

impl From<TraceError> for MergeError {
    fn from(e: TraceError) -> Self {
        MergeError::Trace(e)
    }
}

impl From<Stopped> for MergeError {
    fn from(e: Stopped) -> Self {
        MergeError::Stopped(e)
    }
}

/// `c`, as it faces.
#[must_use]
pub fn as_kept(c: &Classified) -> Outward {
    Outward {
        piece: c.piece.clone(),
        polygon: c.polygon,
        turned: false,
        source: c.polygon,
        front: c.front.clone(),
        back: c.back.clone(),
    }
}

/// `c`, facing the other way.
fn as_turned(c: &Classified) -> Result<Outward, NumError> {
    let mut bounds = c.piece.bounds.clone();
    bounds.reverse();
    Ok(Outward {
        piece: Piece {
            face: opposite(&c.piece.face)?,
            bounds,
        },
        polygon: c.polygon,
        turned: true,
        source: c.polygon,
        front: c.back.clone(),
        back: c.front.clone(),
    })
}

/// `c` facing each way, back to back: a wall between two cells.
///
/// # Errors
///
/// What turning a plane gives, which a plane within budget cannot reach.
pub fn both_ways(c: &Classified) -> Result<[Outward; 2], NumError> {
    Ok([as_kept(c), as_turned(c)?])
}

/// The pieces `test` keeps of `classified`, each facing out of the result, an overlap once.
///
/// # Errors
///
/// What turning a plane gives, which a plane within budget cannot reach.
pub fn select(test: &WindingTest, classified: &[Classified]) -> Result<Vec<Outward>, NumError> {
    let mut out = Vec::new();
    for c in classified.iter().filter(|c| c.first) {
        match kept(test, &c.front, &c.back) {
            Kept::No => {}
            Kept::AsIs => out.push(as_kept(c)),
            Kept::Flipped => out.push(as_turned(c)?),
        }
    }
    Ok(out)
}

/// Pieces that merge: one oriented plane, and under [`Merge::KeepSeams`] one source and one pair of
/// vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sheet {
    /// The plane, facing out.
    pub face: Plane,
    /// The lowest polygon among its pieces.
    pub polygon: usize,
    /// Whether that polygon faces the other way.
    pub turned: bool,
}

/// An edge: a maximal run on one line between the same faces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Its lower vertex.
    pub low: usize,
    /// Its higher vertex.
    pub high: usize,
    /// Its fins, in radial order about the line.
    pub fins: Vec<usize>,
}

/// A side of a face along an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fin {
    /// The edge.
    pub edge: usize,
    /// The face.
    pub face: usize,
    /// The vertex it runs from.
    pub from: usize,
    /// The vertex it runs to.
    pub to: usize,
    /// A plane through the edge, positive on the face's side of it.
    pub bound: Plane,
    /// The piece that holds it, an index into the pieces given to [`merge`].
    pub piece: usize,
}

/// A maximal face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// Its sheet.
    pub sheet: usize,
    /// Its shell.
    pub shell: usize,
    /// Its rings of fins, each in turn with the face on its left.
    pub rings: Vec<Vec<usize>>,
}

/// A connected set of faces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    /// Whether it faces outward; if not it is a cavity.
    pub outer: bool,
    /// For a cavity, the outer shell around it.
    pub holder: Option<usize>,
    /// Its faces.
    pub faces: Vec<usize>,
}

/// Merged pieces, every number in the shape's own order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    /// The sheets.
    pub sheets: Vec<Sheet>,
    /// The points, lexicographically.
    pub vertices: Vec<Tpi>,
    /// The edges.
    pub edges: Vec<Edge>,
    /// The fins.
    pub fins: Vec<Fin>,
    /// The faces.
    pub faces: Vec<Face>,
    /// The shells.
    pub shells: Vec<Shell>,
    /// Each piece given, its face.
    pub piece_faces: Vec<usize>,
}

/// A piece on a sheet.
struct Held<'p> {
    sheet: usize,
    piece: &'p Piece,
    /// Its index in the pieces given.
    given: usize,
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
    Some(left.checked_sub(&right)?.signum().as_i8().cmp(&0))
}

/// Points lexicographically: at most 2^294 · 2^252, in 576 bits.
fn point_order(a: &Corner, b: &Corner) -> Option<Ordering> {
    let mut order = Ordering::Equal;
    for (p, q) in a.x.iter().zip(&b.x) {
        order = order.then(ratio_order::<5, 4, 9>(p, &a.w, q, &b.w)?);
    }
    Some(order)
}

/// A corner of `face` where `p` and `q` meet.
#[allow(
    clippy::many_single_char_names,
    reason = "p and q are planes, and x and w the point, as in Tpi::homogeneous"
)]
fn corner(face: Plane, p: Plane, q: Plane) -> Result<Corner, NumError> {
    let point = Tpi::new(face, p, q)?;
    let (x, w) = point.homogeneous()?;
    let turn = |v: &I320| v.checked_neg().ok_or(NumError::BudgetExceeded);
    let [a, b, c] = &x;
    Ok(match point.weight() {
        Sign::Negative => Corner {
            point,
            x: [turn(a)?, turn(b)?, turn(c)?],
            w: w.checked_neg().ok_or(NumError::BudgetExceeded)?,
        },
        Sign::Positive | Sign::Zero => Corner { point, x, w },
    })
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
    let minor = |a: i128, b: i128, c: i128, d: i128| {
        product(a, b)
            .zip(product(c, d))
            .and_then(|(l, r)| l.checked_sub(&r))
            .ok_or(NumError::BudgetExceeded)
    };
    let ([px, py, pz], pd) = (p.n(), p.d());
    let ([qx, qy, qz], qd) = (q.n(), q.d());
    let mut coordinates = [
        minor(py, qz, pz, qy)?,
        minor(pz, qx, px, qz)?,
        minor(px, qy, py, qx)?,
        minor(pd, qx, qd, px)?,
        minor(pd, qy, qd, py)?,
        minor(pd, qz, qd, pz)?,
    ];
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
    let (over_a, over_b) = (a.coordinates.get(a.first)?, b.coordinates.get(b.first)?);
    let mut order = a.first.cmp(&b.first);
    if order == Ordering::Equal {
        for (p, q) in a.coordinates.iter().zip(&b.coordinates) {
            order = order.then(ratio_order::<4, 4, 8>(p, over_a, q, over_b)?);
        }
    }
    Some(order)
}

/// `order`, with a failure written down: a sort's comparison cannot return one.
fn noted(order: Option<Ordering>, failed: &mut bool) -> Ordering {
    *failed = (*failed).max(order.is_none());
    order.unwrap_or(Ordering::Equal)
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
    /// The piece, an index into the held pieces.
    held: usize,
    /// The bound the side lies on, positive inside the piece.
    bound: Plane,
}

/// One fin of a run: a sheet's boundary, the piece that holds it, and its bound.
#[derive(Clone, Copy)]
struct RunFin {
    sheet: usize,
    rising: bool,
    held: usize,
    bound: Plane,
}

/// A stretch of one line with the same fins all along.
#[derive(Clone)]
struct Run {
    /// Which line it is on: the place of the line's first side.
    line: usize,
    low: usize,
    high: usize,
    fins: Vec<RunFin>,
    /// Two planes through the line.
    on: [Plane; 2],
}

/// The root of `at` in a disjoint-set forest.
fn root(parents: &[usize], mut at: usize) -> usize {
    loop {
        let up = parents.get(at).copied().unwrap_or(at);
        if up == at {
            return at;
        }
        at = up;
    }
}

/// Joins the sets of `a` and `b`, the lower root on top.
fn join(parents: &mut [usize], a: usize, b: usize) {
    let (a, b) = (root(parents, a), root(parents, b));
    let (low, high) = (a.min(b), a.max(b));
    parents
        .iter_mut()
        .skip(high)
        .take(1)
        .for_each(|slot| *slot = low);
}

/// The sheet of a side no face owns: a cut's, which only holds seams open.
const CUT: usize = usize::MAX;

/// What one stretch of a line holds of one sheet: the net of its sides, and a side each way.
type Holding<'s> = (i32, Option<&'s Side>, Option<&'s Side>);

/// The stretches of one line between the ends of `sides`, each with the sides along all of it.
fn stretches(sides: &[Side]) -> Vec<(usize, usize, Vec<&Side>)> {
    let mut marks: Vec<usize> = sides.iter().flat_map(|s| [s.low, s.high]).collect();
    marks.sort_unstable();
    marks.dedup();
    let mut waiting: Vec<&Side> = sides.iter().collect();
    waiting.sort_by_key(|s| Reverse(s.low));
    let mut active: Vec<&Side> = Vec::new();
    let mut out = Vec::with_capacity(marks.len());
    for (low, high) in marks.iter().copied().zip(marks.iter().copied().skip(1)) {
        while waiting.last().is_some_and(|s| s.low <= low) {
            active.extend(waiting.pop());
        }
        active.retain(|s| s.high >= high);
        out.push((low, high, active.clone()));
    }
    out
}

/// Sweeps the sides of one line: pieces of one sheet on both sides of a stretch join unless a cut
/// lies along it, and the rest are runs.
fn sweep(
    line: usize,
    sides: &[Side],
    sheets: &[Sheet],
    parents: &mut [usize],
    runs: &mut Vec<Run>,
) -> Result<(), MergeError> {
    // A line that only cuts lie on crosses no face.
    let Some(first) = sides.iter().find(|side| side.sheet != CUT) else {
        return Ok(());
    };
    let on = [
        sheets.get(first.sheet).ok_or(MergeError::Open)?.face,
        first.bound,
    ];
    for (low, high, active) in stretches(sides) {
        let cut = active.iter().any(|side| side.sheet == CUT);
        let mut held: BTreeMap<usize, Holding<'_>> = BTreeMap::new();
        for side in active.iter().filter(|side| side.sheet != CUT) {
            let slot = held.entry(side.sheet).or_insert((0, None, None));
            if side.rising {
                *slot = (slot.0.saturating_add(1), Some(*side), slot.2);
            } else {
                *slot = (slot.0.saturating_sub(1), slot.1, Some(*side));
            }
        }
        let mut fins = Vec::new();
        for (sheet, (net, up, down)) in held {
            let fin = |side: &Side, rising: bool| RunFin {
                sheet,
                rising,
                held: side.held,
                bound: side.bound,
            };
            match (net, up, down) {
                (0, Some(a), Some(b)) if cut => {
                    fins.push(fin(a, true));
                    fins.push(fin(b, false));
                }
                (0, Some(a), Some(b)) => join(parents, a.held, b.held),
                (1, Some(side), None) => fins.push(fin(side, true)),
                (-1, None, Some(side)) => fins.push(fin(side, false)),
                _ => return Err(MergeError::Overlap),
            }
        }
        if !fins.is_empty() {
            runs.push(Run {
                line,
                low,
                high,
                fins,
                on,
            });
        }
    }
    Ok(())
}

/// `runs` with stretches joined where the same faces run on along one line. Faces are known only
/// once every line is swept: two of one sheet can lie in a line and touch at a vertex.
fn whole_runs(runs: Vec<Run>, parents: &[usize]) -> Vec<Run> {
    let key = |fin: &RunFin| (fin.sheet, fin.rising, root(parents, fin.held));
    let mut whole: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        match whole.last_mut() {
            Some(last)
                if last.line == run.line
                    && last.high == run.low
                    && last.fins.iter().map(key).eq(run.fins.iter().map(key)) =>
            {
                last.high = run.high;
            }
            _ => whole.push(run),
        }
    }
    whole
}

/// The sheets of `pieces`, sorted by exact keys so the input's order counts for nothing, and each
/// piece on its sheet.
fn sheets_of(pieces: &[Outward], merge: Merge) -> Result<(Vec<Sheet>, Vec<Held<'_>>), NumError> {
    type Key = (
        (i128, [i128; 3]),
        bool,
        usize,
        Vec<(u32, i32)>,
        Vec<(u32, i32)>,
    );
    let seams = merge == Merge::KeepSeams;
    let mut keyed: Vec<(Key, usize)> = Vec::with_capacity(pieces.len());
    for (at, piece) in pieces.iter().enumerate() {
        let face = canonical(&piece.piece.face)?;
        let pairs = |w: &Winding| {
            if seams {
                w.pairs().to_vec()
            } else {
                Vec::new()
            }
        };
        let key = (
            (face.plane.d(), face.plane.n()),
            face.flipped,
            if seams { piece.source } else { 0 },
            pairs(&piece.front),
            pairs(&piece.back),
        );
        keyed.push((key, at));
    }
    // The sheet's source faces as the result does where one can, the lowest first (11.5).
    let source = |at: usize| pieces.get(at).map(|p| (p.turned, p.polygon));
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(source(a.1).cmp(&source(b.1))));
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut held = Vec::with_capacity(keyed.len());
    let mut last: Option<&Key> = None;
    for (key, at) in &keyed {
        let outward = pieces.get(*at).ok_or(NumError::Degenerate)?;
        if last != Some(key) {
            sheets.push(Sheet {
                face: outward.piece.face,
                polygon: outward.polygon,
                turned: outward.turned,
            });
            last = Some(key);
        }
        held.push(Held {
            sheet: sheets.len().saturating_sub(1),
            piece: &outward.piece,
            given: *at,
        });
    }
    Ok((sheets, held))
}

/// Every corner of `held`, with each corner's number, in piece order, and the list of the distinct
/// points in order.
fn numbered(
    held: &[Held<'_>],
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<usize>, Vec<Corner>), MergeError> {
    let mut corners = Vec::new();
    for piece in held {
        let after = piece.piece.bounds.iter().cycle().skip(1);
        for (bound, next) in piece.piece.bounds.iter().zip(after) {
            meter.charge(1)?;
            corners.push(Some(corner(piece.piece.face, *bound, *next)?));
        }
    }
    let mut order: Vec<usize> = (0..corners.len()).collect();
    let mut failed = false;
    let at = |i: &usize| corners.get(*i).and_then(Option::as_ref);
    order.sort_by(|a, b| {
        let found = at(a).zip(at(b)).and_then(|(p, q)| point_order(p, q));
        noted(found, &mut failed)
    });
    (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
    let mut number = alloc::vec![0usize; corners.len()];
    let mut places: Vec<Corner> = Vec::new();
    for i in order {
        let held = corners.get_mut(i).and_then(Option::take);
        let held = held.ok_or(MergeError::Open)?;
        let same = places.last().and_then(|last| point_order(last, &held));
        if same != Some(Ordering::Equal) {
            places.push(held);
        }
        number
            .iter_mut()
            .skip(i)
            .take(1)
            .for_each(|slot| *slot = places.len().saturating_sub(1));
    }
    Ok((number, places))
}

/// Every side of `held`, sorted by line.
fn sides_of(
    held: &[Held<'_>],
    number: &[usize],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Side>, MergeError> {
    let mut sides: Vec<Side> = Vec::new();
    let mut base = 0usize;
    for (at, entry) in held.iter().enumerate() {
        let k = entry.piece.bounds.len();
        for (i, bound) in entry.piece.bounds.iter().enumerate() {
            meter.charge(1)?;
            // Bound i runs from corner i − 1 to corner i.
            let before = base.saturating_add(i.checked_sub(1).unwrap_or(k.saturating_sub(1)));
            let from = number.get(before).copied().unwrap_or(0);
            let to = number.get(base.saturating_add(i)).copied().unwrap_or(0);
            sides.push(Side {
                key: line_key(&entry.piece.face, bound)?,
                low: from.min(to),
                high: from.max(to),
                rising: from < to,
                sheet: entry.sheet,
                held: at,
                bound: *bound,
            });
        }
        base = base.saturating_add(k);
    }
    let mut failed = false;
    sides.sort_by(|a, b| noted(line_order(&a.key, &b.key), &mut failed));
    (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
    Ok(sides)
}

/// `runs` without seams that have one face on both sides: a cut that ends inside a face leaves it
/// whole. Only a seam can hold two fins of one sheet, so those two are the seam.
fn without_slits(mut runs: Vec<Run>, parents: &[usize]) -> Vec<Run> {
    for run in &mut runs {
        let fins = run.fins.clone();
        run.fins.retain(|fin| {
            !fins.iter().any(|other| {
                other.sheet == fin.sheet
                    && other.rising != fin.rising
                    && root(parents, other.held) == root(parents, fin.held)
            })
        });
    }
    runs.retain(|run| !run.fins.is_empty());
    runs
}

/// `runs`, each cut wherever another run ends on its line.
fn cut_at_ends(runs: Vec<Run>, places: &[Corner]) -> Result<Vec<Run>, MergeError> {
    let mut ends: Vec<usize> = runs.iter().flat_map(|r| [r.low, r.high]).collect();
    ends.sort_unstable();
    ends.dedup();
    let mut edges: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        let [p, q] = run.on;
        let mut low = run.low;
        for mark in ends.iter().filter(|e| **e > run.low && **e < run.high) {
            let place = places.get(*mark).ok_or(MergeError::Open)?;
            let side = |plane: &Plane| plane.side_of_tpi(&place.point);
            if side(&p)? == Sign::Zero && side(&q)? == Sign::Zero {
                edges.push(Run {
                    low,
                    high: *mark,
                    ..run.clone()
                });
                low = *mark;
            }
        }
        edges.push(Run { low, ..run });
    }
    Ok(edges)
}

/// The face's rings, each a ring of fin numbers, in turn, starting at its lowest fin.
fn rings(fins: &[Fin], of_face: &[usize], plane: &Plane) -> Result<Vec<Vec<usize>>, MergeError> {
    let mut leaving: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for at in of_face {
        let fin = fins.get(*at).ok_or(MergeError::Open)?;
        leaving.entry(fin.from).or_default().push(*at);
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
            let fin = fins.get(at).ok_or(MergeError::Open)?;
            let options: Vec<usize> = leaving
                .get(&fin.to)
                .ok_or(MergeError::Open)?
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
            let ray = |o: usize| -> Result<Ray, MergeError> {
                Ok(Ray {
                    with: fins.get(o).ok_or(MergeError::Open)?.bound,
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

/// The fins of an edge in radial order, the faces each solid wedge joins put in one set; on a sheet
/// every face at an edge is in one set.
fn radial(
    edge: &[usize],
    fins: &[Fin],
    planes: &[Plane],
    open: bool,
    parents: &mut [usize],
) -> Result<Vec<usize>, MergeError> {
    let plane_of = |at: usize| -> Result<(Fin, Plane), MergeError> {
        let fin = *fins.get(at).ok_or(MergeError::Open)?;
        Ok((fin, *planes.get(fin.face).ok_or(MergeError::Open)?))
    };
    let first = *edge.first().ok_or(MergeError::Open)?;
    let (start, start_plane) = plane_of(first)?;
    let line = Line::new(start_plane, start.bound)?;
    let rising = |fin: &Fin| fin.from < fin.to;
    // A fin runs along −`n` × `n_bound`, so the first fin runs against the line's direction, and a
    // fin that runs its way lies on the far side of `n` × v.
    let flap = |at: usize| -> Result<Flap, MergeError> {
        let (fin, plane) = plane_of(at)?;
        Ok(Flap {
            plane,
            reversed: rising(&fin) == rising(&start),
        })
    };
    let reference = flap(first)?;
    let turned =
        |at: usize| -> Result<bool, MergeError> { Ok(rising(&plane_of(at)?.0) != rising(&start)) };
    // Two faces back to back lie at one angle. Past a fin that runs against the line lies the
    // fin's material, so the other comes first, and the first fin's partner comes last of all.
    let last = |at: usize| -> Result<bool, MergeError> {
        let there = about(&line, &reference, &reference, &flap(at)?)?;
        Ok(turned(at)? && there == Ordering::Equal)
    };
    let mut order = alloc::vec![first];
    for at in edge.iter().skip(1) {
        let this = flap(*at)?;
        let mut place = order.len();
        for (k, held) in order
            .iter()
            .enumerate()
            .filter(|_| matches!(last(*at), Ok(false)))
        {
            let by = about(&line, &reference, &this, &flap(*held)?)?;
            if by == Ordering::Less || (by == Ordering::Equal && turned(*at)?) || last(*held)? {
                place = k;
                break;
            }
        }
        order.insert(place, *at);
    }
    // Turning about the line, past a fin that runs against it lies the fin's material.
    let next = order.iter().cycle().skip(1);
    for (at, after) in order.iter().zip(next) {
        let (this, other) = (plane_of(*at)?.0, plane_of(*after)?.0);
        if open {
            join(parents, this.face, other.face);
        } else if rising(&this) == rising(&start) {
            if rising(&other) == rising(&this) {
                return Err(MergeError::Open);
            }
            join(parents, this.face, other.face);
        }
    }
    Ok(order)
}

/// Which shells face out, by the winding, over the shells, of a point in front of each, and the
/// outer shell around each cavity, by that of a point behind it: behind a cavity lies the material
/// it is a cavity of, even where another cell fills it back to back.
fn nest(
    held: &[Held<'_>],
    shell_of_piece: &[usize],
    shells: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(bool, Option<usize>)>, MergeError> {
    let polygons: Vec<Polygon> = held
        .iter()
        .zip(shell_of_piece)
        .map(|(entry, shell)| Polygon {
            piece: entry.piece.clone(),
            entry: u32::try_from(*shell).ok(),
        })
        .collect();
    let cell = box_around(&polygons)?;
    let mut fronts = Vec::with_capacity(shells);
    let mut backs = Vec::with_capacity(shells);
    for shell in 0..shells {
        meter.charge(1)?;
        let chosen = shell_of_piece.iter().position(|s| *s == shell);
        let lists: Vec<Vec<Piece>> = polygons
            .iter()
            .enumerate()
            .map(|(at, p)| {
                (Some(at) == chosen)
                    .then(|| p.piece.clone())
                    .into_iter()
                    .collect::<Vec<Piece>>()
            })
            .collect();
        let found = classify(&cell, &Winding::new(), &polygons, &lists, meter)?;
        let (front, back) = found
            .into_iter()
            .next()
            .map_or_else(|| (Winding::new(), Winding::new()), |c| (c.front, c.back));
        fronts.push(front);
        backs.push(back);
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
                    && backs.get(at).is_some_and(|b| b.count(number(*t)) > 0)
            })
            .collect()
    };
    let mut out = Vec::with_capacity(shells);
    for (at, is_outer) in outer.iter().enumerate() {
        out.push((
            *is_outer,
            if *is_outer {
                None
            } else {
                Some(
                    holders(at)
                        .iter()
                        .copied()
                        .max_by_key(|t| holders(*t).len())
                        .ok_or(MergeError::Inside)?,
                )
            },
        ));
    }
    Ok(out)
}

/// Curves where pieces of different owners meet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curves {
    /// The points, lexicographically.
    pub vertices: Vec<Tpi>,
    /// The straight edges, each from its lower vertex to its higher.
    pub edges: Vec<(usize, usize)>,
}

/// The stretches of one line where pieces of two owners on different planes both have a side.
fn meetings(
    line: usize,
    sides: &[Side],
    held: &[Held<'_>],
    runs: &mut Vec<Run>,
) -> Result<(), MergeError> {
    let opening = runs.len();
    let plane_of = |side: &Side| -> Result<Plane, MergeError> {
        Ok(held.get(side.held).ok_or(MergeError::Open)?.piece.face)
    };
    for (low, high, active) in stretches(sides) {
        let mut meeting = None;
        for (at, one) in active.iter().enumerate() {
            let p = canonical(&plane_of(one)?)?.plane;
            for other in active.iter().skip(at.saturating_add(1)) {
                if other.sheet != one.sheet && canonical(&plane_of(other)?)?.plane != p {
                    meeting = meeting.or(Some([p, one.bound]));
                }
            }
        }
        let Some(on) = meeting else {
            continue;
        };
        match runs.get_mut(opening..).and_then(<[Run]>::last_mut) {
            Some(run) if run.high == low => run.high = high,
            _ => runs.push(Run {
                line,
                low,
                high,
                fins: Vec::new(),
                on,
            }),
        }
    }
    Ok(())
}

/// The curves where the pieces of different owners meet: `pieces` holds each piece with its owner.
/// A side of a piece on a line that another polygon's plane cut it along lies in that polygon too
/// only where the two meet, so a stretch both have a side on is a stretch of the curve.
///
/// # Errors
///
/// A [`MergeError`]: a stopped meter, or what the numbers layer refuses.
pub fn section(pieces: &[(Piece, usize)], meter: &mut WorkMeter<'_>) -> Result<Curves, MergeError> {
    let held: Vec<Held<'_>> = pieces
        .iter()
        .enumerate()
        .map(|(given, (piece, owner))| Held {
            sheet: *owner,
            piece,
            given,
        })
        .collect();
    let (number, points) = numbered(&held, meter)?;
    let sides = sides_of(&held, &number, meter)?;
    let mut runs: Vec<Run> = Vec::new();
    let mut start = 0usize;
    for end in 1..=sides.len() {
        let next = sides.get(end).zip(sides.get(start));
        if next.is_none_or(|(b, a)| line_order(&a.key, &b.key) != Some(Ordering::Equal)) {
            let group = sides.get(start..end).ok_or(MergeError::Open)?;
            meetings(start, group, &held, &mut runs)?;
            start = end;
        }
    }
    let runs = cut_at_ends(runs, &points)?;
    let mut used: Vec<usize> = runs.iter().flat_map(|e| [e.low, e.high]).collect();
    used.sort_unstable();
    used.dedup();
    let vertex = |id: usize| used.binary_search(&id).map_err(|_| MergeError::Open);
    let mut edges = Vec::with_capacity(runs.len());
    for run in &runs {
        edges.push((vertex(run.low)?, vertex(run.high)?));
    }
    let mut vertices = Vec::with_capacity(used.len());
    for id in &used {
        vertices.push(points.get(*id).ok_or(MergeError::Open)?.point);
    }
    Ok(Curves { vertices, edges })
}

/// What numbers a face: its sheet, then its lowest fin's edge and way.
type FaceKey = (usize, usize, usize, bool);

/// Merges `pieces` into the faces, edges and shells of a body, every number the shape's own.
///
/// # Errors
///
/// A [`MergeError`]: pieces that overlap on one sheet or do not close, a shell facing inward with
/// nothing around it, or a stopped meter.
pub fn merge(
    pieces: &[Outward],
    mode: Merge,
    meter: &mut WorkMeter<'_>,
) -> Result<Merged, MergeError> {
    merge_with(pieces, mode, Rules::default(), meter)
}

/// [`merge`], under `rules`.
///
/// # Errors
///
/// As [`merge`]; open pieces need not close.
#[allow(
    clippy::too_many_lines,
    reason = "the steps of one merge in their order, each short, sharing the numbers they fill"
)]
pub fn merge_with(
    pieces: &[Outward],
    mode: Merge,
    rules: Rules<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Merged, MergeError> {
    let (sheets, mut held) = sheets_of(pieces, mode)?;
    let kept = held.len();
    held.extend(rules.cuts.iter().map(|piece| Held {
        sheet: CUT,
        piece,
        given: CUT,
    }));
    let (number, points) = numbered(&held, meter)?;
    let sides = sides_of(&held, &number, meter)?;
    held.truncate(kept);

    // Each line's runs, and which pieces join across them.
    let mut same_piece: Vec<usize> = (0..held.len()).collect();
    let mut runs: Vec<Run> = Vec::new();
    let mut start = 0usize;
    for end in 1..=sides.len() {
        let next = sides.get(end).zip(sides.get(start));
        if next.is_none_or(|(b, a)| line_order(&a.key, &b.key) != Some(Ordering::Equal)) {
            let group = sides.get(start..end).ok_or(MergeError::Open)?;
            sweep(start, group, &sheets, &mut same_piece, &mut runs)?;
            start = end;
        }
    }
    let runs = without_slits(runs, &same_piece);
    let runs = cut_at_ends(whole_runs(runs, &same_piece), &points)?;

    // Faces: the connected pieces of a sheet, numbered by the sheet and their lowest fin, which
    // only one face of a sheet can hold.
    let mut lowest: BTreeMap<usize, FaceKey> = BTreeMap::new();
    for run in &runs {
        for fin in &run.fins {
            let key = (fin.sheet, run.low, run.high, fin.rising);
            let slot = lowest.entry(root(&same_piece, fin.held)).or_insert(key);
            *slot = key.min(*slot);
        }
    }
    let mut face_list: Vec<(FaceKey, usize)> =
        lowest.iter().map(|(top, key)| (*key, *top)).collect();
    face_list.sort_unstable();
    let numbered_faces: BTreeMap<usize, usize> = face_list
        .iter()
        .enumerate()
        .map(|(at, (_, top))| (*top, at))
        .collect();
    let face_of = |at: usize| {
        numbered_faces
            .get(&root(&same_piece, at))
            .copied()
            .ok_or(MergeError::Open)
    };
    let mut planes: Vec<Plane> = Vec::with_capacity(face_list.len());
    for ((sheet, ..), _) in &face_list {
        planes.push(sheets.get(*sheet).ok_or(MergeError::Open)?.face);
    }

    // The vertices edges end at, and the fins, numbered by edge.
    let mut used: Vec<usize> = runs.iter().flat_map(|e| [e.low, e.high]).collect();
    used.sort_unstable();
    used.dedup();
    let vertex = |id: usize| used.binary_search(&id).map_err(|_| MergeError::Open);
    let mut fins: Vec<Fin> = Vec::new();
    let mut around_edge: Vec<Vec<usize>> = Vec::with_capacity(runs.len());
    for (e, run) in runs.iter().enumerate() {
        let mut here = Vec::with_capacity(run.fins.len());
        let (low, high) = (vertex(run.low)?, vertex(run.high)?);
        for fin in &run.fins {
            meter.charge(1)?;
            here.push(fins.len());
            let (from, to) = if fin.rising { (low, high) } else { (high, low) };
            fins.push(Fin {
                edge: e,
                face: face_of(fin.held)?,
                from,
                to,
                bound: fin.bound,
                piece: held.get(fin.held).ok_or(MergeError::Open)?.given,
            });
        }
        if !rules.open && here.len().checked_rem(2) != Some(0) {
            return Err(MergeError::Open);
        }
        around_edge.push(here);
    }

    // Radial order, which joins faces into shells, numbered by their lowest face.
    let mut same_shell: Vec<usize> = (0..face_list.len()).collect();
    let mut edges = Vec::with_capacity(runs.len());
    for (run, here) in runs.iter().zip(&around_edge) {
        edges.push(Edge {
            low: vertex(run.low)?,
            high: vertex(run.high)?,
            fins: radial(here, &fins, &planes, rules.open, &mut same_shell)?,
        });
    }
    let mut tops: Vec<usize> = (0..face_list.len()).map(|f| root(&same_shell, f)).collect();
    tops.sort_unstable();
    tops.dedup();
    let shell_of = |face: usize| {
        tops.binary_search(&root(&same_shell, face))
            .map_err(|_| MergeError::Open)
    };
    let mut shell_of_piece = Vec::with_capacity(held.len());
    let mut piece_faces = alloc::vec![0usize; held.len()];
    for (at, entry) in held.iter().enumerate() {
        let face = face_of(at)?;
        shell_of_piece.push(shell_of(face)?);
        piece_faces
            .iter_mut()
            .skip(entry.given)
            .take(1)
            .for_each(|slot| *slot = face);
    }
    let nested = if rules.open {
        alloc::vec![(true, None); tops.len()]
    } else if held.is_empty() {
        Vec::new()
    } else {
        nest(&held, &shell_of_piece, tops.len(), meter)?
    };

    // Faces and shells.
    let mut own_fins: Vec<Vec<usize>> = alloc::vec![Vec::new(); face_list.len()];
    for (at, fin) in fins.iter().enumerate() {
        own_fins
            .iter_mut()
            .skip(fin.face)
            .take(1)
            .for_each(|list| list.push(at));
    }
    let mut faces = Vec::with_capacity(face_list.len());
    let mut shells: Vec<Shell> = nested
        .iter()
        .map(|(outer, holder)| Shell {
            outer: *outer,
            holder: *holder,
            faces: Vec::new(),
        })
        .collect();
    for (at, (((sheet, ..), _), list)) in face_list.iter().zip(&own_fins).enumerate() {
        let plane = planes.get(at).ok_or(MergeError::Open)?;
        let shell = shell_of(at)?;
        faces.push(Face {
            sheet: *sheet,
            shell,
            rings: rings(&fins, list, plane)?,
        });
        shells
            .iter_mut()
            .skip(shell)
            .take(1)
            .for_each(|s| s.faces.push(at));
    }
    let mut vertices = Vec::with_capacity(used.len());
    for id in &used {
        vertices.push(points.get(*id).ok_or(MergeError::Open)?.point);
    }
    Ok(Merged {
        sheets,
        vertices,
        edges,
        fins,
        faces,
        shells,
        piece_faces,
    })
}
