//! Fuzz target: whole operation logs. On any bytes, a log of primitives, transforms and patterns,
//! each on any earlier result, replays to the same hash, and every step keeps every law.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_OPERATIONS`]
//! operations on the hard cases of chapter 28.5, each transform on any earlier result, each new
//! primitive free to start exactly on a point of an earlier one or one step from it, some patterns
//! of [`MANY_COPIES`] copies. The first operation is always a primitive, and half of them are
//! tamed to sizes the grid can hold, so a log gets past its first step and its later laws run. [`check`] runs it through the kernel's replay, one operation at a
//! time, and holds each to chapter 28.5's oracle stack; then it writes the operations that
//! succeeded as a log and replays that. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, and [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived
//!   below; a unit test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a log
//!   the kernel refuses; a defect, or an error no schema lists; a result without names, or one the
//!   full checker rejects; a history that breaks its laws, alone or composed along a chain; a class
//!   that rises; an input that changes; a log that does not read back as written; a replay of the
//!   log that gives another hash; a budget that changes a result instead of stopping it.
//! - **No hang.** A case's operations run under one work budget of [`WORK`] units between them:
//!   each is given what the earlier ones left. Running out is a typed stop, which is legal; the
//!   case ends at that operation, and its stop is held to laws of its own: it is the budget the
//!   operation was given, a work budget, and a replay of the log with the stopped operation stops
//!   with the same error after the same work.
//! - **Not yet in the stack:** a thread-count rerun waits for the first parallel operation, since
//!   none of the operations a log can hold uses the helper, and an imported Tolerant body waits for
//!   the first importer.
//!
//! Method: chapter 28.5's sequence target and oracle stack
//! (`projects/engines/geometry/architecture/28-testing.md`), run through chapter 28.16's replay.
//!
//! Cost: at most [`MAX_OPERATIONS`] operations, charging at most [`WORK`] units between them,
//! each checked in full, then two replays and one replay under a cut budget, and for a stopped
//! case one replay to its stop; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_check::{Level, check as full_check};
use geom_format::content::ContentId;
use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_make::PrismInput;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{
    ErrorKind, History, NameId, NameSet, OpError, OpErrorDetail, OpId, OpPath, RunDetail,
};
use geom_run::{Budget, BudgetKind, WorkMeter};
use geom_test::fixture::calls::{Call, Input, Place, Turn};
use geom_test::fixture::kernel::{Kernel, id_of};
use geom_test::fixture::run::{Engine, Ran, Replayed, Step as Done, replay};
use geom_test::fixture::{OpLog, Spend};
use geom_test::history::check_chain;
use geom_test::{ByteSource, Entropy};
use geom_topo::Body;

use crate::Finding;
use crate::adversarial::{NUDGE_BYTES, TWO_AND_A_HALF, beside, nudge, repeats};
use crate::make::{Motion, RotationBy, SHAPE_BYTES, Shape, shape, step as read_step};

/// The most operations a case runs.
pub const MAX_OPERATIONS: u8 = 12;

/// The work units each case may spend, its operations together.
///
/// Why this many. An ordinary case is cheap: of a 775-case corpus from a long fuzzing run, the
/// operations of the median case charged 849 units, of 95% of them at most 1,344, and of every
/// case without a wall of [`MANY_COPIES`] copies at most 1,370. `WORK` is 7.4 times that 95th
/// percentile, the headroom the curves and surfaces targets' budgets have over theirs (9.1 and
/// 8.0 times, measured the same way on their own corpora).
///
/// A wall is not an ordinary case: one charges about 19,000 to 40,000 units, by the body it
/// copies, so under `WORK` a wall stops part way, its copies made and checked up to the stop and
/// the stop held to its laws. The test `the_extreme_wall_is_reached` runs a whole wall without
/// the budget, so the laws of a whole wall still run on every `cargo test`.
///
/// What a unit costs. Each operation runs four times (the case, two replays and the cut one) and
/// each result is checked in full and written twice for its content ID, so where walls are a unit
/// charged costs about 70,000 instructions in all. Unbounded, the slowest cases found held one to
/// four walls, charged 40,000 to 158,000 units and ran 3 to 10.6 thousand million instructions.
pub const WORK: u64 = 10_000;

