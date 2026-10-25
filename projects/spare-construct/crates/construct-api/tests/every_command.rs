//! Every command, run on the truth projects: a long scripted session that calls each command of the registry at least once with
//! real parameters, checks what it gives back, and fails if a command is added that the script does not call.

mod support;

use construct_api::Session;
use construct_api::transport::registry;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use support::{input_text, read_truth};

struct T {
    s: Session,
    called: BTreeSet<String>,
}

impl T {
    fn ok(&mut self, cmd: &str, p: Value) -> Value {
        self.called.insert(cmd.to_string());
        self.s
            .run(cmd, &p)
            .unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
    }
    fn err(&mut self, cmd: &str, p: Value, code: &str) -> Value {
        self.called.insert(cmd.to_string());
        match self.s.run(cmd, &p) {
            Ok(v) => panic!("{cmd} {p} should fail with {code}, gave {v}"),
            Err(e) => {
                assert_eq!(e.code, code, "{cmd} {p}: {e}");
                e.to_value()
            }
        }
    }
    fn nodes(&mut self, n: usize) -> Vec<String> {
        let v = self.ok(
            "transport.network.list",
            json!({ "kind": "node", "limit": n }),
        );
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap().to_string())
            .collect()
    }
    fn links(&mut self, n: usize) -> Vec<String> {
        let v = self.ok(
            "transport.network.list",
            json!({ "kind": "link", "limit": n }),
        );
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap().to_string())
            .collect()
    }
}

fn fixture(f: &str) -> Value {
    serde_json::from_str(&read_truth(&format!("fixtures/{f}"))).unwrap()
}

