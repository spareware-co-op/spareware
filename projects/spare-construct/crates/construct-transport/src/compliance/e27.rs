//! E27 trigger screening, the ITA scope and proportionality (packs/nz/compliance.js sections 6 and 7).

use super::access::access_geo;
use super::authority::{authorities, site_info, site_point};
use super::data::{c as table, m as model, re_of};
use super::dates::today;
use super::engine;
use super::geom::{bounds_of, flat_at, to_line};
use super::jv::{Jx, V, jarr, jobj, ns, st, strs};
use super::net;
use super::record::{active_sc, assumption, base_sc, in_auckland, num, numf, pathway_entry, regime, res_of, settings, state};
use super::rx::rx;
use crate::jsnum::{math_max, math_min, truthy_number};
use crate::util;

fn or0(v: &V) -> f64 {
    let x = v.n();
    if truthy_number(x) { x } else { 0.0 }
}

fn r1(x: f64) -> f64 {
    util::r1(x)
}

fn e27_map(k: &str) -> bool {
    table("E27.map").g(k).t()
}

fn lu_label(k: &str) -> String {
    let l = model("LANDUSES").g(k);
    if l.t() { l.g("label").s() } else { k.to_string() }
}

/// `quantities(P, sc)`: the proposal quantities from the development zones (full yield) with the author's overrides, and the busiest-hour trips from the rates.
pub fn quantities(p: &V, sc: &V) -> V {
    let c = state(p);
    let s = p.g("settings");
    let zones = net::dev_zones(sc);
    let mut lu: Vec<(String, f64)> = Vec::new();
    for z in &zones {
        for (k, v) in z.g("lu").entries() {
            let add = or0(v);
            match lu.iter_mut().find(|e| e.0 == k) {
                Some(e) => e.1 = if truthy_number(e.1) { e.1 } else { 0.0 } + add,
                None => lu.push((k.to_string(), 0.0 + add)),
            }
        }
    }
    let l = |k: &str| lu.iter().find(|e| e.0 == k).map_or(0.0, |e| if truthy_number(e.1) { e.1 } else { 0.0 });
    let sub = c.g("ctx").g("pathway").eq_s("rc-subdivision");
    let dt = c.g("site").g("driveThrough").t();
    let sec = c.g("site").g("school").eq_s("secondary");
    let mut q = jobj! { "dwellings" => if sub { 0.0 } else { l("residential") + l("apartment") }, "ird" => l("retirement"), "visitor" => l("hotel"),
        "subdivision" => if sub { l("residential") + l("apartment") } else { 0.0 },
        "primary" => if sec { 0.0 } else { l("school") }, "secondary" => if sec { l("school") } else { 0.0 }, "tertiary" => l("tertiary"), "office" => l("office"),
        "retail" => l("retail") + l("supermarket") + if dt { 0.0 } else { l("fastfood") }, "retailDT" => if dt { l("fastfood") } else { 0.0 }, "warehouse" => l("warehouse"), "industrial" => l("industrial") };
    let mut src = V::object();
    for k in q.keys().into_iter().map(String::from).collect::<Vec<_>>() {
        src.set(k, st("development zones"));
    }
    for (k, v) in c.g("q").entries() {
        if v.entered() && q.has(k) {
            q.set(k, V::Number(v.n()));
            src.set(k, st("entered"));
        }
    }
    let mut trips = V::object();
    let mut unlisted = V::object();
    let (mut direct, mut direct_only) = (false, false);
    for per in ["AM", "PM"] {
        let (mut t, mut u) = (0.0, 0.0);
        for z in &zones {
            let red = 1.0 - util::clamp(or0(z.g("reduction")), 0.0, 100.0) / 100.0;
            let (mut zl, mut zu) = (0.0, 0.0);
            for (k, v) in z.g("lu").entries() {
                let r = s.g("rates").g(k).g(per);
                let lm = model("LANDUSES").g(k);
                if !r.t() || !lm.t() {
                    continue;
                }
                let x = or0(v) / lm.g("per").n() * r.g("rate").n();
                zl += x;
                if !e27_map(k) {
                    zu += x;
                }
            }
            let ex = or0(z.g("extra").g(&format!("{per}in"))) + or0(z.g("extra").g(&format!("{per}out")));
            if ex != 0.0 {
                direct = true;
                if !(zl > 0.0) {
                    direct_only = true;
                }
            }
            let zt = math_max(0.0, zl + ex);
            let share = if zl > 0.0 { zu / zl } else { 1.0 };
            t += zt * red;
            u += zt * share * red;
        }
        trips.set(per, V::Number(r1(t)));
        unlisted.set(per, V::Number(r1(u)));
    }
    let peak = trips.vals().iter().fold(0.0, |a, v| math_max(a, v.n()));
    let peak_p = trips.entries().iter().find(|(_, v)| v.n() == peak).map(|(k, _)| st(*k)).unwrap_or(V::Null);
    let unlisted_peak = unlisted.vals().iter().fold(0.0, |a, v| math_max(a, v.n()));
    let mut unlisted_uses: Vec<String> = lu.iter().filter(|e| e.1 > 0.0 && !e27_map(&e.0)).map(|e| lu_label(&e.0)).collect();
    if direct_only {
        unlisted_uses.push("Trips entered directly (no land use)".into());
    }
    let mut lu_v = V::object();
    for (k, v) in &lu {
        lu_v.set(k.as_str(), V::Number(*v));
    }
    jobj! { "q" => q, "src" => src, "lu" => lu_v, "zones" => zones.len(), "trips" => trips, "peak" => peak, "peakPeriod" => peak_p, "unlistedPeak" => unlisted_peak,
        "unlistedUses" => strs(unlisted_uses), "direct" => direct,
        "basis" => format!("busiest modelled hour (AM or PM) from the trip rates in Settings{}, after trip reductions, at full build-out; indicative unless the rates are replaced", if direct { " plus trips entered directly" } else { "" }) }
}

