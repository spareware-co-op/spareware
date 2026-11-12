//! The matrix estimation fit itself (`Calibrate.fit`) and clearing its factors (`Calibrate.clear`).

use super::estimate::{
    FitOptions, MatrixChange, base_view, better, breach, eligible, matrix_change, step, stats, to_obj, wfit,
};
use super::sites::{CountWeight, Group, ParallelCheck, Rejected, SiteOpts, SiteRow, parallel, sites, units, id_set};
use super::stats::Stats;
use crate::assign::assign;
use crate::datahub::{Data, Matches, matches};
use crate::demand::{apply_me, demand};
use crate::graph::build_graph;
use crate::jsnum::{Maybe, Num, f64_to_i64, math_max, math_round, number_to_js_string, or_default, usize_to_f64};
use crate::project::{MatrixEstimation, MeInfo, OrderedMap, Project, Scenario, Settings, Zone};
use crate::results::{AssignOpts, Res, plain};
use crate::util::{clamp, fmt};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// A progress callback, called as (pass, passes, message).
pub type Progress<'a> = &'a mut (dyn FnMut(f64, f64, &str) + 'a);

/// The options of [`fit`] (the `opts` of `Calibrate.fit`); every field is optional and defaults as the prototype's do.
#[derive(Clone, Copy, Default)]
pub struct FitOpts<'a> {
    /// Period ("AM" or "PM"); the settings' period when None.
    pub period: Option<&'a str>,
    /// Passes (6).
    pub iters: Option<f64>,
    /// Damping of each step (0.7, kept in 0.01 to 1).
    pub damp: Option<f64>,
    /// Largest cumulative factor (3).
    pub max_factor: Option<f64>,
    /// Largest site ratio per step (2).
    pub max_step: Option<f64>,
    /// Sites with a modelled or counted flow below this are left out of a step (10).
    pub min_vol: Option<f64>,
    /// A pass that breaks the matrix-change limits ends the fit (true).
    pub constrain: Option<bool>,
    /// Gateway-to-gateway cells (through traffic) keep the prior.
    pub freeze_gg: bool,
    /// Weigh counts by observation method (`opts.weights === true`).
    pub weights: bool,
    /// A caller's own weight of a count in a period (`opts.weight`); wins over `weights`.
    pub weight: Option<CountWeight<'a>>,
    /// Only these counts take part (`opts.countIds`).
    pub count_ids: Option<&'a [String]>,
    /// Count groups fitted as group totals (`opts.groups`).
    pub groups: Option<&'a [Group]>,
    /// The time to record in the fit record, as an ISO text (the clock when None).
    pub date: Option<&'a str>,
}

/// What [`fit`] gives (`Calibrate.fit`'s result).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FitResult {
    /// Statistics of the prior run.
    pub before: Stats,
    /// Statistics of the best pass (the prior's when nothing changed).
    pub after: Stats,
    /// Passes the best result took (0 when nothing changed).
    #[serde(with = "plain")]
    pub iters: f64,
    /// OD pairs with a factor.
    pub changed_pairs: usize,
    /// How far the matrix moved (None when nothing changed).
    pub matrix_change: Option<MatrixChange>,
    /// Statistics of the prior and of every pass.
    pub history: Vec<Stats>,
    /// Why the fit stopped: "iterations", "no improvement", "matrix total limit", "trip ends limit" or "none".
    pub stopped: String,
    /// The best pass's result (None when nothing changed).
    #[serde(skip)]
    pub res: Option<Res>,
    /// The settings' base year (only when a fit was kept).
    #[serde(skip_serializing_if = "Maybe::is_missing")]
    pub base_year: Num,
    /// Count groups left out because their counts lie in sequence.
    pub rejected: Vec<Rejected>,
    /// A sentence saying what happened.
    pub note: String,
    /// Earlier factors were kept (only when nothing changed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<bool>,
}

