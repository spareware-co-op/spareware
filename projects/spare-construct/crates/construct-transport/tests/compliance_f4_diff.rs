//! The assumption diff against the JavaScript's recorded answers (expected/compliance-f4-diff.json) and the JSON reader of the
//! other party's file against what V8's `JSON.parse` made of a corpus of good and bad texts (expected/compliance-f4-jsonerr.json), both
//! from tools/transport-truth/gen-compliance-f4.mjs.

mod compliance_f4_support;
mod compliance_support;
mod support;

use compliance_f4_support::{check_error, check_ok, load, project, set_clock};
use construct_transport::compliance::expert;
use construct_transport::compliance::jv::{Jx, V};

/// `String(text || '')` of a recorded text (the case holds null for undefined, 0 or null).
fn text_of(v: &V) -> String {
    if v.t() { v.s() } else { String::new() }
}

#[test]
fn assumption_diff_matches_the_prototype() {
    set_clock();
    let all = load("f4-diff");
    let cases = all.g("cases").items();
    assert!(cases.len() > 150, "{} cases", cases.len());
    let mut calls = std::collections::BTreeMap::<String, usize>::new();
    for case in cases {
        let call = case.g("call").s();
        *calls.entry(call.clone()).or_default() += 1;
        let p = project(case);
        let got: Result<V, String> = match call.as_str() {
            "parseOther" => expert::parse_other(&text_of(case.g("text"))),
            "diffParams" => expert::diff_params(&p, case.g("other")),
            "runDiff" => {
                let mut opts = case.g("opts").clone();
                if case.g("scIndex").is_num() {
                    opts.set("sc", p.g("scenarios").ix(case.g("scIndex").n() as usize).clone());
                }
                Ok(expert::run_diff(&p, case.g("diffs"), &opts))
            }
            other => panic!("{}: unknown call {other}", case.g("id").s()),
        };
        match got {
            Ok(v) => check_ok(case, &v),
            Err(e) => check_error(case, &e),
        }
    }
    for call in ["parseOther", "diffParams", "runDiff"] {
        assert!(calls.get(call).copied().unwrap_or(0) >= 10, "too few {call} cases");
    }
}

#[test]
fn json_reader_matches_v8() {
    let all = load("f4-jsonerr");
    let cases = all.g("cases").items();
    assert!(cases.len() > 3000, "{} cases", cases.len());
    let mut bad: Vec<String> = Vec::new();
    for case in cases {
        let s = case.g("s").s();
        match expert::parse_json(&s) {
            Ok(v) => {
                let want = case.g("ok");
                if !want.is_str() || v.stringify() != want.s() {
                    bad.push(format!("{s:?}: read as {} but JSON.parse gave {}", v.stringify(), if want.is_str() { want.s() } else { format!("the error {}", case.g("err").s()) }));
                }
            }
            Err(e) => {
                if !case.has("err") || e != case.g("err").s() {
                    bad.push(format!("{s:?}: refused with {e:?} but JSON.parse {}", if case.has("err") { format!("refused with {:?}", case.g("err").s()) } else { format!("gave {}", case.g("ok").s()) }));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} of {} texts differ:\n{}", bad.len(), cases.len(), bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
}
