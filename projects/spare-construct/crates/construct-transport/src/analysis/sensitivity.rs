//! The sensitivity pack (engine.js SENS, SENS_DEFAULT, scaleRates, scaleTg, dropUnfunded and
//! sensitivity; docs/spec/sensitivity-pack).
//!
//! Each test runs the with-proposal scenario again on a copy of the scenario and of the
//! settings, changed by the test; nothing is saved to the project. The assignment is not
//! called here: [`sensitivity`] takes it as a closure `FnMut(&Scenario, &Settings,
//! &AssignOpts) -> Res`, so the assignment port plugs in and tests can replay recorded runs.

use super::{ExtentOpts, extent_test};
use crate::econ::{Econ, Snr, econ, snr};
use crate::jsnum::{Maybe, Num, math_max, math_round, number_to_js_string, to_fixed};
use crate::project::{Project, Scenario, Settings, in_baseline, link_programmed, link_unfunded};
use crate::results::{AssignOpts, Res, Results, js_sort_by, plain};
use crate::util::{clamp, r1, r3};
use serde::Serialize;
use serde_json::json;

/// How a sensitivity test changes the settings copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsChange {
    /// Trip rates times the factor (scaleRates).
    ScaleRates(f64),
    /// Gravity β times the factor.
    ScaleGravityBeta(f64),
    /// Background growth rate times the factor.
    ScaleGrowth(f64),
}

/// How a sensitivity test changes the scenario copy (and the without copy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScenarioChange {
    /// Trip-generation table loads times the factor (scaleTg).
    ScaleTg(f64),
    /// Every zone's reduction set to 0%.
    NoReductions,
    /// Unfunded and programmed links, and network projects out of the baseline, removed (dropUnfunded).
    DropUnfunded,
    /// Land-use zones' reduction set to the alternative mode share.
    ModeShare,
}

/// One sensitivity test of engine.js SENS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensTest {
    /// Test id ("T1+").
    pub id: &'static str,
    /// Test group ("T1").
    pub test: &'static str,
    /// Label.
    pub label: &'static str,
    /// The input changed, in words.
    pub input: &'static str,
    /// Relative change of the input (for elasticities), when it has one.
    pub rel: Option<f64>,
    /// Change to the settings.
    pub settings: Option<SettingsChange>,
    /// Change to the scenario.
    pub scenario: Option<ScenarioChange>,
    /// Factor on the whole OD matrix.
    pub df: Option<f64>,
}

const fn test(
    id: &'static str,
    group: &'static str,
    label: &'static str,
    input: &'static str,
    rel: Option<f64>,
) -> SensTest {
    SensTest {
        id,
        test: group,
        label,
        input,
        rel,
        settings: None,
        scenario: None,
        df: None,
    }
}

