//! Fuzz target: `geom-op`'s batch driver gives one result per item, each the item's own, and
//! every curve and surface kind's many-point call gives the single calls' bits.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_ITEMS`] batch items
//! (a key from a small alphabet, so keys repeat, the work each charges and whether it fails), each
//! item's own budget, an optional batch total, and parameters for a line, a circle and a plane.
//! [`check`] runs the batch and compares it with the rules of chapter 23.10 read independently: a
//! repeated key refuses the whole batch; otherwise one result per item, at the batch's path plus
//! the item's key, equal to the item run alone ([`geom_op::Batch::single`]) until the running sum
//! of work in key order first passes the total, and the batch's total from that item on. It then
//! evaluates the three geometry kinds at the case's parameters through `points` and through
//! single calls and requires the same bits, and the same failures. [`harness`] is `decode` then
//! `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against chapter 23.10's batch rule read independently
//! (`projects/engines/geometry/architecture/23-performance.md`), and of the flat-array calls
//! against the single calls they are defined by (chapter 23.6).
//!
//! Cost: at most [`MAX_ITEMS`] items of at most 63 work units each, each run twice (in the batch
//! and alone), and [`MAX_PARAMS`] parameters through three kinds twice; at most
//! [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::{Circle, CurveError, CurveKind, Frame, Line, ParamCurve};
use geom_nurbs::Limit;
use geom_op::{Batch, BatchDetail, Key, OpContext, OpError, OpId, OpPath, RunDetail};
use geom_run::{BatchBudget, Budget, ChunkPlan, Exec, Phase, WorkMeter};
use geom_surface::{ParamSurface, Plane, SurfaceKind};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most items a case's batch holds.
pub const MAX_ITEMS: u8 = 16;

/// The most parameters a case evaluates each kind at.
pub const MAX_PARAMS: u8 = 8;

/// The most bytes [`decode`] reads from any input: 60.
///
/// The item count, the per-item budget and the total's byte: 3. Each of at most [`MAX_ITEMS`]
/// items, a key byte and a work byte: 2. The parameter count: 1. Each of at most [`MAX_PARAMS`]
/// parameters, one byte for the curves and two for the surface: 3. So 3 + 16 · 2 + 1 + 8 · 3 = 60.
/// The test `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = 3 + 16 * 2 + 1 + 8 * 3;

const _: () = assert!(MAX_INPUT_BYTES == 60);

/// One batch item: its key, the work it charges, and whether it fails on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// The caller's key.
    pub key: Key,
    /// The work units it charges.
    pub work: u64,
    /// Whether it fails with its own error after charging.
    pub fails: bool,
}

/// One case: the batch and the parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The items.
    pub items: Vec<Item>,
    /// Each item's own budget, in work units.
    pub per_item: u64,
    /// The batch total, in work units, if any.
    pub total: Option<u64>,
    /// Parameters in [0, 1] and just outside, for the curves.
    pub t: Vec<f64>,
    /// Parameter pairs for the surface: all u values, then all v values.
    pub uv: Vec<f64>,
}

/// A parameter from a byte: across [−0.25, 1.25] in 64 steps, or NaN.
fn param(byte: u8) -> f64 {
    if byte == 255 {
        f64::NAN
    } else {
        -0.25 + 1.5 * f64::from(byte % 65) / 64.0
    }
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let n = src.byte() % (MAX_ITEMS + 1);
    let per_item = u64::from(src.byte() % 64);
    let total_byte = src.byte();
    let total = (total_byte % 2 == 1).then(|| u64::from(total_byte / 2) * 4);
    let items = (0..n)
        .map(|_| {
            let k = src.byte();
            let w = src.byte();
            // Keys from an alphabet of 32, so that repeats come up.
            let key = u128::from(k % 32).to_be_bytes();
            Item {
                key,
                work: u64::from(w % 64),
                fails: w >= 224,
            }
        })
        .collect();
    let m = usize::from(src.byte() % (MAX_PARAMS + 1));
    let mut t = Vec::with_capacity(m);
    let mut us = Vec::with_capacity(m);
    let mut vs = Vec::with_capacity(m);
    for _ in 0..m {
        t.push(param(src.byte()));
        us.push(param(src.byte()));
        vs.push(param(src.byte()));
    }
    us.extend(vs);
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            items,
            per_item,
            total,
            t,
            uv: us,
        },
        read,
    )
}

