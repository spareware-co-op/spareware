//! Gateway flows from counts (`DataHub.gatewaysFromCounts`) against the JavaScript's recorded answers
//! (expected/calibrate-gateways-*.json, from tools/transport-truth/gen-calibrate.mjs): the counts near each gateway
//! on its own road, physical counts before RAMM estimates before NZTA virtual sites, directional counts split into
//! flows in and out, two-way counts split by the lanes each way, the source switches, counts switched off, and
//! gateways that already have flows.

mod calibrate_support;
mod support;

use construct_transport::datahub::{Data, gateways_from_counts};
use construct_transport::jsnum::Maybe;
use construct_transport::project::{OrderedMap, Project};
use serde_json::{Map, Value, json};
use support::{assert_close, load_json};

fn maybe_json<T: serde::Serialize>(m: &Maybe<T>) -> Value {
    match m {
        Maybe::Set(v) => serde_json::to_value(v).expect("value"),
        _ => Value::Null,
    }
}

fn gateways(p: &Project) -> Value {
    Value::Object(
        p.scenarios[0]
            .zones
            .iter()
            .filter(|(_, z)| z.kind.is("gateway"))
            .map(|(id, z)| (id.to_string(), json!({ "gw": maybe_json(&z.gw), "gwSrc": maybe_json(&z.gw_src) })))
            .collect::<Map<String, Value>>(),
    )
}

/// What the generator did to the project before running the function.
fn apply(p: &mut Project, spec: &Value) {
    if let Some(d) = p.data.get_mut() {
        if let Some(Value::Array(keep)) = spec.get("keep")
            && let Some(Value::Array(counts)) = d.extra.get_mut("counts")
        {
            counts.retain(|c| keep.contains(&c["id"]));
        }
        if let Some(Value::Object(u)) = spec.get("use") {
            let slot = d.extra.entry("use".to_string()).or_insert_with(|| json!({}));
            for (k, v) in u {
                slot[k] = v.clone();
            }
        }
        if let Some(Value::Array(off)) = spec.get("off")
            && let Some(Value::Array(counts)) = d.extra.get_mut("counts")
        {
            for c in counts.iter_mut().filter(|c| off.contains(&c["id"])) {
                c["off"] = json!(true);
            }
        }
    }
    if let Some(Value::Object(pre)) = spec.get("preGw") {
        for (id, gw) in pre {
            let z = p.scenarios[0].zones.get_mut(id).expect("zone");
            z.gw = if gw.is_null() { Maybe::Null } else { Maybe::Set(serde_json::from_value::<OrderedMap<_>>(gw.clone()).expect("gw")) };
        }
    }
}

#[test]
fn gateway_flows_from_counts() {
    let mut cases = 0;
    let mut set_total = 0;
    for f in ["expected/calibrate-gateways-corridor.json", "expected/calibrate-gateways-grid.json"] {
        let all = load_json(f);
        for c in all["cases"].as_array().expect("cases") {
            let name = c["name"].as_str().expect("name");
            let mut p = Project::from_json(all["projectText"].as_str().expect("text")).expect("project");
            apply(&mut p, &c["spec"]);
            let label = format!("{f}: {name}");
            assert_close(&gateways(&p), &c["before"], &format!("{label}: before"));
            let data = Data::read(&p);
            let settings = p.settings.clone();
            let n = gateways_from_counts(&mut p.scenarios[0], &settings, &data);
            assert_eq!(n as u64, c["n"].as_u64().expect("n"), "{label}: gateways set");
            assert_close(&gateways(&p), &c["after"], &format!("{label}: after"));
            cases += 1;
            set_total += n;
        }
    }
    assert_eq!(cases, 28);
    assert!(set_total >= 40, "{set_total} gateways set over all cases");
}
