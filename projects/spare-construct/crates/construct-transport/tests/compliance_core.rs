//! The Compliance module's project checks against the JavaScript's recorded answers (expected/compliance-core.json, from
//! tools/transport-truth/gen-compliance-core.mjs): authorities, site, E27, ITA scope, access, state highways, FDS, operators, working days
//! and clocks, disclaimers and CSV stamping, the checklist, report sections and exports.

mod compliance_support;
mod support;

use compliance_support::{case_ok, check, variant};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::jv::{Jx, ToV, V, st};
use construct_transport::compliance::{self as cx};

macro_rules! jobj {
    ($($k:expr => $v:expr),* $(,)?) => {{
        let mut o = V::object();
        $( o.set($k, ToV::to_v($v)); )*
        o
    }};
}

macro_rules! jarr {
    ($($v:expr),* $(,)?) => { V::Array(vec![$(ToV::to_v($v)),*]) };
}

fn arr(items: Vec<V>) -> V {
    V::Array(items)
}

fn num(x: f64) -> V {
    V::Number(x)
}

fn links_of(p: &V, sc: usize) -> Vec<V> {
    p.g("scenarios")
        .ix(sc)
        .g("links")
        .vals()
        .into_iter()
        .cloned()
        .collect()
}

const EVS: &str = r#"[{"kind":"lodged","date":"2026-11-02"},{"kind":"s92","date":"2026-12-10"},{"kind":"cl21","date":"2026-09-01"},{"kind":"cl23","date":"2027-01-12"},{"kind":"ftaaInvite","date":"2026-12-01"},{"kind":"ftaaComments","date":"2026-10-20"},{"kind":"ftaaS67","date":"2026-10-21"},{"kind":"ftaaS67","date":"2026-10-21","set":"2026-12-31"},{"kind":"ftaaS67","date":"2026-10-21","set":"2026-10-28"},{"kind":"hearing","date":"2027-02-10"},{"kind":"works","date":"2026-12-22"},{"kind":"bogus","date":"2026-10-01"},{"kind":"s92","date":"x"},{"kind":"s92","date":"2053-06-01"}]"#;

