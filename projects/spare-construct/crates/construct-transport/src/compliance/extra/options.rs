//! Options and alternatives, plan change mode and full build-out (compliance-extra.js section 8), with their CSV exports and report sections.

pub mod util;

use self::util::{cells, fld_s, Put, int32, label, n0, numk, or_s, or_v, put_some, tbl, tbl_th, unique, xs};
use super::ctx::{active_sc, assumption, assumptions, class_of, context, data, day, disc, disclaimer, esc, esc_s, link_name, long_date_s, node_name, num, pair, periods_of, posted, res_of, sc_by_id, stamped};
use super::shared::{changed_links, crashes_on, dev_mix, econ_of, kpi_p, neg_pp, per_period, pp_text, route_p};
use super::{lint, mitigation};
use crate::compliance::data::{m as model, x as xdata};
use crate::compliance::html::{ReportCtx, rt, th, thn};
use crate::compliance::jv::{Jx, V, csv, jarr, jobj, merge, strs};
use crate::compliance::{engine, net};
use crate::graph::js_sort_by;
use crate::util::{fmt, r1, r3};
use std::collections::HashSet;

/* =====================================================================================
   Options and alternatives (reg-alternatives)
   ===================================================================================== */

/// One row of the options list: the option's own fields and the scenario it names (`{ ...o, sc }`).
pub struct OptionEntry<'a> {
    /// The option: the author's tagged option, or one made from a scenario (`id`, `name`, `scId`, `hierarchy`, `cost`, `notes`, `auto`).
    pub o: V,
    /// The scenario the option names, if it exists.
    pub sc: Option<&'a V>,
}

/// `optionList(P)`: the author's tagged options, or every scenario other than the do-minimum when none are tagged.
pub fn option_list(p: &V) -> Vec<OptionEntry<'_>> {
    let cx = data(p);
    let base = pair(p).base;
    let opts = cx.g("options");
    if opts.jlen() > 0 {
        return opts.items().iter().map(|o| OptionEntry { o: merge(&V::object(), o), sc: sc_by_id(p, o.g("scId")) }).collect();
    }
    p.g("scenarios")
        .items()
        .iter()
        .filter(|s| !std::ptr::eq(*s, base) && !s.g("role").eq_s("existing") && !s.g("role").eq_s("baseline"))
        .map(|s| OptionEntry {
            o: jobj! {
                "id" => format!("sc:{}", s.g("id").s()), "name" => s.g("name"), "scId" => s.g("id"),
                "hierarchy" => if s.g("role").eq_s("mitigation") { "capacity" } else { "" }, "cost" => V::Null, "notes" => "", "auto" => true,
            },
            sc: Some(s),
        })
        .collect()
}

/// The level of service text of a result node (`nr.intersectionLos || nr.los` printed in a template).
fn los_text(nr: &V) -> String {
    if nr.g("intersectionLos").t() { nr.g("intersectionLos").s() } else { fld_s(nr, "los") }
}

/// `Engine.yearOf(sc)`: the scenario's year where set (and positive), otherwise the project's year.
fn year_of(p: &V, sc: &V) -> f64 {
    let y = sc.g("year").n();
    if y > 0.0 { y } else { numk(p.g("settings"), "year") }
}

