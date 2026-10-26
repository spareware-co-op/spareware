//! Travel demand against the JavaScript's recorded answers: trip ends of every zone in both truth-test projects
//! (expected/<name>.json), and the demand model on the variants gen-demand.mjs makes of them
//! (expected/demand-cases.json: growth, staging, opening years, reductions, extra trips, trip-generation tables, fitted
//! factors, the manual matrix, matrix estimation, balancing notes), including `applyME` on seeded matrices.

mod recorded;
mod support;

use construct_transport::demand::{apply_me, demand, trip_ends};
use construct_transport::graph::build_graph;
use construct_transport::project::Project;
use serde_json::{Value, json};
use support::{PROJECTS, assert_close, load_expected, load_json, load_project};

#[test]
fn trip_ends_as_recorded() {
    let mut n = 0;
    for name in PROJECTS {
        let p = load_project(name);
        let expected = load_expected(name);
        for sc in &p.scenarios {
            for period in ["AM", "PM"] {
                let want = &expected["scenarios"][&sc.id]["periods"][period]["tripEnds"];
                let ends = want.as_object().expect("recorded trip ends");
                assert!(!ends.is_empty());
                for z in sc.zones.values().filter(|z| !z.kind.is("gateway")) {
                    let t = trip_ends(&p, z, &p.settings, period, sc);
                    assert_close(
                        &json!({ "O": t.o, "D": t.d }),
                        &want[&z.id],
                        &format!("{name}.{}.{period}.tripEnds.{}", sc.id, z.id),
                    );
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 30);
}

fn num_list(v: &[f64]) -> Value {
    Value::Array(v.iter().map(|&x| construct_transport::jsnum::plain_number(x)).collect())
}

#[test]
fn demand_and_apply_me_on_the_variants() {
    let cases = load_json("expected/demand-cases.json");
    let cases = cases.as_array().expect("cases");
    assert_eq!(cases.len(), 33);
    let (mut n_demand, mut n_me) = (0, 0);
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let p = Project::from_json(case["projectText"].as_str().expect("project text"))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let sc = p.scenario(case["scenario"].as_str().expect("scenario")).expect("scenario");
        for period in ["AM", "PM"] {
            let rec = &case["periods"][period];
            let path = format!("{name}.{period}");
            let g = build_graph(sc, &p.settings, Some(period));
            // trip ends of every zone and gateway
            for z in sc.zones.values() {
                let t = trip_ends(&p, z, &p.settings, period, sc);
                assert_close(&json!({ "O": t.o, "D": t.d }), &rec["tripEnds"][&z.id], &format!("{path}.tripEnds.{}", z.id));
            }
            let d = demand(&p, sc, &g, period, &p.settings);
            let got = json!({
                "Z": d.z, "O": num_list(&d.o), "D": num_list(&d.d), "T": num_list(&d.t), "note": d.note, "manual": d.manual,
                "O0": d.o0.as_deref().map_or(Value::Null, num_list), "D0": d.d0.as_deref().map_or(Value::Null, num_list),
            });
            assert_close(&got, &rec["demand"], &format!("{path}.demand"));
            n_demand += 1;
            for (k, me) in rec["applyME"].as_array().expect("applyME cases").iter().enumerate() {
                let mut t: Vec<f64> = me["T0"].as_array().expect("T0").iter().map(|v| v.as_f64().expect("number")).collect();
                let year = me["year"].as_f64();
                let note = apply_me(&p, &g.zones, period, &mut t, year);
                assert_eq!(note, me["note"].as_str().expect("note"), "{path}.applyME[{k}].note");
                assert_close(&num_list(&t), &me["T"], &format!("{path}.applyME[{k}].T"));
                n_me += 1;
            }
        }
    }
    assert_eq!((n_demand, n_me), (66, 264));
}