/// `exemptionOf(zoneName, cfoc, status)`: the E27.6.1(2) exemption that applies, or null.
pub fn exemption_of(zone_name: &V, cfoc: bool, status: &str) -> V {
    let zn = if zone_name.t() { zone_name.s() } else { String::new() };
    if zone_name.t() && re_of(table("E27.exemptZone")).test(&zn) {
        return jobj! { "rule" => "E27.6.1(2)(a)", "text" => format!("Exempt E27.6.1(2)(a): {zn}") };
    }
    if cfoc {
        return jobj! { "rule" => "E27.6.1(2)(a)", "text" => "Exempt E27.6.1(2)(a): Centre Fringe Office Control" };
    }
    if zone_name.t() && re_of(table("E27.openSpace")).test(&zn) && status == "P" {
        return jobj! { "rule" => "E27.6.1(2)(c)", "text" => "Exempt E27.6.1(2)(c): permitted in an H7 open space zone" };
    }
    V::Null
}

/// `e27(P)` for the active scenario.
pub fn e27(p: &V) -> V {
    e27_sc(p, active_sc(p))
}

fn set_cons(rows: &mut [V], f: &dyn Fn(&V) -> String) {
    for r in rows.iter_mut() {
        let t = f(r);
        r.set("consequence", st(t));
    }
}

