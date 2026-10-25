//! The edits of the network commands: add, draw, split, delete, reverse, control, move, zones, gateways and bus routes.

use super::*;
use crate::schema::latlng_of;

// ------------------------------------------------------------------ props

fn node_props(o: Option<&Map<String, Value>>) -> ApiResult<NodeProps> {
    let mut m = o.cloned().unwrap_or_default();
    for k in ["id", "lat", "lng"] {
        m.remove(k);
    }
    m.insert("id".to_string(), json!(""));
    let n: Node = serde_json::from_value(Value::Object(m))
        .map_err(|e| ApiError::bad_params(format!("`props` does not fit a junction: {e}")))?;
    Ok(NodeProps(n))
}

fn node_ll(sc: &Scenario, id: &str, what: &str) -> ApiResult<LatLng> {
    sc.nodes
        .get(id)
        .map(Node::ll)
        .ok_or_else(|| ApiError::not_found(format!("There is no junction {id} ({what}).")))
}

// ------------------------------------------------------------------ add

pub(super) fn network_add_node(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let at = p.latlng("point")?;
    let props = node_props(p.opt_object("props")?)?;
    s.snap("new junction");
    let id = net::add_node(&mut s.project.scenarios[i], &mut s.ids, at, &props);
    s.edited();
    Ok(json!({ "id": id, "node": val(&s.project.scenarios[i].nodes[id.as_str()]) }))
}

pub(super) fn network_add_link(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (a, b) = (p.str("a")?, p.str("b")?);
    node_ll(&s.project.scenarios[i], a, "start")?;
    node_ll(&s.project.scenarios[i], b, "end")?;
    let pts: Vec<LatLng> = match p.opt_array("points")? {
        None => Vec::new(),
        Some(v) => v
            .iter()
            .map(|x| latlng_of(x, "points"))
            .collect::<ApiResult<_>>()?,
    };
    let props = link_props(p.opt_object("props")?)?;
    s.snap("new road");
    let id = net::add_link(&mut s.project.scenarios[i], &mut s.ids, a, b, &pts, &props);
    s.edited();
    let sc = &s.project.scenarios[i];
    Ok(json!({ "id": id, "link": link_json(sc, &sc.links[id.as_str()]) }))
}

/// One point of a road being drawn, after snapping: a junction to use, a road to split at a position, or a free position.
struct DrawPt {
    kind: &'static str,
    id: Option<String>,
    ll: LatLng,
}

/// What `finishRoad` makes of each point: a junction, or a shape point.
enum Seq {
    Node(String),
    Ll(LatLng),
}

fn draw_points(sc: &Scenario, p: &Params) -> ApiResult<Vec<DrawPt>> {
    let snap_m = p.opt_f64("snapM")?.unwrap_or(8.0);
    let mut out = Vec::new();
    for (k, v) in p.array("points")?.iter().enumerate() {
        let o = v.as_object().ok_or_else(|| {
            ApiError::bad_params(format!("`points[{k}]` must be an object with lat and lng"))
        })?;
        let num = |key: &str| {
            o.get(key)
                .and_then(Value::as_f64)
                .filter(|x| x.is_finite())
                .ok_or_else(|| {
                    ApiError::bad_params(format!("`points[{k}].{key}` must be a number"))
                })
        };
        let ll = [num("lat")?, num("lng")?];
        let t = o.get("type").and_then(Value::as_str).unwrap_or("free");
        let id = o.get("id").and_then(Value::as_str).map(str::to_string);
        let pt = match t {
            "node" => {
                let id = id.ok_or_else(|| {
                    ApiError::bad_params(format!("`points[{k}]` is a node and needs an id"))
                })?;
                if !sc.nodes.contains_key(&id) {
                    return Err(ApiError::not_found(format!(
                        "There is no junction {id} (points[{k}])."
                    )));
                }
                DrawPt {
                    kind: "node",
                    ll: sc.nodes[id.as_str()].ll(),
                    id: Some(id),
                }
            }
            "link" => DrawPt {
                kind: "link",
                id,
                ll,
            },
            "auto" => {
                if let Some(n) = net::nearest_node(sc, ll, snap_m, None) {
                    DrawPt {
                        kind: "node",
                        ll: sc.nodes[n.as_str()].ll(),
                        id: Some(n),
                    }
                } else if let Some(nl) = net::nearest_link(sc, ll, snap_m, None) {
                    DrawPt {
                        kind: "link",
                        id: Some(nl.id),
                        ll: nl.p,
                    }
                } else {
                    DrawPt {
                        kind: "free",
                        id: None,
                        ll,
                    }
                }
            }
            _ => DrawPt {
                kind: "free",
                id: None,
                ll,
            },
        };
        out.push(pt);
    }
    if out.len() < 2 {
        return Err(ApiError::bad_params("A road needs at least two points."));
    }
    Ok(out)
}

