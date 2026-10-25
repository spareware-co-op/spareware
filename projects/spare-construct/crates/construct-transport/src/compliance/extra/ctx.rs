//! What every part of Compliance+ shares (features/compliance-extra.js sections 1 to 4 and the helpers of section 4): the user's
//! assumptions, the project record, the dates, the regulatory context read from the Compliance tab, the authority of a road, and the
//! small model helpers (the do-minimum and option scenarios, roads, zones, stamping and the label of a road).

use crate::compliance::data::{m as model, x as xdata};
use crate::compliance::dates::{DAY, date_utc, days_from_civil, parts, today as nz_today};
use crate::compliance::jv::{Jx, NULL, V, csv, jobj, slice};
use crate::compliance::rx::rx;
use crate::compliance::{docs, net, record};
use crate::jsnum::truthy_number;

/// The label of a value Spare Construct chose itself (`ASSUMPTION`).
pub fn assumption() -> &'static str {
    xdata("ASSUMPTION").as_str().unwrap_or("")
}

/// `nn(v)`: a number the user entered, None for null, blank or not a number.
pub fn nn(v: &V) -> Option<f64> {
    if v.nullish() || v.eq_s("") || !v.n().is_finite() { None } else { Some(v.n()) }
}

/// `num(v, d)`: a number printed with d decimals, an en dash for null and non-finite values.
pub fn num(v: &V, d: usize) -> String {
    record::num(v, d)
}

/// `num(x, d)` for a number.
pub fn numf(x: f64, d: usize) -> String {
    record::numf(x, d)
}

/// `esc` of a loose value (null prints as nothing).
pub fn esc(v: &V) -> String {
    record::esc(v)
}

/// `esc` of text.
pub fn esc_s(s: &str) -> String {
    crate::util::esc(s)
}

/* =====================================================================================
   Assumptions the user can change (Settings > Compliance+)
   ===================================================================================== */

/// `A(P)`: the assumptions (`ASSUME` with the project's `settings.complianceExtra` entries over it where they are numbers).
pub fn assumptions(p: &V) -> V {
    let mut out = xdata("ASSUME").clone();
    let s = p.g("settings").g("complianceExtra");
    for k in xdata("ASSUME").keys() {
        let v = s.g(k);
        if v.entered() {
            out.set(k, V::Number(v.n()));
        }
    }
    out
}

/* =====================================================================================
   The project record (P.complianceExtra)
   ===================================================================================== */

/// `BLANK()`: the blank Compliance+ record.
pub fn blank() -> V {
    let arr = || V::Array(vec![]);
    jobj! {
        "v" => 1,
        "ctx" => jobj! { "pathway" => "", "lodgement" => "", "notificationDate" => "", "landInterest" => "", "sigEffect" => false, "reportDate" => "", "mbcmFull" => false },
        "lint" => jobj! { "text" => "", "ignore" => V::object(), "override" => V::object() },
        "precinct" => jobj! { "id" => "", "register" => arr(), "proposal" => jobj! { "ref" => "", "dwellings" => 0, "retail" => 0, "commercial" => 0, "community" => 0 }, "upgrades" => V::object(), "augier" => false, "amendment" => false, "ipLos" => "", "ipNode" => "", "custom" => arr() },
        "mitigation" => arr(), "conditions" => arr(), "triggers" => V::Null, "trigOpts" => jobj! { "scId" => "", "upgrades" => V::Null, "anop" => false },
        "options" => arr(),
        "ftaa" => jobj! { "nztaDesignation" => false, "kiwirailDesignation" => false, "invitation" => "", "approval" => "", "jobs" => "", "reliability" => "", "matrix" => V::object() },
        "planChange" => jobj! { "kind" => "", "notification" => "" },
        "speed" => jobj! { "cost" => 0, "consultationChanged" => false, "tool" => "Spare Construct sketch model", "cbds" => V::Null },
        "anop" => jobj! { "project" => "", "assessor" => "", "option" => "", "corridors" => arr() },
        "parking" => jobj! { "rows" => arr() },
        "subdivision" => jobj! { "lots" => 0, "accesses" => arr() },
        "policy" => jobj! { "rltp" => "", "fdsNote" => "", "npsiDoc" => "" },
        "gps" => jobj! { "routes" => arr(), "destinations" => arr(), "stages" => arr(), "dsiReduction" => V::Null, "collective" => "", "cost" => V::Null },
        "funding" => jobj! { "assets" => arr(), "assetRoads" => "", "polygon" => arr(), "levy" => 0, "approval" => "", "employment" => jobj! { "existing" => V::Null, "future" => V::Null }, "result" => V::Null, "shares" => V::Null },
        "tou" => jobj! { "polygon" => arr(), "kind" => "cordon", "light" => 2, "heavy" => 6, "period" => "AM", "cost" => 0, "result" => V::Null },
        "tsdg" => jobj! { "pct" => 10, "years" => 10, "result" => V::Null },
        "rltp" => jobj! { "objective" => "", "measures" => "", "duration" => "", "costPerYear" => "", "sources" => "" },
    }
}

