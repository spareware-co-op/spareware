//! Precinct staging tracker: I450 Drury Centre and I451 Drury East (compliance-extra.js section 6).
//!
//! Each row of a precinct's staging table caps development until the upgrades listed for that row and every earlier row are operational
//! (I450.6.2(1)). An upgrade under construction or delivered by the applicant counts only with the s108AA(1)(a) Augier agreement.

use super::ctx::{self, esc, num, stamped};
use crate::compliance::data::x as xdata;
use crate::compliance::engine;
use crate::compliance::html::{ReportCtx, kit_table, rt, th, tr};
use crate::compliance::jv::{Jx, NULL, V, csv, jobj, merge};
use crate::jsnum::{math_round, number_to_js_string, truthy_number};
use crate::util::fmt;

/// `${o[k]}` of a loose object in a template string: "undefined" when the field is missing.
fn tstr(o: &V, k: &str) -> String {
    if o.has(k) {
        o.g(k).s()
    } else {
        "undefined".to_string()
    }
}

/// `+x || 0`: a number, NaN and zero read as 0.
fn n0(v: &V) -> f64 {
    let x = v.n();
    if truthy_number(x) { x } else { 0.0 }
}

/// `r1(v)` of the hand-over packs as text: one decimal, empty for null and values that are not finite.
fn r1t(v: &V) -> String {
    if v.nullish() || !v.n().is_finite() {
        String::new()
    } else {
        number_to_js_string(math_round(v.n() * 10.0) / 10.0)
    }
}

/// `r3(v)` of the hand-over packs as text: three decimals, empty for null and values that are not finite.
fn r3t(v: &V) -> String {
    if v.nullish() || !v.n().is_finite() {
        String::new()
    } else {
        number_to_js_string(math_round(v.n() * 1000.0) / 1000.0)
    }
}

/// `precinctList(P)`: the precincts the tracker knows: I450 and I451 as data, then the author's own (`precinct.custom`), keyed by id.
pub fn precinct_list(p: &V) -> V {
    let mut out = xdata("PRECINCTS").clone();
    let cx = ctx::data(p);
    for q in cx.g("precinct").g("custom").items() {
        if q.t() && q.g("id").t() && q.g("rows").is_arr() {
            let defaults = jobj! { "status" => jobj! { "A5" => "Non-complying", "A6" => "Discretionary", "A9" => "Amendment" }, "verified" => false, "edition" => "entered by the user" };
            let e = merge(&merge(&defaults, q), &jobj! { "custom" => true });
            out.set(q.g("id").s(), e);
        }
    }
    out
}

/// `precinctOf(P)`: the project's precinct: the author's choice, or the one the Compliance tab found for the site; None when there is none.
pub fn precinct_of(p: &V) -> Option<V> {
    let cx = ctx::data(p);
    let list = precinct_list(p);
    let id = cx.g("precinct").g("id");
    if id.t() && list.g(&id.s()).t() {
        return Some(list.g(&id.s()).clone());
    }
    let site = crate::compliance::site_info(p);
    let named = site.g("precinct");
    // `pr` is the precinct text, or `{}` when there is none: it has no name, so only a precinct whose own name is empty can match by name
    let id_text = if named.t() { named.s() } else { String::new() };
    let k = list.keys().into_iter().find(|x| {
        let entry = list.g(x);
        let name_text = if entry.has("name") {
            entry.g("name").s()
        } else {
            "undefined".to_string()
        };
        id_text.contains(x) || name_text.is_empty()
    })?;
    Some(list.g(k).clone())
}

/// `deemedMet(st, augier)`: an upgrade counts as in place when it is operational, or under construction or delivered by the applicant with the Augier agreement.
pub fn deemed_met(st: &V, augier: &V) -> bool {
    st.eq_s("operational") || ((st.eq_s("construction") || st.eq_s("applicant")) && augier.t())
}