/// `e27(P, sc)`: the Auckland Unitary Plan E27 trigger screening.
pub fn e27_sc(p: &V, sc: &V) -> V {
    let c = state(p);
    let site = site_info(p);
    let qn = quantities(p, sc);
    let status = if c.g("ctx").g("activityStatus").t() { c.g("ctx").g("activityStatus").s() } else { String::new() };
    let e = table("E27");
    let mut rows: Vec<V> = e
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            let x = qn.g("q").g(&r.g("key").s());
            let q = if x.t() { x.n() } else { 0.0 };
            let mut o = r.clone();
            o.set("q", V::Number(q));
            if qn.g("src").has(&r.g("key").s()) {
                o.set("src", qn.g("src").g(&r.g("key").s()).clone());
            }
            o.set("exceeded", V::Bool(q > r.g("thr").n()));
            o
        })
        .collect();
    let mut applies = true;
    let mut exempt = V::Null;
    let mut redirect = V::Null;
    if site.g("transportPrecinct").t() {
        let pr = site.g("transportPrecinct");
        applies = false;
        redirect = jobj! { "precinct" => pr, "text" => format!("E27.6.1 does not apply ({}): use the precinct staging tracker", pr.g("rule").s()), "cite" => format!("E27.4; {}", pr.g("rule").s()) };
        set_cons(&mut rows, &|_| "Not applied (precinct)".into());
    } else {
        exempt = exemption_of(site.g("zone").g("name"), site.g("cfoc").t(), &status);
        if !exempt.t() && c.g("site").g("zoneTA").t() {
            exempt = jobj! { "rule" => "E27.6.1(2)(d)", "text" => "Exempt E27.6.1(2)(d): the zone or precinct rules already require trip generation effects to be assessed" };
        }
        if exempt.t() {
            applies = false;
            set_cons(&mut rows, &|_| "Not applied (exempt)".into());
        }
    }
    let hit: Vec<V> = rows.iter().filter(|r| r.g("exceeded").t()).cloned().collect();
    let unlisted = c.g("site").g("unlisted").t() || qn.g("unlistedUses").jlen() > 0;
    let peak_u = if c.g("site").g("unlisted").t() { qn.g("peak").n() } else { qn.g("unlistedPeak").n() };
    let thr = e.g("catchAll").g("thr").n();
    let mut ca = jobj! { "id" => e.g("catchAll").g("id"), "thr" => thr, "peak" => r1(peak_u), "unlisted" => unlisted, "status" => status.as_str(), "applies" => false, "exceeded" => false, "why" => "" };
    let why: String;
    if !applies {
        why = format!("Not applied ({}).", if redirect.t() { "precinct" } else { "exempt" });
    } else if !unlisted {
        why = "Every activity is listed in Table E27.6.1.1.".into();
    } else if status == "P" {
        why = "Does not apply to activities permitted in the zone.".into();
    } else if status != "C" && status != "RD" {
        why = if !status.is_empty() {
            let a = table("ACTIVITY").g(&status);
            format!("Applies only to controlled or restricted discretionary activities (this one is {}).", if a.t() { a.s() } else { status.clone() }.to_lowercase())
        } else {
            "Set the activity status to test it (it applies only to controlled or restricted discretionary activities).".into()
        };
    } else if c.g("site").g("zoneTA").t() {
        why = "The zone already requires a transport assessment.".into();
    } else {
        let ex = peak_u > thr;
        ca.set("applies", V::Bool(true));
        ca.set("exceeded", V::Bool(ex));
        why = if ex { format!("Exceeded: {} veh/h in the busiest modelled hour (more than 100). Restricted discretionary (A3).", numf(peak_u, 0)) } else { format!("Not exceeded: {} veh/h in the busiest modelled hour.", numf(peak_u, 0)) };
    }
    ca.set("why", st(why));
    let ca_ex = ca.g("exceeded").t();
    let mut tier = V::Null;
    let mut exceeded_text = V::Null;
    let mut consequence = V::Null;
    let mut supplementary = false;
    let mut notes: Vec<String> = Vec::new();
    let all_t1 = hit.iter().all(|r| r.g("tier").n() == 1.0);
    if applies && (!hit.is_empty() || ca_ex) {
        let t = if !ca_ex && all_t1 { 1 } else { 2 };
        tier = V::Number(t as f64);
        set_cons(&mut rows, &|r| if r.g("exceeded").t() { format!("RD (Table E27.4.1 (A3)); {}", if t == 1 { "E27.8.2(3A)" } else { "E27.8.2(3)" }) } else { "Not exceeded".into() });
        let parts: Vec<&str> = [if !hit.is_empty() { "Table E27.6.1.1" } else { "" }, if ca_ex { "E27.6.1(1)(b)" } else { "" }].into_iter().filter(|s| !s.is_empty()).collect();
        exceeded_text = st(format!("{} exceeded", parts.join(" and ")));
        let vic = settings(p).g("vicinity").clone();
        consequence = st(if t == 1 {
            format!("Restricted discretionary. Tier 1: {}. Study area defaults to a {} m walking catchment ({}).", e.g("tier1Scope").s(), vic.s(), e.g("tier1Area").g("basis").s())
        } else {
            format!("Restricted discretionary. Tier 2: {}.{}", e.g("tier2Scope").s(), if ca_ex && !hit.is_empty() && all_t1 { " The E27.6.1(1)(b) catch-all is not a tier 1 row, so the all-mode network assessment applies." } else { "" })
        });
        supplementary = t == 1;
    } else if applies {
        set_cons(&mut rows, &|_| "Not exceeded".into());
    }
    if applies && (!hit.is_empty() || ca_ex) {
        notes.push("E27.9(2): the council may require a travel plan.".into());
        notes.push("E27.9(4): the council may require a transport assessment prepared by a suitably qualified transport planner or traffic engineer.".into());
    }
    if (status == "NC" || status == "U") && qn.g("peak").n() >= 100.0 {
        notes.push("E27.9(5): may need an ITA under the AT ITA Guidelines in force (a non-complying or unprovided activity generating 100 veh/h or more in any hour).".into());
        notes.push(format!("E27.9(5) footnote: the AUP names the Auckland Transport ITA Guidelines (2025 Version 2 is the version in force); {}", table("AUTH.footnote").s()));
    }
    notes.push(format!("{}.", e.g("s104C").s()));
    let mut out = jobj! { "version" => e.g("version"), "wording" => e.g("wording"), "source" => e.g("source"), "rows" => V::Array(rows), "quantities" => qn.clone(), "site" => site.clone(),
        "applies" => applies, "exempt" => exempt, "redirect" => redirect, "tier" => tier.clone(), "notes" => V::Null, "s104C" => e.g("s104C"), "supplementary" => supplementary };
    out.set("catchAll", ca);
    if tier.t() {
        out.set("exceededText", exceeded_text);
        out.set("consequence", consequence);
    }
    // PC120 overlay: both zonings, and whether the exemption differs
    if site.g("pc120").g("zone").t() {
        let op = site.g("zone").g("name");
        let pr = site.g("pc120").g("zone");
        let ex_op = exemption_of(op, site.g("cfoc").t(), &status).t();
        let ex_pr = exemption_of(pr, site.g("cfoc").t(), &status).t();
        let op_s = if op.t() { op.s() } else { String::new() };
        let text = if ex_op != ex_pr {
            format!("PC120 changes the E27.6.1(2) position: operative {} is {}, PC120 proposed {} would be {}. Test both until PC120 is decided (due by June 2027).", if op.t() { op_s.clone() } else { "?".into() }, if ex_op { "exempt" } else { "not exempt" }, pr.s(), if ex_pr { "exempt" } else { "not exempt" })
        } else if op.t() && op_s != pr.s() {
            format!("PC120 proposes {} (operative: {}). The E27.6.1(2) exemption is the same under both.", pr.s(), op_s)
        } else {
            format!("PC120 shows {}; no zoning change at the site.", pr.s())
        };
        out.set("pc120", jobj! { "operative" => if op.t() { op.clone() } else { st("not known") }, "proposed" => pr, "differs" => op.t() && op_s != pr.s(), "exemptDiffers" => ex_op != ex_pr, "text" => text, "status" => e.g("pc120Status") });
    }
    // E27.2 / E27.3 policy matrix
    let zone_name = site.g("zone").g("name");
    let exempt_zone = exemption_of(zone_name, site.g("cfoc").t(), "").t() && !(site.g("zone").t() && re_of(e.g("openSpace")).test(&zone_name.sn()));
    let major = out.g("tier").n() == 2.0 || qn.g("peak").n() >= 100.0;
    let matrix: Vec<V> = table("POLICY")
        .items()
        .iter()
        .map(|pp| {
            let when = pp.g("when").sn();
            let ap = if when == "outside" { !exempt_zone } else if when == "major" { status == "D" && major } else { true };
            let why = if when == "outside" {
                if exempt_zone { "Site is in an excluded zone or control" } else { "Site is outside the excluded zones" }
            } else if when == "major" {
                if status == "D" { if major { "Discretionary and major (Spare Construct reading: E27 tier 2 or 100 veh/h or more)" } else { "Discretionary, not major" } } else { "Applies to discretionary proposals" }
            } else {
                "Applies"
            };
            let mut o = pp.clone();
            o.set("applies", V::Bool(ap));
            o.set("why", st(why));
            o
        })
        .collect();
    out.set("notes", strs(notes));
    out.set("matrix", V::Array(matrix));
    out
}

