//! Calibration commands: counts and their roles, the model against the counts, matrix estimation, the validation report and status,
//! screenlines, turning counts, gateway flows from counts and the stress test.
//!
//! Counts come in as data (records the host downloaded, or a CSV of your own): nothing here fetches them. Commands that change the
//! project's counts or matrix estimation are not undo steps, as in the app; results that used a changed matrix go out of date.

use super::Session;
use super::cmd_run::held;
use super::helpers::val;
use super::registry::{Registry, Sink};
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::calibrate::{
    self, Ctx, EvOpts, FitOpts, Recorded, RunMeOpts, Screenline, count_rows, in_area, in_set,
};
use construct_transport::datahub::{self, Data};
use construct_transport::edit::IdSource;
use construct_transport::jsnum::{Maybe, json_number};
use construct_transport::project::Point;
use serde_json::{Map, Value, json};

/// What calibration keeps between commands (nothing yet: the counts, roles and fit records live in the project).
#[derive(Default)]
pub(crate) struct CalibState {}

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let per = || {
        choice(
            &["AM", "PM"],
            "Peak hour (default the project's current period).",
        )
    };
    r.add(
        "transport.calibration.counts",
        "List the traffic counts in the project with their source, volumes, role and where they match the network.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        counts_list,
    );
    r.add(
        "transport.calibration.counts_add",
        "Add count records the host has downloaded (NZTA, Auckland Transport) or made, replacing counts with the same id; they are matched to the nearest road.",
        true,
        params(
            vec![
                ("counts", array(free_object("A count: { id, src, name, lat, lng, adt, am, pm, dir, ... }."), "The count records.")),
                ("source", string("Source key (nzta, at, user, ramm) to record as loaded now.")),
            ],
            &["counts"],
        ),
        counts_add,
    );
    r.add(
        "transport.calibration.counts_import_csv",
        "Import your own counts from CSV text with columns lat, lng, am, pm, adt (any of them), name and dir.",
        true,
        params(vec![("csv", string("The CSV text."))], &["csv"]),
        counts_import_csv,
    );
    r.add(
        "transport.calibration.set_role",
        "Give a count a role: valid (held out to validate), off (not used), or me (calibration; clears the role).",
        true,
        params(vec![("count", string("Count id.")), ("role", choice(&["me", "valid", "off"], "The role."))], &["count", "role"]),
        set_role,
    );
    r.add(
        "transport.calibration.hold_out",
        "Hold out about a quarter of the matched counts for validation and make the rest calibration counts (deterministic).",
        true,
        params(vec![("scenario", sc()), ("every", integer("Hold out every this many counts (default 4)."))], &[]),
        hold_out,
    );
    r.add(
        "transport.calibration.reset_roles",
        "Clear every count's role except 'not used'.",
        true,
        none(),
        |s, _| {
            calibrate::reset_roles(&mut s.project);
            s.touch();
            Ok(json!({ "reset": true }))
        },
    );
    r.add(
        "transport.calibration.summary",
        "The model against the counts in use: how many are matched, the share with GEH under 5, R-squared, slope and totals.",
        false,
        params(vec![("scenario", sc()), ("period", per()), ("limit", integer("Most rows to list, worst fit first (default 20)."))], &[]),
        calibration_summary,
    );
    r.add(
        "transport.calibration.rows",
        "Every counted site against the model with GEH, role, method and match, worst first.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("set", choice(&["all", "calib", "valid"], "Which counts: all, calib (calibration) or valid (validation).")),
                ("area", choice(&["all", "inside", "outside"], "Inside or outside the core area.")),
                ("raw", boolean("Roles as given, without the rules that make a seen validation count a calibration count.")),
                ("limit", integer("Most rows (default 200).")),
            ],
            &[],
        ),
        calibration_rows,
    );
    r.add(
        "transport.calibration.report",
        "The validation summary against the chosen standard: statistics, each test with its verdict, the status and the words that go with it.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        calibration_report,
    );
    r.add(
        "transport.calibration.assess",
        "Assess the model's counts against the validation criteria (TMDG category or TAG): each test with its got, target and verdict.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("set", choice(&["all", "calib", "valid"], "Counts to assess (default the calibration settings').")),
                ("area", choice(&["all", "inside", "outside", "auto"], "Area of the counts.")),
                ("strict", boolean("Use only independent validation counts.")),
                ("standard", string("Standard: tmdg or a preset such as tag.")),
                ("category", string("TMDG category A to G.")),
            ],
            &[],
        ),
        calibration_assess,
    );
    r.add(
        "transport.calibration.me_report",
        "Table M1: the prior matrix against the estimated one on the base-year view (totals, trip ends, trip lengths, biggest changes).",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        me_report,
    );
    r.add_streaming(
        "transport.calibration.run_me",
        "Fit the matrix to the calibration counts only, as the panel's Estimate button does: weighted by observation method, screenlines as group totals. Progress is reported.",
        true,
        params(vec![("scenario", sc()), ("period", per()), ("xamax", number("Largest site ratio and cumulative factor (default the panel setting)."))], &[]),
        run_me,
    );
    r.add_streaming(
        "transport.calibration.fit",
        "Matrix estimation against the counts, with every option (passes, damping, factor limits, which counts, weights). Progress is reported.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("iters", integer("Passes (default 6).")),
                ("damp", number("Damping of each step, 0.01 to 1 (default 0.7).")),
                ("maxFactor", number("Largest cumulative factor (default 3).")),
                ("maxStep", number("Largest site ratio per step (default 2).")),
                ("minVol", number("Sites with a modelled or counted flow below this are left out of a step (default 10).")),
                ("constrain", boolean("A pass that breaks the matrix-change limits ends the fit (default true).")),
                ("freezeGg", boolean("Through traffic (gateway to gateway) keeps the prior.")),
                ("weights", boolean("Weigh counts by observation method.")),
                ("countIds", strings("Only these counts take part.")),
            ],
            &[],
        ),
        calibration_fit,
    );
    r.add_streaming(
        "transport.calibration.trip_end_factors",
        "Fit factors on the trip rates of each land use (and the estimated gateways) to the counts, as an alternative to matrix estimation; the result is kept on the scenario until applied. Progress is reported.",
        true,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        trip_end_factors,
    );
    r.add(
        "transport.calibration.trip_end_factors_can_apply",
        "Whether the fitted trip-end factors can be applied to the trip rates, and why not.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (i, _, period) = sci(s, p)?;
            let c = calibrate::can_apply(&s.project.scenarios[i], &period);
            Ok(json!({ "ok": c.ok, "why": c.why }))
        },
    );
    r.add(
        "transport.calibration.trip_end_factors_apply",
        "Apply the fitted trip-end factors to the trip rates of the project (results go out of date).",
        true,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (_, sid, period) = sci(s, p)?;
            let now = s.ids.now_iso();
            let applied = calibrate::apply_factors(&mut s.project, &sid, &period, &now);
            let warnings = after_change(s);
            Ok(json!({ "applied": applied, "warnings": warnings }))
        },
    );
    r.add(
        "transport.calibration.trip_end_factors_revert",
        "Take the applied trip-end factors off again (results go out of date).",
        true,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (_, sid, period) = sci(s, p)?;
            let reverted = calibrate::revert_factors(&mut s.project, &sid, &period);
            let warnings = after_change(s);
            Ok(json!({ "reverted": reverted, "warnings": warnings }))
        },
    );
    r.add(
        "transport.calibration.clear",
        "Remove the matrix estimation factors and fit record of a period (both when none is given); results that used them go out of date.",
        true,
        params(vec![("period", per())], &[]),
        calibration_clear,
    );
    r.add(
        "transport.calibration.validate",
        "Record the base-year validation result for the model status badge (TMDG, the project's category, independent counts inside the area of influence).",
        true,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        calibration_validate,
    );
    r.add(
        "transport.calibration.refresh_validation",
        "Record the validation of both periods for the scenario the validation chapter reports, removing records for any other scenario.",
        true,
        none(),
        |s, _| {
            let now = s.ids.now_iso();
            let changed = calibrate::refresh_validation(&mut s.project, &now);
            s.touch();
            Ok(json!({ "changed": changed }))
        },
    );
    r.add(
        "transport.calibration.gateways_from_counts",
        "Set each gateway's AM and PM flows from the counts near it, in every scenario; results go out of date.",
        true,
        params(vec![("scenario", sc())], &[]),
        gateways_from_counts,
    );
    r.add(
        "transport.calibration.screenlines",
        "List the screenlines and cordons.",
        false,
        none(),
        |s, _| {
            let st = calibrate::CalibStore::read(&s.project);
            Ok(json!({ "screenlines": val(&st.screenlines) }))
        },
    );
    r.add(
        "transport.calibration.screenline_add",
        "Add a screenline or cordon from points across the network.",
        true,
        params(
            vec![
                ("name", string("Name.")),
                (
                    "type",
                    choice(&["screenline", "cordon"], "screenline (default) or cordon."),
                ),
                (
                    "role",
                    choice(
                        &["me", "valid"],
                        "Calibration (me) or validation (valid) screenline.",
                    ),
                ),
                ("motorways", boolean("Include motorway and ramp crossings.")),
                (
                    "points",
                    array(
                        latlng("A point."),
                        "The line's points [[lat, lng], ...] (at least two).",
                    ),
                ),
            ],
            &["points"],
        ),
        screenline_add,
    );
    r.add(
        "transport.calibration.screenline_delete",
        "Delete a screenline or cordon.",
        true,
        params(vec![("id", string("Screenline id."))], &["id"]),
        screenline_delete,
    );
    r.add(
        "transport.calibration.screenline_rows",
        "A screenline's counted and modelled totals by direction with GEH.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("id", string("Screenline id.")),
                (
                    "motorways",
                    boolean("Include motorway crossings (default true)."),
                ),
            ],
            &["id"],
        ),
        screenline_rows,
    );
    r.add(
        "transport.calibration.export",
        "Calibration tables as CSV text: counts against the model, the summary, screenlines or turning counts.",
        false,
        params(vec![("scenario", sc()), ("kind", choice(&["counts", "summary", "screenlines", "turns"], "Which table."))], &["kind"]),
        calibration_export,
    );
    r.add(
        "transport.calibration.turn_counts_import",
        "Read a turning-count file (CSV text) into the project; a site and date already there are replaced.",
        true,
        params(vec![("csv", string("The CSV text.")), ("name", string("The file's name, for the record."))], &["csv"]),
        turn_counts_import,
    );
    r.add_streaming(
        "transport.calibration.stress_test",
        "Run the demand at several factors and report which junctions and roads are most sensitive. Progress is reported.",
        false,
        params(vec![("scenario", sc()), ("period", per()), ("factors", array(json!({ "type": "number" }), "Demand factors (default the panel's)."))], &[]),
        stress_test,
    );
    r.add(
        "transport.calibration.network_checks",
        "Checks that show up against counts: counts far from their road, the worst fits, zones next to motorways and junctions with no delay at V/C over 1.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        network_checks,
    );
}