/// `data(P)`: the project's Compliance+ record with the defaults filled in (the project is not changed).
pub fn data(p: &V) -> V {
    let mut c = if p.g("complianceExtra").is_obj() { p.g("complianceExtra").clone() } else { V::object() };
    record::fill(&mut c, &blank());
    c
}

/// `data(P)` for changing: the record, created and filled with the defaults first.
pub fn data_mut(p: &mut V) -> &mut V {
    if !p.g("complianceExtra").is_obj() {
        p.set("complianceExtra", V::object());
    }
    let b = blank();
    let c = p.get_mut("complianceExtra").expect("set above");
    record::fill(c, &b);
    c
}

/* =====================================================================================
   Dates, regulatory context and authority (read from the Compliance module)
   ===================================================================================== */

/// `isISO(s)`: text that starts with a YYYY-MM-DD day (and nothing shorter).
pub fn is_iso(v: &V) -> bool {
    match v {
        V::String(s) => s.chars().count() >= 10 && rx(r"^\d{4}-\d{2}-\d{2}$", "").test(&slice(s, 0, Some(10))),
        _ => false,
    }
}

/// `day(s)`: the YYYY-MM-DD at the start of text, or "".
pub fn day(v: &V) -> String {
    if is_iso(v) { slice(&v.s(), 0, Some(10)) } else { String::new() }
}

/// `dOf(s)`: midnight UTC of the day, in milliseconds (NaN when it is not a day).
pub fn d_of(s: &str) -> f64 {
    crate::compliance::dates::parse_day(s).unwrap_or(f64::NAN)
}

/// `today()`: today in New Zealand as YYYY-MM-DD.
pub fn today() -> String {
    nz_today(&V::Null)
}

const SHORT_MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `longDate(s)`: "21 Oct 2026", or an en dash when it is not a day.
pub fn long_date(v: &V) -> String {
    if !is_iso(v) {
        return "\u{2013}".to_string();
    }
    match parts(d_of(&day(v))) {
        Some((y, m0, d, _)) => format!("{} {} {}", d, SHORT_MONTHS[m0 as usize], y),
        None => "\u{2013}".to_string(),
    }
}

/// `longDate(s)` for text.
pub fn long_date_s(s: &str) -> String {
    long_date(&V::String(s.to_string()))
}

/// `addMonths(s, m)`: the same day m months on (the last day of a shorter month), as YYYY-MM-DD; None when s is not a day.
pub fn add_months(v: &V, m: f64) -> Option<String> {
    if !is_iso(v) {
        return None;
    }
    let (y, m0, d, _) = parts(d_of(&day(v)))?;
    // d.setUTCDate(1); d.setUTCMonth(d.getUTCMonth() + m); then the last day of that month caps the day
    let total = m0 as f64 + m;
    let ty = y as f64 + (total / 12.0).floor();
    let tm = total.rem_euclid(12.0);
    let t1 = date_utc(ty, tm, 1.0);
    let last = parts(date_utc(ty, tm + 1.0, 0.0)).map(|p| p.2).unwrap_or(28);
    let (ry, rm, rd, _) = parts(t1 + ((d.min(last) - 1) as f64) * DAY)?;
    Some(format!("{}-{:02}-{:02}", ry, rm + 1, rd))
}

/// `addWorkingDays(s, n, kind)`: the day n working days on (Legislation Act s13 days for "ftaa", RMA s2 days for "rma") and the basis.
/// `{ date, basis }`; the date is null when s is not a day.
pub fn add_working_days(v: &V, n: f64, kind: &str) -> V {
    let d = day(v);
    if let Some(r) = crate::compliance::add_working_days(&d, n, kind) {
        if is_iso(&V::String(r.clone())) {
            return jobj! { "date" => r, "basis" => if kind == "ftaa" { "Legislation Act 2019 s13 working days" } else { "RMA s2 working days" } };
        }
    }
    jobj! { "date" => V::Null, "basis" => "" }
}

