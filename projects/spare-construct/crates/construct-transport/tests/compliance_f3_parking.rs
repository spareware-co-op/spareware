//! Parking, cycle parking, end-of-trip facilities and loading against the JavaScript's recorded answers (expected/compliance-f3-parking.json,
//! from tools/transport-truth/gen-compliance-f3.mjs).

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, load, project, set_clock};
use construct_transport::compliance::extra::parking;
use construct_transport::compliance::jv::{Jx, V};

#[test]
fn parking_matches_the_prototype() {
    set_clock();
    let all = load("f3-parking");
    let cases = all.g("cases").items();
    assert!(cases.len() > 600, "{} cases", cases.len());
    let mut n = 0;
    for case in cases {
        let id = case.g("id").s();
        let call = case.g("call").s();
        let args = case.g("args");
        match call.as_str() {
            "cycleRates" => {
                let got = parking::cycle_rates(args.g("act"), args.g("q").n(), args.g("row"));
                check_ok(case, &got);
            }
            "showersFor" => check_ok(case, &V::Number(parking::showers_for(args.g("g").n()))),
            "loadingFor" => check_ok(case, &V::Number(parking::loading_for(&args.g("grp").s(), args.g("g").n()))),
            "accessibleFor" => check_ok(case, &V::Number(parking::accessible_for(args.g("n").n()))),
            "parking" => check_ok(case, &parking::parking(&project(case))),
            "parkingCSV" => check_text(case, &parking::parking_csv(&project(case))),
            "report" => {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() { parking::report(&rid, &p, Some(&Mock)) } else { parking::report(&rid, &p, None) };
                check_text(case, &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")));
            }
            "export" => {
                let p = project(case);
                let eid = case.g("export").s();
                let mut got = V::object();
                got.set("build", V::String(parking::export_build(&eid, &p).expect("export")));
                got.set("enabled", V::Bool(parking::export_enabled(&eid, &p).expect("export")));
                got.set("label", case.g("ok").g("label").clone());
                check_ok(case, &got);
            }
            other => panic!("{id}: unknown call {other}"),
        }
        n += 1;
    }
    assert!(n > 600);
}
