//! Attacks on piece G91's claims: the strict decoder accepts exactly the canonical encoding of
//! every value of SKG's CBOR subset and refuses every other byte string with the rule it breaks,
//! never panicking, never reading past its input, never nesting past its limit, and stopping where
//! its meter says; the columns read exactly; and the text form converts both ways bit for bit.
//!
//! The truth is this file's own encoder (`encode`), written from RFC 8949 sections 3 and 4.2.1
//! without the crate's code: heads in their shortest form, floats in the shortest width found by
//! searching all 65 536 halves and comparing with Rust's conversion to `f32`, and map pairs sorted
//! by the bytes of their keys.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::OnceLock;

use geom_format::cbor::{
    CborError, CborErrorKind, Decoder, Item, TextError, TextErrorKind, check, float_column,
    from_text, int_column, item_len, to_text,
};
use geom_run::{Budget, Cancel, Stop, WorkMeter};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, resolve};

/// A value of the subset, as the test builds it.
#[derive(Clone, Debug)]
enum V {
    U(u64),
    N(u64),
    B(Vec<u8>),
    T(String),
    A(Vec<V>),
    M(Vec<(V, V)>),
    /// A bignum: negative (tag 3) or not (tag 2), and its magnitude's bytes.
    Big(bool, Vec<u8>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    F(f64),
    Bool(bool),
    Null,
}

/// Depth enough for every value the generator makes.
const DEPTH: usize = 64;

/// A budget far above what any test here spends, so a decoder that stops advancing ends in a
/// failed test, not a hang.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1_000_000))
}

/// 2^k exactly, for −1074 ≤ k ≤ 1023, from its bits.
fn p2(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1 << (k + 1074))
    }
}

/// Every finite half's value, by its definition, mapped to its bits.
fn halves() -> &'static BTreeMap<u64, u16> {
    static TABLE: OnceLock<BTreeMap<u64, u16>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = BTreeMap::new();
        for bits in 0..=u16::MAX {
            let e = i32::from((bits >> 10) & 0x1f);
            if e == 31 {
                continue;
            }
            let f = f64::from(bits & 0x3ff);
            let magnitude = if e == 0 {
                f * p2(-24)
            } else {
                (1024.0 + f) * p2(e - 25)
            };
            let x = if bits >> 15 == 1 {
                -magnitude
            } else {
                magnitude
            };
            table.insert(x.to_bits(), bits);
        }
        table
    })
}

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend([m | 0x18, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 0x19);
        out.extend((n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 0x1a);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(m | 0x1b);
        out.extend(n.to_be_bytes());
    }
}

fn float(out: &mut Vec<u8>, x: f64) {
    if let Some(h) = halves().get(&x.to_bits()) {
        out.push(0xf9);
        out.extend(h.to_be_bytes());
    } else if f64::from(x as f32).to_bits() == x.to_bits() {
        out.push(0xfa);
        out.extend((x as f32).to_bits().to_be_bytes());
    } else {
        out.push(0xfb);
        out.extend(x.to_bits().to_be_bytes());
    }
}

/// The canonical encoding of `v`, written from RFC 8949 alone.
fn encode(v: &V, out: &mut Vec<u8>) {
    match v {
        V::U(n) => head(out, 0, *n),
        V::N(n) => head(out, 1, *n),
        V::B(b) => {
            head(out, 2, b.len() as u64);
            out.extend(b);
        }
        V::T(t) => {
            head(out, 3, t.len() as u64);
            out.extend(t.as_bytes());
        }
        V::A(items) => {
            head(out, 4, items.len() as u64);
            for i in items {
                encode(i, out);
            }
        }
        V::M(pairs) => {
            let mut encoded: Vec<(Vec<u8>, Vec<u8>)> = pairs
                .iter()
                .map(|(k, v)| {
                    let (mut kb, mut vb) = (Vec::new(), Vec::new());
                    encode(k, &mut kb);
                    encode(v, &mut vb);
                    (kb, vb)
                })
                .collect();
            // Sorted by key alone, stably, so a repeated key keeps its first value.
            encoded.sort_by(|a, b| a.0.cmp(&b.0));
            encoded.dedup_by(|a, b| a.0 == b.0);
            head(out, 5, encoded.len() as u64);
            for (k, v) in encoded {
                out.extend(k);
                out.extend(v);
            }
        }
        V::Big(negative, magnitude) => {
            head(out, 6, if *negative { 3 } else { 2 });
            head(out, 2, magnitude.len() as u64);
            out.extend(magnitude);
        }
        V::Ints(values) => {
            head(out, 6, 79);
            head(out, 2, 8 * values.len() as u64);
            for v in values {
                out.extend(v.to_le_bytes());
            }
        }
        V::Floats(values) => {
            head(out, 6, 86);
            head(out, 2, 8 * values.len() as u64);
            for v in values {
                out.extend(v.to_le_bytes());
            }
        }
        V::F(x) => float(out, *x),
        V::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        V::Null => out.push(0xf6),
    }
}