/// The harness every fuzzing engine drives: decode `data` into a case, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// The item's operation: charges its work one unit at a time, then fails or returns its work.
fn op(ctx: &OpContext<'_>, item: &Item, meter: &mut WorkMeter<'_>) -> Result<u64, OpError> {
    for _ in 0..item.work {
        // The helper reads the meter, never this error, to report a stop.
        meter
            .charge(1)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))?;
    }
    if item.fails {
        return Err(OpError::new(
            ctx.path().clone(),
            geom_num::NumError::Degenerate,
        ));
    }
    Ok(item.work)
}

/// The batch's results, as the driver gives them.
pub type Results = Result<Vec<Result<u64, OpError>>, OpError>;

/// Runs the case through the batch driver and the flat-array calls, and checks the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, |batch, items| {
        batch.run(&Exec::serial(), items, |i| i.key, op)
    })
}

/// [`check`] with `run` in place of the driver's `Batch::run`: how a test shows that a broken
/// driver is caught.
///
/// # Errors
///
/// As for [`check`].
pub fn check_with<R>(case: &Case, run: R) -> Result<(), Finding>
where
    R: Fn(&Batch<'_>, &[Item]) -> Results,
{
    let path = OpPath::new(OpId::from_bytes([0x42; 16]));
    let ctx = OpContext::new(path.clone(), Budget::UNLIMITED);
    let budget = BatchBudget::new(Budget::of_work(case.per_item));
    let budget = match case.total {
        Some(t) => budget.with_total(Budget::of_work(t)),
        None => budget,
    };
    let plan = ChunkPlan::new(core::num::NonZeroUsize::MIN);
    let batch = Batch::new(&ctx, budget, Phase::new("fuzz"), plan);
    let got = run(&batch, &case.items);
    let expected = expected(case, &batch, &path);
    if got != expected {
        return Err(Finding::new(
            "a batch gives each item its own result, at its own path, cut only by the total in key order",
            || format!("got {got:?}, expected {expected:?}; case {case:?}"),
        ));
    }
    flat(case)
}

/// The batch's results by chapter 23.10's rule, read independently of the driver.
fn expected(case: &Case, batch: &Batch<'_>, path: &OpPath) -> Results {
    let mut keys: Vec<Key> = case.items.iter().map(|i| i.key).collect();
    keys.sort_unstable();
    if let Some(pair) = keys.windows(2).find(|w| w.first() == w.last()) {
        let key = pair.first().copied().unwrap_or_default();
        return Err(OpError::new(path.clone(), BatchDetail::RepeatedKey { key }));
    }
    // Each item alone; then the batch total, summed in key order over what each item used.
    let alone: Vec<Result<u64, OpError>> = case
        .items
        .iter()
        .map(|i| batch.single(i.key, i, op))
        .collect();
    let Some(total) = case.total else {
        return Ok(alone);
    };
    let used = |i: &Item| i.work.min(case.per_item.saturating_add(1));
    let mut order: Vec<usize> = (0..case.items.len()).collect();
    order.sort_by_key(|&i| case.items.get(i).map(|x| x.key));
    let mut sum = 0_u64;
    let mut cut = None;
    for &i in &order {
        let Some(item) = case.items.get(i) else {
            continue;
        };
        sum = sum.saturating_add(used(item));
        if sum > total {
            cut = Some((item.key, sum));
            break;
        }
    }
    let Some((cut_key, cut_sum)) = cut else {
        return Ok(alone);
    };
    let exceeded = geom_run::Exceeded {
        budget: Budget::of_work(total),
        kind: geom_run::BudgetKind::Work,
        phase: Phase::new("fuzz"),
        used: geom_run::Usage::new(u128::from(cut_sum), 0),
    };
    Ok(case
        .items
        .iter()
        .zip(alone)
        .map(|(item, r)| {
            if item.key >= cut_key {
                Err(OpError::new(
                    path.item(item.key),
                    RunDetail::BatchTotal(exceeded),
                ))
            } else {
                r
            }
        })
        .collect())
}

/// Coordinate `k` of point `i` of a flat array of `n` points, as bits.
fn bits(out: &[f64], n: usize, k: usize, i: usize) -> Option<u64> {
    k.checked_mul(n)
        .and_then(|x| x.checked_add(i))
        .and_then(|j| out.get(j))
        .map(|v| v.to_bits())
}

/// The flat-array calls of a line, a circle and a plane equal their single calls, bit for bit.
fn flat(case: &Case) -> Result<(), Finding> {
    let n = case.t.len();
    let mut out = vec![0.0; n.saturating_mul(3)];
    let mut results: Vec<Result<(), CurveError>> = vec![Ok(()); n];
    let mut curves: Vec<CurveKind<3>> = Vec::new();
    if let Ok(line) = Line::new([1.0, 2.0, 3.0], [0.5, -1.0, 2.0], (0.0, 1.0)) {
        curves.push(CurveKind::Line(line));
    }
    if let Ok(frame) = Frame::standard()
        && let Ok(circle) = Circle::new([0.0; 3], 3.0, frame, (0.0, 1.0))
    {
        curves.push(CurveKind::Circle(circle));
    }
    for curve in &curves {
        if curve
            .points(&case.t, Limit::FromRight, &mut out, &mut results)
            .is_err()
        {
            return Err(Finding::new("the arrays fit, so the call runs", || {
                format!("case {case:?}")
            }));
        }
        for (i, &t) in case.t.iter().enumerate() {
            let single = curve.point(t, Limit::FromRight);
            let same = match &single {
                Ok(p) => {
                    results.get(i) == Some(&Ok(()))
                        && (0..3).all(|k| bits(&out, n, k, i) == p.get(k).map(|v| v.to_bits()))
                }
                Err(e) => {
                    results.get(i) == Some(&Err(*e))
                        && (0..3).all(|k| bits(&out, n, k, i) == Some(0))
                }
            };
            if !same {
                return Err(Finding::new(
                    "many points per call give the single calls' bits",
                    || format!("{curve:?} at {t}: {single:?}; case {case:?}"),
                ));
            }
        }
    }
    let domain = ((0.0, 1.0), (0.0, 1.0));
    if let Ok(plane) = Plane::new([0.0; 3], [1.0, 0.0, 0.5], [0.0, 2.0, 0.0], domain) {
        let surface = SurfaceKind::Plane(plane);
        let sides = (Limit::FromRight, Limit::FromRight);
        if surface
            .points(&case.uv, sides, &mut out, &mut results)
            .is_err()
        {
            return Err(Finding::new("the arrays fit, so the call runs", || {
                format!("case {case:?}")
            }));
        }
        for i in 0..n {
            let (u, v) = (
                case.uv.get(i).copied().unwrap_or(0.0),
                n.checked_add(i)
                    .and_then(|j| case.uv.get(j))
                    .copied()
                    .unwrap_or(0.0),
            );
            let single = surface.point(u, v, sides);
            let same = match &single {
                Ok(p) => {
                    results.get(i) == Some(&Ok(()))
                        && (0..3).all(|k| bits(&out, n, k, i) == p.get(k).map(|x| x.to_bits()))
                }
                Err(e) => {
                    results.get(i) == Some(&Err(*e))
                        && (0..3).all(|k| bits(&out, n, k, i) == Some(0))
                }
            };
            if !same {
                return Err(Finding::new(
                    "many points per call give the single calls' bits",
                    || format!("plane at ({u}, {v}): {single:?}; case {case:?}"),
                ));
            }
        }
    }
    Ok(())
}