// ------------------------------------------------------------------ helpers

/// Fits and clears write results into the project: read them back (the ones that cannot be read are named), and count the change.
fn after_change(s: &mut Session) -> Vec<String> {
    let bad = s.load_results();
    s.touch();
    bad
}

fn sci(s: &Session, p: &Params) -> ApiResult<(usize, String, String)> {
    let i = s.pick_scenario(p, "scenario")?;
    Ok((i, s.sid(i), s.period_of(p)?))
}

fn row_json(r: &calibrate::CountRow) -> Value {
    json!({
        "id": r.c.id, "name": val(&r.c.name), "src": val(&r.c.src), "link": r.mt.link, "dir": r.mt.dir, "distanceM": json_number(r.mt.d),
        "linkClass": r.link_cls, "obs": json_number(r.obs), "model": json_number(r.model), "geh": json_number(r.geh), "how": r.how,
        "method": r.method, "role": r.role, "scored": r.scored, "inCore": r.in_core, "motorway": r.mw,
    })
}

// ------------------------------------------------------------------ counts and roles

fn counts_list(s: &mut Session, p: &Params) -> ApiResult {
    let (i, _, period) = sci(s, p)?;
    let ctx = Ctx::new(&s.project);
    let sc = &s.project.scenarios[i];
    let m = ctx.matches(sc);
    let rows: Vec<Value> = ctx
        .data
        .counts
        .iter()
        .map(|c| {
            let mut v = val(&**c);
            v["inUse"] = json!(ctx.data.in_use(c));
            v["role"] = json!(ctx.role_of(c, &period, false));
            v["match"] = match m.get(&c.id) {
                Some(mt) => json!({ "link": mt.link, "dir": mt.dir, "distanceM": json_number(mt.d), "named": mt.named }),
                None => Value::Null,
            };
            v
        })
        .collect();
    let loaded = val(&ctx.data.loaded);
    Ok(
        json!({ "scenario": sc.id, "period": period, "total": rows.len(), "loaded": loaded, "counts": rows }),
    )
}

