//! The canonical CBOR encoder through its public interface: RFC 8949's own examples, each refusal,
//! every binary16 value, and random values of the subset against an independent reference
//! encoding through ciborium-ll (`common`).

#![allow(
    clippy::unreadable_literal,
    clippy::too_many_lines,
    reason = "test cases written as RFC 8949's appendix writes them; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{V, encode, hex, ours, reference, unhex};
use geom_op::cbor::{EncodeError, Encoder, F64_COLUMN_TAG, I64_COLUMN_TAG};
use geom_test::seed::Seed;

/// The bytes the closure's calls write, which must make one whole item.
fn bytes(f: impl FnOnce(&mut Encoder) -> Result<(), EncodeError>) -> String {
    let mut e = Encoder::new();
    f(&mut e).unwrap();
    hex(e.finish().unwrap().as_bytes())
}

/// RFC 8949 Appendix A's examples that lie in SKG's subset, with the encodings it gives.
#[test]
fn rfc_8949_appendix_a() {
    let unsigned: &[(u64, &str)] = &[
        (0, "00"),
        (1, "01"),
        (10, "0a"),
        (23, "17"),
        (24, "1818"),
        (25, "1819"),
        (100, "1864"),
        (1000, "1903e8"),
        (1000000, "1a000f4240"),
        (1000000000000, "1b000000e8d4a51000"),
        (18446744073709551615, "1bffffffffffffffff"),
    ];
    for &(n, want) in unsigned {
        assert_eq!(bytes(|e| e.uint(n)), want, "{n}");
        assert_eq!(bytes(|e| e.int(i128::from(n))), want, "{n}");
    }
    let signed: &[(i128, &str)] = &[
        (18446744073709551616, "c249010000000000000000"),
        (-18446744073709551616, "3bffffffffffffffff"),
        (-18446744073709551617, "c349010000000000000000"),
        (-1, "20"),
        (-10, "29"),
        (-100, "3863"),
        (-1000, "3903e7"),
    ];
    for &(n, want) in signed {
        assert_eq!(bytes(|e| e.int(n)), want, "{n}");
    }
    let floats: &[(f64, &str)] = &[
        (0.0, "f90000"),
        (-0.0, "f98000"),
        (1.0, "f93c00"),
        (1.1, "fb3ff199999999999a"),
        (1.5, "f93e00"),
        (65504.0, "f97bff"),
        (100000.0, "fa47c35000"),
        (3.4028234663852886e+38, "fa7f7fffff"),
        (1.0e+300, "fb7e37e43c8800759c"),
        (5.960464477539063e-8, "f90001"),
        (0.00006103515625, "f90400"),
        (-4.0, "f9c400"),
        (-4.1, "fbc010666666666666"),
    ];
    for &(x, want) in floats {
        assert_eq!(bytes(|e| e.float(x)), want, "{x}");
    }
    assert_eq!(bytes(|e| e.bytes(&[])), "40");
    assert_eq!(bytes(|e| e.bytes(&[1, 2, 3, 4])), "4401020304");
    assert_eq!(bytes(|e| e.text("")), "60");
    assert_eq!(bytes(|e| e.text("a")), "6161");
    assert_eq!(bytes(|e| e.text("IETF")), "6449455446");
    assert_eq!(bytes(|e| e.text("\"\\")), "62225c");
    assert_eq!(bytes(|e| e.text("\u{00fc}")), "62c3bc");
    assert_eq!(bytes(|e| e.text("\u{6c34}")), "63e6b0b4");
    assert_eq!(bytes(|e| e.text("\u{10151}")), "64f0908591");
    assert_eq!(bytes(|e| e.array(0)), "80");
    assert_eq!(
        bytes(|e| {
            e.array(3)?;
            (1..=3).try_for_each(|n| e.uint(n))
        }),
        "83010203"
    );
    assert_eq!(
        bytes(|e| {
            e.array(3)?;
            e.uint(1)?;
            e.array(2)?;
            e.uint(2)?;
            e.uint(3)?;
            e.array(2)?;
            e.uint(4)?;
            e.uint(5)
        }),
        "8301820203820405"
    );
    assert_eq!(
        bytes(|e| {
            e.array(25)?;
            (1..=25).try_for_each(|n| e.uint(n))
        }),
        "98190102030405060708090a0b0c0d0e0f101112131415161718181819"
    );
    assert_eq!(bytes(|e| e.map(0)), "a0");
    assert_eq!(
        bytes(|e| {
            e.map(2)?;
            e.key(1)?;
            e.uint(2)?;
            e.key(3)?;
            e.uint(4)
        }),
        "a201020304"
    );
}

