//! Fuzz target: `geom-op`'s canonical CBOR encoder, on any stream of calls, refuses without a
//! trace and writes only canonical items.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as a stream of encoder calls, as `geom-op`'s
//! `tests/fuzz.rs` does: each byte chooses the next call and the bytes after it give the call's
//! arguments, so every call is tried in every place, refused ones included (NaN and the
//! infinities, keys out of order or out of place, sets with repeats, items after the whole). Each
//! item that finishes starts a new encoder and may be handed to later calls.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws (the encoder's module header): a refused call leaves the encoder as it
//!   was, so a copy taken before the call finishes the same as the encoder after it; whatever
//!   finishes is one well-formed item with nothing after it, every length definite and every map's
//!   keys unsigned and ascending, and ciborium-ll, reading it head by head and writing each head
//!   again in its fewest bytes and each double as its shortest float, writes exactly the same
//!   bytes; and `geom-format`'s strict decoder (piece G91) accepts it whole.
//!
//! Method: the canonical form of RFC 8949 section 4.2.1
//! (<https://www.rfc-editor.org/rfc/rfc8949#section-4.2.1>) as chapter 24.3 restricts it
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`), checked against an
//! encoder written elsewhere, ciborium-ll 0.2.2 (the crate's dependencies), and against the
//! kernel's own decoder, the property harness of `geom-op`'s `tests/fuzz.rs` made a fuzz target
//! (28.5, `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: each call does work linear in its arguments; each finished item is read twice, by
//! ciborium-ll and by the decoder, each linear in its length, without recursion.

use ciborium_io::Read as _;
use ciborium_ll::{Decoder, Header};
use geom_op::cbor::{EncodeError, Encoder, Item};
use geom_run::{Budget, WorkMeter};

use crate::Finding;
use crate::cbor::hex;

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
    fn i64(&mut self) -> i64 {
        i64::from_le_bytes(core::array::from_fn(|_| self.byte()))
    }
    fn i128(&mut self) -> i128 {
        i128::from_le_bytes(core::array::from_fn(|_| self.byte()))
    }
    fn f64(&mut self) -> f64 {
        match self.byte() % 4 {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::from(f32::from_bits(u32::from_le_bytes(core::array::from_fn(
                |_| self.byte(),
            )))),
            _ => f64::from_bits(self.u64()),
        }
    }
    fn small(&mut self) -> u8 {
        self.byte() % 8
    }
    fn bytes(&mut self, n: u8) -> Vec<u8> {
        (0..n).map(|_| self.byte()).collect()
    }
}

/// One call chosen by `op`, its arguments read from `args`.
fn call(e: &mut Encoder, op: u8, args: &mut Args<'_>, made: &[Item]) -> Result<(), EncodeError> {
    match op % 14 {
        0 => e.uint(args.u64()),
        1 => e.int(args.i128()),
        2 => e.float(args.f64()),
        3 => {
            let n = args.small().saturating_mul(5);
            e.bytes(&args.bytes(n))
        }
        4 => {
            let n = args.small().saturating_mul(3);
            e.text(&String::from_utf8_lossy(&args.bytes(n)))
        }
        5 => {
            let column: Vec<i64> = (0..args.small()).map(|_| args.i64()).collect();
            e.i64_column(&column)
        }
        6 => {
            let column: Vec<f64> = (0..args.small()).map(|_| args.f64()).collect();
            e.f64_column(&column)
        }
        7 => e.array(u64::from(args.small())),
        8 => e.map(u64::from(args.small())),
        9 | 10 => {
            let key = u64::from(args.byte());
            e.key(key.checked_shl(u32::from(args.byte() % 64)).unwrap_or(key))
        }
        11 => match made.get(usize::from(args.small())) {
            Some(item) => e.item(item),
            None => e.uint(0),
        },
        12 => {
            let picked: Vec<Item> = (0..args.small())
                .filter_map(|_| made.get(usize::from(args.small())).cloned())
                .collect();
            if args.byte() % 2 == 0 {
                e.set(picked)
            } else {
                e.sorted(picked)
            }
        }
        _ => e.clone().finish().map(drop),
    }
}

/// What is left to read of an open container.
enum Open {
    /// Items, of an array, a tag or the whole.
    Items(u64),
    /// Key and value pairs of a map, the last key read, and whether a key comes next.
    Map(u64, Option<u64>, bool),
}

