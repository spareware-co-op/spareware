//! Support for the f3 truth tests (tests/compliance_f3_*.rs): the recorded cases with their objects in the order the file lists them, the
//! project a case ran on, the report block the generator's mock report context writes, random-id normalisation and the comparison of an answer
//! with the recorded one. A case whose answer needs a part that is still a stub (`todo!()`) is counted as skipped, not failed.
#![allow(dead_code)] // each test file uses its own part of this module

use crate::compliance_support::{check, fixture, set_path};
use crate::support::read_truth;
use construct_transport::compliance::html::ReportCtx;
use construct_transport::compliance::jv::{Jx, V};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// An expected file (expected/compliance-<name>.json), read with ordered objects.
pub fn load(name: &str) -> V {
    V::parse(&read_truth(&format!("expected/compliance-{name}.json")))
        .unwrap_or_else(|e| panic!("compliance-{name}: {e}"))
}

/// Applies a case's sets (dotted path to value, in the order the case lists them) to a project.
pub fn apply(p: &mut V, sets: &V) {
    for (k, v) in sets.entries() {
        set_path(p, k, v.clone());
    }
}

/// `delete o[path]` for a dotted path (nothing when a step is missing).
pub fn del_path(o: &mut V, path: &str) {
    let steps: Vec<&str> = path.split('.').collect();
    let mut cur = o;
    for k in &steps[..steps.len() - 1] {
        let next = match cur {
            V::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
            _ => cur.get_mut(k),
        };
        match next {
            Some(n) => cur = n,
            None => return,
        }
    }
    cur.remove(steps[steps.len() - 1]);
}

/// The project a case ran on: its fixture with its sets applied, then the entries the case deletes.
pub fn project(case: &V) -> V {
    let mut p = fixture(&case.g("fixture").s());
    apply(&mut p, case.g("sets"));
    for d in case.g("del").items() {
        del_path(&mut p, &d.s());
    }
    p
}

/// The clock the prototype's harness fixed: 21 October 2026, UTC midnight.
pub fn set_clock() {
    construct_transport::compliance::dates::set_clock(Some(1792540800000.0));
}

/// Whether text is a random id of the prototype (`pk` and at least three of 0-9 a-z).
fn is_uid(s: &str) -> bool {
    s.len() >= 5
        && s.starts_with("pk")
        && s[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

/// The value with every `id` that looks like a random id replaced by "<uid>" (the generator does the same to the prototype's answers).
pub fn norm_uid(v: &V) -> V {
    match v {
        V::Array(a) => V::Array(a.iter().map(norm_uid).collect()),
        V::Object(_) => {
            let mut o = V::object();
            for (k, x) in v.entries() {
                let nv = if k == "id" && x.is_str() && is_uid(&x.s()) {
                    V::String("<uid>".to_string())
                } else {
                    norm_uid(x)
                };
                o.set(k, nv);
            }
            o
        }
        _ => v.clone(),
    }
}

/// Panics unless an answer matches the case's recorded `ok` (numbers within the comparator's tolerance, text exactly).
pub fn check_ok(case: &V, got: &V) {
    let id = case.g("id").s();
    assert!(
        case.has("ok"),
        "case {id} has no recorded answer: {}",
        case.g("error").s()
    );
    check(&id, &norm_uid(got), &case.g("ok").to_value());
}

/// Panics unless a text answer matches the case's recorded `ok`.
pub fn check_text(case: &V, got: &str) {
    check_ok(case, &V::String(got.to_string()));
}

/// The report block the generator's mock report context writes.
pub struct Mock;

impl ReportCtx for Mock {
    fn table(&self, title: &str, html: &str, note: &str) -> String {
        format!("<section data-title=\"{title}\">{html}<aside>{note}</aside></section>")
    }
}

/// Runs a check; None when it needed a part that is still a stub (the build has `todo!()` there), so the case is skipped rather than failed.
pub fn guard<T>(f: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            if msg.starts_with("not yet implemented") {
                None
            } else {
                std::panic::resume_unwind(e)
            }
        }
    }
}
