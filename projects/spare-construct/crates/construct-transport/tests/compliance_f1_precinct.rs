//! The precinct staging tracker against the JavaScript's recorded answers (expected/compliance-f1-precinct.json, from
//! tools/transport-truth/gen-compliance-f1.mjs).

mod compliance_f1_support;
mod compliance_support;
mod support;

use compliance_f1_support::{args, ok_of, run_export, run_report};
use compliance_support::{check, variant};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::precinct;
use construct_transport::compliance::jv::{Jx, V};

#[test]
fn precinct_staging() {
    set_clock(Some(1792540800000.0));
    let all = compliance_support::expected("f1-precinct");
    let cases = all["cases"].as_array().expect("cases");
    assert!(cases.len() > 400, "{} cases", cases.len());
    let mut n = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
        let a = args(case);
        let want = ok_of(case);
        let got: V = match call {
            "precinctCheck" => {
                if a.has("sc") {
                    precinct::precinct_check(&p, Some(p.g("scenarios").ix(a.g("sc").n() as usize)))
                } else {
                    precinct::precinct_check(&p, None)
                }
            }
            "precinctOf" => precinct::precinct_of(&p).unwrap_or(V::Null),
            "precinctList" => precinct::precinct_list(&p),
            "precinctCSV" => V::String(precinct::precinct_csv(&p)),
            "report" => run_report(case, &p),
            "export" => run_export(case, &p),
            "mixTrips" => {
                precinct::mix_trips(&p, a.g("mix"), p.g("scenarios").ix(a.g("sc").n() as usize))
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
        n += 1;
    }
    assert_eq!(n, cases.len());
}
