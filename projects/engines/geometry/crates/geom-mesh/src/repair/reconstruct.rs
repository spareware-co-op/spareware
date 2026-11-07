//! Reconstruction by winding number (19.7): a closed mesh from a soup that healing cannot close.
//!
//! What it does: [`reconstruct()`] takes the faces of a [`RawMesh`] as they are, cuts the space
//! round them into convex cells by the planes of their triangles, finds each cell's generalised
//! winding number at a point inside it, keeps the cells whose number is above one half, and
//! returns the boundary between the cells kept and the cells not kept as an [`ExactMesh`] whose
//! faces lie on the planes the cells were cut by. Where the input bounds the result, its faces
//! survive exactly; where a hole is closed, the new faces lie on a plane the input already had.
//!
//! How it runs: a binary space partition of a box round the input, split by the plane of a
//! triangle fragment still inside the cell, until no fragment is left. Every cell is a convex
//! polytope kept as its faces, each a [`Piece`]; cutting one by a plane is `geom-poly`'s exact
//! clip of each face, and the cut's own face is that plane clipped by the cell's planes. A cell
//! with no triangle left in it gets its winding number: the sum over every triangle of the solid
//! angle it makes at the cell's centroid, divided by 4π, in interval arithmetic. The solid angle
//! is libigl's `solid_angle`, converted to intervals in `winding.rs`, which is the one file of this
//! crate under the Mozilla Public License. The boundary pieces are the
//! portals (a cut plane's part inside a cell) that have a kept cell on one side and a cell not
//! kept on the other, found by pushing each portal down both sides' trees, and are merged into
//! maximal faces by `geom-poly`'s merge and built as any boolean's result is.
//!
//! Choices 19.7 leaves open, made here:
//!
//! - **A binary space partition, not boxes.** 19.7 says `geom-poly`'s subdivision cuts boxes by the
//!   planes of the faces they hold. Cutting a cell by the planes of the faces *in it* and nothing
//!   else gives the same cells inside each box without a seam on the boxes' walls, which would
//!   need a second pass to match the two sides of every wall.
//! - **The box.** The input's bounding box, grown on every side by a sixteenth of its longest side
//!   and at least 16 steps. Outside it the winding number is 0. A cell touching the box that is
//!   kept (a hole the input leaves open to the outside) is closed by the box's wall, so the hole
//!   is closed at the box, not at the hole: close holes in planar faces and the box does not show.
//! - **Faces are used as oriented, patch by patch.** Step 4 of healing runs first (without its
//!   welding): each manifold patch faces one way, and a closed patch faces outward by its exact
//!   volume. An open patch keeps the way its lowest-keyed face faces.
//! - **Undecided cells are not kept, and are reported.** A cell whose interval holds ½ is neither
//!   kept nor dropped by a rule; it is left out of the result like a dropped cell, listed in
//!   [`Reconstructed::undecided`] with its point and its interval, and counted in the log.
//!
//! Guarantees:
//!
//! - **Exact cells.** Every cell, portal and kept piece is exact: planes of the input's triangles
//!   and of the box, and exact predicates on them. No cell is guessed.
//! - **Proven numbers, said.** A cell is kept only if the whole interval of its winding number is
//!   above ½. The interval encloses the number at every point within half a step of the centroid
//!   (the centroid of the cell's corners rounded to the grid), so it is outward-rounded.
//! - **Approximate, measured both ways.** The result's class is [`Exactness::Approximate`] with
//!   the larger of two deviations ([`Deviation`]): the farthest a corner of the result is from the
//!   input, and the farthest a vertex of the input is from the result, each measured to whole
//!   steps by the exact point-triangle distance of 19.10 on the rounded corners, plus one step for
//!   the rounding. It is a measurement at corners, not a proven Hausdorff bound.
//! - **Deterministic.** Cells are cut in an order that depends on the faces and their numbers
//!   alone, every decision is an exact predicate, and the numbers come out the same on any pool.
//! - **No recursion.** The partition, its portals and their pushing are explicit stacks.
//! - **Refused, never guessed.** A mesh with no face, or whose every cell is dropped or undecided,
//!   is an error that lists the undecided cells.
//!
//! Method: Jacobson, Kavan and Sorkine-Hornung, "Robust inside-outside segmentation using
//! generalized winding numbers", ACM Trans. Graph. 32(4), 2013,
//! <https://doi.org/10.1145/2461912.2461916>, with its cells found exactly instead of by a graph
//! cut; the partition is Fuchs, Kedem and Naylor 1980, <https://doi.org/10.1145/800250.807481>,
//! its portals as in Thibault and Naylor 1987, <https://doi.org/10.1145/37402.37430>.
//!
//! Cost: for n triangles and c cells, O(c · n) interval solid angles, and the partition's clips:
//! for a cell holding m fragments, up to 4 m clips to choose a plane and m to cut by it. The two
//! deviations are O(v · t) exact point-triangle distances for v vertices and t triangles. Charges
//! the meter one unit per clip, per solid angle and per distance.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b and c are corners, p and q points, and n, u and v vectors, as in the formulas"
)]

