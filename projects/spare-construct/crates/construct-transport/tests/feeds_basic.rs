//! The data feeds' groundwork against the JavaScript's recorded answers: New Zealand time and dates (daylight saving rule, the
//! date text, `Date.parse`), the regular expression matcher (many patterns and inputs against JavaScript's own), the sources and
//! thresholds as data, and the data-vintage table.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-time.json, feeds-regex.json, feeds-consts.json, feeds-vintage.json).

mod feeds_support;
mod support;

use construct_transport::datahub::{SOURCES, source};
use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::jsregex::Regex;
use construct_transport::feeds::nztime::*;
use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Map, Value, json};
use support::assert_close;

#[test]
fn new_zealand_daylight_saving_matches_node() {
    let e = expected("time");
    let tr = e["transitions"].as_array().unwrap();
    assert!(tr.len() >= 80, "{} transitions", tr.len());
    for t in tr {
        let at = t["at"].as_f64().unwrap();
        assert_eq!(
            nz_offset_hours(at - 1.0),
            t["before"].as_f64().unwrap(),
            "just before {at}"
        );
        assert_eq!(nz_offset_hours(at), t["after"].as_f64().unwrap(), "at {at}");
    }
    // every day at 00:00 and 12:00 UTC from 1996 to 2040
    let d = &e["dayOffsets"];
    let text: Vec<char> = d["text"].as_str().unwrap().chars().collect();
    let (t0, days) = (
        d["from"].as_f64().unwrap(),
        d["days"].as_u64().unwrap() as usize,
    );
    assert_eq!(text.len(), days * 2);
    for i in 0..days {
        let t = t0 + i as f64 * DAY_MS;
        for (k, at) in [t, t + DAY_MS / 2.0].into_iter().enumerate() {
            let want = if nz_offset_hours(at) == 13.0 {
                'S'
            } else {
                'W'
            };
            assert_eq!(text[i * 2 + k], want, "offset at {at}");
        }
    }
}

#[test]
fn new_zealand_dates_match() {
    let e = expected("time");
    for t in e["today"].as_array().unwrap() {
        let at = t["at"].as_f64().unwrap();
        assert_eq!(today(at), t["date"].as_str().unwrap(), "today at {at}");
    }
    for t in e["localYears"].as_array().unwrap() {
        let at = t["at"].as_f64().unwrap();
        let year = nz_parts(at).year as f64;
        assert_eq!(year, t["year"].as_f64().unwrap(), "year at {at}");
        assert_eq!(
            nz_midnight_ms(year, 0.0, 1.0),
            t["start"].as_f64().unwrap(),
            "start of {year}"
        );
        assert_eq!(
            nz_midnight_ms(year + 1.0, 0.0, 1.0),
            t["next"].as_f64().unwrap(),
            "start of the next year"
        );
    }
    for t in e["local"].as_array().unwrap() {
        let at = t["at"].as_f64().unwrap();
        assert_eq!(day_month_year(at), t["dayMonthYear"].as_str().unwrap());
        assert_eq!(month_year(at), t["monthYear"].as_str().unwrap());
    }
}

fn date_value(v: &Value) -> Option<JsonValue> {
    match v {
        Value::Object(o) if o.contains_key("undefined") => None,
        other => Some(JsonValue::from_value(other)),
    }
}

#[test]
fn date_text_and_parse_match() {
    let e = expected("time");
    for t in e["dateText"].as_array().unwrap() {
        let v = date_value(&t["at"]);
        assert_eq!(
            date_text(v.as_ref()),
            t["text"].as_str().unwrap(),
            "dateText of {}",
            t["at"]
        );
    }
    for t in e["parse"].as_array().unwrap() {
        let got = parse_date(t["text"].as_str().unwrap());
        assert_eq!(got, t["ms"].as_f64(), "Date.parse of {:?}", t["text"]);
    }
}

fn regex_flags(f: &str) -> String {
    f.to_string()
}

