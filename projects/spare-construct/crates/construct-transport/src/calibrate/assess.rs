//! The validation assessment of the calibration panel: the evidence a model's results and the counts give
//! (`Calibration.evidence`), each test judged against its criterion (`evaluate`), the overall status (`assess`) and
//! the record the model-status badge reads (`recordValidation`).

use super::criteria::{
    CATS, Test, need, preset_tests, standard_name, tag, target_text, tmdg_tests,
};
use super::m1::{MeStats, PairVal, me_stats};
use super::rows::{
    CalStats, Core, CountRow, JourneyRow, JtStats, core_area, count_rows, in_area, in_set, journeys, jt_stats, set_label,
    stats_of_count_rows, stats_of_rows,
};
use super::roles::Ctx;
use super::screen::{SlSummary, sl_observations};
use super::stats::MIN_SITES;
use super::turnrows::{TurnCheckRow, turn_rows};
use crate::jsnum::{Maybe, Num, number_to_js_string, value_truthy};
use crate::project::{OrderedMap, Project, Scenario, ValidationRecord};
use crate::results::Res;
use crate::util::fmt;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// What `Calibration.convStats` gives: how well the assignment converged.
#[derive(Clone, Debug)]
pub struct ConvStats {
    /// 1 when there is a result, 0 when not.
    pub n: usize,
    /// Relative gap (%).
    pub gap_pct: f64,
    /// Relative gap.
    pub gap: f64,
    /// Iterations.
    pub it: f64,
    /// The run converged.
    pub converged: bool,
    /// Gap target.
    pub gap_tol: f64,
    /// The result's convergence sentence.
    pub note: String,
    /// More than 98% of links changed by less than 1%, four iterations running (TAG M3.1 Table 4).
    pub p4: bool,
    /// The last iteration's share of stable links.
    pub last_p: Option<f64>,
}

/// `convStats(res)`: the gap and the TAG stability rule of a result.
pub fn conv_stats(res: Option<&Res>) -> ConvStats {
    let Some(res) = res else {
        return ConvStats {
            n: 0,
            gap_pct: f64::NAN,
            gap: f64::NAN,
            it: f64::NAN,
            converged: false,
            gap_tol: f64::NAN,
            note: String::new(),
            p4: false,
            last_p: None,
        };
    };
    let k = res.conv.len().saturating_sub(tag::P_CONSEC);
    let last = &res.conv[k..];
    ConvStats {
        n: 1,
        gap_pct: res.gap * 100.0,
        gap: res.gap,
        it: res.it,
        converged: res.converged,
        gap_tol: res.gap_tol,
        note: res.conv_note.clone(),
        p4: last.len() == tag::P_CONSEC && last.iter().all(|e| e.p.is_some_and(|p| p > tag::P_SHARE)),
        last_p: last.last().and_then(|e| e.p),
    }
}

/// The evidence a result and the counts give for validation (`Calibration.evidence`).
#[derive(Clone, Debug)]
pub struct Evidence {
    /// Period.
    pub period: String,
    /// The set the statistics use after any fall-back ("valid", "calib" or "all").
    pub set: String,
    /// The set asked for.
    pub requested: String,
    /// The area used ("all", "inside" or "outside").
    pub area: String,
    /// The area of influence.
    pub core: Core,
    /// The area is not the one that decides the status, so pass and fail are for information only.
    pub info_only: bool,
    /// Too few independent counts: the calibration counts stand in.
    pub fallback: bool,
    /// Validation counts in the area.
    pub valid_n: usize,
    /// The sites the statistics use.
    pub rows: Vec<CountRow>,
    /// Every count that matches the network, worst GEH first.
    pub all: Vec<CountRow>,
    /// Statistics of the sites.
    pub link: CalStats,
    /// Statistics of the sites with a measured peak hour.
    pub measured: CalStats,
    /// Statistics of the turning movements.
    pub turn: CalStats,
    /// The turning movements.
    pub turn_rows: Vec<TurnCheckRow>,
    /// Screenline totals.
    pub screen: SlSummary,
    /// Journey times.
    pub jt: JtStats,
    /// Journey-time rows.
    pub jt_rows: Vec<JourneyRow>,
    /// Convergence.
    pub conv: ConvStats,
    /// Matrix estimation.
    pub me: Option<MeStats>,
    /// The result is for the base year (or has no year).
    pub base_year: bool,
    /// The result's year.
    pub year: Option<f64>,
    /// There is a result.
    pub has_res: bool,
}

