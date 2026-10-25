//! Expert disclosure, sign-off, data sources, the model status badge, supersession, disclaimer checks and the JWS table against the
//! JavaScript's recorded answers (expected/compliance-f4-expert.json, from tools/transport-truth/gen-compliance-f4.mjs).

mod compliance_f4_support;
mod compliance_support;
mod support;

use compliance_f4_support::{apply, check_error, check_ok, load, project, set_clock};
use compliance_support::check;
use construct_transport::compliance::jv::{Jx, V, arr, st};
use construct_transport::compliance::{self, expert};

/// Replays the steps of a scripted case: each step changes the project or reads it, and has its recorded answer.
fn replay(case: &V) {
    let id = case.g("id").s();
    let mut p = project(case);
    for (n, step) in case.g("steps").items().iter().enumerate() {
        let at = format!("{id} step {n} {}", step.g("op").s());
        let got: V = match step.g("op").s().as_str() {
            "sets" => {
                apply(&mut p, step.g("sets"));
                continue;
            }
            "signOff" => match expert::sign_off(&mut p, step.g("who"), step.g("now")) {
                Ok(s) => s,
                Err(e) => {
                    assert_eq!(e, step.g("error").s(), "{at}");
                    continue;
                }
            },
            "clearStale" => V::Number(expert::clear_stale(&mut p, step.g("now")) as f64),
            "expert" => compliance::state(&p).g("expert").clone(),
            "appendixG" => expert::appendix_g(&p),
            "jwsCSV" => st(expert::jws_csv(&p, step.g("result"))),
            "dataSources" => expert::data_sources(&p),
            other => panic!("{at}: unknown op {other}"),
        };
        assert!(step.has("ok"), "{at}: the prototype refused with {}", step.g("error").s());
        check(&at, &got, &step.g("ok").to_value());
    }
}

#[test]
fn expert_matches_the_prototype() {
    set_clock();
    let all = load("f4-expert");
    let cases = all.g("cases").items();
    assert!(cases.len() > 300, "{} cases", cases.len());
    let mut calls = std::collections::BTreeMap::<String, usize>::new();
    for case in cases {
        let call = case.g("call").s();
        *calls.entry(call.clone()).or_default() += 1;
        if call == "script" {
            replay(case);
            continue;
        }
        let p = project(case);
        let got: V = match call.as_str() {
            "dataSources" => expert::data_sources(&p),
            "appendixG" => expert::appendix_g(&p),
            "badge" => {
                let period = if case.g("period").t() { case.g("period").s() } else { "AM".to_string() };
                expert::badge(&p, &period)
            }
            "supersession" => expert::supersession(&p, case.g("now")),
            "missingDisclaimers" => {
                let kinds: Vec<String> = case.g("kinds").items().iter().map(|k| k.s()).collect();
                let kinds: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
                arr(expert::missing_disclaimers(&case.g("html").s(), &kinds).into_iter().map(st))
            }
            "jwsCSV" => st(expert::jws_csv(&p, case.g("result"))),
            other => panic!("{}: unknown call {other}", case.g("id").s()),
        };
        if case.has("error") {
            check_error(case, &got.s());
        } else {
            check_ok(case, &got);
        }
    }
    for call in ["script", "dataSources", "appendixG", "badge", "supersession", "missingDisclaimers", "jwsCSV"] {
        assert!(calls.get(call).copied().unwrap_or(0) > 5, "too few {call} cases");
    }
}
