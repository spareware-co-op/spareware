//! Loose JSON as the prototype's JavaScript handles it: the compliance rules read a project, its results and the user's
//! entries as plain objects whose fields may be missing, null, text or numbers, so they work on [`JsonValue`] (objects keep
//! their key order) and use these helpers for the JavaScript readings of a value.

use crate::edit::json::JsonValue;
use crate::jsnum::{is_js_space, number_to_js_string};

/// A loose JSON value (objects keep the order the file lists them in).
pub type V = JsonValue;

/// A null to hand out for a missing field (`undefined` and null read alike).
pub static NULL: V = JsonValue::Null;

/// JavaScript's ways of reading a loose value.
pub trait Jx {
    /// `x[k]`: the field of an object, null when the field or the object is missing.
    fn g(&self, k: &str) -> &V;
    /// `x[i]`: the item of an array, null when missing.
    fn ix(&self, i: usize) -> &V;
    /// `k in x` for an object (an own field, even if null).
    fn has(&self, k: &str) -> bool;
    /// The items of an array (empty for anything else).
    fn items(&self) -> &[V];
    /// `x.length` for an array (0 for anything else).
    fn jlen(&self) -> usize;
    /// `Object.entries(x)`: the fields of an object in JavaScript order (empty for anything else).
    fn entries(&self) -> Vec<(&str, &V)>;
    /// `Object.keys(x)`.
    fn keys(&self) -> Vec<&str>;
    /// `Object.values(x)`.
    fn vals(&self) -> Vec<&V>;
    /// `+x` (null reads as 0, text as a number or NaN).
    fn n(&self) -> f64;
    /// `String(x)`.
    fn s(&self) -> String;
    /// `x == null ? '' : String(x)` (what `esc` and the table cells print).
    fn sn(&self) -> String;
    /// `!!x`.
    fn t(&self) -> bool;
    /// `x == null`.
    fn nullish(&self) -> bool;
    /// `isObj(x)`: a plain object (not an array, not null).
    fn is_obj(&self) -> bool;
    /// `typeof x === 'string'`.
    fn is_str(&self) -> bool;
    /// `typeof x === 'number'`.
    fn is_num(&self) -> bool;
    /// `Array.isArray(x)`.
    fn is_arr(&self) -> bool;
    /// `x === 'text'`.
    fn eq_s(&self, s: &str) -> bool;
    /// `isFinite(x)` (the number the value reads as is finite).
    fn finite(&self) -> bool;
    /// `x != null && x !== '' && isFinite(+x)`: a number the user entered.
    fn entered(&self) -> bool;
}

impl Jx for V {
    fn g(&self, k: &str) -> &V {
        self.get(k).unwrap_or(&NULL)
    }
    fn ix(&self, i: usize) -> &V {
        self.as_array().and_then(|a| a.get(i)).unwrap_or(&NULL)
    }
    fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }
    fn items(&self) -> &[V] {
        self.as_array().map_or(&[], |a| a.as_slice())
    }
    fn jlen(&self) -> usize {
        self.as_array().map_or(0, |a| a.len())
    }
    fn entries(&self) -> Vec<(&str, &V)> {
        self.as_object()
            .map_or_else(Vec::new, |m| m.iter().collect())
    }
    fn keys(&self) -> Vec<&str> {
        self.as_object()
            .map_or_else(Vec::new, |m| m.keys().collect())
    }
    fn vals(&self) -> Vec<&V> {
        self.as_object()
            .map_or_else(Vec::new, |m| m.values().collect())
    }
    fn n(&self) -> f64 {
        self.to_number()
    }
    fn s(&self) -> String {
        self.js_string()
    }
    fn sn(&self) -> String {
        if self.is_null() {
            String::new()
        } else {
            self.js_string()
        }
    }
    fn t(&self) -> bool {
        self.truthy()
    }
    fn nullish(&self) -> bool {
        self.is_null()
    }
    fn is_obj(&self) -> bool {
        self.is_object()
    }
    fn is_str(&self) -> bool {
        matches!(self, JsonValue::String(_))
    }
    fn is_num(&self) -> bool {
        matches!(self, JsonValue::Number(_))
    }
    fn is_arr(&self) -> bool {
        self.is_array()
    }
    fn eq_s(&self, s: &str) -> bool {
        self.as_str() == Some(s)
    }
    fn finite(&self) -> bool {
        self.n().is_finite()
    }
    fn entered(&self) -> bool {
        !self.is_null() && !self.eq_s("") && self.n().is_finite()
    }
}

