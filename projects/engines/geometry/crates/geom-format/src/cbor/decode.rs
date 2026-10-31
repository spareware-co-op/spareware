//! The strict decoder: reads one item of SKG's deterministic CBOR, head by head, and refuses any
//! byte that is not in the one canonical encoding of its value.
//!
//! What it does: a [`Decoder`] walks one data item and hands out its heads as [`Item`]s, in the
//! order the bytes hold them, borrowing strings from the input. Before handing out a head it checks
//! every rule of 24.3's canonical form that the head, and the bytes so far, can break. [`check`]
//! walks one whole item and refuses trailing bytes; [`item_len`] walks the first item of a CBOR
//! Sequence and says where it ends.
//!
//! The rules it enforces (RFC 8949 section 4.2.1, with SKG's own of 24.3):
//!
//! - every integer, length, count and tag number in the shortest head that holds it;
//! - definite lengths only;
//! - every float in the shortest of half, single and double precision that holds it, and finite;
//! - map keys in strictly increasing bytewise order of their encodings, so no key repeats;
//! - text in UTF-8;
//! - only the simple values `false`, `true` and `null`;
//! - only the tags 2 and 3 (bignums, whose bytes have no leading zero and whose value does not fit
//!   in 64 bits, RFC 8949 section 3.4.3), 79 (a column of little-endian `i64`) and 86 (a column of
//!   little-endian `f64`, every one finite), each tagging a byte string (RFC 8746).
//!
//! Guarantees:
//!
//! - **Canonical or refused.** The decoder accepts bytes only if they are the one encoding SKG's
//!   canonical form gives their value; any other input is refused with the offset of the item at
//!   fault and the rule it breaks. So two accepted inputs that decode to the same items are the same
//!   bytes.
//! - **Never panics, never reads past the input.** Every length and count is checked against the
//!   bytes left before it is used: a string must fit in them, an array of n items needs n bytes and a
//!   map of n pairs 2n.
//! - **Bounded nesting without recursion.** Containers and tags are walked with an explicit stack
//!   no deeper than the caller's limit; deeper input is refused, never recursed into (24.7).
//! - **Metered.** Every head charges the caller's work meter one unit, and every
//!   [`SCAN_BYTES_PER_CHARGE`] bytes of text or column it scans charge one more, so a budget or a
//!   cancellation stops a read within a bounded amount of work, and the same input stops at the same
//!   charge on every machine.
//! - **Independent of how the input arrives.** The decoder reads a byte slice and nothing else; the
//!   items, the errors and the offsets depend only on the bytes.
//!
//! Method: RFC 8949 (Concise Binary Object Representation), sections 3 (encoding), 3.4.3
//! (bignums), 4.1 (preferred serialization) and 4.2.1 (core deterministic encoding requirements),
//! <https://www.rfc-editor.org/rfc/rfc8949>; RFC 8746 (typed arrays as byte strings), section 2,
//! <https://www.rfc-editor.org/rfc/rfc8746#section-2>; and SKG's canonical bytes,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` (24.3, 24.7). A head's
//! argument is in its shortest form when additional information 24, 25, 26 or 27 carries a value
//! that would not fit in the next shorter one (RFC 8949, section 4.2.1, first rule). Map keys are
//! compared as the byte slices of their encodings, which is the bytewise lexicographic order of
//! section 4.2.1; RFC 7049's length-first order is not used.
//!
//! Why our own code: chapter 29.1's dependency rule, clause 2. A deterministic decoder must refuse
//! non-canonical input, never recurse without a bound, and be `no_std` with no `unsafe`; no crate
//! on the kernel's vetted allow-list decodes CBOR (only `libm` and `sha2` are on it).
//!
//! Cost: O(n) time in the input's length, one pass; no allocation but the explicit stack, which
//! holds one small frame per open container or tag, at most the caller's depth limit. Recursion:
//! none.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use super::error::{CborError, CborErrorKind};
use super::float::{Width, half_to_f64, shortest, single_to_f64};
use super::item::Item;

