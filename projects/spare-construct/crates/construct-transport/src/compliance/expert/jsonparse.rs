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
    let mut p = Parser {
        src: text.encode_utf16().collect(),
        pos: 0,
    };
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
        Some(0x2d) | Some(0x30..=0x39) => Tok::Num,
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
    match c? {
        d @ 0x30..=0x39 => Some(d - 0x30),
        d @ 0x61..=0x66 => Some(d - 0x61 + 10),
        d @ 0x41..=0x46 => Some(d - 0x41 + 10),
        _ => None,
    }
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

    fn skip_ws(&mut self) {
        while matches!(
            self.cur(),
            Some(0x20) | Some(0x09) | Some(0x0a) | Some(0x0d)
        ) {
            self.pos += 1;
        }
    }

    /// The line and column of the cursor (a `\r\n` counts as one line break).
    fn line_col(&self) -> (usize, usize) {
        let end = self.pos;
        let (mut line, mut last_break, mut i) = (1usize, 0usize, 0usize);
        while i < end {
            if self.src[i] == 0x0d && i + 1 < end && self.src[i + 1] == 0x0a {
                i += 1;
            }
            if self.src[i] == 0x0d || self.src[i] == 0x0a {
                line += 1;
                last_break = i + 1;
            }
            i += 1;
        }
        (line, 1 + (i - last_break))
    }

    /// `<what> in JSON at position P (line L column C)`.
    fn at(&self, what: &str) -> String {
        let (line, col) = self.line_col();
        format!(
            "{what} in JSON at position {} (line {line} column {col})",
            self.pos
        )
    }

    /// `Unexpected non-whitespace character after JSON at position P (line L column C)`.
    fn after_json(&self) -> String {
        let (line, col) = self.line_col();
        format!(
            "Unexpected non-whitespace character after JSON at position {} (line {line} column {col})",
            self.pos
        )
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
                let ch = text_of(&self.src[self.pos..self.pos + 1]);
                if n < MIN_FOR_CONTEXT {
                    format!("Unexpected token '{ch}', \"{whole}\" is not valid JSON")
                } else if self.pos < CONTEXT {
                    format!(
                        "Unexpected token '{ch}', \"{}\"... is not valid JSON",
                        text_of(&self.src[..self.pos + CONTEXT])
                    )
                } else if self.pos < n - CONTEXT {
                    format!(
                        "Unexpected token '{ch}', ...\"{}\"... is not valid JSON",
                        text_of(&self.src[self.pos - CONTEXT..self.pos + CONTEXT])
                    )
                } else {
                    format!(
                        "Unexpected token '{ch}', ...\"{}\" is not valid JSON",
                        text_of(&self.src[self.pos - CONTEXT..])
                    )
                }
            }
        }
    }

    /// Skips white space and takes the token if it is the next one.
    fn check(&mut self, tok: Tok) -> bool {
        self.skip_ws();
        if self.peek() == tok {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Skips white space and takes the token, or fails with the message.
    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), String> {
        if self.check(tok) {
            Ok(())
        } else {
            Err(self.at(what))
        }
    }

    /// A property name and the colon after it.
    fn key(&mut self, what: &str) -> Result<String, String> {
        self.skip_ws();
        if self.peek() != Tok::Str {
            return Err(self.at(what));
        }
        self.pos += 1;
        let k = self.string()?;
        self.expect(Tok::Colon, "Expected ':' after property name")?;
        Ok(k)
    }

    /// The text of a string, the cursor just after its opening quote.
    fn string(&mut self) -> Result<String, String> {
        let mut buf: Vec<u16> = Vec::new();
        loop {
            let Some(c) = self.cur() else {
                return Err(self.at("Unterminated string"));
            };
            match c {
                0x22 => {
                    self.pos += 1;
                    return Ok(text_of(&buf));
                }
                0x5c => {
                    self.pos += 1;
                    let Some(e) = self.cur() else {
                        return Err(self.unexpected(Tok::Eos));
                    };
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
                            for _ in 0..4 {
                                self.pos += 1;
                                let Some(h) = hex_value(self.cur()) else {
                                    return Err(self.at("Bad Unicode escape"));
                                };
                                v = v * 16 + h;
                            }
                            buf.push(v);
                        }
                        _ => return Err(self.at("Bad escaped character")),
                    }
                    self.pos += 1;
                }
                c if c < 0x20 => return Err(self.at("Bad control character in string literal")),
                c => {
                    buf.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// A number, the cursor on its first character.
    fn number(&mut self) -> Result<V, String> {
        let start = self.pos;
        if self.cur() == Some(0x2d) {
            self.pos += 1;
        }
        if self.cur() == Some(0x30) {
            self.pos += 1;
            if is_digit(self.cur()) {
                return Err(self.unexpected(Tok::Num));
            }
        } else {
            if !is_digit(self.cur()) {
                return Err(self.at("No number after minus sign"));
            }
            while is_digit(self.cur()) {
                self.pos += 1;
            }
        }
        if self.cur() == Some(0x2e) {
            self.pos += 1;
            if !is_digit(self.cur()) {
                return Err(self.at("Unterminated fractional number"));
            }
            while is_digit(self.cur()) {
                self.pos += 1;
            }
        }
        if matches!(self.cur(), Some(0x65) | Some(0x45)) {
            self.pos += 1;
            if matches!(self.cur(), Some(0x2b) | Some(0x2d)) {
                self.pos += 1;
            }
            if !is_digit(self.cur()) {
                return Err(self.at("Exponent part is missing a number"));
            }
            while is_digit(self.cur()) {
                self.pos += 1;
            }
        }
        let text = text_of(&self.src[start..self.pos]);
        Ok(V::Number(text.parse::<f64>().unwrap_or(f64::NAN)))
    }

    /// `true`, `false` or `null`, the cursor on its first letter.
    fn literal(&mut self, word: &str) -> Result<(), String> {
        let w: Vec<u16> = word.encode_utf16().collect();
        let remaining = self.src.len() - self.pos;
        if remaining >= w.len() && self.src[self.pos + 1..self.pos + w.len()] == w[1..] {
            self.pos += w.len();
            return Ok(());
        }
        self.pos += 1;
        for i in 0..(w.len() - 1).min(remaining - 1) {
            if w[1 + i] != self.src[self.pos] {
                return Err(self.unexpected(self.peek()));
            }
            self.pos += 1;
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
                    self.pos += 1;
                    V::String(self.string()?)
                }
                Tok::Num => self.number()?,
                Tok::LBrace => {
                    self.pos += 1;
                    if self.check(Tok::RBrace) {
                        V::object()
                    } else {
                        let key = self.key("Expected property name or '}'")?;
                        stack.push(Frame::Obj {
                            map: V::object(),
                            key,
                        });
                        continue;
                    }
                }
                Tok::LBrack => {
                    self.pos += 1;
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
                Tok::Colon | Tok::Comma | Tok::Illegal | Tok::RBrace | Tok::RBrack | Tok::Eos => {
                    return Err(self.unexpected(tok));
                }
            };
            // the value is done: add it to what holds it, and read the next member or close
            loop {
                let Some(frame) = stack.pop() else {
                    return Ok(value);
                };
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
        assert_eq!(
            parse("{").unwrap_err(),
            "Expected property name or '}' in JSON at position 1 (line 1 column 2)"
        );
        assert_eq!(
            parse("[1,]").unwrap_err(),
            "Unexpected token ']', \"[1,]\" is not valid JSON"
        );
        assert_eq!(
            parse("{\"a\":1}\n\n x").unwrap_err(),
            "Unexpected non-whitespace character after JSON at position 10 (line 3 column 2)"
        );
        assert_eq!(
            parse("{\"settings\":{\"growth\":1.5,\"hvPct\":}}").unwrap_err(),
            "Unexpected token '}', ...\"5,\"hvPct\":}}\" is not valid JSON"
        );
        assert_eq!(
            parse("[1").unwrap_err(),
            "Expected ',' or ']' after array element in JSON at position 2 (line 1 column 3)"
        );
        assert_eq!(
            parse("[\"abc\\").unwrap_err(),
            "Unexpected end of JSON input"
        );
    }
}
