//! Tests of every promise `geom_op`'s public interface makes, each through the interface alone
//! (the mutation gate judges the crate by these files only).
//!
//! What this file proves:
//!
//! - IDs and names keep their bytes and print them as hexadecimal; step labels are exactly the
//!   printable ASCII strings of 1 to 64 bytes that neither start nor end with a space; a path keeps
//!   its operation and labels in order, adds a step or an item key at its end, and contains exactly
//!   itself and the paths below it.
//! - A context keeps its path, budget, cancellation and precision, starts final and uncancellable,
//!   and prints no address.
//! - Every base kind has chapter 22's name, and only `Cancelled` and `OutOfMemory` are not
//!   replayable.
//! - Every detail has its base kind, its remedies and its message, and every conversion from the
//!   helper's and the numbers layer's errors lands on the detail of the same meaning.
//! - An error is built from its detail, keeps what the operation adds in order, and prints its
//!   kind, path and message.
//! - Evidence, remedies, warnings, outputs and schema classes print and keep what they are given.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their cases.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::error::Error as _;

use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, NumError, P2, P3, Steps};
use geom_op::{
    Bound, Class, Diagnostic, ErrorKind, Evidence, Label, NameId, NumDetail, OpContext, OpError,
    OpErrorDetail, OpId, OpOutput, OpPath, Precision, Remedy, RunDetail, Value,
};
use geom_run::{
    Budget, BudgetKind, Cancel, CancelToken, Exceeded, Interrupted, ItemError, Phase, RunError,
    Usage,
};

const PHASE: Phase = Phase::new("contract");

fn bytes(first: u8) -> [u8; 16] {
    core::array::from_fn(|i| first.wrapping_add(i as u8 * 17))
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0xab; 16]))
}

fn exceeded() -> Exceeded {
    Exceeded {
        budget: Budget::new(10, 100),
        kind: BudgetKind::LogicalBytes,
        phase: PHASE,
        used: Usage::new(7, 101),
    }
}

// Identity.

#[test]
fn ids_and_names_keep_their_bytes_and_print_them_as_hexadecimal() {
    let b: [u8; 16] = [
        0x00, 0x01, 0x0f, 0x10, 0x7f, 0x80, 0xab, 0xff, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde,
        0xf0,
    ];
    let hex = "00010f107f80abff123456789abcdef0";
    let id = OpId::from_bytes(b);
    assert_eq!(id.to_bytes(), b);
    assert_eq!(id.to_string(), hex);
    assert_eq!(format!("{id:?}"), format!("OpId({hex})"));
    let name = NameId::from_bytes(b);
    assert_eq!(name.to_bytes(), b);
    assert_eq!(name.to_string(), hex);
    assert_eq!(format!("{name:?}"), format!("NameId({hex})"));
}

#[test]
fn a_step_label_is_printable_ascii_of_one_to_64_bytes_without_outer_spaces() {
    for good in ["a", "!", "~", "cut", "blend faces", "a b", &"x".repeat(64)] {
        assert_eq!(
            Label::step(good),
            Some(Label::Step(good.to_owned())),
            "{good:?}"
        );
    }
    for bad in [
        "",
        " ",
        " cut",
        "cut ",
        "tab\there",
        "line\nbreak",
        "del\u{7f}",
        "caf\u{e9}",
        "nul\0",
        &"x".repeat(65),
    ] {
        assert_eq!(Label::step(bad), None, "{bad:?}");
    }
    assert_eq!(Label::key(bytes(3)), Label::Key(bytes(3)));
}

#[test]
fn labels_print_steps_quoted_and_keys_as_hexadecimal() {
    let step = Label::step("blend faces").unwrap();
    assert_eq!(step.to_string(), "\"blend faces\"");
    assert_eq!(format!("{step:?}"), "\"blend faces\"");
    let key = Label::key([0x0a; 16]);
    assert_eq!(key.to_string(), "#0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a");
    assert_eq!(format!("{key:?}"), "#0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a");
}

