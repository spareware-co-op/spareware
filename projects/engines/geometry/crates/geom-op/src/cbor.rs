//! Deterministic CBOR for SKG's subset: the one encoding every stored and hashed value has (24.3).
//!
//! What it does: an [`Encoder`] writes exactly one CBOR data item (RFC 8949) from SKG's subset of
//! values, in the core deterministic encoding of RFC 8949 section 4.2.1 with chapter 24.3's rules
//! on top, and [`Encoder::finish`] hands it back as an [`Item`]. The subset is what chapter 24.3
//! stores: unsigned and signed integers (big ones as bignums), finite doubles, byte strings, UTF-8
//! text, arrays, maps keyed by unsigned integers (24.3's extension maps), sets and sorted lists
//! (arrays sorted by the bytes of each element's encoding, 32.19), and coordinate columns as RFC
//! 8746 typed arrays of little-endian `i64` (tag 79) and `f64` (tag 86). An [`Item`] can only be
//! made by an encoder, so an item placed inside another ([`Encoder::item`], [`Encoder::set`],
//! [`Encoder::sorted`]) is canonical by construction. Persistent names (G87) and the native
//! format's records and decoder (G91, G92) are built on it, and never encode a value a second way.
//!
//! Guarantees:
//!
//! - **One encoding per value.** Every head carries its argument in the fewest bytes (RFC 8949
//!   sections 4.1 and 4.2.1); lengths are always definite; an integer is a bignum (tag 2 or 3) only
//!   when it does not fit major type 0 or 1, and then with no leading zero byte (24.3, RFC 8949
//!   section 3.4.3); a double is written as the shortest of binary16, binary32 and binary64 that
//!   holds its value bit for bit, so −0.0 stays −0.0 (RFC 8949 section 4.1; 24.3); map keys are
//!   written in ascending order, which for unsigned keys is the bytewise order of their encodings
//!   that RFC 8949 section 4.2.1 asks for; a set's elements are sorted by their encodings' bytes,
//!   without repeats, and a sorted list's the same way with repeats kept (32.19).
//! - **Only the subset, and only well-formed items.** NaN and the infinities are refused, as a
//!   double and inside an `f64` column (24.3); a map key that is not greater than the one before
//!   it, an item where a map wants its key, a key anywhere else, a second top-level item, a set
//!   with a repeat, and a finish before the item is whole are each refused with their own
//!   [`EncodeError`]. A refused call writes nothing and leaves the encoder as it was, so the caller
//!   can still finish the item correctly. An array or map closes as soon as it holds the items it
//!   declared, so the next item belongs to the container around it.
//! - **The same bytes on every target.** The bytes depend only on the calls made: every step is
//!   integer and bit work on the value's own bits (`f64::to_bits`), with no float arithmetic,
//!   no hash-map order and no platform width beyond `usize` lengths, which fit `u64` on every
//!   target Rust supports.
//! - **No panic.** Nothing here indexes, unwraps or overflows: lengths are counted with checked
//!   arithmetic and a length too large for the format is refused with [`EncodeError::TooLong`].
//!
//! Method: RFC 8949 (CBOR, IETF STD 94), sections 3 (the head and its argument), 3.4.3 (bignums),
//! 4.1 (preferred serialization, shortest floats) and 4.2.1 (core deterministic encoding),
//! <https://www.rfc-editor.org/rfc/rfc8949>; RFC 8746 section 2 (typed arrays: tag 79 for
//! little-endian `sint64`, tag 86 for little-endian binary64),
//! <https://www.rfc-editor.org/rfc/rfc8746>; IEEE 754-2019 section 3.4 for the binary16 and
//! binary32 interchange formats the shortest-float test narrows to; and the kernel's rules in
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` (24.3) and
//! `projects/engines/geometry/architecture/32-decisions.md` (32.19 and 32.20).
//!
//! Why our own code: no crate on the kernel's vetted list (`deny-no-std.toml`) encodes CBOR. The
//! `no_std` encoders on crates.io that were read (ciborium-ll 0.2.2, Apache-2.0; minicbor 1.1.0,
//! BlueOak-1.0.0, a licence chapter 29.2 does not list; cbor4ii 1.2.4, MIT) write one head at a
//! time and leave the deterministic rules, which are this module's job, to the caller: map order,
//! repeats, the bignum rule and 24.3's refusals. minicbor writes a double at the width it is
//! asked for. ciborium-ll chooses the shortest float the way this module does, but would bring
//! itself, ciborium-io and half into the kernel's allow-list for the forty lines of head encoding
//! below. dcbor (BSD-2-Clause-Patent, `std`) is deterministic, but its numeric reduction writes a
//! whole double as an integer, which 24.3 forbids. So the encoding is written here from RFC 8949,
//! and ciborium-ll and Python's cbor2 serve as its oracles in the tests, never as its code.
//!
//! Consulted: ciborium-ll, Apache-2.0,
//!   <https://github.com/enarx/ciborium/blob/fdf3ec5c53256000f8e12f06c361ca18026a8d99/ciborium-ll/src/hdr.rs>,
//!   its choice of the shortest float that keeps a double's bits; used as a test oracle only.
//!
//! Cost: linear in the bytes written, with one amortised `Vec` growth; [`Encoder::set`] and
//! [`Encoder::sorted`] sort their elements, O(n log n) comparisons of their encodings. Memory is
//! the output plus one small frame per open array or map. No recursion: open containers are an
//! explicit stack.

use alloc::vec::Vec;
use core::fmt;

/// The first byte of a head, by major type (RFC 8949 section 3.1), before its argument bits.
const UNSIGNED: u8 = 0x00;
/// Major type 1, a negative integer `−1 − argument`.
const NEGATIVE: u8 = 0x20;
/// Major type 2, a byte string.
const BYTES: u8 = 0x40;
/// Major type 3, a UTF-8 text string.
const TEXT: u8 = 0x60;
/// Major type 4, an array.
const ARRAY: u8 = 0x80;
/// Major type 5, a map.
const MAP: u8 = 0xa0;
/// Major type 6, a tag.
const TAG: u8 = 0xc0;
/// Major type 7 with additional information 25, 26 and 27: a binary16, binary32 or binary64
/// float follows (RFC 8949 section 3.3).
const FLOAT16: u8 = 0xf9;
/// A binary32 float follows.
const FLOAT32: u8 = 0xfa;
/// A binary64 float follows.
const FLOAT64: u8 = 0xfb;

/// Tag 2, an unsigned bignum: a byte string of the value, big-endian (RFC 8949 section 3.4.3).
const POSITIVE_BIGNUM: u64 = 2;
/// Tag 3, a negative bignum: a byte string of `−1 − value`, big-endian.
const NEGATIVE_BIGNUM: u64 = 3;
/// Tag 79, a typed array of little-endian `sint64` (RFC 8746 section 2.1).
pub const I64_COLUMN_TAG: u64 = 79;
/// Tag 86, a typed array of little-endian binary64 (RFC 8746 section 2.1).
pub const F64_COLUMN_TAG: u64 = 86;

/// Why a call has no encoding in SKG's subset. The call that returns one writes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// A double, or a value of an `f64` column, that is NaN or infinite: chapter 24.3 refuses them
    /// both ways.
    NotFinite,
    /// A map key not greater than the key before it in the same map. Keys are written in
    /// ascending order (RFC 8949 section 4.2.1), each once.
    KeyNotAscending,
    /// An item where the open map wants its next key.
    KeyExpected,
    /// A key where no map wants one: at the top level, inside an array, or where a map wants the
    /// value of the key just written.
    KeyNotExpected,
    /// An item after the encoder's one item is whole: an encoder writes exactly one data item.
    Complete,
    /// A finish before the item is whole: no item yet, or an array or map still short of the
    /// length it declared.
    Unfinished,
    /// A set with two equal elements; a set has no repeats (32.19).
    Repeated,
    /// A length that does not fit the 64-bit argument of a head.
    TooLong,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EncodeError::NotFinite => "a double that is NaN or infinite has no SKG encoding",
            EncodeError::KeyNotAscending => "a map key must be greater than the key before it",
            EncodeError::KeyExpected => "the open map wants its next key, not an item",
            EncodeError::KeyNotExpected => "no open map wants a key here",
            EncodeError::Complete => "the encoder already holds its one complete item",
            EncodeError::Unfinished => "the item is not whole yet",
            EncodeError::Repeated => "a set holds two equal elements",
            EncodeError::TooLong => "the length does not fit a CBOR head",
        })
    }
}

impl core::error::Error for EncodeError {}

/// The canonical encoding of one complete CBOR data item, made only by [`Encoder::finish`].
///
/// Items order by their bytes, lexicographically with a shorter prefix first: the order RFC 8949
/// section 4.2.1 sorts by and 32.19 sorts sets in.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item(Vec<u8>);

impl Item {
    /// The item's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The item's bytes, owned.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for Item {
    /// The bytes in hexadecimal, as RFC 8949's examples write them: `Item(820102)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Item(")?;
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(")")
    }
}

