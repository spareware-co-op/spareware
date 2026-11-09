//! Step 4 of healing (19.7): each patch faces one way, and each closed patch faces out.
//!
//! What it does: a manifold edge is one exactly two faces use. A patch is the faces such edges
//! join. Orientation floods across each patch from its lowest-keyed face, which keeps its turn:
//! a neighbour that runs along the shared edge the same way is turned. A patch that, once flooded,
//! uses each of its edges as often one way as the other is closed on its own, even where it
//! touches another patch along an edge, and it is turned outward when its exact volume is
//! negative. A face the
//! turning makes repeat another the same way round is kept once. Last, every edge is counted each
//! way: one used more often one way than the other cannot be paired, and is listed.
//!
//! Guarantees:
//!
//! - **Exact.** The volume is six times the signed volume, the sum of integer determinants of a fan
//!   of each face from the frame's origin, each below 2^123, added in 256 bits.
//! - **Deterministic.** Faces are keyed by their corners from the lowest, so the patches, their
//!   floods and every turn are the same whatever order the faces came in.
//! - **Every turn and every failure said.** The log lists the input faces turned, an edge of each
//!   patch that cannot be oriented, each closed patch with no volume, the repeats removed, and the
//!   edges that cannot be paired.
//!
//! Method: 19.7's step 4 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): a
//! breadth-first flood over manifold edges, and 19.2's mass properties for the volume's sign.
//!
//! Cost: a sort of the edge uses and of the faces, then linear in the uses for the flood and the
//! volume.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use geom_num::grid::P3;
use geom_num::wide::I256;
use geom_num::{NumError, Sign};
use geom_run::WorkMeter;

use super::HealError;
use super::loops::{Face, by_edge, cross, dot, point, reversed, uses};
use crate::exact::{MeshError, index};

/// What step 4 did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrientLog {
    /// How many patches there were.
    pub patches: u32,
    /// The input faces turned round, ascending.
    pub turned: Vec<u32>,
    /// For each patch that cannot be oriented, an edge where its flood met itself turned, its
    /// lower-keyed end first.
    pub non_orientable: Vec<[P3; 2]>,
    /// Each closed patch with no volume, by the lowest corner of its lowest-keyed face.
    pub flat: Vec<P3>,
    /// The input faces whose parts were removed for repeating another the same way round once
    /// turned, ascending.
    pub repeated: Vec<u32>,
}

/// Faces after step 4, the edges that cannot be paired, and what was done.
pub(crate) struct Oriented {
    pub faces: Vec<Face>,
    pub unpaired: Vec<[P3; 2]>,
    pub log: OrientLog,
}

/// `faces` on `points` oriented patch by patch, closed patches outward.
pub(crate) fn orient(
    points: &[P3],
    mut faces: Vec<Face>,
    meter: &mut WorkMeter<'_>,
) -> Result<Oriented, HealError> {
    let all = uses(&faces)?;
    // Each face's manifold neighbours: (face, neighbour, whether they run along it the same way,
    // the edge's ends).
    let mut next: Vec<(u32, u32, bool, u32, u32)> = Vec::new();
    for group in by_edge(&all) {
        meter.charge(1)?;
        if let &[a, b] = group {
            let same = a.rising == b.rising;
            next.push((a.face, b.face, same, a.low, a.high));
            next.push((b.face, a.face, same, a.low, a.high));
        }
    }
    next.sort_unstable();
    let mut order: Vec<usize> = (0..faces.len()).collect();
    order.sort_by(|&a, &b| {
        let corners = |i: usize| faces.get(i).map(|f| &f.corners);
        corners(a).cmp(&corners(b)).then(a.cmp(&b))
    });
    let mut log = OrientLog::default();
    let mut turn: Vec<Option<bool>> = vec![None; faces.len()];
    let mut queue = VecDeque::new();
    for &start in &order {
        if turn.get(start).copied().flatten().is_some() {
            continue;
        }
        log.patches = log.patches.saturating_add(1);
        let (mut members, mut met) = (Vec::new(), false);
        set(&mut turn, start, false);
        queue.push_back(start);
        while let Some(f) = queue.pop_front() {
            meter.charge(1)?;
            members.push(f);
            let mine = turn.get(f).copied().flatten().unwrap_or(false);
            let face = index(f)?;
            let first = next.partition_point(|n| n.0 < face);
            for &(at, g, same, low, high) in next.get(first..).unwrap_or(&[]) {
                if at != face {
                    break;
                }
                let want = mine ^ same;
                match turn.get(g as usize).copied().flatten() {
                    None => {
                        set(&mut turn, g as usize, want);
                        queue.push_back(g as usize);
                    }
                    Some(had) if had != want && !met => {
                        met = true;
                        log.non_orientable
                            .push([point(points, low)?, point(points, high)?]);
                    }
                    Some(_) => {}
                }
            }
        }
        if !met && closed(&faces, &members, &turn)? {
            match volume(points, &faces, &members, &turn, meter)? {
                Sign::Negative => {
                    for &f in &members {
                        let was = turn.get(f).copied().flatten().unwrap_or(false);
                        set(&mut turn, f, !was);
                    }
                }
                Sign::Zero => {
                    let corner = faces
                        .get(start)
                        .and_then(|f| f.corners.first())
                        .ok_or(MeshError::Defect)?;
                    log.flat.push(point(points, *corner)?);
                }
                Sign::Positive => {}
            }
        }
    }
    for (face, t) in faces.iter_mut().zip(&turn) {
        if *t == Some(true) {
            face.corners = reversed(&face.corners);
            log.turned.push(face.input);
        }
    }
    log.turned.sort_unstable();
    log.turned.dedup();
    let faces = unrepeated(faces, &mut log.repeated);
    let unpaired = unpaired(points, &faces)?;
    Ok(Oriented {
        faces,
        unpaired,
        log,
    })
}