fn bytes_of(v: &V) -> Vec<u8> {
    let mut out = Vec::new();
    encode(v, &mut out);
    out
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A random finite double, from every family the decoder treats differently.
fn random_float(rng: &mut SplitMix64) -> f64 {
    loop {
        let x = match below(rng, 6) {
            0 => {
                let table = halves();
                let i = below(rng, table.len() as u64) as usize;
                f64::from_bits(*table.keys().nth(i).unwrap())
            }
            1 => f64::from(f32::from_bits(rng.next_u64() as u32)),
            2 => f64::from_bits(rng.next_u64()),
            3 => f64::from_bits(rng.next_u64() & 0x800f_ffff_ffff_ffff),
            4 => [
                0.0,
                -0.0,
                f64::MAX,
                f64::MIN,
                f64::MIN_POSITIVE,
                5e-324,
                65504.0,
                65520.0,
            ][below(rng, 8) as usize],
            _ => (below(rng, 2001) as f64 - 1000.0) / 8.0,
        };
        if x.is_finite() {
            return x;
        }
    }
}

fn random_text(rng: &mut SplitMix64) -> String {
    let pool = [
        'a', 'Z', '0', ' ', '"', '\\', '\n', '\u{0}', '\u{1f}', '\u{7f}', 'é', '\u{2028}', '𝄞', '/',
    ];
    (0..below(rng, 8))
        .map(|_| pool[below(rng, pool.len() as u64) as usize])
        .collect()
}

fn random_value(rng: &mut SplitMix64, depth: u32) -> V {
    let leaf = depth == 0 || below(rng, 3) == 0;
    let choice = if leaf {
        below(rng, 10)
    } else {
        10 + below(rng, 2)
    };
    match choice {
        0 => V::U(match below(rng, 3) {
            0 => below(rng, 30),
            1 => [
                23,
                24,
                255,
                256,
                65535,
                65536,
                0xffff_ffff,
                1 << 32,
                u64::MAX,
            ][below(rng, 9) as usize],
            _ => rng.next_u64() >> below(rng, 64),
        }),
        1 => V::N(rng.next_u64() >> below(rng, 64)),
        2 => {
            let mut b = vec![0; below(rng, 40) as usize];
            rng.fill_bytes(&mut b);
            V::B(b)
        }
        3 => V::T(random_text(rng)),
        4 => {
            let mut magnitude = vec![0; 9 + below(rng, 8) as usize];
            rng.fill_bytes(&mut magnitude);
            magnitude[0] |= 1;
            V::Big(rng.coin(), magnitude)
        }
        5 => V::Ints((0..below(rng, 6)).map(|_| rng.next_u64() as i64).collect()),
        6 => V::Floats((0..below(rng, 6)).map(|_| random_float(rng)).collect()),
        7 => V::F(random_float(rng)),
        8 => V::Bool(rng.coin()),
        9 => V::Null,
        10 => V::A(
            (0..below(rng, 5))
                .map(|_| random_value(rng, depth - 1))
                .collect(),
        ),
        _ => V::M(
            (0..below(rng, 5))
                .map(|_| (random_value(rng, depth - 1), random_value(rng, depth - 1)))
                .collect(),
        ),
    }
}

/// The items the decoder hands out for `bytes`, or its error.
fn items(bytes: &[u8]) -> Result<Vec<String>, CborError> {
    let mut decoder = Decoder::new(bytes, DEPTH);
    let mut out = Vec::new();
    let mut m = meter();
    while let Some(item) = decoder.next(&mut m)? {
        out.push(match item {
            Item::Float(x) => format!("Float({:#x})", x.to_bits()),
            other => format!("{other:?}"),
        });
    }
    Ok(out)
}

/// The items the test expects for an encoded value, read from its own encoding by RFC 8949's
/// structure alone (a second walk, independent of the decoder).
fn expected_items(v: &V, out: &mut Vec<String>) {
    match v {
        V::U(n) => out.push(format!("Uint({n})")),
        V::N(n) => out.push(format!("Nint({n})")),
        V::B(b) => out.push(format!("Bytes({b:?})")),
        V::T(t) => out.push(format!("Text({t:?})")),
        V::A(items) => {
            out.push(format!("Array({})", items.len()));
            for i in items {
                expected_items(i, out);
            }
        }
        V::M(pairs) => {
            let mut sorted: Vec<(Vec<u8>, &V, &V)> =
                pairs.iter().map(|(k, v)| (bytes_of(k), k, v)).collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            sorted.dedup_by(|a, b| a.0 == b.0);
            out.push(format!("Map({})", sorted.len()));
            for (_, k, v) in sorted {
                expected_items(k, out);
                expected_items(v, out);
            }
        }
        V::Big(negative, magnitude) => {
            out.push(format!("Tag({})", if *negative { 3 } else { 2 }));
            out.push(format!("Bytes({magnitude:?})"));
        }
        V::Ints(values) => {
            out.push("Tag(79)".into());
            let b: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            out.push(format!("Bytes({b:?})"));
        }
        V::Floats(values) => {
            out.push("Tag(86)".into());
            let b: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            out.push(format!("Bytes({b:?})"));
        }
        V::F(x) => out.push(format!("Float({:#x})", x.to_bits())),
        V::Bool(b) => out.push(format!("Bool({b})")),
        V::Null => out.push("Null".into()),
    }
}

fn refused(bytes: &[u8]) -> CborError {
    check(bytes, DEPTH, &mut meter()).expect_err("accepted bytes that are not canonical")
}

fn kind(bytes: &[u8]) -> CborErrorKind {
    refused(bytes).kind
}

#[test]
fn every_random_canonical_value_is_accepted_read_exactly_and_round_trips_through_text() {
    let seed = resolve("cbor canonical values", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for case in 0..4000 {
        let v = random_value(&mut rng, 4);
        let bytes = bytes_of(&v);
        let mut want = Vec::new();
        expected_items(&v, &mut want);
        assert_eq!(items(&bytes), Ok(want), "case {case}: {v:?}");
        assert_eq!(check(&bytes, DEPTH, &mut meter()), Ok(()), "case {case}");
        let text = to_text(&bytes, DEPTH, &mut meter()).unwrap();
        assert_eq!(
            from_text(&text, DEPTH, &mut meter()),
            Ok(bytes.clone()),
            "case {case}:\n{text}"
        );
        let mut longer = bytes.clone();
        longer.extend([0x00, 0xff]);
        assert_eq!(
            item_len(&longer, DEPTH, &mut meter()),
            Ok(bytes.len()),
            "case {case}"
        );
        assert_eq!(
            check(&longer, DEPTH, &mut meter()),
            Err(CborError {
                offset: bytes.len(),
                kind: CborErrorKind::TrailingBytes
            })
        );
    }
}

#[test]
fn every_proper_prefix_of_a_canonical_value_is_truncated() {
    let seed = resolve("cbor prefixes", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..300 {
        let bytes = bytes_of(&random_value(&mut rng, 3));
        for cut in 0..bytes.len() {
            let e = refused(&bytes[..cut]);
            assert_eq!(
                e.kind,
                CborErrorKind::Truncated,
                "{bytes:02x?} cut at {cut}"
            );
            assert_eq!(e.offset, cut);
        }
    }
}

#[test]
fn every_longer_head_is_refused_and_the_shortest_accepted() {
    let boundaries = [
        0,
        1,
        23,
        24,
        255,
        256,
        65535,
        65536,
        0xffff_ffff,
        1 << 32,
        u64::MAX,
    ];
    for major in [0_u8, 1, 6] {
        for &n in &boundaries {
            // Tags need an allowed number and their content; integers stand alone.
            let (number, content): (u64, Vec<u8>) = if major == 6 {
                if !matches!(n, 2 | 3 | 79 | 86) {
                    continue;
                }
                (
                    n,
                    if n < 4 {
                        vec![0x49, 1, 0, 0, 0, 0, 0, 0, 0, 0]
                    } else {
                        vec![0x40]
                    },
                )
            } else {
                (n, vec![])
            };
            let mut shortest = Vec::new();
            head(&mut shortest, major, number);
            shortest.extend(&content);
            assert_eq!(
                check(&shortest, DEPTH, &mut meter()),
                Ok(()),
                "{shortest:02x?}"
            );
            for (info, width) in [(24_u8, 1_usize), (25, 2), (26, 4), (27, 8)] {
                if width < 8 && number >> (8 * width) != 0 {
                    continue;
                }
                let mut longer = vec![(major << 5) | info];
                longer.extend(&number.to_be_bytes()[8 - width..]);
                longer.extend(&content);
                if longer.len() - content.len() == shortest.len() - content.len() {
                    continue;
                }
                assert_eq!(
                    kind(&longer),
                    CborErrorKind::NonShortestHead,
                    "{longer:02x?}"
                );
            }
        }
    }
    // Lengths and counts too: a one-byte string, array and map with a two-byte head.
    for bytes in [
        vec![0x58, 0x01, 0xaa],
        vec![0x78, 0x01, b'a'],
        vec![0x98, 0x01, 0x00],
        vec![0xb8, 0x01, 0x00, 0x00],
    ] {
        assert_eq!(kind(&bytes), CborErrorKind::NonShortestHead, "{bytes:02x?}");
    }
}

#[test]
fn every_float_wider_than_its_value_needs_is_refused() {
    for (&bits, &half) in halves() {
        let x = f64::from_bits(bits);
        let mut wide = vec![0xfa];
        wide.extend((x as f32).to_bits().to_be_bytes());
        assert_eq!(
            kind(&wide),
            CborErrorKind::NonShortestFloat,
            "{x:e} ({half:#06x}) as single"
        );
        let mut wider = vec![0xfb];
        wider.extend(bits.to_be_bytes());
        assert_eq!(
            kind(&wider),
            CborErrorKind::NonShortestFloat,
            "{x:e} as double"
        );
    }
    let seed = resolve("cbor singles", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..100_000 {
        let s = f32::from_bits(rng.next_u64() as u32);
        if !s.is_finite() {
            continue;
        }
        let mut wide = vec![0xfb];
        wide.extend(f64::from(s).to_bits().to_be_bytes());
        assert_eq!(kind(&wide), CborErrorKind::NonShortestFloat, "{s:e}");
    }
}

#[test]
fn nan_and_infinity_are_refused_at_every_width_and_in_columns() {
    for bytes in [
        vec![0xf9, 0x7e, 0x00],
        vec![0xf9, 0x7c, 0x00],
        vec![0xf9, 0xfc, 0x00],
        vec![0xf9, 0x7c, 0x01],
        vec![0xfa, 0x7f, 0xc0, 0, 0],
        vec![0xfa, 0xff, 0x80, 0, 0],
        vec![0xfb, 0x7f, 0xf8, 0, 0, 0, 0, 0, 0],
        vec![0xfb, 0xff, 0xf0, 0, 0, 0, 0, 0, 0],
        vec![0xfb, 0x7f, 0xf0, 0, 0, 0, 0, 0, 1],
    ] {
        assert_eq!(kind(&bytes), CborErrorKind::NotFinite, "{bytes:02x?}");
    }
    for k in 0..5 {
        let mut values = vec![1.5; 5];
        values[k] = if k % 2 == 0 {
            f64::NAN
        } else {
            f64::NEG_INFINITY
        };
        let bytes = bytes_of(&V::Floats(values.clone()));
        let e = refused(&bytes);
        assert_eq!(e.kind, CborErrorKind::NotFinite);
        // The tag's head is 2 bytes (0xd8 0x56), the string's head 2 (0x58 0x28).
        assert_eq!(e.offset, 4 + 8 * k, "{k}");
    }
}

#[test]
fn map_keys_must_rise_in_bytewise_order_not_length_first() {
    // {1: 0, 0: 0} and {0: 0, 0: 0}.
    let e = refused(&[0xa2, 0x01, 0x00, 0x00, 0x00]);
    assert_eq!((e.kind, e.offset), (CborErrorKind::UnsortedKeys, 3));
    let e = refused(&[0xa2, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!((e.kind, e.offset), (CborErrorKind::DuplicateKey, 3));
    // −1 (0x20) and 24 (0x18 0x18): bytewise, 24 comes first; RFC 7049's length-first order, which
    // some encoders still call canonical, puts −1 first.
    assert_eq!(
        check(&[0xa2, 0x18, 0x18, 0x00, 0x20, 0x00], DEPTH, &mut meter()),
        Ok(())
    );
    assert_eq!(
        kind(&[0xa2, 0x20, 0x00, 0x18, 0x18, 0x00]),
        CborErrorKind::UnsortedKeys
    );
    // Keys that are containers compare by their whole encodings; equal ones are duplicates.
    assert_eq!(
        kind(&[0xa2, 0x81, 0x01, 0x00, 0x81, 0x01, 0x00]),
        CborErrorKind::DuplicateKey
    );
    assert_eq!(
        check(
            &[0xa2, 0x81, 0x01, 0x00, 0x81, 0x02, 0x00],
            DEPTH,
            &mut meter()
        ),
        Ok(())
    );
    // A key that is a prefix of another's encoding comes first: "a" < "ab"? No: lengths differ in
    // the head, 0x61 < 0x62, so "a" first.
    assert_eq!(
        check(
            &[0xa2, 0x61, b'b', 0x00, 0x62, b'a', b'a', 0x00],
            DEPTH,
            &mut meter()
        ),
        Ok(())
    );
    // Order is checked in nested maps too, at the nested key.
    let e = refused(&[0x81, 0xa2, 0x02, 0x00, 0x01, 0x00]);
    assert_eq!((e.kind, e.offset), (CborErrorKind::UnsortedKeys, 4));
}

#[test]
fn indefinite_lengths_reserved_values_and_other_simple_values_are_refused() {
    for initial in [0x1f_u8, 0x3f, 0x5f, 0x7f, 0x9f, 0xbf, 0xdf, 0xff] {
        assert_eq!(
            kind(&[initial, 0x00, 0xff]),
            CborErrorKind::Indefinite,
            "{initial:#04x}"
        );
    }
    for major in 0..8_u8 {
        for info in 28..=30 {
            assert_eq!(
                kind(&[(major << 5) | info, 0, 0, 0, 0, 0, 0, 0, 0]),
                CborErrorKind::ReservedInfo
            );
        }
    }
    for bytes in [
        vec![0xe0],
        vec![0xf3],
        vec![0xf7],
        vec![0xf8, 0x20],
        vec![0xf8, 0x00],
        vec![0xf8, 0xff],
    ] {
        assert_eq!(
            kind(&bytes),
            CborErrorKind::UnsupportedSimple,
            "{bytes:02x?}"
        );
    }
    for (bytes, item) in [
        (vec![0xf4], Item::Bool(false)),
        (vec![0xf5], Item::Bool(true)),
        (vec![0xf6], Item::Null),
    ] {
        assert_eq!(Decoder::new(&bytes, 0).next(&mut meter()), Ok(Some(item)));
    }
}

#[test]
fn only_four_tags_each_on_a_well_formed_byte_string() {
    let nine = [0x49_u8, 1, 0, 0, 0, 0, 0, 0, 0, 0];
    for tag in [0_u64, 1, 4, 24, 55799, 55800, 0x534b_4746, 78, 80, 85, 87] {
        let mut bytes = Vec::new();
        head(&mut bytes, 6, tag);
        bytes.push(0x40);
        assert_eq!(kind(&bytes), CborErrorKind::UnsupportedTag, "tag {tag}");
    }
    for tag in [0xc2_u8, 0xc3] {
        let mut ok = vec![tag];
        ok.extend(nine);
        assert_eq!(check(&ok, DEPTH, &mut meter()), Ok(()));
        // Eight bytes fit in 64 bits; a leading zero byte is never canonical.
        let e = refused(&[tag, 0x48, 1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!((e.kind, e.offset), (CborErrorKind::NonCanonicalBignum, 0));
        let e = refused(&[tag, 0x4a, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(e.kind, CborErrorKind::NonCanonicalBignum);
        assert_eq!(kind(&[tag, 0x40]), CborErrorKind::NonCanonicalBignum);
        // The content must be a byte string, not text, an integer, a container or another tag.
        for content in [
            vec![0x61, b'a'],
            vec![0x01],
            vec![0x80],
            vec![0xa0],
            vec![0xc2, 0x40],
            vec![0xf6],
        ] {
            let mut bytes = vec![tag];
            bytes.extend(&content);
            let e = refused(&bytes);
            assert_eq!(
                (e.kind, e.offset),
                (CborErrorKind::TagContent, 1),
                "{bytes:02x?}"
            );
        }
    }
    for tag in [0x4f_u8, 0x56] {
        for length in [1_u8, 7, 9, 15] {
            let mut bytes = vec![0xd8, tag, 0x40 | length];
            bytes.extend(vec![0; usize::from(length)]);
            assert_eq!(kind(&bytes), CborErrorKind::ColumnLength, "{bytes:02x?}");
        }
    }
}

#[test]
fn text_must_be_utf8() {
    for bytes in [
        vec![0x62, 0xc3, 0x28],
        vec![0x62, 0xc0, 0x80],
        vec![0x63, 0xed, 0xa0, 0x80],
        vec![0x61, 0xff],
        vec![0x64, 0xf4, 0x90, 0x80, 0x80],
    ] {
        let e = refused(&bytes);
        assert_eq!(
            (e.kind, e.offset),
            (CborErrorKind::InvalidUtf8, 0),
            "{bytes:02x?}"
        );
    }
}

#[test]
fn counts_and_lengths_beyond_the_input_are_refused_before_anything_is_read() {
    let mut huge = Vec::new();
    for major in [2_u8, 3, 4, 5] {
        for n in [u64::MAX, 1 << 63, 1 << 40, 5] {
            huge.clear();
            head(&mut huge, major, n);
            huge.extend([0; 4]);
            let e = refused(&huge);
            assert_eq!(e.kind, CborErrorKind::Truncated, "{huge:02x?}");
        }
    }
    // A map of n pairs needs 2n bytes: two pairs in three bytes is refused at once.
    assert_eq!(kind(&[0xa2, 0x00, 0x00, 0x00]), CborErrorKind::Truncated);
}

#[test]
fn nesting_deeper_than_the_limit_is_refused_and_tags_count() {
    for depth in 0..40_usize {
        let mut bytes = vec![0x81; depth];
        bytes.push(0x00);
        assert_eq!(check(&bytes, depth, &mut meter()), Ok(()), "{depth}");
        if depth > 0 {
            let e = check(&bytes, depth - 1, &mut meter()).unwrap_err();
            assert_eq!((e.kind, e.offset), (CborErrorKind::TooDeep, depth - 1));
        }
    }
    // Empty containers open nothing; a tag does.
    assert_eq!(check(&[0x80], 0, &mut meter()), Ok(()));
    assert_eq!(check(&[0xa0], 0, &mut meter()), Ok(()));
    assert_eq!(
        check(&[0xd8, 0x4f, 0x40], 0, &mut meter())
            .unwrap_err()
            .kind,
        CborErrorKind::TooDeep
    );
    // A million nested arrays are refused at the limit without recursion.
    let mut deep = vec![0x81; 1_000_000];
    deep.push(0x00);
    assert_eq!(
        check(&deep, 64, &mut meter()).unwrap_err().kind,
        CborErrorKind::TooDeep
    );
}

#[test]
fn the_meter_stops_the_read_at_the_same_head_every_time() {
    // A two-byte array head, then 100 one-byte items: head j starts at byte j + 1.
    let bytes = bytes_of(&V::A(vec![V::U(0); 100]));
    for work in [0_u64, 1, 50, 100] {
        let run = || {
            let mut m = WorkMeter::new(Budget::of_work(work));
            check(&bytes, DEPTH, &mut m)
        };
        let e = run().unwrap_err();
        assert!(
            matches!(e.kind, CborErrorKind::Stopped(s) if s.reason() == Stop::OverBudget(geom_run::BudgetKind::Work))
        );
        // One unit per head: the head after the budget's last is the one refused.
        assert_eq!(e.offset, if work == 0 { 0 } else { work as usize + 1 });
        assert_eq!(run().unwrap_err(), e);
    }
    assert_eq!(
        check(&bytes, DEPTH, &mut WorkMeter::new(Budget::of_work(101))),
        Ok(())
    );
    // A long column pays one unit per 4 KiB scanned, so a budget stops it part-way.
    let column = bytes_of(&V::Floats(vec![2.0; 4096]));
    assert!(check(&column, DEPTH, &mut WorkMeter::new(Budget::of_work(4))).is_err());
    assert_eq!(
        check(&column, DEPTH, &mut WorkMeter::new(Budget::of_work(2 + 8))),
        Ok(())
    );
}

struct Always;
impl Cancel for Always {
    fn requested(&self) -> bool {
        true
    }
}

#[test]
fn a_cancelled_read_stops_within_one_poll() {
    // A three-byte array head, then 1000 one-byte items: head j starts at byte j + 2.
    let bytes = bytes_of(&V::A(vec![V::U(0); 1000]));
    let cancel = Always;
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&cancel);
    let e = check(&bytes, DEPTH, &mut m).unwrap_err();
    assert!(matches!(e.kind, CborErrorKind::Stopped(s) if s.reason() == Stop::Cancelled));
    assert!(e.offset <= geom_run::POLL_EVERY + 2, "{}", e.offset);
}

#[test]
fn columns_read_exactly_and_refuse_what_the_decoder_refuses() {
    let seed = resolve("cbor columns", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let ints: Vec<i64> = (0..5000).map(|_| rng.next_u64() as i64).collect();
    let bytes: Vec<u8> = ints.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut out = vec![7];
    int_column(&bytes, &mut out, &mut meter()).unwrap();
    assert_eq!(out[0], 7);
    assert_eq!(&out[1..], &ints[..]);
    let floats: Vec<f64> = (0..5000).map(|_| random_float(&mut rng)).collect();
    let bytes: Vec<u8> = floats.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut out = Vec::new();
    float_column(&bytes, &mut out, &mut meter()).unwrap();
    assert!(
        out.iter()
            .zip(&floats)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    let mut bad = bytes.clone();
    bad[8 * 4321..8 * 4322].copy_from_slice(&f64::NAN.to_le_bytes());
    let mut out = vec![1.0];
    let e = float_column(&bad, &mut out, &mut meter()).unwrap_err();
    assert_eq!((e.kind, e.offset), (CborErrorKind::NotFinite, 8 * 4321));
    assert_eq!(out, vec![1.0]);
    for length in [1, 7, 9] {
        let e = int_column(&vec![0; length], &mut Vec::new(), &mut meter()).unwrap_err();
        assert_eq!(e.kind, CborErrorKind::ColumnLength);
        let e = float_column(&vec![0; length], &mut Vec::new(), &mut meter()).unwrap_err();
        assert_eq!(e.kind, CborErrorKind::ColumnLength);
    }
    let e = int_column(
        &bytes,
        &mut Vec::new(),
        &mut WorkMeter::new(Budget::new(u64::MAX, 100)),
    )
    .unwrap_err();
    assert!(matches!(e.kind, CborErrorKind::Stopped(_)));
}

#[test]
fn hexadecimal_floats_are_written_normalised_and_read_back_exactly() {
    let cases = [
        (1.5, "0x1.8p+0"),
        (-0.0, "-0x0p+0"),
        (0.0, "0x0p+0"),
        (1.0, "0x1p+0"),
        (0.1, "0x1.999999999999ap-4"),
        (5e-324, "0x1p-1074"),
        (f64::MIN_POSITIVE, "0x1p-1022"),
        (f64::MAX, "0x1.fffffffffffffp+1023"),
        (-65504.0, "-0x1.ffcp+15"),
        (3.0 * p2(-1070), "0x1.8p-1069"),
    ];
    for (x, text) in cases {
        let bytes = bytes_of(&V::F(x));
        assert_eq!(to_text(&bytes, 0, &mut meter()).unwrap(), text);
        assert_eq!(from_text(text, 0, &mut meter()), Ok(bytes), "{text}");
    }
    // Other spellings of the same values read the same.
    for (text, x) in [
        ("0x18p-4", 1.5),
        ("0X1.8P0", 1.5),
        ("0x.8p1", 1.0),
        ("0x0.0000000000001p-1022", 5e-324),
        ("-0x0.0p0", -0.0),
    ] {
        assert_eq!(
            from_text(text, 0, &mut meter()),
            Ok(bytes_of(&V::F(x))),
            "{text}"
        );
    }
}

fn text_error(text: &str, depth: usize) -> TextError {
    from_text(text, depth, &mut meter()).expect_err(text)
}

#[test]
fn text_that_is_not_exact_or_not_canonical_is_refused_where_it_goes_wrong() {
    let cases: [(&str, u32, u32, TextErrorKind); 20] = [
        ("", 1, 1, TextErrorKind::UnexpectedEnd),
        ("1.5", 1, 1, TextErrorKind::InexactFloat),
        ("1e3", 1, 1, TextErrorKind::InexactFloat),
        ("0x1p-1075", 1, 3, TextErrorKind::InexactFloat),
        ("0x1.00000000000008p0", 1, 3, TextErrorKind::InexactFloat),
        ("0x1p1024", 1, 3, TextErrorKind::InexactFloat),
        ("0x10", 1, 3, TextErrorKind::InexactFloat),
        ("18446744073709551616", 1, 1, TextErrorKind::IntegerRange),
        ("-18446744073709551617", 1, 1, TextErrorKind::IntegerRange),
        ("-0", 1, 1, TextErrorKind::IntegerRange),
        ("[1,\n  2,]", 2, 5, TextErrorKind::Unexpected),
        (
            "{1: 0, 0: 0}",
            1,
            8,
            TextErrorKind::Cbor(CborErrorKind::UnsortedKeys),
        ),
        (
            "{0: 0,\n 0: 1}",
            2,
            2,
            TextErrorKind::Cbor(CborErrorKind::DuplicateKey),
        ),
        (
            "1(h'')",
            1,
            1,
            TextErrorKind::Cbor(CborErrorKind::UnsupportedTag),
        ),
        (
            "2(h'01')",
            1,
            1,
            TextErrorKind::Cbor(CborErrorKind::NonCanonicalBignum),
        ),
        ("\"\\ud800\"", 1, 3, TextErrorKind::BadEscape),
        ("h'abc'", 1, 6, TextErrorKind::BadHex),
        ("/ open", 1, 1, TextErrorKind::OpenComment),
        ("1 2", 1, 3, TextErrorKind::Trailing),
        ("\"a\u{1}\"", 1, 3, TextErrorKind::Unexpected),
    ];
    for (text, line, column, kind) in cases {
        assert_eq!(
            text_error(text, 4),
            TextError { line, column, kind },
            "{text:?}"
        );
    }
    assert_eq!(
        text_error("[[1]]", 1).kind,
        TextErrorKind::Cbor(CborErrorKind::TooDeep)
    );
    assert_eq!(
        text_error("79(h'')", 0).kind,
        TextErrorKind::Cbor(CborErrorKind::TooDeep)
    );
}

#[test]
fn text_reads_comments_spacing_escapes_and_the_integer_extremes() {
    let text = "/ a comment / [ -18446744073709551616 , 18446744073709551615,\n\t\"\\u00e9\\ud834\\udd1e\\/\\b\\f\", h' 00 FF '/c/, true,false ,null, 79(h''), {}, [] ]";
    let want = bytes_of(&V::A(vec![
        V::N(u64::MAX),
        V::U(u64::MAX),
        V::T("é𝄞/\u{8}\u{c}".into()),
        V::B(vec![0, 0xff]),
        V::Bool(true),
        V::Bool(false),
        V::Null,
        V::Ints(vec![]),
        V::M(vec![]),
        V::A(vec![]),
    ]));
    assert_eq!(from_text(text, 2, &mut meter()), Ok(want.clone()));
    let written = to_text(&want, 2, &mut meter()).unwrap();
    assert_eq!(
        written,
        "[\n  -18446744073709551616,\n  18446744073709551615,\n  \"é𝄞/\\b\\f\",\n  h'00ff',\n  true,\n  false,\n  null,\n  79(h''),\n  {},\n  []\n]"
    );
}

#[test]
fn the_layout_is_fixed() {
    let v = V::M(vec![
        (
            V::U(1),
            V::A(vec![V::F(1.5), V::M(vec![(V::T("k".into()), V::Null)])]),
        ),
        (
            V::T("a\"\\\n\u{7f}".into()),
            V::Big(true, vec![1, 0, 0, 0, 0, 0, 0, 0, 0]),
        ),
    ]);
    let text = to_text(&bytes_of(&v), 3, &mut meter()).unwrap();
    assert_eq!(
        text,
        "{\n  1: [\n    0x1.8p+0,\n    {\n      \"k\": null\n    }\n  ],\n  \"a\\\"\\\\\\n\\u007f\": 3(h'010000000000000000')\n}"
    );
}

#[test]
fn every_byte_string_of_up_to_three_bytes_is_refused_or_round_trips() {
    // Exhaustive over short inputs: no panic, and anything accepted converts both ways.
    let mut accepted = 0;
    for length in 0..=3 {
        for n in 0..(1_u32 << (8 * length)) {
            let bytes = &n.to_be_bytes()[4 - length..];
            let verdict = check(bytes, 2, &mut meter());
            assert!(
                !matches!(&verdict, Err(e) if matches!(e.kind, CborErrorKind::Stopped(_))),
                "three bytes never use up the budget: {bytes:02x?}"
            );
            if verdict.is_ok() {
                accepted += 1;
                let text = to_text(bytes, 2, &mut meter()).unwrap();
                assert_eq!(
                    from_text(&text, 2, &mut meter()).as_deref(),
                    Ok(bytes),
                    "{text}"
                );
            }
        }
    }
    assert!(accepted > 1000, "{accepted}");
}

#[test]
fn offset_and_depth_follow_the_heads() {
    // [[1], 2]: heads at 0 (outer array), 1 (inner array), 2 (1), 3 (2).
    let bytes = [0x82, 0x81, 0x01, 0x02];
    let mut decoder = Decoder::new(&bytes, 2);
    let mut m = meter();
    let mut seen = Vec::new();
    loop {
        let at = (decoder.offset(), decoder.depth());
        let Some(item) = decoder.next(&mut m).unwrap() else {
            break;
        };
        seen.push((at, item));
    }
    assert_eq!(
        seen,
        vec![
            ((0, 0), Item::Array(2)),
            ((1, 1), Item::Array(1)),
            ((2, 2), Item::Uint(1)),
            ((3, 1), Item::Uint(2)),
        ]
    );
    assert_eq!((decoder.offset(), decoder.depth()), (4, 0));
    assert_eq!(decoder.end(), Ok(4));
}

#[test]
fn end_is_an_error_until_the_item_is_complete() {
    let truncated = |len| {
        Err(CborError {
            offset: len,
            kind: CborErrorKind::Truncated,
        })
    };
    // Before anything is read, the item is not complete.
    assert_eq!(Decoder::new(&[0x00], 0).end(), truncated(1));
    // Part-way through an array.
    let bytes = [0x82, 0x01, 0x02];
    let mut decoder = Decoder::new(&bytes, 1);
    decoder.next(&mut meter()).unwrap();
    assert_eq!(decoder.end(), truncated(3));
    // After an error inside an open array, nothing more is handed out and the item never ends.
    let bytes = [0x82, 0x01, 0xf7];
    let mut decoder = Decoder::new(&bytes, 1);
    let mut m = meter();
    decoder.next(&mut m).unwrap();
    decoder.next(&mut m).unwrap();
    assert!(decoder.next(&mut m).is_err());
    assert_eq!(decoder.next(&mut m), Ok(None));
    assert_eq!(decoder.end(), truncated(3));
}

#[test]
fn a_non_finite_value_past_the_first_scan_chunk_is_found_at_its_byte() {
    for row in [511_usize, 512, 600, 1023, 1024, 1500] {
        let mut values = vec![0.25; 1600];
        values[row] = f64::INFINITY;
        let bytes = bytes_of(&V::Floats(values));
        // Tag 86 takes two bytes, and a byte string of 12,800 bytes a three-byte head.
        let e = refused(&bytes);
        assert_eq!(
            (e.kind, e.offset),
            (CborErrorKind::NotFinite, 5 + 8 * row),
            "{row}"
        );
    }
}

#[test]
fn text_and_columns_pay_one_unit_per_scanned_chunk() {
    // A 10,000-byte text: one unit for its head and three for its 4 KiB chunks.
    let text = "x".repeat(10_000);
    let bytes = bytes_of(&V::T(text));
    assert!(matches!(
        check(&bytes, 0, &mut WorkMeter::new(Budget::of_work(3)))
            .unwrap_err()
            .kind,
        CborErrorKind::Stopped(_)
    ));
    assert_eq!(
        check(&bytes, 0, &mut WorkMeter::new(Budget::of_work(4))),
        Ok(())
    );
    // 513 values are two chunks of 512.
    let values: Vec<u8> = (0..513_i64).flat_map(i64::to_le_bytes).collect();
    let budget = |work| WorkMeter::new(Budget::new(work, u64::MAX));
    assert!(int_column(&values, &mut Vec::new(), &mut budget(1)).is_err());
    assert_eq!(int_column(&values, &mut Vec::new(), &mut budget(2)), Ok(()));
    let floats: Vec<u8> = (0..513_i32)
        .flat_map(|i| f64::from(i).to_le_bytes())
        .collect();
    assert!(float_column(&floats, &mut Vec::new(), &mut budget(1)).is_err());
    assert_eq!(
        float_column(&floats, &mut Vec::new(), &mut budget(2)),
        Ok(())
    );
}

#[test]
fn every_error_says_what_and_where() {
    let kinds = [
        CborErrorKind::Truncated,
        CborErrorKind::ReservedInfo,
        CborErrorKind::Indefinite,
        CborErrorKind::NonShortestHead,
        CborErrorKind::NonShortestFloat,
        CborErrorKind::NotFinite,
        CborErrorKind::UnsupportedSimple,
        CborErrorKind::UnsupportedTag,
        CborErrorKind::TagContent,
        CborErrorKind::NonCanonicalBignum,
        CborErrorKind::ColumnLength,
        CborErrorKind::InvalidUtf8,
        CborErrorKind::UnsortedKeys,
        CborErrorKind::DuplicateKey,
        CborErrorKind::TooDeep,
        CborErrorKind::TrailingBytes,
    ];
    let texts: std::collections::BTreeSet<String> = kinds.iter().map(ToString::to_string).collect();
    assert_eq!(texts.len(), kinds.len());
    assert!(texts.iter().all(|t| !t.is_empty()));
    let e = CborError {
        offset: 17,
        kind: CborErrorKind::UnsortedKeys,
    };
    assert_eq!(
        e.to_string(),
        "not SKG's deterministic CBOR at byte 17: map keys out of order"
    );
    let text_kinds = [
        TextErrorKind::Unexpected,
        TextErrorKind::UnexpectedEnd,
        TextErrorKind::IntegerRange,
        TextErrorKind::InexactFloat,
        TextErrorKind::BadEscape,
        TextErrorKind::BadHex,
        TextErrorKind::OpenComment,
        TextErrorKind::Trailing,
        TextErrorKind::Cbor(CborErrorKind::TooDeep),
    ];
    let texts: std::collections::BTreeSet<String> =
        text_kinds.iter().map(ToString::to_string).collect();
    assert_eq!(texts.len(), text_kinds.len());
    let e = TextError {
        line: 3,
        column: 9,
        kind: TextErrorKind::Cbor(CborErrorKind::DuplicateKey),
    };
    assert_eq!(
        e.to_string(),
        "not SKG's diagnostic text at line 3, column 9: a repeated map key"
    );
}

#[test]
fn text_escapes_tab_and_carriage_return_by_name_and_refuses_a_second_point() {
    let bytes = bytes_of(&V::T("a\tb\rc\u{b}".into()));
    assert_eq!(
        to_text(&bytes, 0, &mut meter()).unwrap(),
        "\"a\\tb\\rc\\u000b\""
    );
    assert_eq!(
        text_error("0x1.8.8p0", 0),
        TextError {
            line: 1,
            column: 3,
            kind: TextErrorKind::InexactFloat
        }
    );
    assert_eq!(
        text_error("18446744073709551616(h'')", 1).kind,
        TextErrorKind::IntegerRange
    );
}
