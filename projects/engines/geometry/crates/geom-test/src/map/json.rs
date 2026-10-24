//! Writing JSON deterministically: one text per value, the same bytes on every target.
//!
//! What it does: [`Value`] is a JSON value whose objects are ordered maps; [`Value::to_text`]
//! writes it as JSON text indented by two spaces, with `\n` line ends and a final newline.
//!
//! Guarantees:
//!
//! - **One text per value.** Object keys are written in byte order (a `BTreeMap`), arrays in their
//!   own order, and nothing depends on the platform, a clock or a hash seed, so the same map is
//!   the same bytes on every target.
//! - **Valid JSON.** Strings are escaped as RFC 8259 section 7 requires: `"` and `\` with a
//!   backslash, control characters below U+0020 as `\b`, `\f`, `\n`, `\r`, `\t` or `\u00XX`; every
//!   other character is written as its UTF-8 bytes, as section 8.1 asks of JSON exchanged between
//!   systems. The tests read every text back with `serde_json`.
//! - **No floating point.** Numbers are unsigned integers, so no rounding or formatting choice can
//!   differ between targets.
//!
//! Method: the JSON grammar of RFC 8259 (<https://www.rfc-editor.org/rfc/rfc8259>), written by
//! recursive descent over the value; recursion is as deep as the value is nested, which the map
//! builds at most six levels deep.
//!
//! Cost: linear in the length of the text written.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A non-negative integer.
    Number(u64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object, its keys in byte order.
    Object(BTreeMap<String, Value>),
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl From<Option<String>> for Value {
    fn from(s: Option<String>) -> Value {
        s.map_or(Value::Null, Value::String)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<Vec<String>> for Value {
    fn from(items: Vec<String>) -> Value {
        Value::Array(items.into_iter().map(Value::String).collect())
    }
}

/// An object from `(key, value)` pairs; a later pair replaces an earlier one with the same key.
#[must_use]
pub fn object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// Writes `s` as a JSON string.
fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                // Writing to a String cannot fail.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A new line and `depth` levels of indentation.
fn newline(out: &mut String, depth: usize) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str("  ");
    }
}

impl Value {
    /// The value as JSON text, indented by two spaces, ending in a newline.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let inner = depth.saturating_add(1);
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => string(out, s),
            Value::Array(items) if items.is_empty() => out.push_str("[]"),
            Value::Array(items) => {
                out.push('[');
                for (k, item) in items.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    newline(out, inner);
                    item.write(out, inner);
                }
                newline(out, depth);
                out.push(']');
            }
            Value::Object(map) if map.is_empty() => out.push_str("{}"),
            Value::Object(map) => {
                out.push('{');
                for (k, (key, value)) in map.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    newline(out, inner);
                    string(out, key);
                    out.push_str(": ");
                    value.write(out, inner);
                }
                newline(out, depth);
                out.push('}');
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn writes_sorted_indented_valid_json() {
        let v = object([
            ("zeta", Value::Number(3)),
            ("alpha", Value::from("a \"q\" \\ \n\t\u{1} − ε")),
            (
                "list",
                Value::Array(vec![Value::Null, Value::Bool(true), Value::Array(vec![])]),
            ),
            ("empty", object([])),
        ]);
        let text = v.to_text();
        assert_eq!(
            text,
            "{\n  \"alpha\": \"a \\\"q\\\" \\\\ \\n\\t\\u0001 − ε\",\n  \"empty\": {},\n  \
             \"list\": [\n    null,\n    true,\n    []\n  ],\n  \"zeta\": 3\n}\n"
        );
        let back: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back["alpha"], "a \"q\" \\ \n\t\u{1} − ε");
        assert_eq!(back["zeta"], 3);
    }
}
