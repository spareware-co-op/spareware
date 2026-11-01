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
//! - **Not yet in the stack:** a thread-count rerun waits for the first parallel operation, since
//!   none of the operations a log can hold uses the helper, and an imported Tolerant body waits for
//!   the first importer.
//!
//! Method: chapter 28.5's sequence target and oracle stack
//! (`projects/engines/geometry/architecture/28-testing.md`), run through chapter 28.16's replay.
//!
//! Cost: at most [`MAX_OPERATIONS`] operations, each checked in full, then two replays and one
//! replay under a cut budget; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_check::{Level, check as full_check};
use geom_format::content::ContentId;
use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_make::PrismInput;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{ErrorKind, History, NameId, NameSet, OpError, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::fixture::OpLog;
use geom_test::fixture::calls::{Call, Input, Place, Turn};
use geom_test::fixture::kernel::{Kernel, id_of};
use geom_test::fixture::run::{Engine, Ran, Replayed, Step as Done, replay};
use geom_test::history::check_chain;
use geom_test::{ByteSource, Entropy};
use geom_topo::Body;

use crate::Finding;
use crate::adversarial::{NUDGE_BYTES, TWO_AND_A_HALF, beside, nudge, repeats};
use crate::make::{Motion, RotationBy, SHAPE_BYTES, Shape, shape, step as read_step};

/// The most operations a case runs.
pub const MAX_OPERATIONS: u8 = 12;

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
        /// Which of the taken result's points to start on, and how far from it.
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

/// The record of operation number `at`.
fn record(at: usize, call: &Call, input: Option<&ContentId>) -> Option<Operation> {
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
        budget: Budget::UNLIMITED,
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

/// What running a case's operations left: the operations that succeeded, each result and each
/// step's history.
struct Run {
    log: Vec<Operation>,
    results: Vec<Held>,
    steps: Vec<(History, NameSet)>,
}

/// Runs each operation of `case` and holds it to the laws of one step.
#[allow(
    clippy::too_many_lines,
    reason = "one operation's whole round: its call, its run, then each law in turn"
)]
fn run(case: &Case) -> Result<Run, Finding> {
    let mut kernel = Kernel::new();
    kernel
        .start(Steps(0), &[])
        .map_err(|why| Finding::new("a log with no inputs starts", || why))?;
    let mut run = Run {
        log: Vec::new(),
        results: Vec::new(),
        steps: Vec::new(),
    };
    for entry in &case.entries {
        let from = usize::from(entry.on).checked_rem(run.results.len());
        let taken = from.and_then(|i| run.results.get(i));
        let (call, input) = match &entry.item {
            Item::Make { shape, at } => {
                let point = at.and_then(|(which, near)| {
                    let points: Vec<&P3> = taken?.body.arenas().points.iter().collect();
                    let which = usize::from(which).checked_rem(points.len())?;
                    Some(beside(**points.get(which)?, near))
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
        let Some(operation) = record(run.log.len(), &call, input.as_ref().map(|i| &i.0)) else {
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
                    history, exactness, ..
                } = step;
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
            Ran::Failed(e, _) => refused(case, &e)?,
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
    }
    Ok(run)
}

/// Holds the log as a whole to its laws: it reads back, replays alike, and a cut budget stops it.
fn replays(case: &Case, log: Vec<Operation>) -> Result<(), Finding> {
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
    let Replayed::Done { hash, .. } = &first else {
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
    Ok(())
}

/// Holds the case's every outcome to the laws.
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
    let done = run(case).ok()?;
    if done.log.is_empty() {
        return None;
    }
    let log = OpLog::new(Vec::new(), done.log);
    match replay(&mut Kernel::new(), &log) {
        Replayed::Done { hash, measures } => Some(format!("{} {measures:?}", String::from(hash))),
        Replayed::Failed { hash, kind, .. } => {
            Some(format!("{} {}", String::from(hash), kind.name()))
        }
        Replayed::Broken(why) => Some(format!("broken {why}")),
    }
}

/// [`check`], and how many of the case's operations succeeded: what a test uses to show that a
/// generator's cases get past their first step.
///
/// # Errors
///
/// As [`check`].
pub fn reached(case: &Case) -> Result<usize, Finding> {
    let done = run(case)?;
    if let Some(last) = done.results.len().checked_sub(1) {
        let chain = chain_to(&done.results, &done.steps, last);
        check_chain(&chain).map_err(|e| {
            Finding::new("a chain of histories keeps its laws", || {
                format!("{e:?}: {case:?}")
            })
        })?;
    }
    let count = done.log.len();
    replays(case, done.log)?;
    Ok(count)
}
