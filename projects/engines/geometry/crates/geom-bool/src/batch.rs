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
//!   is marked and named in `bool.tool_misses_host`. A subtraction cannot go missing silently. A
//!   tool cuts exactly when its inside meets the host's, so a tool that holds a whole part of the
//!   host, with none of its faces reaching it, cuts too (issue #967).
//! - **Atomic by default.** A tool that cannot enter fails the call and names itself. Under
//!   [`BatchRule::SkipFailedTools`] it is left out and listed beside the result, and which tools
//!   are left out depends only on the inputs.
//! - **A cut again is a cut afresh.** A [`Cut`] may carry the last cut's [`CutResult::cache`]:
//!   operands it holds the same bodies for are not prepared again, boxes it holds the same
//!   polygons for are not solved again, and the result, its report, the cache and the work are
//!   what a cut without it gives (11.6, and G139's incremental re-cut,
//!   `projects/kit/notes/G139-incremental-rebuild.md`; tested on a moved door).
//! - **Hosts run in parallel, and the threads do not show.** Results come in input order, and one
//!   host alone has the threads for its boxes.
//!
//! Method: chapter 11.6 (`projects/engines/geometry/architecture/11-booleans.md`).
//!
//! Recursion:
//!
//! - Cycle: `<geom_pool::halves::Inline as Join>::join`, `<geom_pool::native::Rayon as Join>::join`, `<geom_pool::native::Rayon as Pool>::run_all`, `geom_bool::batch::cut`, `geom_bool::batch::one_cut`, `geom_bool::combine::Scene::cells`, `geom_bool::combine::Scene::fused`, `geom_bool::combine::Scene::keep`, `geom_bool::combine::Scene::kept`, `geom_bool::combine::Scene::partition`, `geom_bool::combine::Scene::section`, `geom_bool::combine::Scene::sheet`, `geom_bool::combine::Scene::split`, `geom_bool::combine::Scene::trim`, `geom_bool::combine::Scene::walled`, `geom_bool::combine::arranged_input`, `geom_bool::combine::selected`, `geom_poly::merge::around_shell`, `geom_poly::merge::by_line`, `geom_poly::merge::corners_of`, `geom_poly::merge::cut_at_ends`, `geom_poly::merge::merge_on`, `geom_poly::merge::merge_with`, `geom_poly::merge::merge_with_reusing`, `geom_poly::merge::merged`, `geom_poly::merge::nest`, `geom_poly::merge::numbered`, `geom_poly::merge::section`, `geom_poly::merge::sheets_of`, `geom_poly::merge::sides_of`, `geom_poly::merge::sides_of_first`, `geom_poly::subdivide::around_on`, `geom_poly::subdivide::arrange_reusing`, `geom_pool::halves::run_halves`, `geom_run::exec::Exec::sort_stable`, `geom_run::exec::Exec::try_map`, `geom_run::phase::run_items`, `geom_run::phase::run_phase`, `geom_run::pool::drive`, `geom_run::pool::run_wave`, `geom_run::sort::key_order`, `geom_run::sort::sort_stable`.
//!   Bound: 290.
//!   Why: the checker sees the pool's tasks call back into every function that runs a parallel
//!   phase, but a phase runs inside another only here: with several hosts `cut` spreads them
//!   over the pool and runs each host's cut on `Exec::serial()`, whose phases run inline on the
//!   caller and never reach the pool; with one host `cut` keeps the pool for that host's
//!   arrangement, and the merge after it runs its own phases on `Exec::serial()`. So at most two
//!   parallel phases are ever open at once, the hosts' and one inside it. Each holds its five
//!   frames from `try_map` to rayon's `run_all`, and `run_all` halves its wave with `run_halves`,
//!   each level one `run_halves` and one `join`, at most 64 levels (`geom_pool::native`: a slice
//!   is under 2^63 tasks). The longest way from the hosts' phase to one inside it is the merge's
//!   nesting: `run_items`, `cut`'s closure, `one_cut`, `selected`, `Scene::split`,
//!   `Scene::partition`, `Scene::walled`, `merge_with`, `merge_on`, `merged`, `nest`,
//!   `around_shell`, `sides_of_first` and `around_on`, whose phase runs `run_items` once more;
//!   a sort's phase is one frame longer to `run_all` and three nearer, and the arrangement's is
//!   nearer still. `cut` itself is on top. So at most 1 + 2 · (5 + 2 · 64) + 15 = 282 frames,
//!   under 290.
//!   Test: `parallel_phases_nest_at_most_twice`.
//!
//! Cost: one arrangement per host, linear in its faces plus local work around each tool. A tool
//! none of whose pieces has the host behind it, and whose box meets the host's, adds one trace of
//! the host's polygons against that tool's alone, stopping at the first the tool holds.

