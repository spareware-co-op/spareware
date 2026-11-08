//! Future Development Strategy consistency (packs/nz/compliance.js section 10).

use super::authority::site_info;
use super::data::{c as table, m as model};
use super::dates::{long_date_v, today};
use super::engine;
use super::jv::{Jx, V, jobj, st, strs};
use super::record::{active_sc, pathway_entry, regime, state, state_mut};
use super::rx::rx;
use crate::util;

/// `fdsCheck(P)` for the active scenario.
pub fn fds_check(p: &V) -> V {
    fds_check_sc(p, active_sc(p))
}

/// `fdsCheck(P, sc)`: the Future Development Strategy consistency check: `{ status, text, occupation, timing, start, red, infra, cite, heading, paragraph, outOfSequence, fundingPrompt, reference, applies, source }`.
pub fn fds_check_sc(p: &V, sc: &V) -> V {
    let c = state(p);
    let site = site_info(p);
    let r = regime(p);
    let pe = pathway_entry(c.g("ctx").g("pathway"));
    let occ_in = c.g("ctx").g("occupation").n();
    let occ: V = if occ_in.is_finite() && occ_in != 0.0 {
        V::Number(occ_in)
    } else if p.g("assessment").g("opening").t() {
        p.g("assessment").g("opening").clone()
    } else {
        V::Number(engine::year_of(p, sc))
    };
    let rsp = if c.g("fds").g("rsp").g("name").t() {
        Some(c.g("fds").g("rsp").clone())
    } else {
        None
    };
    let reference = match &rsp {
        Some(r) => format!(
            "{}{}",
            r.g("name").s(),
            if r.g("date").t() {
                format!(" (decided {})", long_date_v(r.g("date")))
            } else {
                String::new()
            }
        ),
        None => "the Auckland Future Development Strategy 2023\u{2013}2053".to_string(),
    };
    let f = site.g("fds");
    let timing = if f.t() && f.g("timing").t() {
        f.g("timing").s()
    } else {
        String::new()
    };
    let start: V = match rx(r"(\d{4})\s*\+", "").exec(&timing) {
        Some(m) => V::Number(m.get(1).parse::<f64>().unwrap_or(f64::NAN)),
        None => V::Null,
    };
    let mut status = "unknown";
    let text: String;
    let fname = f.g("name").s();
    if !site.g("looked").t() && !f.t() {
        text = "Look up the site to read the FDS future urban areas layer, or enter the FDS area and timing by hand.".into();
    } else if !f.t() {
        status = "outside";
        text = "The site is not in an FDS future urban area (existing urban area or outside the strategy's growth areas).".into();
    } else if rx("removal", "i").test(&timing) {
        status = "removal";
        text = format!("{fname}: the FDS marks this area for removal from future urbanisation.");
    } else if rx("live zoned", "i").test(&timing) {
        status = "live";
        text = format!("{fname}: already live zoned in the FDS.");
    } else if start.t() && occ.t() && occ.n() < start.n() {
        status = "out";
        text = format!(
            "{fname}: FDS timing {timing}, proposed occupation {}: out of sequence.",
            occ.s()
        );
    } else if start.t() {
        status = "in";
        text = format!(
            "{fname}: FDS timing {timing}, proposed occupation {}: in sequence.",
            if occ.t() { occ.s() } else { "?".into() }
        );
    } else {
        text = format!(
            "{fname}: FDS timing {}.",
            if timing.is_empty() {
                "not stated".to_string()
            } else {
                timing.clone()
            }
        );
    }
    let red = rx("red flag", "i").test(&timing);
    let mut infra: Vec<V> = Vec::new();
    for i in c.g("fds").g("infra").items() {
        let row = p
            .g("register")
            .items()
            .iter()
            .find(|x| x.g("id").strict_eq(i.g("reg")));
        let inc = row.map(|r| engine::in_baseline(p, r));
        let mut o = i.clone();
        o.set(
            "name",
            if i.g("name").t() {
                i.g("name").clone()
            } else if row.is_some_and(|r| r.g("name").t()) {
                row.map(|r| r.g("name").clone()).unwrap_or(V::Null)
            } else {
                st("Unnamed project")
            },
        );
        let funding = match row {
            Some(r) if r.g("funding").t() => {
                let f = model("FUNDING").g(&r.g("funding").s());
                if f.t() {
                    f.clone()
                } else {
                    r.g("funding").clone()
                }
            }
            _ => st("not recorded"),
        };
        o.set("funding", funding);
        o.set(
            "included",
            inc.as_ref().map_or(V::Null, |x| x.g("included").clone()),
        );
        o.set(
            "reason",
            inc.as_ref()
                .map_or(st("not in the register"), |x| x.g("reason").clone()),
        );
        infra.push(o);
    }
    let side = r.g("side");
    let trans = side.eq_s("transition") || side.eq_s("after");
    let kind = pe.map_or(String::new(), |e| e.g("kind").s());
    let proc_ = pe.map_or(String::new(), |e| e.g("proc").sn());
    let nor = pe.is_some() && proc_ == "nor";
    let consent = pe.is_some() && proc_ == "consent";
    let ftaa = pe.is_some() && kind == "ftaa";
    let cite: Option<&str> = if !trans {
        None
    } else if nor {
        Some(if c.g("ctx").g("raCouncil").t() {
            "s168A(3C)"
        } else {
            "s171(1D)"
        })
    } else if consent {
        Some("s104(1D)")
    } else {
        None
    };
    let heading = match cite {
        Some(ci) => format!(
            "Consistency with {} (RMA {ci})",
            if rsp.is_some() {
                "the regional spatial plan"
            } else {
                "the Future Development Strategy"
            }
        ),
        None => "Policy context: AT ITA s5.1 out-of-sequence development".to_string(),
    };
    let page = if f.t() && f.g("page").t() {
        format!(
            " ({}, p {})",
            if f.g("source").t() {
                f.g("source").s()
            } else {
                table("SRC.fds.name").s()
            },
            f.g("page").s()
        )
    } else {
        String::new()
    };
    let who = if ftaa {
        "the panel to take into account (through FTAA Sch 5; verify)"
    } else if nor {
        "the territorial authority to consider"
    } else {
        "the consent authority to have regard to"
    };
    let lead = match cite {
        Some(ci) => format!(
            "RMA {ci}, inserted by the Planning Act 2026 Sch 12 Pt 2, requires {who} whether the proposal is consistent with {reference}."
        ),
        None => format!(
            "The AT ITA Guidelines 2025 v2 (s5.1) ask out-of-sequence development to show how its transport needs will be met, with {reference} as the reference."
        ),
    };
    let infra_text = if !infra.is_empty() {
        format!(
            "The proposal relies on {} FDS-assumed infrastructure item{}: {}.",
            infra.len(),
            if infra.len() == 1 { "" } else { "s" },
            infra
                .iter()
                .map(|i| format!("{} ({})", i.g("name").s(), i.g("funding").s()))
                .collect::<Vec<_>>()
                .join("; ")
        )
    } else {
        "No FDS-assumed infrastructure has been listed yet.".to_string()
    };
    let body = format!(
        "{lead} {text}{page}{} {infra_text} [Author to complete.]",
        if red {
            " The FDS red-flags this area."
        } else {
            ""
        }
    );
    jobj! { "status" => status, "text" => text.as_str(), "occupation" => occ, "timing" => timing.as_str(), "start" => start, "red" => red, "infra" => V::Array(infra), "cite" => cite.map_or(V::Null, st), "heading" => heading, "paragraph" => body,
    "outOfSequence" => status == "out", "fundingPrompt" => if status == "out" || status == "removal" { "Explain how the capital and operating funding gap for the infrastructure this site needs early will be met (AT ITA s5.1)." } else { "" },
    "reference" => reference, "applies" => pe.is_none() || pe.is_some_and(|e| e.g("proc").t()), "source" => if f.t() && f.g("src").t() { f.g("src").clone() } else { V::Null } }
}