/// The options of [`evidence`] and [`assess`].
#[derive(Clone, Debug, Default)]
pub struct EvOpts {
    /// Counts to use: "valid", "calib" or "all" (the project's setting when None).
    pub set: Option<String>,
    /// Area: "auto", "inside", "outside" or "all" (the project's setting when None).
    pub area: Option<String>,
    /// Roles without the rules that turn a validation count seen by estimation into a calibration count.
    pub raw: bool,
    /// No fall-back to calibration counts when there are too few validation counts.
    pub strict: bool,
    /// Criteria set ("tmdg", "rms", "atap"; the project's setting when None).
    pub standard: Option<String>,
    /// TMDG category (the project's when None).
    pub category: Option<char>,
}

// `a === b` for the result's year and the settings' base year
fn base_year_of(res: &Res, base: &Num) -> bool {
    !(res.year > 0.0) || matches!(base, Maybe::Set(crate::jsnum::JsNum::Number(b)) if *b == res.year)
}

/// `evidence(sc, period, o)`: counts, screenlines, turning movements and journey times against a result.
pub fn evidence(ctx: &Ctx, sc: &Scenario, period: &str, res: Option<&Res>, o: &EvOpts) -> Evidence {
    let set = o.set.clone().unwrap_or_else(|| crate::jsnum::value_to_js_string(&ctx.cfg.set));
    let core = core_area(ctx, sc);
    let gate = if core.kind == "none" { "all" } else { "inside" };
    let req = o
        .area
        .clone()
        .unwrap_or_else(|| crate::jsnum::value_to_js_string(&ctx.cfg.area));
    let area = if req == "auto" || req == "inside" || req.is_empty() { gate.to_string() } else { req };
    let info_only = area != gate;
    let all = count_rows(ctx, sc, period, res, o.raw);
    let pick = |s: &str| -> Vec<CountRow> { all.iter().filter(|r| in_set(r, s) && in_area(r, &area)).cloned().collect() };
    let valid_n = pick("valid").len();
    let (mut used, mut fallback, mut rows) = (set.clone(), false, pick(&set));
    if set == "valid" && valid_n < MIN_SITES && !o.strict {
        fallback = true;
        used = "calib".to_string();
        rows = pick("calib");
    }
    let tr = turn_rows(ctx, sc, period, &used, &area, res);
    let jr = journeys(ctx, sc, period, res);
    let measured: Vec<&CountRow> = rows.iter().filter(|r| r.method == "measured_hour").collect();
    let link = stats_of_count_rows(&rows.iter().collect::<Vec<_>>());
    let turn_triples: Vec<(f64, f64, f64)> = tr.iter().map(|r| (r.obs, r.model.unwrap_or(f64::NAN), r.geh.unwrap_or(f64::NAN))).collect();
    Evidence {
        period: period.to_string(),
        requested: set,
        measured: stats_of_count_rows(&measured),
        link,
        turn: stats_of_rows(&turn_triples),
        screen: sl_observations(ctx, sc, period, &used, res),
        jt: jt_stats(&jr),
        conv: conv_stats(res),
        me: me_stats(ctx.p, period),
        base_year: res.is_some_and(|r| base_year_of(r, &ctx.s().base_year)),
        year: res.map(|r| r.year),
        has_res: res.is_some(),
        set: used,
        area,
        core,
        info_only,
        fallback,
        valid_n,
        rows,
        all,
        turn_rows: tr,
        jt_rows: jr,
    }
}

