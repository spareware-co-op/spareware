//! `JSON.parse` the way V8 reads and refuses text, for the other party's untrusted file (`parseOther`): the same values (objects keep
//! their key order, numbers are doubles, a repeated key keeps its first place and its last value) and the same error messages
//! (`Expected ',' or '}' after property value in JSON at position 6 (line 1 column 7)`, `Unexpected token '}', "{"a":}" is not valid JSON`).
//!
//! The text is read as UTF-16 units, as V8 does, so positions count units. A lone surrogate (an escape such as `\ud800`, or one cut
//! out of a longer text for a message) cannot be held in a Rust string and reads as U+FFFD.

use crate::compliance::jv::V;

/// The kind of the next token (V8's `JsonToken`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Eos,
    Str,
    Num,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    Colon,
    Comma,
    True,
    False,
    Null,
    Illegal,
}

/// A value being built while its members are read.
enum Frame {
    Obj { map: V, key: String },
    Arr { items: Vec<V> },
}

struct Parser {
    src: Vec<u16>,
    pos: usize,
}

/// Messages that name an offending character carry the context V8 shows: up to this many units each side of it.
const CONTEXT: usize = 10;
/// A text this long or longer gets context in an "Unexpected token" message; a shorter one is shown whole.
const MIN_FOR_CONTEXT: usize = CONTEXT * 2 + 1;

/// `JSON.parse(text)`: the value, or the message V8's error carries.
pub fn parse(text: &str) -> Result<V, String> {
    let mut p = Parser { src: text.encode_utf16().collect(), pos: 0 };
    let v = p.value()?;
    p.skip_ws();
    if p.peek() != Tok::Eos {
        return Err(p.after_json());
    }
    Ok(v)
}

fn tok_of(c: Option<u16>) -> Tok {
    match c {
        None => Tok::Eos,
        Some(0x22) => Tok::Str,
        Some(0x2d | 0x30..=0x39) => Tok::Num,
        Some(0x7b) => Tok::LBrace,
        Some(0x7d) => Tok::RBrace,
        Some(0x5b) => Tok::LBrack,
        Some(0x5d) => Tok::RBrack,
        Some(0x3a) => Tok::Colon,
        Some(0x2c) => Tok::Comma,
        Some(0x74) => Tok::True,
        Some(0x66) => Tok::False,
        Some(0x6e) => Tok::Null,
        _ => Tok::Illegal,
    }
}

fn is_digit(c: Option<u16>) -> bool {
    matches!(c, Some(0x30..=0x39))
}

fn hex_value(c: Option<u16>) -> Option<u16> {
    // to_digit(16) takes 0-9, a-f and A-F, as V8's hex reader does
    let d = char::from_u32(u32::from(c?))?.to_digit(16)?;
    u16::try_from(d).ok()
}

