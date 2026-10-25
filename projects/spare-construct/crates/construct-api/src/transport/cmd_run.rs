//! Running the model and reading its results: summary, roads, junctions, turns, bus routes, sites, the OD matrix and trip ends.

use super::Session;
use super::helpers::val;
use super::registry::{Registry, Sink};
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::assign::assign;
use construct_transport::demand::trip_ends;
use construct_transport::jsnum::json_number;
use construct_transport::results::{AssignOpts, Res};
use construct_transport::util::fmt;
use serde_json::{Map, Value, json};

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let per = || {
        choice(
            &["AM", "PM"],
            "Peak hour (default the project's current period).",
        )
    };
    r.add_streaming(
        "transport.run",
        "Run the model: the equilibrium assignment of one or more scenarios in AM, PM or both, with every assignment option. Progress is reported while it runs.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("scenarios", strings("Several scenarios by id or name (instead of scenario).")),
                ("all", boolean("Run every scenario.")),
                ("onlyStale", boolean("Run only the results that are out of date (every scenario).")),
                ("periods", array(json!({ "type": "string", "enum": ["AM", "PM"] }), "Periods to run (default AM and PM).")),
                (
                    "assign",
                    free_object("Assignment options: gapTol (relative gap target), maxIter, demandFactor (factor on the whole matrix), select ({ arc: key } | { turn: [fromKey, toKey] } | { zone: id }), arcPenalty (extra seconds by arc key or road id)."),
                ),
            ],
            &[],
        ),
        run_model,
    );
    r.add(
        "transport.results.list",
        "List every result the project holds: scenario, period, year, convergence, whether it is out of date and the headline numbers.",
        false,
        none(),
        results_list,
    );
    r.add(
        "transport.results.get",
        "The whole result of a run as the model keeps it (every road, junction, turn, site, class and total); the stored paths only when asked for.",
        false,
        params(
            vec![("scenario", sc()), ("period", per()), ("paths", boolean("Include the paths and arc keys (large)."))],
            &[],
        ),
        results_get,
    );
    r.add(
        "transport.results.summary",
        "The headline results of a run: trips, vehicle hours and kilometres, speed, roads over capacity, the worst junction, convergence (with the gap by iteration), growth and junction methods.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        results_summary,
    );
    r.add(
        "transport.results.arcs",
        "Road results by direction (flow, capacity, V/C, time, speed, junction delay, level of service), sorted and paged.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("sortBy", choice(&["v", "pcu", "vc", "t", "spd", "nd", "len"], "Sort by this field, largest first (default the engine's order).")),
                ("ascending", boolean("Sort smallest first.")),
                ("minVc", number("Only directions with V/C at or above this.")),
                ("links", strings("Only these road ids.")),
                ("limit", integer("Most rows to return (default 200).")),
                ("offset", integer("Rows to skip (default 0).")),
            ],
            &[],
        ),
        results_arcs,
    );
    r.add(
        "transport.results.nodes",
        "Junction results (delay, degree of saturation, level of service, method, volume), sorted and paged; with detail the approaches and movements too.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("sortBy", choice(&["delay", "avgDelay", "worstDelay", "dos", "vol"], "Sort by this field, largest first (default the engine's order).")),
                ("nodes", strings("Only these junction ids.")),
                ("detail", boolean("Include each junction's approaches and movements.")),
                ("limit", integer("Most rows to return (default 100).")),
                ("offset", integer("Rows to skip (default 0).")),
            ],
            &[],
        ),
        results_nodes,
    );
    r.add(
        "transport.results.turns",
        "Turning movements by junction from the assignment (arc in, arc out, movement, flow in veh/h and pcu/h).",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("node", string("Only this junction id.")),
                ("limit", integer("Most junctions to return (default 100).")),
                ("offset", integer("Junctions to skip (default 0).")),
            ],
            &[],
        ),
        results_turns,
    );
    r.add(
        "transport.results.routes",
        "Bus route results: length, running time, speed and frequency.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (i, period) = pick(s, p)?;
            let res = held(s, i, &period)?;
            Ok(json!({ "scenario": s.sid(i), "period": period, "busPcu": res.bus_pcu, "routes": val(&res.routes) }))
        },
    );
    r.add(
        "transport.results.sites",
        "Junction site results (legs, result by site) and the movements through each site, when the scenario has sites.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (i, period) = pick(s, p)?;
            let res = held(s, i, &period)?;
            Ok(json!({
                "scenario": s.sid(i), "period": period,
                "sites": val(&res.sites), "siteTurns": val(&res.site_turns), "siteTurnsBasis": res.site_turns_basis,
            }))
        },
    );
    r.add(
        "transport.results.od",
        "The origin-destination matrix of a run (veh/h), by zone, and the trips from and to each zone.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        results_od,
    );
    r.add(
        "transport.results.trip_ends",
        "Trips leaving and arriving each zone and gateway in the AM and PM peaks (the Trips tab), with totals.",
        false,
        params(vec![("scenario", sc())], &[]),
        results_trip_ends,
    );
}

