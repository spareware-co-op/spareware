//! Import and export commands: roads from an Overpass answer, GeoJSON in and out, links CSV, existing land use, OD matrices and the study-area estimate.
//!
//! No HTTP happens here: the commands that need downloaded data hand out the query (`overpass_query`, `landuse.queries`)
//! and take the answer as input, so the host (a browser, a script, an AI agent) does the fetching.

use super::Session;
use super::helpers::{link_props, road_template};
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::edit::IdSource;
use construct_transport::edit::sites::at_points;
use construct_transport::importers::osm::{
    Bounds, OVERPASS, OsmOpts, OverpassOpts, auto_gateways, osm_to_scenario, overpass_query,
};
use construct_transport::importers::{export_geojson, import_geojson, links_csv};
use construct_transport::jsnum::{Maybe, Num, json_number, string_to_number};
use construct_transport::landuse::{self, BuildOpts, StudyBox};
use construct_transport::project::{OdMatrix, OrderedMap, Zone};
use construct_transport::util::{self, csv, dist};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let bounds = || {
        any(
            "The map box: { south, west, north, east } in degrees, or two corners [[lat, lng], [lat, lng]].",
        )
    };
    let template = |v: &mut Vec<(&'static str, Value)>| {
        v.push((
            "cls",
            string("Road class for the roads added (default arterial)."),
        ));
        v.push(("lanes", integer("Lanes each way (default 1).")));
        v.push(("oneway", boolean("One way (default false).")));
        v.push(("speed", number("Speed limit in km/h (default 50).")));
        v.push((
            "status",
            choice(
                &["existing", "proposed", "closed"],
                "Status (default proposed).",
            ),
        ));
    };
    r.add(
        "transport.studyarea.estimate",
        "Size a study-area box: its area in km2, a warning when it is large (over 36 km2) and whether it is within the 120 km2 limit.",
        false,
        params(vec![("bounds", bounds())], &["bounds"]),
        studyarea_estimate,
    );
    r.add(
        "transport.import.overpass_query",
        "Give the Overpass query that downloads the roads inside a box, and the servers to ask. The host sends it and passes the answer to transport.import.osm_roads.",
        false,
        params(
            vec![
                ("bounds", bounds()),
                ("local", boolean("Include local and residential streets (default true).")),
                ("service", boolean("Include service lanes and alleys (default false).")),
            ],
            &["bounds"],
        ),
        import_overpass_query,
    );
    {
        let props = vec![
            ("scenario", sc()),
            (
                "answer",
                any("The Overpass JSON answer (an object, or its text)."),
            ),
            ("bounds", bounds()),
            (
                "gateways",
                boolean(
                    "Add gateways where main roads leave the box, with estimated through traffic (default true when bounds are given).",
                ),
            ),
            (
                "allScenarios",
                boolean(
                    "Add the new roads, junctions and zones to every other scenario too (default true).",
                ),
            ),
            (
                "detectSites",
                boolean("Group junction nodes into sites after the import (default false)."),
            ),
        ];
        r.add(
            "transport.import.osm_roads",
            "Add the roads of an Overpass answer to the scenario, with junction control, and optionally gateways at the box edge; one undo step.",
            true,
            params(props, &["answer"]),
            import_osm_roads,
        );
    }
    r.add(
        "transport.import.auto_gateways",
        "Add gateways where main roads leave the area (dead ends of collector class or above near the box's edge), with indicative through traffic from the road class.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("bounds", bounds()),
                ("nodes", strings("Only these dead-end junctions (default every one in the scenario).")),
            ],
            &["bounds"],
        ),
        import_auto_gateways,
    );
    r.add(
        "transport.data.set",
        "Load downloaded data into the project: the AT controlled-intersection points (used to group junction sites) and the regional model's growth (for the growth check).",
        true,
        params(
            vec![
                ("controlled", array(free_object("A point: { lat, lng, no | id }."), "Controlled-intersection points.")),
                ("msm", free_object("Regional growth: { links: { <road id>: { growth } } }, with validated true when it has been checked.")),
            ],
            &[],
        ),
        data_set,
    );
    {
        let mut props = vec![
            ("scenario", sc()),
            (
                "geojson",
                any("The GeoJSON (an object, or its text): line features become roads."),
            ),
        ];
        template(&mut props);
        props.push(("props", free_object("Other road fields copied onto each road, for example { \"busLane\": true, \"cycle\": \"painted\" }.")));
        r.add(
            "transport.import.geojson",
            "Add the line features of a GeoJSON file as roads, snapping their ends to a junction within 2 m or making one.",
            true,
            params(props, &["geojson"]),
            import_geojson_cmd,
        );
    }
    r.add(
        "transport.export.geojson",
        "The network (roads with their results, controlled junctions, zones) as GeoJSON text, with the period's results when the model has run.",
        false,
        params(vec![("scenario", sc()), ("period", choice(&["AM", "PM"], "Period of the results (default the current period)."))], &[]),
        export_geojson_cmd,
    );
    r.add(
        "transport.export.links_csv",
        "One CSV row per road direction (lanes, speed, length and, with results, volume, capacity, V/C, level of service, speed and delay).",
        false,
        params(vec![("scenario", sc()), ("period", choice(&["AM", "PM"], "Period of the results (default the current period)."))], &[]),
        export_links_csv,
    );
    r.add(
        "transport.landuse.constants",
        "The numbers and assumptions behind the existing land use build (floor-area limits, weights, which tags count).",
        false,
        none(),
        |_, _| Ok(landuse::constants()),
    );
    r.add(
        "transport.landuse.queries",
        "The Overpass queries that download the buildings, land-use areas, schools and shops for a study area, in tiles. The host sends each and passes the answers to transport.landuse.apply.",
        false,
        params(vec![("bounds", bounds())], &["bounds"]),
        landuse_queries,
    );
    r.add(
        "transport.landuse.apply",
        "Make existing land-use zones from Overpass answers as the study-area setup does: zones this tool made earlier inside the box are replaced, near-duplicates are skipped; one undo step.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("answers", array(any("One Overpass JSON answer."), "The answers to the queries from transport.landuse.queries (objects, or their text).")),
                ("bounds", bounds()),
                ("cell", number("Grid cell size in metres (at least 50, default 400).")),
                ("allScenarios", boolean("Give the same zones to every other scenario (default true).")),
            ],
            &["answers", "bounds"],
        ),
        landuse_apply,
    );
    r.add(
        "transport.demand.od_import",
        "Set a manual OD matrix for a period from CSV text (first column and first row are zone names that match zones of the scenario).",
        true,
        params(
            vec![("scenario", sc()), ("csv", string("The CSV text.")), ("period", choice(&["AM", "PM"], "Period (default the current period)."))],
            &["csv"],
        ),
        od_import,
    );
    r.add(
        "transport.demand.od_clear",
        "Go back to the gravity model for a period by removing its manual OD matrix.",
        true,
        params(
            vec![
                ("scenario", sc()),
                (
                    "period",
                    choice(&["AM", "PM"], "Period (default the current period)."),
                ),
            ],
            &[],
        ),
        od_clear,
    );
    r.add(
        "transport.export.od_csv",
        "The OD matrix of a run as CSV text (zones by name, trips in veh/h).",
        false,
        params(
            vec![
                ("scenario", sc()),
                (
                    "period",
                    choice(&["AM", "PM"], "Period (default the current period)."),
                ),
            ],
            &[],
        ),
        export_od_csv,
    );
}

