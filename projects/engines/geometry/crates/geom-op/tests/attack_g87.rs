//! Attacks on piece G87's claims in `geom-op`: every `TopoName` variant encodes to the bytes
//! 32.19 fixes and hashes to the `NameId` an independent implementation computes; a name's
//! encoding does not depend on the order its sets were given in; text that is not NFC, sets with
//! repeats and ordinals past their count are refused.
//!
//! The truth: cbor2 6.1.5's canonical encoding of the same values (arrays, byte strings, text and
//! unsigned integers, whose deterministic forms RFC 8949 fixes) and Python's `hashlib` SHA-256 over
//! `SKG NameScheme v1`, a zero byte and those bytes, computed once off CI and written here. The
//! same constants are checked on every target, WebAssembly included, so the names agree everywhere.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failing test should stop loudly"
)]

use geom_op::name::{
    EdgeOrVertex, NAME_SCHEME_V1_PREFIX, NameError, NameList, NameSet, Ordinal, PieceKey, Role,
    RoleArg, Side, TopoName, name_id,
};
use geom_op::{NameId, OpId, OpPath};

const OP: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const N1: NameId = NameId::from_bytes([0x11; 16]);
const N2: NameId = NameId::from_bytes([0x22; 16]);
const N3: NameId = NameId::from_bytes([0x33; 16]);

/// The vectors: each name's canonical bytes and `NameId`, from cbor2 and hashlib.
const VECTORS: [(&str, &str, &str); 8] = [
    (
        "created_cap",
        "840083500102030405060708090a0b0c0d0e0f10676578747275646550aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa81696361702e73746172748250111111111111111111111111111111115022222222222222222222222222222222",
        "76ce22d041a419d049c3ea5ed6b979fd",
    ),
    (
        "created_side",
        "840081500102030405060708090a0b0c0d0e0f108364736964655011111111111111111111111111111111038250111111111111111111111111111111115011111111111111111111111111111111",
        "7989e0f8b58ab54e1d8cfcce4fdb00c0",
    ),
    (
        "created_e_acute",
        "840082500102030405060708090a0b0c0d0e0f10636375748162c3a980",
        "d3b8f9fc65231a408fa046b446fedd75",
    ),
    (
        "split",
        "840183500102030405060708090a0b0c0d0e0f10676578747275646550aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa50111111111111111111111111111111118300825022222222222222222222222222222222503333333333333333333333333333333301",
        "7cd648f288d7f3c08a9ed7d18c47fe80",
    ),
    (
        "merged",
        "830283500102030405060708090a0b0c0d0e0f10676578747275646550aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa8250111111111111111111111111111111115022222222222222222222222222222222",
        "f8b8bca7f4a014f6a58c9ae8c3a31cfa",
    ),
    (
        "bounded_edge",
        "8403008250111111111111111111111111111111115022222222222222222222222222222222815033333333333333333333333333333333",
        "3a131e5db74106f1c4b9a03f85d607f8",
    ),
    (
        "bounded_vertex",
        "8403018350111111111111111111111111111111115022222222222222222222222222222222503333333333333333333333333333333380",
        "b8d4aefe1886b4a9b4d4a066fa065a17",
    ),
    (
        "weak",
        "83045011111111111111111111111111111111820205",
        "e38946e9bcfac68e51609033c04de1c2",
    ),
];

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes(OP))
        .step("extrude")
        .unwrap()
        .item([0xaa; 16])
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

/// Each vector's name, built through the public interface with its sets given out of order.
fn name(case: &str) -> TopoName {
    match case {
        "created_cap" => TopoName::Created {
            op: path(),
            role: Role::new("cap.start", []).unwrap(),
            from: NameList::new([N2, N1]),
        },
        "created_side" => TopoName::Created {
            op: OpPath::new(OpId::from_bytes(OP)),
            role: Role::new("side", [RoleArg::Name(N1), RoleArg::Count(3)]).unwrap(),
            from: NameList::new([N1, N1]),
        },
        "created_e_acute" => TopoName::Created {
            op: OpPath::new(OpId::from_bytes(OP)).step("cut").unwrap(),
            role: Role::new("\u{e9}", []).unwrap(),
            from: NameList::new([]),
        },
        "split" => TopoName::Split {
            op: path(),
            of: N1,
            piece: PieceKey::ByTools {
                tools: set(&[N3, N2]),
                side: Side::Front,
            },
        },
        "merged" => TopoName::Merged {
            op: path(),
            of: set(&[N2, N1]),
        },
        "bounded_edge" => TopoName::Bounded {
            kind: EdgeOrVertex::Edge,
            by: set(&[N2, N1]),
            ends: set(&[N3]),
        },
        "bounded_vertex" => TopoName::Bounded {
            kind: EdgeOrVertex::Vertex,
            by: set(&[N3, N1, N2]),
            ends: set(&[]),
        },
        "weak" => TopoName::Weak {
            base: N1,
            ord: Ordinal::new(2, 5).unwrap(),
        },
        other => panic!("no case {other}"),
    }
}

#[test]
fn every_variant_encodes_and_hashes_as_the_independent_oracle_does() {
    for (case, bytes, id) in VECTORS {
        let name = name(case);
        let encoded = name.encode().unwrap();
        assert_eq!(hex(encoded.as_bytes()), bytes, "{case}");
        assert_eq!(hex(&name.id().unwrap().to_bytes()), id, "{case}");
        assert_eq!(name_id(encoded.as_bytes()), name.id().unwrap(), "{case}");
    }
}

