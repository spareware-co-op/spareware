//! The boolean's entry: one arrangement, one body per selection (11.1).
//!
//! What it does: [`combine`] prepares its operands, arranges their polygons, and for each selection
//! rebuilds what it keeps and holds the body to the checker. [`unite`], [`subtract`] and
//! [`intersect`] are its three commonest calls. [`COMBINE`] is its schema.
//!
//! Guarantees:
//!
//! - **One arrangement, several results.** The selections share every clip, cut and trace (11.1).
//! - **Inputs are never changed, and nothing is returned unchecked.** A result that fails the
//!   checker is an error, never a body (11.10).
//! - **As exact as the least exact operand.** A flat arrangement adds no error of its own (11.3).
//! - **The same bytes on any threads, and in any order of the operands.** The order is the keys',
//!   and the work is deterministic (23.4).
//!
//! Method: chapter 11.1's selections on 11.3's exact polyhedral path
//! (`projects/engines/geometry/architecture/11-booleans.md`), EMBER (Trettner, Nehring-Wirxel and
//! Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: what [`prepare`], [`arrange`] and [`rebuild`] cost, and the checker per result.

use alloc::vec::Vec;

use geom_check::{Level, check};
use geom_num::{Exactness, NumError};
use geom_op::{
    Charge, Class, CombineDetail, DiagnosticSpec, ErrorKind, ErrorSpec, Family, Field, OpContext,
    OpError, OpOutput, OpSchema, RoleSpec, RunDetail, Unit,
};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::TraceError;
use geom_poly::winding::Winding;
use geom_run::{Exec, Phase, RunError, Stopped, WorkMeter};
use geom_topo::Body;

use crate::operand::{BooleanInput, Operand, OperandKey, PrepareError, Selection, prepare};
use crate::rebuild::{Merge, RebuildError, rebuild, select};

/// The arrangement's phase.
const ARRANGEMENT: Phase = Phase::new("arrangement");

/// The rebuild's phase.
const REBUILD: Phase = Phase::new("rebuild");

/// The boolean's schema (10.1).
pub const COMBINE: OpSchema = OpSchema {
    name: "combine.boolean",
    version: 1,
    family: Family::Combine,
    doc: "Unites, subtracts or intersects bodies in one arrangement, one result per selection.",
    inputs: &[
        Field {
            name: "operands",
            ty: "[Operand]",
            unit: Unit::None,
            optional: false,
            doc: "The bodies, each with a stable key and a kind.",
        },
        Field {
            name: "select",
            ty: "[Selection]",
            unit: Unit::None,
            optional: false,
            doc: "What to keep: one body comes back for each.",
        },
        Field {
            name: "merge",
            ty: "Merge",
            unit: Unit::None,
            optional: true,
            doc: "Whether flush faces merge, or keep their seams.",
        },
    ],
    outputs: &[Field {
        name: "bodies",
        ty: "[Body]",
        unit: Unit::None,
        optional: false,
        doc: "One body per selection, in order.",
    }],
    roles: &[RoleSpec {
        name: "tool",
        doc: "A face a tool brought into the result.",
    }],
    errors: &[
        ErrorSpec {
            detail: "DuplicateOperand",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "Two operands share a key.",
        },
        ErrorSpec {
            detail: "InvalidOperand",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "An operand cannot enter the arrangement.",
        },
        ErrorSpec {
            detail: "UnboundedResult",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The selection would include the outside.",
        },
    ],
    diagnostics: &[DiagnosticSpec {
        code: "TouchingOnly",
        doc: "An intersection is empty because its operands only touch.",
    }],
    classes: &[Class::Exact, Class::Certified],
    charges: &[Charge::new(ARRANGEMENT, "polygon a split clips", 1)],
    batch: false,
    example: "a 4 m cube less a 2 m cube at its corner: a cube with a corner notch",
};

/// The operation's error for a failure of the numbers layer.
fn numbers(ctx: &OpContext<'_>, e: NumError) -> OpError {
    OpError::new(ctx.path().clone(), e)
}

/// The operation's error for `detail`.
fn fail(ctx: &OpContext<'_>, detail: CombineDetail) -> OpError {
    OpError::new(ctx.path().clone(), detail)
}

/// `error` as the operation's.
fn prepared(ctx: &OpContext<'_>, error: PrepareError, meter: &WorkMeter<'_>) -> OpError {
    match error {
        PrepareError::Stopped(stopped) => ctx.stopped(meter, stopped, ARRANGEMENT),
        PrepareError::Num(e) => numbers(ctx, e),
        PrepareError::DuplicateOperand(_) => fail(ctx, CombineDetail::DuplicateOperand),
        PrepareError::UnknownOperand(_) => fail(ctx, CombineDetail::UnknownOperand),
        PrepareError::BadTest(_) => fail(ctx, CombineDetail::BadSelection),
        PrepareError::NotPlaneBased(_) => fail(ctx, CombineDetail::NotPlaneBased),
        PrepareError::Broken(_) => fail(ctx, CombineDetail::InvalidOperand),
    }
}

