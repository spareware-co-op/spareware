//! Import truth tests: OpenStreetMap (Overpass) answers read into a scenario, gateways, GeoJSON lines read as roads, and the
//! network written as GeoJSON and CSV, against the prototype.
//!
//! tools/transport-truth/gen-import.mjs ran model.js `Importers` on the made-up files in tools/transport-truth/fixtures and
//! wrote expected/import-cases.json; these tests replay the same operations with the same seeded id generator.

mod edit_ops;
mod edit_support;
mod support;

use construct_transport::edit::net::LinkProps;
use construct_transport::edit::sites::at_points;
use construct_transport::importers::{
    Bounds, OsmOpts, OverpassOpts, auto_gateways, export_geojson, import_geojson, links_csv,
    osm_to_scenario, overpass_query,
};
use construct_transport::project::{Link, Project};
use construct_transport::results::Res;
use edit_ops::{Ctx, active_index, apply, ctx_for, project_state, resolve_in, sc_state};
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Value, json};
use std::collections::HashSet;

struct Marks(HashSet<String>);

fn bounds(v: &Value) -> Bounds {
    let n = |i: usize| v[i].as_f64().expect("number");
    Bounds {
        south: n(0),
        west: n(1),
        north: n(2),
        east: n(3),
    }
}

fn template(v: &Value) -> LinkProps {
    let mut j = v.clone();
    let o = j.as_object_mut().expect("template");
    for k in ["id", "a", "b"] {
        o.insert(k.into(), json!(""));
    }
    LinkProps(serde_json::from_value::<Link>(j).expect("template"))
}

fn res_of(name: &str, scenario: &str, period: &str) -> Res {
    let ex = load_json(&format!("expected/{name}.json"));
    serde_json::from_value(ex["scenarios"][scenario]["periods"][period]["res"].clone())
        .expect("a result")
}

/// One operation: the import ones here, the rest by the editing replayer.
fn apply_import(ctx: &mut Ctx, marks: &mut Marks, o: &Value) -> (Value, Option<Value>) {
    let name = o["op"].as_str().expect("op");
    if ![
        "osm",
        "mark",
        "gateways",
        "geojson",
        "openProject",
        "export",
    ]
    .contains(&name)
    {
        return apply(ctx, o);
    }
    let a = resolve_in(ctx, o);
    let i = active_index(&ctx.p);
    let (ret, mode) = match name {
        "osm" => {
            let text = read_truth(&format!("fixtures/{}", a["file"].as_str().expect("file")));
            let data: Value = serde_json::from_str(&text).expect("JSON");
            let opts = OsmOpts {
                sites: a.get("sites").and_then(Value::as_bool) == Some(true),
                points: at_points(&ctx.p),
            };
            let n = osm_to_scenario(&mut ctx.p.scenarios[i], &mut ctx.ids, &data, &opts);
            (json!(n), Some("sc"))
        }
        "mark" => {
            marks.0 = ctx.p.scenarios[i]
                .nodes
                .keys()
                .map(str::to_string)
                .collect();
            (Value::Null, None)
        }
        "gateways" => {
            let only: Option<HashSet<String>> =
                (a.get("only").and_then(Value::as_str) == Some("fresh")).then(|| {
                    ctx.p.scenarios[i]
                        .nodes
                        .keys()
                        .filter(|id| !marks.0.contains(*id))
                        .map(str::to_string)
                        .collect()
                });
            let n = auto_gateways(
                &mut ctx.p.scenarios[i],
                &mut ctx.ids,
                &bounds(&a["bounds"]),
                only.as_ref(),
            );
            (json!(n), Some("sc"))
        }
        "geojson" => {
            let text = read_truth(&format!("fixtures/{}", a["file"].as_str().expect("file")));
            let gj: Value = serde_json::from_str(&text).expect("JSON");
            let n = import_geojson(
                &mut ctx.p.scenarios[i],
                &mut ctx.ids,
                &gj,
                &template(&a["template"]),
            );
            (json!(n), Some("sc"))
        }
        "openProject" => {
            let text = read_truth(&format!(
                "inputs/{}.json",
                a["name"].as_str().expect("name")
            ));
            ctx.p = Project::from_json(&text).expect("a project");
            (Value::Null, Some("project"))
        }
        "export" => {
            let sc = &ctx.p.scenarios[i];
            let res = a
                .get("period")
                .and_then(Value::as_str)
                .map(|p| res_of(a["name"].as_str().expect("name"), &sc.id, p));
            (
                json!({ "geojson": export_geojson(sc, res.as_ref()), "csv": links_csv(sc, res.as_ref()) }),
                None,
            )
        }
        _ => unreachable!(),
    };
    if let Some(b) = a.get("as").and_then(Value::as_str) {
        ctx.env.insert(b.to_string(), ret.clone());
    }
    let state = match mode {
        Some("sc") => Some(sc_state(ctx)),
        Some(_) => Some(project_state(ctx)),
        None => None,
    };
    (ret, state)
}

#[test]
fn imports_and_exports_match_the_prototype() {
    let cases = load_json("expected/import-cases.json");
    let mut ck = Checker::default();
    let mut ops_run = 0usize;
    for case in cases.as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let mut ctx = ctx_for(case);
        let mut marks = Marks(HashSet::new());
        let ops = case["ops"].as_array().expect("ops");
        let steps = case["steps"].as_array().expect("steps");
        assert_eq!(ops.len(), steps.len());
        for (k, (o, step)) in ops.iter().zip(steps).enumerate() {
            let at = format!("{name}[{k}]({})", o["op"].as_str().unwrap_or("?"));
            let (ret, state) = apply_import(&mut ctx, &mut marks, o);
            // The exported text must match character for character
            if o["op"] == "export" {
                ck.same(
                    &format!("{at}.geojson"),
                    &ret["geojson"].as_str().map(str::to_string),
                    &step["ret"]["geojson"].as_str().map(str::to_string),
                );
                ck.same(
                    &format!("{at}.csv"),
                    &ret["csv"].as_str().map(str::to_string),
                    &step["ret"]["csv"].as_str().map(str::to_string),
                );
            } else {
                ck.check(&format!("{at}.ret"), &ret, &step["ret"]);
            }
            match (state, step.get("state")) {
                (Some(s), Some(e)) => ck.check(&format!("{at}.state"), &s, e),
                (None, None) => {}
                _ => ck.same(&format!("{at}: state recorded"), &false, &true),
            }
            ops_run += 1;
        }
    }
    println!("import operations replayed: {ops_run}");
    ck.finish("imports and exports");
}

#[test]
fn the_overpass_query_matches_the_prototype() {
    let qs = load_json("expected/import-queries.json");
    let mut ck = Checker::default();
    for (k, c) in qs.as_array().expect("queries").iter().enumerate() {
        let b = bounds(&c["b"]);
        let opts = OverpassOpts {
            service: c["opts"]["service"].as_bool() == Some(true),
            local: c["opts"]["local"].as_bool(),
        };
        ck.same(
            &format!("query[{k}] {} {}", c["b"], c["opts"]),
            &overpass_query(&b, &opts),
            &c["q"].as_str().expect("text").to_string(),
        );
    }
    ck.finish("Overpass queries");
}
