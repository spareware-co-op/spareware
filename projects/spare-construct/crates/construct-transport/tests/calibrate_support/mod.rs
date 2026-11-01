//! Shared by the calibration truth tests: the projections of results the generator (tools/transport-truth/gen-calibrate.mjs)
//! records, and the replay of one recorded fit.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::assign::assign;
use construct_transport::calibrate::{
    FitOpts, Group, ParallelCheck, SiteOpts, SiteRow, Unit, base_view, fit, id_set, parallel,
    report, sites_for, stats, stored_res, units,
};
use construct_transport::datahub::Data;
use construct_transport::jsnum::{Maybe, plain_number};
use construct_transport::project::{Project, Scenario};
use construct_transport::results::{AssignOpts, Res};
use serde_json::{Map, Value, json};

/// A number the way the generator writes it (non-finite as text).
pub fn pn(x: f64) -> Value {
    plain_number(x)
}

/// A list of numbers.
pub fn pv(v: &[f64]) -> Value {
    Value::Array(v.iter().map(|&x| pn(x)).collect())
}

/// An optional number (null when None).
pub fn po(x: Option<f64>) -> Value {
    x.map_or(Value::Null, pn)
}

/// The generator's `resOut`: what is compared of a result.
pub fn res_out(res: &Res) -> Value {
    let mut m = Map::new();
    m.insert("period".into(), json!(res.period));
    m.insert("year".into(), pn(res.year));
    m.insert("it".into(), pn(res.it));
    m.insert("gap".into(), pn(res.gap));
    m.insert("converged".into(), json!(res.converged));
    m.insert("stale".into(), json!(res.stale));
    m.insert("scenario".into(), json!(res.scenario.id));
    m.insert("note".into(), json!(res.note));
    m.insert("manual".into(), json!(res.manual));
    m.insert("O".into(), pv(&res.o));
    m.insert("D".into(), pv(&res.d));
    if let Some(o0) = &res.o0 {
        m.insert("O0".into(), pv(o0));
    }
    if let Some(d0) = &res.d0 {
        m.insert("D0".into(), pv(d0));
    }
    let arcs: Map<String, Value> = res
        .arcs
        .iter()
        .map(|(k, a)| (k.to_string(), pn(a.v)))
        .collect();
    m.insert("arcsV".into(), Value::Object(arcs));
    let paths = match res.paths.get() {
        Some(ps) => json!(ps.iter().map(|od| od.list.len()).sum::<usize>()),
        None => Value::Null,
    };
    m.insert("pathsN".into(), paths);
    Value::Object(m)
}

/// The generator's `rowOut`: a counted site.
pub fn row_out(r: &SiteRow) -> Value {
    json!({ "id": r.c.id, "link": r.mt.link, "dir": r.mt.dir, "d": pn(r.mt.d), "obs": pn(r.obs), "model": pn(r.model),
        "geh": pn(r.geh), "arcs": r.arcs, "w": pn(r.w) })
}

/// A fitting unit as `plain(units(...))` gives it.
pub fn unit_out(u: &Unit) -> Value {
    let mut m = Map::new();
    m.insert("obs".into(), pn(u.obs));
    m.insert("model".into(), pn(u.model));
    m.insert("arcs".into(), json!(u.arcs));
    m.insert("w".into(), pn(u.w));
    m.insert("ids".into(), json!(u.ids));
    if let Some(g) = &u.group {
        m.insert("group".into(), g.clone());
    }
    Value::Object(m)
}

/// The groups the generator passed, from its record.
pub fn groups_of(v: &Value) -> Vec<Group> {
    v.as_array()
        .expect("groups")
        .iter()
        .map(|g| Group {
            id: g.get("id").filter(|x| !x.is_null()).cloned(),
            label: g.get("label").and_then(|x| x.as_str()).map(str::to_string),
            count_ids: g["countIds"]
                .as_array()
                .expect("countIds")
                .iter()
                .map(|x| x.as_str().expect("id").to_string())
                .collect(),
        })
        .collect()
}

