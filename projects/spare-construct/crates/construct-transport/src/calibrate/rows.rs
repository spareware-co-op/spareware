//! Counts against a result in the calibration panel: the area of influence, the counted sites with their role and
//! method (`countRows`), the statistics of a set of sites (`statsOf`) and journey-time checks.

use super::common::{dist_to_line, is_mw, point_in_poly};
use super::roles::{Ctx, method_of};
use super::stats::{GehBand, geh_bands};
use crate::datahub::{Count, Match, jt_check, model_flow, period_vol, regression, JtCheck, geh};
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, value_to_js_string};
use crate::project::{Point, Scenario};
use crate::results::{Res, plain, travel, Travel};
use crate::util::{LatLng, fmt};
use serde::Serialize;
use serde_json::Value;
use std::rc::Rc;

/// The area of influence (`coreArea`): where a change to the network matters, so whose counts decide the status.
#[derive(Clone, Debug, PartialEq)]
pub struct Core {
    /// "user" (a drawn polygon), "buffer" (a buffer round the roads that differ from the base scenario) or "none" (every count is inside).
    pub kind: &'static str,
    /// The polygon of a drawn area.
    pub poly: Vec<LatLng>,
    /// The roads of a buffered area.
    pub lines: Vec<Vec<LatLng>>,
    /// The buffer (m).
    pub r: f64,
    /// In words.
    pub label: String,
}

/// `coreArea(sc)`: a drawn "core" polygon, else a buffer round the links that differ from the base scenario, else everything.
pub fn core_area(ctx: &Ctx, sc: &Scenario) -> Core {
    let poly = ctx
        .store
        .screenlines
        .iter()
        .find(|s| s.is_type("core") && s.pts.len() >= 3);
    if let Some(poly) = poly {
        return Core {
            kind: "user",
            poly: poly.pts.iter().map(|p: &Point| p.0).collect(),
            lines: Vec::new(),
            r: 0.0,
            label: format!("inside \u{201c}{}\u{201d}", poly.name.js_string()),
        };
    }
    let none = |label: &str| Core {
        kind: "none",
        poly: Vec::new(),
        lines: Vec::new(),
        r: 0.0,
        label: label.to_string(),
    };
    let Some(base) = ctx.p.base_scenario() else {
        return none("no area of influence: this is the base scenario, so every count is inside");
    };
    if base.id == sc.id {
        return none("no area of influence: this is the base scenario, so every count is inside");
    }
    let mut lines: Vec<Vec<LatLng>> = Vec::new();
    for l in sc.links.values() {
        let differs = match base.links.get(&l.id) {
            None => true,
            Some(b) => {
                l.a != b.a
                    || l.b != b.b
                    || !same(&l.cls, &b.cls)
                    || !same_num(&l.lanes_ab, &b.lanes_ab)
                    || !same_num(&l.lanes_ba, &b.lanes_ba)
                    || !same_num(&l.speed, &b.speed)
                    || !same(&l.status, &b.status)
                    || !same_num(&l.cap_per_lane, &b.cap_per_lane)
            }
        };
        if differs && sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b) {
            lines.push(sc.geom(l));
        }
    }
    for b in base.links.values() {
        if !sc.links.contains_key(&b.id) && base.nodes.contains_key(&b.a) && base.nodes.contains_key(&b.b) {
            lines.push(base.geom(b));
        }
    }
    if lines.is_empty() {
        return none("no area of influence: no road differs from the base scenario, so every count is inside");
    }
    let r = ctx.cfg.core_buffer_n();
    let n = lines.len();
    Core {
        kind: "buffer",
        label: format!(
            "within {} m of the {} road{} that differ from the base scenario (Spare Construct assumption)",
            fmt::int(r),
            n,
            if n == 1 { "" } else { "s" }
        ),
        poly: Vec::new(),
        lines,
        r,
    }
}

