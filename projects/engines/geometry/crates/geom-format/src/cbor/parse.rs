//! Reading the text form back: diagnostic notation with hexadecimal floats, turned into SKG's
//! deterministic CBOR bytes.
//!
//! What it does: [`from_text`] reads the one item of the text into a flat list of heads, in three
//! passes: it parses the text with an explicit stack, counting each array's items and each map's
//! pairs as their brackets close; it writes every head in its shortest form and every float in the
//! shortest width that holds it; and it checks the bytes with the strict decoder, so whatever the
//! decoder refuses (keys out of order, a repeated key, a tag outside the subset, a bignum that fits
//! in 64 bits, a column of the wrong length) is refused here too, at the line and column of the
//! item at fault.
//!
//! What it reads: the layout [`super::text::to_text`] writes, and more freely spaced text of the
//! same items. Whitespace and comments (`/ ... /`, RFC 8610 Appendix G.6) may stand between any two
//! tokens and inside `h'...'`. Integers are decimal, from −2^64 to 2^64 − 1. A float must be a
//! hexadecimal float (`0x1.8p+1`, or `-0x0p+0` for −0.0) that a double holds exactly: decimal
//! fractions are refused, because most of them no double holds. A text string takes JSON's escapes,
//! a surrogate pair included, and no raw control character. A tag is its number followed at once by
//! `(`, its item and `)`.
//!
//! Guarantees:
//!
//! - **Canonical bytes or an error.** What it returns is accepted by the strict decoder at the same
//!   depth: the one encoding SKG's canonical form gives the items the text describes.
//! - **Lossless.** `from_text(to_text(bytes))` gives back `bytes` for every input the decoder
//!   accepts, floats bit for bit, −0.0 included.
//! - **Exact or refused.** A number is never rounded: an integer out of range or a float no double
//!   holds is an error.
//! - **Never panics, no recursion.** Nesting is walked with an explicit stack no deeper than the
//!   caller's limit; every error carries the line and column where it was found.
//!
//! Method: RFC 8610 Appendix G (extended diagnostic notation), with G.5's hexadecimal floats and
//! G.6's comments, <https://www.rfc-editor.org/rfc/rfc8610#appendix-G>; RFC 8949 section 8
//! (diagnostic notation), <https://www.rfc-editor.org/rfc/rfc8949#section-8>; JSON's string
//! escapes, RFC 8259 section 7, <https://www.rfc-editor.org/rfc/rfc8259#section-7>; and RFC 8949
//! sections 3, 4.1 and 4.2.1 for the bytes written. A hexadecimal float m·2^e, m an integer, is a
//! double exactly when, with its trailing zero bits moved into e, m has at most 53 bits, e is at
//! least −1074 and its leading bit's exponent is at most 1023 (IEEE 754-2019, binary64).
//!
//! Cost: O(n) time and memory in the text: one list of heads, then the bytes, then one decoder
//! pass. Recursion: none.

use alloc::string::String;
use alloc::vec::Vec;

use geom_run::WorkMeter;

use super::decode::check;
use super::error::{CborErrorKind, TextError, TextErrorKind};
use super::float::{Width, shortest};

/// One head read from the text, with where its item starts.
#[derive(Debug)]
struct Node {
    line: u32,
    column: u32,
    value: Value,
}

/// What a head says, with a string's content.
#[derive(Debug)]
enum Value {
    Uint(u64),
    Nint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(u64),
    Map(u64),
    Tag(u64),
    Float(Width),
    Bool(bool),
    Null,
}

/// An open container or tag while parsing.
#[derive(Debug)]
enum Open {
    /// An array whose head is node `node`, with `count` items so far.
    Array { node: usize, count: u64 },
    /// A map whose head is node `node`, with `count` pairs so far, and whether the pair in
    /// progress has its key.
    Map {
        node: usize,
        count: u64,
        has_key: bool,
    },
    /// A tag, closed by `)`.
    Tag,
}

