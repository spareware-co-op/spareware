//! Tracing: each piece's front and back winding vectors, step 3 of 11.3.
//!
//! What it does: [`classify`] takes the polygons of one box, the box and the winding vector at its
//! reference point, and gives every piece of those polygons its two vectors. The vector just off a
//! piece comes from a path to the reference point, adding up the signed crossings of the polygons
//! on the way. No piece is crossed by another (11.3, step 2), so one path answers for a whole piece.
//!
//! The path: it starts at a corner of the piece, moved in along both of the corner's edges and off
//! the face, and every segment runs along a line where two planes meet, so that every crossing is
//! a three-plane point and every test is the ordinary classify of 6.2, however long the path. First
//! along the line of the piece's edge to the first face of the box, then along the piece's plane to
//! the first edge of the box, then along the box's edges to its reference corner: five segments at
//! most. EMBER's paths take three because its segments may leave the box; the box's faces are the
//! only planes known to stay in it, and a crossing outside the box would meet no polygon of it.
//!
//! Guarantees:
//!
//! - **Exact, within 6.2's budget.** Every sign is a 4 × 4 determinant of plane coefficients, at
//!   most 384 bits, or a 3 × 3 determinant of normals, at most 252. Nothing is rounded, and the
//!   path's points are never computed for a sign. A segment and a polygon whose grid cells, the
//!   floors of their corners a step wider each way, do not meet share no point, so the segment
//!   cannot cross it, and the pair is passed over: a filter that never decides a crossing.
//! - **Ties are settled by moving the path, never by a tolerance.** Every plane of the path is
//!   moved by its own infinitesimal, in a fixed order, so the path misses every edge and corner of
//!   every polygon, and each sign that would be zero is decided by the determinant's first term in
//!   those infinitesimals (Edelsbrunner and Mücke's Simulation of Simplicity, in the order of
//!   `Rank`). Real ties, faces on one plane, are counted, not moved apart: a piece's two vectors
//!   differ by every coincident face over it, whichever way that face looks (11.5).
//! - **One emission of an overlap.** Where polygons on one plane overlap, [`Classified::first`]
//!   marks the copy of the lowest polygon in the order given, which a caller makes
//!   (operand key, name).
//! - **Cutters change nothing.** A polygon with no entry cuts but counts no crossing (11.1).
//! - **No recursion, the same answer every time.** Polygons and pieces are taken in the order
//!   given; nothing depends on addresses or threads.
//!
//! Method: chapter 11.3's "Classification" (`projects/engines/geometry/architecture/11-booleans.md`),
//! after EMBER's section 4.4 (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>); ties after Edelsbrunner and Mücke 1990,
//! <https://doi.org/10.1145/77635.77639>.
//!
//! Cost: at most five segments per piece, each testing each polygon: one or two sign evaluations,
//! and one per bound of a polygon the segment's line crosses. The last three run along the box's
//! edges and are the same for every piece of a polygon that leaves the box by the same faces, so
//! they are walked once for each. A pair whose spans miss costs a few comparisons. Charges the
//! meter one unit per polygon a segment tests, walked, passed over or not.

use alloc::vec::Vec;
use core::fmt;

use geom_num::plane::{Plane, determinant_sign};
use geom_num::{NumError, Sign};
use geom_run::{Stopped, WorkMeter};

use crate::convex::{ConvexError, Piece};
use crate::order::{Meeting, normals_det};
use crate::plane::{Canonical, canonical};
use crate::winding::{Entry, Winding, WindingError};

/// A polygon of a box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polygon {
    /// The polygon, its face's normal pointing out of the material.
    pub piece: Piece,
    /// The entry it bounds, or `None` for a cutter, which counts no crossing.
    pub entry: Option<Entry>,
}

/// A box of the subdivision, in half steps, with its reference point at the low corner, moved
/// just inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The low corner.
    pub low: [i64; 3],
    /// The high corner.
    pub high: [i64; 3],
}

