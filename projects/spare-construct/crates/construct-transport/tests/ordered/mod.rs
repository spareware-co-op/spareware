//! JSON read with its key order kept, for checking that typed results write keys in the
//! JavaScript's order (serde_json's Value sorts keys in this workspace).
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::project::OrderedMap;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;

/// A JSON value with object keys in file order.
#[derive(Clone, Debug, PartialEq)]
pub enum OJ {
    /// null
    Null,
    /// true or false
    Bool(bool),
    /// a number
    Num(f64),
    /// text
    Str(String),
    /// an array
    Arr(Vec<OJ>),
    /// an object, keys in order
    Obj(OrderedMap<OJ>),
}

impl<'de> Deserialize<'de> for OJ {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OJ;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<OJ, E> {
                Ok(OJ::Null)
            }
            fn visit_none<E>(self) -> Result<OJ, E> {
                Ok(OJ::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<OJ, E> {
                Ok(OJ::Bool(b))
            }
            fn visit_f64<E>(self, x: f64) -> Result<OJ, E> {
                Ok(OJ::Num(x))
            }
            fn visit_i64<E>(self, x: i64) -> Result<OJ, E> {
                Ok(OJ::Num(x as f64))
            }
            fn visit_u64<E>(self, x: u64) -> Result<OJ, E> {
                Ok(OJ::Num(x as f64))
            }
            fn visit_str<E>(self, s: &str) -> Result<OJ, E> {
                Ok(OJ::Str(s.to_string()))
            }
            fn visit_string<E>(self, s: String) -> Result<OJ, E> {
                Ok(OJ::Str(s))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<OJ, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(OJ::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<OJ, A::Error> {
                let mut m = OrderedMap::new();
                while let Some((k, v)) = a.next_entry::<String, OJ>()? {
                    m.insert(k, v);
                }
                Ok(OJ::Obj(m))
            }
        }
        d.deserialize_any(V)
    }
}

impl OJ {
    /// The same value as a serde_json Value (keys sorted), for the tolerance comparator.
    pub fn to_value(&self) -> Value {
        match self {
            OJ::Null => Value::Null,
            OJ::Bool(b) => Value::Bool(*b),
            OJ::Num(x) => construct_transport::jsnum::json_number(*x),
            OJ::Str(s) => Value::String(s.clone()),
            OJ::Arr(a) => Value::Array(a.iter().map(OJ::to_value).collect()),
            OJ::Obj(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.to_string(), v.to_value()))
                    .collect(),
            ),
        }
    }
}

/// Text of a value serialized by serde_json, read back with key order kept.
pub fn ordered_of<T: serde::Serialize>(v: &T) -> OJ {
    let text = serde_json::to_string(v).expect("serializes");
    serde_json::from_str(&text).expect("reads back")
}

/// The first place two ordered values differ (key order, keys, numbers bit for bit, text), or None when they are the same.
pub fn first_difference(a: &OJ, e: &OJ, path: &str) -> Option<String> {
    match (a, e) {
        (OJ::Obj(am), OJ::Obj(em)) => {
            let ak: Vec<&str> = am.keys().collect();
            let ek: Vec<&str> = em.keys().collect();
            if ak != ek {
                return Some(format!("{path}: keys {ak:?}, expected {ek:?}"));
            }
            for (k, ev) in em.iter() {
                if let Some(d) = first_difference(&am[k], ev, &format!("{path}.{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (OJ::Arr(aa), OJ::Arr(ea)) => {
            if aa.len() != ea.len() {
                return Some(format!(
                    "{path}: length {}, expected {}",
                    aa.len(),
                    ea.len()
                ));
            }
            for (i, (x, y)) in aa.iter().zip(ea).enumerate() {
                if let Some(d) = first_difference(x, y, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            None
        }
        (OJ::Num(x), OJ::Num(y)) if x.to_bits() == y.to_bits() || (*x == 0.0 && *y == 0.0) => None,
        _ if a == e => None,
        _ => Some(format!("{path}: {a:?}, expected {e:?}")),
    }
}
