//! The statutory scope and wording linter against the JavaScript's recorded answers (expected/compliance-f1-lint.json, from
//! tools/transport-truth/gen-compliance-f1.mjs).

mod compliance_f1_support;
mod compliance_support;
mod support;

use compliance_f1_support::{args, obj, ok_of, run_export};
use compliance_support::{check, variant};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::lint;
use construct_transport::compliance::jv::{Jx, V};

#[test]
fn linter() {
    set_clock(Some(1792540800000.0));
    let all = compliance_support::expected("f1-lint");
    let cases = all["cases"].as_array().expect("cases");
    assert!(cases.len() > 500, "{} cases", cases.len());
    let mut n = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let mut p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
        let a = args(case);
        let want = ok_of(case);
        let got: V = match call {
            "lint" => lint::lint(&p, a.g("text").as_str()),
            "overrideLint" => {
                let reason = if a.g("reason").t() { a.g("reason").s() } else { String::new() };
                let r = lint::override_lint(&mut p, &a.g("key").s(), &reason);
                let ov = construct_transport::compliance::extra::ctx::data(&p).g("lint").g("override").clone();
                if want.get("record").is_some() {
                    obj(vec![("r", r), ("record", construct_transport::compliance::extra::ctx::data(&p).g("lint").clone())])
                } else {
                    obj(vec![("r", r), ("override", ov)])
                }
            }
            "exportBlock" => lint::export_block(&p).map_or(V::Null, V::String),
            "lintCSV" => V::String(lint::lint_csv(&p)),
            "export" => run_export(case, &p),
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
        n += 1;
    }
    assert_eq!(n, cases.len());
}
