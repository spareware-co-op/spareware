//! Effects, the mitigation register, condition parameters and the trigger finder (compliance-extra.js section 7).
//!
//! The effects of a scenario are found from the difference between the do-minimum and the option; each measure of the register addresses one
//! of them; each condition parameter row needs a modelled effect (s108AA(1)(b)) or the applicant's agreement (s108AA(1)(a)). Spare Construct
//! never writes condition wording, only parameters. The trigger finder ([`trigger`]) steps the build-out and finds where the criteria first fail.

use super::ctx::{self, esc, num, stamped};
use super::shared::network_diff;
use crate::compliance::data::x as xdata;
use crate::compliance::html::{ReportCtx, kit_table, rt, th, tr};
use crate::compliance::jv::{Jx, V, arr_at, csv, jobj};
use crate::compliance::net;
use crate::graph::js_sort_by;
use crate::jsnum::{math_min, math_round, number_to_js_string, to_int32, truthy_number};
use crate::util::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

mod trigger;
pub use trigger::{anop_fit_of, fails_at, ordinal, storage_of, trigger_finder, trigger_tables};

/// `${o[k]}` of a loose object in a template string: "undefined" when the field is missing.
fn tstr(o: &V, k: &str) -> String {
    if o.has(k) { o.g(k).s() } else { "undefined".to_string() }
}

/// `a || b` of two fields of an object in a template string (`${o[a] || o[b]}`).
fn or_str(o: &V, a: &str, b: &str) -> String {
    if o.g(a).t() { o.g(a).s() } else { tstr(o, b) }
}

/// `h(t)`: a heading of the Compliance+ tab and its hand-over tables.
pub fn h(t: &str) -> String {
    format!("<h4>{t}</h4>")
}

/// `TABLE[v] || v || fallback` for a value looked up in a table of labels (the label, else the value itself, else the fallback).
fn label_or(table: &V, v: &V, fallback: &str) -> V {
    if v.t() && table.g(&v.s()).t() {
        table.g(&v.s()).clone()
    } else if v.t() {
        v.clone()
    } else {
        V::String(fallback.to_string())
    }
}

/// `committedText(v)`: Y when the applicant commits, N when not, "Not recorded" otherwise.
pub fn committed_text(v: &V) -> &'static str {
    match v {
        V::Bool(true) => "Y",
        V::Bool(false) => "N",
        _ => "Not recorded",
    }
}

static UID: AtomicU64 = AtomicU64::new(1);

/// `U.uid(prefix)`: an id that is new each time. The prototype's has a random part, so these cannot match it: they are the prefix and a counter.
fn uid(prefix: &str) -> String {
    format!("{prefix}{:08}", UID.fetch_add(1, Ordering::Relaxed))
}

/// `SameValueZero` of two loose values where a missing field is `undefined` (None).
fn same(a: Option<&V>, b: Option<&V>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.strict_eq(y),
        _ => false,
    }
}

/// `x[k] = v` for a value that may be undefined (None): an undefined field is not kept (it has no JSON form).
fn set_opt(o: &mut V, k: &str, v: Option<&V>) {
    match v {
        Some(x) => o.set(k, x.clone()),
        None => {
            o.remove(k);
        }
    }
}

/* =====================================================================================
   Effects
   ===================================================================================== */

/// The sort key of an effect (`x.dos || x.vc || 0`).
fn effect_key(x: &V) -> f64 {
    let d = x.g("dos").n();
    if truthy_number(d) {
        return d;
    }
    let c = x.g("vc").n();
    if truthy_number(c) { c } else { 0.0 }
}

