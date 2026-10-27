//! Network commands: read the network, draw roads, split, delete, reverse, junction control, zones, gateways, bus routes and field changes.
//!
//! The compositions are ported from the app: `finishRoad`, `finishBus`, the zone and gateway tools, `deleteSelected` and the
//! drag-to-move step of ui.js, and `applyField` of ui-inspector.js (with the rules it applies after a field changes).

use super::Session;
use super::helpers::{
    from_jv, kit_get_path, kit_set_path, link_props, parse_num, r7, road_template, to_jv, val,
};
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::edit::IdSource;
use construct_transport::edit::ROUTE_COLOURS;
use construct_transport::edit::json::JsonValue;
use construct_transport::edit::net::{self, NodeProps};
use construct_transport::jsnum::{Maybe, Num, math_max, math_round, to_int32};
use construct_transport::project::{
    CLASSES, Link, Node, Route, Scenario, Zone, road_class, road_class_exact,
};
use construct_transport::util::LatLng;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let kind4 = || {
        choice(
            &["node", "link", "zone", "route"],
            "What kind of thing: node (junction), link (road), zone (zone or gateway) or route (bus route).",
        )
    };
    r.add(
        "transport.network.list",
        "List the junctions, roads, zones (and gateways) or bus routes of a scenario, optionally inside a box or by id.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("kind", kind4()),
                ("ids", strings("Only these ids.")),
                ("bbox", array(json!({ "type": "number" }), "Only things inside [south, west, north, east].")),
                ("limit", integer("Most items to return (default 200).")),
                ("offset", integer("Items to skip (default 0).")),
            ],
            &["kind"],
        ),
        network_list,
    );
    r.add(
        "transport.network.get",
        "Read one junction, road, zone or bus route with the facts around it (road length, the roads at a junction, a route's stops).",
        false,
        params(vec![("scenario", sc()), ("kind", kind4()), ("id", string("Its id."))], &["kind", "id"]),
        network_get,
    );
    r.add(
        "transport.network.summary",
        "Count the junctions, roads, zones, gateways and bus routes of a scenario, with road length by class and the proposed length.",
        false,
        params(vec![("scenario", sc())], &[]),
        network_summary,
    );
    r.add(
        "transport.network.nearest",
        "Find the junction or road nearest to a point, within a distance.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("kind", choice(&["node", "link"], "node or link.")),
                ("point", latlng("[latitude, longitude].")),
                ("maxM", number("Search distance in metres (default 100).")),
                (
                    "connectedOnly",
                    boolean("For junctions: only those with at least one road (default true)."),
                ),
            ],
            &["kind", "point"],
        ),
        network_nearest,
    );
    r.add(
        "transport.network.add_node",
        "Add a junction at a position (uncontrolled unless a control is given).",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("point", latlng("[latitude, longitude].")),
                (
                    "props",
                    free_object("Other junction fields, for example { \"control\": \"signals\" }."),
                ),
            ],
            &["point"],
        ),
        network_add_node,
    );
    r.add(
        "transport.network.add_link",
        "Add one road between two existing junctions, with optional shape points and road fields (class, lanes each way, speed, status, name...).",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("a", string("Junction id at the start.")),
                ("b", string("Junction id at the end.")),
                ("points", array(latlng("A shape point."), "Shape points between the two junctions, [[lat, lng], ...].")),
                ("props", free_object("Road fields, for example { \"cls\": \"collector\", \"lanesAB\": 1, \"lanesBA\": 1, \"speed\": 50, \"status\": \"proposed\", \"name\": \"New Road\" }.")),
            ],
            &["a", "b"],
        ),
        network_add_link,
    );
    r.add(
        "transport.network.add_road",
        "Draw a road along points as the Draw road tool does: ends and clicks on roads snap or split, new junctions are made, roads are added between them and junction control is set; one undo step.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("points", array(road_point(), "The points in order: at least two. Each has lat and lng; type is node (with id), link (a point on a road, which splits it), free, or auto (default free).")),
                ("snapM", number("For type auto: snap to a junction or road within this many metres (default 8).")),
                ("cls", string("Road class (default arterial).")),
                ("lanes", integer("Lanes each way (default 1).")),
                ("oneway", boolean("One way, from the first point to the last (default false).")),
                ("speed", number("Speed limit in km/h (default 50).")),
                ("status", choice(&["existing", "proposed", "closed"], "Status (default proposed).")),
            ],
            &["points"],
        ),
        network_add_road,
    );
    r.add(
        "transport.network.split",
        "Split a road at the point nearest a position and give the junction there (within 2.5 m of an end no split is made).",
        true,
        params(vec![("scenario", sc()), ("link", string("Road id.")), ("point", latlng("[latitude, longitude]."))], &["link", "point"]),
        network_split,
    );
    r.add(
        "transport.network.delete",
        "Delete a road, junction, zone (or gateway) or bus route as the app's Delete does: a road leaves its unused junctions pruned, a junction takes its roads with it.",
        true,
        params(vec![("scenario", sc()), ("kind", kind4()), ("id", string("Its id."))], &["kind", "id"]),
        network_delete,
    );
    r.add(
        "transport.network.reverse",
        "Swap the lanes each way of a road (a one-way road then runs the other way).",
        true,
        params(
            vec![("scenario", sc()), ("link", string("Road id."))],
            &["link"],
        ),
        network_reverse,
    );
    r.add(
        "transport.network.auto_control",
        "Set default junction control: three or more roads and no control give give-way control; fewer than three lose give-way or stop control.",
        true,
        params(vec![("scenario", sc()), ("nodes", strings("Junction ids to look at (default all)."))], &[]),
        network_auto_control,
    );
    r.add(
        "transport.network.move",
        "Move a junction or zone to a new position, as dragging it does.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("kind", choice(&["node", "zone"], "node or zone.")),
                ("id", string("Its id.")),
                ("point", latlng("The new position [latitude, longitude].")),
            ],
            &["kind", "id", "point"],
        ),
        network_move,
    );
    r.add(
        "transport.network.set",
        "Change fields of a road or junction as the panel does: a road's class resets its speed when the speed was the class default, a road needs a lane, a bus lane needs two lanes, a control sets the upgrade flag. One undo step.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("kind", choice(&["link", "node"], "link (road) or node (junction).")),
                ("id", string("Its id.")),
                ("values", free_object("Fields to set: { \"cls\": \"collector\", \"lanesAB\": 2 } or { \"control\": \"signals\" }. Numbers may be text such as 12,000.")),
            ],
            &["kind", "id", "values"],
        ),
        |s, p| {
            let kind = match p.str("kind")? {
                "link" => FieldKind::Link,
                _ => FieldKind::Node,
            };
            set_fields(s, p, kind)
        },
    );
    r.add(
        "transport.zone.add",
        "Add a land-use zone at a position as the Zone tool does (200 houses unless land use is given).",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("point", latlng("[latitude, longitude].")),
                ("name", string("Name (default Zone N).")),
                ("lu", free_object("Land use quantities by key, for example { \"residential\": 120, \"retail\": 800 } (default 200 houses).")),
                ("props", free_object("Other zone fields, for example { \"role\": \"development\", \"connectors\": 2 }.")),
            ],
            &["point"],
        ),
        zone_add,
    );
    r.add(
        "transport.zone.add_gateway",
        "Add a gateway where traffic enters or leaves the study area, with flows estimated from the road class and lanes as the Gateway tool does.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("node", string("Junction id at the end of the road (default: the nearest connected junction to point).")),
                ("point", latlng("Where to place the gateway; with no node, the nearest junction within maxM is used (default: the junction's position).")),
                ("maxM", number("Search distance in metres when no node is given (default 80).")),
            ],
            &[],
        ),
        zone_add_gateway,
    );
    r.add(
        "transport.zone.set",
        "Change fields of a zone or gateway as the panel does (name, land use as lu.residential, gateway flows as gw.AMin, role, access lines). One undo step.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("id", string("Zone or gateway id.")),
                ("values", free_object("Fields to set: { \"name\": \"North\", \"lu.residential\": 300, \"gw.AMin\": 450 }.")),
            ],
            &["id", "values"],
        ),
        |s, p| set_fields(s, p, FieldKind::Zone),
    );
    r.add(
        "transport.zone.delete",
        "Delete a zone or gateway.",
        true,
        params(
            vec![("scenario", sc()), ("id", string("Zone or gateway id."))],
            &["id"],
        ),
        |s, p| delete_kind(s, p, "zone"),
    );
    r.add(
        "transport.route.add",
        "Add a bus route through junctions in order, at 6 buses an hour unless given, as the Bus route tool does.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("stops", strings("Junction ids in order along the route: at least two.")),
                ("name", string("Name (default Bus route N).")),
                ("color", string("Map colour such as #7048e8 (default from the route colours in turn).")),
                ("freq", number("Buses an hour (default 6).")),
            ],
            &["stops"],
        ),
        route_add,
    );
    r.add(
        "transport.route.set",
        "Change fields of a bus route (name, freq, color). One undo step.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("id", string("Route id.")),
                ("values", free_object("Fields to set: { \"freq\": 10 }.")),
            ],
            &["id", "values"],
        ),
        |s, p| set_fields(s, p, FieldKind::Route),
    );
    r.add(
        "transport.route.delete",
        "Delete a bus route.",
        true,
        params(
            vec![("scenario", sc()), ("id", string("Route id."))],
            &["id"],
        ),
        |s, p| delete_kind(s, p, "route"),
    );
}