// `a === b` for two values that may be missing, null, text or numbers
fn same_num(a: &crate::jsnum::Num, b: &crate::jsnum::Num) -> bool {
    use crate::jsnum::JsNum;
    match (a, b) {
        (Maybe::Missing, Maybe::Missing) | (Maybe::Null, Maybe::Null) => true,
        (Maybe::Set(JsNum::Number(x)), Maybe::Set(JsNum::Number(y))) => x == y,
        (Maybe::Set(JsNum::Text(x)), Maybe::Set(JsNum::Text(y))) => x == y,
        (Maybe::Set(JsNum::Bool(x)), Maybe::Set(JsNum::Bool(y))) => x == y,
        (Maybe::Set(x), Maybe::Set(y)) => x.to_value() == y.to_value() && !matches!(x, JsNum::Other(_)),
        _ => false,
    }
}

fn same(a: &Maybe<String>, b: &Maybe<String>) -> bool {
    a == b
}

/// `inCore(core, ll)`: a point inside the area of influence (everywhere when there is none).
pub fn in_core(core: &Core, ll: &LatLng) -> bool {
    match core.kind {
        "none" => true,
        "user" => point_in_poly(ll, &core.poly),
        _ => core.lines.iter().any(|g| dist_to_line(ll, g) <= core.r),
    }
}

/// A counted site in the panel's statistics (`countRows`): the count, where it matched, counted and modelled flow, GEH, method, role and area.
#[derive(Clone, Debug)]
pub struct CountRow {
    /// The count.
    pub c: Rc<Count>,
    /// Where it was matched.
    pub mt: Match,
    /// The link's road class.
    pub link_cls: Option<String>,
    /// Counted volume (veh/h).
    pub obs: f64,
    /// Modelled flow (veh/h).
    pub model: f64,
    /// How the counted volume was made.
    pub how: String,
    /// GEH.
    pub geh: f64,
    /// Observation method of the panel ("measured_hour", "weekday_factored", "adt_factored", "virtual").
    pub method: &'static str,
    /// Role: "me", "valid", "gateway" or "off".
    pub role: &'static str,
    /// Counts in the statistics (not virtual, not switched off).
    pub scored: bool,
    /// Inside the area of influence.
    pub in_core: bool,
    /// On a motorway or ramp.
    pub mw: bool,
}

/// `countRows(sc, period, res, raw)`: every count in use that matches the network, worst GEH first.
///
/// `raw` gives roles without the rules that turn a validation count seen by estimation into a calibration count.
/// Note: unlike `DataHub.calibration` and `Calibrate.report`, this includes RAMM centreline estimates (as
/// "adt_factored" sites): the prototype does not leave them out here.
pub fn count_rows(ctx: &Ctx, sc: &Scenario, period: &str, res: Option<&Res>, raw: bool) -> Vec<CountRow> {
    let Some(res) = res else { return Vec::new() };
    let m = ctx.matches(sc);
    let core = core_area(ctx, sc);
    let mut out: Vec<CountRow> = Vec::new();
    for c in &ctx.data.counts {
        if !ctx.data.in_use(c) {
            continue;
        }
        let Some(mt) = m.get(&c.id) else { continue };
        let Some(pv) = period_vol(c, period, ctx.s()) else { continue };
        if !(pv.v >= 0.0) {
            continue;
        }
        let Some(model) = model_flow(res, mt) else { continue };
        let l = sc.links.get(&mt.link);
        let method = method_of(c, period);
        let role = ctx.role_of(c, period, raw);
        out.push(CountRow {
            c: c.clone(),
            mt: mt.clone(),
            link_cls: l.and_then(|l| l.cls.as_str().map(String::from)),
            obs: pv.v,
            model,
            how: pv.how,
            geh: geh(model, pv.v),
            method,
            role,
            scored: method != "virtual" && role != "off",
            in_core: in_core(&core, &c.ll()),
            mw: is_mw(l),
        });
    }
    js_sort_by(&mut out, |x, y| y.geh - x.geh);
    out
}

/// `inSet(r, set)`: a site belongs to a set of counts ("valid", "calib" or all).
pub fn in_set(r: &CountRow, set: &str) -> bool {
    r.scored
        && match set {
            "valid" => r.role == "valid",
            "calib" => r.role == "me" || r.role == "gateway",
            _ => true,
        }
}

/// `inArea(r, area)`: a site belongs to an area ("inside", "outside" or all).
pub fn in_area(r: &CountRow, area: &str) -> bool {
    match area {
        "inside" => r.in_core,
        "outside" => !r.in_core,
        _ => true,
    }
}

