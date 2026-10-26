//! GeoJSON and CSV in and out: model.js `Importers.importGeoJSON`, `geojson` and `linksCSV`.
//!
//! [`import_geojson`] adds the line features of a GeoJSON file as roads, snapping their ends to a junction within 2 m
//! or making one. [`geojson`] writes the network (roads with their results, controlled junctions, zones) as a
//! FeatureCollection, and [`links_csv`] writes one row per road direction, both with the results of a run when given.

use crate::edit::IdSource;
use crate::edit::json::JsonValue;
use crate::edit::net::{self, LinkProps, NodeProps};
use crate::edit::{STATUS, name_of};
use crate::jsnum::{Maybe, Num, math_round, number_to_js_string, value_to_number, value_truthy};
use crate::project::{Scenario, road_class_exact};
use crate::results::Res;
use crate::util::{self, LatLng};
use serde_json::Value;

/// `Importers.importGeoJSON(sc, gj, template)`: adds the LineString and MultiLineString features of a GeoJSON value as roads and
/// returns how many. The template gives the class, lanes, speed and status a feature does not set itself (`cls`, `lanesAB`,
/// `lanesBA`, `speed`, `status` of its props; any other key it has is copied onto each road). A feature's own `cls`, `name`,
/// `speed`, `lanesAB`, `lanesBA` and `status` properties are read. Afterwards default junction control is applied everywhere.
pub fn import_geojson(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    gj: &Value,
    template: &LinkProps,
) -> usize {
    let single;
    let feats: Vec<&Value> = match gj.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => gj
            .get("features")
            .and_then(Value::as_array)
            .map_or(Vec::new(), |a| a.iter().collect()),
        Some("Feature") => vec![gj],
        _ => {
            single = serde_json::json!({ "type": "Feature", "geometry": gj, "properties": {} });
            vec![&single]
        }
    };
    let mut added = 0usize;
    for f in feats {
        let Some(g) = f.get("geometry").filter(|g| value_truthy(g)) else {
            continue;
        };
        let lines: Vec<&Value> = match g.get("type").and_then(Value::as_str) {
            Some("LineString") => g.get("coordinates").into_iter().collect(),
            Some("MultiLineString") => g
                .get("coordinates")
                .and_then(Value::as_array)
                .map_or(Vec::new(), |a| a.iter().collect()),
            _ => Vec::new(),
        };
        let empty = serde_json::json!({});
        let p = f
            .get("properties")
            .filter(|p| value_truthy(p))
            .unwrap_or(&empty);
        for line in lines {
            let Some(coords) = line.as_array() else {
                continue;
            };
            if coords.len() < 2 {
                continue;
            }
            // [lng, lat] to [lat, lng]
            let ll: Vec<LatLng> = coords
                .iter()
                .map(|c| {
                    [
                        value_to_number(c.get(1).unwrap_or(&Value::Null)),
                        value_to_number(c.get(0).unwrap_or(&Value::Null)),
                    ]
                })
                .collect();
            let (a, b) = {
                let first = snap(sc, ids, ll[0]);
                let last = snap(sc, ids, ll[ll.len() - 1]);
                (first, last)
            };
            if a == b {
                continue;
            }
            let t = &template.0;
            let mut props = template.clone();
            // const cls = CLASSES[p.cls] ? p.cls : template.cls
            props.0.cls = match p.get("cls").and_then(Value::as_str) {
                Some(c) if road_class_exact(c).is_some() => Maybe::Set(c.to_string()),
                _ => t.cls.clone(),
            };
            // name: p.name || ''
            props.0.name = Maybe::Set(match p.get("name") {
                Some(n) if value_truthy(n) => crate::jsnum::value_to_js_string(n),
                _ => String::new(),
            });
            // speed: +p.speed || template.speed
            let sp = p.get("speed").map_or(f64::NAN, value_to_number);
            props.0.speed = if sp != 0.0 && !sp.is_nan() {
                Num::number(sp)
            } else {
                t.speed.clone()
            };
            props.0.lanes_ab = match p.get("lanesAB") {
                Some(v) if !v.is_null() => Num::number(value_to_number(v)),
                _ => t.lanes_ab.clone(),
            };
            props.0.lanes_ba = match p.get("lanesBA") {
                Some(v) if !v.is_null() => Num::number(value_to_number(v)),
                _ => t.lanes_ba.clone(),
            };
            props.0.status = match p.get("status").and_then(Value::as_str) {
                Some(s) if name_of(&STATUS, s).is_some() => Maybe::Set(s.to_string()),
                _ => t.status.clone(),
            };
            net::add_link(sc, ids, &a, &b, &ll[1..ll.len() - 1], &props);
            added += 1;
        }
    }
    let all: Vec<String> = sc.nodes.keys().map(str::to_string).collect();
    net::auto_control(sc, &all);
    added
}