/// Bytes of text or column one work unit pays for, beyond each head's own unit. Chapter 23.10 asks
/// that a cancelled read stop within about 1 ms; the meter reads the cancellation every
/// [`geom_run::POLL_EVERY`] (64) charges, so at the 500 MB/s per core the board asks of column
/// decoding a charge must cover at most 500 000 / 64 ≈ 7 800 bytes. 4 KiB, 512 doubles, covers about
/// half of that: a poll every 256 KiB, about 0.5 ms.
pub const SCAN_BYTES_PER_CHARGE: usize = 4096;

/// The bytes of one value in a column (tags 79 and 86).
const COLUMN_VALUE_BYTES: usize = 8;

/// What a tag requires of the byte string it tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TagRule {
    /// Tags 2 and 3: a bignum's magnitude, at least 2^64, with no leading zero byte.
    Bignum,
    /// Tag 79: little-endian `i64` values.
    IntColumn,
    /// Tag 86: little-endian `f64` values, every one finite.
    FloatColumn,
}

/// One open container or tag on the explicit stack.
#[derive(Clone, Copy, Debug)]
enum Frame {
    /// An array with this many items still to come.
    Array { left: u64 },
    /// A map with this many pairs still to come.
    Map {
        /// Pairs still to come, the one in progress included.
        left: u64,
        /// Whether the next item to complete is a key.
        key_next: bool,
        /// Where the key in progress, or the next one, starts.
        key_start: usize,
        /// The encoding of the previous key, as a range of the input.
        previous: Option<(usize, usize)>,
    },
    /// A tag whose one item has not completed; `at` is the tag's head.
    Tag { rule: TagRule, at: usize },
}

/// A strict decoder of one item of SKG's deterministic CBOR.
#[derive(Debug)]
pub struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
    stack: Vec<Frame>,
    depth: usize,
    done: bool,
}

/// One head's major type, additional information and argument, and the offset after it.
struct Head {
    major: u8,
    info: u8,
    argument: u64,
    end: usize,
}

impl<'a> Decoder<'a> {
    /// A decoder of the item that starts at the first byte of `bytes`, refusing containers and tags
    /// nested more than `depth` deep. The caller's format fixes the depth: a record's schema says
    /// how deep its payload goes (24.2).
    #[must_use]
    pub fn new(bytes: &'a [u8], depth: usize) -> Self {
        Self {
            bytes,
            at: 0,
            stack: Vec::new(),
            depth,
            done: false,
        }
    }