/// The scenario and period a results command asks about.
pub(super) fn pick(s: &Session, p: &Params) -> ApiResult<(usize, String)> {
    Ok((s.pick_scenario(p, "scenario")?, s.period_of(p)?))
}

/// The result held for a scenario and period, or the reason there is none.
pub(super) fn held<'a>(s: &'a Session, i: usize, period: &str) -> ApiResult<&'a Res> {
    let sc = &s.project.scenarios[i];
    s.result(&sc.id, period).ok_or_else(|| {
        ApiError::invalid_state(format!(
            "{} has no {period} result. Run the model first (transport.run).",
            sc.name.as_str().unwrap_or(&sc.id)
        ))
    })
}

fn page(p: &Params, default_limit: usize) -> ApiResult<(usize, usize)> {
    let limit = p
        .opt_f64("limit")?
        .map_or(default_limit, |x| x.max(0.0) as usize);
    let offset = p.opt_f64("offset")?.map_or(0, |x| x.max(0.0) as usize);
    Ok((limit, offset))
}

fn headline(res: &Res) -> Value {
    json!({
        "trips": val(&res.kpi.trips), "spd": val(&res.kpi.spd), "over": val(&res.kpi.over), "vht": val(&res.kpi.vht), "unserved": val(&res.kpi.unserved),
    })
}

// ------------------------------------------------------------------ run