fn counts_add(s: &mut Session, p: &Params) -> ApiResult {
    let list = p.array("counts")?;
    let mut parsed: Vec<datahub::Count> = Vec::new();
    for (k, v) in list.iter().enumerate() {
        let c: datahub::Count = serde_json::from_value(v.clone())
            .map_err(|e| ApiError::bad_params(format!("`counts[{k}]` is not a count: {e}")))?;
        if c.id.is_empty() {
            return Err(ApiError::bad_params(format!("`counts[{k}]` needs an id")));
        }
        parsed.push(c);
    }
    datahub::ensure_data(&mut s.project);
    let now = s.ids.now_ms();
    let d = s.project.data.get_mut().expect("ensured");
    let arr = d
        .extra
        .entry("counts".to_string())
        .or_insert_with(|| json!([]));
    let Value::Array(a) = arr else {
        return Err(ApiError::invalid_state(
            "The project's counts are not a list.",
        ));
    };
    for c in &parsed {
        let v = serde_json::to_value(c).map_err(|e| ApiError::new("internal", e.to_string()))?;
        match a
            .iter()
            .position(|x| x.get("id").and_then(Value::as_str) == Some(c.id.as_str()))
        {
            Some(at) => a[at] = v,
            None => a.push(v),
        }
    }
    let total = a.len();
    if let Some(src) = p.opt_str("source")?
        && let Maybe::Set(l) = &mut d.loaded
    {
        l.insert(src, json!({ "at": now, "n": parsed.len() }));
    }
    s.touch();
    Ok(json!({ "added": parsed.len(), "total": total }))
}