#[test]
fn the_prefix_is_the_seventeen_bytes_and_a_zero() {
    assert_eq!(NAME_SCHEME_V1_PREFIX, b"SKG NameScheme v1\0");
    assert_eq!(NAME_SCHEME_V1_PREFIX.len(), 18);
    // The empty encoding hashes to SHA-256 of the prefix alone, cut to 16 bytes (hashlib).
    assert_eq!(
        hex(&name_id(&[]).to_bytes()),
        "f485d9b863fca0706cbf9eabb487dd72"
    );
}

#[test]
fn sets_and_lists_are_in_name_order_whatever_order_they_were_given_in() {
    let names = [
        N3,
        N1,
        N2,
        NameId::from_bytes([0; 16]),
        NameId::from_bytes([0xff; 16]),
    ];
    let sorted = {
        let mut s = names.to_vec();
        s.sort();
        s
    };
    for rotation in 0..names.len() {
        let mut given = names.to_vec();
        given.rotate_left(rotation);
        assert_eq!(
            NameSet::new(given.iter().copied()).unwrap().names(),
            &sorted[..]
        );
        given.reverse();
        assert_eq!(NameList::new(given.iter().copied()).names(), &sorted[..]);
    }
    // A list keeps repeats; a set refuses them.
    assert_eq!(NameList::new([N2, N1, N2]).names(), &[N1, N2, N2]);
    assert_eq!(NameSet::new([N2, N1, N2]), Err(NameError::Repeated));
    assert_eq!(NameSet::new([N1, N1]), Err(NameError::Repeated));
    assert_eq!(NameSet::new([]).unwrap().names().len(), 0);
}

#[test]
fn role_names_must_be_nfc_and_not_empty() {
    // U+00E9 is NFC; e followed by U+0301, the same letter decomposed, is not.
    assert!(Role::new("\u{e9}", []).is_ok());
    assert_eq!(Role::new("e\u{301}", []), Err(NameError::NotNfc));
    // A Hangul syllable is NFC; its jamo are not.
    assert!(Role::new("\u{d55c}", []).is_ok());
    assert_eq!(
        Role::new("\u{1112}\u{1161}\u{11ab}", []),
        Err(NameError::NotNfc)
    );
    // The angstrom sign normalises to U+00C5, so it is not NFC.
    assert_eq!(Role::new("\u{212b}", []), Err(NameError::NotNfc));
    assert_eq!(Role::new("", []), Err(NameError::EmptyRole));
    let role = Role::new("roll", [RoleArg::Name(N2), RoleArg::Count(0)]).unwrap();
    assert_eq!(role.name(), "roll");
    assert_eq!(role.args(), &[RoleArg::Name(N2), RoleArg::Count(0)]);
}

#[test]
fn ordinals_count_from_zero_below_their_count() {
    assert_eq!(Ordinal::new(0, 0), Err(NameError::Ordinal));
    assert_eq!(Ordinal::new(5, 5), Err(NameError::Ordinal));
    assert_eq!(Ordinal::new(6, 5), Err(NameError::Ordinal));
    let last = Ordinal::new(u32::MAX - 1, u32::MAX).unwrap();
    assert_eq!((last.n(), last.of()), (u32::MAX - 1, u32::MAX));
    let all: Vec<(u32, u32)> = Ordinal::all(4).map(|o| (o.n(), o.of())).collect();
    assert_eq!(all, vec![(0, 4), (1, 4), (2, 4), (3, 4)]);
    assert_eq!(Ordinal::all(0).count(), 0);
}

#[test]
fn every_field_changes_the_name() {
    let base = name("split");
    let base_id = base.id().unwrap();
    let variants = [
        TopoName::Split {
            op: OpPath::new(OpId::from_bytes(OP)),
            of: N1,
            piece: PieceKey::ByTools {
                tools: set(&[N3, N2]),
                side: Side::Front,
            },
        },
        TopoName::Split {
            op: path(),
            of: N2,
            piece: PieceKey::ByTools {
                tools: set(&[N3, N2]),
                side: Side::Front,
            },
        },
        TopoName::Split {
            op: path(),
            of: N1,
            piece: PieceKey::ByTools {
                tools: set(&[N3]),
                side: Side::Front,
            },
        },
        TopoName::Split {
            op: path(),
            of: N1,
            piece: PieceKey::ByTools {
                tools: set(&[N3, N2]),
                side: Side::Back,
            },
        },
    ];
    for v in variants {
        assert_ne!(v.id().unwrap(), base_id, "{v:?}");
    }
    let weak = |n, of| TopoName::Weak {
        base: N1,
        ord: Ordinal::new(n, of).unwrap(),
    };
    // `of` is part of the name: the same n of a different count is a different name (20.2).
    assert_ne!(weak(1, 3).id().unwrap(), weak(1, 4).id().unwrap());
    assert_ne!(weak(1, 3).id().unwrap(), weak(2, 3).id().unwrap());
    let error = NameError::Encode(geom_op::cbor::EncodeError::TooLong);
    assert!(
        error
            .to_string()
            .starts_with("the name could not be encoded: ")
    );
    for e in [
        NameError::EmptyRole,
        NameError::NotNfc,
        NameError::Repeated,
        NameError::Ordinal,
    ] {
        assert_ne!(e.to_string(), "");
    }
}