fn run_model(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let periods: Vec<String> = match p.opt_strings("periods")? {
        Some(v) if !v.is_empty() => v,
        _ => vec!["AM".to_string(), "PM".to_string()],
    };
    for x in &periods {
        if x != "AM" && x != "PM" {
            return Err(ApiError::bad_params("`periods` can only hold AM and PM"));
        }
    }
    let only_stale = p.flag("onlyStale")?;
    let mut pairs: Vec<(usize, String)> = Vec::new();
    if only_stale {
        for (i, sc) in s.project.scenarios.iter().enumerate() {
            for per in ["AM", "PM"] {
                if s.result(&sc.id, per).is_some_and(|r| r.stale) {
                    pairs.push((i, per.to_string()));
                }
            }
        }
        if pairs.is_empty() {
            return Ok(
                json!({ "runs": [], "skipped": [], "message": "Every result is up to date." }),
            );
        }
    } else {
        let idx: Vec<usize> = if p.flag("all")? {
            (0..s.project.scenarios.len()).collect()
        } else if let Some(list) = p.opt_strings("scenarios")? {
            list.iter()
                .map(|k| s.scenario_index(k))
                .collect::<ApiResult<_>>()?
        } else {
            vec![s.pick_scenario(p, "scenario")?]
        };
        for i in idx {
            for per in &periods {
                pairs.push((i, per.clone()));
            }
        }
    }
    let base_opts: AssignOpts = match p.opt_object("assign")? {
        Some(o) => serde_json::from_value(Value::Object(o.clone())).map_err(|e| {
            ApiError::bad_params(format!("`assign` does not fit the assignment options: {e}"))
        })?,
        None => AssignOpts::default(),
    };
    let total = pairs.len();
    let mut runs: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut not_converged: Vec<String> = Vec::new();
    let mut done_sc: Vec<usize> = Vec::new();
    for (n, (i, period)) in pairs.into_iter().enumerate() {
        let (sid, name) = {
            let sc = &s.project.scenarios[i];
            (
                sc.id.clone(),
                sc.name.as_str().unwrap_or(&sc.id).to_string(),
            )
        };
        let sc = &s.project.scenarios[i];
        if sc.links.is_empty() {
            skipped.push(json!({ "scenario": sid, "name": name, "period": period, "reason": format!("{name} has no roads yet. Import or draw some first.") }));
            continue;
        }
        if sc.zones.is_empty() {
            skipped.push(json!({ "scenario": sid, "name": name, "period": period, "reason": format!("{name} has no zones or gateways, so there is no traffic to load. Add a zone or a gateway.") }));
            continue;
        }
        let mut opts = base_opts.clone();
        opts.period = Some(period.clone());
        let res = {
            let project = &s.project;
            sink(
                &json!({ "event": "run_start", "scenario": sid, "name": name, "period": period, "run": n + 1, "runs": total }),
            );
            let mut cb = |it: u32, gap: f64, msg: &str| {
                sink(&json!({
                    "event": "progress", "scenario": sid, "name": name, "period": period, "run": n + 1, "runs": total,
                    "iteration": it, "gap": json_number(gap), "message": msg,
                }));
            };
            assign(
                project,
                &project.scenarios[i],
                &project.settings,
                &opts,
                Some(&mut cb),
            )
        };
        let summary = json!({
            "scenario": sid, "name": name, "period": period, "year": res.year, "iterations": res.it, "gap": val(&res.gap), "converged": res.converged,
            "hitMax": res.hit_max, "ms": res.ms, "note": res.note, "convNote": res.conv_note,
            "kpi": headline(&res),
        });
        if !res.converged {
            not_converged.push(format!("{name} {period}"));
        }
        sink(
            &json!({ "event": "run_done", "scenario": sid, "period": period, "run": n + 1, "runs": total }),
        );
        s.store_res(i, &period, res)?;
        if !done_sc.contains(&i) {
            done_sc.push(i);
        }
        runs.push(summary);
    }
    let ok = runs.len();
    let mut message = if ok == 0 {
        "Nothing was run.".to_string()
    } else if ok == 1 {
        let r = &runs[0];
        format!(
            "Model run ({}): {} peak {} veh/h, average speed {} km/h, {} road{} over capacity.",
            r["name"].as_str().unwrap_or(""),
            r["period"].as_str().unwrap_or(""),
            fmt::int(r["kpi"]["trips"].as_f64().unwrap_or(f64::NAN)),
            fmt::num(r["kpi"]["spd"].as_f64().unwrap_or(f64::NAN), 1),
            fmt::int(r["kpi"]["over"].as_f64().unwrap_or(f64::NAN)),
            if r["kpi"]["over"].as_f64() == Some(1.0) {
                ""
            } else {
                "s"
            }
        )
    } else {
        format!(
            "Ran {ok} model runs ({} scenario{}).",
            done_sc.len(),
            if done_sc.len() == 1 { "" } else { "s" }
        )
    };
    if !not_converged.is_empty() {
        message.push_str(&format!(
            " Not fully converged: {}.",
            not_converged.join(", ")
        ));
    }
    Ok(
        json!({ "runs": runs, "skipped": skipped, "notConverged": not_converged, "message": message }),
    )
}

// ------------------------------------------------------------------ results