/// `effects(P, sc)`: the effects found in the scenario difference (do-minimum to option; `sc` is a scenario of the project, which is compared with the
/// do-minimum by identity, and the option by default), worst first: junctions whose degree of saturation is over
/// 0.9 (or their own practical value), that reach LoS E or F or whose delay rises 10 s or more, and roads whose v/c is over 0.9 and rises 0.05 or more.
/// `[{ id, kind, ... text }]`, with the ids `J:<node>:<period>` and `L:<arc key>:<period>`.
pub fn effects(p: &V, sc: Option<&V>) -> V {
    let pr = ctx::pair(p);
    let opt: &V = match sc {
        Some(s) if s.t() => s,
        _ => pr.opt,
    };
    let base = pr.base;
    let eff = xdata("EFFECT");
    let (e_dos, e_dd, e_vc, e_dvc) = (eff.g("dos").n(), eff.g("dDelay").n(), eff.g("vc").n(), eff.g("dvc").n());
    let mut out: Vec<V> = Vec::new();
    for per in ctx::periods_of(opt) {
        let ro = opt.g("res").g(per);
        let rb: Option<&V> = if base.t() && !std::ptr::eq(base, opt) { ctx::res_of(base, per) } else { None };
        for (id, nr) in ro.g("nodes").entries() {
            let b = rb.map(|r| r.g("nodes").g(id)).filter(|b| b.t());
            let d_d: Option<f64> = b.map(|b| nr.g("delay").n() - b.g("delay").n());
            let xp = if truthy_number(nr.g("xp").n()) { nr.g("xp").n() } else { e_dos };
            let los = if nr.g("los").t() { nr.g("los").s() } else { String::new() };
            if !(nr.g("dos").n() > math_min(xp, e_dos) || los.contains('E') || los.contains('F') || d_d.is_some_and(|d| d >= e_dd)) {
                continue;
            }
            let mut e = jobj! { "id" => format!("J:{id}:{per}"), "kind" => "junction", "node" => id, "period" => per };
            for k in ["dos", "los", "delay"] {
                if let Some(v) = nr.get(k) {
                    e.set(k, v.clone());
                }
            }
            e.set("dDelay", d_d.map_or(V::Null, V::Number));
            let against = match d_d {
                Some(d) => format!(", {} s against {}", fmt::signed(d, 1), tstr(base, "name")),
                None => String::new(),
            };
            e.set("text", V::String(format!("{} {}: DoS {}, LoS {}{}", ctx::node_name(opt, id), per, num(nr.g("dos"), 2), or_str(nr, "losText", "los"), against)));
            out.push(e);
        }
        for (key, a) in ro.g("arcs").entries() {
            let b = rb.map(|r| r.g("arcs").g(key)).filter(|b| b.t());
            let dvc = match b {
                Some(b) => a.g("vc").n() - b.g("vc").n(),
                None => a.g("vc").n(),
            };
            if !(a.g("vc").n() > e_vc && dvc >= e_dvc) {
                continue;
            }
            let l = opt.g("links").g(&ctx::arc_link(key));
            let dir = if key.ends_with(":ab") { "A\u{2192}B" } else { "B\u{2192}A" };
            let mut e = jobj! { "id" => format!("L:{key}:{per}"), "kind" => "link", "key" => key, "period" => per };
            if let Some(v) = a.get("vc") {
                e.set("vc", v.clone());
            }
            e.set("text", V::String(format!("{} ({}) {}: v/c {}", ctx::link_name(l), dir, per, num(a.g("vc"), 2))));
            out.push(e);
        }
    }
    js_sort_by(&mut out, |x, y| effect_key(y) - effect_key(x));
    V::Array(out)
}

/* =====================================================================================
   Mitigation register and condition parameters
   ===================================================================================== */

/// `blankMit()`: a new row of the mitigation register.
pub fn blank_mit() -> V {
    jobj! { "id" => uid("mit"), "ref" => "", "measure" => "", "effect" => "", "options" => "", "nonInfra" => false, "trigger" => "", "responsible" => "applicant", "cost" => V::Null,
        "funder" => "applicant", "rltp" => "", "mechanism" => "condition", "committed" => V::Null, "landOutside" => false }
}

/// `blankCond()`: a new row of the condition parameters.
pub fn blank_cond() -> V {
    jobj! { "id" => uid("cond"), "mitigation" => "", "metric" => "dwellings occupied", "threshold" => "", "survey" => "", "frequency" => "", "response" => "", "effect" => "", "agreed" => false, "review" => "" }
}