/// An array or map still waiting for items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    /// An array with `left` items still to come.
    Array { left: u64 },
    /// A map with `left` pairs still to come, the last key written, and whether the next thing
    /// written is the value of that key.
    Map {
        left: u64,
        last: Option<u64>,
        value_next: bool,
    },
}

/// Writes one CBOR data item in the deterministic encoding (see the module header).
///
/// Scalars ([`uint`](Encoder::uint), [`int`](Encoder::int), [`float`](Encoder::float),
/// [`bytes`](Encoder::bytes), [`text`](Encoder::text), the columns) are items; an
/// [`array`](Encoder::array) or [`map`](Encoder::map) opens a container of a declared length that
/// the next items fill, a map taking a [`key`](Encoder::key) before each of its values.
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    out: Vec<u8>,
    open: Vec<Frame>,
    whole: bool,
}

impl Encoder {
    /// An encoder holding nothing yet.
    #[must_use]
    pub fn new() -> Encoder {
        Encoder::default()
    }

    /// The finished item, once every array and map is full.
    ///
    /// # Errors
    ///
    /// [`EncodeError::Unfinished`] before the item is whole.
    pub fn finish(self) -> Result<Item, EncodeError> {
        if self.whole {
            Ok(Item(self.out))
        } else {
            Err(EncodeError::Unfinished)
        }
    }

