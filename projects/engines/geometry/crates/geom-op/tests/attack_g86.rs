//! Adversarial tests for piece G86 (`geom-op`'s canonical CBOR encoder and SHA-256).
//!
//! Each test tries one way a claim of `cbor` or `hash` could be wrong, against an oracle that is
//! not the code under test: the widths IEEE 754 gives each power of two, refusals slipped into a
//! valid run of calls at random, two routes to the same value, map keys at their limits, and the
//! 128-bit cut against the whole digest. A test that fails names the claim it breaks.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{V, encode, hex, ours};
use geom_op::cbor::{EncodeError, Encoder, Item};
use geom_op::hash::{Sha256, sha256, sha256_128};
use geom_test::seed::Seed;
use proptest::prelude::*;

/// 2^k as a double, for k from −1074 to 1023, built from its bits.
fn pow2(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1_u64 << (k + 1074))
    }
}

/// "Shortest float": every power of two takes the width IEEE 754 gives it. binary16 holds 2^k for
/// k from −24 (its least subnormal) to 15, binary32 from −149 to 127, and every other power needs
/// binary64; the sign never changes the width.
#[test]
fn every_power_of_two_takes_its_ieee_width() {
    for k in -1074..=1023 {
        let want = if (-24..=15).contains(&k) {
            3
        } else if (-149..=127).contains(&k) {
            5
        } else {
            9
        };
        for x in [pow2(k), -pow2(k)] {
            let bytes = ours(&V::F(x));
            assert_eq!(bytes.len(), want, "2^{k} ({x:e})");
        }
    }
}

/// "Shortest float", at the edges where rounding would hide a wrong answer: the largest binary16
/// and the doubles just past it, the least binary16 subnormal and a value between its steps, and
/// binary32's largest and least values with their binary64 neighbours.
#[test]
fn the_edges_of_each_width() {
    let next = |x: f64| f64::from_bits(x.to_bits() + 1);
    let cases: &[(f64, usize)] = &[
        (65504.0, 3),
        (next(65504.0), 9),
        // 65520 is a binary32, and rounds to infinity in binary16.
        (65520.0, 5),
        (65536.0, 5),
        (pow2(-24), 3),
        (1.5 * pow2(-24), 5),
        (pow2(-14) - pow2(-24), 3),
        (pow2(-14), 3),
        (pow2(-14) + pow2(-25), 5),
        (f64::from(f32::MAX), 5),
        (next(f64::from(f32::MAX)), 9),
        (pow2(-149), 5),
        (1.5 * pow2(-149), 9),
        (pow2(-126) - pow2(-149), 5),
        (f64::MAX, 9),
        (f64::from_bits(1), 9),
        (1.0 + pow2(-10), 3),
        (1.0 + pow2(-11), 5),
        (1.0 + pow2(-23), 5),
        (1.0 + pow2(-24), 9),
    ];
    for &(x, want) in cases {
        assert_eq!(ours(&V::F(x)).len(), want, "{x:e}");
        assert_eq!(ours(&V::F(-x)).len(), want, "{:e}", -x);
    }
}

/// "One encoding per value": a whole double stays a double (24.3 forbids dCBOR's numeric
/// reduction), −0.0 is not 0.0, and an integer is the same bytes through `uint` and `int`.
#[test]
fn two_routes_to_one_value() {
    assert_eq!(hex(&ours(&V::F(1.0))), "f93c00");
    assert_ne!(ours(&V::F(1.0)), ours(&V::U(1)));
    assert_eq!(hex(&ours(&V::F(-0.0))), "f98000");
    assert_eq!(hex(&ours(&V::F(0.0))), "f90000");
    for n in [
        0,
        1,
        23,
        24,
        255,
        256,
        65535,
        65536,
        u64::from(u32::MAX),
        u64::MAX,
    ] {
        assert_eq!(ours(&V::U(n)), ours(&V::I(i128::from(n))), "{n}");
    }
}

/// "Map keys ascending": the least and greatest keys work, and nothing follows the greatest.
#[test]
fn map_keys_at_their_limits() {
    let mut e = Encoder::new();
    e.map(3).unwrap();
    e.key(0).unwrap();
    e.uint(0).unwrap();
    assert_eq!(e.key(0), Err(EncodeError::KeyNotAscending));
    e.key(u64::MAX).unwrap();
    e.uint(1).unwrap();
    assert_eq!(e.key(u64::MAX), Err(EncodeError::KeyNotAscending));
    assert_eq!(e.key(1), Err(EncodeError::KeyNotAscending));
    assert_eq!(e.clone().finish(), Err(EncodeError::Unfinished));
}

/// One call of a run: the calls that write a value, in order.
#[derive(Clone, Debug)]
enum Call {
    Uint(u64),
    Int(i128),
    Float(f64),
    Bytes(Vec<u8>),
    Text(String),
    I64s(Vec<i64>),
    F64s(Vec<f64>),
    Array(u64),
    Map(u64),
    Key(u64),
    Item(Item),
}

