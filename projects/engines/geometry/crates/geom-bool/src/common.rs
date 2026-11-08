//! Common blocks: pave blocks of different edges that lie on one curve, such as collinear wall
//! edges or two cylinders sharing a line (11.4 step 3, G243).
//!
//! What it does: [`common_blocks`] groups the pave blocks of an arrangement's edges that run
//! between the same two vertices, and in each group proves which blocks are one stretch of curve.
//! Two straight blocks on grid data are compared exactly: their lines are one when their
//! directions and the gap between their origins are parallel, in whole numbers. Any other pair is
//! held to the contact rule over its whole length, both ways: block A is cut into pieces, each
//! piece's box is measured from the other block's curve by the certified projection
//! (`geom_intersect::project::project_curve`), and the pair is one only when no piece is proven
//! apart and every piece, at the finest refinement, lies within the contact radius. So a circle
//! stored as a circle and a NURBS close to it form a common block only by the contact rule, and a
//! circle and a line through the same two vertices never do.
//!
//! Guarantees:
//!
//! - **Exact within one analytic kind on grid data.** Two lines whose origins and directions are
//!   whole numbers of steps are one or not by an exact test in 128-bit integers, with no
//!   enclosure; the common block says [`Proof::Exact`].
//! - **The contact rule across kinds.** Any other pair is one only when every piece of each is
//!   proven within the contact radius of the other at the finest refinement, and is apart as soon
//!   as one piece is proven apart; a piece neither apart nor touching at the deepest cut fails with
//!   [`crate::rule::NearContact`]. Never a sampled check.
//! - **Blocks of one operand are never common.** An operand's own edges meet only at their
//!   vertices, so only blocks of different operands are compared.
//! - **Deterministic.** Groups are taken in the order of their end vertices, blocks within a group
//!   in edge order, and each common block lists its members in that order.
//!
//! Method: chapter 11.4, "Common blocks"
//! (`projects/engines/geometry/architecture/11-booleans.md`), the common blocks of `OpenCascade`'s
//! General Fuse studied for method only
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`), decided exactly within one
//! analytic kind and by the contact rule across kinds. The distance of a curve from another is
//! bounded both ways on the boxes of its pieces: a ball round each box, and the certified distance
//! of the ball's centre from the other curve.
//!
//! Cost: one exact test per straight pair; otherwise at most 2^`DEEPEST` pieces each way, each a
//! box and one certified projection, all charged to the meter. No recursion.

use alloc::vec::Vec;

use geom_intersect::IntersectError;
use geom_intersect::project::{Projection, project_curve};
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::math::round_ties_even;
use geom_run::WorkMeter;
use geom_topo::{CurveKind, ParamCurve};

use crate::distance::FIRST_FLOOR;
use crate::interfere::{Arrangement, BuildError, Entity};
use crate::pave::PaveBlock;
use crate::rule::{ContactError, EPS_FIT, NearContact, REFINEMENTS};

/// The deepest a block is cut, in halvings, before a piece that is neither apart nor touching
/// fails.
pub const DEEPEST: u32 = 24;

/// What proves that blocks are one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Proof {
    /// Their carriers are one, by an exact test.
    Exact,
    /// Every piece of each lies within the contact radius of the other: the widest enclosure of a
    /// piece's distance from the other block.
    Contact(Interval),
}

/// Pave blocks of different edges that are one stretch of curve.
#[derive(Clone, Debug, PartialEq)]
pub struct CommonBlock {
    /// The blocks, by index into the list given, in edge order.
    pub members: Vec<usize>,
    /// What proves them one: the weakest proof among the pairs that joined them.
    pub proof: Proof,
}

/// A double that is a whole number, as an integer.
fn whole(x: f64) -> Option<i128> {
    let r = round_ties_even(x).ok()?.get();
    if r != x || r.abs() > 9.0e15 {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number below 2^53 in size, which an integer holds exactly"
    )]
    Some(i128::from(r as i64))
}

/// Three doubles as whole numbers, if all are.
fn whole3(v: [f64; 3]) -> Option<[i128; 3]> {
    let [x, y, z] = v;
    Some([whole(x)?, whole(y)?, whole(z)?])
}

/// The cross product of two integer vectors, if it holds in 128 bits.
fn cross(a: [i128; 3], b: [i128; 3]) -> Option<[i128; 3]> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let m = |p: i128, q: i128, r: i128, s: i128| p.checked_mul(q)?.checked_sub(r.checked_mul(s)?);
    Some([m(a1, b2, a2, b1)?, m(a2, b0, a0, b2)?, m(a0, b1, a1, b0)?])
}

/// Whether two lines on grid data are one line: `Some(true)` or `Some(false)` exactly, `None` when
/// either is not on grid data or a product passes 128 bits.
fn same_line(a: &CurveKind<3>, b: &CurveKind<3>) -> Option<bool> {
    let (CurveKind::Line(la), CurveKind::Line(lb)) = (a, b) else {
        return None;
    };
    let (oa, da) = (whole3(la.origin())?, whole3(la.direction())?);
    let (ob, db) = (whole3(lb.origin())?, whole3(lb.direction())?);
    let [o0, o1, o2] = oa;
    let [p0, p1, p2] = ob;
    let gap = [
        p0.checked_sub(o0)?,
        p1.checked_sub(o1)?,
        p2.checked_sub(o2)?,
    ];
    let parallel = cross(da, db)? == [0, 0, 0];
    let through = cross(gap, da)? == [0, 0, 0];
    Some(parallel && through)
}

/// What a one-sided distance came to.
enum Side {
    /// Some piece is proven apart from the other block.
    Apart,
    /// Every piece is within the contact radius: the widest enclosure.
    Touch(Interval),
}

/// An edge's curve and a span of its parameter.
type Span<'a> = (&'a CurveKind<3>, (f64, f64));

/// The edge curve and parameter span of a block: `inner` gives the span inside the end paves'
/// enclosures, otherwise the span covering them.
fn span_of<'a>(
    arr: &'a Arrangement,
    block: &PaveBlock,
    inner: bool,
) -> Result<Option<Span<'a>>, BuildError> {
    let edge = arr
        .edges
        .get(block.edge)
        .ok_or(BuildError::Missing(Entity::Edge(block.edge)))?;
    let curve = arr
        .carriers
        .curve(edge.curve)
        .ok_or(BuildError::Missing(Entity::Edge(block.edge)))?;
    let (Some(from), Some(to)) = (block.from, block.to) else {
        return Ok(None);
    };
    // A block that runs round a seam is not one interval of its edge's parameter; it is left to
    // the face splitting, which cuts at the seam.
    if block.wraps {
        return Ok(None);
    }
    let (a, b) = curve.domain();
    let (lo, hi) = if inner {
        (from.t.hi(), to.t.lo())
    } else {
        (
            if from.t.lo() > a { from.t.lo() } else { a },
            if to.t.hi() < b { to.t.hi() } else { b },
        )
    };
    Ok((lo < hi).then_some((curve, (lo, hi))))
}

/// The grid-aligned centre of a box and the radius of a ball about it holding the box.
fn ball(lo: [f64; 3], hi: [f64; 3]) -> Result<([f64; 3], f64), NumError> {
    let mut centre = [0.0; 3];
    let mut sum = 0.0_f64;
    for ((c, &a), &b) in centre.iter_mut().zip(&lo).zip(&hi) {
        *c = round_ties_even(a / 2.0 + b / 2.0)?.get();
        // Each difference and product rounded up, so the sum is at least the true one.
        let reach = (if (b - *c) > (*c - a) { b - *c } else { *c - a }).next_up();
        sum = ((reach * reach).next_up() + sum).next_up();
    }
    let radius = Interval::point(sum)?.sqrt()?.hi();
    Ok((centre, radius))
}

/// The one-sided distance of block `a` (its inner span) from block `b` (its outer span), held to
/// the contact rule piece by piece.
fn one_side(
    near_side: Span<'_>,
    far_side: Span<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Side, BuildError> {
    let (curve_a, (a0, a1)) = near_side;
    let (curve_b, (b0, b1)) = far_side;
    #[allow(
        clippy::cast_precision_loss,
        reason = "`ε_fit` is a count of 64 steps, which a double holds exactly"
    )]
    let radius = EPS_FIT.0 as f64;
    let mut widest: Option<Interval> = None;
    let mut stack: Vec<(f64, f64, u32)> = alloc::vec![(a0, a1, 0)];
    while let Some((lo, hi, depth)) = stack.pop() {
        meter.charge(1).map_err(IntersectError::from)?;
        let bounds = curve_a
            .bound((lo, hi), meter)
            .map_err(IntersectError::from)?;
        let (centre, reach) = ball(bounds.lo, bounds.hi)?;
        let level = depth.min(REFINEMENTS);
        let scale = (0..level).fold(1.0_f64, |s, _| s * 2.0);
        let (near, inside) = match project_curve(curve_b, &centre, FIRST_FLOOR / scale, meter)? {
            Projection::Points(found) => {
                let mut best: Option<(Interval, bool)> = None;
                for nearest in &found {
                    let param = nearest
                        .at
                        .first()
                        .copied()
                        .ok_or(IntersectError::Internal)?;
                    let within = param.lo() >= b0 && param.hi() <= b1;
                    best = match best {
                        Some((dist, held)) if dist.lo() <= nearest.distance.lo() => {
                            Some((dist, held))
                        }
                        _ => Some((nearest.distance, within)),
                    };
                }
                best.ok_or(IntersectError::Internal)?
            }
            Projection::Set { distance, .. } => (distance, true),
            Projection::NonIsolated { .. } => (Interval::new(0.0, f64::MAX)?, false),
        };
        // The distance of every point of the piece from the other block: at least the centre's
        // distance from the whole curve less the radius, and at most its distance from a point of
        // the block plus the radius.
        let floor = near.lo() - reach;
        if floor > 0.0 {
            return Ok(Side::Apart);
        }
        let ceiling = if inside {
            near.hi() + reach
        } else {
            f64::INFINITY
        };
        let enclosure = Interval::new(floor, ceiling)?;
        if depth >= REFINEMENTS && ceiling < radius {
            widest = Some(match widest {
                Some(w) if w.hi() >= ceiling => w,
                _ => enclosure,
            });
            continue;
        }
        if depth >= DEEPEST {
            return Err(ContactError::NearContact(NearContact {
                enclosure,
                refinement: depth,
            })
            .into());
        }
        let mid = lo / 2.0 + hi / 2.0;
        stack.push((mid, hi, depth.saturating_add(1)));
        stack.push((lo, mid, depth.saturating_add(1)));
    }
    Ok(Side::Touch(widest.ok_or(IntersectError::Internal)?))
}

/// Whether two blocks of different edges, between the same two vertices, are one stretch of curve.
fn coincide(
    arr: &Arrangement,
    a: &PaveBlock,
    b: &PaveBlock,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Proof>, BuildError> {
    let (Some(outer_a), Some(outer_b)) = (span_of(arr, a, false)?, span_of(arr, b, false)?) else {
        return Ok(None);
    };
    if let Some(one) = same_line(outer_a.0, outer_b.0) {
        return Ok(one.then_some(Proof::Exact));
    }
    let (Some(inner_a), Some(inner_b)) = (span_of(arr, a, true)?, span_of(arr, b, true)?) else {
        return Ok(None);
    };
    let there = one_side(inner_a, outer_b, meter)?;
    let back = one_side(inner_b, outer_a, meter)?;
    match (there, back) {
        (Side::Touch(x), Side::Touch(y)) => {
            Ok(Some(Proof::Contact(if x.hi() >= y.hi() { x } else { y })))
        }
        _ => Ok(None),
    }
}

/// The weaker of two proofs.
fn weaker(a: Proof, b: Proof) -> Proof {
    match (a, b) {
        (Proof::Exact, other) | (other, Proof::Exact) => other,
        (Proof::Contact(x), Proof::Contact(y)) => {
            Proof::Contact(if x.hi() >= y.hi() { x } else { y })
        }
    }
}

/// The common blocks among `blocks`: groups of blocks of different operands' edges, between the
/// same two vertices, proven one stretch of curve.
///
/// # Errors
///
/// [`BuildError::Contact`] for a pair the contact rule cannot decide, or a stopped meter;
/// [`BuildError::Missing`] for a block of an edge that is not there.
pub fn common_blocks(
    arr: &Arrangement,
    blocks: &[PaveBlock],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<CommonBlock>, BuildError> {
    let mut keyed: Vec<((usize, usize), usize, usize)> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if let Some(ends) = block.ends() {
            keyed.push((ends, block.edge, i));
        }
    }
    keyed.sort_unstable();
    let operand = |b: &PaveBlock| arr.edges.get(b.edge).map(|e| e.operand);
    let mut out: Vec<CommonBlock> = Vec::new();
    let mut start = 0_usize;
    while let Some(&(ends, _, _)) = keyed.get(start) {
        let len = keyed
            .iter()
            .skip(start)
            .take_while(|(k, _, _)| *k == ends)
            .count();
        let group: Vec<usize> = keyed
            .iter()
            .skip(start)
            .take(len)
            .map(|(_, _, i)| *i)
            .collect();
        start = start.saturating_add(len);
        // Join the blocks proven one, each to the first of its group it is one with.
        let mut joined: Vec<Option<(usize, Proof)>> = alloc::vec![None; group.len()];
        for (j, &bj) in group.iter().enumerate() {
            let block_j = blocks.get(bj).ok_or(IntersectError::Internal)?;
            for (k, &bk) in group.iter().enumerate().take(j) {
                let block_k = blocks.get(bk).ok_or(IntersectError::Internal)?;
                if operand(block_j) == operand(block_k) {
                    continue;
                }
                if let Some(proof) = coincide(arr, block_k, block_j, meter)? {
                    let root = joined.get(k).copied().flatten().map_or(k, |(r, _)| r);
                    if let Some(slot) = joined.get_mut(j) {
                        *slot = Some((root, proof));
                    }
                    break;
                }
            }
        }
        for (r, &br) in group.iter().enumerate() {
            if joined.get(r).copied().flatten().is_some() {
                continue;
            }
            let mut members = alloc::vec![br];
            let mut proof = Proof::Exact;
            for (j, slot) in joined.iter().enumerate() {
                if let Some((_, p)) = slot.filter(|(root, _)| *root == r) {
                    members.push(group.get(j).copied().ok_or(IntersectError::Internal)?);
                    proof = weaker(proof, p);
                }
            }
            if members.len() > 1 {
                out.push(CommonBlock { members, proof });
            }
        }
    }
    Ok(out)
}
