//! Bus timetables (GTFS) against the JavaScript's recorded runs: searching the Mobility Database list for feeds that cover an area (and
//! the built-in list when it fails), reading a zip (every kind of broken file, the day chosen, school terms and holidays, the peak
//! windows), the whole download (a HEAD then the zip), the bus routes made on each scenario's network and applying them.
//! Recorded by tools/transport-truth/gen-feeds.mjs (expected/feeds-gtfs.json, fixtures/data/gtfs-*.zip and gtfs-catalogue-*.csv).

mod feeds_support;
mod support;

use construct_transport::feeds::*;
use feeds_support::*;
use serde_json::{Value, json};
use support::assert_close;

fn feeds_of_run(p: &construct_transport::project::Project) -> Value {
    p.extra
        .get("feeds")
        .cloned()
        .unwrap_or_else(|| json!({ "vintage": {} }))
}

#[test]
fn catalogue_searches_match() {
    let e = expected("gtfs");
    let recs = e["catalogues"].as_array().unwrap();
    assert_eq!(recs.len(), 18);
    for rec in recs {
        let label = format!("{} {}", rec["project"], rec["label"]);
        let mut p = project(rec["project"].as_str().unwrap());
        let b = bounds_of(&rec["bounds"]);
        let timeout_s = rec["cfg"]["timeout"].as_f64().unwrap_or(30.0);
        let log = rec["requests"].as_array().unwrap();
        let mut first = Catalogue::new(b.clone(), timeout_s, None);
        let (used, res) = replay(&mut first, log, 0, &label);
        res.expect("the search");
        all_used(log, used, &label);
        let kept: Option<Vec<GtfsFeed>> = first.fresh_list().map(<[GtfsFeed]>::to_vec);
        if let Some(l) = &kept {
            apply_catalogue(&mut p, l, NOW);
        }
        let mut result = first.result();
        if rec["label"] == "twice" {
            // the second search uses the list kept from the first
            let second = Catalogue::new(b, timeout_s, kept);
            result = second.result();
        }
        assert_close(
            &serde_json::to_value(&result).unwrap(),
            &rec["result"],
            &format!("{label} result"),
        );
        assert_close(&feeds_of_run(&p), &rec["feeds"], &format!("{label} feeds"));
    }
}

fn opts_of(v: &Value) -> GtfsOpts {
    let f = |x: &Value| x.as_f64().unwrap();
    let pair = |x: &Value| [f(&x[0]), f(&x[1])];
    GtfsOpts {
        box_: [
            f(&v["box"][0]),
            f(&v["box"][1]),
            f(&v["box"][2]),
            f(&v["box"][3]),
        ],
        today: v["today"].as_str().unwrap().to_string(),
        holidays: v["holidays"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect(),
        terms: v["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                [
                    t[0].as_str().unwrap().to_string(),
                    t[1].as_str().unwrap().to_string(),
                ]
            })
            .collect(),
        am: pair(&v["am"]),
        pm: pair(&v["pm"]),
    }
}

fn opts_value(o: &GtfsOpts) -> Value {
    json!({ "box": o.box_, "today": o.today, "holidays": o.holidays, "terms": o.terms, "am": o.am, "pm": o.pm })
}

// the reading without the time it took, which the prototype records and the Rust does not
fn without_ms(v: &Value) -> Value {
    let mut v = v.clone();
    v.as_object_mut().unwrap().remove("ms");
    v
}

#[test]
fn unreadable_files_are_refused_as_recorded() {
    let e = expected("gtfs");
    let recs = e["errors"].as_array().unwrap();
    assert_eq!(recs.len(), 12);
    for rec in recs {
        let label = rec["label"].as_str().unwrap();
        let buf = fixture_bytes(rec["file"].as_str().unwrap());
        let got = parse_gtfs(&buf, &opts_of(&rec["opts"]));
        match (&got, rec.get("thrown")) {
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (Ok(r), None) => assert_close(
                &serde_json::to_value(r).unwrap(),
                &without_ms(&rec["result"]),
                &format!("{label} reading"),
            ),
            (g, t) => panic!(
                "{label}: got {:?}, recorded {t:?}",
                g.as_ref().map(|_| "a reading")
            ),
        }
    }
}