/// A value of a test's statistic: a number, a flag, or a pair with its words.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// A number.
    Num(f64),
    /// A flag.
    Bool(bool),
    /// A pair (cells, trip ends, trip lengths).
    Pair(PairVal),
}

impl Ev<'_> {
    fn n_of(&self, on: &str) -> usize {
        match on {
            "link" => self.0.link.n,
            "turn" => self.0.turn.n,
            "screen" => self.0.screen.n,
            "jt" => self.0.jt.n,
            "conv" => self.0.conv.n,
            "me" => self.0.me.as_ref().map_or(0, |m| m.n),
            _ => 0,
        }
    }
    fn value(&self, on: &str, stat: &str) -> Option<Val> {
        match on {
            "link" => self.0.link.stat(stat).map(Val::Num),
            "turn" => self.0.turn.stat(stat).map(Val::Num),
            "screen" => self.0.screen.share(stat).map(Val::Num),
            "jt" => match stat {
                "jt1" => self.0.jt.jt1.map(Val::Num),
                "jt2" => self.0.jt.jt2.map(Val::Num),
                _ => None,
            },
            "conv" => {
                let c = &self.0.conv;
                if c.n == 0 {
                    return None;
                }
                match stat {
                    "gapPct" => Some(Val::Num(c.gap_pct)),
                    "p4" => Some(Val::Bool(c.p4)),
                    _ => None,
                }
            }
            "me" => {
                let m = self.0.me.as_ref()?;
                match stat {
                    "totalAbs" => Some(Val::Num(m.total_abs)),
                    "cells" => Some(Val::Pair(m.cells.clone())),
                    "ends" => Some(Val::Pair(m.ends.clone())),
                    "tld" => m.tld.clone().map(Val::Pair),
                    "cr" => m.cr.map(Val::Num),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fn cnt(&self, on: &str, stat: &str) -> Option<usize> {
        match on {
            "link" => self.0.link.cnt_of(stat),
            "turn" => self.0.turn.cnt_of(stat),
            "screen" => self.0.screen.cnt_of(stat),
            "jt" => match stat {
                "jt1" => Some(self.0.jt.jt1_n),
                "jt2" => Some(self.0.jt.jt2_n),
                _ => None,
            },
            _ => None,
        }
    }
    fn has_cnt(&self, on: &str) -> bool {
        matches!(on, "link" | "turn" | "screen" | "jt")
    }
}

struct Ev<'a>(&'a Evidence);

const COUNT_GROUPS: [&str; 8] = ["link", "turn", "band", "tag", "scatter", "rmse", "screen", "jt"];

fn pct(x: Option<f64>, d: usize) -> String {
    match x {
        Some(v) if v.is_finite() => fmt::pct(v, d),
        _ => "\u{2013}".to_string(),
    }
}

fn numf(x: f64, d: usize) -> String {
    if x.is_finite() { fmt::num(x, d) } else { "\u{2013}".to_string() }
}

/// One test judged (`evaluate`'s result): the value, how many sites were needed, whether it is met and the words for it.
#[derive(Clone, Debug)]
pub struct Eval {
    /// The test.
    pub t: Test,
    /// Observations the test is judged on.
    pub n: usize,
    /// The statistic.
    pub value: Option<Val>,
    /// Sites meeting a share criterion.
    pub got: Option<usize>,
    /// Sites needed.
    pub need: Option<usize>,
    /// The test is met; None when it has no verdict.
    pub ok: Option<bool>,
    /// "Pass", "Fail (11 of 13; 12 needed)", "Info", "Indicative", "No data", "N/A", "Requires clarification".
    pub status: String,
    /// The statistic in words ("8/10 · 80.0%").
    pub shown: String,
    /// The value rests on Google's predicted times.
    pub predicted: bool,
    /// %RMSE needs clarification rather than a pass (only for the rmse kind).
    pub clarify: Option<bool>,
    /// The verdict before it is made information only or indicative.
    pub verdict: Option<String>,
    /// Fewer than ten observations.
    pub indicative: bool,
}

fn show(t: &Test, ev: &Ev, v: &Val) -> String {
    let on = t.on;
    let n = ev.n_of(on);
    let vn = if let Val::Num(x) = v { Some(*x) } else { None };
    match t.kind {
        "share" => format!("{}/{} \u{b7} {}", ev.cnt(on, t.stat).unwrap_or(0), n, pct(vn, 1)),
        "pair" => match v {
            Val::Pair(p) => p.text.clone(),
            _ => "\u{2013}".to_string(),
        },
        "bool" => {
            let yes = match v {
                Val::Bool(b) => *b,
                Val::Num(x) => *x != 0.0 && !x.is_nan(),
                Val::Pair(_) => true,
            };
            if yes { "yes".to_string() } else { "no".to_string() }
        }
        _ => {
            if t.stat == "rmse" || t.stat == "gapPct" || t.stat == "totalAbs" {
                let x = vn.unwrap_or(f64::NAN);
                return if t.stat == "gapPct" { format!("{}%", numf(x, 3)) } else { format!("{}%", numf(x, 1)) };
            }
            if t.kind == "none" && ev.has_cnt(on) && ev.cnt(on, t.stat).is_some() {
                return format!("{}/{} \u{b7} {}", ev.cnt(on, t.stat).unwrap_or(0), n, pct(vn, 1));
            }
            numf(vn.unwrap_or(f64::NAN), 3)
        }
    }
}

/// `evaluate(t, ev)`: one test against the evidence.
pub fn evaluate(t: &Test, evidence: &Evidence) -> Eval {
    let ev = Ev(evidence);
    let n = ev.n_of(t.on);
    let val = ev.value(t.on, t.stat);
    let mut out = Eval {
        t: t.clone(),
        n,
        value: val.clone(),
        got: None,
        need: None,
        ok: None,
        status: String::new(),
        shown: "\u{2013}".to_string(),
        predicted: false,
        clarify: None,
        verdict: None,
        indicative: false,
    };
    if t.use_ == "not-expected" {
        out.status = "N/A".to_string();
        out.shown = match &val {
            None => "\u{2013}".to_string(),
            Some(v) => show(t, &ev, v),
        };
        return out;
    }
    if n == 0 || (val.is_none() && t.kind != "pair") {
        out.status = "No data".to_string();
        return out;
    }
    let val_ref = val.clone().unwrap_or(Val::Num(f64::NAN));
    out.shown = show(t, &ev, &val_ref);
    // Google typical times are predicted, not observed: reported, never deciding the status
    if t.on == "jt" && evidence.jt.predicted {
        out.shown.push_str(" (Google, predicted)");
        out.predicted = true;
    }
    let vnum = match &val_ref {
        Val::Num(x) => *x,
        Val::Bool(b) => *b as i32 as f64,
        Val::Pair(_) => f64::NAN,
    };
    match t.kind {
        "share" => {
            let got = ev.cnt(t.on, t.stat).unwrap_or(0);
            let nd = need(t.min.unwrap_or(f64::NAN), n);
            out.got = Some(got);
            out.need = Some(nd);
            out.ok = Some(got >= nd);
        }
        "min" => out.ok = Some(vnum > t.min.unwrap_or(f64::NAN)),
        "range" => out.ok = Some(vnum >= t.lo.unwrap_or(f64::NAN) && vnum <= t.hi.unwrap_or(f64::NAN)),
        "max" => out.ok = Some(vnum < t.max.unwrap_or(f64::NAN)),
        "rmse" => {
            let ok = vnum < t.max.unwrap_or(f64::NAN);
            out.ok = Some(ok);
            out.clarify = Some(!ok && vnum <= t.clarify.unwrap_or(f64::NAN));
        }
        "bool" => {
            out.ok = Some(match &val_ref {
                Val::Bool(b) => *b,
                Val::Num(x) => *x != 0.0 && !x.is_nan(),
                Val::Pair(_) => true,
            })
        }
        "pair" => match &val {
            Some(Val::Pair(p)) => out.ok = Some(p.ok),
            _ => {
                out.status = "No data".to_string();
                return out;
            }
        },
        _ => {}
    }
    if t.kind == "none" {
        out.status = "Info".to_string();
        return out;
    }
    let verdict = if out.ok == Some(true) {
        "Pass".to_string()
    } else if out.clarify == Some(true) {
        "Requires clarification".to_string()
    } else if t.kind == "share" {
        format!("Fail ({} of {}; {} needed)", out.got.unwrap_or(0), n, out.need.unwrap_or(0))
    } else {
        "Fail".to_string()
    };
    out.verdict = Some(verdict.clone());
    if out.predicted {
        out.status = "Info".to_string();
        return out;
    }
    if COUNT_GROUPS.contains(&t.group) && n < MIN_SITES {
        out.status = "Indicative".to_string();
        out.indicative = true;
        return out;
    }
    out.status = if t.use_ == "useful" {
        if out.clarify == Some(true) { "Info (requires clarification)".to_string() } else { "Info".to_string() }
    } else if evidence.info_only && COUNT_GROUPS.contains(&t.group) {
        "Info".to_string()
    } else {
        verdict
    };
    out
}

/// `expected(r)`: a row of the tests that decide the status: expected, with a verdict, on counts, not predicted.
pub fn expected(r: &Eval) -> bool {
    r.t.use_ == "expected"
        && r.t.kind != "none"
        && r.status != "No data"
        && r.status != "N/A"
        && COUNT_GROUPS.contains(&r.t.group)
        && !r.predicted
}

/// `gating(r)`: an expected row with enough observations to decide the status.
pub fn gating(r: &Eval) -> bool {
    expected(r) && !r.indicative
}

/// A category that fails a test the project's category passes (`assess().fails`).
#[derive(Clone, Debug, PartialEq)]
pub struct CatFail {
    /// Category.
    pub cat: char,
    /// The test's name.
    pub test: String,
    /// What was achieved.
    pub got: String,
    /// The target.
    pub target: String,
}

/// The assessment of a period (`Calibration.assess`): every test judged, the overall status and what it says.
#[derive(Clone, Debug)]
pub struct Assessment {
    /// Period.
    pub period: String,
    /// Criteria set.
    pub standard: String,
    /// TMDG category.
    pub category: char,
    /// The evidence.
    pub ev: Evidence,
    /// Each test judged.
    pub rows: Vec<Eval>,
    /// "none", "info", "indicative", "not base year", "pass" or "fail".
    pub status: String,
    /// True for pass, false for fail, None otherwise.
    pub pass: Option<bool>,
    /// Other categories the model also meets.
    pub also: Vec<char>,
    /// The first test each other category fails.
    pub fails: Vec<CatFail>,
    /// The status in a sentence.
    pub line: String,
}

/// The tests of a criteria set (`tests(std, cat)`).
pub fn tests_for(ctx: &Ctx, std: &str, cat: char) -> Vec<Test> {
    if std == "tmdg" || std.is_empty() {
        tmdg_tests(cat, ctx.cfg.prior_high())
    } else {
        preset_tests(std, ctx.cfg.prior_high())
    }
}

/// `assess(sc, period, o)`: the evidence, each test of the criteria set judged, and the overall status.
pub fn assess(ctx: &Ctx, sc: &Scenario, period: &str, res: Option<&Res>, o: &EvOpts) -> Assessment {
    let ev = evidence(ctx, sc, period, res, o);
    let std = o
        .standard
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::jsnum::value_to_js_string(&ctx.cfg.standard));
    let cat = o.category.unwrap_or_else(|| super::common::category(Some(ctx.s())));
    let rows: Vec<Eval> = tests_for(ctx, &std, cat).iter().map(|t| evaluate(t, &ev)).collect();
    let status = if !ev.has_res || ev.link.n == 0 {
        "none"
    } else if ev.info_only {
        "info"
    } else if rows.iter().any(|r| expected(r) && r.indicative && r.t.group == "link") {
        "indicative"
    } else if !ev.base_year {
        "not base year"
    } else if rows.iter().filter(|r| gating(r)).all(|r| r.ok == Some(true)) {
        "pass"
    } else {
        "fail"
    };
    let mut out = Assessment {
        period: period.to_string(),
        standard: std.clone(),
        category: cat,
        ev,
        rows,
        status: status.to_string(),
        pass: match status {
            "pass" => Some(true),
            "fail" => Some(false),
            _ => None,
        },
        also: Vec::new(),
        fails: Vec::new(),
        line: String::new(),
    };
    if std == "tmdg" && (status == "pass" || status == "fail") {
        for c2 in CATS {
            if c2 == cat {
                continue;
            }
            let r2: Vec<Eval> = tmdg_tests(c2, ctx.cfg.prior_high())
                .iter()
                .map(|t| evaluate(t, &out.ev))
                .filter(gating)
                .collect();
            match r2.iter().find(|r| r.ok != Some(true)) {
                None => out.also.push(c2),
                Some(bad) => {
                    let got = match bad.shown.rfind("\u{b7} ") {
                        Some(i) => bad.shown[i + "\u{b7} ".len()..].to_string(),
                        None => bad.shown.clone(),
                    };
                    let tt = target_text(&bad.t);
                    out.fails.push(CatFail {
                        cat: c2,
                        test: bad.t.label.clone(),
                        got,
                        target: tt.strip_prefix("> ").unwrap_or(&tt).to_string(),
                    });
                }
            }
        }
    }
    out.line = status_line(ctx, &out);
    out
}

/// `statusLine(a)`: the assessment in a sentence.
pub fn status_line(ctx: &Ctx, a: &Assessment) -> String {
    let ev = &a.ev;
    let lab = if a.standard == "tmdg" {
        format!("TMDG Category {}", a.category)
    } else {
        standard_name(&a.standard).unwrap_or("undefined").to_string()
    };
    let where_ = match ev.area.as_str() {
        "inside" => " inside the area of influence",
        "outside" => " outside the area of influence",
        _ => "",
    };
    let who = format!("{}{}, n={}", set_label(&ev.set), where_, ev.link.n);
    match a.status.as_str() {
        "none" => {
            return if !ev.has_res {
                format!("Run the model for the {} peak to compare it with the counts.", a.period)
            } else {
                format!("No {}{} match the network for the {} peak.", set_label(&ev.set), where_, a.period)
            }
        }
        "info" => {
            return format!(
                "For information only ({}): pass or fail is decided on the counts inside the area of influence, {}. Choose Area: Area of influence to see the status.",
                who, ev.core.label
            )
        }
        "indicative" => {
            return format!("Indicative only: {} (fewer than {}), so no pass or fail is given.", who, MIN_SITES)
        }
        "not base year" => {
            return format!(
                "Not assessed: the {} results are compared with base-year ({}) counts. Validate a base-year scenario; these figures are for information.",
                ev.year.map_or("null".to_string(), number_to_js_string),
                ctx.s().base_year.js_string()
            )
        }
        _ => {}
    }
    let failed: Vec<&str> = a.rows.iter().filter(|r| gating(r) && r.ok != Some(true)).map(|r| r.t.label.as_str()).collect();
    let mut s = format!(
        "Assessed against {}: {} ({}).",
        lab,
        if a.pass == Some(true) { "all expected tests pass".to_string() } else { format!("fails {}", failed.join(", ")) },
        who
    );
    if !a.also.is_empty() {
        s.push_str(&format!(" Also meets Category {}.", a.also.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")));
    }
    if !a.fails.is_empty() && a.standard == "tmdg" {
        s.push_str(&format!(
            " Fails: {}.",
            a.fails
                .iter()
                .take(3)
                .map(|f| format!("{} on {} ({} v {})", f.cat, f.test, f.got, f.target))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let mut ind: Vec<String> = Vec::new();
    for r in &a.rows {
        if expected(r) && r.indicative && r.t.group != "link" {
            let g = match r.t.group {
                "turn" => "turn",
                "screen" => "screenline",
                "jt" => "journey-time",
                x => x,
            };
            if !ind.iter().any(|x| x == g) {
                ind.push(g.to_string());
            }
        }
    }
    if !ind.is_empty() {
        s.push_str(&format!(
            " The {} tests have fewer than {} observations, so they are indicative and do not decide the status.",
            ind.join(" and "),
            MIN_SITES
        ));
    }
    let dr = &ev.screen.dropped;
    if !dr.is_empty() {
        s.push_str(&format!(
            " {} validation screenline total{} left out because {} in calibration or gateway counts.",
            dr.len(),
            if dr.len() == 1 { " is" } else { "s are" },
            if dr.len() == 1 { "it adds" } else { "they add" }
        ));
    }
    s.push_str(" Meeting criteria does not by itself show the model is fit for purpose (TAG M3.1 3.2.2).");
    s
}

/* ---------------- the model-status record ---------------- */

/// `reportScenario(P)`: the scenario the validation chapter reports: the base scenario when it has base-year results, else another scenario with base-year results, else the base scenario.
pub fn report_scenario(p: &Project) -> Option<&Scenario> {
    let runs = |s: &Scenario, f: &dyn Fn(f64) -> bool| -> bool {
        ["AM", "PM"].iter().any(|pd| match s.res.get() {
            Some(Value::Object(m)) => m.get(*pd).filter(|v| value_truthy(v)).is_some_and(|r| f(r.get("year").map_or(f64::NAN, crate::jsnum::value_to_number))),
            _ => false,
        })
    };
    let base_year = p.settings.base_year.clone();
    let is_base_yr = |s: &Scenario| runs(s, &|y| matches!(&base_year, Maybe::Set(crate::jsnum::JsNum::Number(b)) if *b == y));
    let b = p.scenarios.iter().find(|s| p.base.is(&s.id));
    if let Some(b) = b
        && is_base_yr(b)
    {
        return Some(b);
    }
    p.scenarios.iter().find(|s| is_base_yr(s)).or(b).or(p.scenarios.first())
}

/// What `recordValidation` did to the project's record for the period.
#[derive(Clone, Debug, PartialEq)]
pub enum Recorded {
    /// A record was written: pass, independent.
    Written { pass: bool, independent: bool },
    /// The record was removed.
    Dropped,
    /// Nothing changed (no record, none needed).
    Nothing,
}

fn drop_validation(p: &mut Project, period: &str) {
    if let Some(v) = p.validation.get_mut() {
        v.remove(period);
        if v.is_empty() {
            p.validation = Maybe::Missing;
        }
    }
}

/// `recordValidation(sc, period)`: records the base-year validation result for the model-status badge (TMDG, the project's category, independent validation counts inside the area of influence).
///
/// The record is removed when the base-year result is out of date or no longer gives a pass or fail (too few counts,
/// roles changed), so the badge never keeps an old "Validated". `now` is the ISO time written into the record.
pub fn record_validation(p: &mut Project, sc_id: &str, period: &str, now: &str) -> Recorded {
    let (old_scenario_is_this, res) = {
        let sc = match p.scenario(sc_id) {
            Some(s) => s,
            None => return Recorded::Nothing,
        };
        let old = p.validation.get().and_then(|v| v.get(period)).and_then(|v| v.get());
        let res = super::fit::stored_res(sc, period);
        (old.is_some_and(|o| o.extra.get("scenario").is_some_and(|s| s.as_str() == Some(sc_id))), res)
    };
    let drop_if_mine = |p: &mut Project| -> Recorded {
        if old_scenario_is_this {
            drop_validation(p, period);
            Recorded::Dropped
        } else {
            Recorded::Nothing
        }
    };
    let Some(res) = res else { return drop_if_mine(p) };
    let base = base_year_of(&res, &p.settings.base_year);
    if res.stale || !base {
        return drop_if_mine(p);
    }
    let (record, status_ok) = {
        let ctx = Ctx::new(p);
        let sc = p.scenario(sc_id).expect("scenario");
        let std = {
            let s = crate::jsnum::value_to_js_string(&ctx.cfg.standard);
            if value_truthy(&ctx.cfg.standard) { s } else { "tmdg".to_string() }
        };
        let a = assess(
            &ctx,
            sc,
            period,
            Some(&res),
            &EvOpts {
                set: Some("valid".into()),
                strict: true,
                area: Some("auto".into()),
                standard: Some(std.clone()),
                ..EvOpts::default()
            },
        );
        let ok = a.status == "pass" || a.status == "fail";
        let independent = a.ev.link.n >= MIN_SITES && !a.ev.fallback;
        let mut extra = Map::new();
        extra.insert("area".into(), Value::String(a.ev.area.clone()));
        extra.insert("n".into(), Value::from(a.ev.link.n as u64));
        extra.insert("scenario".into(), Value::String(sc_id.to_string()));
        let rec = ValidationRecord {
            pass: Maybe::Set(crate::jsnum::JsNum::Bool(a.pass == Some(true))),
            independent: Maybe::Set(crate::jsnum::JsNum::Bool(independent)),
            category: Maybe::Set(crate::jsnum::JsNum::Text(a.category.to_string())),
            standard: Maybe::Set(crate::jsnum::JsNum::Text(if a.standard.is_empty() { std } else { a.standard.clone() })),
            date: Maybe::Set(Value::String(now.to_string())),
            extra,
        };
        (rec, ok)
    };
    if !status_ok {
        // a.status is not pass or fail: the record goes
        drop_validation(p, period);
        return Recorded::Dropped;
    }
    let pass = record.pass.is_true();
    let independent = record.independent.is_true();
    if !p.validation.is_set() {
        p.validation = Maybe::Set(OrderedMap::new());
    }
    p.validation
        .get_mut()
        .expect("just made")
        .insert(period.to_string(), Maybe::Set(record));
    Recorded::Written { pass, independent }
}

/// `refreshValidation(P)`: records the validation of both periods for the scenario the validation chapter reports (after a change of category, roles or estimation), removing records for any other scenario; true when the records changed (apart from their dates).
pub fn refresh_validation(p: &mut Project, now: &str) -> bool {
    let strip = |p: &Project| -> String {
        let v = serde_json::to_value(&p.validation).unwrap_or(Value::Null);
        let mut v = v;
        fn strip_dates(v: &mut Value) {
            match v {
                Value::Object(m) => {
                    m.remove("date");
                    m.values_mut().for_each(strip_dates);
                }
                Value::Array(a) => a.iter_mut().for_each(strip_dates),
                _ => {}
            }
        }
        strip_dates(&mut v);
        v.to_string()
    };
    let before = strip(p);
    let sc_id = report_scenario(p).map(|s| s.id.clone());
    for pd in ["AM", "PM"] {
        let mine = p
            .validation
            .get()
            .and_then(|v| v.get(pd))
            .and_then(|v| v.get())
            .map(|v| v.extra.get("scenario").and_then(|s| s.as_str()).map(String::from));
        if let Some(scenario) = mine
            && sc_id.as_deref().is_none_or(|id| scenario.as_deref() != Some(id))
        {
            drop_validation(p, pd);
        }
        if let Some(id) = &sc_id {
            record_validation(p, id, pd, now);
        }
    }
    strip(p) != before
}

#[allow(dead_code)]
fn _keep(_: &HashSet<String>) {}