/// Every head width at both ends: the argument in the first byte up to 23, then in 1, 2, 4 and 8
/// more bytes, never longer than needed (RFC 8949 section 4.2.1).
#[test]
fn heads_take_the_fewest_bytes() {
    let cases: &[(u64, &str)] = &[
        (23, "17"),
        (24, "1818"),
        (255, "18ff"),
        (256, "190100"),
        (65535, "19ffff"),
        (65536, "1a00010000"),
        (4294967295, "1affffffff"),
        (4294967296, "1b0000000100000000"),
    ];
    for &(n, want) in cases {
        assert_eq!(bytes(|e| e.uint(n)), want, "{n}");
        // The same argument in every other major type.
        let mut neg = unhex(want);
        neg[0] |= 0x20;
        assert_eq!(bytes(|e| e.int(-1 - i128::from(n))), hex(&neg), "-1-{n}");
    }
    // A length is a head too: a 24-byte string has a one-byte length after 0x58.
    assert_eq!(bytes(|e| e.bytes(&[7; 24]))[..4], *"5818");
    assert_eq!(bytes(|e| e.text(&"x".repeat(256)))[..6], *"790100");
}

/// A bignum only beyond 64 bits, and then with no leading zero byte (24.3; RFC 8949 section
/// 3.4.3), up to `i128`'s extremes.
#[test]
fn bignums_only_beyond_64_bits_and_without_leading_zeros() {
    assert_eq!(bytes(|e| e.int(i128::from(u64::MAX))), "1bffffffffffffffff");
    assert_eq!(
        bytes(|e| e.int(-1 - i128::from(u64::MAX))),
        "3bffffffffffffffff"
    );
    assert_eq!(
        bytes(|e| e.int(i128::MAX)),
        "c2507fffffffffffffffffffffffffffffff"
    );
    assert_eq!(
        bytes(|e| e.int(i128::MIN)),
        "c3507fffffffffffffffffffffffffffffff"
    );
    assert_eq!(bytes(|e| e.int(1 << 72)), "c24a01000000000000000000");
    assert_eq!(bytes(|e| e.int(-(1 << 72) - 1)), "c34a01000000000000000000");
}

/// Every finite binary16 value is written in three bytes, its own bits, and reads back as itself;
/// the infinities and NaNs among them are refused.
#[test]
fn every_binary16_value_is_three_bytes() {
    for h in 0..=u16::MAX {
        let x = half::f16::from_bits(h).to_f64();
        let mut e = Encoder::new();
        if x.is_finite() {
            e.float(x).unwrap();
            let [hi, lo] = h.to_be_bytes();
            assert_eq!(e.finish().unwrap().as_bytes(), [0xf9, hi, lo], "{h:04x}");
        } else {
            assert_eq!(e.float(x), Err(EncodeError::NotFinite), "{h:04x}");
        }
    }
}

