//! NZTA daily counts against the JavaScript's recorded runs: the public holiday and school term calendar, the statistics of a site
//! from its rows (weekday means, targets, growth, dropped days), choosing the sites, the whole download (paging, the days to ask for,
//! every failure) and the peak profile file, with what each puts on the counts.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-tms.json, fixtures/data/tms-rows.json, tms-profiles.json).

mod feeds_support;
mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

#[test]
fn holidays_and_school_terms_match() {
    let e = expected("tms");
    let hs = e["holidays"].as_array().unwrap();
    assert_eq!(hs.len(), 138);
    for h in hs {
        let region = match &h["region"] {
            Value::String(s) => s.as_str(),
            _ => "Auckland",
        };
        let got: Vec<Value> = holidays(h["year"].as_i64().unwrap(), region)
            .into_iter()
            .map(|d| json!(d))
            .collect();
        assert_close(
            &Value::Array(got),
            &h["days"],
            &format!("holidays of {} in {:?}", h["year"], h["region"]),
        );
    }
    let ds = e["dates"].as_array().unwrap();
    assert_eq!(ds.len(), 68);
    for d in ds {
        let n = match d["iso"].as_str() {
            Some(iso) => {
                assert_eq!(dnum(iso), d["dnum"].as_f64().unwrap(), "dnum of {iso}");
                dnum(iso)
            }
            None => d["n"].as_f64().unwrap(),
        };
        assert_eq!(diso(n), d["diso"].as_str().unwrap(), "diso of {n}");
        assert_eq!(
            in_term(n).map_or(Value::Null, Value::Bool),
            d["inTerm"],
            "in term on {n}"
        );
    }
}

// What the prototype's `F()` shows: it makes the feeds record (with an empty vintage table) the first time anything looks at it.
fn feeds_of_run(p: &construct_transport::project::Project) -> Value {
    p.extra
        .get("feeds")
        .cloned()
        .unwrap_or_else(|| json!({ "vintage": {} }))
}

fn site_rows(file_ref: &str) -> Vec<JsonValue> {
    fixture_features("tms-rows.json")
        .into_iter()
        .filter_map(|f| {
            let a = f.get("attributes")?.clone();
            let r = ["SiteRef", "siteref", "SITEREF"]
                .iter()
                .find_map(|k| a.get(k).filter(|v| !v.is_null()))?;
            (r.js_string() == file_ref).then_some(a)
        })
        .collect()
}