/// The ISO text of a time in ms since 1970 (`new Date(ms).toISOString()`).
#[must_use]
pub fn iso_of_ms(ms: f64) -> String {
    let total = f64_to_i64(ms.floor());
    let days = total.div_euclid(86_400_000);
    let rem = total.rem_euclid(86_400_000);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm). Bound: a JavaScript date is within
    // 10^8 days of 1970, so none of these steps comes near i64's range; the saturation only keeps them total.
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = doe
        .saturating_sub(doe / 1460)
        .saturating_add(doe / 36524)
        .saturating_sub(doe / 146_096)
        / 365;
    let y = yoe.saturating_add(era.saturating_mul(400));
    let doy = doe.saturating_sub(yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100));
    let mp = doy.saturating_mul(5).saturating_add(2) / 153;
    let d = doy.saturating_sub(mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(1);
    let m = if mp < 10 { mp.saturating_add(3) } else { mp.saturating_sub(9) };
    let y = if m <= 2 { y.saturating_add(1) } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y,
        m,
        d,
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

fn now_iso() -> String {
    iso_of_ms(crate::util::now_ms())
}

/// `meOf()`: the project's matrix estimation record, made empty when the project has none.
///
/// Always Some: the record is made when it is missing (an Option only because the record is reached through
/// `Project::me`, which can hold none).
pub fn me_of(p: &mut Project) -> Option<&mut MatrixEstimation> {
    if !p.me.is_set() {
        p.me = Maybe::Set(MatrixEstimation {
            info: Maybe::Missing,
            factors: OrderedMap::new(),
        });
    }
    p.me.get_mut()
}

fn info_of<'a>(p: &'a Project, period: &str) -> Option<&'a MeInfo> {
    p.me.get()?.info.get()?.get(period)?.get()
}

// Puts a fit record in `P.me.info[period]`, making `P.me.info` if it has none
fn set_info(me: &mut MatrixEstimation, period: &str, info: MeInfo) {
    if !me.info.is_set() {
        me.info = Maybe::Set(OrderedMap::new());
    }
    if let Some(m) = me.info.get_mut() {
        m.insert(period.to_string(), Maybe::Set(info));
    }
}

// What a failed or empty fit puts back: this period's factors, fit record and the scenario's result
struct Keep {
    old: Option<OrderedMap<Num>>,
    info: Option<MeInfo>,
    res: Option<Value>,
}

// Everything an empty fit must put back, read before the fit removes this period's factors (which it then does)
fn keep_of(p: &mut Project, sc: &Scenario, period: &str) -> Keep {
    let old = me_of(p).and_then(|me| me.factors.get(period).and_then(|f| f.get()).cloned());
    let keep = Keep {
        old,
        info: info_of(p, period).cloned(),
        res: match sc.res.get() {
            Some(Value::Object(m)) => m.get(period).filter(|v| !v.is_null()).cloned(),
            _ => None,
        },
    };
    if let Some(me) = me_of(p) {
        me.factors.remove(period);
    }
    keep
}

fn restore(p: &mut Project, idx: usize, period: &str, k: &Keep) {
    if let Some(me) = me_of(p) {
        match &k.old {
            Some(o) => {
                me.factors.insert(period.to_string(), Maybe::Set(o.clone()));
            }
            None => {
                me.factors.remove(period);
            }
        }
        match &k.info {
            Some(i) => set_info(me, period, i.clone()),
            None => {
                if let Some(m) = me.info.get_mut() {
                    m.remove(period);
                }
            }
        }
    }
    if let Some(Value::Object(res)) = p.scenarios.get_mut(idx).and_then(|sc| sc.res.get_mut()) {
        match &k.res {
            Some(v) => {
                res.insert(period.to_string(), v.clone());
            }
            None => {
                res.remove(period);
            }
        }
    }
}

fn mc_of(p: &Project, zones: &[Zone], period: &str, prior: &[f64]) -> MatrixChange {
    let mut est = prior.to_vec();
    apply_me(p, zones, period, &mut est, None);
    matrix_change(zones.len(), prior, &est)
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// The best pass so far
struct Best {
    f: OrderedMap<f64>,
    st: Stats,
    wf: super::estimate::WFit,
    res: Res,
    k: f64,
    mc: Option<MatrixChange>,
}

impl Best {
    // The prior run, before any pass
    fn prior(before: &Stats, rows: &[SiteRow], res0: &Res) -> Best {
        Best {
            f: OrderedMap::new(),
            st: before.clone(),
            wf: wfit(rows),
            res: res0.clone(),
            k: 0.0,
            mc: None,
        }
    }
}

// The scenario's position in the project and a copy of it
fn scenario_of(p: &Project, sc_id: &str) -> Result<(usize, Scenario), String> {
    p.scenarios
        .iter()
        .enumerate()
        .find(|(_, cand)| cand.id == sc_id)
        .map(|(at, found)| (at, found.clone()))
        .ok_or_else(|| format!("the project has no scenario {sc_id:?}"))
}

// What every pass of a fit reads: the base-year view and its prior matrix, the settings, the counts and where they
// match, and the prior run
struct Run<'r> {
    view: &'r Scenario,
    settings: &'r Settings,
    ao: &'r AssignOpts,
    so: &'r SiteOpts<'r>,
    data: &'r Data,
    matched: &'r Matches,
    chk: Option<&'r ParallelCheck>,
    zones: &'r [Zone],
    prior: &'r [f64],
    period: &'r str,
    res0: &'r Res,
    sc_id: &'r str,
    opts: &'r FitOpts<'r>,
}