/// `x && x !== 'applicant'` for a field that may be missing: the value itself when it is falsy, else whether it names a third party.
fn names_third_party(v: Option<&V>) -> Option<V> {
    let x = v?;
    Some(if x.t() { V::Bool(!x.eq_s("applicant")) } else { x.clone() })
}

/// `mitigationTable(P)`: the mitigation register rows (AT ITA Appendix 3), each with its effect text and the limited-consideration tag: the row's own
/// fields, then `ref` (M1, M2 ... when blank), `effectText`, `effectFound`, `limited`, `tag`, `mechanismText`, `responsibleText` and `s84a`.
pub fn mitigation_table(p: &V) -> V {
    let cx = ctx::data(p);
    let eff = effects(p, None);
    let ftaa = ctx::context(p).g("ftaa").t();
    let (mechanisms, responsible) = (xdata("MECHANISMS"), xdata("RESPONSIBLE"));
    let labels = xdata("STD.labels");
    let mut rows: Vec<V> = Vec::new();
    for (i, m) in cx.g("mitigation").items().iter().enumerate() {
        // a Map keeps the last effect of an id
        let e = eff.items().iter().rev().find(|e| e.g("id").strict_eq(m.g("effect")));
        let first = names_third_party(m.get("funder"));
        let third = if first.as_ref().is_some_and(|v| v.t()) { first } else { names_third_party(m.get("responsible")) };
        let limited: Option<V> = if m.get("committed").is_some_and(|c| matches!(c, V::Bool(false))) { Some(V::Bool(true)) } else { third };
        let is_limited = limited.as_ref().is_some_and(|v| v.t());
        let mut o = m.clone();
        o.set("ref", if m.g("ref").t() { m.g("ref").clone() } else { V::String(format!("M{}", i + 1)) });
        o.set(
            "effectText",
            match e {
                Some(e) => e.g("text").clone(),
                None if m.g("effect").t() => V::String(format!("{} (no longer in the results: re-run or relink)", m.g("effect").s())),
                None => V::String("Not linked".into()),
            },
        );
        o.set("effectFound", V::Bool(e.is_some()));
        set_opt(&mut o, "limited", limited.as_ref());
        o.set("tag", if is_limited { labels.g("limited").clone() } else { V::String(String::new()) });
        o.set("mechanismText", label_or(mechanisms, m.g("mechanism"), "\u{2013}"));
        o.set("responsibleText", label_or(responsible, m.g("responsible"), "\u{2013}"));
        o.set("s84a", if ftaa && m.g("mechanism").eq_s("s84a") { labels.g("s84a").clone() } else { V::String(String::new()) });
        rows.push(o);
    }
    V::Array(rows)
}