/// The copies of the longest pattern: the 300 openings of chapter 28.5's extreme wall.
pub const MANY_COPIES: u16 = 300;

/// The bytes an operation reads before its own: what it is, what it takes, and where it starts.
const HEAD_BYTES: usize = 3 + NUDGE_BYTES;

/// The most bytes [`decode`] reads from any input: a count, then for each operation its head and
/// the largest it can read, a primitive's.
pub const MAX_INPUT_BYTES: usize = 1 + 1 + 8 + 12 * (HEAD_BYTES + SHAPE_BYTES);

/// One operation of a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A primitive, on a point of an earlier result when `at` says so.
    Make {
        /// The primitive.
        shape: Shape,
        /// Which of the taken result's grid points to start on, and how far from it.
        at: Option<(u8, [i64; 3])>,
    },
    /// A transform or a pattern of an earlier result.
    Transform(crate::make::Step),
}

/// One operation and the earlier result it takes, counted modulo how many there are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// What it does.
    pub item: Item,
    /// Which result it takes.
    pub on: u8,
}

/// One case: the operations, and the operation and work of the replay under a cut budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The operations, in order.
    pub entries: Vec<Entry>,
    /// The operation of the log to run under `cut_work`, modulo the log's length.
    pub cut_at: u8,
    /// The work budget of the cut replay.
    pub cut_work: u64,
}

/// `size` brought under a metre, and never zero.
const fn smaller(size: Steps) -> Steps {
    Steps((size.0 % (1 << 16)).saturating_add(1))
}

/// `at` brought within a few metres of the origin.
fn nearer(at: P3) -> P3 {
    let c = |v: i64| v % (1 << 24);
    P3::new(c(at.x.get()), c(at.y.get()), c(at.z.get())).unwrap_or(at)
}

/// `made` with its sizes and its place brought to what the grid can hold, keeping which steps of
/// the sub-grids they fell on.
fn tame(made: Shape) -> Shape {
    match made {
        Shape::Box(mut i) => {
            i.min = nearer(i.min);
            i.size = i.size.map(smaller);
            Shape::Box(i)
        }
        Shape::Prism(i) => Shape::Prism(PrismInput {
            base: i.base.into_iter().map(nearer).collect(),
            height: smaller(i.height),
        }),
        Shape::Cylinder(mut i) => {
            i.base = nearer(i.base);
            (i.radius, i.height) = (smaller(i.radius), smaller(i.height));
            Shape::Cylinder(i)
        }
        Shape::Cone(mut i) => {
            i.base = nearer(i.base);
            (i.radius, i.height) = (smaller(i.radius), smaller(i.height));
            Shape::Cone(i)
        }
        Shape::Sphere(mut i) => {
            i.center = nearer(i.center);
            i.radius = smaller(i.radius);
            Shape::Sphere(i)
        }
        Shape::Torus(mut i) => {
            i.center = nearer(i.center);
            i.minor = smaller(i.minor);
            i.major = Steps(
                (i.major.0 % (1 << 16))
                    .saturating_add(i.minor.0)
                    .saturating_add(1),
            );
            Shape::Torus(i)
        }
    }
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let count = src.byte() % (MAX_OPERATIONS + 1);
    let cut_at = src.byte();
    let cut_work = src.word() % 4000;
    let mut entries = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let rule = src.byte();
        let on = src.byte();
        let anchored = src.byte();
        let near = nudge(&mut src);
        // Nothing can be transformed until something is made.
        let kind = if entries.is_empty() { 0 } else { rule % 4 };
        let item = match kind {
            0 => {
                let made = shape(&mut src);
                Item::Make {
                    shape: if rule & 0x80 == 0 { tame(made) } else { made },
                    at: (anchored & 1 != 0).then_some((anchored >> 1, near)),
                }
            }
            // Few rules are the extreme wall: its copies stand one pitch apart.
            1 if rule >> 3 == 0 => Item::Transform(crate::make::Step {
                motions: repeats(MANY_COPIES, TWO_AND_A_HALF)
                    .into_iter()
                    .map(Motion::Move)
                    .collect(),
                pattern: true,
                accept: None,
                work: 0,
            }),
            _ => Item::Transform(read_step(&mut src)),
        };
        entries.push(Entry { item, on });
    }
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            entries,
            cut_at,
            cut_work,
        },
        read,
    )
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// A result of the log so far.
struct Held {
    id: ContentId,
    body: Body,
    exactness: Exactness,
    /// The step that made it, by its place in the kernel's log, for walking a chain back.
    made_by: usize,
    /// The result it was made from, and whether it stands alone (not a pattern's copy).
    from: Option<usize>,
    alone: bool,
}