/// `error` as the operation's.
fn rebuilt(ctx: &OpContext<'_>, error: RebuildError, meter: &WorkMeter<'_>) -> OpError {
    match error {
        RebuildError::Stopped(stopped) | RebuildError::Trace(TraceError::Stopped(stopped)) => {
            ctx.stopped(meter, stopped, REBUILD)
        }
        RebuildError::Inside => fail(ctx, CombineDetail::UnboundedResult),
        RebuildError::Num(e) => numbers(ctx, e),
        _ => fail(ctx, CombineDetail::ResultInvalid),
    }
}

/// `error` as the operation's.
fn arranged(ctx: &OpContext<'_>, error: RunError<TraceError>, meter: &WorkMeter<'_>) -> OpError {
    match error {
        RunError::Failed(TraceError::Stopped(stopped)) => ctx.stopped(meter, stopped, ARRANGEMENT),
        RunError::Failed(TraceError::Num(e)) => numbers(ctx, e),
        RunError::Failed(TraceError::Winding(_)) => fail(ctx, CombineDetail::ResultInvalid),
        RunError::BudgetExceeded(exceeded) => {
            OpError::new(ctx.path().clone(), RunDetail::BudgetExceeded(exceeded))
        }
        RunError::Cancelled => OpError::new(ctx.path().clone(), RunDetail::Cancelled),
        RunError::OutOfMemory => OpError::new(ctx.path().clone(), RunDetail::OutOfMemory),
        RunError::Internal(what) => OpError::new(ctx.path().clone(), RunDetail::Internal { what }),
    }
}

/// One body for each of `input`'s selections.
///
/// # Errors
///
/// An [`OpError`]: operands that cannot enter, a selection that includes the outside, a stopped
/// meter, or a result the checker refuses, which is a defect.
pub fn combine(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    input: &BooleanInput<'_>,
) -> Result<OpOutput<Vec<Body>>, OpError> {
    let mut meter = ctx.meter();
    let ready = prepare(input, &mut meter).map_err(|e| prepared(ctx, e, &meter))?;
    let mut exactness = Exactness::Exact;
    for operand in &input.operands {
        exactness = exactness
            .combine(operand.exactness)
            .ok_or_else(|| numbers(ctx, NumError::BudgetExceeded))?;
    }
    let cell = around(&ready.polygons).map_err(|e| numbers(ctx, e))?;
    let classified = arrange(
        exec,
        cell,
        Winding::new(),
        &ready.polygons,
        LEAF_POLYGONS,
        &ready.tests,
        &mut meter,
    )
    .map_err(|e| arranged(ctx, e, &meter))?;
    let mut bodies = Vec::with_capacity(ready.tests.len());
    for test in &ready.tests {
        let kept = select(test, &classified);
        let body = rebuild(&kept, &ready.carriers, input.merge, &mut meter)
            .map_err(|e| rebuilt(ctx, e, &meter))?;
        let report = check(&body, exactness, Level::Cheap, &mut meter)
            .map_err(|stopped: Stopped| ctx.stopped(&meter, stopped, REBUILD))?;
        if !report.passed() {
            return Err(fail(ctx, CombineDetail::ResultInvalid));
        }
        bodies.push(body);
    }
    Ok(OpOutput::new(bodies, exactness))
}

/// The one body of `operands` under `selection`.
fn one(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
    selection: Selection,
) -> Result<OpOutput<Body>, OpError> {
    let input = BooleanInput {
        operands: operands.to_vec(),
        select: alloc::vec![selection],
        merge: Merge::RemoveRedundant,
    };
    let OpOutput {
        mut result,
        exactness,
        diagnostics,
    } = combine(ctx, exec, &input)?;
    let body = result
        .pop()
        .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
    let mut out = OpOutput::new(body, exactness);
    out.diagnostics = diagnostics;
    Ok(out)
}

/// Every cell inside any of `operands`.
///
/// # Errors
///
/// As [`combine`].
pub fn unite(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Unite)
}

/// Every cell inside `target` and outside every other operand, however many tools there are, in
/// one arrangement (11.6).
///
/// # Errors
///
/// As [`combine`].
pub fn subtract(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    target: OperandKey,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Subtract { target })
}

/// Every cell inside all of `operands`.
///
/// # Errors
///
/// As [`combine`].
pub fn intersect(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Intersect)
}