    /// An unsigned integer, as major type 0.
    ///
    /// # Errors
    ///
    /// The placement errors of the module header: [`EncodeError::KeyExpected`] where the open map
    /// wants its next key, and [`EncodeError::Complete`] once the item is whole.
    pub fn uint(&mut self, value: u64) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, UNSIGNED, value);
        self.item_written();
        Ok(())
    }

    /// A signed integer: major type 0 or 1 when it fits, otherwise a bignum, tag 2 for a positive
    /// value and tag 3 for a negative one, whose byte string has no leading zero byte (24.3).
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn int(&mut self, value: i128) -> Result<(), EncodeError> {
        self.check_item()?;
        // A negative value is written as −1 − value, which is !value, at least zero.
        let (major, tag, magnitude) = if value < 0 {
            (NEGATIVE, NEGATIVE_BIGNUM, (!value).unsigned_abs())
        } else {
            (UNSIGNED, POSITIVE_BIGNUM, value.unsigned_abs())
        };
        if let Ok(small) = u64::try_from(magnitude) {
            head(&mut self.out, major, small);
        } else {
            // Above 2^64 − 1, so at least nine significant bytes and at most sixteen.
            let be = magnitude.to_be_bytes();
            let significant = be.iter().skip_while(|&&byte| byte == 0);
            head(&mut self.out, TAG, tag);
            head(&mut self.out, BYTES, count(significant.clone()));
            self.out.extend(significant);
        }
        self.item_written();
        Ok(())
    }

    /// A finite double, as the shortest of binary16, binary32 and binary64 that holds its bits.
    ///
    /// # Errors
    ///
    /// [`EncodeError::NotFinite`] for NaN or an infinity, and the placement errors of
    /// [`Encoder::uint`].
    pub fn float(&mut self, value: f64) -> Result<(), EncodeError> {
        if !value.is_finite() {
            return Err(EncodeError::NotFinite);
        }
        self.check_item()?;
        let bits = value.to_bits();
        if let Some(half) = narrow(bits, &BINARY16) {
            let [.., b6, b7] = half.to_be_bytes();
            self.out.extend_from_slice(&[FLOAT16, b6, b7]);
        } else if let Some(single) = narrow(bits, &BINARY32) {
            let [.., b4, b5, b6, b7] = single.to_be_bytes();
            self.out.extend_from_slice(&[FLOAT32, b4, b5, b6, b7]);
        } else {
            self.out.push(FLOAT64);
            self.out.extend_from_slice(&bits.to_be_bytes());
        }
        self.item_written();
        Ok(())
    }

    /// A byte string.
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, BYTES, length(value.len()));
        self.out.extend_from_slice(value);
        self.item_written();
        Ok(())
    }

    /// A UTF-8 text string, as given. Text hashed into a name must be in Normalization Form C,
    /// which the name's writer checks (32.19, piece G87).
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn text(&mut self, value: &str) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, TEXT, length(value.len()));
        self.out.extend_from_slice(value.as_bytes());
        self.item_written();
        Ok(())
    }

    /// A column of `i64`s: tag 79 over their little-endian bytes (24.3, RFC 8746).
    ///
    /// # Errors
    ///
    /// [`EncodeError::TooLong`] for a column of 2^61 values or more, and the placement errors of
    /// [`Encoder::uint`].
    pub fn i64_column(&mut self, values: &[i64]) -> Result<(), EncodeError> {
        let size = column_bytes(values.len())?;
        self.check_item()?;
        head(&mut self.out, TAG, I64_COLUMN_TAG);
        head(&mut self.out, BYTES, size);
        for value in values {
            self.out.extend_from_slice(&value.to_le_bytes());
        }
        self.item_written();
        Ok(())
    }

    /// A column of finite `f64`s: tag 86 over their little-endian bytes, each value's own bits
    /// (24.3, RFC 8746).
    ///
    /// # Errors
    ///
    /// [`EncodeError::NotFinite`] if any value is NaN or infinite, [`EncodeError::TooLong`] for a
    /// column of 2^61 values or more, and the placement errors of [`Encoder::uint`].
    pub fn f64_column(&mut self, values: &[f64]) -> Result<(), EncodeError> {
        if !values.iter().all(|value| value.is_finite()) {
            return Err(EncodeError::NotFinite);
        }
        let size = column_bytes(values.len())?;
        self.check_item()?;
        head(&mut self.out, TAG, F64_COLUMN_TAG);
        head(&mut self.out, BYTES, size);
        for value in values {
            self.out.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        self.item_written();
        Ok(())
    }

    /// Opens an array of `len` items, which the next `len` items fill.
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn array(&mut self, len: u64) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, ARRAY, len);
        self.open.push(Frame::Array { left: len });
        self.close_full();
        Ok(())
    }

    /// Opens a map of `len` pairs, each a [`key`](Encoder::key) and then one item, keys ascending.
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn map(&mut self, len: u64) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, MAP, len);
        self.open.push(Frame::Map {
            left: len,
            last: None,
            value_next: false,
        });
        self.close_full();
        Ok(())
    }

    /// The next key of the open map, greater than the key before it.
    ///
    /// # Errors
    ///
    /// [`EncodeError::KeyNotExpected`] where no map wants a key, and
    /// [`EncodeError::KeyNotAscending`] for a key not greater than the one before it.
    pub fn key(&mut self, key: u64) -> Result<(), EncodeError> {
        let Some(Frame::Map {
            last, value_next, ..
        }) = self.open.last_mut()
        else {
            return Err(EncodeError::KeyNotExpected);
        };
        if *value_next {
            return Err(EncodeError::KeyNotExpected);
        }
        if last.is_some_and(|before| key <= before) {
            return Err(EncodeError::KeyNotAscending);
        }
        *last = Some(key);
        *value_next = true;
        head(&mut self.out, UNSIGNED, key);
        Ok(())
    }

    /// One finished item, written as it is: it is canonical, since only an encoder makes one.
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn item(&mut self, item: &Item) -> Result<(), EncodeError> {
        self.check_item()?;
        self.out.extend_from_slice(item.as_bytes());
        self.item_written();
        Ok(())
    }

    /// A set: an array of the items sorted by their bytes, with no two equal (32.19).
    ///
    /// # Errors
    ///
    /// [`EncodeError::Repeated`] if two items are equal, and the placement errors of
    /// [`Encoder::uint`].
    pub fn set(&mut self, mut items: Vec<Item>) -> Result<(), EncodeError> {
        items.sort_unstable();
        if items
            .windows(2)
            .any(|pair| matches!(pair, [a, b] if a == b))
        {
            return Err(EncodeError::Repeated);
        }
        self.sorted_run(&items)
    }

    /// A sorted list: an array of the items sorted by their bytes, repeats kept (32.19).
    ///
    /// # Errors
    ///
    /// The placement errors of [`Encoder::uint`].
    pub fn sorted(&mut self, mut items: Vec<Item>) -> Result<(), EncodeError> {
        items.sort_unstable();
        self.sorted_run(&items)
    }

    /// Writes already sorted items as one array item.
    fn sorted_run(&mut self, items: &[Item]) -> Result<(), EncodeError> {
        self.check_item()?;
        head(&mut self.out, ARRAY, length(items.len()));
        for item in items {
            self.out.extend_from_slice(item.as_bytes());
        }
        self.item_written();
        Ok(())
    }

    /// Refuses an item where none may go, without changing anything. A full container is never
    /// open: [`Encoder::close_full`] closes it as its last item is written.
    fn check_item(&self) -> Result<(), EncodeError> {
        if self.whole {
            return Err(EncodeError::Complete);
        }
        match self.open.last() {
            Some(Frame::Map {
                value_next: false, ..
            }) => Err(EncodeError::KeyExpected),
            _ => Ok(()),
        }
    }

    /// Counts the item just written against the open container, after [`Encoder::check_item`]
    /// allowed it, and closes every container it fills.
    fn item_written(&mut self) {
        match self.open.last_mut() {
            Some(Frame::Array { left }) => *left = left.saturating_sub(1),
            Some(Frame::Map {
                left, value_next, ..
            }) => {
                *left = left.saturating_sub(1);
                *value_next = false;
            }
            None => {}
        }
        self.close_full();
    }

    /// Closes every full container from the innermost out, each one an item of its parent; the
    /// item is whole when the last one closes, or at once for a top-level scalar.
    fn close_full(&mut self) {
        loop {
            match self.open.last() {
                Some(Frame::Array { left: 0 } | Frame::Map { left: 0, .. }) => {
                    self.open.pop();
                    match self.open.last_mut() {
                        Some(Frame::Array { left }) => *left = left.saturating_sub(1),
                        Some(Frame::Map {
                            left, value_next, ..
                        }) => {
                            *left = left.saturating_sub(1);
                            *value_next = false;
                        }
                        None => {
                            self.whole = true;
                            return;
                        }
                    }
                }
                Some(_) => return,
                None => {
                    self.whole = true;
                    return;
                }
            }
        }
    }
}