// What a fit that changes nothing gives, and what it puts back into the project
struct Unchanged<'u> {
    idx: usize,
    period: &'u str,
    keep: Keep,
    before: &'u Stats,
    rejected: &'u [Rejected],
}

impl Unchanged<'_> {
    fn result(&self, p: &mut Project, note: String, history: Vec<Stats>) -> FitResult {
        restore(p, self.idx, self.period, &self.keep);
        FitResult {
            before: self.before.clone(),
            after: self.before.clone(),
            iters: 0.0,
            changed_pairs: self.keep.old.as_ref().map_or(0, OrderedMap::len),
            matrix_change: None,
            history,
            stopped: "none".to_string(),
            res: None,
            base_year: Maybe::Missing,
            rejected: self.rejected.to_vec(),
            note,
            kept: Some(self.keep.old.is_some()),
        }
    }

    // No pass improved the fit, or every pass broke a limit
    fn no_pass(&self, p: &mut Project, stopped: String, history: Vec<Stats>) -> FitResult {
        let why = if stopped.ends_with("limit") {
            format!("every pass broke the {stopped} (TMDG section 8 / TAG M3.1 Table 5)")
        } else {
            "no pass improved the fit".to_string()
        };
        let note = format!(
            "Matrix estimation made no change: {why}. {}",
            if self.keep.old.is_some() {
                "The earlier factors were kept."
            } else {
                "Check the trip rates and gateway counts."
            }
        );
        let mut out = self.result(p, note, history);
        out.stopped = stopped;
        out
    }
}

// `o` of the prototype's fit: the options with their defaults
fn fit_options(opts: &FitOpts, only: Option<&HashSet<String>>) -> FitOptions {
    let nan = f64::NAN;
    FitOptions {
        iters: math_max(0.0, math_round(opts.iters.unwrap_or(6.0))),
        damp: clamp(or_default(opts.damp.unwrap_or(0.7), 0.7), 0.01, 1.0),
        max_factor: math_max(1.0, or_default(opts.max_factor.unwrap_or(nan), 3.0)),
        max_step: math_max(1.0, or_default(opts.max_step.unwrap_or(nan), 2.0)),
        min_vol: opts.min_vol.unwrap_or(10.0),
        constrain: opts.constrain != Some(false),
        freeze_gg: opts.freeze_gg,
        weights: if opts.weights {
            "method"
        } else if opts.weight.is_some() {
            "caller"
        } else {
            "none"
        },
        n_counts: only.map(HashSet::len),
        groups: opts.groups.map(<[Group]>::len),
    }
}

// Why nothing can be fitted, when nothing can
fn cannot_fit(
    before: &Stats,
    res0: &Res,
    rows: &[SiteRow],
    chk: Option<&ParallelCheck>,
    view: &Scenario,
    period: &str,
) -> Option<String> {
    if before.n == 0 {
        return Some(format!(
            "No counts match the network for the {period} peak. Load counts for this area first."
        ));
    }
    if !res0.paths.is_set() || res0.paths.is_null() {
        return Some("This model has too many zones to keep the paths matrix estimation needs.".to_string());
    }
    if !units(rows, chk).iter().any(|u| u.w > 0.0) {
        return Some(
            "Every count that matches the network has zero weight (NZTA estimates only), so nothing can be fitted. Load measured counts first.".to_string(),
        );
    }
    if res0
        .zone_ids
        .iter()
        .filter(|id| eligible(view.zones.get(id.as_str())))
        .count()
        < 2
    {
        return Some(
            "No OD pairs can be estimated. Mark the base-year zones as existing land use, or add gateways.".to_string(),
        );
    }
    None
}