#[test]
fn a_path_keeps_its_operation_and_adds_labels_at_its_end() {
    let op = OpId::from_bytes(bytes(1));
    let root = OpPath::new(op);
    assert_eq!(root.op(), op);
    assert_eq!(root.labels(), []);
    let cut = root.step("cut").unwrap();
    assert_eq!(cut.op(), op);
    assert_eq!(cut.labels(), [Label::Step("cut".to_owned())]);
    let item = cut.item(bytes(9));
    assert_eq!(
        item.labels(),
        [Label::Step("cut".to_owned()), Label::Key(bytes(9))]
    );
    assert_eq!(item, cut.child(Label::Key(bytes(9))));
    assert_eq!(root.step(" cut"), None);
    assert_eq!(root.step(""), None);
    // The parent is unchanged by its children.
    assert_eq!(cut.labels().len(), 1);
}

#[test]
fn a_path_contains_exactly_itself_and_the_paths_below_it() {
    let root = path();
    let cut = root.step("cut").unwrap();
    let item = cut.item(bytes(2));
    let other_step = root.step("trim").unwrap();
    let other_op = OpPath::new(OpId::from_bytes([0xac; 16]));
    assert!(root.contains(&root));
    assert!(root.contains(&cut));
    assert!(root.contains(&item));
    assert!(cut.contains(&item));
    assert!(!cut.contains(&root));
    assert!(!item.contains(&cut));
    assert!(!cut.contains(&other_step));
    assert!(!other_step.contains(&item));
    assert!(!root.contains(&other_op));
    assert!(!other_op.contains(&root));
    // Same labels, another operation.
    assert!(!other_op.step("cut").unwrap().contains(&item));
}

#[test]
fn a_path_prints_its_operation_and_labels() {
    let item = path().step("cut").unwrap().item([1; 16]);
    let text = "abababababababababababababababab/\"cut\"/#01010101010101010101010101010101";
    assert_eq!(item.to_string(), text);
    assert_eq!(format!("{item:?}"), format!("OpPath({text})"));
    assert_eq!(path().to_string(), "ab".repeat(16));
}

// The context.

#[test]
fn a_context_keeps_what_it_is_given_and_starts_final_and_uncancellable() {
    let token = CancelToken::new();
    let at = path().step("cut").unwrap();
    let ctx = OpContext::new(at.clone(), Budget::new(5, 6));
    assert_eq!(ctx.path(), &at);
    assert_eq!(ctx.op(), at.op());
    assert_eq!(ctx.budget(), Budget::new(5, 6));
    assert_eq!(ctx.precision(), Precision::Final);
    assert!(ctx.cancel().is_none());
    let preview = Precision::Preview {
        bound: Steps(640),
        step: GridStep::Q0,
    };
    let ctx = ctx.with_cancel(&token).with_precision(preview);
    assert_eq!(ctx.precision(), preview);
    assert_eq!(ctx.path(), &at);
    assert_eq!(ctx.budget(), Budget::new(5, 6));
    let cancel = ctx.cancel().unwrap();
    assert!(!cancel.requested());
    token.cancel();
    assert!(cancel.requested());
    // with_cancel keeps the precision, with_precision keeps the cancellation.
    let other = CancelToken::new();
    let again = ctx.clone().with_cancel(&other);
    assert_eq!(again.precision(), preview);
    assert!(!again.cancel().unwrap().requested());
    assert!(ctx.with_precision(Precision::Final).cancel().is_some());
}

#[test]
fn a_context_prints_its_values_and_no_address() {
    let token = CancelToken::new();
    let ctx = OpContext::new(path(), Budget::new(5, 6));
    assert_eq!(
        format!("{ctx:?}"),
        format!(
            "OpContext {{ path: OpPath({}), budget: Budget {{ work: 5, logical_bytes: 6 }}, \
             cancellable: false, precision: Final }}",
            "ab".repeat(16)
        )
    );
    assert!(format!("{:?}", ctx.with_cancel(&token)).contains("cancellable: true"));
}

