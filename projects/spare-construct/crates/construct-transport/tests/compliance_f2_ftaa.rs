//! The Fast-track package (compliance-extra.js section 8, invitees to package files), its files, JSON bundle, export and report section against the
//! prototype's recorded answers (expected/compliance-f2-ftaa.json from tools/transport-truth/gen-compliance-f2.mjs).

mod compliance_support;
mod support;

use compliance_support::{case_error, case_ok, check, expected, fixture, set_path};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::docs;
use construct_transport::compliance::extra::{self, ftaa};
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
    CACHE.with(|c| {
        c.borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| fixture(name))
            .clone()
    })
}

/// The recorded cases with their sets in the order the generator wrote them (`Object.entries` order matters to some checks).
fn ordered(name: &str) -> V {
    V::parse(&support::read_truth(&format!(
        "expected/compliance-{name}.json"
    )))
    .expect("expected file")
}

/// The model inputs file of the package is long: the recorded answers hold its length (in UTF-16 units) and fingerprint instead.
fn stand_in(text: &str) -> String {
    format!(
        "<{} characters {}>",
        text.encode_utf16().count(),
        docs::hash(text)
    )
}

/// Runs the recorded cases whose call the filter accepts.
fn fill_records(p: &mut V) {
    let _ = construct_transport::compliance::state_mut(p);
    let _ = extra::ctx::data_mut(p);
    // `DataHub.D()` makes the project's data record, and fills what is missing, the first time anything reads it
    if !p.g("data").is_obj() {
        p.set("data", V::object());
    }
    let d = p.get_mut("data").unwrap();
    for (k, empty) in [
        ("counts", "[]"),
        ("crashes", "[]"),
        ("loaded", "{}"),
        ("jt", "[]"),
    ] {
        if !d.g(k).t() {
            d.set(k, V::parse(empty).unwrap());
        }
    }
    let mut u =
        V::parse(r#"{"nzta":true,"at":true,"user":true,"virtual":false,"ramm":true}"#).unwrap();
    for (k, v) in d.g("use").entries() {
        u.set(k, v.clone());
    }
    d.set("use", u);
}

fn run(filter: &dyn Fn(&str) -> bool) -> usize {
    set_clock(Some(1792540800000.0));
    let all = expected("f2-ftaa");
    let cases = all["cases"].as_array().expect("cases");
    let ord = ordered("f2-ftaa");
    let mut n = 0;
    for (i, case) in cases.iter().enumerate() {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        if !filter(call) {
            continue;
        }
        let oc = ord.g("cases").ix(i);
        let mut p = cached_fixture(case["fixture"].as_str().unwrap());
        for (k, v) in oc.g("sets").entries() {
            set_path(&mut p, k, v.clone());
        }
        if case_error(case).is_some() {
            panic!(
                "{id}: the prototype refused this case: {}",
                case_error(case).unwrap()
            );
        }
        let want = case_ok(case);
        let model_name = "reviewer/model-inputs.json";
        let got: V = match call {
            "invitees" => ftaa::invitees(&p),
            "ftaaTimeline" => ftaa::ftaa_timeline(&p),
            "ftaaPackage" => ftaa::ftaa_package(&p),
            "ftaaFiles" => {
                fill_records(&mut p);
                let files = ftaa::ftaa_files(&p);

                let mut fo = V::object();
                let mut man: Vec<V> = Vec::new();
                for (k, v) in &files {
                    man.push(V::String(k.clone()));
                    fo.set(
                        k.as_str(),
                        V::String(if k == model_name {
                            stand_in(v)
                        } else {
                            v.clone()
                        }),
                    );
                }
                let mut r = V::object();
                r.set("manifest", V::Array(man));
                r.set("files", fo);
                r
            }
            "ftaaBundle" | "ftaaZip" => {
                fill_records(&mut p);
                let text = if call == "ftaaBundle" {
                    ftaa::ftaa_bundle(&p)
                } else {
                    ftaa::ftaa_zip(&p)
                };
                let model = ftaa::ftaa_files(&p)
                    .into_iter()
                    .find(|(k, _)| k == model_name)
                    .expect("model inputs")
                    .1;
                V::String(text.replace(
                    &V::String(model.clone()).stringify(),
                    &V::String(stand_in(&model)).stringify(),
                ))
            }
            "ftaaReadme" => {
                fill_records(&mut p);
                V::String(ftaa::ftaa_readme(&p))
            }
            "ftaaReadmeOld" => V::String(ftaa::ftaa_readme(&p)),
            "report-ftaa" | "report-ftaa-ctx" => {
                let ctx = TestCtx;
                let c: Option<&dyn ReportCtx> = if call.ends_with("-ctx") {
                    Some(&ctx)
                } else {
                    None
                };
                V::String(extra::report("compliance-extra-ftaa", &p, c).expect("report section"))
            }
            "export-ftaa-enabled" => {
                V::Bool(extra::export_enabled("compliance-extra-ftaa", &p).expect("enabled"))
            }
            "export-ftaa-ext" => {
                let mut r = V::object();
                r.set(
                    "ext",
                    construct_transport::compliance::data::x("EXPORTS")
                        .items()
                        .iter()
                        .find(|e| e.ix(0).eq_s("ftaa"))
                        .map_or(V::Null, |e| e.ix(2).clone()),
                );
                r.set(
                    "text",
                    V::String(
                        extra::export_build("compliance-extra-ftaa", &p)
                            .expect("export")
                            .chars()
                            .take(60)
                            .collect(),
                    ),
                );
                r
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
        n += 1;
    }
    n
}

#[test]
fn ftaa_invitees_and_timeline() {
    let n = run(&|c| c == "invitees" || c == "ftaaTimeline");
    assert!(n > 30, "{n} cases");
}

#[test]
fn ftaa_package_files_and_report() {
    let n = run(&|c| c != "invitees" && c != "ftaaTimeline");
    assert!(n > 100, "{n} cases");
}