fn pass_note(pass: f64, fo: &FitOptions, tries: u32) -> String {
    format!(
        "Matrix estimation pass {} of {}{}",
        number_to_js_string(pass),
        number_to_js_string(fo.iters),
        if tries > 0 {
            format!(" (smaller steps, up to \u{d7}{})", number_to_js_string(fo.max_step))
        } else {
            String::new()
        }
    )
}

// The passes: from the prior, ME2 steps until a pass does not improve the fit, breaks a limit or the passes run out;
// when the very first pass breaks a limit, again from the prior with smaller steps. The best pass and why it stopped.
fn search(
    p: &mut Project,
    run: &Run,
    fo: &mut FitOptions,
    start: Best,
    history: &mut Vec<Stats>,
    prog: &mut dyn FnMut(f64, f64, &str),
) -> (Best, String) {
    let mut best = start;
    let mut stopped = "iterations".to_string();
    let first_step = fo.max_step;
    let mut steps: Vec<f64> = [2.0, 1.5, 1.25, 1.1].into_iter().filter(|&x| x < first_step).collect();
    // Bound: at most one retry per smaller step (four)
    let mut tries = 0u32;
    loop {
        let mut factors: OrderedMap<f64> = OrderedMap::new();
        let mut res = run.res0.clone();
        let mut broke = "";
        let mut rows = sites(run.res0, run.period, run.so, run.settings, run.data, run.matched);
        let mut pass = 1.0f64;
        while pass <= fo.iters {
            step(run.view, &res, &units(&rows, run.chk), &mut factors, fo);
            if let Some(me) = me_of(p) {
                me.factors.insert(run.period.to_string(), Maybe::Set(to_obj(&factors)));
            }
            prog(pass, fo.iters, &pass_note(pass, fo, tries));
            let mc = mc_of(p, run.zones, run.period, run.prior);
            if fo.constrain {
                broke = breach(&mc);
                if !broke.is_empty() {
                    break;
                }
            }
            res = assign(p, run.view, run.settings, run.ao, None);
            rows = sites(&res, run.period, run.so, run.settings, run.data, run.matched);
            let st = stats(&rows);
            let wf = wfit(&rows);
            history.push(st.clone());
            if better(&wf, &best.wf) {
                best = Best {
                    f: factors.clone(),
                    st,
                    wf,
                    res: res.clone(),
                    k: pass,
                    mc: Some(mc),
                };
            } else {
                stopped = "no improvement".to_string();
                break;
            }
            pass += 1.0;
        }
        if !broke.is_empty() {
            stopped = format!("{broke} limit");
        }
        // The very first pass broke a limit: retry from the prior with smaller steps
        if !broke.is_empty() && best.k == 0.0 && !steps.is_empty() {
            fo.max_step = steps.remove(0);
            history.truncate(1);
            tries = tries.saturating_add(1);
            continue;
        }
        break;
    }
    (best, stopped)
}

/* Additive deltas for future years (docs/spec/matrix-estimation: futures inherit meDelta = T_post - T_prior):
   veh/h per estimated pair, from the base-year prior; applyME uses them for other scenario years */
fn deltas(factors: &OrderedMap<Num>, zones: &[Zone], prior: &[f64]) -> Map<String, Value> {
    let zi: HashMap<&str, usize> = zones.iter().enumerate().map(|(i, z)| (z.id.as_str(), i)).collect();
    let zn = zones.len();
    let mut delta = Map::new();
    for (key, fv) in factors.iter() {
        // key.indexOf('>') is -1: slice(0, -1) and slice(0) look up ids that are not zones
        let Some((from, to)) = key.split_once('>') else {
            continue;
        };
        let (Some(&i), Some(&j)) = (zi.get(from), zi.get(to)) else {
            continue;
        };
        // Bound: i and j are zone positions, so i * zn + j is a cell of the zn by zn prior (NaN, as undefined, if not)
        let cell = i.saturating_mul(zn).saturating_add(j);
        let d = prior.get(cell).copied().unwrap_or(f64::NAN) * (fv.num() - 1.0);
        if d.is_finite() && d.abs() >= 0.0005 {
            delta.insert(key.to_string(), crate::jsnum::json_number(math_round(d * 1000.0) / 1000.0));
        }
    }
    delta
}