/// A piece with its vectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classified {
    /// The index of the polygon it is a piece of.
    pub polygon: usize,
    /// The piece.
    pub piece: Piece,
    /// The vector in front of it.
    pub front: Winding,
    /// The vector behind it.
    pub back: Winding,
    /// Whether no earlier polygon on its plane covers it: the copy of an overlap to emit.
    pub first: bool,
}

/// Why a box could not be classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceError {
    /// The numbers layer refused: a plane over budget, or a piece that is no polygon.
    Num(NumError),
    /// A winding vector could not hold a crossing.
    Winding(WindingError),
    /// The meter stopped the run.
    Stopped(Stopped),
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::Num(e) => write!(f, "a box that cannot be traced: {e:?}"),
            TraceError::Winding(e) => write!(f, "{e}"),
            TraceError::Stopped(_) => f.write_str("the meter stopped the trace"),
        }
    }
}

impl core::error::Error for TraceError {}

impl From<NumError> for TraceError {
    fn from(e: NumError) -> Self {
        TraceError::Num(e)
    }
}

impl From<ConvexError> for TraceError {
    fn from(e: ConvexError) -> Self {
        match e {
            ConvexError::Num(e) => TraceError::Num(e),
            ConvexError::Stopped(e) => TraceError::Stopped(e),
        }
    }
}

impl From<WindingError> for TraceError {
    fn from(e: WindingError) -> Self {
        TraceError::Winding(e)
    }
}

impl From<Stopped> for TraceError {
    fn from(e: Stopped) -> Self {
        TraceError::Stopped(e)
    }
}

/// How far a plane of the path moves, largest first: a corner moves in along its edges, a good way
/// more than it moves off the face, and off the face a good way more than the box's faces move in,
/// so the start of a path lies inside the moved box even on a piece that lies in a face of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// The corner's first edge.
    First,
    /// The corner's second edge.
    Second,
    /// The piece's plane.
    Face,
    /// A face of the box, by axis and end.
    Box(usize, usize),
    /// Not moved.
    Fixed,
}

/// A plane, moved by an infinitesimal: the points where it reads `by` times its ε.
#[derive(Clone, Copy, Debug)]
struct Moved {
    plane: Plane,
    rank: Rank,
    by: Sign,
}

impl Moved {
    const fn fixed(plane: Plane) -> Self {
        Moved {
            plane,
            rank: Rank::Fixed,
            by: Sign::Zero,
        }
    }
}

/// The sign of the 4 × 4 determinant of the four planes' coefficients, each moved. The determinant
/// is linear in each offset, and the moves are of different orders, so its sign is that of the
/// unmoved determinant, or failing that of the largest move with a non-zero cofactor. A plane's
/// reading of `by` ε subtracts `by` ε from its offset. `at`, where `p`, `q` and `r` meet in
/// machine words, reads the unmoved determinant in four products if it has it.
#[allow(
    clippy::many_single_char_names,
    reason = "p, q, r and s are the moved planes, and a, b, c and d the same planes unmoved"
)]
fn moved_determinant(
    (p, q, r): (&Moved, &Moved, &Moved),
    s: &Moved,
    at: Option<&Meeting>,
) -> Result<Sign, NumError> {
    let [a, b, c, d] = [&p.plane, &q.plane, &r.plane, &s.plane];
    let base = match at.and_then(|point| point.reading(d)) {
        Some(sign) => sign,
        None => determinant_sign(a, b, c, d)?,
    };
    if base != Sign::Zero {
        return Ok(base);
    }
    // Rows p, q, r, s and the offset's column give the signs −, +, −, + of the cofactors.
    let mut rows = [
        (p, normals_det(b, c, d)?),
        (q, normals_det(a, c, d)?.negate()),
        (r, normals_det(a, b, d)?),
        (s, normals_det(a, b, c)?.negate()),
    ];
    rows.sort_unstable_by_key(|(row, _)| row.rank);
    Ok(rows
        .iter()
        .map(|(row, term)| row.by.mul(*term))
        .find(|sign| *sign != Sign::Zero)
        .unwrap_or(Sign::Zero))
}

