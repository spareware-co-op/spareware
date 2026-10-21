//! ANOP indicative multimodal level of service and fit (compliance-extra.js section 10) with its CSV export and report section against the prototype's
//! recorded answers (expected/compliance-f2-anop.json from tools/transport-truth/gen-compliance-f2.mjs).

mod compliance_support;
mod support;

use compliance_support::{case_error, case_ok, check, expected, fixture, set_path};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::{self, anop};
use construct_transport::compliance::html::ReportCtx;
use construct_transport::compliance::jv::{Jx, V};

/// The report context of the generator: a table comes back wrapped so the report lane's block is visible in the text.
struct TestCtx;
impl ReportCtx for TestCtx {
    fn table(&self, title: &str, html: &str, note: &str) -> String {
        format!("<TABLE title=\"{title}\" note=\"{note}\">{html}</TABLE>")
    }
}

/// A fixture, read from disk once.
fn cached_fixture(name: &str) -> V {
    thread_local! { static CACHE: std::cell::RefCell<std::collections::HashMap<String, V>> = std::cell::RefCell::new(std::collections::HashMap::new()); }
    CACHE.with(|c| c.borrow_mut().entry(name.to_string()).or_insert_with(|| fixture(name)).clone())
}

/// The recorded cases with their sets and args in the order the generator wrote them (`Object.entries` order matters to some checks).
fn ordered(name: &str) -> V {
    V::parse(&support::read_truth(&format!("expected/compliance-{name}.json"))).expect("expected file")
}

#[test]
fn anop_levels_fit_and_exports() {
    set_clock(Some(1792540800000.0));
    let all = expected("f2-anop");
    let cases = all["cases"].as_array().expect("cases");
    let ord = ordered("f2-anop");
    assert!(cases.len() > 600, "{} cases", cases.len());
    for (i, case) in cases.iter().enumerate() {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let oc = ord.g("cases").ix(i);
        if case_error(case).is_some() {
            panic!("{id}: the prototype refused this case: {}", case_error(case).unwrap());
        }
        let want = case_ok(case);
        let ok = oc.g("ok");
        let args = oc.g("args").clone();
        // the pure tables need no project
        if call == "anopPref" {
            let rows: Vec<V> = ok
                .items()
                .iter()
                .map(|r| {
                    let mut o = V::object();
                    for k in ["mode", "h", "lu"] {
                        o.set(k, r.g(k).clone());
                    }
                    o.set("pref", anop::anop_pref(&r.g("mode").s(), &r.g("h").s(), &r.g("lu").s()));
                    o
                })
                .collect();
            assert_eq!(rows.len() as f64, args.g("n").n());
            check(id, &V::Array(rows), want);
            continue;
        }
        if call == "anopFit" {
            let rows: Vec<V> = ok
                .items()
                .iter()
                .map(|r| {
                    let mut o = V::object();
                    o.set("modes", r.g("modes").clone());
                    o.set("fit", anop::anop_fit(r.g("modes").items()).map_or(V::Null, V::String));
                    o
                })
                .collect();
            check(id, &V::Array(rows), want);
            continue;
        }
        let mut p = cached_fixture(case["fixture"].as_str().unwrap());
        for (k, v) in oc.g("sets").entries() {
            set_path(&mut p, k, v.clone());
        }
        let sc_arg: Option<V> = if args.has("sc") { Some(p.g("scenarios").ix(args.g("sc").n() as usize).clone()) } else { None };
        let ids: Vec<String> = args.g("ids").items().iter().map(|x| x.s()).collect();
        let got: V = match call {
            "hierOf" => anop::hier_of(sc_arg.as_ref().expect("scenario"), args.g("corridor"), &args.g("mode").s()),
            "corridorAxis" => anop::corridor_axis(sc_arg.as_ref().expect("scenario"), &ids).map_or(V::Null, V::Number),
            "corridorSpeed" => {
                let sc = sc_arg.as_ref().expect("scenario");
                let res = if args.g("per").nullish() { V::Null } else { sc.g("res").g(&args.g("per").s()).clone() };
                anop::corridor_speed(sc, &res, &ids, args.g("pt").t())
            }
            "anopLoS" => {
                let sc = sc_arg.as_ref().expect("scenario");
                anop::anop_los(sc, sc.g("res").g(&args.g("per").s()), args.g("corridor"))
            }
            "anopCorridors" => anop::anop_corridors(&p),
            "anop" => anop::anop(&p),
            "anopCSV" => V::String(anop::anop_csv(&p)),
            "export-anop" => {
                let mut r = V::object();
                r.set("text", V::String(extra::export_build("compliance-extra-anop", &p).expect("export")));
                r.set("enabled", V::Bool(extra::export_enabled("compliance-extra-anop", &p).expect("enabled")));
                r
            }
            "report-anop" | "report-anop-ctx" => {
                let ctx = TestCtx;
                let c: Option<&dyn ReportCtx> = if call.ends_with("-ctx") { Some(&ctx) } else { None };
                V::String(extra::report("compliance-extra-anop", &p, c).expect("report section"))
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
    }
}
