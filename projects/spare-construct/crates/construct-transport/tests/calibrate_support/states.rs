//! Replays of the calibration panel's states (tools/transport-truth/gen-calibrate.mjs, `calibrationStates`): the
//! snapshot of everything the panel computes for a scenario, and the steps each state takes beforehand.
#![allow(dead_code)]

use super::panel::*;
use super::{pn, res_out, store_res};
use construct_transport::assign::assign;
use construct_transport::calibrate::*;
use construct_transport::datahub::{Data, gateways_from_counts};
use construct_transport::jsnum::{JsNum, Maybe, Num};
use construct_transport::project::{CalibSettings, Project, TmdgSettings};
use construct_transport::results::{AssignOpts, Res};
use serde_json::{Map, Value, json};

/// The clock the generator's context stands still on: 2026-10-21T00:00:00.000Z.
pub const ISO_NOW: &str = "2026-10-21T00:00:00.000Z";

/// Runs and stores both periods of a scenario (the generator's `runBoth`).
pub fn run_both(p: &mut Project, sc_id: &str) {
    for pd in ["AM", "PM"] {
        run_period(p, sc_id, pd);
    }
}

/// Runs and stores one period of a scenario.
pub fn run_period(p: &mut Project, sc_id: &str, pd: &str) {
    let idx = p
        .scenarios
        .iter()
        .position(|s| s.id == sc_id)
        .expect("scenario");
    let res = assign(
        p,
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

fn by_key<'a, T>(items: impl Iterator<Item = (String, T)>) -> Map<String, Value>
where
    T: Into<Value>,
{
    items.map(|(k, v)| (k, v.into())).collect()
}

/// `snapshot(c, P, sc, label, { full, level })`: the panel's numbers for a scenario in both periods.
pub fn snapshot(p: &Project, sc_id: &str, label: &str, level: &str, full: bool) -> Value {
    let ctx = Ctx::new(p);
    let sc = p.scenario(sc_id).expect("scenario");
    let mut rec = Map::new();
    rec.insert("label".into(), json!(label));
    rec.insert("scenario".into(), json!(sc_id));
    let role_map = |pd: &str, raw: bool| -> Value {
        Value::Object(by_key(
            ctx.data
                .counts
                .iter()
                .map(|c| (c.id.clone(), ctx.role_of(c, pd, raw).to_string())),
        ))
    };
    rec.insert("roles".into(), json!({ "AM": role_map("AM", false), "PM": role_map("PM", false), "rawAM": role_map("AM", true) }));
    rec.insert(
        "methods".into(),
        Value::Object(
            ctx.data
                .counts
                .iter()
                .map(|c| {
                    (
                        c.id.clone(),
                        json!({ "AM": method_of(c, "AM"), "PM": method_of(c, "PM") }),
                    )
                })
                .collect(),
        ),
    );
    let gw = ctx.gateway_ids();
    rec.insert(
        "gatewayIds".into(),
        json!(
            ctx.data
                .counts
                .iter()
                .filter(|c| gw.contains(&c.id))
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        ),
    );
    rec.insert("core".into(), core_out(&core_area(&ctx, sc)));
    rec.insert(
        "store".into(),
        serde_json::to_value(&ctx.store).expect("store"),
    );
    let mut periods = Map::new();
    for pd in ["AM", "PM"] {
        let res = stored_res(sc, pd);
        let mut pr = Map::new();
        pr.insert("hasRes".into(), json!(res.is_some()));
        let Some(res) = res else {
            periods.insert(pd.to_string(), Value::Object(pr));
            continue;
        };
        let rows = count_rows(&ctx, sc, pd, Some(&res), false);
        pr.insert(
            "rows".into(),
            Value::Array(rows.iter().map(count_row_out).collect()),
        );
        let raw = count_rows(&ctx, sc, pd, Some(&res), true);
        pr.insert(
            "rowsRaw".into(),
            Value::Array(raw.iter().map(count_row_out).collect()),
        );
        let all_refs: Vec<&CountRow> = rows.iter().collect();
        let scored_refs: Vec<&CountRow> = rows.iter().filter(|r| r.scored).collect();
        pr.insert("stats".into(), json!({ "all": cal_stats_out(&stats_of_count_rows(&all_refs)), "scored": cal_stats_out(&stats_of_count_rows(&scored_refs)) }));
        let mut sls = Map::new();
        for sl in &ctx.store.screenlines {
            let cr: Vec<Value> = sl_crossings(sc, sl)
                .iter()
                .map(|x| json!({ "link": x.link, "side": x.side, "ll": [pn(x.ll[0]), pn(x.ll[1])], "bAB": pn(x.b_ab), "mw": x.mw }))
                .collect();
            sls.insert(
                sl.id.clone(),
                json!({ "crossings": cr,
                    "rows": sl_rows(&ctx, sc, sl, pd, Some(&res), true).iter().map(sl_row_out).collect::<Vec<_>>(),
                    "rowsNoMw": sl_rows(&ctx, sc, sl, pd, Some(&res), false).iter().map(sl_row_out).collect::<Vec<_>>() }),
            );
        }
        pr.insert("sl".into(), Value::Object(sls));
        let mut obs = Map::new();
        for set in ["valid", "calib", "all"] {
            obs.insert(
                set.to_string(),
                sl_obs_out(&sl_observations(&ctx, sc, pd, set, Some(&res)), &ctx.store),
            );
        }
        pr.insert("slObs".into(), Value::Object(obs));
        let jr = journeys(&ctx, sc, pd, Some(&res));
        pr.insert(
            "journeys".into(),
            Value::Array(jr.iter().map(jr_out).collect()),
        );
        pr.insert("jtStats".into(), jt_stats_out(&jt_stats(&jr)));
        let mut turn = Map::new();
        for s in &ctx.store.turn_sites {
            turn.insert(
                s.id.clone(),
                site_rows_out(&site_rows(&ctx, sc, s, pd, Some(&res)), &s.id),
            );
        }
        pr.insert("turn".into(), Value::Object(turn));
        let nc = network_checks(&ctx, sc, pd, Some(&res));
        pr.insert(
            "network".into(),
            json!({ "far": nc.far.iter().map(count_row_out).collect::<Vec<_>>(), "worst": nc.worst.iter().map(count_row_out).collect::<Vec<_>>(),
                "mwZones": nc.mw_zones, "noDelay": nc.no_delay.iter().map(|x| json!({ "key": x.key, "link": x.link, "vc": pn(x.vc) })).collect::<Vec<_>>() }),
        );
        if !full {
            periods.insert(pd.to_string(), Value::Object(pr));
            continue;
        }
        let sets: &[&str] = match level {
            "max" => &["valid", "calib", "all"],
            "mid" => &["valid", "calib"],
            _ => &["valid"],
        };
        let areas: &[&str] = match level {
            "max" => &["auto", "all", "inside", "outside"],
            "mid" => &["auto", "all"],
            _ => &["auto"],
        };
        let mut evs = Map::new();
        for set in sets {
            for area in areas {
                let o = EvOpts {
                    set: Some(set.to_string()),
                    area: Some(area.to_string()),
                    ..EvOpts::default()
                };
                evs.insert(
                    format!("{set}|{area}"),
                    ev_out(
                        &evidence(&ctx, sc, pd, Some(&res), &o),
                        &ctx.store,
                        *set == "valid" && *area == "auto",
                    ),
                );
                if *set == "valid" && *area == "auto" {
                    let o2 = EvOpts {
                        strict: true,
                        ..o.clone()
                    };
                    evs.insert(
                        "valid|auto|strict".into(),
                        ev_out(&evidence(&ctx, sc, pd, Some(&res), &o2), &ctx.store, false),
                    );
                }
            }
        }
        if level != "min" {
            let o = EvOpts {
                set: Some("valid".into()),
                raw: true,
                ..EvOpts::default()
            };
            evs.insert(
                "valid|auto|raw".into(),
                ev_out(&evidence(&ctx, sc, pd, Some(&res), &o), &ctx.store, false),
            );
        }
        pr.insert("evidence".into(), Value::Object(evs));
        let mut asx = Map::new();
        let stds: &[&str] = if level == "max" {
            &["tmdg", "rms", "atap"]
        } else {
            &["tmdg", "rms"]
        };
        for std in stds {
            let cats: Vec<char> = if *std == "tmdg" {
                if level == "max" {
                    vec!['A', 'B', 'C', 'D', 'E', 'F', 'G']
                } else {
                    vec!['A', 'E', 'F']
                }
            } else {
                vec!['E']
            };
            for cat in cats {
                let sets2: Vec<&str> = if *std == "tmdg" && cat == 'E' {
                    vec!["valid", "calib", "all"]
                } else {
                    vec!["valid"]
                };
                for set in sets2 {
                    let o = EvOpts {
                        standard: Some(std.to_string()),
                        category: Some(cat),
                        set: Some(set.to_string()),
                        ..EvOpts::default()
                    };
                    asx.insert(
                        format!("{std}|{cat}|{set}"),
                        assess_out(&assess(&ctx, sc, pd, Some(&res), &o)),
                    );
                }
            }
        }
        asx.insert(
            "default".into(),
            assess_out(&assess(&ctx, sc, pd, Some(&res), &EvOpts::default())),
        );
        let o = EvOpts {
            set: Some("valid".into()),
            strict: true,
            area: Some("auto".into()),
            ..EvOpts::default()
        };
        asx.insert(
            "valid|strict|auto".into(),
            assess_out(&assess(&ctx, sc, pd, Some(&res), &o)),
        );
        pr.insert("assess".into(), Value::Object(asx));
        periods.insert(pd.to_string(), Value::Object(pr));
    }
    rec.insert("periods".into(), Value::Object(periods));
    if full && level == "max" {
        let prior = ctx.cfg.prior_high();
        let _ = prior;
        let mut tests = Map::new();
        for std in ["tmdg", "rms", "atap"] {
            let cats: Vec<char> = if std == "tmdg" {
                vec!['A', 'B', 'C', 'D', 'E', 'F', 'G']
            } else {
                vec!['E']
            };
            for cat in cats {
                tests.insert(
                    format!("{std}|{cat}"),
                    Value::Array(tests_for(&ctx, std, cat).iter().map(test_out).collect()),
                );
            }
        }
        rec.insert("tests".into(), Value::Object(tests));
        let e = tests_for(&ctx, "tmdg", 'E');
        rec.insert(
            "tagPeers".into(),
            Value::Object(
                e.iter()
                    .map(|t| {
                        (
                            t.id.clone(),
                            tag_peer(t).map_or(Value::Null, |p| test_out(&p)),
                        )
                    })
                    .collect(),
            ),
        );
        rec.insert(
            "targetText".into(),
            Value::Object(
                e.iter()
                    .map(|t| (t.id.clone(), json!(target_text(t))))
                    .collect(),
            ),
        );
        let mut tips = Map::new();
        for t in e.iter().chain(tests_for(&ctx, "tmdg", 'A').iter()) {
            tips.insert(t.id.clone(), json!(tooltip(t)));
        }
        rec.insert("tooltips".into(), Value::Object(tips));
    }
    if full {
        let res_of = |pd: &str| stored_res(sc, pd);
        rec.insert(
            "csv".into(),
            json!({ "counts": counts_csv(&ctx, sc, &res_of), "summary": summary_csv(&ctx, sc, &res_of),
                "screenlines": screenlines_csv(&ctx, sc, &res_of), "turns": turns_csv(&ctx, sc, &res_of) }),
        );
    }
    Value::Object(rec)
}

fn fit_json(out: &FitResult) -> Value {
    let mut v = serde_json::to_value(out).expect("fit result");
    v["res"] = out.res.as_ref().map_or(Value::Null, res_out);
    v
}

fn saved_roles(p: &Project) -> Value {
    serde_json::to_value(&CalibStore::read(p).roles).expect("roles")
}

fn count_roles(p: &Project) -> Value {
    let arr = match p.data.get().and_then(|d| d.extra.get("counts")) {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    Value::Array(
        arr.iter()
            .map(|c| json!([c["id"], c.get("role").cloned().unwrap_or(Value::Null)]))
            .collect(),
    )
}

fn stale_flags(p: &Project) -> Value {
    Value::Array(
        p.scenarios
            .iter()
            .map(|s| {
                let mut m = Map::new();
                if let Some(Value::Object(r)) = s.res.get() {
                    for (k, v) in r {
                        m.insert(
                            k.clone(),
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
            })
            .collect(),
    )
}

fn calib_of(sc: &construct_transport::project::Scenario) -> Value {
    sc.extra.get("calib").cloned().unwrap_or(Value::Null)
}

fn tef_json(r: &TefResult) -> Value {
    match &r.rec {
        Some(Value::Object(m)) => {
            let mut o = Map::new();
            o.insert("ok".into(), json!(true));
            for (k, v) in m {
                o.insert(k.clone(), v.clone());
            }
            Value::Object(o)
        }
        _ => json!({ "ok": false, "note": r.note }),
    }
}

fn can_apply_json(c: &CanApply) -> Value {
    json!({ "ok": c.ok, "why": c.why })
}

fn record_json(p: &Project, pd: &str, r: &Recorded) -> Value {
    match r {
        Recorded::Written { .. } => p
            .validation
            .get()
            .and_then(|v| v.get(pd))
            .and_then(|v| v.get())
            .map_or(Value::Null, |v| serde_json::to_value(v).expect("record")),
        _ => Value::Null,
    }
}

fn period_res_flag(p: &Project, sc_id: &str, pd: &str) -> Option<Res> {
    stored_res(p.scenario(sc_id).expect("scenario"), pd)
}

/// The steps one panel state takes on the project (a fresh copy with both periods run), and the records it makes.
pub fn replay_state(
    p: &mut Project,
    name: &str,
    fixture: &str,
    extra_core: &Value,
) -> Map<String, Value> {
    let sc_id = p.scenarios[0].id.clone();
    let mut out = Map::new();
    let date = ISO_NOW;
    match name {
        "held-out" => {
            let (v, c) = hold_out_default(p, &sc_id);
            out.insert("holdOut".into(), json!({ "valid": v, "calib": c }));
            out.insert("afterRoles".into(), saved_roles(p));
        }
        "roles-by-hand" => {
            let ids: Vec<String> = Data::read(p).counts.iter().map(|c| c.id.clone()).collect();
            set_role(p, &ids[0], "valid");
            set_role(p, &ids[1], "off");
            set_role(p, &ids[2], "valid");
            set_role(p, &ids[3], "gateway");
            set_role(p, &ids[4], "me");
            set_role(p, &ids[5], "bogus");
            set_role(p, &ids[2], "valid");
            out.insert("afterRoles".into(), saved_roles(p));
            out.insert("counts".into(), count_roles(p));
            hold_out_default(p, &sc_id);
            out.insert("holdOutOnlyOff".into(), saved_roles(p));
            set_role(p, &ids[1], "off");
            hold_out_default(p, &sc_id);
            out.insert("afterHoldOutKeepingOff".into(), saved_roles(p));
            reset_roles(p);
            out.insert("afterReset".into(), saved_roles(p));
            out.insert("countsAfterReset".into(), count_roles(p));
            hold_out(p, &sc_id, 3);
            out.insert("afterHoldOut3".into(), saved_roles(p));
        }
        "after-me" => {
            hold_out_default(p, &sc_id);
            let mut prog: Vec<Value> = Vec::new();
            let mut cb = |k: f64, n: f64, m: &str| prog.push(json!([pn(k), pn(n), m]));
            let r = run_me(
                p,
                &sc_id,
                &RunMeOpts {
                    period: Some("AM"),
                    xamax: None,
                    date: Some(date),
                },
                Some(&mut cb),
            )
            .expect("runME");
            out.insert("runMe".into(), json!({ "ok": r.ok, "note": r.note, "out": r.out.as_ref().map_or(Value::Null, fit_json), "progress": prog }));
            out.insert("me".into(), serde_json::to_value(&p.me).expect("me"));
            let sc = p.scenario(&sc_id).expect("sc").clone();
            out.insert(
                "meReport".into(),
                me_report(p, &sc, "AM", date).map_or(Value::Null, |m| {
                    let mut v = serde_json::to_value(&m).expect("m1");
                    null_non_finite(&mut v);
                    v
                }),
            );
            out.insert("meStats".into(), me_out(&me_stats(p, "AM")));
            let (r2, tried) = reduce_me(
                p,
                &sc_id,
                &RunMeOpts {
                    period: Some("PM"),
                    xamax: None,
                    date: Some(date),
                },
                Some(2.0),
                None,
            )
            .expect("reduceME");
            out.insert(
                "reduceMe".into(),
                json!({ "ok": r2.ok, "note": r2.note, "tried": tried.iter().map(|t| json!({ "xamax": pn(t.xamax), "ok": t.ok, "note": t.note })).collect::<Vec<_>>(),
                    "out": r2.out.as_ref().map_or(Value::Null, fit_json) }),
            );
            out.insert("me2".into(), serde_json::to_value(&p.me).expect("me"));
            run_both(p, &sc_id);
            let a = record_validation(p, &sc_id, "AM", date);
            let am = record_json(p, "AM", &a);
            let b = record_validation(p, &sc_id, "PM", date);
            let pm = record_json(p, "PM", &b);
            let refreshed = refresh_validation(p, date);
            out.insert(
                "validation".into(),
                json!({ "recorded": { "AM": am, "PM": pm }, "refreshed": refreshed, "P": serde_json::to_value(&p.validation).expect("validation") }),
            );
            out.insert(
                "reportScenario".into(),
                json!(report_scenario(p).map(|s| s.id.clone())),
            );
        }
        "after-tef" => {
            hold_out_default(p, &sc_id);
            let mut prog: Vec<Value> = Vec::new();
            let mut cb = |k: f64, n: f64, m: &str| prog.push(json!([pn(k), pn(n), m]));
            let r = trip_end_factors(
                p,
                &sc_id,
                &TefOpts {
                    period: Some("AM"),
                    date: Some(date),
                },
                Some(&mut cb),
            )
            .expect("tef");
            out.insert("tef".into(), tef_json(&r));
            out.insert("tefProgress".into(), Value::Array(prog));
            out.insert("sc_calib".into(), calib_of(p.scenario(&sc_id).expect("sc")));
            out.insert(
                "canApply".into(),
                can_apply_json(&can_apply(p.scenario(&sc_id).expect("sc"), "AM")),
            );
            let applied = apply_factors(p, &sc_id, "AM", date);
            out.insert("applied".into(), json!(applied));
            out.insert(
                "luFactors".into(),
                serde_json::to_value(&p.lu_factors).expect("lu"),
            );
            out.insert(
                "scCalibApplied".into(),
                calib_of(p.scenario(&sc_id).expect("sc")),
            );
            out.insert(
                "canApplyAfter".into(),
                can_apply_json(&can_apply(p.scenario(&sc_id).expect("sc"), "AM")),
            );
            out.insert("staleAfter".into(), stale_flags(p));
            run_both(p, &sc_id);
            let rv = revert_factors(p, &sc_id, "AM");
            out.insert("revert".into(), json!(rv));
            out.insert(
                "luAfterRevert".into(),
                serde_json::to_value(&p.lu_factors).expect("lu"),
            );
            out.insert(
                "scCalibReverted".into(),
                calib_of(p.scenario(&sc_id).expect("sc")),
            );
            apply_factors(p, &sc_id, "AM", date);
            out.insert("setupApplyAgain".into(), json!(true));
            run_both(p, &sc_id);
            let t2 = trip_end_factors(
                p,
                &sc_id,
                &TefOpts {
                    period: Some("PM"),
                    date: Some(date),
                },
                None,
            )
            .expect("tef");
            out.insert("tefPM".into(), tef_json(&t2));
            out.insert(
                "canApplyPM".into(),
                can_apply_json(&can_apply(p.scenario(&sc_id).expect("sc"), "PM")),
            );
            out.insert(
                "canApplyNone".into(),
                can_apply_json(&can_apply(p.scenario(&sc_id).expect("sc"), "PM")),
            );
            let last = p.scenarios.last().expect("scenario").clone();
            out.insert(
                "canApplyNoRec".into(),
                can_apply_json(&can_apply(&last, "AM")),
            );
        }
        "tef-screenlines-off" => {
            let mut st = CalibStore::read(p);
            st.screenlines.retain(|s| s.is_type("core") || s.is_valid());
            st.write(p);
            let r = trip_end_factors(
                p,
                &sc_id,
                &TefOpts {
                    period: Some("AM"),
                    date: Some(date),
                },
                None,
            )
            .expect("tef");
            out.insert("tef".into(), tef_json(&r));
            let ids: Vec<String> = Data::read(p).counts.iter().map(|c| c.id.clone()).collect();
            for id in &ids {
                set_role(p, id, "valid");
            }
            let r2 = trip_end_factors(
                p,
                &sc_id,
                &TefOpts {
                    period: Some("PM"),
                    date: Some(date),
                },
                None,
            )
            .expect("tef");
            out.insert("tefNone".into(), tef_json(&r2));
            for id in &ids {
                set_role(p, id, "off");
            }
            let r3 = run_me(
                p,
                &sc_id,
                &RunMeOpts {
                    period: Some("AM"),
                    xamax: None,
                    date: Some(date),
                },
                None,
            )
            .expect("runME");
            out.insert("runMeNone".into(), json!({ "ok": r3.ok, "note": r3.note }));
            let (r4, tried) = reduce_me(
                p,
                &sc_id,
                &RunMeOpts {
                    period: Some("AM"),
                    xamax: None,
                    date: Some(date),
                },
                None,
                None,
            )
            .expect("reduceME");
            out.insert(
                "reduceNone".into(),
                json!({ "ok": r4.ok, "note": r4.note, "tried": tried.iter().map(|t| json!({ "xamax": pn(t.xamax), "ok": t.ok, "note": t.note })).collect::<Vec<_>>() }),
            );
            reset_roles(p);
        }
        "other-scenario" => {
            let s2_id = p.scenarios[1].id.clone();
            {
                let by = p.settings.base_year.clone();
                let s2 = &mut p.scenarios[1];
                let ids: Vec<String> = s2.links.keys().map(String::from).collect();
                {
                    let l = s2.links.get_mut(&ids[2]).expect("link");
                    l.lanes_ab = Num::number(3.0);
                    l.speed = Num::number(70.0);
                }
                s2.links.remove(ids.last().expect("a link"));
                s2.year = by;
            }
            run_both(p, &s2_id);
            let ctx_core = {
                let ctx = Ctx::new(p);
                core_out(&core_area(&ctx, p.scenario(&s2_id).expect("sc")))
            };
            out.insert("scenarioIndex".into(), json!(1));
            out.insert("core2".into(), ctx_core);
            out.insert(
                "snap2".into(),
                snapshot(p, &s2_id, "scenario 1", "mid", true),
            );
            let ctx = Ctx::new(p);
            let s2 = p.scenario(&s2_id).expect("sc");
            let res = stored_res(s2, "AM");
            let mut cordon = Map::new();
            for sl in &ctx.store.screenlines {
                let r = cordon_od(&ctx, s2, res.as_ref(), Some(sl));
                cordon.insert(
                    sl.id.clone(),
                    match r {
                        Err(e) => json!({ "error": e }),
                        Ok(r) => json!({ "o": r.o, "csv": r.csv, "geojson": serde_json::from_str::<Value>(&r.geojson).expect("geojson"),
                            "zones": r.zones, "boundary": r.boundary, "trips": pn(r.trips) }),
                    },
                );
            }
            out.insert("cordon".into(), Value::Object(cordon));
            let cord = ctx.store.screenlines.iter().find(|s| s.is_type("cordon"));
            out.insert(
                "cordonNoRes".into(),
                cordon_od(&ctx, s2, None, cord)
                    .err()
                    .map_or(Value::Null, |e| json!({ "error": e })),
            );
            out.insert(
                "cordonNoCordon".into(),
                cordon_od(&ctx, s2, res.as_ref(), None)
                    .err()
                    .map_or(Value::Null, |e| json!({ "error": e })),
            );
        }
        "core-drawn" => {
            let mut st = CalibStore::read(p);
            let core: Screenline = serde_json::from_value(extra_core.clone()).expect("core area");
            st.screenlines.push(core);
            st.write(p);
            hold_out_default(p, &sc_id);
            p.settings.calib = Maybe::Set(
                serde_json::from_value::<CalibSettings>(json!({ "coreBuffer": 300 }))
                    .expect("calib"),
            );
        }
        "settings" => {
            p.settings.tmdg = Maybe::Set(
                serde_json::from_value::<TmdgSettings>(
                    json!({ "category": "c", "justification": "Corridor assessment" }),
                )
                .expect("tmdg"),
            );
            p.settings.calib = Maybe::Set(
                serde_json::from_value::<CalibSettings>(json!({ "standard": "rms", "set": "all", "area": "all", "minComplete": 0.95, "priorQuality": "low",
                    "xamax": 1.5, "freezeGG": false, "windows": { "AM": ["07:30", "08:30"], "PM": ["16:30", "17:30"] } }))
                .expect("calib"),
            );
            if let Some(d) = p.settings.data.get_mut() {
                d.am_share = Num::number(9.0);
                d.pm_share = Num::number(10.0);
            }
            hold_out_default(p, &sc_id);
            let n = repeak(p);
            out.insert("repeaked".into(), json!(n));
            let st = CalibStore::read(p);
            out.insert(
                "peaks".into(),
                Value::Array(
                    st.turn_sites
                        .iter()
                        .map(|s| json!({ "id": s.id, "peak": serde_json::to_value(&s.peak).expect("peak"), "peakWin": serde_json::to_value(&s.peak_win).expect("win"),
                            "bal": serde_json::to_value(&s.bal).expect("bal") }))
                        .collect(),
                ),
            );
            out.insert("repeakedAgain".into(), json!(repeak(p)));
        }
        "few-counts" => {
            if let Some(d) = p.data.get_mut()
                && let Some(Value::Array(a)) = d.extra.get_mut("counts")
            {
                let user: Vec<Value> = a
                    .iter()
                    .filter(|c| c["src"] == "user")
                    .take(6)
                    .cloned()
                    .collect();
                *a = user;
            }
            let mut st = CalibStore::read(p);
            for s in st.turn_sites.iter_mut() {
                s.role = Maybe::Set(JsNum::Text("off".into()));
            }
            st.write(p);
            hold_out_default(p, &sc_id);
        }
        "gateway-counts" => {
            let data = Data::read(p);
            let settings = p.settings.clone();
            let n = gateways_from_counts(&mut p.scenarios[0], &settings, &data);
            out.insert("gatewaysSet".into(), json!(n));
            let gw_src: Map<String, Value> = p.scenarios[0]
                .zones
                .iter()
                .filter(|(_, z)| {
                    z.gw_src
                        .get()
                        .is_some_and(construct_transport::jsnum::value_truthy)
                })
                .map(|(id, z)| {
                    (
                        id.to_string(),
                        z.gw_src.get().cloned().unwrap_or(Value::Null),
                    )
                })
                .collect();
            out.insert("gwSrc".into(), Value::Object(gw_src));
            hold_out_default(p, &sc_id);
            run_both(p, &sc_id);
        }
        "balanced" => {
            match_sites(p, &sc_id);
            let res = period_res_flag(p, &sc_id, "AM");
            let (audit, n) = balance(p, &sc_id, "AM", None, res.as_ref());
            out.insert(
                "balance".into(),
                json!({ "audit": audit_json(&audit), "sites": n }),
            );
            out.insert("balStore".into(), bal_store(p, true));
            let res = period_res_flag(p, &sc_id, "PM");
            let (audit2, n2) = balance(p, &sc_id, "PM", Some(true), res.as_ref());
            out.insert(
                "balanceNoSinks".into(),
                json!({ "audit": audit_json(&audit2), "sites": n2 }),
            );
            out.insert("balStore2".into(), bal_store(p, false));
            let res = period_res_flag(p, &sc_id, "AM");
            let (audit3, n3) = balance(p, &sc_id, "AM", Some(false), res.as_ref());
            out.insert(
                "balanceAgain".into(),
                json!({ "audit": audit_json(&audit3), "sites": n3 }),
            );
            let st = CalibStore::read(p);
            let sc = p.scenario(&sc_id).expect("sc");
            let mut phf_all = Map::new();
            for id in sc.nodes.keys() {
                let a = construct_transport::calibrate::phf(&st, sc, id, "AM");
                let b = construct_transport::calibrate::phf(&st, sc, id, "PM");
                if a.is_some() || b.is_some() {
                    phf_all.insert(id.to_string(), json!({ "AM": a.map_or(Value::Null, |x| pn(x)), "PM": b.map_or(Value::Null, |x| pn(x)) }));
                }
            }
            out.insert("phf".into(), Value::Object(phf_all));
            let mut st2 = st.clone();
            if let Some(s) = st2.turn_sites.first_mut() {
                s.role = Maybe::Set(JsNum::Text("off".into()));
            }
            let mut off = Map::new();
            for id in sc.nodes.keys() {
                if let Some(x) = construct_transport::calibrate::phf(&st2, sc, id, "AM") {
                    off.insert(id.to_string(), pn(x));
                }
            }
            out.insert("phfOff".into(), Value::Object(off));
            let mut st3 = st;
            if let Some(s) = st3.turn_sites.first_mut() {
                s.role = Maybe::Set(JsNum::Text("valid".into()));
            }
            st3.write(p);
        }
        "no-sites" => {
            p.scenarios[0].sites = Maybe::Set(construct_transport::project::OrderedMap::new());
            match_sites(p, &sc_id);
            let st = CalibStore::read(p);
            out.insert(
                "nodes".into(),
                Value::Array(
                    st.turn_sites
                        .iter()
                        .map(|s| {
                            json!([
                                s.id,
                                serde_json::to_value(&s.node).expect("node"),
                                serde_json::to_value(&s.site).expect("site")
                            ])
                        })
                        .collect(),
                ),
            );
        }
        _ => {}
    }
    let _ = fixture;
    out
}

fn audit_json(a: &[BalanceAudit]) -> Value {
    Value::Array(
        a.iter()
            .map(|x| json!({ "site": x.site, "siteId": x.site_id, "movement": x.movement, "observed": pn(x.observed), "balanced": pn(x.balanced), "d": pn(x.d),
                "pct": x.pct.map_or(Value::Null, pn), "reason": x.reason, "sink": x.sink }))
            .collect(),
    )
}

fn bal_store(p: &Project, with_node: bool) -> Value {
    let st = CalibStore::read(p);
    Value::Array(
        st.turn_sites
            .iter()
            .map(|s| {
                let mut o = Map::new();
                o.insert("id".into(), json!(s.id));
                if with_node {
                    o.insert("node".into(), serde_json::to_value(&s.node).expect("node"));
                }
                if s.bal.is_set() {
                    o.insert("bal".into(), serde_json::to_value(&s.bal).expect("bal"));
                }
                if s.audit.is_set() {
                    o.insert(
                        "audit".into(),
                        serde_json::to_value(&s.audit).expect("audit"),
                    );
                }
                Value::Object(o)
            })
            .collect(),
    )
}

fn null_non_finite(v: &mut Value) {
    match v {
        Value::String(s) if s == "NaN" || s == "Infinity" || s == "-Infinity" => *v = Value::Null,
        Value::Array(a) => a.iter_mut().for_each(null_non_finite),
        Value::Object(m) => m.values_mut().for_each(null_non_finite),
        _ => {}
    }
}
