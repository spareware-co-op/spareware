//! The DEFLATE decoder under [`crate::inflate`]'s reader: a resumable state machine that turns raw
//! DEFLATE bytes (RFC 1951) into the bytes they encode, a piece of input and a stretch of output at
//! a time.
//!
//! What it does: [`Decoder::step`] reads from a slice of compressed bytes and writes into the
//! output buffer after a given position, until the input runs out, the output is full, the final
//! block ends or the bytes are found not to be DEFLATE. The buffer holds all the output before that
//! position too, since a match copies from it; called again with more input or more room, the
//! decoder carries on exactly where it stopped. Stored, fixed-code and dynamic-code blocks are
//! read; zlib and gzip wrappers are not (a zip entry holds raw DEFLATE).
//!
//! Guarantees:
//!
//! - **No panics, no indexing, no unchecked arithmetic.** Every table and buffer is read with
//!   `get`, every count is a checked or saturating step whose bound is stated beside it, and a
//!   cast is a `try_from`, an `unsigned_abs` or a byte of `to_le_bytes`.
//! - **Malformed or refused, never misread.** A block type of 3, a stored length whose complement
//!   disagrees, more than 286 literal and length codes or 30 distance codes, an over-subscribed or
//!   incomplete code, a repeat with no length before it, repeats that run past the code lengths,
//!   the literal and length symbols 286 and 287, the distance symbols 30 and 31, and a distance
//!   reaching before the first byte of output each end the stream with their own
//!   [`InflateErrorKind`]. The decoder stays in that failure.
//! - **The split never matters.** The output, and where in the input a failure is found, are the
//!   same however the input and the room are handed out: the decoder keeps every bit it has read
//!   and every match it has half written, and its fast loop reads exactly the bits the careful path
//!   reads.
//! - **Bounded work per byte, no recursion.** Each state reads at most two bytes or writes at most
//!   one match before it moves on; walking a code's tree visits at most one node per bit.
//!
//! Method: RFC 1951's block format, Huffman codes and LZ77 matches; the decoder of `miniz_oxide`
//! (below), itself a port of Rich Geldreich's miniz `tinfl`, which reads codes of up to 10 bits
//! from one table and longer ones by walking a tree, and keeps a fast loop for when at least 14
//! input bytes and 259 bytes of room remain. The checks on code lengths follow zlib's `inftrees.c`
//! as `miniz_oxide` does.
//!
//! - P. Deutsch, "DEFLATE Compressed Data Format Specification version 1.3", RFC 1951 (1996),
//!   <https://www.rfc-editor.org/rfc/rfc1951>.
//!
//! Source: PORT from `miniz_oxide`, MIT,
//!   <https://github.com/Frommi/miniz_oxide/blob/b28c237471f63efebe152bddd962dc57bbb531eb/miniz_oxide/src/inflate/core.rs>
//!   <https://github.com/Frommi/miniz_oxide/blob/b28c237471f63efebe152bddd962dc57bbb531eb/miniz_oxide/src/inflate/output_buffer.rs>
//!   <https://github.com/Frommi/miniz_oxide/blob/b28c237471f63efebe152bddd962dc57bbb531eb/miniz_oxide/src/shared.rs>
//!   Copyright 2013-2014 RAD Game Tools and Valve Software
//!   Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC
//!   Copyright (c) 2017 Frommi
//!   Copyright (c) 2017-2024 oyvindln
//!   Changed for Spare Kit: taken under the MIT choice of its MIT OR Zlib OR Apache-2.0; only raw
//!   DEFLATE into an output buffer that holds the whole output is kept, so the zlib header and
//!   Adler-32 states, the wrapping ring buffer and its masks, the 32-bit bit buffer and the
//!   block-boundary feature are left out; every slice is read with `get`, every count is checked
//!   or saturating within its stated bound, every cast is lossless; the match copy keeps its three
//!   cases (one byte repeated, apart, overlapping) as safe slice copies in place of the source's
//!   unrolled four-byte loop; the end of input is always "needs more input", and the caller says
//!   whether more exists, where the source took a flag and reported a full output in its place;
//!   failures are typed kinds instead of failure states with one status; a match that cannot fit
//!   no longer tests for a zero length it cannot have.
//! SPDX-License-Identifier: Apache-2.0 AND MIT
//!
//! Cost: linear in the input and output: a code costs one table read, or one per bit past ten; a
//! match is one copy, or one per repeat of a distance shorter than the match. The decoder is
//! about 10 KiB (three code tables) and allocates nothing.

use super::InflateErrorKind;

/// The codes of up to this many bits are read from the fast table at once.
const FAST_LOOKUP_BITS: u32 = 10;
/// Entries in the fast table, 2^[`FAST_LOOKUP_BITS`].
const FAST_LOOKUP_SIZE: usize = 1_024;
/// Nodes in a code's tree: two for each of up to 288 symbols.
const MAX_HUFF_TREE_SIZE: usize = 576;
/// The literal and length table, the distance table and the code-length table.
const LITLEN_TABLE: usize = 0;
const DIST_TABLE: usize = 1;
const HUFFLEN_TABLE: usize = 2;
/// The code lengths of the literal and length table and of the distance table, read as one run.
const LEN_CODES_SIZE: usize = 512;

/// A fast-table entry for bits no code starts with: length 1 and the symbol 286, which no table
/// may yield, so a stray code fails where the symbol is checked.
const INVALID_CODE: i16 = 798;

/// The fewest entries of each table a dynamic block declares (HLIT + 257, HDIST + 1, HCLEN + 4).
const MIN_TABLE_SIZES: [u16; 3] = [257, 1, 4];