/// engine.js SENS: T1± trip rates ±10%, T2 PT and active-mode reductions removed, T3± gravity β ±25%, T4± background growth ±50%, T5 unfunded projects removed, D± total demand ±10%, T6 an alternative mode share.
pub const SENS: [SensTest; 11] = [
    SensTest {
        settings: Some(SettingsChange::ScaleRates(1.1)),
        scenario: Some(ScenarioChange::ScaleTg(1.1)),
        ..test(
            "T1+",
            "T1",
            "Trip rates +10%",
            "trip rates \u{d7} 1.10",
            Some(0.10),
        )
    },
    SensTest {
        settings: Some(SettingsChange::ScaleRates(0.9)),
        scenario: Some(ScenarioChange::ScaleTg(0.9)),
        ..test(
            "T1-",
            "T1",
            "Trip rates \u{2212}10%",
            "trip rates \u{d7} 0.90",
            Some(-0.10),
        )
    },
    SensTest {
        scenario: Some(ScenarioChange::NoReductions),
        ..test(
            "T2",
            "T2",
            "PT and active-mode reductions removed",
            "zone reductions set to 0%",
            None,
        )
    },
    SensTest {
        settings: Some(SettingsChange::ScaleGravityBeta(1.25)),
        ..test(
            "T3+",
            "T3",
            "Gravity \u{3b2} +25%",
            "\u{3b2} \u{d7} 1.25",
            Some(0.25),
        )
    },
    SensTest {
        settings: Some(SettingsChange::ScaleGravityBeta(0.75)),
        ..test(
            "T3-",
            "T3",
            "Gravity \u{3b2} \u{2212}25%",
            "\u{3b2} \u{d7} 0.75",
            Some(-0.25),
        )
    },
    SensTest {
        settings: Some(SettingsChange::ScaleGrowth(1.5)),
        ..test(
            "T4+",
            "T4",
            "Background growth +50%",
            "growth rate \u{d7} 1.5",
            Some(0.5),
        )
    },
    SensTest {
        settings: Some(SettingsChange::ScaleGrowth(0.5)),
        ..test(
            "T4-",
            "T4",
            "Background growth \u{2212}50%",
            "growth rate \u{d7} 0.5",
            Some(-0.5),
        )
    },
    SensTest {
        scenario: Some(ScenarioChange::DropUnfunded),
        ..test(
            "T5",
            "T5",
            "Unfunded projects removed",
            "unfunded links and register items out",
            None,
        )
    },
    SensTest {
        df: Some(1.1),
        ..test(
            "D+",
            "D",
            "Total demand +10%",
            "OD matrix \u{d7} 1.10",
            Some(0.10),
        )
    },
    SensTest {
        df: Some(0.9),
        ..test(
            "D-",
            "D",
            "Total demand \u{2212}10%",
            "OD matrix \u{d7} 0.90",
            Some(-0.10),
        )
    },
    SensTest {
        scenario: Some(ScenarioChange::ModeShare),
        ..test("T6", "T6", "Alternative mode share", "mode share", None)
    },
];

/// engine.js SENS_DEFAULT: the tests run when none are named (8 runs per period).
pub const SENS_DEFAULT: [&str; 8] = ["T1+", "T1-", "T2", "T3+", "T3-", "T4+", "T4-", "T5"];

/// `SENS.find(t => t.id === id)`.
pub fn sens_test(id: &str) -> Option<&'static SensTest> {
    SENS.iter().find(|t| t.id == id)
}

/// engine.js scaleRates(S, f): every finite trip rate times f.
pub fn scale_rates(s: &mut Settings, f: f64) {
    let Some(rates) = s.rates.get_mut() else {
        return;
    };
    for r in rates.values_mut() {
        let Some(r) = r.get_mut() else { continue };
        for p in r.values_mut() {
            if let Some(p) = p.get_mut()
                && p.rate.num().is_finite()
            {
                p.rate = Num::number(p.rate.num() * f);
            }
        }
    }
}

/// engine.js scaleTg(sc, f): the loads of zones whose trip-generation table is on, times f (finite values only).
pub fn scale_tg(sc: &mut Scenario, f: f64) {
    for z in sc.zones.values_mut() {
        let on = z.tg.get().is_some_and(|tg| {
            crate::jsnum::value_truthy(tg) && tg.get("on").is_some_and(crate::jsnum::value_truthy)
        });
        if !on {
            continue;
        }
        let Some(load) = z.tg_load.get_mut() else {
            continue;
        };
        for v in load.values_mut() {
            if v.num().is_finite() {
                *v = Num::number(v.num() * f);
            }
        }
    }
}