/// The label of a pathway (`PATHS[pathway] || pathway || 'Not set'`).
pub fn path_label(pathway: &str) -> String {
    let t = xdata("PATHS");
    if t.has(pathway) && t.g(pathway).t() {
        t.g(pathway).s()
    } else if pathway.is_empty() {
        "Not set".into()
    } else {
        pathway.to_string()
    }
}

/// `context(P)`: what every check here reads about the regulatory context: pathway, label, kind (rc, pc, nor, ftaa, fund, speed, dc,
/// tou), lodgement, notification, asAt, side, post, landInterest, sigEffect, mbcmFull and the regime in words.
pub fn context(p: &V) -> V {
    let own = data(p);
    let own_ctx = own.g("ctx");
    let ext = record::state(p);
    let ext_ctx = ext.g("ctx");
    let reg = record::regime(p);
    let pick = |k: &str| -> V {
        let v = ext_ctx.g(k);
        if !v.nullish() && !v.eq_s("") { v.clone() } else { own_ctx.g(k).clone() }
    };
    let pathway = pick("pathway");
    let pathway = if pathway.t() { pathway.s() } else { String::new() };
    let lodgement = day(&pick("lodgement"));
    let notification = day(&pick("notificationDate"));
    let as_at = {
        let r = day(own_ctx.g("reportDate"));
        if !r.is_empty() {
            r
        } else if !lodgement.is_empty() {
            lodgement.clone()
        } else {
            today()
        }
    };
    let kind = if rx(r"^rc-|^precinct$", "").test(&pathway) {
        "rc"
    } else if rx(r"^pc-", "").test(&pathway) {
        "pc"
    } else if pathway == "nor" || pathway == "outline-plan" {
        "nor"
    } else if rx(r"^ftaa", "").test(&pathway) {
        "ftaa"
    } else if pathway == "nltf" {
        "fund"
    } else if pathway == "speed-limit" {
        "speed"
    } else if pathway == "dc-iff" {
        "dc"
    } else if pathway == "tou" {
        "tou"
    } else {
        ""
    };
    let transition = xdata("STD.dates.transition").s();
    let side: V = if reg.g("side").t() {
        reg.g("side").clone()
    } else if !lodgement.is_empty() {
        V::String(if lodgement.as_str() < transition.as_str() { "pre" } else { "transition" }.to_string())
    } else {
        V::Null
    };
    let key = if !lodgement.is_empty() { lodgement.clone() } else { as_at.clone() };
    let regime_text = if reg.t() {
        reg.g("text").clone()
    } else if lodgement.is_empty() {
        V::String("Lodgement date not set: regime not determined".into())
    } else if side.eq_s("pre") {
        V::String("RMA as in force immediately before 30 Sep 2026".into())
    } else {
        V::String("RMA as at lodgement incl. Planning Act Sch 12 Pt 2".into())
    };
    let land_interest = pick("landInterest");
    jobj! {
        "pathway" => pathway.as_str(), "label" => path_label(&pathway), "kind" => kind, "lodgement" => lodgement.as_str(), "notification" => notification, "asAt" => as_at.as_str(), "side" => side,
        "post" => key.as_str() >= transition.as_str(),
        "rmaProc" => ["rc", "nor", "ftaa"].contains(&kind) || pathway == "ftaa-consent",
        "norLike" => pathway == "nor" || pathway == "outline-plan" || pathway == "ftaa-nor", "ftaa" => kind == "ftaa",
        "landInterest" => if land_interest.t() { land_interest.s() } else { String::new() },
        "sigEffect" => own_ctx.g("sigEffect").t(), "mbcmFull" => own_ctx.g("mbcmFull").t(),
        "regimeText" => regime_text, "source" => "Compliance tab",
    }
}

/// `stamp(P)`: the version stamp line (the Compliance module's).
pub fn stamp(p: &V) -> String {
    record::stamp(p, &V::Null).g("text").s()
}

/// `isSH(L, P)`: the link is a state highway (the Compliance module's resolver).
pub fn is_sh(p: &V, l: &V) -> bool {
    crate::compliance::sh_of(p, l).g("sh").t()
}

/// `authorityOf(L, P, sc)`: who controls the road and who decides on it: `{ name, decision, nzta }`.
pub fn authority_of(p: &V, l: &V, sc: &V) -> V {
    let a = crate::compliance::link_authority(p, l, sc);
    jobj! { "name" => a.g("name"), "decision" => a.g("decision").g("who"), "nzta" => a.g("operator").eq_s("nzta") }
}

/* =====================================================================================
   Shared model helpers
   ===================================================================================== */

