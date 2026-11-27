//! Where a railing runs: a sketched path on its level, or a stair it follows, on one side or both;
//! and the lines that come of it, in the railing's frame in grid steps.
//!
//! - **A sketched path** ([`Path::Sketched`]) is two to [`MOST_POINTS`] points in plan, joined by
//!   straight segments, level, on the railing's base. The sketch solver has arcs, but the engine's
//!   extrusion takes polygons, so a curved rail cannot be built exactly yet: a path of arcs comes
//!   with curved walls (job 14.4), as a floor's boundary refuses an arc today. A segment has a
//!   length in plan and at most [`MOST_SEGMENT`]; a path never turns straight back on itself.
//! - **A path on a stair** ([`Path::OnStair`]) follows the stair's right side, its left, or both
//!   (seen going up), `inset` in from its edge: each run's side from its first nosing to its last,
//!   then round the landing's edge to the next run's side, as construct-stairs lays the stair out
//!   ([`stair_lines`]). On a run the line follows the pitch line; on a landing its top. Where a run
//!   starts from the floor or a landing, the pitch line stands one riser above the floor at the
//!   run's foot, so the line steps up there ([`Vertex::low`] and [`Vertex::high`]): its post
//!   reaches from the lower to the rail above the higher. Where a stair turns, the line turns with
//!   it, inset from each edge; where a half turn brings the next run back beside the last, the line
//!   on the inside goes round the end of the well, `inset` past it. Where a run ends or starts at a
//!   turn, the line runs on to the run's end square to it before it turns, so the slope of a flight
//!   ends and starts exactly where the stair's does.
//! - **A line** ([`Line`]) is the vertices of one railing in grid steps, the height under each
//!   (the floor, a landing's top or the pitch line), which pieces are on a flight ([`Flight`]: the
//!   tallest riser and the going), and the side its handrail is on ([`Hand`]). Everything on a stair
//!   runs along the plan's axes, so every vertex is the stair's corners moved by whole lengths, with
//!   no rounding.

use crate::error::RailingError;
use construct_params::Length;
use construct_stairs::{Layout, PieceKind, Stair};
use kit_core::Id;

/// The most points a sketched path takes.
pub const MOST_POINTS: usize = 256;
/// The longest a segment may be in plan: 100 m.
pub const MOST_SEGMENT: Length = Length::from_mm(100_000);
/// The farthest a railing on a stair stands in from its edge: 500 mm.
pub const MOST_INSET: Length = Length::from_mm(500);
/// The farthest a point may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
pub const GRID_LIMIT: i64 = 1_099_511_627_775;

/// Which side of a stair a railing follows, seen going up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Its left side.
    Left,
    /// Its right side.
    Right,
    /// Both: two lines, the right first.
    Both,
}

impl Side {
    /// Every side, in a fixed order.
    pub const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Both];

    /// Its stored name: `left`, `right` or `both`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Both => "both",
        }
    }

    /// The side of a stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The other side; none for both.
    #[must_use]
    pub fn flipped(self) -> Option<Self> {
        match self {
            Self::Left => Some(Self::Right),
            Self::Right => Some(Self::Left),
            Self::Both => None,
        }
    }
}

/// Which side of a line's direction its handrail is on: the side it is used from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hand {
    /// To the left, walking from its first point to its last.
    Left,
    /// To the right.
    Right,
}

/// Where a railing runs, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Path {
    /// Points in plan on the railing's level.
    Sketched {
        /// The points, in order.
        points: Vec<[Length; 2]>,
    },
    /// A stair's side or sides.
    OnStair {
        /// The stair's permanent ID.
        stair: Id,
        /// Which side.
        side: Side,
        /// How far in from the stair's edge the railing's line is.
        inset: Length,
    },
}

/// One vertex of a line, in grid steps in the railing's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vertex {
    /// Where it is in plan.
    pub at: [i64; 2],
    /// The height under the line as it comes into the vertex.
    pub low: i64,
    /// The height under it as it leaves: above `low` only where a run starts.
    pub high: i64,
}

