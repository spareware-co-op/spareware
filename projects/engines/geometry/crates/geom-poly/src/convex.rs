//! Plane-based faces into convex polygons: the autopartition of 11.3, which lets a face with
//! notches and holes enter the arrangement without one number growing.
//!
//! What it does: EMBER takes convex polygons, each a supporting plane and one bounding plane per
//! edge. [`convex_pieces`] takes a face as its plane and its rings, each ring a cycle of [`Side`]s,
//! and splits it along its own edges' planes only: a binary space partition of the face, by each
//! edge in the order given, which a caller makes name order. Every piece is a [`Piece`]: the face's
//! plane and its bounding planes in turn around it, each with the interior on its positive side.
//!
//! How it runs: the work is a stack of cells, each a list of the half-planes that bound it so far
//! and the parts of edges inside it. A cell takes its first edge's plane as a cut, drops the edges
//! on that line, and sends each other edge to the side or sides it lies on, cutting an edge that
//! crosses at the point where it meets the cut. A cell with no edges left is whole: inside the face
//! if it lies on an edge's positive side, outside if not. A cell inside the face becomes a piece.
//! Every cut runs through a cell's open interior, as an edge on a cell's boundary went with the cut
//! that made it, so no cell is empty and no plane bounds a cell twice.
//!
//! Guarantees:
//!
//! - **Nothing new.** Every plane of a piece is the face's or one of its sides': each edge of a
//!   piece lies on an existing plane, and each corner is the meeting of three, so 6.2's budget
//!   holds, as EMBER's section 3.2 needs.
//! - **A partition of the face.** The pieces are convex, cover the face and overlap nowhere, each
//!   side of the partition being a half-plane of the cut (tested against exact areas).
//! - **The same pieces every time.** Pieces depend only on the planes and the order of the sides;
//!   nothing is hashed, and every decision is an exact predicate.
//! - **No recursion.** The tree is an explicit stack, so its depth never threatens a WebAssembly
//!   stack (principle 13).
//! - **Refused, never guessed.** A side parallel to the face is `Degenerate`; so are two adjacent
//!   sides on one line, which leave a corner undefined. Merge such sides first: the corner they
//!   share is no corner. So are rings that leave a piece without an end, such as a hole with no
//!   outline around it.
//!
//! Method: chapter 11.3's "How faces enter"
//! (`projects/engines/geometry/architecture/11-booleans.md`), after EMBER's sections 3.2 and 4.3
//! (Trettner, Nehring-Wirxel and Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>); the
//! binary space partition is Fuchs, Kedem and Naylor 1980,
//! <https://doi.org/10.1145/800250.807481>, with the leaves read as in Thibault and Naylor 1987,
//! <https://doi.org/10.1145/37402.37430>.
//!
//! Cost: for n edges, at most n cuts, each classifying the edges left in its cell: O(n²) exact
//! predicates at worst, and a piece of k bounds takes O(k²) more to put in order. Charges the meter
//! one unit per edge classified and per bound ordered.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_run::{Stopped, WorkMeter};

use crate::order::{Line, Ray, along, around, normals_det};

/// An edge of a ring: the plane it lies on, positive on the face's inside.
pub type Side = Plane;

/// A convex piece of a face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// The face's plane.
    pub face: Plane,
    /// The bounding planes, in order counter-clockwise seen from where the face's normal points,
    /// each positive on the inside.
    pub bounds: Vec<Plane>,
}

/// Why a face could not be split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvexError {
    /// The numbers layer refused: a side parallel to the face, two adjacent sides on one line, or
    /// rings that leave a piece without an end.
    Num(NumError),
    /// The meter stopped the run.
    Stopped(Stopped),
}

impl fmt::Display for ConvexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvexError::Num(e) => write!(f, "a face that cannot be split: {e:?}"),
            ConvexError::Stopped(_) => f.write_str("the meter stopped the split"),
        }
    }
}

impl core::error::Error for ConvexError {}

impl From<NumError> for ConvexError {
    fn from(e: NumError) -> Self {
        ConvexError::Num(e)
    }
}

impl From<Stopped> for ConvexError {
    fn from(e: Stopped) -> Self {
        ConvexError::Stopped(e)
    }
}

/// A part of an edge inside a cell: its plane, and the planes that end it.
#[derive(Clone, Copy, Debug)]
struct Run {
    side: Plane,
    start: Plane,
    end: Plane,
}

/// A cell of the partition: the half-planes around it so far, the parts of edges inside it.
struct Cell {
    bounds: Vec<Plane>,
    runs: Vec<Run>,
    /// Whether it lies on the positive side of the last cut.
    inside: bool,
}

/// Which side of `cut` a point lies on, from the point's planes.
fn side_of(cut: &Plane, face: &Plane, a: &Plane, b: &Plane) -> Result<Sign, NumError> {
    cut.side_of_tpi(&Tpi::new(*face, *a, *b)?)
}

/// `plane`, facing the other way.
fn opposite(plane: &Plane) -> Result<Plane, NumError> {
    let [x, y, z] = plane.n();
    let negate = |v: i128| v.checked_neg().ok_or(NumError::BudgetExceeded);
    let n = [negate(x)?, negate(y)?, negate(z)?];
    Plane::new(n, negate(plane.d())?)
}

