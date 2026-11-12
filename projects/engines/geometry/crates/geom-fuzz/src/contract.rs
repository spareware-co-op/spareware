//! Fuzz target: the operation contract (piece G82). On any operation path, any error detail of any
//! family with any entities, evidence, remedies and message added, any warnings, any context and
//! any choice of the facade's operations, an error's kind is its detail's, it carries the data and
//! the remedies chapter 22's table gives its kind, every value and remedy writes text that names
//! what it holds, every schema the facade lists names details the contract has, with their kinds,
//! and its tool description parses and round-trips, and the same input gives the same verdict
//! twice.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two operation paths (an
//! operation's ID and up to [`MAX_LABELS`] labels each, step names drawn as any text, often
//! printable, or batch keys); up to [`MAX_ERRORS`] errors, each a detail of any family (the
//! parallel helper's stops, the numbers layer's failures, a batch's, the Make and the Query
//! families') at one of the paths, with up to [`MAX_ITEMS`] entities, evidence values and remedies
//! of every kind added and its message kept or replaced; up to [`MAX_DIAGNOSTICS`] warnings, with
//! codes valid and not; an exactness class; a context's budget, precision, cancellation and
//! tally; a choice of the facade's operations; and the bytes every detail of the contract is built
//! from once more. [`check`] holds each to the laws below, and [`harness`] decodes and checks
//! twice.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; [`decode`] reads at most
//!   [`MAX_INPUT_BYTES`] bytes, derived below, and a unit test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case. The
//!   laws, chapter 22's and the module headers' of `geom-op`'s G82 files:
//!   - *the kinds*: [`ErrorKind::ALL`] holds the thirteen kinds once each, in chapter 22's order,
//!     each named by its variant, and only `Cancelled` and `OutOfMemory` are not replayable;
//!   - *the error*: an error's kind is its detail's, whatever is added; it starts with the detail's
//!     message and remedies, keeps entities, evidence and remedies in the order given, and writes
//!     `kind at path: message`; the detail is its source;
//!   - *evidence and remedies as chapter 22 says*: every remedy a detail knows is of a form its
//!     kind's row allows ("what the caller can do"); a budget failure offers to raise exactly the
//!     limit it passed to what was used, to split the call and to preview; a defect offers a report;
//!     `Cancelled` and `OutOfMemory` offer nothing; a class not accepted offers to accept the class
//!     it carries; and the detail's message names its data (the limit, the used amount and the
//!     phase; the coordinate; the key; the input, count, class or piece);
//!   - *the details map by meaning*: every `NumError` lands on the kind chapter 22 gives it, and
//!     every conversion from the helper's errors keeps the stop it is;
//!   - *the schemas*: every error a schema of the facade's `OPERATIONS` lists is a detail the
//!     contract builds, of the kind the schema states, that offers at least one remedy; no schema
//!     lists `Cancelled` or `OutOfMemory`, or a detail twice; every warning code a schema lists is
//!     one a [`Diagnostic`] takes; and the MCP tool descriptions of any choice of operations parse
//!     as JSON, write and parse back to the same value, and give back each operation's name,
//!     version, errors with their kinds (each kind's name read back to the kind), warnings,
//!     classes, batch entry and inputs;
//!   - *text*: every value's text reads back to the value (integers, points, the length and its
//!     step, the interval's bounds bit for bit), evidence writes its name before its value, every
//!     remedy names its input, value, candidates in order, route, class, step, budget or piece,
//!     and two remedies write the same text only if they are equal;
//!   - *identity*: a step label is taken exactly when it is 1 to 64 bytes of printable ASCII not
//!     starting or ending with a space; a path keeps its labels in order, writes its ID's hex and
//!     its labels, contains exactly the paths that start with it, orders as its bytes and labels
//!     do, and its canonical encoding reads back to it; IDs and names write their sixteen bytes as
//!     hex;
//!   - *the context*: it holds its path, budget, precision (final unless a preview is asked),
//!     cancellation and tally, and its `Debug` shows whether it can be cancelled but never the
//!     cancellation's state;
//!   - *the output*: a warning is made exactly for a valid code, keeps its entities and evidence in
//!     order, and an output keeps its result, class and warnings in order; [`Class::of`] reads the
//!     class of any exactness;
//!   - *determinism*: the errors built twice from the case are equal and write the same text, and
//!     [`harness`] gives the same verdict twice.
//!
//! Method: chapter 22's `OpError`, its table of base kinds with "what the caller can do" as the
//! remedies' oracle, its mapping of the numbers layer's errors, and its rule that every operation
//! is described as data from which the MCP tools are generated
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`), chapter 20.1's identities
//! and section 32.19's canonical path, driven by coverage-guided fuzzing (28.5). The tool
//! descriptions are written by `geom-gen`'s facade generator and read with `serde_json`; the
//! canonical path is read with `ciborium-ll`, an encoder written elsewhere.
//!
//! Cost: about 40 details built and checked per input, the facade's schemas walked once and the
//! chosen operations' tool descriptions written, parsed and written again, all twice; at most
//! [`MAX_INPUT_BYTES`] bytes read.

use std::collections::BTreeSet;

use ciborium_io::Read as _;
use ciborium_ll::{Decoder, Header};
use engine_geometry::operations::OPERATIONS;
use engine_geometry::schema::Operation;
use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, NumError, P2, P3, Steps};
use geom_op::cbor::Encoder;
use geom_op::{
    BatchDetail, Bound, Class, Diagnostic, ErrorKind, Evidence, Label, MakeDetail, NameId,
    NumDetail, OpContext, OpError, OpErrorDetail, OpId, OpOutput, OpPath, Precision, QueryDetail,
    Remedy, RunDetail, Tally, Value,
};
use geom_run::{
    Budget, BudgetKind, Cancel as _, CancelToken, Exceeded, Interrupted, ItemError, Phase,
    RunError, Usage,
};
use geom_test::contract::detail_name;
use geom_test::{ByteSource, Entropy};
use serde_json::Value as Json;

use crate::Finding;
use crate::cbor::hex;

/// The most labels a drawn path has.
pub const MAX_LABELS: usize = 4;
/// The most errors a case builds.
pub const MAX_ERRORS: usize = 2;
/// The most entities, evidence values and remedies added to one error.
pub const MAX_ITEMS: usize = 3;
/// The most warnings a case builds.
pub const MAX_DIAGNOSTICS: usize = 2;
/// The most evidence values added to one warning.
pub const MAX_WARNING_EVIDENCE: usize = 2;

/// The bytes a value is drawn from.
const PAYLOAD: usize = 8;
/// A label: its tag, then a step's length and four pattern bytes (a key reads two).
const LABEL_BYTES: usize = 6;
/// A path: its ID's two bytes, the label count and the labels.
const PATH_BYTES: usize = 2 + 1 + MAX_LABELS * LABEL_BYTES;
/// An entity's name: two bytes.
const NAME_BYTES: usize = 2;
/// An evidence value: its name, its kind and a payload.
const EVIDENCE_BYTES: usize = 1 + 1 + PAYLOAD;
/// A remedy: its kind, a selector and a payload.
const REMEDY_BYTES: usize = 1 + 1 + PAYLOAD;
/// One error: its detail (family, variant, payload), its entities, evidence and remedies with
/// their counts, and its message (a selector and four bytes of text).
const ERROR_BYTES: usize = (2 + PAYLOAD)
    + (1 + MAX_ITEMS * NAME_BYTES)
    + (1 + MAX_ITEMS * EVIDENCE_BYTES)
    + (1 + MAX_ITEMS * REMEDY_BYTES)
    + (1 + 4);
/// One warning: its code, its entities and evidence with their counts, and its message.
const WARNING_BYTES: usize =
    1 + (1 + MAX_ITEMS * NAME_BYTES) + (1 + MAX_WARNING_EVIDENCE * EVIDENCE_BYTES) + 1;
/// The context: two limits of two bytes, a preview's bound and step, and the flags.
const CONTEXT_BYTES: usize = 4 + 3 + 1;

/// The most bytes [`decode`] reads: two paths (27 each), the error count and two errors (84
/// each), the warning count and two warnings (30 each), the output's class (8), the context (8),
/// the operations chosen (4) and the catalogue's payload (8): 54 + 169 + 61 + 8 + 8 + 4 + 8 = 312.
/// The test `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = 2 * PATH_BYTES
    + (1 + MAX_ERRORS * ERROR_BYTES)
    + (1 + MAX_DIAGNOSTICS * WARNING_BYTES)
    + PAYLOAD
    + CONTEXT_BYTES
    + 4
    + PAYLOAD;

