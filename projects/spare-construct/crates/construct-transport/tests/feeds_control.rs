//! Signal and sign downloads, the control proposals and applying them, and OpenStreetMap turn restrictions with the bans they
//! propose, against the JavaScript's recorded runs (every layer failure, both test projects, several states of the network).
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-control.json, fixtures/data/control-*.json, bans-*.json).

mod feeds_support;
mod support;

use construct_transport::edit::ids::IdSource;
use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::*;
use construct_transport::project::{Project, Scenario};
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

/// Hands out the ids the prototype made, in order.
struct Recorded {
    ids: Vec<String>,
    at: usize,
}

impl IdSource for Recorded {
    fn uid(&mut self, _prefix: &str) -> String {
        let id = self
            .ids
            .get(self.at)
            .unwrap_or_else(|| panic!("more ids asked for than the {} recorded", self.ids.len()))
            .clone();
        self.at += 1;
        id
    }
    fn now_ms(&self) -> f64 {
        NOW
    }
}

fn scenario_at<'a>(p: &'a mut Project, id: &str) -> &'a mut Scenario {
    p.scenarios
        .iter_mut()
        .find(|s| s.id == id)
        .expect("scenario")
}

fn nodes_of(sc: &Scenario) -> Value {
    serde_json::to_value(&sc.nodes).unwrap()
}

fn links_of(sc: &Scenario) -> Value {
    serde_json::to_value(&sc.links).unwrap()
}

