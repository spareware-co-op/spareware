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

use alloc::vec::Vec;
use core::cmp::{Ordering, Reverse};
use core::fmt;

use geom_num::grid::Coord;
use geom_num::plane::{Plane, Tpi, normals_sign};
use geom_num::{I256, I320, NumError, Sign, Wide};
use geom_run::{ChunkPlan, Exec, Phase, RunError, Stopped, WorkMeter};

use crate::convex::{Piece, opposite};
use crate::order::{Flap, Line, Meeting, Ray, about, around, plane_words, wide_product};
use crate::plane::{Canonical, canonical};
use crate::subdivide::around_on;
use crate::trace::{Cell as Box3, Classified, Polygon, TraceError, classify_with};
use crate::winding::{Entry, Kept, Winding, WindingTest, kept};

/// Making corners or sides, a few dozen pieces a task.
const PIECES: ChunkPlan = ChunkPlan::fixed::<64>();

/// The sorts of corners and sides, in runs of this many.
const SORT_PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// Making each piece's corners.
const CORNERS: Phase = Phase::new("merge: corners");

/// Sorting the corners.
const SORT_CORNERS: Phase = Phase::new("merge: sort corners");

/// Telling where each run of equal points starts, in the corners' sorted order.
const PLACES: Phase = Phase::new("merge: places");

/// The chunk of the places phase: one comparison of two points per item.
const PLACES_PLAN: ChunkPlan = ChunkPlan::fixed::<1024>();

/// Each piece's sheet key.
const SHEETS: Phase = Phase::new("merge: sheet keys");

/// Sorting the pieces by sheet.
const SORT_SHEETS: Phase = Phase::new("merge: sort sheets");

/// Making each piece's sides.
const SIDES: Phase = Phase::new("merge: sides");

/// Cutting each run where other runs end on it.
const ENDS: Phase = Phase::new("merge: cut runs at ends");

/// Each edge's radial order.
const RADIAL: Phase = Phase::new("merge: radial order");

/// Each face's rings.
const RINGS: Phase = Phase::new("merge: rings");

/// Each vertex's name, its lowest three planes.
const NAMES: Phase = Phase::new("merge: vertex names");

/// Runs or edges, a few dozen a task.
const RUNS: ChunkPlan = ChunkPlan::fixed::<64>();

/// Sorting the sides by line.
const SORT_SIDES: Phase = Phase::new("merge: sort sides");

/// Telling where each line's sides start, in the sides' sorted order.
const LINES: Phase = Phase::new("merge: lines");

/// Sweeping the lines.
const SWEEPS: Phase = Phase::new("merge: sweep lines");

/// What sweeping one line gives: the pairs of pieces it joins, and its runs.
type Swept = (Vec<(usize, usize)>, Vec<Run>);

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
    /// The parallel helper broke one of its own rules: a defect, reported instead of a wrong
    /// merge.
    Defect,
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::Num(e) => write!(f, "pieces that cannot be merged: {e:?}"),
            MergeError::Trace(e) => write!(f, "{e}"),
            MergeError::Stopped(_) => f.write_str("the meter stopped the merge"),
            MergeError::Defect => f.write_str("the merge's parallel helper broke a rule"),
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
    /// The face of each piece given: a piece wholly inside a face has no fin to say so.
    pub piece_face: Vec<usize>,
}

/// A piece on a sheet.
struct Held<'p> {
    sheet: usize,
    piece: &'p Piece,
    /// Its index in the pieces given.
    given: usize,
    /// Its face in canonical form, reduced once, which [`nest`]'s tracing reads.
    face: Canonical,
}

/// A corner of a piece: the point where its face and two of its bounds meet, which holds those
/// three planes.
struct Corner {
    point: Tpi,
    /// Each coordinate `X_i` / w, w made positive, as a [`Cell`], where X and w fit machine
    /// words; the wide coordinates are made only to compare a corner that has none.
    cells: Option<[Cell; 3]>,
}

/// One point of the merge: the first of its corners in the sorted order, and where the run of its
/// corners lies in that order, whose planes [`named`] reads where it is off the grid.
struct Place<'c> {
    corner: &'c Corner,
    run: (usize, usize),
}

/// The merge's points: every corner by reference, in piece order, each staying in the list its
/// piece's task made; their sorted order; and the distinct points, each a run of that order.
struct Points<'c> {
    corners: Vec<&'c Corner>,
    order: Vec<usize>,
    places: Vec<Place<'c>>,
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

/// [`ratio_order`] for a, b, c and d in machine words: each product is below 2^254, so [`I256`]
/// holds it, at a quarter of the limbs of the wide rung. Most corners and lines of a building's
/// faces fit.
fn narrow_order(a: i128, b: i128, c: i128, d: i128) -> Option<Ordering> {
    let wide = I256::from_i128;
    let left = wide(a).checked_mul(&wide(d))?;
    let right = wide(c).checked_mul(&wide(b))?;
    Some(left.checked_sub(&right)?.signum().as_i8().cmp(&0))
}

/// A ratio v / over, over positive, as its floor and its remainder in [0, over): two ratios
/// compare by their floors, a zero remainder settles a tie, identical terms are equal, and only
/// then do their cross products decide, in `i128` where the terms fit `i64`, else
/// [`narrow_order`]'s, whose products of two `i128` terms stay below 2^254. The terms are the
/// ratio's own, not reduced: nothing reads them but this order, which decides equal fractions in
/// other terms by the cross products, and reducing every ratio cost a greatest common divisor, a
/// loop of 128-bit remainders, on each of a corner's three cells and a line's six. A corner of a building's faces on the grid has every
/// remainder zero, one on a box's split a half, and so do most of a line's coordinates over a
/// short direction.
#[derive(Clone, Copy, Debug)]
struct Cell {
    floor: i128,
    rest: i128,
    over: i128,
}

impl Cell {
    /// v / `over` split, or `None` unless `over` is positive. Every v has its cell: the floor
    /// and the Euclidean remainder of any `i128` by a positive divisor fit `i128`. One division
    /// gives both, in `i64` where v and `over` fit it, the machine's own division, else in `i128`.
    fn new(v: i128, over: i128) -> Option<Self> {
        if over <= 0 {
            return None;
        }
        if let (Ok(a), Ok(b)) = (i64::try_from(v), i64::try_from(over)) {
            let (floor, rest) = floor_and_rest_in_words(a, b)?;
            return Some(Cell {
                floor: i128::from(floor),
                rest: i128::from(rest),
                over,
            });
        }
        let (floor, rest) = floor_and_rest(v, over)?;
        Some(Cell { floor, rest, over })
    }

    /// The order of the two ratios, exactly.
    fn order(&self, other: &Cell) -> Option<Ordering> {
        let by_floor = self.floor.cmp(&other.floor);
        if by_floor != Ordering::Equal {
            return Some(by_floor);
        }
        if self.rest == 0 || other.rest == 0 {
            return Some((self.rest != 0).cmp(&(other.rest != 0)));
        }
        if (self.rest, self.over) == (other.rest, other.over) {
            return Some(Ordering::Equal);
        }
        let words = |v: i128| i64::try_from(v).ok().map(i128::from);
        if let (Some(a), Some(b), Some(c), Some(d)) = (
            words(self.rest),
            words(self.over),
            words(other.rest),
            words(other.over),
        ) {
            // Each is an i64 widened, so each product is at most 2^126 and the wrapping one exact.
            return Some(a.wrapping_mul(d).cmp(&c.wrapping_mul(b)));
        }
        narrow_order(self.rest, self.over, other.rest, other.over)
    }
}

/// The Euclidean floor and remainder of a by b, b positive, from one division: the quotient
/// rounded towards zero, and one less, with the remainder one b more, where the remainder it leaves
/// is negative. The truncated remainder has a's sign and is smaller than b, so a less it, the
/// quotient times b, fits, and so does each step after.
fn floor_and_rest(a: i128, b: i128) -> Option<(i128, i128)> {
    let quotient = a.checked_div(b)?;
    let rest = a.checked_sub(quotient.checked_mul(b)?)?;
    if rest < 0 {
        return Some((quotient.checked_sub(1)?, rest.checked_add(b)?));
    }
    Some((quotient, rest))
}

/// [`floor_and_rest`] in `i64`, the machine's own division.
fn floor_and_rest_in_words(a: i64, b: i64) -> Option<(i64, i64)> {
    let quotient = a.checked_div(b)?;
    let rest = a.checked_sub(quotient.checked_mul(b)?)?;
    if rest < 0 {
        return Some((quotient.checked_sub(1)?, rest.checked_add(b)?));
    }
    Some((quotient, rest))
}

/// Each `X_i` / w as a [`Cell`], w made positive, if every step fits.
fn cells_of(x: [i128; 3], w: i128) -> Option<[Cell; 3]> {
    let turn = |v: i128| {
        if w.is_negative() {
            v.checked_neg()
        } else {
            Some(v)
        }
    };
    let over = turn(w)?;
    let [first, second, third] = x;
    Some([
        Cell::new(turn(first)?, over)?,
        Cell::new(turn(second)?, over)?,
        Cell::new(turn(third)?, over)?,
    ])
}

/// A corner's X and w with w positive, in wide integers: at most 2^294 and 2^252.
fn wide_of(corner: &Corner) -> Option<([I320; 3], I256)> {
    let (coordinates, weight) = corner.point.homogeneous().ok()?;
    if !weight.is_negative() {
        return Some((coordinates, weight));
    }
    let [first, second, third] = coordinates;
    Some((
        [
            first.checked_neg()?,
            second.checked_neg()?,
            third.checked_neg()?,
        ],
        weight.checked_neg()?,
    ))
}

/// A cell's [`lead`]: its floor, the first 32 binary digits of its fraction, and whether the
/// fraction runs on past them.
type Lead = (i128, u32, bool);