/// `conditionTable(P)`: the condition parameter rows with their nexus: the row's own fields, then `ref` (C1, C2 ...), `measure`, `effect`, `nexus`
/// ("effect", "agreed" or "missing"), `basis`, `review`, `ftaa` and `ok`.
pub fn condition_table(p: &V) -> V {
    let cx = ctx::data(p);
    let eff = effects(p, None);
    let c = ctx::context(p);
    let mits = mitigation_table(p);
    let labels = xdata("STD.labels");
    let mut rows: Vec<V> = Vec::new();
    for (i, r) in cx.g("conditions").items().iter().enumerate() {
        let m = mits.items().iter().find(|x| same(x.get("id"), r.get("mitigation")));
        let effect: V = if r.g("effect").t() {
            r.g("effect").clone()
        } else if m.is_some_and(|m| m.g("effect").t()) {
            m.map_or(V::Null, |m| m.g("effect").clone())
        } else {
            V::String(String::new())
        };
        let in_results = effect.t() && eff.items().iter().any(|e| e.g("id").strict_eq(&effect));
        let nexus = if in_results {
            "effect"
        } else if r.g("agreed").t() {
            "agreed"
        } else {
            "missing"
        };
        let basis = match nexus {
            "effect" => format!("Nexus to modelled effect {} (s108AA(1)(b))", effect.s()),
            "agreed" => "Applicant agreement (s108AA(1)(a))".to_string(),
            _ => "No nexus: link a modelled effect or record the applicant's agreement".to_string(),
        };
        let metric = if r.g("metric").t() { r.g("metric").s() } else { "metric".to_string() };
        let threshold = if r.g("threshold").t() { r.g("threshold").s() } else { "the threshold".to_string() };
        let mut o = r.clone();
        o.set("ref", V::String(format!("C{}", i + 1)));
        match m {
            Some(m) => set_opt(&mut o, "measure", m.get("measure")),
            None => o.set("measure", V::String(String::new())),
        }
        o.set("effect", effect);
        o.set("nexus", V::String(nexus.to_string()));
        o.set("basis", V::String(basis));
        o.set(
            "review",
            if r.g("review").t() { r.g("review").clone() } else { V::String(format!("Review under s128(1)(a) if the monitored {metric} passes {threshold} before the measure is in place")) },
        );
        o.set("ftaa", if c.g("ftaa").t() { labels.g("s84a").clone() } else { V::String(String::new()) });
        o.set("ok", V::Bool(nexus != "missing"));
        rows.push(o);
    }
    V::Array(rows)
}

/// `seedMitigation(P)`: adds rows to the register from what the mitigation scenario changes against the option (junction controls, upgrades and new
/// roads) and from effects no measure addresses yet; never changes a row. Changes the project; returns the number of rows added.
pub fn seed_mitigation(p: &mut V) -> usize {
    let added: Vec<V> = {
        let pv: &V = p;
        let cx = ctx::data(pv);
        let pr = ctx::pair(pv);
        let eff = effects(pv, None);
        let seen: Vec<String> = cx.g("mitigation").items().iter().map(|m| m.g("seed")).filter(|s| s.t()).map(|s| s.s()).collect();
        let mut linked: Vec<String> = cx.g("mitigation").items().iter().filter_map(|m| m.g("effect").as_str().map(|s| s.to_string())).collect();
        let econ = pv.g("settings").g("econ");
        // the capital cost of a change in the mitigation scenario (NZ$m), null when it cannot be priced
        let capital = |sc: &V, d: &V| -> V {
            if d.g("kind").eq_s("node") {
                let v = econ.g("node").g(&sc.g("nodes").g(&d.g("id").s()).g("control").s());
                if v.nullish() { V::Null } else { v.clone() }
            } else {
                let l = sc.g("links").g(&d.g("id").s());
                if !l.t() {
                    return V::Null;
                }
                let lanes = to_int32(l.g("lanesAB").n()) as f64 + to_int32(l.g("lanesBA").n()) as f64;
                let k = econ.g("laneKm").g(&l.g("cls").s());
                V::Number(net::len(sc, l) / 1000.0 * lanes * if k.nullish() { 3.0 } else { k.n() })
            }
        };
        let mut rows: Vec<V> = Vec::new();
        if let Some(mit) = pr.mit.filter(|m| !std::ptr::eq(*m, pr.opt)) {
            for d in network_diff(pr.opt, mit).items() {
                let seed = format!("diff:{}:{}", d.g("kind").s(), d.g("id").s());
                if seen.contains(&seed) {
                    continue;
                }
                let e = if d.g("kind").eq_s("node") {
                    eff.items().iter().find(|x| x.g("node").strict_eq(d.g("id")))
                } else {
                    eff.items().iter().find(|x| x.g("key").t() && V::String(ctx::arc_link(&x.g("key").s())).strict_eq(d.g("id")))
                };
                let mut r = blank_mit();
                r.set("seed", V::String(seed));
                r.set("measure", d.g("text").clone());
                r.set("effect", e.map_or(V::String(String::new()), |e| e.g("id").clone()));
                r.set("cost", capital(mit, d));
                r.set("options", V::String("Seeded from the mitigation scenario: record the options considered, including non-infrastructure ones.".into()));
                rows.push(r);
                if let Some(e) = e {
                    linked.push(e.g("id").s());
                }
            }
        }
        for e in eff.items() {
            let seed = format!("eff:{}", e.g("id").s());
            if seen.contains(&seed) || linked.contains(&e.g("id").s()) {
                continue;
            }
            let mut r = blank_mit();
            r.set("seed", V::String(seed));
            r.set("measure", V::String(String::new()));
            r.set("effect", e.g("id").clone());
            r.set("mechanism", V::String("none".into()));
            r.set("options", V::String("Effect not yet addressed: record a measure or why none is needed.".into()));
            rows.push(r);
        }
        rows
    };
    let n = added.len();
    arr_at(ctx::data_mut(p), "mitigation").extend(added);
    n
}