#[test]
fn only_final_is_final() {
    assert!(Precision::Final.is_final());
    assert!(
        !Precision::Preview {
            bound: Steps(1),
            step: GridStep::Q1
        }
        .is_final()
    );
}

// Kinds.

#[test]
fn every_kind_has_chapter_22s_name_in_chapter_22s_order() {
    let names: Vec<&str> = ErrorKind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(
        names,
        [
            "InvalidInput",
            "Degenerate",
            "NoSolution",
            "Ambiguous",
            "Undecided",
            "ExactnessNotAccepted",
            "ToleranceExceeded",
            "FrameRangeExceeded",
            "BudgetExceeded",
            "Cancelled",
            "OutOfMemory",
            "Unsupported",
            "Internal",
        ]
    );
    for kind in ErrorKind::ALL {
        assert_eq!(kind.to_string(), kind.name());
        assert_eq!(format!("{kind:?}"), kind.name());
    }
}

#[test]
fn only_cancelled_and_out_of_memory_are_not_replayable() {
    for kind in ErrorKind::ALL {
        let stopped = matches!(kind, ErrorKind::Cancelled | ErrorKind::OutOfMemory);
        assert_eq!(kind.is_replayable(), !stopped, "{kind}");
    }
}

// Details.

#[test]
fn every_numbers_layer_error_lands_on_the_detail_of_the_same_meaning() {
    let cases = [
        (
            NumError::OutOfRange { value: -5 },
            NumDetail::OutOfRange { value: -5 },
            ErrorKind::FrameRangeExceeded,
        ),
        (
            NumError::Degenerate,
            NumDetail::Degenerate,
            ErrorKind::Degenerate,
        ),
        (
            NumError::NotFinite,
            NumDetail::NotFinite,
            ErrorKind::Degenerate,
        ),
        (
            NumError::WrongDirection,
            NumDetail::WrongDirection,
            ErrorKind::Internal,
        ),
        (
            NumError::DivisorContainsZero,
            NumDetail::DivisorContainsZero,
            ErrorKind::Undecided,
        ),
        (
            NumError::NegativeSqrt,
            NumDetail::NegativeSqrt,
            ErrorKind::Undecided,
        ),
        (
            NumError::EmptyInterval,
            NumDetail::EmptyInterval,
            ErrorKind::Internal,
        ),
        (
            NumError::OutsideDomain,
            NumDetail::OutsideDomain,
            ErrorKind::NoSolution,
        ),
        (
            NumError::BudgetExceeded,
            NumDetail::BitBudget,
            ErrorKind::Internal,
        ),
        (
            NumError::ToleranceExceeded,
            NumDetail::BoundOverCeiling,
            ErrorKind::ExactnessNotAccepted,
        ),
        (
            NumError::UnprovenMaths,
            NumDetail::UnprovenMaths,
            ErrorKind::Unsupported,
        ),
    ];
    for (error, detail, kind) in cases {
        assert_eq!(NumDetail::from(error), detail, "{error:?}");
        assert_eq!(detail.kind(), kind, "{detail:?}");
        assert_eq!(OpErrorDetail::from(error), OpErrorDetail::Num(detail));
        assert_eq!(OpErrorDetail::from(detail), OpErrorDetail::Num(detail));
        assert_eq!(OpErrorDetail::Num(detail).kind(), kind);
        let remedies = match kind {
            ErrorKind::FrameRangeExceeded => vec![Remedy::ChangeFrame],
            ErrorKind::Internal => vec![Remedy::Report],
            _ => Vec::new(),
        };
        assert_eq!(detail.remedies(), remedies, "{detail:?}");
        assert_eq!(OpErrorDetail::Num(detail).remedies(), remedies);
    }
}