/// `linkChange(P, sc)`: the largest percentage change on a link between the base scenario and the active one (`{ p, name, link, period, included, cls, extent }`), or null.
pub fn link_change(p: &V, sc: &V) -> V {
    let base = base_sc(p);
    if std::ptr::eq(sc, base) || sc.g("id").strict_eq(base.g("id")) {
        return V::Null;
    }
    let mut best = V::Null;
    for per in ["AM", "PM"] {
        let (Some(w), Some(wo)) = (res_of(sc, per), res_of(base, per)) else { continue };
        let x = engine::extent_test(sc, wo, w, Some(base));
        let floor = if x.g("floor").t() { x.g("floor").n() } else { 20.0 };
        for r in x.g("rows").items() {
            if !r.g("p").nullish() && r.g("d").n() >= floor && (!best.t() || r.g("p").n() > best.g("p").n()) {
                best = jobj! { "p" => r.g("p"), "name" => if r.g("name").t() { r.g("name").clone() } else { st("unnamed road") }, "link" => r.g("link"), "period" => per, "included" => r.g("included"), "cls" => r.g("cls"), "extent" => x.clone() };
            }
        }
        if x.t() && best.t() && best.g("period").eq_s(per) {
            best.set("included", V::Number(x.g("links").jlen() as f64));
        }
    }
    best
}