use alloc::vec::Vec;

use geom_num::grid::{GridStep, P3, Steps};
use geom_num::interval::Interval;
use geom_num::plane::{Plane, Tpi};
use geom_num::{Exactness, NumError, Sign};
use geom_poly::convex::Piece;
use geom_poly::cut::{Clipped, clip};
use geom_poly::merge::{Merge, Outward, Rules, merge_on};
use geom_poly::plane::canonical;
use geom_poly::winding::Winding;
use geom_run::{Exec, WorkMeter};

use super::loops::{Face, collinear, cross, difference};
use super::orient::orient;
use super::weld::weld;
use super::winding::winding_number;
use super::{HealError, RawMesh};
use crate::boolean::polygons_of;
use crate::contact::{Closest, closest_feature, point_triangle};
use crate::exact::build::{Source, build};
use crate::exact::{ExactMesh, MeshError};

/// The frame's limit on a coordinate: ±(2^40 − 1).
const FRAME: i128 = (1_i128 << 40) - 1;

/// Half the width of the box round a cell's centroid, in steps: the centroid is of corners
/// rounded to the grid, each within half a step on an axis, and an average of such is too.
const CENTROID_REACH: f64 = 0.5 + 1.0 / 512.0;

/// How many fragments' planes are tried to choose one to cut by.
const CANDIDATES: usize = 4;

/// The size of the square a portal starts as, on a plane: past the frame on every axis.
const BIG: i128 = 1_i128 << 41;

/// How to reconstruct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reconstruct {
    /// The frame's grid step, which the result's deviation counts.
    pub step: GridStep,
}

impl Default for Reconstruct {
    /// `Q0`'s grid.
    fn default() -> Self {
        Reconstruct { step: GridStep::Q0 }
    }
}

/// A cell whose winding number the arithmetic could not place on one side of ½.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Undecided {
    /// The grid point nearest the cell's centroid, where the number was taken.
    pub at: P3,
    /// The interval's lower end, or −∞ where the arithmetic refused (the point is on a triangle).
    pub low: f64,
    /// The interval's upper end, or +∞ likewise.
    pub high: f64,
}

/// How many cells of each kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cells {
    /// Cells whose whole interval is above ½.
    pub kept: u32,
    /// Cells whose whole interval is below ½.
    pub dropped: u32,
    /// Cells whose interval holds ½.
    pub undecided: u32,
}

/// How far the result is from the input, measured both ways.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deviation {
    /// The farthest a corner of the result is from the input, in steps, rounding included.
    pub from_input: Steps,
    /// The farthest a vertex of the input is from the result, in steps, rounding included.
    pub from_result: Steps,
}

/// What reconstruction did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReconstructLog {
    /// The triangles the faces made (a face of more than three corners is a fan).
    pub triangles: u32,
    /// The planes the space was cut by.
    pub cuts: u32,
    /// The cells, by kind.
    pub cells: Cells,
    /// The convex pieces of the boundary, before they were merged into faces.
    pub pieces: u32,
    /// The deviation.
    pub deviation: Deviation,
}

/// A reconstructed mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Reconstructed {
    /// The mesh: closed, its class [`Exactness::Approximate`] by the larger deviation.
    pub mesh: ExactMesh,
    /// The cells left out because their interval held ½, the lowest-keyed first.
    pub undecided: Vec<Undecided>,
    /// What it did.
    pub log: ReconstructLog,
}

/// Why reconstruction failed.
#[derive(Clone, Debug, PartialEq)]
pub enum ReconstructError {
    /// The faces' first steps (welding to the keys, orienting) failed.
    Heal(HealError),
    /// No face has any area.
    NoSurface,
    /// No cell is kept: the faces enclose nothing, or every enclosed cell is undecided.
    NothingKept {
        /// The cells whose interval held ½.
        undecided: Vec<Undecided>,
    },
    /// A failure of the mesh, its numbers or its budget.
    Mesh(MeshError),
}

impl From<MeshError> for ReconstructError {
    fn from(e: MeshError) -> Self {
        ReconstructError::Mesh(e)
    }
}

impl From<HealError> for ReconstructError {
    fn from(e: HealError) -> Self {
        ReconstructError::Heal(e)
    }
}

impl From<NumError> for ReconstructError {
    fn from(e: NumError) -> Self {
        ReconstructError::Mesh(MeshError::Num(e))
    }
}

