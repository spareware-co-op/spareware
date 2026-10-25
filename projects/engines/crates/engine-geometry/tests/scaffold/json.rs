//! A small, strict JSON reader for `cargo metadata`'s output, and the writer the merge gate's
//! seeded `cargo deny` runs use to hand cargo-deny a changed copy of that output.
//!
//! What it does: parses one JSON text (RFC 8259) into a tree of values, and writes a tree back as
//! JSON text (`Json::to_text`).
//!
//! What it guarantees: it accepts exactly RFC 8259's grammar (no comments, no trailing commas, no
//! leading zeros, no unescaped control characters, no lone surrogates), refuses an object that
//! repeats a key, and returns an error with the byte offset instead of panicking. Nesting deeper
//! than `MAX_DEPTH` is an error (the outermost value is depth 1, so `MAX_DEPTH` nested arrays are
//! read and one more is refused), so recursion is bounded (principle 13). Numbers are kept as their
//! text, because the check never computes with them. The writer writes what the reader reads back
//! to the same tree: numbers as their text, strings with `"`, `\` and the control characters
//! escaped (RFC 8259 section 7), and it refuses a tree nested deeper than `MAX_DEPTH`.
//!
//! Method: recursive descent over RFC 8259's grammar
//! (<https://www.rfc-editor.org/rfc/rfc8259>), written here so the check adds no dependency to
//! `engine-geometry` or to `Cargo.lock` (chapter 29.1: the kernel's only registry crate is `libm`).
//!
//! Cost: linear in the length of the text; recursion at most `MAX_DEPTH` deep.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: its
// readers count positions in a text they hold whole, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

use std::collections::BTreeMap;

/// The deepest nesting accepted. `cargo metadata` nests about six deep.
pub const MAX_DEPTH: usize = 128;

/// A JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as written.
    Number(String),
    /// A string, unescaped.
    Str(String),
    /// An array.
    Array(Vec<Json>),
    /// An object; its keys are unique.
    Object(BTreeMap<String, Json>),
}

/// Why a text is not JSON, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonError {
    /// The byte offset of the problem.
    pub offset: usize,
    /// What is wrong.
    pub what: &'static str,
}

impl Json {
    /// The member `key` of an object, if this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members.get(key),
            _ => None,
        }
    }

    /// The text of a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    /// The items of an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The members of an object.
    pub fn as_object(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Object(members) => Some(members),
            _ => None,
        }
    }

    /// The value of a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The member `key` of an object, to change, if this is an object that has it.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(members) => members.get_mut(key),
            _ => None,
        }
    }

    /// The items of an array, to change.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The value as JSON text, which `parse` reads back to an equal value.
    ///
    /// # Panics
    ///
    /// If the value nests deeper than `MAX_DEPTH`, which `parse` would refuse.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 1);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        assert!(
            depth <= MAX_DEPTH,
            "a JSON value nests deeper than MAX_DEPTH"
        );
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Number(text) => out.push_str(text),
            Json::Str(text) => write_string(out, text),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out, depth + 1);
                }
                out.push(']');
            }
            Json::Object(members) => {
                out.push('{');
                for (i, (key, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, key);
                    out.push(':');
                    value.write(out, depth + 1);
                }
                out.push('}');
            }
        }
    }
}