/// `snap(ll)`: the junction within 2 m, or a new one.
fn snap(sc: &mut Scenario, ids: &mut dyn IdSource, ll: LatLng) -> String {
    match net::nearest_node(sc, ll, 2.0, None) {
        Some(id) => id,
        None => net::add_node(sc, ids, ll, &NodeProps::new()),
    }
}

fn text_json(m: &Maybe<String>) -> Option<JsonValue> {
    match m {
        Maybe::Missing => None,
        Maybe::Null => Some(JsonValue::Null),
        Maybe::Set(s) => Some(s.as_str().into()),
    }
}

fn num_json(n: &Num) -> Option<JsonValue> {
    match n {
        Maybe::Missing => None,
        Maybe::Null => Some(JsonValue::Null),
        Maybe::Set(v) => Some(JsonValue::from_value(&v.to_value())),
    }
}

fn put(o: &mut JsonValue, k: &str, v: Option<JsonValue>) {
    if let Some(v) = v {
        o.set(k, v);
    }
}

fn opt_num(x: Option<f64>) -> JsonValue {
    x.map_or(JsonValue::Null, JsonValue::Number)
}

/// `Importers.geojson(sc, res)`: the network as GeoJSON text (`JSON.stringify(x, null, 1)`): every road as a LineString with its
/// class, lanes, speed, length and, with results, volume, volume-to-capacity ratio and speed each way; every junction that has a
/// control as a Point with its delay and level of service; every zone as a Point with its land use or gateway flows.
pub fn geojson(sc: &Scenario, res: Option<&Res>) -> String {
    let mut features: Vec<JsonValue> = Vec::new();
    for l in sc.links.values() {
        let g = sc.geom(l);
        let a = res.and_then(|r| r.arcs.get(&format!("{}:ab", l.id)));
        let b = res.and_then(|r| r.arcs.get(&format!("{}:ba", l.id)));
        let mut props = JsonValue::object();
        props.set("id", l.id.as_str().into());
        put(&mut props, "name", text_json(&l.name));
        put(&mut props, "cls", text_json(&l.cls));
        put(&mut props, "status", text_json(&l.status));
        put(&mut props, "speed", num_json(&l.speed));
        put(&mut props, "lanesAB", num_json(&l.lanes_ab));
        put(&mut props, "lanesBA", num_json(&l.lanes_ba));
        put(&mut props, "busLane", num_json(&l.bus_lane));
        put(&mut props, "cycle", text_json(&l.cycle));
        props.set("length_m", math_round(util::poly_len(&g)).into());
        props.set("vol_ab", opt_num(a.map(|x| x.v)));
        props.set("vc_ab", opt_num(a.map(|x| x.vc)));
        props.set("spd_ab", opt_num(a.map(|x| x.spd)));
        props.set("vol_ba", opt_num(b.map(|x| x.v)));
        props.set("vc_ba", opt_num(b.map(|x| x.vc)));
        props.set("spd_ba", opt_num(b.map(|x| x.spd)));
        let coords: Vec<JsonValue> = g
            .iter()
            .map(|p| JsonValue::Array(vec![p[1].into(), p[0].into()]))
            .collect();
        let mut geom = JsonValue::object();
        geom.set("type", "LineString".into());
        geom.set("coordinates", JsonValue::Array(coords));
        let mut f = JsonValue::object();
        f.set("type", "Feature".into());
        f.set("geometry", geom);
        f.set("properties", props);
        features.push(f);
    }
    for n in sc.nodes.values() {
        if n.control.is("none") {
            continue;
        }
        let r = res.and_then(|r| r.nodes.get(&n.id));
        let mut props = JsonValue::object();
        props.set("id", n.id.as_str().into());
        put(&mut props, "control", text_json(&n.control));
        props.set("delay_s", opt_num(r.map(|x| x.delay)));
        props.set("los", r.map_or(JsonValue::Null, |x| x.los.as_str().into()));
        let mut geom = JsonValue::object();
        geom.set("type", "Point".into());
        geom.set(
            "coordinates",
            JsonValue::Array(vec![
                num_json(&n.lng).unwrap_or(JsonValue::Null),
                num_json(&n.lat).unwrap_or(JsonValue::Null),
            ]),
        );
        let mut f = JsonValue::object();
        f.set("type", "Feature".into());
        f.set("geometry", geom);
        f.set("properties", props);
        features.push(f);
    }
    for z in sc.zones.values() {
        let mut props = JsonValue::object();
        props.set("id", z.id.as_str().into());
        put(&mut props, "zone", text_json(&z.name));
        put(&mut props, "kind", text_json(&z.kind));
        if let Some(lu) = z.lu.get() {
            for (k, v) in lu.iter() {
                put(&mut props, k, num_json(v));
            }
        }
        if let Some(gw) = z.gw.get() {
            for (k, v) in gw.iter() {
                put(&mut props, k, num_json(v));
            }
        }
        let mut geom = JsonValue::object();
        geom.set("type", "Point".into());
        geom.set(
            "coordinates",
            JsonValue::Array(vec![
                num_json(&z.lng).unwrap_or(JsonValue::Null),
                num_json(&z.lat).unwrap_or(JsonValue::Null),
            ]),
        );
        let mut f = JsonValue::object();
        f.set("type", "Feature".into());
        f.set("geometry", geom);
        f.set("properties", props);
        features.push(f);
    }
    let mut fc = JsonValue::object();
    fc.set("type", "FeatureCollection".into());
    fc.set("features", JsonValue::Array(features));
    fc.stringify_pretty(1)
}

