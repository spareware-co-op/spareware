//! Replaying a fixture's log and judging it: Pass, Fail or Improved (chapter 28.16; piece G97).
//!
//! What it does: [`replay`] runs a log through an [`Engine`], operation by operation, hashes each
//! operation's canonical output ([`crate::canonical`]) and combines them into the log's hash; the
//! first operation that fails ends the log. [`judge`] compares a [`Replayed`] with a fixture's
//! expectation. [`run`] runs one fixture under every key of an edition, twice on two perturbed
//! heaps, and returns its [`Verdict`].
//!
//! The log's hash is SHA-256 over [`PREFIX`] and each operation's 32-byte canonical output, in
//! order, a failing operation's included. Every operation counts, so an intermediate result that
//! moves shows up even when the last one does not.
//!
//! Guarantees:
//!
//! - **Exact.** A `Pass` fixture must match `expect` exactly: hash and measures, or kind and
//!   entities in order.
//! - **Only what 28.16 allows is `Known`.** A `Known` fixture passes while it gives its expected
//!   typed error, is [`Verdict::Improved`] once it gives its wanted result, and fails on anything
//!   else: another error, another success, a panic. A panic or a log the engine refuses is
//!   [`Replayed::Broken`], which no status accepts.
//! - **Twice, on two heaps.** As every golden does (28.13), each key runs [`REPEATS`] times, each
//!   time from a fresh [`Engine::start`] on a differently perturbed heap, and every run must agree
//!   before the store's value is looked at; a run that differs is [`Failure::Repeat`].
//! - **Nothing is written**: judging never changes a fixture.
//!
//! Method: chapter 28.16's statuses, chapter 28.13's canonical output and perturbations
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: the log's cost twice per key, plus the perturbations.

use core::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use geom_format::operation::Operation;
use geom_format::record::Record;
use geom_op::{Diagnostic, ErrorKind, Exactness, History, NameId, NameSet, OpError};

use super::{Expect, Fixture, Hash, Kind, Measures, Name, OpLog, OpVersions, Spend, Status};
use crate::canonical::{CanonicalError, Outcome, canonical_output};
use crate::golden::Sha256;
use crate::golden::check::{PERTURBATION_SEEDS, REPEATS};
use crate::golden::perturb::Heap;

/// The bytes hashed before a log's outputs: the 18 ASCII bytes `SKG log output v1` and a zero byte.
pub const PREFIX: &[u8; 18] = b"SKG log output v1\0";

/// What one operation gave.
#[derive(Clone, Debug)]
pub struct Step {
    /// Each result body's SHA-256 over its native canonical encoding, in the operation's order.
    pub bodies: Vec<[u8; 32]>,
    /// The new names it made.
    pub names: NameSet,
    /// Its history; `None` for an operation that records none.
    pub history: Option<History>,
    /// Its exactness class and bound.
    pub exactness: Exactness,
    /// Its warnings, in the order it gave them.
    pub diagnostics: Vec<Diagnostic>,
    /// What it charged.
    pub work: Spend,
}

/// How an operation ended.
#[derive(Clone, Debug)]
pub enum Ran {
    /// It succeeded.
    Done(Step),
    /// It failed with a typed error, having charged this much.
    Failed(OpError, Spend),
    /// The engine cannot run it at all: an unknown operation or version, or a broken input. No
    /// operation of the kernel reported this; the log did.
    Refused(String),
}

/// What runs a log: the kernel, or a test's stand-in.
pub trait Engine {
    /// Forgets every earlier run and takes the log's stored inputs.
    ///
    /// # Errors
    ///
    /// Why the inputs cannot be taken; the log is then [`Replayed::Broken`].
    fn start(&mut self, heal: geom_num::Steps, inputs: &[Record]) -> Result<(), String>;

    /// Runs one operation on the bodies so far.
    fn run(&mut self, operation: &Operation) -> Ran;

    /// The measures of the bodies so far, taken once the log has run.
    fn measures(&mut self) -> Measures;
}

/// What a log gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Replayed {
    /// Every operation succeeded.
    Done {
        /// The log's hash.
        hash: Hash,
        /// The measures of the result.
        measures: Measures,
        /// What the log charged.
        work: Spend,
    },
    /// An operation failed with a typed error, and the log stopped there.
    Failed {
        /// The log's hash, the error's output included.
        hash: Hash,
        /// The error's base kind.
        kind: ErrorKind,
        /// The entities it names.
        entities: Vec<NameId>,
        /// What the log charged.
        work: Spend,
    },
    /// A panic, an input the engine refused, or an output that has no encoding.
    Broken(String),
}