/// `alternatives(P)`: the options against the intervention hierarchy: `{ base, rows, warnings, sections, label, source }`. `base` is a copy of
/// the do-minimum scenario, as the prototype hands the scenario itself back.
pub fn alternatives(p: &V) -> V {
    let pr = pair(p);
    let base = pr.base;
    let c = context(p);
    let mut rows: Vec<V> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for e in option_list(p) {
        let o = &e.o;
        let sc = e.sc;
        let ps: Vec<&str> = sc.map_or_else(Vec::new, |s| periods_of(s).into_iter().filter(|x| res_of(base, x).is_some()).collect());
        let name = if o.g("name").t() {
            o.g("name").clone()
        } else if sc.is_some_and(|s| s.g("name").t()) {
            sc.map_or(V::Null, |s| s.g("name").clone())
        } else {
            V::String("Option".into())
        };
        let hk = o.g("hierarchy").s();
        let lab = xdata("HIER").g(&hk);
        let mut row = V::object();
        put_some(&mut row, "id", o.get("id"));
        row.put("name", name);
        row.put("hierarchy", or_s(o.g("hierarchy"), ""));
        row.put("label", if lab.t() { lab.clone() } else { V::String("Not tagged".into()) });
        row.put("scName", sc.map_or(V::String("No scenario".into()), |s| s.g("name").clone()));
        row.put("ran", !ps.is_empty());
        row.put("notes", or_s(o.g("notes"), ""));
        if let (Some(sc), false) = (sc, ps.is_empty()) {
            // per modelled peak hour, each period on its own (the means fill the numeric columns)
            let vht = per_period(base, sc, &|s, pd| kpi_p("vht", s, pd), &ps, 1, "veh\u{b7}h/h");
            let vkt = per_period(base, sc, &|s, pd| kpi_p("vkt", s, pd), &ps, 0, "veh-km/h");
            let co2 = per_period(base, sc, &|s, pd| kpi_p("co2", s, pd), &ps, 2, "t/h");
            let route = per_period(base, sc, &|s, pd| route_p(s, pd), &ps, 0, "s");
            let means = [vht.g("mean").clone(), vkt.g("mean").clone(), co2.g("mean").clone(), route.g("mean").clone()];
            row.put("pp", jobj! { "vht" => vht, "vkt" => vkt, "co2" => co2, "route" => route });
            row.put("dVht", means[0].clone());
            row.put("dVkt", means[1].clone());
            row.put("dCo2", means[2].clone());
            row.put("dRoute", means[3].clone());
            row.put("periods", strs(ps.iter()));
            // the busiest junction
            let mut nodes: Vec<(String, &str, &V)> = Vec::new();
            for &pd in &ps {
                for (id, nr) in res_of(sc, pd).map_or_else(Vec::new, |r| r.g("nodes").entries()) {
                    nodes.push((id.to_string(), pd, nr));
                }
            }
            js_sort_by(&mut nodes, |a, b| b.2.g("dos").n() - a.2.g("dos").n());
            row.put(
                "worst",
                match nodes.first() {
                    Some((id, pd, nr)) => format!("{} {}: DoS {}, LoS {}", node_name(sc, id), pd, num(nr.g("dos"), 2), los_text(nr)),
                    None => fmt::DASH.to_string(),
                },
            );
            let ch = changed_links(base, sc);
            let mut km = 0.0;
            for id in &ch {
                let l = sc.g("links").g(id);
                if !l.t() || !l.g("cycle").t() || l.g("cycle").eq_s("none") {
                    continue;
                }
                let bl = base.g("links").g(&l.g("id").s());
                if !bl.t() || !bl.g("cycle").strict_eq(l.g("cycle")) {
                    km += net::len(sc, l) / 1000.0;
                }
            }
            row.put("cycleKm", km);
            row.put("cas", crashes_on(p, base, &ch));
            let ec = econ_of(p, base, sc);
            let bcr = if ec.t() && !ec.g("mismatch").t() {
                if ec.g("bcrText").t() { ec.g("bcrText").clone() } else { V::String(num(ec.g("bcr"), 1)) }
            } else if ec.t() && ec.g("mismatch").t() {
                V::String("years differ".into())
            } else {
                V::String(fmt::DASH.into())
            };
            row.put("bcr", bcr);
            let ipm = if ec.t() && ec.g("ipm").t() && ec.g("ipm").jlen() > 0 {
                ec.g("ipm").items().iter().map(|b| format!("{}{}", fld_s(b, "band"), if b.g("draft").t() { " (draft)" } else { "" })).collect::<Vec<_>>().join(" / ")
            } else {
                fmt::DASH.to_string()
            };
            row.put("ipm", ipm);
            if !o.g("cost").nullish() && !o.g("cost").eq_s("") {
                row.put("cost", V::Number(o.g("cost").n()));
            } else if ec.t() {
                put_some(&mut row, "cost", ec.get("cap"));
            } else {
                row.put("cost", V::Null);
            }
            let identical = year_of(p, sc) == year_of(p, base) && dev_mix(sc) == dev_mix(base);
            row.put("identical", identical);
            if !identical {
                warnings.push(format!(
                    "{} does not use identical inputs to {} (year or development land use differ): options must be compared on the same inputs.",
                    row.g("name").s(),
                    base.g("name").s()
                ));
            }
        }
        row.put("dsi", if !o.g("dsi").nullish() && !o.g("dsi").eq_s("") { V::Number(o.g("dsi").n()) } else { V::Null });
        rows.push(row);
    }
    let has = |t: &str| rows.iter().any(|r| r.g("hierarchy").eq_s(t));
    if !rows.is_empty() && !has("demand") && !has("optimise") {
        warnings.insert(0, "No demand-management or optimisation option was tested. The intervention hierarchy expects them to be considered before new capacity (MBCM s1.5; Ministerial statement 13 Aug 2026).".into());
    }
    if rows.iter().any(|r| !r.g("hierarchy").t()) {
        warnings.push("Tag every option with its place in the intervention hierarchy.".into());
    }
    jobj! {
        "base" => base, "rows" => V::Array(rows), "warnings" => strs(warnings.iter()), "sections" => alternative_sections(p, Some(&c)),
        "label" => label("vkt"), "source" => xs("HIER_SRC"),
    }
}