/// `finishRoad()` of ui.js: snaps, splits, adds the roads between junctions and sets junction control.
pub(super) fn network_add_road(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let pts = draw_points(&s.project.scenarios[i], p)?;
    let props = road_template(p)?;
    let cls = props.0.cls.as_str().unwrap_or("arterial").to_string();
    let cls = cls.as_str();
    s.snap("new road");
    let n_pts = pts.len();
    let mut seq: Vec<Seq> = Vec::new();
    let mut failure: Option<ApiError> = None;
    let mut made: Vec<String> = Vec::new();
    let nodes: Vec<String>;
    {
        let Session { project, ids, .. } = &mut *s;
        let sc = &mut project.scenarios[i];
        for (k, pt) in pts.iter().enumerate() {
            if pt.kind == "node" && pt.id.as_ref().is_some_and(|id| sc.nodes.contains_key(id)) {
                seq.push(Seq::Node(pt.id.clone().expect("checked")));
                continue;
            }
            if pt.kind == "link"
                && let Some(nl) = net::nearest_link(sc, pt.ll, 6.0, None)
            {
                match net::split_link(sc, ids, &nl.id, pt.ll) {
                    Ok(n) => {
                        seq.push(Seq::Node(n));
                        continue;
                    }
                    Err(e) => {
                        failure = Some(e.into());
                        break;
                    }
                }
            }
            if k == 0 || k == n_pts - 1 {
                seq.push(Seq::Node(net::add_node(sc, ids, pt.ll, &NodeProps::new())));
            } else {
                seq.push(Seq::Ll(pt.ll));
            }
        }
        if failure.is_none() {
            let mut start = match &seq[0] {
                Seq::Node(n) => n.clone(),
                Seq::Ll(_) => String::new(),
            };
            let mut shape: Vec<LatLng> = Vec::new();
            for st in &seq[1..] {
                match st {
                    Seq::Ll(ll) => shape.push(*ll),
                    Seq::Node(n) => {
                        if *n != start {
                            made.push(net::add_link(sc, ids, &start, n, &shape, &props));
                        }
                        start = n.clone();
                        shape.clear();
                    }
                }
            }
        }
        nodes = seq
            .iter()
            .filter_map(|x| {
                if let Seq::Node(n) = x {
                    Some(n.clone())
                } else {
                    None
                }
            })
            .collect();
        if failure.is_none() {
            net::auto_control(sc, &nodes);
        }
    }
    if let Some(e) = failure {
        s.edited();
        return Err(e);
    }
    if made.is_empty() {
        s.discard_snap();
        s.touch();
        return Ok(
            json!({ "links": [], "nodes": nodes, "message": "No road was added: the points do not run between two different junctions." }),
        );
    }
    let sc = &s.project.scenarios[i];
    let len: f64 = made
        .iter()
        .map(|id| net::len(sc, &sc.links[id.as_str()]))
        .sum();
    let joined = pts.iter().any(|x| x.kind != "free") || sc.links.len() == made.len();
    s.edited();
    let n = made.len();
    let label = road_class(Some(cls)).label.to_lowercase();
    let message = if joined {
        format!(
            "Added {} of {label} road in {n} segment{}.",
            construct_transport::util::fmt::len(len),
            if n > 1 { "s" } else { "" }
        )
    } else {
        format!(
            "Added {} of road, but it isn\u{2019}t connected to the network. Start or end on an existing road to join it.",
            construct_transport::util::fmt::len(len)
        )
    };
    Ok(
        json!({ "links": made, "nodes": nodes, "lengthM": len, "joined": joined, "message": message }),
    )
}

