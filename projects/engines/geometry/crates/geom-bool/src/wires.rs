//! Wire targets of Keep and Trim: a wire's edges cut where the other operands' faces cross them,
//! each piece chosen by the others' winding numbers or by seeds (11.1).
//!
//! What it does: each straight edge of the target runs between grid points, on the line two planes
//! through its ends meet in. A polygon of another operand whose plane has the edge's ends strictly
//! on either side, and holds the meeting point within its bounds, splits the edge there: a point
//! where three planes meet. The splits are put in order along the edge by the sign each split's
//! plane takes at the others. Each piece between stops takes the vector just past its start,
//! traced from the corner of a box around every polygon and the wire, with the plane it starts on
//! facing along the edge and moved towards its end, as a piece of face takes the vector in front
//! of it (11.2). [`kept`] keeps the pieces whose vector passes the test, a run of kept pieces along
//! one edge as one; [`trimmed`] keeps the runs of pieces, unbroken by a split or by a wire vertex
//! on another operand's face, that hold a seed. Both give the [`Curves`] the wire rebuild takes,
//! their vertices in the order the wire meets them.
//!
//! Guarantees:
//!
//! - **Exact.** Every stop is the meeting of three planes and every comparison a sign; nothing
//!   rounds but the published edges' shadows (8.3).
//! - **Chosen by the others alone.** A wire has no inside and adds nothing to any vector, so a piece
//!   is never classified by its own wire (11.1).
//! - **Refused, never guessed.** A seed on no piece is [`CombineDetail::SeedOutside`], and one on a
//!   split, on a wire vertex on another operand's face, or between pieces of two runs, is
//!   [`CombineDetail::SeedOnBoundary`].
//!
//! Method: chapter 11.1's sheet and wire targets and 11.2's winding vectors
//! (`projects/engines/geometry/architecture/11-booleans.md`).
//!
//! Cost: O(e · p) plane tests for the e edges of the target and the p polygons of the others, and
//! a trace over every polygon for each piece. Charges the meter what the traces charge.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_op::CombineDetail;
use geom_poly::merge::Curves;
use geom_poly::trace::{Cell, Polygon, TraceError, winding_at};
use geom_poly::winding::{Winding, WindingTest};
use geom_run::WorkMeter;

use crate::operand::turned;

/// Why a wire target gave no curves.
#[derive(Clone, Copy, Debug)]
pub(crate) enum WireError {
    /// The numbers layer refused.
    Num(NumError),
    /// A trace refused, or the meter stopped it.
    Trace(TraceError),
    /// The seeds do not choose pieces.
    Detail(CombineDetail),
}

impl From<NumError> for WireError {
    fn from(e: NumError) -> Self {
        WireError::Num(e)
    }
}

impl From<TraceError> for WireError {
    fn from(e: TraceError) -> Self {
        WireError::Trace(e)
    }
}

/// Where a piece of an edge starts or ends.
#[derive(Clone, Copy, Debug)]
struct Stop {
    /// The point.
    point: Tpi,
    /// A plane through it across the edge, positive towards the edge's end.
    across: Plane,
    /// The wire vertex it is, for an end of the edge.
    vertex: Option<P3>,
    /// Which split it is, counted over every edge, for a split.
    split: Option<usize>,
}

/// A piece of an edge of the target, between two stops, with the vector along it.
#[derive(Clone, Debug)]
pub(crate) struct Run {
    /// The edge, as a place in the target's segments.
    edge: usize,
    /// The two planes whose line holds the edge.
    line: [Plane; 2],
    from: Stop,
    to: Stop,
    vector: Winding,
}

