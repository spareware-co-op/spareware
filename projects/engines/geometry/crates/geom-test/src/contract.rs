//! The contract every operation keeps: chapter 28.2's laws, as a kit each operation is held to
//! (piece G103).
//!
//! What it does: a [`Subject`] is one call of one operation, on inputs fixed by its driver. The
//! laws run it. [`transactional`] injects cancellation and budget exhaustion at each check point.
//! [`errors_tell_the_truth`] holds an operation's schema to what its [`Trigger`]s make it return.
//! [`remedy_holds`] and [`accept_holds`] apply a remedy that gives a number. [`exactness_holds`]
//! and [`no_tolerance_grows`] compare a result with its inputs. [`range_holds`] tests the edge of
//! the frame. [`example_holds`] holds a schema's example to what the example call really gave.
//! `engine-geometry`'s `tests/contract_laws.rs` drives every registered operation through them, so
//! an operation added without a driver fails there.
//!
//! Guarantees:
//!
//! - **Nothing is sampled unless it must be.** Below [`EVERY_POINT_BELOW`] check points every one
//!   is injected. Above it, [`SAMPLE`] are, drawn from the given generator, and always with the
//!   first and last (chapter 28.2), so a failure replays from its printed seed.
//! - **A broken law names itself.** Each law returns a [`Broken`] naming the law, the case and what
//!   happened; none panics on a broken operation.
//! - **Every law can fail.** `tests/contract_kit.rs` breaks each with a seeded operation.
//!
//! Method: chapter 28.2's contract (`projects/engines/geometry/architecture/28-testing.md`), on the
//! budgets and cancellation of chapter 23.10. A work budget is searched for the least that passes,
//! which is the operation's own count, so nothing here needs to be told what it charges.
//!
//! Cost: for [`transactional`], at most `2 × SAMPLE` runs per budget, one per cancellation point up
//! to the same bound, plus a logarithmic search for each budget. Every other law is a few runs.

use core::fmt;
use std::sync::{Mutex, PoisonError};

use geom_num::{Exactness, Steps};
use geom_op::{Bound, Remedy, Value};
use geom_op::{ErrorKind, NameId, OpContext, OpError, OpId, OpPath, OpSchema};
use geom_run::{Budget, Cancel};

use crate::rng::SplitMix64;

/// Check points at or above which [`transactional`] samples (chapter 28.2's "fixtures below a size
/// limit"): a run of this many check points is small enough to try all of them on every pull
/// request.
pub const EVERY_POINT_BELOW: u64 = 1_024;

/// How many check points [`transactional`] tries above [`EVERY_POINT_BELOW`].
pub const SAMPLE: u64 = 256;

/// One of chapter 28.2's laws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Law {
    /// A result passes or an error returns, inputs unchanged; a stop publishes nothing.
    Transactional,
    /// Every error the schema lists is triggered, and none it does not list.
    ErrorsTellTheTruth,
    /// A remedy's number works, and one step beyond it does not.
    RemedyHolds,
    /// A result is no more exact than its least exact input.
    ExactnessIsNotLost,
    /// No tolerance rises.
    NoToleranceGrows,
    /// The frame's edge passes and one step past it is a typed error.
    RangeHolds,
    /// The schema's example gives what it says.
    ExampleRuns,
}

/// A law an operation broke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broken {
    /// The law.
    pub law: Law,
    /// The operation and case.
    pub case: String,
    /// What happened.
    pub why: String,
}

impl fmt::Display for Broken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} broken by {}: {}", self.law, self.case, self.why)
    }
}

impl std::error::Error for Broken {}

fn broken(law: Law, case: &str, why: impl Into<String>) -> Broken {
    Broken {
        law,
        case: case.to_owned(),
        why: why.into(),
    }
}

/// What a successful run gave: `answer` is compared between runs, so it must hold everything that
/// must not change.
#[derive(Clone, Debug, PartialEq)]
pub struct Done<A> {
    /// The result, in a form that compares.
    pub answer: A,
    /// Its exactness.
    pub exactness: Exactness,
}

