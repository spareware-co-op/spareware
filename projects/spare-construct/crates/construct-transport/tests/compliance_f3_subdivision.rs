//! Urban subdivision (AUP E38) against the JavaScript's recorded answers (expected/compliance-f3-subdivision.json, from
//! tools/transport-truth/gen-compliance-f3.mjs).

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, load, project, set_clock};
use construct_transport::compliance::extra::subdivision;
use construct_transport::compliance::jv::Jx;

#[test]
fn subdivision_matches_the_prototype() {
    set_clock();
    let all = load("f3-subdivision");
    let cases = all.g("cases").items();
    assert!(cases.len() > 200, "{} cases", cases.len());
    for case in cases {
        let id = case.g("id").s();
        match case.g("call").s().as_str() {
            "subdivision" => check_ok(case, &subdivision::subdivision(&project(case))),
            "rearAccess" => check_ok(case, &subdivision::rear_access(case.g("args").g("a"))),
            "report" => {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() {
                    subdivision::report(&rid, &p, Some(&Mock))
                } else {
                    subdivision::report(&rid, &p, None)
                };
                check_text(
                    case,
                    &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")),
                );
            }
            other => panic!("{id}: unknown call {other}"),
        }
    }
}