    /// The offset of the next head, or of the end of the item once it is complete.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.at
    }

    /// How many containers and tags are open.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// The next head of the item, or `None` once the item is complete.
    ///
    /// # Errors
    ///
    /// [`CborError`] when the input breaks a rule of the canonical form, ends inside the item,
    /// nests too deep, or the meter stops. After an error the decoder hands out nothing more.
    pub fn next(&mut self, meter: &mut WorkMeter<'_>) -> Result<Option<Item<'a>>, CborError> {
        if self.done {
            return Ok(None);
        }
        let start = self.at;
        let result = self.step(start, meter);
        if result.is_err() {
            self.done = true;
        }
        result.map(Some)
    }

    /// Reads and checks the head at `start`, and what follows it for a string.
    fn step(&mut self, start: usize, meter: &mut WorkMeter<'_>) -> Result<Item<'a>, CborError> {
        let fail = |kind| CborError {
            offset: start,
            kind,
        };
        meter
            .charge(1)
            .map_err(|s| fail(CborErrorKind::Stopped(s)))?;
        let head = self.head(start)?;
        let tagged = match self.stack.last() {
            Some(&Frame::Tag { rule, at }) => Some((rule, at)),
            _ => None,
        };
        if tagged.is_some() && head.major != 2 {
            return Err(fail(CborErrorKind::TagContent));
        }
        match head.major {
            0 => {
                self.complete(head.end)?;
                Ok(Item::Uint(head.argument))
            }
            1 => {
                self.complete(head.end)?;
                Ok(Item::Nint(head.argument))
            }
            2 => {
                let (content, end) = self.string(&head)?;
                if let Some((rule, at)) = tagged {
                    check_tagged(rule, at, head.end, content, meter)?;
                }
                self.complete(end)?;
                Ok(Item::Bytes(content))
            }
            3 => {
                let (content, end) = self.string(&head)?;
                charge_scan(content.len(), start, meter)?;
                let text = utf8(content).ok_or_else(|| fail(CborErrorKind::InvalidUtf8))?;
                self.complete(end)?;
                Ok(Item::Text(text))
            }
            4 => {
                if head.argument == 0 {
                    self.complete(head.end)?;
                } else {
                    let frame = Frame::Array {
                        left: head.argument,
                    };
                    self.open(head.argument, 1, (start, head.end), frame, meter)?;
                }
                Ok(Item::Array(head.argument))
            }
            5 => {
                if head.argument == 0 {
                    self.complete(head.end)?;
                } else {
                    let frame = Frame::Map {
                        left: head.argument,
                        key_next: true,
                        key_start: head.end,
                        previous: None,
                    };
                    self.open(head.argument, 2, (start, head.end), frame, meter)?;
                }
                Ok(Item::Map(head.argument))
            }
            6 => {
                let rule = match head.argument {
                    2 | 3 => TagRule::Bignum,
                    79 => TagRule::IntColumn,
                    86 => TagRule::FloatColumn,
                    _ => return Err(fail(CborErrorKind::UnsupportedTag)),
                };
                // The tagged byte string needs at least its one head byte.
                let frame = Frame::Tag { rule, at: start };
                self.open(1, 1, (start, head.end), frame, meter)?;
                Ok(Item::Tag(head.argument))
            }
            _ => {
                let item = simple(&head).map_err(fail)?;
                self.complete(head.end)?;
                Ok(item)
            }
        }
    }

    /// The head at `start`, with its argument checked for the shortest form (major types 0 to 6) or
    /// read as a float's bits (major type 7).
    fn head(&self, start: usize) -> Result<Head, CborError> {
        let fail = |kind| CborError {
            offset: start,
            kind,
        };
        let initial = *self.bytes.get(start).ok_or(self.truncated())?;
        let major = initial >> 5;
        let info = initial & 0x1f;
        let (length, smallest) = match info {
            0..=23 => {
                return Ok(Head {
                    major,
                    info,
                    argument: u64::from(info),
                    end: start.checked_add(1).ok_or(self.truncated())?,
                });
            }
            24 => (1, 24),
            25 => (2, 0x100),
            26 => (4, 0x1_0000),
            27 => (8, 0x1_0000_0000),
            28..=30 => return Err(fail(CborErrorKind::ReservedInfo)),
            _ => return Err(fail(CborErrorKind::Indefinite)),
        };
        let first = start.checked_add(1).ok_or(self.truncated())?;
        let end = first.checked_add(length).ok_or(self.truncated())?;
        let argument_bytes = self.bytes.get(first..end).ok_or(self.truncated())?;
        let argument = argument_bytes
            .iter()
            .fold(0_u64, |value, &byte| (value << 8) | u64::from(byte));
        // Major type 7 uses 24 for a one-byte simple value and 25 to 27 for floats, whose bits are
        // never shortened as integers.
        if major != 7 && argument < smallest {
            return Err(fail(CborErrorKind::NonShortestHead));
        }
        Ok(Head {
            major,
            info,
            argument,
            end,
        })
    }

    /// The content of the string whose head is `head`, and the offset after it.
    fn string(&self, head: &Head) -> Result<(&'a [u8], usize), CborError> {
        let length = usize::try_from(head.argument).map_err(|_| self.truncated())?;
        let end = head.end.checked_add(length).ok_or(self.truncated())?;
        let content = self.bytes.get(head.end..end).ok_or(self.truncated())?;
        Ok((content, end))
    }

    /// Opens a container or tag of `count` items, each at least `per_item` bytes, whose head runs
    /// from `head.0` to `head.1`.
    fn open(
        &mut self,
        count: u64,
        per_item: u64,
        head: (usize, usize),
        frame: Frame,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), CborError> {
        let (start, end) = head;
        let left = self.bytes.len().saturating_sub(end);
        let needed = count.checked_mul(per_item).ok_or(self.truncated())?;
        if u64::try_from(left).is_ok_and(|left| needed > left) {
            return Err(self.truncated());
        }
        if self.stack.len() >= self.depth {
            return Err(CborError {
                offset: start,
                kind: CborErrorKind::TooDeep,
            });
        }
        // The stack never holds more than `depth` frames: the check above refuses one more.
        self.stack.try_reserve(1).map_err(|_| CborError {
            offset: start,
            kind: CborErrorKind::Stopped(meter.out_of_memory()),
        })?;
        self.stack.push(frame);
        self.at = end;
        Ok(())
    }

    /// Records that an item ended at `end`, closing every container and tag it completes, and
    /// checks map keys' order as each key completes.
    fn complete(&mut self, end: usize) -> Result<(), CborError> {
        self.at = end;
        loop {
            let Some(frame) = self.stack.last_mut() else {
                self.done = true;
                return Ok(());
            };
            match frame {
                Frame::Tag { .. } => {}
                Frame::Array { left } => {
                    *left = left.saturating_sub(1);
                    if *left != 0 {
                        return Ok(());
                    }
                }
                Frame::Map {
                    left,
                    key_next,
                    key_start,
                    previous,
                } => {
                    if *key_next {
                        let key = (*key_start, end);
                        if let Some(before) = *previous {
                            let kind = match key_order(self.bytes, before, key) {
                                core::cmp::Ordering::Less => None,
                                core::cmp::Ordering::Equal => Some(CborErrorKind::DuplicateKey),
                                core::cmp::Ordering::Greater => Some(CborErrorKind::UnsortedKeys),
                            };
                            if let Some(kind) = kind {
                                return Err(CborError {
                                    offset: key.0,
                                    kind,
                                });
                            }
                        }
                        *previous = Some(key);
                        *key_next = false;
                        return Ok(());
                    }
                    *left = left.saturating_sub(1);
                    *key_next = true;
                    *key_start = end;
                    if *left != 0 {
                        return Ok(());
                    }
                }
            }
            self.stack.pop();
        }
    }

    /// The error for input that ends inside the item.
    fn truncated(&self) -> CborError {
        CborError {
            offset: self.bytes.len(),
            kind: CborErrorKind::Truncated,
        }
    }

    /// The offset just past the item, once it is complete.
    ///
    /// # Errors
    ///
    /// [`CborErrorKind::Truncated`] if the item is not complete.
    pub fn end(&self) -> Result<usize, CborError> {
        if self.done && self.stack.is_empty() {
            Ok(self.at)
        } else {
            Err(self.truncated())
        }
    }
}

/// The item a major type 7 head stands for: `false`, `true`, `null` or a finite float in its
/// shortest width.
fn simple(head: &Head) -> Result<Item<'static>, CborErrorKind> {
    match head.info {
        20 => Ok(Item::Bool(false)),
        21 => Ok(Item::Bool(true)),
        22 => Ok(Item::Null),
        25 => {
            let bits = u16::try_from(head.argument).map_err(|_| CborErrorKind::Truncated)?;
            half_to_f64(bits)
                .map(Item::Float)
                .ok_or(CborErrorKind::NotFinite)
        }
        26 => {
            let bits = u32::try_from(head.argument).map_err(|_| CborErrorKind::Truncated)?;
            let value = single_to_f64(bits).ok_or(CborErrorKind::NotFinite)?;
            if shortest(value) == Some(Width::Single(bits)) {
                Ok(Item::Float(value))
            } else {
                Err(CborErrorKind::NonShortestFloat)
            }
        }
        27 => {
            let value = f64::from_bits(head.argument);
            if !value.is_finite() {
                Err(CborErrorKind::NotFinite)
            } else if shortest(value) == Some(Width::Double(head.argument)) {
                Ok(Item::Float(value))
            } else {
                Err(CborErrorKind::NonShortestFloat)
            }
        }
        _ => Err(CborErrorKind::UnsupportedSimple),
    }
}