fn counts_import_csv(s: &mut Session, p: &Params) -> ApiResult {
    let n = datahub::import_csv(&mut s.project, p.str("csv")?, &mut s.ids)
        .map_err(ApiError::bad_params)?;
    s.touch();
    Ok(
        json!({ "imported": n, "message": format!("Imported {n} count{}. They are matched to the nearest road.", if n == 1 { "" } else { "s" }) }),
    )
}

fn set_role(s: &mut Session, p: &Params) -> ApiResult {
    let id = p.str("count")?;
    if !Data::read(&s.project).counts.iter().any(|c| c.id == id) {
        return Err(ApiError::not_found(format!("There is no count {id}.")));
    }
    calibrate::set_role(&mut s.project, id, p.str("role")?);
    s.touch();
    Ok(json!({ "count": id, "role": p.str("role")? }))
}

fn hold_out(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let every = p
        .opt_f64("every")?
        .map_or(calibrate::corridor::HOLD_EVERY, |x| x.max(2.0) as usize);
    let sid = s.sid(i);
    let (valid, calib) = calibrate::hold_out(&mut s.project, &sid, every);
    s.touch();
    Ok(json!({ "validation": valid, "calibration": calib }))
}

// ------------------------------------------------------------------ the model against the counts

fn calibration_summary(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let limit = p.opt_f64("limit")?.map_or(20, |x| x.max(0.0) as usize);
    let data = Data::read(&s.project);
    let sc = &s.project.scenarios[i];
    let sum = datahub::calibration(
        sc,
        s.result(&sid, &period),
        &period,
        &s.project.settings,
        &data,
    );
    Ok(match sum {
        None => {
            json!({ "scenario": sid, "period": period, "run": false, "message": "Run the model to compare it with the counts." })
        }
        Some(x) => json!({
            "scenario": sid, "period": period, "run": true, "matched": x.n, "stats": val(&x.stats),
            "worst": x.rows.iter().take(limit).map(|r| json!({
                "id": r.c.id, "name": val(&r.c.name), "link": r.mt.link, "obs": json_number(r.obs), "model": json_number(r.model), "geh": json_number(r.geh), "how": r.how,
            })).collect::<Vec<_>>(),
        }),
    })
}

