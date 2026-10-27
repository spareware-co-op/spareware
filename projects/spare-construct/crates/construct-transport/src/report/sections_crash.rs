//! Crash history (section 2.7), the sites it is worked out for, and the future receiving environment (2.8).

use super::calc::{collective_band, personal_band, personal_risk, pof, severity_index};
use super::consts::{PERIODS, attrib, cec, hrig};
use super::ctx::{Ctx, FigRequest, Row, Tbl, TableOpts, disc, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_env::{CrashLeg, CrashSite, control_label, external_legs, study_bounds};
use super::sections_front::int_f;
use super::shared::{
    assessed_sites, cnum, crash_years, ctext, cx, junction_groups, kit_junction_name, plural, speed_of,
};
use super::Rep;
use crate::edit::json::JsonValue;
use crate::graph::{build_graph, js_sort_by, zone_class};
use crate::jsnum::{JsNum, Maybe, math_max, number_to_js_string, value_truthy};
use crate::util::{self, esc, fmt};

/// Development accesses of a scenario: road node id to the names of the zones that connect to it, in order (`accessesOf`).
pub(crate) fn accesses_of(r: &Rep, sc: usize) -> Vec<(String, Vec<String>)> {
    let scn = r.scenario(sc);
    let dz: Vec<&crate::project::Zone> = scn
        .zones
        .values()
        .filter(|z| !z.kind.is("gateway") && zone_class(Some(z)) == "development")
        .collect();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    if dz.is_empty() {
        return out;
    }
    let g = build_graph(scn, r.st(), None);
    for z in dz {
        let conns: Vec<String> = match g.conns.get(&z.id) {
            Some(c) if !c.is_empty() || true => c.clone(),
            _ => Vec::new(),
        };
        let conns = if g.conns.get(&z.id).is_some() {
            conns
        } else if let Maybe::Set(JsNum::Other(serde_json::Value::Array(a))) = &z.connectors {
            a.iter().map(crate::jsnum::value_to_js_string).collect()
        } else {
            Vec::new()
        };
        for id in conns {
            if !scn.nodes.contains_key(&id) {
                continue;
            }
            let name = match z.name.as_str() {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => z.id.clone(),
            };
            match out.iter().position(|(k, _)| *k == id) {
                Some(p) => out[p].1.push(name),
                None => out.push((id, vec![name])),
            }
        }
    }
    out
}

/// `crashSites(P, c)`: crash history at the assessed, altered and new sites and the development accesses (once per build).
pub(crate) fn crash_sites(c: &mut Ctx) -> Vec<CrashSite> {
    if let Some(v) = &c.crash_cache {
        return v.clone();
    }
    let v = crash_sites0(c);
    c.crash_cache = Some(v.clone());
    v
}

fn crash_sites0(c: &Ctx) -> Vec<CrashSite> {
    let r = c.r;
    let Some(cy) = crash_years(r) else {
        return Vec::new();
    };
    let sc = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let scw = c.sel.with.unwrap_or(sc);
    let (sc_n, scw_n) = (r.scenario(sc), r.scenario(scw));
    let cr = js::items(js::get(r.data(), "crashes"));
    let mut ids: Vec<String> = assessed_sites(r, c.sel.with).ids;
    let add = |ids: &mut Vec<String>, id: &str| {
        if !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    };
    for n in scw_n.nodes.values() {
        if n.upgrade.truthy() {
            add(&mut ids, &n.id);
        }
    }
    for l in scw_n.links.values() {
        if l.status.is("proposed") {
            for e in [&l.a, &l.b] {
                if sc_n.nodes.contains_key(e) {
                    add(&mut ids, e);
                }
            }
        }
    }
    // and every development access: the road nodes the development zones connect to (crash-history search areas)
    let acc = accesses_of(r, scw);
    for (id, _) in &acc {
        if scw_n.nodes.contains_key(id) || sc_n.nodes.contains_key(id) {
            add(&mut ids, id);
        }
    }
    let lr: Vec<(String, super::host::LinkRiskInfo)> = r.host.link_risk(sc).unwrap_or_default();
    let lr_of = |id: &str| lr.iter().find(|(k, _)| k == id).map(|(_, v)| v);
    let mut out: Vec<CrashSite> = Vec::new();
    let groups = junction_groups(r, sc, scw, &ids);
    for g in groups.iter().take(30) {
        let id = &g.id;
        let home = if scw_n.nodes.contains_key(id) { scw_n } else { sc_n };
        let Some(n) = home.nodes.get(id) else {
            continue;
        };
        let members: Vec<String> = g.nodes.iter().filter(|x| home.nodes.contains_key(*x)).cloned().collect();
        let pts: Vec<[f64; 2]> = members.iter().map(|x| home.nodes[x].ll()).collect();
        let (mut f, mut s, mut m, mut nn, mut injury, mut p, mut cc, mut mc, mut dsi) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0f64);
        let mut list: Vec<(usize, f64)> = Vec::new();
        for (ci, k) in cr.iter().enumerate() {
            let yr = js::num(js::idx(Some(k), 3));
            if !(yr >= cy.from && yr <= cy.to) {
                continue;
            }
            let at = [js::num(js::idx(Some(k), 0)), js::num(js::idx(Some(k), 1))];
            let d = pts.iter().map(|q| util::dist(q, &at)).fold(f64::INFINITY, f64::min);
            if d > hrig::RADIUS {
                continue;
            }
            let sev = js::text(js::idx(Some(k), 2));
            match sev.as_str() {
                "F" => f += 1.0,
                "S" => s += 1.0,
                "M" => m += 1.0,
                "N" => nn += 1.0,
                _ => {}
            }
            list.push((ci, d));
            if !sev.is_empty() && "FSM".contains(sev.as_str()) {
                injury += 1.0;
                let flags = js::text_or_empty(js::idx(Some(k), 5));
                dsi += severity_index(&flags, n.control.as_str().unwrap_or(""), false);
                for fl in ['p', 'c', 'm'] {
                    if flags.contains(fl) {
                        match fl {
                            'p' => p += 1.0,
                            'c' => cc += 1.0,
                            _ => mc += 1.0,
                        }
                    }
                }
            }
        }
        let lg = external_legs(home, &members);
        let links: Vec<&crate::project::Link> = lg.iter().flat_map(|x| x.links.iter().copied()).collect();
        let mut legs: Vec<CrashLeg> = lg
            .iter()
            .map(|x| {
                let v: Vec<Option<f64>> = x
                    .links
                    .iter()
                    .map(|l| lr_of(&l.id).and_then(|r| r.aadt).filter(|a| *a > 0.0))
                    .collect();
                let sv: Vec<f64> = v.iter().flatten().copied().collect();
                CrashLeg {
                    id: x.links[0].id.clone(),
                    name: x.name.clone(),
                    aadt: if sv.is_empty() { None } else { Some(sv.iter().fold(0.0, |a, b| a + b)) },
                    src: x
                        .links
                        .iter()
                        .filter_map(|l| lr_of(&l.id).and_then(|r| r.aadt_src.clone()))
                        .find(|s| !s.is_empty()),
                }
            })
            .collect();
        js_sort_by(&mut legs, |a, b| b.aadt.unwrap_or(0.0) - a.aadt.unwrap_or(0.0));
        let q: Vec<f64> = legs.iter().map(|l| l.aadt.unwrap_or(0.0)).collect();
        let pf = if legs.len() >= 3 && q[0] > 0.0 {
            Some(pof(&[q[0], q[1], q[2], if legs.len() >= 4 { q[3] } else { 0.0 }]))
        } else {
            None
        };
        let pr = pf.and_then(|pf| personal_risk(dsi, pf));
        let pb = personal_band(pr, injury);
        let speed = links
            .iter()
            .map(|l| {
                let v = speed_of(l).v.unwrap_or(0.0);
                if v.is_nan() { 0.0 } else { v }
            })
            .fold(0.0, math_max);
        let mut zs: Vec<String> = Vec::new();
        for x in &g.ids {
            if let Some((_, names)) = acc.iter().find(|(k, _)| k == x) {
                for nme in names {
                    if !zs.contains(nme) {
                        zs.push(nme.clone());
                    }
                }
            }
        }
        let base_name = if !g.name.is_empty() { g.name.clone() } else { kit_junction_name(home, id) };
        out.push(CrashSite {
            id: id.clone(),
            ids: g.ids.clone(),
            nodes: members,
            name: format!(
                "{}{}",
                base_name,
                if zs.is_empty() { String::new() } else { format!(" (access to {})", zs.join(", ")) }
            ),
            access: zs,
            control: n.control.as_str().map(str::to_string),
            speed,
            f,
            s,
            m,
            n: nn,
            injury,
            p,
            c: cc,
            mc,
            dsi_eq: dsi,
            list,
            pof: pf,
            pr,
            pband: pb,
            collective: collective_band(dsi, f + s),
            estimated: legs.iter().any(|l| l.src.as_deref().is_some_and(|s| s.contains("model"))),
            speed_flag: if speed != 0.0 && speed != cec::SPEED_REF {
                if speed > 70.0 {
                    "urban indices used above 70 km/h; check CEC rural indices".to_string()
                } else {
                    cec::SPEED_NOTE.to_string()
                }
            } else {
                String::new()
            },
            legs,
        });
    }
    // two sites with one name (a road with unnamed side roads) are told apart by their position
    let names: Vec<String> = out.iter().map(|s| s.name.clone()).collect();
    for s in out.iter_mut() {
        if names.iter().filter(|n| **n == s.name).count() > 1
            && let Some(nd) = scw_n.nodes.get(&s.id).or_else(|| sc_n.nodes.get(&s.id))
        {
            s.name.push_str(&format!(" ({}, {})", rfmt::num(nd.lat.num(), 4), rfmt::num(nd.lng.num(), 4)));
        }
    }
    js_sort_by(&mut out, |a, b| {
        let d = b.dsi_eq - a.dsi_eq;
        if d != 0.0 && !d.is_nan() { d } else { b.injury - a.injury }
    });
    out
}

