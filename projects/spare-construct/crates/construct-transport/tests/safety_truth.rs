//! Crash analysis against the JavaScript's recorded answers: the CAS query window, crash records as tuples, loading the crash
//! and Future Connect layers into a project, the risk bands, Future Connect conflation onto links, crashes near a link or
//! junction, and link risk, junction risk and the safety summary of both test projects in many situations (counts, modelled
//! volumes now and in a forecast year, load dates around new year, a project with no load time or window).
//! Recorded by tools/transport-truth/gen-safety.mjs (expected/safety-*.json, fixtures/data/cas-*.json, fc-*.json).

mod safety_support;
mod support;

use construct_transport::calibrate::stored_res;
use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::parse_future_connect;
use construct_transport::jsnum::Maybe;
use construct_transport::safety::*;
use safety_support::*;
use serde_json::{Value, json};
use support::{assert_close, load_json};

fn years_arg(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "undefined" | "null" | "NaN" | "x" => None,
            t => t.parse().ok(),
        },
        _ => None,
    }
}

#[test]
fn query_windows_match() {
    let j = load_json("expected/safety-cas.json");
    let list = j["window"].as_array().unwrap();
    assert!(list.len() > 200);
    for (i, rec) in list.iter().enumerate() {
        let got = cas_window(rec["now"].as_f64().unwrap(), years_arg(&rec["years"]));
        assert_close(&serde_json::to_value(&got).unwrap(), &rec["result"], &format!("$.window[{i}] now {} years {}", rec["now"], rec["years"]));
    }
    // the constants
    let f = &j["fields"];
    assert_eq!(json!(CAS_FIELDS), f["fields"]);
    for (k, p) in f["pos"].as_object().unwrap() {
        assert_eq!(cas_pos(k), p.as_u64().map(|p| p as usize), "position of {k}");
    }
    assert_eq!(f["window"]["years"].as_f64(), Some(CAS_YEARS_DEFAULT));
    assert_eq!(f["window"]["longYears"].as_f64(), Some(CAS_LONG_YEARS));
    assert_eq!(f["window"]["lagMonths"].as_f64(), Some(CAS_LAG_MONTHS));
    assert_eq!(f["window"]["ytdLabel"].as_str(), Some(CAS_YTD_LABEL));
    assert_eq!(f["window"]["source"].as_str(), Some(CAS_SOURCE));
}

#[test]
fn crash_tuples_match() {
    let j = load_json("expected/safety-cas.json");
    let mut n = 0;
    for set in j["tuples"].as_array().unwrap() {
        let label = set["label"].as_str().unwrap();
        let file = format!("cas-{label}.json");
        let feats: Vec<JsonValue> = features_of(&file).into_iter().filter(|f| f.get("geometry").is_some_and(JsonValue::truthy)).collect();
        let exp = set["tuples"].as_array().unwrap();
        assert_eq!(feats.len(), exp.len(), "{label}: features with a position");
        for (i, f) in feats.iter().enumerate() {
            assert_close(&Value::Array(cas_tuple(f)), &exp[i], &format!("$.tuples[{label}][{i}]"));
            n += 1;
        }
    }
    assert!(n > 300);
}

#[test]
fn title_rule_matches() {
    assert_eq!(title("M\u{100}NUKA ROAD"), "M\u{101}Nuka Road");
    assert_eq!(title("st mary's avenue"), "St Mary'S Avenue");
    assert_eq!(title("2nd avenue"), "2nd Avenue");
    assert_eq!(title("a_b road"), "A_b Road");
}