/// The calls that write `v`, flattened; sets and sorted lists go in as their sorted items.
fn calls(v: &V, out: &mut Vec<Call>) {
    match v {
        V::U(n) => out.push(Call::Uint(*n)),
        V::I(n) => out.push(Call::Int(*n)),
        V::F(x) => out.push(Call::Float(*x)),
        V::B(b) => out.push(Call::Bytes(b.clone())),
        V::T(t) => out.push(Call::Text(t.clone())),
        V::Ci(xs) => out.push(Call::I64s(xs.clone())),
        V::Cf(xs) => out.push(Call::F64s(xs.clone())),
        V::A(xs) => {
            out.push(Call::Array(xs.len() as u64));
            for x in xs {
                calls(x, out);
            }
        }
        V::M(kv) => {
            out.push(Call::Map(kv.len() as u64));
            for (k, x) in kv {
                out.push(Call::Key(*k));
                calls(x, out);
            }
        }
        V::Set(_) | V::Sorted(_) => out.push(Call::Item(encode(v).unwrap())),
    }
}

fn apply(e: &mut Encoder, call: &Call) -> Result<(), EncodeError> {
    match call {
        Call::Uint(n) => e.uint(*n),
        Call::Int(n) => e.int(*n),
        Call::Float(x) => e.float(*x),
        Call::Bytes(b) => e.bytes(b),
        Call::Text(t) => e.text(t),
        Call::I64s(xs) => e.i64_column(xs),
        Call::F64s(xs) => e.f64_column(xs),
        Call::Array(n) => e.array(*n),
        Call::Map(n) => e.map(*n),
        Call::Key(k) => e.key(*k),
        Call::Item(item) => e.item(item),
    }
}

/// Calls that are refused at some point of a run, whatever the point: each is wrong in itself.
fn always_refused() -> Vec<Call> {
    vec![
        Call::Float(f64::NAN),
        Call::Float(f64::INFINITY),
        Call::Float(f64::NEG_INFINITY),
        Call::F64s(vec![0.0, f64::NAN]),
        Call::F64s(vec![f64::NEG_INFINITY]),
    ]
}

/// "A refused call writes nothing": a valid run of calls, with refused calls slipped in at random
/// places, writes the same bytes as the run alone. Calls that are valid somewhere but refused where
/// they land (a key where an item goes, an item after the whole) are tried at every place, and kept
/// only where refused.
#[test]
fn refusals_slipped_into_a_run_change_nothing() {
    let strategy = (common::value(), any::<u64>(), 0_usize..4);
    let result = geom_test::prop::run(
        "refusals slipped into a run",
        Seed(0x6386_a7a1),
        400,
        &strategy,
        |(v, salt, every)| {
            let mut run = Vec::new();
            calls(&v, &mut run);
            let clean = ours(&v);
            let mut e = Encoder::new();
            let mut refused = 0_usize;
            for (place, call) in run.iter().enumerate() {
                if (place + salt as usize).is_multiple_of(every + 1) {
                    for bad in always_refused() {
                        prop_assert!(apply(&mut e, &bad).is_err());
                        refused += 1;
                    }
                    // A key and an item, each tried on a copy first: kept only where refused.
                    for probe in [Call::Key(salt), Call::Uint(salt)] {
                        if apply(&mut e.clone(), &probe).is_err() {
                            prop_assert!(apply(&mut e, &probe).is_err());
                            refused += 1;
                        }
                    }
                    let repeat = encode(&V::U(salt)).unwrap();
                    prop_assert_eq!(
                        e.clone().set(vec![repeat.clone(), repeat]),
                        Err(EncodeError::Repeated)
                    );
                }
                prop_assert!(apply(&mut e, call).is_ok(), "{call:?}");
            }
            for bad in always_refused() {
                prop_assert!(apply(&mut e, &bad).is_err());
            }
            prop_assert_eq!(apply(&mut e, &Call::Uint(0)), Err(EncodeError::Complete));
            prop_assert!(refused > 0 || run.len() < 2);
            prop_assert_eq!(hex(e.finish().unwrap().as_bytes()), hex(&clean));
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}

/// "The 128-bit cut is the digest's first 16 bytes", for messages of every length to 300 bytes
/// and seeded contents, whole and through the streaming hasher.
#[test]
fn the_cut_is_the_digest_prefix() {
    let mut state = 0x0638_6c07_u64;
    for len in 0..300 {
        let message: Vec<u8> = (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 56) as u8
            })
            .collect();
        let whole = sha256(&message);
        assert_eq!(sha256_128(&message), whole[..16]);
        let mut hasher = Sha256::new();
        hasher.write(&message);
        assert_eq!(hasher.finish_128(), whole[..16]);
        assert_eq!(whole, geom_test::golden::sha256(&message));
    }
}