/// Appends a head: the major type's first three bits and the argument in the fewest bytes (RFC
/// 8949 sections 3 and 4.2.1): in the first byte below 24, else in the next 1, 2, 4 or 8 bytes,
/// after the additional information 24 (`0x18`), 25, 26 or 27 (`0x1b`).
fn head(out: &mut Vec<u8>, major: u8, argument: u64) {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = argument.to_be_bytes();
    if argument < 24 {
        out.push(major | b7);
    } else if argument <= 0xff {
        out.extend_from_slice(&[major | 0x18, b7]);
    } else if argument <= 0xffff {
        out.extend_from_slice(&[major | 0x19, b6, b7]);
    } else if argument <= 0xffff_ffff {
        out.extend_from_slice(&[major | 0x1a, b4, b5, b6, b7]);
    } else {
        out.extend_from_slice(&[major | 0x1b, b0, b1, b2, b3, b4, b5, b6, b7]);
    }
}

/// A length as a head's argument. `usize` is at most 64 bits wide on every target Rust supports,
/// so the conversion never loses a value.
fn length(len: usize) -> u64 {
    len as u64
}

/// The number of items an iterator yields, as a head's argument.
fn count<I: Iterator>(items: I) -> u64 {
    length(items.count())
}

/// The byte length of a column of `len` eight-byte values.
fn column_bytes(len: usize) -> Result<u64, EncodeError> {
    length(len).checked_mul(8).ok_or(EncodeError::TooLong)
}