const _: () = assert!(MAX_INPUT_BYTES == 312);

/// Input names a detail or a remedy names.
const INPUTS: [&str; 8] = [
    "radius",
    "size.y",
    "base",
    "minor",
    "body",
    "instances",
    "keys",
    "",
];
/// Pieces a remedy waits for.
const PIECES: [&str; 3] = ["G177", "G227", "G459"];
/// Routes a remedy takes.
const ROUTES: [&str; 2] = ["mesh.boolean", "combine.subtract"];
/// What evidence values are.
const WHATS: [&str; 4] = ["largest radius", "station", "gap", ""];
/// What broke in the helper.
const BROKE: [&str; 2] = ["a chunk filled twice", "a slot left empty"];
/// Phases a budget failure names.
const PHASES: [Phase; 2] = [Phase::new("split faces"), Phase::new("check")];
/// Messages an operation gives in place of its detail's.
const MESSAGES: [&str; 3] = ["the radius is too large", "", "ünïcode words"];
/// The longest valid warning code, and one byte past it.
const LONGEST: &str = "abcdefghijklmnopqrstuvwxyz0123456789._abcdefghijklmnopqrstuvwxyz";
const TOO_LONG: &str = "abcdefghijklmnopqrstuvwxyz0123456789._abcdefghijklmnopqrstuvwxyz0";
const _: () = assert!(LONGEST.len() == 64 && TOO_LONG.len() == 65);
/// Warning codes, valid and not.
const CODES: [&str; 11] = [
    "bool.sliver",
    "sweep.short_edge",
    "a",
    "0.9_z",
    LONGEST,
    "",
    "Bool",
    "a-b",
    "x y",
    "é",
    TOO_LONG,
];
/// The grid steps.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];
/// The bounds a remedy's value takes.
const BOUNDS: [Bound; 3] = [Bound::AtMost, Bound::AtLeast, Bound::Exactly];
/// The largest coordinate a frame holds, 2^40 - 1 steps.
const COORD_MAX: i64 = 1_099_511_627_775;

/// A label as drawn: a step's name, which the contract may refuse, or a batch key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelDraw {
    /// A step named so.
    Step(String),
    /// A batch item keyed so.
    Key([u8; 16]),
}

/// A path as drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathDraw {
    /// The operation.
    pub op: OpId,
    /// Its labels, outermost first.
    pub labels: Vec<LabelDraw>,
}

/// One error as drawn: its detail, the path it is at, and what the operation adds.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorDraw {
    /// The detail.
    pub detail: OpErrorDetail,
    /// Which of the two paths it is at.
    pub on: usize,
    /// The entities added, in two calls split at the middle.
    pub entities: Vec<NameId>,
    /// The evidence added.
    pub evidence: Vec<Evidence>,
    /// The remedies added.
    pub remedies: Vec<Remedy>,
    /// The message given in place of the detail's, if any.
    pub message: Option<String>,
}

/// One warning as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct WarningDraw {
    /// Its code, valid or not.
    pub code: &'static str,
    /// The entities it concerns.
    pub entities: Vec<NameId>,
    /// The values that explain it.
    pub evidence: Vec<Evidence>,
    /// Its words.
    pub message: &'static str,
}

/// The context as drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextDraw {
    /// The work limit.
    pub work: u64,
    /// The logical-memory limit.
    pub bytes: u64,
    /// A preview's bound and step, if one is asked for.
    pub preview: Option<(Steps, GridStep)>,
    /// Whether a cancellation is attached.
    pub cancel: bool,
    /// Whether it has been requested.
    pub cancelled: bool,
    /// Whether a tally is attached.
    pub tally: bool,
}

/// One case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The two paths.
    pub paths: [PathDraw; 2],
    /// The errors.
    pub errors: Vec<ErrorDraw>,
    /// The warnings.
    pub warnings: Vec<WarningDraw>,
    /// The output's class.
    pub exactness: Exactness,
    /// The context.
    pub context: ContextDraw,
    /// Which of the facade's operations are described, bit `i % 24` for the `i`-th after the turn.
    pub chosen: u32,
    /// Where the choice starts in the list.
    pub turn: u8,
    /// The bytes every detail of the contract is built from once more.
    pub payload: [u8; PAYLOAD],
}

/// `pool`'s entry at `b`, counted round the pool.
fn pick<T: Copy>(pool: &[T], fallback: T, b: u8) -> T {
    usize::from(b)
        .checked_rem(pool.len())
        .and_then(|i| pool.get(i))
        .copied()
        .unwrap_or(fallback)
}

/// A count from 0 to `max` from one byte.
fn count(s: &mut ByteSource<'_>, max: usize) -> usize {
    usize::from(s.byte())
        .checked_rem(max.saturating_add(1))
        .unwrap_or(0)
}

/// Eight bytes.
fn payload(s: &mut ByteSource<'_>) -> [u8; PAYLOAD] {
    core::array::from_fn(|_| s.byte())
}

/// Sixteen bytes from two, the first eight from `a` and the last from `b`.
fn sixteen(a: u8, b: u8) -> [u8; 16] {
    core::array::from_fn(|i| if i < 8 { a } else { b })
}

/// A name from two bytes.
fn name(s: &mut ByteSource<'_>) -> NameId {
    NameId::from_bytes(sixteen(s.byte(), s.byte()))
}

/// Two bytes of `p` from `at`, little-endian.
fn pair(p: [u8; PAYLOAD], at: usize) -> u16 {
    let byte = |i: usize| p.get(i).copied().unwrap_or(0);
    u16::from_le_bytes([byte(at), byte(at.saturating_add(1))])
}

/// Four bytes of `p` from `at`, little-endian.
fn quad(p: [u8; PAYLOAD], at: usize) -> u32 {
    let byte = |i: usize| p.get(i).copied().unwrap_or(0);
    u32::from_le_bytes([
        byte(at),
        byte(at.saturating_add(1)),
        byte(at.saturating_add(2)),
        byte(at.saturating_add(3)),
    ])
}

/// Byte `at` of `p`.
fn at(p: [u8; PAYLOAD], at: usize) -> u8 {
    p.get(at).copied().unwrap_or(0)
}

/// An exactness class and its bound from `p`.
fn exactness(p: [u8; PAYLOAD]) -> Exactness {
    let (bound, step) = (Steps(u64::from(quad(p, 1))), pick(&STEPS, GridStep::Q0, at(p, 5)));
    match at(p, 0) % 3 {
        0 => Exactness::Exact,
        1 => Exactness::Certified { bound, step },
        _ => Exactness::Approximate {
            deviation: bound,
            step,
        },
    }
}

/// A double from four bytes as a single's bits: every single is a double exactly; a NaN or an
/// infinity becomes zero.
fn double(bits: u32) -> f64 {
    let x = f64::from(f32::from_bits(bits));
    if x.is_finite() { x } else { 0.0 }
}

/// A value of the kind `kind` picks, from `p`.
fn value(kind: u8, p: [u8; PAYLOAD]) -> Value {
    let wide = u64::from_le_bytes(p);
    let fallback = Value::Count(u128::from(wide));
    match kind % 7 {
        0 => P2::new(
            i64::from(i32::from_le_bytes(quad(p, 0).to_le_bytes())),
            i64::from(i32::from_le_bytes(quad(p, 4).to_le_bytes())),
        )
        .map_or(fallback, Value::Point2),
        1 => {
            let x = if at(p, 6) & 1 == 1 {
                COORD_MAX
            } else {
                i64::from(i16::from_le_bytes(pair(p, 0).to_le_bytes()))
            };
            P3::new(
                x,
                i64::from(i16::from_le_bytes(pair(p, 2).to_le_bytes())),
                i64::from(i16::from_le_bytes(pair(p, 4).to_le_bytes())).saturating_neg(),
            )
            .map_or(fallback, Value::Point3)
        }
        2 => Value::Coordinate(coordinate(p)),
        3 => Value::Length {
            steps: Steps(wide),
            step: pick(&STEPS, GridStep::Q0, at(p, 0)),
        },
        4 => {
            let (a, b) = (double(quad(p, 0)), double(quad(p, 4)));
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            Interval::new(lo, hi).map_or(fallback, Value::Parameter)
        }
        5 => Value::Exactness(exactness(p)),
        _ => Value::Count(if at(p, 7) == 0xFF {
            u128::MAX
        } else {
            u128::from(wide)
        }),
    }
}

