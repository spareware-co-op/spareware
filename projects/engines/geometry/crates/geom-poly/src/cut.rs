//! Polygon clipping and local cuts: step 2 of 11.3, after which no piece crosses another.
//!
//! What it does: [`clip`] splits a convex [`Piece`] by a plane. [`crosses`] says whether two pieces
//! on different planes truly pass through each other: each has points on both sides of the other's
//! plane, and their stretches of the line the planes share overlap by a length. [`local_cuts`]
//! takes the polygons of one box and cuts each by every polygon that crosses it or ends on it,
//! keeping each as a list of convex pieces of the polygon, the leaves of the small tree of EMBER's
//! section 4.3.
//!
//! Guarantees:
//!
//! - **Nothing new.** A cut adds the cutting plane to a piece's bounds, and every corner stays the
//!   meeting of three planes the input holds, so 6.2's budget holds however many cuts run.
//! - **A partition.** [`clip`]'s two parts cover the piece and overlap only along the cut, each
//!   convex, each still in turn around its face (tested against exact areas).
//! - **No piece crosses another.** After [`local_cuts`], [`crosses`] is false of any two pieces
//!   (tested), so step 3 can classify each piece as a whole. A polygon that ends on a piece, as one
//!   clipped by a box does where it left the box, cuts it too: the winding can change across it on
//!   the side it stands on.
//! - **A plane through a piece's corner cuts only what it passes.** A piece that only touches the
//!   plane, or lies in it, is left whole: [`Clipped::Front`], [`Clipped::Back`], [`Clipped::On`].
//! - **An overlap on one plane is a piece of each.** Polygons on one plane never cross, but where
//!   they overlap, each is cut by the other's bounds, so a piece lies wholly inside the other or
//!   wholly outside it (11.5), and [`crate::trace`] can count it as one.
//! - **The same cuts every time.** Polygons are cut in the order given, which a caller makes (operand
//!   key, name) (11.3), and nothing depends on addresses or threads.
//!
//! Method: chapter 11.3's "Local cuts" (`projects/engines/geometry/architecture/11-booleans.md`),
//! after EMBER's section 4.3 (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>); [`clip`] is Sutherland and Hodgman's clip of a convex
//! polygon by one plane, 1974, <https://doi.org/10.1145/360767.360802>, on implicit corners.
//!
//! Cost: [`clip`] classifies each corner once, O(k) exact predicates for k bounds. [`crosses`] is
//! O(k + m) classifications and O(k + m) comparisons along a line. [`local_cuts`] is O(n²) pairs
//! of polygons at worst, in a box the subdivision keeps small; a pair whose planes cannot cut is
//! told from the two polygons whole, O(k + m), and its pieces are not compared. Charges the meter
//! one unit per pair of pieces, compared or not.

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_run::WorkMeter;

use crate::convex::{ConvexError, Piece, opposite};
use crate::order::{Line, Meeting, along, normals_det};
use crate::plane::canonical;

/// What a plane does to a piece.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clipped {
    /// The piece lies on the plane's positive side, perhaps touching it.
    Front,
    /// The piece lies on the plane's negative side, perhaps touching it.
    Back,
    /// The piece lies in the plane.
    On,
    /// The plane passes through it: the positive part, then the negative part.
    Split(Piece, Piece),
}

/// The sign of `plane` at each corner of `piece`, corner i being where bounds i and i + 1 meet.
fn corners(piece: &Piece, plane: &Plane) -> Result<Vec<Sign>, NumError> {
    let after = piece.bounds.iter().cycle().skip(1);
    piece
        .bounds
        .iter()
        .zip(after)
        .map(|(bound, next)| plane.side_of_tpi(&Tpi::meeting(piece.face, *bound, *next)?))
        .collect()
}

/// The part of `piece` on the side `keep`, closed by `cut`: the bounds with a corner on that side,
/// in turn, from just past the corners on the side `away`, whose gap `cut` closes. Bound i runs from
/// corner i − 1 to corner i.
fn part(piece: &Piece, signs: &[Sign], keep: Sign, away: Sign, cut: Plane) -> Piece {
    let count = signs.len();
    let from = signs.iter().cycle().skip(count.saturating_sub(1));
    let edges: Vec<(Plane, Sign, Sign)> = piece
        .bounds
        .iter()
        .zip(from)
        .zip(signs)
        .map(|((bound, a), b)| (*bound, *a, *b))
        .collect();
    let start = edges
        .iter()
        .position(|(_, a, b)| *a == away && *b != away)
        .unwrap_or(0);
    let mut bounds: Vec<Plane> = edges
        .iter()
        .cycle()
        .skip(start)
        .take(count)
        .filter(|(_, a, b)| *a == keep || *b == keep)
        .map(|(bound, _, _)| *bound)
        .collect();
    bounds.push(cut);
    Piece {
        face: piece.face,
        bounds,
    }
}