/// The modelled periods, in the order the checks list them (`PERIODS`).
pub const PERIODS: [&str; 3] = ["AM", "PM", "IP"];

/// `periodsOf(sc)`: the periods that have results that are not a year mismatch.
pub fn periods_of(sc: &V) -> Vec<&'static str> {
    PERIODS.into_iter().filter(|p| sc.g("res").g(p).t() && !sc.g("res").g(p).g("mismatch").t()).collect()
}

/// `resOf(sc, p)`: a scenario's results for a period.
pub fn res_of<'a>(sc: &'a V, p: &str) -> Option<&'a V> {
    let r = sc.g("res").g(p);
    if r.t() { Some(r) } else { None }
}

/// `scById(P, id)`.
pub fn sc_by_id<'a>(p: &'a V, id: &V) -> Option<&'a V> {
    p.g("scenarios").items().iter().find(|s| s.g("id").strict_eq(id))
}

/// `activeSc(P)`.
pub fn active_sc(p: &V) -> &V {
    sc_by_id(p, p.g("active")).or_else(|| p.g("scenarios").items().first()).unwrap_or(&NULL)
}

/// `baseSc(P)`.
pub fn base_sc(p: &V) -> &V {
    sc_by_id(p, p.g("base")).or_else(|| p.g("scenarios").items().first()).unwrap_or(&NULL)
}

/// `roleSc(P, role)`: the first scenario with a role.
pub fn role_sc<'a>(p: &'a V, role: &str) -> Option<&'a V> {
    p.g("scenarios").items().iter().find(|s| s.g("role").eq_s(role))
}

/// The scenarios a check compares (`pair(P)`): the do-minimum, the option (the active scenario) and the mitigation scenario.
pub struct Pair<'a> {
    /// The do-minimum (the baseline when the base scenario is the option itself).
    pub base: &'a V,
    /// The option: the active scenario.
    pub opt: &'a V,
    /// The scenario with the mitigation role, if any.
    pub mit: Option<&'a V>,
    /// The do-minimum and the option are the same scenario.
    pub same: bool,
}

/// `pair(P)`.
pub fn pair(p: &V) -> Pair<'_> {
    let opt = active_sc(p);
    let mit = role_sc(p, "mitigation");
    let mut base = base_sc(p);
    if std::ptr::eq(base, opt) {
        if let Some(b) = role_sc(p, "baseline") {
            if !std::ptr::eq(b, opt) {
                base = b;
            }
        }
    }
    Pair { base, opt, mit, same: std::ptr::eq(base, opt) }
}

/// `nodeName(sc, id)`: the name of a junction.
pub fn node_name(sc: &V, id: &str) -> String {
    net::junction_name(sc, id)
}

/// `isDev(z)`: a zone that is a development (`Engine.zoneClass`).
pub fn is_dev(z: &V) -> bool {
    z.t() && !z.g("kind").eq_s("gateway") && net::zone_class(z) == "development"
}

/// `devZones(sc)`.
pub fn dev_zones(sc: &V) -> Vec<&V> {
    sc.g("zones").vals().into_iter().filter(|z| is_dev(z)).collect()
}

/// `luOf(z, k)`: the quantity of a land use in a zone.
pub fn lu_of(z: &V, k: &str) -> f64 {
    net::lu_of(z, k)
}

/// `stageOf(z, sc)`.
pub fn stage_of(z: &V, sc: &V) -> f64 {
    net::stage_of(z, sc)
}

/// `dwellings(z, sc)`: the dwellings in a zone at the scenario's build-out.
pub fn dwellings(z: &V, sc: &V) -> f64 {
    (lu_of(z, "residential") + lu_of(z, "apartment") + lu_of(z, "retirement")) * stage_of(z, sc)
}

/// `sumDw(sc)`: the dwellings in the development zones.
pub fn sum_dw(sc: &V) -> f64 {
    dev_zones(sc).iter().fold(0.0, |s, z| s + dwellings(z, sc))
}

/// The land uses counted as gross floor area (`GFA_KEYS`).
pub const GFA_KEYS: [&str; 7] = ["retail", "supermarket", "fastfood", "office", "medical", "industrial", "warehouse"];

/// `sumGfa(sc)`: the floor area in the development zones at the scenario's build-out.
pub fn sum_gfa(sc: &V) -> f64 {
    dev_zones(sc).iter().fold(0.0, |s, z| s + GFA_KEYS.iter().fold(0.0, |t, k| t + lu_of(z, k)) * stage_of(z, sc))
}