fn results_list(s: &mut Session, _: &Params) -> ApiResult {
    let mut rows: Vec<Value> = Vec::new();
    for sc in &s.project.scenarios {
        if let Some(r) = s.results.get(&sc.id) {
            for (period, res) in r.iter() {
                rows.push(json!({
                    "scenario": sc.id, "name": sc.name.as_str(), "period": period, "year": res.year, "stale": res.stale,
                    "converged": res.converged, "iterations": res.it, "gap": val(&res.gap), "ms": res.ms, "kpi": headline(res),
                }));
            }
        }
    }
    Ok(json!({ "results": rows }))
}

fn results_get(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let mut v = val(res);
    if !p.flag("paths")?
        && let Value::Object(m) = &mut v
    {
        m.remove("paths");
        m.remove("arcKeys");
    }
    Ok(v)
}

fn results_summary(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    Ok(json!({
        "scenario": s.sid(i), "name": s.project.scenarios[i].name.as_str(), "period": res.period, "year": res.year, "stale": res.stale,
        "kpi": val(&res.kpi), "iterations": res.it, "gap": val(&res.gap), "gapTol": res.gap_tol, "maxIter": res.max_iter,
        "converged": res.converged, "hitMax": res.hit_max, "stableRule": res.stable_rule, "convNote": res.conv_note,
        "conv": val(&res.conv), "noise95": val(&res.noise95), "growth": val(&res.growth), "junction": val(&res.junction),
        "meta": val(&res.meta), "note": res.note, "manual": res.manual, "busPcu": res.bus_pcu, "ms": res.ms,
        "counts": { "arcs": res.arcs.len(), "junctions": res.nodes.len(), "zones": res.zone_ids.len(), "busRoutes": res.routes.len(), "sites": res.sites.as_ref().map_or(0, |x| x.len()) },
        "hasPaths": res.paths.get().is_some(),
    }))
}