/// `piece` against `plane`.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a piece whose adjacent bounds do not meet, and what
/// [`Plane::side_of_tpi`] gives, which planes within budget cannot reach.
pub fn clip(piece: &Piece, plane: &Plane) -> Result<Clipped, NumError> {
    let signs = corners(piece, plane)?;
    let front = signs.contains(&Sign::Positive);
    let back = signs.contains(&Sign::Negative);
    Ok(match (front, back) {
        (true, true) => Clipped::Split(
            part(piece, &signs, Sign::Positive, Sign::Negative, *plane),
            part(
                piece,
                &signs,
                Sign::Negative,
                Sign::Positive,
                opposite(plane)?,
            ),
        ),
        (true, false) => Clipped::Front,
        (false, true) => Clipped::Back,
        (false, false) => Clipped::On,
    })
}

/// The planes where `line` enters and leaves `piece`, which it passes through or touches.
fn stretch(piece: &Piece, line: &Line) -> Result<(Plane, Plane), NumError> {
    let [p, q] = line.planes();
    let mut enter: Option<Plane> = None;
    let mut leave: Option<Plane> = None;
    for bound in &piece.bounds {
        match normals_det(&p, &q, bound)? {
            // Parallel to the line, which reaches the piece: it bounds nothing here.
            Sign::Zero => {}
            Sign::Positive => {
                let later = match &enter {
                    Some(held) => along(line, held, bound)? == Ordering::Less,
                    None => true,
                };
                if later {
                    enter = Some(*bound);
                }
            }
            Sign::Negative => {
                let earlier = match &leave {
                    Some(held) => along(line, bound, held)? == Ordering::Less,
                    None => true,
                };
                if earlier {
                    leave = Some(*bound);
                }
            }
        }
    }
    enter.zip(leave).ok_or(NumError::Degenerate)
}

/// Whether `a` and `b` pass through each other: each has corners on both sides of the other's
/// plane, and their stretches of the line the planes share overlap by a length.
///
/// # Errors
///
/// As [`clip`].
pub fn crosses(a: &Piece, b: &Piece) -> Result<bool, NumError> {
    let Ok(line) = Line::new(a.face, b.face) else {
        return Ok(false);
    };
    let both = |piece: &Piece, plane: &Plane| -> Result<bool, NumError> {
        let signs = corners(piece, plane)?;
        Ok(signs.contains(&Sign::Positive) && signs.contains(&Sign::Negative))
    };
    if !both(a, &b.face)? || !both(b, &a.face)? {
        return Ok(false);
    }
    let (a_in, a_out) = stretch(a, &line)?;
    let (b_in, b_out) = stretch(b, &line)?;
    Ok(along(&line, &a_in, &b_out)? == Ordering::Less
        && along(&line, &b_in, &a_out)? == Ordering::Less)
}

/// Whether `other`, whose plane passes through `piece`, meets the inside of `piece` along a length:
/// it crosses `piece`'s plane or ends on it, and holds a length of the line the planes share that
/// overlaps `piece`'s stretch by a length.
fn reaches(piece: &Piece, other: &Piece) -> Result<bool, NumError> {
    let line = Line::new(piece.face, other.face)?;
    let on = corners(other, &piece.face)?;
    if !on.contains(&Sign::Zero) && (!on.contains(&Sign::Positive) || !on.contains(&Sign::Negative))
    {
        return Ok(false);
    }
    let (a_in, a_out) = stretch(piece, &line)?;
    let (b_in, b_out) = stretch(other, &line)?;
    Ok(along(&line, &b_in, &b_out)? == Ordering::Less
        && along(&line, &a_in, &b_out)? == Ordering::Less
        && along(&line, &b_in, &a_out)? == Ordering::Less)
}

/// The points of `piece`'s corners, corner i where bounds i and i + 1 meet, in machine words where
/// they fit, for reading against many planes.
fn meetings(piece: &Piece) -> Vec<Option<Meeting>> {
    let after = piece.bounds.iter().cycle().skip(1);
    piece
        .bounds
        .iter()
        .zip(after)
        .map(|(bound, next)| Meeting::new(&piece.face, bound, next))
        .collect()
}