/// A double one unit in the last place away from a binary16 or binary32 value is not that value,
/// so it takes a wider form; the binary32 values that are no binary16 take five bytes.
#[test]
fn a_neighbour_of_a_narrow_value_is_wider() {
    let geom_test_seed = geom_test::seed::resolve("cbor neighbours", Seed(0x6386_0001)).unwrap();
    let mut state = geom_test_seed.get();
    for _ in 0..200_000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let s = (state >> 32) as u32;
        let single = f32::from_bits(s);
        if !single.is_finite() {
            continue;
        }
        let x = f64::from(single);
        let half = half::f16::from_f32(single);
        let narrow16 = half.to_f32().to_bits() == s;
        let written = ours(&V::F(x));
        if narrow16 {
            assert_eq!(written.len(), 3, "{x:e}");
        } else {
            let mut want = vec![0xfa];
            want.extend_from_slice(&s.to_be_bytes());
            assert_eq!(written, want, "{x:e}");
        }
        if x != 0.0 {
            for d in [-1_i64, 1] {
                let y = f64::from_bits(x.to_bits().wrapping_add_signed(d));
                if y.is_finite() {
                    let mut want = vec![0xfb];
                    want.extend_from_slice(&y.to_bits().to_be_bytes());
                    assert_eq!(ours(&V::F(y)), want, "{y:e}");
                }
            }
        }
    }
}

/// Columns are tag 79 or 86 over a byte string of each value's little-endian bytes; an `f64`
/// column keeps every value's own bits, −0.0 and subnormals included.
#[test]
fn columns_are_typed_arrays() {
    assert_eq!(I64_COLUMN_TAG, 79);
    assert_eq!(F64_COLUMN_TAG, 86);
    assert_eq!(bytes(|e| e.i64_column(&[])), "d84f40");
    assert_eq!(
        bytes(|e| e.i64_column(&[1, -1])),
        "d84f500100000000000000ffffffffffffffff"
    );
    assert_eq!(
        bytes(|e| e.f64_column(&[-0.0, f64::from_bits(1)])),
        "d856500000000000000080".to_owned() + "0100000000000000"
    );
    assert_eq!(bytes(|e| e.f64_column(&[1.0; 3]))[..8], *"d8565818");
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut e = Encoder::new();
        assert_eq!(e.f64_column(&[1.0, bad]), Err(EncodeError::NotFinite));
        e.uint(5).unwrap();
        assert_eq!(e.finish().unwrap().as_bytes(), [5]);
    }
}

/// A set sorts its elements by their bytes and refuses a repeat; a sorted list keeps repeats.
#[test]
fn sets_and_sorted_lists_sort_by_bytes() {
    let elements = [V::T("a".into()), V::I(-1), V::U(24), V::U(1)];
    let items: Vec<_> = elements.iter().map(|v| encode(v).unwrap()).collect();
    // 01 < 1818 < 20 < 6161: bytewise, not by kind or by length.
    assert_eq!(
        bytes(|e| e.set(items.clone())),
        "8401181820".to_owned() + "6161"
    );
    let mut twice = items.clone();
    twice.push(items[2].clone());
    let mut e = Encoder::new();
    assert_eq!(e.set(twice.clone()), Err(EncodeError::Repeated));
    assert_eq!(
        bytes(|e| e.sorted(twice)),
        "850118181818".to_owned() + "206161"
    );
    // A shorter item that is a prefix of a longer one sorts first.
    let short = encode(&V::B(vec![])).unwrap();
    let long = encode(&V::B(vec![0])).unwrap();
    assert!(short < long);
    assert_eq!(bytes(|e| e.set(vec![long, short])), "82404100");
}