/// Checks the byte string `content`, starting at `start`, against what the tag at `at` requires.
fn check_tagged(
    rule: TagRule,
    at: usize,
    start: usize,
    content: &[u8],
    meter: &mut WorkMeter<'_>,
) -> Result<(), CborError> {
    let fail = |offset, kind| CborError { offset, kind };
    match rule {
        TagRule::Bignum => {
            // At least 2^64 and no leading zero: nine bytes or more, the first not zero.
            if content.len() < 9 || content.first() == Some(&0) {
                return Err(fail(at, CborErrorKind::NonCanonicalBignum));
            }
        }
        TagRule::IntColumn => {
            if !content.len().is_multiple_of(COLUMN_VALUE_BYTES) {
                return Err(fail(at, CborErrorKind::ColumnLength));
            }
        }
        TagRule::FloatColumn => {
            let (values, rest) = content.as_chunks::<COLUMN_VALUE_BYTES>();
            if !rest.is_empty() {
                return Err(fail(at, CborErrorKind::ColumnLength));
            }
            for (chunk, values) in values
                .chunks(SCAN_BYTES_PER_CHARGE / COLUMN_VALUE_BYTES)
                .enumerate()
            {
                meter
                    .charge(1)
                    .map_err(|s| fail(at, CborErrorKind::Stopped(s)))?;
                if let Some(i) = values
                    .iter()
                    .position(|v| !f64::from_le_bytes(*v).is_finite())
                {
                    let row = chunk
                        .checked_mul(SCAN_BYTES_PER_CHARGE / COLUMN_VALUE_BYTES)
                        .and_then(|r| r.checked_add(i))
                        .and_then(|r| r.checked_mul(COLUMN_VALUE_BYTES))
                        .and_then(|r| r.checked_add(start))
                        .unwrap_or(at);
                    return Err(fail(row, CborErrorKind::NotFinite));
                }
            }
        }
    }
    Ok(())
}