/// The bytes ciborium-ll writes back from reading `item` head by head, which must use all of them;
/// anything outside SKG's subset or out of its deterministic form is refused.
///
/// # Errors
///
/// Why `item` is not one item of the subset.
pub fn rewritten(item: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = Decoder::from(item);
    let mut again = Vec::new();
    let mut out = ciborium_ll::Encoder::from(&mut again);
    let mut open = vec![Open::Items(1)];
    while let Some(top) = open.last_mut() {
        match top {
            Open::Items(0) | Open::Map(0, _, true) => {
                open.pop();
                continue;
            }
            Open::Items(left) => *left = left.saturating_sub(1),
            Open::Map(_, last, want) if *want => {
                let Header::Positive(key) = dec.pull().map_err(|e| format!("{e:?}"))? else {
                    return Err("a map key that is not an unsigned integer".into());
                };
                if last.is_some_and(|before| key <= before) {
                    return Err(format!("map key {key} after {last:?}"));
                }
                (*last, *want) = (Some(key), false);
                out.push(Header::Positive(key))
                    .map_err(|e| format!("{e:?}"))?;
                continue;
            }
            Open::Map(left, _, want) => (*left, *want) = (left.saturating_sub(1), true),
        }
        let header = dec.pull().map_err(|e| format!("{e:?}"))?;
        let rest = item.len().saturating_sub(dec.offset());
        match header {
            Header::Positive(_) | Header::Negative(_) | Header::Float(_) => {}
            Header::Tag(2 | 3 | 79 | 86) => open.push(Open::Items(1)),
            Header::Bytes(Some(n)) | Header::Text(Some(n)) if n <= rest => {
                let mut buf = vec![0; n];
                dec.read_exact(&mut buf).map_err(|e| format!("{e:?}"))?;
                match header {
                    Header::Text(_) => {
                        let text = String::from_utf8(buf).map_err(|e| e.to_string())?;
                        out.text(&text, None).map_err(|e| format!("{e:?}"))?;
                    }
                    _ => out.bytes(&buf, None).map_err(|e| format!("{e:?}"))?,
                }
                continue;
            }
            Header::Array(Some(n)) => open.push(Open::Items(u64::try_from(n).unwrap_or(u64::MAX))),
            Header::Map(Some(n)) => {
                open.push(Open::Map(u64::try_from(n).unwrap_or(u64::MAX), None, true));
            }
            other => return Err(format!("outside the subset: {other:?}")),
        }
        out.push(header).map_err(|e| format!("{e:?}"))?;
    }
    if dec.offset() != item.len() {
        return Err(format!("bytes after the item, from {}", dec.offset()));
    }
    Ok(again)
}

/// Runs the stream of calls `input` gives; the items finished along the way.
///
/// # Errors
///
/// A [`Finding`] if a refused call changed the encoder.
pub fn drive(input: &[u8]) -> Result<Vec<Vec<u8>>, Finding> {
    let mut args = Args(input);
    let mut made: Vec<Item> = Vec::new();
    let mut finished = Vec::new();
    let mut e = Encoder::new();
    while !args.0.is_empty() {
        let op = args.byte();
        let before = e.clone();
        if call(&mut e, op, &mut args, &made).is_err() {
            let (was, now) = (
                format!("{:?}", before.finish()),
                format!("{:?}", e.clone().finish()),
            );
            if was != now {
                return Err(Finding::new(
                    "a refused call leaves the encoder as it was",
                    || format!("{}: call {op} turned {was} into {now}", hex(input)),
                ));
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

/// The fuzz harness: [`drive`], then the laws on every item that finished.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let depth = data.len().saturating_mul(2).saturating_add(2);
    for item in drive(data)? {
        match rewritten(&item) {
            Ok(again) if again == item => {}
            other => {
                return Err(Finding::new("a finished item is canonical", || {
                    format!("{}: {} gave {other:?}", hex(data), hex(&item))
                }));
            }
        }
        let meter = &mut WorkMeter::new(Budget::UNLIMITED);
        if geom_format::cbor::item_len(&item, depth, meter) != Ok(item.len()) {
            return Err(Finding::new(
                "the decoder reads a finished item whole",
                || format!("{}: {}", hex(data), hex(&item)),
            ));
        }
    }
    Ok(())
}