/// A cell's floor, the first 32 binary digits of its fraction, rest / over, and whether the
/// fraction runs on past them, exactly: a key that never decreases as the cell grows (where floor
/// and digits agree, a fraction that stops there is the smaller), so where two cells' leads differ
/// they order as their leads do, two leads that agree and stop are equal cells, and only two that
/// agree and run on leave the cells themselves to decide. A remainder below 2^96 takes one
/// division of it times 2^32 by over, which fits a `u128`; a wider one long division, a digit at
/// a time: the remainder stays below over, itself below 2^127, so doubling it fits a `u128`.
fn lead(cell: &Cell) -> Lead {
    let over = cell.over.unsigned_abs();
    let rest = cell.rest.unsigned_abs();
    if rest == 0 {
        return (cell.floor, 0, false);
    }
    if rest.leading_zeros() >= u32::BITS {
        // rest · 2^32 < over · 2^32 fits, and the quotient is below 2^32.
        let scaled = rest.wrapping_shl(u32::BITS);
        let digits = scaled.checked_div(over).unwrap_or(0);
        // digits · over is at most scaled, so the product is exact.
        let more = scaled != digits.wrapping_mul(over);
        return (cell.floor, u32::try_from(digits).unwrap_or(u32::MAX), more);
    }
    let mut rest = rest;
    let mut digits = 0u32;
    for _ in 0..u32::BITS {
        rest = rest.wrapping_shl(1);
        let one = rest >= over;
        digits = digits.wrapping_shl(1) | u32::from(one);
        rest = rest.wrapping_sub(over.wrapping_mul(u128::from(one)));
    }
    (cell.floor, digits, rest != 0)
}

/// How far a floor reaches and still has its own [`mark`]: 2^62, so a floor in reach plus 2^63
/// lies in [2^62, 3 · 2^62), below 2^64, and its mark above 1 and below `u128::MAX`.
const REACH: i128 = 1 << 62;

/// A cell's [`lead`] in one `u128` that orders as the lead does: the floor plus 2^63 in the high
/// 64 bits, the digits in the next 32, and the lowest bit set where the fraction runs on. A floor
/// past `REACH` marks 1 below, or `u128::MAX` above, every mark in reach, each with its lowest bit
/// set, so two cells past it on one side leave the cells to decide.
fn mark(cell: &Cell) -> u128 {
    let (floor, digits, more) = lead(cell);
    if floor < REACH.wrapping_neg() {
        return 1;
    }
    if floor >= REACH {
        return u128::MAX;
    }
    // In reach, floor + 2^63 lies in [2^62, 3 · 2^62), a `u64`.
    let shifted = u64::try_from(floor.wrapping_add(1 << 63)).map_or(0, u128::from);
    shifted.wrapping_shl(64) | u128::from(digits).wrapping_shl(32) | u128::from(more)
}

/// A corner's or a line's key in its sort, where it has [`Cell`]s: the [`mark`]s of the first `M`
/// cells that can differ, and the cells, read only where two marks agree and run on, or where
/// every mark agrees and cells past them are left. The marks of one point, or one line, are its
/// cells' own, so the key is a total order, the exact one of [`point_order`] or [`line_order`],
/// and a sort and the runs of equal keys after it read the copied marks first. A corner marks all
/// three of its cells, as nearly every corner of a mesh is a grid point, whose marks stop, so that
/// equal points are told by their marks alone; a line marks two, which tell most lines apart, as
/// the cells of a line over its direction mostly run on, and a key holding all five would be twice
/// as long.
#[derive(Clone, Copy, Debug)]
struct MarkKey<'a, const N: usize, const M: usize> {
    marks: [u128; M],
    /// Whether cells past the marked ones are left to compare.
    past: bool,
    cells: &'a [Cell; N],
}

impl<'a, const N: usize, const M: usize> MarkKey<'a, N, M> {
    /// The key of `cells`, marking `M` of them from `from` on; any past the end mark 0.
    fn new(cells: &'a [Cell; N], from: usize) -> Self {
        let mut marks = [0u128; M];
        for (slot, cell) in marks.iter_mut().zip(cells.iter().skip(from)) {
            *slot = mark(cell);
        }
        let past = from.saturating_add(M) < N;
        MarkKey { marks, past, cells }
    }
}

impl<const N: usize, const M: usize> PartialEq for MarkKey<'_, N, M> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<const N: usize, const M: usize> Eq for MarkKey<'_, N, M> {}

impl<const N: usize, const M: usize> PartialOrd for MarkKey<'_, N, M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize, const M: usize> Ord for MarkKey<'_, N, M> {
    fn cmp(&self, other: &Self) -> Ordering {
        for (a, b) in self.marks.iter().zip(&other.marks) {
            if a != b {
                return a.cmp(b);
            }
            if a & 1 != 0 {
                return CellKey(self.cells).cmp(&CellKey(other.cells));
            }
        }
        if self.past || other.past {
            return CellKey(self.cells).cmp(&CellKey(other.cells));
        }
        Ordering::Equal
    }
}

/// A corner's place in [`point_order`], or a line's after its `first` in [`line_order`], where it
/// has [`Cell`]s: comparing cells cannot fail, as each product they need is of two `i128`s, below
/// 2^254 in [`I256`]. It borrows the cells.
#[derive(Clone, Copy, Debug)]
struct CellKey<'a, const N: usize>(&'a [Cell; N]);

impl<const N: usize> PartialEq for CellKey<'_, N> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<const N: usize> Eq for CellKey<'_, N> {}