/// A number cell as `String(x)`.
fn cell_num(x: f64) -> String {
    number_to_js_string(x)
}

fn cell_text(s: &str) -> String {
    crate::util::csv_cell(&Value::String(s.to_string()))
}

fn cell_maybe_text(m: &Maybe<String>) -> String {
    match m {
        Maybe::Set(s) => cell_text(s),
        _ => String::new(),
    }
}

fn cell_num_field(n: &Num) -> String {
    match n {
        Maybe::Set(v) => crate::util::csv_cell(&v.to_value()),
        _ => String::new(),
    }
}

/// `Importers.linksCSV(sc, res)`: one row per road direction that has lanes (link id, name, class, status, direction, lanes,
/// speed limit, length, and with results volume in vehicles and in pcu, capacity, volume-to-capacity ratio, level of service,
/// speed, travel time and junction delay), rows joined by CRLF.
pub fn links_csv(sc: &Scenario, res: Option<&Res>) -> String {
    let head = [
        "link_id",
        "name",
        "class",
        "status",
        "direction",
        "lanes",
        "speed_limit_kmh",
        "length_m",
        "volume_veh_h",
        "volume_pcu_h",
        "capacity_pcu_h",
        "v_c",
        "los",
        "speed_kmh",
        "travel_time_s",
        "junction_delay_s",
    ];
    let mut rows: Vec<String> = vec![
        head.iter()
            .map(|h| cell_text(h))
            .collect::<Vec<_>>()
            .join(","),
    ];
    for l in sc.links.values() {
        for d in ["ab", "ba"] {
            let lanes = if d == "ab" { &l.lanes_ab } else { &l.lanes_ba };
            if !lanes.truthy() {
                continue;
            }
            let r = res.and_then(|r| r.arcs.get(&format!("{}:{}", l.id, d)));
            let mut cells = vec![
                cell_text(&l.id),
                cell_maybe_text(&l.name),
                cell_maybe_text(&l.cls),
                cell_maybe_text(&l.status),
                cell_text(&d.to_uppercase()),
                cell_num_field(lanes),
                cell_num_field(&l.speed),
                cell_num(math_round(sc.link_len(l))),
            ];
            match r {
                Some(r) => {
                    for x in [r.v, r.pcu, r.cap, r.vc] {
                        cells.push(cell_num(x));
                    }
                    cells.push(cell_text(&r.los));
                    for x in [r.spd, r.t, r.nd] {
                        cells.push(cell_num(x));
                    }
                }
                None => {
                    for _ in 0..8 {
                        cells.push(String::new());
                    }
                }
            }
            rows.push(cells.join(","));
        }
    }
    rows.join("\r\n")
}