/// A position in the text.
#[derive(Debug)]
struct Cursor<'t> {
    text: &'t str,
    at: usize,
    line: u32,
    column: u32,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<char> {
        self.text
            .get(self.at..)
            .and_then(|rest| rest.chars().next())
    }

    fn rest(&self) -> &str {
        self.text.get(self.at..).unwrap_or("")
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at = self.at.saturating_add(c.len_utf8());
        if c == '\n' {
            self.line = self.line.saturating_add(1);
            self.column = 1;
        } else {
            self.column = self.column.saturating_add(1);
        }
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn error(&self, kind: TextErrorKind) -> TextError {
        TextError {
            line: self.line,
            column: self.column,
            kind,
        }
    }

    /// Skips whitespace and `/ ... /` comments.
    fn skip_space(&mut self) -> Result<(), TextError> {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\n' | '\r') => {
                    self.bump();
                }
                Some('/') => {
                    let start = self.error(TextErrorKind::OpenComment);
                    self.bump();
                    loop {
                        match self.bump() {
                            None => return Err(start),
                            Some('/') => break,
                            Some(_) => {}
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// Consumes `word` if the text continues with it.
    fn word(&mut self, word: &str) -> bool {
        if self.rest().starts_with(word) {
            for _ in word.chars() {
                self.bump();
            }
            true
        } else {
            false
        }
    }
}

/// A number read from the text.
enum Number {
    /// An integer, its sign and its magnitude, below 2^65.
    Int { negative: bool, magnitude: u128 },
    /// A float a double holds exactly.
    Float(f64),
}

/// The bytes of SKG's deterministic CBOR that the one item of `text` describes, nested at most
/// `depth` deep.
///
/// # Errors
///
/// [`TextError`] with the line and column of the first fault: text that is not diagnostic notation
/// of the subset, or that describes an item the strict decoder refuses (the kind is then
/// [`TextErrorKind::Cbor`] with the decoder's reason), or the meter's stop.
pub fn from_text(
    text: &str,
    depth: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u8>, TextError> {
    let nodes = parse(text, depth, meter)?;
    let mut bytes = Vec::new();
    let mut starts: Vec<(usize, u32, u32)> = Vec::new();
    for node in &nodes {
        starts.push((bytes.len(), node.line, node.column));
        write(&mut bytes, &node.value);
    }
    check(&bytes, depth, meter).map_err(|e| {
        let index = starts
            .partition_point(|start| start.0 <= e.offset)
            .saturating_sub(1);
        let (line, column) = starts.get(index).map_or((1, 1), |s| (s.1, s.2));
        TextError {
            line,
            column,
            kind: TextErrorKind::Cbor(e.kind),
        }
    })?;
    Ok(bytes)
}

/// Parses the one item of `text` into its heads, in order.
fn parse(text: &str, depth: usize, meter: &mut WorkMeter<'_>) -> Result<Vec<Node>, TextError> {
    let mut parser = Parser {
        cursor: Cursor {
            text,
            at: 0,
            line: 1,
            column: 1,
        },
        nodes: Vec::new(),
        open: Vec::new(),
        depth,
    };
    loop {
        parser.cursor.skip_space()?;
        meter.charge(1).map_err(|s| {
            parser
                .cursor
                .error(TextErrorKind::Cbor(CborErrorKind::Stopped(s)))
        })?;
        if parser.item()? && parser.close()? {
            return Ok(parser.nodes);
        }
    }
}

/// The parser's state: the text, the heads read so far, and the containers and tags still open.
#[derive(Debug)]
struct Parser<'t> {
    cursor: Cursor<'t>,
    nodes: Vec<Node>,
    open: Vec<Open>,
    depth: usize,
}

impl Parser<'_> {
    /// Reads the head of the item that starts here, and the whole item if it is not a non-empty
    /// container or a tag. Returns whether the item is complete.
    fn item(&mut self) -> Result<bool, TextError> {
        let (line, column) = (self.cursor.line, self.cursor.column);
        let at = |kind| start_error(line, column, kind);
        let value = match self.cursor.peek() {
            None => return Err(at(TextErrorKind::UnexpectedEnd)),
            Some('[') => {
                self.cursor.bump();
                self.cursor.skip_space()?;
                if self.cursor.eat(']') {
                    Value::Array(0)
                } else {
                    let node = self.nodes.len();
                    let open = Open::Array { node, count: 0 };
                    return self.open(line, column, Value::Array(0), open);
                }
            }
            Some('{') => {
                self.cursor.bump();
                self.cursor.skip_space()?;
                if self.cursor.eat('}') {
                    Value::Map(0)
                } else {
                    let node = self.nodes.len();
                    let open = Open::Map {
                        node,
                        count: 0,
                        has_key: false,
                    };
                    return self.open(line, column, Value::Map(0), open);
                }
            }
            Some('"') => Value::Text(string(&mut self.cursor)?),
            Some('h') => Value::Bytes(hex_bytes(&mut self.cursor)?),
            Some('t' | 'f' | 'n') => {
                if self.cursor.word("true") {
                    Value::Bool(true)
                } else if self.cursor.word("false") {
                    Value::Bool(false)
                } else if self.cursor.word("null") {
                    Value::Null
                } else {
                    return Err(at(TextErrorKind::Unexpected));
                }
            }
            Some('-' | '0'..='9') => match number(&mut self.cursor)? {
                Number::Float(x) => {
                    Value::Float(shortest(x).ok_or_else(|| at(TextErrorKind::InexactFloat))?)
                }
                Number::Int {
                    negative: false,
                    magnitude,
                } if self.cursor.eat('(') => {
                    let tag =
                        u64::try_from(magnitude).map_err(|_| at(TextErrorKind::IntegerRange))?;
                    return self.open(line, column, Value::Tag(tag), Open::Tag);
                }
                Number::Int {
                    negative: false,
                    magnitude,
                } => Value::Uint(
                    u64::try_from(magnitude).map_err(|_| at(TextErrorKind::IntegerRange))?,
                ),
                Number::Int {
                    negative: true,
                    magnitude,
                } => {
                    // −m is the CBOR negative integer −1 − n with n = m − 1, for 1 ≤ m ≤ 2^64.
                    let n = magnitude
                        .checked_sub(1)
                        .and_then(|n| u64::try_from(n).ok())
                        .ok_or_else(|| at(TextErrorKind::IntegerRange))?;
                    Value::Nint(n)
                }
            },
            Some(_) => return Err(at(TextErrorKind::Unexpected)),
        };
        self.nodes.push(Node {
            line,
            column,
            value,
        });
        Ok(true)
    }

    /// Records the head of a container or tag whose items follow, refusing it past the depth limit.
    /// Returns `false`: the item is not complete.
    fn open(
        &mut self,
        line: u32,
        column: u32,
        value: Value,
        open: Open,
    ) -> Result<bool, TextError> {
        if self.open.len() >= self.depth {
            return Err(start_error(
                line,
                column,
                TextErrorKind::Cbor(CborErrorKind::TooDeep),
            ));
        }
        self.open.push(open);
        self.nodes.push(Node {
            line,
            column,
            value,
        });
        Ok(false)
    }

    /// After an item completes, closes every container and tag it completes, and reads the
    /// separator before the next item. Returns whether the whole text's item is complete.
    fn close(&mut self) -> Result<bool, TextError> {
        loop {
            self.cursor.skip_space()?;
            let unexpected = self.cursor.error(TextErrorKind::Unexpected);
            let Some(top) = self.open.last_mut() else {
                return if self.cursor.peek().is_some() {
                    Err(self.cursor.error(TextErrorKind::Trailing))
                } else {
                    Ok(true)
                };
            };
            match top {
                Open::Tag => {
                    if !self.cursor.eat(')') {
                        return Err(unexpected);
                    }
                }
                Open::Array { node, count } => {
                    *count = count.saturating_add(1);
                    if self.cursor.eat(',') {
                        return Ok(false);
                    }
                    if !self.cursor.eat(']') {
                        return Err(unexpected);
                    }
                    let (node, count) = (*node, *count);
                    set_count(&mut self.nodes, node, Value::Array(count));
                }
                Open::Map {
                    node,
                    count,
                    has_key,
                } => {
                    if *has_key {
                        *has_key = false;
                        *count = count.saturating_add(1);
                        if self.cursor.eat(',') {
                            return Ok(false);
                        }
                        if !self.cursor.eat('}') {
                            return Err(unexpected);
                        }
                        let (node, count) = (*node, *count);
                        set_count(&mut self.nodes, node, Value::Map(count));
                    } else {
                        if !self.cursor.eat(':') {
                            return Err(unexpected);
                        }
                        *has_key = true;
                        return Ok(false);
                    }
                }
            }
            self.open.pop();
        }
    }
}

/// Sets the count of the array or map whose head is node `index`, once its bracket closes.
fn set_count(nodes: &mut [Node], index: usize, value: Value) {
    if let Some(head) = nodes.get_mut(index) {
        head.value = value;
    }
}

/// An error at a given line and column.
fn start_error(line: u32, column: u32, kind: TextErrorKind) -> TextError {
    TextError { line, column, kind }
}

/// Reads a decimal integer, or a hexadecimal float, with an optional leading `-`.
fn number(cursor: &mut Cursor<'_>) -> Result<Number, TextError> {
    let start = cursor.error(TextErrorKind::Unexpected);
    let negative = cursor.eat('-');
    if cursor.word("0x") || cursor.word("0X") {
        let x = hex_float(cursor)?;
        return Ok(Number::Float(if negative { -x } else { x }));
    }
    let mut magnitude: u128 = 0;
    let mut digits = 0_u32;
    while let Some(d) = cursor.peek().and_then(|c| c.to_digit(10)) {
        cursor.bump();
        digits = digits.saturating_add(1);
        // Above 2^64 the integer is out of range whatever follows; stop growing at 2^65.
        if magnitude <= 1 << 65 {
            magnitude = magnitude
                .checked_mul(10)
                .and_then(|m| m.checked_add(u128::from(d)))
                .ok_or_else(|| {
                    start_error(start.line, start.column, TextErrorKind::IntegerRange)
                })?;
        }
    }
    if digits == 0 {
        return Err(start);
    }
    if matches!(cursor.peek(), Some('.' | 'e' | 'E')) {
        return Err(start_error(
            start.line,
            start.column,
            TextErrorKind::InexactFloat,
        ));
    }
    if magnitude > 1 << 64 || (negative && magnitude == 0) {
        return Err(start_error(
            start.line,
            start.column,
            TextErrorKind::IntegerRange,
        ));
    }
    Ok(Number::Int {
        negative,
        magnitude,
    })
}

/// Reads the rest of a hexadecimal float after `0x`: hexadecimal digits with at most one `.`, then
/// `p`, an optional sign and a decimal exponent. The value must be one a double holds exactly.
fn hex_float(cursor: &mut Cursor<'_>) -> Result<f64, TextError> {
    let inexact = cursor.error(TextErrorKind::InexactFloat);
    let mut significand: u128 = 0;
    let mut after_point: i64 = 0;
    let mut point = false;
    let mut any = false;
    loop {
        match cursor.peek() {
            Some('.') if !point => {
                point = true;
                cursor.bump();
            }
            Some(c) if c.is_ascii_hexdigit() => {
                let d = c.to_digit(16).ok_or(inexact)?;
                cursor.bump();
                any = true;
                // 31 significant digits are 124 bits, more than any double needs.
                if significand >> 124 != 0 {
                    return Err(inexact);
                }
                significand = (significand << 4) | u128::from(d);
                if point {
                    after_point = after_point.checked_add(1).ok_or(inexact)?;
                }
            }
            _ => break,
        }
    }
    if !any || !(cursor.eat('p') || cursor.eat('P')) {
        return Err(inexact);
    }
    let negative = cursor.eat('-');
    if !negative {
        cursor.eat('+');
    }
    let mut exponent: i64 = 0;
    let mut digits = false;
    while let Some(d) = cursor.peek().and_then(|c| c.to_digit(10)) {
        cursor.bump();
        digits = true;
        exponent = exponent
            .checked_mul(10)
            .and_then(|e| e.checked_add(i64::from(d)))
            .ok_or(inexact)?;
    }
    if !digits {
        return Err(inexact);
    }
    if negative {
        exponent = exponent.checked_neg().ok_or(inexact)?;
    }
    let scale = after_point
        .checked_mul(4)
        .and_then(|bits| exponent.checked_sub(bits))
        .ok_or(inexact)?;
    exact(significand, scale).ok_or(inexact)
}

/// The double `m · 2^e`, if a double holds it exactly.
fn exact(m: u128, e: i64) -> Option<f64> {
    if m == 0 {
        return Some(0.0);
    }
    let zeros = m.trailing_zeros();
    let m = m >> zeros;
    let e = e.checked_add(i64::from(zeros))?;
    // Its leading bit's exponent: e plus the position of m's leading bit.
    let top = e.checked_add(i64::from(m.ilog2()))?;
    if m >> 53 != 0 || e < -1074 || top > 1023 {
        return None;
    }
    let m = u64::try_from(m).ok()?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "m is below 2^53, so the double holds it exactly"
    )]
    let m = m as f64;
    // m · 2^e is a double; the scaling is exact. Below 2^−1022 it is done in two steps, so each
    // power of two is a normal double.
    if e >= -1022 {
        Some(m * pow2(e)?)
    } else {
        Some(m * pow2(-1022)? * pow2(e.checked_add(1022)?)?)
    }
}