/// `plain(parallelCheck)`.
pub fn parallel_out(c: &ParallelCheck, groups: &[Group]) -> Value {
    let g_out = |g: &Group| {
        let mut m = Map::new();
        if let Some(id) = &g.id {
            m.insert("id".into(), id.clone());
        }
        match &g.label {
            Some(l) => {
                m.insert("label".into(), json!(l));
            }
            None => {
                m.insert("label".into(), Value::Null);
            }
        }
        m.insert("countIds".into(), json!(g.count_ids));
        Value::Object(m)
    };
    let _ = groups;
    json!({
        "ok": c.ok.iter().map(|x| json!({ "g": g_out(&x.g), "gi": x.gi, "members": x.members, "ids": x.ids })).collect::<Vec<_>>(),
        "rejected": serde_json::to_value(&c.rejected).expect("rejected"),
    })
}

/// Which results of the other scenarios are marked out of date, as the generator records it: scenario, period, flag.
pub fn stale_of(p: &Project, from: usize) -> Value {
    let mut out = Map::new();
    for s in p.scenarios.iter().skip(from) {
        let mut m = Map::new();
        if let Some(Value::Object(r)) = s.res.get() {
            for (pd, v) in r {
                m.insert(
                    pd.clone(),
                    if v.is_null() {
                        Value::Null
                    } else {
                        json!(
                            v.get("stale")
                                .is_some_and(construct_transport::jsnum::value_truthy)
                        )
                    },
                );
            }
        }
        out.insert(s.id.clone(), Value::Object(m));
    }
    Value::Object(out)
}

/// A scenario's own results' out-of-date flags by period.
pub fn stale_by_period(sc: &Scenario) -> Value {
    let mut m = Map::new();
    if let Some(Value::Object(r)) = sc.res.get() {
        for (pd, v) in r {
            m.insert(
                pd.clone(),
                if v.is_null() {
                    Value::Null
                } else {
                    json!(
                        v.get("stale")
                            .is_some_and(construct_transport::jsnum::value_truthy)
                    )
                },
            );
        }
    }
    Value::Object(m)
}

/// A result stored in the scenario, as the prototype's `sc.res[period] = res`.
pub fn store_res(sc: &mut Scenario, period: &str, res: &Res) {
    if !matches!(sc.res, Maybe::Set(Value::Object(_))) {
        sc.res = Maybe::Set(Value::Object(Map::new()));
    }
    if let Some(Value::Object(r)) = sc.res.get_mut() {
        r.insert(
            period.to_string(),
            serde_json::to_value(res).expect("a result serialises"),
        );
    }
}

/// The options of a recorded fit, with what they borrow kept alive by the caller.
pub struct RecOpts {
    pub period: String,
    pub count_ids: Option<Vec<String>>,
    pub groups: Option<Vec<Group>>,
    pub opts: Value,
    pub caller: bool,
    pub date: String,
}

impl RecOpts {
    pub fn of(rec: &Value) -> RecOpts {
        RecOpts {
            period: rec["period"].as_str().expect("period").to_string(),
            count_ids: rec.get("countIds").filter(|v| !v.is_null()).map(|v| {
                v.as_array()
                    .expect("ids")
                    .iter()
                    .map(|x| x.as_str().expect("id").to_string())
                    .collect()
            }),
            groups: rec.get("groups").filter(|v| !v.is_null()).map(groups_of),
            opts: rec["opts"].clone(),
            caller: rec["weightKind"] == "caller",
            date: rec["date"].as_str().expect("date").to_string(),
        }
    }
    pub fn fit_opts<'a>(
        &'a self,
        weight: Option<&'a dyn Fn(&construct_transport::datahub::Count, &str) -> f64>,
    ) -> FitOpts<'a> {
        let o = &self.opts;
        let f = |k: &str| o.get(k).and_then(|v| v.as_f64());
        FitOpts {
            period: Some(&self.period),
            iters: f("iters"),
            damp: f("damp"),
            max_factor: f("maxFactor"),
            max_step: f("maxStep"),
            min_vol: f("minVol"),
            constrain: o.get("constrain").and_then(|v| v.as_bool()),
            freeze_gg: o.get("freezeGG").and_then(|v| v.as_bool()).unwrap_or(false),
            weights: o.get("weights").and_then(|v| v.as_bool()).unwrap_or(false),
            weight,
            count_ids: self.count_ids.as_deref(),
            groups: self.groups.as_deref(),
            date: Some(&self.date),
        }
    }
}