#[test]
fn numbers_layer_details_say_what_happened() {
    let messages = [
        (
            NumDetail::OutOfRange { value: 1 << 41 },
            "coordinate 2199023255552 grid steps is outside the frame's range of ±(2^40 − 1) steps",
        ),
        (
            NumDetail::Degenerate,
            "the inputs are degenerate for this construction",
        ),
        (
            NumDetail::NotFinite,
            "a value was not finite: the configuration is singular here (a division by zero, a \
             pole or an overflow)",
        ),
        (
            NumDetail::OutsideDomain,
            "a maths function was asked for a value it does not have",
        ),
        (
            NumDetail::DivisorContainsZero,
            "whether a divisor is zero cannot be decided at this resolution",
        ),
        (
            NumDetail::NegativeSqrt,
            "whether a value under a square root is negative cannot be decided at this resolution",
        ),
        (
            NumDetail::EmptyInterval,
            "internal error: an empty interval was asked for; please report it",
        ),
        (
            NumDetail::WrongDirection,
            "internal error: a grid-step conversion went the wrong way; please report it",
        ),
        (
            NumDetail::BitBudget,
            "internal error: an exact computation exceeded its proven bit budget; please report it",
        ),
        (
            NumDetail::BoundOverCeiling,
            "a certified bound would pass the ceiling the caller accepted",
        ),
        (
            NumDetail::UnprovenMaths,
            "this needs a maths function with no certified enclosure yet",
        ),
    ];
    for (detail, text) in messages {
        assert_eq!(detail.to_string(), text);
        assert_eq!(OpErrorDetail::Num(detail).to_string(), text);
        assert!(detail.source().is_none());
    }
}

#[test]
fn every_helper_stop_lands_on_the_detail_of_the_same_meaning() {
    let x = exceeded();
    let cases = [
        (
            RunDetail::BudgetExceeded(x),
            ErrorKind::BudgetExceeded,
            "phase `contract` passed its logical-memory budget of 100 with 101 used; raise the \
             budget, split the call, or ask for a preview"
                .to_owned(),
        ),
        (
            RunDetail::BatchTotal(x),
            ErrorKind::BudgetExceeded,
            format!("the batch's total ran out at or before this item: {x}"),
        ),
        (
            RunDetail::Cancelled,
            ErrorKind::Cancelled,
            "cancelled by the caller; nothing was published".to_owned(),
        ),
        (
            RunDetail::OutOfMemory,
            ErrorKind::OutOfMemory,
            "the machine refused a large allocation; nothing was published".to_owned(),
        ),
        (
            RunDetail::Internal {
                what: "a chunk ran twice",
            },
            ErrorKind::Internal,
            "internal error in the parallel helper (a chunk ran twice); nothing was published, \
             please report it"
                .to_owned(),
        ),
    ];
    for (detail, kind, text) in cases {
        assert_eq!(detail.kind(), kind, "{detail:?}");
        assert_eq!(OpErrorDetail::Run(detail).kind(), kind);
        assert_eq!(detail.to_string(), text);
        assert_eq!(OpErrorDetail::Run(detail).to_string(), text);
        assert_eq!(OpErrorDetail::from(detail), OpErrorDetail::Run(detail));
        assert!(detail.source().is_none());
    }
}

#[test]
fn a_budget_failure_suggests_the_budget_it_needed_a_split_or_a_preview() {
    let x = exceeded();
    let expected = vec![
        Remedy::RaiseBudget {
            kind: BudgetKind::LogicalBytes,
            at_least: 101,
        },
        Remedy::SplitCall,
        Remedy::Preview,
    ];
    assert_eq!(RunDetail::BudgetExceeded(x).remedies(), expected);
    assert_eq!(RunDetail::BatchTotal(x).remedies(), expected);
    let work = Exceeded {
        kind: BudgetKind::Work,
        ..x
    };
    assert_eq!(
        RunDetail::BudgetExceeded(work).remedies()[0],
        Remedy::RaiseBudget {
            kind: BudgetKind::Work,
            at_least: 7
        }
    );
    assert_eq!(
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x)).remedies(),
        expected
    );
    assert_eq!(RunDetail::Cancelled.remedies(), []);
    assert_eq!(RunDetail::OutOfMemory.remedies(), []);
    assert_eq!(
        RunDetail::Internal { what: "x" }.remedies(),
        [Remedy::Report]
    );
}

