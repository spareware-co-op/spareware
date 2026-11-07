//! The value model shared by the CBOR test files: SKG's subset as a test-side tree ([`V`]), a
//! proptest strategy that draws it, the calls that write it with the encoder under test, an
//! independent reference encoding of it through ciborium-ll (Apache-2.0), and the JSON form the
//! cbor2 oracle reads and writes (`oracles/cbor2.py`). The reference never calls the code under
//! test.

#![allow(
    dead_code,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    reason = "each test file uses part of the model; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ciborium_io::Write as _;
use ciborium_ll::Header;
use geom_op::cbor::{EncodeError, Encoder, Item};
use proptest::prelude::*;
use serde_json::{Value, json};

/// One value of SKG's subset.
#[derive(Clone, Debug)]
pub enum V {
    /// An unsigned integer.
    U(u64),
    /// A signed integer, a bignum beyond 64 bits.
    I(i128),
    /// A finite double.
    F(f64),
    /// A byte string.
    B(Vec<u8>),
    /// A text string.
    T(String),
    /// An array.
    A(Vec<V>),
    /// A map, its keys strictly ascending.
    M(Vec<(u64, V)>),
    /// A typed array of `i64`, tag 79.
    Ci(Vec<i64>),
    /// A typed array of finite `f64`, tag 86.
    Cf(Vec<f64>),
    /// A set: elements whose encodings differ.
    Set(Vec<V>),
    /// A sorted list: repeats allowed.
    Sorted(Vec<V>),
}

/// Writes `v` with the encoder under test.
pub fn write(e: &mut Encoder, v: &V) -> Result<(), EncodeError> {
    match v {
        V::U(n) => e.uint(*n),
        V::I(n) => e.int(*n),
        V::F(x) => e.float(*x),
        V::B(b) => e.bytes(b),
        V::T(t) => e.text(t),
        V::A(xs) => {
            e.array(xs.len() as u64)?;
            xs.iter().try_for_each(|x| write(e, x))
        }
        V::M(kv) => {
            e.map(kv.len() as u64)?;
            kv.iter().try_for_each(|(k, x)| {
                e.key(*k)?;
                write(e, x)
            })
        }
        V::Ci(xs) => e.i64_column(xs),
        V::Cf(xs) => e.f64_column(xs),
        V::Set(xs) => e.set(items(xs)?),
        V::Sorted(xs) => e.sorted(items(xs)?),
    }
}

/// Each value's own finished item.
fn items(xs: &[V]) -> Result<Vec<Item>, EncodeError> {
    xs.iter().map(encode).collect()
}

/// `v` as one finished item, by the encoder under test.
pub fn encode(v: &V) -> Result<Item, EncodeError> {
    let mut e = Encoder::new();
    write(&mut e, v)?;
    e.finish()
}

/// `v` by the encoder under test, as bytes.
pub fn ours(v: &V) -> Vec<u8> {
    encode(v).unwrap().into_bytes()
}

/// `v` encoded through ciborium-ll, which writes each head in its fewest bytes and each double as
/// the shortest float that keeps its bits; the deterministic rules (key order, bignums, sorting
/// sets) are applied here by RFC 8949's text, independently of the code under test.
pub fn reference(v: &V) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = ciborium_ll::Encoder::from(&mut out);
        reference_into(&mut enc, v);
    }
    out
}