#[test]
fn site_statistics_match() {
    let e = expected("tms");
    let cases = e["stats"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    for rec in cases {
        let label = rec["label"].as_str().unwrap();
        let rows: Vec<JsonValue> = match rec["rows"].get("fixtureSite") {
            Some(s) => site_rows(s.as_str().unwrap()),
            None => rec["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(ordered)
                .collect(),
        };
        let o = &rec["opts"];
        let hist: Option<Vec<JsonValue>> = o
            .get("hist")
            .map(|h| h.as_array().unwrap().iter().map(ordered).collect());
        let region = o
            .get("region")
            .and_then(Value::as_str)
            .unwrap_or("Auckland");
        let now = o["now"].as_f64().unwrap();
        let got = tms_stats(&rows, &o["ref"], region, hist.as_deref(), now, NOW);
        match (&got, rec.get("thrown")) {
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (Ok(r), None) => assert_close(
                &serde_json::to_value(r).unwrap(),
                &rec["result"],
                &format!("stats {label}"),
            ),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
    }
}

#[test]
fn count_sites_are_chosen_as_recorded() {
    let e = expected("tms");
    let mut p = project("corridor");
    construct_transport::datahub::ensure_data(&mut p);
    let counts = fixture_counts(&e);
    p.data
        .get_mut()
        .unwrap()
        .extra
        .insert("counts".to_string(), counts);
    let ids: Vec<String> = tms_sites(&p).into_iter().map(|s| s.reference).collect();
    let want: Vec<String> = e["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().trim_start_matches("nzta:").to_string())
        .collect();
    assert_eq!(ids, want);
}

// the counts the generator starts the first download from
fn fixture_counts(e: &Value) -> Value {
    e["loads"][0]["startCounts"].clone()
}

#[test]
fn downloads_match() {
    let e = expected("tms");
    let loads = e["loads"].as_array().unwrap();
    assert_eq!(loads.len(), 14);
    for rec in loads {
        let label = rec["label"].as_str().unwrap();
        let mut p = project("corridor");
        construct_transport::datahub::ensure_data(&mut p);
        p.data
            .get_mut()
            .unwrap()
            .extra
            .insert("counts".to_string(), rec["startCounts"].clone());
        if let Some(f) = rec["startFeeds"].as_object() {
            p.settings
                .extra
                .insert("feeds".to_string(), Value::Object(f.clone()));
        }
        let cfg = FeedsConfig::of(&p);
        let days = if rec["days"].get("undefined").is_some() {
            cfg.tms_days.clone()
        } else {
            rec["days"].clone()
        };
        let mut load = TmsLoad::new(tms_sites(&p), &days, NOW, cfg.limit_ms());
        let log = rec["requests"].as_array().unwrap();
        let (used, res) = replay(&mut load, log, 0, label);
        all_used(log, used, label);
        let statuses: Vec<String> = load.statuses().to_vec();
        assert_eq!(json!(statuses), rec["statuses"], "{label}: status lines");
        let result = match res {
            Err(m) => Err(m),
            Ok(()) => load.finish(&mut p),
        };
        match (&result, rec.get("thrown")) {
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (Ok(r), None) => assert_close(
                &serde_json::to_value(r).unwrap(),
                &rec["result"],
                &format!("{label} result"),
            ),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
        assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
        assert_close(
            &p.data.get().unwrap().extra["counts"],
            &rec["counts"],
            &format!("{label} counts"),
        );
        let mut fresh = project_of_text(&p.serialise());
        assert_eq!(
            attach_tms(&mut fresh) as u64,
            rec["attachFresh"].as_u64().unwrap(),
            "{label}: counts changed when the project is reopened"
        );
        assert_close(
            &fresh.data.get().unwrap().extra["counts"],
            &rec["countsFresh"],
            &format!("{label} counts after reopening"),
        );
    }
}

#[test]
fn peak_profiles_match() {
    let e = expected("tms");
    let recs = e["profiles"].as_array().unwrap();
    assert_eq!(recs.len(), 5);
    for rec in recs {
        let label = rec["label"].as_str().unwrap();
        let mut p = project("corridor");
        construct_transport::datahub::ensure_data(&mut p);
        p.data
            .get_mut()
            .unwrap()
            .extra
            .insert("counts".to_string(), e["loads"][0]["startCounts"].clone());
        p.extra.insert(
            "feeds".to_string(),
            json!({ "vintage": {}, "tms": {
                "01N00201": { "target": { "mean": 31000, "n": 30, "ok": true } },
                "02E00310": { "target": { "mean": 11000, "n": 3, "ok": false } },
                "03S00099": { "target": null } } }),
        );
        let mut get: GetOnce = profiles_request();
        let log = rec["requests"].as_array().unwrap();
        let (used, res) = replay(&mut get, log, 0, label);
        all_used(log, used, label);
        let result = match res {
            Err(m) => Err(m),
            Ok(()) => match get.into_answer() {
                Some(Answer::Json(j)) => apply_profiles(&mut p, &j, NOW),
                other => panic!("{label}: answered {other:?}"),
            },
        };
        match (&result, rec.get("thrown")) {
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (Ok(n), None) => assert_eq!(
                *n as u64,
                rec["n"].as_u64().unwrap(),
                "{label}: counts changed"
            ),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
        assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
        assert_close(
            &p.data.get().unwrap().extra["counts"],
            &rec["counts"],
            &format!("{label} counts"),
        );
        let mut fresh = project_of_text(&p.serialise());
        assert_eq!(
            attach_tms(&mut fresh) as u64,
            rec["attachFresh"].as_u64().unwrap(),
            "{label}: counts changed when the project is reopened"
        );
        assert_close(
            &fresh.data.get().unwrap().extra["counts"],
            &rec["countsFresh"],
            &format!("{label} counts after reopening"),
        );
    }
}