pub(super) fn network_split(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (id, at) = (p.str("link")?, p.latlng("point")?);
    match s.project.scenarios[i].links.get(id) {
        None => {
            return Err(ApiError::not_found(format!(
                "There is no road {id} in this scenario."
            )));
        }
        Some(l) if missing_end(&s.project.scenarios[i], l) => {
            return Err(ApiError::invalid_state(format!(
                "Road {id} has a missing end junction."
            )));
        }
        _ => {}
    }
    s.snap("splitting road");
    let before = s.project.scenarios[i].nodes.len();
    let node = net::split_link(&mut s.project.scenarios[i], &mut s.ids, id, at)?;
    let created = s.project.scenarios[i].nodes.len() > before;
    if created {
        s.edited();
    } else {
        s.discard_snap();
    }
    Ok(json!({ "node": node, "created": created }))
}

// ------------------------------------------------------------------ delete, reverse, control, move

pub(super) fn delete_kind(s: &mut Session, p: &Params, kind: &str) -> ApiResult {
    delete_object(s, p, kind, p.str("id")?)
}

pub(super) fn network_delete(s: &mut Session, p: &Params) -> ApiResult {
    delete_object(s, p, p.str("kind")?, p.str("id")?)
}

/// `deleteSelected()` of ui.js.
fn delete_object(s: &mut Session, p: &Params, kind: &str, id: &str) -> ApiResult {
    let i = s.target(p)?;
    let sc = &s.project.scenarios[i];
    let (name, gateway) = match kind {
        "link" => {
            let l = sc
                .links
                .get(id)
                .ok_or_else(|| ApiError::not_found(format!("There is no road {id}.")))?;
            (
                non_empty(&l.name).map_or_else(
                    || format!("{} road", road_class(l.cls.as_str()).label),
                    str::to_string,
                ),
                false,
            )
        }
        "node" => {
            if !sc.nodes.contains_key(id) {
                return Err(ApiError::not_found(format!("There is no junction {id}.")));
            }
            (kit_junction_name(sc, id), false)
        }
        "zone" => {
            let z = sc
                .zones
                .get(id)
                .ok_or_else(|| ApiError::not_found(format!("There is no zone {id}.")))?;
            (
                non_empty(&z.name).unwrap_or("zone").to_string(),
                z.kind.is("gateway"),
            )
        }
        _ => {
            let r = sc
                .routes
                .get(id)
                .ok_or_else(|| ApiError::not_found(format!("There is no bus route {id}.")))?;
            (non_empty(&r.name).unwrap_or("bus route").to_string(), false)
        }
    };
    let what = format!(
        "{} {}",
        if gateway { "gateway" } else { kind_name(kind) },
        name
    );
    s.snap(&format!("deleting {what}"));
    let sc = &mut s.project.scenarios[i];
    match kind {
        "link" => net::delete_link(sc, id),
        "node" => net::delete_node(sc, id),
        "zone" => {
            sc.zones.remove(id);
        }
        _ => {
            sc.routes.remove(id);
        }
    }
    s.edited();
    Ok(json!({ "deleted": id, "kind": kind, "message": format!("Deleted {what}.") }))
}

pub(super) fn network_reverse(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let id = p.str("link")?;
    if !s.project.scenarios[i].links.contains_key(id) {
        return Err(ApiError::not_found(format!(
            "There is no road {id} in this scenario."
        )));
    }
    s.snap("");
    net::reverse(&mut s.project.scenarios[i], id)?;
    s.edited();
    let l = &s.project.scenarios[i].links[id];
    Ok(json!({ "link": id, "lanesAB": val(&l.lanes_ab), "lanesBA": val(&l.lanes_ba) }))
}

pub(super) fn network_auto_control(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let ids: Vec<String> = match p.opt_strings("nodes")? {
        Some(v) => v,
        None => s.project.scenarios[i]
            .nodes
            .keys()
            .map(str::to_string)
            .collect(),
    };
    let control_of = |sc: &Scenario, n: &str| {
        sc.nodes
            .get(n)
            .and_then(|x| x.control.as_str().map(str::to_string))
    };
    let before: Vec<Option<String>> = ids
        .iter()
        .map(|n| control_of(&s.project.scenarios[i], n))
        .collect();
    s.snap("default junction control");
    net::auto_control(&mut s.project.scenarios[i], &ids);
    let sc = &s.project.scenarios[i];
    let changed = ids
        .iter()
        .zip(&before)
        .filter(|(n, b)| control_of(sc, n) != **b)
        .count();
    s.edited();
    Ok(json!({ "looked": ids.len(), "changed": changed }))
}