#[test]
fn the_helpers_errors_convert_by_meaning() {
    let x = exceeded();
    assert_eq!(
        RunDetail::from(Interrupted::Cancelled),
        RunDetail::Cancelled
    );
    assert_eq!(
        RunDetail::from(Interrupted::OutOfMemory),
        RunDetail::OutOfMemory
    );
    assert_eq!(
        RunDetail::from(Interrupted::Internal("w")),
        RunDetail::Internal { what: "w" }
    );
    assert_eq!(
        OpErrorDetail::from(Interrupted::Cancelled),
        OpErrorDetail::Run(RunDetail::Cancelled)
    );
    let run = |e: RunError<NumError>| OpErrorDetail::from(e);
    assert_eq!(
        run(RunError::Failed(NumError::NegativeSqrt)),
        OpErrorDetail::Num(NumDetail::NegativeSqrt)
    );
    assert_eq!(
        run(RunError::BudgetExceeded(x)),
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x))
    );
    assert_eq!(
        run(RunError::Cancelled),
        OpErrorDetail::Run(RunDetail::Cancelled)
    );
    assert_eq!(
        run(RunError::OutOfMemory),
        OpErrorDetail::Run(RunDetail::OutOfMemory)
    );
    assert_eq!(
        run(RunError::Internal("v")),
        OpErrorDetail::Run(RunDetail::Internal { what: "v" })
    );
    let item = |e: ItemError<NumError>| OpErrorDetail::from(e);
    assert_eq!(
        item(ItemError::Failed(NumError::Degenerate)),
        OpErrorDetail::Num(NumDetail::Degenerate)
    );
    assert_eq!(
        item(ItemError::BudgetExceeded(x)),
        OpErrorDetail::Run(RunDetail::BudgetExceeded(x))
    );
    assert_eq!(
        item(ItemError::BatchTotal(x)),
        OpErrorDetail::Run(RunDetail::BatchTotal(x))
    );
}

// Errors.

#[test]
fn an_error_is_built_from_its_detail() {
    let at = path().step("cut").unwrap();
    let error = OpError::new(at.clone(), NumError::OutOfRange { value: 1 << 41 });
    assert_eq!(error.kind(), ErrorKind::FrameRangeExceeded);
    assert_eq!(
        error.detail(),
        &OpErrorDetail::Num(NumDetail::OutOfRange { value: 1 << 41 })
    );
    assert_eq!(error.op(), &at);
    assert_eq!(error.entities(), []);
    assert_eq!(error.evidence(), []);
    assert_eq!(
        error.message(),
        NumDetail::OutOfRange { value: 1 << 41 }.to_string()
    );
    assert_eq!(error.remedies(), [Remedy::ChangeFrame]);
    assert!(error.is_replayable());
    let source = error.source().unwrap();
    assert_eq!(source.to_string(), error.detail().to_string());
}

#[test]
fn an_error_keeps_what_the_operation_adds_in_order() {
    let (a, b, c) = (
        NameId::from_bytes([3; 16]),
        NameId::from_bytes([1; 16]),
        NameId::from_bytes([2; 16]),
    );
    let gap = Evidence {
        what: "gap",
        value: Value::Count(4),
    };
    let station = Evidence {
        what: "station",
        value: Value::Coordinate(-3),
    };
    let error = OpError::new(path(), RunDetail::Internal { what: "w" })
        .with_entities([a, b])
        .with_entities([c])
        .with_evidence(gap)
        .with_evidence(station)
        .with_remedy(Remedy::SplitCall)
        .with_message("the cut missed the wall");
    assert_eq!(error.entities(), [a, b, c]);
    assert_eq!(error.evidence(), [gap, station]);
    assert_eq!(error.remedies(), [Remedy::Report, Remedy::SplitCall]);
    assert_eq!(error.message(), "the cut missed the wall");
    assert_eq!(error.kind(), ErrorKind::Internal);
    assert_eq!(
        error.to_string(),
        format!("Internal at {}: the cut missed the wall", "ab".repeat(16))
    );
}