/// The prior run of a recorded fit: the base-year view assigned without this period's factors, its counted sites, statistics, units and the parallel check.
pub fn prior_block(
    p: &Project,
    sc_id: &str,
    ro: &RecOpts,
    weight: Option<&dyn Fn(&construct_transport::datahub::Count, &str) -> f64>,
) -> Value {
    let sc = p.scenario(sc_id).expect("scenario");
    let period = ro.period.as_str();
    let bv = base_view(sc, &p.settings);
    let mut p2 = p.clone();
    if let Some(me) = p2.me.get_mut() {
        me.factors.remove(period);
    }
    let ao = AssignOpts {
        period: Some(period.to_string()),
        ..AssignOpts::default()
    };
    let res0 = assign(&p2, &bv.view, &p.settings, &ao, None);
    let data = Data::read(p);
    let only = ro.count_ids.as_ref().map(|v| id_set(v));
    let o = ro.fit_opts(weight);
    let so = SiteOpts {
        count_ids: only.as_ref(),
        estimates: true,
        weights: o.weights,
        weight,
    };
    let rows = sites_for(&bv.view, &res0, period, &so, &p.settings, &data);
    let plain_rows = sites_for(
        &bv.view,
        &res0,
        period,
        &SiteOpts::default(),
        &p.settings,
        &data,
    );
    let mut m = Map::new();
    m.insert("same".into(), json!(bv.same));
    m.insert("viewId".into(), json!(bv.view.id));
    m.insert(
        "rows".into(),
        Value::Array(rows.iter().map(row_out).collect()),
    );
    m.insert(
        "stats".into(),
        serde_json::to_value(stats(&rows)).expect("stats"),
    );
    m.insert(
        "plainRows".into(),
        Value::Array(plain_rows.iter().map(row_out).collect()),
    );
    match &ro.groups {
        Some(gs) => {
            let chk = if res0.paths.get().is_some() {
                Some(parallel(&res0, &rows, gs, &p.settings))
            } else {
                None
            };
            m.insert(
                "parallel".into(),
                chk.as_ref().map_or(Value::Null, |c| parallel_out(c, gs)),
            );
            m.insert(
                "units".into(),
                Value::Array(units(&rows, chk.as_ref()).iter().map(unit_out).collect()),
            );
        }
        None => {
            m.insert(
                "units".into(),
                Value::Array(units(&rows, None).iter().map(unit_out).collect()),
            );
        }
    }
    Value::Object(m)
}

