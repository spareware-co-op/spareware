//! The project's compliance record, the regulatory regime and the version stamp (packs/nz/compliance.js sections 2 and 3).
//!
//! The record lives in the project as `compliance` (see [`blank`]); [`state`] gives it with the defaults filled in, and the
//! functions that remember something ([`record_outputs`]) change the project. Everything here reads the project the way the
//! prototype does, as loose JSON.

use super::data::{c as table, core};
use super::dates::{long_date, long_date_v, today, valid_day};
use super::jv::{Jx, V, jarr, jobj, merge, obj_at, slice, st};
use super::rx::rx;

/// The version date of the regulatory data (`VERSION`).
pub fn version() -> &'static str {
    table("VERSION").as_str().unwrap_or("")
}

/// The label of an assumption the author can change (`ASSUMPTION`).
pub fn assumption() -> &'static str {
    table("ASSUMPTION").as_str().unwrap_or("")
}

/// A project's blank compliance record (`blank()`): the defaults every entry starts from.
pub fn blank() -> V {
    jobj! {
        "v" => 1,
        "ctx" => jobj! { "pathway" => "", "lodgement" => "", "lodgementActual" => false, "activityStatus" => "", "region" => "auckland", "landInterest" => "", "cl82Date" => "", "transferDate" => "", "notificationDate" => "",
            "occupation" => V::Null, "context" => "urban", "zoningConsistent" => true, "vest" => false, "raCouncil" => false, "devHvPct" => V::Null },
        "site" => jobj! { "lat" => V::Null, "lng" => V::Null, "src" => "", "lookup" => V::Null, "zoneOverride" => "", "precinctOverride" => "", "pc120Override" => "", "zoneTA" => false, "unlisted" => false, "school" => "primary", "driveThrough" => false,
            "fdsOverride" => jobj! { "name" => "", "timing" => "", "source" => "", "page" => "" } },
        "q" => V::object(), "access" => V::Array(vec![]), "net" => jobj! { "sh" => V::object(), "shUser" => V::object(), "aupArterial" => V::object(), "loaded" => V::object() },
        "overlays" => V::Null, "peds" => V::object(), "mwConfirm" => V::object(),
        "scoping" => V::object(), "proportion" => jobj! { "text" => "", "confirmed" => false, "by" => "", "date" => "" },
        "fds" => jobj! { "infra" => V::Array(vec![]), "fundingGap" => "", "rsp" => jobj! { "name" => "", "date" => "" }, "pair" => V::Null },
        "rfi" => jobj! { "events" => V::Array(vec![]), "hearing" => "", "worksDate" => "" },
        "expert" => jobj! { "author" => jobj! { "name" => "", "quals" => "", "org" => "" }, "signoffs" => V::Array(vec![]), "history" => V::Array(vec![]), "confidence" => jobj! { "level" => "", "why" => "" }, "limitations" => "", "gaps" => "", "tests" => "", "supplementary" => V::Null },
        "diff" => jobj! { "text" => "", "items" => V::Array(vec![]), "result" => V::Null, "other" => "" },
        "checks" => V::object(), "notes" => V::object(), "used" => V::object(), "outputs" => V::Null, "shown" => V::object(),
    }
}

/// `fill(dst, def)`: every field of def that dst lacks is copied in, and an object field is filled the same way.
pub fn fill(dst: &mut V, def: &V) {
    for (k, v) in def.entries() {
        let missing = !dst.has(k);
        if missing {
            dst.set(k, v.clone());
        } else if v.is_obj() && dst.g(k).is_obj() {
            if let Some(d) = dst.get_mut(k) {
                fill(d, v);
            }
        }
    }
}

/// `state(P)`: the project's compliance record with the defaults filled in (the project is not changed).
pub fn state(p: &V) -> V {
    let mut c = if p.g("compliance").is_obj() {
        p.g("compliance").clone()
    } else {
        V::object()
    };
    fill(&mut c, &blank());
    c
}

/// `state(P)` for changing: the project's compliance record, created and filled with the defaults first.
pub fn state_mut(p: &mut V) -> &mut V {
    if !p.g("compliance").is_obj() {
        p.set("compliance", V::object());
    }
    let b = blank();
    let c = p.get_mut("compliance").expect("set above");
    fill(c, &b);
    c
}