/// Whether `members`, turned as `turn` says, use each of their edges as often one way as the
/// other: a surface closed on its own.
fn closed(faces: &[Face], members: &[usize], turn: &[Option<bool>]) -> Result<bool, HealError> {
    let mut ways: Vec<(u32, u32, bool)> = Vec::new();
    for &f in members {
        let face = faces.get(f).ok_or(MeshError::Defect)?;
        let turned = turn.get(f).copied().flatten() == Some(true);
        let looped = &face.corners;
        for (&a, &b) in looped.iter().zip(looped.iter().cycle().skip(1)) {
            ways.push((a.min(b), a.max(b), (a < b) != turned));
        }
    }
    ways.sort_unstable();
    Ok(ways
        .chunk_by(|x, y| (x.0, x.1) == (y.0, y.1))
        .all(|edge| edge.iter().filter(|w| w.2).count().saturating_mul(2) == edge.len()))
}

/// Records face `f`'s turn.
fn set(turn: &mut [Option<bool>], f: usize, to: bool) {
    if let Some(slot) = turn.get_mut(f) {
        *slot = Some(to);
    }
}

/// The sign of the volume `members`, turned as `turn` says, bound: six times it is the sum over
/// each face's fan of det[a; b; c].
fn volume(
    points: &[P3],
    faces: &[Face],
    members: &[usize],
    turn: &[Option<bool>],
    meter: &mut WorkMeter<'_>,
) -> Result<Sign, HealError> {
    let mut sum = I256::from_i128(0);
    for &f in members {
        let face = faces.get(f).ok_or(MeshError::Defect)?;
        let mut part = I256::from_i128(0);
        let Some((&first, rest)) = face.corners.split_first() else {
            continue;
        };
        let a = point(points, first)?;
        for pair in rest.windows(2) {
            meter.charge(1)?;
            let &[b, c] = pair else { continue };
            let det = determinant(a, point(points, b)?, point(points, c)?);
            part = part
                .checked_add(&I256::from_i128(det))
                .ok_or(NumError::BudgetExceeded)?;
        }
        if turn.get(f).copied().flatten() == Some(true) {
            part = part.checked_neg().ok_or(NumError::BudgetExceeded)?;
        }
        sum = sum.checked_add(&part).ok_or(NumError::BudgetExceeded)?;
    }
    Ok(sum.signum())
}

/// det[a; b; c] = a · (b × c) of grid points: below 2^123 (19.2, "Mass properties").
fn determinant(a: P3, b: P3, c: P3) -> i128 {
    let wide = |p: P3| [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
    dot(wide(a), cross(wide(b), wide(c)))
}

/// `faces` without a loop repeated the same way round: the first is kept, each repeat listed.
fn unrepeated(faces: Vec<Face>, repeated: &mut Vec<u32>) -> Vec<Face> {
    let corners = |i: usize| faces.get(i).map(|f| &f.corners);
    let mut order: Vec<usize> = (0..faces.len()).collect();
    order.sort_by(|&a, &b| corners(a).cmp(&corners(b)).then(a.cmp(&b)));
    let mut gone = vec![false; faces.len()];
    for run in order.chunk_by(|&a, &b| corners(a) == corners(b)) {
        for &i in run.get(1..).unwrap_or(&[]) {
            if let (Some(slot), Some(f)) = (gone.get_mut(i), faces.get(i)) {
                *slot = true;
                repeated.push(f.input);
            }
        }
    }
    repeated.sort_unstable();
    faces
        .into_iter()
        .zip(gone)
        .filter_map(|(f, g)| (!g).then_some(f))
        .collect()
}

/// The edges of `faces` used more often one way than the other, the lower-keyed end first.
fn unpaired(points: &[P3], faces: &[Face]) -> Result<Vec<[P3; 2]>, HealError> {
    let all = uses(faces)?;
    let mut out = Vec::new();
    for group in by_edge(&all) {
        let rising = group.iter().filter(|u| u.rising).count();
        if rising.saturating_mul(2) != group.len()
            && let Some(u) = group.first()
        {
            out.push([point(points, u.low)?, point(points, u.high)?]);
        }
    }
    Ok(out)
}