/// `body`'s names as a set; empty for a body with none.
fn names_of(body: &Body) -> Option<NameSet> {
    let names = body.names()?;
    NameSet::new(
        names
            .regions
            .iter()
            .chain(&names.faces)
            .chain(&names.edges)
            .chain(&names.vertices)
            .map(|k| NameId::from_bytes(k.0)),
    )
    .ok()
}

/// The call a shape stands for.
fn make_call(shape: &Shape, at: Option<P3>) -> Call {
    match shape {
        Shape::Box(i) => {
            let mut i = *i;
            i.min = at.unwrap_or(i.min);
            Call::Box(i)
        }
        Shape::Prism(i) => Call::Prism(i.clone()),
        Shape::Cylinder(i) => {
            let mut i = *i;
            i.base = at.unwrap_or(i.base);
            Call::Cylinder(i)
        }
        Shape::Cone(i) => {
            let mut i = *i;
            i.base = at.unwrap_or(i.base);
            Call::Cone(i)
        }
        Shape::Sphere(i) => {
            let mut i = *i;
            i.center = at.unwrap_or(i.center);
            Call::Sphere(i)
        }
        Shape::Torus(i) => {
            let mut i = *i;
            i.center = at.unwrap_or(i.center);
            Call::Torus(i)
        }
    }
}

const fn turn(by: RotationBy) -> Turn {
    match by {
        RotationBy::Quarter(turns) => Turn::Quarter(turns),
        RotationBy::Direction(cos, sin) => Turn::Direction(cos, sin),
    }
}

/// Where a pattern's copy goes, from a motion: a pattern's transforms have no centre.
const fn place(motion: Motion) -> Place {
    match motion {
        Motion::Move(offset) => Place::Move(offset),
        Motion::Rotate(axis, by, _) => Place::Rotate(axis, turn(by)),
        Motion::Mirror(axis, _) => Place::Mirror(axis),
        Motion::Scale([(num, den), ..], _) => Place::Scale(num, den),
    }
}

/// The call of a transform of a body whose record is `exactness`.
fn transform_call(step: &crate::make::Step, exactness: Exactness) -> Option<Call> {
    let input = Input {
        exactness,
        accept: step.accept.map(|bound| Exactness::Certified {
            bound: Steps(bound),
            step: GridStep::Q0,
        }),
    };
    if step.pattern {
        let copies = step
            .motions
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let mut key = [0u8; 16];
                let bytes = u64::try_from(k).unwrap_or(u64::MAX).to_be_bytes();
                key.iter_mut().zip(bytes).for_each(|(slot, b)| *slot = b);
                (key, place(*m))
            })
            .collect();
        return Some(Call::Pattern(input, copies));
    }
    Some(match *step.motions.first()? {
        Motion::Move(offset) => Call::Move(input, offset),
        Motion::Rotate(axis, by, center) => Call::Rotate(input, axis, turn(by), center),
        Motion::Mirror(axis, at) => Call::Mirror(input, axis, at),
        Motion::Scale(factors, center) => Call::Scale(input, factors, center),
    })
}

