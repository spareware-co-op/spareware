//! Options and alternatives, plan change mode and full build-out (compliance-extra.js section 8) with their CSV exports and report sections,
//! against the prototype's recorded answers (expected/compliance-f2-options.json from tools/transport-truth/gen-compliance-f2.mjs).

mod compliance_support;
mod support;

use compliance_support::{case_error, case_ok, check, expected, fixture, set_path};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::{self, options};
use construct_transport::compliance::html::ReportCtx;
use construct_transport::compliance::jv::{Jx, V};

/// The report context of the generator: a table comes back wrapped so the report lane's block is visible in the text.
struct TestCtx;
impl ReportCtx for TestCtx {
    fn table(&self, title: &str, html: &str, note: &str) -> String {
        format!("<TABLE title=\"{title}\" note=\"{note}\">{html}</TABLE>")
    }
}

/// The new scenario of a full build-out with its random id replaced (`norm` of the generator).
fn norm(v: &V) -> V {
    if v.t() && v.g("id").t() {
        let mut o = v.clone();
        o.set("id", V::String("<uid>".into()));
        o
    } else {
        v.clone()
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
fn options_plan_change_and_exports() {
    set_clock(Some(1792540800000.0));
    let all = expected("f2-options");
    let cases = all["cases"].as_array().expect("cases");
    let ord = ordered("f2-options");
    assert!(cases.len() > 250, "{} cases", cases.len());
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
        let got: V = match call {
            "optionList" => V::Array(
                options::option_list(&p)
                    .iter()
                    .map(|e| {
                        let mut o = e.o.clone();
                        o.set("sc", e.sc.map_or(V::Null, |s| s.g("id").clone()));
                        o
                    })
                    .collect(),
            ),
            "alternatives" => {
                let mut a = options::alternatives(&p);
                let name = a.g("base").g("name").clone();
                a.set("base", name);
                a
            }
            "alternativeSections" => options::alternative_sections(&p, None),
            "mergeZoned" => options::merge_zoned(args.g("current"), args.g("zoned")),
            "fullBuildout" => {
                if args.has("sc") {
                    let sc = p.g("scenarios").ix(args.g("sc").n() as usize).clone();
                    options::full_buildout(&p, Some(&sc))
                } else {
                    options::full_buildout(&p, None)
                }
            }
            "createFullBuildout" => {
                let before = p.g("scenarios").jlen();
                let ns = if args.has("sc") {
                    let sc = p.g("scenarios").ix(args.g("sc").n() as usize).clone();
                    options::create_full_buildout(&mut p, Some(&sc))
                } else {
                    options::create_full_buildout(&mut p, None)
                };
                let names: Vec<V> = p.g("scenarios").items().iter().map(|s| s.g("name").clone()).collect();
                let last = if ns.t() { V::Bool(p.g("scenarios").items().last() == Some(&ns)) } else { V::Null };
                let mut r = V::object();
                r.set("ns", norm(&ns));
                r.set("n", V::Number((p.g("scenarios").jlen() - before) as f64));
                r.set("names", V::Array(names));
                r.set("last", last);
                r
            }
            "rltpRelied" => V::String(options::rltp_relied(&p)),
            "planChange" => options::plan_change(&p),
            "optionsCSV" => V::String(options::options_csv(&p)),
            "planChangeCSV" => V::String(options::plan_change_csv(&p)),
            "export-options" | "export-planchange" => {
                let eid = if call == "export-options" { "compliance-extra-options" } else { "compliance-extra-planchange" };
                let mut r = V::object();
                r.set("text", V::String(extra::export_build(eid, &p).expect("export")));
                r.set("enabled", V::Bool(extra::export_enabled(eid, &p).expect("enabled")));
                r
            }
            "report-alternatives" | "report-alternatives-ctx" | "report-planchange" | "report-planchange-ctx" => {
                let rid = if call.starts_with("report-alternatives") { "compliance-extra-alternatives" } else { "compliance-extra-planchange" };
                let ctx = TestCtx;
                let c: Option<&dyn ReportCtx> = if call.ends_with("-ctx") { Some(&ctx) } else { None };
                V::String(extra::report(rid, &p, c).expect("report section"))
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
    }
}