/// `accessLong(P, c)`: ten-year crash history at the development accesses, or a note saying how to load it.
fn access_long(c: &mut Ctx) -> String {
    let r = c.r;
    let acc: Vec<CrashSite> = crash_sites(c).into_iter().filter(|s| !s.access.is_empty()).collect();
    if acc.is_empty() {
        return String::new();
    }
    let long = js::get(r.data(), "crashesLong").filter(|v| v.is_array());
    let l = js::get(r.loaded(), "casLong").filter(|v| v.truthy());
    let (Some(long), Some(l)) = (long, l) else {
        return note("For an access on a state highway, NZTA asks for ten years of crash history. Load it in the Traffic data tab (Crashes \u{203a} Ten years) to add it here; the five-year tables are not changed.", false);
    };
    let sc = c.sel.with.or(c.sel.base).unwrap_or(0);
    let sc0 = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let rows: Vec<Row> = acc
        .iter()
        .map(|s| {
            let home = if r.scenario(sc).nodes.contains_key(&s.id) { r.scenario(sc) } else { r.scenario(sc0) };
            let pts: Vec<[f64; 2]> = s.nodes.iter().filter_map(|x| home.nodes.get(x)).map(|n| n.ll()).collect();
            let mut t = [0.0f64; 4];
            for k in js::items(Some(long)) {
                if pts.is_empty() {
                    break;
                }
                let at = [js::num(js::idx(Some(k), 0)), js::num(js::idx(Some(k), 1))];
                let d = pts.iter().map(|q| util::dist(q, &at)).fold(f64::INFINITY, f64::min);
                if d <= hrig::RADIUS {
                    match js::text(js::idx(Some(k), 2)).as_str() {
                        "F" => t[0] += 1.0,
                        "S" => t[1] += 1.0,
                        "M" => t[2] += 1.0,
                        "N" => t[3] += 1.0,
                        _ => {}
                    }
                }
            }
            Row::new(vec![
                esc(&s.name),
                int_f(t[0]),
                int_f(t[1]),
                int_f(t[2]),
                int_f(t[3]),
                int_f(t[0] + t[1] + t[2]),
            ])
        })
        .collect();
    let label = js::get(Some(l), "label").filter(|v| v.truthy());
    c.table(
        &format!(
            "Table CR4 \u{b7} Ten-year crash history at development accesses (50 m radius{})",
            label.map_or(String::new(), |x| format!(", {}", esc(&x.js_string())))
        ),
        Tbl::new(vec![h("Access"), hn("F"), hn("S"), hn("M"), hn("N"), hn("Injury")], rows),
        &TableOpts::src_note(
            &format!("{}; ten-year download of {}", esc(attrib::CAS), esc(&super::ctx::day_of(js::get(Some(l), "at")))),
            "For state highway access applications (NZTA). The latest months may be incomplete (D8).",
        ),
    )
}