#[test]
fn a_stopped_error_is_not_replayable() {
    for (detail, replayable) in [
        (RunDetail::Cancelled, false),
        (RunDetail::OutOfMemory, false),
        (RunDetail::BudgetExceeded(exceeded()), true),
        (RunDetail::Internal { what: "w" }, true),
    ] {
        assert_eq!(
            OpError::new(path(), detail).is_replayable(),
            replayable,
            "{detail:?}"
        );
    }
}

// Evidence and remedies.

#[test]
fn evidence_prints_its_name_and_value() {
    let cases = [
        (Value::Point2(P2::new(1, -2).unwrap()), "(1, -2) steps"),
        (
            Value::Point3(P3::new(1, -2, 3).unwrap()),
            "(1, -2, 3) steps",
        ),
        (Value::Coordinate(-7), "-7 steps"),
        (
            Value::Length {
                steps: Steps(64),
                step: GridStep::Q1,
            },
            "64 steps of Q1",
        ),
        (
            Value::Parameter(Interval::new(0.25, 0.5).unwrap()),
            "[0.25, 0.5]",
        ),
        (Value::Exactness(Exactness::Exact), "Exact"),
        (Value::Count(12), "12"),
    ];
    for (value, text) in cases {
        assert_eq!(value.to_string(), text);
        let evidence = Evidence {
            what: "here",
            value,
        };
        assert_eq!(evidence.to_string(), format!("here: {text}"));
    }
}

#[test]
fn remedies_print_as_instructions() {
    let cases = [
        (
            Remedy::ChangeInput {
                input: "radius",
                to: None,
            },
            "change `radius`".to_owned(),
        ),
        (
            Remedy::ChangeInput {
                input: "radius",
                to: Some((Bound::AtMost, Value::Count(4))),
            },
            "set `radius` to at most 4".to_owned(),
        ),
        (
            Remedy::ChangeInput {
                input: "radius",
                to: Some((Bound::AtLeast, Value::Count(4))),
            },
            "set `radius` to at least 4".to_owned(),
        ),
        (
            Remedy::ChangeInput {
                input: "radius",
                to: Some((Bound::Exactly, Value::Count(4))),
            },
            "set `radius` to exactly 4".to_owned(),
        ),
        (
            Remedy::Choose {
                candidates: vec![NameId::from_bytes([1; 16]), NameId::from_bytes([2; 16])],
            },
            format!(
                "choose one of {} {}, or add what decides",
                "01".repeat(16),
                "02".repeat(16)
            ),
        ),
        (
            Remedy::UseRoute {
                route: "mesh boolean",
            },
            "use `mesh boolean`".to_owned(),
        ),
        (
            Remedy::Accept {
                class: Exactness::Exact,
            },
            "accept Exact explicitly".to_owned(),
        ),
        (
            Remedy::Heal {
                gap: Steps(640),
                step: GridStep::Q0,
            },
            "heal first, closing gaps up to 640 steps of Q0".to_owned(),
        ),
        (
            Remedy::ChangeFrame,
            "work in a frame placed where the data overlaps, or split it across frames".to_owned(),
        ),
        (
            Remedy::CoarserStep { step: GridStep::Q0 },
            "use the coarser grid step Q0".to_owned(),
        ),
        (
            Remedy::RaiseBudget {
                kind: BudgetKind::Work,
                at_least: 9,
            },
            "raise the work budget to at least 9".to_owned(),
        ),
        (
            Remedy::SplitCall,
            "split the call into smaller ones".to_owned(),
        ),
        (Remedy::Preview, "ask for a preview precision".to_owned()),
        (
            Remedy::AwaitPiece { piece: "G227" },
            "wait for piece G227, which adds a route".to_owned(),
        ),
        (
            Remedy::Report,
            "report it: this is a defect in the kernel".to_owned(),
        ),
    ];
    for (remedy, text) in cases {
        assert_eq!(remedy.to_string(), text);
    }
    assert_eq!(Bound::AtMost.to_string(), "at most");
}

