//! Reading loose project JSON the way the prototype's JavaScript does (`a.b.c`, `x || ''`, `+x`, `String(x ?? '')`).

use crate::edit::json::JsonValue;
use crate::jsnum::number_to_js_string;

/// `v[k]` of an object (None for any other value or a missing key).
pub(crate) fn get<'a>(v: Option<&'a JsonValue>, k: &str) -> Option<&'a JsonValue> {
    v.and_then(|v| match v {
        JsonValue::Object(m) => m.get(k),
        _ => None,
    })
}

/// `v.a.b.c` along keys; None when any step is missing.
pub(crate) fn path<'a>(v: Option<&'a JsonValue>, ks: &[&str]) -> Option<&'a JsonValue> {
    let mut cur = v;
    for k in ks {
        cur = get(cur, k);
    }
    cur
}

/// `v[i]` of an array.
pub(crate) fn idx(v: Option<&JsonValue>, i: usize) -> Option<&JsonValue> {
    match v {
        Some(JsonValue::Array(a)) => a.get(i),
        _ => None,
    }
}

/// The items of an array (empty for anything else).
pub(crate) fn items(v: Option<&JsonValue>) -> &[JsonValue] {
    match v {
        Some(JsonValue::Array(a)) => a,
        _ => &[],
    }
}

/// The entries of an object in order (empty for anything else).
pub(crate) fn entries(v: Option<&JsonValue>) -> Vec<(&str, &JsonValue)> {
    match v {
        Some(JsonValue::Object(m)) => m.iter().collect(),
        _ => Vec::new(),
    }
}

/// `!!v`.
pub(crate) fn truthy(v: Option<&JsonValue>) -> bool {
    v.is_some_and(JsonValue::truthy)
}

/// `+v`: NaN for a missing value.
pub(crate) fn num(v: Option<&JsonValue>) -> f64 {
    v.map_or(f64::NAN, JsonValue::to_number)
}

/// `String(v ?? '')`: the text an escaped cell prints for a loose value (null and missing print nothing).
pub(crate) fn text(v: Option<&JsonValue>) -> String {
    match v {
        None | Some(JsonValue::Null) => String::new(),
        Some(x) => x.js_string(),
    }
}

/// `v || ''` as text: a falsy value prints nothing.
pub(crate) fn text_or_empty(v: Option<&JsonValue>) -> String {
    match v {
        Some(x) if x.truthy() => x.js_string(),
        _ => String::new(),
    }
}

/// The text of a value that is text; None for anything else.
pub(crate) fn as_str(v: Option<&JsonValue>) -> Option<&str> {
    v.and_then(JsonValue::as_str)
}

/// A number from a loose value as JavaScript prints it (`String(+v)`), for cells that coerce.
pub(crate) fn num_text(v: Option<&JsonValue>) -> String {
    number_to_js_string(num(v))
}

/// Whether the value is an object (not an array).
pub(crate) fn is_obj(v: Option<&JsonValue>) -> bool {
    v.is_some_and(JsonValue::is_object)
}

/// Whether the value is an array.
pub(crate) fn is_arr(v: Option<&JsonValue>) -> bool {
    v.is_some_and(JsonValue::is_array)
}
