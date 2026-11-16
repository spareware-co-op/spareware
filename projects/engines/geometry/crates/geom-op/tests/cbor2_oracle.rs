//! Piece G86's out-of-process cross-check with Python's cbor2 (MIT; chapter 28.10's oracles). The
//! `cbor2` job of `kernel-oracles.yml` runs three steps:
//!
//! 1. `write_the_cases` (ignored) draws the run's values of SKG's subset from `SPAREKIT_SEED`
//!    and writes each to the file named by `CBOR2_CASES`, one line each: the kernel's encoding in
//!    hexadecimal, a tab, and the value in the JSON form of `common` (`common::to_json`).
//! 2. `oracles/cbor2.sh` installs cbor2 6.1.5, pinned by hash, in a container pinned by digest,
//!    and `oracles/cbor2.py` writes, for each line, cbor2's own canonical encoding of the value and
//!    whether cbor2 decodes the kernel's bytes back to it, to the file named by `CBOR2_ANSWERS`.
//! 3. `compare_with_the_answers` (ignored) draws the same values again and fails on any value
//!    whose encoding differs from cbor2's, or whose bytes cbor2 reads as another value.
//!
//! cbor2's canonical mode sorts map keys by length first (RFC 8949 section 4.2.3), which is
//! section 4.2.1's bytewise order for unsigned keys, the only map keys SKG writes (24.3), so the two
//! must agree byte for byte on every value. `the_comparison_runs_on_our_own_bytes` runs steps 1
//! and 3 on every build with the kernel's own bytes as the answers, then with one answer moved,
//! so the comparison itself is tested where cbor2 is not installed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure stops the test"
)]

mod common;

use std::fmt::Write as _;

use common::{V, from_json, hex, ours, to_json};
use geom_test::seed::{Seed, resolve};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;

/// Values per night.
const CASES: u32 = 4000;

/// The night's values: the same for the same seed on every target.
fn values(seed: Seed) -> Vec<V> {
    let mut runner = TestRunner::new(geom_test::prop::config(seed, CASES));
    let strategy = common::value();
    (0..CASES)
        .map(|_| strategy.new_tree(&mut runner).unwrap().current())
        .collect()
}

/// The cases file: each value's encoding and its JSON form.
fn cases(values: &[V]) -> String {
    let mut out = String::new();
    for v in values {
        writeln!(out, "{}\t{}", hex(&ours(v)), to_json(v)).unwrap();
    }
    out
}

/// Every line of `answers` that disagrees with the kernel, as `case <n>: <why>`.
fn disagreements(values: &[V], answers: &str) -> Vec<String> {
    let lines: Vec<&str> = answers.lines().collect();
    let mut out = Vec::new();
    if lines.len() != values.len() {
        out.push(format!(
            "{} answers for {} cases",
            lines.len(),
            values.len()
        ));
    }
    for (n, (v, line)) in values.iter().zip(&lines).enumerate() {
        let ours = hex(&ours(v));
        match line.split_once('\t') {
            Some((theirs, "same")) if theirs == ours => {}
            Some((theirs, read)) => out.push(format!(
                "case {n}: ours {ours}, cbor2's {theirs}, cbor2 reads ours as the value: {read}; \
                 value {}",
                to_json(v)
            )),
            None => out.push(format!("case {n}: malformed answer {line:?}")),
        }
    }
    out
}

/// The seed: `SPAREKIT_SEED` at night, the fixed default otherwise.
fn seed() -> Seed {
    resolve("cbor2 oracle", Seed(0x6386_cb02)).unwrap()
}

#[test]
#[ignore = "kernel-oracles.yml's cbor2 job's first step"]
fn write_the_cases() {
    let path = std::env::var("CBOR2_CASES").expect("CBOR2_CASES names the cases file");
    std::fs::write(path, cases(&values(seed()))).unwrap();
}

#[test]
#[ignore = "kernel-oracles.yml's cbor2 job's last step"]
fn compare_with_the_answers() {
    let path = std::env::var("CBOR2_ANSWERS").expect("CBOR2_ANSWERS names the answers file");
    let answers = std::fs::read_to_string(path).unwrap();
    let wrong = disagreements(&values(seed()), &answers);
    assert!(
        wrong.is_empty(),
        "{} of {CASES} cases disagree with cbor2:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn the_comparison_runs_on_our_own_bytes() {
    let values = values(Seed(0x6386_cb03));
    let text = cases(&values);
    assert_eq!(text.lines().count(), values.len());
    let mut answers: Vec<String> = text
        .lines()
        .map(|line| format!("{}\tsame", line.split_once('\t').unwrap().0))
        .collect();
    assert_eq!(
        disagreements(&values, &(answers.join("\n") + "\n")),
        Vec::<String>::new()
    );

    // One answer moved by one byte, one read back as another value, and one missing: each found.
    answers[7] = format!("00{}", answers[7]);
    answers[9] = answers[9].replace("\tsame", "\tdiffers");
    answers.pop();
    let wrong = disagreements(&values, &answers.join("\n"));
    assert_eq!(wrong.len(), 3, "{wrong:#?}");
    assert!(wrong[0].contains("answers for"));
    assert!(wrong[1].starts_with("case 7:"));
    assert!(wrong[2].starts_with("case 9:"));
}

/// The JSON form the oracle reads (`common::to_json`) reads back, as text, to a value with the same
/// encoding, for one value of every kind. A small test that Miri finishes, so `serde_json`,
/// whose unsafe code the kernel's tests use, is driven under Miri (32.58).
#[test]
fn the_json_form_reads_back_as_text_for_every_kind() {
    let v = V::A(vec![
        V::U(u64::MAX),
        V::I(-(1_i128 << 100)),
        V::F(-0.0),
        V::B(vec![0, 255, 7]),
        V::T("e\u{301}".to_owned()),
        V::M(vec![(1, V::F(f64::MIN_POSITIVE)), (24, V::U(0))]),
        V::Ci(vec![i64::MIN, 0]),
        V::Cf(vec![1.5, -2.25]),
        V::Set(vec![V::U(1), V::U(2)]),
        V::Sorted(vec![V::U(1), V::U(1)]),
    ]);
    let text = serde_json::to_string(&to_json(&v)).unwrap();
    let back = from_json(&serde_json::from_str(&text).unwrap());
    assert_eq!(hex(&ours(&back)), hex(&ours(&v)), "{text}");
}