/// `shInvolved(P, sc)`: whether the proposal involves a state highway and why: `{ involved, why, inArea, links }`.
pub fn sh_involved(p: &V, sc: &V) -> V {
    let c = state(p);
    let sh_ids: Vec<String> = sc.g("links").vals().into_iter().filter(|l| super::authority::sh_of(p, l).g("sh").t()).map(|l| l.g("id").s()).collect();
    let mut why: Vec<String> = Vec::new();
    for a in c.g("access").items() {
        let g = access_geo(p, sc, a);
        if g.t() && g.g("link").t() && sh_ids.contains(&g.g("link").g("id").s()) {
            why.push(format!("access {} is on a state highway", a.g("name").sn()).replace("  ", " "));
        }
    }
    for per in ["AM", "PM"] {
        let Some(r) = res_of(sc, per) else { continue };
        if !r.g("cls").t() {
            continue;
        }
        let mut dv = 0.0;
        for (k, v) in r.g("cls").g("arcs").entries() {
            let id = &k[..k.rfind(':').unwrap_or(k.len())];
            if sh_ids.iter().any(|s| s == id) {
                dv = math_max(dv, or0(v.ix(2)));
            }
        }
        if dv >= 1.0 {
            why.push(format!("development traffic uses a state highway (up to {} veh/h in the {} peak)", numf(dv, 0), per));
            break;
        }
    }
    if let Some(pt) = site_point(p, sc) {
        if !sh_ids.is_empty() {
            let lat = pt.g("lat").n();
            let f = flat_at(lat);
            let mut d = f64::INFINITY;
            for id in &sh_ids {
                if let Some(g) = net::geom(sc, sc.g("links").g(id)) {
                    d = math_min(d, to_line(&[lat, pt.g("lng").n()], &g, &f).d);
                }
            }
            let nm = table("SH.noiseM").n();
            if d <= nm {
                why.push(format!("the site is within {} m of a state highway ({} m)", ns(nm), numf(d, 0)));
            }
        }
    }
    jobj! { "involved" => !why.is_empty(), "why" => strs(why), "inArea" => !sh_ids.is_empty(), "links" => strs(sh_ids) }
}

/// `table1(Q)`: the NZTA G00907 Table 1 ITA Lite and Full test for the proposal quantities.
pub fn table1(qn: &V) -> V {
    let q = qn.g("q");
    let g = |k: &str| q.g(k).n();
    let res = g("dwellings") + g("ird") + g("visitor") + g("subdivision");
    let edu = g("primary") + g("secondary") + g("tertiary");
    let amt = |k: &str| -> f64 {
        match k {
            "res" => res,
            "edu" => edu,
            "office" => g("office"),
            "retail" => g("retail"),
            "warehouse" => g("warehouse"),
            "industrial" => g("industrial"),
            "other" => qn.g("unlistedPeak").n(),
            _ => 0.0,
        }
    };
    let t1 = table("NZTA_T1");
    let rows: Vec<V> = t1
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            let a = amt(&r.g("key").s());
            let a = if truthy_number(a) { a } else { 0.0 };
            let mut o = r.clone();
            o.set("q", V::Number(r1(a)));
            o.set("lShare", V::Number(a / r.g("lite").n()));
            o.set("fShare", V::Number(a / r.g("full").n()));
            o
        })
        .collect();
    let l = rows.iter().fold(0.0, |s, r| s + r.g("lShare").n());
    let f = rows.iter().fold(0.0, |s, r| s + r.g("fShare").n());
    let dt = q.g("retailDT").n() > 0.0;
    let used: Vec<&V> = rows.iter().filter(|r| r.g("q").n() > 0.0).collect();
    let res1 = rows.iter().find(|r| r.g("key").eq_s("res")).cloned().unwrap_or(V::Null);
    let (result, text);
    if dt {
        result = "Full";
        text = format!("{}: the proposal has a drive-through component.", t1.g("driveThrough").s());
    } else if f >= 1.0 {
        result = "Full";
        text = if used.len() == 1 { format!("ITA Full threshold ({}) reached: {} {}.", numf(used[0].g("full").n(), 0), numf(used[0].g("q").n(), 0), used[0].g("unit").s()) } else { format!("ITA Full threshold reached in aggregate ({}% of Full, mixed use assessed proportionately).", numf(f * 100.0, 0)) };
    } else if l >= 1.0 {
        result = "Lite";
        text = if used.len() == 1 { format!("ITA Lite threshold ({}) reached: {} {}.", numf(used[0].g("lite").n(), 0), numf(used[0].g("q").n(), 0), used[0].g("unit").s()) } else { format!("ITA Lite threshold reached in aggregate ({}% of Lite, mixed use assessed proportionately).", numf(l * 100.0, 0)) };
    } else {
        result = "None";
        text = format!("Below the Table 1 ITA Lite thresholds ({}% of Lite in aggregate).", numf(l * 100.0, 0));
    }
    jobj! { "rows" => V::Array(rows), "lite" => util::r3(l), "full" => util::r3(f), "result" => result, "text" => text, "driveThrough" => dt, "res" => res1, "source" => t1.g("source"), "order" => t1.g("order"), "label" => t1.g("label") }
}

/// `depthOf(result, consistent, context)`: the G00907 Table 2 depth of assessment.
pub fn depth_of(result: &str, consistent: bool, context: &str) -> V {
    if result == "None" {
        return jobj! { "depth" => V::Null, "text" => "No Table 2 depth (below Table 1): the district plan and council guidance still apply." };
    }
    let urbanish = context == "urban" || context == "periurban";
    let d = if result == "Lite" {
        if consistent { if context == "periurban" { "Targeted" } else { "Basic" } } else if context == "rural" { "Targeted" } else { "Moderate" }
    } else if !consistent && urbanish {
        "Comprehensive"
    } else {
        "Moderate"
    };
    let cx = match context {
        "periurban" => "peri-urban",
        o => o,
    };
    jobj! { "depth" => d, "text" => format!("G00907 Table 2: {} ({}, {} zoning, {}).", d, result, if consistent { "consistent with" } else { "inconsistent with" }, cx) }
}