/// Which signs `plane` takes at the corners of `piece`, whose points are `at`, as whether it is
/// positive, negative and zero at any: [`corners`]'s signs, each read from its point where that
/// fits, else by the predicate as [`corners`] does.
fn signs_seen(piece: &Piece, at: &[Option<Meeting>], plane: &Plane) -> Result<[bool; 3], NumError> {
    let mut seen = [false; 3];
    let after = piece.bounds.iter().cycle().skip(1);
    for ((bound, next), point) in piece.bounds.iter().zip(after).zip(at) {
        let read = point
            .filter(|p| p.weight() != Sign::Zero)
            .and_then(|p| Some(p.reading(plane)?.mul(p.weight())));
        let sign = match read {
            Some(sign) => sign,
            None => plane.side_of_tpi(&Tpi::meeting(piece.face, *bound, *next)?)?,
        };
        let at = match sign {
            Sign::Positive => 0,
            Sign::Negative => 1,
            Sign::Zero => 2,
        };
        if let Some(slot) = seen.get_mut(at) {
            *slot = true;
        }
    }
    Ok(seen)
}

/// Whether `other`, on another plane, can cut any piece of `polygon`, whose corners' points are
/// `at`: `polygon` has corners on both sides of `other`'s plane, and `other` crosses `polygon`'s
/// plane or ends on it. A piece lies in its polygon, so where the first fails [`clip`] splits no
/// piece, and where the second fails [`reaches`] holds for none, whatever the piece.
fn can_cut(
    (polygon, other): (&Piece, &Piece),
    at: (&[Option<Meeting>], &[Option<Meeting>]),
) -> Result<bool, NumError> {
    let [positive, negative, _] = signs_seen(polygon, at.0, &other.face)?;
    if !positive || !negative {
        return Ok(false);
    }
    let [positive, negative, zero] = signs_seen(other, at.1, &polygon.face)?;
    Ok(zero || (positive && negative))
}

/// `piece`, cut by the bounds of `other`, on its plane, where the two overlap: the parts beyond each
/// bound in turn, then the part inside them all. If they share no area, `piece` whole.
fn carve(piece: &Piece, other: &Piece) -> Result<Vec<Piece>, NumError> {
    let mut rest = piece.clone();
    let mut parts = Vec::new();
    for bound in &other.bounds {
        match clip(&rest, bound)? {
            Clipped::Split(inside, beyond) => {
                parts.push(beyond);
                rest = inside;
            }
            Clipped::Back => return Ok(alloc::vec![piece.clone()]),
            Clipped::Front | Clipped::On => {}
        }
    }
    parts.push(rest);
    Ok(parts)
}

/// `polygons`, each as the convex pieces that every polygon crossing it or ending on it left of it,
/// in the order given.
///
/// # Errors
///
/// A [`ConvexError`]: what [`clip`] gives, or a stopped meter.
pub fn local_cuts(
    polygons: &[Piece],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<Piece>>, ConvexError> {
    let mut planes = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        planes.push(canonical(&polygon.face)?.plane);
    }
    let points: Vec<Vec<Option<Meeting>>> = polygons.iter().map(meetings).collect();
    let mut out: Vec<Vec<Piece>> = Vec::with_capacity(polygons.len());
    for (((at, polygon), own), own_points) in polygons.iter().enumerate().zip(&planes).zip(&points)
    {
        let mut pieces = alloc::vec![polygon.clone()];
        let others = polygons.iter().enumerate().zip(&planes).zip(&points);
        for (((other_at, other), plane), other_points) in others {
            if other_at == at {
                continue;
            }
            let coplanar = own == plane;
            if !coplanar && !can_cut((polygon, other), (own_points, other_points))? {
                // Charged as if each piece had been compared, so the work counted is the same.
                meter.charge(u64::try_from(pieces.len()).unwrap_or(u64::MAX))?;
                continue;
            }
            let mut next = Vec::with_capacity(pieces.len());
            for piece in pieces {
                meter.charge(1)?;
                if coplanar {
                    next.extend(carve(&piece, other)?);
                    continue;
                }
                match clip(&piece, &other.face)? {
                    Clipped::Split(front, back) if reaches(&piece, other)? => {
                        next.push(front);
                        next.push(back);
                    }
                    _ => next.push(piece),
                }
            }
            pieces = next;
        }
        out.push(pieces);
    }
    Ok(out)
}
