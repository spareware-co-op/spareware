//! The fuzz harness of the canonical CBOR encoder (30.1's standard bar: a fuzz target with no
//! panic and no hang), driven here as a property test over arbitrary bytes, as `geom-math`'s
//! `tests/fuzz.rs` drives its operations: each input byte chooses the next call and the bytes after
//! it give the call's arguments, so every call is tried in every place, refused ones included
//! (NaN and the infinities, keys out of order or out of place, sets with repeats, items after the
//! whole). What the harness proves, for every input:
//!
//! - no panic, and every call returns (each does work linear in its arguments);
//! - a refused call is one of `EncodeError`'s documented refusals, and leaves the encoder as it was:
//!   a copy taken before the call finishes the same as the encoder after it;
//! - whatever finishes is one well-formed CBOR data item with nothing after it, every length
//!   definite and every map's keys unsigned and ascending, and ciborium-ll, reading it head by head
//!   and writing each head again in its fewest bytes and each double as its shortest float, writes
//!   exactly the same bytes: the item is canonical.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "a test harness; the rules are for kernel code"
)]

use ciborium_io::Read as _;
use ciborium_ll::{Decoder, Header};
use geom_op::cbor::{EncodeError, Encoder, Item};
use geom_test::seed::Seed;
use proptest::prelude::*;

/// Reads the input's bytes as arguments, zeros once they run out.
struct Args<'a>(&'a [u8]);

impl Args<'_> {
    fn byte(&mut self) -> u8 {
        let (first, rest) = self.0.split_first().map_or((0, &[][..]), |(f, r)| (*f, r));
        self.0 = rest;
        first
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(core::array::from_fn(|_| self.byte()))
    }
    fn i128(&mut self) -> i128 {
        i128::from_le_bytes(core::array::from_fn(|_| self.byte()))
    }
    fn f64(&mut self) -> f64 {
        match self.byte() % 4 {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::from(f32::from_bits(self.u64() as u32)),
            _ => f64::from_bits(self.u64()),
        }
    }
    fn small(&mut self) -> usize {
        usize::from(self.byte() % 8)
    }
}

/// One call chosen by `op`, its arguments read from `args`.
fn call(e: &mut Encoder, op: u8, args: &mut Args<'_>, made: &[Item]) -> Result<(), EncodeError> {
    match op % 14 {
        0 => e.uint(args.u64()),
        1 => e.int(args.i128()),
        2 => e.float(args.f64()),
        3 => {
            let n = args.small() * 5;
            let bytes: Vec<u8> = (0..n).map(|_| args.byte()).collect();
            e.bytes(&bytes)
        }
        4 => {
            let n = args.small() * 3;
            let bytes: Vec<u8> = (0..n).map(|_| args.byte()).collect();
            e.text(&String::from_utf8_lossy(&bytes))
        }
        5 => {
            let column: Vec<i64> = (0..args.small()).map(|_| args.u64() as i64).collect();
            e.i64_column(&column)
        }
        6 => {
            let column: Vec<f64> = (0..args.small()).map(|_| args.f64()).collect();
            e.f64_column(&column)
        }
        7 => e.array(args.small() as u64),
        8 => e.map(args.small() as u64),
        9 | 10 => e.key(u64::from(args.byte()) << (args.byte() % 64)),
        11 => match made.get(args.small()) {
            Some(item) => e.item(item),
            None => e.uint(0),
        },
        12 => {
            let picked: Vec<Item> = (0..args.small())
                .filter_map(|_| made.get(args.small()).cloned())
                .collect();
            if args.byte().is_multiple_of(2) {
                e.set(picked)
            } else {
                e.sorted(picked)
            }
        }
        _ => e.clone().finish().map(drop),
    }
}

