//! Results queries, appraisal and assessment on inputs the two projects do not reach, against the
//! JavaScript's answers in expected/analysis-cases.json (gen-analysis-cases.mjs): discount factors,
//! analysis periods, IPM bands, BCR text, Future Connect classes and the extent thresholds, signal
//! to noise, export readiness, travel and isochrones on the graph port's graph, cycling stress,
//! capital, economics under other settings, model status
//! and metadata, the MSM check, and the extent test, difference plots, with / without and
//! development distribution with options.
//!
//! A case names its project; results are { scenario, period } in expected/<project>.json with an
//! optional patch, and settings are the project's with a patch (see the generator).

mod recorded;
mod support;

use construct_transport::analysis::{
    DiffOpts, ExtentOpts, FcOf, dev_distribution, difference, extent_class, extent_test, fc_of,
    msm_check, with_without,
};
use construct_transport::econ::{
    agpwf, bcr_text, capital, econ, econ_period, ipm_band, pwf, snr, uspwf,
};
use construct_transport::jsnum::{Maybe, value_to_number};
use construct_transport::project::{
    EconSettings, Link, MatrixEstimation, OrderedMap, Project, ProjectData, Scenario, Settings,
    ValidationRecord,
};
use construct_transport::results::{
    Convergence, Res, Results, export_ready, isochrone, lts, model_meta, model_status, select_link,
    travel,
};
use recorded::{Recorded, json};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use support::{assert_close, load_project, read_truth};