fn sites_of(sc: &Scenario) -> Value {
    match sc.sites.get() {
        Some(_) => serde_json::to_value(&sc.sites).unwrap(),
        None => Value::Null,
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

fn feeds_value(p: &Project) -> Value {
    p.extra.get("feeds").cloned().unwrap_or(Value::Null)
}

#[test]
fn signal_and_sign_downloads_match() {
    let e = expected("control");
    let loads = e["loads"].as_array().unwrap();
    assert_eq!(loads.len(), 6);
    for rec in loads {
        let at = format!("{} {}", rec["project"], rec["label"]);
        let mut p = project(rec["project"].as_str().unwrap());
        let log = rec["requests"].as_array().unwrap();
        let mut load = ControlLoad::new(bounds_of(&rec["bounds"]), 30_000.0, &MetaCache::new());
        let (used, res) = replay(&mut load, log, 0, &at);
        all_used(log, used, &at);
        if let Some(t) = rec.get("thrown") {
            assert_eq!(
                res.expect_err("the download fails"),
                t.as_str().unwrap(),
                "{at}: error"
            );
            continue;
        }
        res.expect("the download");
        let data = load.finish();
        // the JavaScript stamps the download with the time it finished (`at`); the Rust is given that time when applying it
        let mut want = rec["data"].clone();
        want.as_object_mut().unwrap().remove("at");
        assert_close(
            &serde_json::to_value(&data).unwrap(),
            &want,
            &format!("{at} data"),
        );
        apply_control_load(&mut p, &data, NOW);
        assert_close(&feeds_value(&p), &rec["feeds"], &format!("{at} feeds"));
        let d = serde_json::to_value(&p.data).unwrap();
        assert_close(
            &d["controlled"],
            &rec["dataD"]["controlled"],
            &format!("{at} shared sites"),
        );
        assert_close(
            &d["loaded"],
            &rec["dataD"]["loaded"],
            &format!("{at} loaded"),
        );
    }
}

fn recorded_data(e: &Value, project: &str) -> ControlData {
    let rec = e["loads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["project"] == project && l["label"] == "with-meta")
        .unwrap();
    serde_json::from_value(rec["data"].clone()).expect("recorded data reads")
}

#[test]
fn control_proposals_and_applying_match() {
    let e = expected("control");
    let recs = e["proposals"].as_array().unwrap();
    assert_eq!(recs.len(), 20);
    for rec in recs {
        let (name, variant, scid) = (
            rec["project"].as_str().unwrap(),
            rec["variant"].as_str().unwrap(),
            rec["scenario"].as_str().unwrap(),
        );
        let at = format!("{name} {variant} {scid}");
        let data = recorded_data(&e, name);
        let text = fixture_text(rec["startFile"].as_str().unwrap());
        let mut p = project_of_text(&text);
        let sc = scenario_at(&mut p, scid);
        assert_close(
            &nodes_of(sc),
            &rec["startNodes"],
            &format!("{at} start nodes"),
        );
        assert_close(
            &sites_of(sc),
            &rec["startSites"],
            &format!("{at} start sites"),
        );
        let prop = control_proposals(sc, Some(&data));
        assert_close(
            &serde_json::to_value(&prop.rows).unwrap(),
            &rec["rows"],
            &format!("{at} rows"),
        );
        assert_close(
            &serde_json::to_value(&prop.unmatched).unwrap(),
            &rec["unmatched"],
            &format!("{at} unmatched"),
        );
        assert_close(
            &serde_json::to_value(control_proposals(sc, None)).unwrap(),
            &rec["noData"],
            &format!("{at} without data"),
        );
        for pick in ["all", "some", "none", "splits"] {
            let a = &rec[format!("apply_{pick}")];
            let mut p2 = project_of_text(&text);
            let sc2 = scenario_at(&mut p2, scid);
            let rows = control_proposals(sc2, Some(&data)).rows;
            let mut ids = Recorded {
                ids: strings(&a["uids"]),
                at: 0,
            };
            let n = apply_controls(sc2, &mut ids, &rows, &strings(&a["ids"]), "2026-10-21");
            let label = format!("{at} apply {pick}");
            assert_eq!(n as u64, a["n"].as_u64().unwrap(), "{label}: applied");
            assert_eq!(ids.at, ids.ids.len(), "{label}: ids made");
            assert_close(&nodes_of(sc2), &a["nodes"], &format!("{label} nodes"));
            assert_close(&links_of(sc2), &a["links"], &format!("{label} links"));
            assert_close(&sites_of(sc2), &a["sites"], &format!("{label} sites"));
        }
    }
}

#[test]
fn restriction_downloads_match() {
    let e = expected("control");
    let loads = e["banLoads"].as_array().unwrap();
    assert_eq!(loads.len(), 8);
    for rec in loads {
        let at = format!("{} {}", rec["project"], rec["label"]);
        let mut p = project(rec["project"].as_str().unwrap());
        let log = rec["requests"].as_array().unwrap();
        let mut load = RestrictionsLoad::new(&bounds_of(&rec["bounds"]), 30_000.0);
        let (used, _) = replay(&mut load, log, 0, &at);
        all_used(log, used, &at);
        match load.finish() {
            Err(msg) => assert_eq!(
                msg,
                rec["thrown"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{at}: unexpected failure {msg}")),
                "{at}: error"
            ),
            Ok((rels, url)) => {
                assert!(rec.get("thrown").is_none(), "{at}: the recording failed");
                let got: Vec<Value> = rels.iter().map(JsonValue::to_value).collect();
                assert_close(
                    &Value::Array(got),
                    &rec["data"]["rels"],
                    &format!("{at} relations"),
                );
                assert_eq!(json!(url), rec["data"]["url"], "{at}: server");
                apply_restrictions(&mut p, &rels, &url, NOW);
                assert_close(&feeds_value(&p), &rec["feeds"], &format!("{at} feeds"));
            }
        }
    }
}

#[test]
fn turn_bans_and_applying_match() {
    let e = expected("control");
    let recs = e["bans"].as_array().unwrap();
    assert_eq!(recs.len(), 12);
    for rec in recs {
        let (name, variant, scid) = (
            rec["project"].as_str().unwrap(),
            rec["variant"].as_str().unwrap(),
            rec["scenario"].as_str().unwrap(),
        );
        let at = format!("{name} {variant} {scid}");
        let load = e["banLoads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["project"] == name && l["label"] == "first-server")
            .unwrap();
        let rels: Vec<JsonValue> = load["data"]["rels"]
            .as_array()
            .unwrap()
            .iter()
            .map(ordered)
            .collect();
        let text = fixture_text(rec["startFile"].as_str().unwrap());
        let mut p = project_of_text(&text);
        let sc = scenario_at(&mut p, scid);
        assert_close(
            &nodes_of(sc),
            &rec["startNodes"],
            &format!("{at} start nodes"),
        );
        let prop = ban_proposals(sc, Some(&rels));
        assert_close(
            &serde_json::to_value(&prop.rows).unwrap(),
            &rec["rows"],
            &format!("{at} rows"),
        );
        assert_close(
            &serde_json::to_value(&prop.skipped).unwrap(),
            &rec["skipped"],
            &format!("{at} skipped"),
        );
        assert_close(
            &serde_json::to_value(ban_proposals(sc, None)).unwrap(),
            &rec["noData"],
            &format!("{at} without data"),
        );
        for pick in ["all", "some"] {
            let a = &rec[format!("apply_{pick}")];
            let mut p2 = project_of_text(&text);
            let sc2 = scenario_at(&mut p2, scid);
            let rows = ban_proposals(sc2, Some(&rels)).rows;
            let n = apply_bans(sc2, &rows, &strings(&a["ids"]), "2026-10-21");
            let label = format!("{at} apply {pick}");
            assert_eq!(n as u64, a["n"].as_u64().unwrap(), "{label}: applied");
            assert_close(&nodes_of(sc2), &a["nodes"], &format!("{label} nodes"));
        }
    }
}