#[test]
fn loads_match() {
    let j = load_json("expected/safety-cas.json");
    let loads = j["loads"].as_array().unwrap();
    assert!(loads.len() >= 16);
    for (i, rec) in loads.iter().enumerate() {
        let name = rec["project"].as_str().unwrap();
        let label = rec["label"].as_str().unwrap();
        let mut p = base_project(name);
        let (msgs, statuses) = replay_load(&mut p, name, rec);
        let at = format!("$.loads[{i}] {name} {label}");
        assert_eq!(json!(msgs), rec["msgs"], "{at} messages");
        assert_eq!(json!(statuses), rec["statuses"], "{at} status lines");
        // what the load left in the project
        let d = p.data.get().expect("data");
        let mut got = serde_json::Map::new();
        for k in ["crashes", "crashesLong", "fc"] {
            if let Some(v) = d.extra.get(k) {
                got.insert(k.to_string(), v.clone());
            }
        }
        got.insert("loaded".to_string(), serde_json::to_value(d.loaded.get().expect("loaded")).unwrap());
        assert_close(&Value::Object(got), &rec["data"], &format!("{at} data"));
        // Future Connect on the links of every scenario
        let fcs: serde_json::Map<String, Value> = p
            .scenarios
            .iter()
            .map(|s| {
                let m: serde_json::Map<String, Value> = s.links.values().filter_map(|l| l.fc.get().map(|f| (l.id.clone(), f.clone()))).collect();
                (s.id.clone(), Value::Object(m))
            })
            .collect();
        assert_close(&Value::Object(fcs), &rec["fcLinks"], &format!("{at} fcLinks"));
    }
}