fn coordinates(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// A grid point one step from `p` along `axis`, either way.
fn beside(p: P3, axis: usize) -> Result<P3, NumError> {
    let [x, y, z] = coordinates(p);
    let step = |by: i64| {
        let moved = |k: usize, v: i64| if k == axis { v.saturating_add(by) } else { v };
        P3::new(moved(0, x), moved(1, y), moved(2, z))
    };
    step(1).or_else(|_| step(-1))
}

/// The plane through `p` square to `axis`, positive where that coordinate is greater, or less when
/// `toward` is negative.
fn square(p: P3, axis: usize, toward: Sign) -> Result<Plane, NumError> {
    let v = coordinates(p)
        .get(axis)
        .copied()
        .ok_or(NumError::Degenerate)?;
    let (unit, d) = if toward == Sign::Negative {
        (-1, i128::from(v))
    } else {
        (1, i128::from(v).saturating_neg())
    };
    let n = [0usize, 1, 2].map(|k| if k == axis { unit } else { 0 });
    Plane::new(n, d)
}

/// Two planes through `a` and `b`, whose line is the edge's, and the axis along which the edge
/// changes most with the way it changes there: neither other axis is parallel to the edge.
fn line(a: P3, b: P3) -> Result<([Plane; 2], usize, Sign), NumError> {
    let (ca, cb) = (coordinates(a), coordinates(b));
    let mut axis = 0;
    let mut most = 0i128;
    for (k, (u, v)) in ca.iter().zip(&cb).enumerate() {
        let change = i128::from(*v).saturating_sub(i128::from(*u));
        if change.saturating_abs() > most {
            most = change.saturating_abs();
            axis = k;
        }
    }
    if most == 0 {
        return Err(NumError::Degenerate);
    }
    let along = ca.get(axis).zip(cb.get(axis));
    let toward = if along.is_some_and(|(u, v)| v > u) {
        Sign::Positive
    } else {
        Sign::Negative
    };
    let [i, j] = [axis.saturating_add(1) % 3, axis.saturating_add(2) % 3];
    let planes = [
        Plane::through(a, b, beside(a, i)?)?,
        Plane::through(a, b, beside(a, j)?)?,
    ];
    Ok((planes, axis, toward))
}

/// Whether the meeting point `t` lies on `polygon`, its boundary included.
fn holds(polygon: &Polygon, t: &Tpi) -> Result<bool, NumError> {
    for bound in &polygon.piece.bounds {
        if bound.side_of_tpi(t)? == Sign::Negative {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether the grid point `p` lies on `polygon`, its boundary included.
fn holds_point(polygon: &Polygon, p: P3) -> Result<bool, NumError> {
    if polygon.piece.face.side(p)? != Sign::Zero {
        return Ok(false);
    }
    for bound in &polygon.piece.bounds {
        if bound.side(p)? == Sign::Negative {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The box around `polygons` and `segments`, in half steps, as [`geom_poly::subdivide::around`]
/// gives it around polygons alone.
fn widened(mut cell: Cell, segments: &[[P3; 2]]) -> Cell {
    for end in segments.iter().flatten() {
        let at = coordinates(*end);
        for ((low, high), v) in cell.low.iter_mut().zip(&mut cell.high).zip(at) {
            *low = (*low).min(v.saturating_mul(2).saturating_sub(3));
            *high = (*high).max(v.saturating_mul(2).saturating_add(3));
        }
    }
    cell
}

/// The pieces of each of `segments`, the target's edges in order, cut by `others`, each with the
/// vector `polygons` give it from `cell`'s corner.
///
/// # Errors
///
/// A [`WireError`]: an edge of no length, what the numbers layer refuses, a stopped meter.
pub(crate) fn runs(
    segments: &[[P3; 2]],
    others: &[&Polygon],
    polygons: &[Polygon],
    cell: Cell,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Run>, WireError> {
    let cell = widened(cell, segments);
    let mut out = Vec::new();
    let mut counted = 0usize;
    for (edge, [a, b]) in segments.iter().enumerate() {
        let (line, axis, toward) = line(*a, *b)?;
        let [l1, l2] = line;
        let end = |p: P3| -> Result<Stop, NumError> {
            Ok(Stop {
                point: Tpi::new(l1, l2, square(p, axis, Sign::Positive)?)?,
                across: square(p, axis, toward)?,
                vertex: Some(p),
                split: None,
            })
        };
        let mut splits: Vec<Stop> = Vec::new();
        for polygon in others {
            let face = polygon.piece.face;
            let (at_a, at_b) = (face.side(*a)?, face.side(*b)?);
            if at_a == Sign::Zero || at_b == Sign::Zero || at_a == at_b {
                continue;
            }
            let point = Tpi::new(l1, l2, face)?;
            if !holds(polygon, &point)? {
                continue;
            }
            let across = if at_b == Sign::Positive {
                face
            } else {
                turned(&face)?
            };
            // In order from `a`: a split goes before the first held one whose plane, positive
            // towards `b`, has it on its negative side; on that plane it is the same point.
            let mut place = splits.len();
            let mut same = false;
            for (k, held) in splits.iter().enumerate() {
                match held.across.side_of_tpi(&point)? {
                    Sign::Negative => {
                        place = k;
                        break;
                    }
                    Sign::Zero => {
                        same = true;
                        break;
                    }
                    Sign::Positive => {}
                }
            }
            if !same {
                splits.insert(
                    place,
                    Stop {
                        point,
                        across,
                        vertex: None,
                        split: None,
                    },
                );
            }
        }
        let mut stops = Vec::with_capacity(splits.len().saturating_add(2));
        stops.push(end(*a)?);
        for mut stop in splits {
            stop.split = Some(counted);
            counted = counted.saturating_add(1);
            stops.push(stop);
        }
        stops.push(end(*b)?);
        for pair in stops.windows(2) {
            let [from, to] = pair else {
                continue;
            };
            let vector = if polygons.is_empty() {
                Winding::new()
            } else {
                winding_at(
                    &cell,
                    &Winding::new(),
                    polygons,
                    [from.across, l1, l2],
                    meter,
                )?
            };
            out.push(Run {
                edge,
                line,
                from: *from,
                to: *to,
                vector,
            });
        }
    }
    Ok(out)
}

/// The vertices and edges of a wire as it is built: each wire vertex and each split once.
#[derive(Default)]
struct Built {
    vertices: Vec<Tpi>,
    at: BTreeMap<[i64; 3], usize>,
    splits: BTreeMap<usize, usize>,
    edges: Vec<(usize, usize)>,
}

impl Built {
    fn vertex(&mut self, stop: &Stop) -> usize {
        let held = match (stop.vertex, stop.split) {
            (Some(p), _) => self.at.get(&coordinates(p)).copied(),
            (None, Some(split)) => self.splits.get(&split).copied(),
            (None, None) => None,
        };
        if let Some(held) = held {
            return held;
        }
        let at = self.vertices.len();
        match (stop.vertex, stop.split) {
            (Some(p), _) => {
                self.at.insert(coordinates(p), at);
            }
            (None, Some(split)) => {
                self.splits.insert(split, at);
            }
            (None, None) => {}
        }
        self.vertices.push(stop.point);
        at
    }

    fn edge(&mut self, from: &Stop, to: &Stop) {
        let (u, v) = (self.vertex(from), self.vertex(to));
        self.edges.push((u.min(v), u.max(v)));
    }

    fn curves(self) -> Curves {
        // A wire target's edge meets no pieces: it is cut by them, not made where they meet.
        let meets = alloc::vec![Vec::new(); self.edges.len()];
        Curves {
            vertices: self.vertices,
            edges: self.edges,
            meets,
        }
    }
}

/// The pieces of `runs` whose vector passes `test`, kept pieces next to each other on one edge as
/// one edge.
pub(crate) fn kept(test: &WindingTest, runs: &[Run]) -> Curves {
    let mut built = Built::default();
    let mut open: Option<(usize, Stop, Stop)> = None;
    for run in runs {
        let keep = test.holds(&run.vector);
        open = match open.take() {
            Some((edge, from, _)) if edge == run.edge && keep => Some((edge, from, run.to)),
            Some((_, from, to)) => {
                built.edge(&from, &to);
                keep.then_some((run.edge, run.from, run.to))
            }
            None => keep.then_some((run.edge, run.from, run.to)),
        };
    }
    if let Some((_, from, to)) = open {
        built.edge(&from, &to);
    }
    built.curves()
}

/// Whether the wire vertex `p` cuts the wire: it lies on a polygon of another operand.
fn cuts(p: P3, others: &[&Polygon]) -> Result<bool, NumError> {
    for polygon in others {
        if holds_point(polygon, p)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The root of `at` in `parents`, the runs joined so far.
fn root(parents: &[usize], mut at: usize) -> usize {
    while let Some(up) = parents.get(at).copied().filter(|up| *up != at) {
        at = up;
    }
    at
}

/// The runs of `runs`, unbroken by a split or by a wire vertex on one of `others`, that hold one
/// of `seeds`.
///
/// # Errors
///
/// [`CombineDetail::SeedOutside`] for a seed on no piece, [`CombineDetail::SeedOnBoundary`] for
/// one where the wire is cut or between two runs; what the numbers layer refuses.
pub(crate) fn trimmed(
    runs: &[Run],
    others: &[&Polygon],
    seeds: &[P3],
) -> Result<Curves, WireError> {
    let mut parents: Vec<usize> = (0..runs.len()).collect();
    let mut at_vertex: BTreeMap<[i64; 3], Vec<usize>> = BTreeMap::new();
    for (k, run) in runs.iter().enumerate() {
        for stop in [&run.from, &run.to] {
            if let Some(p) = stop.vertex {
                at_vertex.entry(coordinates(p)).or_default().push(k);
            }
        }
    }
    let mut cut: BTreeMap<[i64; 3], bool> = BTreeMap::new();
    for (key, list) in &at_vertex {
        let [kx, ky, kz] = *key;
        let is_cut = cuts(P3::new(kx, ky, kz)?, others)?;
        cut.insert(*key, is_cut);
        if !is_cut && let Some((first, rest)) = list.split_first() {
            for other in rest {
                let (u, v) = (root(&parents, *first), root(&parents, *other));
                if let Some(slot) = parents.get_mut(v) {
                    *slot = u;
                }
            }
        }
    }
    let mut chosen: Vec<usize> = Vec::new();
    for seed in seeds {
        let mut found: Vec<usize> = Vec::new();
        for (k, run) in runs.iter().enumerate() {
            let [l1, l2] = run.line;
            if l1.side(*seed)? != Sign::Zero || l2.side(*seed)? != Sign::Zero {
                continue;
            }
            let (after, before) = (run.from.across.side(*seed)?, run.to.across.side(*seed)?);
            let end = match (after, before) {
                (Sign::Positive, Sign::Negative) => None,
                (Sign::Zero, _) => Some(run.from),
                (_, Sign::Zero) => Some(run.to),
                _ => continue,
            };
            if let Some(stop) = end {
                let on_cut = stop
                    .vertex
                    .is_none_or(|p| cut.get(&coordinates(p)).copied().unwrap_or(true));
                if on_cut {
                    return Err(WireError::Detail(CombineDetail::SeedOnBoundary));
                }
            }
            found.push(root(&parents, k));
        }
        found.sort_unstable();
        found.dedup();
        match found.as_slice() {
            [] => return Err(WireError::Detail(CombineDetail::SeedOutside)),
            [class] => chosen.push(*class),
            _ => return Err(WireError::Detail(CombineDetail::SeedOnBoundary)),
        }
    }
    let mut built = Built::default();
    for (k, run) in runs.iter().enumerate() {
        if chosen.contains(&root(&parents, k)) {
            built.edge(&run.from, &run.to);
        }
    }
    Ok(built.curves())
}