fn reference_into<W: ciborium_io::Write>(enc: &mut ciborium_ll::Encoder<W>, v: &V)
where
    W::Error: core::fmt::Debug,
{
    match v {
        V::U(n) => enc.push(Header::Positive(*n)).unwrap(),
        V::I(n) => {
            if let Ok(small) = u64::try_from(*n) {
                enc.push(Header::Positive(small)).unwrap();
            } else if *n < 0 && *n >= -(1_i128 << 64) {
                enc.push(Header::Negative((-1 - *n) as u64)).unwrap();
            } else {
                let (tag, magnitude) = if *n < 0 {
                    (3, (-1 - *n) as u128)
                } else {
                    (2, *n as u128)
                };
                let skip = (magnitude.leading_zeros() / 8) as usize;
                enc.push(Header::Tag(tag)).unwrap();
                enc.bytes(&magnitude.to_be_bytes()[skip..], None).unwrap();
            }
        }
        V::F(x) => enc.push(Header::Float(*x)).unwrap(),
        V::B(b) => enc.bytes(b, None).unwrap(),
        V::T(t) => enc.text(t, None).unwrap(),
        V::A(xs) => {
            enc.push(Header::Array(Some(xs.len()))).unwrap();
            for x in xs {
                reference_into(enc, x);
            }
        }
        V::M(kv) => {
            enc.push(Header::Map(Some(kv.len()))).unwrap();
            for (k, x) in kv {
                enc.push(Header::Positive(*k)).unwrap();
                reference_into(enc, x);
            }
        }
        V::Ci(xs) => {
            let bytes: Vec<u8> = xs.iter().flat_map(|x| x.to_le_bytes()).collect();
            enc.push(Header::Tag(79)).unwrap();
            enc.bytes(&bytes, None).unwrap();
        }
        V::Cf(xs) => {
            let bytes: Vec<u8> = xs.iter().flat_map(|x| x.to_le_bytes()).collect();
            enc.push(Header::Tag(86)).unwrap();
            enc.bytes(&bytes, None).unwrap();
        }
        V::Set(xs) | V::Sorted(xs) => {
            let mut each: Vec<Vec<u8>> = xs.iter().map(reference).collect();
            each.sort();
            enc.push(Header::Array(Some(each.len()))).unwrap();
            for bytes in each {
                enc.write_all(&bytes).unwrap();
            }
        }
    }
}

/// Lowercase hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// Bytes from hexadecimal.
pub fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "odd hexadecimal: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// `v` in the JSON form `oracles/cbor2.py` reads: one key per value naming its kind, integers as
/// decimal strings, doubles as the sixteen hexadecimal digits of their bits.
pub fn to_json(v: &V) -> Value {
    match v {
        V::U(n) => json!({ "u": n.to_string() }),
        V::I(n) => json!({ "i": n.to_string() }),
        V::F(x) => json!({ "f": format!("{:016x}", x.to_bits()) }),
        V::B(b) => json!({ "b": hex(b) }),
        V::T(t) => json!({ "t": t }),
        V::A(xs) => json!({ "a": xs.iter().map(to_json).collect::<Vec<_>>() }),
        V::M(kv) => json!({
            "m": kv.iter().map(|(k, x)| json!([k.to_string(), to_json(x)])).collect::<Vec<_>>()
        }),
        V::Ci(xs) => json!({ "ci": xs.iter().map(ToString::to_string).collect::<Vec<_>>() }),
        V::Cf(xs) => json!({
            "cf": xs.iter().map(|x| format!("{:016x}", x.to_bits())).collect::<Vec<_>>()
        }),
        V::Set(xs) => json!({ "set": xs.iter().map(to_json).collect::<Vec<_>>() }),
        V::Sorted(xs) => json!({ "sorted": xs.iter().map(to_json).collect::<Vec<_>>() }),
    }
}

/// The value a JSON form describes.
pub fn from_json(j: &Value) -> V {
    let object = j.as_object().unwrap();
    assert!(object.len() == 1, "one kind per value: {j}");
    let (kind, x) = object.iter().next().unwrap();
    let list = || x.as_array().unwrap().iter().map(from_json).collect();
    let bits = |s: &Value| f64::from_bits(u64::from_str_radix(s.as_str().unwrap(), 16).unwrap());
    match kind.as_str() {
        "u" => V::U(x.as_str().unwrap().parse().unwrap()),
        "i" => V::I(x.as_str().unwrap().parse().unwrap()),
        "f" => V::F(bits(x)),
        "b" => V::B(unhex(x.as_str().unwrap())),
        "t" => V::T(x.as_str().unwrap().to_owned()),
        "a" => V::A(list()),
        "m" => V::M(
            x.as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().unwrap();
                    (
                        pair[0].as_str().unwrap().parse().unwrap(),
                        from_json(&pair[1]),
                    )
                })
                .collect(),
        ),
        "ci" => V::Ci(
            x.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_str().unwrap().parse().unwrap())
                .collect(),
        ),
        "cf" => V::Cf(x.as_array().unwrap().iter().map(bits).collect()),
        "set" => V::Set(list()),
        "sorted" => V::Sorted(list()),
        other => panic!("no such kind: {other}"),
    }
}