/// Anything that becomes a JSON value in an object built with [`jobj!`].
pub trait ToV {
    /// The value as JSON.
    fn to_v(self) -> V;
}
impl ToV for V {
    fn to_v(self) -> V {
        self
    }
}
impl ToV for &V {
    fn to_v(self) -> V {
        self.clone()
    }
}
impl ToV for f64 {
    fn to_v(self) -> V {
        V::Number(self)
    }
}
impl ToV for i32 {
    fn to_v(self) -> V {
        V::Number(self as f64)
    }
}
impl ToV for i64 {
    fn to_v(self) -> V {
        V::Number(self as f64)
    }
}
impl ToV for u32 {
    fn to_v(self) -> V {
        V::Number(self as f64)
    }
}
impl ToV for usize {
    fn to_v(self) -> V {
        V::Number(self as f64)
    }
}
impl ToV for bool {
    fn to_v(self) -> V {
        V::Bool(self)
    }
}
impl ToV for &str {
    fn to_v(self) -> V {
        V::String(self.to_string())
    }
}
impl ToV for String {
    fn to_v(self) -> V {
        V::String(self)
    }
}
impl ToV for &String {
    fn to_v(self) -> V {
        V::String(self.clone())
    }
}
impl<T: ToV> ToV for Option<T> {
    fn to_v(self) -> V {
        self.map_or(V::Null, ToV::to_v)
    }
}
impl<T: ToV> ToV for Vec<T> {
    fn to_v(self) -> V {
        V::Array(self.into_iter().map(ToV::to_v).collect())
    }
}

/// `{ k: v, ... }`: an object whose fields keep the order given.
macro_rules! jobj {
    ($($k:expr => $v:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut o = $crate::compliance::jv::V::object();
        $( o.set($k, $crate::compliance::jv::ToV::to_v($v)); )*
        o
    }};
}
pub(crate) use jobj;

/// `[a, b, ...]` from anything that becomes values.
macro_rules! jarr {
    ($($v:expr),* $(,)?) => {
        $crate::compliance::jv::V::Array(vec![$($crate::compliance::jv::ToV::to_v($v)),*])
    };
}
pub(crate) use jarr;

/// A number value.
pub fn f(x: f64) -> V {
    V::Number(x)
}

/// A text value.
pub fn st(s: impl Into<String>) -> V {
    V::String(s.into())
}

/// An array value from values.
pub fn arr(items: impl IntoIterator<Item = V>) -> V {
    V::Array(items.into_iter().collect())
}

/// An array of text values.
pub fn strs<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> V {
    V::Array(
        items
            .into_iter()
            .map(|s| V::String(s.as_ref().to_string()))
            .collect(),
    )
}

/// `{ ...a, ...b }`: the fields of a, then those of b (a field in both keeps a's place and takes b's value).
pub fn merge(a: &V, b: &V) -> V {
    let mut o = if a.is_object() {
        a.clone()
    } else {
        V::object()
    };
    for (k, v) in b.entries() {
        o.set(k, v.clone());
    }
    o
}

/// `x[k] = v` on an object (nothing for another type).
pub fn set(o: &mut V, k: &str, v: impl ToV) {
    o.set(k, v.to_v());
}