/// 2^k as a double, for −1022 ≤ k ≤ 1023.
fn pow2(k: i64) -> Option<f64> {
    let field = u64::try_from(k.checked_add(1023)?).ok()?;
    (1..=2046)
        .contains(&field)
        .then(|| f64::from_bits(field << 52))
}

/// Reads a text string in double quotes, with JSON's escapes.
fn string(cursor: &mut Cursor<'_>) -> Result<String, TextError> {
    cursor.bump();
    let mut out = String::new();
    loop {
        let here = cursor.error(TextErrorKind::Unexpected);
        match cursor.bump() {
            None => return Err(cursor.error(TextErrorKind::UnexpectedEnd)),
            Some('"') => return Ok(out),
            Some('\\') => out.push(escape(cursor)?),
            Some(c) if c < ' ' => return Err(here),
            Some(c) => out.push(c),
        }
    }
}

/// Reads one escape after `\`.
fn escape(cursor: &mut Cursor<'_>) -> Result<char, TextError> {
    let bad = cursor.error(TextErrorKind::BadEscape);
    Ok(match cursor.bump().ok_or(bad)? {
        '"' => '"',
        '\\' => '\\',
        '/' => '/',
        'b' => '\u{8}',
        'f' => '\u{c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'u' => {
            let unit = hex4(cursor).ok_or(bad)?;
            if (0xd800..0xdc00).contains(&unit) {
                if !(cursor.word("\\u")) {
                    return Err(bad);
                }
                let low = hex4(cursor).ok_or(bad)?;
                if !(0xdc00..0xe000).contains(&low) {
                    return Err(bad);
                }
                let code = (((unit & 0x3ff) << 10) | (low & 0x3ff))
                    .checked_add(0x1_0000)
                    .ok_or(bad)?;
                char::from_u32(code).ok_or(bad)?
            } else {
                char::from_u32(unit).ok_or(bad)?
            }
        }
        _ => return Err(bad),
    })
}