impl From<geom_run::Stopped> for ReconstructError {
    fn from(e: geom_run::Stopped) -> Self {
        ReconstructError::Mesh(MeshError::from(e))
    }
}

impl core::fmt::Display for ReconstructError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReconstructError::Heal(e) => write!(f, "{e}"),
            ReconstructError::NoSurface => f.write_str("no face has any area"),
            ReconstructError::NothingKept { undecided } => write!(
                f,
                "no cell is inside: the faces enclose nothing ({} cells are undecided)",
                undecided.len()
            ),
            ReconstructError::Mesh(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for ReconstructError {}

/// What a cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Label {
    /// Inside the result.
    Kept,
    /// Outside it: the winding number is below ½, or the cell is outside the box.
    Dropped,
    /// Left out, and reported.
    Undecided,
}

impl Label {
    /// Whether the cell is in the result.
    fn is_kept(self) -> bool {
        self == Label::Kept
    }
}

/// A node of the partition.
enum Node {
    /// A cell with no triangle left in it.
    Leaf(Label),
    /// A cell cut by `plane`, whose part inside it is `portal`.
    Split {
        plane: Plane,
        portal: Piece,
        front: usize,
        back: usize,
    },
}

/// A cell on its way down the partition: the faces of its polytope, each positive towards the
/// cell's inside, and the fragments of triangles inside it.
struct Cell {
    faces: Vec<Piece>,
    fragments: Vec<Piece>,
}

/// The unit vector of axis `a`.
fn unit(a: usize, sign: i128) -> [i128; 3] {
    let mut v = [0_i128; 3];
    if let Some(slot) = v.get_mut(a) {
        *slot = sign;
    }
    v
}

/// `plane` facing the other way.
fn opposite(plane: &Plane) -> Result<Plane, NumError> {
    let [x, y, z] = plane.n();
    let negate = |v: i128| v.checked_neg().ok_or(NumError::BudgetExceeded);
    Plane::new([negate(x)?, negate(y)?, negate(z)?], negate(plane.d())?)
}

/// The axis of `n`'s largest part, the lowest on a tie.
fn dominant(n: [i128; 3]) -> usize {
    let [x, y, z] = n.map(i128::unsigned_abs);
    if x >= y && x >= z {
        0
    } else if y >= z {
        1
    } else {
        2
    }
}

/// A square on `plane`, counter-clockwise seen from where its normal points, that reaches past the
/// frame: the piece a portal starts as, to be clipped by the planes round its cell.
fn big_square(plane: &Plane) -> Result<Piece, NumError> {
    let n = plane.n();
    let a = dominant(n);
    // The other two axes, so that a, b and c go round in turn.
    let (b, c) = [(1, 2), (2, 0), (0, 1)].get(a).copied().unwrap_or((1, 2));
    let low = |axis: usize| Plane::new(unit(axis, 1), BIG);
    let high = |axis: usize| Plane::new(unit(axis, -1), BIG);
    let positive = n.get(a).is_some_and(|v| *v > 0);
    let bounds = if positive {
        alloc::vec![low(c)?, high(b)?, high(c)?, low(b)?]
    } else {
        alloc::vec![low(b)?, high(c)?, high(b)?, low(c)?]
    };
    Ok(Piece {
        face: *plane,
        bounds,
    })
}

