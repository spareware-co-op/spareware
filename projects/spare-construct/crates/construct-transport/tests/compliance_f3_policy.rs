//! Zone-to-zone times, the roads a proposal affects, accessibility and policy alignment against the JavaScript's recorded answers
//! (expected/compliance-f3-policy.json, from tools/transport-truth/gen-compliance-f3.mjs). Cases that need the Future Development Strategy
//! check or the ANOP assessment of another part are skipped while those are still stubs (the count is printed).

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, guard, load, project, set_clock};
use construct_transport::compliance::extra::policy;
use construct_transport::compliance::jv::{Jx, V};

#[test]
fn policy_matches_the_prototype() {
    set_clock();
    let all = load("f3-policy");
    let cases = all.g("cases").items();
    assert!(cases.len() > 300, "{} cases", cases.len());
    let (mut checked, mut skipped) = (0, 0);
    for case in cases {
        let id = case.g("id").s();
        let call = case.g("call").s();
        let args = case.g("args");
        let done = match call.as_str() {
            "skim" => {
                let p = project(case);
                let sc = p.g("scenarios").ix(args.g("sci").n() as usize);
                let ids: Vec<String> = sc
                    .g("zones")
                    .keys()
                    .into_iter()
                    .map(|k| k.to_string())
                    .collect();
                let list = |v: &V| -> Vec<String> {
                    if v.is_arr() {
                        v.items().iter().map(|x| x.s()).collect()
                    } else {
                        ids.clone()
                    }
                };
                let (from, to) = (list(args.g("from")), list(args.g("to")));
                let res = if args.g("per").t() {
                    Some(sc.g("res").g(&args.g("per").s()))
                } else {
                    None
                };
                let got = policy::skim(&p, sc, res, &from, &to);
                let mut o = V::object();
                for (f, m) in &got {
                    let mut inner = V::object();
                    for (t, x) in m {
                        inner.set(t.as_str(), V::Number(*x));
                    }
                    o.set(f.as_str(), inner);
                }
                check_ok(case, &o);
                Some(())
            }
            "jobsOf" => {
                check_ok(case, &V::Number(policy::jobs_of(args.g("z"))));
                Some(())
            }
            "placeOf" => {
                check_ok(
                    case,
                    &policy::place_of(args.g("L")).map_or(V::Null, V::Number),
                );
                Some(())
            }
            "affectedLinks" => {
                let p = project(case);
                let a = policy::affected_links(&p);
                let mut o = V::object();
                o.set("ids", V::Array(a.ids));
                o.set("basis", V::String(a.basis));
                check_ok(case, &o);
                Some(())
            }
            "accessibility" => {
                check_ok(case, &policy::accessibility(&project(case)));
                Some(())
            }
            "policy" => guard(|| check_ok(case, &policy::policy(&project(case)))),
            "policyCSV" => guard(|| check_ok(case, &V::String(policy::policy_csv(&project(case))))),
            "report" => guard(|| {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() {
                    policy::report(&rid, &p, Some(&Mock))
                } else {
                    policy::report(&rid, &p, None)
                };
                check_text(
                    case,
                    &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")),
                );
            }),
            "export" => guard(|| {
                let p = project(case);
                let eid = case.g("export").s();
                let mut got = V::object();
                got.set(
                    "build",
                    V::String(policy::export_build(&eid, &p).expect("export")),
                );
                got.set(
                    "enabled",
                    V::Bool(policy::export_enabled(&eid, &p).expect("export")),
                );
                got.set("label", case.g("ok").g("label").clone());
                check_ok(case, &got);
            }),
            other => panic!("{id}: unknown call {other}"),
        };
        if done.is_some() {
            checked += 1;
        } else {
            skipped += 1;
        }
    }
    println!(
        "policy: {checked} cases checked, {skipped} skipped (a part this one calls is still a stub)"
    );
    assert_eq!(checked + skipped, cases.len());
    assert!(checked > 200);
}