/// `content` as text, if it is UTF-8.
fn utf8(content: &[u8]) -> Option<&str> {
    core::str::from_utf8(content).ok()
}

/// How the bytes of the key at `before` compare with those of the key at `key`.
fn key_order(bytes: &[u8], before: (usize, usize), key: (usize, usize)) -> core::cmp::Ordering {
    bytes.get(before.0..before.1).cmp(&bytes.get(key.0..key.1))
}

/// Charges the meter for scanning `length` bytes: one unit per [`SCAN_BYTES_PER_CHARGE`] or part.
fn charge_scan(length: usize, at: usize, meter: &mut WorkMeter<'_>) -> Result<(), CborError> {
    let units = length.div_ceil(SCAN_BYTES_PER_CHARGE);
    meter
        .charge(u64::try_from(units).unwrap_or(u64::MAX))
        .map_err(|s| CborError {
            offset: at,
            kind: CborErrorKind::Stopped(s),
        })
}

/// Checks that `bytes` hold exactly one item of SKG's deterministic CBOR, nested at most `depth`
/// deep.
///
/// # Errors
///
/// [`CborError`] for the first rule the bytes break, [`CborErrorKind::TrailingBytes`] if bytes
/// follow the item, or the meter's stop.
pub fn check(bytes: &[u8], depth: usize, meter: &mut WorkMeter<'_>) -> Result<(), CborError> {
    let mut decoder = Decoder::new(bytes, depth);
    while decoder.next(meter)?.is_some() {}
    check_end(&decoder, bytes)
}