/// `precinctNode(P, sc, pr)`: the junction of the I450.9(4) inter-peak test in a scenario: the author's pick, or the junction whose roads carry
/// both names (the one with most roads when several do); null when none is found.
pub fn precinct_node(p: &V, sc: &V, pr: &V) -> V {
    let cx = ctx::data(p);
    let pick = cx.g("precinct").g("ipNode");
    if pick.t() && sc.g("nodes").g(&pick.s()).t() {
        return pick.clone();
    }
    if !pr.t() || !pr.g("node").t() {
        return V::Null;
    }
    let mut best: Option<(&str, usize)> = None;
    for id in sc.g("nodes").keys() {
        let links = crate::compliance::net::road_links(sc, id);
        let names = links
            .iter()
            .map(|l| {
                if l.g("name").t() {
                    l.g("name").s()
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join(" | ");
        if pr
            .g("node")
            .items()
            .iter()
            .all(|n| names.contains(n.s().as_str()))
        {
            // a stable sort by road count, most first: the first of the largest wins
            if best.is_none_or(|(_, c)| links.len() > c) {
                best = Some((id, links.len()));
            }
        }
    }
    best.map_or(V::Null, |(id, _)| V::String(id.to_string()))
}

/// `mixTrips(mix, sc)`: the trips (leaving plus arriving) of a mix of dwellings and floor area for the AM and PM peaks, from the engine's trip
/// rates on a made-up zone with the project's settings: `[{ p, trips }]`.
pub fn mix_trips(p: &V, mix: &V, sc: &V) -> V {
    let z = jobj! { "id" => "_mix", "kind" => "zone", "lu" => jobj! { "residential" => n0(mix.g("dw")), "retail" => n0(mix.g("retail")), "office" => n0(mix.g("commercial")), "medical" => n0(mix.g("community")) } };
    V::Array(
        ["AM", "PM"]
            .iter()
            .map(|per| {
                let (o, d) = engine::trip_ends(p, &z, per, sc);
                jobj! { "p" => *per, "trips" => o + d }
            })
            .collect(),
    )
}

/// `precinctCheck(P, sc)`: the staging check of the project's precinct: `{ precinct, rows (with upgrade states), cumulative, proposal, total, row
/// (applicable), rowIndex, breach, why, status, statusText, headroom, ip, trips, banner, pc120Note, warnings, register, monitorText }`, or null when no
/// precinct applies.
pub fn precinct_check(p: &V, sc: Option<&V>) -> V {
    let Some(pr) = precinct_of(p) else {
        return V::Null;
    };
    let sc = sc.unwrap_or_else(|| ctx::active_sc(p));
    let cx = ctx::data(p);
    let pc = cx.g("precinct");
    let ups = pc.g("upgrades");
    let reg: Vec<V> = pc
        .g("register")
        .items()
        .iter()
        .filter(|r| r.t() && !r.g("excluded").strict_eq(&V::Bool(true)))
        .cloned()
        .collect();
    let measures = xdata("MEASURES").items();
    let (mut cumulative, mut proposal, mut total) = (V::object(), V::object(), V::object());
    let (mut cum_n, mut prop_n, mut tot_n) = (Vec::new(), Vec::new(), Vec::new());
    for m in measures {
        let (k, f) = (m.ix(0).s(), m.ix(1).s());
        let c = reg.iter().fold(0.0, |s, r| s + n0(r.g(&f)));
        cumulative.set(k, V::Number(c));
        cum_n.push(c);
    }
    for m in measures {
        let (k, f) = (m.ix(0).s(), m.ix(1).s());
        let x = n0(pc.g("proposal").g(&f));
        proposal.set(k, V::Number(x));
        prop_n.push(x);
    }
    for (i, m) in measures.iter().enumerate() {
        let t = cum_n[i] + prop_n[i];
        total.set(m.ix(0).s(), V::Number(t));
        tot_n.push(t);
    }
    let statuses = xdata("UPGRADE_STATUS");
    let mut rows: Vec<V> = pr
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            let up: Vec<V> = r
                .g("up")
                .items()
                .iter()
                .map(|u| {
                    let given = ups.g(&u.g("id").s());
                    let st = if given.t() { given.clone() } else { V::String("not-started".into()) };
                    let label = if statuses.g(&st.s()).t() { statuses.g(&st.s()).clone() } else { st.clone() };
                    let met = deemed_met(&st, pc.g("augier"));
                    let needs = (st.eq_s("construction") || st.eq_s("applicant")) && !pc.g("augier").t();
                    merge(u, &jobj! { "state" => st, "stateLabel" => label, "met" => met, "needsAugier" => needs })
                })
                .collect();
            merge(r, &jobj! { "up" => up })
        })
        .collect();
    for r in rows.iter_mut() {
        let m = r.g("up").items().iter().all(|u| u.g("met").t());
        r.set("met", V::Bool(m));
    }
    let idx = rows.iter().position(|r| !r.g("met").t());
    let beyond = idx.is_none();
    let row_index = idx.unwrap_or(rows.len() - 1);
    let row = rows[row_index].clone();
    let mut why: Vec<String> = Vec::new();
    for (i, m) in measures.iter().enumerate() {
        let cap = row.g(&m.ix(0).s());
        if !cap.nullish() && tot_n[i] > cap.n() {
            why.push(format!(
                "{}: {} against the row ({}) cap of {}",
                m.ix(2).s(),
                fmt::int(tot_n[i]),
                tstr(&row, "row"),
                fmt::int(cap.n())
            ));
        }
    }
    let breach = !why.is_empty();
    let unmet_a5 = row
        .g("up")
        .items()
        .iter()
        .filter(|u| !u.g("met").t())
        .any(|u| u.g("status").eq_s("A5"));
    let status: Option<&str> = if breach {
        Some(if pc.g("amendment").t() {
            "A9"
        } else if beyond {
            "A6"
        } else if unmet_a5 {
            "A5"
        } else {
            "A6"
        })
    } else {
        None
    };
    let mut headroom = V::object();
    for (i, m) in measures.iter().enumerate() {
        let cap = row.g(&m.ix(0).s());
        headroom.set(
            m.ix(0).s(),
            if cap.nullish() {
                V::Null
            } else {
                V::Number(cap.n() - tot_n[i])
            },
        );
    }
    // inter-peak LoS at the named junction (I450.9(4)): an IP result if there is one, otherwise the LoS the author enters
    let node_id = precinct_node(p, sc, &pr);
    let nr: &V = match ctx::res_of(sc, "IP") {
        Some(r) if node_id.t() && r.g("nodes").t() => r.g("nodes").g(&node_id.s()),
        _ => &NULL,
    };
    let los: V = if nr.t() {
        if nr.g("intersectionLos").t() {
            nr.g("intersectionLos").clone()
        } else {
            nr.g("los").clone()
        }
    } else {
        let t = pc.g("ipLos");
        V::String(if t.t() {
            t.s().to_uppercase()
        } else {
            String::new()
        })
    };
    let test_los = xdata("STD.precinctTest.los");
    let mut ip = jobj! { "node" => if node_id.t() { node_id.clone() } else { V::Null } };
    if node_id.t() {
        ip.set("name", V::String(ctx::node_name(sc, &node_id.s())));
    } else if pr.has("nodeLabel") {
        ip.set("name", pr.g("nodeLabel").clone());
    }
    ip.set("los", if los.t() { los.clone() } else { V::Null });
    ip.set(
        "source",
        V::String(if nr.t() {
            format!("modelled inter-peak ({})", xdata("STD.labels.junction").s())
        } else if los.t() {
            "entered by the author (inter-peak, e.g. SIDRA)".to_string()
        } else {
            "not available".to_string()
        }),
    );
    ip.set(
        "pass",
        if los.t() {
            V::Bool(
                !los.as_str()
                    .is_some_and(|l| ctx::los_worse(l, &test_los.s())),
            )
        } else {
            V::Null
        },
    );
    if pr.has("ita") {
        ip.set("rule", pr.g("ita").clone());
    }
    ip.set(
        "test",
        V::String(format!(
            "inter-peak overall LoS generally no worse than {}",
            test_los.s()
        )),
    );
    // trip equivalence: cumulative + proposed mix against the table mix at the same row
    let or0 = |k: &str| {
        if row.g(k).t() {
            row.g(k).clone()
        } else {
            V::Number(0.0)
        }
    };
    let mix_row = jobj! { "dw" => row.g("dw"), "retail" => or0("retail"), "commercial" => or0("commercial"), "community" => or0("community") };
    let (a, b) = (mix_trips(p, &total, sc), mix_trips(p, &mix_row, sc));
    let similar = xdata("STD.precinctTest.similar").n();
    let mut rr: Vec<V> = Vec::new();
    let mut all_similar = true;
    for (i, x) in a.items().iter().enumerate() {
        let (xt, bt) = (x.g("trips").n(), b.ix(i).g("trips").n());
        let ratio = if bt > 0.0 { Some(xt / bt) } else { None };
        if let Some(r) = ratio {
            if !(r <= similar) {
                all_similar = false;
            }
        }
        rr.push(jobj! { "period" => x.g("p"), "proposed" => xt, "table" => bt, "ratio" => ratio });
    }
    let trips = jobj! { "rows" => rr, "pass" => all_similar, "basis" => xdata("STD.precinctTest.similarBasis"),
    "mapping" => format!("{}: community GFA uses the medical-centre rate and other commercial the office rate", ctx::assumption()) };
    let mut warnings: Vec<V> = Vec::new();
    if rows
        .iter()
        .any(|r| r.g("up").items().iter().any(|u| u.g("needsAugier").t()))
    {
        warnings.push(V::String(format!(
            "An upgrade under construction or delivered by the applicant counts only with express agreement under s108AA(1)(a), on an Augier basis, to conditions preventing occupation or s224(c) certification until it is operational ({}). Tick the agreement to rely on it.",
            tstr(&pr, "deemed")
        )));
    }
    if beyond && row.has("dw") && tot_n[0] > row.g("dw").n() {
        warnings.push(V::String(format!(
            "The total exceeds the final row of {}: the table does not provide for it.",
            tstr(&pr, "table")
        )));
    }
    if pr.g("note").t() {
        warnings.push(pr.g("note").clone());
    }
    let mut out = jobj! { "precinct" => &pr, "rows" => rows.clone(), "cumulative" => cumulative, "proposal" => proposal, "total" => total, "row" => &row, "rowIndex" => row_index,
    "breach" => breach, "why" => why.iter().map(|w| V::String(w.clone())).collect::<Vec<V>>(), "status" => status };
    let status_text: Option<V> = match status {
        Some(s) => pr.g("status").get(s).cloned(),
        None => Some(V::String("Complies with the staging standard".into())),
    };
    if let Some(t) = status_text {
        out.set("statusText", t);
    }
    out.set("headroom", headroom);
    out.set("ip", ip);
    out.set("trips", trips);
    out.set(
        "banner",
        if pr.g("pc120").t() {
            xdata("STD.labels.pc120").clone()
        } else {
            V::String(String::new())
        },
    );
    out.set(
        "pc120Note",
        if pr.g("pc120").t() {
            xdata("STD.labels.pc120Withdrawal").clone()
        } else {
            V::String(String::new())
        },
    );
    out.set("warnings", V::Array(warnings));
    out.set("register", V::Array(reg));
    out.set(
        "monitorText",
        V::String(format!(
            "State the dwellings and GFA this application enables ({}).",
            tstr(&pr, "monitor")
        )),
    );
    out
}

/// `precinctCSV(P)`: the precinct staging table, register, proposal and tests as a CSV with the version stamp and disclaimers in footer rows.
pub fn precinct_csv(p: &V) -> String {
    let c = precinct_check(p, None);
    let s = |x: &str| V::String(x.to_string());
    if c.nullish() {
        return stamped(&csv(&[vec![s("No precinct selected")]]), p, None);
    }
    let blank_if_null = |v: &V| if v.nullish() { s("") } else { v.clone() };
    let pr = c.g("precinct");
    let mut rows: Vec<Vec<V>> = vec![
        vec![s(&format!(
            "{} {}: {}",
            tstr(pr, "id"),
            tstr(pr, "name"),
            tstr(pr, "table")
        ))],
        vec![c.g("banner").clone()],
        vec![c.g("pc120Note").clone()],
        vec![],
        [
            "Row",
            "Dwellings cap",
            "Retail GFA cap (m\u{b2})",
            "Other commercial GFA cap (m\u{b2})",
            "Community GFA cap (m\u{b2})",
            "Upgrade",
            "Status",
            "Deemed met",
            "Activity status on breach",
        ]
        .iter()
        .map(|h| s(h))
        .collect(),
    ];
    for r in c.g("rows").items() {
        for u in r.g("up").items() {
            rows.push(vec![
                r.g("row").clone(),
                blank_if_null(r.g("dw")),
                blank_if_null(r.g("retail")),
                blank_if_null(r.g("commercial")),
                blank_if_null(r.g("community")),
                u.g("name").clone(),
                u.g("stateLabel").clone(),
                s(if u.g("met").t() { "yes" } else { "no" }),
                u.g("status").clone(),
            ]);
        }
    }
    rows.push(vec![]);
    rows.push(
        [
            "Register: consent ref",
            "Dwellings",
            "Retail GFA (m\u{b2})",
            "Other commercial GFA (m\u{b2})",
            "Community GFA (m\u{b2})",
            "Date",
            "Enabled yield",
        ]
        .iter()
        .map(|h| s(h))
        .collect(),
    );
    for r in c.g("register").items() {
        rows.push(vec![
            r.g("ref").clone(),
            r.g("dwellings").clone(),
            r.g("retail").clone(),
            r.g("commercial").clone(),
            r.g("community").clone(),
            r.g("date").clone(),
            if r.g("enabled").t() {
                r.g("enabled").clone()
            } else {
                s("")
            },
        ]);
    }
    let (proposal, total) = (c.g("proposal"), c.g("total"));
    rows.push(vec![
        s("Proposal"),
        proposal.g("dw").clone(),
        proposal.g("retail").clone(),
        proposal.g("commercial").clone(),
        proposal.g("community").clone(),
    ]);
    rows.push(vec![
        s("Total"),
        total.g("dw").clone(),
        total.g("retail").clone(),
        total.g("commercial").clone(),
        total.g("community").clone(),
    ]);
    rows.push(vec![]);
    rows.push(vec![s("Applicable row"), c.g("row").g("row").clone()]);
    let why = c
        .g("why")
        .items()
        .iter()
        .map(|w| w.s())
        .collect::<Vec<_>>()
        .join("; ");
    rows.push(vec![
        s("Result"),
        s(&if c.g("breach").t() {
            format!("Breach: {why}")
        } else {
            "Within the caps".to_string()
        }),
    ]);
    rows.push(vec![s("Activity status"), c.g("statusText").clone()]);
    let ip = c.g("ip");
    let pass = if ip.g("pass").nullish() {
        ""
    } else if ip.g("pass").t() {
        "passes"
    } else {
        "fails"
    };
    rows.push(vec![
        s("Inter-peak LoS test"),
        s(&format!(
            "{}: {} ({}) {} {}",
            tstr(ip, "name"),
            if ip.g("los").t() {
                ip.g("los").s()
            } else {
                "not available".into()
            },
            tstr(ip, "source"),
            pass,
            tstr(ip, "rule")
        )),
    ]);
    for t in c.g("trips").g("rows").items() {
        rows.push(vec![
            s(&format!("Trip equivalence {}", tstr(t, "period"))),
            s(&format!(
                "{} veh/h proposed against {} veh/h table mix (ratio {})",
                r1t(t.g("proposed")),
                r1t(t.g("table")),
                r3t(t.g("ratio"))
            )),
        ]);
    }
    stamped(&csv(&rows), p, None)
}

/// The precinct staging table of the report (section "compliance-extra-precinct"); an empty string when no precinct applies.
fn precinct_report(p: &V, rctx: Option<&dyn ReportCtx>) -> String {
    let x = precinct_check(p, None);
    if x.nullish() {
        return String::new();
    }
    let head = [
        th("Row"),
        th("Dwellings cap"),
        th("Prerequisite upgrades"),
        th("Status"),
    ];
    let body: Vec<_> = x
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            tr([
                format!("({})", esc(r.g("row"))),
                num(r.g("dw"), 0),
                r.g("up")
                    .items()
                    .iter()
                    .map(|u| format!("{} ({})", esc(u.g("name")), esc(u.g("stateLabel"))))
                    .collect::<Vec<_>>()
                    .join("; "),
                if r.g("met").t() {
                    "Deemed met".to_string()
                } else {
                    "Not met".to_string()
                },
            ])
        })
        .collect();
    let (total, cum, prop) = (x.g("total"), x.g("cumulative"), x.g("proposal"));
    let why = x
        .g("why")
        .items()
        .iter()
        .map(|w| w.s())
        .collect::<Vec<_>>()
        .join("; ");
    let verdict = if x.g("breach").t() {
        ctx::esc_s(&format!(
            "Breach of row ({}): {}. {}.",
            tstr(x.g("row"), "row"),
            why,
            tstr(&x, "statusText")
        ))
    } else {
        "Within the applicable row.".to_string()
    };
    let ip = x.g("ip");
    let los = if ip.g("los").t() {
        ip.g("los").clone()
    } else {
        V::String("not available".into())
    };
    let note = format!(
        "Total {} dwellings ({} approved + {} proposed). {} Inter-peak LoS at {}: {} ({}).",
        fmt::int(total.g("dw").n()),
        fmt::int(cum.g("dw").n()),
        fmt::int(prop.g("dw").n()),
        verdict,
        esc(ip.g("name")),
        esc(&los),
        esc(ip.g("source"))
    );
    let title = format!(
        "{} staging against approved and proposed development",
        esc(x.g("precinct").g("table"))
    );
    format!(
        "<p class=\"note\"><b>{}</b> {}.</p>{}",
        esc(x.g("banner")),
        esc(x.g("pc120Note")),
        rt(rctx, &title, &kit_table(&head, &body, true, "", ""), &note)
    )
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-precinct"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, rctx: Option<&dyn ReportCtx>) -> Option<String> {
    (id == "compliance-extra-precinct").then(|| precinct_report(p, rctx))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-precinct"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-precinct").then(|| precinct_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-precinct").then(|| precinct_of(p).is_some())
}