fn results_arcs(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let (limit, offset) = page(p, 200)?;
    let min_vc = p.opt_f64("minVc")?;
    let only: Option<std::collections::HashSet<String>> =
        p.opt_strings("links")?.map(|v| v.into_iter().collect());
    let mut rows: Vec<(&str, &construct_transport::results::ArcRes)> = res
        .arcs
        .iter()
        .filter(|(k, a)| {
            min_vc.is_none_or(|m| a.vc >= m)
                && only
                    .as_ref()
                    .is_none_or(|o| o.contains(k.rsplit_once(':').map_or(*k, |x| x.0)))
        })
        .collect();
    if let Some(key) = p.opt_str("sortBy")? {
        let get = |a: &construct_transport::results::ArcRes| match key {
            "v" => a.v,
            "pcu" => a.pcu,
            "vc" => a.vc,
            "t" => a.t,
            "spd" => a.spd,
            "nd" => a.nd,
            _ => a.len,
        };
        rows.sort_by(|x, y| {
            get(y.1)
                .partial_cmp(&get(x.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if p.flag("ascending")? {
            rows.reverse();
        }
    }
    let sc = &s.project.scenarios[i];
    let total = rows.len();
    let items: Vec<Value> = rows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(k, a)| {
            let link = k.rsplit_once(':').map_or(k, |x| x.0);
            let mut v = val(a);
            v["key"] = json!(k);
            v["link"] = json!(link);
            v["name"] = json!(sc.links.get(link).and_then(|l| l.name.as_str()));
            v
        })
        .collect();
    Ok(
        json!({ "scenario": sc.id, "period": period, "total": total, "offset": offset, "count": items.len(), "arcs": items }),
    )
}

fn results_nodes(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let (limit, offset) = page(p, 100)?;
    let detail = p.flag("detail")?;
    let only: Option<std::collections::HashSet<String>> =
        p.opt_strings("nodes")?.map(|v| v.into_iter().collect());
    let mut rows: Vec<(&str, &construct_transport::results::NodeRes)> = res
        .nodes
        .iter()
        .filter(|(k, _)| only.as_ref().is_none_or(|o| o.contains(*k)))
        .collect();
    if let Some(key) = p.opt_str("sortBy")? {
        let get = |n: &construct_transport::results::NodeRes| match key {
            "delay" => n.delay,
            "avgDelay" => n.avg_delay,
            "worstDelay" => n.worst_delay,
            "dos" => n.dos,
            _ => n.vol,
        };
        rows.sort_by(|x, y| {
            get(y.1)
                .partial_cmp(&get(x.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    let sc = &s.project.scenarios[i];
    let total = rows.len();
    let items: Vec<Value> = rows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(k, n)| {
            let mut v = val(n);
            if !detail && let Value::Object(m) = &mut v {
                let approaches = n.appr.len();
                m.remove("appr");
                m.insert("approaches".to_string(), json!(approaches));
            }
            v["id"] = json!(k);
            v["name"] = json!(construct_transport::turns::junction_name(sc, k, None));
            v
        })
        .collect();
    Ok(
        json!({ "scenario": sc.id, "period": period, "total": total, "offset": offset, "count": items.len(), "nodes": items }),
    )
}

fn results_turns(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let (limit, offset) = page(p, 100)?;
    if let Some(n) = p.opt_str("node")? {
        let rows = res.turns.get(n).ok_or_else(|| {
            ApiError::not_found(format!("There are no turns at {n} in this result."))
        })?;
        return Ok(
            json!({ "scenario": s.sid(i), "period": period, "node": n, "turns": val(rows) }),
        );
    }
    let total = res.turns.len();
    let mut m = Map::new();
    for (k, rows) in res.turns.iter().skip(offset).take(limit) {
        m.insert(k.to_string(), val(rows));
    }
    Ok(
        json!({ "scenario": s.sid(i), "period": period, "total": total, "offset": offset, "count": m.len(), "turns": m }),
    )
}

fn results_od(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let sc = &s.project.scenarios[i];
    let z = res.zone_ids.len();
    let name = |id: &str| {
        sc.zones
            .get(id)
            .and_then(|x| x.name.as_str().filter(|n| !n.is_empty()))
            .unwrap_or(id)
            .to_string()
    };
    let zones: Vec<Value> = res.zone_ids.iter().enumerate().map(|(k, id)| json!({ "id": id, "name": name(id), "O": val(&res.o[k]), "D": val(&res.d[k]) })).collect();
    let matrix: Value = match &res.t {
        Some(t) => Value::Array(
            (0..z)
                .map(|a| Value::Array((0..z).map(|b| json_number(t[a * z + b])).collect()))
                .collect(),
        ),
        None => Value::Null,
    };
    Ok(json!({
        "scenario": sc.id, "period": period, "year": res.year, "manual": res.manual, "note": res.note,
        "zones": zones, "matrix": matrix,
        "matrixNote": if res.t.is_none() { "The matrix is not kept for more than 150 zones." } else { "" },
    }))
}

fn results_trip_ends(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let st = &s.project.settings;
    let mut rows: Vec<Value> = Vec::new();
    let mut tot = [0.0f64; 4];
    for z in sc.zones.values() {
        let am = trip_ends(&s.project, z, st, "AM", sc);
        let pm = trip_ends(&s.project, z, st, "PM", sc);
        tot[0] += am.o;
        tot[1] += am.d;
        tot[2] += pm.o;
        tot[3] += pm.d;
        rows.push(json!({
            "id": z.id, "name": z.name.as_str(), "type": if z.kind.is("gateway") { "gateway" } else { "zone" },
            "AMout": val(&am.o), "AMin": val(&am.d), "PMout": val(&pm.o), "PMin": val(&pm.d),
        }));
    }
    Ok(json!({
        "scenario": sc.id, "zones": rows,
        "totals": { "AMout": val(&tot[0]), "AMin": val(&tot[1]), "PMout": val(&tot[2]), "PMin": val(&tot[3]) },
    }))
}
