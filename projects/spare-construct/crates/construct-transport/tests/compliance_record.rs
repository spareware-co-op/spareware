//! The compliance record, the regime and the version stamp against the JavaScript's recorded answers
//! (expected/compliance-record.json, from tools/transport-truth/gen-compliance-record.mjs).

mod compliance_support;
mod support;

use compliance_support::{case_ok, check, variant};
use construct_transport::compliance::jv::V;
use construct_transport::compliance::{self, today};

#[test]
fn record_regime_stamp() {
    compliance::dates::set_clock(Some(1792540800000.0));
    let all = compliance_support::expected("record");
    let cases = all["cases"].as_array().expect("cases");
    assert!(cases.len() > 100, "{} cases", cases.len());
    let mut n = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let mut p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
        let want = case_ok(case);
        let got: V = match call {
            "regime" => compliance::regime(&p),
            "state" => compliance::state(&p),
            "recheck" => compliance::recheck(&p),
            "stamp" => {
                // the case name carries the day the stamp was made on
                let now = id
                    .split(' ')
                    .nth(1)
                    .filter(|d| d.starts_with("20"))
                    .map_or(V::Null, |d| V::String(d.to_string()));
                compliance::stamp(&p, &now)
            }
            "today" => {
                let arg = id.strip_prefix("today ").unwrap();
                let v = match arg {
                    "clock" => V::Null,
                    a if a.parse::<f64>().is_ok() => V::Number(a.parse().unwrap()),
                    a => V::String(a.to_string()),
                };
                if arg == "clock" {
                    // the clock is the real one here: only the shape is checked
                    assert_eq!(today(&v).len(), 10);
                    continue;
                }
                V::String(today(&v))
            }
            "recordOutputs" => {
                let o = compliance::record_outputs(&mut p, &V::String("2026-10-21".into()));
                let used = compliance::state(&p);
                let mut r = V::object();
                r.set("o", o);
                r.set("used", used.get("used").cloned().unwrap());
                r.set(
                    "docs",
                    V::Number(compliance::used_docs(&V::object()).len() as f64),
                );
                r
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
        n += 1;
    }
    assert!(n > 100);
}