fn calibration_rows(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let res = held(s, i, &period)?;
    let ctx = Ctx::new(&s.project);
    let sc = &s.project.scenarios[i];
    let set = p.opt_str("set")?.unwrap_or("all");
    let area = p.opt_str("area")?.unwrap_or("all");
    let limit = p.opt_f64("limit")?.map_or(200, |x| x.max(0.0) as usize);
    let rows = count_rows(&ctx, sc, &period, Some(res), p.flag("raw")?);
    let total = rows.len();
    let keep: Vec<Value> = rows
        .iter()
        .filter(|r| in_set(r, set) && in_area(r, area))
        .take(limit)
        .map(row_json)
        .collect();
    Ok(
        json!({ "scenario": sid, "period": period, "total": total, "count": keep.len(), "rows": keep }),
    )
}

fn calibration_report(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let r = calibrate::report(
        &s.project,
        &s.project.scenarios[i],
        &period,
        s.result(&sid, &period),
    );
    Ok(json!({ "scenario": sid, "report": val(&r) }))
}

fn calibration_assess(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let ctx = Ctx::new(&s.project);
    let category = match p.opt_str("category")? {
        Some(c) => {
            let mut it = c.chars();
            match (it.next(), it.next()) {
                (Some(ch), None) if ('A'..='G').contains(&ch.to_ascii_uppercase()) => {
                    Some(ch.to_ascii_uppercase())
                }
                _ => {
                    return Err(ApiError::bad_params(
                        "`category` must be a letter from A to G",
                    ));
                }
            }
        }
        None => None,
    };
    let o = EvOpts {
        set: p.opt_str("set")?.map(str::to_string),
        area: p.opt_str("area")?.map(str::to_string),
        raw: false,
        strict: p.flag("strict")?,
        standard: p.opt_str("standard")?.map(str::to_string),
        category,
    };
    let a = calibrate::assess(
        &ctx,
        &s.project.scenarios[i],
        &period,
        s.result(&sid, &period),
        &o,
    );
    let rows: Vec<Value> = a
        .rows
        .iter()
        .map(|r| {
            json!({
                "id": r.t.id, "label": r.t.label, "group": r.t.group, "criterion": r.t.crit, "source": r.t.src, "use": r.t.use_,
                "n": r.n, "got": r.shown, "status": r.status, "ok": r.ok, "indicative": r.indicative, "verdict": r.verdict,
            })
        })
        .collect();
    Ok(json!({
        "scenario": sid, "period": period, "standard": a.standard, "category": a.category.to_string(), "status": a.status, "pass": a.pass,
        "also": a.also.iter().map(|c| c.to_string()).collect::<Vec<_>>(), "line": a.line, "rows": rows,
        "fails": a.fails.iter().map(|f| json!({ "category": f.cat.to_string(), "test": f.test, "got": f.got, "target": f.target })).collect::<Vec<_>>(),
    }))
}

