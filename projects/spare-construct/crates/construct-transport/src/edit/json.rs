//! Plain JSON as JavaScript holds it: objects keep their key order, numbers are doubles.
//!
//! `serde_json::Value` sorts object keys, which loses the order a saved project lists its
//! nodes, links and rates in. [`JsonValue`] keeps the order, reads and writes numbers the way
//! `JSON.parse` and `JSON.stringify` do ([`stringify_pretty`](JsonValue::stringify_pretty) with an
//! indent of one is `JSON.stringify(x, null, 1)`), and gives the JavaScript readings of a value
//! (`!!x`, `+x`, `String(x)`). Objects list whole-number keys (array indices) first in
//! ascending order, as a JavaScript object does. The project upgrade ([`super::migrate`]) works on it.

use crate::jsnum::{number_to_js_string, value_to_js_string, value_to_number};
use crate::project::OrderedMap;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;

/// A JSON value with ordered objects.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    /// null.
    Null,
    /// true or false.
    Bool(bool),
    /// A number (JavaScript's only number type).
    Number(f64),
    /// Text.
    String(String),
    /// An array.
    Array(Vec<JsonValue>),
    /// An object, keys in JavaScript order.
    Object(OrderedMap<JsonValue>),
}

/// True for a key JavaScript treats as an array index: "0", or digits without a leading zero, below 2^32 - 1.
fn is_array_index(k: &str) -> bool {
    if k.is_empty() || k.len() > 10 || !k.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if k.len() > 1 && k.starts_with('0') {
        return false;
    }
    k.parse::<u64>().is_ok_and(|n| n < 4_294_967_295)
}