/// A coordinate that need not fit a frame, from `p`: the ends of `i128` when the last byte says so.
fn coordinate(p: [u8; PAYLOAD]) -> i128 {
    match at(p, 7) {
        0xFF => i128::MAX,
        0xFE => i128::MIN,
        shift => i128::from(i64::from_le_bytes(p)).wrapping_shl(u32::from(shift % 64)),
    }
}

/// A budget failure from `p`.
fn exceeded(p: [u8; PAYLOAD]) -> Exceeded {
    Exceeded {
        budget: Budget::new(u64::from(pair(p, 0)), u64::from(pair(p, 2))),
        kind: pick(&BudgetKind::ALL, BudgetKind::Work, at(p, 4)),
        phase: pick(&PHASES, Phase::new("check"), at(p, 5)),
        used: Usage::new(u128::from(pair(p, 5)), u128::from(pair(p, 6))),
    }
}

/// The numbers layer's error `variant` picks, from `p`.
fn num_error(variant: u8, p: [u8; PAYLOAD]) -> NumError {
    match variant % 11 {
        0 => NumError::OutOfRange {
            value: coordinate(p),
        },
        1 => NumError::Degenerate,
        2 => NumError::NotFinite,
        3 => NumError::WrongDirection,
        4 => NumError::DivisorContainsZero,
        5 => NumError::NegativeSqrt,
        6 => NumError::EmptyInterval,
        7 => NumError::OutsideDomain,
        8 => NumError::BudgetExceeded,
        9 => NumError::ToleranceExceeded,
        _ => NumError::UnprovenMaths,
    }
}

/// The helper's stop `variant` picks, from `p`.
fn run_detail(variant: u8, p: [u8; PAYLOAD]) -> RunDetail {
    match variant % 5 {
        0 => RunDetail::BudgetExceeded(exceeded(p)),
        1 => RunDetail::BatchTotal(exceeded(p)),
        2 => RunDetail::Cancelled,
        3 => RunDetail::OutOfMemory,
        _ => RunDetail::Internal {
            what: pick(&BROKE, "", at(p, 0)),
        },
    }
}

/// The Make family's detail `variant` picks, from `p`.
fn make_detail(variant: u8, p: [u8; PAYLOAD]) -> MakeDetail {
    let wide = u64::from_le_bytes(p);
    match variant % 14 {
        0 => MakeDetail::ZeroLength {
            input: pick(&INPUTS, "", at(p, 0)),
        },
        1 => MakeDetail::TooFewCorners { corners: wide },
        2 => MakeDetail::BaseNotLevel,
        3 => MakeDetail::BaseNotConvex,
        4 => MakeDetail::MinorNotBelowMajor { major: wide },
        5 => MakeDetail::NotAccepted {
            would: exactness(p),
        },
        6 => MakeDetail::KindNotCertified,
        7 => MakeDetail::NotCertifiedYet {
            piece: pick(&PIECES, "", at(p, 0)),
        },
        8 => MakeDetail::BodyNotNamed,
        9 => MakeDetail::EmptyPattern,
        10 => MakeDetail::RepeatedKey,
        11 => MakeDetail::SingularTransform,
        12 => MakeDetail::ResultInvalid,
        _ => MakeDetail::BuildFailed,
    }
}

/// The Query family's detail `variant` picks, from `p`.
fn query_detail(variant: u8, p: [u8; PAYLOAD]) -> QueryDetail {
    let row = quad(p, 0);
    match variant % 6 {
        0 => QueryDetail::Malformed,
        1 => QueryDetail::CurvedEdge { edge: row },
        2 => QueryDetail::CurvedFace { face: row },
        3 => QueryDetail::NoLoops { face: row },
        4 => QueryDetail::NotSolid,
        _ => QueryDetail::Overflow,
    }
}

/// The variants of each family, in [`detail`]'s order of families.
const FAMILY_SIZES: [u8; 5] = [5, 11, 1, 14, 6];

/// The detail of family `family` and variant `variant`, from `p`.
fn detail(family: u8, variant: u8, p: [u8; PAYLOAD]) -> OpErrorDetail {
    match family % 5 {
        0 => OpErrorDetail::Run(run_detail(variant, p)),
        1 => OpErrorDetail::Num(NumDetail::from(num_error(variant, p))),
        2 => OpErrorDetail::Batch(BatchDetail::RepeatedKey {
            key: sixteen(at(p, 0), at(p, 1)),
        }),
        3 => OpErrorDetail::Make(make_detail(variant, p)),
        _ => OpErrorDetail::Query(query_detail(variant, p)),
    }
}

/// Every detail of every family the contract builds, from `p`.
fn catalogue(p: [u8; PAYLOAD]) -> Vec<OpErrorDetail> {
    (0u8..)
        .zip(FAMILY_SIZES)
        .flat_map(|(family, size)| (0..size).map(move |variant| detail(family, variant, p)))
        .collect()
}

/// The remedy of the kind `kind` picks, with `x` choosing among names and `p` its values.
fn remedy(kind: u8, x: u8, p: [u8; PAYLOAD]) -> Remedy {
    let step = pick(&STEPS, GridStep::Q0, x);
    match kind % 13 {
        0 => Remedy::ChangeInput {
            input: pick(&INPUTS, "", x),
            to: None,
        },
        1 => Remedy::ChangeInput {
            input: pick(&INPUTS, "", x & 7),
            to: Some((pick(&BOUNDS, Bound::AtMost, x >> 3), value(x >> 5, p))),
        },
        2 => Remedy::Choose {
            candidates: (0..usize::from(at(p, 0) % 4))
                .map(|i| {
                    let j = i.saturating_mul(2);
                    NameId::from_bytes(sixteen(
                        at(p, j.saturating_add(1)),
                        at(p, j.saturating_add(2)),
                    ))
                })
                .collect(),
        },
        3 => Remedy::UseRoute {
            route: pick(&ROUTES, "", x),
        },
        4 => Remedy::Accept {
            class: exactness(p),
        },
        5 => Remedy::Heal {
            gap: Steps(u64::from_le_bytes(p)),
            step,
        },
        6 => Remedy::ChangeFrame,
        7 => Remedy::CoarserStep { step },
        8 => Remedy::RaiseBudget {
            kind: pick(&BudgetKind::ALL, BudgetKind::Work, x),
            at_least: u128::from(u64::from_le_bytes(p)),
        },
        9 => Remedy::SplitCall,
        10 => Remedy::Preview,
        11 => Remedy::AwaitPiece {
            piece: pick(&PIECES, "", x),
        },
        _ => Remedy::Report,
    }
}

/// One evidence value.
fn evidence(s: &mut ByteSource<'_>) -> Evidence {
    let what = pick(&WHATS, "", s.byte());
    let kind = s.byte();
    Evidence {
        what,
        value: value(kind, payload(s)),
    }
}

/// One label: a step's name, any text and often printable, or a batch key.
fn label(s: &mut ByteSource<'_>) -> LabelDraw {
    let tag = s.byte();
    if tag & 1 == 1 {
        return LabelDraw::Key(sixteen(s.byte(), s.byte()));
    }
    let len = usize::from(s.byte() % 72);
    let printable = tag & 2 == 2;
    let pattern: [u8; 4] = core::array::from_fn(|_| {
        let b = s.byte();
        if printable { 0x20 | (b & 0x3F) } else { b }
    });
    let bytes: Vec<u8> = pattern.iter().copied().cycle().take(len).collect();
    LabelDraw::Step(String::from_utf8_lossy(&bytes).into_owned())
}