/// Checks that the decoder's item is complete and is the whole of `bytes`.
pub(super) fn check_end(decoder: &Decoder<'_>, bytes: &[u8]) -> Result<(), CborError> {
    let end = decoder.end()?;
    if end == bytes.len() {
        Ok(())
    } else {
        Err(CborError {
            offset: end,
            kind: CborErrorKind::TrailingBytes,
        })
    }
}

/// The length of the item of SKG's deterministic CBOR that starts at the first byte of `bytes`,
/// the first item of a CBOR Sequence (RFC 8742), checked as [`Decoder`] checks it.
///
/// # Errors
///
/// [`CborError`] for the first rule the item breaks, or the meter's stop.
pub fn item_len(bytes: &[u8], depth: usize, meter: &mut WorkMeter<'_>) -> Result<usize, CborError> {
    let mut decoder = Decoder::new(bytes, depth);
    while decoder.next(meter)?.is_some() {}
    decoder.end()
}

/// Bounded proofs (chapter 28.7; piece G112), proven by `kernel-fuzz.yml`'s `kani-format` job.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
pub(crate) mod proofs {
    use core::cmp::Ordering;

    use geom_run::{Budget, WorkMeter};

    use super::{CborError, CborErrorKind, Decoder, TagRule};

    /// Stands in for `check_tagged`: the tagged columns' own checks, which are tested
    /// (`tests/attack_g91.rs`), either way.
    pub(crate) fn any_tagged(
        _: TagRule,
        _: usize,
        _: usize,
        _: &[u8],
        _: &mut WorkMeter<'_>,
    ) -> Result<(), CborError> {
        if kani::any() {
            Ok(())
        } else {
            Err(CborError {
                offset: 0,
                kind: CborErrorKind::TagContent,
            })
        }
    }

    /// Stands in for `key_order`: the standard library's comparison, any way.
    pub(crate) fn any_order(_: &[u8], _: (usize, usize), _: (usize, usize)) -> Ordering {
        match kani::any::<u8>() {
            0 => Ordering::Less,
            1 => Ordering::Equal,
            _ => Ordering::Greater,
        }
    }

    /// Stands in for `utf8`: the standard library's validation, either way.
    pub(crate) fn any_text(_: &[u8]) -> Option<&str> {
        if kani::any() { Some("") } else { None }
    }

    /// One head of any bytes up to the nine a head can take, at any nesting allowance up to 2:
    /// an item that ends inside them, or a refusal, never a panic.
    #[kani::proof]
    #[kani::unwind(10)]
    #[kani::stub(super::check_tagged, any_tagged)]
    #[kani::stub(super::utf8, any_text)]
    #[kani::stub(super::key_order, any_order)]
    fn one_head_never_panics() {
        let bytes: [u8; 9] = kani::any();
        let depth: usize = kani::any();
        kani::assume(depth <= 2);
        let mut decoder = Decoder::new(&bytes, depth);
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let _ = decoder.next(&mut meter);
        assert!(decoder.offset() <= bytes.len());
    }

    /// Two heads of any four bytes, the second under whatever the first opened (an array, a map
    /// or a tag), never a panic.
    #[kani::proof]
    #[kani::unwind(6)]
    #[kani::stub(super::check_tagged, any_tagged)]
    #[kani::stub(super::utf8, any_text)]
    #[kani::stub(super::key_order, any_order)]
    fn two_heads_never_panic() {
        let bytes: [u8; 4] = kani::any();
        let mut decoder = Decoder::new(&bytes, 2);
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let _ = decoder.next(&mut meter);
        let _ = decoder.next(&mut meter);
        assert!(decoder.offset() <= bytes.len());
    }
}