/// The record of operation number `at`, under `budget`.
fn record(at: usize, call: &Call, input: Option<&ContentId>, budget: Budget) -> Option<Operation> {
    let mut id = [0u8; 16];
    let bytes = u64::try_from(at).ok()?.to_be_bytes();
    id.iter_mut().zip(bytes).for_each(|(slot, b)| *slot = b);
    Some(Operation {
        op: OpPath::new(OpId::from_bytes(id)),
        name: call.name().to_owned(),
        version: 1,
        inputs: input
            .map(|id| vec![RecipeInput::Body(id.clone())])
            .unwrap_or_default(),
        params: call.params().ok()?,
        budget,
    })
}

/// A refusal is typed, and of a kind some schema lists; a defect is a finding.
fn refused(case: &Case, e: &OpError) -> Result<(), Finding> {
    let listed = matches!(
        e.kind(),
        ErrorKind::Degenerate
            | ErrorKind::InvalidInput
            | ErrorKind::FrameRangeExceeded
            | ErrorKind::ExactnessNotAccepted
            | ErrorKind::Unsupported
            | ErrorKind::BudgetExceeded
    );
    if listed {
        Ok(())
    } else {
        Err(Finding::new("a refusal is typed, never a defect", || {
            format!("{:?}: {}: {case:?}", e.detail(), e.message())
        }))
    }
}

/// `body` carries its names and passes the full checker at its record.
///
/// # Errors
///
/// A [`Finding`] for a body without names, or one the checker fails.
pub fn judge(body: &Body, exactness: Exactness) -> Result<(), Finding> {
    if body.names().is_none() {
        return Err(Finding::new("a result carries its names", String::new));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = full_check(body, exactness, Level::Full, &mut meter)
        .map_err(|_| Finding::new("an unlimited check finishes", String::new))?;
    if report.passed() {
        Ok(())
    } else {
        Err(Finding::new("every result passes the full checker", || {
            format!("{:?}", report.faults())
        }))
    }
}

/// The histories along the chain of single results that ends at `at`, oldest first, with the
/// names each took.
fn chain_to(results: &[Held], steps: &[(History, NameSet)], at: usize) -> Vec<(History, NameSet)> {
    let mut places = Vec::new();
    let mut here = Some(at);
    while let Some(i) = here {
        let Some(result) = results.get(i) else { break };
        if !result.alone {
            break;
        }
        places.push(result.made_by);
        here = result.from;
    }
    places.reverse();
    places
        .into_iter()
        .filter_map(|p| steps.get(p).cloned())
        .collect()
}

/// Where the case's budget stopped it: the operation it stopped, the error and what the operation
/// charged before it stopped.
struct Stop {
    operation: Operation,
    error: OpError,
    charged: Spend,
}

/// What running a case's operations left: the operations that succeeded, each result and each
/// step's history, what the operations charged between them, failed ones included, and the stop,
/// if the case's budget ran out.
struct Run {
    log: Vec<Operation>,
    results: Vec<Held>,
    steps: Vec<(History, NameSet)>,
    spent: u64,
    stop: Option<Stop>,
}

/// A stop is the case's budget's: a work budget passed, and the one the operation was given.
fn the_budgets(case: &Case, e: &OpError, given: Budget) -> Result<(), Finding> {
    match e.detail() {
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x))
            if x.kind == BudgetKind::Work && x.budget == given =>
        {
            Ok(())
        }
        other => Err(Finding::new("a stop is the case budget's", || {
            format!("{other:?} under {given:?}: {case:?}")
        })),
    }
}