/// The part of `plane` inside the cell whose faces' planes are `round` (each positive inside), or
/// `None` if the plane does not cut the cell's inside.
fn portal_on(
    plane: &Plane,
    round: impl Iterator<Item = Plane>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Piece>, ReconstructError> {
    let mut piece = big_square(plane)?;
    for h in round {
        meter.charge(1)?;
        match clip(&piece, &h)? {
            Clipped::Front => {}
            Clipped::Back | Clipped::On => return Ok(None),
            Clipped::Split(front, _) => piece = front,
        }
    }
    Ok(Some(piece))
}

/// The corners of `piece`, as exact points.
fn corners(piece: &Piece) -> Result<Vec<Tpi>, NumError> {
    let mut out = Vec::new();
    out.try_reserve_exact(piece.bounds.len())
        .map_err(|_| NumError::BudgetExceeded)?;
    // Corner i is where bound i meets the next, the first after the last.
    let next = piece.bounds.iter().cycle().skip(1);
    for (bound, after) in piece.bounds.iter().zip(next) {
        out.push(Tpi::meeting(piece.face, *bound, *after)?);
    }
    Ok(out)
}

/// A triangle's piece: its plane and the plane of each edge through the triangle's dominant axis,
/// each positive towards the inside.
fn triangle_piece(t: [P3; 3]) -> Result<Piece, NumError> {
    let [a, b, c] = t;
    let face = Plane::through(a, b, c)?;
    let axis = dominant(face.n());
    let mut bounds = Vec::new();
    for (p, q, r) in [(a, b, c), (b, c, a), (c, a, b)] {
        let shifted = |sign: i64| {
            let mut v = [p.x.get(), p.y.get(), p.z.get()];
            if let Some(slot) = v.get_mut(axis) {
                *slot = slot.checked_add(sign).ok_or(NumError::OutOfRange {
                    value: i128::from(*slot),
                })?;
            }
            P3::new(v[0], v[1], v[2])
        };
        let off = shifted(1).or_else(|_| shifted(-1))?;
        let edge = Plane::through(p, q, off)?;
        bounds.push(match edge.side(r)? {
            Sign::Positive => edge,
            Sign::Negative => opposite(&edge)?,
            Sign::Zero => return Err(NumError::Degenerate),
        });
    }
    Ok(Piece { face, bounds })
}

/// The label of a cell whose polytope has `faces`, with the point where it was decided if it is
/// undecided.
fn label_of(
    faces: &[Piece],
    triangles: &[[P3; 3]],
    meter: &mut WorkMeter<'_>,
) -> Result<(Label, Option<Undecided>), ReconstructError> {
    let (mut sum, mut count) = ([0_i128; 3], 0_i128);
    for face in faces {
        for corner in corners(face)? {
            let g = corner.to_grid()?.value();
            for (s, v) in sum.iter_mut().zip([g.x.get(), g.y.get(), g.z.get()]) {
                *s = s.saturating_add(i128::from(v));
            }
            count = count.saturating_add(1);
        }
    }
    if count == 0 {
        return Ok((Label::Dropped, None));
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "a sum of at most a few hundred coordinates below 2^40 is below 2^53"
    )]
    let centre = sum.map(|s| s as f64 / count as f64);
    let at = [0, 1, 2].map(|i| {
        let c = centre.get(i).copied().unwrap_or(0.0);
        Interval::new(c - CENTROID_REACH, c + CENTROID_REACH)
    });
    let [x, y, z] = at;
    let box3 = [x?, y?, z?];
    // The nearest grid point to the centroid, from the sums: (sum + count / 2) / count, floored.
    let nearest = |v: i128| -> i64 {
        let half = count / 2;
        i64::try_from(v.saturating_add(half).div_euclid(count)).unwrap_or(0)
    };
    let spot = P3::new(nearest(sum[0]), nearest(sum[1]), nearest(sum[2]))?;
    let half = Interval::point(0.5)?;
    let Some(w) = winding_number(triangles, box3, meter)? else {
        return Ok((
            Label::Undecided,
            Some(Undecided {
                at: spot,
                low: f64::NEG_INFINITY,
                high: f64::INFINITY,
            }),
        ));
    };
    match decide(w, half) {
        Label::Undecided => Ok((
            Label::Undecided,
            Some(Undecided {
                at: spot,
                low: w.lo(),
                high: w.hi(),
            }),
        )),
        label => Ok((label, None)),
    }
}

/// Which side of one half the whole of `w` is on: above keeps the cell, below drops it, and an
/// interval that holds the half decides nothing.
fn decide(w: Interval, half: Interval) -> Label {
    if w.lo() > half.hi() {
        Label::Kept
    } else if w.hi() < half.lo() {
        Label::Dropped
    } else {
        Label::Undecided
    }
}

/// How `plane` divides `fragments`: how many are in front, behind, on it and cut by it.
fn tally(
    plane: &Plane,
    fragments: &[Piece],
    meter: &mut WorkMeter<'_>,
) -> Result<[usize; 4], ReconstructError> {
    let (mut front, mut back, mut on, mut cut) = (0_usize, 0_usize, 0_usize, 0_usize);
    for f in fragments {
        meter.charge(1)?;
        match clip(f, plane)? {
            Clipped::Front => front = front.saturating_add(1),
            Clipped::Back => back = back.saturating_add(1),
            Clipped::On => on = on.saturating_add(1),
            Clipped::Split(..) => cut = cut.saturating_add(1),
        }
    }
    Ok([front, back, on, cut])
}

/// The plane to cut a cell by: among the planes of a few of its fragments, the one that cuts the
/// fewest others, then divides the rest most evenly, the first on a tie.
fn choose(fragments: &[Piece], meter: &mut WorkMeter<'_>) -> Result<Plane, ReconstructError> {
    let stride = (fragments.len() / CANDIDATES).max(1);
    let mut best: Option<((usize, usize), Plane)> = None;
    for f in fragments.iter().step_by(stride).take(CANDIDATES) {
        let plane = canonical(&f.face)?.plane;
        let [front, back, _, cut] = tally(&plane, fragments, meter)?;
        let score = (cut, front.abs_diff(back));
        if best.as_ref().is_none_or(|(held, _)| score < *held) {
            best = Some((score, plane));
        }
    }
    best.map(|(_, plane)| plane)
        .ok_or(ReconstructError::NoSurface)
}

