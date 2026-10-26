//! The Motor Vehicle Register fleet (the download, results kept for 90 days, the CO2 factor made from it) and the live traffic
//! services (TomTom speeds and map tiles, Google drive times, the next Tuesday to Thursday in New Zealand) against the JavaScript's
//! recorded runs. Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-fleet.json and feeds-live.json).

mod feeds_support;
mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::*;
use construct_transport::project::Project;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

fn feeds_of_run(p: &Project) -> Value {
    p.extra.get("feeds").cloned().unwrap_or_else(|| json!({ "vintage": {} }))
}

#[test]
fn fleet_downloads_match() {
    let e = expected("fleet");
    let runs = e["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 16);
    for rec in runs.iter().filter(|r| r["label"] != "kept") {
        let label = rec["label"].as_str().unwrap();
        let mut p = project("corridor");
        let district = match fleet_district(rec["district"].as_str().unwrap()) {
            Ok(d) => d,
            Err(m) => {
                assert_eq!(m, rec["thrown"].as_str().unwrap(), "{label}: error");
                assert!(rec["requests"].as_array().unwrap().is_empty());
                continue;
            }
        };
        let mut load = FleetLoad::new(&district, rec["now"].as_f64().unwrap(), 30_000.0, &MetaCache::new());
        let log = rec["requests"].as_array().unwrap();
        let (used, res) = replay(&mut load, log, 0, label);
        all_used(log, used, label);
        let fleet = match res {
            Err(m) => {
                assert_eq!(m, rec["thrown"].as_str().unwrap_or_else(|| panic!("{label}: unexpected failure {m}")), "{label}: error");
                assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
                continue;
            }
            Ok(()) => load.finish().unwrap_or_else(|| panic!("{label}: no fleet, recorded {:?}", rec.get("thrown"))),
        };
        assert!(rec.get("thrown").is_none(), "{label}: the recording failed");
        assert_close(&fleet.to_value(), &rec["result"], &format!("{label} result"));
        let order: Vec<&str> = fleet.shares.iter().map(|(k, _)| k.as_str()).collect();
        let want: Vec<&str> = rec["shareOrder"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert_eq!(order, want, "{label}: order of the shares");
        apply_fleet(&mut p, &fleet, rec["now"].as_f64().unwrap());
        assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
    }
}

#[test]
fn kept_results_are_used_for_90_days() {
    let e = expected("fleet");
    let rec = e["runs"].as_array().unwrap().iter().find(|r| r["label"] == "kept").unwrap();
    let log = rec["requests"].as_array().unwrap();
    let mut p = project("corridor");
    let cache = MetaCache::new();
    let mut kept: Option<Value> = None;
    let mut used = 0;
    for step in rec["steps"].as_array().unwrap() {
        let days = step["days"].as_f64().unwrap();
        let now = NOW + days * 86_400_000.0;
        let label = format!("after {days} days");
        let before = used;
        let (result, cached) = match kept.as_ref().and_then(|k| use_cached_fleet(&mut p, k, now)) {
            Some(r) => (r, true),
            None => {
                let mut load = FleetLoad::new("AUCKLAND", now, 30_000.0, &cache);
                let (u, res) = replay(&mut load, log, used, &label);
                res.expect("the download");
                used = u;
                let fleet = load.finish().expect("a fleet");
                // the vintage table is stamped with the clock of the run, not the date the register was asked for
                apply_fleet(&mut p, &fleet, NOW);
                let v = fleet.to_value();
                kept = Some(v.clone());
                (v, false)
            }
        };
        assert_eq!(cached, step["cached"].as_bool().unwrap(), "{label}: from the kept result");
        assert_eq!((used - before) as u64, step["requests"].as_u64().unwrap(), "{label}: requests made");
        // the prototype marks a kept result as such when it gives it back
        let mut result = result;
        if cached {
            result["cached"] = json!(true);
        }
        assert_close(&result, &step["result"], &format!("{label} result"));
        assert_close(&p.extra["feeds"]["vintage"]["mvr"], &step["vintage"], &format!("{label} vintage"));
    }
    all_used(log, used, "kept results");
    let text: Value = serde_json::from_str(rec["keptText"].as_str().unwrap()).unwrap();
    assert_close(kept.as_ref().unwrap(), &text, "the result kept at the end");
}

#[test]
fn co2_from_the_fleet_matches() {
    let eo = expected_ordered("fleet");
    let recs = eo.get("co2").unwrap().as_array().unwrap();
    assert_eq!(recs.len(), 9);
    for rec in recs {
        let label = rec.get("label").unwrap().js_string();
        let shares: Vec<(String, f64)> = match rec.get("shares") {
            Some(JsonValue::Object(o)) => o.iter().map(|(k, v)| (k.to_string(), v.to_number())).collect(),
            _ => panic!("{label}: shares"),
        };
        let mut p = project("corridor");
        let (factor, text) = fleet_co2("AUCKLAND", "MVR_Oct26", &shares);
        apply_fleet_co2(&mut p, factor, &text);
        let want = rec.to_value();
        assert_close(&p.settings.co2_factor.get().map_or(Value::Null, |v| v.to_value()), &want["co2Factor"], &format!("{label} factor"));
        assert_close(&p.settings.extra.get("co2FactorSrc").cloned().unwrap_or(Value::Null), &want["co2FactorSrc"], &format!("{label} source text"));
        clear_fleet_co2(&mut p);
        assert!(p.settings.co2_factor.get().is_none() && !p.settings.extra.contains_key("co2FactorSrc"), "{label}: cleared");
        assert_eq!(want["afterOff"]["has"], json!(false));
    }
}

// `setKey` keeps a key trimmed
const TOMTOM_KEY: &str = "ab c/1&2?#\u{e9}";

#[test]
fn map_tiles_and_current_speeds_match() {
    let e = nan_as_null(&expected("live"));
    for t in e["tiles"].as_array().unwrap() {
        let key = t["key"].as_str().map(construct_transport::jsnum::js_trim);
        assert_eq!(tomtom_tiles(key), t["url"].as_str().map(str::to_string), "tiles for {}", t["key"]);
    }
    let recs = e["speeds"].as_array().unwrap();
    assert_eq!(recs.len(), 32);
    for rec in recs {
        let label = format!("{} at {}", rec["label"], rec["ll"]);
        let ll = [rec["ll"][0].as_f64().unwrap(), rec["ll"][1].as_f64().unwrap()];
        let key = if rec["label"] == "no-key" { None } else { Some(TOMTOM_KEY) };
        let req = match tomtom_speed_request(ll, key) {
            Err(m) => {
                assert_eq!(m, rec["thrown"].as_str().unwrap(), "{label}: error");
                assert!(rec["requests"].as_array().unwrap().is_empty());
                continue;
            }
            Ok(r) => r,
        };
        assert_eq!(req.method, "GET");
        assert_eq!(req.url, rec["requests"][0]["url"].as_str().unwrap(), "{label}: address");
        let body = |v: Value| JsonValue::from_value(&v);
        let answer: Result<JsonValue, FetchFailure> = match rec["label"].as_str().unwrap() {
            "ok" => Ok(body(json!({ "flowSegmentData": { "currentSpeed": 38, "freeFlowSpeed": 52, "confidence": 0.93, "roadClosure": false, "frc": "FRC3" } }))),
            "ok-sparse" => Ok(body(json!({ "flowSegmentData": { "currentSpeed": 38 } }))),
            "403" => Err(FetchFailure::Http(403)),
            "401" => Err(FetchFailure::Http(401)),
            "500" => Err(FetchFailure::Http(500)),
            "no-segment" => Ok(body(json!({}))),
            "network" => Err(FetchFailure::Unreachable("Failed to fetch".to_string())),
            other => panic!("unknown answer {other}"),
        };
        match (tomtom_speed_result(answer.as_ref().map_err(Clone::clone), NOW), rec.get("thrown")) {
            (Ok(r), None) => assert_close(&serde_json::to_value(&r).unwrap(), &rec["result"], &format!("{label} result")),
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
    }
}

#[test]
fn google_drive_times_match() {
    let e = nan_as_null(&expected("live"));
    let recs = e["google"].as_array().unwrap();
    assert_eq!(recs.len(), 144);
    for rec in recs {
        let label = format!("{} {} {}", rec["label"], rec["nowLabel"], rec["period"]);
        let now = rec["now"].as_f64().unwrap();
        let period = rec["period"].as_str().unwrap_or("undefined");
        let key = if rec["label"] == "no-key" { None } else { Some("gkey-123") };
        let (req, when) = match google_request([-37.1, 174.95], [-37.2, 175.01234567], period, key, now) {
            Err(m) => {
                assert_eq!(m, rec["thrown"].as_str().unwrap(), "{label}: error");
                assert!(rec["requests"].as_array().unwrap().is_empty());
                continue;
            }
            Ok(r) => r,
        };
        check_request(&req, &rec["requests"][0], &label);
        let body = |v: Value| JsonValue::from_value(&v);
        let (status, ok, b) = match rec["label"].as_str().unwrap() {
            "ok" => (200, true, body(json!({ "routes": [{ "duration": "512s", "staticDuration": "388s", "distanceMeters": 4821 }] }))),
            "float" => (200, true, body(json!({ "routes": [{ "duration": "51.25s", "staticDuration": "38s", "distanceMeters": 100 }] }))),
            "no-routes" => (200, true, body(json!({ "routes": [] }))),
            "no-routes-key" => (200, true, body(json!({}))),
            "error-body" => (200, true, body(json!({ "error": { "message": "API key not valid" } }))),
            "error-status" => (403, false, body(json!({ "error": { "message": "Forbidden here" } }))),
            "error-no-message" => (500, false, body(json!({ "error": {} }))),
            // an unreadable body is read as an empty object
            "bad-json" => (502, false, body(json!({}))),
            "not-ok-no-error" => (429, false, body(json!({}))),
            "odd-duration" => (200, true, body(json!({ "routes": [{ "duration": "abc", "staticDuration": 12, "distanceMeters": null }] }))),
            "network" => {
                // the request never gets an answer: the host reports the network's own message
                assert_eq!(rec["thrown"], "Failed to fetch");
                continue;
            }
            other => panic!("unknown answer {other}"),
        };
        match (google_result(status, ok, &b, &when, now), rec.get("thrown")) {
            (Ok(r), None) => assert_close(&serde_json::to_value(&r).unwrap(), &rec["result"], &format!("{label} result")),
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
    }
}

#[test]
fn next_tuesday_to_thursday_matches() {
    let e = nan_as_null(&expected("live"));
    let recs = e["weekdays"].as_array().unwrap();
    assert_eq!(recs.len(), 240);
    for rec in recs {
        let now = rec["now"].as_f64().unwrap();
        for (k, hour) in [("am", 8.0), ("pm", 17.0), ("noon", 12.0), ("midnight", 0.0), ("late", 23.0)] {
            let got = next_weekday_nz(now, hour).map_or(Value::Null, |(iso, label)| json!({ "iso": iso, "label": label }));
            assert_eq!(got, rec[k], "{k} from {now}");
        }
    }
}