fn me_report(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let now = s.ids.now_iso();
    match calibrate::me_report(&s.project, &s.project.scenarios[i], &period, &now) {
        Some(m) => Ok(json!({ "scenario": sid, "period": period, "report": val(&m) })),
        None => Err(ApiError::invalid_state(format!(
            "The {period} peak has no matrix estimation factors. Fit to counts first."
        ))),
    }
}

// ------------------------------------------------------------------ estimation

fn run_me(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let date = s.ids.now_iso();
    let opts = RunMeOpts {
        period: Some(&period),
        xamax: p.opt_f64("xamax")?,
        date: Some(&date),
    };
    let name = s.project.scenarios[i]
        .name
        .as_str()
        .unwrap_or("")
        .to_string();
    let mut cb = |k: f64, n: f64, msg: &str| {
        sink(
            &json!({ "event": "progress", "name": name, "period": period, "pass": json_number(k), "passes": json_number(n), "message": msg }),
        );
    };
    let r =
        calibrate::run_me(&mut s.project, &sid, &opts, Some(&mut cb)).map_err(ApiError::engine)?;
    let warnings = after_change(s);
    Ok(
        json!({ "scenario": sid, "period": period, "ok": r.ok, "note": r.note, "fit": val(&r.out), "warnings": warnings }),
    )
}

fn calibration_fit(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let date = s.ids.now_iso();
    let ids = p.opt_strings("countIds")?;
    let opts = FitOpts {
        period: Some(&period),
        iters: p.opt_f64("iters")?,
        damp: p.opt_f64("damp")?,
        max_factor: p.opt_f64("maxFactor")?,
        max_step: p.opt_f64("maxStep")?,
        min_vol: p.opt_f64("minVol")?,
        constrain: p.opt_bool("constrain")?,
        freeze_gg: p.flag("freezeGg")?,
        weights: p.flag("weights")?,
        weight: None,
        count_ids: ids.as_deref(),
        groups: None,
        date: Some(&date),
    };
    let name = s.project.scenarios[i]
        .name
        .as_str()
        .unwrap_or("")
        .to_string();
    let mut cb = |k: f64, n: f64, msg: &str| {
        sink(
            &json!({ "event": "progress", "name": name, "period": period, "pass": json_number(k), "passes": json_number(n), "message": msg }),
        );
    };
    let r = calibrate::fit(&mut s.project, &sid, &opts, Some(&mut cb)).map_err(ApiError::engine)?;
    let warnings = after_change(s);
    Ok(json!({ "scenario": sid, "period": period, "fit": val(&r), "warnings": warnings }))
}

fn trip_end_factors(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let date = s.ids.now_iso();
    let name = s.project.scenarios[i]
        .name
        .as_str()
        .unwrap_or("")
        .to_string();
    let opts = calibrate::TefOpts {
        period: Some(&period),
        date: Some(&date),
    };
    let mut cb = |k: f64, n: f64, msg: &str| {
        sink(
            &json!({ "event": "progress", "name": name, "period": period, "step": json_number(k), "steps": json_number(n), "message": msg }),
        );
    };
    let r = calibrate::trip_end_factors(&mut s.project, &sid, &opts, Some(&mut cb))
        .map_err(ApiError::engine)?;
    let warnings = after_change(s);
    Ok(
        json!({ "scenario": sid, "period": period, "ok": r.ok, "note": r.note, "record": r.rec, "warnings": warnings }),
    )
}