impl JsonValue {
    /// An empty object.
    pub fn object() -> JsonValue {
        JsonValue::Object(OrderedMap::new())
    }
    /// An object from (key, value) pairs in the given order (a repeated key keeps the last value).
    pub fn from_pairs<K: Into<String>>(
        pairs: impl IntoIterator<Item = (K, JsonValue)>,
    ) -> JsonValue {
        let mut o = JsonValue::object();
        for (k, v) in pairs {
            o.set(k, v);
        }
        o
    }
    /// Reads JSON text (`JSON.parse`).
    pub fn parse(text: &str) -> Result<JsonValue, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
    /// True for an object.
    pub fn is_object(&self) -> bool {
        matches!(self, JsonValue::Object(_))
    }
    /// True for an array.
    pub fn is_array(&self) -> bool {
        matches!(self, JsonValue::Array(_))
    }
    /// True for null.
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
    /// `typeof x === 'object' && x !== null` (arrays count).
    pub fn is_object_like(&self) -> bool {
        matches!(self, JsonValue::Object(_) | JsonValue::Array(_))
    }
    /// The object's map, if an object.
    pub fn as_object(&self) -> Option<&OrderedMap<JsonValue>> {
        match self {
            JsonValue::Object(m) => Some(m),
            _ => None,
        }
    }
    /// The object's map for changing, if an object.
    pub fn as_object_mut(&mut self) -> Option<&mut OrderedMap<JsonValue>> {
        match self {
            JsonValue::Object(m) => Some(m),
            _ => None,
        }
    }
    /// The array, if an array.
    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }
    /// The array for changing, if an array.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<JsonValue>> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }
    /// The text, if text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }
    /// The number, if a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(x) => Some(*x),
            _ => None,
        }
    }
    /// `obj[key]` of an object (None for a missing key or another type).
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object().and_then(|m| m.get(key))
    }
    /// `obj[key]` of an object, for changing.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut JsonValue> {
        self.as_object_mut().and_then(|m| m.get_mut(key))
    }
    /// `obj[key] = value`: a new key goes last (whole-number keys go among the leading whole-number keys), an
    /// existing key keeps its place. Does nothing when this is not an object.
    pub fn set(&mut self, key: impl Into<String>, value: JsonValue) {
        let key = key.into();
        let JsonValue::Object(m) = self else {
            return;
        };
        if m.contains_key(&key) || !is_array_index(&key) {
            m.insert(key, value);
            return;
        }
        // A new array-index key: rebuild with it in sorted place among the leading index keys
        let n: u64 = key.parse().expect("array index");
        let old = std::mem::take(m);
        let mut placed = false;
        let mut value = Some(value);
        for (k, v) in old {
            if !placed && (!is_array_index(&k) || k.parse::<u64>().expect("array index") > n) {
                m.insert(key.clone(), value.take().expect("value"));
                placed = true;
            }
            m.insert(k, v);
        }
        if !placed {
            m.insert(key, value.take().expect("value"));
        }
    }
    /// `delete obj[key]`: returns the value.
    pub fn remove(&mut self, key: &str) -> Option<JsonValue> {
        self.as_object_mut().and_then(|m| m.remove(key))
    }
    /// `!!x`.
    pub fn truthy(&self) -> bool {
        match self {
            JsonValue::Null => false,
            JsonValue::Bool(b) => *b,
            JsonValue::Number(x) => crate::jsnum::truthy_number(*x),
            JsonValue::String(s) => !s.is_empty(),
            _ => true,
        }
    }
    /// `+x`.
    pub fn to_number(&self) -> f64 {
        match self {
            JsonValue::Number(x) => *x,
            other => value_to_number(&other.to_value()),
        }
    }
    /// `String(x)`.
    pub fn js_string(&self) -> String {
        match self {
            JsonValue::Number(x) => number_to_js_string(*x),
            JsonValue::String(s) => s.clone(),
            other => value_to_js_string(&other.to_value()),
        }
    }
    /// `x === y` for two values (objects and arrays are never identical).
    pub fn strict_eq(&self, other: &JsonValue) -> bool {
        match (self, other) {
            (JsonValue::Null, JsonValue::Null) => true,
            (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
            (JsonValue::Number(a), JsonValue::Number(b)) => a == b,
            (JsonValue::String(a), JsonValue::String(b)) => a == b,
            _ => false,
        }
    }
    /// As a `serde_json::Value` (object keys come out sorted).
    pub fn to_value(&self) -> Value {
        match self {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(b) => Value::Bool(*b),
            JsonValue::Number(x) => crate::jsnum::json_number(*x),
            JsonValue::String(s) => Value::String(s.clone()),
            JsonValue::Array(a) => Value::Array(a.iter().map(JsonValue::to_value).collect()),
            JsonValue::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.to_string(), v.to_value()))
                    .collect(),
            ),
        }
    }
    /// From a `serde_json::Value` (objects keep the order the value lists them in, which is sorted).
    pub fn from_value(v: &Value) -> JsonValue {
        match v {
            Value::Null => JsonValue::Null,
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::Number(n) => JsonValue::Number(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(s) => JsonValue::String(s.clone()),
            Value::Array(a) => JsonValue::Array(a.iter().map(JsonValue::from_value).collect()),
            Value::Object(m) => {
                JsonValue::from_pairs(m.iter().map(|(k, x)| (k.clone(), JsonValue::from_value(x))))
            }
        }
    }
    /// `JSON.stringify(x)`.
    pub fn stringify(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, None, 0);
        s
    }
    /// `JSON.stringify(x, null, indent)` with an indent of that many spaces (1 to 10).
    pub fn stringify_pretty(&self, indent: usize) -> String {
        let mut s = String::new();
        self.write(&mut s, Some(indent.min(10)), 0);
        s
    }
    fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        match self {
            JsonValue::Null => out.push_str("null"),
            JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JsonValue::Number(x) => {
                if x.is_finite() {
                    out.push_str(&number_to_js_string(*x));
                } else {
                    out.push_str("null");
                }
            }
            JsonValue::String(s) => quote(out, s),
            JsonValue::Array(a) => {
                if a.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, indent, depth + 1);
                    v.write(out, indent, depth + 1);
                }
                newline(out, indent, depth);
                out.push(']');
            }
            JsonValue::Object(m) => {
                if m.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, indent, depth + 1);
                    quote(out, k);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    v.write(out, indent, depth + 1);
                }
                newline(out, indent, depth);
                out.push('}');
            }
        }
    }
}

fn newline(out: &mut String, indent: Option<usize>, depth: usize) {
    if let Some(n) = indent {
        out.push('\n');
        for _ in 0..n * depth {
            out.push(' ');
        }
    }
}