/// `S_(P)`: the regulatory settings (affected operator thresholds, tier 1 area, evidence radius, extra precincts), filled with the defaults.
pub fn settings(p: &V) -> V {
    let mut o = jobj! { "dvc" => table("OPS.dvc"), "ddelay" => table("OPS.ddelay"), "vicinity" => table("E27.tier1Area.m"), "evidence" => table("ACCESS.evidenceM"), "precincts" => "" };
    let own = p.g("settings").g("compliance");
    if own.is_obj() {
        for (k, v) in own.entries() {
            o.set(k, v.clone());
        }
    }
    o
}

/// `activeSc(P)`: the active scenario (the first when none is named).
pub fn active_sc(p: &V) -> &V {
    let scs = p.g("scenarios").items();
    scs.iter()
        .find(|s| s.g("id").strict_eq(p.g("active")))
        .or(scs.first())
        .unwrap_or(&super::jv::NULL)
}

/// `baseSc(P)`: the base scenario (the first when none is named).
pub fn base_sc(p: &V) -> &V {
    let scs = p.g("scenarios").items();
    scs.iter()
        .find(|s| s.g("id").strict_eq(p.g("base")))
        .or(scs.first())
        .unwrap_or(&super::jv::NULL)
}

/// `resOf(sc, period)`: the results of a scenario for a period (null when none).
pub fn res_of<'a>(sc: &'a V, period: &str) -> Option<&'a V> {
    let r = sc.g("res").g(period);
    if r.t() { Some(r) } else { None }
}

/// `periodsOf(sc)`: the modelled peaks that have results, in AM, PM order.
pub fn periods_of(sc: &V) -> Vec<&'static str> {
    ["AM", "PM"]
        .into_iter()
        .filter(|p| res_of(sc, p).is_some())
        .collect()
}

/// `PATHWAYS[pathway]`: the pathway's entry (None when the pathway is not set or unknown).
pub fn pathway_entry(key: &V) -> Option<&'static V> {
    let t = table("PATHWAYS");
    let k = key.s();
    if t.has(&k) { Some(t.g(&k)) } else { None }
}

/// `pw(P)`: the entry of the project's pathway.
pub fn pw(p: &V) -> Option<&'static V> {
    pathway_entry(state(p).g("ctx").g("pathway"))
}

/// `inAuckland(P)`: the project is in Auckland (every region but "other").
pub fn in_auckland(p: &V) -> bool {
    !state(p).g("ctx").g("region").eq_s("other")
}

/// `esc` of the compliance tab: null and undefined print as nothing, then HTML-escaped.
pub fn esc(v: &V) -> String {
    crate::util::esc(&v.sn())
}

/// `esc` of text.
pub fn esc_s(s: &str) -> String {
    crate::util::esc(s)
}

/// `num(v, d)`: a number as the reports print it, an en dash for null and non-finite values.
pub fn num(v: &V, d: usize) -> String {
    if v.nullish() || !v.n().is_finite() {
        crate::util::fmt::DASH.to_string()
    } else {
        crate::util::fmt::num(v.n(), d)
    }
}

/// `num(x, d)` for a number.
pub fn numf(x: f64, d: usize) -> String {
    num(&V::Number(x), d)
}

/// The day part of a lodgement date held as text (`c.lodgement.slice(0, 10)`).
fn day10(v: &V) -> String {
    slice(&v.s(), 0, Some(10))
}

/* ====================================================================================================
3. Regime and version stamp (reg-regime-stamp)
==================================================================================================== */

