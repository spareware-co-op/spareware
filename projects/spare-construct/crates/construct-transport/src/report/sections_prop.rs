//! Chapter 3: the proposal: land-use schedule, access, parking, staging, construction traffic and trip generation.

use super::ctx::{Ctx, FigRequest, Row, Tbl, TableOpts, h, hn, note};
use super::js;
use super::rfmt;
use super::sections_env::study_bounds;
use super::sections_front::year_txt;
use super::build::lp_tables;
use crate::analysis::dev_distribution;
use crate::edit::json::JsonValue;
use crate::graph::zone_class;
use crate::jsnum::{JsNum, Maybe, number_to_js_string};
use crate::project::{Scenario, Zone, default_settings, land_use};
use crate::util::{clamp, esc, fmt};

/// `devZones(sc)`: the zones that are not gateways and not existing land use.
pub(crate) fn dev_zones(sc: &Scenario) -> Vec<&Zone> {
    sc.zones
        .values()
        .filter(|z| !z.kind.is("gateway") && zone_class(Some(z)) != "existing")
        .collect()
}

/// `lpParts(sc, parts)`: the land-use feature's trip tables for the parts asked for, cut at T2 for an older feature that prints them all.
fn lp_parts(c: &Ctx, sc: usize, parts: &[&str]) -> Option<String> {
    let scope = js::path(Some(c.r.raw), &["landuse", "scope"]);
    let mut hh = c.r.host.landuse_trip_tables(sc, scope, parts)?;
    if let Some(cut) = hh.find("<h4>T2 ")
        && cut > 0
    {
        if parts.len() == 1 && parts[0] == "T1" {
            hh.truncate(cut);
        } else if !parts.contains(&"T1") {
            hh = hh[cut..].to_string();
        }
    }
    Some(hh)
}

/// `report-landuse`.
pub(crate) fn landuse(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(sc) = c.sel.with else {
        return String::new();
    };
    if let Some(lp) = lp_parts(c, sc, &["T1"]) {
        return lp_tables(&lp, "Land-use schedule");
    }
    let scn = r.scenario(sc);
    let mut rows: Vec<Row> = Vec::new();
    for z in dev_zones(scn) {
        let Some(lu) = z.lu.get() else {
            continue;
        };
        for (k, q) in lu.iter() {
            let q = q.num();
            let Some(l) = land_use(k).filter(|_| q > 0.0) else {
                continue;
            };
            rows.push(Row::new(vec![
                esc(&z.name.as_str().filter(|n| !n.is_empty()).map_or(z.id.clone(), str::to_string)),
                esc(l.label),
                rfmt::num(q, 0),
                esc(l.unit),
                esc(zone_class(Some(z))),
                year_txt(&z.year.js_string()),
                esc(&{
                    let t = js::text_or_empty(r.rs.text_of(&format!("hours-{}", z.id)));
                    if t.is_empty() { fmt::DASH.to_string() } else { t }
                }),
            ]));
        }
    }
    c.table(
        "Table T1 \u{b7} Land-use schedule",
        Tbl::new(vec![h("Zone"), h("Activity"), hn("Quantity"), h("Unit"), h("Class"), h("Opening year"), h("Hours")], rows)
            .empty("No development zones in the assessed scenario."),
        &TableOpts {
            source: Some(format!("Project inputs ({})", esc(r.sc(sc).name()))),
            ..TableOpts::default()
        },
    )
}

/// `report-access`.
pub(crate) fn access(c: &mut Ctx) -> String {
    c.author("access", "Describe each vehicle, pedestrian and cycle access, its form and control, sight distances, and compliance with AUP E27.6.4.")
}

/// `report-accessibility`.
pub(crate) fn accessibility(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        super::ctx::para("Walking, cycling and public transport access is described in section 2.4, and levels of service in section 4."),
        c.author("parking", "Describe parking, loading and accessible parking against AUP E27.6.2 and E27.6.3 (no parking minimums apply under NPS-UD Policy 11).")
    )
}

/// `report-staging-plan`.
pub(crate) fn staging_plan(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        if c.r.rs.staging.is_some() {
            super::ctx::para("Build-out increments and the staging triggers are set out in section 5 (Table M2).")
        } else {
            note("The staging test has not been run (Report tab \u{203a} Run the staging test).", false)
        },
        c.author("staging", "Describe the proposed staging and its sequencing with infrastructure.")
    )
}

/// `report-construction`.
pub(crate) fn construction(c: &mut Ctx) -> String {
    c.author("construction", "Describe construction traffic volumes, routes, hours and the draft construction traffic management plan (AT ITA s3.5).")
}