use alloc::vec::Vec;

use geom_op::{CombineDetail, Diagnostic, OpContext, OpError, OpOutput, RunDetail};
use geom_poly::convex::Piece;
use geom_poly::merge::{Memo, Merge};
use geom_poly::subdivide::{Solved, around};
use geom_poly::trace::{Classified, Polygon, TraceError, classify};
use geom_poly::winding::Winding;
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::combine::{Combined, arranged_input, fail, prepared, selected, traced};
use crate::names::NameIds;
use crate::operand::{
    BooleanInput, Operand, OperandKey, Operands, PrepareError, Prepared, Selection, prepare_reusing,
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
    /// What an earlier cut kept, from its [`CutResult::cache`]: an operand with the same body is not
    /// prepared again and a box holding the same polygons is not solved again, each charged what
    /// it cost (11.6). Whatever it does not match is ignored, so any cache gives the answer and the
    /// work of none.
    pub reuse: Option<&'b Recut>,
}

/// What a cut keeps for the next cut of its host (G139): each operand's preparation and the boxes
/// it solved, each keyed by what it was made from, what its merge kept, and its names' ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recut {
    operands: Operands,
    boxes: Solved,
    merge: Memo,
    names: NameIds,
}

impl Recut {
    /// The operands' preparations.
    #[must_use]
    pub const fn operands(&self) -> &Operands {
        &self.operands
    }

    /// The solved boxes.
    #[must_use]
    pub const fn boxes(&self) -> &Solved {
        &self.boxes
    }

    /// What the merge kept.
    #[must_use]
    pub const fn merge(&self) -> &Memo {
        &self.merge
    }

    /// The names' ids.
    #[must_use]
    pub const fn names(&self) -> &NameIds {
        &self.names
    }
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
    /// What this cut kept, for the next cut of the host to reuse.
    pub cache: Recut,
}