/// What plane `s` reads at the point where `p`, `q` and `r` meet, which the callers know they do.
/// The reading of a homogeneous point is n · X + d · w over w, and n · X + d · w is the determinant
/// (6.3).
fn read(p: &Moved, q: &Moved, r: &Moved, s: &Moved) -> Result<Sign, NumError> {
    let end = End::new(p, q, r)?;
    Ok(moved_determinant((p, q, r), s, end.at.as_ref())?.mul(end.weight))
}

/// The point where three planes meet, for reading against many: the sign of its weight, and the
/// point in machine words where it fits.
#[derive(Clone, Copy)]
struct End {
    weight: Sign,
    at: Option<Meeting>,
}

impl End {
    fn new(p: &Moved, q: &Moved, r: &Moved) -> Result<Self, NumError> {
        let at = Meeting::new(&p.plane, &q.plane, &r.plane);
        let weight = match &at {
            Some(point) => point.weight(),
            None => normals_det(&p.plane, &q.plane, &r.plane)?,
        };
        Ok(End { weight, at })
    }
}

/// The grid cells around some points, a step wider each way: every point within a unit of one
/// of them, an infinitesimal move included, lies inside. Two spans that miss share no point.
#[derive(Clone, Copy, Debug)]
struct Span {
    low: [i64; 3],
    high: [i64; 3],
}

impl Span {
    /// The span of `points`, if every one has its floors.
    fn of(points: impl IntoIterator<Item = Option<Meeting>>) -> Option<Self> {
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for point in points {
            let floors = point?.floors()?;
            for ((l, h), v) in low.iter_mut().zip(&mut high).zip(floors) {
                *l = (*l).min(v);
                *h = (*h).max(v);
            }
        }
        // A point lies in [floor, floor + 1) on each axis; a step more each way holds the moves.
        Some(Span {
            low: low.map(|v| v.saturating_sub(1)),
            high: high.map(|v| v.saturating_add(2)),
        })
    }

    /// Whether the two spans are apart on some axis.
    fn misses(&self, other: &Span) -> bool {
        let apart = |(a, b): (&Span, &Span)| a.high.iter().zip(&b.low).any(|(h, l)| h < l);
        apart((self, other)) || apart((other, self))
    }
}

/// The span of `piece`'s corners.
fn piece_span(piece: &Piece) -> Option<Span> {
    let after = piece.bounds.iter().cycle().skip(1);
    Span::of(
        piece
            .bounds
            .iter()
            .zip(after)
            .map(|(bound, next)| Meeting::new(&piece.face, bound, next)),
    )
}

/// A stretch of the line where `p` and `q` meet, from plane `from` to plane `to`, with its two
/// ends, which every polygon's test of it reads, and the span of those ends.
struct Segment {
    p: Moved,
    q: Moved,
    from: Moved,
    to: Moved,
    ends: (End, End),
    span: Option<Span>,
}

impl Segment {
    fn new(p: Moved, q: Moved, from: Moved, to: Moved) -> Result<Self, NumError> {
        let ends = (End::new(&p, &q, &from)?, End::new(&p, &q, &to)?);
        Ok(Segment {
            p,
            q,
            from,
            to,
            span: Span::of([ends.0.at, ends.1.at]),
            ends,
        })
    }
}

/// What `face`, unmoved, reads at the end of a segment where `p`, `q` and `r` meet, as [`read`]
/// gives it: from the end's meeting where that settles it, with a sign not zero, else by the moved
/// determinant, which a reading of zero needs.
fn end_reading(
    (p, q, r): (&Moved, &Moved, &Moved),
    end: &End,
    face: &Plane,
) -> Result<Sign, NumError> {
    let quick = end
        .at
        .and_then(|point| point.reading(face))
        .filter(|sign| *sign != Sign::Zero);
    let base = match quick {
        Some(sign) => sign,
        None => moved_determinant((p, q, r), &Moved::fixed(*face), end.at.as_ref())?,
    };
    Ok(base.mul(end.weight))
}

