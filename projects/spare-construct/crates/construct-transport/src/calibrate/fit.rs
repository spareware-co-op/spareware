//! The matrix estimation fit itself (`Calibrate.fit`) and clearing its factors (`Calibrate.clear`).

use super::estimate::{
    FitOptions, MatrixChange, base_view, better, breach, eligible, matrix_change, step, stats, to_obj, wfit,
};
use super::sites::{Group, ParallelCheck, Rejected, SiteOpts, parallel, sites, units, id_set};
use super::stats::Stats;
use crate::assign::assign;
use crate::datahub::{Count, Data, matches};
use crate::demand::{apply_me, demand};
use crate::graph::build_graph;
use crate::jsnum::{Maybe, Num, math_max, math_round, or_default};
use crate::project::{MatrixEstimation, MeInfo, OrderedMap, Project};
use crate::results::{AssignOpts, Res, plain};
use crate::util::{clamp, fmt};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashSet;

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
    pub weight: Option<&'a dyn Fn(&Count, &str) -> f64>,
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
pub fn iso_of_ms(ms: f64) -> String {
    let total = ms.floor() as i64;
    let days = total.div_euclid(86_400_000);
    let rem = total.rem_euclid(86_400_000);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
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
pub fn me_of(p: &mut Project) -> &mut MatrixEstimation {
    if !p.me.is_set() {
        p.me = Maybe::Set(MatrixEstimation {
            info: Maybe::Missing,
            factors: OrderedMap::new(),
        });
    }
    p.me.get_mut().expect("just made")
}

fn info_of<'a>(p: &'a Project, period: &str) -> Option<&'a MeInfo> {
    p.me.get()?.info.get()?.get(period)?.get()
}

// What a failed or empty fit puts back: this period's factors, fit record and the scenario's result
struct Keep {
    old: Option<OrderedMap<Num>>,
    info: Option<MeInfo>,
    res: Option<Value>,
}

