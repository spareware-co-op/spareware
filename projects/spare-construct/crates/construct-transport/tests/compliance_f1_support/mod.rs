//! Shared by the f1 truth tests (scope linter, precinct staging, mitigation and trigger finder): the report context the cases use, the
//! normalisation of the random ids the prototype makes, and the calls every case file makes the same way (report sections and exports).
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::compliance::extra;
use construct_transport::compliance::html::ReportCtx;
use construct_transport::compliance::jv::{Jx, V};
use serde_json::Value;

/// The report lane's table block, as the generator's `REPORT_CTX` writes it.
pub struct TestCtx;

impl ReportCtx for TestCtx {
    fn table(&self, title: &str, html: &str, note: &str) -> String {
        format!("<section class=\"rt\"><h3>{title}</h3>{html}<p class=\"src\">{note}</p></section>")
    }
}

/// The arguments of a recorded case as loose JSON.
pub fn args(case: &Value) -> V {
    V::from_value(&case["args"])
}

/// A recorded case's `ok` answer, or None when the prototype threw.
pub fn ok_of(case: &Value) -> &Value {
    case.get("ok").unwrap_or_else(|| panic!("case {} has no ok answer: {}", case["id"], case))
}

/// Replaces the ids the prototype makes at random (`mit...`, `cond...`) by "<uid>", as the generator does in the recorded answers.
pub fn norm_uids(v: &mut V) {
    match v {
        V::String(s) => {
            let tail = s.strip_prefix("mit").or_else(|| s.strip_prefix("cond"));
            if tail.is_some_and(|t| t.len() >= 6 && t.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())) {
                *s = "<uid>".to_string();
            }
        }
        V::Array(a) => a.iter_mut().for_each(norm_uids),
        V::Object(m) => m.values_mut().for_each(norm_uids),
        _ => {}
    }
}

/// `REPORT[id].render(P, ctx)` of a case `{ id, ctx }`.
pub fn run_report(case: &Value, p: &V) -> V {
    let a = args(case);
    let id = a.g("id").s();
    let ctx: Option<&dyn ReportCtx> = if a.g("ctx").t() { Some(&TestCtx) } else { None };
    match extra::report(&id, p, ctx) {
        Some(h) => V::String(h),
        None => panic!("{}: no report section {id}", case["id"]),
    }
}

/// `{ enabled, text }` of the export of a case `{ id }` (the prototype's registry id is "compliance-extra-<id>").
pub fn run_export(case: &Value, p: &V) -> V {
    let id = format!("compliance-extra-{}", args(case).g("id").s());
    let enabled = extra::export_enabled(&id, p).unwrap_or_else(|| panic!("{}: no export {id}", case["id"]));
    let text = extra::export_build(&id, p).unwrap_or_else(|| panic!("{}: no export {id}", case["id"]));
    obj(vec![("enabled", V::Bool(enabled)), ("text", V::String(text))])
}

/// `{ k: v, ... }` from pairs.
pub fn obj(pairs: Vec<(&str, V)>) -> V {
    let mut o = V::object();
    for (k, v) in pairs {
        o.set(k, v);
    }
    o
}