/// `devFigures(P, c, sc)`: the development traffic figures of section 3.6.
fn dev_figures(c: &mut Ctx, sc: usize) -> String {
    let r = c.r;
    let scn = r.scenario(sc);
    let year = r.year_of(sc).map_or("null".to_string(), number_to_js_string);
    let bounds = study_bounds(r, &c.sel);
    let periods = c.periods.clone();
    let mut out = String::new();
    for p in periods {
        let f = FigRequest {
            id: format!("dev-{}-{}", scn.id, p),
            title: format!(
                "Development traffic only (assignment of the development's trips), {}, {} peak hour {} (veh/h, rounded to the nearest 10; line width shows the flow)",
                scn.name.as_str().unwrap_or(""),
                p,
                year
            ),
            mode: "dev".to_string(),
            scenario_id: scn.id.clone(),
            base_scenario_id: None,
            period: Some(p.clone()),
            bounds: bounds.clone(),
            show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
            source: None,
        };
        out.push_str(&c.figure(f));
        if r.host.has_mode("devshare") {
            let f = FigRequest {
                id: format!("devshare-{}-{}", scn.id, p),
                title: format!(
                    "Development traffic as a share of each road's modelled flow, {}, {} peak hour {} (%)",
                    scn.name.as_str().unwrap_or(""),
                    p,
                    year
                ),
                mode: "devshare".to_string(),
                scenario_id: scn.id.clone(),
                base_scenario_id: None,
                period: Some(p.clone()),
                bounds: bounds.clone(),
                show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
                source: None,
            };
            out.push_str(&c.figure(f));
        }
    }
    out
}

/// `a === b` for loose numbers (the same type and value).
fn strict_same(a: &Maybe<JsNum>, b: &Maybe<JsNum>) -> bool {
    match (a, b) {
        (Maybe::Set(JsNum::Number(x)), Maybe::Set(JsNum::Number(y))) => x == y,
        (Maybe::Set(JsNum::Text(x)), Maybe::Set(JsNum::Text(y))) => x == y,
        (Maybe::Set(JsNum::Bool(x)), Maybe::Set(JsNum::Bool(y))) => x == y,
        (Maybe::Null, Maybe::Null) | (Maybe::Missing, Maybe::Missing) => true,
        _ => false,
    }
}