/// Counts `polygon`'s crossing by `segment`, if it has one, into `winding`: walking back along a
/// path, a crossing toward the front of a face leaves its material. A polygon whose `span` the
/// segment's misses cannot be crossed, as a crossing is a point of both, and is not read.
fn count_crossing(
    segment: &Segment,
    (polygon, span): (&Polygon, Option<&Span>),
    winding: &mut Winding,
) -> Result<(), TraceError> {
    let Some(entry) = polygon.entry else {
        return Ok(());
    };
    if let (Some(own), Some(other)) = (&segment.span, span)
        && own.misses(other)
    {
        return Ok(());
    }
    let Segment {
        p,
        q,
        from,
        to,
        ends: (at_from, at_to),
        span: _,
    } = segment;
    let start = end_reading((p, q, from), at_from, &polygon.piece.face)?;
    let end = end_reading((p, q, to), at_to, &polygon.piece.face)?;
    if start == end || !covers(polygon, p, q, &Moved::fixed(polygon.piece.face))? {
        return Ok(());
    }
    let by = if end == Sign::Positive { 1 } else { -1 };
    Ok(winding.add(entry, by)?)
}

/// The box's faces, each positive inside and moved just inside: `[axis][end]`, end 0 the low one.
fn faces(cell: &Cell) -> Result<[[Moved; 2]; 3], NumError> {
    let mut out = [[Moved::fixed(Plane::new([1, 0, 0], 0)?); 2]; 3];
    let ends = cell.low.iter().zip(&cell.high);
    for (axis, ((low, high), row)) in ends.zip(&mut out).enumerate() {
        let mut n = [0i128; 3];
        for (at, component) in n.iter_mut().enumerate() {
            *component = if at == axis { 2 } else { 0 };
        }
        let turned = n.map(i128::saturating_neg);
        let low_plane = Plane::new(n, i128::from(*low).saturating_neg())?;
        let high_plane = Plane::new(turned, i128::from(*high))?;
        *row = [
            Moved {
                plane: low_plane,
                rank: Rank::Box(axis, 0),
                by: Sign::Positive,
            },
            Moved {
                plane: high_plane,
                rank: Rank::Box(axis, 1),
                by: Sign::Positive,
            },
        ];
    }
    Ok(out)
}

/// The face of the box that the line where `p` and `q` meet leaves it by first, going along
/// `n_p` × `n_q`, as (axis, end). The box is bounded, so one exists.
fn exit(p: &Moved, q: &Moved, faces: &[[Moved; 2]; 3]) -> Result<(usize, usize), NumError> {
    let mut first: Option<(usize, usize)> = None;
    for (axis, row) in faces.iter().enumerate() {
        for (end, face) in row.iter().enumerate() {
            match normals_det(&p.plane, &q.plane, &face.plane)? {
                Sign::Negative => {
                    let earlier = match first.and_then(|(a, e)| faces.get(a)?.get(e)) {
                        Some(held) => read(p, q, held, face)? != Sign::Positive,
                        None => true,
                    };
                    if earlier {
                        first = Some((axis, end));
                    }
                }
                Sign::Positive | Sign::Zero => {}
            }
        }
    }
    first.ok_or(NumError::Degenerate)
}

/// A face of the box by (axis, end), which [`exit`] or an index below 3 and 2 gave.
fn face_at(faces: &[[Moved; 2]; 3], (axis, end): (usize, usize)) -> Result<Moved, NumError> {
    faces
        .get(axis)
        .and_then(|row| row.get(end))
        .copied()
        .ok_or(NumError::Degenerate)
}

/// The faces of the box a path from a corner of a piece on `face` leaves by, as (axis, end): the
/// one the line of the piece's edge on `first` meets first, and then the one that the line where
/// `face` meets that face meets first.
type Exits = ((usize, usize), (usize, usize));

/// [`Exits`] for the piece's edge on `first`.
fn exits(face: &Moved, first: &Moved, faces: &[[Moved; 2]; 3]) -> Result<Exits, NumError> {
    let out = exit(face, first, faces)?;
    let along = exit(face, &face_at(faces, out)?, faces)?;
    Ok((out, along))
}

/// The first two legs of the path from the corner where `first` and `second` meet on `face`:
/// along the edge's line to the box, then along the box's face to its edge.
fn opening(
    face: Moved,
    (first, second): (Moved, Moved),
    (out, along): Exits,
    faces: &[[Moved; 2]; 3],
) -> Result<[Segment; 2], NumError> {
    let wall = face_at(faces, out)?;
    Ok([
        Segment::new(face, first, second, wall)?,
        Segment::new(face, wall, first, face_at(faces, along)?)?,
    ])
}

