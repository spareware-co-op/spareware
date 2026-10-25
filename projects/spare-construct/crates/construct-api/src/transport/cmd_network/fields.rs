//! Changing fields of a road, junction, zone, bus route or scenario: `applyField` of ui-inspector.js.

use super::*;
use construct_transport::project::land_use;

/// What a field change is applied to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FieldKind {
    /// A road.
    Link,
    /// A junction.
    Node,
    /// A zone or gateway.
    Zone,
    /// A bus route.
    Route,
    /// A scenario.
    Scenario,
}

/// The type the panel binds each known field with (`Kit.bind`): int, num, numnull, bool or str. Other fields are set as given.
fn field_type(kind: FieldKind, k: &str) -> &'static str {
    match (kind, k) {
        (FieldKind::Link, "lanesAB" | "lanesBA" | "speed") => "int",
        (FieldKind::Link, "capPerLane") => "numnull",
        (FieldKind::Link, "busLane") => "bool",
        (FieldKind::Link, "cls" | "status" | "cycle" | "name") => "str",
        (FieldKind::Node, "cycle") => "numnull",
        (FieldKind::Node, "fixedTiming" | "upgrade") => "bool",
        (FieldKind::Node, "control" | "name") => "str",
        (FieldKind::Zone, "connectors") => "numnull",
        (FieldKind::Zone, "role" | "name") => "str",
        (FieldKind::Zone, k) if k.starts_with("lu.") || k.starts_with("gw.") => "num",
        (FieldKind::Route, "freq") => "num",
        (FieldKind::Route, "name" | "color") => "str",
        (FieldKind::Scenario, "name" | "notes") => "str",
        _ => "json",
    }
}

/// The name the undo label gives a field (`FIELD_NAME` of ui-inspector.js).
fn field_name(kind: FieldKind, k: &str) -> String {
    if kind == FieldKind::Node && k == "cycle" {
        return "cycle time".to_string();
    }
    match k {
        "lanesAB" | "lanesBA" => "lanes".to_string(),
        "speed" => "speed limit".to_string(),
        "cls" => "road class".to_string(),
        "status" => "status".to_string(),
        "capPerLane" => "capacity".to_string(),
        "busLane" => "bus lane".to_string(),
        "cycle" => "cycling facility".to_string(),
        "control" => "junction control".to_string(),
        "fixedTiming" => "signal timing".to_string(),
        "upgrade" => "upgrade flag".to_string(),
        "connectors" => "zone access lines".to_string(),
        "freq" => "bus frequency".to_string(),
        "name" => "name".to_string(),
        "notes" => "notes".to_string(),
        "role" => "zone type".to_string(),
        k if k.starts_with("lu.") => "land use".to_string(),
        k if k.starts_with("gw.") => "gateway flows".to_string(),
        k => k.rsplit('.').next().unwrap_or(k).to_string(),
    }
}

/// `set` commands: the element is named by `id` (the scenario by the `scenario` parameter).
pub(super) fn set_fields(s: &mut Session, p: &Params, kind: FieldKind) -> ApiResult {
    s.target(p)?;
    let id = p.str("id")?.to_string();
    let values = p.object("values")?;
    apply_fields(s, kind, &id, values)
}