/// Reads four hexadecimal digits.
fn hex4(cursor: &mut Cursor<'_>) -> Option<u32> {
    let mut unit = 0_u32;
    for _ in 0..4 {
        let d = cursor.peek()?.to_digit(16)?;
        cursor.bump();
        unit = (unit << 4) | d;
    }
    Some(unit)
}

/// Reads `h'...'`: pairs of hexadecimal digits, with whitespace and comments allowed between them.
fn hex_bytes(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, TextError> {
    let start = cursor.error(TextErrorKind::Unexpected);
    if !cursor.word("h'") {
        return Err(start);
    }
    let mut out = Vec::new();
    let mut high: Option<u32> = None;
    loop {
        cursor.skip_space()?;
        let here = cursor.error(TextErrorKind::BadHex);
        match cursor.bump() {
            None => return Err(cursor.error(TextErrorKind::UnexpectedEnd)),
            Some('\'') => {
                return if high.is_some() { Err(here) } else { Ok(out) };
            }
            Some(c) => {
                let d = c.to_digit(16).ok_or(here)?;
                match high.take() {
                    None => high = Some(d),
                    Some(h) => out.push(u8::try_from((h << 4) | d).map_err(|_| here)?),
                }
            }
        }
    }
}

/// Appends a head of major type `major` with argument `argument`, in its shortest form.
fn head(out: &mut Vec<u8>, major: u8, argument: u64) {
    let initial = major << 5;
    let bytes = argument.to_be_bytes();
    // Below 24 the argument is the additional information itself.
    if let Ok(small) = u8::try_from(argument)
        && small < 24
    {
        out.push(initial | small);
        return;
    }
    let (info, length) = if argument <= 0xff {
        (24, 1)
    } else if argument <= 0xffff {
        (25, 2)
    } else if argument <= 0xffff_ffff {
        (26, 4)
    } else {
        (27, 8)
    };
    out.push(initial | info);
    out.extend_from_slice(
        bytes
            .get(8_usize.saturating_sub(length)..)
            .unwrap_or(&bytes),
    );
}

/// Appends the canonical encoding of one head and, for a string, its content.
fn write(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Uint(n) => head(out, 0, *n),
        Value::Nint(n) => head(out, 1, *n),
        Value::Bytes(b) => {
            head(out, 2, u64::try_from(b.len()).unwrap_or(u64::MAX));
            out.extend_from_slice(b);
        }
        Value::Text(t) => {
            head(out, 3, u64::try_from(t.len()).unwrap_or(u64::MAX));
            out.extend_from_slice(t.as_bytes());
        }
        Value::Array(n) => head(out, 4, *n),
        Value::Map(n) => head(out, 5, *n),
        Value::Tag(n) => head(out, 6, *n),
        Value::Float(Width::Half(h)) => {
            out.push(0xf9);
            out.extend_from_slice(&h.to_be_bytes());
        }
        Value::Float(Width::Single(bits)) => {
            out.push(0xfa);
            out.extend_from_slice(&bits.to_be_bytes());
        }
        Value::Float(Width::Double(bits)) => {
            out.push(0xfb);
            out.extend_from_slice(&bits.to_be_bytes());
        }
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Null => out.push(0xf6),
    }
}