/// `alternativeSections(P, c)`: which alternatives sections the document needs, by pathway and the author's flags: `[{ id, title, ref, include, why }]`.
pub fn alternative_sections(p: &V, c: Option<&V>) -> V {
    let own;
    let c = match c {
        Some(c) => c,
        None => {
            own = context(p);
            &own
        }
    };
    let mut out: Vec<V> = Vec::new();
    let mut add = |id: &str, title: &str, rf: &str, include: V, why: &str| out.push(jobj! { "id" => id, "title" => title, "ref" => rf, "include" => include, "why" => why });
    let kind = c.g("kind");
    let pathway = c.g("pathway");
    let sig = c.g("sigEffect").t();
    if kind.eq_s("rc") || pathway.eq_s("ftaa-consent") {
        add(
            "cl6-1a", "Alternative locations or methods", "RMA Sch 4 cl 6(1)(a)", V::Bool(sig),
            if sig { "The author flags a likely significant adverse effect." } else { "Shown only if the author flags a likely significant adverse effect." },
        );
    }
    if pathway.eq_s("nor") || pathway.eq_s("outline-plan") {
        let li = c.g("landInterest");
        if li.eq_s("no") {
            add("s168-3b-b", "Alternative sites, routes and methods", "RMA s168(3B)(b), s171(1)(b)", V::Bool(true), "The requiring authority has no sufficient interest in the land.");
            add("necessity", "Reasonable necessity", "RMA s171(1)(c)", V::Bool(true), "Required with the alternatives assessment.");
        } else if li.eq_s("yes") {
            add(
                "s168-3b-c", "Alternative sites, routes and methods", "RMA s168(3B)(c)", V::Bool(sig),
                if sig { "Sufficient land interest, but the author flags a likely significant adverse effect." } else { "Sufficient land interest and no significant adverse effect flagged: not required." },
            );
        } else {
            add("s168-3b", "Alternative sites, routes and methods", "RMA s168(3B), s171(1)(b)", V::Null, "Record whether the requiring authority has sufficient interest in the land (Compliance tab).");
        }
    }
    if pathway.eq_s("ftaa-nor") {
        add("ftaa-12g", "Alternatives", "FTAA Sch 5 cl 12(1)(g)", V::Bool(true), "Fast-track NoR.");
        add("ftaa-12h", "Reasonable necessity", "FTAA Sch 5 cl 12(1)(h)", V::Bool(true), "Fast-track NoR.");
    }
    if kind.eq_s("pc") {
        add("s32-1b", "Reasonably practicable options", "RMA s32(1)(b)", V::Bool(true), "Plan change evaluation.");
    }
    if kind.eq_s("fund") {
        add("ltma-20", "Alternatives and options", "LTMA s20(2)(c)(ii)", V::Bool(true), "NLTF activity.");
    }
    V::Array(out)
}

/* =====================================================================================
   Plan change mode (reg-plan-change)
   ===================================================================================== */

/// `zonedOf(z, proposed)`: a zone's entered zoned capacity (operative, or PC120 proposed) when it is an object, else null.
pub fn zoned_of(z: &V, proposed: bool) -> V {
    let x = z.g("cx").g(if proposed { "zonedProposed" } else { "zoned" });
    if x.is_obj() { x.clone() } else { V::Null }
}

/// The dwelling land uses (`DW_KEYS`).
pub const DW_KEYS: [&str; 3] = ["residential", "apartment", "retirement"];

/// `mergeZoned(current, zoned)`: a zone's full build-out land use: the entered zoned capacity merged over its current land use. Entered
/// dwelling capacity (of any type) replaces all of the zone's dwellings; every other land use without an entered capacity is kept. Blank
/// entries are ignored. `{ lu, replaced, kept }`.
pub fn merge_zoned(current: &V, zoned: &V) -> V {
    let landuses = model("LANDUSES");
    let mut entered = V::object();
    for (k, v) in zoned.entries() {
        if landuses.has(k) && landuses.g(k).t() && !v.nullish() && !v.eq_s("") && v.n().is_finite() {
            entered.set(k, V::Number(v.n()));
        }
    }
    let cur = if current.t() { current.clone() } else { V::object() };
    let mut out = merge(&V::object(), &cur);
    let dw_in = DW_KEYS.iter().any(|k| entered.has(k));
    if dw_in {
        for k in DW_KEYS {
            out.remove(k);
        }
    }
    for (k, v) in entered.entries() {
        out.set(k, v.clone());
    }
    let mut replaced: Vec<String> = entered.keys().iter().map(|k| k.to_string()).collect();
    if dw_in {
        replaced.extend(DW_KEYS.iter().filter(|k| !entered.has(k) && cur.g(k).n() > 0.0).map(|k| k.to_string()));
    }
    let kept: Vec<String> = cur
        .keys()
        .into_iter()
        .filter(|k| !entered.has(k) && !(dw_in && DW_KEYS.contains(k)) && cur.g(k).n() > 0.0)
        .map(|k| k.to_string())
        .collect();
    jobj! { "lu" => out, "replaced" => strs(replaced.iter()), "kept" => strs(kept.iter()) }
}