/// One path.
fn path(s: &mut ByteSource<'_>) -> PathDraw {
    let op = OpId::from_bytes(sixteen(s.byte(), s.byte()));
    let n = count(s, MAX_LABELS);
    PathDraw {
        op,
        labels: (0..n).map(|_| label(s)).collect(),
    }
}

/// One error.
fn error(s: &mut ByteSource<'_>) -> ErrorDraw {
    let (family, variant) = (s.byte(), s.byte());
    let detail = detail(family, variant, payload(s));
    let n = count(s, MAX_ITEMS);
    let entities = (0..n).map(|_| name(s)).collect();
    let n = count(s, MAX_ITEMS);
    let evidence = (0..n).map(|_| evidence(s)).collect();
    let n = count(s, MAX_ITEMS);
    let remedies = (0..n)
        .map(|_| {
            let (kind, x) = (s.byte(), s.byte());
            remedy(kind, x, payload(s))
        })
        .collect();
    let m = s.byte();
    let message = match m & 3 {
        0 => None,
        1 => Some(pick(&MESSAGES, "", m >> 2).to_owned()),
        2 => {
            let text: [u8; 4] = core::array::from_fn(|_| s.byte());
            Some(String::from_utf8_lossy(&text).into_owned())
        }
        _ => Some(String::new()),
    };
    ErrorDraw {
        detail,
        on: usize::from(m >> 7),
        entities,
        evidence,
        remedies,
        message,
    }
}

/// One warning.
fn warning(s: &mut ByteSource<'_>) -> WarningDraw {
    let code = pick(&CODES, "", s.byte());
    let n = count(s, MAX_ITEMS);
    let entities = (0..n).map(|_| name(s)).collect();
    let n = count(s, MAX_WARNING_EVIDENCE);
    let evidence = (0..n).map(|_| evidence(s)).collect();
    WarningDraw {
        code,
        entities,
        evidence,
        message: pick(&MESSAGES, "", s.byte()),
    }
}

/// The context.
fn context(s: &mut ByteSource<'_>) -> ContextDraw {
    let work = u64::from(u16::from_le_bytes([s.byte(), s.byte()]));
    let bytes = u64::from(u16::from_le_bytes([s.byte(), s.byte()]));
    let (bound, step) = (u16::from_le_bytes([s.byte(), s.byte()]), s.byte());
    let flags = s.byte();
    ContextDraw {
        work,
        bytes,
        preview: (flags & 1 == 1)
            .then(|| (Steps(u64::from(bound)), pick(&STEPS, GridStep::Q0, step))),
        cancel: flags & 2 == 2,
        cancelled: flags & 4 == 4,
        tally: flags & 8 == 8,
    }
}

/// The fuzzer's bytes as a case, and how many were read (at most [`MAX_INPUT_BYTES`]).
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut s = ByteSource::new(data);
    let paths = [path(&mut s), path(&mut s)];
    let n = count(&mut s, MAX_ERRORS);
    let errors = (0..n).map(|_| error(&mut s)).collect();
    let n = count(&mut s, MAX_DIAGNOSTICS);
    let warnings = (0..n).map(|_| warning(&mut s)).collect();
    let exactness = exactness(payload(&mut s));
    let context = context(&mut s);
    let chosen = u32::from_le_bytes([s.byte(), s.byte(), s.byte(), 0]);
    let turn = s.byte();
    let payload = payload(&mut s);
    let read = data.len().saturating_sub(s.remaining());
    (
        Case {
            paths,
            errors,
            warnings,
            exactness,
            context,
            chosen,
            turn,
            payload,
        },
        read,
    )
}

/// The fuzz harness: the laws on `data` decoded as a case, checked twice, which must agree.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let (case, _) = decode(data);
    let first = check(&case);
    if check(&case) != first {
        return Err(Finding::new(
            "the same input gives the same verdict twice",
            || hex(data),
        ));
    }
    first.map_err(|f| Finding {
        law: f.law,
        detail: format!("{}: {}", hex(data), f.detail),
    })
}

/// How a law names its break: the law, and what broke it.
type Law<'a> = &'a dyn Fn(&str, String) -> Finding;

/// `Err` of `law(name, what())` unless `holds`.
fn hold(holds: bool, law: Law<'_>, name: &str, what: impl FnOnce() -> String) -> Result<(), Finding> {
    if holds { Ok(()) } else { Err(law(name, what())) }
}

/// Holds the case to every law of the module header.
///
/// # Errors
///
/// A [`Finding`] naming the law the case broke.
pub fn check(case: &Case) -> Result<(), Finding> {
    let law = |law: &str, what: String| Finding::new(law, || format!("{case:?}: {what}"));
    check_kinds(&law)?;
    let paths = check_ids(case, &law)?;
    check_context(&case.context, &paths[0], &law)?;
    let built = build(case, &paths);
    hold(
        build(case, &paths) == built
            && built
                .iter()
                .zip(build(case, &paths))
                .all(|(a, b)| a.to_string() == b.to_string() && format!("{a:?}") == format!("{b:?}")),
        &law,
        "the errors built twice from the case are equal and write the same text",
        String::new,
    )?;
    for (draw, error) in case.errors.iter().zip(&built) {
        let at = paths.get(draw.on).unwrap_or(&paths[0]);
        check_error(draw, at, error, &law)?;
    }
    let remedies: Vec<&Remedy> = built.iter().flat_map(OpError::remedies).collect();
    check_remedy_texts(&remedies, &law)?;
    check_conversions(case.payload, &law)?;
    check_schemas(case, &law)?;
    check_output(case, &law)
}

/// The errors of `case`, built at `paths`.
fn build(case: &Case, paths: &[OpPath; 2]) -> Vec<OpError> {
    case.errors
        .iter()
        .map(|d| {
            let at = paths.get(d.on).unwrap_or(&paths[0]).clone();
            let (first, second) = d.entities.split_at(d.entities.len() / 2);
            let mut e = OpError::new(at, d.detail).with_entities(first.iter().copied());
            for ev in &d.evidence {
                e = e.with_evidence(*ev);
            }
            for r in &d.remedies {
                e = e.with_remedy(r.clone());
            }
            e = e.with_entities(second.iter().copied());
            match &d.message {
                Some(m) => e.with_message(m.clone()),
                None => e,
            }
        })
        .collect()
}

/// The kinds' laws.
fn check_kinds(law: Law<'_>) -> Result<(), Finding> {
    let all = ErrorKind::ALL;
    hold(
        all.windows(2).all(|w| matches!(w, [a, b] if a < b)),
        law,
        "ErrorKind::ALL holds every kind once, in chapter 22's order",
        || format!("{all:?}"),
    )?;
    for k in all {
        let replayable = !matches!(k, ErrorKind::Cancelled | ErrorKind::OutOfMemory);
        hold(
            k.name() == format!("{k:?}")
                && k.to_string() == k.name()
                && all.iter().filter(|j| j.name() == k.name()).count() == 1
                && k.is_replayable() == replayable,
            law,
            "a kind is named by its variant, once, and only Cancelled and OutOfMemory are not \
             replayable",
            || format!("{k:?}"),
        )?;
    }
    Ok(())
}

/// Whether the contract takes `name` as a step label: 1 to 64 bytes of printable ASCII, not
/// starting or ending with a space.
fn step_ok(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=64).contains(&b.len())
        && b.iter().all(|c| (0x20..=0x7E).contains(c))
        && b.first() != Some(&b' ')
        && b.last() != Some(&b' ')
}

/// A path's order key: its ID's bytes, then each label as a step (0) or a key (1) and its bytes.
fn order_key(path: &OpPath) -> ([u8; 16], Vec<(u8, Vec<u8>)>) {
    let labels = path
        .labels()
        .iter()
        .map(|l| match l {
            Label::Step(s) => (0, s.as_bytes().to_vec()),
            Label::Key(k) => (1, k.to_vec()),
        })
        .collect();
    (path.op().to_bytes(), labels)
}

