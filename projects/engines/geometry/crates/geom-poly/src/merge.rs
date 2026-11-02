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

use geom_math::gcd_u128;
use geom_num::plane::{Plane, Tpi, normals_sign};
use geom_num::{I256, I320, NumError, Sign, Wide};
use geom_run::{ChunkPlan, Exec, Phase, RunError, Stopped, WorkMeter};

use crate::convex::{Piece, opposite};
use crate::order::{Flap, Line, Meeting, Ray, about, around, plane_words, wide_product};
use crate::plane::canonical;
use crate::subdivide::around_on;
use crate::trace::{Classified, Polygon, TraceError, classify};
use crate::winding::{Entry, Kept, Winding, WindingTest, kept};

/// Making corners or sides, a few dozen pieces a task.
const PIECES: ChunkPlan = ChunkPlan::fixed::<64>();

/// The sorts of corners and sides, in runs of this many.
const SORT_PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// Making each piece's corners.
const CORNERS: Phase = Phase::new("merge: corners");

/// Sorting the corners.
const SORT_CORNERS: Phase = Phase::new("merge: sort corners");

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
}

/// A corner of a piece.
struct Corner {
    point: Tpi,
    /// Each coordinate `X_i` / w, w made positive, as a [`Cell`], where X and w fit machine
    /// words; the wide coordinates are made only to compare a corner that has none.
    cells: Option<[Cell; 3]>,
    /// The planes of every corner at this point: its own, until [`numbered`] joins them.
    through: Vec<Plane>,
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

/// A ratio v / over, over positive, as its floor and its remainder in [0, over), the remainder
/// and divisor in lowest terms: two ratios compare by their floors, a zero remainder settles a
/// tie, equal fractions are equal terms, and only then do their cross products decide, in `i128`
/// where the terms fit `i64`, else [`narrow_order`]'s. A corner of a building's faces on the grid
/// has every remainder zero, one on a box's split a half, and so do most of a line's coordinates
/// over a short direction.
#[derive(Clone, Copy, Debug)]
struct Cell {
    floor: i128,
    rest: i128,
    over: i128,
}

impl Cell {
    /// v / `over` split, or `None` unless `over` is positive.
    fn new(v: i128, over: i128) -> Option<Self> {
        if over <= 0 {
            return None;
        }
        let rest = v.checked_rem_euclid(over)?;
        // Both are positive or the rest zero, so the divisor is at least 1.
        let common = i128::try_from(gcd_u128(rest.unsigned_abs(), over.unsigned_abs())).ok()?;
        Some(Cell {
            floor: v.checked_div_euclid(over)?,
            rest: rest.checked_div(common)?,
            over: over.checked_div(common)?,
        })
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

/// A corner's place in [`point_order`], or a line's after its `first` in [`line_order`], where it
/// has [`Cell`]s, for a sort on the pool: comparing cells cannot fail, as each product they need
/// is of two `i128`s, below 2^254 in [`I256`]. It borrows the cells, as the sort makes a key for
/// every comparison.
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

/// A parallel phase's error as the merge's: the item's own, or the meter's stop, which the helper
/// latches; a stop it did not latch is the helper's defect.
fn phase_error(error: RunError<MergeError>, meter: &mut WorkMeter<'_>) -> MergeError {
    match error {
        RunError::Failed(e) => e,
        _ => meter
            .poll()
            .err()
            .map_or(MergeError::Defect, MergeError::Stopped),
    }
}

/// A parallel arrangement step's error as the merge's, as [`phase_error`] reads the merge's own.
fn traced(error: RunError<TraceError>, meter: &mut WorkMeter<'_>) -> MergeError {
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
    Ok(Corner {
        point,
        cells,
        through: alloc::vec![face, p, q],
    })
}

/// The Plücker coordinates of a line: its direction, then its moment, the direction's first
/// non-zero component positive. Below 2^209, in [`I256`].
#[derive(Clone, Copy, Debug)]
struct LineKey {
    coordinates: [I256; 6],
    first: usize,
    /// Each coordinate over the direction's first non-zero component as a [`Cell`], where all
    /// six fit machine words.
    cells: Option<[Cell; 6]>,
}

/// The line where `p` and `q` meet: direction `n_p` × `n_q`, moment `d_p` `n_q` − `d_q` `n_p`, which
/// is the same up to scale for any two planes through the line.
fn line_key(p: &Plane, q: &Plane) -> Result<LineKey, NumError> {
    match line_in_words(p, q) {
        Some((coordinates, first)) => Ok(LineKey {
            cells: line_cells_of(&coordinates, first),
            coordinates: coordinates.map(I256::from_i128),
            first,
        }),
        None => wide_line_key(p, q),
    }
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
    if coordinates.get(first).is_some_and(I256::is_negative) {
        for slot in &mut coordinates {
            *slot = slot.checked_neg().ok_or(NumError::BudgetExceeded)?;
        }
    }
    Ok(LineKey {
        cells: line_cells(&coordinates, first),
        coordinates,
        first,
    })
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
    if coordinates.get(first).is_some_and(|v| *v < 0) {
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
    let over = a.first;
    for i in 0..6 {
        let order = if let (Some(p), Some(q)) = (&a.cells, &b.cells) {
            p.get(i)?.order(q.get(i)?)?
        } else {
            let (p, q) = (&a.coordinates, &b.coordinates);
            ratio_order::<4, 4, 8>(p.get(i)?, p.get(over)?, q.get(i)?, q.get(over)?)?
        };
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
/// lies along it, and the rest are runs.
fn sweep(
    line: usize,
    sides: &[&Side],
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
    let made = exec.try_map(meter, SHEETS, PIECES, pieces, |_, piece, _| {
        let face = canonical(&piece.piece.face)?;
        let pairs = |w: &Winding| {
            if seams {
                w.pairs().to_vec()
            } else {
                Vec::new()
            }
        };
        Ok::<SheetKey, MergeError>((
            (face.plane.d(), face.plane.n()),
            face.flipped,
            if seams { piece.source } else { 0 },
            pairs(&piece.front),
            pairs(&piece.back),
        ))
    });
    let keys = made.map_err(|e| phase_error(e, meter))?;
    // Equal keys by the sheet's source faces as the result does where one can, the lowest first
    // (11.5); the stable sort keeps the input's order past that, as the serial sort did.
    let keyed: Vec<(&SheetKey, (bool, usize))> = keys
        .iter()
        .zip(pieces)
        .map(|(key, piece)| (key, (piece.turned, piece.polygon)))
        .collect();
    let sorted = exec.sort_stable(meter, SORT_SHEETS, SORT_PLAN, &keyed, |(key, source)| {
        (*key, *source)
    });
    let order = sorted.map_err(|e| phase_error(e, meter))?;
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut held = Vec::with_capacity(order.len());
    let mut last: Option<&SheetKey> = None;
    for at in order {
        let outward = pieces.get(at).ok_or(MergeError::Open)?;
        let key = keys.get(at).ok_or(MergeError::Open)?;
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
        });
    }
    Ok((sheets, held))
}

/// Every corner of `held`, with each corner's number, in piece order, and the list of the distinct
/// points in order.
fn numbered(
    held: &[Held<'_>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<usize>, Vec<Corner>), MergeError> {
    let made = exec.try_map(meter, CORNERS, PIECES, held, |_, piece, m| {
        let after = piece.piece.bounds.iter().cycle().skip(1);
        let mut out = Vec::with_capacity(piece.piece.bounds.len());
        for (bound, next) in piece.piece.bounds.iter().zip(after) {
            m.charge(1)?;
            out.push(corner(piece.piece.face, *bound, *next)?);
        }
        Ok::<_, MergeError>(out)
    });
    let made = made.map_err(|e| phase_error(e, meter))?;
    let mut corners: Vec<Option<Corner>> = made.into_iter().flatten().map(Some).collect();
    let keys: Option<Vec<&[Cell; 3]>> = corners
        .iter()
        .map(|c| c.as_ref().and_then(|c| c.cells.as_ref()))
        .collect();
    let order = if let Some(keys) = keys {
        // Equal points keep their order, as the serial stable sort keeps it.
        let sorted = exec.sort_stable(meter, SORT_CORNERS, SORT_PLAN, &keys, |k| CellKey(k));
        sorted.map_err(|e| phase_error(e, meter))?
    } else {
        let mut order: Vec<usize> = (0..corners.len()).collect();
        let mut failed = false;
        let at = |i: &usize| corners.get(*i).and_then(Option::as_ref);
        order.sort_by(|a, b| {
            let found = at(a).zip(at(b)).and_then(|(p, q)| point_order(p, q));
            noted(found, &mut failed)
        });
        (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
        order
    };
    let mut number = alloc::vec![0usize; corners.len()];
    let mut places: Vec<Corner> = Vec::new();
    for i in order {
        let held = corners.get_mut(i).and_then(Option::take);
        let held = held.ok_or(MergeError::Open)?;
        match places.last_mut() {
            Some(last) if point_order(last, &held) == Some(Ordering::Equal) => {
                last.through.extend(held.through);
            }
            _ => places.push(held),
        }
        number
            .iter_mut()
            .skip(i)
            .take(1)
            .for_each(|slot| *slot = places.len().saturating_sub(1));
    }
    Ok((number, places))
}

/// Whether `plane` is a split of [`crate::subdivide`]'s, on an odd half step.
fn splits_boxes(plane: &Plane) -> bool {
    let n = plane.n();
    n.iter().filter(|v| **v != 0).count() == 1
        && n.iter().any(|v| v.unsigned_abs() == 2)
        && plane.d() & 1 != 0
}

/// `corner`'s point under a name the boxes cannot change: the first three planes through it, in
/// plane order and leaving out the boxes' own, that meet in one point. Which corners a point has
/// depends on how the boxes fell, but which faces pass through it does not. A point with no such
/// three, which only a seam has, keeps its corner's.
fn named(corner: &Corner, meter: &mut WorkMeter<'_>) -> Result<Tpi, MergeError> {
    let mut list: Vec<Plane> = corner
        .through
        .iter()
        .filter(|plane| !splits_boxes(plane))
        .copied()
        .collect();
    list.sort_unstable_by_key(|plane| (plane.n(), plane.d()));
    list.dedup();
    for (i, a) in list.iter().enumerate() {
        for (j, b) in list.iter().enumerate().skip(i.saturating_add(1)) {
            for c in list.iter().skip(j.saturating_add(1)) {
                meter.charge(1)?;
                if normals_sign(a, b, c)? != Sign::Zero {
                    return Ok(Tpi::meeting(*a, *b, *c)?);
                }
            }
        }
    }
    Ok(corner.point)
}

/// Every side of `held`, sorted by line: made on `exec`'s pool, and sorted there where every side
/// has its [`Cell`]s.
fn sides_of(
    held: &[Held<'_>],
    number: &[usize],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Side>, Vec<usize>), MergeError> {
    // Each piece's first corner's place in `number`.
    let mut bases = Vec::with_capacity(held.len());
    let mut base = 0usize;
    for entry in held {
        bases.push(base);
        base = base.saturating_add(entry.piece.bounds.len());
    }
    let made = exec.try_map(meter, SIDES, PIECES, held, |at, entry, m| {
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
        Ok::<_, MergeError>(out)
    });
    let made = made.map_err(|e| phase_error(e, meter))?;
    let sides: Vec<Side> = made.into_iter().flatten().collect();
    let keys: Option<Vec<(usize, &[Cell; 6])>> = sides
        .iter()
        .map(|s| s.key.cells.as_ref().map(|cells| (s.key.first, cells)))
        .collect();
    if let Some(keys) = keys {
        // Sides on one line keep their order, as the serial stable sort keeps it.
        let sorted = exec.sort_stable(meter, SORT_SIDES, SORT_PLAN, &keys, |(first, cells)| {
            (*first, CellKey(cells))
        });
        let order = sorted.map_err(|e| phase_error(e, meter))?;
        return Ok((sides, order));
    }
    let mut order: Vec<usize> = (0..sides.len()).collect();
    let mut failed = false;
    let key = |i: &usize| sides.get(*i).map(|side| &side.key);
    order.sort_by(|a, b| {
        let found = key(a).zip(key(b)).and_then(|(p, q)| line_order(p, q));
        noted(found, &mut failed)
    });
    (!failed).then_some(()).ok_or(NumError::BudgetExceeded)?;
    Ok((sides, order))
}

/// The sides in `order`, as references, so that sorting them moves none.
fn in_order<'s>(sides: &'s [Side], order: &[usize]) -> Result<Vec<&'s Side>, MergeError> {
    order
        .iter()
        .map(|at| sides.get(*at).ok_or(MergeError::Open))
        .collect()
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

/// The whole parts of a corner's coordinates, where it has [`Cell`]s: it lies in the unit cell from
/// them.
fn floors(corner: &Corner) -> Option<[i128; 3]> {
    corner.cells.map(|[x, y, z]| [x.floor, y.floor, z.floor])
}

/// Whether a corner whose floors are `at` lies outside the cells from `a` to `b`: a point of the
/// segment between two corners lies within their floors' range and a unit more.
fn beyond(at: &[i128; 3], (a, b): (&[i128; 3], &[i128; 3])) -> bool {
    at.iter()
        .zip(a.iter().zip(b))
        .any(|(v, (p, q))| v < &(*p).min(*q) || v > &(*p).max(*q))
}

/// `runs`, each cut wherever another run ends on its line. A run's line holds only points between
/// its ends' cells, so a point outside them is not read.
fn cut_at_ends(
    runs: &[Run],
    places: &[Corner],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Run>, MergeError> {
    let mut ends: Vec<usize> = runs.iter().flat_map(|r| [r.low, r.high]).collect();
    ends.sort_unstable();
    ends.dedup();
    let cells: Vec<Option<[i128; 3]>> = places.iter().map(floors).collect();
    let made = exec.try_map(meter, ENDS, RUNS, runs, |_, run, _| {
        Ok::<_, MergeError>(cut_run(run, (&ends, &cells), places))
    });
    let made = made.map_err(|e| phase_error(e, meter))?;
    // Each run's parts, in run order, and the first run's failure as a serial walk meets it.
    let mut edges: Vec<Run> = Vec::with_capacity(runs.len());
    for parts in made {
        edges.extend(parts?);
    }
    Ok(edges)
}

/// `run`, cut wherever one of the sorted `ends` lies on its line between its own: those numbered
/// between its ends, found by binary search, whose `cells` fall within its ends' cells.
fn cut_run(
    run: &Run,
    (ends, cells): (&[usize], &[Option<[i128; 3]>]),
    places: &[Corner],
) -> Result<Vec<Run>, MergeError> {
    let [p, q] = run.on;
    let mut low = run.low;
    let span = cells.get(run.low).zip(cells.get(run.high));
    let span = span.and_then(|(a, b)| a.as_ref().zip(b.as_ref()));
    let from = ends.partition_point(|e| *e <= run.low);
    let to = ends.partition_point(|e| *e < run.high);
    let mut parts = Vec::new();
    for mark in ends.get(from..to).unwrap_or_default() {
        let outside = span
            .zip(cells.get(*mark).and_then(Option::as_ref))
            .is_some_and(|(span, at)| beyond(at, span));
        if outside {
            continue;
        }
        let place = places.get(*mark).ok_or(MergeError::Open)?;
        let side = |plane: &Plane| plane.side_of_tpi(&place.point);
        if side(&p)? == Sign::Zero && side(&q)? == Sign::Zero {
            parts.push(Run {
                low,
                high: *mark,
                ..run.clone()
            });
            low = *mark;
        }
    }
    parts.push(Run { low, ..run.clone() });
    Ok(parts)
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

/// `pieces` as polygons of one shell, each with `entry`.
fn polygons_of(pieces: &[&Piece], entry: Option<Entry>) -> Vec<Polygon> {
    pieces
        .iter()
        .map(|piece| Polygon {
            piece: (*piece).clone(),
            entry,
        })
        .collect()
}

/// The vectors in front of and behind the first of `polygons`, traced through all of them.
fn sides_of_first(
    polygons: &[Polygon],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Winding, Winding), MergeError> {
    let cell = around_on(exec, polygons, meter).map_err(|e| traced(e, meter))?;
    let lists: Vec<Vec<Piece>> = polygons
        .iter()
        .take(1)
        .map(|polygon| alloc::vec![polygon.piece.clone()])
        .collect();
    let found = classify(&cell, &Winding::new(), polygons, &lists, meter)?;
    let first = found.into_iter().next();
    Ok(first.map_or_else(|| (Winding::new(), Winding::new()), |c| (c.front, c.back)))
}

/// Which of the shells `among` lie around a point behind shell `at`.
fn around_shell(
    of_shell: &[Vec<&Piece>],
    (at, among): (usize, &[usize]),
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<usize>, MergeError> {
    let number = |shell: usize| u32::try_from(shell).unwrap_or(u32::MAX);
    let list = of_shell.get(at).ok_or(MergeError::Open)?;
    let mut polygons = polygons_of(list, None);
    for shell in among.iter().filter(|shell| **shell != at) {
        let others = of_shell.get(*shell).ok_or(MergeError::Open)?;
        polygons.extend(polygons_of(others, Some(number(*shell))));
    }
    let (_, back) = sides_of_first(&polygons, exec, meter)?;
    Ok(among
        .iter()
        .copied()
        .filter(|shell| *shell != at && back.count(number(*shell)) > 0)
        .collect())
}

/// Which shells face out, and the outer shell around each cavity. A shell faces out if the point in
/// front of it is outside it, which takes only its own pieces, so this is linear in the pieces
/// when there are no cavities. A cavity is held by the innermost outer shell around a point behind
/// it: behind a cavity lies the material it is a cavity of, even where another cell fills it back
/// to back.
fn nest(
    held: &[Held<'_>],
    (shell_of_piece, shells): (&[usize], usize),
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(bool, Option<usize>)>, MergeError> {
    let mut of_shell: Vec<Vec<&Piece>> = alloc::vec![Vec::new(); shells];
    for (entry, shell) in held.iter().zip(shell_of_piece) {
        of_shell
            .iter_mut()
            .skip(*shell)
            .take(1)
            .for_each(|list| list.push(entry.piece));
    }
    let mut outer = Vec::with_capacity(shells);
    for list in &of_shell {
        meter.charge(1)?;
        let (front, _) = sides_of_first(&polygons_of(list, Some(0)), exec, meter)?;
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
    let held: Vec<Held<'_>> = pieces
        .iter()
        .enumerate()
        .map(|(given, (piece, owner))| Held {
            sheet: *owner,
            piece,
            given,
        })
        .collect();
    let (number, points) = numbered(&held, &Exec::serial(), meter)?;
    let (each_side, by_line) = sides_of(&held, &number, &Exec::serial(), meter)?;
    let sides = in_order(&each_side, &by_line)?;
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
    let runs = cut_at_ends(&runs, &points, &Exec::serial(), meter)?;
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
        vertices.push(named(points.get(*id).ok_or(MergeError::Open)?, meter)?);
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
    held.extend(rules.cuts.iter().map(|piece| Held {
        sheet: CUT,
        piece,
        given: CUT,
    }));
    let (number, points) = numbered(&held, exec, meter)?;
    let (each_side, by_line) = sides_of(&held, &number, exec, meter)?;
    let sides = in_order(&each_side, &by_line)?;
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
    let runs = cut_at_ends(&whole_runs(runs, &same_piece), &points, exec, meter)?;

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
    // Each edge's order on the pool; the joins, and the first failure, in edge order as before.
    let orders = exec.try_map(meter, RADIAL, RUNS, &around_edge, |_, here, _| {
        Ok::<_, MergeError>(radial(here, &fins, &planes))
    });
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
        nest(&held, (&shell_of_piece, tops.len()), exec, meter)?
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
    let traced = exec.try_map(meter, RINGS, RUNS, &own_fins, |at, list, _| {
        Ok::<_, MergeError>(
            planes
                .get(at)
                .ok_or(MergeError::Open)
                .and_then(|plane| rings(&fins, list, plane)),
        )
    });
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
    let vertices = exec.try_map(meter, NAMES, RUNS, &used, |_, id, m| {
        named(points.get(*id).ok_or(MergeError::Open)?, m)
    });
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

    use super::{Cell, line_in_words, wide_line_key};

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
        // Terms past i64 after reduction take the wide products.
        agrees((1 << 100) + 1, 1 << 101, (1 << 99) + 1, 1 << 100);
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
                assert_eq!(coordinates.map(I256::from_i128), wide.coordinates);
                assert_eq!(first, wide.first);
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