/// JSON.stringify's quoting of text.
fn quote(out: &mut String, s: &str) {
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
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<f64> for JsonValue {
    fn from(x: f64) -> Self {
        JsonValue::Number(x)
    }
}
impl From<bool> for JsonValue {
    fn from(b: bool) -> Self {
        JsonValue::Bool(b)
    }
}
impl From<&str> for JsonValue {
    fn from(s: &str) -> Self {
        JsonValue::String(s.to_string())
    }
}
impl From<String> for JsonValue {
    fn from(s: String) -> Self {
        JsonValue::String(s)
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = JsonValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_bool<E>(self, b: bool) -> Result<JsonValue, E> {
                Ok(JsonValue::Bool(b))
            }
            fn visit_i64<E>(self, x: i64) -> Result<JsonValue, E> {
                Ok(JsonValue::Number(x as f64))
            }
            fn visit_u64<E>(self, x: u64) -> Result<JsonValue, E> {
                Ok(JsonValue::Number(x as f64))
            }
            fn visit_f64<E>(self, x: f64) -> Result<JsonValue, E> {
                Ok(JsonValue::Number(x))
            }
            fn visit_str<E>(self, s: &str) -> Result<JsonValue, E> {
                Ok(JsonValue::String(s.to_string()))
            }
            fn visit_string<E>(self, s: String) -> Result<JsonValue, E> {
                Ok(JsonValue::String(s))
            }
            fn visit_unit<E>(self) -> Result<JsonValue, E> {
                Ok(JsonValue::Null)
            }
            fn visit_none<E>(self) -> Result<JsonValue, E> {
                Ok(JsonValue::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<JsonValue, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element::<JsonValue>()? {
                    v.push(x);
                }
                Ok(JsonValue::Array(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<JsonValue, A::Error> {
                let mut o = JsonValue::object();
                while let Some((k, v)) = a.next_entry::<String, JsonValue>()? {
                    o.set(k, v);
                }
                Ok(o)
            }
        }
        d.deserialize_any(V)
    }
}

/// `U.deepMerge(base, over)`: objects merge key by key (the base's keys first, in its order, then new keys in the
/// over object's order); anything else in `over` replaces the base (arrays are replaced, not merged).
pub fn deep_merge(base: &JsonValue, over: &JsonValue) -> JsonValue {
    let JsonValue::Object(om) = over else {
        return over.clone();
    };
    // `{ ...base }`: the base's own keys (a primitive or an array has none a merge uses)
    let mut out = if base.is_object() {
        base.clone()
    } else {
        JsonValue::object()
    };
    for (k, v) in om.iter() {
        let merged = match base.get(k) {
            Some(b @ JsonValue::Object(_)) => deep_merge(b, v),
            _ => v.clone(),
        };
        out.set(k, merged);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_writes_like_json_stringify() {
        let v = JsonValue::parse(r#"{"b":1,"a":[1,2.5,{"z":null}],"2":3,"1":4,"s":"x\ny\u0001"}"#)
            .unwrap();
        assert_eq!(
            v.stringify(),
            r#"{"1":4,"2":3,"b":1,"a":[1,2.5,{"z":null}],"s":"x\ny\u0001"}"#
        );
        assert_eq!(JsonValue::parse("{}").unwrap().stringify_pretty(1), "{}");
        let p = JsonValue::parse(r#"{"a":[],"b":{"c":1}}"#)
            .unwrap()
            .stringify_pretty(1);
        assert_eq!(p, "{\n \"a\": [],\n \"b\": {\n  \"c\": 1\n }\n}");
    }

    #[test]
    fn merges_like_deep_merge() {
        let b = JsonValue::parse(r#"{"a":1,"o":{"x":1,"y":2},"arr":[1]}"#).unwrap();
        let o = JsonValue::parse(r#"{"o":{"y":9,"z":3},"arr":[2,3],"n":null}"#).unwrap();
        assert_eq!(
            deep_merge(&b, &o).stringify(),
            r#"{"a":1,"o":{"x":1,"y":9,"z":3},"arr":[2,3],"n":null}"#
        );
    }
}