/// The partition of the box with faces `walls`, and how many cuts it took.
struct Partition {
    nodes: Vec<Node>,
    cuts: u32,
    cells: Cells,
    undecided: Vec<Undecided>,
}

/// Cuts the cell with faces `walls` by the planes of `fragments`, to the leaves.
fn partition(
    walls: Vec<Piece>,
    fragments: Vec<Piece>,
    triangles: &[[P3; 3]],
    meter: &mut WorkMeter<'_>,
) -> Result<Partition, ReconstructError> {
    let mut out = Partition {
        nodes: alloc::vec![Node::Leaf(Label::Dropped)],
        cuts: 0,
        cells: Cells {
            kept: 0,
            dropped: 0,
            undecided: 0,
        },
        undecided: Vec::new(),
    };
    let mut stack = alloc::vec![(
        0_usize,
        Cell {
            faces: walls,
            fragments,
        }
    )];
    while let Some((at, cell)) = stack.pop() {
        if cell.fragments.is_empty() {
            let (label, note) = label_of(&cell.faces, triangles, meter)?;
            match label {
                Label::Kept => out.cells.kept = out.cells.kept.saturating_add(1),
                Label::Dropped => out.cells.dropped = out.cells.dropped.saturating_add(1),
                Label::Undecided => out.cells.undecided = out.cells.undecided.saturating_add(1),
            }
            out.undecided.extend(note);
            if let Some(node) = out.nodes.get_mut(at) {
                *node = Node::Leaf(label);
            }
            continue;
        }
        let plane = choose(&cell.fragments, meter)?;
        let portal = portal_on(&plane, cell.faces.iter().map(|f| f.face), meter)?
            .ok_or(MeshError::Defect)?;
        let (mut front_faces, mut back_faces) = (Vec::new(), Vec::new());
        for face in cell.faces {
            meter.charge(1)?;
            match clip(&face, &plane)? {
                Clipped::Front => front_faces.push(face),
                Clipped::Back => back_faces.push(face),
                Clipped::On => return Err(MeshError::Defect.into()),
                Clipped::Split(front, back) => {
                    front_faces.push(front);
                    back_faces.push(back);
                }
            }
        }
        front_faces.push(Piece {
            face: plane,
            bounds: portal.bounds.clone(),
        });
        back_faces.push(Piece {
            face: opposite(&plane)?,
            bounds: portal.bounds.clone(),
        });
        let (mut front_frags, mut back_frags) = (Vec::new(), Vec::new());
        for f in cell.fragments {
            meter.charge(1)?;
            match clip(&f, &plane)? {
                Clipped::Front => front_frags.push(f),
                Clipped::Back => back_frags.push(f),
                // In the plane: part of the portal, which is the boundary's to decide.
                Clipped::On => {}
                Clipped::Split(front, back) => {
                    front_frags.push(front);
                    back_frags.push(back);
                }
            }
        }
        out.cuts = out.cuts.saturating_add(1);
        let front = out.nodes.len();
        let back = front.saturating_add(1);
        out.nodes.push(Node::Leaf(Label::Dropped));
        out.nodes.push(Node::Leaf(Label::Dropped));
        if let Some(node) = out.nodes.get_mut(at) {
            *node = Node::Split {
                plane,
                portal,
                front,
                back,
            };
        }
        // The back cell goes below the front on the stack, so the front is cut first.
        stack.push((
            back,
            Cell {
                faces: back_faces,
                fragments: back_frags,
            },
        ));
        stack.push((
            front,
            Cell {
                faces: front_faces,
                fragments: front_frags,
            },
        ));
    }
    Ok(out)
}