// ------------------------------------------------------------------ inputs

/// A JSON input given as an object or as its text.
fn json_input(p: &Params, key: &str) -> ApiResult<Value> {
    match p.get(key) {
        None => Err(ApiError::bad_params(format!("missing parameter `{key}`"))),
        Some(Value::String(t)) => serde_json::from_str(t).map_err(|e| {
            ApiError::bad_params(format!("`{key}` is text that is not valid JSON: {e}"))
        }),
        Some(v) => Ok(v.clone()),
    }
}

fn parse_box(v: &Value) -> ApiResult<(f64, f64, f64, f64)> {
    let bad = || {
        ApiError::bad_params(
            "`bounds` must be { south, west, north, east } or two corners [[lat, lng], [lat, lng]]",
        )
    };
    let num = |x: Option<&Value>| {
        x.and_then(Value::as_f64)
            .filter(|f| f.is_finite())
            .ok_or_else(bad)
    };
    match v {
        Value::Object(o) => Ok((
            num(o.get("south"))?,
            num(o.get("west"))?,
            num(o.get("north"))?,
            num(o.get("east"))?,
        )),
        Value::Array(a) if a.len() == 2 => {
            let c = |x: &Value| -> ApiResult<[f64; 2]> { latlng_of(x, "bounds") };
            let (p, q) = (c(&a[0])?, c(&a[1])?);
            Ok((
                p[0].min(q[0]),
                p[1].min(q[1]),
                p[0].max(q[0]),
                p[1].max(q[1]),
            ))
        }
        Value::Array(a) if a.len() == 4 => Ok((
            num(a.first())?,
            num(a.get(1))?,
            num(a.get(2))?,
            num(a.get(3))?,
        )),
        _ => Err(bad()),
    }
}

