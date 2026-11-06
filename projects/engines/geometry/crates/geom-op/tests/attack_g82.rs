//! Attacks on every claim piece G82 makes about the operation contract.
//!
//! What this file proves:
//!
//! - **No clock and no randomness is reachable.** The crate's kernel dependencies are exactly
//!   `geom-run` and `geom-num` (chapter 27's row), and no code token of the crate names a clock,
//!   a random source or `geom-num`'s seeded generator; the context holds only its path, budget,
//!   precision and the caller's cancellation, and its `Debug` shows nothing else.
//! - **The kind always agrees with the detail**, whatever the operation adds to the error, for
//!   every detail of every family this piece builds.
//! - **The numbers layer maps by meaning, completely.** Every `NumError` variant (the match below
//!   stops compiling when one is added) lands on the base kind chapter 22 and this piece decide;
//!   in particular an exceeded bit budget is never `BudgetExceeded` and an exceeded ceiling never
//!   `ToleranceExceeded`.
//! - **The map's error catalogue is the code's.** For every detail of this crate, the base kind
//!   `MAP.json` lists is the one its `kind` function returns, and `ErrorKind`'s kinds are listed in
//!   order, not `#[non_exhaustive]`, while `OpErrorDetail` is.
//! - **Paths never depend on position.** A batch item's path depends only on the batch's path and
//!   the item's key; paths, IDs and names order as their bytes and labels do (property tests with
//!   a fixed seed).
//! - **Step labels are NFC under every Unicode version**: every label accepted is ASCII.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use geom_num::NumError;
use geom_op::{
    Bound, ErrorKind, Evidence, Label, NameId, NumDetail, OpContext, OpError, OpErrorDetail, OpId,
    OpPath, Precision, Remedy, RunDetail, Value,
};
use geom_run::{Budget, BudgetKind, CancelToken, Exceeded, Phase, Usage};
use geom_test::map::rust::{Kind, tokens};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

/// Cases per property, with a fixed seed and no failure files.
fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_6782),
        failure_persistence: None,
        ..Config::default()
    }
}

/// The crate's folder, where the file system can be read (not on WebAssembly without a
/// preopened folder).
fn crate_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("Cargo.toml").is_file().then_some(dir)
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every detail this piece builds, one of each variant.
fn every_detail() -> Vec<OpErrorDetail> {
    let exceeded = Exceeded {
        budget: Budget::new(1, 2),
        kind: BudgetKind::Work,
        phase: Phase::new("attack"),
        used: Usage::new(2, 0),
    };
    let mut details: Vec<OpErrorDetail> = [
        RunDetail::BudgetExceeded(exceeded),
        RunDetail::BatchTotal(exceeded),
        RunDetail::Cancelled,
        RunDetail::OutOfMemory,
        RunDetail::Internal { what: "w" },
    ]
    .into_iter()
    .map(OpErrorDetail::from)
    .collect();
    details.extend(every_num_error().into_iter().map(OpErrorDetail::from));
    details
}

fn every_num_error() -> Vec<NumError> {
    vec![
        NumError::OutOfRange { value: i128::MAX },
        NumError::Degenerate,
        NumError::NotFinite,
        NumError::WrongDirection,
        NumError::DivisorContainsZero,
        NumError::NegativeSqrt,
        NumError::EmptyInterval,
        NumError::OutsideDomain,
        NumError::BudgetExceeded,
        NumError::ToleranceExceeded,
        NumError::UnprovenMaths,
    ]
}

/// The base kind of each numbers-layer error: chapter 22's, and the six this piece decides. The
/// match has no wildcard, so a new `NumError` variant fails to compile here until it is mapped.
fn expected_num_kind(error: NumError) -> ErrorKind {
    match error {
        NumError::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
        NumError::Degenerate | NumError::NotFinite => ErrorKind::Degenerate,
        NumError::OutsideDomain => ErrorKind::NoSolution,
        NumError::DivisorContainsZero | NumError::NegativeSqrt => ErrorKind::Undecided,
        NumError::WrongDirection | NumError::EmptyInterval | NumError::BudgetExceeded => {
            ErrorKind::Internal
        }
        NumError::ToleranceExceeded => ErrorKind::ExactnessNotAccepted,
        NumError::UnprovenMaths => ErrorKind::Unsupported,
    }
}

