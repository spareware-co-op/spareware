//! Fuzz target: `geom-2d`'s arrangement updates keep their laws on any input (piece G199).
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a count of segments and, for
//! each, four bytes (the lattice coordinates of its two ends), then a byte for how to order the
//! insertions and bytes for probe points. [`check`] builds the drawing by inserting the segments one
//! by one in that order, and holds it to the sweep's arrangement of the same segments
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
//! Cost: constant per case, at most [`MAX_SEGMENTS`] segments and [`MAX_PROBES`] probes; at most
//! [`MAX_INPUT_BYTES`] bytes read.

use geom_2d::arrange::arrange;
use geom_2d::update::{Live, Location};
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most segments of a case.
pub const MAX_SEGMENTS: u8 = 8;

/// The most probe points of a case.
pub const MAX_PROBES: u8 = 4;

/// The most bytes [`decode`] reads from any input: a count, four bytes for each of at most
/// [`MAX_SEGMENTS`] segments, an order byte, a probe count and three bytes for each of at most
/// [`MAX_PROBES`] probes: 1 + 4 · 8 + 1 + 1 + 3 · 4 = 47.
pub const MAX_INPUT_BYTES: usize = 1 + 4 * 8 + 1 + 1 + 3 * 4;

const _: () = assert!(MAX_INPUT_BYTES == 47);

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each segment as (x0, y0, x1, y1) on a lattice of eight steps a side.
    pub segments: Vec<[u8; 4]>,
    /// How the insertion order is drawn from the segments.
    pub order: u8,
    /// Probe points as (x, y, denominator choice).
    pub probes: Vec<[u8; 3]>,
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
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            segments,
            order,
            probes,
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

/// The insertion order the case's order byte draws: a rotation, and the reverse if the high bit
/// is set.
fn order_of(case: &Case) -> Vec<usize> {
    let n = case.segments.len();
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

/// Builds the drawing by inserting `order`'s segments, each labelled by its place in `segs`.
fn built(segs: &[(P2, P2)], order: &[usize], meter: &mut WorkMeter<'_>) -> Option<Live> {
    let mut live = Live::new();
    for &i in order {
        let (a, b) = *segs.get(i)?;
        live = live.insert(a, b, u32::try_from(i).ok()?, meter).ok()?;
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
    let segs: Vec<(P2, P2)> = case
        .segments
        .iter()
        .filter_map(|&[a, b, c, d]| Some((grid(a, b)?, grid(c, d)?)))
        .collect();
    let Ok(batch) = Live::batch(&segs, &mut meter) else {
        return Err(fail(case, "the sweep arranges small lattice segments"));
    };
    let order = order_of(case);
    let Some(live) = built(&segs, &order, &mut meter) else {
        return Err(fail(
            case,
            "every insert of small lattice segments succeeds",
        ));
    };
    if live.same_as(&batch) != Ok(true) {
        return Err(fail(case, "incremental equals batch for the case's order"));
    }
    let reversed: Vec<usize> = order.iter().rev().copied().collect();
    match built(&segs, &reversed, &mut meter) {
        Some(other) if other.same_as(&batch) == Ok(true) => {}
        _ => {
            return Err(fail(
                case,
                "incremental equals batch for the opposite order",
            ));
        }
    }
    // Two halves, one over the other.
    let half = segs.len() / 2;
    let (mut left, mut right) = (Live::new(), Live::new());
    for (i, &(a, b)) in segs.iter().enumerate() {
        let label = u32::try_from(i).unwrap_or(0);
        let side = if i < half { &mut left } else { &mut right };
        match side.insert(a, b, label, &mut meter) {
            Ok(next) => *side = next,
            Err(_) => {
                return Err(fail(
                    case,
                    "every insert of small lattice segments succeeds",
                ));
            }
        }
    }
    match left.overlay(&right, &mut meter) {
        Ok(both) if both.same_as(&batch) == Ok(true) => {}
        _ => return Err(fail(case, "an overlay is the arrangement of the union")),
    }
    let Ok(sweep) = arrange(&segs, &mut meter) else {
        return Err(fail(case, "the sweep arranges small lattice segments"));
    };
    for &[x, y, w] in &case.probes {
        let w = i128::from(w % 3).checked_add(1).unwrap_or(1);
        let Ok(at) = Rational2::new(
            i128::from(x % 9).checked_mul(w).unwrap_or(0),
            i128::from(y % 9).checked_mul(w).unwrap_or(0),
            w,
        ) else {
            continue;
        };
        let is_vertex = sweep
            .vertices()
            .iter()
            .filter_map(geom_2d::arrange::At::rational)
            .any(|v| v.same(&at) == Ok(true));
        match live.locate(&at, &mut meter) {
            Ok(Location::Vertex(v)) => {
                if !is_vertex || v.same(&at) != Ok(true) {
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