/// `fullBuildout(P, sc)`: every zone with entered zoned capacity takes that capacity at 100% (PC120 proposed capacity when selected, labelled
/// proposed): `{ zones, dwellings, gfa, proposed, label, rule }`.
pub fn full_buildout(p: &V, sc: Option<&V>) -> V {
    let sc = sc.unwrap_or_else(|| active_sc(p));
    let proposed = data(p).g("planChange").g("useProposed").t();
    let mut zones: Vec<V> = Vec::new();
    for z in sc.g("zones").vals() {
        if z.g("kind").eq_s("gateway") {
            continue;
        }
        let mut zc = zoned_of(z, proposed);
        if !zc.t() && proposed {
            zc = zoned_of(z, false);
        }
        if !zc.t() {
            continue;
        }
        let m = merge_zoned(z.g("lu"), &zc);
        if m.g("replaced").jlen() == 0 {
            continue;
        }
        zones.push(jobj! {
            "id" => z.g("id"), "name" => or_v(z.g("name"), z.g("id")), "lu" => m.g("lu"), "entered" => &zc, "replaced" => m.g("replaced"), "kept" => m.g("kept"),
            "current" => if z.g("lu").t() { z.g("lu").clone() } else { V::object() }, "proposed" => zoned_of(z, proposed).t() && proposed,
        });
    }
    let sum = |keys: &[&str]| zones.iter().fold(0.0, |s, z| s + keys.iter().fold(0.0, |t, k| t + n0(z.g("lu").g(k))));
    let dw = sum(&DW_KEYS);
    let gfa = sum(&super::ctx::GFA_KEYS);
    jobj! {
        "zones" => V::Array(zones), "dwellings" => dw, "gfa" => gfa, "proposed" => proposed,
        "label" => if proposed { "Zoned capacity: PC120 proposed (labelled proposed; check the decision, due by June 2027)" } else { "Zoned capacity: operative zoning (entered yields)" },
        "rule" => "Entered dwelling capacity replaces the zone's dwellings of every type; other land uses without an entered capacity are kept",
    }
}

/// `createFullBuildout(P, sc)`: adds a scenario with every zone at its full build-out capacity (ids kept, results dropped, the scenario at 100%
/// build-out) and returns it; null when no zone has entered capacity. `sc` is the scenario to copy (the active one by default); pass a copy
/// when it belongs to the project being changed. The new scenario's id is a counter-based one.
pub fn create_full_buildout(p: &mut V, sc: Option<&V>) -> V {
    let sc: V = sc.map_or_else(|| active_sc(p).clone(), |s| s.clone());
    let fb = full_buildout(p, Some(&sc));
    if fb.g("zones").jlen() == 0 {
        return V::Null;
    }
    let mut ns = engine::scenario_copy_without_links(&sc, &HashSet::new());
    for z in fb.g("zones").items() {
        let id = z.g("id").s();
        if let Some(nz) = ns.get_mut("zones").and_then(|zs| zs.get_mut(&id)) {
            if nz.t() {
                nz.set("lu", merge(&V::object(), z.g("lu")));
                nz.remove("buildout");
            }
        }
    }
    let taken: Vec<String> = p.g("scenarios").items().iter().map(|s| s.g("id").s()).collect();
    let mut n = 1;
    while taken.contains(&format!("s{n}")) {
        n += 1;
    }
    ns.set("id", V::String(format!("s{n}")));
    ns.set("name", V::String(format!("{} \u{b7} full build-out{}", sc.g("name").s(), if fb.g("proposed").t() { " (PC120 proposed)" } else { "" })));
    ns.set("role", or_s(sc.g("role"), "proposal"));
    ns.set("buildout", V::Number(100.0));
    match sc.get("year") {
        Some(y) => ns.set("year", y.clone()),
        None => {
            ns.remove("year");
        }
    }
    ns.set("notes", V::String(format!("{}. {}. Created by Compliance+ plan change mode.", fb.g("label").s(), fb.g("rule").s())));
    if let Some(list) = p.get_mut("scenarios").and_then(|s| s.as_array_mut()) {
        list.push(ns.clone());
    }
    ns
}

/// `rltpRelied(P)`: the network projects the assessment relies on, in words: the committed roads of the active scenario and the transport rows of the register.
pub fn rltp_relied(p: &V) -> String {
    let sc = active_sc(p);
    let funding = model("FUNDING");
    let mut names: Vec<String> = Vec::new();
    for l in sc.g("links").vals() {
        if net::link_stage(l) == "committed" && !l.g("status").eq_s("closed") {
            let f = l.g("funding").s();
            let fl = if funding.has(&f) && funding.g(&f).t() { funding.g(&f).s() } else { "funding not recorded".to_string() };
            names.push(format!("{} ({})", link_name(l), fl));
        }
    }
    let re = crate::compliance::rx::rx("transport|road|network", "i");
    for r in p.g("register").items() {
        let ty = if r.g("type").t() { r.g("type").s() } else { String::new() };
        if re.test(&ty) {
            names.push(format!("{} ({})", fld_s(r, "name"), if r.g("status").t() { r.g("status").s() } else { "status not recorded".to_string() }));
        }
    }
    let names = unique(names);
    if names.is_empty() {
        return "None recorded: mark committed network projects (road inspector \u{203a} stage) or add them to the register.".to_string();
    }
    let shown = names.iter().take(8).cloned().collect::<Vec<_>>().join("; ");
    if names.len() > 8 { format!("{shown}; and {} more", names.len() - 8) } else { shown }
}

