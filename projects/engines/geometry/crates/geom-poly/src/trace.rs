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
//!   path's points are never computed.
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
//! and one per bound of a polygon the segment's line crosses. Charges the meter one unit per
//! polygon a segment tests.

use alloc::vec::Vec;
use core::fmt;

use geom_num::plane::Plane;
use geom_num::{NumError, Sign, generated};
use geom_run::{Stopped, WorkMeter};

use crate::convex::{ConvexError, Piece};
use crate::order::normals_det;
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

/// The sign of the 4 × 4 determinant of four planes' coefficients, rows in the order given and the
/// offset last, through the generated predicate: its filter, then its exact path (6.2). It is
/// n · X + d · w for the point where the first three meet, and is defined whether they meet or not.
#[allow(
    clippy::many_single_char_names,
    reason = "p, q, r and s are the planes, and a, b and c the first three's normals"
)]
fn determinant(p: &Plane, q: &Plane, r: &Plane, s: &Plane) -> Result<Sign, NumError> {
    let ([ax, ay, az], ad) = (p.n(), p.d());
    let ([bx, by, bz], bd) = (q.n(), q.d());
    let ([cx, cy, cz], cd) = (r.n(), r.d());
    let ([sx, sy, sz], sd) = (s.n(), s.d());
    generated::tpi_side::filter(
        ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
    )
    .or_else(|| {
        generated::tpi_side::exact(
            ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
        )
    })
    .ok_or(NumError::BudgetExceeded)
}

/// The sign of the 4 × 4 determinant of the four planes' coefficients, each moved. The determinant
/// is linear in each offset, and the moves are of different orders, so its sign is that of the
/// unmoved determinant, or failing that of the largest move with a non-zero cofactor. A plane's
/// reading of `by` ε subtracts `by` ε from its offset.
#[allow(
    clippy::many_single_char_names,
    reason = "p, q, r and s are the moved planes, and a, b, c and d the same planes unmoved"
)]
fn moved_determinant(p: &Moved, q: &Moved, r: &Moved, s: &Moved) -> Result<Sign, NumError> {
    let [a, b, c, d] = [&p.plane, &q.plane, &r.plane, &s.plane];
    let base = determinant(a, b, c, d)?;
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
    let weight = normals_det(&p.plane, &q.plane, &r.plane)?;
    Ok(moved_determinant(p, q, r, s)?.mul(weight))
}

/// A stretch of the line where `p` and `q` meet, from plane `from` to plane `to`.
struct Segment {
    p: Moved,
    q: Moved,
    from: Moved,
    to: Moved,
}

