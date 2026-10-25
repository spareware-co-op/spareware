//! Every result the JavaScript recorded (expected/<name>.json, scenarios, periods, res) reads as
//! the typed `Res` and writes back as the same JSON: same keys in the same order, keys left
//! out where the JavaScript leaves them out, nulls where it writes null, and the same numbers.

mod ordered;
mod support;

use construct_transport::project::OrderedMap;
use construct_transport::results::Res;
use ordered::{OJ, first_difference, ordered_of};
use serde::Deserialize;
use support::{PROJECTS, assert_close, read_truth};

#[derive(Deserialize)]
struct Expected<R> {
    scenarios: OrderedMap<ScenarioRec<R>>,
}

#[derive(Deserialize)]
struct ScenarioRec<R> {
    periods: OrderedMap<PeriodRec<R>>,
}

#[derive(Deserialize)]
struct PeriodRec<R> {
    res: R,
}

#[test]
fn every_recorded_result_round_trips() {
    let mut n = 0;
    for name in PROJECTS {
        let text = read_truth(&format!("expected/{name}.json"));
        let typed: Expected<Res> = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("expected/{name}.json results do not read as Res: {e}"));
        let raw: Expected<OJ> = serde_json::from_str(&text).expect("expected JSON");
        for (sid, sc) in raw.scenarios.iter() {
            for (period, rec) in sc.periods.iter() {
                let res = &typed.scenarios[sid].periods[period].res;
                let back = ordered_of(res);
                let path = format!("{name}.{sid}.{period}.res");
                if let Some(d) = first_difference(&back, &rec.res, &path) {
                    panic!("result does not round-trip: {d}");
                }
                assert_close(&back.to_value(), &rec.res.to_value(), &path);
                // and once more through the type
                let again: Res =
                    serde_json::from_str(&serde_json::to_string(res).unwrap()).unwrap();
                assert_eq!(&again, res, "{path}: second reading differs");
                n += 1;
            }
        }
    }
    assert_eq!(n, 10, "results checked");
}

#[test]
fn non_finite_numbers_and_left_out_keys() {
    // gen-expected.mjs writes NaN and infinities as text; a missing paths key stays missing, null stays null
    let text = read_truth("expected/grid.json");
    let raw: Expected<serde_json::Value> = serde_json::from_str(&text).unwrap();
    let (_, sc) = raw.scenarios.iter().next().unwrap();
    let mut v = sc.periods["AM"].res.clone();
    v["noise95"] = serde_json::json!("NaN");
    v["kpi"]["maxVC"] = serde_json::json!("Infinity");
    v.as_object_mut().unwrap().remove("paths");
    v.as_object_mut().unwrap().remove("arcKeys");
    let r: Res = serde_json::from_value(v).unwrap();
    assert!(r.noise95.unwrap().is_nan());
    assert_eq!(r.kpi.max_vc, f64::INFINITY);
    assert!(r.paths.is_missing() && r.arc_keys.is_none());
    let back = serde_json::to_value(&r).unwrap();
    assert_eq!(back["noise95"], "NaN");
    assert_eq!(back["kpi"]["maxVC"], "Infinity");
    assert!(back.get("paths").is_none() && back.get("arcKeys").is_none());
    let mut v2 = back.clone();
    v2["paths"] = serde_json::Value::Null;
    let r2: Res = serde_json::from_value(v2).unwrap();
    assert!(r2.paths.is_null());
    assert!(serde_json::to_value(&r2).unwrap()["paths"].is_null());
}