/// The rest of the path, along the box's edges to its reference corner: the same for every piece
/// on `face` whose path leaves by the same [`Exits`].
fn legs(
    face: Moved,
    (out, along): Exits,
    faces: &[[Moved; 2]; 3],
) -> Result<Vec<Segment>, NumError> {
    let mut segments = Vec::with_capacity(3);
    let (wall, edge) = (face_at(faces, out)?, face_at(faces, along)?);
    // The third axis: 0 + 1 + 2 less the other two.
    let third = 3usize.saturating_sub(out.0).saturating_sub(along.0);
    let floor = face_at(faces, (third, 0))?;
    segments.push(Segment::new(wall, edge, face, floor)?);
    let home = face_at(faces, (out.0, 0))?;
    if out.1 != 0 {
        segments.push(Segment::new(edge, floor, wall, home)?);
    }
    if along.1 != 0 {
        segments.push(Segment::new(
            home,
            floor,
            edge,
            face_at(faces, (along.0, 0))?,
        )?);
    }
    Ok(segments)
}

/// The path from the corner where `first` and `second` meet on `face` to the box's reference
/// corner: along the edge's line, along the face, along the box's edges.
fn path(
    face: Moved,
    first: Moved,
    second: Moved,
    faces: &[[Moved; 2]; 3],
) -> Result<Vec<Segment>, NumError> {
    let exits = exits(&face, &first, faces)?;
    let mut segments = Vec::from(opening(face, (first, second), exits, faces)?);
    segments.extend(legs(face, exits, faces)?);
    Ok(segments)
}

/// What a polygon's legs along the box's edges added, for the face, the side and the exits they
/// were walked from, and how many legs there were.
struct Walked {
    plane: Plane,
    by: Sign,
    exits: Exits,
    crossed: Winding,
    legs: u64,
}

