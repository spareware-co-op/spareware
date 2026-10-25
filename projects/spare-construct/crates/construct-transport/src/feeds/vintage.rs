//! Where every number came from (`P.feeds.vintage`): each source records its query date, address, licence and record count.
//!
//! `stamp` writes one entry, `fail_stamp` one for a download that failed, and [`vintage_rows`] gives the table the Traffic data
//! tab and the About the data section show (as text, without the page's HTML).

use super::nztime::date_text;
use super::sources::src;
use crate::edit::collate::locale_compare_f64;
use crate::edit::json::JsonValue;
use crate::edit::ids::iso_string;
use crate::graph::js_sort_by;
use crate::jsnum::{value_to_js_string, value_to_number, value_truthy};
use crate::project::Project;
use crate::util::fmt;
use serde::Serialize;
use serde_json::{json, Map, Value};

/// `F()`: the project's feeds record, made when it is missing, with its `vintage` table.
pub fn feeds_of(p: &mut Project) -> &mut Map<String, Value> {
    let falsy = |v: Option<&Value>| v.is_none_or(|v| !value_truthy(v));
    if falsy(p.extra.get("feeds")) {
        p.extra.insert("feeds".to_string(), json!({}));
    }
    let f = p.extra.get_mut("feeds").and_then(Value::as_object_mut).expect("feeds is an object");
    if falsy(f.get("vintage")) {
        f.insert("vintage".to_string(), json!({}));
    }
    f
}

/// `stamp(id, extra)`: records that a source was queried now: its name, address, page, licence and the time, with whatever `extra` adds (n for the record count, note, error, or a different `key`, `url` or `licence`). Gives the entry.
pub fn stamp(p: &mut Project, id: &str, extra: &Map<String, Value>, now_ms: f64) -> Value {
    let s = src(id);
    let truthy = |k: &str| extra.get(k).filter(|v| value_truthy(v)).cloned();
    let mut v = Map::new();
    if let Some(s) = s {
        v.insert("source".to_string(), json!(s.name));
    }
    match truthy("url") {
        Some(u) => {
            v.insert("url".to_string(), u);
        }
        None => {
            if let Some(s) = s {
                v.insert("url".to_string(), json!(s.url));
            }
        }
    }
    if let Some(page) = s.and_then(|s| s.page) {
        v.insert("page".to_string(), json!(page));
    }
    match truthy("licence") {
        Some(l) => {
            v.insert("licence".to_string(), l);
        }
        None => {
            if let Some(s) = s {
                v.insert("licence".to_string(), json!(s.licence));
            }
        }
    }
    v.insert("at".to_string(), json!(iso_string(now_ms)));
    for (k, x) in extra {
        v.insert(k.clone(), x.clone());
    }
    let key = match truthy("key") {
        Some(k) => value_to_js_string(&k),
        None => id.to_string(),
    };
    let entry = Value::Object(v);
    feeds_of(p).get_mut("vintage").and_then(Value::as_object_mut).expect("vintage").insert(key, entry.clone());
    entry
}

/// `failStamp(id, e, extra)`: records that a download failed, with the error text and no records.
pub fn fail_stamp(p: &mut Project, id: &str, error: &str, extra: &Map<String, Value>, now_ms: f64) -> Value {
    let mut x = extra.clone();
    x.insert("error".to_string(), json!(error));
    x.insert("n".to_string(), json!(0));
    stamp(p, id, &x, now_ms)
}

/// `safeHref(h)`: the text when it is a web address or a plain path on this site, else "" (a project file could carry `javascript:` in a URL field).
pub fn safe_href(h: &str) -> String {
    let s = crate::jsnum::js_trim(h);
    let web = super::jsregex::Regex::lit(r#"^https?:\/\/[^\s"'<>]+$"#, "i");
    let path = super::jsregex::Regex::lit(r"^[\w][\w./-]*$", "");
    if web.test(s) || path.test(s) { s.to_string() } else { String::new() }
}

/// One row of the data vintage table.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VintageRow {
    /// The source's name.
    pub source: String,
    /// The address the source's name links to (empty when there is none that is safe to link).
    pub href: String,
    /// The host shown for the link.
    pub host: String,
    /// The query date as "9 Oct 2026".
    pub queried: String,
    /// The licence.
    pub licence: String,
    /// The number of records ("-" for a failed download).
    pub records: String,
    /// The note, or the error for a failed download.
    pub notes: String,
    /// The download failed.
    pub error: bool,
}

/// `vintageRows()`: the entries of `P.feeds.vintage` as table rows, sorted by the source's name.
pub fn vintage_rows(p: &Project) -> Vec<VintageRow> {
    let Some(Value::Object(v)) = p.extra.get("feeds").and_then(|f| f.get("vintage")) else { return Vec::new() };
    let get = |e: &Value, k: &str| e.get(k).cloned();
    let text = |x: Option<Value>| match x {
        Some(x) if value_truthy(&x) => value_to_js_string(&x),
        _ => String::new(),
    };
    let mut entries: Vec<&Value> = v.values().collect();
    js_sort_by(&mut entries, |a, b| {
        let s = |e: &Value| e.get("source").map_or("undefined".to_string(), value_to_js_string);
        locale_compare_f64(&s(a), &s(b))
    });
    entries
        .into_iter()
        .map(|e| {
            let page_or_url = {
                let p = text(get(e, "page"));
                if p.is_empty() { text(get(e, "url")) } else { p }
            };
            let host_src = {
                let u = text(get(e, "url"));
                if u.is_empty() { text(get(e, "page")) } else { u }
            };
            let err = get(e, "error").is_some_and(|x| value_truthy(&x));
            let n = get(e, "n").filter(value_truthy).map_or(0.0, |x| value_to_number(&x));
            VintageRow {
                source: text(get(e, "source")),
                href: safe_href(&page_or_url),
                host: super::flow::host_text(&host_src),
                queried: date_text(get(e, "at").map(|x| JsonValue::from_value(&x)).as_ref()),
                licence: text(get(e, "licence")),
                records: if err { fmt::DASH.to_string() } else { fmt::int(n) },
                notes: if err { text(get(e, "error")) } else { text(get(e, "note")) },
                error: err,
            }
        })
        .collect()
}