/// `crashFlags(P, sc)`: the crash flags for the scope: `{ n, text }` (and `dsi` when crash analysis is loaded).
pub fn crash_flags(p: &V, sc: &V) -> V {
    let none = jobj! { "n" => V::Null, "text" => "crash data not loaded" };
    if p.g("data").g("crashes").jlen() == 0 {
        return none;
    }
    let Some(s) = engine::safety_summary(p, sc) else { return none };
    let band = |b: &V| rx("high", "").test(&b.sn());
    let hi = s.g("topLinks").items().iter().filter(|l| l.g("collective").t() && band(l.g("collective").g("band"))).count()
        + s.g("topNodes").items().iter().filter(|n| n.g("band").t() && band(n.g("band").g("band"))).count();
    let yrs = s.g("years");
    let window = if yrs.t() && yrs.g("from").t() { format!("{}\u{2013}{}", yrs.g("from").s(), yrs.g("to").s()) } else { "CAS window".into() };
    let pl = if hi == 1 { "" } else { "s" };
    jobj! { "n" => hi, "dsi" => s.g("dsi"), "text" => format!("{hi} link{pl} or junction{pl} in the medium-high or high risk band; {} DSI crashes in the study area ({window})", num(s.g("dsi"), 0)) }
}

/// `proportionality(P, o)`: the proportionality paragraph (the author confirms or edits it).
pub fn proportionality(p: &V, o: &V) -> V {
    let c = state(p);
    let pr = c.g("proportion");
    if pr.g("text").t() {
        return jobj! { "text" => pr.g("text"), "edited" => true, "confirmed" => pr.g("confirmed").t(), "by" => pr.g("by"), "date" => pr.g("date") };
    }
    let ftaa = o.g("p").g("kind").eq_s("ftaa");
    let trans = o.g("regime").g("side").eq_s("transition");
    let law = if ftaa { "section 44 of the Fast-track Approvals Act 2024 (sufficient detail)".to_string() } else { format!("RMA s88(2AA)\u{2013}(2AB) and Schedule 4 clause 2(3)(c){}", if trans { ", and s18B (succinct documents with sufficient information) as the application is lodged on or after 30 October 2026" } else { "" }) };
    let d = |k: &str| -> String { o.g("drivers").items().iter().find(|x| x.ix(0).eq_s(k)).map_or_else(|| "undefined".to_string(), |x| x.ix(1).s()) };
    let level = o.g("level").s();
    let nz = o.g("nzta");
    let mut text = format!("The depth of this transport assessment is set in proportion to the scale and significance of the proposal's transport effects, as {} require{}. ", law, if ftaa { "s" } else { "" });
    text.push_str(&format!("The proposal generates about {} of net vehicle trips. The largest change on any modelled link is {}. ", d("Net vehicle trips, busiest modelled hour"), d("Largest % change on a modelled link")));
    text.push_str(&format!("Crash flags: {}. State highway involvement: {}. Precinct: {}. ", d("Crash flags"), d("State highway involvement"), d("Precinct")));
    if nz.t() {
        text.push_str(&format!("{} ", nz.g("text").s()));
        if o.g("depth").t() && o.g("depth").g("depth").t() {
            text.push_str(&format!("{} ", o.g("depth").g("text").s()));
        }
    }
    let an = if rx(r"^[AEIOU]", "").test(&level) { "n" } else { "" };
    text.push_str(&format!("On that basis a{an} {} is proposed. [Author to confirm and record agreement at scoping.]", level.to_lowercase().replacen("ita", "ITA", 1)));
    jobj! { "text" => text, "edited" => false, "confirmed" => pr.g("confirmed").t(), "by" => pr.g("by"), "date" => pr.g("date") }
}

/// `itaScope(P)` for the active scenario.
pub fn ita_scope(p: &V) -> V {
    ita_scope_sc(p, active_sc(p))
}