/// The run of a [`Subject`].
type Run<'a, A> = Box<dyn Fn(&OpContext<'_>) -> Result<Done<A>, OpError> + 'a>;

/// One call of one operation.
pub struct Subject<'a, A> {
    label: String,
    run: Run<'a, A>,
    unchanged: Box<dyn Fn() -> bool + 'a>,
}

impl<'a, A> Subject<'a, A> {
    /// The call `run`, named `label` in anything it breaks.
    pub fn new(
        label: impl Into<String>,
        run: impl Fn(&OpContext<'_>) -> Result<Done<A>, OpError> + 'a,
    ) -> Self {
        Self {
            label: label.into(),
            run: Box::new(run),
            unchanged: Box::new(|| true),
        }
    }

    /// The same call, whose inputs `unchanged` compares with their first state after every run.
    #[must_use]
    pub fn watching(self, unchanged: impl Fn() -> bool + 'a) -> Self {
        Self {
            unchanged: Box::new(unchanged),
            ..self
        }
    }

    /// Its label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Runs it once with no budget or cancellation.
    ///
    /// # Errors
    ///
    /// The operation's own.
    pub fn once(&self) -> Result<Done<A>, OpError> {
        (self.run)(&context(Budget::UNLIMITED))
    }

    /// Runs it under `context`, then holds its inputs to their first state.
    fn try_run(&self, context: &OpContext<'_>) -> Result<Result<Done<A>, OpError>, Broken> {
        let out = (self.run)(context);
        if (self.unchanged)() {
            Ok(out)
        } else {
            Err(broken(
                Law::Transactional,
                &self.label,
                "a run changed its inputs",
            ))
        }
    }
}

/// The context of an injected run.
fn context(budget: Budget) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([0x43; 16])), budget)
}

/// A cancellation asked for at its `at`-th read, for good.
struct CancelAt {
    reads: Mutex<u64>,
    at: u64,
}

impl CancelAt {
    const fn new(at: u64) -> Self {
        Self {
            reads: Mutex::new(0),
            at,
        }
    }