/// engine.js dropUnfunded(sc, P): removes unfunded and programmed links, and the links of network projects in the register that are not in the baseline.
pub fn drop_unfunded(sc: &mut Scenario, p: Option<&Project>) {
    let reg_links: Vec<String> = p
        .and_then(|p| p.register.get())
        .map(|rows| {
            rows.iter()
                .filter(|r| r.row_type.is("network") && !in_baseline(Some(r)).0)
                .flat_map(|r| r.links.get().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    sc.links.retain(|id, l| {
        !(link_unfunded(l) || link_programmed(l) || reg_links.iter().any(|x| x == id))
    });
}

impl SensTest {
    /// Applies the test's change to a settings copy (`t.S(S)`).
    pub fn apply_settings(&self, s: &mut Settings) {
        match self.settings {
            Some(SettingsChange::ScaleRates(f)) => scale_rates(s, f),
            Some(SettingsChange::ScaleGravityBeta(f)) => {
                s.gravity_beta = Num::number(s.gravity_beta.num() * f)
            }
            Some(SettingsChange::ScaleGrowth(f)) => s.growth = Num::number(s.growth.or(0.0) * f),
            None => {}
        }
    }
    /// Applies the test's change to a scenario copy (`t.sc(sc, P, opts)`); mode_share is `opts.modeShare` (T6).
    pub fn apply_scenario(&self, sc: &mut Scenario, p: Option<&Project>, mode_share: &Num) {
        match self.scenario {
            Some(ScenarioChange::ScaleTg(f)) => scale_tg(sc, f),
            Some(ScenarioChange::NoReductions) => {
                for z in sc.zones.values_mut() {
                    z.reduction = Num::number(0.0);
                }
            }
            Some(ScenarioChange::DropUnfunded) => drop_unfunded(sc, p),
            Some(ScenarioChange::ModeShare) => {
                let v = clamp(mode_share.or(0.0), 0.0, 100.0);
                for z in sc.zones.values_mut() {
                    if !z.kind.is("gateway") {
                        z.reduction = Num::number(v);
                    }
                }
            }
            None => {}
        }
    }
}

/// The econ inputs of the discount-rate row: the base scenario with its results, and the results of the scenario tested.
#[derive(Clone, Copy, Debug)]
pub struct EconBase<'a> {
    /// The base (do-minimum) scenario.
    pub sc: &'a Scenario,
    /// Its results by period.
    pub res: &'a Results,
    /// The results by period of the scenario tested.
    pub opt_res: &'a Results,
}

/// A progress callback: (run, runs, label).
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &str);

/// Options of the sensitivity pack (engine.js opts).
#[derive(Default)]
pub struct SensOpts<'a> {
    /// Periods (None: the scenario's, otherwise AM and PM).
    pub periods: Option<Vec<String>>,
    /// Test ids (None: SENS_DEFAULT).
    pub tests: Option<Vec<String>>,
    /// The do-minimum scenario: adds the links over 5% / 10% (each run is made without too).
    pub without: Option<&'a Scenario>,
    /// Adds the discount-rate row from econ.
    pub econ_base: Option<EconBase<'a>>,
    /// T6: % of vehicle trips replaced.
    pub mode_share: Num,
    /// Names a junction by its id (None: the id).
    pub name: Option<&'a dyn Fn(&str) -> String>,
    /// Gap target for the runs.
    pub gap_tol: Num,
    /// Iteration limit for the runs.
    pub max_iter: Num,
    /// Progress (run, runs, label).
    pub on_progress: Option<Progress<'a>>,
    /// Clock in ms (`performance.now()`), for the run time; None: 0.
    pub now: Option<&'a dyn Fn() -> f64>,
}

/// The junction with the largest DoS over the periods.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SensWorst {
    /// Site or node id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Degree of saturation.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Period.
    pub period: String,
}

/// The central run's totals.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensCentral {
    /// Trips over the periods (veh/h), 0.1.
    #[serde(with = "plain")]
    pub trips: f64,
    /// VHT over the periods, 0.1.
    #[serde(with = "plain")]
    pub vht: f64,
    /// The junction with the largest DoS; null without junctions.
    pub worst: Option<SensWorst>,
    /// Junctions at LoS E or F.
    #[serde(rename = "losEF", with = "plain")]
    pub los_ef: f64,
    /// Links over the 5% test (with a without scenario; null otherwise).
    #[serde(with = "plain")]
    pub over5: Option<f64>,
    /// Links over the 10% test (with a without scenario; null otherwise).
    #[serde(with = "plain")]
    pub over10: Option<f64>,
}

/// Elasticities: (change in KPI / KPI) / (change in input / input).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Elasticity {
    /// Of trips, 0.001.
    #[serde(with = "plain")]
    pub trips: f64,
    /// Of VHT, 0.001.
    #[serde(with = "plain")]
    pub vht: f64,
}