/// Whether `polygon` covers the point where `p`, `q` and `r` meet.
fn covers(polygon: &Polygon, p: &Moved, q: &Moved, r: &Moved) -> Result<bool, NumError> {
    let end = End::new(p, q, r)?;
    for bound in &polygon.piece.bounds {
        let reading = moved_determinant((p, q, r), &Moved::fixed(*bound), end.at.as_ref())?;
        if reading.mul(end.weight) != Sign::Positive {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Which side of the plane `own` a path should start on: the side inside the box, whose faces in
/// canonical form are `boxes`. A plane that is no face of the box has room on both sides of a
/// piece, and takes its front.
fn inward(own: &Canonical, boxes: &[Canonical]) -> Sign {
    match boxes.iter().find(|held| held.plane == own.plane) {
        Some(held) if held.flipped != own.flipped => Sign::Negative,
        _ => Sign::Positive,
    }
}

/// The vectors in front of and behind `piece`, which lies on polygon `index`, and whether it is
/// the first copy of its overlap. `walked` holds what the box's edges added on the paths of the
/// polygon's pieces so far: a piece whose path leaves the box as one of theirs did reads it there,
/// and the meter is charged as if it had walked them, so the work counted is the same.
fn vectors(
    (piece, index): (&Piece, usize),
    polygons: &[(&Polygon, Canonical, Option<Span>)],
    (faces, boxes): (&[[Moved; 2]; 3], &[Canonical]),
    reference: &Winding,
    walked: &mut Vec<Walked>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Winding, Winding, bool), TraceError> {
    let (Some(first_bound), Some(second_bound)) = (piece.bounds.first(), piece.bounds.get(1))
    else {
        return Err(NumError::Degenerate.into());
    };
    // A piece lies on its polygon's face, whose canonical form is held.
    let own = match polygons.get(index) {
        Some((polygon, held, _)) if polygon.piece.face == piece.face => *held,
        _ => canonical(&piece.face)?,
    };
    let near = inward(&own, boxes);
    let face = Moved {
        plane: piece.face,
        rank: Rank::Face,
        by: near,
    };
    let first = Moved {
        plane: *first_bound,
        rank: Rank::First,
        by: Sign::Positive,
    };
    let second = Moved {
        plane: *second_bound,
        rank: Rank::Second,
        by: Sign::Positive,
    };
    let exits = exits(&face, &first, faces)?;
    let mut near_side = reference.clone();
    for segment in &opening(face, (first, second), exits, faces)? {
        for (polygon, _, span) in polygons {
            meter.charge(1)?;
            count_crossing(segment, (polygon, span.as_ref()), &mut near_side)?;
        }
    }
    let seen = walked
        .iter()
        .position(|w| w.plane == face.plane && w.by == face.by && w.exits == exits);
    let walk = if let Some(at) = seen {
        let held = walked.get(at).ok_or(NumError::Degenerate)?;
        let count = u64::try_from(polygons.len()).unwrap_or(u64::MAX);
        meter.charge(held.legs.saturating_mul(count))?;
        held
    } else {
        let mut crossed = Winding::new();
        let segments = legs(face, exits, faces)?;
        for segment in &segments {
            for (polygon, _, span) in polygons {
                meter.charge(1)?;
                count_crossing(segment, (polygon, span.as_ref()), &mut crossed)?;
            }
        }
        walked.push(Walked {
            plane: face.plane,
            by: face.by,
            exits,
            crossed,
            legs: u64::try_from(segments.len()).unwrap_or(u64::MAX),
        });
        walked.last().ok_or(NumError::Degenerate)?
    };
    // Counts only add, so the legs' crossings can join the vector in one go: a count would have
    // to pass 2^31 crossings to leave `i32` in one order and not the other.
    for (entry, by) in walk.crossed.pairs() {
        near_side.add(*entry, *by)?;
    }
    // Across the piece: every polygon on its plane that covers it, the piece's own among them.
    let mut far_side = near_side.clone();
    let mut is_first = true;
    for (at, (polygon, held, _)) in polygons.iter().enumerate() {
        meter.charge(1)?;
        if held.plane != own.plane || !covers(polygon, &face, &first, &second)? {
            continue;
        }
        is_first &= at >= index;
        if let Some(entry) = polygon.entry {
            // Front to back of a face enters its material.
            let into_material = (held.flipped == own.flipped) == (near == Sign::Positive);
            far_side.add(entry, if into_material { 1 } else { -1 })?;
        }
    }
    Ok(if near == Sign::Positive {
        (near_side, far_side, is_first)
    } else {
        (far_side, near_side, is_first)
    })
}

/// The vector at the reference point of the upper box of `cell` split at half step `at` of `axis`:
/// the point walks up the box's edge from `cell`'s own, whose vector is `reference`.
///
/// # Errors
///
/// As [`classify`].
pub fn reference_above(
    cell: &Cell,
    axis: usize,
    at: i64,
    reference: &Winding,
    polygons: &[Polygon],
    meter: &mut WorkMeter<'_>,
) -> Result<Winding, TraceError> {
    let faces = faces(cell)?;
    let low = face_at(&faces, (axis, 0))?;
    let n = [0, 1, 2].map(|i| if i == axis { 2 } else { 0 });
    let (next, last) = (
        axis.saturating_add(1).checked_rem(3).unwrap_or(0),
        axis.saturating_add(2).checked_rem(3).unwrap_or(0),
    );
    let segment = Segment::new(
        face_at(&faces, (next, 0))?,
        face_at(&faces, (last, 0))?,
        Moved {
            plane: Plane::new(n, i128::from(at).saturating_neg())?,
            ..low
        },
        low,
    )?;
    let mut above = reference.clone();
    for polygon in polygons {
        meter.charge(1)?;
        count_crossing(&segment, (polygon, None), &mut above)?;
    }
    Ok(above)
}

/// The vector at the point where `planes` meet, reached from `cell`'s corner, whose vector is
/// `reference`. The point must lie in `cell` and on no polygon.
///
/// # Errors
///
/// A [`TraceError`]: a stopped meter, or what the numbers layer refuses.
pub fn winding_at(
    cell: &Cell,
    reference: &Winding,
    polygons: &[Polygon],
    planes: [Plane; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Winding, TraceError> {
    let faces = faces(cell)?;
    let [a, b, c] = planes;
    let moved = |plane, rank| Moved {
        plane,
        rank,
        by: Sign::Positive,
    };
    let (face, first, second) = (
        moved(a, Rank::Face),
        moved(b, Rank::First),
        moved(c, Rank::Second),
    );
    let mut here = reference.clone();
    for segment in path(face, first, second, &faces)? {
        for polygon in polygons {
            meter.charge(1)?;
            count_crossing(&segment, (polygon, None), &mut here)?;
        }
    }
    Ok(here)
}

/// Every piece of `pieces`, `pieces[i]` being polygon `i`'s as [`crate::cut::local_cuts`] left it,
/// with its vectors, in the order given. `reference` is the vector at `cell`'s low corner, moved
/// just inside the box along each axis.
///
/// # Errors
///
/// A [`TraceError`]: a piece of fewer than two bounds, or planes over budget; a count that left its
/// range; a stopped meter.
pub fn classify(
    cell: &Cell,
    reference: &Winding,
    polygons: &[Polygon],
    pieces: &[Vec<Piece>],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    let faces = faces(cell)?;
    let mut boxes = Vec::with_capacity(6);
    for face in faces.iter().flatten() {
        boxes.push(canonical(&face.plane)?);
    }
    let mut held = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        let span = piece_span(&polygon.piece);
        held.push((polygon, canonical(&polygon.piece.face)?, span));
    }
    let mut out = Vec::new();
    for (index, list) in pieces.iter().enumerate() {
        let mut walked = Vec::new();
        for piece in list {
            let (front, back, first) = vectors(
                (piece, index),
                &held,
                (&faces, &boxes),
                reference,
                &mut walked,
                meter,
            )?;
            out.push(Classified {
                polygon: index,
                piece: piece.clone(),
                front,
                back,
                first,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "tests: native arithmetic, and a failure stops the test"
    )]

    use geom_num::plane::Plane;
    use num_bigint::BigInt;
    use proptest::prelude::*;

    use super::Span;
    use crate::order::Meeting;

    fn plane() -> impl Strategy<Value = Plane> {
        ([-9i128..9, -9i128..9, -9i128..9], -999i128..999)
            .prop_filter_map("a normal", |(n, d)| Plane::new(n, d).ok())
    }

    proptest! {
        /// Every point a span is made of lies a unit inside it, exactly.
        #[test]
        fn a_span_holds_its_points_a_unit_inside(rows in prop::collection::vec((plane(), plane(), plane()), 1..5)) {
            let points: Vec<Option<Meeting>> = rows.iter().map(|(p, q, r)| Meeting::new(p, q, r)).collect();
            let Some(span) = Span::of(points.iter().copied()) else {
                return Ok(());
            };
            for point in points.iter().flatten() {
                let (x, w) = point.coordinates();
                let (x, w) = if w < 0 { (x.map(|v| -v), -w) } else { (x, w) };
                for ((v, low), high) in x.iter().zip(span.low).zip(span.high) {
                    let at = BigInt::from(*v);
                    let (low, high) = (BigInt::from(low + 1), BigInt::from(high - 1));
                    prop_assert!(at >= &low * w && at <= &high * w);
                }
            }
        }

        /// Spans miss exactly when they are apart on an axis, either way round.
        #[test]
        fn spans_miss_when_apart(a in prop::array::uniform6(-9i64..9), b in prop::array::uniform6(-9i64..9)) {
            let span = |v: [i64; 6]| Span {
                low: [v[0].min(v[1]), v[2].min(v[3]), v[4].min(v[5])],
                high: [v[0].max(v[1]), v[2].max(v[3]), v[4].max(v[5])],
            };
            let (s, t) = (span(a), span(b));
            let apart = (0..3).any(|i| s.high[i] < t.low[i] || t.high[i] < s.low[i]);
            prop_assert_eq!(s.misses(&t), apart);
            prop_assert_eq!(t.misses(&s), apart);
        }
    }
}