fn restore(p: &mut Project, idx: usize, period: &str, k: &Keep) {
    let me = me_of(p);
    match &k.old {
        Some(o) => {
            me.factors.insert(period.to_string(), Maybe::Set(o.clone()));
        }
        None => {
            me.factors.remove(period);
        }
    }
    match &k.info {
        Some(i) => {
            if !me.info.is_set() {
                me.info = Maybe::Set(OrderedMap::new());
            }
            if let Some(m) = me.info.get_mut() {
                m.insert(period.to_string(), Maybe::Set(i.clone()));
            }
        }
        None => {
            if let Some(m) = me.info.get_mut() {
                m.remove(period);
            }
        }
    }
    if let Some(Value::Object(res)) = p.scenarios[idx].res.get_mut() {
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

fn mc_of(p: &Project, zones: &[crate::project::Zone], period: &str, prior: &[f64]) -> MatrixChange {
    let mut est = prior.to_vec();
    apply_me(p, zones, period, &mut est, None);
    matrix_change(zones.len(), prior, &est)
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `Calibrate.fit(sc, opts)`: matrix estimation of one scenario's base year against the project's counts, in a period.
///
/// Starts from the prior matrix (the trip rates and gateway counts as they are, without any earlier estimation),
/// assigns, and takes ME2 steps until a pass does not improve the fit, breaks a limit, or the passes run out; keeps the
/// best pass. On success it sets the factors `P.me[period]`, the fit record `P.me.info[period]` (the date, the
/// statistics before and after, the matrix change, the options, the additive deltas future years use) and the
/// scenario's result for the period, and marks other scenarios' results for the period out of date. When nothing can
/// be fitted, or no pass improves, the earlier factors and record stay and the result says why (`stopped`, `note`).
/// `progress` is called as (pass, passes, message). Errors only when the scenario is not in the project.
pub fn fit(
    p: &mut Project,
    sc_id: &str,
    opts: &FitOpts,
    mut progress: Option<&mut dyn FnMut(f64, f64, &str)>,
) -> Result<FitResult, String> {
    let idx = p
        .scenarios
        .iter()
        .position(|s| s.id == sc_id)
        .ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let sc = p.scenarios[idx].clone();
    let s = p.settings.clone();
    let period: String = match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => s.period.as_str().unwrap_or("AM").to_string(),
    };
    let mut prog = |k: f64, n: f64, msg: &str| {
        if let Some(f) = progress.as_deref_mut() {
            f(k, n, msg);
        }
    };
    let nan = f64::NAN;
    let mut o = FitOptions {
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
        n_counts: None,
        groups: None,
    };
    // Which counts take part and how much each weighs; groups fit as totals once found parallel
    let only: Option<HashSet<String>> = opts.count_ids.map(id_set);
    let so = SiteOpts {
        count_ids: only.as_ref(),
        estimates: true,
        weights: opts.weights,
        weight: opts.weight,
    };
    let groups: Option<&[Group]> = opts.groups;
    if let Some(ids) = &only {
        o.n_counts = Some(ids.len());
    }
    if let Some(g) = groups {
        o.groups = Some(g.len());
    }
    let data = Data::read(p);
    // Everything an empty fit must put back
    let old: Option<OrderedMap<Num>> = {
        let me = me_of(p);
        me.factors.get(&period).and_then(|f| f.get()).cloned()
    };
    let keep = Keep {
        old: old.clone(),
        info: info_of(p, &period).cloned(),
        res: match sc.res.get() {
            Some(Value::Object(m)) => m.get(&period).filter(|v| !v.is_null()).cloned(),
            _ => None,
        },
    };
    me_of(p).factors.remove(&period);

    let bv = base_view(&sc, &s);
    let view: &crate::project::Scenario = &bv.view;
    let same = bv.same;
    let g = build_graph(view, &s, None);
    let prior = demand(p, view, &g, &period, &s).t;
    prog(
        0.0,
        o.iters,
        &format!("Running the base year ({}) with the prior matrix", s.base_year.js_string()),
    );
    let ao = AssignOpts {
        period: Some(period.clone()),
        ..AssignOpts::default()
    };
    let res0 = assign(p, view, &s, &ao, None);
    let m = matches(view, &s, &data);
    let mut rows = sites(&res0, &period, &so, &s, &data, &m);
    let before = stats(&rows);
    let mut history: Vec<Stats> = vec![before.clone()];
    let chk: Option<ParallelCheck> = match groups {
        Some(gs) if res0.paths.is_set() && !res0.paths.is_null() => Some(parallel(&res0, &rows, gs, &s)),
        _ => None,
    };
    let rejected: Vec<Rejected> = chk.as_ref().map(|c| c.rejected.clone()).unwrap_or_default();
    let old_len = old.as_ref().map_or(0, |o| o.len());
    let kept = old.is_some();
    macro_rules! none {
        ($note:expr, $history:expr) => {{
            restore(p, idx, &period, &keep);
            FitResult {
                before: before.clone(),
                after: before.clone(),
                iters: 0.0,
                changed_pairs: old_len,
                matrix_change: None,
                history: $history,
                stopped: "none".to_string(),
                res: None,
                base_year: Maybe::Missing,
                rejected: rejected.clone(),
                note: $note,
                kept: Some(kept),
            }
        }};
    }
    if before.n == 0 {
        return Ok(none!(
            format!("No counts match the network for the {period} peak. Load counts for this area first."),
            history
        ));
    }
    if !res0.paths.is_set() || res0.paths.is_null() {
        return Ok(none!(
            "This model has too many zones to keep the paths matrix estimation needs.".to_string(),
            history
        ));
    }
    if !units(&rows, chk.as_ref()).iter().any(|u| u.w > 0.0) {
        return Ok(none!(
            "Every count that matches the network has zero weight (NZTA estimates only), so nothing can be fitted. Load measured counts first.".to_string(),
            history
        ));
    }
    if res0
        .zone_ids
        .iter()
        .filter(|id| eligible(view.zones.get(id.as_str())))
        .count()
        < 2
    {
        return Ok(none!(
            "No OD pairs can be estimated. Mark the base-year zones as existing land use, or add gateways.".to_string(),
            history
        ));
    }
    struct Best {
        f: OrderedMap<f64>,
        st: Stats,
        wf: super::estimate::WFit,
        res: Res,
        k: f64,
        mc: Option<MatrixChange>,
    }
    let mut best = Best {
        f: OrderedMap::new(),
        st: before.clone(),
        wf: wfit(&rows),
        res: res0.clone(),
        k: 0.0,
        mc: None,
    };
    let mut stopped = "iterations".to_string();
    let step0 = o.max_step;
    let mut steps: Vec<f64> = [2.0, 1.5, 1.25, 1.1].into_iter().filter(|&x| x < step0).collect();
    let mut tries = 0;
    loop {
        let mut f: OrderedMap<f64> = OrderedMap::new();
        let mut res = res0.clone();
        let mut broke = "";
        rows = sites(&res0, &period, &so, &s, &data, &m);
        let mut k = 1.0f64;
        while k <= o.iters {
            step(view, &res, &units(&rows, chk.as_ref()), &mut f, &o);
            me_of(p).factors.insert(period.clone(), Maybe::Set(to_obj(&f)));
            prog(
                k,
                o.iters,
                &format!(
                    "Matrix estimation pass {} of {}{}",
                    crate::jsnum::number_to_js_string(k),
                    crate::jsnum::number_to_js_string(o.iters),
                    if tries > 0 {
                        format!(" (smaller steps, up to \u{d7}{})", crate::jsnum::number_to_js_string(o.max_step))
                    } else {
                        String::new()
                    }
                ),
            );
            let mc = mc_of(p, &g.zones, &period, &prior);
            if o.constrain {
                broke = breach(&mc);
                if !broke.is_empty() {
                    break;
                }
            }
            res = assign(p, view, &s, &ao, None);
            rows = sites(&res, &period, &so, &s, &data, &m);
            let st = stats(&rows);
            let wf = wfit(&rows);
            history.push(st.clone());
            if better(&wf, &best.wf) {
                best = Best {
                    f: f.clone(),
                    st,
                    wf,
                    res: res.clone(),
                    k,
                    mc: Some(mc),
                };
            } else {
                stopped = "no improvement".to_string();
                break;
            }
            k += 1.0;
        }
        if !broke.is_empty() {
            stopped = format!("{broke} limit");
        }
        // The very first pass broke a limit: retry from the prior with smaller steps
        if !broke.is_empty() && best.k == 0.0 && !steps.is_empty() {
            o.max_step = steps.remove(0);
            history.truncate(1);
            tries += 1;
            continue;
        }
        break;
    }
    if best.k == 0.0 {
        let why = if stopped.ends_with("limit") {
            format!("every pass broke the {stopped} (TMDG section 8 / TAG M3.1 Table 5)")
        } else {
            "no pass improved the fit".to_string()
        };
        let note = format!(
            "Matrix estimation made no change: {why}. {}",
            if old.is_some() {
                "The earlier factors were kept."
            } else {
                "Check the trip rates and gateway counts."
            }
        );
        let mut out = none!(note, history);
        out.stopped = stopped;
        return Ok(out);
    }
    let factors = to_obj(&best.f);
    me_of(p).factors.insert(period.clone(), Maybe::Set(factors.clone()));
    let mc = match best.mc.clone() {
        Some(m) => m,
        None => mc_of(p, &g.zones, &period, &prior),
    };
    /* Additive deltas for future years (docs/spec/matrix-estimation: futures inherit meDelta = T_post - T_prior):
       veh/h per estimated pair, from the base-year prior; applyME uses them for other scenario years */
    let zi: std::collections::HashMap<&str, usize> = g.zones.iter().enumerate().map(|(i, z)| (z.id.as_str(), i)).collect();
    let zn = g.zones.len();
    let mut delta = Map::new();
    for (key, fv) in factors.iter() {
        let Some(sp) = key.find('>') else {
            // key.indexOf('>') is -1: slice(0, -1) and slice(0) look up ids that are not zones
            continue;
        };
        let (Some(&i), Some(&j)) = (zi.get(&key[..sp]), zi.get(&key[sp + 1..])) else {
            continue;
        };
        let d = prior[i * zn + j] * (fv.num() - 1.0);
        if d.is_finite() && d.abs() >= 0.0005 {
            delta.insert(key.to_string(), crate::jsnum::json_number(math_round(d * 1000.0) / 1000.0));
        }
    }
    let mut extra: Map<String, Value> = Map::new();
    extra.insert("iters".to_string(), crate::jsnum::json_number(best.k));
    extra.insert("before".to_string(), to_plain(&before));
    extra.insert("after".to_string(), to_plain(&best.st));
    extra.insert("matrixChange".to_string(), to_plain(&mc));
    extra.insert("tripEndR2".to_string(), mc.trip_end_r2.map_or(Value::Null, crate::jsnum::json_number));
    extra.insert("options".to_string(), to_plain(&o));
    extra.insert("scenario".to_string(), Value::String(sc.id.clone()));
    extra.insert("stopped".to_string(), Value::String(stopped.clone()));
    extra.insert("future".to_string(), Value::String("delta".to_string()));
    if let Some(ids) = &only {
        // [...idSet(opts.countIds)]: the ids in the order they were given, once each
        let mut seen = HashSet::new();
        let list: Vec<Value> = opts
            .count_ids
            .unwrap_or(&[])
            .iter()
            .filter(|i| seen.insert(i.as_str()))
            .map(|i| Value::String(i.clone()))
            .collect();
        let _ = ids;
        extra.insert("countIds".to_string(), Value::Array(list));
    }
    if !rejected.is_empty() {
        extra.insert("rejected".to_string(), to_plain(&rejected));
    }
    let date = opts.date.map(str::to_string).unwrap_or_else(now_iso);
    let info = MeInfo {
        base_year: Num::number(s.base_year.num()),
        delta: Maybe::Set(Value::Object(delta)),
        date: Maybe::Set(Value::String(date)),
        extra,
    };
    {
        let me = me_of(p);
        if !me.info.is_set() {
            me.info = Maybe::Set(OrderedMap::new());
        }
        me.info
            .get_mut()
            .expect("just made")
            .insert(period.clone(), Maybe::Set(info));
    }
    // The factors change this period's demand in every scenario with existing land use
    for (i, other) in p.scenarios.iter_mut().enumerate() {
        if i == idx {
            continue;
        }
        if let Some(Value::Object(r)) = other.res.get_mut()
            && let Some(Value::Object(x)) = r.get_mut(&period)
        {
            x.insert("stale".to_string(), Value::Bool(true));
        }
    }
    // The scenario's own result: the fitted one when the scenario is the base year, else a fresh run with the factors
    prog(o.iters, o.iters, "Running the scenario with the fitted matrix");
    let own = if same {
        best.res.clone()
    } else {
        assign(p, &sc, &s, &ao, None)
    };
    {
        let target = &mut p.scenarios[idx];
        if !matches!(target.res, Maybe::Set(Value::Object(_))) {
            target.res = Maybe::Set(Value::Object(Map::new()));
        }
        if let Some(Value::Object(r)) = target.res.get_mut() {
            r.insert(period.clone(), serde_json::to_value(&own).expect("a result serialises"));
        }
    }
    let n = factors.len();
    let mut note = format!(
        "Matrix estimation (base year {}): {} OD pair{} adjusted in {} pass{}; sites with GEH < 5 went from {} to {}; matrix total {}.",
        s.base_year.js_string(),
        fmt::int(n as f64),
        plural(n),
        crate::jsnum::number_to_js_string(best.k),
        if best.k == 1.0 { "" } else { "es" },
        fmt::pct(before.pass.unwrap_or(f64::NAN), 0),
        fmt::pct(best.st.pass.unwrap_or(f64::NAN), 0),
        fmt::signed_pct(mc.total_pct / 100.0, 1),
    );
    if !rejected.is_empty() {
        note.push_str(&format!(
            " {} count group{} left out: counts in sequence ({}).",
            rejected.len(),
            if rejected.len() == 1 { " was" } else { "s were" },
            rejected.iter().map(|r| r.label.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    if !same {
        note.push_str(" Other years carry the estimation forward as an additive delta on the same OD pairs.");
    }
    Ok(FitResult {
        before,
        after: best.st,
        iters: best.k,
        changed_pairs: n,
        matrix_change: Some(mc),
        history,
        stopped,
        res: Some(best.res),
        base_year: s.base_year.clone(),
        rejected,
        note,
        kept: None,
    })
}

// A record as it is saved in the project: JSON, where a number that is not finite is null (as JSON.stringify writes it)
fn to_plain<T: Serialize>(v: &T) -> Value {
    let mut x = serde_json::to_value(v).expect("a record serialises");
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
        let k = me.factors.get(pd).and_then(|f| f.get()).map_or(0, |f| f.len());
        if me.factors.get(pd).is_some_and(|f| f.is_set()) {
            n += k;
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
        for sc in p.scenarios.iter_mut() {
            if let Some(Value::Object(r)) = sc.res.get_mut()
                && let Some(Value::Object(x)) = r.get_mut(pd)
            {
                x.insert("stale".to_string(), Value::Bool(true));
            }
        }
    }
    let me = p.me.get_mut().expect("still there");
    if me.info.get().is_some_and(|i| i.is_empty()) {
        me.info = Maybe::Missing;
    }
    if me.info.is_missing() && me.factors.is_empty() {
        p.me = Maybe::Missing;
    }
    n
}

/// `Store.res(sc, period)`: the scenario's saved result for a period, read back as a typed result (None when it has none).
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
pub(crate) fn lend<'a>(p: &'a mut Option<&mut dyn FnMut(f64, f64, &str)>) -> Option<&'a mut dyn FnMut(f64, f64, &str)> {
    match p {
        Some(f) => Some(&mut **f),
        None => None,
    }
}