fn road_point() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "enum": ["node", "link", "free", "auto"], "description": "node (with id), link, free or auto." },
            "id": { "type": "string", "description": "Junction id, for type node." },
            "lat": { "type": "number", "description": "Latitude." },
            "lng": { "type": "number", "description": "Longitude." }
        },
        "required": ["lat", "lng"],
        "additionalProperties": false
    })
}

// ------------------------------------------------------------------ helpers

fn missing_end(sc: &Scenario, l: &Link) -> bool {
    !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b)
}

/// `Kit.junctionName(id, sc)`: the road names at a junction (up to three), or "Unnamed junction".
pub(super) fn kit_junction_name(sc: &Scenario, id: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    for l in net::links_at(sc, id) {
        if let Some(n) = l.name.as_str().filter(|n| !n.is_empty())
            && !names.contains(&n)
        {
            names.push(n);
        }
    }
    if names.is_empty() {
        "Unnamed junction".to_string()
    } else {
        names.into_iter().take(3).collect::<Vec<_>>().join(" / ")
    }
}

fn non_empty(m: &Maybe<String>) -> Option<&str> {
    m.as_str().filter(|n| !n.is_empty())
}

fn kind_name(kind: &str) -> &'static str {
    match kind {
        "link" => "road",
        "node" => "junction",
        "zone" => "zone",
        _ => "bus route",
    }
}