#[test]
fn regular_expressions_match_javascript() {
    let e = expected("regex");
    let cases = e["cases"].as_array().unwrap();
    assert!(cases.len() > 5000, "{} cases", cases.len());
    for (n, c) in cases.iter().enumerate() {
        let (pat, flags, input) = (
            c["pattern"].as_str().unwrap(),
            regex_flags(c["flags"].as_str().unwrap()),
            c["input"].as_str().unwrap(),
        );
        let re = Regex::new(pat, &flags).unwrap_or_else(|e| panic!("pattern {pat}: {e}"));
        let units: Vec<u16> = input.encode_utf16().collect();
        let at = format!("case {n}: /{pat}/{flags} on {input:?}");
        let m = re.exec_units(&units, 0);
        match (&m, c["exec"].as_object()) {
            (None, None) => {}
            (Some(m), Some(x)) => {
                assert_eq!(m.start as u64, x["index"].as_u64().unwrap(), "{at}: index");
                let groups = x["groups"].as_array().unwrap();
                assert_eq!(groups.len(), m.groups.len() + 1, "{at}: group count");
                for (i, g) in groups.iter().enumerate() {
                    assert_eq!(m.text(&units, i).as_deref(), g.as_str(), "{at}: group {i}");
                }
            }
            _ => panic!(
                "{at}: match {:?} but JavaScript has {}",
                m.as_ref().map(|m| m.start),
                c["exec"]
            ),
        }
        assert_eq!(re.test(input), c["test"].as_bool().unwrap(), "{at}: test");
        let replaced = re.replace(input, |m, u| {
            let gs: Vec<String> = (1..=m.groups.len())
                .map(|i| m.text(u, i).unwrap_or_default())
                .collect();
            format!("[{}|{}]", m.text(u, 0).unwrap_or_default(), gs.join(","))
        });
        assert_eq!(replaced, c["replace"].as_str().unwrap(), "{at}: replace");
        if let Some(all) = c["matchAll"].as_array() {
            let got = re.matches(&units);
            assert_eq!(got.len(), all.len(), "{at}: match count");
            for (g, x) in got.iter().zip(all) {
                assert_eq!(
                    g.start as u64,
                    x["index"].as_u64().unwrap(),
                    "{at}: match index"
                );
                assert_eq!(
                    g.text(&units, 0).unwrap_or_default(),
                    x["text"].as_str().unwrap(),
                    "{at}: match text"
                );
            }
        }
    }
}

#[test]
fn sources_and_thresholds_match() {
    let e = expected("consts");
    assert_close(&src_value(), &e["SRC"], "$.SRC");
    assert_close(&rules_value(), &e["RULES"], "$.RULES");
    assert_close(&terms_value(), &e["TERMS"], "$.TERMS");
    assert_close(&matariki_value(), &e["MATARIKI"], "$.MATARIKI");
    assert_close(&councils_value(), &e["COUNCILS"], "$.COUNCILS");
    assert_close(&nz_feeds_value(), &e["NZ_FEEDS"], "$.NZ_FEEDS");
    assert_close(&defaults_value(), &e["DEFAULTS"], "$.DEFAULTS");
    assert_eq!(AT_LICENCE, e["AT_LICENCE"].as_str().unwrap());
    assert_eq!(ARCGIS_TIMEOUT_MS, e["ARCGIS_TIMEOUT"].as_f64().unwrap());
    assert_eq!(json!(ROUTE_COLOURS), e["ROUTE_COLOURS"]);
    for (k, s) in e["SOURCES"].as_object().unwrap() {
        let mine = source(k).unwrap_or_else(|| panic!("source {k}"));
        assert_eq!(s["label"].as_str().unwrap(), mine.label);
        assert_eq!(s["short"].as_str().unwrap(), mine.short);
        assert_eq!(s["colour"].as_str().unwrap(), mine.colour);
    }
    assert_eq!(SOURCES.len(), e["SOURCES"].as_object().unwrap().len());
    for t in e["safeHref"].as_array().unwrap() {
        let v = t["in"].clone();
        let text = match &v {
            Value::String(s) => s.clone(),
            Value::Object(_) => "undefined".to_string(),
            Value::Null => "null".to_string(),
            other => JsonValue::from_value(other).js_string(),
        };
        // the prototype reads String(h || ''): undefined, null and 0 are empty
        let text = if matches!(&v, Value::Object(_) | Value::Null) || v == json!(0) {
            String::new()
        } else {
            text
        };
        assert_eq!(
            safe_href(&text),
            t["out"].as_str().unwrap(),
            "safeHref of {:?}",
            t["in"]
        );
    }
}