/// A run of UTF-16 units as text (a lone surrogate reads as U+FFFD).
fn text_of(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

impl Parser {
    fn cur(&self) -> Option<u16> {
        self.src.get(self.pos).copied()
    }

    fn peek(&self) -> Tok {
        tok_of(self.cur())
    }

    /// Moves the cursor on one unit. Bound: the cursor stays at or below the text's length, far below `usize::MAX`.
    fn bump(&mut self) {
        self.pos = self.pos.saturating_add(1);
    }

    /// The units from `from` up to `to` as text (empty if the range falls outside the text, which the callers never ask for).
    fn units(&self, from: usize, to: usize) -> String {
        text_of(self.src.get(from..to).unwrap_or_default())
    }

    fn skip_ws(&mut self) {
        while matches!(self.cur(), Some(0x20 | 0x09 | 0x0a | 0x0d)) {
            self.bump();
        }
    }

    /// The line and column of the cursor (a `\r\n` counts as one line break).
    fn line_col(&self) -> (usize, usize) {
        let end = self.pos;
        let (mut line, mut last_break, mut i) = (1usize, 0usize, 0usize);
        // Bound: the cursor is at most the text's length, so i and the counts stay far below usize::MAX
        while i < end {
            let next = i.saturating_add(1);
            if self.src.get(i).copied() == Some(0x0d) && next < end && self.src.get(next).copied() == Some(0x0a) {
                i = next;
            }
            if matches!(self.src.get(i).copied(), Some(0x0d | 0x0a)) {
                line = line.saturating_add(1);
                last_break = i.saturating_add(1);
            }
            i = i.saturating_add(1);
        }
        (line, i.saturating_sub(last_break).saturating_add(1))
    }

    /// `<what> in JSON at position P (line L column C)`.
    fn at(&self, what: &str) -> String {
        let (line, col) = self.line_col();
        format!("{what} in JSON at position {} (line {line} column {col})", self.pos)
    }

    /// `Unexpected non-whitespace character after JSON at position P (line L column C)`.
    fn after_json(&self) -> String {
        let (line, col) = self.line_col();
        format!("Unexpected non-whitespace character after JSON at position {} (line {line} column {col})", self.pos)
    }

    /// The message for an unexpected token at the cursor.
    fn unexpected(&self, tok: Tok) -> String {
        match tok {
            Tok::Eos => "Unexpected end of JSON input".to_string(),
            Tok::Num => self.at("Unexpected number"),
            Tok::Str => self.at("Unexpected string"),
            _ => {
                let whole = text_of(&self.src);
                if matches!(whole.as_str(), "NaN" | "Infinity" | "undefined") {
                    return format!("\"{whole}\" is not valid JSON");
                }
                let n = self.src.len();
                // The cursor is on the offending unit, so pos < n; n is at least MIN_FOR_CONTEXT in the branches with context
                let ch = self.units(self.pos, self.pos.saturating_add(1));
                if n < MIN_FOR_CONTEXT {
                    format!("Unexpected token '{ch}', \"{whole}\" is not valid JSON")
                } else if self.pos < CONTEXT {
                    format!("Unexpected token '{ch}', \"{}\"... is not valid JSON", self.units(0, self.pos.saturating_add(CONTEXT)))
                } else if self.pos < n.saturating_sub(CONTEXT) {
                    format!("Unexpected token '{ch}', ...\"{}\"... is not valid JSON", self.units(self.pos.saturating_sub(CONTEXT), self.pos.saturating_add(CONTEXT)))
                } else {
                    format!("Unexpected token '{ch}', ...\"{}\" is not valid JSON", self.units(self.pos.saturating_sub(CONTEXT), n))
                }
            }
        }
    }

    /// Skips white space and takes the token if it is the next one.
    fn check(&mut self, tok: Tok) -> bool {
        self.skip_ws();
        if self.peek() == tok {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Skips white space and takes the token, or fails with the message.
    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), String> {
        if self.check(tok) { Ok(()) } else { Err(self.at(what)) }
    }

    /// A property name and the colon after it.
    fn key(&mut self, what: &str) -> Result<String, String> {
        self.skip_ws();
        if self.peek() != Tok::Str {
            return Err(self.at(what));
        }
        self.bump();
        let k = self.string()?;
        self.expect(Tok::Colon, "Expected ':' after property name")?;
        Ok(k)
    }

    /// The text of a string, the cursor just after its opening quote.
    fn string(&mut self) -> Result<String, String> {
        let mut buf: Vec<u16> = Vec::new();
        loop {
            let Some(c) = self.cur() else { return Err(self.at("Unterminated string")) };
            match c {
                0x22 => {
                    self.bump();
                    return Ok(text_of(&buf));
                }
                0x5c => {
                    self.bump();
                    let Some(e) = self.cur() else { return Err(self.unexpected(Tok::Eos)) };
                    if e > 0xff {
                        return Err(self.unexpected(Tok::Illegal));
                    }
                    match e {
                        0x22 | 0x5c | 0x2f => buf.push(e),
                        0x62 => buf.push(8),
                        0x66 => buf.push(12),
                        0x6e => buf.push(10),
                        0x72 => buf.push(13),
                        0x74 => buf.push(9),
                        0x75 => {
                            let mut v: u16 = 0;
                            // Bound: four hex digits make at most 0xffff, which u16 holds; the saturating steps never saturate
                            for _ in 0..4 {
                                self.bump();
                                let Some(h) = hex_value(self.cur()) else { return Err(self.at("Bad Unicode escape")) };
                                v = v.saturating_mul(16).saturating_add(h);
                            }
                            buf.push(v);
                        }
                        _ => return Err(self.at("Bad escaped character")),
                    }
                    self.bump();
                }
                c if c < 0x20 => return Err(self.at("Bad control character in string literal")),
                c => {
                    buf.push(c);
                    self.bump();
                }
            }
        }
    }

    /// A number, the cursor on its first character.
    fn number(&mut self) -> Result<V, String> {
        let start = self.pos;
        if self.cur() == Some(0x2d) {
            self.bump();
        }
        if self.cur() == Some(0x30) {
            self.bump();
            if is_digit(self.cur()) {
                return Err(self.unexpected(Tok::Num));
            }
        } else {
            if !is_digit(self.cur()) {
                return Err(self.at("No number after minus sign"));
            }
            while is_digit(self.cur()) {
                self.bump();
            }
        }
        if self.cur() == Some(0x2e) {
            self.bump();
            if !is_digit(self.cur()) {
                return Err(self.at("Unterminated fractional number"));
            }
            while is_digit(self.cur()) {
                self.bump();
            }
        }
        if matches!(self.cur(), Some(0x65 | 0x45)) {
            self.bump();
            if matches!(self.cur(), Some(0x2b | 0x2d)) {
                self.bump();
            }
            if !is_digit(self.cur()) {
                return Err(self.at("Exponent part is missing a number"));
            }
            while is_digit(self.cur()) {
                self.bump();
            }
        }
        let text = self.units(start, self.pos);
        Ok(V::Number(text.parse::<f64>().unwrap_or(f64::NAN)))
    }

    /// `true`, `false` or `null`, the cursor on its first letter.
    fn literal(&mut self, word: &str) -> Result<(), String> {
        let w: Vec<u16> = word.encode_utf16().collect();
        // The cursor is on the word's first letter, so remaining is at least 1
        let remaining = self.src.len().saturating_sub(self.pos);
        let rest = w.get(1..).unwrap_or_default();
        if remaining >= w.len() && self.src.get(self.pos.saturating_add(1)..self.pos.saturating_add(w.len())) == Some(rest) {
            self.pos = self.pos.saturating_add(w.len());
            return Ok(());
        }
        self.bump();
        for &want in rest.iter().take(remaining.saturating_sub(1)) {
            if self.cur() != Some(want) {
                return Err(self.unexpected(self.peek()));
            }
            self.bump();
        }
        Err(self.unexpected(Tok::Eos))
    }

    /// One value with everything inside it (objects and arrays are read without recursion).
    fn value(&mut self) -> Result<V, String> {
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            // produce a value
            self.skip_ws();
            let tok = self.peek();
            let mut value = match tok {
                Tok::Str => {
                    self.bump();
                    V::String(self.string()?)
                }
                Tok::Num => self.number()?,
                Tok::LBrace => {
                    self.bump();
                    if self.check(Tok::RBrace) {
                        V::object()
                    } else {
                        let key = self.key("Expected property name or '}'")?;
                        stack.push(Frame::Obj { map: V::object(), key });
                        continue;
                    }
                }
                Tok::LBrack => {
                    self.bump();
                    if self.check(Tok::RBrack) {
                        V::Array(Vec::new())
                    } else {
                        stack.push(Frame::Arr { items: Vec::new() });
                        continue;
                    }
                }
                Tok::True => {
                    self.literal("true")?;
                    V::Bool(true)
                }
                Tok::False => {
                    self.literal("false")?;
                    V::Bool(false)
                }
                Tok::Null => {
                    self.literal("null")?;
                    V::Null
                }
                Tok::Colon | Tok::Comma | Tok::Illegal | Tok::RBrace | Tok::RBrack | Tok::Eos => return Err(self.unexpected(tok)),
            };
            // the value is done: add it to what holds it, and read the next member or close
            loop {
                let Some(frame) = stack.pop() else { return Ok(value) };
                match frame {
                    Frame::Obj { mut map, key } => {
                        map.set(key, value);
                        if self.check(Tok::Comma) {
                            let key = self.key("Expected double-quoted property name")?;
                            stack.push(Frame::Obj { map, key });
                            break;
                        }
                        self.expect(Tok::RBrace, "Expected ',' or '}' after property value")?;
                        value = map;
                    }
                    Frame::Arr { mut items } => {
                        items.push(value);
                        if self.check(Tok::Comma) {
                            stack.push(Frame::Arr { items });
                            break;
                        }
                        self.expect(Tok::RBrack, "Expected ',' or ']' after array element")?;
                        value = V::Array(items);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_what_json_parse_reads() {
        let v = parse(r#" {"b":1,"a":[true,null,-0.5e1,"xé"],"b":2,"1":3} "#).unwrap();
        assert_eq!(v.stringify(), r#"{"1":3,"b":2,"a":[true,null,-5,"xé"]}"#);
    }

    #[test]
    fn refuses_with_the_message_v8_gives() {
        assert_eq!(parse("{").unwrap_err(), "Expected property name or '}' in JSON at position 1 (line 1 column 2)");
        assert_eq!(parse("[1,]").unwrap_err(), "Unexpected token ']', \"[1,]\" is not valid JSON");
        assert_eq!(parse("{\"a\":1}\n\n x").unwrap_err(), "Unexpected non-whitespace character after JSON at position 10 (line 3 column 2)");
        assert_eq!(parse("{\"settings\":{\"growth\":1.5,\"hvPct\":}}").unwrap_err(), "Unexpected token '}', ...\"5,\"hvPct\":}}\" is not valid JSON");
        assert_eq!(parse("[1").unwrap_err(), "Expected ',' or ']' after array element in JSON at position 2 (line 1 column 3)");
        assert_eq!(parse("[\"abc\\").unwrap_err(), "Unexpected end of JSON input");
    }
}