/// The panel's field change, for one or more fields of one thing, as one undo step. Fields are applied in alphabetical order of their
/// keys. Numbers may be text such as "12,000"; a known number field takes whole numbers or blank as the panel does.
pub(crate) fn apply_fields(
    s: &mut Session,
    kind: FieldKind,
    id: &str,
    values: &Map<String, Value>,
) -> ApiResult {
    if values.is_empty() {
        return Err(ApiError::bad_params("`values` has no fields in it"));
    }
    let i = s.active_index()?;
    let sc = &s.project.scenarios[i];
    let not_found = || {
        ApiError::not_found(format!(
            "There is no {} {id} in this scenario.",
            match kind {
                FieldKind::Link => "road",
                FieldKind::Node => "junction",
                FieldKind::Zone => "zone",
                FieldKind::Route => "bus route",
                FieldKind::Scenario => "scenario",
            }
        ))
    };
    let mut jv = match kind {
        FieldKind::Link => to_jv(sc.links.get(id).ok_or_else(not_found)?)?,
        FieldKind::Node => to_jv(sc.nodes.get(id).ok_or_else(not_found)?)?,
        FieldKind::Zone => to_jv(sc.zones.get(id).ok_or_else(not_found)?)?,
        FieldKind::Route => to_jv(sc.routes.get(id).ok_or_else(not_found)?)?,
        FieldKind::Scenario => scenario_light(&mut s.project.scenarios[i])?,
    };
    let sc = &s.project.scenarios[i];
    let who = match kind {
        FieldKind::Node => kit_junction_name(sc, id),
        FieldKind::Scenario => "the scenario".to_string(),
        _ => {
            let n = jv
                .get("name")
                .and_then(|n| {
                    if let JsonValue::String(t) = n {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
                .filter(|n| !n.is_empty());
            n.unwrap_or_else(|| {
                if kind == FieldKind::Link {
                    "an unnamed road".to_string()
                } else {
                    "it".to_string()
                }
            })
        }
    };
    let mut changed: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut model_change = false;
    for (k, raw) in values {
        let ty = field_type(kind, k);
        let newv: JsonValue = match ty {
            "num" | "int" | "numnull" => match parse_num(raw) {
                Some(x) if x.is_nan() => {
                    let what = match k.strip_prefix("lu.").and_then(land_use) {
                        Some(lu) => format!("{} ({})", lu.label.to_lowercase(), lu.unit),
                        None => field_name(kind, k),
                    };
                    return Err(ApiError::bad_params(format!(
                        "Enter a number for {what}, for example 12000 or 12,000. Nothing was changed."
                    )));
                }
                None => {
                    if ty == "numnull" {
                        JsonValue::Null
                    } else {
                        JsonValue::Number(0.0)
                    }
                }
                Some(x) => {
                    JsonValue::Number(math_max(0.0, if ty == "int" { math_round(x) } else { x }))
                }
            },
            "str" => match raw {
                Value::String(t) => {
                    JsonValue::String(construct_transport::jsnum::js_trim(t).to_string())
                }
                other => JsonValue::from_value(other),
            },
            "bool" => match raw {
                Value::Bool(b) => JsonValue::Bool(*b),
                _ => return Err(ApiError::bad_params(format!("`{k}` must be true or false"))),
            },
            _ => JsonValue::from_value(raw),
        };
        if kit_get_path(&jv, k).is_some_and(|c| c.strict_eq(&newv)) {
            continue;
        }
        if kind == FieldKind::Zone && k.starts_with("gw.") {
            jv.set("gwSrc", JsonValue::Null);
        }
        let prev_cls = match jv.get("cls") {
            Some(JsonValue::String(t)) => Some(t.clone()),
            _ => None,
        };
        if !kit_set_path(&mut jv, k, newv.clone()) {
            return Err(ApiError::bad_params(format!(
                "The field {k} cannot be set: a step of its path holds a plain value."
            )));
        }
        if kind == FieldKind::Link {
            if k == "cls"
                && let JsonValue::String(v) = &newv
                && let Some(c) = road_class_exact(v)
                && matches!(jv.get("speed"), Some(JsonValue::Number(x)) if *x == road_class(prev_cls.as_deref()).speed)
            {
                jv.set("speed", JsonValue::Number(c.speed));
            }
            let lane =
                |jv: &JsonValue, key: &str| jv.get(key).map_or(f64::NAN, JsonValue::to_number);
            let sum = i64::from(to_int32(lane(&jv, "lanesAB")))
                + i64::from(to_int32(lane(&jv, "lanesBA")));
            if sum == 0 {
                jv.set(k, JsonValue::Number(1.0));
                warnings.push("A road needs at least one lane.".to_string());
            }
            if math_max(lane(&jv, "lanesAB"), lane(&jv, "lanesBA")) < 2.0 {
                jv.set("busLane", JsonValue::Bool(false));
            }
        }
        if kind == FieldKind::Node
            && k == "control"
            && let JsonValue::String(v) = &newv
        {
            jv.set("upgrade", JsonValue::Bool(v != "none" && v != "priority"));
        }
        changed.push(k.clone());
        let f = field_name(kind, k);
        if !names.contains(&f) {
            names.push(f);
        }
        if !(k == "name" || k == "notes" || k == "color") {
            model_change = true;
        }
    }
    if changed.is_empty() {
        return Ok(
            json!({ "changed": [], "message": "Nothing changed: the fields already had those values." }),
        );
    }
    let label = format!("{} on {}", names.join(", "), who);
    match kind {
        FieldKind::Link => {
            let v: Link = from_jv(&jv)?;
            s.snap(&label);
            s.project.scenarios[i].links.insert(id, v);
        }
        FieldKind::Node => {
            let v: Node = from_jv(&jv)?;
            s.snap(&label);
            s.project.scenarios[i].nodes.insert(id, v);
        }
        FieldKind::Zone => {
            let v: Zone = from_jv(&jv)?;
            s.snap(&label);
            s.project.scenarios[i].zones.insert(id, v);
        }
        FieldKind::Route => {
            let v: Route = from_jv(&jv)?;
            s.snap(&label);
            s.project.scenarios[i].routes.insert(id, v);
        }
        FieldKind::Scenario => {
            let ns: Scenario = from_jv(&jv)?;
            s.snap(&label);
            scenario_commit(&mut s.project.scenarios[i], ns);
        }
    }
    if model_change {
        s.edited();
    } else {
        s.touch();
    }
    let element = match kind {
        FieldKind::Link => val(&s.project.scenarios[i].links[id]),
        FieldKind::Node => val(&s.project.scenarios[i].nodes[id]),
        FieldKind::Zone => val(&s.project.scenarios[i].zones[id]),
        FieldKind::Route => val(&s.project.scenarios[i].routes[id]),
        FieldKind::Scenario => s.scenario_brief(i),
    };
    Ok(json!({ "changed": changed, "label": label, "warnings": warnings, "element": element }))
}

/// A scenario's own fields as JSON, leaving out its network, zones, routes, matrices, results and sites.
fn scenario_light(sc: &mut Scenario) -> ApiResult<JsonValue> {
    let nodes = std::mem::take(&mut sc.nodes);
    let links = std::mem::take(&mut sc.links);
    let zones = std::mem::take(&mut sc.zones);
    let routes = std::mem::take(&mut sc.routes);
    let od = std::mem::take(&mut sc.od_manual);
    let res = std::mem::take(&mut sc.res);
    let sites = std::mem::take(&mut sc.sites);
    let out = to_jv(&*sc);
    sc.nodes = nodes;
    sc.links = links;
    sc.zones = zones;
    sc.routes = routes;
    sc.od_manual = od;
    sc.res = res;
    sc.sites = sites;
    out
}

/// Puts a scenario's changed own fields in place, keeping its network, zones, routes, matrices, results and sites.
fn scenario_commit(old: &mut Scenario, mut new: Scenario) {
    new.nodes = std::mem::take(&mut old.nodes);
    new.links = std::mem::take(&mut old.links);
    new.zones = std::mem::take(&mut old.zones);
    new.routes = std::mem::take(&mut old.routes);
    new.od_manual = std::mem::take(&mut old.od_manual);
    new.res = std::mem::take(&mut old.res);
    new.sites = std::mem::take(&mut old.sites);
    *old = new;
}