/// Replays one recorded fit and compares everything the generator recorded. `weight` is the caller's weights, for a case that has them.
pub fn replay_fit(
    rec: &Value,
    project_text: &str,
    weight: Option<&dyn Fn(&construct_transport::datahub::Count, &str) -> f64>,
) {
    use crate::support::assert_close;
    let name = format!(
        "{}:{}",
        rec["fixture"].as_str().unwrap_or("?"),
        rec["name"].as_str().unwrap_or("?")
    );
    let mut p = Project::from_json(project_text).expect("project reads");
    let ro = RecOpts::of(rec);
    let sc_id = rec["scenario"].as_str().expect("scenario").to_string();
    let idx = p
        .scenarios
        .iter()
        .position(|s| s.id == sc_id)
        .expect("scenario");
    if let Some(s) = rec["setup"].as_str() {
        let setup = FitOpts {
            period: Some(if s == "fit-pm" { "PM" } else { "AM" }),
            date: Some(&ro.date),
            ..FitOpts::default()
        };
        fit(&mut p, &sc_id, &setup, None).expect("setup fit");
    }
    let period = ro.period.clone();
    assert_close(
        &serde_json::to_value(&p.me).expect("me"),
        &rec["meBefore"],
        &format!("{name}.meBefore"),
    );
    if rec.get("reportBefore").is_some() {
        let res = assign(
            &p,
            &p.scenarios[idx],
            &p.settings,
            &AssignOpts {
                period: Some(period.clone()),
                ..AssignOpts::default()
            },
            None,
        );
        store_res(&mut p.scenarios[idx], &period, &res);
        let r = report(
            &p,
            &p.scenarios[idx],
            &period,
            stored_res(&p.scenarios[idx], &period).as_ref(),
        );
        assert_close(
            &serde_json::to_value(&r).expect("report"),
            &rec["reportBefore"],
            &format!("{name}.reportBefore"),
        );
    }
    assert_close(
        &prior_block(&p, &sc_id, &ro, weight),
        &rec["prior"],
        &format!("{name}.prior"),
    );
    // the fit
    let mut progress: Vec<Value> = Vec::new();
    let mut cb = |k: f64, n: f64, m: &str| progress.push(json!([pn(k), pn(n), m]));
    let out = fit(&mut p, &sc_id, &ro.fit_opts(weight), Some(&mut cb)).expect("fit runs");
    assert_close(
        &Value::Array(progress),
        &rec["progress"],
        &format!("{name}.progress"),
    );
    let mut result = serde_json::to_value(&out).expect("fit result");
    result["res"] = out.res.as_ref().map_or(Value::Null, res_out);
    assert_close(&result, &rec["result"], &format!("{name}.result"));
    assert_close(
        &serde_json::to_value(&p.me).expect("me"),
        &rec["me"],
        &format!("{name}.me"),
    );
    let sc = &p.scenarios[idx];
    let own = stored_res(sc, &period);
    assert_close(
        &own.as_ref().map_or(Value::Null, res_out),
        &rec["scRes"],
        &format!("{name}.scRes"),
    );
    let mut keys: Vec<String> = match sc.res.get() {
        Some(Value::Object(m)) => m.keys().cloned().collect(),
        _ => Vec::new(),
    };
    keys.sort();
    let mut want: Vec<String> = rec["scResKeys"]
        .as_array()
        .expect("keys")
        .iter()
        .map(|k| k.as_str().expect("key").to_string())
        .collect();
    want.sort();
    assert_eq!(keys, want, "{name}: the scenario's result periods");
    assert_close(
        &stale_of(&p, 1),
        &rec["otherStale"],
        &format!("{name}.otherStale"),
    );
    if rec.get("report").is_some() {
        for pd in ["AM", "PM"] {
            if stored_res(&p.scenarios[idx], pd).is_none() {
                let res = assign(
                    &p,
                    &p.scenarios[idx],
                    &p.settings,
                    &AssignOpts {
                        period: Some(pd.to_string()),
                        ..AssignOpts::default()
                    },
                    None,
                );
                store_res(&mut p.scenarios[idx], pd, &res);
            }
            let r = report(
                &p,
                &p.scenarios[idx],
                pd,
                stored_res(&p.scenarios[idx], pd).as_ref(),
            );
            assert_close(
                &serde_json::to_value(&r).expect("report"),
                &rec["report"][pd],
                &format!("{name}.report.{pd}"),
            );
        }
    }
    if let Some(clears) = rec.get("clears").and_then(|c| c.as_array()) {
        for (k, c) in clears.iter().enumerate() {
            let pd = c["period"].as_str();
            let arg = match pd {
                Some("both") | None => None,
                Some(x) => Some(x),
            };
            let n = construct_transport::calibrate::clear(&mut p, arg);
            let path = format!("{name}.clears[{k}]");
            assert_eq!(n as u64, c["n"].as_u64().expect("n"), "{path}.n");
            assert_close(
                &serde_json::to_value(&p.me).expect("me"),
                &c["me"],
                &format!("{path}.me"),
            );
            assert_close(
                &stale_of(&p, 1),
                &c["otherStale"],
                &format!("{path}.otherStale"),
            );
            assert_close(
                &stale_by_period(&p.scenarios[idx]),
                &c["scStale"],
                &format!("{path}.scStale"),
            );
        }
    }
}

pub mod panel;
pub mod states;