#[test]
fn bands_match() {
    let j = load_json("expected/safety-bands.json");
    for (i, rec) in j["risk"].as_array().unwrap().iter().enumerate() {
        let v = match &rec["value"] {
            Value::Number(n) => Some(n.as_f64().unwrap()),
            Value::String(s) if s == "NaN" => Some(f64::NAN),
            Value::String(s) if s == "Infinity" => Some(f64::INFINITY),
            Value::String(s) if s == "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        };
        let kind = rec["kind"].as_str().unwrap();
        // an undefined kind reads as collective
        let got = risk_band(v, if kind == "undefined" { "collective" } else { kind });
        assert_close(&serde_json::to_value(&got).unwrap(), &rec["result"], &format!("$.risk[{i}] {} {kind}", rec["value"]));
    }
    for (i, rec) in j["link"].as_array().unwrap().iter().enumerate() {
        let kind = rec["kind"].as_str().unwrap();
        let r = &rec["r"];
        let risk = (!r.is_null()).then(|| LinkRisk {
            t: Tally::default(),
            km: 1.0,
            road_km: 1.0,
            divided: false,
            years: Some(5.0),
            aadt: r["aadt"].as_f64(),
            aadt_src: None,
            per_km_yr: r["perKmYr"].as_f64(),
            dsi_per_km_yr: r["dsiPerKmYr"].as_f64(),
            personal: r["personal"].as_f64(),
            short: r["short"].as_bool().unwrap(),
            low_volume: r["lowVolume"].as_bool().unwrap(),
        });
        let got = link_band(risk.as_ref(), if kind == "undefined" { "collective" } else { kind });
        assert_close(&serde_json::to_value(&got).unwrap(), &rec["result"], &format!("$.link[{i}] {kind}"));
    }
}

#[test]
fn conflation_matches() {
    let j = load_json("expected/safety-conflate.json");
    let cases = j["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    for (i, rec) in cases.iter().enumerate() {
        let name = rec["project"].as_str().unwrap();
        let mut p = base_project(name);
        let sc_idx = rec["scIndex"].as_u64().unwrap_or(0) as usize;
        let feats = parse_future_connect(&features_of(&format!("fc-{name}.json")));
        let sc = &mut p.scenarios[sc_idx];
        assert_eq!(sc.id, rec["scenario"].as_str().unwrap());
        // the links as the recorded run had them: their saved fc and status
        for (id, fc) in rec["input"]["links"].as_object().unwrap() {
            sc.links.get_mut(id).expect("link").fc = if fc.is_null() { Maybe::Missing } else { Maybe::Set(fc.clone()) };
        }
        for (id, st) in rec["input"]["statuses"].as_object().unwrap() {
            sc.links.get_mut(id).expect("link").status = st.as_str().map_or(Maybe::Missing, |s| Maybe::Set(s.to_string()));
        }
        let (radius, bearing) = (rec["opts"]["radius"].as_f64(), rec["opts"]["bearing"].as_f64());
        let now = 1_792_540_800_000.0; // 2026-10-21T00:00:00Z, the recorded run's clock
        let mut got = conflate_fc(sc, &feats, radius, bearing, now);
        if rec["setup"].as_str() == Some("twice") {
            got = conflate_fc(sc, &feats, radius, bearing, now);
        }
        let at = format!("$.cases[{i}] {name} {}", rec["label"].as_str().unwrap());
        assert_close(&serde_json::to_value(&got).unwrap(), &rec["result"], &format!("{at} result"));
        let fcs: serde_json::Map<String, Value> = sc.links.values().map(|l| (l.id.clone(), l.fc.get().cloned().unwrap_or(Value::Null))).collect();
        assert_close(&Value::Object(fcs), &rec["fcs"], &format!("{at} fcs"));
    }
}

fn risk_file(name: &str, expect_variants: usize) {
    let j = load_json(&format!("expected/safety-risk-{name}.json"));
    let variants = j["variants"].as_array().unwrap();
    assert_eq!(variants.len(), expect_variants);
    let mut checks = 0;
    for v in variants {
        let label = v["label"].as_str().unwrap();
        let p = construct_transport::project::Project::from_json(v["projectText"].as_str().unwrap()).expect("project reads");
        for (si, rec) in v["scenarios"].as_array().unwrap().iter().enumerate() {
            let sc = &p.scenarios[si];
            assert_eq!(sc.id, rec["id"].as_str().unwrap());
            let at = format!("$.{name}.{label}.{}", sc.id);
            let model = SafetyModel::new(&p, sc);
            let Some(m) = model else {
                // no crashes loaded: the prototype answers null for all of them
                assert!(rec["linkRisk"].is_null() && rec["nodeRisk"].is_null() && rec["summary"].is_null(), "{at}: null answers");
                assert!(crashes_near(&p, sc, Near::Link(sc.links.keys().next().unwrap()), 25.0, false).is_none());
                checks += 1;
                continue;
            };
            assert_close(&serde_json::to_value(m.link_risk(None)).unwrap(), &rec["linkRisk"], &format!("{at}.linkRisk"));
            assert_close(&serde_json::to_value(m.node_risk()).unwrap(), &rec["nodeRisk"], &format!("{at}.nodeRisk"));
            assert_close(&serde_json::to_value(m.summary(None)).unwrap(), &rec["summary"], &format!("{at}.summary"));
            let junctions: serde_json::Map<String, Value> =
                sc.nodes.keys().map(|id| (id.to_string(), m.junction_of(id).map_or(Value::Null, |j| json!(j)))).collect();
            assert_close(&Value::Object(junctions), &rec["junctionOf"], &format!("{at}.junctionOf"));
            if !rec["linkRiskAM"].is_null() {
                let am = stored_res(sc, "AM").expect("AM result");
                let pm = stored_res(sc, "PM").expect("PM result");
                assert_close(&serde_json::to_value(m.link_risk(Some(&am))).unwrap(), &rec["linkRiskAM"], &format!("{at}.linkRiskAM"));
                assert_close(&serde_json::to_value(m.summary(Some(&pm))).unwrap(), &rec["summaryPM"], &format!("{at}.summaryPM"));
                checks += 2;
            }
            if let Some(near) = rec["near"].as_array() {
                for (k, n) in near.iter().enumerate() {
                    let id = n["id"].as_str().unwrap();
                    let sel = match n["type"].as_str().unwrap() {
                        "node" => Near::Node(id),
                        "link" => Near::Link(id),
                        _ => Near::Other,
                    };
                    let got = crashes_near(&p, sc, sel, n["R"].as_f64().unwrap(), false);
                    assert_close(&got.map_or(Value::Null, |g| serde_json::to_value(g).unwrap()), &n["result"], &format!("{at}.near[{k}]"));
                    checks += 1;
                }
                let link = sc.links.keys().nth(3).unwrap();
                let got = crashes_near(&p, sc, Near::Link(link), 25.0, false).unwrap();
                assert_close(&serde_json::to_value(got).unwrap(), &rec["nearDefault"], &format!("{at}.nearDefault"));
            }
            if let Some(long) = rec["nearLong"].as_array() {
                for (k, n) in long.iter().enumerate() {
                    let got = crashes_near(&p, sc, Near::Link(n["id"].as_str().unwrap()), 25.0, true);
                    assert_close(&got.map_or(Value::Null, |g| serde_json::to_value(g).unwrap()), &n["result"], &format!("{at}.nearLong[{k}]"));
                    checks += 1;
                }
            }
            checks += 4;
        }
    }
    assert!(checks > 100, "{name}: {checks} checks");
}

#[test]
fn corridor_risk_matches() {
    risk_file("corridor", 12);
}

#[test]
fn grid_risk_matches() {
    risk_file("grid", 12);
}
