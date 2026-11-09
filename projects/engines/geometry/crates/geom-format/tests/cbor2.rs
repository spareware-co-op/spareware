//! The cross-check with cbor2: an independent CBOR implementation's verdict on every committed
//! input, and its text form of every accepted one.
//!
//! `tests/cbor2/generate.py` wrote the vectors with cbor2 6.1.5 (MIT,
//! <https://github.com/agronholm/cbor2>), which says whether bytes are SKG's deterministic CBOR by
//! decoding them leniently and comparing them with the canonical encoding of what it decoded (its
//! own, with map pairs in bytewise key order). `vectors.jsonl` holds random canonical values, other
//! encodings of them, and mutations, with the text form for accepted ones by the script's own
//! printer; `short.txt` holds the verdict for every input of one and two bytes. The decoder must
//! agree on every verdict, and its text form on every accepted input.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_format::cbor::{check, from_text, to_text};
use geom_run::{Budget, WorkMeter};

/// The generator's depth limit.
const DEPTH: usize = 128;

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[test]
fn every_verdict_and_text_matches_cbor2() {
    let vectors = include_str!("cbor2/vectors.jsonl");
    let mut accepted = 0;
    for (n, line) in vectors.lines().enumerate() {
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        let input = bytes(record["hex"].as_str().unwrap());
        let ok = record["ok"].as_bool().unwrap();
        let ours = check(&input, DEPTH, &mut meter());
        assert_eq!(ours.is_ok(), ok, "line {}: {line}: {ours:?}", n + 1);
        if ok {
            accepted += 1;
            let want = record["text"].as_str().unwrap();
            assert_eq!(
                to_text(&input, DEPTH, &mut meter()).unwrap(),
                want,
                "line {}",
                n + 1
            );
            assert_eq!(
                from_text(want, DEPTH, &mut meter()),
                Ok(input),
                "line {}",
                n + 1
            );
        }
    }
    assert!(accepted > 500, "{accepted}");
}

#[test]
fn every_one_and_two_byte_input_matches_cbor2() {
    let short = include_str!("cbor2/short.txt");
    let mut lines = 0;
    for line in short.lines() {
        let (hex, verdict) = line.split_once(' ').unwrap();
        let ours = check(&bytes(hex), DEPTH, &mut meter());
        assert_eq!(ours.is_ok(), verdict == "ok", "{hex}: {ours:?}");
        lines += 1;
    }
    // Every input of one or two bytes but the few whose maps cbor2 cannot judge.
    assert!(lines > 65_000, "{lines}");
}
