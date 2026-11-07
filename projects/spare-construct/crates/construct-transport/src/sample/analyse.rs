//! The model runs, matrix estimation and report tests on a built sample (`Sample.analyse`).

use super::hooks::SampleHooks;
use super::{MAX_SIZE, Modules, PERIODS, SENS_TESTS, STAGING_STEPS, SampleError};
use crate::assign::assign;
use crate::calibrate::{
    Ctx, EvOpts, RunMeOpts, assess, keep_site_traces, match_sites, refresh_validation, run_me,
    site_traces, stored_res,
};
use crate::edit::IdSource;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, json_number, number_to_js_string, value_to_number, value_truthy};
use crate::project::Project;
use crate::results::AssignOpts;
use serde_json::{Map, Value, json};
use std::cell::Cell;
use std::collections::HashMap;

/// What `analyse` is asked to do.
pub struct AnalyseOpts<'a> {
    /// False leaves out the report's staging and sensitivity tests (`opts.extras === false`).
    pub extras: Option<bool>,
    /// Which optional modules are loaded.
    pub modules: Modules,
    /// Says true when the work is cancelled.
    pub cancel: Option<&'a dyn Fn() -> bool>,
    /// Runs AM and PM for the scenarios (indices into the project's scenarios); the default runs the engine on each (`opts.runs`).
    #[allow(clippy::type_complexity)]
    pub runs: Option<
        &'a mut dyn FnMut(&mut Project, &[usize], &mut dyn FnMut(&str)) -> Result<(), SampleError>,
    >,
}

/// What `analyse` gave back (the notes are added to the list it was given).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalyseOut {
    /// How long it took (ms).
    pub ms: f64,
    /// The saved size of the project (characters).
    pub size: f64,
}

fn stop(cancel: Option<&dyn Fn() -> bool>) -> Result<(), SampleError> {
    if cancel.is_some_and(|c| c()) {
        Err(SampleError::Cancelled)
    } else {
        Ok(())
    }
}

/// `Engine.assign` for one scenario and period, kept as the scenario's result; gives the result.
pub fn assign_period(
    p: &mut Project,
    idx: usize,
    period: &str,
    progress: Option<&mut dyn FnMut(f64, f64, &str)>,
) -> crate::results::Res {
    let opts = AssignOpts {
        period: Some(period.to_string()),
        ..Default::default()
    };
    let s = p.settings.clone();
    let mut adapt = progress;
    let mut cb = |it: u32, gap: f64, msg: &str| {
        if let Some(f) = adapt.as_deref_mut() {
            f(f64::from(it), gap, msg);
        }
    };
    let res = assign(p, &p.scenarios[idx], &s, &opts, Some(&mut cb));
    if !matches!(p.scenarios[idx].res, Maybe::Set(Value::Object(_))) {
        p.scenarios[idx].res = Maybe::Set(Value::Object(Map::new()));
    }
    if let Maybe::Set(Value::Object(m)) = &mut p.scenarios[idx].res {
        m.insert(
            period.to_string(),
            serde_json::to_value(&res).expect("a result serialises"),
        );
    }
    res
}

#[allow(clippy::too_many_arguments)]
fn engine_runs(
    p: &mut Project,
    scs: &[usize],
    status: &mut dyn FnMut(&str),
    cancel: Option<&dyn Fn() -> bool>,
    m: Modules,
    hooks: &mut dyn SampleHooks,
    ids: &dyn IdSource,
) -> Result<(), SampleError> {
    for &si in scs {
        for period in PERIODS {
            stop(cancel)?;
            let name = p.scenarios[si].name.str_or("").to_string();
            status(&format!("{name} \u{b7} {period} peak\u{2026}"));
            let aborted = Cell::new(false);
            let old = match &p.scenarios[si].res {
                Maybe::Set(Value::Object(r)) => r.get(period).cloned(),
                _ => None,
            };
            let res = {
                let mut prog = |it: f64, _gap: f64, msg: &str| {
                    if cancel.is_some_and(|c| c()) {
                        aborted.set(true);
                        return;
                    }
                    if it % 20.0 == 0.0 {
                        let t = if msg.is_empty() {
                            format!("iteration {}", number_to_js_string(it))
                        } else {
                            msg.to_string()
                        };
                        status(&format!("{name} \u{b7} {period} peak \u{b7} {t}"));
                    }
                };
                assign_period(p, si, period, Some(&mut prog))
            };
            if aborted.get() {
                if let Maybe::Set(Value::Object(r)) = &mut p.scenarios[si].res {
                    match old {
                        Some(v) => r.insert(period.to_string(), v),
                        None => r.remove(period),
                    };
                }
                return Err(SampleError::Cancelled);
            }
            // the results listeners: the calibration feature keeps the traced site movements and re-records validation; the others are the host's
            if m.cal {
                let s = p.settings.clone();
                if let Some(t) = site_traces(&p.scenarios[si], Some(&res), &s) {
                    keep_site_traces(&mut p.scenarios[si], &res, &t);
                }
                refresh_validation(p, &ids.now_iso());
            }
            hooks.on_results(p, si, period);
        }
    }
    Ok(())
}

