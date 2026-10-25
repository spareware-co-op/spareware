//! Time-of-use charging screening against the JavaScript's recorded answers (expected/compliance-f3-tou.json, from
//! tools/transport-truth/gen-compliance-f3.mjs). The screening runs the assignment, so these cases take a while.

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, load, project, set_clock};
use construct_transport::compliance::engine;
use construct_transport::compliance::extra::gps::polygon_of;
use construct_transport::compliance::extra::tou;
use construct_transport::compliance::jv::{Jx, V};
use std::collections::HashMap;

#[test]
fn time_of_use_matches_the_prototype() {
    set_clock();
    let all = load("f3-tou");
    let cases = all.g("cases").items();
    assert!(cases.len() > 120, "{} cases", cases.len());
    for case in cases {
        let id = case.g("id").s();
        let args = case.g("args");
        match case.g("call").s().as_str() {
            "touCharged" => {
                let p = project(case);
                let sc = p.g("scenarios").ix(args.g("sci").n() as usize);
                let got = tou::tou_charged(sc, &polygon_of(args.g("poly")), &args.g("kind").s());
                let mut o = V::object();
                for (id, how) in got {
                    o.set(id.as_str(), V::String(how.to_string()));
                }
                check_ok(case, &o);
            }
            "touSkim" => {
                let p = project(case);
                let sc = p.g("scenarios").ix(args.g("sci").n() as usize);
                let per = args.g("per").s();
                let g = engine::build_graph(&p, sc, None);
                let zs: Vec<String> = sc.g("zones").keys().into_iter().map(|k| k.to_string()).collect();
                let charged = if args.g("charged").t() { Some(tou::tou_charged(sc, &polygon_of(args.g("poly")), "cordon")) } else { None };
                let applied: Option<HashMap<String, f64>> =
                    if args.g("applied").t() { Some(args.g("applied").entries().into_iter().map(|(k, v)| (k.to_string(), v.n())).collect()) } else { None };
                let got = tou::tou_skim(&p, &g, Some(sc.g("res").g(&per)), charged.as_deref(), applied.as_ref(), args.g("pen").n(), &zs);
                let mut o = V::object();
                for (f, m) in &got {
                    let mut inner = V::object();
                    for (t, x) in m {
                        let mut cell = V::object();
                        cell.set("gc", V::Number(x.gc));
                        cell.set("n", V::Number(x.n));
                        cell.set("real", V::Number(x.real));
                        inner.set(t.as_str(), cell);
                    }
                    o.set(f.as_str(), inner);
                }
                check_ok(case, &o);
            }
            "touRun" => {
                let mut p = project(case);
                let res = tou::tou_run(&mut p, case.g("opts"));
                let mut o = V::object();
                o.set("res", res);
                o.set("saved", V::Bool(p.g("complianceExtra").g("tou").g("result").t()));
                check_ok(case, &o);
            }
            "touCSV" => check_text(case, &tou::tou_csv(&project(case))),
            "report" => {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() { tou::report(&rid, &p, Some(&Mock)) } else { tou::report(&rid, &p, None) };
                check_text(case, &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")));
            }
            "export" => {
                let p = project(case);
                let eid = case.g("export").s();
                let mut got = V::object();
                got.set("build", V::String(tou::export_build(&eid, &p).expect("export")));
                got.set("enabled", V::Bool(tou::export_enabled(&eid, &p).expect("export")));
                got.set("label", case.g("ok").g("label").clone());
                check_ok(case, &got);
            }
            other => panic!("{id}: unknown call {other}"),
        }
    }
}