/// `regime(P)`: which law governs the project: the pathway, the lodgement date and the Planning Act 2026 transition.
pub fn regime(p: &V) -> V {
    let c = state(p);
    let ctx = c.g("ctx");
    let pe = pathway_entry(ctx.g("pathway"));
    let d: Option<String> = if valid_day(ctx.g("lodgement")) {
        Some(day10(ctx.g("lodgement")))
    } else {
        None
    };
    let reg = table("REGIME");
    let std_v = reg
        .g("specifiedTransitionDate")
        .g(if ctx.g("region").eq_s("other") {
            "other"
        } else {
            "auckland"
        });
    let std_text = if std_v.t() {
        format!("Specified transition date: {}", long_date_v(std_v))
    } else {
        let long = long_date_v(reg.g("mustBeAfter"));
        let short =
            rx(r"^(\d+) (\w{3})\w* ", "").replace(&long, |m| format!("{} {} ", m.get(1), m.get(2)));
        format!("Specified transition date: not yet set (must be after {short})")
    };
    let label = pe.map_or(V::String("Pathway not set".into()), |e| {
        e.g("label").clone()
    });
    let base = jobj! { "pathway" => ctx.g("pathway"), "pathwayLabel" => label, "date" => d.clone().map_or(V::Null, V::String), "transitionText" => std_text, "side" => V::Null, "clause" => V::Null };
    let Some(pe) = pe else {
        return merge(
            &base,
            &jobj! { "code" => "unset", "text" => "Choose the pathway to set the regime.", "short" => "Pathway not set" },
        );
    };
    if !pe.g("proc").t() {
        return merge(
            &base,
            &jobj! { "code" => "other", "text" => format!("Not an RMA process: {}.", pe.g("law").sn()), "short" => pe.g("law") },
        );
    }
    let Some(d) = d else {
        return merge(
            &base,
            &jobj! { "code" => "undated", "text" => "Enter the lodgement date (planned or actual) to set the regime.", "short" => "Lodgement date not set" },
        );
    };
    let std_set = std_v.t().then(|| std_v.s());
    let side = if d.as_str() < reg.g("commence").as_str().unwrap_or("") {
        "pre"
    } else if std_set.as_deref().is_some_and(|s| d.as_str() >= s) {
        "after"
    } else {
        "transition"
    };
    let nor = pe.g("proc").eq_s("nor");
    let kind = pe.g("kind");
    if side == "after" {
        return merge(
            &base,
            &jobj! { "side" => side, "code" => "planning-act", "text" => "Planning Act 2026 regime (after the specified transition date). Spare Construct does not yet cover it: get planning advice.", "short" => "Planning Act 2026" },
        );
    }
    if kind.eq_s("ftaa") {
        let clause = if side == "pre" {
            if nor {
                "FTAA Sch 1 cl 19"
            } else {
                "FTAA Sch 1 cl 17"
            }
        } else if nor {
            "FTAA Sch 1 cl 20"
        } else {
            "FTAA Sch 1 cl 18"
        };
        return merge(
            &base,
            &jobj! { "side" => side, "clause" => clause, "code" => format!("ftaa-{side}"),
            "text" => if side == "pre" { format!("FTAA and RMA as when made ({clause})") } else { format!("FTAA as at lodgement, with the RMA as amended by the Planning Act 2026 ({clause})") },
            "short" => if side == "pre" { "FTAA as when made" } else { "FTAA, transition RMA" } },
        );
    }
    if kind.eq_s("pc") {
        return merge(
            &base,
            &jobj! { "side" => side, "code" => format!("plan-{side}"), "clause" => V::Null,
            "text" => if side == "pre" { "RMA plan-change process as in force before 30 Oct 2026 (check the Planning Act 2026 Sch 1 transition for plan changes)" } else { "RMA plan-change process as amended by the Planning Act 2026 Sch 12 Pt 2, including the plan stop (ss80P-80W) and s80YA" },
            "short" => if side == "pre" { "Pre-assent RMA" } else { "Transition RMA" } },
        );
    }
    let clause = if side == "pre" {
        if nor {
            "Planning Act Sch 1 cl 20"
        } else {
            "Planning Act Sch 1 cl 18"
        }
    } else if nor {
        "Planning Act Sch 1 cl 21"
    } else {
        "Planning Act Sch 1 cl 19"
    };
    merge(
        &base,
        &jobj! { "side" => side, "clause" => clause, "code" => format!("rma-{side}"),
        "text" => if side == "pre" { format!("RMA as in force immediately before 30 Sep 2026 ({clause})") } else { format!("RMA as at lodgement incl. Planning Act Sch 12 Pt 2 ({clause})") },
        "short" => if side == "pre" { "Pre-assent RMA" } else { "Transition RMA" } },
    )
}