/// Runs each operation of `case`, under `work` units between them, and holds it to the laws of
/// one step.
#[allow(
    clippy::too_many_lines,
    reason = "one operation's whole round: its call, its run, then each law in turn"
)]
fn run(case: &Case, work: u64) -> Result<Run, Finding> {
    let mut kernel = Kernel::new();
    kernel
        .start(Steps(0), &[])
        .map_err(|why| Finding::new("a log with no inputs starts", || why))?;
    let mut run = Run {
        log: Vec::new(),
        results: Vec::new(),
        steps: Vec::new(),
        spent: 0,
        stop: None,
    };
    for entry in &case.entries {
        // Each operation may spend what the case's earlier ones left of its budget.
        let budget = Budget::of_work(work.saturating_sub(run.spent));
        let from = usize::from(entry.on).checked_rem(run.results.len());
        let taken = from.and_then(|i| run.results.get(i));
        let (call, input) = match &entry.item {
            Item::Make { shape, at } => {
                let point = at.and_then(|(which, near)| {
                    // Grid points only: a meeting of planes has no grid place of its own.
                    let points: Vec<P3> = taken?
                        .body
                        .arenas()
                        .points
                        .iter()
                        .filter_map(|p| p.grid())
                        .collect();
                    let which = usize::from(which).checked_rem(points.len())?;
                    Some(beside(*points.get(which)?, near))
                });
                (make_call(shape, point), None)
            }
            Item::Transform(step) => {
                let Some(result) = taken else { continue };
                let Some(call) = transform_call(step, result.exactness) else {
                    continue;
                };
                (call, Some((result.id.clone(), result.exactness)))
            }
        };
        let Some(operation) = record(run.log.len(), &call, input.as_ref().map(|i| &i.0), budget)
        else {
            continue;
        };
        // A rotation or a scale factor that does not exist is no call: the kernel refuses it, and
        // a generator has no business writing one into a log.
        if !call.exists() {
            continue;
        }
        let before = input.as_ref().map(|(id, _)| id);
        match kernel.run(&operation) {
            Ran::Done(step) => {
                let Done {
                    history,
                    exactness,
                    work,
                    ..
                } = step;
                run.spent = run.spent.saturating_add(work.work);
                let source = before.and_then(|id| kernel.body(id));
                let names = source.map_or_else(|| Some(NameSet::default()), names_of);
                let names = names.ok_or_else(|| {
                    Finding::new("an input has its names", || format!("{case:?}"))
                })?;
                let history = history
                    .ok_or_else(|| Finding::new("an operation records its history", String::new))?;
                history.check(&names).map_err(|e| {
                    Finding::new("a step's history keeps its laws", || {
                        format!("{e:?}: {case:?}")
                    })
                })?;
                if let Some((_, was)) = &input
                    && exactness.rank() < was.rank()
                {
                    return Err(Finding::new("the class never rises", || {
                        format!("{case:?}")
                    }));
                }
                let made = run.steps.len();
                let alone = kernel.last().len() == 1;
                for body in kernel.last().to_vec() {
                    judge(&body, exactness)
                        .map_err(|f| Finding::new(&f.law, || format!("{case:?}")))?;
                    let id = id_of(&body)
                        .map_err(|why| Finding::new("a result has a stored form", || why))?;
                    run.results.push(Held {
                        id,
                        body,
                        exactness,
                        made_by: made,
                        from,
                        alone,
                    });
                }
                run.steps.push((history, names));
                run.log.push(operation);
            }
            // The case's budget ran out: the case ends at this operation, with its stop.
            Ran::Failed(error, charged) if error.kind() == ErrorKind::BudgetExceeded => {
                the_budgets(case, &error, budget)?;
                run.spent = run.spent.saturating_add(charged.work);
                run.stop = Some(Stop {
                    operation,
                    error,
                    charged,
                });
            }
            Ran::Failed(e, charged) => {
                run.spent = run.spent.saturating_add(charged.work);
                refused(case, &e)?;
            }
            Ran::Refused(why) => {
                return Err(Finding::new("the kernel runs what a log writes", || {
                    format!("{why}: {case:?}")
                }));
            }
        }
        if let (Some(id), Some(result)) = (before, from.and_then(|i| run.results.get(i)))
            && id_of(&result.body).as_ref() != Ok(id)
        {
            return Err(Finding::new("an input never changes", || {
                format!("{case:?}")
            }));
        }
        if run.stop.is_some() {
            break;
        }
    }
    Ok(run)
}