/// `SET_LABEL[set]`.
pub fn set_label(set: &str) -> &'static str {
    match set {
        "valid" => "validation counts",
        "calib" => "calibration counts",
        _ => "all counts",
    }
}

/// How many sites meet each criterion (`statsOf`'s `cnt`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct CalCnt {
    /// GEH < 5.
    pub geh5: usize,
    /// GEH < 7.5.
    pub geh75: usize,
    /// GEH < 10.
    pub geh10: usize,
    /// GEH < 12.
    pub geh12: usize,
    /// GEH <= 5.
    pub geh5le: usize,
    /// GEH <= 10.
    pub geh10le: usize,
    /// Within the TMDG count band.
    pub band: usize,
    /// Within the TAG flow band.
    pub tag: usize,
    /// GEH < 5 or within the TAG flow band.
    pub either: usize,
    /// Within 20% of the count.
    pub within20: usize,
}

/// The statistics of a set of counted sites or turning movements (`statsOf`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalStats {
    /// Number of sites.
    pub n: usize,
    /// Number meeting each criterion.
    pub cnt: CalCnt,
    /// Σ counted.
    #[serde(with = "plain")]
    pub tot_obs: f64,
    /// Σ modelled.
    #[serde(with = "plain")]
    pub tot_mod: f64,
    /// %RMSE.
    #[serde(with = "plain")]
    pub rmse: Option<f64>,
    /// Slope through the origin.
    #[serde(with = "plain")]
    pub b0: Option<f64>,
    /// R² through the origin, centred.
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// R² through the origin, Excel's.
    #[serde(with = "plain")]
    pub r2_excel: Option<f64>,
    /// OLS R².
    #[serde(with = "plain")]
    pub r2_ols: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub ols_a: Option<f64>,
    /// OLS slope.
    #[serde(with = "plain")]
    pub ols_b: Option<f64>,
    /// GEH bands.
    pub geh_bands: Vec<GehBand>,
    /// Share GEH < 5.
    #[serde(with = "plain")]
    pub geh5: Option<f64>,
    /// Share GEH < 7.5.
    #[serde(with = "plain")]
    pub geh75: Option<f64>,
    /// Share GEH < 10.
    #[serde(with = "plain")]
    pub geh10: Option<f64>,
    /// Share GEH < 12.
    #[serde(with = "plain")]
    pub geh12: Option<f64>,
    /// Share GEH <= 5.
    #[serde(with = "plain")]
    pub geh5le: Option<f64>,
    /// Share GEH <= 10.
    #[serde(with = "plain")]
    pub geh10le: Option<f64>,
    /// Share within the TMDG count band.
    #[serde(with = "plain")]
    pub band: Option<f64>,
    /// Share within the TAG flow band.
    #[serde(with = "plain")]
    pub tag: Option<f64>,
    /// Share GEH < 5 or within the TAG flow band.
    #[serde(with = "plain")]
    pub either: Option<f64>,
    /// Share within 20%.
    #[serde(with = "plain")]
    pub within20: Option<f64>,
}

impl CalStats {
    /// `cnt[key]` by name.
    pub fn cnt_of(&self, key: &str) -> Option<usize> {
        let c = &self.cnt;
        Some(match key {
            "geh5" => c.geh5,
            "geh75" => c.geh75,
            "geh10" => c.geh10,
            "geh12" => c.geh12,
            "geh5le" => c.geh5le,
            "geh10le" => c.geh10le,
            "band" => c.band,
            "tag" => c.tag,
            "either" => c.either,
            "within20" => c.within20,
            _ => return None,
        })
    }
    /// The statistic with this name (a share, or b0, r2, r2Excel, r2Ols, rmse), as `ev.link[t.stat]` reads it.
    pub fn stat(&self, key: &str) -> Option<f64> {
        match key {
            "geh5" => self.geh5,
            "geh75" => self.geh75,
            "geh10" => self.geh10,
            "geh12" => self.geh12,
            "geh5le" => self.geh5le,
            "geh10le" => self.geh10le,
            "band" => self.band,
            "tag" => self.tag,
            "either" => self.either,
            "within20" => self.within20,
            "b0" => self.b0,
            "r2" => self.r2,
            "r2Excel" => self.r2_excel,
            "r2Ols" => self.r2_ols,
            "rmse" => self.rmse,
            _ => None,
        }
    }
}