/// `Sample.analyse(P, opts)`: the runs, hold-out, matrix estimation and report tests on a built project. Notes are added to `notes`.
pub fn analyse(
    p: &mut Project,
    ids: &mut dyn IdSource,
    hooks: &mut dyn SampleHooks,
    opts: &mut AnalyseOpts,
    notes: &mut Vec<String>,
    status: &mut dyn FnMut(&str),
) -> Result<AnalyseOut, SampleError> {
    let cancel = opts.cancel;
    let m = opts.modules;
    let t0 = ids.now_ms();
    let role = |p: &Project, r: &str| p.scenarios.iter().position(|s| s.role.is(r));
    let Some(i0) = role(p, "existing") else {
        return Err(SampleError::msg(
            "Cannot read properties of undefined (reading 'name')",
        ));
    };
    let rest: Vec<usize> = ["baseline", "proposal", "mitigation"]
        .iter()
        .filter_map(|r| role(p, r))
        .collect();
    let s0 = p.scenarios[i0].id.clone();
    let mut run = |p: &mut Project,
                   scs: &[usize],
                   status: &mut dyn FnMut(&str),
                   hooks: &mut dyn SampleHooks,
                   ids: &dyn IdSource|
     -> Result<(), SampleError> {
        match opts.runs.as_deref_mut() {
            Some(f) => f(p, scs, status),
            None => engine_runs(p, scs, status, cancel, m, hooks, ids),
        }
    };
    // base year first: matrix estimation needs it, and its factors then apply to every scenario
    run(p, &[i0], status, hooks, &*ids)?;
    // Matrix estimation on the calibration counts only: a first fit that makes no change is tried once more with smaller steps (XAMAX 1.5). The outcome of every
    // attempt is kept in P.sample.me.
    super::helpers::object_at(&mut p.extra, "sample")
        .insert("me".into(), Value::Object(Map::new()));
    if m.cal && m.cb {
        for period in PERIODS {
            let mut tries: Vec<(f64, bool, String)> = Vec::new();
            for xamax in [2.0, 1.5] {
                stop(cancel)?;
                status(&format!(
                    "Fitting the {period} peak to the calibration counts{}\u{2026}",
                    if xamax < 2.0 { " (smaller steps)" } else { "" }
                ));
                let aborted = Cell::new(false);
                let date = ids.now_iso();
                // a cancel ends the estimation before it changes anything (the prototype's first progress call throws), so the project is put back
                let snap = if cancel.is_some() {
                    Some(p.clone())
                } else {
                    None
                };
                let r = {
                    let mut prog = |_k: f64, _n: f64, msg: &str| {
                        if cancel.is_some_and(|c| c()) {
                            aborted.set(true);
                            return;
                        }
                        status(&format!("Matrix estimation, {period} peak: {msg}"));
                    };
                    run_me(
                        p,
                        &s0,
                        &RunMeOpts {
                            period: Some(period),
                            xamax: Some(xamax),
                            date: Some(&date),
                        },
                        Some(&mut prog),
                    )
                };
                if aborted.get() {
                    if let Some(s) = snap {
                        *p = s;
                    }
                    return Err(SampleError::Cancelled);
                }
                let (ok, note) = match r {
                    Ok(r) => (r.ok, r.note),
                    Err(e) => (false, format!("failed: {e}")),
                };
                let shown = if !note.is_empty() {
                    note.clone()
                } else if ok {
                    String::new()
                } else {
                    "no calibration counts".to_string()
                };
                tries.push((xamax, ok, shown.trim_end_matches('.').to_string()));
                if ok || !note.to_lowercase().contains("no pass improved") {
                    break;
                }
            }
            let last = tries.last().cloned().expect("a try");
            let info =
                p.me.get()
                    .and_then(|me| me.info.get())
                    .and_then(|i| i.get(period))
                    .is_some_and(|i| i.is_set());
            let entry = json!({
                "at": ids.now_iso(), "ok": last.1,
                "tries": tries.iter().map(|(x, ok, note)| json!({ "xamax": json_number(*x), "ok": ok, "note": note })).collect::<Vec<_>>(),
                "info": info,
            });
            super::helpers::object_at(super::helpers::object_at(&mut p.extra, "sample"), "me")
                .insert(period.to_string(), entry);
            notes.push(format!(
                "{period} peak: {}.{}",
                last.2,
                if last.1 {
                    String::new()
                } else {
                    format!(
                        " The {period} model uses the prior matrix{}.",
                        if tries.len() > 1 {
                            " (smaller steps were tried too)"
                        } else {
                            ""
                        }
                    )
                }
            ));
        }
    }
    if m.cal {
        refresh_validation(p, &ids.now_iso());
    }
    run(p, &rest, status, hooks, &*ids)?;
    if m.cal {
        match_sites(p, &s0);
        refresh_validation(p, &ids.now_iso());
        // the validation line the report prints: independent (held-out) counts only, TMDG Category E
        for period in PERIODS {
            let line = {
                let ctx = Ctx::new(p);
                let sc = &p.scenarios[i0];
                let res = stored_res(sc, period);
                assess(
                    &ctx,
                    sc,
                    period,
                    res.as_ref(),
                    &EvOpts {
                        set: Some("valid".into()),
                        area: Some("auto".into()),
                        standard: Some("tmdg".into()),
                        ..EvOpts::default()
                    },
                )
                .line
            };
            notes.push(format!("Validation ({period}, held-out counts): {line}"));
        }
    }
    // the mitigation register and condition parameters, seeded from the modelled effects and the mitigation scenario
    if m.cx {
        let r = (|| -> Result<(), SampleError> {
            hooks.seed_mitigation(p)?;
            let list = hooks.mitigation(p);
            let eff: HashMap<String, Value> = hooks
                .effects(p)?
                .into_iter()
                .filter_map(|e| {
                    e.get("id")
                        .map(|i| (crate::jsnum::value_to_js_string(i), e.clone()))
                })
                .collect();
            let size = |x: &Value| -> f64 {
                let e = x
                    .get("effect")
                    .and_then(|i| eff.get(&crate::jsnum::value_to_js_string(i)));
                match e {
                    Some(e) => {
                        let pick = |k: &str| e.get(k).filter(|v| value_truthy(v));
                        pick("dos")
                            .or_else(|| pick("vc"))
                            .map_or(0.0, value_to_number)
                    }
                    None => 0.0,
                }
            };
            let seed = |x: &Value| {
                x.get("seed")
                    .filter(|s| value_truthy(s))
                    .map(crate::jsnum::value_to_js_string)
                    .unwrap_or_default()
            };
            let measures: Vec<Value> = list
                .iter()
                .filter(|x| seed(x).starts_with("diff:"))
                .cloned()
                .collect();
            let mut open: Vec<Value> = list
                .iter()
                .filter(|x| seed(x).starts_with("eff:"))
                .cloned()
                .collect();
            js_sort_by(&mut open, |a, b| size(b) - size(a));
            let others: Vec<Value> = list
                .iter()
                .filter(|x| !(seed(x).starts_with("diff:") || seed(x).starts_with("eff:")))
                .cloned()
                .collect();
            let mut out = measures;
            out.extend(open.iter().take(10).cloned());
            out.extend(others);
            hooks.set_mitigation(p, out);
            if open.len() > 10 {
                notes.push(format!("The mitigation register lists the ten largest of {} modelled effects not yet addressed; the Compliance+ tab lists them all.", open.len()));
            }
            hooks.seed_conditions(p)
        })();
        if let Err(e) = r {
            notes.push(format!(
                "The mitigation register could not be seeded: {}",
                e.message()
            ));
        }
    }
    if opts.extras != Some(false) && m.rp {
        stop(cancel)?;
        status("Staging test: the proposal at 0, 50 and 100% build-out\u{2026}");
        let r = hooks.staging(p, &STAGING_STEPS, &mut |k, n, t| {
            stop(cancel)?;
            status(&format!("Staging test: {t} ({k} of {n})"));
            Ok(())
        });
        match r {
            Err(SampleError::Cancelled) => return Err(SampleError::Cancelled),
            Err(e) => notes.push(format!("The staging test did not run: {}", e.message())),
            Ok(()) => {}
        }
        stop(cancel)?;
        status("Sensitivity tests: trip rates +10% and background growth +50%\u{2026}");
        let r = hooks.sensitivity(p, &SENS_TESTS, &mut |k, n, t| {
            stop(cancel)?;
            status(&format!("Sensitivity tests: {t} ({k} of {n})"));
            Ok(())
        });
        match r {
            Err(SampleError::Cancelled) => return Err(SampleError::Cancelled),
            Err(e) => notes.push(format!(
                "The sensitivity tests did not run: {}",
                e.message()
            )),
            Ok(()) => {}
        }
    }
    if m.co {
        hooks.record_outputs(p);
    }
    // the saved size (Store.serialise): the sample is built to stay under 3 MB; say so if a download made it larger
    let size = p.serialise().encode_utf16().count() as f64;
    super::helpers::object_at(&mut p.extra, "sample").insert("size".into(), json_number(size));
    if size > MAX_SIZE {
        notes.push(format!(
            "The saved project is {} MB, more than the sample's {} MB target. Save it to a file if browser storage refuses it.",
            crate::util::fmt::num(size / 1e6, 2),
            crate::util::fmt::num(MAX_SIZE / 1e6, 0)
        ));
    }
    Ok(AnalyseOut {
        ms: ids.now_ms() - t0,
        size,
    })
}