fn bounds_param(p: &Params, key: &str) -> ApiResult<(f64, f64, f64, f64)> {
    parse_box(
        p.get(key)
            .ok_or_else(|| ApiError::bad_params(format!("missing parameter `{key}`")))?,
    )
}

// ------------------------------------------------------------------ study area and queries

/// `StudyArea.estimate(bounds)`: area, warning and whether the area is within the limit.
fn studyarea_estimate(_: &mut Session, p: &Params) -> ApiResult {
    let (s, w, n, e) = bounds_param(p, "bounds")?;
    let c_lat = (s + n) / 2.0;
    let wm = dist(&[c_lat, w], &[c_lat, e]);
    let c_lng = (w + e) / 2.0;
    let hm = dist(&[s, c_lng], &[n, c_lng]);
    let a = wm * hm / 1e6;
    let km2 = if a < 1.0 {
        (a * 100.0).round() / 100.0
    } else {
        (a * 10.0).round() / 10.0
    };
    let mins = (a / 12.0).round().max(1.0);
    let (warn_km2, max_km2) = (36.0, 120.0);
    let warn = if a.is_nan() || a <= 0.0 {
        Some(
            "The study area has no size. Draw a box around the area you want to model.".to_string(),
        )
    } else if a > max_km2 {
        Some(format!(
            "That area is {} km\u{b2}, too large to set up in one go (the limit is {} km\u{b2}). Zoom in or draw a smaller study area.",
            util::fmt::num(a, 0),
            max_km2
        ))
    } else if a > warn_km2 {
        Some(format!(
            "That is a large area ({} km\u{b2}). Setting it up may take about {} minute{}, and the free map servers may refuse it. Areas under {} km\u{b2} work best.",
            util::fmt::num(a, 0),
            mins,
            if mins == 1.0 { "" } else { "s" },
            warn_km2
        ))
    } else {
        None
    };
    Ok(
        json!({ "km2": km2, "widthM": wm, "heightM": hm, "warn": warn, "ok": a > 0.0 && a <= max_km2, "minutes": mins }),
    )
}

fn import_overpass_query(_: &mut Session, p: &Params) -> ApiResult {
    let (south, west, north, east) = bounds_param(p, "bounds")?;
    let opts = OverpassOpts {
        service: p.flag("service")?,
        local: p.opt_bool("local")?,
    };
    let query = overpass_query(
        &Bounds {
            south,
            west,
            north,
            east,
        },
        &opts,
    );
    let c_lat = (south + north) / 2.0;
    let area_km2 = dist(&[c_lat, west], &[c_lat, east])
        * dist(&[south, (west + east) / 2.0], &[north, (west + east) / 2.0])
        / 1e6;
    Ok(json!({ "query": query, "servers": OVERPASS, "areaKm2": area_km2 }))
}

fn landuse_queries(_: &mut Session, p: &Params) -> ApiResult {
    let (s, w, n, e) = bounds_param(p, "bounds")?;
    let jobs = landuse::download_jobs(&StudyBox { s, w, n, e }).map_err(ApiError::bad_params)?;
    Ok(json!({
        "servers": OVERPASS,
        "jobs": jobs.iter().map(|j| json!({ "label": j.label, "query": j.query })).collect::<Vec<_>>(),
    }))
}