/// Whether the tool at `owner` took anything out of the host at `host`, both places in `keys`.
///
/// A tool takes some of the host exactly when the two insides meet, and that happens in one of two
/// ways. Either the tool's boundary runs through the host's inside, or on a face of it looking the
/// same way (11.5), and then a piece of the tool has the host behind it, which the arrangement
/// gives. Or no piece of the tool does, so the tool's boundary meets the host's inside nowhere,
/// and every part of the host's inside lies wholly inside the tool or wholly outside it; then a
/// part inside it has the tool just behind each of its polygons, which [`holds_host`] reads. A
/// tool that only touches the host has the host in front of it, and one that misses has it
/// nowhere. The first way alone missed a tool that holds a whole part of the host with none of its
/// faces reaching it, and reported it as taking nothing (issue #967).
fn cuts(
    ready: &Prepared,
    classified: &[Classified],
    (host, owner): (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<bool, TraceError> {
    let through = classified
        .iter()
        .any(|c| ready.owners.get(c.polygon) == Some(&owner) && c.back.count(0) > 0);
    if through {
        return Ok(true);
    }
    holds_host(ready, (host, owner), meter)
}

/// Whether the solid at `owner` holds some of the host at `host` inside it: whether, against that
/// tool's polygons alone, some polygon of the host has the tool behind it, just behind a corner of
/// the polygon, where the host's material is. The trace is the arrangement's own (11.3), so the
/// answer is exact. A cutter bounds no material and holds none, and a tool whose box misses the
/// host's holds none of it; neither is traced.
fn holds_host(
    ready: &Prepared,
    (host, owner): (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<bool, TraceError> {
    let solid = ready
        .keys
        .get(owner)
        .is_some_and(|(_, entry)| entry.is_some());
    let boxes = |at: usize| {
        ready
            .cells
            .get(at)
            .copied()
            .transpose()
            .map(Option::flatten)
    };
    let (Some(tool_box), Some(host_box)) = (boxes(owner)?, boxes(host)?) else {
        return Ok(false);
    };
    let meet = (0..3).all(|axis| {
        let (Some(a), Some(b), Some(c), Some(d)) = (
            tool_box.low.get(axis),
            host_box.high.get(axis),
            host_box.low.get(axis),
            tool_box.high.get(axis),
        ) else {
            return false;
        };
        a < b && c < d
    });
    if !solid || !meet {
        return Ok(false);
    }
    let of = |at: usize| {
        ready
            .polygons
            .iter()
            .zip(&ready.owners)
            .filter(move |(_, o)| **o == at)
            .map(|(polygon, _)| polygon)
    };
    // The tool's polygons on one entry of their own, then the host's polygon in turn, which counts
    // no crossing: the trace reads the tool alone.
    let mut polygons: Vec<Polygon> = of(owner)
        .map(|polygon| Polygon {
            piece: polygon.piece.clone(),
            entry: Some(0),
        })
        .collect();
    let tool = polygons.len();
    let mut every = polygons.clone();
    every.extend(of(host).cloned());
    let cell = around(&every)?;
    let mut pieces: Vec<Vec<Piece>> = alloc::vec![Vec::new(); tool];
    for polygon in of(host) {
        polygons.truncate(tool);
        polygons.push(Polygon {
            piece: polygon.piece.clone(),
            entry: None,
        });
        pieces.truncate(tool);
        pieces.push(alloc::vec![polygon.piece.clone()]);
        let read = classify(&cell, &Winding::new(), &polygons, &pieces, meter)?;
        if read.iter().any(|c| c.back.count(0) > 0) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The tools of `cut` that can enter, prepared with them.
fn entered<'b>(
    ctx: &OpContext<'_>,
    cut: &Cut<'b>,
    rule: BatchRule,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<OperandKey>, BooleanInput<'b>, Prepared, Operands), OpError> {
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
        match prepare_reusing(&input, true, cut.reuse.map(Recut::operands), meter) {
            Ok((ready, operands)) => return Ok((skipped, input, ready, operands)),
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
    let (skipped, input, ready, operands) = entered(ctx, cut, rule, meter)?;
    let (classified, exactness, boxes) = arranged_input(
        ctx,
        exec,
        &input,
        &ready,
        cut.reuse.map(Recut::boxes),
        meter,
    )?;
    let again = cut.reuse.map(|last| (&last.merge, &last.names));
    let (mut out, kept) = selected(
        ctx,
        &input,
        (&ready, &classified[..], again),
        exactness,
        meter,
    )?;
    let (merge, names) = kept.unwrap_or_default();
    let cache = Recut {
        operands,
        boxes,
        merge,
        names,
    };
    let result = out
        .result
        .pop()
        .and_then(|mut list| list.pop())
        .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
    let mut tools = Vec::with_capacity(ready.keys.len());
    let mut notes = core::mem::take(&mut out.diagnostics);
    let host = ready
        .keys
        .iter()
        .position(|(key, _)| *key == cut.host.key)
        .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
    for (owner, (key, _)) in ready.keys.iter().enumerate() {
        if *key != cut.host.key {
            let took = cuts(&ready, &classified, (host, owner), meter)
                .map_err(|e| traced(ctx, e, meter))?;
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
            cache,
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
