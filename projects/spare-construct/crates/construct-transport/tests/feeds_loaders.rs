//! The data hub's loaders against the JavaScript's recorded runs: NZTA count sites, Auckland Transport counts and controlled
//! intersections loaded into a project (paging, the counts a reload keeps switched off, the messages), and every way a source can
//! fail (HTTP errors, no connection, unreadable or error answers, a time limit, the user cancelling, records that cannot be read).
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-loaders.json, fixtures/data/nzta-sites.json, at-counts.json, at-controlled.json).

mod feeds_support;
mod support;

use construct_transport::datahub::ensure_data;
use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

#[test]
fn recorded_loads_match() {
    let e = expected("loaders");
    let cases = e["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 28);
    for (n, rec) in cases.iter().enumerate() {
        let label = format!("{n} {}", rec["label"].as_str().unwrap());
        let mut p = project("corridor");
        if let Some(start) = rec["startCounts"].as_array() {
            ensure_data(&mut p);
            p.data
                .get_mut()
                .unwrap()
                .extra
                .insert("counts".to_string(), Value::Array(start.clone()));
        }
        let which: Vec<&str> = match rec["which"].as_array() {
            Some(w) => w.iter().map(|x| x.as_str().unwrap()).collect(),
            None => vec!["nzta", "at", "cas"],
        };
        let opts = LoadOptions {
            cas_years: None,
            timeout_ms: rec["opts"]["timeout"].as_f64(),
        };
        let mut load = LoadAll::new(bounds_of(&rec["bounds"]), &which, &opts, NOW);
        if rec["opts"]["cancel"].as_f64() == Some(0.0) {
            load.cancel();
        }
        let log = rec["requests"].as_array().unwrap();
        let mut k = 0;
        let mut thrown: Option<String> = None;
        loop {
            match load.next_request(&mut p) {
                Err(e) => {
                    thrown = Some(e.message().to_string());
                    break;
                }
                Ok(None) => break,
                Ok(Some(req)) => {
                    let r = log
                        .get(k)
                        .unwrap_or_else(|| panic!("{label}: unrecorded request {}", req.url));
                    check_request(&req, r, &format!("{label} request {k}"));
                    k += 1;
                    let ans = answer_of(&r["answer"]).map(|a| match a {
                        Answer::Json(j) => j,
                        other => panic!("{label}: a layer answered {other:?}"),
                    });
                    if let Err(e) = load.answer(&mut p, ans) {
                        thrown = Some(e.message().to_string());
                        break;
                    }
                }
            }
        }
        all_used(log, k, &label);
        assert_eq!(
            thrown.as_deref(),
            rec["thrown"].as_str(),
            "{label}: how it ended"
        );
        if rec["thrown"].is_null() {
            assert_eq!(json!(load.messages()), rec["msgs"], "{label}: messages");
        }
        assert_eq!(
            json!(load.statuses()),
            rec["statuses"],
            "{label}: status lines"
        );
        let d = p.data.get().expect("data");
        let mut got = serde_json::Map::new();
        for key in ["counts", "crashes"] {
            if let Some(v) = d.extra.get(key) {
                got.insert(key.to_string(), v.clone());
            }
        }
        if let Some(c) = d.controlled.get() {
            got.insert("controlled".to_string(), c.clone());
        }
        got.insert(
            "loaded".to_string(),
            serde_json::to_value(d.loaded.get().expect("loaded")).unwrap(),
        );
        assert_close(
            &Value::Object(got),
            &rec["data"],
            &format!("$.cases[{label}].data"),
        );
    }
}

#[test]
fn arcgis_requests_are_built_as_the_prototype_builds_them() {
    let b = Bounds {
        south: -37.123456789,
        west: 174.5,
        north: -37.0,
        east: 174.999999999,
    };
    let pager = ArcgisPager::new(NZTA_SITES, &b, "1=1", "*", ARCGIS_TIMEOUT_MS);
    let r = pager.next_request().unwrap();
    assert_eq!(
        r.url,
        format!(
            "{NZTA_SITES}/query?where=1%3D1&geometry=174.500000%2C-37.123457%2C175.000000%2C-37.000000&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&outFields=*&outSR=4326&f=json&resultOffset=0&orderByFields=OBJECTID"
        )
    );
    assert_eq!(r.timeout_ms, 30_000.0);
    // the way text is encoded in a query
    assert_eq!(
        form_encode("a b+c&d=e/f?g#h\u{101}~*-._!'()"),
        "a+b%2Bc%26d%3De%2Ff%3Fg%23h%C4%81%7E*-._%21%27%28%29"
    );
    assert_eq!(
        encode_uri_component("a b+c&d=e/f?g#h\u{101}~*-._!'()"),
        "a%20b%2Bc%26d%3De%2Ff%3Fg%23h%C4%81~*-._!'()"
    );
    assert_eq!(
        host_of_url("https://Services.ArcGIS.com:443/x/y?z#w").as_deref(),
        Some("services.arcgis.com")
    );
    assert_eq!(
        host_of_url("http://a.example:8080/x").as_deref(),
        Some("a.example:8080")
    );
    assert_eq!(host_of_url("data/tms_profiles.json"), None);
    assert_eq!(host_text("data/tms_profiles.json"), "this site");
    assert_eq!(host_text(""), "");
}