/// A narrower IEEE 754 binary format, described by where a binary64's biased exponent `e` falls
/// in it. With `M` the narrow format's fraction bits and `emin` its least normal exponent, a
/// binary64 `x = q · 2^(e − 1075)`, `q = 2^52 + fraction`, is:
///
/// - a normal number of the narrow format when `normal.0 ≤ e ≤ normal.1`, exactly when its low
///   `dropped = 52 − M` fraction bits are zero; its narrow exponent field is `e − rebias`;
/// - a subnormal one when `subnormal_from ≤ e < normal.0`, exactly when the low
///   `shift = subnormal_shift − e` bits of `q` are zero; its narrow fraction is `q >> shift`.
///
/// Below `subnormal_from`, or above `normal.1`, it is no value of the narrow format.
struct Narrow {
    /// The narrow format's fraction bits, `M`.
    fraction: u32,
    /// The binary64 fraction bits a narrow normal number cannot hold, `52 − M`.
    dropped: u32,
    /// The bit of the narrow format's sign, `E + M` for `E` exponent bits.
    sign: u32,
    /// The binary64 exponents of the narrow normal numbers: `1023 + emin` to `1023 + emax`.
    normal: (u64, u64),
    /// `normal.0 − 1`, so that the narrow exponent field `e − rebias` is 1 for the least normal.
    rebias: u64,
    /// The binary64 exponent of the narrow format's least subnormal, `1023 + emin − M`.
    subnormal_from: u64,
    /// `52 − M + 1023 + emin`, so that a narrow subnormal's shift is `subnormal_shift − e`, from
    /// `53 − M` at the largest subnormal binade to 52 at the least subnormal.
    subnormal_shift: u64,
}