/// The order the code-length code's lengths are stored in (RFC 1951, 3.2.7).
const HUFFMAN_LENGTH_ORDER: [u8; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// The base length of each length symbol from 257, and two spares.
#[rustfmt::skip]
const LENGTH_BASE: [u16; 31] = [
    3,  4,  5,  6,  7,  8,  9,  10,  11,  13,  15,  17,  19,  23,  27,  31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 512, 512,
];

/// The extra bits of each length symbol from 257, and two spares.
#[rustfmt::skip]
const LENGTH_EXTRA: [u8; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0,
];

/// The base distance of each distance symbol.
#[rustfmt::skip]
const DIST_BASE: [u16; 30] = [
    1,    2,    3,    4,    5,    7,      9,      13,     17,     25,    33,
    49,   65,   97,   129,  193,  257,    385,    513,    769,    1025,  1537,
    2049, 3073, 4097, 6145, 8193, 12_289, 16_385, 24_577,
];

/// The extra bits of distance symbol `code`: none for 0 to 3, then one more for every two codes.
const fn num_extra_bits_for_distance_code(code: u8) -> u8 {
    (code >> 1).saturating_sub(1)
}

/// The low `n` bits of `buf`.
fn low_bits(buf: u64, n: u32) -> u64 {
    buf & 1_u64
        .checked_shl(n)
        .map_or(u64::MAX, |b| b.saturating_sub(1))
}

/// One code: a fast table for codes of up to ten bits and a tree for longer ones.
#[derive(Clone, Debug)]
struct HuffmanTable {
    /// Indexed by the next ten bits: a symbol in the low 9 bits and its code's length above them,
    /// or, negative, the root of the tree that holds the longer codes starting with those bits.
    look_up: [i16; FAST_LOOKUP_SIZE],
    /// Positive values are symbols, negative ones the next node (its left child at `!value`).
    tree: [i16; MAX_HUFF_TREE_SIZE],
}

impl HuffmanTable {
    const fn new() -> HuffmanTable {
        HuffmanTable {
            look_up: [0; FAST_LOOKUP_SIZE],
            tree: [0; MAX_HUFF_TREE_SIZE],
        }
    }

    /// The fast table's entry for the next ten bits.
    fn fast_lookup(&self, bit_buf: u64) -> i16 {
        let index = usize::try_from(low_bits(bit_buf, FAST_LOOKUP_BITS)).unwrap_or(0);
        self.look_up.get(index).copied().unwrap_or(INVALID_CODE)
    }

    /// Walks the tree from a negative fast-table entry, one bit a node from bit `code_len`, to a
    /// symbol; returns it and the code's length. The tree's nodes only ever point to later nodes,
    /// and a step outside it reads `i16::MAX`, a symbol no table yields, so the walk ends.
    fn tree_lookup(&self, fast_symbol: i32, bit_buf: u64, mut code_len: u32) -> (i32, u32) {
        let mut symbol = fast_symbol;
        loop {
            let right = bit_buf.checked_shr(code_len).unwrap_or(0) & 1 == 1;
            let node = usize::try_from(!symbol)
                .ok()
                .and_then(|left| left.checked_add(usize::from(right)))
                .and_then(|i| self.tree.get(i))
                .copied()
                .unwrap_or(i16::MAX);
            symbol = i32::from(node);
            // At most the tree's 576 nodes deep.
            code_len = code_len.saturating_add(1);
            if symbol >= 0 {
                break;
            }
        }
        (symbol, code_len)
    }

    /// The symbol the next bits hold and its code's length; for a fast-table hit the symbol keeps
    /// its length in the bits above the ninth.
    fn lookup(&self, bit_buf: u64) -> (i32, u32) {
        let symbol = i32::from(self.fast_lookup(bit_buf));
        if symbol >= 0 {
            (symbol, (symbol >> 9).unsigned_abs())
        } else {
            self.tree_lookup(symbol, bit_buf, FAST_LOOKUP_BITS)
        }
    }
}

/// Where the decoder is in the stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Start,
    ReadBlockHeader,
    BlockTypeNoCompression,
    RawHeader,
    RawMemcpy1,
    RawMemcpy2,
    ReadTableSizes,
    ReadHufflenTableCodeSize,
    ReadLitlenDistTablesCodeSize,
    ReadExtraBitsCodeSize,
    DecodeLitlen,
    WriteSymbol,
    ReadExtraBitsLitlen,
    DecodeDistance,
    ReadExtraBitsDistance,
    RawReadFirstByte,
    RawStoreFirstByte,
    WriteLenBytesToEnd,
    BlockDone,
    HuffDecodeOuterLoop1,
    HuffDecodeOuterLoop2,
    DoneForever,
    /// The stream is malformed; the decoder stays here.
    Failed(InflateErrorKind),
}

/// How a [`Decoder::step`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// The final block ended.
    Done,
    /// Every input byte given is used, and the stream goes on.
    NeedsInput,
    /// The output buffer is full, and the stream goes on.
    OutputFull,
    /// The stream is malformed.
    Failed(InflateErrorKind),
}

/// What one state asks the loop to do next.
enum Action {
    /// Run the same state again.
    None,
    /// Move to another state.
    Jump(State),
    /// Stop the step with this status.
    End(Status),
}

const fn fail(kind: InflateErrorKind) -> Action {
    Action::Jump(State::Failed(kind))
}

/// The input not yet read.
struct Input<'a> {
    rest: &'a [u8],
}

impl Input<'_> {
    const fn len(&self) -> usize {
        self.rest.len()
    }

    fn read_byte(&mut self) -> Result<u8, Action> {
        let (&byte, rest) = self
            .rest
            .split_first()
            .ok_or(Action::End(Status::NeedsInput))?;
        self.rest = rest;
        Ok(byte)
    }
}

/// The output buffer: everything written before `pos` is the window matches copy from.
struct Output<'a> {
    buf: &'a mut [u8],
    /// At most `buf.len()`.
    pos: usize,
}