/// `seedConditions(P)`: adds a condition parameter row for every register measure that has none yet (the threshold from the first trigger found
/// when there is one); never records the applicant's s108AA(1)(a) agreement. Changes the project; returns the number of rows added.
pub fn seed_conditions(p: &mut V) -> usize {
    let added: Vec<V> = {
        let pv: &V = p;
        let cx = ctx::data(pv);
        let have: Vec<Option<&V>> = cx.g("conditions").items().iter().map(|c| c.get("mitigation")).collect();
        let first = cx.g("triggers").g("rows").ix(0).g("first");
        let mut rows: Vec<V> = Vec::new();
        for m in mitigation_table(pv).items() {
            if have.iter().any(|h| same(*h, m.get("id"))) || !m.g("measure").t() {
                continue;
            }
            let mut r = blank_cond();
            set_opt(&mut r, "mitigation", m.get("id"));
            set_opt(&mut r, "effect", m.get("effect"));
            r.set("agreed", V::Bool(false));
            r.set(
                "threshold",
                if first.t() { V::String(format!("{} dwellings", fmt::int(first.g("before").n()))) } else if m.g("trigger").t() { m.g("trigger").clone() } else { V::String(String::new()) },
            );
            r.set("survey", V::String("Building consent and s224(c) records; turning counts at the affected junction".into()));
            r.set("frequency", V::String("Annual".into()));
            r.set("response", V::String(format!("{} operational before the threshold", m.g("measure").s())));
            rows.push(r);
        }
        rows
    };
    let n = added.len();
    arr_at(ctx::data_mut(p), "conditions").extend(added);
    n
}

/* =====================================================================================
   CSV and report
   ===================================================================================== */

/// `r1(v)` of the hand-over packs as text: one decimal, empty for null and values that are not finite.
fn r1t(v: &V) -> String {
    if v.nullish() || !v.n().is_finite() { String::new() } else { number_to_js_string(math_round(v.n() * 10.0) / 10.0) }
}

/// `r3(v)` of the hand-over packs as text: three decimals, empty for null and values that are not finite.
fn r3t(v: &V) -> String {
    if v.nullish() || !v.n().is_finite() { String::new() } else { number_to_js_string(math_round(v.n() * 1000.0) / 1000.0) }
}

/// `mval(x)` of the CSV: a junction's values at a build-out as text ("DoS 0.95, delay 12.3 s, LoS D"), empty when there are none.
fn mval(x: &V) -> String {
    if x.t() { format!("DoS {}, delay {} s, LoS {}", r3t(x.g("dos")), r1t(x.g("delay")), if x.g("los").t() { x.g("los").s() } else { String::new() }) } else { String::new() }
}