/// `itaScope(P, sc)`: the ITA trigger, scope and proportionality: the AUP, AT and NZTA tests, the recommended level, the drivers and the proportionality paragraph.
pub fn ita_scope_sc(p: &V, sc: &V) -> V {
    let c = state(p);
    let ctx = c.g("ctx");
    let pe = pathway_entry(ctx.g("pathway"));
    let e = e27_sc(p, sc);
    let qn = e.g("quantities");
    let r = regime(p);
    let status = ctx.g("activityStatus").s();
    let mut aup: Vec<String> = Vec::new();
    if e.g("redirect").t() {
        aup.push(format!("{}.", e.g("redirect").g("text").s()));
    } else if e.g("exempt").t() {
        aup.push(format!("{}.", e.g("exempt").g("text").s()));
    } else if e.g("tier").t() {
        aup.push(format!("{} (tier {}): {}", e.g("exceededText").s(), e.g("tier").s(), e.g("consequence").s()));
        aup.push(e.g("wording").s());
    } else {
        aup.push("No E27.6.1 trip-generation threshold is exceeded.".into());
    }
    for n in e.g("notes").items() {
        if rx(r"^E27\.9", "").test(&n.s()) {
            aup.push(n.s());
        }
    }
    let major = e.g("tier").n() == 2.0 || qn.g("peak").n() >= 100.0;
    let ch = link_change(p, sc);
    let sh_i = sh_involved(p, sc);
    let mut at: Vec<String> = Vec::new();
    let kind = pe.map_or(String::new(), |x| x.g("kind").s());
    if kind == "pc" {
        at.push("Plan change (AT ITA v2 scoping table: an ITA is likely)".into());
    }
    if kind == "nor" {
        at.push(format!("{} (AT ITA v2: an ITA is likely and should consider alternative sites, routes or methods)", pe.map_or(String::new(), |x| x.g("label").s())));
    }
    let act = |s: &str| table("ACTIVITY").g(s).s();
    if (status == "D" || status == "NC") && major {
        at.push(format!("{} major development (major: E27 tier 2 or 100 veh/h or more in the busiest hour; {})", act(&status), assumption()));
    }
    if status == "D" && major {
        at.push("E27.3(2): major proposals for discretionary consent prepare an integrated transport assessment".into());
    }
    if e.g("redirect").t() {
        at.push(format!("Precinct requirement: an application infringing the staging standard needs an ITA under the AT ITA Guidelines in force ({})", e.g("redirect").g("precinct").g("ita").s()));
    }
    let ch_p = if ch.t() { ch.g("p").n() } else { f64::NAN };
    let net_fx = (ch.t() && ch_p > 0.05) || qn.g("peak").n() >= 100.0;
    if kind == "ftaa" && net_fx {
        at.push(format!("Fast-track application with network effects ({}; AT ITA v2 says the ITA should follow the guidelines)", if ch.t() && ch_p > 0.05 { format!("+{}% on {}", numf(ch_p * 100.0, 1), ch.g("name").s()) } else { format!("{} veh/h", numf(qn.g("peak").n(), 0)) }));
    }
    let nzta = if sh_i.g("involved").t() || !in_auckland(p) { table1(qn) } else { V::Null };
    let consistent = !matches!(ctx.g("zoningConsistent"), V::Bool(false));
    let context = if ctx.g("context").t() { ctx.g("context").s() } else { "urban".into() };
    let depth = if nzta.t() { depth_of(&nzta.g("result").s(), consistent, &context) } else { V::Null };
    let cuts = table("LEVEL_TRIPS.cuts");
    let pk = qn.g("peak").n();
    let mut lvl: usize = if pk >= cuts.ix(2).n() { 3 } else if pk >= cuts.ix(1).n() { 2 } else if pk >= cuts.ix(0).n() { 1 } else { 0 };
    let mut reasons: Vec<String> = vec![format!("{} veh/h in the busiest hour (bands 20 / 100 / 500 veh/h, {})", numf(pk, 0), assumption())];
    let raise = |n: usize, why: String, lvl: &mut usize, reasons: &mut Vec<String>| {
        if n > *lvl {
            *lvl = n;
            reasons.push(why);
        }
    };
    if e.g("tier").t() {
        raise(1, format!("E27 tier {} exceeded", e.g("tier").s()), &mut lvl, &mut reasons);
    }
    if nzta.t() && nzta.g("result").eq_s("Lite") {
        raise(1, "NZTA Table 1 ITA Lite".into(), &mut lvl, &mut reasons);
    }
    if nzta.t() && nzta.g("result").eq_s("Full") {
        raise(2, "NZTA Table 1 ITA Full".into(), &mut lvl, &mut reasons);
    }
    if !at.is_empty() {
        raise(2, "AT ITA v2 likely case".into(), &mut lvl, &mut reasons);
    }
    if ch.t() && ch_p > 0.05 {
        raise(1, format!("link change over 5% ({})", ch.g("name").s()), &mut lvl, &mut reasons);
    }
    let crash = crash_flags(p, sc);
    let precinct = if e.g("redirect").t() { format!("{} {}", e.g("redirect").g("precinct").g("id").s(), e.g("redirect").g("precinct").g("name").s()) } else if e.g("site").g("precinct").t() { e.g("site").g("precinct").s() } else { "none".into() };
    let sh_text = if sh_i.g("involved").t() { sh_i.g("why").items().iter().map(|x| x.s()).collect::<Vec<_>>().join("; ") } else if sh_i.g("inArea").t() { "state highway in the study area, not used by the development in the results".into() } else { "none found".into() };
    let drivers = V::Array(vec![
        jarr!["Net vehicle trips, busiest modelled hour", format!("{} veh/h{}", numf(pk, 0), if qn.g("peakPeriod").t() { format!(" ({})", qn.g("peakPeriod").s()) } else { String::new() })],
        jarr!["Largest % change on a modelled link", if ch.t() { format!("+{}% on {} ({})", numf(ch_p * 100.0, 1), ch.g("name").s(), ch.g("period").s()) } else { "not computed (needs results for a base and a with-development scenario)".into() }],
        jarr!["Crash flags", crash.g("text")],
        jarr!["State highway involvement", sh_text],
        jarr!["Precinct", precinct],
    ]);
    let level = table("LEVELS").ix(lvl).clone();
    let paragraph = proportionality(p, &jobj! { "level" => &level, "drivers" => &drivers, "regime" => &r, "p" => pe.cloned().unwrap_or(V::Null), "nzta" => &nzta, "depth" => &depth });
    jobj! { "aup" => strs(aup), "at" => strs(at), "nzta" => nzta, "depth" => depth, "level" => level, "levelIndex" => lvl, "reasons" => strs(reasons), "drivers" => drivers,
        "change" => ch, "sh" => sh_i, "crash" => crash, "e27" => e, "paragraph" => paragraph, "never" => "E27.6.1 exceedance alone is not stated as an ITA trigger." }
}

