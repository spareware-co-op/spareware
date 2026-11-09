//! Route queries on the result the Rust assignment gives, against the prototype's answers on its own results
//! (expected/routes-<name>.json, from gen-routes.mjs): travel between network nodes and zone centroids, isochrones
//! from them, and select-link on the busiest arcs, a quiet arc and an unknown key.

mod recorded;
mod support;

use construct_transport::assign::assign;
use construct_transport::results::{AssignOpts, isochrone, select_link, travel};
use recorded::json;
use support::{PROJECTS, assert_close, load_json, load_project};

#[test]
fn travel_isochrone_and_select_link_between_graph_nodes() {
    let (mut n_travel, mut n_iso, mut n_sel) = (0, 0, 0);
    for name in PROJECTS {
        let p = load_project(name);
        let rec = load_json(&format!("expected/routes-{name}.json"));
        for sc in &p.scenarios {
            for period in ["AM", "PM"] {
                let opts = AssignOpts {
                    period: Some(period.to_string()),
                    ..AssignOpts::default()
                };
                let res = assign(&p, sc, &p.settings, &opts, None);
                let r = &rec["scenarios"][&sc.id][period];
                let path = format!("{name}.{}.{period}", sc.id);
                for t in r["travel"].as_array().expect("travel") {
                    let (from, to) = (
                        t["from"].as_str().expect("from"),
                        t["to"].as_str().expect("to"),
                    );
                    let got = travel(sc, &p.settings, Some(&res), from, to);
                    assert_close(
                        &json(&got),
                        &t["out"],
                        &format!("{path}.travel {from} to {to}"),
                    );
                    n_travel += 1;
                }
                for t in r["isochrone"].as_array().expect("isochrone") {
                    let from = t["from"].as_str().expect("from");
                    let got = isochrone(sc, &p.settings, Some(&res), from);
                    assert_close(&json(&got), &t["out"], &format!("{path}.isochrone {from}"));
                    n_iso += 1;
                }
                for t in r["selectLink"].as_array().expect("selectLink") {
                    let key = t["key"].as_str().expect("key");
                    let got = select_link(Some(&res), key);
                    assert_close(&json(&got), &t["out"], &format!("{path}.selectLink {key}"));
                    n_sel += 1;
                }
            }
        }
    }
    // reached routes, unreached or unknown ones and the unknown arc all count
    assert_eq!((n_travel, n_iso, n_sel), (100, 60, 80));
}