/// `mitigationCSV(P)`: the mitigation register, the condition parameters and (when the trigger finder has run) the trigger table and the existing
/// deficiencies as a CSV with the version stamp and disclaimers in footer rows.
pub fn mitigation_csv(p: &V) -> String {
    let m = mitigation_table(p);
    let c = condition_table(p);
    let t = ctx::data(p).g("triggers").clone();
    let s = |x: &str| V::String(x.to_string());
    let strs = |xs: &[&str]| -> Vec<V> { xs.iter().map(|x| s(x)).collect() };
    let responsible = xdata("RESPONSIBLE");
    let mut rows: Vec<Vec<V>> = vec![
        strs(&["Table M1 \u{b7} Mitigation register"]),
        strs(&[
            "Ref", "Measure", "Effect addressed", "Options considered", "Non-infrastructure option", "Trigger", "Responsible", "Cost (NZ$m)", "Funder", "RLTP/LTP status", "Mechanism",
            "Committed by applicant", "Land outside applicant control", "Tag",
        ]),
    ];
    for r in m.items() {
        let tags: Vec<String> = [r.g("tag"), r.g("s84a")].iter().filter(|x| x.t()).map(|x| x.s()).collect();
        rows.push(vec![
            r.g("ref").clone(),
            r.g("measure").clone(),
            r.g("effectText").clone(),
            r.g("options").clone(),
            s(if r.g("nonInfra").t() { "Y" } else { "N" }),
            r.g("trigger").clone(),
            r.g("responsibleText").clone(),
            r.g("cost").clone(),
            label_or(responsible, r.g("funder"), ""),
            r.g("rltp").clone(),
            r.g("mechanismText").clone(),
            s(committed_text(r.g("committed"))),
            s(if r.g("landOutside").t() { "Y" } else { "N" }),
            s(&tags.join("; ")),
        ]);
    }
    rows.push(vec![]);
    rows.push(strs(&["Condition parameters (Spare Construct never writes condition wording)"]));
    rows.push(strs(&["Ref", "Measure", "Trigger metric", "Threshold", "Survey method", "Frequency", "Response", "Nexus or agreement", "Review clause suggestion", "Label"]));
    for r in c.items() {
        rows.push(["ref", "measure", "metric", "threshold", "survey", "frequency", "response", "basis", "review", "ftaa"].iter().map(|k| r.g(k).clone()).collect());
    }
    if t.t() {
        rows.push(vec![]);
        rows.push(vec![s("Table M2 \u{b7} Trigger finder"), t.g("basis").clone(), t.g("label").clone()]);
        rows.push(strs(&["Variant", "First failing build-out (%)", "Dwellings at trigger", "Before dwelling", "Site", "Period", "Why", "Resolved by the upgrade"]));
        for r in t.g("rows").items() {
            let f = r.g("first");
            let or_blank = |k: &str| if f.t() { f.g(k).clone() } else { s("") };
            rows.push(vec![
                r.g("label").clone(),
                if f.t() { f.g("pct").clone() } else { s("none") },
                or_blank("dwellings"),
                or_blank("before"),
                or_blank("name"),
                or_blank("period"),
                if f.t() { f.g("why").clone() } else { r.g("text").clone() },
                if r.g("resolved").t() { r.g("resolved").g("text").clone() } else { s("") },
            ]);
        }
        let mut ex: Vec<Vec<V>> = Vec::new();
        for r in t.g("rows").items() {
            for e in r.g("existing").items() {
                ex.push(vec![
                    r.g("label").clone(),
                    e.g("name").clone(),
                    e.g("period").clone(),
                    s(&mval(e.g("at0"))),
                    s(&mval(e.g("at100"))),
                    if !e.g("worsenedAt").nullish() { s(&format!("{}%", e.g("worsenedAt").s())) } else { s("not worsened beyond the threshold") },
                    e.g("why").clone(),
                ]);
            }
        }
        rows.push(vec![]);
        rows.push(strs(&["Existing deficiencies (fail at 0% build-out)"]));
        rows.push(strs(&["Variant", "Junction", "Period", "At 0%", "At 100%", "Worsened by the development from", "Why it fails at 0%"]));
        if ex.is_empty() {
            rows.push(strs(&["None"]));
        } else {
            rows.extend(ex);
        }
    }
    stamped(&csv(&rows), p, None)
}