pub(super) fn network_move(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (kind, id, at) = (p.str("kind")?, p.str("id")?, p.latlng("point")?);
    let sc = &s.project.scenarios[i];
    let label = if kind == "node" {
        if !sc.nodes.contains_key(id) {
            return Err(ApiError::not_found(format!("There is no junction {id}.")));
        }
        format!("moving {}", kit_junction_name(sc, id))
    } else {
        let z = sc
            .zones
            .get(id)
            .ok_or_else(|| ApiError::not_found(format!("There is no zone {id}.")))?;
        format!("moving {}", non_empty(&z.name).unwrap_or("a zone"))
    };
    s.snap(&label);
    let sc = &mut s.project.scenarios[i];
    if kind == "node" {
        let n = sc.nodes.get_mut(id).expect("checked");
        n.lat = Num::number(r7(at[0]));
        n.lng = Num::number(r7(at[1]));
    } else {
        let z = sc.zones.get_mut(id).expect("checked");
        z.lat = Num::number(r7(at[0]));
        z.lng = Num::number(r7(at[1]));
    }
    s.edited();
    Ok(json!({ "kind": kind, "id": id, "point": [r7(at[0]), r7(at[1])] }))
}

// ------------------------------------------------------------------ zones, gateways and bus routes

fn zone_from(m: Map<String, Value>) -> ApiResult<Zone> {
    serde_json::from_value(Value::Object(m))
        .map_err(|e| ApiError::bad_params(format!("that does not fit a zone: {e}")))
}

/// The zone tool of ui.js: a zone with 200 houses at a position.
pub(super) fn zone_add(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let at = p.latlng("point")?;
    let id = s.ids.uid("z");
    let n = s.project.scenarios[i]
        .zones
        .values()
        .filter(|z| !z.kind.is("gateway"))
        .count()
        + 1;
    let name = match p.opt_str("name")? {
        Some(x) if !x.trim().is_empty() => x.trim().to_string(),
        _ => format!("Zone {n}"),
    };
    let mut m = Map::new();
    if let Some(extra) = p.opt_object("props")? {
        for (k, v) in extra {
            if !["id", "kind", "name", "lat", "lng", "lu"].contains(&k.as_str()) {
                m.insert(k.clone(), v.clone());
            }
        }
    }
    m.insert("id".into(), json!(id));
    m.insert("kind".into(), json!("zone"));
    m.insert("name".into(), json!(name));
    m.insert("lat".into(), json!(r7(at[0])));
    m.insert("lng".into(), json!(r7(at[1])));
    m.insert(
        "lu".into(),
        p.get("lu")
            .cloned()
            .unwrap_or_else(|| json!({ "residential": 200 })),
    );
    if !m.contains_key("connectors") {
        m.insert("connectors".into(), Value::Null);
    }
    let z = zone_from(m)?;
    s.snap(&format!("new zone {name}"));
    s.project.scenarios[i].zones.insert(id.clone(), z);
    s.edited();
    Ok(
        json!({ "id": id, "name": name, "message": "Zone added with 200 houses. Change its land use with transport.zone.set." }),
    )
}