/// The field of an object for changing, made an empty object first when missing or not an object-like value.
pub fn obj_at<'a>(o: &'a mut V, k: &str) -> &'a mut V {
    let ok = o.get(k).is_some_and(|x| x.is_object());
    if !ok {
        o.set(k, V::object());
    }
    o.get_mut(k).expect("field set above")
}

/// The field of an object for changing, made an empty array first when missing or not an array.
pub fn arr_at<'a>(o: &'a mut V, k: &str) -> &'a mut Vec<V> {
    let ok = o.get(k).is_some_and(|x| x.is_array());
    if !ok {
        o.set(k, V::Array(Vec::new()));
    }
    o.get_mut(k)
        .expect("field set above")
        .as_array_mut()
        .expect("array set above")
}

/// A dotted path read from a value (`getPath`): null when any step is missing.
pub fn path<'a>(v: &'a V, p: &str) -> &'a V {
    let mut cur = v;
    for k in p.split('.') {
        cur = match cur {
            V::Array(a) => k
                .parse::<usize>()
                .ok()
                .and_then(|i| a.get(i))
                .unwrap_or(&NULL),
            _ => cur.g(k),
        };
    }
    cur
}

/// `String.prototype.trim` (JavaScript's whitespace).
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `n.toString()` for a number (JavaScript's formatting).
pub fn ns(x: f64) -> String {
    number_to_js_string(x)
}

/// `s.slice(a, b)` by character (negative bounds count from the end); b None is the end.
pub fn slice(s: &str, a: i64, b: Option<i64>) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len() as i64;
    let fix = |x: i64| if x < 0 { (n + x).max(0) } else { x.min(n) };
    let (lo, hi) = (fix(a), fix(b.unwrap_or(n)));
    if lo >= hi {
        String::new()
    } else {
        c[lo as usize..hi as usize].iter().collect()
    }
}

/// `s.padStart(n, '0')`.
pub fn pad_start(s: &str, n: usize, with: char) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        std::iter::repeat_n(with, n - len).collect::<String>() + s
    }
}

/// `s.toLowerCase()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `s.toUpperCase()`.
pub fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// `arr.includes(x)` for text.
pub fn has_str(a: &[&str], x: &str) -> bool {
    a.contains(&x)
}

/// `U.csvCell(v)`: one RFC 4180 cell. Text that a spreadsheet would read as a formula gets a leading apostrophe; null is an empty
/// cell; numbers (a number that is not finite prints as NaN or Infinity), booleans, arrays and objects are written as `String(v)`.
pub fn csv_cell(v: &V) -> String {
    let mut s = if v.is_null() {
        String::new()
    } else {
        v.js_string()
    };
    if let V::String(_) = v {
        let first = s.chars().next();
        if matches!(first, Some('=' | '+' | '-' | '@' | '\t' | '\r'))
            && !super::rx::rx(r"^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$", "i").test(trim(&s))
        {
            s.insert(0, '\'');
        }
    }
    if s.contains(['"', ',', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// `U.csv(rows)`: cells joined with commas, rows with CRLF.
pub fn csv(rows: &[Vec<V>]) -> String {
    rows.iter()
        .map(|r| r.iter().map(csv_cell).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// `U.parseCSV(text)`: rows of cells, quotes and doubled quotes read, rows that are blank left out.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let t: Vec<char> = text.chars().collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let (mut row, mut cur, mut q) = (Vec::<String>::new(), String::new(), false);
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        if q {
            if c == '"' && t.get(i + 1) == Some(&'"') {
                cur.push('"');
                i += 1;
            } else if c == '"' {
                q = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            q = true;
        } else if c == ',' {
            row.push(std::mem::take(&mut cur));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && t.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            row.push(std::mem::take(&mut cur));
            rows.push(std::mem::take(&mut row));
        } else {
            cur.push(c);
        }
        i += 1;
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows.into_iter()
        .filter(|r| r.iter().any(|v| !trim(v).is_empty()))
        .collect()
}
