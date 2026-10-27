//! Chapter 4: the transport assessment: scenarios, the model, walking and cycling, extent of effects, junction performance, road
//! user safety, alignment, climate, sensitivity and economics.

use super::calc::{PedInput, band4, cec_model_for, cec_predict, ped_delay, sev_hint, severity_index};
use super::consts::{AT_LICENCE_DEFAULT, PERIODS, cec, hrig, ped, ssm};
use super::ctx::{Ctx, FigRequest, Row, Tbl, TableOpts, disc, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_crash::crash_sites;
use super::sections_env::{CrashSite, control_label, external_legs, j2_table, study_bounds};
use super::sections_front::{int, int_f, year_txt};
use super::shared::{
    assessed_sites, crash_years, junction_rows, plural, site_name,
};
use super::Rep;
use crate::analysis::{ExtentOpts, ExtentTest, extent_test, with_without};
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, number_to_js_string, value_to_js_string, value_truthy};
use crate::results::Res;
use crate::standards::{STANDARDS, growth_basis};
use crate::util::{esc, fmt};
use serde_json::Value;

/// `report-scenarios`.
pub(crate) fn scenarios(c: &mut Ctx) -> String {
    let r = c.r;
    let rows: Vec<Row> = crate::edit::scenarios::table(r.project)
        .iter()
        .map(|d| {
            Row::new(vec![
                esc(if d.id.is_empty() { fmt::DASH } else { &d.id }),
                esc(d.name.as_str().unwrap_or("")),
                esc(&d.role_label),
                year_txt(&number_to_js_string(d.year)),
                esc(&d.land_use),
                esc(&d.network),
                esc(&d.periods),
                d.buildout.map_or(fmt::DASH.to_string(), rfmt::pct),
                esc(&d.growth_basis),
            ])
        })
        .collect();
    let checks: Vec<String> = r
        .project
        .scenarios
        .iter()
        .flat_map(|s| {
            crate::edit::scenarios::check(s, r.project)
                .into_iter()
                .filter(|x| x.level != "info")
                .map(|x| {
                    format!(
                        "{} {}",
                        if x.level == "block" { "<b class=\"bad\">Blocking:</b>" } else { "<b class=\"warn\">Check:</b>" },
                        esc(&x.text)
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let with_name = c.sel.with.map_or(fmt::DASH.to_string(), |i| r.sc(i).name().to_string());
    format!(
        "{}\n      {}\n      {}",
        c.table(
            "Assessment scenarios",
            Tbl::new(
                vec![h("ID"), h("Scenario"), h("Role"), h("Year"), h("Land use"), h("Network"), h("Periods"), hn("Build-out"), h("Growth basis")],
                rows
            ),
            &TableOpts::default()
        ),
        para(&format!(
            "Assessed in this report: <b>{}</b>{}{}.",
            esc(&with_name),
            c.sel.without.map_or(String::new(), |i| format!(" against <b>{}</b>", esc(r.sc(i).name()))),
            c.sel.mitigation.map_or(String::new(), |i| format!(", with mitigation <b>{}</b>", esc(r.sc(i).name())))
        )),
        if checks.is_empty() {
            String::new()
        } else {
            format!("<ul class=\"bul\">{}</ul>", checks.iter().map(|x| format!("<li>{x}</li>")).collect::<String>())
        }
    )
}

/// `report-modelling`.
pub(crate) fn modelling(c: &mut Ctx) -> String {
    let r = c.r;
    let s = r.st();
    let sc = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let reps: Vec<Option<crate::calibrate::Report>> = ["AM", "PM"]
        .iter()
        .map(|p| r.results[sc].get(p).map(|res| crate::calibrate::report(r.project, scn, p, Some(res))))
        .collect();
    let checks: Vec<Vec<Value>> = reps
        .iter()
        .map(|x| x.as_ref().map_or(Vec::new(), |x| x.checks.iter().map(|k| k.to_value()).collect()))
        .collect();
    let mut ids: Vec<String> = Vec::new();
    for cs in &checks {
        for k in cs {
            let id = k.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let find = |i: usize, id: &str| checks[i].iter().find(|k| k.get("id").and_then(Value::as_str) == Some(id));
    let rows: Vec<Row> = ids
        .iter()
        .map(|id| {
            let k0 = find(0, id).or_else(|| find(1, id)).expect("a check");
            let t = |k: &Value, f: &str| -> String {
                match k.get(f) {
                    Some(Value::String(x)) => x.clone(),
                    Some(Value::Null) | None => String::new(),
                    Some(v) => value_to_js_string(v),
                }
            };
            let dash = |x: String| if x.is_empty() { fmt::DASH.to_string() } else { x };
            let mut cs = vec![esc(&t(k0, "label")), esc(&dash(t(k0, "target")))];
            for i in 0..2 {
                cs.push(match find(i, id) {
                    Some(x) => esc(&dash(t(x, "status"))),
                    None => fmt::DASH.to_string(),
                });
            }
            cs.push(esc(&t(k0, "source")));
            cs.push(if k0.get("verified").is_some_and(value_truthy) { "Yes" } else { "No" }.to_string());
            Row::new(cs)
        })
        .collect();
    let bad: Vec<(usize, &str, &Res)> = (0..r.project.scenarios.len())
        .flat_map(|i| r.rep_periods(i).into_iter().filter_map(move |p| r.results[i].get(p).map(|res| (i, p, res))))
        .filter(|(_, _, res)| !res.converged)
        .collect();
    let rep_note = format!(
        "{} Detail in Appendix B.",
        reps.iter()
            .flatten()
            .map(|x| format!("{}: {}", x.period, esc(&x.summary)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let msm = js::path(r.data(), &["msm", "checked"]).is_some_and(JsonValue::truthy);
    format!(
        "{}\n      {}\n      {}\n      {}{}",
        para(&format!(
            "Spare Construct assigns peak-hour vehicle demand (gravity distribution with gateway through-traffic) to the network by conjugate Frank-Wolfe user equilibrium, with BPR link times (\u{3b1} {}, \u{3b2} {}) and HCM 6 / Ak\u{e7}elik junction delay. Background growth: {}. Convergence target: relative gap {}% with the TAG M3.1 stability rule, at most {} iterations. Full inputs are in Appendix A.",
            esc(&s.bpr_alpha.js_string()),
            esc(&s.bpr_beta.js_string()),
            esc(&growth_basis(s)),
            fmt::num(s.gap_tol.num() * 100.0, 3),
            int_f(s.max_iter.num())
        )),
        if !bad.is_empty() {
            note(
                &format!(
                    "{} did not converge: {}. Results from these runs are not reliable.",
                    plural(bad.len() as f64, "run", "runs"),
                    bad.iter()
                        .map(|(i, p, res)| format!("{} {} ({})", esc(r.sc(*i).name()), p, esc(&res.conv_note)))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                true,
            )
        } else {
            para("Every stored run converged to its target (Appendix B).")
        },
        c.table(
            &format!("Table C-SUM \u{b7} Calibration summary, {}", esc(r.sc(sc).name())),
            Tbl::new(vec![h("Criterion"), h("Target"), h("AM"), h("PM"), h("Source"), h("Verified")], rows)
                .empty("Run the existing (base-year) scenario and load counts to see the calibration summary."),
            &TableOpts::note(&rep_note)
        ),
        para(&esc(if msm { "MSM consistency checked (see Appendix B)." } else { STANDARDS.msm.not_checked })),
        disc("D7")
    )
}

/// `report-walk`.
pub(crate) fn walk(c: &mut Ctx) -> String {
    let r = c.r;
    let pd = &r.rs.ped;
    let rows: Vec<Row> = r
        .rs
        .crossings
        .iter()
        .map(|x| {
            let g = |k: &str| js::get(Some(x), k);
            let control = js::text(g("control"));
            let d = ped_delay(
                &control,
                &PedInput {
                    c: js::num(g("C")),
                    g: js::num(g("g")),
                    l: js::num(g("length")),
                    flow: js::num(g("flow")),
                    sp: js::num(pd.get("Sp")),
                    ts: js::num(pd.get("ts")),
                },
            );
            Row::new(vec![
                esc(&js::text(g("name"))),
                esc(&control),
                rfmt::veh(js::num(g("flow"))),
                rfmt::num(js::num(g("length")), 1),
                match d {
                    None => fmt::DASH.to_string(),
                    Some(v) => format!("{}{}", rfmt::num(v, 1), if control == "zebra" { " (drivers give way)" } else { "" }),
                },
                fmt::DASH.to_string(),
                if js::truthy(g("next")) { rfmt::q(js::num(g("next"))) } else { fmt::DASH.to_string() },
                fmt::DASH.to_string(),
            ])
        })
        .collect();
    let sp = {
        let v = pd.get("Sp").filter(|v| v.truthy());
        v.map_or(number_to_js_string(ped::SP), JsonValue::js_string)
    };
    let ts = match pd.get("ts") {
        Some(v) if !v.is_null() => v.js_string(),
        _ => number_to_js_string(ped::TS),
    };
    format!(
        "{}\n      {}",
        c.table(
            "Table WK-1 \u{b7} Pedestrian crossings",
            Tbl::new(
                vec![
                    h("Crossing"),
                    h("Control"),
                    hn("Conflicting veh/h"),
                    hn("Length m"),
                    hn("Pedestrian delay s"),
                    h("ANOP walk LoS (delay)"),
                    hn("Next crossing m"),
                    h("ANOP LoS (spacing)"),
                ],
                rows
            )
            .empty("No crossings entered. Add them in the Report tab."),
            &TableOpts {
                note: Some(format!(
                    "Signals d = (C \u{2212} g)\u{b2} / 2C; uncontrolled d = (e^(\u{3bb}\u{b7}tc) \u{2212} \u{3bb}\u{b7}tc \u{2212} 1) / \u{3bb} with \u{3bb} = conflicting veh/s and tc = L / {} m/s + {} s; a refuge is two stages at half the flow and half the length. ANOP levels of service: confirm in the AT ANOP tool.",
                    esc(&sp),
                    esc(&ts)
                )),
                d: vec!["D5"],
                ..TableOpts::default()
            }
        ),
        c.author("walk", "Assess public transport, walking and cycling levels of service with the proposal (AT ITA s4.2.1).")
    )
}

/// `yearClash(w, wo, periods)`: with and without must be modelled for the same year; otherwise the message to print.
pub(crate) fn year_clash(r: &Rep, w: Option<usize>, wo: Option<usize>) -> String {
    let (Some(w), Some(wo)) = (w, wo) else {
        return String::new();
    };
    let ys = |s: usize| -> Vec<f64> {
        let mut v: Vec<f64> = Vec::new();
        if let Some(y) = r.year_of(s) {
            v.push(y);
        }
        for p in PERIODS {
            if let Some(res) = r.results[s].get(p)
                && res.year != 0.0
                && !res.year.is_nan()
            {
                v.push(res.year);
            }
        }
        v
    };
    let (a, b) = (ys(w), ys(wo));
    if a.iter().all(|y| b.iter().all(|z| z == y)) {
        return String::new();
    }
    format!(
        "{} is modelled for {} and {} for {}. The with-and-without comparison needs both in the same year (AT ITA Guidelines s4.2.1.1); otherwise background growth would be counted as an effect of the proposal. Create a same-year without scenario (Scenarios \u{203a} Create assessment set) or choose one in the Report tab, and run it.",
        r.sc(wo).name(),
        b.first().map_or("?".to_string(), |y| number_to_js_string(*y)),
        r.sc(w).name(),
        a.first().map_or("?".to_string(), |y| number_to_js_string(*y))
    )
}

/// One period's extent test.
pub(crate) struct ExtentPeriod {
    /// Period.
    pub p: String,
    /// The test.
    pub x: ExtentTest,
}

/// `extentData(P, c)`: the extent test per period between the without and with scenarios.
pub(crate) fn extent_data(c: &Ctx) -> Vec<ExtentPeriod> {
    let r = c.r;
    let (Some(w), Some(wo)) = (c.sel.with, c.sel.without) else {
        return Vec::new();
    };
    if !year_clash(r, Some(w), Some(wo)).is_empty() {
        return Vec::new();
    }
    c.periods
        .iter()
        .filter(|p| r.results[w].get(p).is_some() && r.results[wo].get(p).is_some())
        .filter_map(|p| {
            let x = extent_test(
                r.scenario(w),
                r.results[wo].get(p),
                r.results[w].get(p),
                ExtentOpts {
                    sc_without: Some(r.scenario(wo)),
                    ..ExtentOpts::default()
                },
            )?;
            Some(ExtentPeriod { p: p.clone(), x })
        })
        .collect()
}

fn val_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => value_to_js_string(other),
    }
}

/// `report-extent`.
pub(crate) fn extent(c: &mut Ctx) -> String {
    let r = c.r;
    let yc = year_clash(r, c.sel.with, c.sel.without);
    if !yc.is_empty() {
        return note(&esc(&yc), true);
    }
    let ex = extent_data(c);
    if ex.is_empty() {
        return note(
            &format!(
                "The extent test needs a without and a with scenario for the same year, both run for the same period.{} Set them in the Report tab and run both.",
                match (c.sel.with, c.sel.without) {
                    (Some(w), None) => format!(
                        " No same-year without scenario was found for {} ({}).",
                        esc(r.sc(w).name()),
                        r.year_of(w).map_or("null".to_string(), number_to_js_string)
                    ),
                    _ => String::new(),
                }
            ),
            true,
        );
    }
    let (w, wo) = (c.sel.with.expect("with"), c.sel.without.expect("without"));
    let xs = &STANDARDS.extent;
    let mut out = String::new();
    for e in &ex {
        let (p, x) = (&e.p, &e.x);
        let inc: Vec<&crate::analysis::ExtentRow> = x.rows.iter().filter(|q| q.included).collect();
        let list: Vec<&crate::analysis::ExtentRow> = if !inc.is_empty() { inc.clone() } else { x.rows.iter().take(15).collect() };
        let rows: Vec<Row> = list
            .iter()
            .take(60)
            .map(|q| {
                let t = |v: &Value| {
                    let s = val_text(v);
                    if value_truthy(v) { s } else { fmt::DASH.to_string() }
                };
                Row::new(vec![
                    esc(&if q.name.is_empty() { q.link.clone() } else { q.name.clone() }),
                    esc(&q.dir.to_uppercase()),
                    format!(
                        "{}{}",
                        esc(&[t(&q.fc.gt), t(&q.fc.pt), t(&q.fc.freight)].join(" / ")),
                        if value_truthy(&q.basis) { format!(" <span class=\"muted\">({})</span>", esc(&val_text(&q.basis))) } else { String::new() }
                    ),
                    rfmt::veh(q.without),
                    rfmt::veh(q.with),
                    fmt::signed(q.d, 0),
                    esc(&q.p_text),
                    q.threshold.map_or(fmt::DASH.to_string(), |t| rfmt::pct(t * 100.0)),
                    if q.included { "<b>Yes</b>" } else { "No" }.to_string(),
                ])
            })
            .collect();
        let source = format!(
            "{}; Future Connect classes: {}",
            c.source_of(),
            if super::js::get(r.loaded(), "fc").is_some_and(JsonValue::truthy) {
                esc(AT_LICENCE_DEFAULT)
            } else {
                "Spare Construct defaults from road class (Future Connect not loaded)".to_string()
            }
        );
        let nt = format!(
            "{} Included when \u{394}% is over 5% (motorways, arterials, RTN/FTN, freight levels 1A\u{2013}2) or 10% (collectors) and \u{394} \u{2265} {} veh/h. The {} veh/h floor and the freight set are Spare Construct assumptions. {}",
            if !inc.is_empty() {
                format!(
                    "{} and {} are included.{}",
                    plural(inc.len() as f64, "link direction", "link directions"),
                    plural(x.sites.len() as f64, "junction", "junctions"),
                    if list.len() > 60 {
                        format!(" The 60 largest of the {} included link directions are shown; every link is in links.csv in the data pack.", fmt::int(list.len() as f64))
                    } else {
                        String::new()
                    }
                )
            } else {
                "No link passes the thresholds; the 15 largest changes are shown.".to_string()
            },
            number_to_js_string(xs.floor),
            number_to_js_string(xs.floor),
            esc(&x.note)
        );
        out.push_str(&c.table(
            &format!(
                "Extent of effects \u{b7} {} peak \u{b7} {} \u{2192} {}",
                p,
                esc(r.sc(wo).name()),
                esc(r.sc(w).name())
            ),
            Tbl::new(
                vec![h("Link"), h("Dir"), h("FC GT / PT / freight"), hn("Without veh/h"), hn("With veh/h"), hn("\u{394} veh/h"), hn("\u{394} %"), hn("Threshold"), h("Included")],
                rows,
            ),
            &TableOpts {
                sup: Some(true),
                source: Some(source),
                note: Some(nt),
                d: vec!["D13"],
                ..TableOpts::default()
            },
        ));
        let bounds = study_bounds(r, &c.sel);
        for (s, wtxt) in [(wo, "without the proposal"), (w, "with the proposal")] {
            out.push_str(&c.figure(FigRequest {
                id: format!("vol-{}-{}", r.scenario(s).id, p),
                title: format!(
                    "Modelled traffic {}, {}, {} peak hour {} (veh/h; line width shows the flow)",
                    wtxt,
                    r.sc(s).name(),
                    p,
                    r.year_of(s).map_or("null".to_string(), number_to_js_string)
                ),
                mode: "volume".to_string(),
                scenario_id: r.scenario(s).id.clone(),
                base_scenario_id: None,
                period: Some(p.clone()),
                bounds: bounds.clone(),
                show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
                source: None,
            }));
        }
        out.push_str(&c.figure(FigRequest {
            id: format!("diff-{p}"),
            title: format!(
                "Change in traffic, {} peak hour: {} minus {} (veh/h, rounded to the nearest 10)",
                p,
                r.sc(w).name(),
                r.sc(wo).name()
            ),
            mode: "diff".to_string(),
            scenario_id: r.scenario(w).id.clone(),
            base_scenario_id: Some(r.scenario(wo).id.clone()),
            period: Some(p.clone()),
            bounds,
            show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
            source: None,
        }));
    }
    if r.scenario(w).extent.get().is_some_and(value_truthy) {
        out
    } else {
        out + &note("The extent sites are not stored on the scenario yet, so the assessed junctions are Spare Construct's own selection. Store the extent test (junctions feature) to use it for the SIDRA export and this report.", false)
    }
}

/// `report-junctions`.
pub(crate) fn junctions(c: &mut Ctx) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let mut parts: Vec<String> = Vec::new();
    for (sc, t) in [
        (sel.without, "Table J2 \u{b7} Junction performance without the proposal"),
        (sel.with, "Table J2 \u{b7} Junction performance with the proposal"),
        (sel.mitigation, "Table J2 \u{b7} Junction performance with mitigation"),
    ] {
        if let Some(sc) = sc
            && !r.rep_periods(sc).is_empty()
        {
            parts.push(j2_table(c, sc, t));
        }
    }
    // J3 without vs with (same year only)
    let yc = year_clash(r, sel.with, sel.without);
    if !yc.is_empty() {
        parts.push(note(&esc(&yc), true));
    } else if let (Some(wo), Some(w)) = (sel.without, sel.with) {
        let ids = assessed_sites(r, sel.with).ids;
        let mut rows: Vec<Row> = Vec::new();
        for p in c.periods.clone() {
            let (Some(a), Some(b)) = (r.results[wo].get(&p), r.results[w].get(&p)) else {
                continue;
            };
            let scw = r.scenario(w);
            let name = |id: &str| site_name(scw, Some(b), id);
            let Some(ww) = with_without(Some(a), Some(b), Some(&name)) else {
                continue;
            };
            for x in ww.rows.iter().filter(|x| ids.contains(&x.id) && x.delta.is_some()) {
                let d = x.delta.as_ref().expect("a delta");
                let (wo_s, w_s) = (x.without.as_ref().expect("without"), x.with.as_ref().expect("with"));
                rows.push(Row::new(vec![
                    esc(&x.name),
                    p.clone(),
                    rfmt::dos(wo_s.dos),
                    rfmt::dos(w_s.dos),
                    esc(&d.dos_text),
                    rfmt::delay(wo_s.delay),
                    rfmt::delay(w_s.delay),
                    esc(&d.delay_text),
                    esc(&d.los),
                    esc(&d.q95_text),
                ]));
            }
        }
        parts.push(c.table(
            "Table J3 \u{b7} Without and with the proposal",
            Tbl::new(
                vec![
                    h("Junction"),
                    h("Period"),
                    hn("DoS without"),
                    hn("DoS with"),
                    hn("\u{394} DoS"),
                    hn("Delay without s"),
                    hn("Delay with s"),
                    hn("\u{394} delay"),
                    h("LoS change"),
                    hn("\u{394} Q95"),
                ],
                rows,
            ),
            &TableOpts {
                junction: true,
                note: Some(esc(crate::results::SELECT_LABEL)),
                ..TableOpts::default()
            },
        ));
    }
    // queue storage check: approach queue against the approach link length
    let sc = sel.mitigation.or(sel.with);
    let mut qrows: Vec<Row> = Vec::new();
    if let Some(sc) = sc {
        let scn = r.scenario(sc);
        for x in junction_rows(r, &sel, sc, None) {
            let Some(nr) = x.nr else {
                continue;
            };
            for a in &nr.appr {
                let Some(l) = scn.links.get(&a.link) else {
                    continue;
                };
                if !(a.q95 > 0.0) {
                    continue;
                }
                let st = scn.link_len(l);
                let ratio = if st > 0.0 { Some(a.q95 / st) } else { None };
                if let Some(ra) = ratio.filter(|v| *v > 0.5) {
                    qrows.push(Row::new(vec![
                        esc(&x.name),
                        x.p.to_string(),
                        esc(&format!("{}: {}", a.approach, a.name)),
                        rfmt::q(a.q95),
                        rfmt::q(st),
                        rfmt::num(ra, 2),
                        if ra > 1.0 {
                            "<b class=\"bad\">Exceeds storage</b>".to_string()
                        } else if ra > 0.8 {
                            "<span class=\"warn\">Near storage</span>".to_string()
                        } else {
                            String::new()
                        },
                    ]));
                }
            }
        }
        parts.push(c.table(
            &format!("Queue storage check \u{b7} {}", esc(r.sc(sc).name())),
            Tbl::new(vec![h("Junction"), h("Period"), h("Leg"), hn("Q95 m"), hn("Storage m"), hn("Ratio"), h("Flag")], qrows)
                .empty("No approach queue reaches half its storage length."),
            &TableOpts {
                junction: true,
                note: Some("Storage = the length of the approach link to the next junction. Rows with a ratio over 0.5 are listed.".to_string()),
                ..TableOpts::default()
            },
        ));
    }
    let m = js::path(Some(r.raw), &["settings", "junction", "losMethod"])
        .filter(|v| v.truthy())
        .map_or("hcm6".to_string(), JsonValue::js_string);
    let th = STANDARDS.los.get(&m).unwrap_or(&STANDARDS.los.hcm6);
    let n = number_to_js_string;
    let mut lrows: Vec<Row> = ["A", "B", "C", "D", "E"]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            Row::new(vec![
                l.to_string(),
                format!("\u{2264} {}", n(th.signals[i])),
                format!("\u{2264} {}", n(th.roundabout[i])),
                format!("\u{2264} {}", n(th.sign[i])),
            ])
        })
        .collect();
    lrows.push(Row::new(vec![
        "F".to_string(),
        format!("> {} or DoS > 1", n(th.signals[4])),
        format!("> {} or DoS > 1", n(th.roundabout[4])),
        format!("> {} or DoS > 1", n(th.sign[4])),
    ]));
    parts.push(c.table(
        "Level of service definitions",
        Tbl::new(vec![h("LoS"), h("Signals: delay s"), h("Roundabouts: delay s"), h("Give way and stop: delay s")], lrows),
        &TableOpts {
            source: Some(format!(
                "{}, {}{}",
                esc(th.src.name),
                esc(th.src.edition),
                if th.src.verified { "" } else { " (not yet verified)" }
            )),
            junction: true,
            sup: Some(false),
            ..TableOpts::default()
        },
    ));
    if let Some(sc) = sc {
        let bounds = study_bounds(r, &c.sel);
        for p in c.periods.clone() {
            parts.push(c.figure(FigRequest {
                id: format!("vc-{}-{}", r.scenario(sc).id, p),
                title: format!("Volume/capacity and junction level of service, {}, {} peak hour", r.sc(sc).name(), p),
                mode: "vc".to_string(),
                scenario_id: r.scenario(sc).id.clone(),
                base_scenario_id: None,
                period: Some(p.clone()),
                bounds: bounds.clone(),
                show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
                source: None,
            }));
        }
    }
    parts.join("")
        + &note(
            "The junction flow-scale test (+10% on every movement) follows this section, then turning movement diagrams for the key junctions; per-turn movement summaries are in Appendix C. The sensitivity pack (section 4.6) re-runs the whole network with trip rates and total demand \u{b1}10% and background growth \u{b1}50%.",
            false,
        )
}

/// A crash model prediction for a site in one scenario.
struct Pred {
    label: &'static str,
    a: f64,
    dsi: f64,
    flag: String,
}

/// `legAADT(sc, s)`: two-way AADT on each leg of the junction (all its nodes; ring links inside it are not approaches).
fn leg_aadt<'a>(r: &'a Rep, sc: usize, s: &CrashSite) -> Option<Vec<(Vec<&'a crate::project::Link>, f64)>> {
    let scn = r.scenario(sc);
    let rl: Vec<&Res> = r
        .rep_periods(sc)
        .into_iter()
        .filter(|p| *p != "IP")
        .filter_map(|p| r.results[sc].get(p))
        .collect();
    let mem: Vec<String> = if s.nodes.is_empty() { vec![s.id.clone()] } else { s.nodes.clone() }
        .into_iter()
        .filter(|x| scn.nodes.contains_key(x))
        .collect();
    if rl.is_empty() || !scn.nodes.contains_key(&s.id) || mem.is_empty() {
        return None;
    }
    let ptd = {
        let v = r.st().peak_to_daily.num();
        if v == 0.0 || v.is_nan() { 10.0 } else { v }
    };
    let mut legs: Vec<(Vec<&crate::project::Link>, f64)> = external_legs(scn, &mem)
        .into_iter()
        .map(|g| {
            let mut v = 0.0;
            for res in &rl {
                for l in &g.links {
                    let a = res.arcs.get(&format!("{}:ab", l.id));
                    let b = res.arcs.get(&format!("{}:ba", l.id));
                    v += a.map_or(0.0, |x| x.v) + b.map_or(0.0, |x| x.v);
                }
            }
            (g.links, v / rl.len() as f64 * ptd)
        })
        .collect();
    js_sort_by(&mut legs, |a, b| b.1 - a.1);
    Some(legs)
}

fn predict(r: &Rep, sc: usize, s: &CrashSite) -> Option<Pred> {
    let legs = leg_aadt(r, sc, s)?;
    if legs.len() < 3 {
        return None;
    }
    let n = r.scenario(sc).nodes.get(&s.id)?;
    let multi = legs
        .iter()
        .any(|x| x.0.iter().any(|l| math_max(l.lanes_ab.int32() as f64, l.lanes_ba.int32() as f64) >= 2.0));
    let control = n.control.as_str().unwrap_or("");
    let mid = cec_model_for(control, legs.len(), multi)?;
    let q: Vec<f64> = legs.iter().map(|x| x.1).collect();
    let qmaj = (q[0] + q[1]) / 2.0;
    let qmin = if legs.len() >= 4 { (q[2] + q[3]) / 2.0 } else { q[2] };
    let model = super::consts::CEC_MODELS.iter().find(|m| m.key == mid)?;
    let (label, a, flag) = if model.per_approach {
        let first = cec_predict(mid, q[0], 0.0)?;
        let total = legs.iter().fold(0.0, |s, x| s + cec_predict(mid, x.1, 0.0).map_or(f64::NAN, |p| p.a));
        (first.label, total, "range not recorded (check CEC Table 7-3)".to_string())
    } else {
        let p = cec_predict(mid, qmaj, qmin)?;
        (p.label, p.a, p.flag)
    };
    Some(Pred {
        label,
        a,
        dsi: a * severity_index("", control, false),
        flag,
    })
}

/// `ssmMatrix(P, c, s)`: the Safe System screening matrix of a site.
fn ssm_matrix(c: &mut Ctx, s: &CrashSite) -> String {
    let r = c.r;
    let lik = js::get(Some(&r.rs.ssm), &s.id).filter(|v| v.is_object());
    let aadt = s.legs.iter().fold(0.0, |m, l| math_max(m, l.aadt.unwrap_or(0.0)));
    let speed = s.speed;
    let vru = |k: &str| js::get(lik, k);
    struct Cell {
        e: Option<u8>,
        l: Option<f64>,
        s: Option<u8>,
        prod: Option<f64>,
    }
    let cells: Vec<Cell> = ssm::COLS
        .iter()
        .map(|(k, _)| {
            let e = if ["ped", "cyc", "mc"].contains(k) {
                let key = match *k {
                    "ped" => "pedDay",
                    "cyc" => "cycDay",
                    _ => "mcDay",
                };
                let v = vru(key);
                band4(
                    match v {
                        Some(x) if !x.is_null() && !matches!(x, JsonValue::String(t) if t.is_empty()) => Some(x.to_number()),
                        _ => None,
                    },
                    &ssm::VRU_EXPOSURE,
                )
            } else {
                band4(if aadt != 0.0 { Some(aadt) } else { None }, &ssm::VEH_EXPOSURE)
            };
            let lk_v = js::num(vru(k));
            let lk = if lk_v > 0.0 { Some(lk_v) } else { None };
            let sv = sev_hint(speed, ssm::safe(k));
            let prod = match (e, lk, sv) {
                (Some(e), Some(l), Some(sv)) => Some(e as f64 * l * sv as f64),
                _ => None,
            };
            Cell { e, l: lk, s: sv, prod }
        })
        .collect();
    let row = |lab: &str, f: &dyn Fn(&Cell) -> String| {
        let mut v = vec![lab.to_string()];
        v.extend(cells.iter().map(f));
        Row::new(v)
    };
    let q = |x: Option<f64>| x.map_or("?".to_string(), number_to_js_string);
    let rows = vec![
        row("Exposure (1\u{2013}4)", &|x| q(x.e.map(|v| v as f64))),
        row("Likelihood (1\u{2013}4, author)", &|x| q(x.l)),
        row("Severity (1\u{2013}4)", &|x| q(x.s.map(|v| v as f64))),
        row("<b>Product (max 64)</b>", &|x| match x.prod {
            Some(p) => format!("<b>{}</b>", number_to_js_string(p)),
            None => fmt::DASH.to_string(),
        }),
    ];
    let mut head = vec![h("")];
    head.extend(ssm::COLS.iter().map(|(_, l)| h(l)));
    c.table(
        &format!("Safe System screening matrix \u{b7} {}", esc(&s.name)),
        Tbl::new(head, rows),
        &TableOpts {
            note: Some(format!(
                "<b>{}.</b> Exposure from the busiest leg's AADT ({} vpd; < 1k = 1, 1\u{2013}5k = 2, 5\u{2013}10k = 3, > 10k = 4) and from pedestrian, cyclist and motorcyclist counts per day where entered (< 10, 10\u{2013}50, 50\u{2013}100, > 100). Severity hints: {} (Safe System speeds: pedestrians, cyclists and motorcyclists 30, run-off-road 40, side impact 50, head-on 70 km/h; posted {} km/h). Likelihood is the author's.",
                esc(ssm::LABEL),
                rfmt::adt(aadt),
                esc(ssm::SEVERITY_BASIS),
                if speed != 0.0 { number_to_js_string(speed) } else { "?".to_string() }
            )),
            source: Some(format!("{}, {}", esc(ssm::NAME), esc(ssm::EDITION))),
            d: vec!["D13"],
            ..TableOpts::default()
        },
    )
}

/// `report-safety`.
pub(crate) fn safety(c: &mut Ctx) -> String {
    let r = c.r;
    let cy = crash_years(r);
    let sel = c.sel.clone();
    let sites = crash_sites(c);
    let ex_sc = match (sel.existing, sel.with) {
        (Some(e), w) if Some(e) != w => Some(e),
        _ => None,
    };
    let mut rows: Vec<Row> = Vec::new();
    for s in sites.iter().take(20) {
        let a = sel.without.and_then(|i| predict(r, i, s));
        let b = sel.with.and_then(|i| predict(r, i, s));
        let m = sel.mitigation.and_then(|i| predict(r, i, s));
        let e = ex_sc.and_then(|i| predict(r, i, s));
        if a.is_none() && b.is_none() {
            continue;
        }
        let obs = s.injury / hrig::YEARS;
        let refp = a.as_ref().or(b.as_ref()).expect("a prediction");
        rows.push(Row::new(vec![
            esc(&s.name),
            esc(refp.label),
            format!(
                "{} \u{2192} {}{}",
                a.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.a, 3)),
                b.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.a, 3)),
                m.as_ref().map_or(String::new(), |x| format!(" \u{2192} {}", rfmt::num(x.a, 3)))
            ),
            format!(
                "{} \u{2192} {}{}",
                a.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.dsi, 4)),
                b.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.dsi, 4)),
                m.as_ref().map_or(String::new(), |x| format!(" \u{2192} {}", rfmt::num(x.dsi, 4)))
            ),
            match (&a, &b) {
                (Some(a), Some(b)) => fmt::signed(b.dsi - a.dsi, 4),
                _ => fmt::DASH.to_string(),
            },
            rfmt::num(obs, 2),
            match &e {
                Some(e) if e.a > 0.0 => rfmt::num(obs / e.a, 2),
                _ => fmt::DASH.to_string(),
            },
            esc(&refp.flag),
        ]));
    }
    let ssm_rows: String = sites.iter().take(3).map(|s| ssm_matrix(c, s)).collect();
    let nt = format!(
        "A = b0\u{b7}Qmaj^b1\u{b7}Qmin^b2 with two-way AADT (modelled peak \u{d7} peak-to-daily factor); Qmaj and Qmin are the means of the two busiest and the remaining legs (Spare Construct reading). DSI from the CEC severity indices. A junction of several nodes (such as a roundabout) is one site, with the links that leave it as its approaches. Observed \u{f7} predicted divides the observed injury crashes a year by the prediction from the existing (base-year) scenario's volumes. Midblock and rural (80 km/h and over) models are not applied."
    );
    format!(
        "{}\n      {}\n      {}{}{}",
        para(&match &cy {
            Some(cy) => format!(
                "Section 2.7 sets out the crash history for {}\u{2013}{}: {}, {} of them fatal or serious.",
                number_to_js_string(cy.from),
                number_to_js_string(cy.to),
                plural(cy.total.n, "crash", "crashes"),
                number_to_js_string(cy.total.f + cy.total.s)
            ),
            None => "No crash history is loaded (section 2.7).".to_string(),
        }),
        c.table(
            "Effects of additional trips: predicted crashes (CEC)",
            Tbl::new(
                vec![
                    h("Site"),
                    h("Model"),
                    h("Predicted injury crashes/yr (without \u{2192} with \u{2192} mitigation)"),
                    h("Predicted DSI/yr"),
                    hn("\u{394} DSI/yr"),
                    hn("Observed injury/yr"),
                    hn("Observed \u{f7} predicted"),
                    h("Range"),
                ],
                rows
            )
            .empty("Run the without and with scenarios, and load crashes, to predict crash changes."),
            &TableOpts::src_note(&format!("{}, {} (coefficients verified: no)", esc(cec::NAME), esc(cec::EDITION)), &nt)
        ),
        ssm_rows,
        c.author("ssa", "AT ITA 4.3.5: is a Stage 1 or Stage 2 Safe System Audit needed? State why, and who will carry it out."),
        disc("D8")
    )
}