impl<const N: usize> PartialOrd for CellKey<'_, N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for CellKey<'_, N> {
    fn cmp(&self, other: &Self) -> Ordering {
        for (p, q) in self.0.iter().zip(other.0) {
            // `None` needs a product past 2^255, which two `i128`s cannot make.
            let order = p.order(q).unwrap_or(Ordering::Equal);
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

/// A parallel phase's error as the merge's: the item's own, the merge's or the arrangement's, or
/// the meter's stop, which the helper latches; a stop it did not latch is the helper's defect.
fn phase_error<E: Into<MergeError>>(error: RunError<E>, meter: &mut WorkMeter<'_>) -> MergeError {
    match error {
        RunError::Failed(e) => e.into(),
        _ => meter
            .poll()
            .err()
            .map_or(MergeError::Defect, MergeError::Stopped),
    }
}

/// Points lexicographically: by [`Cell`]s where both have them, else at most 2^294 · 2^252 in
/// 576 bits. The first coordinate that differs decides, and the rest are not compared.
fn point_order(a: &Corner, b: &Corner) -> Option<Ordering> {
    if let (Some(own), Some(other)) = (&a.cells, &b.cells) {
        for (p, q) in own.iter().zip(other) {
            let order = p.order(q)?;
            if order != Ordering::Equal {
                return Some(order);
            }
        }
        return Some(Ordering::Equal);
    }
    let ((own, under), (other, beneath)) = (wide_of(a)?, wide_of(b)?);
    for (p, q) in own.iter().zip(&other) {
        let order = ratio_order::<5, 4, 9>(p, &under, q, &beneath)?;
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
    let point = Tpi::meeting(face, p, q)?;
    let cells = Meeting::new(&face, &p, &q).and_then(|at| {
        let (x, w) = at.coordinates();
        cells_of(x, w)
    });
    Ok(Corner { point, cells })
}

/// The Plücker coordinates of a line: its direction, then its moment, the direction's first
/// non-zero component positive. Below 2^209, in [`I256`]. A side holds one for its line, so it
/// keeps only one form: the cells where all six fit machine words, as nearly every line of a mesh
/// does, else the wide coordinates.
#[derive(Clone, Copy, Debug)]
struct LineKey {
    first: usize,
    form: LineForm,
}

/// A line's coordinates in one form.
#[derive(Clone, Copy, Debug)]
enum LineForm {
    /// Each coordinate over the direction's first non-zero component, as a [`Cell`].
    Cells([Cell; 6]),
    /// The coordinates, where some does not fit a machine word.
    Wide([I256; 6]),
}

impl LineKey {
    /// The line of `coordinates`, by its cells where they all fit machine words.
    fn of(coordinates: [I256; 6], first: usize) -> LineKey {
        let form =
            line_cells(&coordinates, first).map_or(LineForm::Wide(coordinates), LineForm::Cells);
        LineKey { first, form }
    }

    /// Its cells, where it has them.
    fn cells(&self) -> Option<&[Cell; 6]> {
        match &self.form {
            LineForm::Cells(cells) => Some(cells),
            LineForm::Wide(_) => None,
        }
    }

    /// Its coordinates: from cells, each the cell's floor times its divisor plus its remainder,
    /// the `i128` it was made from, so in [`I256`] the products and sums cannot fail.
    fn coordinates(&self) -> Option<[I256; 6]> {
        let cells = match &self.form {
            LineForm::Wide(coordinates) => return Some(*coordinates),
            LineForm::Cells(cells) => cells,
        };
        let wide = I256::from_i128;
        let mut out = [wide(0); 6];
        for (slot, cell) in out.iter_mut().zip(cells) {
            let whole = wide(cell.floor).checked_mul(&wide(cell.over))?;
            *slot = whole.checked_add(&wide(cell.rest))?;
        }
        Some(out)
    }
}

/// The line where `p` and `q` meet: direction `n_p` × `n_q`, moment `d_p` `n_q` − `d_q` `n_p`, which
/// is the same up to scale for any two planes through the line.
fn line_key(p: &Plane, q: &Plane) -> Result<LineKey, NumError> {
    match line_in_words(p, q) {
        Some((coordinates, first)) => {
            // Two `i64` planes' coordinates always have cells: the wide form is never taken.
            let wide = LineForm::Wide(coordinates.map(I256::from_i128));
            let form = line_cells_of(&coordinates, first).map_or(wide, LineForm::Cells);
            Ok(LineKey { first, form })
        }
        None => wide_line_key(p, q),
    }
}

/// Whether a line's coordinates are turned round, so that the first non-zero component of its
/// direction is positive: one rule for keys in machine words and in [`I256`], which compare with
/// each other.
fn turned(first: Sign) -> bool {
    first == Sign::Negative
}

/// [`line_key`] in [`I256`], for any two planes: each product below 2^250.
fn wide_line_key(p: &Plane, q: &Plane) -> Result<LineKey, NumError> {
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
    if coordinates.get(first).is_some_and(|v| turned(v.signum())) {
        for slot in &mut coordinates {
            *slot = slot.checked_neg().ok_or(NumError::BudgetExceeded)?;
        }
    }
    Ok(LineKey::of(coordinates, first))
}

/// [`line_key`]'s coordinates and `first` where both planes' coefficients fit `i64`: a product of
/// two `i64`s lies in [−2^126 + 2^63, 2^126], so a minor, the difference of two, has
/// |ab − cd| ≤ 2^127 − 2^63, in `i128` with its negation (the checks never fail). `None` for a
/// direction of zero, which [`line_key`] refuses.
fn line_in_words(p: &Plane, q: &Plane) -> Option<([i128; 6], usize)> {
    let (([px, py, pz], pd), ([qx, qy, qz], qd)) = (plane_words(p)?, plane_words(q)?);
    let minor = |a, b, c, d| wide_product(a, b).checked_sub(wide_product(c, d));
    let mut coordinates = [
        minor(py, qz, pz, qy)?,
        minor(pz, qx, px, qz)?,
        minor(px, qy, py, qx)?,
        minor(pd, qx, qd, px)?,
        minor(pd, qy, qd, py)?,
        minor(pd, qz, qd, pz)?,
    ];
    let first = coordinates.iter().take(3).position(|v| *v != 0)?;
    if coordinates
        .get(first)
        .is_some_and(|v| turned(Sign::of_i128(*v)))
    {
        for slot in &mut coordinates {
            *slot = slot.checked_neg()?;
        }
    }
    Some((coordinates, first))
}

/// A line's coordinates over its `first` as [`Cell`]s, if all six fit machine words.
fn line_cells(coordinates: &[I256; 6], first: usize) -> Option<[Cell; 6]> {
    let mut words = [0i128; 6];
    for (word, coordinate) in words.iter_mut().zip(coordinates) {
        *word = coordinate.to_i128()?;
    }
    line_cells_of(&words, first)
}

/// [`line_cells`] of coordinates in `i128`.
fn line_cells_of(coordinates: &[i128; 6], first: usize) -> Option<[Cell; 6]> {
    let over = *coordinates.get(first)?;
    let mut cells = [Cell {
        floor: 0,
        rest: 0,
        over,
    }; 6];
    for (cell, coordinate) in cells.iter_mut().zip(coordinates) {
        *cell = Cell::new(*coordinate, over)?;
    }
    Some(cells)
}

/// The order of two lines, equal when they are one line: by which component of the direction comes
/// first, then every coordinate over that component, by [`Cell`]s where both have them, else
/// below 2^418 in 512 bits. The first coordinate that differs decides, and the rest are not
/// compared.
fn line_order(a: &LineKey, b: &LineKey) -> Option<Ordering> {
    let order = a.first.cmp(&b.first);
    if order != Ordering::Equal {
        return Some(order);
    }
    if let (Some(p), Some(q)) = (a.cells(), b.cells()) {
        for (x, y) in p.iter().zip(q.iter()) {
            let order = x.order(y)?;
            if order != Ordering::Equal {
                return Some(order);
            }
        }
        return Some(Ordering::Equal);
    }
    let over = a.first;
    let (p, q) = (&a.coordinates()?, &b.coordinates()?);
    for i in 0..6 {
        let order = ratio_order::<4, 4, 8>(p.get(i)?, p.get(over)?, q.get(i)?, q.get(over)?)?;
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
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

/// Points every element of a forest [`join`] made straight at its root, so that [`root`] takes at
/// most one step. [`join`] puts the lower root on top, so each element's parent comes before it,
/// and in index order that parent already points at its root.
fn flatten(parents: &mut [usize]) {
    for at in 0..parents.len() {
        let up = parents.get(at).copied().unwrap_or(at);
        let top = parents.get(up).copied().unwrap_or(up);
        parents
            .iter_mut()
            .skip(at)
            .take(1)
            .for_each(|slot| *slot = top);
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
fn stretches<'s>(sides: &[&'s Side]) -> Vec<(usize, usize, Vec<&'s Side>)> {
    let mut marks: Vec<usize> = sides.iter().flat_map(|s| [s.low, s.high]).collect();
    marks.sort_unstable();
    marks.dedup();
    let mut waiting: Vec<&Side> = sides.to_vec();
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
/// lies along it, and the rest are runs. The joins are listed in `joins`, in the order found, for
/// the caller to make: a sweep reads no join, so lines can be swept apart and joined after.
fn sweep(
    line: usize,
    sides: &[&Side],
    sheets: &[Sheet],
    joins: &mut Vec<(usize, usize)>,
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
    // A line whose sides all run between the same two points, as most do, is one stretch with
    // every side along it, taken in the order [`stretches`] would take them, last first.
    let ends = (first.low, first.high);
    if sides.iter().all(|side| (side.low, side.high) == ends) {
        let along = sides.iter().rev().copied();
        return stretch((line, ends, on), along, joins, runs);
    }
    for (low, high, active) in stretches(sides) {
        stretch((line, (low, high), on), active.iter().copied(), joins, runs)?;
    }
    Ok(())
}

/// One stretch of line `line`, between its two points and on its two planes, with `active` the
/// sides along all of it: pieces of one sheet on both sides of it join unless a cut lies along it,
/// and the rest are a run's fins, in sheet order.
fn stretch<'s, I>(
    (line, (low, high), on): (usize, (usize, usize), [Plane; 2]),
    active: I,
    joins: &mut Vec<(usize, usize)>,
    runs: &mut Vec<Run>,
) -> Result<(), MergeError>
where
    I: Iterator<Item = &'s Side>,
{
    let mut cut = false;
    // Each sheet's holding, in sheet order: a stretch holds few sheets, so a sorted list.
    let mut held: Vec<(usize, Holding<'s>)> = Vec::with_capacity(2);
    for side in active {
        if side.sheet == CUT {
            cut = true;
            continue;
        }
        let at = match held.binary_search_by_key(&side.sheet, |(sheet, _)| *sheet) {
            Ok(at) => at,
            Err(at) => {
                held.insert(at, (side.sheet, (0, None, None)));
                at
            }
        };
        for (_, slot) in held.iter_mut().skip(at).take(1) {
            if side.rising {
                *slot = (slot.0.saturating_add(1), Some(side), slot.2);
            } else {
                *slot = (slot.0.saturating_sub(1), slot.1, Some(side));
            }
        }
    }
    let mut fins = Vec::with_capacity(held.len());
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
            (0, Some(a), Some(b)) => joins.push((a.held, b.held)),
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

/// A piece's place among the sheets: its canonical plane, which way it faces, and with seams kept
/// its source and its two vectors.
type SheetKey = (
    (i128, [i128; 3]),
    bool,
    usize,
    Vec<(u32, i32)>,
    Vec<(u32, i32)>,
);

/// The sheets of `pieces`, sorted by exact keys so the input's order counts for nothing, and each
/// piece on its sheet. The keys are made, and sorted, on `exec`'s pool.
fn sheets_of<'p>(
    pieces: &'p [Outward],
    merge: Merge,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Sheet>, Vec<Held<'p>>), MergeError> {
    let seams = merge == Merge::KeepSeams;
    let made = exec.try_map(
        meter,
        SHEETS,
        PIECES,
        pieces,
        |_, piece, _| -> Result<(SheetKey, Canonical), MergeError> {
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
            Ok((key, face))
        },
    );
    let keys = made.map_err(|e| phase_error(e, meter))?;
    // Equal keys by the sheet's source faces as the result does where one can, the lowest first
    // (11.5); the stable sort keeps the input's order past that, as the serial sort did.
    let keyed: Vec<(&SheetKey, (bool, usize))> = keys
        .iter()
        .zip(pieces)
        .map(|((key, _), piece)| (key, (piece.turned, piece.polygon)))
        .collect();
    let sorted = exec.sort_stable::<_, _, MergeError, _>(
        meter,
        SORT_SHEETS,
        SORT_PLAN,
        &keyed,
        |(key, source)| (*key, *source),
    );
    let order = sorted.map_err(|e| phase_error(e, meter))?;
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut held = Vec::with_capacity(order.len());
    let mut last: Option<&SheetKey> = None;
    for at in order {
        let outward = pieces.get(at).ok_or(MergeError::Open)?;
        let (key, face) = keys.get(at).ok_or(MergeError::Open)?;
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
            given: at,
            face: *face,
        });
    }
    Ok((sheets, held))
}

/// Every corner of `held`, each piece's in a list of its own, made on `exec`'s pool.
fn corners_of(
    held: &[Held<'_>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<Corner>>, MergeError> {
    let made = exec.try_map(
        meter,
        CORNERS,
        PIECES,
        held,
        |_, piece, m| -> Result<Vec<Corner>, MergeError> {
            let after = piece.piece.bounds.iter().cycle().skip(1);
            let mut out = Vec::with_capacity(piece.piece.bounds.len());
            for (bound, next) in piece.piece.bounds.iter().zip(after) {
                m.charge(1)?;
                out.push(corner(piece.piece.face, *bound, *next)?);
            }
            Ok(out)
        },
    );
    made.map_err(|e| phase_error(e, meter))
}

/// The corners `made` gives each piece, with each corner's number, in piece order, and the
/// distinct points in order.
fn numbered<'c>(
    made: &'c [Vec<Corner>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<usize>, Points<'c>), MergeError> {
    // Every corner by reference, in piece order: each stays in the list its piece's task made,
    // rather than being moved into one list, a copy of every corner.
    let corners: Vec<&Corner> = made.iter().flatten().collect();
    // Each key holds every coordinate's [`mark`], so the sort and the runs of equal points after it
    // read the cells only where two marks agree and run on.
    let keys: Option<Vec<MarkKey<'_, 3, 3>>> = corners
        .iter()
        .map(|c| c.cells.as_ref().map(|cells| MarkKey::new(cells, 0)))
        .collect();
    let order = if let Some(keys) = &keys {
        // Equal points keep their order, as the serial stable sort keeps it.
        let sorted =
            exec.sort_stable::<_, _, MergeError, _>(meter, SORT_CORNERS, SORT_PLAN, keys, |key| {
                *key
            });
        sorted.map_err(|e| phase_error(e, meter))?
    } else {
        let mut order: Vec<usize> = (0..corners.len()).collect();
        let mut failed = false;
        let at = |i: &usize| corners.get(*i).copied();
        order.sort_by(|a, b| {
            let found = at(a).zip(at(b)).and_then(|(p, q)| point_order(p, q));
            noted(found, &mut failed)
        });
        (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
        order
    };
    // Each run of equal points in that order is one place: its first corner, and where the run
    // lies, so no corner's planes are copied. Whether a corner starts a run is told on the pool,
    // against the corner before it in the order, by the keys the sort read where there are any:
    // they and [`point_order`] compare points exactly and never fail on corners (a wide coordinate
    // fails only if a budget were wrong), so a corner equal to the one before it is equal to its
    // run's first. The runs are then walked once, in order.
    let order_of = |before: usize, at: usize| match &keys {
        Some(keys) => Some(keys.get(before)?.cmp(keys.get(at)?)),
        None => point_order(corners.get(before)?, corners.get(at)?),
    };
    let starts = exec.try_map(
        meter,
        PLACES,
        PLACES_PLAN,
        &order,
        |k, at, _| -> Result<bool, MergeError> {
            let Some(before) = k.checked_sub(1).and_then(|b| order.get(b)) else {
                return Ok(true);
            };
            Ok(order_of(*before, *at) != Some(Ordering::Equal))
        },
    );
    let starts = starts.map_err(|e| phase_error(e, meter))?;
    let mut number = alloc::vec![0usize; corners.len()];
    let mut places: Vec<Place<'c>> = Vec::with_capacity(starts.iter().filter(|s| **s).count());
    let mut start = 0usize;
    while let Some(first) = order.get(start) {
        let leader = corners.get(*first).ok_or(MergeError::Open)?;
        let after = start.saturating_add(1);
        let end = starts
            .iter()
            .skip(after)
            .position(|starts_a_place| *starts_a_place)
            .map_or(order.len(), |k| after.saturating_add(k));
        for at in order.get(start..end).ok_or(MergeError::Open)? {
            *number.get_mut(*at).ok_or(MergeError::Open)? = places.len();
        }
        places.push(Place {
            corner: leader,
            run: (start, end),
        });
        start = end;
    }
    let points = Points {
        corners,
        order,
        places,
    };
    Ok((number, points))
}

/// Whether `plane` is a split of [`crate::subdivide`]'s, on an odd half step.
fn splits_boxes(plane: &Plane) -> bool {
    let n = plane.n();
    n.iter().filter(|v| **v != 0).count() == 1
        && n.iter().any(|v| v.unsigned_abs() == 2)
        && plane.d() & 1 != 0
}

/// Place `id`'s point under a name the boxes cannot change. A grid point is named by its three
/// axis planes: which corners it has, and so which planes reach it, depends on how far each box's
/// cuts run (a face that only touches a corner of another solid is cut there in one box and not
/// in a smaller one). A point off the grid takes the first three planes through its corners, in
/// plane order and leaving out the boxes' own, that meet in one point; one with no such three,
/// which only a seam has, keeps its first corner's.
fn named(points: &Points<'_>, id: usize, meter: &mut WorkMeter<'_>) -> Result<Tpi, MergeError> {
    let place = points.places.get(id).ok_or(MergeError::Open)?;
    if let Some(at) = grid_point(place.corner)? {
        return Ok(at);
    }
    let (start, end) = place.run;
    let run = points.order.get(start..end).ok_or(MergeError::Open)?;
    let mut list: Vec<Plane> = Vec::with_capacity(run.len().saturating_mul(3));
    for at in run {
        let corner = points.corners.get(*at).ok_or(MergeError::Open)?;
        let planes = corner.point.planes().iter();
        list.extend(planes.filter(|plane| !splits_boxes(plane)));
    }
    list.sort_unstable_by_key(|plane| (plane.n(), plane.d()));
    list.dedup();
    let mut found = None;
    'search: for (i, a) in list.iter().enumerate() {
        for (j, b) in list.iter().enumerate().skip(i.saturating_add(1)) {
            for c in list.iter().skip(j.saturating_add(1)) {
                meter.charge(1)?;
                if normals_sign(a, b, c)? != Sign::Zero {
                    found = Some(Tpi::meeting(*a, *b, *c)?);
                    break 'search;
                }
            }
        }
    }
    Ok(found.unwrap_or(place.corner.point))
}

/// A corner's point named by its three axis planes, when it is a grid point: read from its cells,
/// or from its coordinates when it has none.
fn grid_point(corner: &Corner) -> Result<Option<Tpi>, NumError> {
    let at = if let Some(cells) = corner.cells {
        if cells.iter().any(|c| c.rest != 0) {
            return Ok(None);
        }
        cells.map(|c| c.floor)
    } else {
        let near = corner.point.to_grid()?;
        if !near.is_exact() {
            return Ok(None);
        }
        let p = near.value();
        [p.x.get(), p.y.get(), p.z.get()].map(i128::from)
    };
    let [x, y, z] = at;
    // A grid point's coordinates are within 2^40, so negating one cannot saturate.
    let axis = |n: [i128; 3], v: i128| Plane::new(n, v.saturating_neg());
    let (on_x, on_y, on_z) = (
        axis([1, 0, 0], x)?,
        axis([0, 1, 0], y)?,
        axis([0, 0, 1], z)?,
    );
    Ok(Some(Tpi::meeting(on_x, on_y, on_z)?))
}

/// Every side of `held`, each piece's in a list of its own, made on `exec`'s pool.
fn sides_of(
    held: &[Held<'_>],
    number: &[usize],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<Side>>, MergeError> {
    // Each piece's first corner's place in `number`.
    let mut bases = Vec::with_capacity(held.len());
    let mut base = 0usize;
    for entry in held {
        bases.push(base);
        base = base.saturating_add(entry.piece.bounds.len());
    }
    let made = exec.try_map(
        meter,
        SIDES,
        PIECES,
        held,
        |at, entry, m| -> Result<Vec<Side>, MergeError> {
            let k = entry.piece.bounds.len();
            let base = bases.get(at).copied().ok_or(MergeError::Open)?;
            let mut out = Vec::with_capacity(k);
            for (i, bound) in entry.piece.bounds.iter().enumerate() {
                m.charge(1)?;
                // Bound i runs from corner i − 1 to corner i.
                let before = base.saturating_add(i.checked_sub(1).unwrap_or(k.saturating_sub(1)));
                let from = number.get(before).copied().unwrap_or(0);
                let to = number.get(base.saturating_add(i)).copied().unwrap_or(0);
                out.push(Side {
                    key: line_key(&entry.piece.face, bound)?,
                    low: from.min(to),
                    high: from.max(to),
                    rising: from < to,
                    sheet: entry.sheet,
                    held: at,
                    bound: *bound,
                });
            }
            Ok(out)
        },
    );
    made.map_err(|e| phase_error(e, meter))
}

/// A line's key in the sort of sides: its `first`, then the [`MarkKey`] of its cells after it.
type LineSortKey<'a> = (usize, MarkKey<'a, 6, 2>);

/// Sides in line order, as places in the sides given, and each line's stretch of that order.
type ByLine = (Vec<usize>, Vec<(usize, usize)>);

/// The order of `sides` by line, sorted on `exec`'s pool where every side has its [`Cell`]s, and
/// each line's stretch of that order, from its first side to the next line's.
fn by_line(
    sides: &[&Side],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ByLine, MergeError> {
    // A line's cells up to its `first` are fixed by `first`: 0 for each component of the
    // direction before it, and 1 at it. So after `first`, the [`mark`]s of the two cells after it
    // order lines as their cells do where they differ, and the sort reads the cells only where two
    // marks agree and run on, or both agree.
    let keys: Option<Vec<LineSortKey<'_>>> = sides
        .iter()
        .map(|s| {
            let from = s.key.first.saturating_add(1);
            s.key
                .cells()
                .map(|cells| (s.key.first, MarkKey::new(cells, from)))
        })
        .collect();
    let order = if let Some(keys) = &keys {
        // Sides on one line keep their order, as the serial stable sort keeps it.
        let sorted =
            exec.sort_stable::<_, _, MergeError, _>(meter, SORT_SIDES, SORT_PLAN, keys, |key| *key);
        sorted.map_err(|e| phase_error(e, meter))?
    } else {
        let mut order: Vec<usize> = (0..sides.len()).collect();
        let mut failed = false;
        let key = |i: &usize| sides.get(*i).map(|side| &side.key);
        order.sort_by(|a, b| {
            let found = key(a).zip(key(b)).and_then(|(p, q)| line_order(p, q));
            noted(found, &mut failed)
        });
        (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
        order
    };
    // Where each line's sides start is told on the pool, against the side before it in the order,
    // by the keys the sort read where there are any: they and [`line_order`] are exact and never
    // fail, so a side on the line of the one before it is on its line's first's.
    let order_of = |before: usize, at: usize| match &keys {
        Some(keys) => Some(keys.get(before)?.cmp(keys.get(at)?)),
        None => line_order(&sides.get(before)?.key, &sides.get(at)?.key),
    };
    let starts = exec.try_map(
        meter,
        LINES,
        PLACES_PLAN,
        &order,
        |k, at, _| -> Result<bool, MergeError> {
            let Some(before) = k.checked_sub(1).and_then(|b| order.get(b)) else {
                return Ok(true);
            };
            Ok(order_of(*before, *at) != Some(Ordering::Equal))
        },
    );
    let starts = starts.map_err(|e| phase_error(e, meter))?;
    let mut lines: Vec<(usize, usize)> = Vec::new();
    for (k, starts_a_line) in starts.into_iter().enumerate() {
        if starts_a_line {
            if let Some(last) = lines.last_mut() {
                last.1 = k;
            }
            lines.push((k, order.len()));
        }
    }
    Ok((order, lines))
}

/// The sides in `order`, as references, so that sorting them moves none.
fn in_order<'s>(sides: &[&'s Side], order: &[usize]) -> Result<Vec<&'s Side>, MergeError> {
    order
        .iter()
        .map(|at| sides.get(*at).copied().ok_or(MergeError::Open))
        .collect()
}

/// `runs` without seams that have one face on both sides: a cut that ends inside a face leaves it
/// whole. Only a seam can hold two fins of one sheet, so those two are the seam.
fn without_slits(mut runs: Vec<Run>, parents: &[usize]) -> Vec<Run> {
    for run in &mut runs {
        // A seam's two fins are of one sheet, so a run whose fins are all of different sheets, as
        // an edge between faces is, has none.
        let shared = |(at, fin): (usize, &RunFin)| {
            let mut later = run.fins.iter().skip(at.saturating_add(1));
            later.any(|other| other.sheet == fin.sheet)
        };
        if !run.fins.iter().enumerate().any(shared) {
            continue;
        }
        // Which fins are a seam's, told against every fin of the run before any goes: a flag for
        // each, rather than a copy of the fins.
        let seam: Vec<bool> = run
            .fins
            .iter()
            .map(|fin| {
                run.fins.iter().any(|other| {
                    other.sheet == fin.sheet
                        && other.rising != fin.rising
                        && root(parents, other.held) == root(parents, fin.held)
                })
            })
            .collect();
        let mut flags = seam.iter();
        run.fins
            .retain(|_| flags.next().is_some_and(|in_seam| !*in_seam));
    }
    runs.retain(|run| !run.fins.is_empty());
    runs
}

/// The whole parts of a corner's coordinates, where it has [`Cell`]s: it lies in the unit cell from
/// them.
fn floors(corner: &Corner) -> Option<[i128; 3]> {
    corner.cells.map(|[x, y, z]| [x.floor, y.floor, z.floor])
}

/// Whether a corner whose floors are `at` lies outside the floors' range `low` to `high` of two
/// corners: a point of the segment between them lies within it.
fn beyond(at: &[i128; 3], (low, high): (&[i128; 3], &[i128; 3])) -> bool {
    at.iter()
        .zip(low.iter().zip(high))
        .any(|(v, (l, h))| v < l || v > h)
}

/// The lowest and highest of two corners' floors on each axis.
fn spanned(a: &[i128; 3], b: &[i128; 3]) -> ([i128; 3], [i128; 3]) {
    let (mut low, mut high) = (*a, *a);
    for ((l, h), v) in low.iter_mut().zip(&mut high).zip(b) {
        *l = (*l).min(*v);
        *h = (*h).max(*v);
    }
    (low, high)
}

/// `runs`, each cut wherever another run ends on its line. A run's line holds only points between
/// its ends' cells, so a point outside them is not read.
fn cut_at_ends(
    runs: Vec<Run>,
    places: &[Place<'_>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Run>, MergeError> {
    let mut ends: Vec<usize> = runs.iter().flat_map(|r| [r.low, r.high]).collect();
    ends.sort_unstable();
    ends.dedup();
    let cells: Vec<Option<[i128; 3]>> = places.iter().map(|p| floors(p.corner)).collect();
    let made = exec.try_map(
        meter,
        ENDS,
        RUNS,
        &runs,
        |_, run, _| -> Result<Result<Vec<usize>, MergeError>, MergeError> {
            Ok(cuts_of(run, (&ends, &cells), places))
        },
    );
    let made = made.map_err(|e| phase_error(e, meter))?;
    // Each run's parts, in run order, and the first run's failure as a serial walk meets it. A
    // run is moved into its last part, so one with no cut is not copied.
    let mut edges: Vec<Run> = Vec::with_capacity(runs.len());
    for (run, marks) in runs.into_iter().zip(made) {
        let mut low = run.low;
        for mark in marks? {
            edges.push(Run {
                low,
                high: mark,
                ..run.clone()
            });
            low = mark;
        }
        edges.push(Run { low, ..run });
    }
    Ok(edges)
}

/// Where `run` is cut, in order: each of the sorted `ends` that lies on its line between its own,
/// those numbered between its ends, found by binary search, whose `cells` fall within its ends'
/// cells.
fn cuts_of(
    run: &Run,
    (ends, cells): (&[usize], &[Option<[i128; 3]>]),
    places: &[Place<'_>],
) -> Result<Vec<usize>, MergeError> {
    let [p, q] = run.on;
    // The floors' range on each axis, made once for the run rather than for each mark.
    let span = cells.get(run.low).zip(cells.get(run.high));
    let span = span
        .and_then(|(a, b)| a.as_ref().zip(b.as_ref()))
        .map(|(a, b)| spanned(a, b));
    let from = ends.partition_point(|e| *e <= run.low);
    let to = ends.partition_point(|e| *e < run.high);
    let mut marks = Vec::new();
    for mark in ends.get(from..to).unwrap_or_default() {
        let outside = span
            .zip(cells.get(*mark).and_then(Option::as_ref))
            .is_some_and(|((low, high), at)| beyond(at, (&low, &high)));
        if outside {
            continue;
        }
        let place = places.get(*mark).ok_or(MergeError::Open)?;
        let side = |plane: &Plane| plane.side_of_tpi(&place.corner.point);
        if side(&p)? == Sign::Zero && side(&q)? == Sign::Zero {
            marks.push(*mark);
        }
    }
    Ok(marks)
}

/// The face's rings, each a ring of fin numbers, in turn, starting at its lowest fin.
fn rings(fins: &[Fin], of_face: &[usize], plane: &Plane) -> Result<Vec<Vec<usize>>, MergeError> {
    // Each fin's vertex it runs from, with its place in `of_face`, sorted: the fins leaving a
    // vertex are one stretch of it, in `of_face`'s order. A fin is used by its place.
    let mut leaving: Vec<(usize, usize)> = Vec::with_capacity(of_face.len());
    for (k, at) in of_face.iter().enumerate() {
        leaving.push((fins.get(*at).ok_or(MergeError::Open)?.from, k));
    }
    leaving.sort_unstable();
    let mut used = alloc::vec![false; of_face.len()];
    let fin_at = |k: usize| -> Result<&Fin, MergeError> {
        let at = of_face.get(k).ok_or(MergeError::Open)?;
        fins.get(*at).ok_or(MergeError::Open)
    };
    let mut out = Vec::new();
    for start in 0..of_face.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut ring = Vec::new();
        let mut now = start;
        loop {
            used.iter_mut().skip(now).take(1).for_each(|u| *u = true);
            ring.push(*of_face.get(now).ok_or(MergeError::Open)?);
            let fin = fin_at(now)?;
            let low = leaving.partition_point(|(v, _)| *v < fin.to);
            let high = leaving.partition_point(|(v, _)| *v <= fin.to);
            let here = leaving.get(low..high).filter(|here| !here.is_empty());
            let mut options = here
                .ok_or(MergeError::Open)?
                .iter()
                .map(|(_, option)| *option)
                .filter(|option| used.get(*option) == Some(&false));
            let Some(first) = options.next() else {
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
                    with: fin_at(o)?.bound,
                    reversed: true,
                })
            };
            for option in options {
                if around(plane, &back, &ray(option)?, &ray(best)?)? == Ordering::Greater {
                    best = option;
                }
            }
            now = best;
        }
        out.push(ring);
    }
    Ok(out)
}

/// The fins of an edge in radial order, [`join_around`] then putting the faces each solid wedge
/// joins in one set; on a sheet every face at an edge is in one set.
fn radial(edge: &[usize], fins: &[Fin], planes: &[Plane]) -> Result<Vec<usize>, MergeError> {
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
    // fin's material, so the other comes first, and the first fin's partner comes last of all. A
    // fin that runs the first's way is never last, so its angle is not read for that.
    let last = |at: usize| -> Result<bool, MergeError> {
        if !turned(at)? {
            return Ok(false);
        }
        Ok(about(&line, &reference, &reference, &flap(at)?)? == Ordering::Equal)
    };
    let mut order = alloc::vec![first];
    for at in edge.iter().skip(1) {
        let this = flap(*at)?;
        let mut place = order.len();
        // Whether this fin comes last of all is the same against each fin placed before it.
        let placed = matches!(last(*at), Ok(false));
        for (k, held) in order.iter().enumerate().filter(|_| placed) {
            let by = about(&line, &reference, &this, &flap(*held)?)?;
            if by == Ordering::Less || (by == Ordering::Equal && turned(*at)?) || last(*held)? {
                place = k;
                break;
            }
        }
        order.insert(place, *at);
    }
    Ok(order)
}

/// Joins the faces `order`, an edge's fins in [`radial`] order, puts in one shell: turning about the
/// line, past a fin that runs against it lies the fin's material.
fn join_around(
    order: &[usize],
    fins: &[Fin],
    open: bool,
    parents: &mut [usize],
) -> Result<(), MergeError> {
    let fin_at = |at: usize| fins.get(at).copied().ok_or(MergeError::Open);
    let start = fin_at(*order.first().ok_or(MergeError::Open)?)?;
    let rising = |fin: &Fin| fin.from < fin.to;
    let next = order.iter().cycle().skip(1);
    for (at, after) in order.iter().zip(next) {
        let (this, other) = (fin_at(*at)?, fin_at(*after)?);
        if open {
            join(parents, this.face, other.face);
        } else if rising(&this) == rising(&start) {
            if rising(&other) == rising(&this) {
                return Err(MergeError::Open);
            }
            join(parents, this.face, other.face);
        }
    }
    Ok(())
}

/// `pieces` as polygons of one shell, each with `entry`, and their faces in canonical form.
fn polygons_of(
    pieces: &[(&Piece, Canonical)],
    entry: Option<Entry>,
) -> (Vec<Polygon>, Vec<Canonical>) {
    let mut polygons = Vec::with_capacity(pieces.len());
    let mut faces = Vec::with_capacity(pieces.len());
    for (piece, face) in pieces {
        polygons.push(Polygon {
            piece: (*piece).clone(),
            entry,
        });
        faces.push(*face);
    }
    (polygons, faces)
}

/// The grid range of `corners` where each has [`Cell`]s and lies in the frame: their floors, and
/// their floors rounded up, so each corner, and the grid point nearest it, lies within.
fn grid_range(corners: &[Corner]) -> Option<GridRange> {
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for corner in corners {
        let cells = corner.cells.as_ref()?;
        for ((l, h), cell) in low.iter_mut().zip(&mut high).zip(cells) {
            let up = cell.floor.checked_add(i128::from(cell.rest != 0))?;
            *l = (*l).min(Coord::from_i128(cell.floor).ok()?.get());
            *h = (*h).max(Coord::from_i128(up).ok()?.get());
        }
    }
    Some((low, high))
}

/// A grid range: its lowest and highest coordinates.
type GridRange = ([i64; 3], [i64; 3]);

/// The vectors in front of and behind the first of `polygons`, whose faces in canonical form are
/// `faces`, traced through all of them inside `cell`, or the box [`around_on`] finds for them
/// where there is none.
fn sides_of_first(
    (polygons, faces): (&[Polygon], &[Canonical]),
    cell: Option<Box3>,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Winding, Winding), MergeError> {
    let cell = match cell {
        Some(cell) => cell,
        None => around_on(exec, polygons, meter).map_err(|e| phase_error(e, meter))?,
    };
    let lists: Vec<Vec<Piece>> = polygons
        .iter()
        .take(1)
        .map(|polygon| alloc::vec![polygon.piece.clone()])
        .collect();
    let held: Vec<(&Polygon, Canonical)> = polygons.iter().zip(faces.iter().copied()).collect();
    let found = classify_with(&cell, &Winding::new(), &held, &lists, meter)?;
    let first = found.into_iter().next();
    Ok(first.map_or_else(|| (Winding::new(), Winding::new()), |c| (c.front, c.back)))
}

/// Which of the shells `among` lie around a point behind shell `at`.
fn around_shell(
    of_shell: &[Vec<(&Piece, Canonical)>],
    (at, among): (usize, &[usize]),
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<usize>, MergeError> {
    let number = |shell: usize| u32::try_from(shell).unwrap_or(u32::MAX);
    let list = of_shell.get(at).ok_or(MergeError::Open)?;
    let (mut polygons, mut faces) = polygons_of(list, None);
    for shell in among.iter().filter(|shell| **shell != at) {
        let others = of_shell.get(*shell).ok_or(MergeError::Open)?;
        let (more, their_faces) = polygons_of(others, Some(number(*shell)));
        polygons.extend(more);
        faces.extend(their_faces);
    }
    let (_, back) = sides_of_first((&polygons, &faces), None, exec, meter)?;
    Ok(among
        .iter()
        .copied()
        .filter(|shell| *shell != at && back.count(number(*shell)) > 0)
        .collect())
}

/// Which shells face out, and the outer shell around each cavity. A shell faces out if the point in
/// front of it is outside it, which takes only its own pieces, so this is linear in the pieces
/// when there are no cavities. The box the point's path leaves by is a step and a half clear of
/// the shell's corners' grid range, `ranges` holding each piece's, where every piece has one; any
/// box clear of the shell gives the same count. A cavity is held by the innermost outer shell
/// around a point behind it: behind a cavity lies the material it is a cavity of, even where
/// another cell fills it back to back.
fn nest(
    held: &[Held<'_>],
    (shell_of_piece, shells): (&[usize], usize),
    ranges: &[Option<GridRange>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(bool, Option<usize>)>, MergeError> {
    let mut of_shell: Vec<Vec<(&Piece, Canonical)>> = alloc::vec![Vec::new(); shells];
    let mut range_of_shell: Vec<Option<GridRange>> =
        alloc::vec![Some(([i64::MAX; 3], [i64::MIN; 3])); shells];
    for ((entry, shell), range) in held.iter().zip(shell_of_piece).zip(ranges) {
        of_shell
            .iter_mut()
            .skip(*shell)
            .take(1)
            .for_each(|list| list.push((entry.piece, entry.face)));
        range_of_shell
            .iter_mut()
            .skip(*shell)
            .take(1)
            .for_each(|slot| *slot = slot.zip(*range).map(widest));
    }
    let half = |v: i64, by: i64| v.saturating_mul(2).saturating_add(by);
    let mut outer = Vec::with_capacity(shells);
    for (list, range) in of_shell.iter().zip(&range_of_shell) {
        meter.charge(1)?;
        let cell = range.map(|(low, high)| Box3 {
            low: low.map(|v| half(v, -3)),
            high: high.map(|v| half(v, 3)),
        });
        let (polygons, faces) = polygons_of(list, Some(0));
        let (front, _) = sides_of_first((&polygons, &faces), cell, exec, meter)?;
        outer.push(front.count(0) == 0);
    }
    let outers: Vec<usize> = (0..shells)
        .filter(|at| outer.get(*at) == Some(&true))
        .collect();
    let mut out = Vec::with_capacity(shells);
    for (at, is_outer) in outer.iter().enumerate() {
        if *is_outer {
            out.push((true, None));
            continue;
        }
        meter.charge(1)?;
        let candidates = around_shell(&of_shell, (at, &outers), exec, meter)?;
        // The innermost of those around it lies inside all the others.
        let mut best = None;
        for shell in &candidates {
            let depth = around_shell(&of_shell, (*shell, &candidates), exec, meter)?.len();
            if best.is_none_or(|(most, _)| depth >= most) {
                best = Some((depth, *shell));
            }
        }
        let (_, holder) = best.ok_or(MergeError::Inside)?;
        out.push((false, Some(holder)));
    }
    Ok(out)
}

/// The grid range that holds both `a` and `b`.
fn widest((a, b): (GridRange, GridRange)) -> GridRange {
    let ((a_low, a_high), (b_low, b_high)) = (a, b);
    let mut low = a_low;
    let mut high = a_high;
    for (l, v) in low.iter_mut().zip(b_low) {
        *l = (*l).min(v);
    }
    for (h, v) in high.iter_mut().zip(b_high) {
        *h = (*h).max(v);
    }
    (low, high)
}

/// Curves where pieces of different owners meet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curves {
    /// The points, lexicographically.
    pub vertices: Vec<Tpi>,
    /// The straight edges, each from its lower vertex to its higher.
    pub edges: Vec<(usize, usize)>,
    /// Each edge's pieces, as places in the list given.
    pub meets: Vec<Vec<usize>>,
}

/// The stretches of one line where pieces of two owners on different planes both have a side.
fn meetings(
    line: usize,
    sides: &[&Side],
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
        let fins = active.iter().map(|side| RunFin {
            sheet: side.sheet,
            rising: side.rising,
            held: side.held,
            bound: side.bound,
        });
        match runs.get_mut(opening..).and_then(<[Run]>::last_mut) {
            Some(run) if run.high == low => {
                run.high = high;
                run.fins.extend(fins);
            }
            _ => runs.push(Run {
                line,
                low,
                high,
                fins: fins.collect(),
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
    let mut held: Vec<Held<'_>> = Vec::with_capacity(pieces.len());
    for (given, (piece, owner)) in pieces.iter().enumerate() {
        held.push(Held {
            sheet: *owner,
            piece,
            given,
            face: canonical(&piece.face)?,
        });
    }
    let made = corners_of(&held, &Exec::serial(), meter)?;
    let (number, points) = numbered(&made, &Exec::serial(), meter)?;
    let by_piece = sides_of(&held, &number, &Exec::serial(), meter)?;
    let each_side: Vec<&Side> = by_piece.iter().flatten().collect();
    let (order, lines) = by_line(&each_side, &Exec::serial(), meter)?;
    let sides = in_order(&each_side, &order)?;
    let mut runs: Vec<Run> = Vec::new();
    for (start, end) in lines {
        let group = sides.get(start..end).ok_or(MergeError::Open)?;
        meetings(start, group, &held, &mut runs)?;
    }
    let runs = cut_at_ends(runs, &points.places, &Exec::serial(), meter)?;
    let mut used: Vec<usize> = runs.iter().flat_map(|e| [e.low, e.high]).collect();
    used.sort_unstable();
    used.dedup();
    let vertex = |id: usize| used.binary_search(&id).map_err(|_| MergeError::Open);
    let mut edges = Vec::with_capacity(runs.len());
    let mut meets = Vec::with_capacity(runs.len());
    for run in &runs {
        edges.push((vertex(run.low)?, vertex(run.high)?));
        meets.push(run.fins.iter().map(|fin| fin.held).collect());
    }
    let mut vertices = Vec::with_capacity(used.len());
    for id in &used {
        vertices.push(named(&points, *id, meter)?);
    }
    Ok(Curves {
        vertices,
        edges,
        meets,
    })
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
pub fn merge_with(
    pieces: &[Outward],
    mode: Merge,
    rules: Rules<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Merged, MergeError> {
    merge_on(&Exec::serial(), pieces, (mode, rules), meter)
}

/// [`merge_with`], its corners made and sorted on `exec`'s pool: the same merge on any pool
/// (23.4).
///
/// # Errors
///
/// As [`merge_with`], and [`MergeError::Defect`] if the parallel helper breaks one of its rules.
#[allow(
    clippy::too_many_lines,
    reason = "the steps of one merge in their order, each short, sharing the numbers they fill"
)]
pub fn merge_on(
    exec: &Exec<'_>,
    pieces: &[Outward],
    (mode, rules): (Merge, Rules<'_>),
    meter: &mut WorkMeter<'_>,
) -> Result<Merged, MergeError> {
    let (sheets, mut held) = sheets_of(pieces, mode, exec, meter)?;
    let kept = held.len();
    for piece in rules.cuts {
        held.push(Held {
            sheet: CUT,
            piece,
            given: CUT,
            face: canonical(&piece.face)?,
        });
    }
    let corners = corners_of(&held, exec, meter)?;
    let (number, points) = numbered(&corners, exec, meter)?;
    // Each piece's sides stay in the list its task made, and are reached by reference, in piece
    // order, rather than moved into one list.
    let by_piece = sides_of(&held, &number, exec, meter)?;
    let each_side: Vec<&Side> = by_piece.iter().flatten().collect();
    let (order, lines) = by_line(&each_side, exec, meter)?;
    let sides = in_order(&each_side, &order)?;
    held.truncate(kept);

    // Each line's runs, and which pieces join across them. The lines are swept on the pool, each
    // into its own joins and runs, which are then taken in line order: the joins made and the runs
    // listed are those of one sweep after another, and the first failure is the first line's.
    let swept = exec.try_map(
        meter,
        SWEEPS,
        RUNS,
        &lines,
        |_, (start, end), _| -> Result<Swept, MergeError> {
            let group = sides.get(*start..*end).ok_or(MergeError::Open)?;
            let (mut joins, mut found) = (Vec::new(), Vec::new());
            sweep(*start, group, &sheets, &mut joins, &mut found)?;
            Ok((joins, found))
        },
    );
    let swept = swept.map_err(|e| phase_error(e, meter))?;
    let mut same_piece: Vec<usize> = (0..held.len()).collect();
    let found = swept
        .iter()
        .fold(0usize, |n, (_, found)| n.saturating_add(found.len()));
    let mut runs: Vec<Run> = Vec::with_capacity(found);
    for (joins, found) in swept {
        for (a, b) in joins {
            join(&mut same_piece, a, b);
        }
        runs.extend(found);
    }
    flatten(&mut same_piece);
    let runs = without_slits(runs, &same_piece);
    let runs = cut_at_ends(whole_runs(runs, &same_piece), &points.places, exec, meter)?;

    // Faces: the connected pieces of a sheet, numbered by the sheet and their lowest fin, which
    // only one face of a sheet can hold. Each piece's set is listed at its root, a piece's index,
    // so the faces are found in the order of their roots, as a map by root would list them.
    let mut lowest: Vec<Option<FaceKey>> = alloc::vec![None; held.len()];
    for run in &runs {
        for fin in &run.fins {
            let key = (fin.sheet, run.low, run.high, fin.rising);
            let slot = lowest
                .get_mut(root(&same_piece, fin.held))
                .ok_or(MergeError::Open)?;
            *slot = Some(slot.map_or(key, |held_key| held_key.min(key)));
        }
    }
    let mut face_list: Vec<(FaceKey, usize)> = lowest
        .iter()
        .enumerate()
        .filter_map(|(top, key)| key.map(|key| (key, top)))
        .collect();
    face_list.sort_unstable();
    let mut numbered_faces: Vec<Option<usize>> = alloc::vec![None; held.len()];
    for (at, (_, top)) in face_list.iter().enumerate() {
        *numbered_faces.get_mut(*top).ok_or(MergeError::Open)? = Some(at);
    }
    let face_of = |at: usize| {
        numbered_faces
            .get(root(&same_piece, at))
            .copied()
            .flatten()
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
    let mut fins: Vec<Fin> = Vec::with_capacity(
        runs.iter()
            .fold(0usize, |n, run| n.saturating_add(run.fins.len())),
    );
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
    // Each edge's order on the pool; the joins, and the first failure, in edge order as before.
    let orders = exec.try_map(
        meter,
        RADIAL,
        RUNS,
        &around_edge,
        |_, here, _| -> Result<Result<Vec<usize>, MergeError>, MergeError> {
            Ok(radial(here, &fins, &planes))
        },
    );
    let orders = orders.map_err(|e| phase_error(e, meter))?;
    let mut same_shell: Vec<usize> = (0..face_list.len()).collect();
    let mut edges = Vec::with_capacity(runs.len());
    for (run, order) in runs.iter().zip(orders) {
        let (low, high) = (vertex(run.low)?, vertex(run.high)?);
        let order = order?;
        join_around(&order, &fins, rules.open, &mut same_shell)?;
        edges.push(Edge {
            low,
            high,
            fins: order,
        });
    }
    flatten(&mut same_shell);
    let mut tops: Vec<usize> = (0..face_list.len()).map(|f| root(&same_shell, f)).collect();
    tops.sort_unstable();
    tops.dedup();
    let shell_of = |face: usize| {
        tops.binary_search(&root(&same_shell, face))
            .map_err(|_| MergeError::Open)
    };
    let mut shell_of_piece = Vec::with_capacity(held.len());
    let mut piece_face = alloc::vec![0usize; held.len()];
    for (at, entry) in held.iter().enumerate() {
        let face = face_of(at)?;
        shell_of_piece.push(shell_of(face)?);
        piece_face
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
        let ranges: Vec<Option<GridRange>> = corners.iter().map(|c| grid_range(c)).collect();
        nest(&held, (&shell_of_piece, tops.len()), &ranges, exec, meter)?
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
    // Each face's rings on the pool, each read in face order below, so a failure is the one a
    // walk of the faces meets first.
    let traced = exec.try_map(
        meter,
        RINGS,
        RUNS,
        &own_fins,
        |at, list, _| -> Result<Result<Vec<Vec<usize>>, MergeError>, MergeError> {
            Ok(planes
                .get(at)
                .ok_or(MergeError::Open)
                .and_then(|plane| rings(&fins, list, plane)))
        },
    );
    let traced = traced.map_err(|e| phase_error(e, meter))?;
    for (at, (((sheet, ..), _), ring)) in face_list.iter().zip(traced).enumerate() {
        let shell = shell_of(at)?;
        faces.push(Face {
            sheet: *sheet,
            shell,
            rings: ring?,
        });
        shells
            .iter_mut()
            .skip(shell)
            .take(1)
            .for_each(|s| s.faces.push(at));
    }
    let vertices = exec.try_map(meter, NAMES, RUNS, &used, |_, id, m| named(&points, *id, m));
    let vertices = vertices.map_err(|e| phase_error(e, meter))?;
    Ok(Merged {
        sheets,
        vertices,
        edges,
        fins,
        faces,
        shells,
        piece_face,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::many_single_char_names,
        clippy::unwrap_used,
        reason = "tests: native arithmetic, ratios a / b and c / d, and a failure stops the test"
    )]

    use core::cmp::Ordering;

    use num_bigint::BigInt;
    use proptest::prelude::*;

    use geom_num::plane::Plane;
    use geom_num::wide::I256;

    use geom_num::Sign;

    use super::{
        Cell, CellKey, MarkKey, REACH, corner, lead, line_cells_of, line_in_words, mark,
        point_order, wide_line_key,
    };

    /// a / b against c / d, b and d positive, by cross-multiplying big integers.
    fn exact(a: i128, b: i128, c: i128, d: i128) -> Ordering {
        (BigInt::from(a) * BigInt::from(d)).cmp(&(BigInt::from(c) * BigInt::from(b)))
    }

    fn agrees(a: i128, b: i128, c: i128, d: i128) {
        let order = Cell::new(a, b).unwrap().order(&Cell::new(c, d).unwrap());
        assert_eq!(
            order,
            Some(exact(a, b, c, d)),
            "{a} / {b} against {c} / {d}"
        );
    }

    #[test]
    fn a_cell_key_is_equal_and_ordered_as_its_order_says() {
        let cells = |a: i128| {
            [
                Cell::new(a, 3).unwrap(),
                Cell::new(1, 2).unwrap(),
                Cell::new(-a, 5).unwrap(),
            ]
        };
        for (a, b) in [(1, 1), (1, 2), (4, -2)] {
            let (p, q) = (cells(a), cells(b));
            let (one, other) = (CellKey(&p), CellKey(&q));
            assert_eq!(one == other, one.cmp(&other) == Ordering::Equal);
            assert_eq!(one.partial_cmp(&other), Some(one.cmp(&other)));
        }
    }

    proptest! {
        #[test]
        fn cells_order_as_their_ratios_when_ties_are_common(
            a in -40i128..40, b in 1i128..9, c in -40i128..40, d in 1i128..9,
        ) {
            agrees(a, b, c, d);
        }

        #[test]
        fn cells_order_as_their_ratios_across_i128(
            a in any::<i128>(), b in 1i128.., c in any::<i128>(), d in 1i128..,
        ) {
            agrees(a, b, c, d);
        }
    }

    #[test]
    fn cells_order_at_the_ends_of_i128() {
        for (a, b) in [
            (i128::MIN, 1),
            (i128::MAX, 1),
            (i128::MIN, i128::MAX),
            (-1, i128::MAX),
        ] {
            for (c, d) in [(i128::MIN, 3), (i128::MAX, 2), (0, 1), (1, i128::MAX)] {
                agrees(a, b, c, d);
            }
        }
    }

    #[test]
    fn equal_ratios_in_other_terms_are_equal() {
        for (a, b, c, d) in [
            (1, 2, 2, 4),
            (7, 2, 21, 6),
            (-3, 4, -6, 8),
            (5, 10, 1 << 70, 1 << 71),
        ] {
            agrees(a, b, c, d);
            assert_eq!(
                Cell::new(a, b).unwrap().order(&Cell::new(c, d).unwrap()),
                Some(Ordering::Equal)
            );
        }
        // Terms past i64 take the wide products.
        agrees((1 << 100) + 1, 1 << 101, (1 << 99) + 1, 1 << 100);
    }

    /// Corners too wide for machine words have no cells, and are ordered by their wide coordinates,
    /// whichever way their planes turn the weight: three axis planes with coefficients of 2^64 meet
    /// at (5, 7, 9), in two orders whose weights are ±2^192, beside a corner in words.
    #[test]
    fn corners_without_cells_order_by_wide_coordinates_with_either_weight() {
        let big = 1i128 << 64;
        let axis = |n: [i128; 3], at: i128| Plane::new(n.map(|v| v * big), -at * big).unwrap();
        let (x, y, z) = (axis([1, 0, 0], 5), axis([0, 1, 0], 7), axis([0, 0, 1], 9));
        let (up, down) = (corner(x, y, z).unwrap(), corner(x, z, y).unwrap());
        assert!(up.cells.is_none() && down.cells.is_none());
        assert_eq!(
            (up.point.weight(), down.point.weight()),
            (Sign::Positive, Sign::Negative)
        );
        assert_eq!(point_order(&up, &down), Some(Ordering::Equal));
        let unit = |n: [i128; 3], at: i128| Plane::new(n, -at).unwrap();
        let near = corner(unit([1, 0, 0], 6), unit([0, 0, 1], 9), unit([0, 1, 0], 7)).unwrap();
        assert!(near.cells.is_some());
        assert_eq!(point_order(&down, &near), Some(Ordering::Less));
        assert_eq!(point_order(&near, &down), Some(Ordering::Greater));
        assert_eq!(point_order(&near, &up), Some(Ordering::Greater));
    }

    /// A cell's lead against big integers: its floor, the floor of its fraction times 2^32, and
    /// whether that leaves a remainder.
    fn leads(v: i128, over: i128) {
        let cell = Cell::new(v, over).unwrap();
        let (floor, digits, more) = lead(&cell);
        let (v, over) = (BigInt::from(v), BigInt::from(over));
        // Floor division by a positive divisor: the quotient rounded towards zero, one less when
        // that leaves a negative remainder.
        let floor_by = |n: &BigInt| {
            let q = n / &over;
            if n - &q * &over < BigInt::from(0) {
                q - 1
            } else {
                q
            }
        };
        let whole = floor_by(&v);
        let scaled = (&v - &whole * &over) << 32u32;
        let fraction = floor_by(&scaled);
        assert_eq!(BigInt::from(floor), whole, "{v} / {over}");
        assert_eq!(BigInt::from(digits), fraction, "{v} / {over}");
        assert_eq!(more, scaled != fraction * &over, "{v} / {over}");
    }

    proptest! {
        #[test]
        fn a_lead_is_the_floor_and_the_fractions_first_32_binary_digits(
            v in any::<i128>(), over in 1i128..,
        ) {
            leads(v, over);
        }

        #[test]
        fn cells_whose_leads_differ_order_as_their_leads(
            a in any::<i128>(), b in 1i128.., c in any::<i128>(), d in 1i128..,
        ) {
            let (p, q) = (Cell::new(a, b).unwrap(), Cell::new(c, d).unwrap());
            if lead(&p) != lead(&q) {
                prop_assert_eq!(p.order(&q), Some(lead(&p).cmp(&lead(&q))));
            }
        }
    }

    #[test]
    fn leads_at_the_ends_and_on_exact_halves() {
        for (v, over) in [
            (i128::MIN, 1),
            (i128::MAX, 1),
            (i128::MIN, i128::MAX),
            (i128::MAX, i128::MAX),
            (-1, i128::MAX),
            (i128::MAX - 1, i128::MAX),
            (1, 2),
            (3, 4),
            (-1, 3),
            ((1 << 100) + 1, 1 << 101),
            (1 << 99, 1 << 100),
            (1, 1 << 70),
            (-5, 1 << 70),
            ((1 << 96) - 1, 1 << 97),
            (1 << 96, (1 << 96) + 1),
        ] {
            leads(v, over);
        }
    }

    /// Two cells' marks order as the cells do where they differ, and say the cells are equal
    /// where they agree and stop.
    fn marks_agree(p: Cell, q: Cell) {
        let (a, b) = (mark(&p), mark(&q));
        let exact = p.order(&q).unwrap();
        if a != b {
            assert_eq!(a.cmp(&b), exact, "{p:?} against {q:?}");
        } else if a & 1 == 0 {
            assert_eq!(exact, Ordering::Equal, "{p:?} against {q:?}");
        }
    }

    proptest! {
        #[test]
        fn marks_order_as_their_cells(
            a in any::<i128>(), b in 1i128.., c in any::<i128>(), d in 1i128..,
        ) {
            marks_agree(Cell::new(a, b).unwrap(), Cell::new(c, d).unwrap());
        }

        #[test]
        fn marks_order_as_their_cells_when_ties_are_common(
            a in -40i128..40, b in 1i128..9, c in -40i128..40, d in 1i128..9,
        ) {
            marks_agree(Cell::new(a, b).unwrap(), Cell::new(c, d).unwrap());
        }
    }

    #[test]
    fn floors_past_reach_mark_below_or_above_every_other() {
        let cell = |v: i128, over: i128| Cell::new(v, over).unwrap();
        let low = REACH.wrapping_neg();
        for (v, over, want) in [
            (low - 1, 1, 1),
            (low * 3 - 1, 2, 1),
            (i128::MIN, 1, 1),
            (REACH, 1, u128::MAX),
            (i128::MAX, 3, u128::MAX),
        ] {
            assert_eq!(mark(&cell(v, over)), want, "{v} / {over}");
        }
        // The edges of reach have marks of their own.
        let (bottom, top) = (mark(&cell(low, 1)), mark(&cell(REACH - 1, 1)));
        assert_eq!((bottom & 1, top & 1), (0, 0));
        assert!(1 < bottom && bottom < top && top < u128::MAX);
        for (p, q) in [
            (cell(low - 1, 1), cell(low, 1)),
            (cell(REACH - 1, 1), cell(REACH, 1)),
            (cell(i128::MIN, 1), cell(low - 7, 1)),
            (cell(i128::MAX, 1), cell(REACH, 1)),
            (cell(REACH * 2 + 1, 2), cell(REACH * 2, 2)),
        ] {
            marks_agree(p, q);
            marks_agree(q, p);
        }
    }

    /// A key of marks orders as the cells it holds, from `from` on.
    fn keys_agree(p: &[Cell; 3], q: &[Cell; 3]) {
        let (one, other) = (MarkKey::<3, 3>::new(p, 0), MarkKey::<3, 3>::new(q, 0));
        let exact = CellKey(p).cmp(&CellKey(q));
        assert_eq!(one.cmp(&other), exact, "{p:?} against {q:?}");
        assert_eq!(one == other, exact == Ordering::Equal);
        assert_eq!(one.partial_cmp(&other), Some(exact));
    }

    proptest! {
        #[test]
        fn a_key_of_marks_orders_as_its_cells(
            p in [(-6i128..6, 1i128..4), (-6i128..6, 1i128..4), (-6i128..6, 1i128..4)],
            q in [(-6i128..6, 1i128..4), (-6i128..6, 1i128..4), (-6i128..6, 1i128..4)],
        ) {
            let cells = |c: [(i128, i128); 3]| c.map(|(v, over)| Cell::new(v, over).unwrap());
            keys_agree(&cells(p), &cells(q));
        }
    }

    #[test]
    fn a_key_reads_its_cells_where_marks_agree_and_run_on() {
        let cell = |v: i128, over: i128| Cell::new(v, over).unwrap();
        // 1 / 3 and (2^33 + 1) / (3 · 2^33 + 2) agree on 32 digits and both run on; so do two
        // floors past reach.
        let third = cell(1, 3);
        let near = cell((1 << 33) + 1, 3 * (1 << 33) + 2);
        assert_eq!(mark(&third), mark(&near));
        assert_eq!(mark(&third) & 1, 1);
        let far = cell(i128::MAX, 1);
        for (p, q) in [
            ([third, third, cell(1, 1)], [near, third, cell(0, 1)]),
            ([cell(0, 1), third, third], [cell(0, 1), near, far]),
            (
                [far, cell(0, 1), cell(0, 1)],
                [cell(i128::MAX - 1, 1), cell(0, 1), cell(0, 1)],
            ),
            ([cell(2, 4), third, far], [cell(1, 2), third, far]),
        ] {
            keys_agree(&p, &q);
            keys_agree(&q, &p);
        }
        // Marks from past the first cell, and past the end, which mark 0.
        let line = [cell(0, 5), cell(5, 5), third, near, far, cell(7, 5)];
        let key = MarkKey::<6, 5>::new(&line, 2);
        assert_eq!(
            key.marks[..4],
            [mark(&third), mark(&near), u128::MAX, mark(&cell(7, 5))]
        );
        assert_eq!(key.marks[4], 0);
        assert!(!key.past);
        // Two marks of a line leave its cells past them to compare, where both agree and stop.
        let (one, two) = (cell(1, 2), cell(3, 4));
        let lines = [
            [cell(1, 1), one, two, one, two, cell(0, 1)],
            [cell(1, 1), one, two, one, two, cell(1, 1)],
            [cell(1, 1), one, two, two, one, cell(0, 1)],
        ];
        let keys = lines.each_ref().map(|line| MarkKey::<6, 2>::new(line, 1));
        assert!(keys.iter().all(|key| key.past));
        assert_eq!(keys[0].marks, keys[1].marks);
        for (a, b) in [(0, 1), (1, 2), (0, 2), (2, 2)] {
            let (p, q) = (&lines[a], &lines[b]);
            assert_eq!(keys[a].cmp(&keys[b]), CellKey(p).cmp(&CellKey(q)));
            assert_eq!(keys[b].cmp(&keys[a]), CellKey(q).cmp(&CellKey(p)));
        }
        assert!(!MarkKey::<6, 2>::new(&lines[0], 4).past);
    }

    /// A cell's floor and remainder are the Euclidean ones, in machine words or not.
    fn splits(v: i128, over: i128) {
        let cell = Cell::new(v, over).unwrap();
        assert_eq!(
            (cell.floor, cell.rest, cell.over),
            (v.div_euclid(over), v.rem_euclid(over), over),
            "{v} / {over}"
        );
    }

    proptest! {
        #[test]
        fn a_cell_is_the_euclidean_floor_and_remainder(v in any::<i128>(), over in 1i128..) {
            splits(v, over);
        }

        #[test]
        fn a_cell_in_words_is_the_euclidean_floor_and_remainder(
            v in any::<i64>(), over in 1i64..,
        ) {
            splits(i128::from(v), i128::from(over));
        }
    }

    #[test]
    fn cells_at_the_ends_of_words() {
        for v in [
            i128::MIN,
            i128::from(i64::MIN) - 1,
            i128::from(i64::MIN),
            -7,
            -1,
            0,
            1,
            7,
        ] {
            for over in [
                1,
                2,
                3,
                i128::from(i64::MAX),
                i128::from(i64::MAX) + 1,
                i128::MAX,
            ] {
                splits(v, over);
                splits(-v.max(-i128::MAX), over);
            }
        }
    }

    #[test]
    fn a_divisor_that_is_not_positive_has_no_cell() {
        assert!(Cell::new(1, 0).is_none());
        assert!(Cell::new(1, -2).is_none());
    }

    /// The machine-word line is the wide one, and is refused only where the wide one is.
    fn same_line(p: ([i64; 3], i64), q: ([i64; 3], i64)) {
        let plane = |(n, d): ([i64; 3], i64)| Plane::new(n.map(i128::from), i128::from(d));
        let (Ok(p), Ok(q)) = (plane(p), plane(q)) else {
            return;
        };
        let wide = wide_line_key(&p, &q);
        match line_in_words(&p, &q) {
            Some((coordinates, first)) => {
                let wide = wide.unwrap();
                assert_eq!(Some(coordinates.map(I256::from_i128)), wide.coordinates());
                assert_eq!(first, wide.first);
                assert_eq!(
                    wide.cells().is_some(),
                    line_cells_of(&coordinates, first).is_some()
                );
            }
            None => assert!(wide.is_err()),
        }
    }

    proptest! {
        #[test]
        fn a_line_in_words_is_the_wide_line(
            p in ([-9i64..9, -9i64..9, -9i64..9], -99i64..99),
            q in ([-9i64..9, -9i64..9, -9i64..9], -99i64..99),
        ) {
            same_line(p, q);
        }

        #[test]
        fn a_line_in_words_is_the_wide_line_across_i64(
            p in (any::<[i64; 3]>(), any::<i64>()),
            q in (any::<[i64; 3]>(), any::<i64>()),
        ) {
            same_line(p, q);
        }
    }

    #[test]
    fn a_line_at_the_ends_of_i64_stays_in_words() {
        let (low, high) = (i64::MIN, i64::MAX);
        let ends = [low, low + 1, -1, 0, 1, high - 1, high];
        for (a, b, c) in ends.iter().flat_map(|a| ends.map(|b| (*a, b, b))) {
            for (p, q) in [
                (([a, b, c], a), ([b, a, c], c)),
                (([a, 0, 0], a), ([b, c, 0], b)),
                (([a, b, 0], c), ([c, a, b], a)),
            ] {
                same_line(p, q);
            }
        }
        // The largest minor: (−2^63)(−2^63) − (−2^63)(2^63 − 1) = 2^127 − 2^63.
        let p = Plane::new([i128::from(low), 0, 0], i128::from(low)).unwrap();
        let q = Plane::new([i128::from(low), i128::from(low), 0], i128::from(high)).unwrap();
        let (coordinates, first) = line_in_words(&p, &q).unwrap();
        assert_eq!(first, 2);
        assert_eq!(coordinates.get(3), Some(&(i128::MAX - (1 << 63) + 1)));
        same_line(([low, 0, 0], low), ([low, low, 0], high));
    }
}