/// One test's row of Table SENS-1.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensTestRow {
    /// Test id.
    pub id: String,
    /// Test group.
    pub test: String,
    /// Label.
    pub label: String,
    /// The input changed, in words.
    pub input: String,
    /// Trips (veh/h), 0.1.
    #[serde(with = "plain")]
    pub trips: f64,
    /// VHT, 0.1.
    #[serde(with = "plain")]
    pub vht: f64,
    /// Change in trips from the central run (fraction), 0.001.
    #[serde(with = "plain")]
    pub d_trips: f64,
    /// Change in VHT from the central run (fraction), 0.001.
    #[serde(with = "plain")]
    pub d_vht: f64,
    /// The junction with the largest DoS.
    pub worst: Option<SensWorst>,
    /// Junctions at LoS E or F.
    #[serde(rename = "losEF", with = "plain")]
    pub los_ef: f64,
    /// Links over the 5% test, or null.
    #[serde(with = "plain")]
    pub over5: Option<f64>,
    /// Links over the 10% test, or null.
    #[serde(with = "plain")]
    pub over10: Option<f64>,
    /// Elasticities, for tests with a relative change.
    pub elasticity: Option<Elasticity>,
    /// A junction crosses its practical DoS or capacity, or changes LoS.
    pub sensitive: bool,
    /// What changed.
    pub changes: Vec<String>,
    /// The least signal to noise against the central run, or null.
    pub noise: Option<Snr>,
    /// The test in a sentence.
    pub sentence: String,
}

/// The discount-rate row (R8) of Table SENS-1.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensDiscountRow {
    /// "R8".
    pub id: String,
    /// "R8".
    pub test: String,
    /// Label.
    pub label: String,
    /// The input, in words.
    pub input: String,
    /// Central BCR.
    #[serde(with = "plain")]
    pub bcr: Option<f64>,
    /// BCR at the sensitivity rate.
    #[serde(with = "plain")]
    pub bcr8: Option<f64>,
    /// Central BCR text.
    pub bcr_text: String,
    /// BCR text at the sensitivity rate.
    pub bcr8_text: String,
    /// The row in a sentence.
    pub sentence: String,
    /// null.
    pub elasticity: Option<Elasticity>,
    /// An IPM band changes at the sensitivity rate.
    pub sensitive: bool,
}

/// A row of Table SENS-1.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SensRow {
    /// A test.
    Test(SensTestRow),
    /// The discount rate.
    Discount(SensDiscountRow),
}

/// What `Engine.sensitivity` returns: Table SENS-1.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SensTable {
    /// "Table SENS-1: sensitivity tests".
    pub label: String,
    /// Periods run.
    pub periods: Vec<String>,
    /// The central run.
    pub central: SensCentral,
    /// One row per test (and the discount-rate row).
    pub rows: Vec<SensRow>,
    /// The tests in sentences.
    pub sentences: Vec<String>,
    /// Conclusions that are sensitive.
    pub flags: Vec<String>,
    /// Runs made.
    #[serde(with = "plain")]
    pub runs: f64,
    /// Run time (ms).
    #[serde(with = "plain")]
    pub ms: f64,
    /// How to read the table.
    pub note: String,
}

/// Why the sensitivity pack could not run (`{ error }`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SensError {
    /// The reason.
    pub error: String,
}

// One test's runs: with and without results by period
struct Runs {
    with: crate::project::OrderedMap<Res>,
    without: crate::project::OrderedMap<Res>,
}

// A junction's state in one period
struct State {
    band: f64,
    los: String,
    dos: f64,
    name: String,
    p: String,
}

struct Summary {
    trips: f64,
    vht: f64,
    worst: Option<SensWorst>,
    los_ef: usize,
    over5: Option<f64>,
    over10: Option<f64>,
    state: crate::project::OrderedMap<State>,
    runs: Runs,
}