/// `report-alignment`.
pub(crate) fn alignment(c: &mut Ctx) -> String {
    let r = c.r;
    let w = c.sel.with;
    let mut cls: Vec<(String, f64)> = Vec::new();
    if let Some(w) = w {
        let scn = r.scenario(w);
        for l in scn.links.values() {
            let Some(fc) = l.fc.get().filter(|f| value_truthy(f)) else {
                continue;
            };
            if l.status.is("closed") {
                continue;
            }
            let k = match fc.get("gt") {
                Some(v) if value_truthy(v) => value_to_js_string(v),
                _ => "Unclassified".to_string(),
            };
            let len = scn.link_len(l) / 1000.0;
            match cls.iter().position(|(x, _)| *x == k) {
                Some(i) => cls[i].1 += len,
                None => cls.push((k, len)),
            }
        }
    }
    let has_fc = w.is_some_and(|w| {
        r.scenario(w)
            .links
            .values()
            .any(|l| l.fc.get().is_some_and(value_truthy) && !l.status.is("closed"))
    });
    let t = if has_fc {
        js_sort_by(&mut cls, |a, b| b.1 - a.1);
        c.table(
            "Future Connect general traffic classes in the study area",
            Tbl::new(vec![h("Class"), hn("km")], cls.iter().map(|(k, v)| Row::new(vec![esc(k), rfmt::num(*v, 2)])).collect()),
            &TableOpts {
                source: Some(esc(AT_LICENCE_DEFAULT)),
                ..TableOpts::default()
            },
        )
    } else {
        String::new()
    };
    format!(
        "{}\n      {}",
        t,
        c.author("alignment", "Assess alignment with Future Connect, ANOP, RLTP and LTP projects, the Future Development Strategy (where required) and NPS-UD Policy 1.")
    )
}