impl Replayed {
    /// What a fixture holds of this run; `None` for a broken run, which no fixture may hold.
    #[must_use]
    pub fn expectation(&self) -> Option<Expect> {
        match self {
            Replayed::Done {
                hash,
                measures,
                work,
            } => Some(Expect::Ok {
                hash: Some(*hash),
                work: Some(*work),
                measures: measures.clone(),
            }),
            Replayed::Failed {
                kind,
                entities,
                work,
                ..
            } => Some(Expect::Err {
                kind: Kind(*kind),
                entities: entities.iter().copied().map(Name).collect(),
                work: Some(*work),
            }),
            Replayed::Broken(_) => None,
        }
    }
}

/// The log's hash, from each operation's canonical output.
fn combine(outputs: &[[u8; 32]]) -> Hash {
    let mut sha = Sha256::new();
    sha.write(PREFIX);
    for output in outputs {
        sha.write(output);
    }
    Hash(sha.finish())
}

/// One step's canonical output.
fn output_of(operation: &Operation, step: &Step) -> Result<[u8; 32], CanonicalError> {
    canonical_output(&Outcome::Output {
        bodies: &step.bodies,
        names: &step.names,
        history: step.history.as_ref().map(|h| (&operation.op, h)),
        exactness: &step.exactness,
        diagnostics: &step.diagnostics,
    })
}

/// A panic's message, where it has one.
fn panic_text(payload: &(dyn core::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".to_owned())
}

/// Runs `log` through `engine`, stopping at the first typed error.
pub fn replay(engine: &mut dyn Engine, log: &OpLog) -> Replayed {
    let mut outputs = Vec::with_capacity(log.operations.len());
    let mut work = Spend::default();
    if let Err(why) = engine.start(log.heal, &log.inputs) {
        return Replayed::Broken(format!("the inputs: {why}"));
    }
    for (at, operation) in log.operations.iter().enumerate() {
        let ran = catch_unwind(AssertUnwindSafe(|| engine.run(operation)));
        let ran = match ran {
            Ok(ran) => ran,
            Err(payload) => {
                return Replayed::Broken(format!(
                    "operation {at} ({}) panicked: {}",
                    operation.name,
                    panic_text(payload.as_ref())
                ));
            }
        };
        match ran {
            Ran::Done(step) => match output_of(operation, &step) {
                Ok(output) => {
                    outputs.push(output);
                    work = work.plus(step.work);
                }
                Err(e) => return Replayed::Broken(format!("operation {at}: {e}")),
            },
            Ran::Failed(error, charged) => {
                return match canonical_output(&Outcome::Error(&error)) {
                    Ok(output) => {
                        outputs.push(output);
                        Replayed::Failed {
                            hash: combine(&outputs),
                            kind: error.kind(),
                            entities: error.entities().to_vec(),
                            work: work.plus(charged),
                        }
                    }
                    Err(e) => Replayed::Broken(format!("operation {at}: {e}")),
                };
            }
            Ran::Refused(why) => {
                return Replayed::Broken(format!(
                    "operation {at} ({} version {}): {why}",
                    operation.name, operation.version
                ));
            }
        }
    }
    match catch_unwind(AssertUnwindSafe(|| engine.measures())) {
        Ok(measures) => Replayed::Done {
            hash: combine(&outputs),
            measures,
            work,
        },
        Err(payload) => Replayed::Broken(format!(
            "the measures panicked: {}",
            panic_text(payload.as_ref())
        )),
    }
}

/// The work count `want` states, if it states one, when `got` gave another.
fn moved_work(got: &Replayed, want: &Expect) -> Option<(Spend, Spend)> {
    let (Replayed::Done { work, .. } | Replayed::Failed { work, .. }) = got else {
        return None;
    };
    let (Expect::Ok { work: stated, .. } | Expect::Err { work: stated, .. }) = want;
    stated.filter(|s| s != work).map(|s| (*work, s))
}

/// Whether `got` is what `want` states, the work count aside. A hash left out of `want` is not
/// compared.
#[must_use]
pub fn matches(got: &Replayed, want: &Expect) -> bool {
    match (got, want) {
        (
            Replayed::Done { hash, measures, .. },
            Expect::Ok {
                hash: want_hash,
                measures: want_measures,
                ..
            },
        ) => want_hash.is_none_or(|h| h == *hash) && measures == want_measures,
        (
            Replayed::Failed { kind, entities, .. },
            Expect::Err {
                kind: want_kind,
                entities: want_entities,
                ..
            },
        ) => {
            Kind(*kind) == *want_kind
                && entities.len() == want_entities.len()
                && entities
                    .iter()
                    .zip(want_entities)
                    .all(|(a, b)| Name(*a) == *b)
        }
        _ => false,
    }
}

/// What a run of a fixture says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It matches what its status asks.
    Pass,
    /// A `Known` fixture gave its wanted result: move it to `Pass` (`fixture improve`).
    Improved,
    /// It failed.
    Fail(Failure),
}