/// `statsOf(rows)` for sites given as (counted, modelled, GEH): the shares under each criterion, %RMSE and the regressions.
pub fn stats_of_rows(rows: &[(f64, f64, f64)]) -> CalStats {
    use super::criteria::{tag_band, tmdg_band};
    let n = rows.len();
    let mut cnt = CalCnt::default();
    let (mut so, mut sm) = (0.0f64, 0.0f64);
    for &(obs, model, g) in rows {
        so += obs;
        sm += model;
        if g < 5.0 {
            cnt.geh5 += 1;
        }
        if g < 7.5 {
            cnt.geh75 += 1;
        }
        if g < 10.0 {
            cnt.geh10 += 1;
        }
        if g < 12.0 {
            cnt.geh12 += 1;
        }
        if g <= 5.0 {
            cnt.geh5le += 1;
        }
        if g <= 10.0 {
            cnt.geh10le += 1;
        }
        let b = tmdg_band(model, obs);
        let t = tag_band(model, obs);
        if b {
            cnt.band += 1;
        }
        if t {
            cnt.tag += 1;
        }
        if g < 5.0 || t {
            cnt.either += 1;
        }
        if (model - obs).abs() <= 0.2 * obs {
            cnt.within20 += 1;
        }
    }
    let share = |k: usize| if n > 0 { Some(k as f64 / n as f64) } else { None };
    let xs: Vec<f64> = rows.iter().map(|r| r.0).collect();
    let ys: Vec<f64> = rows.iter().map(|r| r.1).collect();
    let gehs: Vec<f64> = rows.iter().map(|r| r.2).collect();
    let reg = if n >= 2 { Some(regression(&xs, &ys)) } else { None };
    let se = rows.iter().fold(0.0f64, |s, r| s + (r.1 - r.0) * (r.1 - r.0));
    CalStats {
        n,
        cnt,
        tot_obs: so,
        tot_mod: sm,
        rmse: if rows.len() > 1 && so > 0.0 {
            Some(100.0 * (se / (rows.len() - 1) as f64).sqrt() / (so / rows.len() as f64))
        } else {
            None
        },
        b0: reg.as_ref().and_then(|r| r.b0),
        r2: reg.as_ref().and_then(|r| r.r2_origin_centred),
        r2_excel: reg.as_ref().and_then(|r| r.r2_origin_excel),
        r2_ols: reg.as_ref().and_then(|r| r.r2_ols),
        ols_a: reg.as_ref().and_then(|r| r.ols_a),
        ols_b: reg.as_ref().and_then(|r| r.ols_b),
        geh_bands: geh_bands(&gehs),
        geh5: share(cnt.geh5),
        geh75: share(cnt.geh75),
        geh10: share(cnt.geh10),
        geh12: share(cnt.geh12),
        geh5le: share(cnt.geh5le),
        geh10le: share(cnt.geh10le),
        band: share(cnt.band),
        tag: share(cnt.tag),
        either: share(cnt.either),
        within20: share(cnt.within20),
    }
}

/// `statsOf(rows)` for count rows.
pub fn stats_of_count_rows(rows: &[&CountRow]) -> CalStats {
    let v: Vec<(f64, f64, f64)> = rows.iter().map(|r| (r.obs, r.model, r.geh)).collect();
    stats_of_rows(&v)
}

/* ---------------- journey times ---------------- */

/// A journey-time check with the model's time and the observed or predicted time it is compared with (`journeys` row).
#[derive(Clone, Debug)]
pub struct JourneyRow {
    /// The check's name.
    pub name: String,
    /// Modelled time (s).
    pub model: Option<f64>,
    /// Modelled length (m).
    pub len: Option<f64>,
    /// Observed time (s), with its source.
    pub obs: Option<(f64, String)>,
    /// Google typical time (s), with when it was taken.
    pub google: Option<(f64, Option<Value>)>,
    /// The check against the observed time, else the predicted one.
    pub chk: Option<JtCheck>,
    /// The reference time is an observed one.
    pub ref_observed: bool,
    /// The route is under 3 km.
    pub short: bool,
    /// The route is over 15 km.
    pub long: bool,
}