fn in_box(ll: LatLng, b: &[f64]) -> bool {
    ll[0] >= b[0] && ll[0] <= b[2] && ll[1] >= b[1] && ll[1] <= b[3]
}

fn link_json(sc: &Scenario, l: &Link) -> Value {
    let mut v = val(l);
    if !missing_end(sc, l) {
        v["lengthM"] = json!(construct_transport::util::r1(sc.link_len(l)));
    }
    v
}

// ------------------------------------------------------------------ reads

fn network_list(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let kind = p.str("kind")?;
    let ids: Option<HashSet<String>> = p.opt_strings("ids")?.map(|v| v.into_iter().collect());
    let bbox: Option<Vec<f64>> = match p.opt_array("bbox")? {
        None => None,
        Some(a) => {
            let v: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            if v.len() != 4 {
                return Err(ApiError::bad_params(
                    "`bbox` must be [south, west, north, east]",
                ));
            }
            Some(v)
        }
    };
    let limit = p.opt_f64("limit")?.map_or(200, |x| x.max(0.0) as usize);
    let offset = p.opt_f64("offset")?.map_or(0, |x| x.max(0.0) as usize);
    let keep = |id: &str| ids.as_ref().is_none_or(|s| s.contains(id));
    let mut items: Vec<Value> = Vec::new();
    let mut total = 0usize;
    let mut push = |v: Value| {
        if total >= offset && items.len() < limit {
            items.push(v);
        }
        total += 1;
    };
    match kind {
        "node" => {
            for n in sc
                .nodes
                .values()
                .filter(|n| keep(&n.id) && bbox.as_ref().is_none_or(|b| in_box(n.ll(), b)))
            {
                push(val(n));
            }
        }
        "link" => {
            for l in sc.links.values().filter(|l| keep(&l.id)) {
                if let Some(b) = &bbox {
                    if missing_end(sc, l) {
                        continue;
                    }
                    if !sc.geom(l).iter().any(|q| in_box(*q, b)) {
                        continue;
                    }
                }
                push(link_json(sc, l));
            }
        }
        "zone" => {
            for z in sc
                .zones
                .values()
                .filter(|z| keep(&z.id) && bbox.as_ref().is_none_or(|b| in_box(z.ll(), b)))
            {
                push(val(z));
            }
        }
        _ => {
            for r in sc.routes.values().filter(|r| keep(&r.id)) {
                push(val(r));
            }
        }
    }
    Ok(
        json!({ "scenario": sc.id, "kind": kind, "total": total, "offset": offset, "count": items.len(), "items": items }),
    )
}