// Outputs.

/// The longest diagnostic code, and one byte more.
const LONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
const TOO_LONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

#[test]
fn a_diagnostic_code_is_lowercase_ascii_digits_dots_and_underscores() {
    for good in [
        "a",
        "z",
        "0",
        "9",
        ".",
        "_",
        "bool.sliver",
        "sweep.short_edge2",
    ] {
        let d = Diagnostic::new(good, "m").unwrap();
        assert_eq!(d.code, good);
        assert_eq!(d.message, "m");
        assert_eq!(d.entities, []);
        assert_eq!(d.at, []);
    }
    assert_eq!((LONG.len(), TOO_LONG.len()), (64, 65));
    assert!(Diagnostic::new(LONG, "m").is_some());
    let too_long = TOO_LONG;
    for bad in [
        "",
        "A",
        "bool.Sliver",
        "a-b",
        "a b",
        "caf\u{e9}",
        "a/b",
        too_long,
    ] {
        assert_eq!(Diagnostic::new(bad, "m"), None, "{bad:?}");
    }
}

#[test]
fn a_diagnostic_keeps_its_entities_and_evidence_in_order() {
    let (a, b) = (NameId::from_bytes([9; 16]), NameId::from_bytes([1; 16]));
    let e = Evidence {
        what: "length",
        value: Value::Length {
            steps: Steps(3),
            step: GridStep::Q0,
        },
    };
    let d = Diagnostic::new("sweep.short_edge", "an edge of 3 steps")
        .unwrap()
        .with_entities([a])
        .with_entities([b])
        .with_evidence(e)
        .with_evidence(e);
    assert_eq!(d.entities, [a, b]);
    assert_eq!(d.at, [e, e]);
    assert_eq!(d.to_string(), "sweep.short_edge: an edge of 3 steps");
}

#[test]
fn an_output_carries_its_class_and_its_warnings_in_order() {
    let certified = Exactness::Certified {
        bound: Steps(64),
        step: GridStep::Q0,
    };
    let output = OpOutput::new("bodies", certified);
    assert_eq!(output.result, "bodies");
    assert_eq!(output.exactness, certified);
    assert_eq!(output.diagnostics, []);
    let first = Diagnostic::new("a.first", "1").unwrap();
    let second = Diagnostic::new("a.second", "2").unwrap();
    let output = output
        .with_diagnostic(first.clone())
        .with_diagnostic(second.clone());
    assert_eq!(output.diagnostics, [first, second]);
}

#[test]
fn a_class_is_its_exactness_without_the_bound() {
    assert_eq!(Class::of(Exactness::Exact), Class::Exact);
    assert_eq!(
        Class::of(Exactness::Certified {
            bound: Steps(1),
            step: GridStep::Q0
        }),
        Class::Certified
    );
    assert_eq!(
        Class::of(Exactness::Approximate {
            deviation: Steps(1),
            step: GridStep::Q2
        }),
        Class::Approximate
    );
}

#[test]
fn a_cancel_token_through_the_context_is_the_callers() {
    // The context hands back the caller's own cancellation, not a copy of its state.
    let token = CancelToken::new();
    let ctx = OpContext::new(path(), Budget::UNLIMITED).with_cancel(&token);
    let seen: &dyn Cancel = ctx.cancel().unwrap();
    token.cancel();
    assert!(seen.requested());
}