/// Writes `text` as a JSON string: `"` and `\` escaped, and every control character below U+0020
/// as `\u00XX` (RFC 8259 section 7); everything else as it is, in UTF-8.
fn write_string(out: &mut String, text: &str) {
    use std::fmt::Write as _;
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            // Writing to a String cannot fail.
            c if u32::from(c) < 0x20 => write!(out, "\\u{:04x}", u32::from(c)).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses one JSON text, with only whitespace around it.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = parser.value(1)?;
    parser.whitespace();
    if parser.at == parser.bytes.len() {
        Ok(value)
    } else {
        Err(parser.error("text after the value"))
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &'static str) -> JsonError {
        JsonError {
            offset: self.at,
            what,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &'static str, value: Json) -> Result<Json, JsonError> {
        let end = self.at + word.len();
        if self.bytes.get(self.at..end) == Some(word.as_bytes()) {
            self.at = end;
            Ok(value)
        } else {
            Err(self.error("unknown literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting deeper than MAX_DEPTH"));
        }
        self.whitespace();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("not the start of a value")),
            None => Err(self.error("end of text where a value was expected")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1; // '{'
        let mut members = BTreeMap::new();
        self.whitespace();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("an object key must be a string"));
            }
            let key_at = self.at;
            let key = self.string()?;
            self.whitespace();
            if self.next() != Some(b':') {
                return Err(self.error("expected ':' after an object key"));
            }
            let value = self.value(depth + 1)?;
            if members.insert(key, value).is_some() {
                return Err(JsonError {
                    offset: key_at,
                    what: "an object repeats a key",
                });
            }
            self.whitespace();
            match self.next() {
                Some(b',') => {}
                Some(b'}') => return Ok(Json::Object(members)),
                _ => return Err(self.error("expected ',' or '}' in an object")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1; // '['
        let mut items = Vec::new();
        self.whitespace();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.whitespace();
            match self.next() {
                Some(b',') => {}
                Some(b']') => return Ok(Json::Array(items)),
                _ => return Err(self.error("expected ',' or ']' in an array")),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.at += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error("a number has a leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error("a number needs a digit")),
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if self.digits() == 0 {
                return Err(self.error("a fraction needs a digit"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(self.error("an exponent needs a digit"));
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| self.error("a number is not ASCII"))?;
        Ok(Json::Number(text.to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.next() {
                Some(b @ b'0'..=b'9') => b - b'0',
                Some(b @ b'a'..=b'f') => b - b'a' + 10,
                Some(b @ b'A'..=b'F') => b - b'A' + 10,
                _ => return Err(self.error("\\u needs four hexadecimal digits")),
            };
            value = value * 16 + u32::from(digit);
        }
        Ok(value)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.at += 1; // '"'
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(byte) = self.next() else {
                return Err(self.error("a string is not closed"));
            };
            match byte {
                b'"' => break,
                b'\\' => {
                    let escaped = match self.next() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => self.unicode_escape()?,
                        _ => return Err(self.error("unknown escape")),
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(escaped.encode_utf8(&mut buffer).as_bytes());
                }
                0x00..=0x1f => return Err(self.error("a control character inside a string")),
                _ => out.push(byte),
            }
        }
        // The input was a &str and every escape wrote whole UTF-8 sequences, so this holds.
        String::from_utf8(out).map_err(|_| self.error("a string is not UTF-8"))
    }

    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let first = self.hex4()?;
        let code = match first {
            0xD800..=0xDBFF => {
                if self.next() != Some(b'\\') || self.next() != Some(b'u') {
                    return Err(self.error("a high surrogate without its low surrogate"));
                }
                let second = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&second) {
                    return Err(self.error("a high surrogate without its low surrogate"));
                }
                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err(self.error("a low surrogate on its own")),
            _ => first,
        };
        char::from_u32(code).ok_or_else(|| self.error("not a Unicode scalar value"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_cargo_metadata_writes() {
        let text = r#" {"packages": [{"name": "geom-num", "path": "C:\\Users\\x", "optional": false,
            "kind": null, "deps": [], "version": 1, "e": -1.5e+3}],
            "root": "\u00e9\ud83d\ude00"} "#;
        let value = parse(text).unwrap();
        let package = &value.get("packages").unwrap().as_array().unwrap()[0];
        assert_eq!(package.get("name").unwrap().as_str(), Some("geom-num"));
        assert_eq!(package.get("path").unwrap().as_str(), Some("C:\\Users\\x"));
        assert_eq!(package.get("optional").unwrap().as_bool(), Some(false));
        assert_eq!(package.get("kind"), Some(&Json::Null));
        assert_eq!(package.get("e"), Some(&Json::Number("-1.5e+3".to_owned())));
        assert_eq!(value.get("root").unwrap().as_str(), Some("é😀"));
    }

    #[test]
    fn refuses_what_is_not_json() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "01",
            "1.",
            "1e",
            "-",
            "\"\\x\"",
            "\"\\ud800\"",
            "\"\\udc00\"",
            "\"a\nb\"",
            "{\"a\":1,\"a\":2}",
            "{a:1}",
            "nul",
            "[] []",
            "// comment\n1",
        ] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let text = r#" {"packages": [{"name": "geom-num", "path": "C:\\Users\\x", "optional": false,
            "kind": null, "deps": [], "version": 1, "e": -1.5e+3, "q": "a \"b\" \t\u0001"}],
            "root": "\u00e9\ud83d\ude00", "empty": {}} "#;
        let value = parse(text).unwrap();
        let written = value.to_text();
        assert_eq!(parse(&written).unwrap(), value, "{written}");
        // Escapes and numbers exactly as RFC 8259 writes them; keys in order.
        assert_eq!(
            Json::Str("\"\\\u{1}é".to_owned()).to_text(),
            "\"\\\"\\\\\\u0001é\""
        );
        assert_eq!(parse("[1.50,-0,2e-7]").unwrap().to_text(), "[1.50,-0,2e-7]");
        let mut changed = value.clone();
        changed
            .get_mut("packages")
            .and_then(Json::as_array_mut)
            .unwrap()
            .push(Json::Null);
        assert_ne!(parse(&changed.to_text()).unwrap(), value);
    }

    #[test]
    fn nesting_is_bounded() {
        // The bound itself and one past it, in arrays, objects and a mix.
        let arrays = |n: usize| "[".repeat(n) + &"]".repeat(n);
        let objects = |n: usize| "{\"a\":".repeat(n - 1) + "{}" + &"}".repeat(n - 1);
        let mixed = |n: usize| "[{\"a\":".repeat(n / 2) + "[]" + &"}]".repeat(n / 2);
        for text in [arrays(MAX_DEPTH), objects(MAX_DEPTH), mixed(MAX_DEPTH - 1)] {
            assert!(
                parse(&text).is_ok(),
                "refused a text nested at most {MAX_DEPTH} deep"
            );
        }
        for text in [
            arrays(MAX_DEPTH + 1),
            objects(MAX_DEPTH + 1),
            mixed(MAX_DEPTH + 1),
        ] {
            assert_eq!(
                parse(&text).unwrap_err().what,
                "nesting deeper than MAX_DEPTH"
            );
        }
    }
}