fn corridor_project() -> construct_transport::project::Project {
    project("corridor")
}

#[test]
fn stamps_and_failed_stamps_match() {
    let e = expected("vintage");
    let mut p = corridor_project();
    for s in e["stamps"].as_array().unwrap() {
        let extra: Map<String, Value> = s["extra"].as_object().unwrap().clone();
        let got = stamp(&mut p, s["id"].as_str().unwrap(), &extra, NOW);
        assert_close(&got, &s["result"], &format!("$.stamps[{}]", s["id"]));
        let vintage = p.extra["feeds"]["vintage"].clone();
        assert_close(
            &vintage,
            &s["vintage"],
            &format!("$.stamps[{}].vintage", s["id"]),
        );
    }
}

#[test]
fn vintage_table_matches() {
    let e = expected("vintage");
    let mut p = corridor_project();
    let stamps: [(&str, Value); 12] = [
        (
            "nslr",
            json!({ "n": 12000, "note": "Current versions only. 80% of roads matched." }),
        ),
        ("controlled", json!({ "n": 4 })),
        ("tms", json!({ "n": 3, "note": "a" })),
        (
            "gtfs",
            json!({ "key": "gtfs", "url": "https://files.mobilitydatabase.org/mdb-1029/latest.zip", "n": 400, "note": "b" }),
        ),
        (
            "council",
            json!({ "key": "council:x", "url": "https://services1.arcgis.com/R6s0QqCMQdwKY6yp/x", "source": "Hamilton City Council", "n": 5 }),
        ),
        ("mvr", json!({ "error": "No <b>cars</b> here", "n": 0 })),
        (
            "council",
            json!({ "key": "council:y", "url": "javascript:alert(1)", "source": "Hamilton City Council", "n": 0 }),
        ),
        (
            "osm",
            json!({ "n": 1234567, "url": "data/x.json", "page": "data/x.json" }),
        ),
        (
            "ags",
            json!({ "n": 3, "note": "It's \"quoted\" & <tagged>" }),
        ),
        ("flood", json!({ "n": 6, "source": "Zed" })),
        ("coastal", json!({ "n": 6, "source": "Alpha" })),
        ("unknown", json!({ "n": 1, "note": "x" })),
    ];
    for (id, extra) in &stamps {
        stamp(&mut p, id, extra.as_object().unwrap(), NOW);
    }
    {
        let v = p
            .extra
            .get_mut("feeds")
            .unwrap()
            .get_mut("vintage")
            .unwrap()
            .as_object_mut()
            .unwrap();
        v.get_mut("nslr").unwrap()["at"] = json!("2025-09-09T12:00:00.000Z");
        v.get_mut("tms").unwrap()["at"] = json!("bad");
        v.get_mut("controlled")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("n");
        v.get_mut("unknown")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("source");
    }
    assert_close(&p.extra["feeds"]["vintage"], &e["vintage"], "$.vintage");
    let rows = vintage_rows(&p);
    let want = e["rows"].as_array().unwrap();
    assert_eq!(rows.len(), want.len());
    for (i, (r, w)) in rows.iter().zip(want).enumerate() {
        assert_close(
            &serde_json::to_value(r).unwrap(),
            w,
            &format!("$.rows[{i}]"),
        );
    }
}