#[test]
fn no_clock_or_random_source_is_reachable_from_the_crates_code() {
    let Some(dir) = crate_dir() else {
        return;
    };
    // Chapter 27's row, geom-run and geom-num, and the two vetted crates the allow-list lets geom-op
    // name (deny-no-std.toml's registry): sha2 for content IDs and names (G86, 32.20) and
    // unicode-normalization for the NFC check of names (G87, 32.19). Their audits find no clock and
    // no random source (projects/kit/notes/G86-sha2-audit.md, G87-unicode-normalization-audit.md).
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml"))
        .unwrap()
        .replace("\r\n", "\n");
    let section = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap()
        .split("\n[")
        .next()
        .unwrap();
    let deps: Vec<&str> = section
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split([' ', '=']).next().unwrap())
        .collect();
    assert_eq!(
        deps,
        ["geom-run", "geom-num", "sha2", "unicode-normalization"]
    );
    // No code token of the crate's source names a clock or a random source.
    let banned = [
        "rng",
        "Philox",
        "Instant",
        "SystemTime",
        "getrandom",
        "rand",
        "random",
        "RandomState",
        "thread_rng",
        "time",
    ];
    let mut files = Vec::new();
    rust_files(&dir.join("src"), &mut files);
    assert!(files.len() >= 9, "{files:?}");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for t in tokens(&text).unwrap() {
            if t.kind == Kind::Ident {
                assert!(
                    !banned.contains(&t.text.as_str()),
                    "{}: names `{}`",
                    file.display(),
                    t.text
                );
            }
        }
    }
}

#[test]
fn the_context_holds_its_path_budget_precision_and_cancellation_and_nothing_else() {
    let token = CancelToken::new();
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([0; 16])), Budget::new(1, 2))
        .with_cancel(&token)
        .with_precision(Precision::Final);
    let debug = format!("{ctx:?}");
    let fields: Vec<&str> = debug
        .trim_start_matches("OpContext { ")
        .trim_end_matches(" }")
        .split(", ")
        .filter_map(|f| f.split_once(": ").map(|(name, _)| name))
        .filter(|name| !name.contains(' ') && !name.contains('{'))
        .collect();
    assert_eq!(
        fields,
        [
            "path",
            "budget",
            "logical_bytes",
            "cancellable",
            "precision"
        ]
    );
    assert!(!debug.contains("0x"), "{debug}");
    // The same context twice prints the same.
    assert_eq!(debug, format!("{ctx:?}"));
}

#[test]
fn the_kind_agrees_with_the_detail_whatever_the_operation_adds() {
    let path = OpPath::new(OpId::from_bytes([1; 16]))
        .step("attack")
        .unwrap();
    for detail in every_detail() {
        let error = OpError::new(path.clone(), detail)
            .with_entities([NameId::from_bytes([2; 16])])
            .with_evidence(Evidence {
                what: "x",
                value: Value::Count(1),
            })
            .with_remedy(Remedy::ChangeInput {
                input: "x",
                to: Some((Bound::AtMost, Value::Count(0))),
            })
            .with_message("another message");
        assert_eq!(error.kind(), detail.kind(), "{detail:?}");
        assert_eq!(error.detail(), &detail);
        assert_eq!(error.is_replayable(), detail.kind().is_replayable());
        // The detail's own remedies come first, then the operation's.
        let mut remedies = detail.remedies();
        remedies.push(Remedy::ChangeInput {
            input: "x",
            to: Some((Bound::AtMost, Value::Count(0))),
        });
        assert_eq!(error.remedies(), remedies.as_slice());
        // Never empty of meaning.
        assert_ne!(OpError::new(path.clone(), detail).message(), "");
    }
}

#[test]
fn every_numbers_layer_error_maps_by_meaning() {
    for error in every_num_error() {
        let kind = NumDetail::from(error).kind();
        assert_eq!(kind, expected_num_kind(error), "{error:?}");
        assert_eq!(OpErrorDetail::from(error).kind(), kind);
    }
    // The two misleading names never reach their namesake kinds.
    assert_eq!(
        NumDetail::from(NumError::BudgetExceeded).kind(),
        ErrorKind::Internal
    );
    assert_eq!(
        NumDetail::from(NumError::ToleranceExceeded).kind(),
        ErrorKind::ExactnessNotAccepted
    );
    // A saturated coordinate keeps its value.
    assert_eq!(
        NumDetail::from(NumError::OutOfRange { value: i128::MIN }),
        NumDetail::OutOfRange { value: i128::MIN }
    );
}