/// `scopingMemo(P, sc)`: the scoping memo items, proposed from the project; each can be locked with who agreed and when.
pub fn scoping_memo(p: &V, sc: &V) -> V {
    let c = state(p);
    let b = bounds_of(sc);
    let loaded = p.g("data").g("loaded");
    let assessment = p.g("assessment");
    let mut years: Vec<f64> = Vec::new();
    for s in p.g("scenarios").items() {
        let y = engine::year_of(p, s);
        if !years.contains(&y) {
            years.push(y);
        }
    }
    let mut ys: Vec<String> = years.iter().map(|y| ns(*y)).collect();
    ys.sort();
    let years_s = ys.join(", ");
    let data: Vec<String> = [("nzta", "NZTA counts"), ("at", "AT counts"), ("cas", "NZTA CAS crashes"), ("fc", "AT Future Connect"), ("user", "own counts")]
        .iter()
        .filter(|(k, _)| loaded.g(k).t() && loaded.g(k).g("at").t())
        .map(|(k, n)| format!("{} ({})", n, today(loaded.g(k).g("at"))))
        .collect();
    let auth = authorities(p, &V::Null);
    let periods = if assessment.t() && assessment.g("periods").t() { assessment.g("periods").items().iter().map(|x| x.s()).collect::<Vec<_>>().join(", ") } else { "AM, PM".into() };
    let reg = p.g("register");
    let proposed = jobj! {
        "area" => match b { Some((s, w, n, e)) => format!("Network extent {} \u{d7} {} km; extend to links over the AT 5%/10% test", numf(util::dist(&[s, w], &[s, e]) / 1000.0, 1), numf(util::dist(&[s, w], &[n, w]) / 1000.0, 1)), None => "not set".into() },
        "periods" => format!("{periods} peak hour (one hour each)"),
        "years" => if years_s.is_empty() { "\u{2013}".to_string() } else { years_s },
        "data" => if data.is_empty() { "none loaded yet".to_string() } else { data.join("; ") },
        "rates" => "Spare Construct indicative peak-hour vehicle trip rates (Settings); replace with sourced, dated rates (AT ITA v2 Appendix 1)",
        "committed" => if reg.jlen() > 0 { format!("{} register rows; included rows by AT ITA s3.6.1 rules", reg.jlen()) } else { "register empty".into() },
        "tools" => "Spare Construct (screening); SIDRA for junctions where required; MSM consistency check where network-wide",
        "stakeholders" => auth.g("rows").items().iter().map(|r| r.g("name").s()).collect::<Vec<_>>().join("; "),
    };
    V::Array(
        table("SCOPE_ITEMS")
            .items()
            .iter()
            .map(|it| {
                let k = it.ix(0).s();
                let s = c.g("scoping").g(&k);
                jobj! { "key" => k.as_str(), "label" => it.ix(1), "value" => if s.g("value").t() { s.g("value").clone() } else { proposed.g(&k).clone() }, "proposed" => proposed.g(&k), "edited" => s.g("value").t(), "locked" => s.g("locked").t(), "by" => if s.g("by").t() { s.g("by").clone() } else { st("") }, "date" => if s.g("date").t() { s.g("date").clone() } else { st("") } }
            })
            .collect(),
    )
}