/// `piece` pushed down the partition from `from`: the parts of it that end in each leaf.
fn leaves(
    piece: Piece,
    from: usize,
    nodes: &[Node],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(Piece, Label)>, ReconstructError> {
    let mut out = Vec::new();
    let mut stack = alloc::vec![(piece, from)];
    while let Some((piece, at)) = stack.pop() {
        match nodes.get(at).ok_or(MeshError::Defect)? {
            Node::Leaf(label) => out.push((piece, *label)),
            Node::Split {
                plane, front, back, ..
            } => {
                meter.charge(1)?;
                match clip(&piece, plane)? {
                    Clipped::Front | Clipped::On => stack.push((piece, *front)),
                    Clipped::Back => stack.push((piece, *back)),
                    Clipped::Split(f, b) => {
                        stack.push((b, *back));
                        stack.push((f, *front));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// `piece` turned to face the other way: the plane opposite, the bounds in the other order.
fn reversed(piece: &Piece) -> Result<Piece, NumError> {
    let mut bounds = piece.bounds.clone();
    bounds.reverse();
    Ok(Piece {
        face: opposite(&piece.face)?,
        bounds,
    })
}

/// The boundary pieces between kept and dropped cells, each facing out of the kept side, with the
/// material behind it.
fn boundary(
    cut: &Partition,
    walls: &[Piece],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, ReconstructError> {
    let mut out = Vec::new();
    // The box's walls: inside is the front, and past them is dropped.
    for wall in walls {
        for (piece, label) in leaves(wall.clone(), 0, &cut.nodes, meter)? {
            if label.is_kept() {
                out.push(reversed(&piece)?);
            }
        }
    }
    for node in &cut.nodes {
        let Node::Split {
            portal,
            front,
            back,
            ..
        } = node
        else {
            continue;
        };
        for (piece, ahead) in leaves(portal.clone(), *front, &cut.nodes, meter)? {
            for (part, behind) in leaves(piece, *back, &cut.nodes, meter)? {
                match (ahead.is_kept(), behind.is_kept()) {
                    // Material in front: the face looks back.
                    (true, false) => out.push(reversed(&part)?),
                    (false, true) => out.push(part),
                    _ => {}
                }
            }
        }
    }
    Ok(out)
}

/// `x`, or 1 if it is less: a divisor that is never zero.
fn at_least_one(x: f64) -> f64 {
    if x > 1.0 { x } else { 1.0 }
}

/// The nearest triangle's distance to `p` in squares, in doubles: for choosing which triangle to
/// ask the exact question of.
#[allow(
    clippy::cast_precision_loss,
    reason = "an estimate to choose among triangles; the exact test follows"
)]
fn approximate(p: P3, t: [P3; 3]) -> f64 {
    let to = |q: P3| difference(q, p).map(|v| v as f64);
    let sq = |v: [f64; 3]| v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    let edge = |a: P3, b: P3| {
        let ab = difference(b, a).map(|v| v as f64);
        let ap = difference(p, a).map(|v| v as f64);
        let c = [
            ab[1] * ap[2] - ab[2] * ap[1],
            ab[2] * ap[0] - ab[0] * ap[2],
            ab[0] * ap[1] - ab[1] * ap[0],
        ];
        sq(c) / at_least_one(sq(ab))
    };
    let Ok(feature) = closest_feature(p, t) else {
        return f64::INFINITY;
    };
    match feature {
        Closest::Vertex(i) => sq(to(*t.get(i).unwrap_or(&t[0]))),
        Closest::Edge(i) => edge(
            *t.get(i).unwrap_or(&t[0]),
            *[1_usize, 2, 0]
                .get(i)
                .and_then(|&next| t.get(next))
                .unwrap_or(&t[1]),
        ),
        Closest::Face => {
            let n = cross(difference(t[1], t[0]), difference(t[2], t[0])).map(|v| v as f64);
            let ap = difference(p, t[0]).map(|v| v as f64);
            let along = n[0] * ap[0] + n[1] * ap[1] + n[2] * ap[2];
            along * along / at_least_one(sq(n))
        }
    }
}

/// The distance from `p` to the nearest of `triangles`, in whole steps rounded up, exact for the
/// triangle the estimate chose.
fn distance_to(
    p: P3,
    triangles: &[[P3; 3]],
    meter: &mut WorkMeter<'_>,
) -> Result<u64, ReconstructError> {
    let mut best: Option<(f64, [P3; 3])> = None;
    for t in triangles {
        meter.charge(1)?;
        let d = approximate(p, *t);
        if best.as_ref().is_none_or(|(held, _)| d < *held) {
            best = Some((d, *t));
        }
    }
    let Some((_, t)) = best else {
        return Ok(0);
    };
    // The smallest clearance the distance does not exceed, by doubling then halving.
    let mut high = 1_u64;
    while point_triangle(p, t, high)? == Sign::Positive {
        high = high.checked_mul(2).ok_or(NumError::OutOfRange {
            value: i128::from(high),
        })?;
    }
    let mut low = 0_u64;
    while low < high {
        let mid = low.saturating_add(high.saturating_sub(low) / 2);
        if point_triangle(p, t, mid)? == Sign::Positive {
            low = mid.saturating_add(1);
        } else {
            high = mid;
        }
    }
    Ok(low)
}

/// The triangles of the convex `pieces`, from their corners rounded to the grid, and those
/// corners.
fn rounded(pieces: &[Outward]) -> Result<(Vec<[P3; 3]>, Vec<P3>), NumError> {
    let mut triangles = Vec::new();
    let mut points = Vec::new();
    for piece in pieces {
        let mut ring = Vec::new();
        for corner in corners(&piece.piece)? {
            ring.push(corner.to_grid()?.value());
        }
        for &p in &ring {
            points.push(p);
        }
        if let Some((&first, rest)) = ring.split_first() {
            for pair in rest.windows(2) {
                if let &[b, c] = pair
                    && !collinear(first, b, c)
                {
                    triangles.push([first, b, c]);
                }
            }
        }
    }
    points.sort_unstable();
    points.dedup();
    Ok((triangles, points))
}

/// Both deviations of the kept `pieces` from `triangles`, each with one step for the rounding.
fn deviation_of(
    pieces: &[Outward],
    triangles: &[[P3; 3]],
    meter: &mut WorkMeter<'_>,
) -> Result<Deviation, ReconstructError> {
    let (result, corners) = rounded(pieces)?;
    let mut from_input = 0_u64;
    for p in corners {
        from_input = from_input.max(distance_to(p, triangles, meter)?);
    }
    let mut vertices: Vec<P3> = triangles.iter().flatten().copied().collect();
    vertices.sort_unstable();
    vertices.dedup();
    let mut from_result = 0_u64;
    for p in vertices {
        from_result = from_result.max(distance_to(p, &result, meter)?);
    }
    Ok(Deviation {
        from_input: Steps(from_input.saturating_add(1)),
        from_result: Steps(from_result.saturating_add(1)),
    })
}

/// The triangles of `faces` on `points` with area, each fan from its first corner.
fn triangles_of(points: &[P3], faces: &[Face]) -> Vec<[P3; 3]> {
    let mut out = Vec::new();
    for face in faces {
        let corners: Vec<P3> = face
            .corners
            .iter()
            .filter_map(|&v| points.get(v as usize).copied())
            .collect();
        if let Some((&first, rest)) = corners.split_first() {
            for pair in rest.windows(2) {
                if let &[b, c] = pair
                    && !collinear(first, b, c)
                {
                    out.push([first, b, c]);
                }
            }
        }
    }
    out
}

/// The box round `triangles`: the bounding box grown by a sixteenth of its longest side, at least
/// 16 steps, as the two corners.
fn around(triangles: &[[P3; 3]]) -> Result<([i128; 3], [i128; 3]), ReconstructError> {
    let (mut low, mut high) = ([i128::MAX; 3], [i128::MIN; 3]);
    for p in triangles.iter().flatten() {
        for (i, v) in [p.x.get(), p.y.get(), p.z.get()].into_iter().enumerate() {
            let v = i128::from(v);
            if let Some(l) = low.get_mut(i) {
                *l = (*l).min(v);
            }
            if let Some(h) = high.get_mut(i) {
                *h = (*h).max(v);
            }
        }
    }
    let longest = low
        .iter()
        .zip(&high)
        .map(|(l, h)| h.saturating_sub(*l))
        .max()
        .unwrap_or(0);
    let pad = (longest / 16).max(16);
    let grown = |v: [i128; 3], sign: i128| v.map(|c| c.saturating_add(sign.saturating_mul(pad)));
    let (low, high) = (grown(low, -1), grown(high, 1));
    if low.iter().chain(&high).any(|c| c.abs() > FRAME) {
        let worst = low.iter().chain(&high).map(|c| c.abs()).max().unwrap_or(0);
        return Err(NumError::OutOfRange { value: worst }.into());
    }
    Ok((low, high))
}

/// The six faces of the box from `low` to `high`, each positive inside.
fn walls_of(
    (low, high): ([i128; 3], [i128; 3]),
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, ReconstructError> {
    let mut planes = Vec::new();
    for axis in 0..3 {
        let l = low.get(axis).copied().unwrap_or(0);
        let h = high.get(axis).copied().unwrap_or(0);
        planes.push(Plane::new(unit(axis, 1), l.saturating_neg())?);
        planes.push(Plane::new(unit(axis, -1), h)?);
    }
    let mut walls = Vec::new();
    for (i, plane) in planes.iter().enumerate() {
        let others = planes
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| *p);
        walls.push(portal_on(plane, others, meter)?.ok_or(MeshError::Defect)?);
    }
    Ok(walls)
}

/// The boundary pieces as a merge takes them: material behind, nothing in front.
fn outward(pieces: Vec<Piece>) -> Result<Vec<Outward>, ReconstructError> {
    let mut back = Winding::new();
    back.add(0, 1).map_err(|_| MeshError::Defect)?;
    Ok(pieces
        .into_iter()
        .zip(0_usize..)
        .map(|(piece, n)| Outward {
            piece,
            polygon: n,
            turned: false,
            source: n,
            front: Winding::new(),
            back: back.clone(),
        })
        .collect())
}

/// `raw` reconstructed by winding number, its faces one run of `source`.
///
/// # Errors
///
/// [`ReconstructError::NoSurface`] for faces with no area; [`ReconstructError::NothingKept`] when
/// no cell is inside; [`ReconstructError::Heal`] for what welding the vertices to their keys and
/// orienting the patches refuse; and [`ReconstructError::Mesh`] for the mesh checker's faults, a
/// coordinate that leaves the frame once the box is grown, a stopped meter or a refused
/// allocation.
pub fn reconstruct(
    raw: &RawMesh,
    settings: Reconstruct,
    source: Source,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Reconstructed, ReconstructError> {
    let welded = weld(raw, Steps::ZERO, exec, meter)?;
    let points = welded.mesh.vertices();
    let mut faces = Vec::new();
    for (f, &input) in welded.source.iter().enumerate() {
        let start = *welded.mesh.face_start().get(f).ok_or(MeshError::Defect)? as usize;
        let end = *welded
            .mesh
            .face_start()
            .get(f.saturating_add(1))
            .ok_or(MeshError::Defect)? as usize;
        let corners = welded
            .mesh
            .corners()
            .get(start..end)
            .ok_or(MeshError::Defect)?;
        faces.push(Face {
            corners: corners.to_vec(),
            input,
            moved: false,
        });
    }
    let oriented = orient(points, faces, meter)?;
    // In key order, so the winding numbers' sums run in the same order whatever the faces' order.
    let mut triangles = triangles_of(points, &oriented.faces);
    triangles.sort_unstable();
    if triangles.is_empty() {
        return Err(ReconstructError::NoSurface);
    }
    let mut fragments = Vec::new();
    for t in &triangles {
        fragments.push(triangle_piece(*t)?);
    }
    let walls = walls_of(around(&triangles)?, meter)?;
    let cut = partition(walls.clone(), fragments, &triangles, meter)?;
    if cut.cells.kept == 0 {
        return Err(ReconstructError::NothingKept {
            undecided: cut.undecided,
        });
    }
    let pieces = outward(boundary(&cut, &walls, meter)?)?;
    let deviation = deviation_of(&pieces, &triangles, meter)?;
    let merged = merge_on(
        exec,
        &pieces,
        (Merge::RemoveRedundant, Rules::default()),
        meter,
    )
    .map_err(MeshError::from)?;
    let worst = if deviation.from_input.get() >= deviation.from_result.get() {
        deviation.from_input
    } else {
        deviation.from_result
    };
    let class = Exactness::Approximate {
        deviation: worst,
        step: settings.step,
    };
    let face_run = alloc::vec![0_u32; merged.faces.len()];
    let polygons = polygons_of(&merged, (face_run, alloc::vec![source], class), exec, meter)?;
    // The boundary of a set of cells has a winding number of 1 inside it and 0 outside it.
    let mesh = build(&polygons, exec, meter)?.proven_solid();
    let log = ReconstructLog {
        triangles: crate::exact::index(triangles.len())?,
        cuts: cut.cuts,
        cells: cut.cells,
        pieces: crate::exact::index(pieces.len())?,
        deviation,
    };
    Ok(Reconstructed {
        mesh,
        undecided: cut.undecided,
        log,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64, z: i64) -> P3 {
        P3::new(x, y, z).unwrap()
    }

    /// An interval wholly above a half keeps a cell, wholly below drops it, and one that holds it
    /// decides nothing, at either end.
    #[test]
    fn a_winding_interval_that_holds_a_half_is_undecided() {
        let half = Interval::point(0.5).unwrap();
        let w = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
        assert_eq!(decide(w(0.75, 1.25), half), Label::Kept);
        assert_eq!(decide(w(-0.25, 0.25), half), Label::Dropped);
        assert_eq!(decide(w(0.25, 0.75), half), Label::Undecided);
        assert_eq!(decide(w(0.5, 0.75), half), Label::Undecided);
        assert_eq!(decide(w(0.25, 0.5), half), Label::Undecided);
        assert_eq!(decide(w(0.5, 0.5), half), Label::Undecided);
        assert_eq!(decide(w(0.500_000_1, 0.6), half), Label::Kept);
    }

    /// The partition's big square is a piece whose corners all lie on the plane and in turn.
    #[test]
    fn a_big_square_has_four_corners_on_its_plane() {
        let plane = Plane::through(p(0, 0, 5), p(10, 0, 5), p(0, 10, 5)).unwrap();
        let square = big_square(&plane).unwrap();
        let cs = corners(&square).unwrap();
        assert_eq!(cs.len(), 4);
        for c in &cs {
            assert_eq!(plane.side_of_tpi(c).unwrap(), Sign::Zero);
        }
    }
}