fn network_get(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let (kind, id) = (p.str("kind")?, p.str("id")?);
    let nf = || {
        ApiError::not_found(format!(
            "There is no {} {id} in this scenario.",
            kind_name(kind)
        ))
    };
    match kind {
        "node" => {
            let n = sc.nodes.get(id).ok_or_else(nf)?;
            let roads: Vec<Value> = net::links_at(sc, id)
                .iter()
                .map(|l| json!({ "id": l.id, "name": l.name.as_str(), "cls": l.cls.as_str(), "a": l.a, "b": l.b }))
                .collect();
            let site = construct_transport::edit::sites::of(sc, id).map(|x| x.id.clone());
            Ok(
                json!({ "node": val(n), "name": kit_junction_name(sc, id), "roads": roads, "site": site }),
            )
        }
        "link" => {
            let l = sc.links.get(id).ok_or_else(nf)?;
            let geometry = if missing_end(sc, l) {
                Value::Null
            } else {
                json!(sc.geom(l))
            };
            Ok(json!({ "link": link_json(sc, l), "geometry": geometry }))
        }
        "zone" => Ok(json!({ "zone": val(sc.zones.get(id).ok_or_else(nf)?) })),
        _ => {
            let r = sc.routes.get(id).ok_or_else(nf)?;
            let stops: Vec<Value> = r
                .wp
                .get()
                .map(|w| w.iter().map(|n| json!({ "id": n, "name": if sc.nodes.contains_key(n) { Value::String(kit_junction_name(sc, n)) } else { Value::Null } })).collect())
                .unwrap_or_default();
            Ok(json!({ "route": val(r), "stops": stops }))
        }
    }
}

fn network_summary(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let mut km = 0.0;
    let mut proposed = 0.0;
    let mut by_class: Vec<(String, f64, usize)> = CLASSES
        .iter()
        .map(|c| (c.key.to_string(), 0.0, 0))
        .collect();
    let mut controls: Vec<(String, usize)> = Vec::new();
    for l in sc.links.values() {
        if missing_end(sc, l) {
            continue;
        }
        let len = sc.link_len(l) / 1000.0;
        km += len;
        if l.status.is("proposed") {
            proposed += len;
        }
        let key = road_class(l.cls.as_str()).key;
        if let Some(e) = by_class.iter_mut().find(|e| e.0 == key) {
            e.1 += len;
            e.2 += 1;
        }
    }
    for n in sc.nodes.values() {
        let c = n.control.as_str().unwrap_or("none").to_string();
        match controls.iter_mut().find(|e| e.0 == c) {
            Some(e) => e.1 += 1,
            None => controls.push((c, 1)),
        }
    }
    let gateways = sc.zones.values().filter(|z| z.kind.is("gateway")).count();
    Ok(json!({
        "scenario": sc.id, "name": sc.name.as_str(),
        "nodes": sc.nodes.len(), "links": sc.links.len(), "zones": sc.zones.len() - gateways, "gateways": gateways, "routes": sc.routes.len(),
        "lengthKm": km, "proposedKm": proposed,
        "byClass": by_class.iter().map(|(k, l, n)| json!({ "cls": k, "links": n, "lengthKm": l })).collect::<Vec<_>>(),
        "controls": controls.iter().map(|(k, n)| json!({ "control": k, "nodes": n })).collect::<Vec<_>>(),
    }))
}

fn network_nearest(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let at = p.latlng("point")?;
    let max_m = p.opt_f64("maxM")?.unwrap_or(100.0);
    if p.str("kind")? == "node" {
        let connected = p.opt_bool("connectedOnly")?.unwrap_or(true);
        let deg = net::degree(sc);
        let f = move |n: &Node| !connected || deg.get(&n.id).copied().unwrap_or(0) > 0;
        let id = net::nearest_node(sc, at, max_m, Some(&f));
        Ok(match id.and_then(|id| sc.nodes.get(&id)) {
            Some(n) => {
                json!({ "found": true, "id": n.id, "point": n.ll(), "distanceM": construct_transport::util::dist(&at, &n.ll()), "name": kit_junction_name(sc, &n.id) })
            }
            None => json!({ "found": false }),
        })
    } else {
        Ok(match net::nearest_link(sc, at, max_m, None) {
            Some(nl) => json!({ "found": true, "id": nl.id, "point": nl.p, "distanceM": nl.d }),
            None => json!({ "found": false }),
        })
    }
}

mod edit;
mod fields;
use edit::*;
use fields::*;
pub(super) use fields::{FieldKind, apply_fields};