/// Counts `polygon`'s crossing by `segment`, if it has one, into `winding`: walking back along a
/// path, a crossing toward the front of a face leaves its material.
fn count_crossing(
    segment: &Segment,
    polygon: &Polygon,
    winding: &mut Winding,
) -> Result<(), TraceError> {
    let Some(entry) = polygon.entry else {
        return Ok(());
    };
    let Segment { p, q, from, to } = segment;
    let face = Moved::fixed(polygon.piece.face);
    let start = read(p, q, from, &face)?;
    let end = read(p, q, to, &face)?;
    if start == end {
        return Ok(());
    }
    for bound in &polygon.piece.bounds {
        if read(p, q, &face, &Moved::fixed(*bound))? != Sign::Positive {
            return Ok(());
        }
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
            if normals_det(&p.plane, &q.plane, &face.plane)? != Sign::Negative {
                continue;
            }
            let earlier = match first.and_then(|(a, e)| faces.get(a)?.get(e)) {
                Some(held) => read(p, q, held, face)? != Sign::Positive,
                None => true,
            };
            if earlier {
                first = Some((axis, end));
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

/// The path from the corner where `first` and `second` meet on `face` to the box's reference
/// corner: along the edge's line, along the face, along the box's edges.
fn path(
    face: Moved,
    first: Moved,
    second: Moved,
    faces: &[[Moved; 2]; 3],
) -> Result<Vec<Segment>, NumError> {
    let mut segments = Vec::with_capacity(5);
    let out = exit(&face, &first, faces)?;
    let wall = face_at(faces, out)?;
    segments.push(Segment {
        p: face,
        q: first,
        from: second,
        to: wall,
    });
    let along = exit(&face, &wall, faces)?;
    let edge = face_at(faces, along)?;
    segments.push(Segment {
        p: face,
        q: wall,
        from: first,
        to: edge,
    });
    // The third axis: 0 + 1 + 2 less the other two.
    let third = 3usize.saturating_sub(out.0).saturating_sub(along.0);
    let floor = face_at(faces, (third, 0))?;
    segments.push(Segment {
        p: wall,
        q: edge,
        from: face,
        to: floor,
    });
    let home = face_at(faces, (out.0, 0))?;
    if out.1 != 0 {
        segments.push(Segment {
            p: edge,
            q: floor,
            from: wall,
            to: home,
        });
    }
    if along.1 != 0 {
        segments.push(Segment {
            p: home,
            q: floor,
            from: edge,
            to: face_at(faces, (along.0, 0))?,
        });
    }
    Ok(segments)
}

/// Whether `polygon` covers the point where `p`, `q` and `r` meet.
fn covers(polygon: &Polygon, p: &Moved, q: &Moved, r: &Moved) -> Result<bool, NumError> {
    for bound in &polygon.piece.bounds {
        if read(p, q, r, &Moved::fixed(*bound))? != Sign::Positive {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Which side of `plane` a path should start on: the side inside the box. A plane that is no face
/// of the box has room on both sides of a piece, and takes its front.
fn inward(plane: &Plane, faces: &[[Moved; 2]; 3]) -> Result<Sign, NumError> {
    let own = canonical(plane)?;
    for row in faces {
        for face in row {
            let held = canonical(&face.plane)?;
            if held.plane == own.plane {
                return Ok(if held.flipped == own.flipped {
                    Sign::Positive
                } else {
                    Sign::Negative
                });
            }
        }
    }
    Ok(Sign::Positive)
}

/// The vectors in front of and behind `piece`, which lies on polygon `index`, and whether it is
/// the first copy of its overlap.
fn vectors(
    piece: &Piece,
    index: usize,
    polygons: &[(&Polygon, Canonical)],
    faces: &[[Moved; 2]; 3],
    reference: &Winding,
    meter: &mut WorkMeter<'_>,
) -> Result<(Winding, Winding, bool), TraceError> {
    let (Some(first_bound), Some(second_bound)) = (piece.bounds.first(), piece.bounds.get(1))
    else {
        return Err(NumError::Degenerate.into());
    };
    let near = inward(&piece.face, faces)?;
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
    let mut near_side = reference.clone();
    for segment in path(face, first, second, faces)? {
        for (polygon, _) in polygons {
            meter.charge(1)?;
            count_crossing(&segment, polygon, &mut near_side)?;
        }
    }
    // Across the piece: every polygon on its plane that covers it, the piece's own among them.
    let own = canonical(&piece.face)?;
    let mut far_side = near_side.clone();
    let mut is_first = true;
    for (at, (polygon, held)) in polygons.iter().enumerate() {
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
    let mut n = [0i128; 3];
    if let Some(component) = n.get_mut(axis) {
        *component = 2;
    }
    let (next, last) = (
        axis.saturating_add(1).checked_rem(3).unwrap_or(0),
        axis.saturating_add(2).checked_rem(3).unwrap_or(0),
    );
    let segment = Segment {
        p: face_at(&faces, (next, 0))?,
        q: face_at(&faces, (last, 0))?,
        from: Moved {
            plane: Plane::new(n, i128::from(at).saturating_neg())?,
            ..low
        },
        to: low,
    };
    let mut above = reference.clone();
    for polygon in polygons {
        meter.charge(1)?;
        count_crossing(&segment, polygon, &mut above)?;
    }
    Ok(above)
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
    let mut held = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        held.push((polygon, canonical(&polygon.piece.face)?));
    }
    let mut out = Vec::new();
    for (index, list) in pieces.iter().enumerate() {
        for piece in list {
            let (front, back, first) = vectors(piece, index, &held, &faces, reference, meter)?;
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