/// A piece of line on a stair's flight: what its opening under the balusters is worked from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flight {
    /// The flight's tallest riser.
    pub riser: Length,
    /// Its going.
    pub going: Length,
}

/// One line of a railing, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Its vertices, from its start.
    pub vertices: Vec<Vertex>,
    /// For each piece between two vertices, the flight it is on, if it is.
    pub flights: Vec<Option<Flight>>,
    /// The side its handrail is on.
    pub inner: Hand,
}

fn range(what: &str) -> RailingError {
    RailingError::OutOfRange {
        what: what.to_owned(),
    }
}

/// `a - b` exactly as an `i128`.
fn diff(a: i64, b: i64) -> i128 {
    i128::from(a).saturating_sub(i128::from(b))
}

/// Checks a sketched path's points: as the module header says.
///
/// # Errors
/// [`RailingError::FewPoints`], [`RailingError::OutOfRange`] (too many points, a point beyond the
/// grid, a segment over [`MOST_SEGMENT`]), [`RailingError::NoLength`] and
/// [`RailingError::TurnsBack`].
pub fn check_points(points: &[[Length; 2]]) -> Result<(), RailingError> {
    if points.len() < 2 {
        return Err(RailingError::FewPoints {
            points: points.len(),
        });
    }
    if points.len() > MOST_POINTS {
        return Err(range(&format!("path of {} points", points.len())));
    }
    if points
        .iter()
        .flatten()
        .any(|c| c.steps().unsigned_abs() > GRID_LIMIT.unsigned_abs())
    {
        return Err(range("point, beyond the kernel's grid"));
    }
    let most = i128::from(MOST_SEGMENT.steps());
    let mut previous: Option<[i128; 2]> = None;
    for (k, pair) in points.windows(2).enumerate() {
        let (Some(p), Some(q)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let d = [
            diff(q[0].steps(), p[0].steps()),
            diff(q[1].steps(), p[1].steps()),
        ];
        let n = d[0]
            .saturating_mul(d[0])
            .saturating_add(d[1].saturating_mul(d[1]));
        if n == 0 {
            return Err(RailingError::NoLength {
                segment: k.saturating_add(1),
            });
        }
        if n > most.saturating_mul(most) {
            return Err(range(&format!(
                "segment {}, over 100 m",
                k.saturating_add(1)
            )));
        }
        if let Some(e) = previous {
            let cross = e[0]
                .saturating_mul(d[1])
                .saturating_sub(e[1].saturating_mul(d[0]));
            let dot = e[0]
                .saturating_mul(d[0])
                .saturating_add(e[1].saturating_mul(d[1]));
            if cross == 0 && dot < 0 {
                return Err(RailingError::TurnsBack {
                    point: k.saturating_add(1),
                });
            }
        }
        previous = Some(d);
    }
    Ok(())
}

/// The one line of a sketched path: level on the railing's base, its handrail on the left of its
/// direction, or on the right when `flipped`.
///
/// # Errors
/// As [`check_points`].
pub fn sketched_line(points: &[[Length; 2]], flipped: bool) -> Result<Line, RailingError> {
    check_points(points)?;
    Ok(Line {
        vertices: points
            .iter()
            .map(|p| Vertex {
                at: [p[0].steps(), p[1].steps()],
                low: 0,
                high: 0,
            })
            .collect(),
        flights: vec![None; points.len().saturating_sub(1)],
        inner: if flipped { Hand::Right } else { Hand::Left },
    })
}

/// A unit vector along an axis from `p` to `q`, or none if they are not on one axis line.
fn axis(p: [i64; 2], q: [i64; 2]) -> Option<[i64; 2]> {
    let (dx, dy) = (diff(q[0], p[0]), diff(q[1], p[1]));
    match (dx.signum(), dy.signum()) {
        (s, 0) if s != 0 => Some([i64::try_from(s).ok()?, 0]),
        (0, s) if s != 0 => Some([0, i64::try_from(s).ok()?]),
        _ => None,
    }
}

/// `p + k v`, checked.
fn moved(p: [i64; 2], v: [i64; 2], k: i64) -> Result<[i64; 2], RailingError> {
    let step = |c: i64, d: i64| {
        d.checked_mul(k)
            .and_then(|m| c.checked_add(m))
            .ok_or_else(|| range("point of a stair's railing"))
    };
    Ok([step(p[0], v[0])?, step(p[1], v[1])?])
}

/// One point of a stair's side, before it is inset: where, the heights under it coming in and
/// going on, and the flight of the piece that comes into it.
#[derive(Clone, Copy)]
struct Edge {
    at: [i64; 2],
    low: i64,
    high: i64,
    flight_in: Option<Flight>,
}

/// Adds `e` to a side, folding it into the last point when it is at the same place: the height
/// coming in stays the first's, the height going on becomes the second's.
fn push(chain: &mut Vec<Edge>, e: Edge) {
    match chain.last_mut() {
        Some(last) if last.at == e.at => last.high = e.high,
        _ => chain.push(e),
    }
}

/// The length of a rectangle's edge from corner `k` to the next.
fn edge_length(corners: &[[i64; 2]; 4], k: usize) -> i128 {
    let a = corners.get(k).copied().unwrap_or_default();
    let b = corners
        .get(k.saturating_add(1) % 4)
        .copied()
        .unwrap_or_default();
    diff(b[0], a[0])
        .abs()
        .saturating_add(diff(b[1], a[1]).abs())
}

fn between(a: i64, b: i64, v: i64) -> bool {
    a.min(b) <= v && v <= a.max(b)
}

/// Where a point lies round a rectangle's edge, anticlockwise from its first corner, in steps;
/// none off its edge.
fn round_at(corners: &[[i64; 2]; 4], p: [i64; 2]) -> Option<i128> {
    let mut walked: i128 = 0;
    for k in 0..4 {
        let a = *corners.get(k)?;
        let b = *corners.get(k.saturating_add(1) % 4)?;
        let on = (a[0] == b[0] && a[0] == p[0] && between(a[1], b[1], p[1]))
            || (a[1] == b[1] && a[1] == p[1] && between(a[0], b[0], p[0]));
        if on {
            let into = diff(p[0], a[0])
                .abs()
                .saturating_add(diff(p[1], a[1]).abs());
            return Some(walked.saturating_add(into));
        }
        walked = walked.saturating_add(edge_length(corners, k));
    }
    None
}

/// The corners of a landing's outline passed walking its edge from `from` to `to`, anticlockwise
/// or clockwise: those strictly between the two, in the order they are passed.
fn walked(
    corners: &[[i64; 2]; 4],
    from: [i64; 2],
    to: [i64; 2],
    anticlockwise: bool,
) -> Result<Vec<[i64; 2]>, RailingError> {
    let off = || RailingError::Host("A stair's run does not meet its landing's edge.".to_owned());
    let perimeter = (0..4).fold(0_i128, |sum, k| sum.saturating_add(edge_length(corners, k)));
    let start = round_at(corners, from).ok_or_else(off)?;
    let end = round_at(corners, to).ok_or_else(off)?;
    if perimeter <= 0 {
        return Err(off());
    }
    let ahead = |at: i128| -> i128 {
        let d = if anticlockwise {
            at.saturating_sub(start)
        } else {
            start.saturating_sub(at)
        };
        d.rem_euclid(perimeter)
    };
    let goal = ahead(end);
    let mut passed: Vec<(i128, [i64; 2])> = corners
        .iter()
        .filter_map(|c| round_at(corners, *c).map(|at| (ahead(at), *c)))
        .filter(|(d, _)| *d > 0 && *d < goal)
        .collect();
    passed.sort_by_key(|(d, _)| *d);
    Ok(passed.into_iter().map(|(_, c)| c).collect())
}

/// A run's side as a railing follows it: from its first nosing to its last.
struct RunSide {
    start: [i64; 2],
    end: [i64; 2],
    base: i64,
    first: i64,
    top: i64,
    flight: Flight,
}

/// The side of a stair, `left_side` or its right, before it is inset ([`stair_lines`]).
fn stair_side(
    runs: &[&construct_stairs::Piece],
    landings: &[&construct_stairs::Piece],
    layout: &Layout,
    left_side: bool,
) -> Result<Vec<Edge>, RailingError> {
    let mut sides = Vec::new();
    for (piece, run) in runs.iter().zip(&layout.runs) {
        let [foot, far, _, foot_left] = piece.plan;
        let dir = axis(foot, far)
            .ok_or_else(|| RailingError::Host("A stair's run has no direction.".to_owned()))?;
        let start = if left_side { foot_left } else { foot };
        let risers = layout
            .risers
            .get(run.first..run.first.saturating_add(run.risers))
            .unwrap_or_default();
        let base = run.base.steps();
        let first = risers.first().map_or(0, |r| r.steps());
        sides.push(RunSide {
            start,
            end: moved(start, dir, run.length.steps())?,
            base,
            first: base
                .checked_add(first)
                .ok_or_else(|| range("height of a run"))?,
            top: base
                .checked_add(run.rise.steps())
                .ok_or_else(|| range("height of a run"))?,
            flight: Flight {
                riser: Length::from_steps(risers.iter().map(|r| r.steps()).max().unwrap_or(0)),
                going: layout.going,
            },
        });
    }
    let mut chain: Vec<Edge> = Vec::new();
    for (k, side) in sides.iter().enumerate() {
        push(
            &mut chain,
            Edge {
                at: side.start,
                low: side.base,
                high: side.first,
                flight_in: None,
            },
        );
        push(
            &mut chain,
            Edge {
                at: side.end,
                low: side.top,
                high: side.top,
                flight_in: Some(side.flight),
            },
        );
        let (Some(landing), Some(laid), Some(next)) = (
            landings.get(k),
            layout.landings.get(k),
            sides.get(k.saturating_add(1)),
        ) else {
            continue;
        };
        let h = laid.height.steps();
        // The right side walks round a landing anticlockwise (the stair on its left), the left
        // side clockwise.
        for corner in walked(&landing.plan, side.end, next.start, !left_side)? {
            push(
                &mut chain,
                Edge {
                    at: corner,
                    low: h,
                    high: h,
                    flight_in: None,
                },
            );
        }
    }
    Ok(chain)
}

/// A side of a stair inset `by` toward the stair, whose side is `inner`, as the module header
/// says: each vertex moved square to the edges it joins, round the end of a well at a half turn.
fn inset_side(
    chain: &[Edge],
    inner: Hand,
    by: i64,
) -> Result<(Vec<Vertex>, Vec<Option<Flight>>), RailingError> {
    let normal = |d: [i64; 2]| match inner {
        Hand::Left => [d[1].saturating_neg(), d[0]],
        Hand::Right => [d[1], d[0].saturating_neg()],
    };
    let plus = |p: [i64; 2], v: [i64; 2]| moved(p, v, by);
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut flights: Vec<Option<Flight>> = Vec::new();
    for (k, e) in chain.iter().enumerate() {
        let d_in = k
            .checked_sub(1)
            .and_then(|j| chain.get(j))
            .and_then(|p| axis(p.at, e.at));
        let d_out = chain
            .get(k.saturating_add(1))
            .and_then(|n| axis(e.at, n.at));
        let vertex = |at: [i64; 2], low: i64, high: i64| Vertex { at, low, high };
        let out: Vec<(Vertex, Option<Flight>)> = match (d_in, d_out) {
            (None, Some(d)) | (Some(d), None) => {
                vec![(vertex(plus(e.at, normal(d))?, e.low, e.high), e.flight_in)]
            }
            (Some(a), Some(b)) if a == b => {
                vec![(vertex(plus(e.at, normal(a))?, e.low, e.high), e.flight_in)]
            }
            (Some(a), Some(b)) if a == [b[0].saturating_neg(), b[1].saturating_neg()] => {
                // A half turn: on to the vertex's foot square to the piece coming in, round the end
                // of the well `by` past it, and back to the foot square to the piece going on, where
                // the line steps up; so a flight's slope ends and starts where the stair's does.
                let end = plus(e.at, normal(a))?;
                let start = plus(e.at, normal(b))?;
                vec![
                    (vertex(end, e.low, e.low), e.flight_in),
                    (vertex(plus(end, a)?, e.low, e.low), None),
                    (vertex(plus(start, a)?, e.low, e.low), None),
                    (vertex(start, e.low, e.high), None),
                ]
            }
            (Some(a), Some(b)) => {
                let corner = plus(plus(e.at, normal(a))?, normal(b))?;
                if normal(b) == a && normal(a) == [b[0].saturating_neg(), b[1].saturating_neg()] {
                    // A corner on the stair's inner side, past the vertex: the line runs on to the
                    // vertex's foot square to the piece coming in, to the corner, and from the foot
                    // square to the piece going on, so a flight's slope ends and starts where the
                    // stair's does.
                    vec![
                        (vertex(plus(e.at, normal(a))?, e.low, e.low), e.flight_in),
                        (vertex(corner, e.low, e.low), None),
                        (vertex(plus(e.at, normal(b))?, e.low, e.high), None),
                    ]
                } else {
                    vec![(vertex(corner, e.low, e.high), e.flight_in)]
                }
            }
            (None, None) => {
                return Err(RailingError::Host(
                    "A stair's side has one point.".to_owned(),
                ));
            }
        };
        for (v, flight_in) in out {
            if v.at
                .iter()
                .any(|c| c.unsigned_abs() > GRID_LIMIT.unsigned_abs())
            {
                return Err(range("point, beyond the kernel's grid"));
            }
            if vertices.last().is_some_and(|last| last.at == v.at) {
                return Err(RailingError::NoLength {
                    segment: vertices.len(),
                });
            }
            if !vertices.is_empty() {
                flights.push(flight_in);
            }
            vertices.push(v);
        }
    }
    Ok((vertices, flights))
}

/// The lines of a railing on `stair` laid out as `layout`, as the module header says: the right
/// side, the left, or both (right first), `inset` in from the edge, every height raised by
/// `shift` (the stair's base above the railing's).
///
/// # Errors
/// [`RailingError::Host`] for a stair construct-stairs cannot shape; [`RailingError::Negative`] or
/// [`RailingError::OutOfRange`] for an inset below nothing or over [`MOST_INSET`], or a point
/// beyond the grid; [`RailingError::NoLength`] for an inset that closes a piece of the line up.
pub fn stair_lines(
    stair: &Stair,
    layout: &Layout,
    side: Side,
    inset: Length,
    shift: Length,
) -> Result<Vec<Line>, RailingError> {
    if inset.steps() < 0 {
        return Err(RailingError::Negative {
            what: "inset from the stair's edge",
        });
    }
    if inset.steps() > MOST_INSET.steps() {
        return Err(range(&format!("inset from the stair's edge, {inset}")));
    }
    let shape =
        construct_stairs::shape(stair, layout).map_err(|e| RailingError::Host(e.to_string()))?;
    let runs: Vec<_> = shape
        .pieces
        .iter()
        .filter(|p| matches!(p.kind, PieceKind::Run { .. }))
        .collect();
    let landings: Vec<_> = shape
        .pieces
        .iter()
        .filter(|p| matches!(p.kind, PieceKind::Landing))
        .collect();
    let sides: &[Side] = match side {
        Side::Both => &[Side::Right, Side::Left],
        Side::Right => &[Side::Right],
        Side::Left => &[Side::Left],
    };
    let mut lines = Vec::new();
    for &which in sides {
        let left_side = which == Side::Left;
        let chain = stair_side(&runs, &landings, layout, left_side)?;
        // The right side's stair is on its left, the left side's on its right.
        let inner = if left_side { Hand::Right } else { Hand::Left };
        let (vertices, flights) = inset_side(&chain, inner, inset.steps())?;
        let vertices = vertices
            .into_iter()
            .map(|v| {
                Some(Vertex {
                    at: v.at,
                    low: v.low.checked_add(shift.steps())?,
                    high: v.high.checked_add(shift.steps())?,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| range("height of a stair's railing"))?;
        lines.push(Line {
            vertices,
            flights,
            inner,
        });
    }
    Ok(lines)
}