/// `report-tripgen`.
pub(crate) fn tripgen(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(sc) = c.sel.with else {
        return String::new();
    };
    let scn = r.scenario(sc);
    let s = r.st();
    let d0 = default_settings();
    // with the land-use feature: its person trips, mode share and gross-to-net vehicle trips (T2-T4), its gateway distribution (T5)
    // and its without/with junction list, printed once here
    if let Some(lp) = lp_parts(c, sc, &["T2", "T3", "T4", "T4b", "check", "flags"]) {
        let t5 = match r.host.landuse_t5_html(sc) {
            Some(h5) => format!("<h4>T5 Development traffic by gateway</h4>{h5}"),
            None => String::new(),
        };
        let ww = match r.host.landuse_ww_html(sc, c.sel.without.unwrap_or(usize::MAX)) {
            Some(w) if c.sel.without.is_some() => {
                format!("<h4>Junctions without and with the development (largest changes and new accesses)</h4>{w}")
            }
            _ if r.host.landuse_ww_html(sc, 0).is_some() || c.sel.without.is_none() && r.host.landuse_ww_html(sc, 0).is_some() => note(
                &format!(
                    "No same-year scenario without the development for {} ({}): choose one in the Report tab to list junction changes.",
                    esc(r.sc(sc).name()),
                    r.year_of(sc).map_or("null".to_string(), number_to_js_string)
                ),
                true,
            ),
            _ => String::new(),
        };
        return format!("{}{}", lp_tables(&format!("{lp}{t5}{ww}"), "Trip generation"), dev_figures(c, sc));
    }
    let mut rows: Vec<Row> = Vec::new();
    for z in dev_zones(scn) {
        for p in ["AM", "PM"] {
            if let Some(lu) = z.lu.get() {
                for (k, q) in lu.iter() {
                    let qv = q.num();
                    let rate = s
                        .rates
                        .get()
                        .and_then(|m| m.get(k))
                        .and_then(|m| m.get())
                        .and_then(|m| m.get(p))
                        .and_then(|m| m.get());
                    let Some((rate, l)) = rate.zip(land_use(k)).filter(|_| qv > 0.0) else {
                        continue;
                    };
                    let gross = qv / l.per * rate.rate.num();
                    let red = clamp(z.reduction.or(0.0), 0.0, 100.0);
                    let stage = crate::demand::stage_of(Some(z), Some(scn));
                    let open = !(z.year.num() > r.year_of(sc).unwrap_or(f64::NAN));
                    let open = open || r.year_of(sc).is_none();
                    let net = if open { gross * (1.0 - red / 100.0) * stage } else { 0.0 };
                    let dflt = d0
                        .rates
                        .get()
                        .and_then(|m| m.get(k))
                        .and_then(|m| m.get())
                        .and_then(|m| m.get(p))
                        .and_then(|m| m.get())
                        .is_some_and(|dr| strict_same(&dr.rate, &rate.rate));
                    let inp = rate.in_pct.num();
                    rows.push(Row::new(vec![
                        esc(&z.name.as_str().filter(|n| !n.is_empty()).map_or(z.id.clone(), str::to_string)),
                        esc(l.label),
                        p.to_string(),
                        rfmt::num(rate.rate.num(), 2),
                        if dflt { "Spare Construct indicative default (replace with surveyed rates)".to_string() } else { "User-entered".to_string() },
                        rfmt::num(gross, 0),
                        if red != 0.0 { format!("\u{2212}{}", rfmt::pct(red)) } else { fmt::DASH.to_string() },
                        if stage < 1.0 { format!("{} built", rfmt::pct(stage * 100.0)) } else { fmt::DASH.to_string() },
                        rfmt::num(net, 0),
                        rfmt::pct(inp),
                        rfmt::pct(100.0 - inp),
                        rfmt::num(net * inp / 100.0, 0),
                        rfmt::num(net * (1.0 - inp / 100.0), 0),
                    ]));
                }
            }
            // z.extra mirrors the land-use feature's table while it is synced there (z.tg.synced 'extra'): not the author's own extra trips
            let tg_synced = z
                .tg
                .get()
                .is_some_and(|tg| tg.get("on").is_some_and(crate::jsnum::value_truthy) && tg.get("synced").and_then(|v| v.as_str()) == Some("extra"));
            if !tg_synced && let Some(ex) = z.extra_trips.get() {
                let xin = ex.num_at(&format!("{p}in")).num();
                let xout = ex.num_at(&format!("{p}out")).num();
                if ex.num_at(&format!("{p}in")).truthy() || ex.num_at(&format!("{p}out")).truthy() {
                    let a = crate::jsnum::or_default(xin, 0.0);
                    let b = crate::jsnum::or_default(xout, 0.0);
                    rows.push(Row::new(vec![
                        esc(z.name.as_str().unwrap_or("")),
                        "Extra trips (user)".to_string(),
                        p.to_string(),
                        fmt::DASH.to_string(),
                        "User-entered".to_string(),
                        fmt::DASH.to_string(),
                        fmt::DASH.to_string(),
                        fmt::DASH.to_string(),
                        rfmt::num(a + b, 0),
                        fmt::DASH.to_string(),
                        fmt::DASH.to_string(),
                        rfmt::num(a, 0),
                        rfmt::num(b, 0),
                    ]));
                }
            }
        }
    }
    let res_l: Vec<&crate::results::Res> = r
        .rep_periods(sc)
        .iter()
        .filter_map(|p| r.results[sc].get(p))
        .collect();
    let dd = if res_l.is_empty() { None } else { Some(dev_distribution(Some(scn), &res_l)) };
    let t5: Vec<Row> = match &dd {
        Some(dd) => dd
            .rows
            .iter()
            .map(|x| {
                let share = |k: &str| x.shares.get(k).copied().unwrap_or(f64::NAN);
                Row::new(vec![
                    esc(&x.name),
                    esc(if scn.zones.get(&x.id).is_some_and(|z| z.gw_src.get().is_some_and(crate::jsnum::value_truthy)) { "Counts" } else { "Estimated" }),
                    rfmt::pct(share("AMin")),
                    rfmt::pct(share("AMout")),
                    rfmt::pct(share("PMin")),
                    rfmt::pct(share("PMout")),
                    x.vph.iter().map(|(p, v)| format!("{} {}", p, rfmt::veh(*v))).collect::<Vec<_>>().join(", "),
                ])
            })
            .collect(),
        None => Vec::new(),
    };
    let t4 = c.table(
        "Table T4 \u{b7} Vehicle trip generation",
        Tbl::new(
            vec![
                h("Zone"),
                h("Activity"),
                h("Period"),
                hn("Rate veh trips/unit/h"),
                h("Rate source"),
                hn("Gross veh trips"),
                hn("\u{2212} Reduction"),
                hn("Build-out"),
                hn("Net"),
                hn("In %"),
                hn("Out %"),
                hn("In veh/h"),
                hn("Out veh/h"),
            ],
            rows,
        )
        .empty("No development zones."),
        &TableOpts::src_note(
            "Trip rates: project settings (Appendix A); land-use quantities, build-out and opening years: project zones (user)",
            "Reductions are the zone's trip reduction for public transport and active modes; internal and pass-by trips are not deducted separately. Person trips and mode share (Tables T2 and T3) are added by the land-use feature when person-trip rates are loaded.",
        ),
    );
    let t5_note = match &dd {
        Some(dd) => format!(
            "{}.",
            dd.periods
                .iter()
                .map(|(p, x)| format!("{}: {} of development trips stay inside the study area", p, x.internal_pct.map_or(fmt::DASH.to_string(), rfmt::pct)))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        None => String::new(),
    };
    let t5t = c.table(
        "Table T5 \u{b7} Gateway distribution of development traffic",
        Tbl::new(
            vec![h("Gateway"), h("Basis"), hn("AM in %"), hn("AM out %"), hn("PM in %"), hn("PM out %"), h("veh/h")],
            t5,
        )
        .empty("Run the model for the assessed scenario to see where development traffic enters and leaves."),
        &TableOpts::note(&t5_note),
    );
    format!("{}\n      {}\n      {}", t4, t5t, dev_figures(c, sc))
}

fn _unused(_: JsonValue) {}