fn calibration_clear(s: &mut Session, p: &Params) -> ApiResult {
    let period = p.opt_str("period")?;
    let n = calibrate::clear(&mut s.project, period);
    let warnings = after_change(s);
    Ok(json!({ "pairsCleared": n, "warnings": warnings }))
}

fn calibration_validate(s: &mut Session, p: &Params) -> ApiResult {
    let (_, sid, period) = sci(s, p)?;
    let now = s.ids.now_iso();
    let r = calibrate::record_validation(&mut s.project, &sid, &period, &now);
    s.touch();
    Ok(match r {
        Recorded::Written { pass, independent } => {
            json!({ "recorded": true, "pass": pass, "independent": independent })
        }
        Recorded::Dropped => json!({ "recorded": false, "dropped": true }),
        Recorded::Nothing => json!({ "recorded": false, "dropped": false }),
    })
}

fn gateways_from_counts(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let data = Data::read(&s.project);
    let settings = s.project.settings.clone();
    let mut n_active = 0;
    for j in 0..s.project.scenarios.len() {
        let k = datahub::gateways_from_counts(&mut s.project.scenarios[j], &settings, &data);
        if j == i {
            n_active = k;
        }
        s.mark_stale_at(j);
    }
    Ok(json!({
        "gateways": n_active,
        "message": if n_active > 0 {
            format!("Set {n_active} gateway{} from real counts in every scenario. Run the model again.", if n_active == 1 { "" } else { "s" })
        } else {
            "No counts found on the roads at the gateways. Load counts first, or the gateway roads have no count sites.".to_string()
        },
    }))
}

// ------------------------------------------------------------------ screenlines, exports, turning counts, tests

fn screenline_add(s: &mut Session, p: &Params) -> ApiResult {
    let pts: Vec<Point> = p
        .array("points")?
        .iter()
        .map(|v| latlng_of(v, "points").map(Point))
        .collect::<ApiResult<_>>()?;
    if pts.len() < 2 {
        return Err(ApiError::bad_params(
            "A screenline needs at least two points.",
        ));
    }
    let mut st = calibrate::CalibStore::read(&s.project);
    let id = s.ids.uid("sl");
    let n = st.screenlines.len() + 1;
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m.insert(
        "name".into(),
        json!(
            p.opt_str("name")?
                .map_or_else(|| format!("Screenline {n}"), str::to_string)
        ),
    );
    m.insert(
        "type".into(),
        json!(p.opt_str("type")?.unwrap_or("screenline")),
    );
    m.insert("role".into(), json!(p.opt_str("role")?.unwrap_or("me")));
    m.insert(
        "motorways".into(),
        json!(p.opt_bool("motorways")?.unwrap_or(true)),
    );
    m.insert(
        "pts".into(),
        serde_json::to_value(&pts).map_err(|e| ApiError::new("internal", e.to_string()))?,
    );
    let sl: Screenline = serde_json::from_value(Value::Object(m))
        .map_err(|e| ApiError::bad_params(e.to_string()))?;
    st.screenlines.push(sl);
    st.write(&mut s.project);
    s.touch();
    Ok(json!({ "id": id, "screenlines": st.screenlines.len() }))
}

fn screenline_delete(s: &mut Session, p: &Params) -> ApiResult {
    let id = p.str("id")?;
    let mut st = calibrate::CalibStore::read(&s.project);
    let before = st.screenlines.len();
    st.screenlines.retain(|x| x.id != id);
    if st.screenlines.len() == before {
        return Err(ApiError::not_found(format!("There is no screenline {id}.")));
    }
    st.write(&mut s.project);
    s.touch();
    Ok(json!({ "deleted": id }))
}