/// A path's text, written from its parts: the ID's hex, then `/` and each label, a step quoted
/// and a key after `#`.
fn path_text(path: &OpPath) -> String {
    let mut text = hex(&path.op().to_bytes());
    for l in path.labels() {
        text.push('/');
        match l {
            Label::Step(s) => {
                text.push('"');
                text.push_str(&s.replace('\\', "\\\\").replace('"', "\\\""));
                text.push('"');
            }
            Label::Key(k) => {
                text.push('#');
                text.push_str(&hex(k));
            }
        }
    }
    text
}

/// A path read back from its canonical encoding with an encoder written elsewhere: an array of
/// the ID's sixteen bytes and each label, a step as text and a key as sixteen bytes.
fn read_path(bytes: &[u8]) -> Result<OpPath, String> {
    let mut dec = Decoder::from(bytes);
    let mut take = |n: usize, dec: &mut Decoder<&[u8]>| {
        let mut buf = vec![0; n];
        dec.read_exact(&mut buf).map(|()| buf).map_err(|e| format!("{e:?}"))
    };
    let Ok(Header::Array(Some(n))) = dec.pull() else {
        return Err("not an array of known length".into());
    };
    let Ok(Header::Bytes(Some(16))) = dec.pull() else {
        return Err("no sixteen-byte ID first".into());
    };
    let id: [u8; 16] = take(16, &mut dec)?
        .try_into()
        .map_err(|_| "a short ID".to_owned())?;
    let mut path = OpPath::new(OpId::from_bytes(id));
    for _ in 1..n {
        path = match dec.pull() {
            Ok(Header::Text(Some(len))) => {
                let text = String::from_utf8(take(len, &mut dec)?).map_err(|e| e.to_string())?;
                path.step(&text).ok_or_else(|| format!("{text:?} is no step"))?
            }
            Ok(Header::Bytes(Some(16))) => {
                let key: [u8; 16] = take(16, &mut dec)?
                    .try_into()
                    .map_err(|_| "a short key".to_owned())?;
                path.item(key)
            }
            other => return Err(format!("a label read as {other:?}")),
        };
    }
    if dec.offset() == bytes.len() {
        Ok(path)
    } else {
        Err(format!("bytes after the path, from {}", dec.offset()))
    }
}

/// The identity laws; the two paths built.
fn check_ids(case: &Case, law: Law<'_>) -> Result<[OpPath; 2], Finding> {
    let [a, b] = &case.paths;
    let built = [check_path(a, law)?, check_path(b, law)?];
    let [p, q] = &built;
    hold(
        p.cmp(q) == order_key(p).cmp(&order_key(q)),
        law,
        "paths order as their bytes and labels do",
        || format!("{p:?} {q:?}"),
    )?;
    let starts = |x: &OpPath, y: &OpPath| x.op() == y.op() && y.labels().starts_with(x.labels());
    hold(
        p.contains(q) == starts(p, q) && q.contains(p) == starts(q, p),
        law,
        "a path contains exactly the paths that start with it",
        || format!("{p:?} {q:?}"),
    )?;
    for e in case.errors.iter().flat_map(|e| &e.entities) {
        let bytes = e.to_bytes();
        hold(
            NameId::from_bytes(bytes) == *e
                && e.to_string() == hex(&bytes)
                && format!("{e:?}") == format!("NameId({})", hex(&bytes)),
            law,
            "a name writes its sixteen bytes as hex",
            || format!("{e:?}"),
        )?;
    }
    Ok(built)
}

/// One path's laws, as it is built label by label; the path.
fn check_path(draw: &PathDraw, law: Law<'_>) -> Result<OpPath, Finding> {
    let op = draw.op;
    let bytes = op.to_bytes();
    hold(
        OpId::from_bytes(bytes) == op
            && op.to_string() == hex(&bytes)
            && format!("{op:?}") == format!("OpId({})", hex(&bytes)),
        law,
        "an ID writes its sixteen bytes as hex",
        || format!("{op:?}"),
    )?;
    let mut path = OpPath::new(op);
    for l in &draw.labels {
        let next = match l {
            LabelDraw::Step(name) => {
                let made = Label::step(name);
                hold(
                    made.is_some() == step_ok(name) && path.step(name).is_some() == step_ok(name),
                    law,
                    "a step label is taken exactly when it is 1 to 64 bytes of printable ASCII, \
                     not starting or ending with a space",
                    || format!("{name:?}"),
                )?;
                let Some(label) = made else { continue };
                let next = path.child(label.clone());
                hold(
                    path.step(name).as_ref() == Some(&next),
                    law,
                    "a step's path is the path with its label added",
                    || format!("{name:?}"),
                )?;
                next
            }
            LabelDraw::Key(key) => {
                let next = path.item(*key);
                hold(
                    next == path.child(Label::key(*key)),
                    law,
                    "a batch item's path is the batch's path plus its key",
                    || hex(key),
                )?;
                next
            }
        };
        hold(
            next.labels().len() == path.labels().len() + 1
                && next.labels().starts_with(path.labels())
                && path.contains(&next)
                && !next.contains(&path)
                && path.contains(&path),
            law,
            "a child keeps its parent's labels and lies inside it",
            || format!("{path:?} {next:?}"),
        )?;
        path = next;
    }
    hold(
        path.to_string() == path_text(&path) && format!("{path:?}") == format!("OpPath({path})"),
        law,
        "a path writes its ID's hex and its labels",
        || format!("{path:?}"),
    )?;
    let mut e = Encoder::new();
    let encoded = path
        .encode(&mut e)
        .map_err(|x| format!("{x:?}"))
        .and_then(|()| e.finish().map_err(|x| format!("{x:?}")))
        .map(geom_op::cbor::Item::into_bytes);
    let back = encoded.as_deref().map_err(Clone::clone).and_then(read_path);
    hold(
        back.as_ref() == Ok(&path),
        law,
        "a path's canonical encoding reads back to it",
        || format!("{path:?}: {back:?}"),
    )?;
    Ok(path)
}

/// A tally that hears nothing.
struct Deaf;

impl Tally for Deaf {
    fn add(&self, _used: Usage) {}
}

/// The context's laws.
fn check_context(c: &ContextDraw, path: &OpPath, law: Law<'_>) -> Result<(), Finding> {
    let budget = Budget::new(c.work, c.bytes);
    let (token, other) = (CancelToken::new(), CancelToken::new());
    if c.cancelled {
        token.cancel();
    } else {
        other.cancel();
    }
    let fresh = OpContext::new(path.clone(), budget);
    hold(
        fresh.precision() == Precision::Final
            && fresh.precision().is_final()
            && fresh.cancel().is_none()
            && fresh.tally().is_none(),
        law,
        "a context is made final, with no cancellation and no tally",
        || format!("{fresh:?}"),
    )?;
    let precision = c
        .preview
        .map_or(Precision::Final, |(bound, step)| Precision::Preview { bound, step });
    let tally = Deaf;
    let made = |cancel: &'_ CancelToken| {
        let mut ctx = OpContext::new(path.clone(), budget);
        if c.preview.is_some() {
            ctx = ctx.with_precision(precision);
        }
        if c.cancel {
            ctx = ctx.with_cancel(cancel);
        }
        if c.tally {
            ctx = ctx.with_tally(&tally);
        }
        ctx
    };
    let ctx = made(&token);
    hold(
        ctx.path() == path
            && ctx.op() == path.op()
            && ctx.budget() == budget
            && ctx.precision() == precision
            && ctx.precision().is_final() == c.preview.is_none()
            && ctx.cancel().map(|x| x.requested()) == c.cancel.then_some(c.cancelled)
            && ctx.tally().is_some() == c.tally,
        law,
        "a context holds its path, budget, precision, cancellation and tally",
        || format!("{ctx:?}"),
    )?;
    let text = format!("{ctx:?}");
    hold(
        text.contains(&format!("cancellable: {}", c.cancel))
            && text.contains(&format!("{path:?}"))
            && text.contains(&format!("{precision:?}"))
            && text == format!("{:?}", made(&other))
            && text == format!("{:?}", ctx.clone()),
        law,
        "a context shows whether it can be cancelled, never the cancellation's state",
        || text.clone(),
    )
}