/// The gateway tool of ui.js: a gateway on a road end with flows estimated from the busiest road's class and lanes.
pub(super) fn zone_add_gateway(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let max_m = p.opt_f64("maxM")?.unwrap_or(80.0);
    let at = p.opt_latlng("point")?;
    let sc = &s.project.scenarios[i];
    let nid = match p.opt_str("node")? {
        Some(n) => {
            if !sc.nodes.contains_key(n) {
                return Err(ApiError::not_found(format!("There is no junction {n}.")));
            }
            n.to_string()
        }
        None => {
            let at = at.ok_or_else(|| {
                ApiError::bad_params("Give a `node`, or a `point` near the end of a road.")
            })?;
            let deg = net::degree(sc);
            let f = move |n: &Node| deg.get(&n.id).copied().unwrap_or(0) > 0;
            net::nearest_node(sc, at, max_m, Some(&f)).ok_or_else(|| {
                ApiError::not_found(
                    "There is no road end near that point. Click on or near the end of a road.",
                )
            })?
        }
    };
    let ls = net::links_at(sc, &nid);
    if ls.is_empty() {
        return Err(ApiError::invalid_state(format!(
            "Junction {nid} has no roads, so a gateway cannot go there."
        )));
    }
    let mut l0 = ls[0];
    for l in &ls {
        if road_class(l.cls.as_str()).rank > road_class(l0.cls.as_str()).rank {
            l0 = l;
        }
    }
    let per = match road_class(l0.cls.as_str()).key {
        "motorway" => 1500.0,
        "ramp" => 650.0,
        "arterial" => 550.0,
        "collector" => 320.0,
        _ => 200.0,
    };
    let (in_l, out_l) = if l0.a == nid {
        (l0.lanes_ab.num(), l0.lanes_ba.num())
    } else {
        (l0.lanes_ba.num(), l0.lanes_ab.num())
    };
    let name = format!(
        "{} gateway",
        non_empty(&l0.name).map_or_else(
            || road_class(l0.cls.as_str()).label.to_string(),
            str::to_string
        )
    );
    let at = at.unwrap_or_else(|| sc.nodes[nid.as_str()].ll());
    let flow = |lanes: f64| math_round(lanes * per * 0.85 / 10.0) * 10.0;
    let (fin, fout) = (flow(in_l), flow(out_l));
    s.snap(&format!("new gateway {name}"));
    let id = s.ids.uid("z");
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m.insert("kind".into(), json!("gateway"));
    m.insert("name".into(), json!(name));
    m.insert("lat".into(), json!(r7(at[0])));
    m.insert("lng".into(), json!(r7(at[1])));
    m.insert("node".into(), json!(nid));
    m.insert(
        "gw".into(),
        json!({ "AMin": fin, "AMout": fout, "PMin": fin, "PMout": fout }),
    );
    let z = zone_from(m)?;
    s.project.scenarios[i].zones.insert(id.clone(), z);
    s.edited();
    Ok(json!({
        "id": id, "name": name, "node": nid, "gw": { "AMin": fin, "AMout": fout, "PMin": fin, "PMout": fout },
        "message": "Gateway added with flows estimated from the road class. Replace them with traffic counts if you have them."
    }))
}

/// `finishBus()` of ui.js.
pub(super) fn route_add(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let stops = p.opt_strings("stops")?.unwrap_or_default();
    if stops.len() < 2 {
        return Err(ApiError::bad_params(
            "A bus route needs at least two stops.",
        ));
    }
    for (k, n) in stops.iter().enumerate() {
        if !s.project.scenarios[i].nodes.contains_key(n) {
            return Err(ApiError::not_found(format!(
                "There is no junction {n} (stops[{k}])."
            )));
        }
        if k > 0 && stops[k - 1] == *n {
            return Err(ApiError::bad_params(format!(
                "`stops[{k}]` repeats the stop before it."
            )));
        }
    }
    let freq = p.opt_f64("freq")?.unwrap_or(6.0);
    let count = s.project.scenarios[i].routes.len();
    s.snap("new bus route");
    let id = s.ids.uid("r");
    let name = match p.opt_str("name")? {
        Some(x) if !x.trim().is_empty() => x.trim().to_string(),
        _ => format!("Bus route {}", count + 1),
    };
    let color = p.opt_str("color")?.map_or_else(
        || ROUTE_COLOURS[count % ROUTE_COLOURS.len()].to_string(),
        str::to_string,
    );
    let r = Route {
        id: id.clone(),
        name: Maybe::Set(name.clone()),
        color: Maybe::Set(color.clone()),
        freq: Num::number(freq),
        freqs: Maybe::Missing,
        wp: Maybe::Set(stops),
        extra: Map::new(),
    };
    s.project.scenarios[i].routes.insert(id.clone(), r);
    s.edited();
    Ok(
        json!({ "id": id, "name": name, "color": color, "freq": freq, "message": "Bus route added at 6 buses an hour. Set the frequency with transport.route.set." }),
    )
}