fn obj_num(v: &Maybe<Value>, k: &str, sub: Option<&str>) -> f64 {
    match v.get() {
        Some(Value::Object(m)) => match (m.get(k), sub) {
            (Some(x), None) => crate::jsnum::value_to_number(x),
            (Some(Value::Object(o)), Some(s)) => o.get(s).map_or(f64::NAN, crate::jsnum::value_to_number),
            _ => f64::NAN,
        },
        _ => f64::NAN,
    }
}

/// `journeys(sc, period, res)`: each journey check against the model's time between its nodes.
pub fn journeys(ctx: &Ctx, sc: &Scenario, period: &str, res: Option<&Res>) -> Vec<JourneyRow> {
    let mut out = Vec::new();
    for j in &ctx.data.jt {
        let (mut mt, mut len) = (None, None);
        let from = j.from.js_string();
        let to = j.to.js_string();
        if sc.nodes.contains_key(&from) && sc.nodes.contains_key(&to)
            && let Some(Travel::Route(r)) = travel(sc, ctx.s(), res, &from, &to)
        {
            mt = Some(r.time);
            len = Some(r.len);
        }
        let obs = {
            let t = obj_num(&j.obs, period, None);
            if t > 0.0 {
                let src = match j.obs.get() {
                    Some(Value::Object(m)) => m.get("src").filter(|v| crate::jsnum::value_truthy(v)).map(value_to_js_string),
                    _ => None,
                };
                Some((t, src.unwrap_or_else(|| "observed".to_string())))
            } else {
                None
            }
        };
        let google = {
            let t = obj_num(&j.g, period, Some("t"));
            if t > 0.0 {
                let when = match j.g.get() {
                    Some(Value::Object(m)) => m.get(period).and_then(|x| x.get("when")).cloned(),
                    _ => None,
                };
                Some((t, when))
            } else {
                None
            }
        };
        let reference = obs.as_ref().map(|o| o.0).or(google.as_ref().map(|g| g.0));
        let chk = match (mt, reference) {
            (Some(m), Some(t)) => jt_check(m, t),
            _ => None,
        };
        out.push(JourneyRow {
            name: j.name.js_string(),
            model: mt,
            len,
            ref_observed: obs.is_some(),
            obs,
            google,
            chk,
            short: len.is_some_and(|l| l < 3000.0),
            long: len.is_some_and(|l| l > 15000.0),
        });
    }
    out
}

/// What `jtStats` gives: how many routes meet each tier.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JtStats {
    /// Number of routes.
    pub n: usize,
    /// Routes meeting tier 1.
    pub jt1_n: usize,
    /// Routes meeting tier 2.
    pub jt2_n: usize,
    /// Share meeting tier 1.
    #[serde(with = "plain")]
    pub jt1: Option<f64>,
    /// Share meeting tier 2.
    #[serde(with = "plain")]
    pub jt2: Option<f64>,
    /// Only Google's predicted times were available.
    pub predicted: bool,
}

/// `jtStats(rows)`: the routes with an observed time (else those with a predicted one) against the two tiers.
pub fn jt_stats(rows: &[JourneyRow]) -> JtStats {
    let with_obs: Vec<&JourneyRow> = rows.iter().filter(|r| r.chk.is_some() && r.obs.is_some()).collect();
    let usable: Vec<&JourneyRow> = if with_obs.is_empty() {
        rows.iter().filter(|r| r.chk.is_some()).collect()
    } else {
        with_obs.clone()
    };
    let n = usable.len();
    let t1 = usable.iter().filter(|r| r.chk.as_ref().is_some_and(|c| c.tier1)).count();
    let t2 = usable.iter().filter(|r| r.chk.as_ref().is_some_and(|c| c.tier2)).count();
    JtStats {
        n,
        jt1_n: t1,
        jt2_n: t2,
        jt1: if n > 0 { Some(t1 as f64 / n as f64) } else { None },
        jt2: if n > 0 { Some(t2 as f64 / n as f64) } else { None },
        predicted: with_obs.is_empty() && n > 0,
    }
}

#[allow(dead_code)]
fn _unused(_: &Option<Count>) {}
