//! Flood plains and coastal inundation against the JavaScript's recorded runs: the label a flood polygon is merged under, the
//! download (the layer's description, paging, the coastal layers the council lists, each way a layer can fail) and the roads inside
//! the hazards (merged models never counted twice, holes, slivers, the only road to a proposal zone) for every scenario in turn.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-hazards.json, fixtures/data/hazards-*.json, flood-meta.json).

mod feeds_support;
mod support;

use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

#[test]
fn flood_plain_labels_match() {
    let e = expected_ordered("hazards");
    let recs = e.get("labels").unwrap().as_array().unwrap();
    assert_eq!(recs.len(), 24);
    for rec in recs {
        let attrs = rec.get("attrs").unwrap();
        assert_eq!(flood_label(attrs), rec.get("label").unwrap().js_string(), "label for {}", attrs.stringify());
    }
}

fn run_load(rec: &Value, label: &str) -> (Hazards, HazardsLoaded, construct_transport::project::Project, Vec<String>) {
    let mut p = reopen(&fixture_text(&format!("hazards-{}-project.json", rec["project"].as_str().unwrap())));
    let mut load = HazardsLoad::new(bounds_of(&rec["bounds"]), 30_000.0, &MetaCache::new());
    let log = rec["requests"].as_array().unwrap();
    let (used, res) = replay(&mut load, log, 0, label);
    res.expect("the download");
    all_used(log, used, label);
    let statuses = load.statuses().to_vec();
    let (hz, loaded) = load.finish(&mut p, NOW);
    (hz, loaded, p, statuses)
}

#[test]
fn downloads_match() {
    let e = expected("hazards");
    let recs = e["loads"].as_array().unwrap();
    assert_eq!(recs.len(), 18);
    for rec in recs {
        let label = format!("{} {}", rec["project"], rec["label"]);
        let (hz, loaded, p, statuses) = run_load(rec, &label);
        assert_eq!(json!(statuses), rec["statuses"], "{label}: status lines");
        assert_close(&serde_json::to_value(&loaded).unwrap(), &rec["result"], &format!("{label} result"));
        assert_close(&serde_json::to_value(&hz.layers).unwrap(), &rec["layers"], &format!("{label} layers"));
        assert_close(&p.extra.get("feeds").cloned().unwrap_or_else(|| json!({ "vintage": {} })), &rec["feeds"], &format!("{label} feeds"));
    }
}

#[test]
fn roads_inside_hazards_match() {
    let e = expected("hazards");
    let recs = e["links"].as_array().unwrap();
    assert_eq!(recs.len(), 6);
    for rec in recs {
        let name = rec["project"].as_str().unwrap();
        let label = format!("{name} order {}", rec["order"]);
        let all = e["loads"].as_array().unwrap().iter().find(|l| l["project"] == name && l["label"] == "all").unwrap();
        let (hz, ..) = run_load(all, &label);
        let p = reopen(rec["text"].as_str().unwrap());
        let order: Vec<usize> = rec["order"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
        let mut got = Vec::new();
        for si in order {
            let Some(sc) = p.scenarios.get(si) else { continue };
            got.push(json!({ "scenario": sc.id, "rows": hazard_links(sc, &hz, &p.settings) }));
        }
        assert_close(&Value::Array(got), &rec["rows"], &format!("{label} rows"));
        let empty = hazard_links(&p.scenarios[0], &Hazards::default(), &p.settings);
        assert_close(&serde_json::to_value(&empty).unwrap(), &rec["empty"], &format!("{label} without data"));
    }
}