/// `cell`'s children by `cut`, the positive one first.
fn split(
    face: &Plane,
    cell: Cell,
    cut: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<[Cell; 2], ConvexError> {
    let mut front = Vec::new();
    let mut back = Vec::new();
    for run in &cell.runs {
        meter.charge(1)?;
        let a = side_of(cut, face, &run.side, &run.start)?;
        let b = side_of(cut, face, &run.side, &run.end)?;
        match (a, b) {
            // On the cut's own line: the cut holds it.
            (Sign::Zero, Sign::Zero) => {}
            (Sign::Positive, Sign::Negative) => {
                front.push(Run { end: *cut, ..*run });
                back.push(Run {
                    start: *cut,
                    ..*run
                });
            }
            (Sign::Negative, Sign::Positive) => {
                back.push(Run { end: *cut, ..*run });
                front.push(Run {
                    start: *cut,
                    ..*run
                });
            }
            (Sign::Positive, _) | (_, Sign::Positive) => front.push(*run),
            _ => back.push(*run),
        }
    }
    let mut ahead = cell.bounds.clone();
    ahead.push(*cut);
    let mut behind = cell.bounds;
    behind.push(opposite(cut)?);
    Ok([
        Cell {
            bounds: ahead,
            runs: front,
            inside: true,
        },
        Cell {
            bounds: behind,
            runs: back,
            inside: false,
        },
    ])
}

/// Whether `bound`'s line holds a length of the cell `bounds` make; `Degenerate` if that length has
/// no end, as a cell without an end is not a piece.
fn holds_length(face: &Plane, bound: &Plane, bounds: &[Plane]) -> Result<bool, NumError> {
    let line = Line::new(*face, *bound)?;
    let mut start: Option<Plane> = None;
    let mut end: Option<Plane> = None;
    for other in bounds {
        // Along n_F × n_bound, a plane whose determinant is positive is entered, and one whose
        // determinant is negative is left.
        match normals_det(face, bound, other)? {
            Sign::Zero => {
                // Parallel to the line: the same line with the inside the other way shuts it.
                let point = Tpi::new(*face, *bound, first_crossing(face, bound, bounds)?)?;
                if other.side_of_tpi(&point)? == Sign::Negative {
                    return Ok(false);
                }
            }
            Sign::Positive => {
                let later = match &start {
                    Some(held) => along(&line, held, other)? == Ordering::Less,
                    None => true,
                };
                if later {
                    start = Some(*other);
                }
            }
            Sign::Negative => {
                let earlier = match &end {
                    Some(held) => along(&line, other, held)? == Ordering::Less,
                    None => true,
                };
                if earlier {
                    end = Some(*other);
                }
            }
        }
    }
    match (start, end) {
        (Some(s), Some(e)) => Ok(along(&line, &s, &e)? == Ordering::Less),
        _ => Err(NumError::Degenerate),
    }
}

/// A bound that crosses `bound`'s line, for a point of that line.
fn first_crossing(face: &Plane, bound: &Plane, bounds: &[Plane]) -> Result<Plane, NumError> {
    for other in bounds {
        if normals_det(face, bound, other)? != Sign::Zero {
            return Ok(*other);
        }
    }
    Err(NumError::Degenerate)
}

/// The piece of an inside cell: its bounds that hold an edge, in turn around it. Directions turn
/// one way around a convex polygon, so the order of the edges is the order of the angles of their
/// lines, each `n_F` × `n_bound`; an order by shared planes would stall at a corner where a third
/// bound only touches.
fn piece(face: &Plane, bounds: &[Plane], meter: &mut WorkMeter<'_>) -> Result<Piece, ConvexError> {
    let ray = |with: &Plane| Ray {
        with: *with,
        reversed: false,
    };
    let mut ordered: Vec<Plane> = Vec::new();
    for bound in bounds {
        meter.charge(1)?;
        if !holds_length(face, bound, bounds)? {
            continue;
        }
        let mut at = ordered.len();
        if let Some(first) = ordered.first() {
            for (k, held) in ordered.iter().enumerate() {
                let order = around(face, &ray(first), &ray(bound), &ray(held))?;
                if order == Ordering::Less {
                    at = k;
                    break;
                }
            }
        }
        ordered.insert(at, *bound);
    }
    Ok(Piece {
        face: *face,
        bounds: ordered,
    })
}

/// The ring's edges as runs, each ended by the ring's sides before and after it.
fn ring_runs(ring: &[Side], runs: &mut Vec<Run>) {
    let before = ring.iter().cycle().skip(ring.len().saturating_sub(1));
    let after = ring.iter().cycle().skip(1);
    for ((side, start), end) in ring.iter().zip(before).zip(after) {
        runs.push(Run {
            side: *side,
            start: *start,
            end: *end,
        });
    }
}

/// `face`, bounded by `rings`, as convex pieces: each ring runs with the inside on its sides'
/// positive side, and the sides are taken as cuts in the order given.
///
/// # Errors
///
/// A [`ConvexError`]: a side parallel to the face, adjacent sides on one line, rings that leave a
/// piece without an end, or a stopped meter.
pub fn convex_pieces(
    face: &Plane,
    rings: &[Vec<Side>],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, ConvexError> {
    let mut runs = Vec::new();
    for ring in rings {
        ring_runs(ring, &mut runs);
    }
    let mut pieces = Vec::new();
    let mut work = alloc::vec![Cell {
        bounds: Vec::new(),
        runs,
        inside: false,
    }];
    while let Some(mut cell) = work.pop() {
        if cell.runs.is_empty() {
            if cell.inside {
                pieces.push(piece(face, &cell.bounds, meter)?);
            }
            continue;
        }
        let cut = cell.runs.remove(0).side;
        let [front, back] = split(face, cell, &cut, meter)?;
        // The positive side comes off the stack first, so pieces come in tree order.
        work.push(back);
        work.push(front);
    }
    Ok(pieces)
}
