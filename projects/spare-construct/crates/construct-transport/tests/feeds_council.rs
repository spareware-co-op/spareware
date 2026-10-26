//! Council count layers against the JavaScript's recorded runs: cleaning the address a user pastes, guessing which field holds what,
//! reading a layer (and every way that fails), and importing its counts as the user's own (the fields guessed or chosen, the
//! presets that drop old counts, replacing what an earlier import of the layer put in the project).
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-council.json, fixtures/data/council-*.json).

mod feeds_support;
mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

fn text_of(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

#[test]
fn pasted_addresses_are_cleaned_as_recorded() {
    let e = expected("council");
    let recs = e["urls"].as_array().unwrap();
    assert_eq!(recs.len(), 21);
    for rec in recs {
        let url = text_of(&rec["url"]);
        match (council_url(&url), rec.get("thrown")) {
            (Ok(u), None) => {
                assert_eq!(json!(u), rec["cleaned"], "cleaned {url:?}");
                let mut read = CouncilRead::new(&url, 30_000.0, &MetaCache::new()).unwrap();
                let req = read.next_request().expect("a request");
                assert_eq!(req.url, rec["requests"][0]["url"].as_str().unwrap(), "request for {url:?}");
            }
            (Err(m), Some(t)) => {
                assert_eq!(m, t.as_str().unwrap(), "error for {url:?}");
                assert!(rec["requests"].as_array().unwrap().is_empty());
            }
            (g, t) => panic!("{url:?}: got {g:?}, recorded {t:?}"),
        }
    }
}

#[test]
fn guessed_fields_match() {
    let e = expected_ordered("council");
    let recs = e.get("guesses").unwrap().as_array().unwrap();
    assert_eq!(recs.len(), 6);
    for rec in recs {
        let got = serde_json::to_value(guess_fields(rec.get("meta").unwrap())).unwrap();
        assert_close(&got, &rec.get("guess").unwrap().to_value(), &format!("guess for {}", rec.get("label").unwrap().js_string()));
    }
}

// the layer description each of the recorded layers answers
fn meta_file(url: &str) -> &'static str {
    if url.contains("Hamilton") {
        "council-meta-ham.json"
    } else if url.contains("Latest_Counts") {
        "council-meta-qldc.json"
    } else {
        "council-meta-gen.json"
    }
}

fn read_layer(url: &str) -> CouncilLayer {
    let mut read = CouncilRead::new(url, 30_000.0, &MetaCache::new()).expect("an address");
    while let Some(req) = read.next_request() {
        assert_eq!(req.url, format!("{}?f=json", council_url(url).unwrap()));
        read.answer(Ok(Answer::Json(fixture_json(meta_file(url))))).expect("answered");
    }
    read.finish().expect("a layer")
}

#[test]
fn layers_are_read_as_recorded() {
    let e = expected("council");
    let recs = e["reads"].as_array().unwrap();
    assert_eq!(recs.len(), 12);
    for rec in recs {
        let url = rec["url"].as_str().unwrap();
        let label = format!("{} {} {}", rec["project"], url, rec["label"]);
        if let Some(t) = rec.get("thrown") {
            let mut read = CouncilRead::new(url, 30_000.0, &MetaCache::new()).unwrap();
            let log = rec["requests"].as_array().unwrap();
            let (used, res) = replay(&mut read, log, 0, &label);
            all_used(log, used, &label);
            let err = match res {
                Err(m) => m,
                Ok(()) => read.finish().expect_err("not a layer"),
            };
            assert_eq!(err, t.as_str().unwrap(), "{label}: error");
            continue;
        }
        let layer = read_layer(url);
        let got = json!({ "url": layer.url, "name": layer.name, "map": serde_json::to_value(&layer.map).unwrap(), "fields": layer.fields });
        assert_close(&got, &rec["result"], &format!("{label} result"));
    }
}

#[test]
fn imports_match() {
    let e = expected("council");
    let eo = expected_ordered("council");
    let recs = e["imports"].as_array().unwrap();
    let ordered_recs = eo.get("imports").unwrap().as_array().unwrap();
    assert_eq!(recs.len(), 40);
    for (rec, orec) in recs.iter().zip(ordered_recs) {
        let url = rec["url"].as_str().unwrap();
        let label = format!("{} {} {} preset {} at {}", rec["project"], url, rec["mapLabel"], rec["preset"], rec["now"]);
        let now = rec["now"].as_f64().unwrap();
        let layer = read_layer(url);
        let map: JsonValue = orec.get("map").unwrap().clone();
        let mut p = project(rec["project"].as_str().unwrap());
        construct_transport::datahub::ensure_data(&mut p);
        p.data.get_mut().unwrap().extra.insert("counts".to_string(), rec["startCounts"].clone());
        let mut imp = CouncilImport::new(layer, map, &bounds_of(&rec["bounds"]), rec["preset"].as_str(), now, 30_000.0);
        let log = rec["requests"].as_array().unwrap();
        let mut k = 0;
        let mut statuses: Vec<String> = Vec::new();
        while let Some(req) = imp.next_request() {
            let r = log.get(k).unwrap_or_else(|| panic!("{label}: unrecorded request {}", req.url));
            check_request(&req, r, &format!("{label} request {k}"));
            k += 1;
            imp.answer(answer_of(&r["answer"])).expect("a page");
            statuses.push(format!("Downloading council counts\u{2026} {}", imp.count()));
        }
        all_used(log, k, &label);
        assert_eq!(json!(statuses), rec["statuses"], "{label}: status lines");
        let res = imp.finish(&mut p);
        assert_close(&serde_json::to_value(&res).unwrap(), &rec["result"], &format!("{label} result"));
        assert_close(&p.data.get().unwrap().extra["counts"], &rec["counts"], &format!("{label} counts"));
        assert_close(&p.extra.get("feeds").cloned().unwrap_or_else(|| json!({ "vintage": {} })), &rec["feeds"], &format!("{label} feeds"));
    }
}
