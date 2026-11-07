//! Tests of the canonical encodings of what an operation reports (piece G96), in `geom-op` itself
//! so its mutation run sees them; `geom-test`'s `tests/canonical.rs` tests the hash over them.
//!
//! What this file proves: each encoding is exactly the bytes its doc comment states, for every
//! class, step and kind of value, and lists hold every item in the operation's order; words are
//! left out.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, NumError, P2, P3, Steps};
use geom_op::canonical::{encode_diagnostic, encode_error, encode_evidence, encode_exactness};
use geom_op::cbor::{EncodeError, Item};
use geom_op::{Diagnostic, Evidence, NameId, OpError, OpId, OpPath, Value};

fn n(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0x96; 16]))
}

fn bytes(item: Result<Item, EncodeError>) -> Vec<u8> {
    item.unwrap().into_bytes()
}

#[test]
fn every_exactness_class_and_step() {
    assert_eq!(bytes(encode_exactness(&Exactness::Exact)), [0x81, 0x00]);
    for (step, code) in [(GridStep::Q0, 0), (GridStep::Q1, 1), (GridStep::Q2, 2)] {
        assert_eq!(
            bytes(encode_exactness(&Exactness::Certified {
                bound: Steps(64),
                step,
            })),
            [0x83, 0x01, 0x18, 0x40, code]
        );
        assert_eq!(
            bytes(encode_exactness(&Exactness::Approximate {
                deviation: Steps(2),
                step,
            })),
            [0x83, 0x02, 0x02, code]
        );
    }
}

#[test]
fn every_kind_of_evidence() {
    let cases: [(Value, &[u8]); 7] = [
        (
            Value::Point2(P2::new(1, -2).unwrap()),
            &[0x83, 0x00, 0x01, 0x21],
        ),
        (
            Value::Point3(P3::new(1, -2, 3).unwrap()),
            &[0x84, 0x01, 0x01, 0x21, 0x03],
        ),
        (Value::Coordinate(-1), &[0x82, 0x02, 0x20]),
        (
            Value::Length {
                steps: Steps(3),
                step: GridStep::Q2,
            },
            &[0x83, 0x03, 0x03, 0x02],
        ),
        (
            Value::Parameter(Interval::new(0.5, 1.0).unwrap()),
            &[0x83, 0x04, 0xf9, 0x38, 0x00, 0xf9, 0x3c, 0x00],
        ),
        (
            Value::Exactness(Exactness::Exact),
            &[0x82, 0x05, 0x81, 0x00],
        ),
        (Value::Count(7), &[0x82, 0x06, 0x07]),
    ];
    for (value, tail) in cases {
        let mut want = vec![0x82, 0x61, b'x'];
        want.extend_from_slice(tail);
        assert_eq!(
            bytes(encode_evidence(&Evidence { what: "x", value })),
            want,
            "{value:?}"
        );
    }
}

/// `count` lists of the given names and evidence, with their heads.
fn lists(names: &[u8], counts: &[u8]) -> Vec<u8> {
    let mut want = vec![0x80 + names.len() as u8];
    for name in names {
        want.push(0x50);
        want.extend([*name; 16]);
    }
    want.push(0x80 + counts.len() as u8);
    for count in counts {
        want.extend([0x82, 0x61, b'x', 0x82, 0x06, *count]);
    }
    want
}

#[test]
fn a_warning_holds_every_entity_and_value_in_order() {
    for (names, counts) in [
        (&[][..], &[][..]),
        (&[1][..], &[4][..]),
        (&[2, 1, 3][..], &[5, 4][..]),
    ] {
        let mut d = Diagnostic::new("a.b", "words")
            .unwrap()
            .with_entities(names.iter().map(|x| n(*x)));
        for count in counts {
            d = d.with_evidence(Evidence {
                what: "x",
                value: Value::Count(u64::from(*count).into()),
            });
        }
        let mut want = vec![0x83, 0x63, b'a', b'.', b'b'];
        want.extend(lists(names, counts));
        assert_eq!(bytes(encode_diagnostic(&d)), want);
    }
    // The message is words, and is left out.
    let one = Diagnostic::new("a.b", "one").unwrap();
    let two = Diagnostic::new("a.b", "two").unwrap();
    assert_eq!(
        bytes(encode_diagnostic(&one)),
        bytes(encode_diagnostic(&two))
    );
}

#[test]
fn an_error_holds_its_kind_entities_and_evidence() {
    let plain = OpError::new(path(), NumError::Degenerate);
    let mut want = vec![0x83, 0x6a];
    want.extend(b"Degenerate");
    let head = want.clone();
    want.extend(lists(&[], &[]));
    assert_eq!(bytes(encode_error(&plain)), want);
    let full = OpError::new(path(), NumError::Degenerate)
        .with_entities([n(9), n(8)])
        .with_evidence(Evidence {
            what: "x",
            value: Value::Count(1),
        })
        .with_evidence(Evidence {
            what: "x",
            value: Value::Count(2),
        });
    let mut want = head;
    want.extend(lists(&[9, 8], &[1, 2]));
    assert_eq!(bytes(encode_error(&full)), want);
    // Words are left out.
    assert_eq!(
        bytes(encode_error(&plain.clone().with_message("other words"))),
        bytes(encode_error(&plain))
    );
}