/// `cloneSc(sc, over)`: a copy of a scenario without results, with fields replaced.
pub fn clone_sc(sc: &V, over: &[(&str, V)]) -> V {
    let mut c = sc.clone();
    c.set("res", V::object());
    for (k, v) in over {
        c.set(*k, v.clone());
    }
    c
}

/// `arcLink(key)`: the link id of an arc key ("l1:ab" gives "l1").
pub fn arc_link(key: &str) -> String {
    match key.rfind(':') {
        Some(i) => key[..i].to_string(),
        None => key.chars().take(key.chars().count().saturating_sub(1)).collect(),
    }
}

/// `CLASSES[cls] || CLASSES.local`.
pub fn class_of(cls: &V) -> &'static V {
    let t = model("CLASSES");
    let k = cls.s();
    if cls.t() && t.has(&k) { t.g(&k) } else { t.g("local") }
}

/// `linkName(L)`: the road's name, or "Unnamed arterial road" and so on.
pub fn link_name(l: &V) -> String {
    if l.g("name").t() {
        return l.g("name").s();
    }
    if l.t() && model("CLASSES").has(&l.g("cls").s()) {
        return format!("Unnamed {} road", model("CLASSES").g(&l.g("cls").s()).g("label").s().to_lowercase());
    }
    "Unnamed road".to_string()
}

/// `posted(L)`: the posted speed limit of a road (km/h): its legal limit, the NSLR limit, its link speed, or its class's speed.
pub fn posted(l: &V) -> f64 {
    let a = l.g("speedLimit").n();
    if truthy_number(a) {
        return a;
    }
    if l.g("nslr").t() {
        let b = l.g("nslr").g("v").n();
        if truthy_number(b) {
            return b;
        }
    }
    let c = l.g("speed").n();
    if truthy_number(c) {
        return c;
    }
    class_of(l.g("cls")).g("speed").n()
}

/// `pct(a, b)`: the change from b to a as a percentage of b, None when b is not above 0.
pub fn pct(a: f64, b: f64) -> Option<f64> {
    if b > 0.0 && a.is_finite() { Some((a - b) / b * 100.0) } else { None }
}

/// `LETTERS.indexOf(a) > LETTERS.indexOf(b)`: level of service a is worse than b.
pub fn los_worse(a: &str, b: &str) -> bool {
    let idx = |x: &str| ["A", "B", "C", "D", "E", "F"].iter().position(|l| *l == x).map_or(-1, |i| i as i32);
    idx(a) > idx(b)
}

/// `stamped(text, P, tool)`: a CSV with the version stamp and the disclaimers in footer comment rows (and D12 for a hand-over tool).
pub fn stamped(text: &str, p: &V, tool: Option<&str>) -> String {
    docs::stamp_csv(text, p, tool)
}

/// `csv(rows)`.
pub fn csv_text(rows: &[Vec<V>]) -> String {
    csv(rows)
}

/// `disclaimer(id, P, tool)`.
pub fn disclaimer(id: &str, p: &V, tool: Option<&str>) -> String {
    docs::disclaimer(id, Some(p), tool)
}

/// `disc(id, P, tool)`: the disclaimer as a paragraph of a report.
pub fn disc(id: &str, p: &V, tool: Option<&str>) -> String {
    format!("<p class=\"note disclaimer\" data-disclaimer=\"{id}\"><b>{id}.</b> {}</p>", esc_s(&disclaimer(id, p, tool)))
}

/// `verify(s)`: the "verify" tag for a source that has not been checked against the document.
pub fn verify(s: &V) -> String {
    if s.g("verified").strict_eq(&V::Bool(false)) { " <span class=\"pill warn\" title=\"Not yet checked against the source document\">verify</span>".to_string() } else { String::new() }
}

/// `link(url, t)`: a link that opens in a new tab.
pub fn link(url: &str, t: &str) -> String {
    format!("<a href=\"{}\" target=\"_blank\" rel=\"noopener\">{}</a>", esc_s(url), esc_s(t))
}

/// `assumed(t)`: a value with the assumption label.
pub fn assumed(t: &str) -> String {
    format!("{t} <span class=\"note\">({})</span>", assumption())
}

/// The day number (days since 1970) of a YYYY-MM-DD day, for ordering.
pub fn day_number(s: &str) -> Option<i64> {
    let (y, m, d) = (s.get(0..4)?.parse::<i64>().ok()?, s.get(5..7)?.parse::<u32>().ok()?, s.get(8..10)?.parse::<u32>().ok()?);
    Some(days_from_civil(y, m, d))
}
