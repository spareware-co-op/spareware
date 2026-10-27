//! The speed limit validator and the cost benefit disclosure statement (compliance-extra.js section 9) with their CSV export and report section against
//! the prototype's recorded answers (expected/compliance-f2-speed.json from tools/transport-truth/gen-compliance-f2.mjs).

mod compliance_support;
mod support;

use compliance_support::{case_error, case_ok, check, expected, fixture, set_path};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::{self, speed};
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

/// The ids a case lists, as text.
fn ids_of(args: &V) -> Vec<String> {
    args.g("ids").items().iter().map(|x| x.s()).collect()
}

#[test]
fn speed_limits_cbds_and_exports() {
    set_clock(Some(1792540800000.0));
    let all = expected("f2-speed");
    let cases = all["cases"].as_array().expect("cases");
    let ord = ordered("f2-speed");
    assert!(cases.len() > 450, "{} cases", cases.len());
    for (i, case) in cases.iter().enumerate() {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let oc = ord.g("cases").ix(i);
        let mut p = cached_fixture(case["fixture"].as_str().unwrap());
        for (k, v) in oc.g("sets").entries() {
            set_path(&mut p, k, v.clone());
        }
        if case_error(case).is_some() {
            panic!("{id}: the prototype refused this case: {}", case_error(case).unwrap());
        }
        let want = case_ok(case);
        let args = oc.g("args").clone();
        let sc_arg: Option<V> = if args.has("sc") && !args.g("sc").nullish() { Some(p.g("scenarios").ix(args.g("sc").n() as usize).clone()) } else { None };
        let got: V = match call {
            "minLenFor" => V::Number(speed::min_len_for(args.g("v").n())),
            "speedSections" => {
                let sc = sc_arg.as_ref().expect("scenario");
                V::Array(speed::speed_sections(sc).iter().map(|ls| V::Array(ls.iter().map(|l| l.g("id").clone()).collect())).collect())
            }
            "roadLen" => {
                let sc = sc_arg.as_ref().expect("scenario");
                let ls: Vec<&V> = ids_of(&args).iter().map(|id| sc.g("links").g(id)).collect();
                V::Number(speed::road_len(sc, &ls))
            }
            "speedCheck" => speed::speed_check(&p, sc_arg.as_ref()),
            "schoolSpeeds" => speed::school_speeds(sc_arg.as_ref().expect("scenario")),
            "sectionCrashes" => {
                let sc = sc_arg.as_ref().expect("scenario");
                let ls: Vec<&V> = ids_of(&args).iter().map(|id| sc.g("links").g(id)).collect();
                speed::section_crashes(&p, sc, &ls).unwrap_or(V::Null)
            }
            "crashHistory" => {
                let sc = sc_arg.as_ref().expect("scenario");
                let ls: Vec<&V> = ids_of(&args).iter().map(|id| sc.g("links").g(id)).collect();
                let since = if args.g("since").nullish() { None } else { Some(args.g("since").n()) };
                speed::crash_history(&p, sc, &ls, since)
            }
            "cbds" => speed::cbds(&p, &ids_of(&args), sc_arg.as_ref()),
            "speedCSV" => V::String(speed::speed_csv(&p)),
            "export-speed" => {
                let mut r = V::object();
                r.set("text", V::String(extra::export_build("compliance-extra-speed", &p).expect("export")));
                r.set("enabled", V::Bool(extra::export_enabled("compliance-extra-speed", &p).expect("enabled")));
                r
            }
            "report-speed" | "report-speed-ctx" => {
                let ctx = TestCtx;
                let c: Option<&dyn ReportCtx> = if call.ends_with("-ctx") { Some(&ctx) } else { None };
                V::String(extra::report("compliance-extra-speed", &p, c).expect("report section"))
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
    }
}