impl Output<'_> {
    const fn left(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Writes `byte` at `pos`; the caller has checked there is room.
    fn write_byte(&mut self, byte: u8) {
        if let Some(slot) = self.buf.get_mut(self.pos) {
            *slot = byte;
            self.pos = self.pos.saturating_add(1);
        }
    }

    /// Writes `data` at `pos`; the caller has checked there is room.
    fn write_slice(&mut self, data: &[u8]) {
        let end = self.pos.saturating_add(data.len());
        if let Some(into) = self.buf.get_mut(self.pos..end) {
            into.copy_from_slice(data);
            self.pos = end;
        }
    }

    /// Copies `len` bytes from `dist` bytes back to `pos`, byte by byte in effect, so a distance
    /// shorter than the length repeats its bytes (RFC 1951, 3.2.3). `None`, writing nothing, if
    /// the distance reaches before the buffer or the bytes do not fit.
    fn copy_match(&mut self, dist: usize, len: usize) -> Option<()> {
        let from = self.pos.checked_sub(dist)?;
        let end = self.pos.checked_add(len)?;
        if dist == 0 || end > self.buf.len() {
            return None;
        }
        if dist == 1 {
            // One byte repeated.
            let byte = *self.buf.get(from)?;
            self.buf.get_mut(self.pos..end)?.fill(byte);
        } else {
            // Apart, in one copy; overlapping, in copies of `dist` bytes, each from bytes already
            // written.
            let mut at = self.pos;
            while at < end {
                let n = dist.min(end.saturating_sub(at));
                let (written, rest) = self.buf.split_at_mut_checked(at)?;
                let src = written.get(at.saturating_sub(dist)..)?.get(..n)?;
                rest.get_mut(..n)?.copy_from_slice(src);
                at = at.saturating_add(n);
            }
        }
        self.pos = end;
        Some(())
    }
}

/// The hot fields, kept apart from the decoder while a step runs.
#[derive(Clone, Copy)]
struct Locals {
    /// Bits read and not yet used, the next in the lowest place.
    bit_buf: u64,
    /// How many: at most 61 (fewer than 30 topped up by 32).
    num_bits: u32,
    dist: u32,
    counter: u32,
    num_extra: u8,
}

impl Locals {
    /// Appends `bytes` bits of `value` above the bits held.
    fn push(&mut self, value: u64, bytes: u32) {
        self.bit_buf |= value.checked_shl(self.num_bits).unwrap_or(0);
        // At most 61, as above.
        self.num_bits = self.num_bits.saturating_add(bytes.saturating_mul(8));
    }

    /// Drops the next `n` bits, `n` at most the bits held.
    fn consume(&mut self, n: u32) {
        self.bit_buf = self.bit_buf.checked_shr(n).unwrap_or(0);
        self.num_bits = self.num_bits.saturating_sub(n);
    }

    /// Takes the next `n` bits, `n` at most the bits held.
    fn take(&mut self, n: u32) -> u64 {
        let bits = low_bits(self.bit_buf, n);
        self.consume(n);
        bits
    }

    /// With fewer than 30 bits held, reads four bytes, so at least 30 are held; the caller has
    /// checked that four remain.
    fn fill(&mut self, input: &mut Input<'_>) {
        if self.num_bits < 30
            && let [a, b, c, d, rest @ ..] = input.rest
        {
            self.push(u64::from(u32::from_le_bytes([*a, *b, *c, *d])), 4);
            input.rest = rest;
        }
    }
}

/// Reads bits until `amount` are held, then takes them.
fn read_bits(l: &mut Locals, amount: u32, input: &mut Input<'_>) -> Result<u64, Action> {
    while l.num_bits < amount {
        let byte = input.read_byte()?;
        l.push(u64::from(byte), 1);
    }
    Ok(l.take(amount))
}

/// Drops the bits up to the next byte boundary.
fn pad_to_bytes(l: &mut Locals) {
    let n = l.num_bits & 7;
    l.consume(n);
}

/// The DEFLATE decoder of one stream.
#[derive(Clone, Debug)]
pub(crate) struct Decoder {
    state: State,
    num_bits: u32,
    /// Whether the current block is the last.
    finish: bool,
    /// The current block's type, or, while a dynamic block's tables are built, the table being
    /// built.
    block_type: u8,
    dist: u32,
    counter: u32,
    num_extra: u8,
    table_sizes: [u16; 3],
    bit_buf: u64,
    tables: [HuffmanTable; 3],
    code_size_literal: [u8; 288],
    code_size_dist: [u8; 32],
    code_size_huffman: [u8; 19],
    raw_header: [u8; 4],
    len_codes: [u8; LEN_CODES_SIZE],
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder {
            state: State::Start,
            num_bits: 0,
            finish: false,
            block_type: 0,
            dist: 0,
            counter: 0,
            num_extra: 0,
            table_sizes: [0; 3],
            bit_buf: 0,
            tables: [
                HuffmanTable::new(),
                HuffmanTable::new(),
                HuffmanTable::new(),
            ],
            code_size_literal: [0; 288],
            code_size_dist: [0; 32],
            code_size_huffman: [0; 19],
            raw_header: [0; 4],
            len_codes: [0; LEN_CODES_SIZE],
        }
    }
}

/// What one [`Decoder::step`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) status: Status,
    /// Input bytes used; whole bytes read ahead into the bit buffer are given back, except when
    /// the step stopped for more input.
    pub(crate) consumed: usize,
    /// Output bytes written.
    pub(crate) written: usize,
}

impl Decoder {
    /// Bits read from the input and not yet used. After [`Status::Done`] they are whole bytes past
    /// the end of the stream that this step could not give back, since an earlier step read them.
    pub(crate) const fn held_bits(&self) -> u32 {
        self.num_bits
    }

