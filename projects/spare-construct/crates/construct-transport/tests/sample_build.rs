//! Drury sample truth tests: `Sample.build` and `Sample.analyse` against the prototype with the network stubbed (tools/transport-truth/gen-sample.mjs): the
//! requests, the progress messages, the notes, the sources that failed, the project made and the results of the runs, in many ways.

mod census_support;
mod edit_support;
mod sample_support;
mod support;

use census_support::{Replay, ids_for};
use construct_transport::census::Downloads;
use construct_transport::jsnum::Maybe;
use construct_transport::project::Project;
use construct_transport::sample::{AnalyseOpts, BuildOpts, Modules, PERIODS, SampleError, analyse, assign_period, build, turn_count_csv, CsvOpts};
use edit_support::{Checker, load_json};
use sample_support::{Fakes, exchanges};
use serde_json::{Value, json};
use std::cell::Cell;

fn res_summary(p: &Project) -> Value {
    Value::Array(
        p.scenarios
            .iter()
            .map(|s| {
                let mut m = serde_json::Map::new();
                if let Maybe::Set(Value::Object(res)) = &s.res {
                    for (k, r) in res {
                        m.insert(
                            k.clone(),
                            if r.is_null() { Value::Null } else { json!({ "period": r["period"], "it": r["it"], "gap": r["gap"], "converged": r["converged"], "stale": r["stale"], "kpi": r["kpi"] }) },
                        );
                    }
                }
                json!({ "id": s.id, "res": Value::Object(m) })
            })
            .collect(),
    )
}

fn state_of(p: &Project) -> Value {
    let mut v = p.to_value();
    for sc in v["scenarios"].as_array_mut().unwrap() {
        sc.as_object_mut().unwrap().remove("res");
    }
    if let Some(s) = v.get_mut("sample").and_then(Value::as_object_mut) {
        s.remove("size");
    }
    v
}

fn size_of(p: &Project) -> Option<f64> {
    p.extra.get("sample").and_then(|s| s.get("size")).and_then(Value::as_f64)
}

fn check_size(ck: &mut Checker, label: &str, got: Option<f64>, want: &Value) {
    match (got, want.as_f64()) {
        (Some(g), Some(w)) => ck.same(&format!("{label}: size within 1%"), &((g - w).abs() / w < 0.01), &true),
        (None, None) => {}
        (g, w) => ck.same(&format!("{label}: size"), &g, &w),
    }
}