/// The remedy forms chapter 22's row for `kind` allows ("what the caller can do").
fn allowed(kind: ErrorKind, remedy: &Remedy) -> bool {
    use ErrorKind as K;
    match remedy {
        Remedy::ChangeInput { .. } => matches!(
            kind,
            K::InvalidInput
                | K::Degenerate
                | K::NoSolution
                | K::Ambiguous
                | K::Undecided
                | K::ExactnessNotAccepted
                | K::ToleranceExceeded
        ),
        Remedy::Choose { .. } => kind == K::Ambiguous,
        Remedy::UseRoute { .. } => matches!(kind, K::Undecided | K::Unsupported),
        Remedy::Accept { .. } => kind == K::ExactnessNotAccepted,
        Remedy::Heal { .. } => matches!(kind, K::Undecided | K::ToleranceExceeded),
        Remedy::ChangeFrame | Remedy::CoarserStep { .. } => kind == K::FrameRangeExceeded,
        Remedy::SplitCall => matches!(kind, K::FrameRangeExceeded | K::BudgetExceeded),
        Remedy::RaiseBudget { .. } | Remedy::Preview => kind == K::BudgetExceeded,
        Remedy::AwaitPiece { .. } => kind == K::Unsupported,
        Remedy::Report => kind == K::Internal,
        _ => false,
    }
}

/// The data `detail`'s message must name.
fn data_texts(detail: &OpErrorDetail) -> Vec<String> {
    match detail {
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x) | RunDetail::BatchTotal(x)) => vec![
            format!("`{}`", x.phase.name()),
            x.kind.to_string(),
            x.budget.limit(x.kind).to_string(),
            x.used.get(x.kind).to_string(),
        ],
        OpErrorDetail::Run(RunDetail::Internal { what }) => vec![(*what).to_owned()],
        OpErrorDetail::Num(NumDetail::OutOfRange { value }) => vec![value.to_string()],
        OpErrorDetail::Batch(BatchDetail::RepeatedKey { key }) => vec![hex(key)],
        OpErrorDetail::Make(m) => match m {
            MakeDetail::ZeroLength { input } => vec![(*input).to_owned()],
            MakeDetail::TooFewCorners { corners: n } | MakeDetail::MinorNotBelowMajor { major: n } => {
                vec![n.to_string()]
            }
            MakeDetail::NotAccepted { would } => vec![format!("{would:?}")],
            MakeDetail::NotCertifiedYet { piece } => vec![(*piece).to_owned()],
            _ => Vec::new(),
        },
        OpErrorDetail::Query(
            QueryDetail::CurvedEdge { edge: row }
            | QueryDetail::CurvedFace { face: row }
            | QueryDetail::NoLoops { face: row },
        ) => vec![row.to_string()],
        _ => Vec::new(),
    }
}

/// What chapter 22 says `detail`'s own remedies must hold, beyond their forms; `None` if they do.
fn carried(detail: &OpErrorDetail, remedies: &[Remedy]) -> Option<&'static str> {
    let has = |r: &Remedy| remedies.contains(r);
    let kind = detail.kind();
    if matches!(kind, ErrorKind::Cancelled | ErrorKind::OutOfMemory) && !remedies.is_empty() {
        return Some("Cancelled and OutOfMemory offer nothing");
    }
    if kind == ErrorKind::Internal && !has(&Remedy::Report) {
        return Some("a defect offers a report");
    }
    match detail {
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x) | RunDetail::BatchTotal(x)) => {
            let raise = Remedy::RaiseBudget {
                kind: x.kind,
                at_least: x.used.get(x.kind),
            };
            (!(has(&raise) && has(&Remedy::SplitCall) && has(&Remedy::Preview))).then_some(
                "a budget failure offers to raise the limit it passed to what was used, to split \
                 the call and to preview",
            )
        }
        OpErrorDetail::Num(NumDetail::OutOfRange { .. }) => (!has(&Remedy::ChangeFrame))
            .then_some("a coordinate past the frame offers another frame"),
        OpErrorDetail::Make(MakeDetail::NotAccepted { would }) => {
            (!has(&Remedy::Accept { class: *would }))
                .then_some("a class not accepted offers to accept the class it carries")
        }
        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }) => {
            (!has(&Remedy::AwaitPiece { piece })).then_some("a missing route names its piece")
        }
        OpErrorDetail::Make(MakeDetail::ZeroLength { input }) => (!remedies.iter().any(|r| {
            matches!(r, Remedy::ChangeInput { input: named, .. } if named == input)
        }))
        .then_some("a zero length offers to change the input it names"),
        _ => None,
    }
}

/// The kind a schema of the facade gives the detail named `name`, if any lists it.
fn listed_kind(name: &str) -> Option<ErrorKind> {
    OPERATIONS
        .iter()
        .flat_map(|op| op.schema.errors)
        .find(|spec| spec.detail == name)
        .map(|spec| spec.kind)
}

/// One error's laws: `error` is `draw` built at `path`.
fn check_error(
    draw: &ErrorDraw,
    path: &OpPath,
    error: &OpError,
    law: Law<'_>,
) -> Result<(), Finding> {
    let d = draw.detail;
    let fresh = OpError::new(path.clone(), d);
    let own = d.remedies();
    hold(
        fresh.kind() == d.kind()
            && fresh.detail() == &d
            && fresh.op() == path
            && fresh.entities().is_empty()
            && fresh.evidence().is_empty()
            && fresh.message() == d.to_string()
            && fresh.remedies() == own.as_slice(),
        law,
        "an error starts with its detail's kind, message and remedies, and nothing else",
        || format!("{fresh:?}"),
    )?;
    check_detail(&d, law)?;
    let mut remedies = own;
    remedies.extend(draw.remedies.iter().cloned());
    let message = draw.message.clone().unwrap_or_else(|| d.to_string());
    hold(
        error.kind() == d.kind()
            && error.detail() == &d
            && error.op() == path
            && error.entities() == draw.entities.as_slice()
            && error.evidence() == draw.evidence.as_slice()
            && error.remedies() == remedies.as_slice()
            && error.message() == message,
        law,
        "an error's kind is its detail's whatever is added, and it keeps what is added in order",
        || format!("{error:?}"),
    )?;
    let source = core::error::Error::source(error).map(ToString::to_string);
    hold(
        error.to_string() == format!("{} at {}: {}", d.kind().name(), path_text(path), message)
            && source == Some(d.to_string())
            && error.is_replayable()
                == !matches!(d.kind(), ErrorKind::Cancelled | ErrorKind::OutOfMemory)
            && error.clone() == *error,
        law,
        "an error writes `kind at path: message`, its detail is its source, and only Cancelled \
         and OutOfMemory are not replayable",
        || format!("{error}"),
    )?;
    for ev in error.evidence() {
        let text = ev.to_string();
        let rest = text
            .strip_prefix(ev.what)
            .and_then(|r| r.strip_prefix(": "));
        hold(
            rest.is_some_and(|r| reads_back(&ev.value, r)) && ev.value.to_string() == rest.unwrap_or(""),
            law,
            "evidence writes its name, then its value, whose text reads back to it",
            || text.clone(),
        )?;
    }
    Ok(())
}

/// A detail's laws: its kind, its remedies and the data its message names.
fn check_detail(d: &OpErrorDetail, law: Law<'_>) -> Result<(), Finding> {
    let own = d.remedies();
    let name = detail_name(&OpError::new(OpPath::new(OpId::from_bytes([0; 16])), *d));
    hold(
        listed_kind(&name).is_none_or(|k| k == d.kind()),
        law,
        "a detail a schema lists has the kind the schema states",
        || format!("{d:?}"),
    )?;
    hold(
        own.iter().all(|r| allowed(d.kind(), r)),
        law,
        "every remedy a detail knows is of a form its kind's row in chapter 22 allows",
        || format!("{d:?}: {own:?}"),
    )?;
    if let Some(broken) = carried(d, &own) {
        return Err(law(broken, format!("{d:?}: {own:?}")));
    }
    let message = d.to_string();
    hold(
        !message.is_empty() && data_texts(d).iter().all(|t| message.contains(t.as_str())),
        law,
        "a detail's message names its data",
        || format!("{d:?}: {message}"),
    )
}