    /// Decodes from `input` into `out` from `out_pos`, with `out[..out_pos]` the output so far.
    /// A position past the buffer's end is a full buffer.
    pub(crate) fn step(&mut self, input: &[u8], out: &mut [u8], out_pos: usize) -> Step {
        if out_pos > out.len() {
            return Step {
                status: Status::OutputFull,
                consumed: 0,
                written: 0,
            };
        }
        let mut input = Input { rest: input };
        let given = input.len();
        let mut out = Output {
            buf: out,
            pos: out_pos,
        };
        let mut l = Locals {
            bit_buf: self.bit_buf,
            num_bits: self.num_bits,
            dist: self.dist,
            counter: self.counter,
            num_extra: self.num_extra,
        };
        let mut state = self.state;
        let status = loop {
            let action = match self.run(state, &mut l, &mut input, &mut out) {
                Ok(a) | Err(a) => a,
            };
            match action {
                Action::None => {}
                Action::Jump(next) => state = next,
                Action::End(status) => break status,
            }
        };
        let read = given.saturating_sub(input.len());
        // Whole bytes read ahead go back to the caller, at most those read in this step.
        let undo = if status == Status::NeedsInput {
            0
        } else {
            usize::try_from(l.num_bits / 8).unwrap_or(0).min(read)
        };
        let undo_bits = u32::try_from(undo).unwrap_or(0).saturating_mul(8);
        l.num_bits = l.num_bits.saturating_sub(undo_bits);
        self.state = state;
        self.bit_buf = low_bits(l.bit_buf, l.num_bits);
        self.num_bits = l.num_bits;
        self.dist = l.dist;
        self.counter = l.counter;
        self.num_extra = l.num_extra;
        Step {
            status,
            consumed: read.saturating_sub(undo),
            written: out.pos.saturating_sub(out_pos),
        }
    }