fn count_text(n: f64) -> String {
    number_to_js_string(n)
}

/// `report-crashes`.
pub(crate) fn crashes(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(cy) = crash_years(r) else {
        return format!(
            "{}{}",
            note("No crash data is loaded. Load NZTA CAS crashes in the Traffic data tab: AT ITA Guidelines s4.3.2 expect five full years.", true),
            disc("D8")
        );
    };
    let yr_row = |lab: String, t: &super::shared::Tally, cls: &str| {
        Row::cls(
            vec![
                lab,
                count_text(t.f),
                count_text(t.s),
                count_text(t.m),
                count_text(t.nn),
                count_text(t.n),
                if cy.has.deaths { count_text(t.deaths) } else { fmt::DASH.to_string() },
                if cy.has.si { count_text(t.si) } else { fmt::DASH.to_string() },
                count_text(t.f + t.s),
            ],
            cls,
        )
    };
    let mut cr1: Vec<Row> = cy
        .years
        .iter()
        .map(|(y, t)| {
            yr_row(
                format!(
                    "{}{}",
                    number_to_js_string(*y),
                    if cy.lag_year == Some(*y) { " (may be incomplete)" } else { "" }
                ),
                t,
                "",
            )
        })
        .collect();
    cr1.push(yr_row(
        format!("<b>{}\u{2013}{} total</b>", number_to_js_string(cy.from), number_to_js_string(cy.to)),
        &cy.total,
        "tot",
    ));
    cr1.push(yr_row(format!("{} to date (YTD, incomplete)", number_to_js_string(cy.ytd)), &cy.y, "ytd"));
    let sites = crash_sites(c);
    let cr2: Vec<Row> = sites
        .iter()
        .map(|s| {
            Row::new(vec![
                esc(&s.name),
                esc(&s.control.as_deref().map_or(String::new(), control_label)),
                if s.speed != 0.0 { number_to_js_string(s.speed) } else { fmt::DASH.to_string() },
                count_text(s.f),
                count_text(s.s),
                count_text(s.m),
                count_text(s.n),
                count_text(s.injury),
                count_text(s.p),
                count_text(s.c),
                count_text(s.mc),
                format!(
                    "{}{}",
                    rfmt::num(s.dsi_eq, 2),
                    if s.speed_flag.is_empty() {
                        String::new()
                    } else {
                        format!(" <span class=\"muted\" title=\"{}\">*</span>", esc(&s.speed_flag))
                    }
                ),
                esc(s.collective),
                format!(
                    "{}{}",
                    esc(&s
                        .legs
                        .iter()
                        .map(|l| match l.aadt {
                            Some(a) if a != 0.0 => rfmt::adt(a),
                            _ => fmt::DASH.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" / ")),
                    if s.estimated { " <span class=\"muted\">(estimated)</span>" } else { "" }
                ),
                match s.pof {
                    Some(p) if p != 0.0 => rfmt::num(p, 0),
                    _ => fmt::DASH.to_string(),
                },
                s.pr.map_or(fmt::DASH.to_string(), |p| rfmt::num(p, 1)),
                esc(&s.pband.label),
                if s.pband.reliable { "Yes".to_string() } else { "No (fewer than 4 injury crashes)".to_string() },
            ])
        })
        .collect();
    let on_roads = r.host.crashes_on_links(c.sel.existing.or(c.sel.base).unwrap_or(0));
    let t1 = c.table(
        &format!(
            "Table CR1 \u{b7} Crashes by year and severity, {}\u{2013}{} plus YTD",
            number_to_js_string(cy.from),
            number_to_js_string(cy.to)
        ),
        Tbl::new(
            vec![h("Year"), hn("Fatal"), hn("Serious"), hn("Minor"), hn("Non-injury"), hn("Total"), hn("Deaths"), hn("Serious injuries"), hn("DSI crashes")],
            cr1,
        ),
        &TableOpts::src_note(
            &esc(attrib::CAS),
            &format!(
                "Crash counts by the most severe injury. {} Every crash in the downloaded CAS area is counted{}.",
                if cy.has.deaths && cy.has.si {
                    "Deaths and serious injuries are the people killed and seriously injured (CAS fatalCount and seriousInjuryCount)."
                } else {
                    "Deaths and serious-injury counts are not in the CAS fields Spare Construct keeps, so they show \u{2013}."
                },
                match on_roads {
                    Some(n) => format!("; {} of all years are within 25 m of a modelled road", plural(n, "crash", "crashes")),
                    None => String::new(),
                }
            ),
        ),
    );
    let peak_to_daily = js::path(Some(r.raw), &["settings", "peakToDaily"]);
    let t2 = c.table(
        "Table CR2 \u{b7} Crash history at assessed, altered and new sites and development accesses (50 m radius, five full years)",
        Tbl::new(
            vec![
                h("Site"),
                h("Control"),
                hn("Speed km/h"),
                hn("F"),
                hn("S"),
                hn("M"),
                hn("N"),
                hn("Injury"),
                hn("Ped"),
                hn("Cyclist"),
                hn("Motorcyclist"),
                hn("DSI equiv (5 yr)"),
                h("Collective band"),
                h("Leg AADTs vpd"),
                hn("PoF"),
                hn("Personal risk DSI/100M veh"),
                h("Personal band"),
                h("Reliable"),
            ],
            cr2,
        )
        .empty("No assessed sites. Run the model for the assessed scenario."),
        &TableOpts::src_note(
            &format!("{}; {} ({}); {} ({})", esc(attrib::CAS), esc(cec::NAME), esc(cec::EDITION), esc(hrig::NAME), esc(hrig::EDITION)),
            &format!(
                "DSI equivalents = injury crashes \u{d7} CEC Table 10-4 urban severity indices (vehicle by control, pedestrian, cyclist and motorcyclist; movement codes are not public). * speed is not 50 km/h: {}. Collective band (HRIG Table 4-1): High \u{2265} 1.6 or \u{2265} 3 fatal and serious crashes. Personal risk = DSI\u{b7}10\u{2078} / (PoF\u{b7}5\u{b7}365\u{b7}1.7); High and Medium-high need 4 or more injury crashes, otherwise indicative. Leg AADTs from counts, else modelled peak \u{d7} {} (estimated). Standards verified: no.",
                esc(cec::SPEED_NOTE),
                esc(&js::text(peak_to_daily))
            ),
        ),
    );
    let t3 = c.table(
        "Table CR3 \u{b7} Crash conditions (five full years)",
        Tbl::new(
            vec![hn("Dark (all crashes)"), hn("Wet weather (all crashes)"), hn("Pedestrian injury crashes"), hn("Cyclist injury crashes"), hn("Motorcyclist injury crashes")],
            vec![Row::new(vec![
                if cy.has.light { count_text(cy.total.dark) } else { fmt::DASH.to_string() },
                if cy.has.weather { count_text(cy.total.wet) } else { fmt::DASH.to_string() },
                count_text(cy.total.p),
                count_text(cy.total.c),
                count_text(cy.total.mc),
            ])],
        ),
        &TableOpts::note(if cy.has.light && cy.has.weather {
            "Dark: CAS light is dark. Wet weather: CAS weatherA is rain, snow, hail or sleet (Spare Construct reading of \"wet\")."
        } else {
            "Light and weather are not in the CAS fields Spare Construct keeps yet, so Dark and Wet show \u{2013}."
        }),
    );
    let al = access_long(c);
    let scid = r.scenario(c.sel.existing.or(c.sel.base).unwrap_or(0)).id.clone();
    let mut f = FigRequest {
        id: "crashes".to_string(),
        title: "Reported crashes by severity, with the 50 m search areas of the assessed sites".to_string(),
        mode: "class".to_string(),
        scenario_id: scid,
        base_scenario_id: None,
        period: None,
        bounds: study_bounds(r, &c.sel),
        show: vec![("crashes".to_string(), true), ("counts".to_string(), false), ("report-sites".to_string(), true)],
        source: Some(attrib::CAS.to_string()),
    };
    f.source = Some(attrib::CAS.to_string());
    format!(
        "{}\n      {}\n      {}\n      {}\n      {}\n      {}\n      {}{}",
        para(&format!(
            "Crash history from NZTA CAS for {}. The tables use the five full years {}\u{2013}{}; the year to date is shown apart and is incomplete.",
            esc(&cy.label),
            number_to_js_string(cy.from),
            number_to_js_string(cy.to)
        )),
        t1,
        t2,
        t3,
        al,
        c.figure(f),
        c.author("crash-narrative", "AT ITA 4.3.3: explain why the system failed at the main crash sites, whether less severe crashes could become severe, and whether the proposal makes the pattern worse."),
        disc("D8")
    )
}

/// `report-receiving`.
pub(crate) fn receiving(c: &mut Ctx) -> String {
    let r = c.r;
    let rows: Vec<Row> = crate::edit::register::table(r.project)
        .iter()
        .map(|x| {
            let g = |k: &str| x.get(k);
            let s = |k: &str| match g(k) {
                Some(serde_json::Value::String(t)) => t.clone(),
                Some(serde_json::Value::Null) | None => String::new(),
                Some(v) => crate::jsnum::value_to_js_string(v),
            };
            let or_dash = |k: &str| {
                let t = s(k);
                if t.is_empty() { fmt::DASH.to_string() } else { t }
            };
            Row::new(vec![
                esc(&s("name")),
                esc(&s("type")),
                esc(&or_dash("status")),
                esc(&or_dash("funding")),
                esc(&s("source")),
                esc(&s("asAt")),
                if g("included").and_then(|v| v.as_bool()) == Some(true) { "Yes".to_string() } else { "No".to_string() },
                format!(
                    "{}{}",
                    esc(&s("reason")),
                    if g("verify").and_then(|v| v.as_bool()) == Some(true) { " <span class=\"warn\">(verify)</span>" } else { "" }
                ),
            ])
        })
        .collect();
    let pb: Vec<usize> = (0..r.project.scenarios.len())
        .filter(|i| r.scenario(*i).role.is("permitted"))
        .collect();
    let pb_text = if pb.is_empty() {
        note("No permitted baseline scenario (RMA s104(2)) is set. If one is relied on, add a scenario with the role \"Permitted baseline\" so its effects are shown separately from the future receiving environment.", false)
    } else {
        para(&format!(
            "<b>Permitted baseline (RMA s104(2)):</b> {}. Under s104(2) the consent authority <i>may</i> disregard an adverse effect of the activity if a national environmental standard or the plan permits an activity with that effect; whether it does is a matter for the consent authority, not this model.",
            pb.iter()
                .map(|&i| format!(
                    "{} ({})",
                    esc(r.sc(i).name()),
                    r.year_of(i).map_or(fmt::DASH.to_string(), number_to_js_string)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    };
    format!(
        "{}\n      {}{}",
        c.table(
            "Committed and consented development and funded projects",
            Tbl::new(vec![h("Item"), h("Type"), h("Status"), h("Funding"), h("Source"), h("As at"), h("Included in S1"), h("Reason")], rows)
                .empty("The register is empty. Add committed developments and network projects (AT ITA s3.6.1)."),
            &TableOpts::default()
        ),
        pb_text,
        c.author("receiving", "Describe the future receiving environment: committed and consented developments, funded projects, and any permitted baseline (RMA s104(2)).")
    )
}

fn _unused(_: &JsonValue) {
    let _ = (PERIODS, cx::ID, cnum, ctext, value_truthy);
}