// ------------------------------------------------------------------ roads in

fn keys_of<V>(m: &OrderedMap<V>) -> HashSet<String> {
    m.keys().map(str::to_string).collect()
}

/// Copies what an import added (the nodes, links and zones that were not there before) into every other scenario that lacks them.
fn copy_new_to_others(
    s: &mut Session,
    i: usize,
    before: &(HashSet<String>, HashSet<String>, HashSet<String>),
) -> usize {
    let sc = &s.project.scenarios[i];
    let nodes: Vec<_> = sc
        .nodes
        .iter()
        .filter(|(k, _)| !before.0.contains(*k))
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let links: Vec<_> = sc
        .links
        .iter()
        .filter(|(k, _)| !before.1.contains(*k))
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let zones: Vec<_> = sc
        .zones
        .iter()
        .filter(|(k, _)| !before.2.contains(*k))
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let mut n = 0;
    for j in 0..s.project.scenarios.len() {
        if j == i {
            continue;
        }
        let other = &mut s.project.scenarios[j];
        for (k, v) in &nodes {
            if !other.nodes.contains_key(k) {
                other.nodes.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &links {
            if !other.links.contains_key(k) {
                other.links.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &zones {
            if !other.zones.contains_key(k) {
                other.zones.insert(k.clone(), v.clone());
            }
        }
        s.mark_stale_at(j);
        n += 1;
    }
    n
}

/// `runImport` of ui-dialogs.js after the download: the roads, the gateways at this import's new road ends, and the other scenarios.
fn import_osm_roads(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let data = json_input(p, "answer")?;
    let bounds = match p.get("bounds") {
        Some(b) => Some(parse_box(b)?),
        None => None,
    };
    let want_gw = p.opt_bool("gateways")?.unwrap_or(bounds.is_some());
    if want_gw && bounds.is_none() {
        return Err(ApiError::bad_params(
            "Gateways need the `bounds` of the area that was downloaded.",
        ));
    }
    let all = p.opt_bool("allScenarios")?.unwrap_or(true);
    let opts = OsmOpts {
        sites: p.flag("detectSites")?,
        points: at_points(&s.project),
    };
    s.snap("importing roads");
    let sc = &s.project.scenarios[i];
    let before = (keys_of(&sc.nodes), keys_of(&sc.links), keys_of(&sc.zones));
    let n = osm_to_scenario(&mut s.project.scenarios[i], &mut s.ids, &data, &opts);
    let sc = &s.project.scenarios[i];
    let fresh: HashSet<String> = sc
        .nodes
        .keys()
        .filter(|k| !before.0.contains(*k))
        .map(str::to_string)
        .collect();
    let g = match (want_gw, bounds) {
        (true, Some((south, west, north, east))) => auto_gateways(
            &mut s.project.scenarios[i],
            &mut s.ids,
            &Bounds {
                south,
                west,
                north,
                east,
            },
            Some(&fresh),
        ),
        _ => 0,
    };
    let others = if all {
        copy_new_to_others(s, i, &before)
    } else {
        0
    };
    s.edited();
    let message = if n > 0 {
        format!(
            "Imported {} road segments{}.",
            util::fmt::int(n as f64),
            if g > 0 {
                format!(" and {g} gateways")
            } else {
                String::new()
            }
        )
    } else {
        "No new roads found in this view.".to_string()
    };
    Ok(json!({ "segments": n, "gateways": g, "otherScenarios": others, "message": message }))
}

fn import_auto_gateways(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (south, west, north, east) = bounds_param(p, "bounds")?;
    let only: Option<HashSet<String>> = p.opt_strings("nodes")?.map(|v| v.into_iter().collect());
    s.snap("adding gateways");
    let n = auto_gateways(
        &mut s.project.scenarios[i],
        &mut s.ids,
        &Bounds {
            south,
            west,
            north,
            east,
        },
        only.as_ref(),
    );
    s.edited();
    Ok(json!({ "gateways": n }))
}

fn data_set(s: &mut Session, p: &Params) -> ApiResult {
    use construct_transport::project::ProjectData;
    if !p.has("controlled") && !p.has("msm") {
        return Err(ApiError::bad_params("Give `controlled` or `msm`."));
    }
    if !s.project.data.is_set() {
        s.project.data = Maybe::Set(ProjectData {
            loaded: Maybe::Missing,
            msm: Maybe::Missing,
            controlled: Maybe::Missing,
            extra: Map::new(),
        });
    }
    let d = s.project.data.get_mut().expect("just made");
    if let Some(c) = p.get("controlled") {
        d.controlled = Maybe::Set(c.clone());
    }
    if let Some(m) = p.get("msm") {
        d.msm = Maybe::Set(m.clone());
    }
    let out = json!({ "controlled": d.controlled.get().and_then(Value::as_array).map(Vec::len), "msm": d.msm.is_set() });
    s.touch();
    Ok(out)
}

fn import_geojson_cmd(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let gj = json_input(p, "geojson")?;
    let mut template = road_template(p)?;
    if let Some(extra) = p.opt_object("props")? {
        // the template's own fields, then the extra ones over them
        let mut m = match serde_json::to_value(&template.0) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        };
        for (k, v) in extra {
            m.insert(k.clone(), v.clone());
        }
        template = link_props(Some(&m))?;
    }
    s.snap("");
    let n = import_geojson(&mut s.project.scenarios[i], &mut s.ids, &gj, &template);
    s.edited();
    Ok(
        json!({ "roads": n, "message": format!("Imported {n} road{}.", if n == 1 { "" } else { "s" }) }),
    )
}

// ------------------------------------------------------------------ exports

fn export_geojson_cmd(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let period = s.period_of(p)?;
    let sid = s.sid(i);
    let text = export_geojson(&s.project.scenarios[i], s.result(&sid, &period));
    Ok(json!({ "period": period, "withResults": s.result(&sid, &period).is_some(), "text": text }))
}

fn export_links_csv(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let period = s.period_of(p)?;
    let sid = s.sid(i);
    let text = links_csv(&s.project.scenarios[i], s.result(&sid, &period));
    Ok(json!({ "period": period, "withResults": s.result(&sid, &period).is_some(), "text": text }))
}

// ------------------------------------------------------------------ existing land use

/// `fromLU(z)` of studyarea.js: a zone an earlier setup made from existing land use.
fn from_lu(z: &Zone) -> bool {
    z.kind.is("zone")
        && z.existing.is_true()
        && (z.src.get().and_then(Value::as_str) == Some("existing-lu")
            || z.name
                .as_str()
                .is_some_and(|n| n.starts_with("Existing \u{b7} ")))
}

fn landuse_apply(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (south, west, north, east) = bounds_param(p, "bounds")?;
    let b = StudyBox {
        s: south,
        w: west,
        n: north,
        e: east,
    };
    let answers = p.array("answers")?;
    let mut batches: Vec<Vec<Value>> = Vec::new();
    for (k, a) in answers.iter().enumerate() {
        let v: Value = match a {
            Value::String(t) => serde_json::from_str(t).map_err(|e| {
                ApiError::bad_params(format!(
                    "`answers[{k}]` is text that is not valid JSON: {e}"
                ))
            })?,
            o => o.clone(),
        };
        batches.push(
            landuse::overpass_elements(&v, &format!("answer {}", k + 1))
                .map_err(ApiError::bad_params)?,
        );
    }
    let elements = landuse::merge_elements(&batches);
    let all = p.opt_bool("allScenarios")?.unwrap_or(true);
    let opts = BuildOpts {
        cell: p.opt_f64("cell")?,
        links: landuse::road_lines(&s.project.scenarios[i]),
    };
    let built = landuse::build(&Value::Array(elements), Some(&b), &mut s.ids, &opts);
    s.snap("existing land use");
    let inside = |z: &Zone| {
        z.lat.num().is_finite()
            && z.lng.num().is_finite()
            && z.lat.num() >= b.s
            && z.lat.num() <= b.n
            && z.lng.num() >= b.w
            && z.lng.num() <= b.e
    };
    let removed: Vec<String> = s.project.scenarios[i]
        .zones
        .values()
        .filter(|z| from_lu(z) && inside(z))
        .map(|z| z.id.clone())
        .collect();
    for id in &removed {
        s.project.scenarios[i].zones.remove(id);
    }
    let (mut added, mut dwellings) = (0usize, 0.0f64);
    let mut added_ids: Vec<String> = Vec::new();
    for z in built.zones {
        if !z.lat.num().is_finite() || !z.lng.num().is_finite() {
            continue;
        }
        let nm = z.name.as_str().unwrap_or("").trim().to_lowercase();
        let dup = s.project.scenarios[i].zones.values().any(|y| {
            !y.kind.is("gateway")
                && y.name.as_str().unwrap_or("").trim().to_lowercase() == nm
                && dist(&y.ll(), &z.ll()) < 150.0
        });
        if dup {
            continue;
        }
        let id = if !z.id.is_empty() && !s.project.scenarios[i].zones.contains_key(&z.id) {
            z.id.clone()
        } else {
            s.ids.uid("z")
        };
        let mut z = z;
        if z.connectors.is_missing() {
            z.connectors = Maybe::Null;
        }
        if z.lu.is_missing() {
            z.lu = Maybe::Set(OrderedMap::new());
        }
        z.id = id.clone();
        z.kind = Maybe::Set("zone".to_string());
        z.existing = true.into();
        z.src = Maybe::Set(json!("existing-lu"));
        if let Some(lu) = z.lu.get() {
            dwellings += lu.num_at("residential").or(0.0) + lu.num_at("apartment").or(0.0);
        }
        s.project.scenarios[i].zones.insert(id.clone(), z);
        added_ids.push(id);
        added += 1;
    }
    let dwellings = construct_transport::jsnum::math_round(dwellings);
    let mut warnings: Vec<String> = Vec::new();
    let mut others = 0usize;
    if all {
        let new_zones: Vec<(String, Zone)> = added_ids
            .iter()
            .filter_map(|id| {
                s.project.scenarios[i]
                    .zones
                    .get(id)
                    .map(|z| (id.clone(), z.clone()))
            })
            .collect();
        for j in 0..s.project.scenarios.len() {
            if j == i {
                continue;
            }
            let other = &mut s.project.scenarios[j];
            for id in &removed {
                if other.zones.get(id).is_some_and(from_lu) {
                    other.zones.remove(id);
                }
            }
            for (id, z) in &new_zones {
                if !other.zones.contains_key(id) {
                    other.zones.insert(id.clone(), z.clone());
                }
            }
            s.mark_stale_at(j);
            others += 1;
        }
    }
    if !removed.is_empty()
        && let Some(me) = s.project.me.get()
    {
        let hit = ["AM", "PM"].iter().any(|per| {
            me.factors.get(per).and_then(Maybe::get).is_some_and(|f| {
                f.keys()
                    .any(|k| k.split('>').any(|x| removed.iter().any(|r| r == x)))
            })
        });
        if hit {
            warnings.push("Existing land use was replaced, so matrix estimation factors for the old zones no longer apply. Fit to counts again.".to_string());
        }
    }
    s.edited();
    Ok(json!({
        "zones": added, "replaced": removed.len(), "dwellings": dwellings, "otherScenarios": others,
        "warnings": warnings, "stats": built.stats.to_value(),
    }))
}

// ------------------------------------------------------------------ OD matrices

/// `U.parseCSV(text)`: rows of cells, quotes and doubled quotes read, blank rows dropped.
pub(super) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let (mut row, mut cur, mut q) = (Vec::<String>::new(), String::new(), false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if q {
            if c == '"' && chars.get(i + 1) == Some(&'"') {
                cur.push('"');
                i += 1;
            } else if c == '"' {
                q = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            q = true;
        } else if c == ',' {
            row.push(std::mem::take(&mut cur));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            row.push(std::mem::take(&mut cur));
            rows.push(std::mem::take(&mut row));
        } else {
            cur.push(c);
        }
        i += 1;
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows.into_iter()
        .filter(|r| {
            r.iter()
                .any(|v| !construct_transport::jsnum::js_trim(v).is_empty())
        })
        .collect()
}

/// The OD import of ui-dialogs.js: zones matched by name, the matrix over the zones that appear.
fn od_import(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let period = s.period_of(p)?;
    let rows = parse_csv(p.str("csv")?);
    if rows.is_empty() {
        return Err(ApiError::bad_params("The CSV has no rows."));
    }
    let key = |t: &str| construct_transport::jsnum::js_trim(t).to_lowercase();
    let mut by_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for z in s.project.scenarios[i].zones.values() {
        by_name.insert(key(z.name.as_str().unwrap_or("")), z.id.clone());
    }
    let cols: Vec<Option<String>> = rows[0]
        .iter()
        .skip(1)
        .map(|n| by_name.get(&key(n)).cloned())
        .collect();
    let mut ids: Vec<String> = Vec::new();
    for r in rows.iter().skip(1) {
        if let Some(id) = by_name.get(&key(r.first().map_or("", String::as_str))) {
            ids.push(id.clone());
        }
    }
    let mut zones_all: Vec<String> = Vec::new();
    for id in ids.iter().chain(cols.iter().flatten()) {
        if !zones_all.contains(id) {
            zones_all.push(id.clone());
        }
    }
    if zones_all.is_empty() {
        return Err(ApiError::bad_params(
            "No zone names in the CSV match zones in this scenario.",
        ));
    }
    let n = zones_all.len();
    let pos = |id: &str| zones_all.iter().position(|z| z == id).expect("listed");
    let mut m = vec![vec![0.0f64; n]; n];
    for r in rows.iter().skip(1) {
        let Some(o) = by_name.get(&key(r.first().map_or("", String::as_str))) else {
            continue;
        };
        for (j, v) in r.iter().skip(1).enumerate() {
            if let Some(Some(d)) = cols.get(j) {
                let x = string_to_number(v);
                m[pos(o)][pos(d)] = if x.is_nan() || x == 0.0 { 0.0 } else { x };
            }
        }
    }
    s.snap("");
    let od = OdMatrix {
        zones: Maybe::Set(zones_all.clone()),
        m: Maybe::Set(
            m.into_iter()
                .map(|row| Maybe::Set(row.into_iter().map(Num::number).collect()))
                .collect(),
        ),
        extra: Map::new(),
    };
    let sc = &mut s.project.scenarios[i];
    if !sc.od_manual.is_set() {
        sc.od_manual = Maybe::Set(OrderedMap::new());
    }
    sc.od_manual
        .get_mut()
        .expect("set")
        .insert(period.clone(), Maybe::Set(od));
    s.edited();
    Ok(
        json!({ "period": period, "zones": n, "message": format!("Manual {period} OD matrix loaded for {n} zones. Run the model to use it.") }),
    )
}

fn od_clear(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let period = s.period_of(p)?;
    if !s.project.scenarios[i]
        .od_manual
        .get()
        .is_some_and(|m| m.contains_key(&period))
    {
        return Err(ApiError::invalid_state(format!(
            "This scenario has no manual {period} OD matrix."
        )));
    }
    s.snap("");
    if let Some(m) = s.project.scenarios[i].od_manual.get_mut() {
        m.remove(&period);
    }
    s.edited();
    Ok(json!({ "period": period, "cleared": true }))
}

fn export_od_csv(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let period = s.period_of(p)?;
    let sid = s.sid(i);
    let res = s
        .result(&sid, &period)
        .ok_or_else(|| ApiError::invalid_state("Run the model first."))?;
    let t = res.t.as_ref().ok_or_else(|| {
        ApiError::invalid_state("This result has no OD matrix (more than 150 zones).")
    })?;
    let sc = &s.project.scenarios[i];
    let nm = |id: &str| {
        sc.zones
            .get(id)
            .and_then(|z| z.name.as_str().filter(|n| !n.is_empty()))
            .unwrap_or(id)
            .to_string()
    };
    let z = res.zone_ids.len();
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut head: Vec<Value> = vec![json!("from \\ to")];
    head.extend(res.zone_ids.iter().map(|id| json!(nm(id))));
    rows.push(head);
    for (a, id) in res.zone_ids.iter().enumerate() {
        let mut row: Vec<Value> = vec![json!(nm(id))];
        for b in 0..z {
            row.push(json_number(t[a * z + b]));
        }
        rows.push(row);
    }
    Ok(json!({ "period": period, "zones": z, "text": csv(&rows) }))
}
