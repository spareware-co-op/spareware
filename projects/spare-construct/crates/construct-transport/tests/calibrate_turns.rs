//! Reading turning-count files, one function at a time, against the JavaScript's recorded answers
//! (expected/calibrate-turns.json, from tools/transport-truth/gen-calibrate.mjs): times, dates, vehicle classes, about
//! forty files in long, wide, SCATS and leg-total layouts (bad rows, odd dates, every interval), peak hours under
//! several window sets, and Furness turn estimation.

mod calibrate_support;
mod support;

use construct_transport::calibrate::{
    LEGS, TurnSite, calib_cfg, class_of, furness, iso_date, parse_turn_csv, peaks, to_min,
};
use construct_transport::edit::IdSource;
use construct_transport::jsnum::number_to_js_string;
use serde_json::{Value, json};
use support::{assert_close, load_json};

/// 2026-10-21T00:00:00.000Z, the clock the generator's context stands still on.
const NOW_MS: f64 = 1_792_540_800_000.0;

fn cases() -> Value {
    load_json("expected/calibrate-turns.json")
}

/// The generator's stand-in for `U.uid`: prefix + "x" + a counter.
struct Ids {
    n: usize,
    made: Vec<String>,
}

impl IdSource for Ids {
    fn uid(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}x{}", self.n);
        self.n += 1;
        self.made.push(id.clone());
        id
    }
    fn now_ms(&self) -> f64 {
        NOW_MS
    }
}

/// `String(v ?? '')` for the loose inputs the generator feeds the readers.
fn text_of(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) if s == "__undefined__" => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => number_to_js_string(n.as_f64().expect("number")),
        other => other.to_string(),
    }
}

#[test]
fn times() {
    for c in cases()["toMin"].as_array().expect("toMin") {
        let got = to_min(&text_of(&c["t"])).map_or(Value::Null, |m| json!(m));
        assert_close(&got, &c["out"], &format!("toMin({})", c["t"]));
    }
}

#[test]
fn dates() {
    for c in cases()["isoDate"].as_array().expect("isoDate") {
        let got = iso_date(&text_of(&c["v"])).map_or(Value::Null, |d| json!(d));
        assert_close(&got, &c["out"], &format!("isoDate({})", c["v"]));
    }
}

#[test]
fn classes() {
    for c in cases()["classOf"].as_array().expect("classOf") {
        let got = class_of(&text_of(&c["v"]));
        assert_close(
            &got.map_or(Value::Null, |d| json!(d)),
            &c["out"],
            &format!("classOf({})", c["v"]),
        );
    }
    assert_close(&json!(LEGS), &cases()["legBearings"], "the compass legs");
}

#[test]
fn files() {
    let cfg = calib_cfg(None);
    let all = cases();
    let list = all["parse"].as_array().expect("parse");
    assert_eq!(list.len(), 34, "file cases");
    for rec in list {
        let name = rec["name"].as_str().expect("name");
        let mut ids = Ids {
            n: 0,
            made: Vec::new(),
        };
        let r = parse_turn_csv(rec["text"].as_str().expect("text"), "f.csv", &cfg, &mut ids);
        match (&r, rec.get("error")) {
            (Err(e), Some(x)) => assert_close(&json!(e), x, &format!("{name}: error")),
            (Ok(_), Some(x)) => panic!("{name}: expected the error {x}"),
            (Err(e), None) => panic!("{name}: unexpected error {e}"),
            (Ok(r), None) => {
                let got = json!({ "sites": serde_json::to_value(&r.sites).expect("sites"), "skipped": r.skipped, "skippedRows": r.skipped_rows,
                    "reasons": r.reasons, "fromModel": r.from_model });
                assert_close(&got, &rec["out"], name);
            }
        }
        assert_close(&json!(ids.made), &rec["ids"], &format!("{name}: ids"));
    }
}

#[test]
fn peak_hours() {
    let all = cases();
    let list = all["peaks"].as_array().expect("peaks");
    assert_eq!(list.len(), 7);
    for (k, rec) in list.iter().enumerate() {
        let mut site: TurnSite = serde_json::from_value(rec["site"].clone()).expect("site");
        let windows: Vec<(String, [String; 2])> = rec["windows"]
            .as_object()
            .expect("windows")
            .iter()
            .map(|(p, w)| (p.clone(), [text_of(&w[0]), text_of(&w[1])]))
            .collect();
        peaks(&mut site, &windows);
        let got = if site.peak.is_set() {
            serde_json::to_value(&site.peak).expect("peak")
        } else {
            Value::Null
        };
        assert_close(
            &got,
            rec.get("peak").unwrap_or(&Value::Null),
            &format!("peaks case {k} {}", rec["windows"]),
        );
    }
}

#[test]
fn furness_turns() {
    let all = cases();
    let list = all["furness"].as_array().expect("furness");
    assert_eq!(list.len(), 10);
    let nums = |v: &Value| -> Vec<f64> {
        v.as_array()
            .expect("array")
            .iter()
            .map(|x| x.as_f64().unwrap_or(f64::NAN))
            .collect()
    };
    for (k, rec) in list.iter().enumerate() {
        let seed: Vec<Vec<f64>> = rec["seed"]
            .as_array()
            .expect("seed")
            .iter()
            .map(nums)
            .collect();
        let tol = rec["opts"]
            .get("tol")
            .and_then(Value::as_f64)
            .unwrap_or(0.001);
        let max_it = rec["opts"]
            .get("maxIt")
            .and_then(Value::as_u64)
            .unwrap_or(50) as usize;
        let r = furness(&nums(&rec["ins"]), &nums(&rec["outs"]), &seed, tol, max_it);
        let mut want = rec["out"].clone();
        want.as_object_mut().expect("out").remove("tag");
        let got = json!({ "m": r.m, "it": r.it, "err": r.err, "converged": r.converged, "outsScaled": r.outs_scaled });
        assert_close(&got, &want, &format!("furness case {k}"));
    }
}