/// The mitigation register and condition parameters of the report (section "compliance-extra-mitigation"); an empty string when there are none.
fn mitigation_report(p: &V, rctx: Option<&dyn ReportCtx>) -> String {
    let m = mitigation_table(p);
    let cd = condition_table(p);
    if m.items().is_empty() && cd.items().is_empty() {
        return String::new();
    }
    let responsible = xdata("RESPONSIBLE");
    let head = ["Ref", "Measure", "Effect addressed", "Options considered", "Trigger", "Responsible", "Cost NZ$m", "Funder", "RLTP/LTP", "Mechanism", "Committed", "Land outside control"].map(th);
    let rows: Vec<_> = m
        .items()
        .iter()
        .map(|r| {
            tr([
                esc(r.g("ref")),
                format!("{}{}", esc(r.g("measure")), if r.g("tag").t() { format!("<br><i>{}</i>", esc(r.g("tag"))) } else { String::new() }),
                esc(r.g("effectText")),
                esc(r.g("options")),
                esc(r.g("trigger")),
                esc(r.g("responsibleText")),
                if !r.g("cost").nullish() { num(r.g("cost"), 2) } else { "\u{2013}".to_string() },
                esc(&label_or(responsible, r.g("funder"), "")),
                esc(r.g("rltp")),
                format!("{}{}", esc(r.g("mechanismText")), if r.g("s84a").t() { format!(" ({})", esc(r.g("s84a"))) } else { String::new() }),
                committed_text(r.g("committed")).to_string(),
                if r.g("landOutside").t() { "Y".to_string() } else { "N".to_string() },
            ])
        })
        .collect();
    let first = rt(rctx, "Mitigation register (AT ITA Appendix 3)", &kit_table(&head, &rows, true, "", ""), &ctx::esc_s(xdata("EFFECT.basis").as_str().unwrap_or("")));
    let second = if cd.items().is_empty() {
        String::new()
    } else {
        let head = ["Ref", "Measure", "Trigger metric", "Threshold", "Survey", "Frequency", "Response", "Nexus", "Review"].map(th);
        let rows: Vec<_> = cd
            .items()
            .iter()
            .map(|r| {
                tr([
                    esc(r.g("ref")),
                    esc(r.g("measure")),
                    esc(r.g("metric")),
                    esc(r.g("threshold")),
                    esc(r.g("survey")),
                    esc(r.g("frequency")),
                    esc(r.g("response")),
                    format!("{}{}", esc(r.g("basis")), if r.g("ftaa").t() { format!("; {}", esc(r.g("ftaa"))) } else { String::new() }),
                    esc(r.g("review")),
                ])
            })
            .collect();
        rt(rctx, "Condition parameters (not condition wording)", &kit_table(&head, &rows, true, "", ""), "")
    };
    format!("{first}\n        {second}")
}

/// The trigger finder's results of the report (section "compliance-extra-triggers"); an empty string when the trigger finder has not run.
fn triggers_report(p: &V, rctx: Option<&dyn ReportCtx>) -> String {
    let cx = ctx::data(p);
    let t = cx.g("triggers");
    if !t.t() {
        return String::new();
    }
    let periods = t.g("periods").items().iter().map(|x| x.sn()).collect::<Vec<_>>().join(" and ");
    let title = format!("Build-out at which the criteria first fail ({}, {})", esc(t.g("scName")), ctx::esc_s(&periods));
    let note = format!("{}. {}. {}", esc(t.g("basis")), esc(t.g("label")), esc(t.g("copy")));
    format!("{}{}", rt(rctx, &title, &trigger_tables(t), &note), ctx::disc("D3", p, None))
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-mitigation", "compliance-extra-triggers"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, rctx: Option<&dyn ReportCtx>) -> Option<String> {
    match id {
        "compliance-extra-mitigation" => Some(mitigation_report(p, rctx)),
        "compliance-extra-triggers" => Some(triggers_report(p, rctx)),
        _ => None,
    }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-mitigation"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's. The export carries author text: the caller
/// refuses it while the linter blocks (see `lint::export_block`), as the prototype's guard does.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-mitigation").then(|| mitigation_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-mitigation").then(|| {
        let cx = ctx::data(p);
        !cx.g("mitigation").items().is_empty() || !cx.g("conditions").items().is_empty()
    })
}