#[derive(Deserialize)]
#[serde(untagged)]
enum ScenarioArg {
    Id(String),
    Full(Box<Scenario>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    #[serde(rename = "fn")]
    func: String,
    project: Option<String>,
    settings_patch: Option<Value>,
    scenario: Option<ScenarioArg>,
    i: Option<f64>,
    n: Option<f64>,
    rates: Option<Value>,
    #[serde(rename = "E")]
    e: Option<Value>,
    bcr: Option<f64>,
    edition: Option<String>,
    x: Option<f64>,
    link: Option<Value>,
    link_patch: Option<Value>,
    decade: Option<bool>,
    fc: Option<Value>,
    b: Option<Value>,
    o: Option<Value>,
    res: Option<Value>,
    base: Option<Value>,
    opt: Option<Value>,
    without: Option<Value>,
    with: Option<Value>,
    fut: Option<Value>,
    c: Option<Value>,
    period: Option<String>,
    results: Option<Value>,
    me: Option<Value>,
    validation: Option<Value>,
    data: Option<Value>,
    msm: Option<Value>,
    pct: Option<f64>,
    opts: Option<Value>,
    drop_first: Option<bool>,
    sc_without: Option<String>,
    named: Option<bool>,
    list: Option<Vec<String>>,
    key: Option<String>,
    from: Option<String>,
    to: Option<String>,
    out: Value,
}

#[derive(Deserialize)]
struct CasesFile {
    cases: Vec<Case>,
}

struct Ctx {
    projects: HashMap<String, (Project, Recorded)>,
}

impl Ctx {
    fn project(&mut self, name: &str) -> &(Project, Recorded) {
        self.projects
            .entry(name.to_string())
            .or_insert_with(|| (load_project(name), recorded::load(name)))
    }
}

// The project's settings with the case's patch: top-level keys replaced; econ keys replaced, null removes one
fn patched_settings(base: &Settings, patch: Option<&Value>) -> Settings {
    let mut v = serde_json::to_value(base).unwrap();
    if let Some(Value::Object(p)) = patch {
        for (k, x) in p {
            if k != "econ" {
                v[k] = x.clone();
                continue;
            }
            let e = v["econ"].as_object_mut().expect("an econ block");
            for (ek, ev) in x.as_object().expect("an econ patch") {
                if ev.is_null() {
                    e.remove(ek);
                } else {
                    e.insert(ek.clone(), ev.clone());
                }
            }
        }
    }
    serde_json::from_value(v).expect("patched settings")
}

// A recorded result with the case's patch of top-level numbers
fn patch_res(mut r: Res, patch: Option<&Value>) -> Res {
    let Some(Value::Object(p)) = patch else {
        return r;
    };
    let num = |v: &Value| match v {
        Value::String(s) if s == "NaN" => f64::NAN,
        v => v.as_f64().expect("a number"),
    };
    for (k, v) in p {
        match k.as_str() {
            "year" => r.year = num(v),
            "gap" => r.gap = num(v),
            "gapTol" => r.gap_tol = num(v),
            "noise95" => r.noise95 = if v.is_null() { None } else { Some(num(v)) },
            "kpi.vht" => r.kpi.vht = num(v),
            "kpi.co2" => r.kpi.co2 = num(v),
            "paths" => {
                assert_eq!(v, "missing");
                r.paths = Maybe::Missing;
            }
            other => panic!("patch of {other}"),
        }
    }
    r
}

// { scenario, period, patch } in the recorded results, or None for null
fn res_ref(rec: &Recorded, v: Option<&Value>) -> Option<Res> {
    let v = v.filter(|v| !v.is_null())?;
    let sid = v["scenario"].as_str().unwrap();
    let period = v["period"].as_str().unwrap();
    let r = rec.scenarios[sid].periods[period].res.clone();
    Some(patch_res(r, v.get("patch")))
}

fn loose(v: Option<&Value>) -> Option<f64> {
    v.filter(|x| !x.is_null()).map(value_to_number)
}

fn run(case: &Case, ctx: &mut Ctx) -> Value {
    let pure = |c: &Case| -> Option<Value> {
        Some(match c.func.as_str() {
            "uspwf" => json!(uspwf(c.i.unwrap(), c.n.unwrap())),
            "agpwf" => json!(agpwf(c.i.unwrap(), c.n.unwrap())),
            "pwf" => json(&pwf(c.rates.as_ref().unwrap(), c.n.unwrap())),
            "econPeriod" => {
                let e: EconSettings = serde_json::from_value(c.e.clone().unwrap()).unwrap();
                json(&econ_period(&e))
            }
            "ipmBand" => json(&ipm_band(c.bcr, c.edition.as_deref())),
            "bcrText" => json!(bcr_text(c.x)),
            "fcOf" => {
                let l: Link = serde_json::from_value(c.link.clone().unwrap()).unwrap();
                json(&fc_of(&l, c.decade.unwrap()))
            }
            "extentClass" => {
                let f = c.fc.as_ref().unwrap();
                let fc = FcOf {
                    gt: f["gt"].clone(),
                    pt: f["pt"].clone(),
                    freight: f["freight"].clone(),
                    basis: Value::Null,
                };
                json(&extent_class(&fc))
            }
            _ => return None,
        })
    };
    if let Some(v) = pure(case) {
        return v;
    }
    let (p, rec) = ctx.project(case.project.as_deref().expect("a project"));
    let s = patched_settings(&p.settings, case.settings_patch.as_ref());
    let scenario_by_id = |id: &str| -> &Scenario { p.scenario(id).expect("scenario") };
    match case.func.as_str() {
        "snr" => {
            let b = res_ref(rec, case.b.as_ref());
            let o = res_ref(rec, case.o.as_ref());
            json(&snr(b.as_ref(), o.as_ref(), &s))
        }
        "exportReady" => json(&export_ready(res_ref(rec, case.res.as_ref()).as_ref())),
        "lts" => {
            let Some(ScenarioArg::Id(id)) = &case.scenario else {
                panic!("scenario id")
            };
            let l0 = scenario_by_id(id).links.values().next().unwrap();
            let mut v = serde_json::to_value(l0).unwrap();
            for (k, x) in case.link_patch.as_ref().unwrap().as_object().unwrap() {
                v[k] = x.clone();
            }
            let l: Link = serde_json::from_value(v).unwrap();
            json!(lts(&l, res_ref(rec, case.res.as_ref()).as_ref(), &s))
        }
        "capital" => {
            let Some(ScenarioArg::Full(sc)) = &case.scenario else {
                panic!("full scenario")
            };
            json(&capital(sc, &s))
        }
        "econ" => {
            let base = case.base.as_ref().unwrap().as_str().unwrap();
            let opt = case.opt.as_ref().unwrap().as_str().unwrap();
            let results_of = |sid: &str| -> Results {
                let changes = case.results.as_ref().and_then(|r| r.get(sid));
                let mut out = OrderedMap::new();
                for period in ["AM", "PM"] {
                    let change = changes.and_then(|c| c.get(period));
                    if change.is_some_and(Value::is_null) {
                        continue;
                    }
                    let r = rec.scenarios[sid].periods[period].res.clone();
                    out.insert(period, patch_res(r, change));
                }
                out
            };
            let (rb, ro) = (results_of(base), results_of(opt));
            let period = case.period.as_deref().unwrap();
            json(&econ(
                &s,
                scenario_by_id(base),
                &rb,
                scenario_by_id(opt),
                &ro,
                period,
            ))
        }
        "modelStatus" => {
            let mut q = p.clone();
            q.settings = s;
            q.me = match &case.me {
                Some(v) if !v.is_null() => {
                    Maybe::Set(serde_json::from_value::<MatrixEstimation>(v.clone()).unwrap())
                }
                _ => Maybe::Missing,
            };
            q.validation = match &case.validation {
                Some(v) if !v.is_null() => Maybe::Set(
                    serde_json::from_value::<OrderedMap<Maybe<ValidationRecord>>>(v.clone())
                        .unwrap(),
                ),
                _ => Maybe::Missing,
            };
            json(&model_status(&q, case.period.as_deref().unwrap()))
        }
        "modelMeta" => {
            let mut q = p.clone();
            q.data = Maybe::Set(
                serde_json::from_value::<ProjectData>(case.data.clone().unwrap()).unwrap(),
            );
            let Some(ScenarioArg::Id(id)) = &case.scenario else {
                panic!("scenario id")
            };
            let r = res_ref(rec, case.c.as_ref()).unwrap();
            let conv = Convergence {
                converged: r.converged,
                hit_max: r.hit_max,
                gap: r.gap,
                gap_tol: r.gap_tol,
                it: r.it,
                note: r.conv_note.clone(),
            };
            json(&model_meta(
                &q,
                Some(scenario_by_id(id)),
                case.period.as_deref().unwrap(),
                &s,
                conv,
            ))
        }
        "msmCheck" => {
            let b = res_ref(rec, case.base.as_ref());
            let f = res_ref(rec, case.fut.as_ref());
            json(&msm_check(
                b.as_ref(),
                f.as_ref(),
                case.msm.as_ref().filter(|m| !m.is_null()),
                case.pct,
            ))
        }
        "extentTest" => {
            let Some(ScenarioArg::Id(id)) = &case.scenario else {
                panic!("scenario id")
            };
            let mut sc = scenario_by_id(id).clone();
            let fcs = case.fc.as_ref().unwrap().as_array().unwrap();
            let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
            for (k, f) in fcs.iter().enumerate() {
                if let Some(l) = ids.get(k).and_then(|lid| sc.links.get_mut(lid)) {
                    l.fc = Maybe::Set(f.clone());
                }
            }
            if case.drop_first == Some(true) {
                sc.links.remove(&ids[0]);
            }
            let opts = case.opts.as_ref().unwrap();
            let sw = case.sc_without.as_deref().map(scenario_by_id);
            let o = ExtentOpts {
                floor: loose(opts.get("floor")),
                decade: opts.get("decade").and_then(Value::as_str),
                sc_without: sw,
            };
            let w0 = res_ref(rec, case.without.as_ref());
            let w1 = res_ref(rec, case.with.as_ref());
            json(&extent_test(&sc, w0.as_ref(), w1.as_ref(), o))
        }
        "difference" => {
            let b = res_ref(rec, case.base.as_ref());
            let q = res_ref(rec, case.opt.as_ref());
            let o = case.o.as_ref().unwrap();
            json(&difference(
                b.as_ref(),
                q.as_ref(),
                DiffOpts {
                    k: loose(o.get("k")),
                    zoom: loose(o.get("zoom")),
                },
            ))
        }
        "withWithout" => {
            let a = res_ref(rec, case.without.as_ref());
            let b = res_ref(rec, case.with.as_ref());
            let name = |id: &str| format!("J {}", id.to_uppercase());
            let named: Option<&dyn Fn(&str) -> String> = if case.named == Some(true) {
                Some(&name)
            } else {
                None
            };
            json(&with_without(a.as_ref(), b.as_ref(), named))
        }
        "devDistribution" => {
            let sc = match &case.scenario {
                Some(ScenarioArg::Id(id)) => Some(scenario_by_id(id)),
                _ => None,
            };
            let sid = match &case.scenario {
                Some(ScenarioArg::Id(id)) => id.as_str(),
                _ => "s-opt",
            };
            let list: Vec<&Res> = case
                .list
                .as_ref()
                .unwrap()
                .iter()
                .map(|per| &rec.scenarios[sid].periods[per.as_str()].res)
                .collect();
            json(&dev_distribution(sc, &list))
        }
        "travel" | "isochrone" => {
            let Some(ScenarioArg::Id(id)) = &case.scenario else {
                panic!("scenario id")
            };
            let r = res_ref(rec, case.res.as_ref());
            let from = case.from.as_deref().unwrap();
            if case.func == "travel" {
                let to = case.to.as_deref().unwrap();
                json(&travel(
                    scenario_by_id(id),
                    &p.settings,
                    r.as_ref(),
                    from,
                    to,
                ))
            } else {
                json(&isochrone(
                    scenario_by_id(id),
                    &p.settings,
                    r.as_ref(),
                    from,
                ))
            }
        }
        "selectLink" => json(&select_link(
            res_ref(rec, case.res.as_ref()).as_ref(),
            case.key.as_deref().unwrap(),
        )),
        other => panic!("no case runner for {other}"),
    }
}

#[test]
fn analysis_cases_as_recorded() {
    let file: CasesFile =
        serde_json::from_str(&read_truth("expected/analysis-cases.json")).expect("cases");
    let mut ctx = Ctx {
        projects: HashMap::new(),
    };
    let mut counts: OrderedMap<usize> = OrderedMap::new();
    for (k, case) in file.cases.iter().enumerate() {
        let got = run(case, &mut ctx);
        let path = format!(
            "case[{k}] {}{}",
            case.func,
            case.project
                .as_deref()
                .map_or(String::new(), |p| format!(" ({p})"))
        );
        assert_close(&got, &case.out, &path);
        let n = counts.get(&case.func).copied().unwrap_or(0);
        counts.insert(case.func.clone(), n + 1);
    }
    assert_eq!(file.cases.len(), 1023, "cases: {counts:?}");
}
