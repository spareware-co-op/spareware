//! The place search against the JavaScript's recorded runs: reading typed coordinates in every notation, the road abbreviations
//! written out, the display name cut short, the boxes around a point, and what a typed query makes of every kind of answer
//! (the request it sends, the places made, each failure's message, the odd records that make the prototype throw).
//! Recorded by tools/transport-truth/gen-search.mjs (expected/search-cases.json, fixtures/data/nominatim-answer.json).

mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::FetchFailure;
use construct_transport::search::*;
use serde_json::{Value, json};
use support::assert_close;

fn cases() -> Value {
    support::load_json("expected/search-cases.json")
}

// a query the prototype was given: text, or something that reads as no text (null, undefined, 0) or as its number written out
fn text_of(q: &Value) -> String {
    match q {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.as_f64() != Some(0.0) => {
            construct_transport::jsnum::number_to_js_string(n.as_f64().unwrap())
        }
        _ => String::new(),
    }
}

fn place_value(p: Option<Place>) -> Value {
    p.map_or(Value::Null, |p| serde_json::to_value(p).unwrap())
}

#[test]
fn typed_coordinates_match() {
    let e = cases();
    let recs = e["coords"].as_array().unwrap();
    assert_eq!(recs.len(), 66);
    for rec in recs {
        let q = text_of(&rec["q"]);
        assert_close(
            &place_value(parse_coords(&q)),
            &rec["result"],
            &format!("coordinates of {:?}", rec["q"]),
        );
    }
}

#[test]
fn road_abbreviations_and_display_names_match() {
    let e = cases();
    let ex = e["expand"].as_array().unwrap();
    assert_eq!(ex.len(), 89);
    for rec in ex {
        assert_eq!(
            expand(rec["q"].as_str().unwrap()),
            rec["result"].as_str().unwrap(),
            "expand {:?}",
            rec["q"]
        );
    }
    let sh = e["shorten"].as_array().unwrap();
    assert_eq!(sh.len(), 44);
    for rec in sh {
        assert_eq!(
            shorten(&text_of(&rec["q"])),
            rec["result"].as_str().unwrap(),
            "shorten {:?}",
            rec["q"]
        );
    }
    assert_eq!(GAP_MS, e["gap"].as_f64().unwrap());
}

#[test]
fn boxes_around_points_match() {
    let e = cases();
    let recs = e["boxes"].as_array().unwrap();
    assert_eq!(recs.len(), 125);
    for rec in recs {
        let (lat, lng) = (rec["lat"].as_f64().unwrap(), rec["lng"].as_f64().unwrap());
        let q = format!(
            "{}, {}",
            construct_transport::jsnum::number_to_js_string(lat),
            construct_transport::jsnum::number_to_js_string(lng)
        );
        assert_close(
            &place_value(parse_coords(&q)),
            &rec["result"],
            &format!("box at {lat}, {lng}"),
        );
    }
}

// what each recorded answer was: a status and a body (or no connection)
fn answer_of(a: &Value) -> Result<JsonValue, FetchFailure> {
    if a == "net" {
        return Err(FetchFailure::Unreachable("Failed to fetch".to_string()));
    }
    let status = a["status"].as_u64().unwrap() as u16;
    if !(200..300).contains(&status) {
        return Err(FetchFailure::Http(status));
    }
    if a["body"] == "throw" {
        return Err(FetchFailure::Unreadable(
            "Unexpected end of JSON input".to_string(),
        ));
    }
    Ok(JsonValue::from_value(&a["body"]))
}

#[test]
fn queries_make_the_recorded_requests_and_places() {
    let e = cases();
    let recs = e["geocode"].as_array().unwrap();
    assert_eq!(recs.len(), 296);
    for rec in recs {
        let label = format!("{} / {:?}", rec["label"], rec["q"]);
        let q = text_of(&rec["q"]);
        let requests = rec["requests"].as_array().unwrap();
        let result: Result<Vec<Place>, String> = match plan(&q) {
            Plan::Empty => {
                assert!(
                    requests.is_empty(),
                    "{label}: no request for an empty query"
                );
                Ok(Vec::new())
            }
            Plan::Coords(p) => {
                assert!(requests.is_empty(), "{label}: no request for coordinates");
                Ok(vec![p])
            }
            Plan::Ask { request, .. } => {
                assert_eq!(requests.len(), 1, "{label}: one request");
                assert_eq!(request.method, "GET");
                assert_eq!(
                    request.url,
                    requests[0].as_str().unwrap(),
                    "{label}: address"
                );
                let answer = answer_of(&e["answers"][rec["label"].as_str().unwrap()]);
                read_answer(answer.as_ref().map_err(Clone::clone))
            }
        };
        match (result, rec.get("thrown")) {
            (Ok(places), None) => {
                assert_close(&json!(places), &rec["result"], &format!("{label} places"))
            }
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (g, t) => panic!("{label}: got {g:?}, recorded {t:?}"),
        }
    }
}