#[test]
fn every_command_runs_on_the_truth_projects() {
    let mut t = T {
        s: Session::seeded(5, 1_790_000_000_000.0),
        called: BTreeSet::new(),
    };

    // ---- project and settings
    let info = t.ok("transport.project.info", json!({}));
    assert_eq!(info["name"], json!("Untitled project"));
    t.ok("transport.project.new", json!({ "name": "Scratch" }));
    t.ok("transport.project.rename", json!({ "name": "  Renamed  " }));
    assert_eq!(
        t.ok("transport.project.info", json!({}))["name"],
        json!("Renamed")
    );
    let grid = input_text("grid");
    let opened = t.ok("transport.project.open", json!({ "text": grid }));
    assert_eq!(opened["scenarios"].as_array().unwrap().len(), 3);
    // saving writes the file as the prototype wrote it
    let saved = t.ok("transport.project.save", json!({}));
    assert_eq!(saved["fileName"], json!("truth-test-grid.corridor.json"));
    assert_eq!(
        serde_json::from_str::<Value>(saved["text"].as_str().unwrap()).unwrap(),
        serde_json::from_str::<Value>(&grid).unwrap()
    );
    t.ok("transport.project.set_period", json!({ "period": "PM" }));
    t.ok("transport.project.set_period", json!({ "period": "AM" }));
    t.ok("transport.reference.tables", json!({}));
    assert!(
        t.ok("transport.settings.get", json!({ "path": "econ.vot" }))
            .is_number()
    );
    t.err(
        "transport.settings.get",
        json!({ "path": "nope.nothing" }),
        "not_found",
    );
    assert_eq!(
        t.ok(
            "transport.settings.set",
            json!({ "values": { "growth": 3 } })
        )["changed"],
        json!(true)
    );
    assert_eq!(
        t.ok(
            "transport.settings.set",
            json!({ "values": { "growth": 3 } })
        )["changed"],
        json!(false)
    );
    assert_eq!(
        t.ok("transport.settings.get", json!({ "path": "growth" })),
        json!(3)
    );
    t.ok("transport.settings.reset", json!({}));
    t.ok("transport.settings.adopt_defaults", json!({}));
    t.ok("transport.settings.keep", json!({}));

    // ---- scenarios and the register
    let list = t.ok("transport.scenario.list", json!({}));
    let base = list["base"].as_str().unwrap().to_string();
    t.ok(
        "transport.scenario.select",
        json!({ "scenario": "Option 2036" }),
    );
    t.ok("transport.scenario.select", json!({ "scenario": base }));
    let copy = t.ok("transport.scenario.copy", json!({ "name": "Copy of base" }));
    let copy_id = copy["id"].as_str().unwrap().to_string();
    t.ok(
        "transport.scenario.rename",
        json!({ "scenario": copy_id, "name": "Renamed copy" }),
    );
    t.ok(
        "transport.scenario.set_year",
        json!({ "scenario": copy_id, "year": 2031 }),
    );
    t.err(
        "transport.scenario.set_year",
        json!({ "scenario": copy_id, "year": 1800 }),
        "bad_params",
    );
    t.ok(
        "transport.scenario.set",
        json!({ "scenario": copy_id, "values": { "notes": "a note", "role": "proposal" } }),
    );
    t.ok(
        "transport.scenario.describe",
        json!({ "scenario": copy_id }),
    );
    t.ok("transport.scenario.check", json!({ "scenario": copy_id }));
    t.ok("transport.scenario.table", json!({}));
    t.ok("transport.register.table", json!({}));
    t.ok("transport.register.presets", json!({}));
    let added = t.ok("transport.scenario.add", json!({}));
    t.ok("transport.scenario.set_base", json!({ "scenario": base }));
    t.ok(
        "transport.scenario.sensitivity_copy",
        json!({ "scenario": base, "unfunded": true }),
    );
    t.ok(
        "transport.scenario.create_set",
        json!({ "countYear": 2026, "opening": 2031, "design": 2036, "mitigation": false }),
    );
    t.ok(
        "transport.scenario.clear_results",
        json!({ "scenario": added["id"] }),
    );
    t.ok(
        "transport.scenario.delete",
        json!({ "scenario": added["id"] }),
    );
    t.err(
        "transport.scenario.delete",
        json!({ "scenario": "no such scenario" }),
        "not_found",
    );
    t.ok("transport.scenario.select", json!({ "scenario": base }));

    // ---- reading and editing the network
    let summary = t.ok("transport.network.summary", json!({}));
    assert_eq!(summary["nodes"], json!(20));
    let nodes = t.nodes(25);
    let links = t.links(40);
    t.ok(
        "transport.network.get",
        json!({ "kind": "node", "id": nodes[6] }),
    );
    t.ok(
        "transport.network.get",
        json!({ "kind": "link", "id": links[3] }),
    );
    t.ok(
        "transport.network.get",
        json!({ "kind": "zone", "id": "zA" }),
    );
    t.ok(
        "transport.network.get",
        json!({ "kind": "route", "id": "r1" }),
    );
    let near = t.ok(
        "transport.network.nearest",
        json!({ "kind": "node", "point": [-36.9001, 174.7601] }),
    );
    assert_eq!(near["id"], json!(nodes[0]));
    t.ok(
        "transport.network.nearest",
        json!({ "kind": "link", "point": [-36.9025, 174.7645], "maxM": 20 }),
    );
    let n1 = t.ok(
        "transport.network.add_node",
        json!({ "point": [-36.896, 174.7615], "props": { "control": "none" } }),
    );
    let new_node = n1["id"].as_str().unwrap().to_string();
    let l1 = t.ok("transport.network.add_link", json!({ "a": new_node, "b": nodes[1], "points": [[-36.8975, 174.7612]], "props": { "cls": "collector", "name": "Link Road" } }));
    let new_link = l1["id"].as_str().unwrap().to_string();
    let split = t.ok(
        "transport.network.split",
        json!({ "link": new_link, "point": [-36.8975, 174.7612] }),
    );
    assert_eq!(split["created"], json!(true));
    t.ok("transport.network.reverse", json!({ "link": new_link }));
    let road = t.ok("transport.network.add_road", json!({ "points": [{ "type": "node", "id": nodes[4], "lat": -36.9, "lng": 174.772 }, { "type": "free", "lat": -36.896, "lng": 174.775 }], "cls": "arterial", "lanes": 2 }));
    assert_eq!(road["links"].as_array().unwrap().len(), 1);
    t.ok(
        "transport.network.auto_control",
        json!({ "nodes": [nodes[6], nodes[7]] }),
    );
    t.ok(
        "transport.network.move",
        json!({ "kind": "node", "id": new_node, "point": [-36.8962, 174.7616] }),
    );
    t.ok(
        "transport.network.set",
        json!({ "kind": "link", "id": new_link, "values": { "speed": 40, "lanesAB": 2 } }),
    );
    t.ok(
        "transport.network.set",
        json!({ "kind": "node", "id": nodes[12], "values": { "control": "signals" } }),
    );
    t.err(
        "transport.network.set",
        json!({ "kind": "link", "id": new_link, "values": { "lanesAB": "lots" } }),
        "bad_params",
    );
    let z = t.ok(
        "transport.zone.add",
        json!({ "point": [-36.9105, 174.766], "lu": { "residential": 40 } }),
    );
    let zone_id = z["id"].as_str().unwrap().to_string();
    t.ok(
        "transport.zone.set",
        json!({ "id": zone_id, "values": { "name": "Infill", "lu.residential": "1,200" } }),
    );
    let gw = t.ok("transport.zone.add_gateway", json!({ "node": nodes[19] }));
    t.ok("transport.zone.delete", json!({ "id": gw["id"] }));
    t.ok("transport.zone.delete", json!({ "id": zone_id }));
    let r = t.ok(
        "transport.route.add",
        json!({ "stops": [nodes[0], nodes[5], nodes[10]], "freq": 10 }),
    );
    t.ok(
        "transport.route.set",
        json!({ "id": r["id"], "values": { "freq": 12 } }),
    );
    t.ok("transport.route.delete", json!({ "id": r["id"] }));
    t.ok(
        "transport.network.delete",
        json!({ "kind": "link", "id": road["links"][0] }),
    );
    t.ok(
        "transport.network.delete",
        json!({ "kind": "node", "id": new_node }),
    );
    // undo and redo them
    let hist = t.ok("transport.history.state", json!({}));
    assert!(hist["undo"].as_array().unwrap().len() > 10);
    assert_eq!(
        t.ok("transport.history.undo", json!({}))["done"],
        json!(true)
    );
    assert_eq!(
        t.ok("transport.history.redo", json!({}))["done"],
        json!(true)
    );

    // ---- junction sites
    t.ok("transport.sites.detect", json!({}));
    t.ok("transport.sites.list", json!({}));
    let merged = t.ok(
        "transport.sites.merge",
        json!({ "items": [nodes[6], nodes[7]] }),
    );
    let site = merged["site"].as_str().unwrap().to_string();
    t.ok("transport.sites.set", json!({ "site": site, "values": { "name": "Centre", "control": "signals", "atSiteNo": "A1, A2" } }));
    t.ok(
        "transport.sites.remove_node",
        json!({ "site": site, "node": nodes[7] }),
    );
    t.err(
        "transport.sites.split",
        json!({ "site": "nope" }),
        "not_found",
    );

    // ---- import and export
    t.ok("transport.studyarea.estimate", json!({ "bounds": { "south": -37.0682, "west": 174.935, "north": -37.0612, "east": 174.948 } }));
    let q = t.ok(
        "transport.import.overpass_query",
        json!({ "bounds": [[-37.0682, 174.935], [-37.0612, 174.948]], "service": true }),
    );
    assert!(q["query"].as_str().unwrap().contains("highway"));
    t.ok("transport.project.new", json!({ "name": "Imports" }));
    let imp = t.ok("transport.import.osm_roads", json!({ "answer": fixture("overpass-papakura.json"), "bounds": { "south": -37.0682, "west": 174.935, "north": -37.0612, "east": 174.948 } }));
    assert!(imp["segments"].as_u64().unwrap() > 10);
    t.ok(
        "transport.import.auto_gateways",
        json!({ "bounds": [[-37.09, 174.91], [-37.04, 174.97]] }),
    );
    t.ok(
        "transport.data.set",
        json!({ "controlled": [{ "lat": -37.0648282, "lng": 174.9388742, "no": "AT-GSR1" }] }),
    );
    t.ok("transport.import.geojson", json!({ "geojson": read_truth("fixtures/papakura-proposals.geojson"), "cls": "arterial", "lanes": 2 }));
    assert!(
        t.ok("transport.export.geojson", json!({}))["text"]
            .as_str()
            .unwrap()
            .contains("FeatureCollection")
    );
    assert!(
        t.ok("transport.export.links_csv", json!({}))["text"]
            .as_str()
            .unwrap()
            .starts_with("link_id")
    );
    t.ok("transport.landuse.constants", json!({}));
    let lq = t.ok("transport.landuse.queries", json!({ "bounds": { "south": -36.975, "west": 174.852, "north": -36.965, "east": 174.868 } }));
    assert!(lq["jobs"].as_array().unwrap().len() >= 2);
    let lu = t.ok(
        "transport.landuse.apply",
        json!({ "answers": [fixture("landuse-papatoetoe-buildings-0.json"), fixture("landuse-papatoetoe-areas-0.json")], "bounds": { "south": -36.975, "west": 174.852, "north": -36.965, "east": 174.868 } }),
    );
    assert!(lu["zones"].as_u64().unwrap() > 0);

    // ---- the model and its results, on the grid project again
    t.ok("transport.project.open", json!({ "text": grid }));
    let run = t.ok("transport.run", json!({ "scenario": base, "periods": ["AM"], "assign": { "gapTol": 0.0001, "maxIter": 100, "demandFactor": 1.0 } }));
    assert_eq!(run["runs"].as_array().unwrap().len(), 1);
    t.ok("transport.run", json!({ "all": true }));
    assert_eq!(
        t.ok("transport.run", json!({ "onlyStale": true }))["message"],
        json!("Every result is up to date.")
    );
    t.ok("transport.results.list", json!({}));
    let g = t.ok("transport.results.get", json!({}));
    assert!(g["arcs"].is_object());
    t.ok("transport.results.summary", json!({}));
    t.ok(
        "transport.results.arcs",
        json!({ "sortBy": "vc", "limit": 5 }),
    );
    t.ok(
        "transport.results.nodes",
        json!({ "sortBy": "delay", "detail": true }),
    );
    t.ok("transport.results.turns", json!({ "node": nodes[6] }));
    t.ok("transport.results.routes", json!({}));
    t.ok("transport.results.sites", json!({}));
    t.ok("transport.results.od", json!({}));
    t.ok("transport.results.trip_ends", json!({}));
    t.err(
        "transport.results.summary",
        json!({ "scenario": "nonsense" }),
        "not_found",
    );

    // ---- analysis
    t.ok(
        "transport.analysis.travel",
        json!({ "from": "z:zA", "to": "z:zB" }),
    );
    t.ok("transport.analysis.isochrone", json!({ "from": nodes[0] }));
    t.ok(
        "transport.analysis.isochrone",
        json!({ "from": nodes[0], "mode": "walk" }),
    );
    t.ok(
        "transport.analysis.isochrone",
        json!({ "from": nodes[0], "mode": "cycle" }),
    );
    t.ok(
        "transport.analysis.select_link",
        json!({ "link": links[3] }),
    );
    t.ok("transport.analysis.export_ready", json!({}));
    t.ok("transport.analysis.lts", json!({}));
    t.ok("transport.analysis.capital", json!({ "scenario": "s-opt" }));
    t.ok(
        "transport.analysis.difference",
        json!({ "base": "s-dm", "scenario": "s-opt" }),
    );
    t.ok(
        "transport.analysis.with_without",
        json!({ "without": "s-dm", "scenario": "s-opt" }),
    );
    t.ok(
        "transport.analysis.extent_test",
        json!({ "without": "s-dm", "scenario": "s-opt" }),
    );
    t.ok(
        "transport.analysis.dev_distribution",
        json!({ "scenario": "s-opt" }),
    );
    t.ok(
        "transport.analysis.econ",
        json!({ "base": "s-dm", "scenario": "s-opt" }),
    );
    t.ok("transport.analysis.compare", json!({}));
    t.ok(
        "transport.analysis.msm_check",
        json!({ "base": "s-dm", "scenario": "s-opt" }),
    );
    let events;
    {
        // the sensitivity pack reports progress while it runs
        let counter = std::rc::Rc::new(std::cell::Cell::new(0));
        let c2 = counter.clone();
        t.s.set_progress(move |_| c2.set(c2.get() + 1));
        t.ok(
            "transport.analysis.sensitivity",
            json!({ "scenario": "s-opt", "periods": ["AM"], "tests": ["T1+", "T2"] }),
        );
        events = counter.get();
        t.s.clear_progress();
    }
    assert!(events >= 3, "progress events: {events}");
    t.ok("transport.model.status", json!({}));
    t.ok("transport.model.meta", json!({}));

    // ---- turns
    t.ok("transport.turns.movements", json!({ "id": nodes[6] }));
    t.ok("transport.turns.junction_name", json!({ "node": nodes[6] }));
    assert!(
        t.ok("transport.turns.diagram", json!({ "node": nodes[6] }))["svg"]
            .as_str()
            .unwrap()
            .starts_with("<svg")
    );
    t.ok("transport.turns.count_csv", json!({ "node": nodes[6] }));
    t.ok(
        "transport.turns.sidra_csv",
        json!({ "codes": true, "force": true }),
    );

    // ---- origin-destination matrices
    let od = t.ok("transport.export.od_csv", json!({}));
    let text = od["text"].as_str().unwrap().to_string();
    t.ok("transport.demand.od_import", json!({ "csv": text }));
    t.ok("transport.demand.od_clear", json!({}));
    t.err("transport.demand.od_clear", json!({}), "invalid_state");

    // ---- the simulation (the model is run first when the result is out of date)
    t.ok(
        "transport.run",
        json!({ "scenario": base, "periods": ["AM"] }),
    );
    let sim = t.ok(
        "transport.sim.create",
        json!({ "scenario": base, "period": "AM", "seed": 9 }),
    );
    assert!(sim["routes"].as_u64().unwrap() > 0);
    t.ok("transport.sim.step", json!({ "seconds": 600 }));
    assert!(
        t.ok("transport.sim.hud", json!({}))["released"]
            .as_u64()
            .unwrap()
            > 0
    );
    t.ok("transport.sim.series", json!({ "last": 10 }));
    assert_eq!(
        t.ok("transport.sim.validation", json!({}))["available"],
        json!(true)
    );
    assert!(
        t.ok("transport.sim.vehicles", json!({ "limit": 5 }))["total"]
            .as_u64()
            .unwrap()
            > 0
    );
    t.ok("transport.sim.restart", json!({}));
    t.ok("transport.sim.stop", json!({}));
    t.err("transport.sim.hud", json!({}), "invalid_state");

    // ---- calibration
    let csv = "lat,lng,am,pm,adt,name,dir\n\
        -36.9,174.7615,500,520,9000,Row 0 0,\n\
        -36.9,174.7645,480,500,8800,Row 0 1,\n\
        -36.9,174.7675,470,480,8600,Row 0 2,\n\
        -36.9025,174.7615,420,430,7600,Row 1 0,\n\
        -36.9025,174.7645,410,420,7400,Row 1 1,\n\
        -36.9025,174.7675,400,410,7200,Row 1 2,\n\
        -36.905,174.7615,300,310,5400,Row 2 0,\n\
        -36.905,174.7645,310,300,5500,Row 2 1,\n\
        -36.905,174.7675,290,300,5300,Row 2 2,\n\
        -36.9075,174.7615,200,210,3600,Row 3 0,\n\
        -36.9075,174.7645,210,200,3700,Row 3 1,\n\
        -36.9075,174.7675,190,200,3500,Row 3 2\n";
    assert_eq!(
        t.ok(
            "transport.calibration.counts_import_csv",
            json!({ "csv": csv })
        )["imported"],
        json!(12)
    );
    t.ok("transport.calibration.counts_add", json!({ "source": "user", "counts": [{ "id": "extra-1", "src": "user", "name": "Extra", "lat": -36.9, "lng": 174.7585, "am": 450, "pm": 440, "adt": 8000 }] }));
    let counts = t.ok("transport.calibration.counts", json!({}));
    assert_eq!(counts["total"], json!(13));
    t.ok(
        "transport.calibration.set_role",
        json!({ "count": "extra-1", "role": "off" }),
    );
    t.err(
        "transport.calibration.set_role",
        json!({ "count": "nope", "role": "off" }),
        "not_found",
    );
    let ho = t.ok("transport.calibration.hold_out", json!({ "every": 4 }));
    assert!(ho["validation"].as_u64().unwrap() >= 1);
    t.ok(
        "transport.run",
        json!({ "scenario": base, "periods": ["AM", "PM"] }),
    );
    let sum = t.ok("transport.calibration.summary", json!({}));
    assert_eq!(sum["run"], json!(true));
    assert!(sum["matched"].as_u64().unwrap() >= 10);
    t.ok(
        "transport.calibration.rows",
        json!({ "set": "calib", "limit": 5 }),
    );
    t.ok("transport.calibration.report", json!({}));
    t.ok("transport.calibration.assess", json!({}));
    t.ok("transport.calibration.network_checks", json!({}));
    t.err(
        "transport.calibration.me_report",
        json!({}),
        "invalid_state",
    );
    let passes;
    {
        let counter = std::rc::Rc::new(std::cell::Cell::new(0));
        let c2 = counter.clone();
        t.s.set_progress(move |_| c2.set(c2.get() + 1));
        let fit = t.ok(
            "transport.calibration.fit",
            json!({ "iters": 3, "weights": true }),
        );
        assert!(fit["fit"]["note"].is_string());
        passes = counter.get();
        t.s.clear_progress();
    }
    assert!(passes >= 2, "fit progress events: {passes}");
    t.ok("transport.calibration.me_report", json!({}));
    t.ok("transport.calibration.trip_end_factors", json!({}));
    t.ok(
        "transport.calibration.trip_end_factors_can_apply",
        json!({}),
    );
    t.ok("transport.calibration.trip_end_factors_apply", json!({}));
    t.ok("transport.calibration.trip_end_factors_revert", json!({}));
    t.ok("transport.calibration.run_me", json!({ "period": "PM" }));
    t.ok("transport.calibration.validate", json!({}));
    t.ok("transport.calibration.refresh_validation", json!({}));
    t.ok("transport.calibration.gateways_from_counts", json!({}));
    t.ok(
        "transport.run",
        json!({ "scenario": base, "periods": ["AM"] }),
    );
    let sl = t.ok(
        "transport.calibration.screenline_add",
        json!({ "name": "Middle", "points": [[-36.9013, 174.759], [-36.9013, 174.773]] }),
    );
    t.ok("transport.calibration.screenlines", json!({}));
    t.ok(
        "transport.calibration.screenline_rows",
        json!({ "id": sl["id"] }),
    );
    for kind in ["counts", "summary", "screenlines", "turns"] {
        assert!(t.ok("transport.calibration.export", json!({ "kind": kind }))["text"].is_string());
    }
    t.ok(
        "transport.calibration.screenline_delete",
        json!({ "id": sl["id"] }),
    );
    let turn_csv = "site_id,site_name,lat,lng,date,time_start,interval_min,from_leg,movement,class,count\n\
        T1,Centre,-36.9025,174.763,2026-03-10,07:30,15,W,T,LV,120\n\
        T1,Centre,-36.9025,174.763,2026-03-10,07:30,15,W,L,LV,30\n\
        T1,Centre,-36.9025,174.763,2026-03-10,07:45,15,W,T,LV,130\n\
        T1,Centre,-36.9025,174.763,2026-03-10,08:00,15,W,T,LV,140\n\
        T1,Centre,-36.9025,174.763,2026-03-10,08:15,15,W,T,LV,125\n";
    let tc = t.ok(
        "transport.calibration.turn_counts_import",
        json!({ "csv": turn_csv, "name": "centre.csv" }),
    );
    assert_eq!(tc["sites"], json!(1));
    t.ok(
        "transport.calibration.stress_test",
        json!({ "factors": [1.1, 1.2] }),
    );
    t.ok("transport.calibration.clear", json!({ "period": "AM" }));
    t.ok("transport.calibration.reset_roles", json!({}));

    // ---- every command in the registry was called
    let all: BTreeSet<String> = registry()
        .all()
        .iter()
        .map(|c| c.name.to_string())
        .collect();
    let missing: Vec<&String> = all.difference(&t.called).collect();
    assert!(
        missing.is_empty(),
        "commands the script never called: {missing:?}"
    );
    let unknown: Vec<&String> = t.called.difference(&all).collect();
    assert!(
        unknown.is_empty(),
        "commands called that are not registered: {unknown:?}"
    );
}