#[test]
fn readings_with_other_options_match() {
    let e = expected("gtfs");
    let recs = e["parses"].as_array().unwrap();
    assert_eq!(recs.len(), 9);
    for rec in recs {
        let label = rec["label"].as_str().unwrap();
        let buf = fixture_bytes(rec["file"].as_str().unwrap());
        let got = parse_gtfs(&buf, &opts_of(&rec["opts"]));
        match (&got, rec.get("thrown")) {
            (Err(m), Some(t)) => assert_eq!(m, t.as_str().unwrap(), "{label}: error"),
            (Ok(r), None) => assert_close(
                &serde_json::to_value(r).unwrap(),
                &without_ms(&rec["result"]),
                &format!("{label} reading"),
            ),
            (g, t) => panic!(
                "{label}: got {:?}, recorded {t:?}",
                g.as_ref().map(|_| "a reading")
            ),
        }
    }
}

fn feed_of(v: &Value) -> GtfsFeed {
    let s = |k: &str| v[k].as_str().map(str::to_string);
    GtfsFeed {
        id: s("id"),
        provider: s("provider").unwrap(),
        name: s("name"),
        cc: None,
        latest: s("latest"),
        licence: s("licence"),
        box_: [0.0; 4],
    }
}

#[test]
fn downloads_routes_and_keeping_match() {
    let e = expected("gtfs");
    let recs = e["loads"].as_array().unwrap();
    assert_eq!(recs.len(), 28);
    for rec in recs {
        let label = format!("{} {}", rec["project"], rec["label"]);
        let now = rec["now"].as_f64().unwrap();
        let mut load = GtfsLoad::new(feed_of(&rec["feed"]), &bounds_of(&rec["bounds"]), now, 30.0);
        assert_close(
            &opts_value(load.opts()),
            &rec["opts"],
            &format!("{label} reader options"),
        );
        let log = rec["requests"].as_array().unwrap();
        let (used, res) = replay(&mut load, log, 0, &label);
        all_used(log, used, &label);
        if let Some(t) = rec.get("thrown") {
            assert_eq!(
                res.expect_err("the download fails"),
                t.as_str().unwrap(),
                "{label}: error"
            );
            continue;
        }
        res.expect("the download");
        assert_eq!(
            json!(load.statuses()),
            rec["statuses"],
            "{label}: status lines"
        );
        let g = load.finish().expect("a reading");
        assert_close(
            &serde_json::to_value(&g).unwrap(),
            &without_ms(&rec["loaded"]),
            &format!("{label} reading"),
        );
        let mut p = project(rec["project"].as_str().unwrap());
        for (k, want) in rec["routes"].as_array().unwrap().iter().enumerate() {
            let sc = &mut p.scenarios[k];
            assert_eq!(sc.id, want["scenario"].as_str().unwrap());
            let routes = gtfs_routes(sc, &g);
            assert_close(
                &serde_json::to_value(&routes).unwrap(),
                &want["routes"],
                &format!("{label} routes on {}", sc.id),
            );
            assert_close(
                &serde_json::to_value(&sc.routes).unwrap(),
                &want["before"],
                &format!("{label} routes before on {}", sc.id),
            );
            sc.routes = serde_json::from_value(json!({
                "oldGtfs": { "id": "oldGtfs", "name": "Old", "src": "GTFS", "freq": 3, "wp": [] },
                "mine": { "id": "mine", "name": "Mine", "freq": 4, "wp": [] } }))
            .expect("routes read");
            let n = apply_gtfs(sc, &routes);
            assert_eq!(
                n as u64,
                want["applied"].as_u64().unwrap(),
                "{label}: applied on {}",
                sc.id
            );
            assert_close(
                &serde_json::to_value(&sc.routes).unwrap(),
                &want["scRoutes"],
                &format!("{label} routes after on {}", sc.id),
            );
        }
        let routes = gtfs_routes(&p.scenarios[0], &g);
        keep_gtfs(&mut p, &g, &routes, now);
        assert_close(
            &feeds_of_run(&p),
            &rec["feeds"],
            &format!("{label} feeds kept"),
        );
        keep_gtfs(&mut p, &g, &[], now);
        assert_close(
            &feeds_of_run(&p),
            &rec["feedsNoRoutes"],
            &format!("{label} feeds kept without routes"),
        );
    }
}