// The fit record's own keys, in the prototype's order
fn record_extra(
    best: &Best,
    before: &Stats,
    mc: &MatrixChange,
    fo: &FitOptions,
    sc_id: &str,
    stopped: &str,
) -> Map<String, Value> {
    let mut extra: Map<String, Value> = Map::new();
    extra.insert("iters".to_string(), crate::jsnum::json_number(best.k));
    extra.insert("before".to_string(), to_plain(before));
    extra.insert("after".to_string(), to_plain(&best.st));
    extra.insert("matrixChange".to_string(), to_plain(mc));
    extra.insert("tripEndR2".to_string(), mc.trip_end_r2.map_or(Value::Null, crate::jsnum::json_number));
    extra.insert("options".to_string(), to_plain(fo));
    extra.insert("scenario".to_string(), Value::String(sc_id.to_string()));
    extra.insert("stopped".to_string(), Value::String(stopped.to_string()));
    extra.insert("future".to_string(), Value::String("delta".to_string()));
    extra
}

// [...idSet(opts.countIds)]: the ids in the order they were given, once each
fn ids_once(ids: &[String]) -> Value {
    let mut seen = HashSet::new();
    Value::Array(
        ids.iter()
            .filter(|i| seen.insert(i.as_str()))
            .map(|i| Value::String(i.clone()))
            .collect(),
    )
}

// Saves a fit that improved: the factors `P.me[period]` and the fit record; marks the results it changes out of date
fn save_fit(
    p: &mut Project,
    run: &Run,
    fo: &FitOptions,
    base: &Unchanged,
    best: &Best,
    stopped: &str,
) -> (OrderedMap<Num>, MatrixChange) {
    let period = run.period;
    let factors = to_obj(&best.f);
    if let Some(me) = me_of(p) {
        me.factors.insert(period.to_string(), Maybe::Set(factors.clone()));
    }
    let mc = match best.mc.clone() {
        Some(x) => x,
        None => mc_of(p, run.zones, period, run.prior),
    };
    let delta = deltas(&factors, run.zones, run.prior);
    let mut extra = record_extra(best, base.before, &mc, fo, run.sc_id, stopped);
    if let Some(ids) = run.opts.count_ids {
        extra.insert("countIds".to_string(), ids_once(ids));
    }
    if !base.rejected.is_empty() {
        extra.insert("rejected".to_string(), to_plain(&base.rejected));
    }
    let fit_date = run.opts.date.map_or_else(now_iso, str::to_string);
    let info = MeInfo {
        base_year: Num::number(run.settings.base_year.num()),
        delta: Maybe::Set(Value::Object(delta)),
        date: Maybe::Set(Value::String(fit_date)),
        extra,
    };
    if let Some(me) = me_of(p) {
        set_info(me, period, info);
    }
    mark_stale(p, base.idx, period);
    (factors, mc)
}

// The factors change this period's demand in every scenario with existing land use
fn mark_stale(p: &mut Project, idx: usize, period: &str) {
    for (i, other) in p.scenarios.iter_mut().enumerate() {
        if i == idx {
            continue;
        }
        if let Some(Value::Object(r)) = other.res.get_mut()
            && let Some(Value::Object(x)) = r.get_mut(period)
        {
            x.insert("stale".to_string(), Value::Bool(true));
        }
    }
}

// The scenario's own result for the period
fn store_res(p: &mut Project, idx: usize, period: &str, own: &Res) {
    let Some(target) = p.scenarios.get_mut(idx) else {
        return;
    };
    if !matches!(target.res, Maybe::Set(Value::Object(_))) {
        target.res = Maybe::Set(Value::Object(Map::new()));
    }
    // A result is plain data with text keys, so serialising it cannot fail
    if let Some(Value::Object(r)) = target.res.get_mut()
        && let Ok(v) = serde_json::to_value(own)
    {
        r.insert(period.to_string(), v);
    }
}