/// `planChange(P)`: the plan change process, s32 table, policy rows, road function table and the cl 23 pack:
/// `{ kind, notification, process, buildout, s32, s32Label, policy, roads, cl23, triggers }`.
pub fn plan_change(p: &V) -> V {
    let c = context(p);
    let cx = data(p);
    let pc = cx.g("planChange");
    let kind = if c.g("kind").eq_s("pc") { c.g("pathway").s() } else if pc.g("kind").t() { pc.g("kind").s() } else { String::new() };
    let n = if c.g("notification").t() { c.g("notification").s() } else { day(pc.g("notification")) };
    let transition = xs("STD.dates.transition");
    let private_bar = xs("STD.dates.privatePcBar");
    let mut proc: Vec<V> = Vec::new();
    if kind == "pc-council" {
        proc.push(if !n.is_empty() && n < transition {
            jobj! { "level" => "info", "text" => format!("Notified {}, before 30 Oct 2026: check the Planning Act 2026 Sch 1 transition for plan changes.", long_date_s(&n)) }
        } else {
            jobj! { "level" => "warn", "text" => "Plan stop until the specified transition date (s80P) unless exempt (s80U-80W)." }
        });
    }
    if kind == "pc-private" {
        proc.push(if !n.is_empty() && n >= private_bar {
            jobj! { "level" => "warn", "text" => format!("Notified {}: Ministerial exemption required (s80YA).", long_date_s(&n)) }
        } else {
            jobj! { "level" => "info", "text" => format!("A private plan change request notified on or after {} needs a Ministerial exemption (s80YA){}.", long_date_s(&private_bar),
                if !n.is_empty() { format!("; this one is notified {}", long_date_s(&n)) } else { "; enter the notification date".to_string() }) }
        });
    }
    if kind.is_empty() {
        proc.push(jobj! { "level" => "info", "text" => "Choose a plan change pathway (Compliance tab) to check the process dates." });
    }
    let pr = pair(p);
    let base = pr.base;
    let fb = full_buildout(p, None);
    let alt = alternatives(p);
    let econ_settings = if p.g("settings").g("econ").t() { p.g("settings").g("econ").clone() } else { V::object() };
    let hrs = if econ_settings.g("periodHours").t() { econ_settings.g("periodHours").clone() } else { jobj! { "AM" => 600, "PM" => 600 } };
    let a = assumptions(p);
    let opts = option_list(p);
    let voc = a.g("vocPerKm");
    let mut s32: Vec<V> = Vec::new();
    for r in alt.g("rows").items() {
        if !r.g("ran").t() {
            continue;
        }
        let empty = V::object();
        let found = opts.iter().find(|x| x.o.g("id").strict_eq(r.g("id")));
        let (o, sc) = match found {
            Some(e) => (&e.o, e.sc),
            None => (&empty, None),
        };
        let ps: Vec<&str> = sc.map_or_else(Vec::new, |s| periods_of(s).into_iter().filter(|x| res_of(base, x).is_some()).collect());
        let kpi_of = |x: &V, pd: &str| res_of(x, pd).map_or(V::Null, |r| r.g("kpi").clone());
        let ann = |k: &str| {
            ps.iter().fold(0.0, |s, &pd| {
                let ks = sc.map_or(V::Null, |x| kpi_of(x, pd));
                s + (numk(&ks, k) - numk(&kpi_of(base, pd), k)) * n0(hrs.g(pd))
            })
        };
        let (ann_h, ann_vkt) = (ann("vht"), ann("vkt"));
        let unf: Vec<&V> = sc.map_or_else(Vec::new, |s| s.g("links").vals().into_iter().filter(|l| !l.g("status").eq_s("closed") && net::link_unfunded(l)).collect());
        let cas = r.g("cas");
        let safety = if cas.t() {
            format!(
                "{} crashes ({} DSI) on changed roads in the CAS window; predicted change {}",
                cas.g("crashes").s(), cas.g("dsi").s(),
                if !r.g("dsi").nullish() { format!("{} DSI/yr", fmt::signed(r.g("dsi").n(), 1)) } else { "not modelled".to_string() }
            )
        } else {
            "CAS not loaded".to_string()
        };
        let timing = o.g("timing");
        let names = unique(unf.iter().map(|l| link_name(l)).collect());
        s32.push(jobj! {
            "name" => r.g("name"), "label" => r.g("label"),
            "travel" => format!("{}; about {} veh\u{b7}h a year", pp_text(&neg_pp(r.g("pp").g("vht")), 1, "veh\u{b7}h saved per modelled peak hour"), fmt::signed(-ann_h, 0)),
            "safety" => safety,
            "voc" => format!("{} NZ$m a year ({} M veh-km \u{d7} NZ${}/km, {})", fmt::signed(ann_vkt * voc.n() / 1e6, 2), fmt::signed(ann_vkt / 1e6, 2), voc.s(), assumption()),
            "cost" => if !r.g("cost").nullish() { format!("{}{}", fmt::money(r.g("cost").n()), if timing.t() { format!(", {}", timing.s()) } else { String::new() }) } else { "Not costed".to_string() },
            "timing" => or_s(timing, ""),
            "acting" => if !unf.is_empty() {
                format!("Relies on {} unfunded project{} ({}): effects return if they are not funded", unf.len(), if unf.len() == 1 { "" } else { "s" }, names.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
            } else { "Relies on no unfunded project".to_string() },
            "notActing" => format!("Do-minimum: {} effects screened in {}", mitigation::effects(p, Some(base)).jlen(), base.g("name").s()),
        });
    }
    let vkt_label = label("vkt");
    let rltp = rltp_relied(p);
    let policy = vec![
        jobj! { "ref" => "RMA s74(2)(b)(i)", "policy" => "Regional land transport plan", "text" => rltp.as_str() },
        jobj! { "ref" => "RMA s74(2)(d); ERP2", "policy" => "Emissions reduction plan (information only)", "text" => format!("\u{394}VKT and \u{394}CO2e are reported in the s32 table and alternatives table: {vkt_label}.") },
        jobj! { "ref" => "NPS-UD Policy 1(c)", "policy" => "Good accessibility for all people (jobs, services, open space) by all modes", "text" => "See the accessibility rows in the policy alignment table." },
        jobj! { "ref" => "NPS-UD Policy 1(e)", "policy" => "Supports reductions in greenhouse gas emissions", "text" => format!("Information only: {vkt_label}.") },
        jobj! { "ref" => "NPS-I Policies 10-11", "policy" => "Infrastructure integrated with land use and funded", "text" => format!("Network projects relied on: {rltp}") },
    ];
    let sc = active_sc(p);
    let fn_default = |cls: &V| -> String {
        match cls.s().as_str() {
            "motorway" => "Strategic (motorway)",
            "ramp" => "Strategic (motorway ramp)",
            "arterial" => "Arterial: movement with place where it passes centres",
            "collector" => "Collector: connects neighbourhoods to arterials",
            "local" => "Local street: access and place",
            _ => "Local street",
        }
        .to_string()
    };
    let cycles = model("CYCLE");
    let statuses = model("STATUS");
    let mut roads: Vec<V> = Vec::new();
    for l in sc.g("links").vals() {
        let lcx = l.g("cx");
        if !(l.g("status").eq_s("proposed") || (lcx.t() && (lcx.g("rasf").t() || lcx.g("fn").t()))) {
            continue;
        }
        let cyc = l.g("cycle").s();
        let st = l.g("status").s();
        roads.push(jobj! {
            "id" => l.g("id"), "name" => link_name(l), "cls" => class_of(l.g("cls")).g("label"),
            "fn" => if lcx.t() && lcx.g("fn").t() { lcx.g("fn").clone() } else { V::String(fn_default(l.g("cls"))) },
            "rasf" => if lcx.t() && lcx.g("rasf").t() { lcx.g("rasf").clone() } else { V::String(fmt::DASH.into()) },
            "fc" => if l.g("fc").t() && l.g("fc").g("gt").t() { l.g("fc").g("gt").clone() } else { V::String(fmt::DASH.into()) },
            "speed" => posted(l), "lanes" => int32(l.g("lanesAB")) + int32(l.g("lanesBA")),
            "cycle" => if cycles.has(&cyc) && cycles.g(&cyc).t() { cycles.g(&cyc).clone() } else { V::String("None".into()) },
            "status" => if statuses.has(&st) && statuses.g(&st).t() { statuses.g(&st).clone() } else { l.g("status").clone() },
        });
    }
    let lint_n = lint::lint(p, None).g("findings").jlen();
    let stat = engine::model_status(p, "AM").g("label").clone();
    let sens_rows = p.g("report").g("sens").g("rows");
    let cl23 = vec![
        jarr!["Model status and purpose", stat],
        jarr!["Sensitivity tests", if sens_rows.t() { format!("{} tests run", sens_rows.jlen()) } else { "Not run: run the sensitivity pack".to_string() }],
        jarr![
            "Full build-out",
            if fb.g("zones").jlen() > 0 {
                format!("{} dwellings, {} m\u{b2} GFA ({})", fmt::int(fb.g("dwellings").n()), fmt::int(fb.g("gfa").n()), fb.g("label").s())
            } else {
                "Zoned capacity not entered".to_string()
            }
        ],
        jarr!["Options assessed (s32(1)(b))", format!("{} option{}", alt.g("rows").jlen(), if alt.g("rows").jlen() == 1 { "" } else { "s" })],
        jarr!["Scope and wording", if lint_n > 0 { format!("{} linter finding{} open", lint_n, if lint_n == 1 { "" } else { "s" }) } else { "No linter findings".to_string() }],
        jarr!["Staging triggers", if cx.g("triggers").t() { "Trigger finder run" } else { "Not run" }],
    ];
    jobj! {
        "kind" => kind.as_str(), "notification" => n, "process" => V::Array(proc), "buildout" => fb, "s32" => V::Array(s32), "s32Label" => label("s32"), "policy" => V::Array(policy),
        "roads" => V::Array(roads), "cl23" => V::Array(cl23), "triggers" => cx.g("triggers"),
    }
}

/* =====================================================================================
   CSV exports
   ===================================================================================== */

/// `optionsCSV(P)`: the options appraisal, ready for the EAST and AST tools, with the version stamp.
pub fn options_csv(p: &V) -> String {
    let a = alternatives(p);
    let pv = |r: &V, k: &str, pd: &str, f: &dyn Fn(f64) -> f64| -> V {
        let x = r.g("pp").g(k).g("by").g(pd);
        if r.g("pp").t() && r.g("pp").g(k).t() && !x.nullish() { V::Number(f(x.n())) } else { V::String(String::new()) }
    };
    let junction = label("junction");
    let mut rows: Vec<Vec<V>> = vec![
        cells!["Appraisal summary (EAST/AST-ready)", format!("Do-minimum: {}", a.g("base").g("name").s()), "Values are per modelled peak hour, each period on its own"],
        cells![
            "Option", "Intervention hierarchy", "Scenario", "\u{394}VHT AM (veh\u{b7}h/h)", "\u{394}VHT PM (veh\u{b7}h/h)", format!("Worst junction ({junction})"),
            "Bus route time change AM (min)", "Bus route time change PM (min)", "New cycle facility (km)", "CAS crashes on changed roads", "CAS DSI", "Predicted DSI change",
            "\u{394}VKT AM (veh-km/h, information)", "\u{394}VKT PM (veh-km/h, information)", "\u{394}CO2e AM (t/h, information)", "\u{394}CO2e PM (t/h, information)",
            "Indicative BCR", "IPM band", "Cost (NZ$m)", "Notes"
        ],
    ];
    let blank = || V::String(String::new());
    for r in a.g("rows").items() {
        let cas = r.g("cas");
        rows.push(cells![
            r.g("name"), r.g("label"), r.g("scName"), pv(r, "vht", "AM", &r1), pv(r, "vht", "PM", &r1), or_s(r.g("worst"), ""),
            pv(r, "route", "AM", &|x| r1(x / 60.0)), pv(r, "route", "PM", &|x| r1(x / 60.0)),
            if !r.g("cycleKm").nullish() { V::Number(r3(r.g("cycleKm").n())) } else { blank() },
            if cas.t() { cas.g("crashes").clone() } else { blank() }, if cas.t() { cas.g("dsi").clone() } else { blank() },
            if !r.g("dsi").nullish() { r.g("dsi").clone() } else { V::String("not modelled".into()) },
            pv(r, "vkt", "AM", &r1), pv(r, "vkt", "PM", &r1), pv(r, "co2", "AM", &r3), pv(r, "co2", "PM", &r3), or_s(r.g("bcr"), ""), or_s(r.g("ipm"), ""),
            if !r.g("cost").nullish() { V::Number(r3(r.g("cost").n())) } else { blank() }, r.g("notes")
        ]);
    }
    rows.push(vec![]);
    for w in a.g("warnings").items() {
        rows.push(cells!["Warning", w]);
    }
    rows.push(cells!["Label", label("vkt")]);
    rows.push(cells!["Junction results", disclaimer("D3", p, None)]);
    stamped(&csv(&rows), p, None)
}

/// `planChangeCSV(P)`: the plan change s32 pack: process, s32 table, policy rows, road function table, full build-out and the cl 23 pack.
pub fn plan_change_csv(p: &V) -> String {
    let pc = plan_change(p);
    let bo = pc.g("buildout");
    let mut rows: Vec<Vec<V>> = vec![cells!["Plan change process"]];
    for x in pc.g("process").items() {
        rows.push(cells![if x.g("level").eq_s("warn") { "Warning" } else { "Note" }, x.g("text")]);
    }
    rows.push(vec![]);
    rows.push(cells!["s32 transport table", pc.g("s32Label")]);
    rows.push(cells!["Option", "Hierarchy", "Travel time", "Safety", "Vehicle operating cost", "Infrastructure cost and timing", "Risk of acting", "Risk of not acting"]);
    for r in pc.g("s32").items() {
        rows.push(cells![r.g("name"), r.g("label"), r.g("travel"), r.g("safety"), r.g("voc"), r.g("cost"), r.g("acting"), r.g("notActing")]);
    }
    rows.push(vec![]);
    rows.push(cells!["Policy rows"]);
    rows.push(cells!["Reference", "Policy", "Spare Construct evidence"]);
    for r in pc.g("policy").items() {
        rows.push(cells![r.g("ref"), r.g("policy"), r.g("text")]);
    }
    rows.push(vec![]);
    rows.push(cells!["Draft road function table"]);
    rows.push(cells!["Road", "Class", "Draft function", "RASF", "Future Connect (general traffic)", "Limit (km/h)", "Lanes", "Cycling", "Status"]);
    for r in pc.g("roads").items() {
        rows.push(cells![r.g("name"), r.g("cls"), r.g("fn"), r.g("rasf"), r.g("fc"), r.g("speed"), r.g("lanes"), r.g("cycle"), r.g("status")]);
    }
    rows.push(vec![]);
    rows.push(cells!["Full build-out", bo.g("label"), format!("{} dwellings", bo.g("dwellings").s()), format!("{} m\u{b2} GFA", bo.g("gfa").s()), bo.g("rule")]);
    rows.push(vec![]);
    rows.push(cells!["Sch 1 cl 23 response pack"]);
    rows.push(cells!["Item", "Status"]);
    for r in pc.g("cl23").items() {
        rows.push(cells![r.ix(0), r.ix(1)]);
    }
    stamped(&csv(&rows), p, None)
}

/* =====================================================================================
   Report sections
   ===================================================================================== */

/// `altTable(a)`: the multi-criteria table of the options (against the do-minimum).
pub fn alt_table(a: &V) -> String {
    let head = [
        th("Option"), th("Hierarchy"), th("\u{394}VHT veh\u{b7}h/h"), th("Worst junction"), th("Bus time \u{394} min"), thn("New cycle km"), th("CAS (DSI)"), th("Predicted DSI"),
        th("\u{394}VKT veh-km/h"), th("\u{394}CO2e t/h"), th("BCR"), th("IPM band"), thn("Cost"),
    ];
    let dash = || fmt::DASH.to_string();
    let rows: Vec<Vec<String>> = a
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            if r.g("ran").t() {
                let mut route = V::object();
                for (pd, v) in r.g("pp").g("route").g("by").entries() {
                    route.set(pd, V::Number(v.n() / 60.0));
                }
                let route_pp = jobj! { "by" => route };
                let cas = r.g("cas");
                vec![
                    esc(r.g("name")), esc(r.g("label")), esc_s(&pp_text(r.g("pp").g("vht"), 1, "")), esc(r.g("worst")), esc_s(&pp_text(&route_pp, 1, "")), num(r.g("cycleKm"), 2),
                    if cas.t() { format!("{} ({})", cas.g("crashes").s(), cas.g("dsi").s()) } else { dash() },
                    if !r.g("dsi").nullish() { fmt::signed(r.g("dsi").n(), 1) } else { "not modelled".to_string() },
                    esc_s(&pp_text(r.g("pp").g("vkt"), 0, "")), esc_s(&pp_text(r.g("pp").g("co2"), 2, "")), esc(r.g("bcr")), esc(r.g("ipm")),
                    fmt::money(if r.g("cost").nullish() { f64::NAN } else { r.g("cost").n() }),
                ]
            } else {
                let mut v = vec![esc(r.g("name")), esc(r.g("label")), "Run this scenario and the do-minimum".to_string()];
                v.extend(std::iter::repeat_n(String::new(), 10));
                v
            }
        })
        .collect();
    tbl_th(&head, rows, "No options.")
}