fn screenline_rows(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let id = p.str("id")?;
    let ctx = Ctx::new(&s.project);
    let sl = ctx
        .store
        .screenlines
        .iter()
        .find(|x| x.id == id)
        .ok_or_else(|| ApiError::not_found(format!("There is no screenline {id}.")))?;
    let rows = calibrate::sl_rows(
        &ctx,
        &s.project.scenarios[i],
        sl,
        &period,
        s.result(&sid, &period),
        p.opt_bool("motorways")?.unwrap_or(true),
    );
    Ok(json!({ "scenario": sid, "period": period, "screenline": id, "rows": val(&rows) }))
}

fn calibration_export(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sid = s.sid(i);
    let ctx = Ctx::new(&s.project);
    let sc = &s.project.scenarios[i];
    let res_of = |per: &str| s.result(&sid, per).cloned();
    let kind = p.str("kind")?;
    let text = match kind {
        "counts" => calibrate::counts_csv(&ctx, sc, &res_of),
        "summary" => calibrate::summary_csv(&ctx, sc, &res_of),
        "screenlines" => calibrate::screenlines_csv(&ctx, sc, &res_of),
        _ => calibrate::turns_csv(&ctx, sc, &res_of),
    };
    Ok(json!({ "kind": kind, "text": text }))
}

fn turn_counts_import(s: &mut Session, p: &Params) -> ApiResult {
    let name = p.opt_str("name")?.unwrap_or("turning counts");
    let r = calibrate::import_turn_csv(&mut s.project, p.str("csv")?, name, &mut s.ids)
        .map_err(ApiError::bad_params)?;
    s.touch();
    Ok(
        json!({ "sites": r.sites.len(), "skipped": r.skipped, "skippedRows": r.skipped_rows, "reasons": r.reasons, "turnSites": val(&r.sites) }),
    )
}

fn stress_test(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let (_, sid, period) = sci(s, p)?;
    let factors: Option<Vec<f64>> = p
        .opt_array("factors")?
        .map(|a| a.iter().filter_map(Value::as_f64).collect());
    let mut cb = |k: f64, n: f64, msg: &str| {
        sink(
            &json!({ "event": "progress", "period": period, "run": json_number(k), "runs": json_number(n), "message": msg }),
        );
    };
    let r = calibrate::stress_test(
        &s.project,
        &sid,
        Some(&period),
        factors.as_deref(),
        Some(&mut cb),
    )
    .map_err(ApiError::engine)?;
    Ok(json!({
        "scenario": r.sc_id, "period": r.period, "factors": r.factors,
        "junctions": r.nodes.iter().map(|n| json!({
            "id": n.id, "name": n.name, "dos": json_number(n.dos0), "sensitivity": json_number(n.sens), "flag": n.flag,
            "runs": n.runs.iter().map(|x| json!({ "factor": x.f, "dos": x.dos.map(json_number), "rise": x.rise.map(json_number) })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "roads": r.links.iter().map(|l| json!({ "key": l.key, "link": l.link, "vc": json_number(l.vc), "factor": l.f })).collect::<Vec<_>>(),
    }))
}

fn network_checks(s: &mut Session, p: &Params) -> ApiResult {
    let (i, sid, period) = sci(s, p)?;
    let ctx = Ctx::new(&s.project);
    let r = calibrate::network_checks(
        &ctx,
        &s.project.scenarios[i],
        &period,
        s.result(&sid, &period),
    );
    Ok(json!({
        "scenario": sid, "period": period,
        "farCounts": r.far.iter().map(row_json).collect::<Vec<_>>(),
        "worstFits": r.worst.iter().map(row_json).collect::<Vec<_>>(),
        "zonesNextToMotorways": r.mw_zones,
        "noDelay": r.no_delay.iter().map(|n| json!({ "key": n.key, "link": n.link, "vc": json_number(n.vc) })).collect::<Vec<_>>(),
    }))
}