/// Reads one item from `dec` and writes it again through ciborium-ll; refuses anything outside
/// SKG's subset or not in its deterministic form.
fn rewrite(
    dec: &mut Decoder<&[u8]>,
    out: &mut ciborium_ll::Encoder<&mut Vec<u8>>,
) -> Result<(), String> {
    let header = dec.pull().map_err(|e| format!("{e:?}"))?;
    match header {
        Header::Positive(_) | Header::Negative(_) | Header::Float(_) => {
            out.push(header).unwrap();
        }
        Header::Tag(tag @ (2 | 3 | 79 | 86)) => {
            out.push(Header::Tag(tag)).unwrap();
            rewrite(dec, out)?;
        }
        Header::Bytes(Some(n)) => {
            let mut buf = vec![0; n];
            dec.read_exact(&mut buf).map_err(|e| format!("{e:?}"))?;
            out.bytes(&buf, None).unwrap();
        }
        Header::Text(Some(n)) => {
            let mut buf = vec![0; n];
            dec.read_exact(&mut buf).map_err(|e| format!("{e:?}"))?;
            let text = String::from_utf8(buf).map_err(|e| e.to_string())?;
            out.text(&text, None).unwrap();
        }
        Header::Array(Some(n)) => {
            out.push(header).unwrap();
            for _ in 0..n {
                rewrite(dec, out)?;
            }
        }
        Header::Map(Some(n)) => {
            out.push(header).unwrap();
            let mut last = None;
            for _ in 0..n {
                let Header::Positive(key) = dec.pull().map_err(|e| format!("{e:?}"))? else {
                    return Err("a map key that is not an unsigned integer".into());
                };
                if last.is_some_and(|before| key <= before) {
                    return Err(format!("map key {key} after {last:?}"));
                }
                last = Some(key);
                out.push(Header::Positive(key)).unwrap();
                rewrite(dec, out)?;
            }
        }
        other => return Err(format!("outside the subset: {other:?}")),
    }
    Ok(())
}

/// The bytes ciborium-ll writes back from reading `item`, which must use all of them.
fn rewritten(item: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = Decoder::from(item);
    let mut again = Vec::new();
    {
        let mut out = ciborium_ll::Encoder::from(&mut again);
        rewrite(&mut dec, &mut out)?;
    }
    if dec.offset() != item.len() {
        return Err(format!(
            "{} bytes after the item",
            item.len() - dec.offset()
        ));
    }
    Ok(again)
}

/// Runs a stream of calls; returns the items finished along the way.
fn drive(input: &[u8]) -> Result<Vec<Vec<u8>>, TestCaseError> {
    let mut args = Args(input);
    let mut made: Vec<Item> = Vec::new();
    let mut finished = Vec::new();
    let mut e = Encoder::new();
    while !args.0.is_empty() {
        let op = args.byte();
        let before = e.clone();
        match call(&mut e, op, &mut args, &made) {
            Ok(()) => {}
            Err(_) => {
                // A refusal changed nothing: the copy and the encoder finish alike.
                prop_assert_eq!(
                    format!("{:?}", before.clone().finish()),
                    format!("{:?}", e.clone().finish())
                );
            }
        }
        if let Ok(item) = e.clone().finish() {
            finished.push(item.as_bytes().to_vec());
            made.push(item);
            e = Encoder::new();
        }
    }
    Ok(finished)
}

#[test]
fn any_stream_of_calls_writes_only_canonical_items() {
    let result = geom_test::prop::run(
        "cbor fuzz",
        Seed(0x6386_f022),
        3000,
        &proptest::collection::vec(any::<u8>(), 0..1500),
        |input| {
            for item in drive(&input)? {
                match rewritten(&item) {
                    Ok(again) => prop_assert_eq!(again, item),
                    Err(why) => prop_assert!(false, "{why}: {item:02x?}"),
                }
            }
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}

/// The harness's own check: a stream that writes a known item is read back, and bytes that are
/// not canonical are caught (a head in two bytes where one does, a double wider than it needs, a
/// map key out of order, a byte after the item).
#[test]
fn the_check_catches_what_is_not_canonical() {
    // An array of two (op 7, length 2), then two empty texts (op 4, length 0).
    let finished = drive(&[7, 2, 4, 0, 4, 0]).unwrap_or_default();
    assert_eq!(finished, [vec![0x82, 0x60, 0x60]]);
    assert_eq!(rewritten(&finished[0]).as_deref(), Ok(&finished[0][..]));
    for bad in [
        &[0x18, 0x05][..],
        &[0xfa, 0x3f, 0x80, 0x00, 0x00],
        &[0xa2, 0x02, 0x00, 0x01, 0x00],
        &[0x01, 0x01],
        &[0x20 | 0x18, 0x02],
    ] {
        let again = rewritten(bad);
        assert!(
            again.as_deref() != Ok(bad),
            "{bad:02x?} passed as canonical"
        );
    }
}