#[test]
fn the_drury_sample_is_built_and_analysed_as_the_prototype_does() {
    construct_transport::util::set_clock(|| 1_792_540_800_000.0);
    let ex = load_json("expected/sample-build.json");
    let table = exchanges();
    let mut ck = Checker::default();
    for case in ex.as_array().expect("cases") {
        let name = case["name"].as_str().unwrap();
        let mut ids = ids_for(case);
        let mut net = Replay::new(&case["log"], &table);
        let mut dl = Downloads::default();
        let mut hooks = Fakes::new(case);
        let cancel_on: Option<String> = case["cancelOn"].as_str().map(str::to_string);
        let cancelled = Cell::new(false);
        let mut status: Vec<Value> = Vec::new();
        let cancel_fn = || cancelled.get();
        // without the feature scripts the prototype still has the existing land use and calibration engine, which are core scripts
        let modules = if case["features"].as_bool().unwrap() { Modules::default() } else { Modules { elu: true, cb: true, ..Modules::none() } };
        let o = &case["opts"];
        let opts = BuildOpts { gtfs: o.get("gtfs").and_then(Value::as_bool), run: o.get("run").and_then(Value::as_bool) == Some(true), extras: o.get("extras").and_then(Value::as_bool), modules, cancel: Some(&cancel_fn) };
        let mut on_status = |t: &str| {
            status.push(json!(t));
            if cancel_on.as_deref().is_some_and(|c| t.contains(c)) {
                cancelled.set(true);
            }
        };
        let r = build(&mut ids, &mut net, &mut dl, &mut hooks, &opts, &mut on_status);
        let status_len_build = status.len();
        ck.same(&format!("{name}: requests"), &net.used(), &case["log"].as_array().unwrap().len());
        match (&r, case.get("error")) {
            (Err(e), Some(want)) => {
                ck.check(&format!("{name}: error"), &json!({ "message": e.message(), "stage": match e { SampleError::Failed { stage, .. } => json!(stage), _ => Value::Null }, "abort": e.is_cancel() }), want);
                ck.check(&format!("{name}: progress"), &Value::Array(status.clone()), &case["status"]);
                continue;
            }
            (Err(e), None) => {
                ck.same(&format!("{name}: no error"), &e.message(), &String::new());
                continue;
            }
            (Ok(_), Some(want)) => {
                ck.same(&format!("{name}: expected the error"), &Value::Null, want);
                continue;
            }
            (Ok(_), None) => {}
        }
        let mut b = r.ok().unwrap();
        ck.check(&format!("{name}: notes"), &json!(b.notes), &case["notes"]);
        ck.check(&format!("{name}: failed"), &Value::Array(b.failed.iter().map(|f| f.to_value()).collect()), &case["failed"]);
        ck.check(&format!("{name}: ms"), &json!(b.ms), &case["ms"]);
        let sv = &case["scenarios"];
        ck.check(&format!("{name}: scenarios"), &json!({ "s0": b.scenarios.s0, "s1": b.scenarios.s1, "s2": b.scenarios.s2, "s3": b.scenarios.s3 }), &json!({ "s0": sv["S0"], "s1": sv["S1"], "s2": sv["S2"], "s3": sv["S3"] }));
        ck.check(&format!("{name}: stand-in calls"), &Value::Array(hooks.calls.clone()), &case["calls"]);
        ck.same(&format!("{name}: every junction answer used"), &hooks.approaches_done(), &true);
        ck.same(&format!("{name}: every request made"), &net.done(), &true);
        ck.check(&format!("{name}: progress"), &Value::Array(status[..status_len_build].to_vec()), &json!(case["status"].as_array().unwrap()[..status_len_build]));
        ck.check(&format!("{name}: results"), &res_summary(&b.project), &case["res"]);
        ck.check(&format!("{name}: project"), &state_of(&b.project), &case["after"]);
        check_size(&mut ck, name, size_of(&b.project), &case["size"]);
        if let Some(t) = case.get("turnCsv") {
            let s0 = b.project.scenarios.iter().find(|s| s.role.is("existing")).unwrap();
            for (sid, want) in t.as_object().unwrap() {
                if sid == "named" {
                    continue;
                }
                let site = s0.sites.get().unwrap().get(sid).unwrap();
                let got = turn_count_csv(s0, site, &CsvOpts::default());
                ck.check(&format!("{name}: turning count {sid}"), &json!({ "csv": got.csv, "legs": got.legs.iter().cloned().collect::<std::collections::BTreeMap<_, _>>(), "centre": got.centre, "name": got.name }), want);
            }
            let first = s0.sites.get().unwrap().values().next().unwrap();
            let got = turn_count_csv(s0, first, &CsvOpts { name: Some("Named junction".into()), date: Some("2026-04-01".into()) });
            ck.check(&format!("{name}: named turning count"), &json!({ "csv": got.csv, "legs": got.legs.iter().cloned().collect::<std::collections::BTreeMap<_, _>>(), "centre": got.centre, "name": got.name }), &t["named"][0]);
        }
        if let Some(a) = case.get("analyse") {
            let ao = &a["opts"];
            let cancel_on2: Option<String> = ao["cancelOn"].as_str().map(str::to_string);
            let mut notes = b.notes.clone();
            let mut st2 = |t: &str| {
                status.push(json!(t));
                if cancel_on2.as_deref().is_some_and(|c| t.contains(c)) {
                    cancelled.set(true);
                }
            };
            hooks.calls.clear();
            let mut own = |p: &mut Project, scs: &[usize], _s: &mut dyn FnMut(&str)| -> Result<(), SampleError> {
                for &si in scs {
                    for period in PERIODS {
                        assign_period(p, si, period, None);
                    }
                }
                Ok(())
            };
            let mut aopts = AnalyseOpts { extras: ao["extras"].as_bool(), modules, cancel: Some(&cancel_fn), runs: None };
            if ao["ownRuns"].as_bool() == Some(true) {
                aopts.runs = Some(&mut own);
            }
            let out = analyse(&mut b.project, &mut ids, &mut hooks, &mut aopts, &mut notes, &mut st2);
            let label = format!("{name} (analyse)");
            match (&out, a.get("error")) {
                (Ok(o), None) => {
                    ck.check(&format!("{label}: ms"), &json!(o.ms), &a["out"]["ms"]);
                    check_size(&mut ck, &label, Some(o.size), &a["out"]["size"]);
                }
                (Err(e), Some(want)) => ck.check(&format!("{label}: error"), &json!({ "message": e.message(), "abort": e.is_cancel() }), want),
                (Ok(_), Some(_)) => ck.same(&format!("{label}: expected an error"), &false, &true),
                (Err(e), None) => ck.same(&format!("{label}: no error"), &e.message(), &String::new()),
            }
            ck.check(&format!("{label}: notes"), &json!(notes), &a["notes"]);
            ck.check(&format!("{label}: stand-in calls"), &Value::Array(hooks.calls.clone()), &a["calls"]);
            ck.check(&format!("{label}: progress"), &Value::Array(status[status_len_build..].to_vec()), &json!(case["status"].as_array().unwrap()[a["statusFrom"].as_u64().unwrap() as usize..]));
            ck.check(&format!("{label}: results"), &res_summary(&b.project), &a["res"]);
            ck.check(&format!("{label}: project"), &state_of(&b.project), &a["after"]);
            check_size(&mut ck, &label, size_of(&b.project), &a["size"]);
        }
    }
    ck.finish("Drury sample builds");
}