fn fit_note(
    settings: &Settings,
    n_pairs: usize,
    best: &Best,
    before: &Stats,
    mc: &MatrixChange,
    rejected: &[Rejected],
    same: bool,
) -> String {
    let mut note = format!(
        "Matrix estimation (base year {}): {} OD pair{} adjusted in {} pass{}; sites with GEH < 5 went from {} to {}; matrix total {}.",
        settings.base_year.js_string(),
        fmt::int(usize_to_f64(n_pairs)),
        plural(n_pairs),
        number_to_js_string(best.k),
        if best.k == 1.0 { "" } else { "es" },
        fmt::pct(before.pass.unwrap_or(f64::NAN), 0),
        fmt::pct(best.st.pass.unwrap_or(f64::NAN), 0),
        fmt::signed_pct(mc.total_pct / 100.0, 1),
    );
    if !rejected.is_empty() {
        // Writing to a String cannot fail
        let _ = write!(
            note,
            " {} count group{} left out: counts in sequence ({}).",
            rejected.len(),
            if rejected.len() == 1 { " was" } else { "s were" },
            rejected.iter().map(|r| r.label.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    if !same {
        note.push_str(" Other years carry the estimation forward as an additive delta on the same OD pairs.");
    }
    note
}

/// `Calibrate.fit(sc, opts)`: matrix estimation of one scenario's base year against the project's counts, in a period.
///
/// Starts from the prior matrix (the trip rates and gateway counts as they are, without any earlier estimation),
/// assigns, and takes ME2 steps until a pass does not improve the fit, breaks a limit, or the passes run out; keeps the
/// best pass. On success it sets the factors `P.me[period]`, the fit record `P.me.info[period]` (the date, the
/// statistics before and after, the matrix change, the options, the additive deltas future years use) and the
/// scenario's result for the period, and marks other scenarios' results for the period out of date. When nothing can
/// be fitted, or no pass improves, the earlier factors and record stay and the result says why (`stopped`, `note`).
/// `progress` is called as (pass, passes, message).
///
/// # Errors
///
/// When the scenario is not in the project.
pub fn fit(
    p: &mut Project,
    sc_id: &str,
    opts: &FitOpts,
    mut progress: Option<Progress<'_>>,
) -> Result<FitResult, String> {
    let (idx, sc) = scenario_of(p, sc_id)?;
    let settings = p.settings.clone();
    let period: String = match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => settings.period.as_str().unwrap_or("AM").to_string(),
    };
    let mut prog = |pass: f64, passes: f64, msg: &str| {
        if let Some(cb) = progress.as_deref_mut() {
            cb(pass, passes, msg);
        }
    };
    // Which counts take part and how much each weighs; groups fit as totals once found parallel
    let only: Option<HashSet<String>> = opts.count_ids.map(id_set);
    let mut fo = fit_options(opts, only.as_ref());
    let so = SiteOpts {
        count_ids: only.as_ref(),
        estimates: true,
        weights: opts.weights,
        weight: opts.weight,
    };
    let data = Data::read(p);
    let keep = keep_of(p, &sc, &period);

    let bv = base_view(&sc, &settings);
    let view: &Scenario = &bv.view;
    let graph = build_graph(view, &settings, None);
    let prior = demand(p, view, &graph, &period, &settings).t;
    prog(0.0, fo.iters, &format!("Running the base year ({}) with the prior matrix", settings.base_year.js_string()));
    let ao = AssignOpts { period: Some(period.clone()), ..AssignOpts::default() };
    let res0 = assign(p, view, &settings, &ao, None);
    let matched = matches(view, &settings, &data);
    let rows = sites(&res0, &period, &so, &settings, &data, &matched);
    let before = stats(&rows);
    let mut history: Vec<Stats> = vec![before.clone()];
    let chk: Option<ParallelCheck> = match opts.groups {
        Some(gs) if res0.paths.is_set() && !res0.paths.is_null() => Some(parallel(&res0, &rows, gs, &settings)),
        _ => None,
    };
    let rejected: Vec<Rejected> = chk.as_ref().map(|c| c.rejected.clone()).unwrap_or_default();
    let base = Unchanged { idx, period: &period, keep, before: &before, rejected: &rejected };
    if let Some(note) = cannot_fit(&before, &res0, &rows, chk.as_ref(), view, &period) {
        return Ok(base.result(p, note, history));
    }
    let run = Run {
        view,
        settings: &settings,
        ao: &ao,
        so: &so,
        data: &data,
        matched: &matched,
        chk: chk.as_ref(),
        zones: &graph.zones,
        prior: &prior,
        period: &period,
        res0: &res0,
        sc_id: &sc.id,
        opts,
    };
    let start = Best::prior(&before, &rows, &res0);
    let (best, stopped) = search(p, &run, &mut fo, start, &mut history, &mut prog);
    if best.k == 0.0 {
        return Ok(base.no_pass(p, stopped, history));
    }
    let (factors, mc) = save_fit(p, &run, &fo, &base, &best, &stopped);
    // The scenario's own result: the fitted one when the scenario is the base year, else a fresh run with the factors
    prog(fo.iters, fo.iters, "Running the scenario with the fitted matrix");
    let own = if bv.same {
        best.res.clone()
    } else {
        assign(p, &sc, &settings, &ao, None)
    };
    store_res(p, idx, &period, &own);
    let n_pairs = factors.len();
    let note = fit_note(&settings, n_pairs, &best, &before, &mc, &rejected, bv.same);
    Ok(FitResult {
        before,
        after: best.st,
        iters: best.k,
        changed_pairs: n_pairs,
        matrix_change: Some(mc),
        history,
        stopped,
        res: Some(best.res),
        base_year: settings.base_year.clone(),
        rejected,
        note,
        kept: None,
    })
}

// A record as it is saved in the project: JSON, where a number that is not finite is null (as JSON.stringify writes it)
fn to_plain<T: Serialize>(v: &T) -> Value {
    // A record is plain data with text keys, so serialising it cannot fail (null if it ever did)
    let mut x = serde_json::to_value(v).unwrap_or(Value::Null);
    null_non_finite(&mut x);
    x
}

fn null_non_finite(v: &mut Value) {
    match v {
        Value::String(s) if s == "NaN" || s == "Infinity" || s == "-Infinity" => *v = Value::Null,
        Value::Array(a) => a.iter_mut().for_each(null_non_finite),
        Value::Object(m) => m.values_mut().for_each(null_non_finite),
        _ => {}
    }
}

/// `Calibrate.clear(period)`: removes the estimation factors and fit record of a period (both periods when None), marks the results that used them out of date, and returns how many OD pairs had factors.
pub fn clear(p: &mut Project, period: Option<&str>) -> usize {
    let Some(me) = p.me.get_mut() else { return 0 };
    let mut n = 0usize;
    let mut hit: Vec<String> = Vec::new();
    let periods: Vec<String> = match period {
        Some(x) if !x.is_empty() => vec![x.to_string()],
        _ => vec!["AM".to_string(), "PM".to_string()],
    };
    for pd in &periods {
        let k = me.factors.get(pd).and_then(|f| f.get()).map_or(0, crate::project::OrderedMap::len);
        if me.factors.get(pd).is_some_and(crate::jsnum::Maybe::is_set) {
            // Bound: counts of OD pairs in memory, far below usize::MAX
            n = n.saturating_add(k);
            me.factors.remove(pd);
        }
        if let Some(info) = me.info.get_mut() {
            info.remove(pd);
        }
        if k > 0 {
            hit.push(pd.clone());
        }
    }
    // Only results that used factors are out of date
    for pd in &hit {
        for sc in &mut p.scenarios {
            if let Some(Value::Object(r)) = sc.res.get_mut()
                && let Some(Value::Object(x)) = r.get_mut(pd)
            {
                x.insert("stale".to_string(), Value::Bool(true));
            }
        }
    }
    // P.me was there above and nothing since removed it
    let Some(me) = p.me.get_mut() else { return n };
    if me.info.get().is_some_and(crate::project::OrderedMap::is_empty) {
        me.info = Maybe::Missing;
    }
    if me.info.is_missing() && me.factors.is_empty() {
        p.me = Maybe::Missing;
    }
    n
}

/// `Store.res(sc, period)`: the scenario's saved result for a period, read back as a typed result (None when it has none).
#[must_use]
pub fn stored_res(sc: &crate::project::Scenario, period: &str) -> Option<Res> {
    match sc.res.get() {
        Some(Value::Object(m)) => m
            .get(period)
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value::<Res>(v.clone()).ok()),
        _ => None,
    }
}

/// A progress callback lent to a call for a moment (the callback itself stays with the caller).
pub(crate) fn lend<'a>(p: &'a mut Option<Progress<'_>>) -> Option<Progress<'a>> {
    match p {
        Some(f) => Some(&mut **f),
        None => None,
    }
}