/// A case its budget stopped stops alike when replayed: `got`, the replay of the log and the
/// stopped operation, is a budget's stop with the error the case met, after the work the log of
/// successes charged on replay (`before`) and what the stop charged in the case: so at the same
/// operation, at the same point of it.
fn stops_alike(case: &Case, got: &Replayed, before: Spend, stop: &Stop) -> Result<(), Finding> {
    match got {
        Replayed::Failed {
            kind: ErrorKind::BudgetExceeded,
            entities,
            work,
            ..
        } if entities.as_slice() == stop.error.entities() && *work == before.plus(stop.charged) => {
            Ok(())
        }
        other => Err(Finding::new(
            "a replay of a stopped case stops where it stopped",
            || format!("{other:?} after {before:?}: {case:?}"),
        )),
    }
}

/// `log` with the stopped operation at its end.
fn to_the_stop(log: &OpLog, stop: &Stop) -> OpLog {
    let mut stopped = log.clone();
    stopped.operations.push(stop.operation.clone());
    stopped
}

/// Holds the log as a whole to its laws: it reads back, replays alike, and a cut budget stops it;
/// and where the case's budget stopped it, the replay stops there too.
fn replays(case: &Case, log: Vec<Operation>, stop: Option<&Stop>) -> Result<(), Finding> {
    let log = OpLog::new(Vec::new(), log);
    let bytes = log
        .encode()
        .map_err(|e| Finding::new("a log encodes", || format!("{e:?}: {case:?}")))?;
    let back = OpLog::decode(&bytes)
        .map_err(|e| Finding::new("a log reads back", || format!("{e:?}: {case:?}")))?;
    if back != log {
        return Err(Finding::new("a log reads back as written", || {
            format!("{case:?}")
        }));
    }
    let first = replay(&mut Kernel::new(), &log);
    let Replayed::Done { hash, work, .. } = &first else {
        return Err(Finding::new("a log of successes replays", || {
            format!("{first:?}: {case:?}")
        }));
    };
    if replay(&mut Kernel::new(), &back) != first {
        return Err(Finding::new("a replay gives the same hash", || {
            format!("{case:?}")
        }));
    }
    if let Some(at) = usize::from(case.cut_at).checked_rem(log.operations.len()) {
        let mut cut = log.clone();
        if let Some(operation) = cut.operations.get_mut(at) {
            operation.budget = Budget::new(case.cut_work, u64::MAX);
        }
        match replay(&mut Kernel::new(), &cut) {
            Replayed::Done { hash: got, .. } if got == *hash => {}
            Replayed::Failed {
                kind: ErrorKind::BudgetExceeded,
                ..
            } => {}
            other => {
                return Err(Finding::new(
                    "a budget stops a result or leaves it alone",
                    || format!("{other:?}: {case:?}"),
                ));
            }
        }
    }
    match stop {
        Some(stop) => stops_alike(
            case,
            &replay(&mut Kernel::new(), &to_the_stop(&log, stop)),
            *work,
            stop,
        ),
        None => Ok(()),
    }
}

/// Holds the case's every outcome to the laws, under [`WORK`].
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    reached(case).map(|_| ())
}