/// Each refusal is its own error, and a refused call writes nothing: the encoder still finishes
/// the item it was writing, byte for byte as if the call had not been made.
#[test]
fn refusals_write_nothing() {
    let e = Encoder::new();
    assert_eq!(e.finish().unwrap_err(), EncodeError::Unfinished);

    let mut e = Encoder::new();
    assert_eq!(e.key(0), Err(EncodeError::KeyNotExpected));
    e.map(2).unwrap();
    assert_eq!(e.uint(1), Err(EncodeError::KeyExpected));
    assert_eq!(e.clone().finish(), Err(EncodeError::Unfinished));
    e.key(5).unwrap();
    assert_eq!(e.key(6), Err(EncodeError::KeyNotExpected));
    e.uint(50).unwrap();
    assert_eq!(e.key(5), Err(EncodeError::KeyNotAscending));
    assert_eq!(e.key(4), Err(EncodeError::KeyNotAscending));
    assert_eq!(e.float(f64::NAN), Err(EncodeError::NotFinite));
    e.key(6).unwrap();
    e.array(1).unwrap();
    assert_eq!(e.key(0), Err(EncodeError::KeyNotExpected));
    e.text("x").unwrap();
    assert_eq!(e.uint(0), Err(EncodeError::Complete));
    assert_eq!(e.key(7), Err(EncodeError::KeyNotExpected));
    assert_eq!(hex(e.finish().unwrap().as_bytes()), "a205183206816178");

    let mut e = Encoder::new();
    e.array(2).unwrap();
    e.map(0).unwrap();
    assert_eq!(e.key(0), Err(EncodeError::KeyNotExpected));
    e.array(0).unwrap();
    assert_eq!(e.array(0), Err(EncodeError::Complete));
    assert_eq!(hex(e.finish().unwrap().as_bytes()), "82a080");

    // A full map or array closes at once, so the next item goes to the one around it, and after
    // the outermost closes, nothing more goes anywhere.
    let mut e = Encoder::new();
    e.array(2).unwrap();
    e.map(1).unwrap();
    e.key(1).unwrap();
    e.uint(1).unwrap();
    e.uint(2).unwrap();
    assert_eq!(e.uint(3), Err(EncodeError::Complete));
    assert_eq!(hex(e.finish().unwrap().as_bytes()), "82a1010102");
}

/// An encoder holds one item: after a top-level scalar, anything more is refused.
#[test]
fn one_item_per_encoder() {
    let mut e = Encoder::new();
    e.uint(1).unwrap();
    for refused in [
        e.clone().uint(2),
        e.clone().int(2),
        e.clone().float(2.0),
        e.clone().bytes(&[2]),
        e.clone().text("2"),
        e.clone().array(0),
        e.clone().map(0),
        e.clone().i64_column(&[2]),
        e.clone().f64_column(&[2.0]),
        e.clone().set(vec![]),
        e.clone().sorted(vec![]),
        e.clone().item(&encode(&V::U(2)).unwrap()),
    ] {
        assert_eq!(refused, Err(EncodeError::Complete));
    }
    assert_eq!(e.finish().unwrap().as_bytes(), [1]);
}

/// An item placed inside another is copied as it is.
#[test]
fn items_nest_as_they_are() {
    let inner = encode(&V::A(vec![V::U(1), V::F(1.5)])).unwrap();
    assert_eq!(format!("{inner:?}"), "Item(8201f93e00)");
    assert_eq!(
        bytes(|e| {
            e.array(2)?;
            e.item(&inner)?;
            e.item(&inner)
        }),
        "828201f93e008201f93e00"
    );
    assert_eq!(inner.clone().into_bytes(), inner.as_bytes());
}

/// Every error says what it refused, each in its own words.
#[test]
fn errors_say_what_they_refused() {
    let all = [
        EncodeError::NotFinite,
        EncodeError::KeyNotAscending,
        EncodeError::KeyExpected,
        EncodeError::KeyNotExpected,
        EncodeError::Complete,
        EncodeError::Unfinished,
        EncodeError::Repeated,
        EncodeError::TooLong,
    ];
    let words: std::collections::BTreeSet<String> = all.iter().map(ToString::to_string).collect();
    assert_eq!(words.len(), all.len());
    assert!(words.iter().all(|w| w.len() > 10));
}

/// Random values of the subset, nested, are written byte for byte as the independent reference
/// writes them.
#[test]
fn random_values_match_the_reference() {
    let result = geom_test::prop::run(
        "cbor random values",
        Seed(0x6386_0002),
        512,
        &common::value(),
        |v| {
            let ours = ours(&v);
            let reference = reference(&v);
            proptest::prop_assert_eq!(hex(&ours), hex(&reference));
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}

/// Random scalars, many more of them.
#[test]
fn random_scalars_match_the_reference() {
    let result = geom_test::prop::run(
        "cbor random scalars",
        Seed(0x6386_0003),
        20_000,
        &common::scalar(),
        |v| {
            proptest::prop_assert_eq!(hex(&ours(&v)), hex(&reference(&v)));
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}
