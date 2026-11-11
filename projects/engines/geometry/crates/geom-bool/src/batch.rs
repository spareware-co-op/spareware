//! Batch cutting: any number of tools through any number of hosts (11.6).
//!
//! What it does: [`cut`] subtracts each [`Cut`]'s tools from its host in one arrangement, with the
//! tools summed into one entry, so a winding vector stays two entries long however many tools
//! there are, and tools that overlap or share a jamb need no union first. Each host gives one
//! [`CutResult`], in input order, which accounts for every tool.
//!
//! Guarantees:
//!
//! - **Every tool is accounted for.** Each is reported, and one that touched nothing inside the host
//!   is marked and named in `bool.tool_misses_host`. A subtraction cannot go missing silently.
//! - **Atomic by default.** A tool that cannot enter fails the call and names itself. Under
//!   [`BatchRule::SkipFailedTools`] it is left out and listed beside the result, and which tools
//!   are left out depends only on the inputs.
//! - **Hosts run in parallel, and the threads do not show.** Results come in input order, and one
//!   host alone has the threads for its boxes.
//!
//! Method: chapter 11.6 (`projects/engines/geometry/architecture/11-booleans.md`).
//!
//! Cost: one arrangement per host, linear in its faces plus local work around each tool.

use alloc::vec::Vec;

use geom_op::{CombineDetail, Diagnostic, OpContext, OpError, OpOutput, RunDetail};
use geom_poly::merge::Merge;
use geom_poly::trace::Classified;
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::combine::{Combined, arranged_input, fail, prepared, selected};
use crate::operand::{
    BooleanInput, Operand, OperandKey, PrepareError, Prepared, Selection, prepare_with,
};

/// One host at a time on each thread.
const ONE_HOST: ChunkPlan = ChunkPlan::fixed::<1>();

/// The phase of the hosts.
const HOSTS: Phase = Phase::new("hosts");

/// What to do with a tool that cannot enter (11.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRule {
    /// Fail the call, and name it.
    Atomic,
    /// Cut the rest, and list it beside the result.
    SkipFailedTools,
}

/// One host and its tools.
#[derive(Clone, Debug)]
pub struct Cut<'b> {
    /// The host.
    pub host: Operand<'b>,
    /// The tools.
    pub tools: Vec<Operand<'b>>,
}

/// What one tool did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolReport {
    /// The tool's key.
    pub key: OperandKey,
    /// Whether it took anything out of the host.
    pub cut: bool,
}

/// What one host gives.
#[derive(Clone, Debug, PartialEq)]
pub struct CutResult {
    /// The cut body.
    pub result: Combined,
    /// Every tool that entered, by key.
    pub tools: Vec<ToolReport>,
    /// The tools left out under [`BatchRule::SkipFailedTools`], by key.
    pub skipped: Vec<OperandKey>,
}

/// Whether the tool at `owner` took anything out: a piece of it with the host behind it. A tool
/// that only touches the host has the host in front of it, and one that misses has it nowhere.
fn cuts(ready: &Prepared, classified: &[Classified], owner: usize) -> bool {
    classified
        .iter()
        .any(|c| ready.owners.get(c.polygon) == Some(&owner) && c.back.count(0) > 0)
}

/// The tools of `cut` that can enter, prepared with them.
fn entered<'b>(
    ctx: &OpContext<'_>,
    cut: &Cut<'b>,
    rule: BatchRule,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<OperandKey>, BooleanInput<'b>, Prepared), OpError> {
    let mut skipped = Vec::new();
    let mut tools = cut.tools.clone();
    loop {
        let mut operands = alloc::vec![cut.host];
        operands.extend(tools.iter().copied());
        let input = BooleanInput {
            operands,
            select: alloc::vec![Selection::Subtract {
                target: cut.host.key
            }],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        match prepare_with(&input, true, meter) {
            Ok(ready) => return Ok((skipped, input, ready)),
            Err(PrepareError::NotPlaneBased(key) | PrepareError::Broken(key))
                if rule == BatchRule::SkipFailedTools && key != cut.host.key =>
            {
                skipped.push(key);
                tools.retain(|tool| tool.key != key);
            }
            Err(e) => return Err(prepared(ctx, e, meter)),
        }
    }
}

/// One host's cut.
fn one_cut(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    cut: &Cut<'_>,
    rule: BatchRule,
    meter: &mut WorkMeter<'_>,
) -> Result<OpOutput<CutResult>, OpError> {
    let (skipped, input, ready) = entered(ctx, cut, rule, meter)?;
    let (classified, exactness) = arranged_input(ctx, exec, &input, &ready, meter)?;
    let mut out = selected(ctx, &input, &ready, &classified, exactness, meter)?;
    let result = out
        .result
        .pop()
        .and_then(|mut list| list.pop())
        .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
    let mut tools = Vec::with_capacity(ready.keys.len());
    let mut notes = core::mem::take(&mut out.diagnostics);
    for (owner, (key, _)) in ready.keys.iter().enumerate() {
        if *key != cut.host.key {
            let took = cuts(&ready, &classified, owner);
            if !took {
                notes.extend(Diagnostic::new(
                    "bool.tool_misses_host",
                    alloc::format!("tool {:02x?} took nothing out of its host", key.0),
                ));
            }
            tools.push(ToolReport {
                key: *key,
                cut: took,
            });
        }
    }
    let mut done = OpOutput::new(
        CutResult {
            result,
            tools,
            skipped,
        },
        exactness,
    );
    done.diagnostics = notes;
    Ok(done)
}

/// Each of `cuts`, in input order.
///
/// # Errors
///
/// An [`OpError`]: what [`crate::combine::combine`] gives, and under [`BatchRule::Atomic`] a tool
/// that cannot enter.
pub fn cut(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    cuts: &[Cut<'_>],
    rule: BatchRule,
) -> Result<OpOutput<Vec<CutResult>>, OpError> {
    let mut meter = ctx.meter();
    // One host has the threads to itself, for its boxes; many share them out, one host each.
    if let [one] = cuts {
        let done = one_cut(ctx, exec, one, rule, &mut meter)?;
        let (result, exactness, diagnostics) = (done.result, done.exactness, done.diagnostics);
        let mut out = OpOutput::new(alloc::vec![result], exactness);
        out.diagnostics = diagnostics;
        return Ok(out);
    }
    let outputs = exec
        .try_map(&mut meter, HOSTS, ONE_HOST, cuts, |_, one, m| {
            one_cut(ctx, &Exec::serial(), one, rule, m)
        })
        .map_err(|e| match e {
            RunError::Failed(e) => e,
            RunError::BudgetExceeded(exceeded) => {
                OpError::new(ctx.path().clone(), RunDetail::BudgetExceeded(exceeded))
            }
            RunError::Cancelled => OpError::new(ctx.path().clone(), RunDetail::Cancelled),
            RunError::OutOfMemory => OpError::new(ctx.path().clone(), RunDetail::OutOfMemory),
            RunError::Internal(what) => {
                OpError::new(ctx.path().clone(), RunDetail::Internal { what })
            }
        })?;
    let mut exactness = geom_num::Exactness::Exact;
    let mut results = Vec::with_capacity(outputs.len());
    let mut notes = Vec::new();
    for output in outputs {
        exactness = exactness
            .combine(output.exactness)
            .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
        notes.extend(output.diagnostics);
        results.push(output.result);
    }
    let mut out = OpOutput::new(results, exactness);
    out.diagnostics = notes;
    Ok(out)
}
