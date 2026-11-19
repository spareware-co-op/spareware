//! Fuzz target: `geom-2d`'s arrangement updates keep their laws on any input (pieces G199 and
//! G199a).
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a count of segments and, for
//! each, four bytes (the lattice coordinates of its two ends), then a byte for how to order the
//! insertions, bytes for probe points, and last a count of arcs and, for each, five bytes (its
//! ends, and which centre on their bisector), so that a byte string without the arcs reads as it
//! did before them. [`check`] builds the drawing by inserting the curves (the segments, then the
//! arcs) one by one in that order, and holds it to the sweep's arrangement of the same curves
//! (`geom_2d::arrange`, an independent method): the same pieces with the same sources
//! ([`Live::same_as`]), the same again in the opposite order, and the same when the drawing is two
//! halves put one over the other; a probe point that is a vertex of the sweep's arrangement is
//! located as a vertex, and one in a face is located as a face whose boundary cycle closes.
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing of the incremental arrangement against the sweep, the law of
//! chapter 14.4 of the kernel architecture
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`): incremental equals batch for any
//! insertion order.
//!
//! Cost: constant per case, at most [`MAX_SEGMENTS`] segments, [`MAX_ARCS`] arcs and
//! [`MAX_PROBES`] probes; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_2d::arrange::{At, Curve, arrange_curves};
use geom_2d::update::{Live, Location};
use geom_num::arc::Arc2;
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most segments of a case.
pub const MAX_SEGMENTS: u8 = 8;

/// The most probe points of a case.
pub const MAX_PROBES: u8 = 4;

/// The most arcs of a case: two, so that arcs cross arcs, at a cost a test run can bear (each
/// exact question on a one-root point is dozens of wide products, slow in a debug build).
pub const MAX_ARCS: u8 = 2;

/// The most bytes [`decode`] reads from any input: a count, four bytes for each of at most
/// [`MAX_SEGMENTS`] segments, an order byte, a probe count and three bytes for each of at most
/// [`MAX_PROBES`] probes, then an arc count and five bytes for each of at most [`MAX_ARCS`] arcs:
/// 1 + 4 · 8 + 1 + 1 + 3 · 4 + 1 + 5 · 2 = 58.
pub const MAX_INPUT_BYTES: usize = 1 + 4 * 8 + 1 + 1 + 3 * 4 + 1 + 5 * 2;

const _: () = assert!(MAX_INPUT_BYTES == 58);

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each segment as (x0, y0, x1, y1) on a lattice of eight steps a side.
    pub segments: Vec<[u8; 4]>,
    /// How the insertion order is drawn from the segments.
    pub order: u8,
    /// Probe points as (x, y, denominator choice).
    pub probes: Vec<[u8; 3]>,
    /// Each arc as (x0, y0, x1, y1, centre choice) on the same lattice: counter-clockwise from the
    /// first point to the second about a centre on their bisector a quarter step apart.
    pub arcs: Vec<[u8; 5]>,
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let count = usize::from(src.byte() % (MAX_SEGMENTS + 1));
    let segments = (0..count)
        .map(|_| {
            let mut s = [0_u8; 4];
            for v in &mut s {
                *v = src.byte() % 8;
            }
            s
        })
        .collect();
    let order = src.byte();
    let probe_count = usize::from(src.byte() % (MAX_PROBES + 1));
    let probes = (0..probe_count)
        .map(|_| [src.byte(), src.byte(), src.byte()])
        .collect();
    let arc_count = usize::from(src.byte() % (MAX_ARCS + 1));
    let arcs = (0..arc_count)
        .map(|_| {
            let mut a = [0_u8; 5];
            for v in &mut a {
                *v = src.byte();
            }
            for v in a.iter_mut().take(4) {
                *v %= 8;
            }
            a
        })
        .collect();
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            segments,
            order,
            probes,
            arcs,
        },
        read,
    )
}

fn fail(case: &Case, law: &str) -> Finding {
    Finding::new(law, || format!("{case:?}"))
}

fn grid(x: u8, y: u8) -> Option<P2> {
    P2::new(i64::from(x), i64::from(y)).ok()
}

/// The arc `[x0, y0, x1, y1, s]` stands for: from P = (x0, y0) counter-clockwise to Q = (x1, y1)
/// about (2(P + Q) + t (Q − P) turned a quarter) / 4, t = s mod 9 − 4, a point of their bisector;
/// none when P and Q are one point.
fn arc_of([x0, y0, x1, y1, s]: [u8; 5]) -> Option<Curve> {
    let (p, q) = (grid(x0, y0)?, grid(x1, y1)?);
    if p == q {
        return None;
    }
    let t = i128::from(s % 9).checked_sub(4)?;
    let (px, py, qx, qy) = (
        i128::from(x0),
        i128::from(y0),
        i128::from(x1),
        i128::from(y1),
    );
    // 2(P + Q) + t (Q - P) turned a quarter, each part below 2^7: checked all the same.
    let twice = |a: i128, b: i128| a.checked_add(b)?.checked_mul(2);
    let cx = twice(px, qx)?.checked_sub(t.checked_mul(qy.checked_sub(py)?)?)?;
    let cy = twice(py, qy)?.checked_add(t.checked_mul(qx.checked_sub(px)?)?)?;
    let centre = Rational2::new(cx, cy, 4).ok()?;
    Some(Curve::Arc(Arc2::new(p, q, centre).ok()?))
}

/// The insertion order the case's order byte draws over its `n` curves: a rotation, and the
/// reverse if the high bit is set.
fn order_of(case: &Case, n: usize) -> Vec<usize> {
    let turn = usize::from(case.order & 0x7f)
        .checked_rem(n.max(1))
        .unwrap_or(0);
    let mut order: Vec<usize> = (0..n)
        .map(|i| i.saturating_add(turn).checked_rem(n.max(1)).unwrap_or(0))
        .collect();
    if case.order & 0x80 != 0 {
        order.reverse();
    }
    order
}

/// Builds the drawing by inserting `order`'s curves, each labelled by its place in `curves`.
fn built(curves: &[Curve], order: &[usize], meter: &mut WorkMeter<'_>) -> Option<Live> {
    let mut live = Live::new();
    for &i in order {
        live = live
            .insert_curve(curves.get(i)?, u32::try_from(i).ok()?, meter)
            .ok()?;
    }
    Some(live)
}

/// Holds the incremental drawing of `case` to the sweep's arrangement.
///
/// # Errors
///
/// A [`Finding`] naming the law the case broke.
pub fn check(case: &Case) -> Result<(), Finding> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut curves: Vec<Curve> = case
        .segments
        .iter()
        .filter_map(|&[a, b, c, d]| Some(Curve::Segment(grid(a, b)?, grid(c, d)?)))
        .collect();
    curves.extend(case.arcs.iter().copied().filter_map(arc_of));
    let Ok(sweep) = arrange_curves(&curves, &mut meter) else {
        return Err(fail(case, "the sweep arranges small lattice curves"));
    };
    let Ok(batch) = Live::of_arrangement(&sweep) else {
        return Err(fail(case, "the sweep's arrangement is a drawing"));
    };
    let order = order_of(case, curves.len());
    let Some(live) = built(&curves, &order, &mut meter) else {
        return Err(fail(case, "every insert of small lattice curves succeeds"));
    };
    if live.same_as(&batch) != Ok(true) {
        return Err(fail(case, "incremental equals batch for the case's order"));
    }
    let reversed: Vec<usize> = order.iter().rev().copied().collect();
    match built(&curves, &reversed, &mut meter) {
        Some(other) if other.same_as(&batch) == Ok(true) => {}
        _ => {
            return Err(fail(
                case,
                "incremental equals batch for the opposite order",
            ));
        }
    }
    // Two halves, one over the other.
    let half = curves.len() / 2;
    let (mut left, mut right) = (Live::new(), Live::new());
    for (i, curve) in curves.iter().enumerate() {
        let label = u32::try_from(i).unwrap_or(0);
        let side = if i < half { &mut left } else { &mut right };
        match side.insert_curve(curve, label, &mut meter) {
            Ok(next) => *side = next,
            Err(_) => {
                return Err(fail(case, "every insert of small lattice curves succeeds"));
            }
        }
    }
    match left.overlay(&right, &mut meter) {
        Ok(both) if both.same_as(&batch) == Ok(true) => {}
        _ => return Err(fail(case, "an overlay is the arrangement of the union")),
    }
    for &[x, y, w] in &case.probes {
        let w = i128::from(w % 3).checked_add(1).unwrap_or(1);
        let Ok(at) = Rational2::new(
            i128::from(x % 9).checked_mul(w).unwrap_or(0),
            i128::from(y % 9).checked_mul(w).unwrap_or(0),
            w,
        ) else {
            continue;
        };
        let here = At::Rational(at);
        let is_vertex = sweep.vertices().iter().any(|v| v.same(&here) == Ok(true));
        match live.locate(&at, &mut meter) {
            Ok(Location::Vertex(v)) => {
                if !is_vertex || v.same(&here) != Ok(true) {
                    return Err(fail(case, "a located vertex is a vertex of the sweep"));
                }
            }
            Ok(Location::Edge(..)) => {
                if is_vertex {
                    return Err(fail(case, "a vertex is not located as an edge"));
                }
            }
            Ok(Location::Face(half)) => {
                if is_vertex {
                    return Err(fail(case, "a vertex is not located as a face"));
                }
                if let Some(h) = half {
                    match live.face_cycle(&h, &mut meter) {
                        Ok(cycle) if cycle.len() >= 2 => {}
                        _ => return Err(fail(case, "the boundary of a located face closes")),
                    }
                }
            }
            Err(_) => return Err(fail(case, "a probe on a small lattice is located")),
        }
    }
    Ok(())
}

/// The fuzzer's entry: decode, then check.
///
/// # Errors
///
/// As [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