/// Finite doubles, weighted towards the edges of the three widths: binary16 and binary32 values,
/// their neighbours, zeros, subnormals and the extremes, and doubles of any bits.
pub fn finite() -> BoxedStrategy<f64> {
    let special = prop_oneof![
        Just(0.0),
        Just(-0.0),
        Just(f64::MIN_POSITIVE),
        Just(f64::from_bits(1)),
        Just(f64::MAX),
        Just(f64::MIN),
        Just(65504.0),
        Just(65505.0),
        Just(2f64.powi(-24)),
        Just(2f64.powi(-25)),
        Just(2f64.powi(-14)),
        Just(2f64.powi(-149)),
        Just(2f64.powi(-150)),
        Just(2f64.powi(-126)),
        Just(f64::from(f32::MAX)),
    ];
    let half = any::<u16>().prop_map(|h| half::f16::from_bits(h).to_f64());
    let single = any::<u32>().prop_map(|s| f64::from(f32::from_bits(s)));
    let double = any::<u64>().prop_map(f64::from_bits);
    let near = (any::<u32>(), -2_i64..=2).prop_map(|(s, d)| {
        let x = f64::from(f32::from_bits(s));
        f64::from_bits(x.to_bits().wrapping_add_signed(d))
    });
    prop_oneof![special, half, single, double, near]
        .prop_filter("finite", |x| x.is_finite())
        .boxed()
}

/// Integers across every width a head or bignum takes.
pub fn integer() -> BoxedStrategy<i128> {
    prop_oneof![
        -30_i128..30,
        any::<i64>().prop_map(i128::from),
        any::<u64>().prop_map(i128::from),
        any::<u64>().prop_map(|n| -1 - i128::from(n)),
        any::<i128>(),
        (0_u32..127).prop_map(|k| 1_i128 << k),
        (0_u32..128).prop_map(|k| -1_i128 << k),
    ]
    .boxed()
}

/// Unsigned integers across every head width.
pub fn unsigned() -> BoxedStrategy<u64> {
    prop_oneof![
        0_u64..30,
        230_u64..300,
        65500_u64..65600,
        Just(u64::from(u32::MAX)),
        Just(u64::from(u32::MAX) + 1),
        any::<u64>(),
    ]
    .boxed()
}

/// One scalar of the subset.
pub fn scalar() -> BoxedStrategy<V> {
    prop_oneof![
        unsigned().prop_map(V::U),
        integer().prop_map(V::I),
        finite().prop_map(V::F),
        proptest::collection::vec(any::<u8>(), 0..300).prop_map(V::B),
        ".{0,40}".prop_map(V::T),
        proptest::collection::vec(any::<i64>(), 0..12).prop_map(V::Ci),
        proptest::collection::vec(finite(), 0..12).prop_map(V::Cf),
    ]
    .boxed()
}

/// Values of the subset, nested up to four deep.
pub fn value() -> BoxedStrategy<V> {
    scalar()
        .prop_recursive(4, 64, 8, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..30).prop_map(V::A),
                proptest::collection::btree_map(unsigned(), inner.clone(), 0..8)
                    .prop_map(|m| V::M(m.into_iter().collect())),
                proptest::collection::vec(inner.clone(), 0..8).prop_map(|xs| {
                    let distinct: BTreeMap<Vec<u8>, V> =
                        xs.into_iter().map(|x| (reference(&x), x)).collect();
                    V::Set(distinct.into_values().collect())
                }),
                proptest::collection::vec(inner, 0..8).prop_map(V::Sorted),
            ]
        })
        .boxed()
}
