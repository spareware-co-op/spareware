//! Small JavaScript readings of loose values that the options, Fast-track, speed and ANOP parts share (they belong to no other part).

use crate::compliance::jv::{Jx, V};
use crate::jsnum::to_int32;

/// `${o[k]}` of a field that may be missing: a missing field reads "undefined", as a template string prints it.
pub fn fld_s(o: &V, k: &str) -> String {
    if o.has(k) { o.g(k).s() } else { "undefined".to_string() }
}

/// `+o[k]` of a field that may be missing: a missing field reads NaN (null reads 0).
pub fn numk(o: &V, k: &str) -> f64 {
    if o.has(k) { o.g(k).n() } else { f64::NAN }
}

/// `+x || 0`: the number a value reads as, 0 when that is NaN.
pub fn n0(v: &V) -> f64 {
    let x = v.n();
    if x.is_nan() { 0.0 } else { x }
}

/// `x | 0` of a loose value.
pub fn int32(v: &V) -> f64 {
    to_int32(v.n()) as f64
}

/// `a || b`: the first value when it is truthy, else the second.
pub fn or_v(a: &V, b: &V) -> V {
    if a.t() { a.clone() } else { b.clone() }
}

/// `a || 'text'`.
pub fn or_s(a: &V, b: &str) -> V {
    if a.t() { a.clone() } else { V::String(b.to_string()) }
}

/// `o[k] = v` only when the value exists (a JavaScript `undefined` leaves the field out of the JSON).
pub fn put_some(o: &mut V, k: &str, v: Option<&V>) {
    if let Some(v) = v {
        o.set(k, v.clone());
    }
}

/// `o[k] = v` where a missing source field stays missing: copies the field of `src` when it exists.
pub fn copy_field(o: &mut V, k: &str, src: &V, from: &str) {
    if let Some(v) = src.get(from) {
        o.set(k, v.clone());
    }
}

/// `+v` as a number, or None for NaN (null and missing read as 0 first, as `+null` does).
pub fn num_or_none(v: &V) -> Option<f64> {
    let x = v.n();
    if x.is_nan() { None } else { Some(x) }
}

/// A number as a loose value, null for None.
pub fn opt_num(x: Option<f64>) -> V {
    x.map_or(V::Null, V::Number)
}

/// The numbers of `Object.keys(obj)` order for a table lookup: the text of a loose key (`obj[key]`).
pub fn key_of(v: &V) -> String {
    v.s()
}

/// `[a, b, ...]` as the cells of one CSV row.
macro_rules! cells {
    ($($v:expr),* $(,)?) => {
        vec![$($crate::compliance::jv::ToV::to_v($v)),*]
    };
}
pub(crate) use cells;

/// A label of the Compliance+ rules (`STD.labels[k]`).
pub fn label(k: &str) -> String {
    crate::compliance::data::x(&format!("STD.labels.{k}")).s()
}

/// A text of the Compliance+ tables by dotted path.
pub fn xs(path: &str) -> String {
    crate::compliance::data::x(path).s()
}

/// A table of the report sections: `Kit.table({ head, rows, empty })` with the default compact layout.
pub fn tbl(head: &[&str], rows: Vec<Vec<String>>, empty: &str) -> String {
    let h: Vec<crate::compliance::html::Th> = head.iter().map(|x| crate::compliance::html::th(x)).collect();
    tbl_th(&h, rows, empty)
}

/// A table whose headings may be numeric columns.
pub fn tbl_th(head: &[crate::compliance::html::Th], rows: Vec<Vec<String>>, empty: &str) -> String {
    let r: Vec<crate::compliance::html::Tr> = rows.into_iter().map(crate::compliance::html::tr).collect();
    crate::compliance::html::kit_table(head, &r, true, "", empty)
}

/// `[...new Set(items)]`: the items with repeats left out, first occurrences kept in order.
pub fn unique(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in items {
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

/// `o[k] = v` for anything that becomes a loose value (the loose value's own `set` takes values only).
pub trait Put {
    /// Sets a field (a new field goes last; an existing field keeps its place).
    fn put(&mut self, k: &str, v: impl crate::compliance::jv::ToV);
}
impl Put for V {
    fn put(&mut self, k: &str, v: impl crate::compliance::jv::ToV) {
        self.set(k, v.to_v());
    }
}