/// `report-climate`.
pub(crate) fn climate(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        note("No climate hazard overlay is loaded for this project, so Spare Construct has not screened the network against flood, coastal inundation or slip hazards.", false),
        c.author("climate", "AT ITA s4.5: describe how the proposal and its access routes respond to climate hazards (flooding, coastal inundation, slips, heat), and any resilience measures.")
    )
}

/// `report-sensitivity`.
pub(crate) fn sensitivity(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(sv) = r.rs.sens.as_ref().filter(|s| s.get("rows").is_some_and(JsonValue::truthy)) else {
        return note("The sensitivity pack has not been run (Report tab \u{203a} Run the sensitivity pack). AT ITA s3.6.1(v) and the Practice Note s9.3(a)(x) expect sensitivity testing.", true);
    };
    let g = |x: &JsonValue, k: &str| js::get(Some(x), k).cloned();
    let w = |x: Option<&JsonValue>| -> String {
        match x {
            Some(o) if o.is_object() || o.is_array() => {
                let name = js::text_or_empty(js::get(Some(o), "name"));
                let name = if !name.is_empty() { name } else { js::text_or_empty(js::get(Some(o), "id")) };
                format!(
                    "{}{}",
                    esc(&name),
                    match js::get(Some(o), "dos").filter(|v| !v.is_null()) {
                        Some(d) => format!(" (DoS {})", rfmt::dos(d.to_number())),
                        None => String::new(),
                    }
                )
            }
            Some(v) if v.truthy() => esc(&v.js_string()),
            _ => esc(fmt::DASH),
        }
    };
    let rows_in = js::items(js::get(Some(sv), "rows"));
    let rows: Vec<Row> = rows_in
        .iter()
        .map(|rw| {
            let gv = |k: &str| js::get(Some(rw), k);
            let notnull = |k: &str| gv(k).filter(|v| !v.is_null());
            let input = js::text_or_empty(gv("input"));
            Row::new(vec![
                esc(&js::text(gv("label"))),
                esc(&if input.is_empty() { fmt::DASH.to_string() } else { input }),
                notnull("trips").map_or(fmt::DASH.to_string(), |v| rfmt::veh(v.to_number())),
                notnull("vht").map_or(fmt::DASH.to_string(), |v| rfmt::num(v.to_number(), 1)),
                w(gv("worst")),
                notnull("losEF").map_or(fmt::DASH.to_string(), |v| int(Some(v))),
                match notnull("over5") {
                    Some(o5) => format!("{} / {}", int(Some(o5)), int(gv("over10"))),
                    None => fmt::DASH.to_string(),
                },
                if js::truthy(gv("elasticity")) {
                    rfmt::num(js::num(js::get(gv("elasticity"), "vht")), 2)
                } else if js::truthy(gv("bcrText")) {
                    format!("BCR {}", esc(&js::text(gv("bcr8Text"))))
                } else {
                    fmt::DASH.to_string()
                },
            ])
        })
        .collect();
    let bcr_any = rows_in.iter().any(|rw| js::truthy(js::get(Some(rw), "bcrText")) || js::truthy(js::get(Some(rw), "bcr8Text")));
    let flags = js::items(js::get(Some(sv), "flags"));
    let _ = g;
    format!(
        "{}\n      <ul class=\"bul\">{}</ul>{}",
        c.table(
            &format!(
                "Table SENS-1 \u{b7} Sensitivity tests ({}, run {})",
                esc(&js::text_or_empty(js::get(Some(sv), "scName"))),
                esc(&super::ctx::day_of(js::get(Some(sv), "at")))
            ),
            Tbl::new(
                vec![h("Test"), h("Input change"), hn("Veh trips"), hn("VHT veh\u{b7}h"), h("Worst site (DoS)"), hn("Sites LoS E/F"), h("Links over 5% / 10%"), hn("VHT elasticity")],
                rows
            ),
            &TableOpts {
                note: Some(esc(&js::text_or_empty(js::get(Some(sv), "note")))),
                d: if bcr_any { vec!["D4"] } else { Vec::new() },
                ..TableOpts::default()
            }
        ),
        js::items(js::get(Some(sv), "sentences"))
            .iter()
            .map(|s| format!("<li>{}</li>", esc(&js::text(Some(s)))))
            .collect::<String>(),
        if flags.is_empty() {
            String::new()
        } else {
            note(&flags.iter().map(|f| esc(&js::text(Some(f)))).collect::<Vec<_>>().join(" "), true)
        }
    )
}