/// binary16: 5 exponent bits, 10 fraction bits, `emin = −14`, `emax = 15`.
const BINARY16: Narrow = Narrow {
    fraction: 10,
    dropped: 42,
    sign: 15,
    normal: (1009, 1038),
    rebias: 1008,
    subnormal_from: 999,
    subnormal_shift: 1051,
};

/// binary32: 8 exponent bits, 23 fraction bits, `emin = −126`, `emax = 127`.
const BINARY32: Narrow = Narrow {
    fraction: 23,
    dropped: 29,
    sign: 31,
    normal: (897, 1150),
    rebias: 896,
    subnormal_from: 874,
    subnormal_shift: 926,
};

/// The bits of the narrow format's number equal to the finite binary64 with bits `bits`, or `None`
/// when that format has no number of exactly this value. Zeros keep their sign.
fn narrow(bits: u64, format: &Narrow) -> Option<u64> {
    let negative = bits >> 63 == 1;
    let exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1 << 52) - 1);
    let sign = if negative { 1_u64 << format.sign } else { 0 };
    if exponent == 0 && fraction == 0 {
        return Some(sign);
    }
    let magnitude = if (format.normal.0..=format.normal.1).contains(&exponent) {
        // A zero fraction has 64 trailing zeros, more than any format drops.
        if fraction.trailing_zeros() < format.dropped {
            return None;
        }
        (exponent.checked_sub(format.rebias)? << format.fraction) | (fraction >> format.dropped)
    } else if (format.subnormal_from..format.normal.0).contains(&exponent) {
        let q = (1 << 52) | fraction;
        let shift = format.subnormal_shift.checked_sub(exponent)?;
        if u64::from(q.trailing_zeros()) < shift {
            return None;
        }
        q >> shift
    } else {
        return None;
    };
    Some(sign | magnitude)
}