/// `fdsPair(P, opts)`: runs the scenario with and without the FDS-assumed infrastructure (the links of the register rows), not saved as scenarios, and keeps the comparison in the project: `{ at, period, removed, rows, gap }`.
pub fn fds_pair(p: &mut V, period: Option<&str>) -> Result<V, String> {
    let c = state(p);
    let sc = active_sc(p).clone();
    let per = period.map(String::from).unwrap_or_else(|| {
        let x = p.g("settings").g("period");
        if x.t() { x.s() } else { "AM".into() }
    });
    let mut ids: Vec<String> = Vec::new();
    let push = |id: String, ids: &mut Vec<String>| {
        if !ids.contains(&id) {
            ids.push(id)
        }
    };
    for i in c.g("fds").g("infra").items() {
        if let Some(row) = p
            .g("register")
            .items()
            .iter()
            .find(|r| r.g("id").strict_eq(i.g("reg")))
        {
            for l in row.g("links").items() {
                push(l.s(), &mut ids);
            }
        }
    }
    for i in c.g("fds").g("infra").items() {
        for l in i.g("links").items() {
            push(l.s(), &mut ids);
        }
    }
    if ids.is_empty() {
        return Err("List the FDS-assumed infrastructure and link it to register rows with roads (or give its road ids) first.".into());
    }
    let drop: std::collections::HashSet<String> = ids.iter().cloned().collect();
    let mut without = engine::scenario_copy_without_links(&sc, &drop);
    without.set("year", sc.g("year").clone());
    without.set(
        "name",
        st(format!("{} without FDS infrastructure", sc.g("name").s())),
    );
    let opts = jobj! { "period" => per.as_str() };
    let a = engine::assign(p, &sc, None, &opts);
    let b = engine::assign(p, &without, None, &opts);
    let (ka, kb) = (a.g("kpi"), b.g("kpi"));
    let mut rows: Vec<V> = Vec::new();
    for (k, l, d) in [
        ("trips", "Vehicle trips (veh/h)", 0),
        ("vht", "Vehicle hours (veh\u{b7}h)", 1),
        ("dly", "Delay (veh\u{b7}h)", 1),
        ("over", "Roads over capacity", 0),
        ("spd", "Average speed (km/h)", 1),
    ] {
        rows.push(jobj! { "key" => k, "label" => l, "with" => util::r1(ka.g(k).n()), "without" => util::r1(kb.g(k).n()), "d" => util::r1(kb.g(k).n() - ka.g(k).n()), "dp" => d });
    }
    let (wa, wb) = (ka.g("worst"), kb.g("worst"));
    rows.push(jobj! { "key" => "worst", "label" => "Worst junction delay (s)", "with" => if wa.t() { V::Number(util::r1(wa.g("delay").n())) } else { V::Null }, "without" => if wb.t() { V::Number(util::r1(wb.g("delay").n())) } else { V::Null },
        "d" => if wa.t() && wb.t() { V::Number(util::r1(wb.g("delay").n() - wa.g("delay").n())) } else { V::Null }, "dp" => 0 });
    let pair = jobj! { "at" => today(&V::Null), "period" => per.as_str(), "removed" => strs(ids), "rows" => V::Array(rows), "gap" => V::Array(vec![a.g("gap").clone(), b.g("gap").clone()]) };
    state_mut(p)
        .get_mut("fds")
        .expect("fds")
        .set("pair", pair.clone());
    Ok(pair)
}
