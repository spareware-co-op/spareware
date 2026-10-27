//! IPM stage 2 GPS metrics and funding attribution against the JavaScript's recorded answers (expected/compliance-f3-gps.json, from
//! tools/transport-truth/gen-compliance-f3.mjs). Cases that print the policy alignment (which needs the Future Development Strategy check of
//! another part) are skipped while that is still a stub.

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, guard, load, project, set_clock};
use construct_transport::compliance::extra::{gps, policy};
use construct_transport::compliance::jv::{Jx, V};

#[test]
fn gps_and_funding_match_the_prototype() {
    set_clock();
    let all = load("f3-gps");
    let cases = all.g("cases").items();
    assert!(cases.len() > 700, "{} cases", cases.len());
    let (mut checked, mut skipped) = (0, 0);
    for case in cases {
        let id = case.g("id").s();
        let args = case.g("args");
        let done: Option<()> = match case.g("call").s().as_str() {
            "gtRating" => {
                check_ok(case, &V::String(gps::gt_rating(args.g("s").n()).to_string()));
                Some(())
            }
            "freightRating" => {
                check_ok(case, &V::String(gps::freight_rating(args.g("s").n(), &args.g("sig").s()).to_string()));
                Some(())
            }
            "roadsToLinks" => {
                let p = project(case);
                let sc = p.g("scenarios").ix(args.g("sci").n() as usize);
                check_ok(case, &V::Array(gps::roads_to_links(sc, args.g("names"))));
                Some(())
            }
            "inPoly" => {
                let pt = [args.g("pt").ix(0).n(), args.g("pt").ix(1).n()];
                let poly = gps::polygon_of(args.g("poly"));
                check_ok(case, &V::Bool(gps::in_poly(pt, &poly)));
                Some(())
            }
            "gpsRoutes" => {
                check_ok(case, &V::Array(gps::gps_routes(&project(case))));
                Some(())
            }
            "gpsMetrics" => {
                check_ok(case, &gps::gps_metrics(&project(case)));
                Some(())
            }
            "growthShare" => {
                let p = project(case);
                let sc = if args.has("sci") { p.g("scenarios").ix(args.g("sci").n() as usize) } else { construct_transport::compliance::extra::ctx::pair(&p).opt };
                check_ok(case, &gps::growth_share(&p, sc));
                Some(())
            }
            "fundingAssets" => {
                let p = project(case);
                let fa = gps::funding_assets(&p, construct_transport::compliance::extra::ctx::pair(&p).opt);
                let mut o = V::object();
                o.set("ids", V::Array(fa.ids.into_iter().map(V::String).collect()));
                o.set("basis", V::String(fa.basis.to_string()));
                check_ok(case, &o);
                Some(())
            }
            "funding" => {
                check_ok(case, &gps::funding(&project(case)));
                Some(())
            }
            "fundingShares" => {
                let mut p = project(case);
                let res = gps::funding_shares(&mut p, case.g("opts"));
                let mut o = V::object();
                o.set("res", res);
                o.set("saved", p.g("complianceExtra").g("funding").g("shares").clone());
                check_ok(case, &o);
                Some(())
            }
            "policyCSV" => guard(|| check_text(case, &policy::policy_csv(&project(case)))),
            "report" => {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() { gps::report(&rid, &p, Some(&Mock)) } else { gps::report(&rid, &p, None) };
                check_text(case, &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")));
                Some(())
            }
            other => panic!("{id}: unknown call {other}"),
        };
        if done.is_some() {
            checked += 1;
        } else {
            skipped += 1;
        }
    }
    println!("gps: {checked} cases checked, {skipped} skipped (a part this one calls is still a stub)");
    assert_eq!(checked + skipped, cases.len());
    assert!(checked > 650);
}