    /// Runs one state.
    #[allow(
        clippy::too_many_lines,
        reason = "one state machine: each arm is one state of the source's, kept in its order"
    )]
    fn run(
        &mut self,
        state: State,
        l: &mut Locals,
        input: &mut Input<'_>,
        out: &mut Output<'_>,
    ) -> Result<Action, Action> {
        Ok(match state {
            State::Start => {
                l.bit_buf = 0;
                l.num_bits = 0;
                l.dist = 0;
                l.counter = 0;
                l.num_extra = 0;
                Action::Jump(State::ReadBlockHeader)
            }

            // The block header: the last-block bit and two bits of type.
            State::ReadBlockHeader => {
                let bits = read_bits(l, 3, input)?;
                self.finish = bits & 1 == 1;
                let [kind, ..] = (bits >> 1 & 3).to_le_bytes();
                self.block_type = kind;
                match kind {
                    0 => Action::Jump(State::BlockTypeNoCompression),
                    1 => {
                        self.start_static_table();
                        self.init_tree(l)
                    }
                    2 => {
                        l.counter = 0;
                        Action::Jump(State::ReadTableSizes)
                    }
                    _ => fail(InflateErrorKind::BlockType),
                }
            }

            // A stored block starts at a byte boundary.
            State::BlockTypeNoCompression => {
                pad_to_bytes(l);
                l.counter = 0;
                Action::Jump(State::RawHeader)
            }

            // LEN and NLEN, from the bit buffer while it holds bytes, then from the input.
            State::RawHeader => {
                if l.counter < 4 {
                    let byte = if l.num_bits != 0 {
                        let [b, ..] = read_bits(l, 8, input)?.to_le_bytes();
                        b
                    } else {
                        input.read_byte()?
                    };
                    let at = usize::try_from(l.counter).unwrap_or(usize::MAX);
                    if let Some(slot) = self.raw_header.get_mut(at) {
                        *slot = byte;
                    }
                    l.counter = l.counter.saturating_add(1);
                    Action::None
                } else {
                    let [len_lo, len_hi, check_lo, check_hi] = self.raw_header;
                    let length = u16::from_le_bytes([len_lo, len_hi]);
                    let check = u16::from_le_bytes([check_lo, check_hi]);
                    l.counter = u32::from(length);
                    if length != !check {
                        fail(InflateErrorKind::StoredLength)
                    } else if l.counter == 0 {
                        Action::Jump(State::BlockDone)
                    } else if l.num_bits != 0 {
                        Action::Jump(State::RawReadFirstByte)
                    } else {
                        Action::Jump(State::RawMemcpy1)
                    }
                }
            }

            State::RawReadFirstByte => {
                let bits = read_bits(l, 8, input)?;
                l.dist = u32::try_from(bits).unwrap_or(0);
                Action::Jump(State::RawStoreFirstByte)
            }

            // A stored byte that was already in the bit buffer.
            State::RawStoreFirstByte => {
                if out.left() == 0 {
                    Action::End(Status::OutputFull)
                } else {
                    let [byte, ..] = l.dist.to_le_bytes();
                    out.write_byte(byte);
                    // At least 1: RawHeader moves here only with a length left.
                    l.counter = l.counter.saturating_sub(1);
                    if l.counter == 0 || l.num_bits == 0 {
                        Action::Jump(State::RawMemcpy1)
                    } else {
                        Action::Jump(State::RawReadFirstByte)
                    }
                }
            }

            State::RawMemcpy1 => {
                if l.counter == 0 {
                    Action::Jump(State::BlockDone)
                } else if out.left() == 0 {
                    Action::End(Status::OutputFull)
                } else {
                    Action::Jump(State::RawMemcpy2)
                }
            }

            // As many stored bytes as the input, the room and the block allow, in one copy.
            State::RawMemcpy2 => {
                if input.len() == 0 {
                    Action::End(Status::NeedsInput)
                } else {
                    let n = out
                        .left()
                        .min(input.len())
                        .min(usize::try_from(l.counter).unwrap_or(usize::MAX));
                    let (bytes, rest) = input.rest.split_at_checked(n).unwrap_or((&[], input.rest));
                    out.write_slice(bytes);
                    input.rest = rest;
                    // n is at most the counter, a stored length below 2^16.
                    l.counter = l
                        .counter
                        .saturating_sub(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
                    Action::Jump(State::RawMemcpy1)
                }
            }

            // HLIT, HDIST and HCLEN.
            State::ReadTableSizes => {
                let at = usize::try_from(l.counter).unwrap_or(usize::MAX);
                if at < 3 {
                    let width = if at == 2 { 4 } else { 5 };
                    let bits = read_bits(l, width, input)?;
                    let min = MIN_TABLE_SIZES.get(at).copied().unwrap_or(0);
                    if let Some(size) = self.table_sizes.get_mut(at) {
                        // At most 31 + 257.
                        *size = u16::try_from(bits).unwrap_or(0).saturating_add(min);
                    }
                    l.counter = l.counter.saturating_add(1);
                    Action::None
                } else {
                    self.code_size_huffman.fill(0);
                    l.counter = 0;
                    // RFC 1951 allows at most 286 literal and length codes; zlib refuses more
                    // than 30 distance codes, as the source does.
                    let [lit, dist, _] = self.table_sizes;
                    if lit <= 286 && dist <= 30 {
                        Action::Jump(State::ReadHufflenTableCodeSize)
                    } else {
                        fail(InflateErrorKind::TableSizes)
                    }
                }
            }

            // The code-length code's lengths, three bits each, in their stored order.
            State::ReadHufflenTableCodeSize => {
                let [_, _, hclen] = self.table_sizes;
                if l.counter < u32::from(hclen) {
                    let bits = read_bits(l, 3, input)?;
                    let at = usize::try_from(l.counter).unwrap_or(usize::MAX);
                    let symbol = HUFFMAN_LENGTH_ORDER.get(at).copied().unwrap_or(0);
                    if let Some(slot) = self.code_size_huffman.get_mut(usize::from(symbol)) {
                        let [len, ..] = bits.to_le_bytes();
                        *slot = len;
                    }
                    l.counter = l.counter.saturating_add(1);
                    Action::None
                } else {
                    self.table_sizes = [self.table_sizes[0], self.table_sizes[1], 19];
                    self.init_tree(l)
                }
            }

            // The literal and length and distance code lengths, read with the code-length code.
            State::ReadLitlenDistTablesCodeSize => {
                let [lit, dist, _] = self.table_sizes;
                let total = u32::from(lit).saturating_add(u32::from(dist));
                if l.counter < total {
                    let symbol = self.decode_huffman_code(l, HUFFLEN_TABLE, input)?;
                    l.dist = symbol.unsigned_abs();
                    if l.dist < 16 {
                        let at = usize::try_from(l.counter).unwrap_or(usize::MAX);
                        if let Some(slot) = self.len_codes.get_mut(at) {
                            let [len, ..] = l.dist.to_le_bytes();
                            *slot = len;
                        }
                        l.counter = l.counter.saturating_add(1);
                        Action::None
                    } else if l.dist == 16 && l.counter == 0 {
                        fail(InflateErrorKind::RepeatFirst)
                    } else {
                        // 16, 17 and 18 repeat with 2, 3 and 7 extra bits.
                        l.num_extra = match l.dist {
                            16 => 2,
                            17 => 3,
                            _ => 7,
                        };
                        Action::Jump(State::ReadExtraBitsCodeSize)
                    }
                } else if l.counter != total {
                    fail(InflateErrorKind::RepeatOverrun)
                } else {
                    let lit = usize::from(lit);
                    let end = lit.saturating_add(usize::from(dist));
                    let lit_from = self.len_codes.get(..lit).unwrap_or(&[]);
                    if let Some(into) = self.code_size_literal.get_mut(..lit_from.len()) {
                        into.copy_from_slice(lit_from);
                    }
                    let dist_from = self.len_codes.get(lit..end).unwrap_or(&[]);
                    if let Some(into) = self.code_size_dist.get_mut(..dist_from.len()) {
                        into.copy_from_slice(dist_from);
                    }
                    self.block_type = self.block_type.saturating_sub(1);
                    self.init_tree(l)
                }
            }

            // A repeat: of the previous length (16) or of zero (17, 18).
            State::ReadExtraBitsCodeSize => {
                let extra = read_bits(l, u32::from(l.num_extra), input)?;
                let more = if l.dist == 18 { 11 } else { 3 };
                // At most 127 + 11.
                let count = usize::try_from(extra).unwrap_or(0).saturating_add(more);
                let start = usize::try_from(l.counter).unwrap_or(usize::MAX);
                let value = if l.dist == 16 {
                    // The counter is above 0 here: a first repeat of 16 failed above.
                    self.len_codes
                        .get(start.saturating_sub(1))
                        .copied()
                        .unwrap_or(0)
                } else {
                    0
                };
                // At most 316 + 138, within the 512 entries.
                let end = start.saturating_add(count);
                if let Some(run) = self.len_codes.get_mut(start..end) {
                    run.fill(value);
                }
                l.counter = l.counter.saturating_add(u32::try_from(count).unwrap_or(0));
                Action::Jump(State::ReadLitlenDistTablesCodeSize)
            }

            State::DecodeLitlen => {
                if input.len() < 4 || out.left() < 2 {
                    // Near the end of the input or the room: one code at a time, reading no more
                    // bytes than the code needs.
                    let symbol = self.decode_huffman_code(l, LITLEN_TABLE, input)?;
                    l.counter = symbol.unsigned_abs();
                    Action::Jump(State::WriteSymbol)
                } else if out.left() >= 259 && input.len() >= 14 {
                    self.decompress_fast(l, input, out)
                } else {
                    // Room for two literals and bits for two codes.
                    l.fill(input);
                    let (symbol, code_len) = self.literal_table().lookup(l.bit_buf);
                    l.counter = symbol.unsigned_abs();
                    l.consume(code_len);
                    if l.counter & 256 != 0 {
                        Action::Jump(State::HuffDecodeOuterLoop1)
                    } else {
                        let (symbol, code_len) = self.literal_table().lookup(l.bit_buf);
                        l.consume(code_len);
                        let [first, ..] = l.counter.to_le_bytes();
                        out.write_byte(first);
                        if symbol & 256 != 0 {
                            l.counter = symbol.unsigned_abs();
                            Action::Jump(State::HuffDecodeOuterLoop1)
                        } else {
                            let [second, ..] = symbol.to_le_bytes();
                            out.write_byte(second);
                            Action::None
                        }
                    }
                }
            }

            State::WriteSymbol => {
                if l.counter >= 256 {
                    Action::Jump(State::HuffDecodeOuterLoop1)
                } else if out.left() > 0 {
                    let [byte, ..] = l.counter.to_le_bytes();
                    out.write_byte(byte);
                    Action::Jump(State::DecodeLitlen)
                } else {
                    Action::End(Status::OutputFull)
                }
            }

            // The end of block, or a length's base and extra bits.
            State::HuffDecodeOuterLoop1 => {
                l.counter &= 511;
                if l.counter == 256 {
                    Action::Jump(State::BlockDone)
                } else if l.counter > 285 {
                    fail(InflateErrorKind::Literal)
                } else {
                    let at = usize::try_from(l.counter.saturating_sub(257)).unwrap_or(usize::MAX);
                    l.num_extra = LENGTH_EXTRA.get(at).copied().unwrap_or(0);
                    l.counter = u32::from(LENGTH_BASE.get(at).copied().unwrap_or(0));
                    if l.num_extra != 0 {
                        Action::Jump(State::ReadExtraBitsLitlen)
                    } else {
                        Action::Jump(State::DecodeDistance)
                    }
                }
            }

            State::ReadExtraBitsLitlen => {
                let extra = read_bits(l, u32::from(l.num_extra), input)?;
                // At most 227 + 31.
                l.counter = l.counter.saturating_add(u32::try_from(extra).unwrap_or(0));
                Action::Jump(State::DecodeDistance)
            }

            State::DecodeDistance => {
                let symbol = self.decode_huffman_code(l, DIST_TABLE, input)?;
                let at = usize::try_from(symbol).unwrap_or(usize::MAX);
                match DIST_BASE.get(at) {
                    None => fail(InflateErrorKind::Distance),
                    Some(&base) => {
                        let [code, ..] = at.to_le_bytes();
                        l.num_extra = num_extra_bits_for_distance_code(code);
                        l.dist = u32::from(base);
                        if l.num_extra != 0 {
                            Action::Jump(State::ReadExtraBitsDistance)
                        } else {
                            Action::Jump(State::HuffDecodeOuterLoop2)
                        }
                    }
                }
            }

            State::ReadExtraBitsDistance => {
                let extra = read_bits(l, u32::from(l.num_extra), input)?;
                // At most 24,577 + 8,191.
                l.dist = l.dist.saturating_add(u32::try_from(extra).unwrap_or(0));
                Action::Jump(State::HuffDecodeOuterLoop2)
            }

            // The match: refused if it reaches before the output, written whole if it fits.
            State::HuffDecodeOuterLoop2 => {
                let dist = usize::try_from(l.dist).unwrap_or(usize::MAX);
                let len = usize::try_from(l.counter).unwrap_or(usize::MAX);
                if dist > out.pos {
                    fail(InflateErrorKind::DistanceTooFar)
                } else if len > out.left() {
                    Action::Jump(State::WriteLenBytesToEnd)
                } else {
                    match out.copy_match(dist, len) {
                        Some(()) => Action::Jump(State::DecodeLitlen),
                        None => fail(InflateErrorKind::DistanceTooFar),
                    }
                }
            }

            // A match that does not fit: as much as fits, then the rest when there is room.
            State::WriteLenBytesToEnd => {
                if out.left() > 0 {
                    let dist = usize::try_from(l.dist).unwrap_or(usize::MAX);
                    let len = out
                        .left()
                        .min(usize::try_from(l.counter).unwrap_or(usize::MAX));
                    if out.copy_match(dist, len).is_none() {
                        return Ok(fail(InflateErrorKind::DistanceTooFar));
                    }
                    l.counter = l
                        .counter
                        .saturating_sub(u32::try_from(len).unwrap_or(u32::MAX));
                    if l.counter == 0 {
                        Action::Jump(State::DecodeLitlen)
                    } else {
                        Action::None
                    }
                } else {
                    Action::End(Status::OutputFull)
                }
            }

            // After the final block the bits up to the byte boundary are dropped.
            State::BlockDone => {
                if self.finish {
                    pad_to_bytes(l);
                    Action::Jump(State::DoneForever)
                } else {
                    Action::Jump(State::ReadBlockHeader)
                }
            }

            State::DoneForever => Action::End(Status::Done),
            State::Failed(kind) => Action::End(Status::Failed(kind)),
        })
    }

    fn literal_table(&self) -> &HuffmanTable {
        let [lit, _, _] = &self.tables;
        lit
    }

    /// Decodes one code of `table` into a symbol, reading bytes as it needs them: two at a time
    /// while two remain, and near the end of the input only as many as the code needs, so a code
    /// whose bits are all present is decoded however the input was split.
    fn decode_huffman_code(
        &self,
        l: &mut Locals,
        table: usize,
        input: &mut Input<'_>,
    ) -> Result<i32, Action> {
        let Some(t) = self.tables.get(table) else {
            return Err(fail(InflateErrorKind::CodeLengths));
        };
        // A code is at most 15 bits long.
        if l.num_bits < 15 {
            if input.len() < 2 {
                loop {
                    let mut temp = i32::from(t.fast_lookup(l.bit_buf));
                    if temp >= 0 {
                        let code_len = (temp >> 9).unsigned_abs();
                        if code_len != 0 && l.num_bits >= code_len {
                            break;
                        }
                    } else if l.num_bits > FAST_LOOKUP_BITS {
                        let mut code_len = FAST_LOOKUP_BITS;
                        loop {
                            let right = l.bit_buf.checked_shr(code_len).unwrap_or(0) & 1 == 1;
                            temp = usize::try_from(!temp)
                                .ok()
                                .and_then(|left| left.checked_add(usize::from(right)))
                                .and_then(|i| t.tree.get(i))
                                .map_or(i32::from(i16::MAX), |&node| i32::from(node));
                            // At most the tree's depth.
                            code_len = code_len.saturating_add(1);
                            if temp >= 0 || l.num_bits < code_len.saturating_add(1) {
                                break;
                            }
                        }
                        if temp >= 0 {
                            break;
                        }
                    }
                    let byte = input.read_byte()?;
                    l.push(u64::from(byte), 1);
                    if l.num_bits >= 15 {
                        break;
                    }
                }
            } else if let [a, b, rest @ ..] = input.rest {
                l.push(u64::from(u16::from_le_bytes([*a, *b])), 2);
                input.rest = rest;
            }
        }
        // At least as many bits as the code are held now.
        let mut symbol = i32::from(t.fast_lookup(l.bit_buf));
        let code_len;
        if symbol >= 0 {
            code_len = (symbol >> 9).unsigned_abs();
            symbol &= 511;
        } else {
            (symbol, code_len) = t.tree_lookup(symbol, l.bit_buf, FAST_LOOKUP_BITS);
        }
        l.consume(code_len);
        Ok(symbol)
    }

    /// The fixed codes of a type 1 block (RFC 1951, 3.2.6).
    fn start_static_table(&mut self) {
        self.table_sizes = [288, 32, self.table_sizes[2]];
        for (symbol, size) in self.code_size_literal.iter_mut().enumerate() {
            *size = match symbol {
                0..=143 | 280.. => 8,
                144..=255 => 9,
                _ => 7,
            };
        }
        self.code_size_dist.fill(5);
    }

    /// Builds the table of `block_type` from its code lengths, then the tables below it, as a
    /// dynamic block reads them: the code-length code alone, then distances and literals together.
    /// Refuses a code that is over-subscribed at any length, and an incomplete one unless it is the
    /// code-length code's or has no code longer than one bit (no codes, or one), as zlib does.
    #[allow(
        clippy::too_many_lines,
        reason = "one construction, as the source's: count the lengths, check the code, fill the tables"
    )]
    fn init_tree(&mut self, l: &mut Locals) -> Action {
        loop {
            let bt = usize::from(self.block_type);
            let [lit_table, dist_table, len_table] = &mut self.tables;
            let (code_sizes, table): (&[u8], &mut HuffmanTable) = match bt {
                LITLEN_TABLE => (&self.code_size_literal, lit_table),
                DIST_TABLE => (&self.code_size_dist, dist_table),
                HUFFLEN_TABLE => (&self.code_size_huffman, len_table),
                _ => return fail(InflateErrorKind::CodeLengths),
            };

            table.look_up.fill(INVALID_CODE);
            // The code-length code's codes are at most 7 bits, so its tree is never read.
            if bt != HUFFLEN_TABLE {
                table.tree.fill(0);
            }

            let table_size = usize::from(self.table_sizes.get(bt).copied().unwrap_or(0));
            let Some(sizes) = code_sizes.get(..table_size) else {
                return fail(InflateErrorKind::CodeLengths);
            };

            let mut total_symbols = [0_u16; 16];
            for &size in sizes {
                let Some(count) = total_symbols.get_mut(usize::from(size)) else {
                    return fail(InflateErrorKind::CodeLengths);
                };
                // At most 288.
                *count = count.saturating_add(1);
            }

            // The first code of each length, and the check that no length is over-subscribed
            // (zlib's inftrees.c): `left` codes of the current length are still free.
            let mut next_code = [0_u32; 17];
            let mut total = 0_u32;
            let mut max_code_len = 0_usize;
            let mut left = 1_i32;
            for (len, &count) in total_symbols.iter().enumerate().skip(1) {
                if count > 0 {
                    max_code_len = len;
                }
                // At most 288 codes summed, doubled 15 times: below 2^25.
                total = total.saturating_add(u32::from(count)).saturating_mul(2);
                if let Some(next) = next_code.get_mut(len.saturating_add(1)) {
                    *next = total;
                }
                // Between -288 and 2^15.
                left = left.saturating_mul(2).saturating_sub(i32::from(count));
                if left < 0 {
                    return fail(InflateErrorKind::CodeLengths);
                }
            }
            // A complete code fills all 2^16 codes of 16 bits.
            if total != 65_536 && (bt == HUFFLEN_TABLE || max_code_len > 1) {
                return fail(InflateErrorKind::CodeLengths);
            }

            let mut tree_next: i16 = -1;
            for (symbol_index, &size) in sizes.iter().enumerate() {
                let code_size = u32::from(size & 15);
                if code_size == 0 {
                    continue;
                }
                let Some(next) = next_code.get_mut(usize::from(size & 15)) else {
                    return fail(InflateErrorKind::CodeLengths);
                };
                let cur_code = *next;
                *next = next.saturating_add(1);

                // The code's bits, first bit lowest, as the stream holds them.
                let [lo, hi, ..] = low_bits(u64::from(cur_code), code_size).to_le_bytes();
                let n = u16::from_le_bytes([lo, hi]);
                let mut rev_code = n
                    .reverse_bits()
                    .checked_shr(16_u32.saturating_sub(code_size))
                    .unwrap_or(0);
                let Ok(symbol) = i16::try_from(symbol_index) else {
                    return fail(InflateErrorKind::CodeLengths);
                };

                if code_size <= FAST_LOOKUP_BITS {
                    // Every fast entry whose low bits are this code.
                    let entry = (i16::from(size & 15) << 9) | symbol;
                    let step = 1_u16.checked_shl(code_size).unwrap_or(u16::MAX);
                    while usize::from(rev_code) < FAST_LOOKUP_SIZE {
                        if let Some(slot) = table.look_up.get_mut(usize::from(rev_code)) {
                            *slot = entry;
                        }
                        rev_code = rev_code.saturating_add(step);
                    }
                    continue;
                }

                // A longer code: its first ten bits lead to a tree, one node a bit after them.
                let fast = usize::from(rev_code) & (FAST_LOOKUP_SIZE - 1);
                let Some(root) = table.look_up.get_mut(fast) else {
                    return fail(InflateErrorKind::CodeLengths);
                };
                let mut tree_cur = *root;
                if tree_cur == INVALID_CODE {
                    *root = tree_next;
                    tree_cur = tree_next;
                    // Two nodes a code: at least -577.
                    tree_next = tree_next.saturating_sub(2);
                }

                rev_code >>= FAST_LOOKUP_BITS - 1;
                for _ in FAST_LOOKUP_BITS.saturating_add(1)..code_size {
                    rev_code >>= 1;
                    tree_cur = tree_cur.saturating_sub(i16::from(rev_code & 1 == 1));
                    let Some(node) = usize::try_from(!tree_cur)
                        .ok()
                        .and_then(|i| table.tree.get_mut(i))
                    else {
                        return fail(InflateErrorKind::CodeLengths);
                    };
                    if *node == 0 {
                        *node = tree_next;
                        tree_cur = tree_next;
                        tree_next = tree_next.saturating_sub(2);
                    } else {
                        tree_cur = *node;
                    }
                }

                rev_code >>= 1;
                tree_cur = tree_cur.saturating_sub(i16::from(rev_code & 1 == 1));
                let Some(leaf) = usize::try_from(!tree_cur)
                    .ok()
                    .and_then(|i| table.tree.get_mut(i))
                else {
                    return fail(InflateErrorKind::CodeLengths);
                };
                *leaf = symbol;
            }

            if bt == HUFFLEN_TABLE {
                l.counter = 0;
                return Action::Jump(State::ReadLitlenDistTablesCodeSize);
            }
            if bt == LITLEN_TABLE {
                break;
            }
            self.block_type = self.block_type.saturating_sub(1);
        }
        l.counter = 0;
        Action::Jump(State::DecodeLitlen)
    }

    /// The fast loop, while at least 259 bytes of room (two literals, or a literal and the
    /// longest match) and 14 input bytes (three four-byte fills) remain at the top of each turn:
    /// literals two at a time, and each match whole, with no state change between them.
    fn decompress_fast(
        &self,
        l: &mut Locals,
        input: &mut Input<'_>,
        out: &mut Output<'_>,
    ) -> Action {
        let lit_table = self.literal_table();
        let [_, dist_table, _] = &self.tables;
        loop {
            loop {
                if out.left() < 259 || input.len() < 14 {
                    return Action::Jump(State::DecodeLitlen);
                }
                // At least 30 bits: two codes.
                l.fill(input);
                let (symbol, code_len) = lit_table.lookup(l.bit_buf);
                l.counter = symbol.unsigned_abs();
                l.consume(code_len);
                if l.counter & 256 != 0 {
                    break;
                }
                let (symbol, code_len) = lit_table.lookup(l.bit_buf);
                l.consume(code_len);
                let [first, ..] = l.counter.to_le_bytes();
                out.write_byte(first);
                if symbol & 256 != 0 {
                    l.counter = symbol.unsigned_abs();
                    break;
                }
                let [second, ..] = symbol.to_le_bytes();
                out.write_byte(second);
            }

            l.counter &= 511;
            if l.counter == 256 {
                return Action::Jump(State::BlockDone);
            }
            if l.counter > 285 {
                return fail(InflateErrorKind::Literal);
            }
            let at = usize::try_from(l.counter.saturating_sub(257)).unwrap_or(usize::MAX);
            l.num_extra = LENGTH_EXTRA.get(at).copied().unwrap_or(0);
            l.counter = u32::from(LENGTH_BASE.get(at).copied().unwrap_or(0));

            // At least 30 bits: five extra bits and a distance code.
            l.fill(input);
            if l.num_extra != 0 {
                let extra = l.take(u32::from(l.num_extra));
                l.counter = l.counter.saturating_add(u32::try_from(extra).unwrap_or(0));
            }

            let (symbol, code_len) = dist_table.lookup(l.bit_buf);
            l.consume(code_len);
            let at = usize::try_from(symbol & 511).unwrap_or(usize::MAX);
            let Some(&base) = DIST_BASE.get(at) else {
                return fail(InflateErrorKind::Distance);
            };
            let [code, ..] = at.to_le_bytes();
            l.num_extra = num_extra_bits_for_distance_code(code);
            l.dist = u32::from(base);
            if l.num_extra != 0 {
                // At least 30 bits: thirteen extra bits.
                l.fill(input);
                let extra = l.take(u32::from(l.num_extra));
                l.dist = l.dist.saturating_add(u32::try_from(extra).unwrap_or(0));
            }

            let dist = usize::try_from(l.dist).unwrap_or(usize::MAX);
            if dist > out.pos {
                return fail(InflateErrorKind::DistanceTooFar);
            }
            let len = usize::try_from(l.counter).unwrap_or(usize::MAX);
            if out.copy_match(dist, len).is_none() {
                return fail(InflateErrorKind::DistanceTooFar);
            }
        }
    }
}