/// Whether `text` reads back to `value`: its integers, its points, its length and step, its
/// interval's bounds bit for bit, or its class's own text.
fn reads_back(value: &Value, text: &str) -> bool {
    let ints = |inner: &str| -> Vec<Option<i64>> {
        inner.split(", ").map(|n| n.parse::<i64>().ok()).collect()
    };
    match value {
        Value::Point2(p) => text
            .strip_prefix('(')
            .and_then(|t| t.strip_suffix(") steps"))
            .is_some_and(|t| ints(t) == [Some(p.x.get()), Some(p.y.get())]),
        Value::Point3(p) => text
            .strip_prefix('(')
            .and_then(|t| t.strip_suffix(") steps"))
            .is_some_and(|t| ints(t) == [Some(p.x.get()), Some(p.y.get()), Some(p.z.get())]),
        Value::Coordinate(c) => text
            .strip_suffix(" steps")
            .is_some_and(|t| t.parse::<i128>().ok() == Some(*c)),
        Value::Length { steps, step } => text.split_once(" steps of ").is_some_and(|(n, s)| {
            n.parse::<u64>().ok() == Some(steps.0) && s == format!("{step:?}")
        }),
        Value::Parameter(range) => text
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
            .and_then(|t| t.split_once(", "))
            .is_some_and(|(lo, hi)| {
                lo.parse::<f64>().ok().map(f64::to_bits) == Some(range.lo().to_bits())
                    && hi.parse::<f64>().ok().map(f64::to_bits) == Some(range.hi().to_bits())
            }),
        Value::Exactness(class) => text == format!("{class:?}"),
        Value::Count(n) => text.parse::<u128>().ok() == Some(*n),
        _ => false,
    }
}

/// The words a bound writes, from chapter 22's "at most 4.2 mm".
fn bound_words(bound: Bound) -> &'static str {
    match bound {
        Bound::AtMost => "at most",
        Bound::AtLeast => "at least",
        Bound::Exactly => "exactly",
    }
}

/// Whether `remedy`'s text names what it holds.
fn names_itself(remedy: &Remedy, text: &str) -> bool {
    let all = |parts: &[String]| parts.iter().all(|p| text.contains(p.as_str()));
    match remedy {
        Remedy::ChangeInput { input, to: None } => all(&[format!("`{input}`")]),
        Remedy::ChangeInput {
            input,
            to: Some((bound, value)),
        } => {
            let value = value.to_string();
            all(&[format!("`{input}`"), format!("{} {value}", bound_words(*bound))])
                && text.ends_with(&value)
                && reads_back(&Value::clone(&remedy_value(remedy)), &value)
        }
        Remedy::Choose { candidates } => {
            let mut rest = text;
            candidates.iter().all(|c| match rest.find(&c.to_string()) {
                Some(i) => {
                    rest = rest.get(i.saturating_add(32)..).unwrap_or("");
                    true
                }
                None => false,
            })
        }
        Remedy::UseRoute { route } => all(&[format!("`{route}`")]),
        Remedy::Accept { class } => all(&[format!("{class:?}")]),
        Remedy::Heal { gap, step } => all(&[gap.0.to_string(), format!("{step:?}")]),
        Remedy::CoarserStep { step } => all(&[format!("{step:?}")]),
        Remedy::RaiseBudget { kind, at_least } => all(&[kind.to_string(), at_least.to_string()]),
        Remedy::AwaitPiece { piece } => all(&[(*piece).to_owned()]),
        _ => true,
    }
}

/// The value a remedy changes its input to; a count of zero for any other.
fn remedy_value(remedy: &Remedy) -> Value {
    match remedy {
        Remedy::ChangeInput {
            to: Some((_, value)),
            ..
        } => *value,
        _ => Value::Count(0),
    }
}

/// The remedies' text laws, over every remedy of the case's errors.
fn check_remedy_texts(remedies: &[&Remedy], law: Law<'_>) -> Result<(), Finding> {
    let texts: Vec<String> = remedies.iter().map(ToString::to_string).collect();
    for (r, t) in remedies.iter().zip(&texts) {
        hold(
            !t.is_empty() && names_itself(r, t),
            law,
            "a remedy names its input, value, candidates in order, route, class, step, budget or \
             piece",
            || format!("{r:?}: {t}"),
        )?;
    }
    for (i, (a, s)) in remedies.iter().zip(&texts).enumerate() {
        for (b, t) in remedies.iter().zip(&texts).skip(i.saturating_add(1)) {
            hold(
                s != t || a == b,
                law,
                "two remedies write the same text only if they are equal",
                || format!("{a:?} {b:?}: {s}"),
            )?;
        }
    }
    Ok(())
}

/// The kind chapter 22 gives each error of the numbers layer.
fn chapter_kind(error: NumError) -> ErrorKind {
    match error {
        NumError::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
        NumError::Degenerate | NumError::NotFinite => ErrorKind::Degenerate,
        NumError::OutsideDomain => ErrorKind::NoSolution,
        NumError::DivisorContainsZero | NumError::NegativeSqrt => ErrorKind::Undecided,
        NumError::EmptyInterval | NumError::WrongDirection | NumError::BudgetExceeded => {
            ErrorKind::Internal
        }
        NumError::ToleranceExceeded => ErrorKind::ExactnessNotAccepted,
        NumError::UnprovenMaths => ErrorKind::Unsupported,
    }
}

/// The conversions' laws, on values from `p`.
fn check_conversions(p: [u8; PAYLOAD], law: Law<'_>) -> Result<(), Finding> {
    let (x, q, m) = (exceeded(p), query_detail(at(p, 0), p), make_detail(at(p, 1), p));
    let what = pick(&BROKE, "", at(p, 2));
    let run = |d: RunDetail| OpErrorDetail::Run(d);
    let pairs: [(OpErrorDetail, OpErrorDetail, ErrorKind); 15] = [
        (RunError::Failed(q).into(), OpErrorDetail::Query(q), q.kind()),
        (
            RunError::<QueryDetail>::BudgetExceeded(x).into(),
            run(RunDetail::BudgetExceeded(x)),
            ErrorKind::BudgetExceeded,
        ),
        (RunError::<QueryDetail>::Cancelled.into(), run(RunDetail::Cancelled), ErrorKind::Cancelled),
        (
            RunError::<QueryDetail>::OutOfMemory.into(),
            run(RunDetail::OutOfMemory),
            ErrorKind::OutOfMemory,
        ),
        (
            RunError::<QueryDetail>::Internal(what).into(),
            run(RunDetail::Internal { what }),
            ErrorKind::Internal,
        ),
        (ItemError::Failed(m).into(), OpErrorDetail::Make(m), m.kind()),
        (
            ItemError::<MakeDetail>::BudgetExceeded(x).into(),
            run(RunDetail::BudgetExceeded(x)),
            ErrorKind::BudgetExceeded,
        ),
        (
            ItemError::<MakeDetail>::BatchTotal(x).into(),
            run(RunDetail::BatchTotal(x)),
            ErrorKind::BudgetExceeded,
        ),
        (Interrupted::Cancelled.into(), run(RunDetail::Cancelled), ErrorKind::Cancelled),
        (Interrupted::OutOfMemory.into(), run(RunDetail::OutOfMemory), ErrorKind::OutOfMemory),
        (
            Interrupted::Internal(what).into(),
            run(RunDetail::Internal { what }),
            ErrorKind::Internal,
        ),
        (
            run(RunDetail::from(Interrupted::Internal(what))),
            run(RunDetail::Internal { what }),
            ErrorKind::Internal,
        ),
        (m.into(), OpErrorDetail::Make(m), m.kind()),
        (q.into(), OpErrorDetail::Query(q), q.kind()),
        (RunDetail::Cancelled.into(), run(RunDetail::Cancelled), ErrorKind::Cancelled),
    ];
    for (made, expected, kind) in pairs {
        hold(
            made == expected && made.kind() == kind,
            law,
            "every conversion from the helper's errors keeps the stop it is",
            || format!("{made:?} {expected:?}"),
        )?;
    }
    for v in 0..11 {
        let n = num_error(v, p);
        let d = OpErrorDetail::from(n);
        hold(
            d == OpErrorDetail::Num(NumDetail::from(n))
                && d == OpErrorDetail::from(NumDetail::from(n))
                && d.kind() == chapter_kind(n),
            law,
            "every error of the numbers layer lands on the kind chapter 22 gives it",
            || format!("{n:?}: {d:?}"),
        )?;
    }
    Ok(())
}