/// One line for reference mode's comparison (23.13; piece G106): the log's hash, which holds every
/// operation's bodies, names and history, and the result's measures; `None` for a case with
/// nothing to replay, or whose run breaks a law, which `check` reports.
#[must_use]
pub fn replay_line(case: &Case) -> Option<String> {
    let done = run(case, WORK).ok()?;
    let mut log = OpLog::new(Vec::new(), done.log);
    // A case its budget stopped replays to its stop.
    if let Some(stop) = &done.stop {
        log = to_the_stop(&log, stop);
    }
    if log.operations.is_empty() {
        return None;
    }
    match replay(&mut Kernel::new(), &log) {
        Replayed::Done { hash, measures, .. } => {
            Some(format!("{} {measures:?}", String::from(hash)))
        }
        Replayed::Failed { hash, kind, .. } => {
            Some(format!("{} {}", String::from(hash), kind.name()))
        }
        Replayed::Broken(why) => Some(format!("broken {why}")),
    }
}

/// How far a case got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reached {
    /// How many of its operations succeeded.
    pub succeeded: usize,
    /// The work units its operations charged between them, failed ones included.
    pub work: u64,
    /// Whether its budget stopped it.
    pub stopped: bool,
}

/// [`check`], and how far the case got: what a test uses to show that a generator's cases get
/// past their first step, and where the budget stops a case.
///
/// # Errors
///
/// As [`check`].
pub fn reached(case: &Case) -> Result<Reached, Finding> {
    reached_under(case, WORK)
}

/// [`reached`] with `work` units for the case in place of [`WORK`]: what a test uses to stop a
/// small case, and hold its stop to the laws, without spending a whole budget.
///
/// # Errors
///
/// As [`check`].
pub fn reached_under(case: &Case, work: u64) -> Result<Reached, Finding> {
    let done = run(case, work)?;
    if let Some(last) = done.results.len().checked_sub(1) {
        let chain = chain_to(&done.results, &done.steps, last);
        check_chain(&chain).map_err(|e| {
            Finding::new("a chain of histories keeps its laws", || {
                format!("{e:?}: {case:?}")
            })
        })?;
    }
    let got = Reached {
        succeeded: done.log.len(),
        work: done.spent,
        stopped: done.stop.is_some(),
    };
    replays(case, done.log, done.stop.as_ref())?;
    Ok(got)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests stop loudly"
)]
mod tests {
    use super::*;
    use crate::make::{Motion, Step};
    use geom_make::BoxInput;
    use geom_run::{Exceeded, Phase, Usage};

    /// A box, then a pattern of three of its copies.
    fn small() -> Case {
        let made = Entry {
            item: Item::Make {
                shape: Shape::Box(BoxInput {
                    min: P3::new(0, 0, 0).unwrap(),
                    size: [Steps(10), Steps(20), Steps(30)],
                }),
                at: None,
            },
            on: 0,
        };
        let pattern = Entry {
            item: Item::Transform(Step {
                motions: vec![
                    Motion::Move([100, 0, 0]),
                    Motion::Move([200, 0, 0]),
                    Motion::Move([300, 0, 0]),
                ],
                pattern: true,
                accept: None,
                work: 0,
            }),
            on: 0,
        };
        Case {
            entries: vec![made, pattern],
            cut_at: 0,
            cut_work: 40,
        }
    }

    /// `small` run under a budget the box leaves one unit of: it stops at the pattern.
    fn stopped() -> Run {
        let mut only_box = small();
        only_box.entries.truncate(1);
        let the_box = run(&only_box, u64::MAX).unwrap().spent;
        let done = run(&small(), the_box.saturating_add(1)).unwrap();
        assert_eq!(done.log.len(), 1);
        assert!(done.stop.is_some());
        done
    }

    #[test]
    fn a_small_case_runs_whole_under_its_cost_and_stops_under_less() {
        let whole = reached_under(&small(), u64::MAX).unwrap();
        assert_eq!((whole.succeeded, whole.stopped), (2, false));
        assert_eq!(reached_under(&small(), whole.work).unwrap(), whole);
        let cut = reached_under(&small(), whole.work.saturating_sub(1)).unwrap();
        assert_eq!((cut.succeeded, cut.stopped), (1, true));
        assert!(cut.work > whole.work.saturating_sub(1), "{cut:?}");
        // With nothing left, the first operation stops, and an empty log replays.
        let none = reached_under(&small(), 0).unwrap();
        assert_eq!((none.succeeded, none.stopped), (0, true));
    }