/// `Engine.sensitivity(sc, opts)`: Table SENS-1 for a scenario, running `assign` on changed copies.
///
/// p is `Store.P` (its settings are `Store.S`, copied for each run; its register serves T5).
/// assign(scenario copy, settings copy, options) must do what `Engine.assign(sc, { ...opts,
/// settings })` does. The runs are made in the JavaScript's order: the central run, then each
/// test; in each, every period with the scenario, then (with opts.without) without.
pub fn sensitivity(
    p: &Project,
    sc: &Scenario,
    mut opts: SensOpts,
    mut assign: impl FnMut(&Scenario, &Settings, &AssignOpts) -> Res,
) -> Result<SensTable, SensError> {
    let s0 = &p.settings;
    let t0 = opts.now.map_or(0.0, |f| f());
    let periods: Vec<String> = match (&opts.periods, sc.periods.get()) {
        (Some(ps), _) => ps.clone(),
        (None, Some(ps)) => ps.clone(),
        (None, None) => vec!["AM".to_string(), "PM".to_string()],
    };
    let ids: Vec<String> = opts
        .tests
        .clone()
        .unwrap_or_else(|| SENS_DEFAULT.iter().map(|s| s.to_string()).collect());
    let tests: Vec<&SensTest> = ids.iter().filter_map(|id| sens_test(id)).collect();
    // +opts.modeShare >= 0 && opts.modeShare !== '' && opts.modeShare != null
    let ms = &opts.mode_share;
    let ms_ok =
        ms.num() >= 0.0 && !ms.is_nullish() && ms.get().and_then(|v| v.as_str()) != Some("");
    if tests.iter().any(|t| t.id == "T6") && !ms_ok {
        return Err(SensError {
            error: "T6 needs the alternative mode share (% of vehicle trips replaced).".to_string(),
        });
    }
    let n_runs = (tests.len() + 1) * periods.len() * if opts.without.is_some() { 2 } else { 1 };
    let mut k = 0usize;
    let mut run = |t: Option<&SensTest>, label: &str, opts: &mut SensOpts| -> Runs {
        let mut s = s0.clone();
        let mut s1 = clone_sc(sc);
        let mut w1 = opts.without.map(clone_sc);
        if let Some(t) = t {
            t.apply_settings(&mut s);
            t.apply_scenario(&mut s1, Some(p), &opts.mode_share);
            if let Some(w) = w1.as_mut() {
                t.apply_scenario(w, Some(p), &opts.mode_share);
            }
        }
        // demandFactor: t && t.df (null for the central run)
        let df = match t {
            None => Maybe::Null,
            Some(t) => t.df.map_or(Maybe::Missing, Num::number),
        };
        let mut out = Runs {
            with: Default::default(),
            without: Default::default(),
        };
        for per in &periods {
            let ao = AssignOpts {
                gap_tol: opts.gap_tol.clone(),
                max_iter: opts.max_iter.clone(),
                period: Some(per.clone()),
                demand_factor: df.clone(),
                select: None,
                arc_penalty: None,
            };
            k += 1;
            if let Some(f) = opts.on_progress.as_mut() {
                f(k, n_runs, &format!("{}: {}", label, per));
            }
            out.with.insert(per.clone(), assign(&s1, &s, &ao));
            if let Some(w) = &w1 {
                k += 1;
                if let Some(f) = opts.on_progress.as_mut() {
                    f(k, n_runs, &format!("{}: {} (without)", label, per));
                }
                out.without.insert(per.clone(), assign(w, &s, &ao));
            }
        }
        out
    };
    let has_without = opts.without.is_some();
    let name_of = |key: &str, opts: &SensOpts| match opts.name {
        Some(f) => f(key),
        None => key.to_string(),
    };
    let summary = |o: Runs, opts: &SensOpts| -> Summary {
        let (mut trips, mut vht) = (0.0, 0.0);
        let mut worst: Option<SensWorst> = None;
        let mut ef: Vec<String> = Vec::new();
        let mut state = crate::project::OrderedMap::new();
        let (mut n5, mut n10) = (0.0, 0.0);
        for per in &periods {
            let r = &o.with[per.as_str()];
            trips += r.kpi.trips;
            vht += r.kpi.vht;
            for (id, nr) in r.nodes.iter() {
                let key = nr
                    .site
                    .clone()
                    .filter(|x| !x.is_empty())
                    .unwrap_or_else(|| id.to_string());
                if worst.as_ref().is_none_or(|w| nr.dos > w.dos) {
                    worst = Some(SensWorst {
                        id: key.clone(),
                        name: name_of(&key, opts),
                        dos: nr.dos,
                        period: per.clone(),
                    });
                }
                if (nr.los == "E" || nr.los == "F") && !ef.contains(&key) {
                    ef.push(key.clone());
                }
                let band = nr.appr.iter().fold(0.0, |m, a| {
                    let b = match a.dos_band.as_deref() {
                        Some("over") => 2.0,
                        Some("near") => 1.0,
                        _ => 0.0,
                    };
                    math_max(m, b)
                });
                state.insert(
                    format!("{}|{}", per, key),
                    State {
                        band,
                        los: nr.los.clone(),
                        dos: nr.dos,
                        name: name_of(&key, opts),
                        p: per.clone(),
                    },
                );
            }
            if let Some(wo) = o.without.get(per)
                && let Some(x) = extent_test(sc, Some(wo), Some(r), ExtentOpts::default())
            {
                for row in x.rows.iter().filter(|r| r.included) {
                    if row.cls.as_deref() == Some("5%") {
                        n5 += 1.0;
                    } else {
                        n10 += 1.0;
                    }
                }
            }
        }
        Summary {
            trips,
            vht,
            worst,
            los_ef: ef.len(),
            over5: if has_without { Some(n5) } else { None },
            over10: if has_without { Some(n10) } else { None },
            state,
            runs: o,
        }
    };
    let central_runs = run(None, "Central", &mut opts);
    let central = summary(central_runs, &opts);
    let mut rows = Vec::new();
    let mut sentences = Vec::new();
    let mut flags = Vec::new();
    for t in &tests {
        let runs = run(Some(t), t.label, &mut opts);
        let s = summary(runs, &opts);
        let d_t = if central.trips > 0.0 {
            (s.trips - central.trips) / central.trips
        } else {
            0.0
        };
        let d_v = if central.vht > 0.0 {
            (s.vht - central.vht) / central.vht
        } else {
            0.0
        };
        let el = t
            .rel
            .filter(|r| *r != 0.0 && !r.is_nan())
            .map(|rel| Elasticity {
                trips: r3(d_t / rel),
                vht: r3(d_v / rel),
            });
        // a conclusion changes when a junction crosses its practical DoS (or capacity) or changes LoS
        let mut changed = Vec::new();
        for (key, a) in central.state.iter() {
            let Some(b) = s.state.get(key) else { continue };
            if b.band > a.band {
                changed.push(format!(
                    "{} {} ({}, {})",
                    b.name,
                    if b.band == 2.0 {
                        "exceeds capacity"
                    } else {
                        "exceeds practical DoS"
                    },
                    to_fixed(b.dos, 2),
                    b.p
                ));
            } else if b.band < a.band {
                changed.push(format!(
                    "{} drops below {} ({}, {})",
                    b.name,
                    if a.band == 2.0 {
                        "capacity"
                    } else {
                        "practical DoS"
                    },
                    to_fixed(b.dos, 2),
                    b.p
                ));
            } else if a.los != b.los {
                changed.push(format!(
                    "{} moves from LoS {} to {} ({})",
                    b.name, a.los, b.los, b.p
                ));
            }
        }
        let mut noises: Vec<Snr> = periods
            .iter()
            .filter_map(|per| snr(central.runs.with.get(per), s.runs.with.get(per), s0))
            .collect();
        js_sort_by(&mut noises, |a, b| a.r - b.r);
        let noise = noises.into_iter().next();
        let pc = |x: f64| {
            format!(
                "{}{}%",
                if x >= 0.0 { "+" } else { "\u{2212}" },
                to_fixed((x * 100.0).abs(), 1)
            )
        };
        let lower = t.label.to_lowercase();
        let mut sentence = format!(
            "{}: total vehicle trips {}, VHT {}{}. ",
            t.label,
            pc(d_t),
            pc(d_v),
            match &el {
                Some(e) => format!(" (VHT elasticity {})", to_fixed(e.vht, 2)),
                None => String::new(),
            }
        );
        if changed.is_empty() {
            sentence.push_str("No junction crosses its practical DoS or changes LoS.");
        } else {
            sentence.push_str(&format!(
                "Conclusion sensitive: at {} {}{}.",
                lower,
                changed[0],
                if changed.len() > 1 {
                    format!(", and {} more", changed.len() - 1)
                } else {
                    String::new()
                }
            ));
        }
        if let Some(n) = &noise
            && !n.ok
        {
            sentence.push(' ');
            sentence.push_str(&n.text);
        }
        let input = if t.id == "T6" {
            format!(
                "zone reductions set to {}%",
                number_to_js_string(opts.mode_share.num())
            )
        } else {
            t.input.to_string()
        };
        if !changed.is_empty() {
            flags.push(format!(
                "Conclusion sensitive: at {} {}.",
                lower, changed[0]
            ));
        }
        sentences.push(sentence.clone());
        rows.push(SensRow::Test(SensTestRow {
            id: t.id.to_string(),
            test: t.test.to_string(),
            label: t.label.to_string(),
            input,
            trips: r1(s.trips),
            vht: r1(s.vht),
            d_trips: r3(d_t),
            d_vht: r3(d_v),
            worst: s.worst.clone(),
            los_ef: s.los_ef as f64,
            over5: s.over5,
            over10: s.over10,
            elasticity: el,
            sensitive: !changed.is_empty(),
            changes: changed,
            noise,
            sentence,
        }));
    }
    if let Some(eb) = opts.econ_base {
        let ec =
            periods
                .iter()
                .find_map(|per| match econ(s0, eb.sc, eb.res, sc, eb.opt_res, per) {
                    Some(Econ::Mbcm(m)) => Some(m),
                    _ => None,
                });
        if let Some(ec) = ec {
            let sens = number_to_js_string(ec.sens_rate);
            let sensitive = ec
                .ipm
                .iter()
                .enumerate()
                .any(|(i, b)| b.band != ec.ipm8.get(i).and_then(|x| x.band.clone()));
            rows.push(SensRow::Discount(SensDiscountRow {
                id: "R8".to_string(),
                test: "R8".to_string(),
                label: format!("Discount rate {}% (MBCM sensitivity)", sens),
                input: format!("{}% for all {} years", sens, number_to_js_string(ec.years)),
                bcr: ec.bcr,
                bcr8: ec.bcr8,
                bcr_text: ec.bcr_text.clone(),
                bcr8_text: ec.bcr8_text.clone(),
                sentence: format!(
                    "At {}%, the BCR is {} (central {}).",
                    sens, ec.bcr8_text, ec.bcr_text
                ),
                elasticity: None,
                sensitive,
            }));
        }
    }
    let ms = opts.now.map_or(0.0, |f| math_round(f() - t0));
    Ok(SensTable {
        label: "Table SENS-1: sensitivity tests".to_string(),
        periods,
        central: SensCentral {
            trips: r1(central.trips),
            vht: r1(central.vht),
            worst: central.worst,
            los_ef: central.los_ef as f64,
            over5: central.over5,
            over10: central.over10,
        },
        rows,
        sentences,
        flags,
        runs: k as f64,
        ms,
        note: "Each test changes one input; elasticity = (\u{394}KPI / KPI) / (\u{394}input / input). Each run is checked for signal to noise against the central run (TAG M3.1).".to_string(),
    })
}

/// engine.js cloneSc: a copy of the scenario with no results (`{ ...sc, res: {} }` through JSON).
pub fn clone_sc(sc: &Scenario) -> Scenario {
    let mut c = sc.clone();
    c.res = Maybe::Set(json!({}));
    c
}