/// `stamp(P, now)`: the version stamp printed on the title page and in every exported table footer.
///
/// `now` is the date the stamp is made on (see [`today`]): a day as text, a timestamp, or null for the clock.
pub fn stamp(p: &V, now: &V) -> V {
    let r = regime(p);
    let c = state(p);
    let d = today(now);
    let side = r.g("side");
    let rma: String = if side.eq_s("pre") {
        "as in force immediately before 30 Sep 2026".into()
    } else if side.eq_s("transition") {
        format!("as at {}", long_date_v(r.g("date")))
    } else {
        "\u{2013}".into()
    };
    let ftaa = "as at 10 Jul 2026";
    // an edition whose next known change has passed is marked, so a stamp printed later does not look current
    let ed = |id: &str, e: &str| -> String {
        let s = table("STANDARDS")
            .items()
            .iter()
            .find(|x| x.g("id").eq_s(id));
        let n = s.map(|s| s.g("next")).unwrap_or(&super::jv::NULL);
        if n.t() && n.g("date").t() && d.as_str() >= n.g("date").s().as_str() {
            format!("{e} (changed {}: check)", long_date_v(n.g("date")))
        } else {
            e.to_string()
        }
    };
    let docs: Vec<(String, String)> = table("STAMP_DOCS")
        .items()
        .iter()
        .map(|r| (r.ix(1).s(), ed(&r.ix(0).s(), &r.ix(2).s())))
        .collect();
    let mut lines: Vec<V> = vec![
        jarr!["Pathway", r.g("pathwayLabel")],
        jarr![
            "Lodgement",
            if r.g("date").t() {
                format!(
                    "{}{}",
                    long_date_v(r.g("date")),
                    if c.g("ctx").g("lodgementActual").t() {
                        ""
                    } else {
                        " (planned)"
                    }
                )
            } else {
                "not set".into()
            }
        ],
        jarr!["Regime", r.g("text")],
        jarr!["RMA version", rma.as_str()],
        jarr!["FTAA version", ftaa],
        jarr!["Planning Act", r.g("transitionText")],
    ];
    for (n, e) in &docs {
        lines.push(jarr![n.as_str(), e.as_str()]);
    }
    lines.push(jarr![
        "Spare Construct regulatory data",
        format!("as at {}", long_date(version()))
    ]);
    let clause = if r.g("clause").t() {
        format!(" ({})", r.g("clause").s())
    } else {
        String::new()
    };
    let rma_part = if rma != "\u{2013}" {
        format!("RMA {rma} \u{b7} ")
    } else {
        String::new()
    };
    let text = format!(
        "{}{} \u{b7} {}FTAA {} \u{b7} {} \u{b7} ",
        r.g("short").sn(),
        clause,
        rma_part,
        ftaa,
        r.g("transitionText").sn()
    ) + &docs
        .iter()
        .map(|(n, e)| format!("{n} {e}"))
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
        + &format!(" \u{b7} Spare Construct regulatory data {}", version());
    jobj! { "regime" => r, "lines" => V::Array(lines), "text" => &text, "footer" => format!("Version stamp: {text}"), "at" => d }
}

/// `usedDocs(P)`: the standards an output relies on: the stamped documents plus the pathway's own law.
pub fn used_docs(p: &V) -> Vec<String> {
    let pe = pw(p);
    let mut out: Vec<String> = Vec::new();
    for r in table("STAMP_DOCS").items() {
        let id = r.ix(0).s();
        if !out.contains(&id) {
            out.push(id);
        }
    }
    if let Some(pe) = pe {
        let law = table("LAW_DOCS").g(&pe.g("kind").s());
        for id in law.items() {
            let id = id.s();
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// `recordOutputs(P, now)`: remembers the lodgement date and regime that outputs were generated under, and the date each document
/// they rely on was last used (called after each model run and export). Changes the project.
pub fn record_outputs(p: &mut V, now: &V) -> V {
    let r = regime(p);
    let d = today(now);
    let ids = used_docs(p);
    let c = state_mut(p);
    c.set("outputs", jobj! { "at" => d.as_str(), "lodgement" => r.g("date"), "side" => r.g("side"), "regime" => r.g("code") });
    let used = obj_at(c, "used");
    for id in ids {
        used.set(id, st(d.clone()));
    }
    c.g("outputs").clone()
}

/// `recheck(P)`: the re-check banner: outputs were generated before the lodgement date moved across 30 Oct 2026 (RMA pathways only).
pub fn recheck(p: &V) -> V {
    let c = state(p);
    let o = c.g("outputs");
    let r = regime(p);
    let pe = pathway_entry(c.g("ctx").g("pathway"));
    let commence = table("REGIME.commence").s();
    let side_of = |d: &V| -> Option<&'static str> {
        if d.t() {
            Some(if d.s() < commence { "pre" } else { "post" })
        } else {
            None
        }
    };
    let was = if o.t() {
        side_of(o.g("lodgement"))
    } else {
        None
    };
    let now_side = side_of(r.g("date"));
    let show = was.is_some()
        && now_side.is_some()
        && was != now_side
        && pe.is_some_and(|e| e.g("proc").t());
    jobj! { "show" => show, "text" => if show { table("REGIME.recheck").clone() } else { st("") }, "since" => if o.t() { o.g("at").clone() } else { V::Null } }
}

/// The compliance data tables (for tests and callers that print them).
pub fn data_tables() -> &'static V {
    core()
}