/// Why a run of a fixture failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The fixture holds no expectation for the edition.
    NoGolden {
        /// The edition.
        edition: String,
    },
    /// Two runs of one key differ.
    Repeat {
        /// The key's versions.
        operations: OpVersions,
        /// The first run.
        first: Box<Replayed>,
        /// The run that differs.
        other: Box<Replayed>,
    },
    /// The results match, but the work count moved: a change of schedule, which ships only as a
    /// new operation version (23.10, 26.2).
    Work {
        /// The key's versions.
        operations: OpVersions,
        /// What it charged.
        got: Spend,
        /// The golden counts.
        want: Spend,
    },
    /// The run does not match.
    Mismatch {
        /// The key's versions.
        operations: OpVersions,
        /// What it gave.
        got: Box<Replayed>,
        /// What was expected.
        want: Box<Expect>,
    },
}

impl Failure {
    /// The failure's class: what a minimiser keeps constant. Two failures of one class fail the
    /// same way, whatever the digests.
    #[must_use]
    pub fn class(&self) -> String {
        let of = |got: &Replayed| match got {
            Replayed::Done { .. } => "another result".to_owned(),
            Replayed::Failed { kind, .. } => format!("error {}", kind.name()),
            Replayed::Broken(_) => "broken".to_owned(),
        };
        match self {
            Failure::NoGolden { .. } => "no golden".to_owned(),
            Failure::Repeat { .. } => "not repeatable".to_owned(),
            Failure::Work { .. } => "work count".to_owned(),
            Failure::Mismatch { got, .. } => of(got),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NoGolden { edition } => {
                write!(f, "it holds no expectation for edition {edition}")
            }
            Failure::Repeat {
                operations,
                first,
                other,
            } => write!(
                f,
                "two runs at {operations:?} differ: {first:?} and {other:?}"
            ),
            Failure::Work {
                operations,
                got,
                want,
            } => write!(
                f,
                "at {operations:?}: charged {got:?}, the golden is {want:?}, and a count changes \
                 only with a new operation version"
            ),
            Failure::Mismatch {
                operations,
                got,
                want,
            } => write!(f, "at {operations:?}: gave {got:?}, expected {want:?}"),
        }
    }
}

/// Runs `log` [`REPEATS`] times on perturbed heaps; the first run, or how two runs differ.
fn repeated(engine: &mut dyn Engine, log: &OpLog) -> Result<Replayed, Box<(Replayed, Replayed)>> {
    let mut first: Option<Replayed> = None;
    for seed in PERTURBATION_SEEDS.iter().take(REPEATS) {
        let heap = Heap::shift_and_fragment(*seed);
        let run = replay(engine, log);
        drop(heap);
        match &first {
            None => first = Some(run),
            Some(earlier) if *earlier == run => {}
            Some(earlier) => return Err(Box::new((earlier.clone(), run))),
        }
    }
    Ok(first.unwrap_or_else(|| replay(engine, log)))
}

/// Judges one run of one key: the status decides what is accepted.
fn judge(fixture: &Fixture, operations: &OpVersions, want: &Expect, got: Replayed) -> Verdict {
    if let Status::Known { wanted } = &fixture.status
        && matches(&got, wanted)
    {
        return Verdict::Improved;
    }
    if matches(&got, want) {
        return match moved_work(&got, want) {
            None => Verdict::Pass,
            Some((got, want)) => Verdict::Fail(Failure::Work {
                operations: operations.clone(),
                got,
                want,
            }),
        };
    }
    Verdict::Fail(Failure::Mismatch {
        operations: operations.clone(),
        got: Box::new(got),
        want: Box::new(want.clone()),
    })
}

/// Runs `fixture` under every key of `edition` and gives one verdict: the first failure in key
/// order, otherwise Improved if any key improved, otherwise Pass.
pub fn run(fixture: &Fixture, edition: &str, engine: &mut dyn Engine) -> Verdict {
    let mut verdict = None;
    for (operations, want) in fixture.keys_of(edition) {
        let log = fixture.log.at_versions(operations);
        let got = match repeated(engine, &log) {
            Ok(got) => got,
            Err(runs) => {
                let (first, other) = *runs;
                return Verdict::Fail(Failure::Repeat {
                    operations: operations.clone(),
                    first: Box::new(first),
                    other: Box::new(other),
                });
            }
        };
        match judge(fixture, operations, want, got) {
            Verdict::Fail(failure) => return Verdict::Fail(failure),
            Verdict::Improved => verdict = Some(Verdict::Improved),
            Verdict::Pass => verdict = verdict.or(Some(Verdict::Pass)),
        }
    }
    verdict.unwrap_or_else(|| {
        Verdict::Fail(Failure::NoGolden {
            edition: edition.to_owned(),
        })
    })
}