    #[test]
    fn a_stop_replays_to_where_it_stopped_and_nowhere_else() {
        let done = stopped();
        let stop = done.stop.as_ref().unwrap();
        let log = OpLog::new(Vec::new(), done.log.clone());
        let Replayed::Done { work: before, .. } = replay(&mut Kernel::new(), &log) else {
            panic!("the box replays");
        };
        let got = replay(&mut Kernel::new(), &to_the_stop(&log, stop));
        let case = small();
        assert!(stops_alike(&case, &got, before, stop).is_ok(), "{got:?}");
        let Replayed::Failed {
            hash,
            kind,
            entities,
            work,
        } = got
        else {
            panic!("{got:?}");
        };
        let one = Spend {
            work: 1,
            logical_bytes: 0,
        };
        let law = "a replay of a stopped case stops where it stopped";
        let broken = |got: Replayed, before: Spend| {
            assert_eq!(stops_alike(&case, &got, before, stop).unwrap_err().law, law);
        };
        // A stop one unit later, or after other work before it, is not this stop.
        broken(
            Replayed::Failed {
                hash,
                kind,
                entities: entities.clone(),
                work: work.plus(one),
            },
            before,
        );
        broken(
            Replayed::Failed {
                hash,
                kind,
                entities: entities.clone(),
                work,
            },
            before.plus(one),
        );
        // Another kind of failure, or one naming other entities, is not it either.
        broken(
            Replayed::Failed {
                hash,
                kind: ErrorKind::Internal,
                entities: entities.clone(),
                work,
            },
            before,
        );
        let mut more = entities.clone();
        more.push(NameId::from_bytes([7; 16]));
        broken(
            Replayed::Failed {
                hash,
                kind,
                entities: more,
                work,
            },
            before,
        );
        broken(Replayed::Broken("a panic".to_owned()), before);
        // The log without its stop finishes: it does not stop where the case stopped.
        broken(replay(&mut Kernel::new(), &log), before);
    }

    #[test]
    fn a_stop_is_the_budget_the_operation_was_given() {
        let done = stopped();
        let stop = done.stop.as_ref().unwrap();
        let case = small();
        let given = stop.operation.budget;
        assert!(the_budgets(&case, &stop.error, given).is_ok());
        let law = "a stop is the case budget's";
        let other = Budget::of_work(given.work.saturating_add(1));
        assert_eq!(the_budgets(&case, &stop.error, other).unwrap_err().law, law);
        let OpErrorDetail::Run(RunDetail::BudgetExceeded(x)) = stop.error.detail() else {
            panic!("{:?}", stop.error.detail());
        };
        let bytes = Exceeded {
            kind: BudgetKind::LogicalBytes,
            ..*x
        };
        let path = stop.error.op().clone();
        for e in [
            OpError::new(path.clone(), RunDetail::BudgetExceeded(bytes)),
            OpError::new(path.clone(), RunDetail::BatchTotal(*x)),
            OpError::new(path, RunDetail::Cancelled),
        ] {
            assert_eq!(the_budgets(&case, &e, given).unwrap_err().law, law);
        }
        // An exceeded budget the run reports from elsewhere is not the operation's, even with
        // the same work limit.
        let elsewhere = Exceeded {
            budget: Budget::new(given.work, 0),
            kind: BudgetKind::Work,
            phase: Phase::new("elsewhere"),
            used: Usage::new(2, 0),
        };
        let e = OpError::new(
            stop.error.op().clone(),
            RunDetail::BudgetExceeded(elsewhere),
        );
        assert_eq!(the_budgets(&case, &e, given).unwrap_err().law, law);
    }
}