#[test]
fn the_maps_error_catalogue_is_the_codes() {
    let Some(dir) = crate_dir() else {
        return;
    };
    let map_path = dir.join("../../MAP.json");
    let map: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(map_path).unwrap()).unwrap();
    let krate = map["crates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "geom-op")
        .unwrap();
    let errors = krate["errors"].as_array().unwrap();
    let listed = |name: &str| -> Vec<(String, Option<String>)> {
        errors
            .iter()
            .find(|e| e["name"] == name)
            .unwrap_or_else(|| panic!("MAP.json lists no `{name}`"))["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| {
                (
                    k["name"].as_str().unwrap().to_owned(),
                    k.get("base_kind").map(|b| b.as_str().unwrap().to_owned()),
                )
            })
            .collect()
    };
    let non_exhaustive = |name: &str| {
        errors.iter().find(|e| e["name"] == name).unwrap()["non_exhaustive"]
            .as_bool()
            .unwrap()
    };
    // The base kinds, in chapter 22's order, closed.
    let kinds: Vec<(String, Option<String>)> = ErrorKind::ALL
        .iter()
        .map(|k| (k.name().to_owned(), None))
        .collect();
    assert_eq!(listed("ErrorKind"), kinds);
    assert!(!non_exhaustive("ErrorKind"));
    assert!(non_exhaustive("OpErrorDetail"));
    // Every detail with the base kind its `kind` function returns.
    let mut run = Vec::new();
    let mut num = Vec::new();
    for detail in every_detail() {
        let debug = format!("{detail:?}");
        let variant = debug
            .split(['(', ' ', '{'])
            .nth(1)
            .unwrap()
            .trim_end_matches(')')
            .to_owned();
        let entry = (variant, Some(detail.kind().name().to_owned()));
        match detail {
            OpErrorDetail::Run(_) => run.push(entry),
            OpErrorDetail::Num(_) => num.push(entry),
            _ => panic!("a detail family this test does not know: {detail:?}"),
        }
    }
    // In name order: the map lists variants as declared, the details here come in their source
    // errors' order.
    let sorted = |mut v: Vec<(String, Option<String>)>| {
        v.sort();
        v
    };
    assert_eq!(sorted(listed("RunDetail")), sorted(run));
    assert_eq!(sorted(listed("NumDetail")), sorted(num));
    assert_eq!(
        listed("OpErrorDetail"),
        [
            ("Run".to_owned(), Some("held detail".to_owned())),
            ("Num".to_owned(), Some("held detail".to_owned())),
        ]
    );
}

#[test]
fn every_accepted_step_label_is_ascii() {
    for byte in 0_u8..=255 {
        let one = [byte];
        if let Ok(text) = std::str::from_utf8(&one) {
            let accepted = Label::step(text).is_some();
            assert_eq!(accepted, (b'!'..=b'~').contains(&byte), "{byte:#04x}");
        }
    }
    for c in ['\u{e9}', '\u{301}', '\u{212b}', '\u{fb01}', '\u{3000}'] {
        assert_eq!(Label::step(&format!("a{c}b")), None, "{c:?}");
    }
}

fn op_id() -> impl Strategy<Value = OpId> {
    any::<[u8; 16]>().prop_map(OpId::from_bytes)
}

fn label() -> impl Strategy<Value = Label> {
    prop_oneof![
        "[!-~]([ -~]{0,6}[!-~])?".prop_map(|s| Label::step(&s).unwrap()),
        any::<[u8; 16]>().prop_map(Label::key),
    ]
}

fn op_path() -> impl Strategy<Value = OpPath> {
    (op_id(), prop::collection::vec(label(), 0..4)).prop_map(|(op, labels)| {
        labels
            .into_iter()
            .fold(OpPath::new(op), |path, label| path.child(label))
    })
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn ids_and_names_order_as_their_bytes(a in any::<[u8; 16]>(), b in any::<[u8; 16]>()) {
        prop_assert_eq!(OpId::from_bytes(a).cmp(&OpId::from_bytes(b)), a.cmp(&b));
        prop_assert_eq!(NameId::from_bytes(a).cmp(&NameId::from_bytes(b)), a.cmp(&b));
        prop_assert_eq!(OpId::from_bytes(a) == OpId::from_bytes(b), a == b);
    }

    #[test]
    fn an_items_path_depends_only_on_the_batch_and_its_key(
        batch in op_path(),
        key in any::<[u8; 16]>(),
        others in prop::collection::vec(any::<[u8; 16]>(), 0..8),
    ) {
        let alone = batch.item(key);
        // Whatever other items the batch holds, in whatever order, this item's path is the same.
        for other in &others {
            let _ = batch.item(*other);
            prop_assert_eq!(&batch.item(key), &alone);
        }
        prop_assert!(batch.contains(&alone));
        prop_assert_eq!(alone.labels().len(), batch.labels().len() + 1);
        prop_assert_eq!(alone.labels().last(), Some(&Label::Key(key)));
        prop_assert_eq!(alone.op(), batch.op());
    }

    #[test]
    fn paths_order_by_operation_then_labels(a in op_path(), b in op_path()) {
        let expected = a.op().cmp(&b.op()).then_with(|| a.labels().cmp(b.labels()));
        prop_assert_eq!(a.cmp(&b), expected);
        // Containment is a prefix of the labels within one operation.
        let prefix = a.op() == b.op() && b.labels().starts_with(a.labels());
        prop_assert_eq!(a.contains(&b), prefix);
    }

    #[test]
    fn a_child_is_contained_by_its_parent_and_not_the_other_way(path in op_path(), l in label()) {
        let child = path.child(l);
        prop_assert!(path.contains(&child));
        prop_assert!(!child.contains(&path));
        prop_assert!(path < child);
    }
}