fn run_case(case: &serde_json::Value) -> Option<V> {
    let id = case["id"].as_str().unwrap();
    let call = case["call"].as_str().unwrap();
    let mut p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
    let now = V::String("2026-10-21".into());
    let sc = cx::active_sc(&p).clone();
    Some(match call {
        "authorities" => cx::authorities(&p, &now),
        "siteInfo" => cx::site_info(&p),
        "sitePoint" => cx::site_point(&p, &sc).unwrap_or(V::Null),
        "shOf" => arr(links_of(&p, 0)
            .iter()
            .take(4)
            .map(|l| cx::sh_of(&p, l))
            .collect()),
        "linkAuthority" => arr(links_of(&p, 0)
            .iter()
            .map(|l| cx::link_authority(&p, l, p.g("scenarios").ix(1)))
            .collect()),
        "e27" => cx::e27(&p),
        "itaScope" => cx::ita_scope(&p),
        "scopingMemo" => cx::scoping_memo(&p, &sc),
        "table1" => {
            let qs = [
                (jobj! { "dwellings" => 35 }, 0.0),
                (jobj! { "dwellings" => 150, "office" => 100 }, 30.0),
                (jobj! { "retailDT" => 5 }, 0.0),
                (jobj! { "dwellings" => 0 }, 0.0),
                (
                    jobj! { "dwellings" => 10, "office" => 200, "retail" => 100, "primary" => 40 },
                    0.0,
                ),
            ];
            arr(qs
                .iter()
                .map(|(q, u)| {
                    let mut full = jobj! { "dwellings" => 0, "ird" => 0, "visitor" => 0, "subdivision" => 0, "primary" => 0, "secondary" => 0, "tertiary" => 0, "office" => 0, "retail" => 0, "retailDT" => 0, "warehouse" => 0, "industrial" => 0 };
                    for (k, v) in q.entries() {
                        full.set(k, v.clone());
                    }
                    cx::table1(&jobj! { "q" => full, "unlistedPeak" => *u })
                })
                .collect())
        }
        "depthOf" => arr(["None", "Lite", "Full"]
            .iter()
            .flat_map(|r| {
                [true, false].into_iter().flat_map(move |k| {
                    ["urban", "periurban", "rural", "x"]
                        .into_iter()
                        .map(move |x| cx::depth_of(r, k, x))
                })
            })
            .collect()),
        "sisd" => arr(vec![
            (num(50.0), 0.0),
            (num(80.0), 0.0),
            (st("x"), 0.0),
            (num(100.0), 87.0),
            (num(200.0), 0.0),
            (num(10.0), 0.0),
            (num(60.0), 95.5),
        ]
        .iter()
        .map(|(s, m)| cx::sisd(s, *m))
        .collect()),
        "accessType" => arr(vec![
            cx::access_type(300.0, 5.0, 0.0, None),
            cx::access_type(10.0, 30.0, 0.0, None),
            cx::access_type(20.0, 0.0, 0.0, Some(true)),
            cx::access_type(20.0, 0.0, 0.0, Some(false)),
            cx::access_type(20.0, 0.0, 0.0, None),
            cx::access_type(80.0, 0.0, 0.0, None),
            cx::access_type(20.0, 0.0, 3.0, Some(true)),
        ]),
        "pavement" => arr([
            (0.0, 0.0),
            (0.0, 5.0),
            (100.0, 110.0),
            (100.0, 130.0),
            (100.0, 60.0),
            (50.5, 61.2),
        ]
        .iter()
        .map(|(a, b)| cx::pavement(*a, *b))
        .collect()),
        "accessCheck" => {
            let k = case["arg"].as_u64().unwrap() as usize;
            cx::access_check(&p, cx::state(&p).g("access").ix(k), &sc)
        }
        "accessGeo" => arr(cx::state(&p)
            .g("access")
            .items()
            .iter()
            .map(|a| cx::access_geo(&p, p.g("scenarios").ix(1), a))
            .collect()),
        "shCheck" => cx::sh_check(&p, &sc),
        "shInvolved" => cx::sh_involved(&p, &sc),
        "motorwayConnectors" => {
            let n = p
                .g("scenarios")
                .ix(1)
                .g("links")
                .keys()
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            if let V::Array(scs) = p.get_mut("scenarios").unwrap() {
                for k in &n {
                    scs[1]
                        .get_mut("links")
                        .unwrap()
                        .get_mut(k)
                        .unwrap()
                        .set("cls", st("motorway"));
                }
            }
            cx::motorway_connectors(p.g("scenarios").ix(1), &p)
        }
        "fdsCheck" => cx::fds_check(&p),
        "operators" => {
            if id.starts_with("operators AM") {
                cx::operators_with(&p, &sc, cx::base_sc(&p), Some("AM"))
            } else {
                cx::operators(&p)
            }
        }
        "workingDays" => {
            let mut out: Vec<V> = Vec::new();
            for kind in ["rma", "ftaa"] {
                for d in [
                    "2026-12-18",
                    "2026-12-24",
                    "2027-01-01",
                    "2027-01-04",
                    "2026-06-30",
                    "2026-04-03",
                    "2026-02-06",
                    "2025-01-01",
                    "2028-01-01",
                    "2052-12-20",
                    "2053-12-24",
                ] {
                    for n in [1.0, 5.0, 10.0, 15.0, 20.0, -5.0, -15.0, 0.0] {
                        out.push(jarr![kind, d, n, cx::add_working_days(d, n, kind)]);
                    }
                }
            }
            out.push(jarr![
                "x",
                "rubbish",
                5,
                cx::add_working_days("rubbish", 5.0, "rma")
            ]);
            out.push(jarr![
                "bw",
                cx::working_days_between("2026-10-01", "2026-11-30", "rma"),
                cx::working_days_between("2026-11-30", "2026-10-01", "rma"),
                cx::working_days_between("2026-10-01", "2026-10-01", "rma"),
                cx::working_days_between("x", "2026-10-01", "rma")
            ]);
            out.push(jarr![
                "iw",
                arr(
                    ["2026-10-21", "2026-10-24", "2026-12-25", "2027-06-25", "x"]
                        .iter()
                        .map(|d| jarr![cx::is_working_day(d, "rma"), cx::is_working_day(d, "ftaa")])
                        .collect()
                )
            ]);
            arr(out)
        }
        "holidays" => {
            let mut o = V::object();
            for y in [2021, 2022, 2026, 2027, 2028, 2052, 2053, 2100, 1999] {
                o.set(
                    y.to_string(),
                    arr(cx::holidays(y)
                        .into_iter()
                        .map(|(k, v)| jarr![k, v])
                        .collect()),
                );
            }
            o
        }
        "clockFor" => arr(V::parse(EVS)
            .unwrap()
            .items()
            .iter()
            .map(|e| cx::clock_for(e).unwrap_or(V::Null))
            .collect()),
        "clocks" => cx::clocks(&p),
        "rfiTemplate" => st(cx::rfi_template(&p)),
        "disclaimer" => arr(cx::data::c("DISCLAIMERS")
            .keys()
            .into_iter()
            .chain(["D99"])
            .map(|id| {
                let known = cx::data::c("DISCLAIMERS").has(id);
                jarr![
                    cx::docs::disclaimer(id, Some(&p), None),
                    cx::docs::disclaimer(id, None, None),
                    cx::docs::disclaimer(id, Some(&p), Some("SIDRA 11")),
                    if known {
                        st(cx::docs::disclaimer_html(id, Some(&p), Some("X")))
                    } else {
                        V::Null
                    }
                ]
            })
            .collect()),
        "csv" => {
            if id == "csv true" {
                let fp = cx::docs::fingerprint(&p);
                cx::state_mut(&mut p).get_mut("expert").unwrap().set("signoffs", arr(vec![jobj! { "role" => "prepared", "name" => "A", "quals" => "B", "date" => "2026-10-01", "fp" => fp }, jobj! { "role" => "reviewed", "name" => "C", "quals" => "D", "date" => "2026-10-01", "fp" => "old" }]));
            }
            let twice = cx::docs::stamp_csv(&cx::docs::stamp_csv("a,b", &p, None), &p, None);
            jobj! { "footer" => cx::docs::csv_footer(&p, None), "handover" => cx::docs::csv_footer(&p, Some("SIDRA")), "stamp" => cx::docs::stamp_csv("a,b\r\n1,2\r\n\r\n", &p, None), "stamp2" => cx::docs::stamp_csv("", &p, Some("X")), "twice" => twice,
            "sign" => cx::docs::sign_state(&p), "fp" => cx::docs::fingerprint(&p) }
        }
        "fingerprint" => {
            let a = cx::docs::fingerprint(&p);
            p.get_mut("settings").unwrap().set("growth", num(9.0));
            let b = cx::docs::fingerprint(&p);
            if let V::Array(scs) = p.get_mut("scenarios").unwrap() {
                scs[0].set("role", st("x"));
            }
            jarr![a, b, cx::docs::fingerprint(&p)]
        }
        "checklist" => cx::checklist(&p),
        "checklistCSV" => st(cx::checklist_csv(&p, &now)),
        "report" => st(cx::report_section(case["arg"].as_str().unwrap(), &p, None)?),
        "export" => st(cx::export_build(case["arg"].as_str().unwrap(), &p, &now)?.unwrap()),
        "exportEnabled" => arr([
            "compliance-checklist",
            "compliance-scoping",
            "compliance-register",
            "compliance-timeline",
            "compliance-jws",
        ]
        .iter()
        .map(|i| V::Bool(cx::export_enabled(i, &p).unwrap()))
        .collect()),
        other => panic!("{id}: unknown call {other}"),
    })
}

#[test]
fn core_checks() {
    set_clock(Some(1792540800000.0));
    let all = compliance_support::expected("core");
    let cases = all["cases"].as_array().expect("cases");
    let only = std::env::var("CX_ONLY").ok();
    let mut n = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        if only.as_deref().is_some_and(|o| !id.contains(o)) {
            continue;
        }
        let want = case_ok(case);
        let got = run_case(case).unwrap_or_else(|| panic!("{id}: no answer"));
        check(id, &got, want);
        n += 1;
    }
    assert!(n > 100 || only.is_some(), "{n} cases");
}