    fn reads(&self) -> u64 {
        *self.reads.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Cancel for CancelAt {
    fn requested(&self) -> bool {
        let mut reads = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        let read = *reads;
        *reads = read.saturating_add(1);
        read >= self.at
    }
}

/// The check points to try out of `total`, in order: all of them below [`EVERY_POINT_BELOW`],
/// otherwise the first, the last and a sample between.
fn points(total: u64, rng: &mut SplitMix64) -> Vec<u64> {
    let span = match core::num::NonZeroU64::new(total) {
        Some(span) if total >= EVERY_POINT_BELOW => span,
        _ => return (0..total).collect(),
    };
    let mut chosen = vec![0, total.saturating_sub(1)];
    chosen.extend((0..SAMPLE).map(|_| rng.below(span)));
    chosen.sort_unstable();
    chosen.dedup();
    chosen
}

/// How much [`transactional`] injected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Injected {
    /// Reads of the cancellation in a whole run.
    pub reads: u64,
    /// The least work budget that passes.
    pub work: u64,
    /// The least logical-byte budget that passes.
    pub bytes: u64,
    /// Whether any of the three was sampled.
    pub sampled: bool,
}

/// The least value of a budget under which `same` holds, given that it holds without one: doubling,
/// then bisection. The sweep in [`stops_below`] then proves it was least everywhere it looks.
fn least(same: &dyn Fn(u64) -> Result<bool, Broken>) -> Result<u64, Broken> {
    if same(0)? {
        return Ok(0);
    }
    let (mut low, mut high) = (0u64, 1u64);
    while !same(high)? {
        low = high;
        high = high.saturating_mul(2);
        if high == u64::MAX {
            break;
        }
    }
    while high.saturating_sub(low) > 1 {
        let middle = low.saturating_add(high.saturating_sub(low) / 2);
        if same(middle)? {
            high = middle;
        } else {
            low = middle;
        }
    }
    Ok(high)
}

/// Holds every budget below `limit` to a clean stop.
fn stops_below<A>(
    subject: &Subject<'_, A>,
    limit: u64,
    rng: &mut SplitMix64,
    budget: &dyn Fn(u64) -> Budget,
) -> Result<(), Broken> {
    for point in points(limit, rng) {
        let c = context(budget(point));
        match subject.try_run(&c)? {
            Err(e) if e.kind() == ErrorKind::BudgetExceeded && e.op() == c.path() => {}
            Err(e) => {
                return Err(broken(
                    Law::Transactional,
                    subject.label(),
                    format!("under {:?}, an error of kind {:?}", budget(point), e.kind()),
                ));
            }
            Ok(_) => {
                return Err(broken(
                    Law::Transactional,
                    subject.label(),
                    format!(
                        "published under {:?}, below what a run needs",
                        budget(point)
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Chapter 28.2's "Transactional": `subject` answers the same each time, and a cancellation at any
/// check point or a budget cut below what it needs stops it with `Cancelled` or `BudgetExceeded`,
/// publishes nothing, and changes no input. A budget it does not need changes nothing.
///
/// # Errors
///
/// The law it breaks. A subject that ends in an error must end in the same error, or a stop.
pub fn transactional<A: PartialEq + fmt::Debug>(
    subject: &Subject<'_, A>,
    rng: &mut SplitMix64,
) -> Result<Injected, Broken> {
    let law = |why: String| broken(Law::Transactional, subject.label(), why);
    let baseline = subject.try_run(&context(Budget::UNLIMITED))?;
    if subject.try_run(&context(Budget::UNLIMITED))? != baseline {
        return Err(law("two runs answered differently".to_owned()));
    }

    let counter = CancelAt::new(u64::MAX);
    let _ = subject.try_run(&context(Budget::UNLIMITED).with_cancel(&counter))?;
    let reads = counter.reads();
    for at in points(reads, rng) {
        let cancel = CancelAt::new(at);
        let c = context(Budget::UNLIMITED).with_cancel(&cancel);
        match subject.try_run(&c)? {
            Err(e) if e.kind() == ErrorKind::Cancelled && e.op() == c.path() => {}
            other if baseline.is_err() && other == baseline => {}
            Ok(_) => return Err(law(format!("published past a cancellation at read {at}"))),
            Err(e) => return Err(law(format!("cancelled at read {at}, but {:?}", e.kind()))),
        }
    }

    let same = |budget: Budget| -> Result<bool, Broken> {
        Ok(subject.try_run(&context(budget))? == baseline)
    };
    let by_work = |work: u64| Budget::new(work, u64::MAX);
    let by_bytes = |bytes: u64| Budget::new(u64::MAX, bytes);
    let work = least(&|w| same(by_work(w)))?;
    let bytes = least(&|b| same(by_bytes(b)))?;
    stops_below(subject, work, rng, &by_work)?;
    stops_below(subject, bytes, rng, &by_bytes)?;
    for spare in [1, 2, work.max(1)] {
        if !same(by_work(work.saturating_add(spare)))? {
            return Err(law(format!(
                "{spare} spare units of work changed the answer"
            )));
        }
    }
    Ok(Injected {
        reads,
        work,
        bytes,
        sampled: [reads, work, bytes].iter().any(|&n| n >= EVERY_POINT_BELOW),
    })
}

/// A call that should fail with the error `detail`.
pub struct Trigger<'a, A> {
    /// The error it should give, as the schema names it.
    pub detail: &'static str,
    /// The call.
    pub subject: Subject<'a, A>,
}

/// The name a schema gives an error: the variant inside its detail, read from its `Debug` text,
/// which for a detail is the variant's own name (`Make(ZeroLength { input: "size.y" })`).
#[must_use]
pub fn detail_name(error: &OpError) -> String {
    let text = format!("{:?}", error.detail());
    let inner = text.split_once('(').map_or(text.as_str(), |(_, rest)| rest);
    inner
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect()
}

/// Chapter 28.2's "Errors tell the truth": every error `schema` lists is given by a trigger, with
/// the base kind the schema states, the same way each time, and no trigger gives one it does not
/// list. Two kinds need no trigger: `Internal`, which only a defect can give, and which G110's
/// fault injection reaches, and `BudgetExceeded`, which [`transactional`] injects.
///
/// # Errors
///
/// The law it breaks.
pub fn errors_tell_the_truth<A: PartialEq>(
    schema: &OpSchema,
    triggers: &[Trigger<'_, A>],
) -> Result<(), Broken> {
    for spec in schema.errors {
        let injected = matches!(spec.kind, ErrorKind::Internal | ErrorKind::BudgetExceeded);
        if !injected && !triggers.iter().any(|t| t.detail == spec.detail) {
            return Err(broken(
                Law::ErrorsTellTheTruth,
                schema.name,
                format!("{} is listed, and nothing triggers it", spec.detail),
            ));
        }
    }
    for trigger in triggers {
        let law = |why: String| broken(Law::ErrorsTellTheTruth, trigger.subject.label(), why);
        let c = context(Budget::UNLIMITED);
        let Err(e) = trigger.subject.try_run(&c)? else {
            return Err(law(format!(
                "succeeded, but should give {}",
                trigger.detail
            )));
        };
        let name = detail_name(&e);
        let Some(spec) = schema.errors.iter().find(|s| s.detail == name) else {
            return Err(law(format!("gave {name}, which the schema does not list")));
        };
        if name != trigger.detail {
            return Err(law(format!("gave {name}, not {}", trigger.detail)));
        }
        if e.kind() != spec.kind {
            return Err(law(format!(
                "gave kind {:?}, and the schema says {:?}",
                e.kind(),
                spec.kind
            )));
        }
        if trigger.subject.try_run(&c)?.err().as_ref() != Some(&e) {
            return Err(law("a second run gave another error".to_owned()));
        }
    }
    Ok(())
}

/// The number in a remedy's value.
fn number(value: &Value) -> Option<u64> {
    match value {
        Value::Count(n) => u64::try_from(*n).ok(),
        Value::Length { steps, .. } => Some(steps.0),
        _ => None,
    }
}

/// Chapter 28.2's remedy: the call `at(failing)` fails and offers to change `input`; at the number
/// it gives the call succeeds, and one step beyond the number it fails. Returns the number.
///
/// # Errors
///
/// The law it breaks.
pub fn remedy_holds<'a, A>(
    input: &'static str,
    failing: u64,
    at: &dyn Fn(u64) -> Subject<'a, A>,
) -> Result<u64, Broken> {
    let start = at(failing);
    let label = start.label().to_owned();
    let law = |why: String| broken(Law::RemedyHolds, &label, why);
    let runs = |n: u64| at(n).try_run(&context(Budget::UNLIMITED));
    let Err(e) = runs(failing)? else {
        return Err(law(format!("{input} = {failing} succeeded")));
    };
    let offered = e.remedies().iter().find_map(|r| match r {
        Remedy::ChangeInput {
            input: named,
            to: Some((bound, value)),
        } if *named == input => Some((*bound, number(value))),
        _ => None,
    });
    let Some((bound, Some(value))) = offered else {
        return Err(law(format!("no remedy gives a number for {input}")));
    };
    if runs(value)?.is_err() {
        return Err(law(format!("{input} = {value}, the remedy's value, fails")));
    }
    let beyond: &[Option<u64>] = match bound {
        Bound::AtMost => &[value.checked_add(1)],
        Bound::AtLeast => &[value.checked_sub(1)],
        Bound::Exactly => &[value.checked_add(1), value.checked_sub(1)],
    };
    for n in beyond.iter().flatten() {
        if runs(*n)?.is_ok() {
            return Err(law(format!(
                "{input} = {n}, one step past {value}, succeeds"
            )));
        }
    }
    Ok(value)
}

/// One step tighter than `class`, if it has a step to lose.
fn tighter(class: Exactness) -> Option<Exactness> {
    match class {
        Exactness::Exact => None,
        Exactness::Certified { bound, step } => {
            bound.0.checked_sub(1).map(|n| Exactness::Certified {
                bound: Steps(n),
                step,
            })
        }
        Exactness::Approximate { deviation, step } => {
            deviation.0.checked_sub(1).map(|n| Exactness::Approximate {
                deviation: Steps(n),
                step,
            })
        }
    }
}

/// Chapter 28.2's `ExactnessNotAccepted`: `at(None)` refuses and offers a class; `at` that class
/// succeeds, at a result no less exact than it, and one step tighter refuses again. Returns the
/// class.
///
/// # Errors
///
/// The law it breaks.
pub fn accept_holds<'a, A>(
    at: &dyn Fn(Option<Exactness>) -> Subject<'a, A>,
) -> Result<Exactness, Broken> {
    let start = at(None);
    let label = start.label().to_owned();
    let law = |why: String| broken(Law::RemedyHolds, &label, why);
    let runs = |class| at(class).try_run(&context(Budget::UNLIMITED));
    let Err(e) = runs(None)? else {
        return Err(law("succeeded with nothing accepted".to_owned()));
    };
    if e.kind() != ErrorKind::ExactnessNotAccepted {
        return Err(law(format!("refused with kind {:?}", e.kind())));
    }
    let offered = e.remedies().iter().find_map(|r| match r {
        Remedy::Accept { class } => Some(*class),
        _ => None,
    });
    let Some(class) = offered else {
        return Err(law("no remedy offers a class".to_owned()));
    };
    match runs(Some(class))? {
        Ok(done) if done.exactness.rank() <= class.rank() => {}
        Ok(done) => {
            return Err(law(format!(
                "accepted {class:?}, gave {:?}",
                done.exactness
            )));
        }
        Err(e) => return Err(law(format!("refused the class it offered: {:?}", e.kind()))),
    }
    if let Some(less) = tighter(class) {
        match runs(Some(less))? {
            Err(e) if e.kind() == ErrorKind::ExactnessNotAccepted => {}
            _ => {
                return Err(law(format!(
                    "{less:?}, tighter than it offered, was not refused"
                )));
            }
        }
    }
    Ok(class)
}

/// Chapter 28.2's "Exactness is never lost unasked": `result` is no more exact than its least exact
/// input, and its bound is at least the sum of theirs.
///
/// # Errors
///
/// The law it breaks.
pub fn exactness_holds(case: &str, inputs: &[Exactness], result: Exactness) -> Result<(), Broken> {
    let law = |why: String| broken(Law::ExactnessIsNotLost, case, why);
    let mut floor = Exactness::Exact;
    for input in inputs {
        floor = floor
            .combine(*input)
            .ok_or_else(|| law(format!("{floor:?} and {input:?} have no sum")))?;
    }
    // A result that kept the sum changes nothing when the sum is added to it in the finer step,
    // only if it was already at least as loose; so compare the two in the finer step.
    let joined = floor
        .combine(result)
        .ok_or_else(|| law(format!("{floor:?} and {result:?} have no sum")))?;
    if joined.rank() != result.rank() {
        return Err(law(format!(
            "{result:?} is more exact than the inputs' {floor:?}"
        )));
    }
    if let (Some((want, want_step)), Some((have, have_step))) = (floor.bound(), result.bound()) {
        let finer = if want_step.rung() >= have_step.rung() {
            want_step
        } else {
            have_step
        };
        match (want.refine(want_step, finer), have.refine(have_step, finer)) {
            (Some(want), Some(have)) if have >= want => {}
            _ => {
                return Err(law(format!(
                    "{result:?}'s bound is below the inputs' {floor:?}"
                )));
            }
        }
    }
    Ok(())
}

/// Chapter 28.2's "No tolerance grows": no entity of `after` has a tolerance above its own in
/// `before`. An entity new in `after` has none to compare with.
///
/// # Errors
///
/// The law it breaks.
pub fn no_tolerance_grows(
    case: &str,
    before: &[(NameId, Steps)],
    after: &[(NameId, Steps)],
) -> Result<(), Broken> {
    for (name, now) in after {
        if let Some((_, was)) = before.iter().find(|(n, _)| n == name) {
            if now.0 > was.0 {
                return Err(broken(
                    Law::NoToleranceGrows,
                    case,
                    format!("{name:?} rose from {} to {} steps", was.0, now.0),
                ));
            }
        }
    }
    Ok(())
}

/// Chapter 28.2's "The range holds": `inside`, whose result reaches 2^40 − 1 steps, succeeds, and
/// `outside`, whose result would reach 2^40, is a typed `FrameRangeExceeded`.
///
/// # Errors
///
/// The law it breaks.
pub fn range_holds<A>(inside: &Subject<'_, A>, outside: &Subject<'_, A>) -> Result<(), Broken> {
    let c = context(Budget::UNLIMITED);
    if let Err(e) = inside.try_run(&c)? {
        return Err(broken(
            Law::RangeHolds,
            inside.label(),
            format!("refused inside the frame: {:?}", e.kind()),
        ));
    }
    match outside.try_run(&c)? {
        Err(e) if e.kind() == ErrorKind::FrameRangeExceeded => Ok(()),
        Err(e) => Err(broken(
            Law::RangeHolds,
            outside.label(),
            format!("past the frame, kind {:?}", e.kind()),
        )),
        Ok(_) => Err(broken(
            Law::RangeHolds,
            outside.label(),
            "a result past the frame was published",
        )),
    }
}

/// The class word an example ends with.
fn class_word(class: Exactness) -> String {
    match class {
        Exactness::Exact => "Exact.".to_owned(),
        Exactness::Certified { bound, .. } => format!(
            "Certified within {} step{}.",
            bound.0,
            if bound.0 == 1 { "" } else { "s" }
        ),
        Exactness::Approximate { deviation, .. } => format!(
            "Approximate within {} step{}.",
            deviation.0,
            if deviation.0 == 1 { "" } else { "s" }
        ),
    }
}

/// Chapter 28.2's "The schema's example runs": the example's text after `gives` ends with the class
/// `class`, and every count it states of a thing in `counts`, such as `("face", 6)`, is the one the
/// call gave. A thing the example does not mention is not held to it.
///
/// # Errors
///
/// The law it breaks.
pub fn example_holds(
    schema: &OpSchema,
    counts: &[(&str, u64)],
    class: Exactness,
) -> Result<(), Broken> {
    let law = |why: String| broken(Law::ExampleRuns, schema.name, why);
    let Some((_, gives)) = schema.example.split_once(" gives ") else {
        return Err(law("the example says nothing it gives".to_owned()));
    };
    let gives = gives.split_whitespace().collect::<Vec<_>>().join(" ");
    if !gives.ends_with(&class_word(class)) {
        return Err(law(format!(
            "it gave {class:?}, and the example says: {gives}"
        )));
    }
    let words: Vec<&str> = gives.split(' ').collect();
    for pair in words.windows(2) {
        let [first, thing] = pair else { continue };
        let Ok(n) = first.parse::<u64>() else {
            continue;
        };
        for (name, count) in counts {
            let plural = name.strip_suffix("ex").map(|stem| format!("{stem}ices"));
            let same = thing.starts_with(name)
                || plural
                    .as_ref()
                    .is_some_and(|p| thing.starts_with(p.as_str()));
            if same && n != *count {
                return Err(law(format!(
                    "it gave {count} {name}, and the example says {n}"
                )));
            }
        }
    }
    Ok(())
}
