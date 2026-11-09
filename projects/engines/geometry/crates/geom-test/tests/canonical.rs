//! Tests of the canonical output and its hash (piece G96), through geom-test's and geom-op's public
//! interfaces.
//!
//! What this file proves:
//!
//! - Each part's encoding is exactly the bytes its doc comment states (exactness, evidence,
//!   diagnostics, errors), words left out.
//! - The hash is a pure function of the outcome: computed twice it agrees; names given in any
//!   order agree; a reworded message or other remedies leave an error's hash unchanged, and a
//!   reworded warning leaves an output's hash unchanged.
//! - Every value counts: another kind, entity, piece of evidence, body, name, history, class or
//!   warning changes the hash, and a success never hashes like a failure.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, NumError, P2, P3, Steps};
use geom_op::canonical::{encode_diagnostic, encode_error, encode_evidence, encode_exactness};
use geom_op::{
    Diagnostic, Evidence, History, NameId, NameSet, OpError, OpId, OpPath, Origin, Record, Remedy,
    RunDetail, Value,
};
use geom_test::canonical::{Outcome, PREFIX, canonical_bytes, canonical_output};

fn n(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0x96; 16]))
}

#[test]
fn each_part_encodes_as_documented() {
    let bytes = |item: geom_op::cbor::Item| item.into_bytes();
    assert_eq!(
        bytes(encode_exactness(&Exactness::Exact).unwrap()),
        [0x81, 0x00]
    );
    let certified = Exactness::Certified {
        bound: Steps(64),
        step: GridStep::Q1,
    };
    assert_eq!(
        bytes(encode_exactness(&certified).unwrap()),
        [0x83, 0x01, 0x18, 0x40, 0x01]
    );
    let approximate = Exactness::Approximate {
        deviation: Steps(2),
        step: GridStep::Q2,
    };
    assert_eq!(
        bytes(encode_exactness(&approximate).unwrap()),
        [0x83, 0x02, 0x02, 0x02]
    );
    let ev = |value| Evidence { what: "x", value };
    let cases: [(Value, &[u8]); 7] = [
        (
            Value::Point2(P2::new(1, -2).unwrap()),
            &[0x83, 0x00, 0x01, 0x21],
        ),
        (
            Value::Point3(P3::new(1, -2, 0).unwrap()),
            &[0x84, 0x01, 0x01, 0x21, 0x00],
        ),
        (Value::Coordinate(-1), &[0x82, 0x02, 0x20]),
        (
            Value::Length {
                steps: Steps(3),
                step: GridStep::Q0,
            },
            &[0x83, 0x03, 0x03, 0x00],
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
            bytes(encode_evidence(&ev(value)).unwrap()),
            want,
            "{value:?}"
        );
    }
    let d = Diagnostic::new("a.b", "words")
        .unwrap()
        .with_entities([n(1)])
        .with_evidence(ev(Value::Count(1)));
    let mut want = vec![0x83, 0x63, b'a', b'.', b'b', 0x81, 0x50];
    want.extend([1; 16]);
    want.extend([0x81, 0x82, 0x61, b'x', 0x82, 0x06, 0x01]);
    assert_eq!(bytes(encode_diagnostic(&d).unwrap()), want);
    let error = OpError::new(path(), NumError::Degenerate);
    let mut want = vec![0x83, 0x6a];
    want.extend(b"Degenerate");
    want.extend([0x80, 0x80]);
    assert_eq!(bytes(encode_error(&error).unwrap()), want);
}

fn history() -> History {
    History::new(
        [Record {
            output: n(2),
            origin: Origin::Modified {
                from: NameSet::new([n(1)]).unwrap(),
            },
        }],
        NameSet::default(),
        [],
    )
    .unwrap()
}

fn output_hash(
    bodies: &[[u8; 32]],
    names: &NameSet,
    with_history: bool,
    class: &Exactness,
    diagnostics: &[Diagnostic],
) -> [u8; 32] {
    let h = history();
    let p = path();
    canonical_output(&Outcome::Output {
        bodies,
        names,
        history: with_history.then_some((&p, &h)),
        exactness: class,
        diagnostics,
    })
    .unwrap()
}

#[test]
fn the_hash_is_a_pure_function_of_the_values() {
    let names = NameSet::new([n(3), n(1)]).unwrap();
    let names_again = NameSet::new([n(1), n(3)]).unwrap();
    let warn = |message: &str| {
        Diagnostic::new("bool.sliver", message)
            .unwrap()
            .with_entities([n(5)])
    };
    let a = output_hash(&[[7; 32]], &names, true, &Exactness::Exact, &[warn("one")]);
    assert_eq!(
        a,
        output_hash(
            &[[7; 32]],
            &names_again,
            true,
            &Exactness::Exact,
            &[warn("two")]
        )
    );
    // Every value counts.
    let certified = Exactness::Certified {
        bound: Steps(1),
        step: GridStep::Q0,
    };
    for other in [
        output_hash(&[[8; 32]], &names, true, &Exactness::Exact, &[warn("one")]),
        output_hash(&[], &names, true, &Exactness::Exact, &[warn("one")]),
        output_hash(
            &[[7; 32]],
            &NameSet::default(),
            true,
            &Exactness::Exact,
            &[warn("one")],
        ),
        output_hash(&[[7; 32]], &names, false, &Exactness::Exact, &[warn("one")]),
        output_hash(&[[7; 32]], &names, true, &certified, &[warn("one")]),
        output_hash(&[[7; 32]], &names, true, &Exactness::Exact, &[]),
    ] {
        assert_ne!(other, a);
    }
    // An error: words and remedies left out, values counted.
    let base = OpError::new(path(), RunDetail::Internal { what: "w" })
        .with_entities([n(1)])
        .with_evidence(Evidence {
            what: "gap",
            value: Value::Count(4),
        });
    let hash = |e: &OpError| canonical_output(&Outcome::Error(e)).unwrap();
    let reworded = base
        .clone()
        .with_message("another sentence")
        .with_remedy(Remedy::SplitCall);
    assert_eq!(hash(&reworded), hash(&base));
    assert_eq!(hash(&base), hash(&base.clone()));
    for other in [
        OpError::new(path(), NumError::Degenerate)
            .with_entities([n(1)])
            .with_evidence(Evidence {
                what: "gap",
                value: Value::Count(4),
            }),
        base.clone().with_entities([n(2)]),
        base.clone().with_evidence(Evidence {
            what: "gap",
            value: Value::Count(5),
        }),
    ] {
        assert_ne!(hash(&other), hash(&base));
    }
    // A success never hashes like a failure, and the prefix is hashed.
    assert_ne!(hash(&base), a);
    let bytes = canonical_bytes(&Outcome::Error(&base)).unwrap();
    assert_eq!(bytes[..2], [0x82, 0x01]);
    let mut prefixed = PREFIX.to_vec();
    prefixed.extend(&bytes);
    assert_eq!(geom_test::golden::sha256(&prefixed), hash(&base));
    assert_eq!(PREFIX, b"SKG canonical output v1\0");
}