/// The class of `e`, read by its variant.
fn class(e: Exactness) -> Class {
    match e {
        Exactness::Exact => Class::Exact,
        Exactness::Certified { .. } => Class::Certified,
        Exactness::Approximate { .. } => Class::Approximate,
    }
}

/// The schemas' laws: every schema of the facade, then the tool descriptions of the chosen ones.
fn check_schemas(case: &Case, law: Law<'_>) -> Result<(), Finding> {
    let details = catalogue(case.payload);
    let at = OpPath::new(OpId::from_bytes([0x82; 16]));
    let names: Vec<(String, &OpErrorDetail)> = details
        .iter()
        .map(|d| (detail_name(&OpError::new(at.clone(), *d)), d))
        .collect();
    for d in &details {
        check_detail(d, law)?;
    }
    for op in OPERATIONS {
        let s = op.schema;
        for (i, spec) in s.errors.iter().enumerate() {
            hold(
                !matches!(spec.kind, ErrorKind::Cancelled | ErrorKind::OutOfMemory)
                    && s.errors.iter().skip(i.saturating_add(1)).all(|o| o.detail != spec.detail),
                law,
                "no schema lists Cancelled, OutOfMemory or a detail twice",
                || format!("{}: {}", s.name, spec.detail),
            )?;
            let built: Vec<&OpErrorDetail> = names
                .iter()
                .filter(|(n, _)| n == spec.detail)
                .map(|(_, d)| *d)
                .collect();
            hold(
                !built.is_empty()
                    && built.iter().all(|d| {
                        let remedies = OpError::new(at.clone(), **d).remedies().to_vec();
                        d.kind() == spec.kind && !remedies.is_empty()
                    }),
                law,
                "every error a schema lists is a detail of the contract, of the schema's kind, \
                 that offers a remedy",
                || format!("{}: {} {:?}", s.name, spec.detail, spec.kind),
            )?;
        }
        for w in s.diagnostics {
            hold(
                Diagnostic::new(w.code, w.doc).is_some(),
                law,
                "every warning code a schema lists is one a Diagnostic takes",
                || format!("{}: {}", s.name, w.code),
            )?;
        }
    }
    let n = OPERATIONS.len();
    let chosen: Vec<Operation> = (0..n)
        .filter(|i| {
            u32::try_from(i % 24).is_ok_and(|b| case.chosen.checked_shr(b).is_some_and(|m| m & 1 == 1))
        })
        .filter_map(|i| OPERATIONS.get((i + usize::from(case.turn)) % n).copied())
        .collect();
    check_tools(&chosen, law)
}

/// The tool descriptions of `ops`: they parse as JSON, write and parse back to the same value,
/// and give back each operation's schema.
fn check_tools(ops: &[Operation], law: Law<'_>) -> Result<(), Finding> {
    let parsed = geom_gen::facade::mcp(ops)
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice::<Json>(&bytes).map_err(|e| e.to_string()));
    let again = parsed.as_ref().map_err(Clone::clone).and_then(|j| {
        serde_json::to_vec(j)
            .and_then(|b| serde_json::from_slice::<Json>(&b))
            .map_err(|e| e.to_string())
    });
    let (Ok(json), Ok(again)) = (&parsed, &again) else {
        return Err(law(
            "the tool descriptions of any choice of operations parse as JSON",
            format!("{parsed:?} {again:?}"),
        ));
    };
    let tools = json.get("tools").and_then(Json::as_array);
    hold(
        json == again && tools.is_some_and(|t| t.len() == ops.len()),
        law,
        "the tool descriptions write and parse back to the same value, one per operation",
        || format!("{json}"),
    )?;
    for (tool, op) in tools.into_iter().flatten().zip(ops) {
        let s = op.schema;
        let meta = tool.get("_meta").and_then(|m| m.get("org.spareware/kernel"));
        let field = |k: &str| meta.and_then(|m| m.get(k));
        let list = |k: &str| field(k).and_then(Json::as_array).cloned().unwrap_or_default();
        let text = |j: &Json, k: &str| j.get(k).and_then(Json::as_str).map(str::to_owned);
        let errors = list("errors");
        let errors_back = errors.len() == s.errors.len()
            && errors.iter().zip(s.errors).all(|(j, spec)| {
                let kind = text(j, "kind");
                text(j, "detail").as_deref() == Some(spec.detail)
                    && ErrorKind::ALL
                        .iter()
                        .find(|k| Some(k.name()) == kind.as_deref())
                        == Some(&spec.kind)
            });
        let warnings = list("warnings");
        let warnings_back = warnings.len() == s.diagnostics.len()
            && warnings
                .iter()
                .zip(s.diagnostics)
                .all(|(j, w)| text(j, "code").as_deref() == Some(w.code));
        let inputs: BTreeSet<&str> = tool
            .get("inputSchema")
            .and_then(|i| i.get("properties"))
            .and_then(Json::as_object)
            .map(|o| o.keys().map(String::as_str).collect())
            .unwrap_or_default();
        hold(
            text(tool, "name").as_deref() == Some(s.name)
                && field("version").and_then(Json::as_u64) == Some(u64::from(s.version))
                && errors_back
                && warnings_back
                && list("exactness").len() == s.classes.len()
                && field("batch").and_then(Json::as_bool) == Some(s.batch)
                && inputs == s.inputs.iter().map(|f| f.name).collect(),
            law,
            "a tool description gives back its operation's name, version, errors with their \
             kinds, warnings, classes, batch entry and inputs",
            || format!("{}: {tool}", s.name),
        )?;
    }
    Ok(())
}

/// Whether `code` is a valid warning code: 1 to 64 bytes of lowercase ASCII letters, digits, `.`
/// and `_`.
fn code_ok(code: &str) -> bool {
    (1..=64).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
}

/// The output's and the warnings' laws.
fn check_output(case: &Case, law: Law<'_>) -> Result<(), Finding> {
    let e = case.exactness;
    let mut out = OpOutput::new(case.errors.len(), e);
    hold(
        out.result == case.errors.len()
            && out.exactness == e
            && out.diagnostics.is_empty()
            && Class::of(e) == class(e),
        law,
        "an output is made with its result and class and no warning, and Class::of reads the \
         class",
        || format!("{out:?}"),
    )?;
    let mut kept = Vec::new();
    for w in &case.warnings {
        let made = Diagnostic::new(w.code, w.message);
        hold(
            made.is_some() == code_ok(w.code),
            law,
            "a warning is made exactly for a valid code",
            || w.code.to_owned(),
        )?;
        let Some(made) = made else { continue };
        hold(
            made.code == w.code
                && made.message == w.message
                && made.entities.is_empty()
                && made.at.is_empty(),
            law,
            "a warning starts with its code and words and nothing else",
            || format!("{made:?}"),
        )?;
        let (first, second) = w.entities.split_at(w.entities.len() / 2);
        let mut d = made
            .with_entities(first.iter().copied())
            .with_entities(second.iter().copied());
        for ev in &w.evidence {
            d = d.with_evidence(*ev);
        }
        hold(
            d.entities == w.entities
                && d.at == w.evidence
                && d.to_string() == format!("{}: {}", w.code, w.message),
            law,
            "a warning keeps its entities and evidence in order and writes `code: message`",
            || format!("{d:?}"),
        )?;
        kept.push(d.clone());
        out = out.with_diagnostic(d);
    }
    hold(
        out.diagnostics == kept && out.exactness == e && out.result == case.errors.len(),
        law,
        "an output keeps its result, class and warnings in order",
        || format!("{out:?}"),
    )
}