/// `TRIP_TOL`: the option's vehicle trips within 1% of the do-minimum's.
const TRIP_TOL: f64 = 0.01;

fn ev(e: &Value, k: &str) -> Option<f64> {
    match e.get(k) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => match s.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        },
        _ => None,
    }
}

fn et(e: &Value, k: &str) -> String {
    match e.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => value_to_js_string(v),
    }
}

/// `report-economics`.
pub(crate) fn economics(c: &mut Ctx) -> String {
    let r = c.r;
    let (Some(base), Some(opt)) = (c.sel.with, c.sel.mitigation) else {
        return note("Indicative economics compare a transport option (the mitigation scenario) with the do-minimum (the scenario with the proposal), at the same travel demand. The development itself is not a transport option, so no benefit-cost ratio is given for it. Create a mitigation scenario (Scenarios \u{203a} Create assessment set) or choose one in the Report tab.", false);
    };
    let period = c.period.clone();
    let econ = |b: usize, o: usize| {
        crate::econ::econ(r.st(), r.scenario(b), &r.results[b], r.scenario(o), &r.results[o], &period)
            .map(|e| serde_json::to_value(&e).expect("economics serialise"))
    };
    let mut out: Vec<Row> = Vec::new();
    let blank = |first: String, second: String| {
        let mut v = vec![first, second];
        v.extend((0..8).map(|_| String::new()));
        Row::new(v)
    };
    let e = econ(base, opt);
    match &e {
        None => out.push(blank(esc(r.sc(opt).name()), "<span class=\"muted\">Run both scenarios first.</span>".to_string())),
        Some(e) if e.get("mismatch").is_some_and(value_truthy) => out.push(blank(esc(r.sc(opt).name()), esc(&et(e, "note")))),
        Some(e) => {
            let tb = r.results[base].get(&period);
            let to = r.results[opt].get(&period);
            let tdiff = match (tb, to) {
                (Some(tb), Some(to)) if tb.kpi.trips > 0.0 => (to.kpi.trips - tb.kpi.trips).abs() / tb.kpi.trips,
                _ => 0.0,
            };
            if tdiff > TRIP_TOL {
                let (tb, to) = (tb.expect("base"), to.expect("option"));
                out.push(blank(
                    esc(r.sc(opt).name()),
                    format!(
                        "<span class=\"warn\">Not comparable: {} has {} vehicle trips and the do-minimum {} has {} ({} apart, more than {}). Benefits would include the change in demand. Give both the same land use.</span>",
                        esc(r.sc(opt).name()),
                        rfmt::veh(to.kpi.trips),
                        esc(r.sc(base).name()),
                        rfmt::veh(tb.kpi.trips),
                        rfmt::pct(tdiff * 100.0),
                        rfmt::pct(TRIP_TOL * 100.0)
                    ),
                ));
            } else {
                let ipm = e
                    .get("ipm")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|b| {
                                format!(
                                    "{}{}: {}",
                                    esc(&et(b, "edition")),
                                    if b.get("draft").is_some_and(value_truthy) { " (draft, subject to GPS 2027)" } else { "" },
                                    esc(&et(b, "label"))
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("<br>")
                    })
                    .unwrap_or_default();
                let bcr_text = {
                    let t = et(e, "bcrText");
                    if !t.is_empty() { t } else { crate::econ::bcr_text(ev(e, "bcr").filter(|v| !v.is_nan())) }
                };
                let bcr8 = {
                    let t = et(e, "bcr8Text");
                    if t.is_empty() { fmt::DASH.to_string() } else { t }
                };
                let snr = e.get("snr").filter(|s| value_truthy(s));
                out.push(Row::new(vec![
                    esc(r.sc(opt).name()),
                    fmt::money(ev(e, "pv").unwrap_or(f64::NAN)),
                    fmt::money(ev(e, "pv8").unwrap_or(f64::NAN)),
                    fmt::money(ev(e, "cap").unwrap_or(f64::NAN)),
                    esc(&bcr_text),
                    esc(&bcr8),
                    match ev(e, "fyrr") {
                        Some(f) if !e.get("fyrr").is_some_and(Value::is_null) => rfmt::pct(f * 100.0),
                        _ => fmt::DASH.to_string(),
                    },
                    fmt::money(ev(e, "npv").unwrap_or(f64::NAN)),
                    if ipm.is_empty() { fmt::DASH.to_string() } else { ipm },
                    match snr.and_then(|s| ev(s, "R")).filter(|_| snr.is_some_and(|s| s.get("R").is_some_and(|v| !v.is_null()))) {
                        Some(rv) => format!(
                            "{}\u{d7}{}",
                            rfmt::num(rv, 1),
                            if snr.and_then(|s| s.get("ok")).is_some_and(value_truthy) { "" } else { " <b class=\"warn\">low</b>" }
                        ),
                        None => fmt::DASH.to_string(),
                    },
                ]));
                for wtxt in e.get("warnings").and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[]) {
                    out.push(Row::span(format!("<span class=\"warn\">{}</span>", esc(&val_text(wtxt))), 10));
                }
            }
        }
    }
    let ok0 = e.as_ref().is_some_and(|e| !e.get("mismatch").is_some_and(value_truthy));
    let e0 = e.as_ref();
    let sens = if ok0 && e0.is_some_and(|e| ev(e, "sensRate").is_some() && !e.get("sensRate").is_some_and(Value::is_null)) {
        format!("{}%", number_to_js_string(e0.and_then(|e| ev(e, "sensRate")).unwrap_or(f64::NAN)))
    } else {
        "sensitivity rate".to_string()
    };
    let cen = if ok0 {
        let e = e0.expect("economics");
        match e.get("rates").and_then(Value::as_array) {
            Some(a) if !a.is_empty() => a
                .iter()
                .map(|x| format!("{}%", val_text(x.get("r").unwrap_or(&Value::Null))))
                .collect::<Vec<_>>()
                .join(" / "),
            _ => {
                let disc = js::path(Some(r.raw), &["settings", "econ", "discount"]).filter(|v| !v.is_null());
                if e.get("method").and_then(Value::as_str) == Some("uniform") && disc.is_some() {
                    format!("{}% flat", number_to_js_string(disc.map_or(f64::NAN, JsonValue::to_number)))
                } else {
                    "central rates".to_string()
                }
            }
        }
    } else {
        "central rates".to_string()
    };
    let gc = e0
        .filter(|_| ok0)
        .and_then(|e| e.get("discount"))
        .filter(|d| value_truthy(d))
        .map_or(String::new(), |d| format!(" Discount rates: {} ({}).", esc(&et(d, "name")), esc(&et(d, "edition"))));
    let label = e0.map(|e| et(e, "label")).filter(|t| !t.is_empty()).unwrap_or_else(|| STANDARDS.mbcm.label.to_string());
    let basis = e0.map(|e| et(e, "basis")).unwrap_or_default();
    let head = vec![
        h("Option"),
        hn(&format!("PV benefits ({})", esc(&cen))),
        hn(&format!("PV ({})", esc(&sens))),
        hn("Capital"),
        hn("BCR"),
        hn(&format!("BCR at {}", esc(&sens))),
        hn("FYRR"),
        hn("NPV"),
        h("IPM efficiency"),
        hn("Signal to noise"),
    ];
    c.table(
        "Indicative economics (screening only)",
        Tbl::new(head, out),
        &TableOpts {
            note: Some(format!(
                "{}. {}.{} Option against the do-minimum {} (with the proposal), {}. Capital costs are Spare Construct's indicative lane-km and junction rates; the 1% trip tolerance is a Spare Construct assumption.",
                esc(&label),
                esc(&basis),
                gc,
                esc(r.sc(base).name()),
                r.year_of(base).map_or(String::new(), number_to_js_string)
            )),
            d: vec!["D4", "D13"],
            ..TableOpts::default()
        },
    )
}

fn _unused(_: &Rep, _: Vec<String>) {
    let _ = (control_label, year_txt, int, super::shared::JRow::clone);
}