fn report_alternatives(p: &V, ctx: Option<&dyn ReportCtx>) -> String {
    let a = alternatives(p);
    if a.g("rows").jlen() == 0 {
        return String::new();
    }
    let note = format!(
        "Values per modelled peak hour, each period on its own. {}. VKT and CO2e: {}. Junction results: {}.",
        esc_s(&xs("HIER_SRC")), esc_s(&label("vkt")), esc_s(&label("junction"))
    );
    let mut out = rt(ctx, "Options against the intervention hierarchy", &alt_table(&a), &note);
    out.push_str(&disc("D3", p, None));
    for w in a.g("warnings").items() {
        out.push_str(&format!("<p class=\"note warn\">{}</p>", esc(w)));
    }
    for s in a.g("sections").items().iter().filter(|s| s.g("include").t()) {
        out.push_str(&format!("<p class=\"note\">{} ({}): {}</p>", esc(s.g("title")), esc(s.g("ref")), esc(s.g("why"))));
    }
    out
}

fn report_plan_change(p: &V, ctx: Option<&dyn ReportCtx>) -> String {
    let pc = plan_change(p);
    if !pc.g("kind").t() {
        return String::new();
    }
    let mut out = String::new();
    for x in pc.g("process").items() {
        out.push_str(&format!("<p class=\"note{}\">{}</p>", if x.g("level").eq_s("warn") { " warn" } else { "" }, esc(x.g("text"))));
    }
    let s32_rows: Vec<Vec<String>> =
        pc.g("s32").items().iter().map(|r| ["name", "travel", "safety", "voc", "cost", "acting", "notActing"].iter().map(|k| esc(r.g(k))).collect()).collect();
    let s32 = tbl(&["Option", "Travel time", "Safety", "VOC", "Cost and timing", "Risk of acting", "Risk of not acting"], s32_rows, "Run the options.");
    out.push_str(&rt(ctx, "s32 transport evaluation", &s32, &format!("<b>{}</b>", esc(pc.g("s32Label")))));
    let pol_rows: Vec<Vec<String>> = pc.g("policy").items().iter().map(|r| vec![esc(r.g("ref")), esc(r.g("policy")), esc(r.g("text"))]).collect();
    let bo = pc.g("buildout");
    out.push_str("\n        ");
    out.push_str(&rt(ctx, "Policy matters", &tbl(&["Reference", "Policy", "Evidence"], pol_rows, ""), ""));
    out.push_str(&format!(
        "<p class=\"note\">Full build-out: {} dwellings, {} m\u{b2} GFA ({}; {}).</p>",
        fmt::int(bo.g("dwellings").n()), fmt::int(bo.g("gfa").n()), esc(bo.g("label")), esc(bo.g("rule"))
    ));
    out
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-alternatives", "compliance-extra-planchange"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    match id {
        "compliance-extra-alternatives" => Some(report_alternatives(p, ctx)),
        "compliance-extra-planchange" => Some(report_plan_change(p, ctx)),
        _ => None,
    }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-options", "compliance-extra-planchange"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    match id {
        "compliance-extra-options" => Some(options_csv(p)),
        "compliance-extra-planchange" => Some(plan_change_csv(p)),
        _ => None,
    }
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    match id {
        "compliance-extra-options" => Some(alternatives(p).g("rows").jlen() > 0),
        "compliance-extra-planchange" => Some(true),
        _ => None,
    }
}