#[cfg(kani)]
mod proofs {
    use super::{BINARY16, BINARY32, Encoder, narrow};

    /// The binary16 value of `h`, decoded as RFC 8949 Appendix D decodes a half: the fraction
    /// times 2^−24 for a subnormal, else (fraction + 1024) times 2^(exponent − 25), each product
    /// exact in binary64.
    fn half_value(h: u64) -> f64 {
        let negative = h >> 15 == 1;
        let exponent = (h >> 10) & 0x1f;
        let fraction = h & 0x3ff;
        // 2^k as a binary64, for the k from −24 to 5 a half needs: biased exponent k + 1023.
        let pow2 = |k: u64| f64::from_bits((k + 1023 - 25) << 52);
        let magnitude = if exponent == 0 {
            f64::from(u32::try_from(fraction).unwrap_or(0)) * pow2(1)
        } else {
            f64::from(u32::try_from(fraction + 1024).unwrap_or(0)) * pow2(exponent)
        };
        if negative { -magnitude } else { magnitude }
    }

    /// A binary16 narrowing is the double's own value, and a binary16 exists for every double
    /// the narrowing refuses only when none has its value: checked against every finite double.
    #[kani::proof]
    fn binary16_is_exact_and_shortest() {
        let bits: u64 = kani::any();
        let x = f64::from_bits(bits);
        kani::assume(x.is_finite());
        match narrow(bits, &BINARY16) {
            Some(h) => {
                assert!(h <= 0xffff);
                assert!(half_value(h).to_bits() == bits);
            }
            None => {
                let h: u16 = kani::any();
                kani::assume((h >> 10) & 0x1f != 0x1f);
                assert!(half_value(u64::from(h)).to_bits() != bits);
            }
        }
    }

    /// A binary32 narrowing is the double's own value, and is refused exactly when the double is
    /// no binary32: checked against the hardware conversion, which is exact for a binary32 value.
    #[kani::proof]
    fn binary32_is_exact_and_shortest() {
        let bits: u64 = kani::any();
        let x = f64::from_bits(bits);
        kani::assume(x.is_finite());
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the rounding is the reference"
        )]
        let rounded = x as f32;
        let fits = f64::from(rounded).to_bits() == bits;
        match narrow(bits, &BINARY32) {
            Some(s) => {
                assert!(fits);
                assert!(u64::from(rounded.to_bits()) == s);
            }
            None => assert!(!fits),
        }
    }

    /// A refused float writes nothing and leaves the encoder able to finish.
    #[kani::proof]
    fn a_refused_float_writes_nothing() {
        let bits: u64 = kani::any();
        let x = f64::from_bits(bits);
        kani::assume(!x.is_finite());
        let mut encoder = Encoder::new();
        assert!(encoder.float(x).is_err());
        assert!(encoder.uint(0).is_ok());
        assert!(encoder.finish().is_ok_and(|item| item.as_bytes() == [0]));
    }
}
