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
use super::{Rep, Sel};
use crate::analysis::{ExtentOpts, ExtentTest, extent_test, with_without};
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, number_to_js_string, usize_to_f64, value_to_js_string, value_truthy};
use crate::results::Res;
use crate::standards::{STANDARDS, growth_basis};
use crate::util::{esc, fmt};
use serde_json::Value;
use std::fmt::Write as _;

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
            let items = checks.iter().fold(String::new(), |mut acc, x| {
                // Writing to a String cannot fail
                let _ = write!(acc, "<li>{x}</li>");
                acc
            });
            format!("<ul class=\"bul\">{items}</ul>")
        }
    )
}

/// `report-modelling`.
pub(crate) fn modelling(c: &mut Ctx) -> String {
    let r = c.r;
    let st = r.st();
    let sc = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let reps: Vec<Option<crate::calibrate::Report>> = ["AM", "PM"]
        .iter()
        .map(|p| r.results.get(sc).and_then(|rs| rs.get(p)).map(|res| crate::calibrate::report(r.project, scn, p, Some(res))))
        .collect();
    let checks: Vec<Vec<Value>> = reps
        .iter()
        .map(|rep| rep.as_ref().map_or(Vec::new(), |rep| rep.checks.iter().map(crate::calibrate::Check::to_value).collect()))
        .collect();
    let mut ids: Vec<String> = Vec::new();
    for cs in &checks {
        for chk in cs {
            let id = chk.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let find = |i: usize, id: &str| checks.get(i)?.iter().find(|chk| chk.get("id").and_then(Value::as_str) == Some(id));
    let rows: Vec<Row> = ids
        .iter()
        .filter_map(|id| {
            // every id comes from one of the checks, so the first or second period always has it
            let k0 = find(0, id).or_else(|| find(1, id))?;
            let text_of = |chk: &Value, field: &str| -> String {
                match chk.get(field) {
                    Some(Value::String(txt)) => txt.clone(),
                    Some(Value::Null) | None => String::new(),
                    Some(val) => value_to_js_string(val),
                }
            };
            let dash = |txt: String| if txt.is_empty() { fmt::DASH.to_string() } else { txt };
            let mut cs = vec![esc(&text_of(k0, "label")), esc(&dash(text_of(k0, "target")))];
            for i in 0..2 {
                cs.push(match find(i, id) {
                    Some(found) => esc(&dash(text_of(found, "status"))),
                    None => fmt::DASH.to_string(),
                });
            }
            cs.push(esc(&text_of(k0, "source")));
            cs.push(if k0.get("verified").is_some_and(value_truthy) { "Yes" } else { "No" }.to_string());
            Some(Row::new(cs))
        })
        .collect();
    let bad: Vec<(usize, &str, &Res)> = (0..r.project.scenarios.len())
        .flat_map(|i| r.rep_periods(i).into_iter().filter_map(move |p| r.results.get(i).and_then(|rs| rs.get(p)).map(|res| (i, p, res))))
        .filter(|(_, _, res)| !res.converged)
        .collect();
    let rep_note = format!(
        "{} Detail in Appendix B.",
        reps.iter()
            .flatten()
            .map(|rep| format!("{}: {}", rep.period, esc(&rep.summary)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let msm = js::path(r.data(), &["msm", "checked"]).is_some_and(JsonValue::truthy);
    format!(
        "{}\n      {}\n      {}\n      {}{}",
        para(&format!(
            "Spare Construct assigns peak-hour vehicle demand (gravity distribution with gateway through-traffic) to the network by conjugate Frank-Wolfe user equilibrium, with BPR link times (\u{3b1} {}, \u{3b2} {}) and HCM 6 / Ak\u{e7}elik junction delay. Background growth: {}. Convergence target: relative gap {}% with the TAG M3.1 stability rule, at most {} iterations. Full inputs are in Appendix A.",
            esc(&st.bpr_alpha.js_string()),
            esc(&st.bpr_beta.js_string()),
            esc(&growth_basis(st)),
            fmt::num(st.gap_tol.num() * 100.0, 3),
            int_f(st.max_iter.num())
        )),
        if bad.is_empty() {
            para("Every stored run converged to its target (Appendix B).")
        } else {
            note(
                &format!(
                    "{} did not converge: {}. Results from these runs are not reliable.",
                    plural(usize_to_f64(bad.len()), "run", "runs"),
                    bad.iter()
                        .map(|(i, p, res)| format!("{} {} ({})", esc(r.sc(*i).name()), p, esc(&res.conv_note)))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                true,
            )
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
            let field = |key: &str| js::get(Some(x), key);
            let control = js::text(field("control"));
            let delay = ped_delay(
                &control,
                &PedInput {
                    c: js::num(field("C")),
                    g: js::num(field("g")),
                    l: js::num(field("length")),
                    flow: js::num(field("flow")),
                    sp: js::num(pd.get("Sp")),
                    ts: js::num(pd.get("ts")),
                },
            );
            Row::new(vec![
                esc(&js::text(field("name"))),
                esc(&control),
                rfmt::veh(js::num(field("flow"))),
                rfmt::num(js::num(field("length")), 1),
                match delay {
                    None => fmt::DASH.to_string(),
                    Some(v) => format!("{}{}", rfmt::num(v, 1), if control == "zebra" { " (drivers give way)" } else { "" }),
                },
                fmt::DASH.to_string(),
                if js::truthy(field("next")) { rfmt::q(js::num(field("next"))) } else { fmt::DASH.to_string() },
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
            if let Some(res) = r.results.get(s).and_then(|rs| rs.get(p))
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
    // w and wo are positions in the project's scenario list, which the results follow
    let (Some(res_with), Some(res_without)) = (r.results.get(w), r.results.get(wo)) else {
        return Vec::new();
    };
    c.periods
        .iter()
        .filter(|p| res_with.get(p).is_some() && res_without.get(p).is_some())
        .filter_map(|p| {
            let x = extent_test(
                r.scenario(w),
                res_without.get(p),
                res_with.get(p),
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

/// The rows of an extent table (at most 60): each link direction's classes, flows, change, threshold and whether it is included.
fn extent_rows(list: &[&crate::analysis::ExtentRow]) -> Vec<Row> {
    list.iter()
        .take(60)
        .map(|q| {
            let shown = |v: &Value| {
                let text = val_text(v);
                if value_truthy(v) { text } else { fmt::DASH.to_string() }
            };
            Row::new(vec![
                esc(&if q.name.is_empty() { q.link.clone() } else { q.name.clone() }),
                esc(&q.dir.to_uppercase()),
                format!(
                    "{}{}",
                    esc(&[shown(&q.fc.gt), shown(&q.fc.pt), shown(&q.fc.freight)].join(" / ")),
                    if value_truthy(&q.basis) { format!(" <span class=\"muted\">({})</span>", esc(&val_text(&q.basis))) } else { String::new() }
                ),
                rfmt::veh(q.without),
                rfmt::veh(q.with),
                fmt::signed(q.d, 0),
                esc(&q.p_text),
                q.threshold.map_or(fmt::DASH.to_string(), |th| rfmt::pct(th * 100.0)),
                if q.included { "<b>Yes</b>" } else { "No" }.to_string(),
            ])
        })
        .collect()
}

/// The note under an extent table: how many link directions and junctions are included, and the thresholds.
fn extent_note(test: &ExtentTest, inc: usize, list: usize) -> String {
    let xs = &STANDARDS.extent;
    format!(
        "{} Included when \u{394}% is over 5% (motorways, arterials, RTN/FTN, freight levels 1A\u{2013}2) or 10% (collectors) and \u{394} \u{2265} {} veh/h. The {} veh/h floor and the freight set are Spare Construct assumptions. {}",
        if inc == 0 {
            "No link passes the thresholds; the 15 largest changes are shown.".to_string()
        } else {
            format!(
                "{} and {} are included.{}",
                plural(usize_to_f64(inc), "link direction", "link directions"),
                plural(usize_to_f64(test.sites.len()), "junction", "junctions"),
                if list > 60 {
                    format!(" The 60 largest of the {} included link directions are shown; every link is in links.csv in the data pack.", fmt::int(usize_to_f64(list)))
                } else {
                    String::new()
                }
            )
        },
        number_to_js_string(xs.floor),
        number_to_js_string(xs.floor),
        esc(&test.note)
    )
}

/// The extent figures for period `p`: modelled traffic without (`wo`) and with (`w`) the proposal, and the change between them.
fn extent_figures(c: &mut Ctx, w: usize, wo: usize, p: &str) -> String {
    let r = c.r;
    let mut out = String::new();
    let bounds = study_bounds(r, &c.sel);
    for (si, wtxt) in [(wo, "without the proposal"), (w, "with the proposal")] {
        out.push_str(&c.figure(FigRequest {
            id: format!("vol-{}-{}", r.scenario(si).id, p),
            title: format!(
                "Modelled traffic {}, {}, {} peak hour {} (veh/h; line width shows the flow)",
                wtxt,
                r.sc(si).name(),
                p,
                r.year_of(si).map_or("null".to_string(), number_to_js_string)
            ),
            mode: "volume".to_string(),
            scenario_id: r.scenario(si).id.clone(),
            base_scenario_id: None,
            period: Some(p.to_string()),
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
        period: Some(p.to_string()),
        bounds,
        show: vec![("counts".to_string(), false), ("crashes".to_string(), false)],
        source: None,
    }));
    out
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
    // extent_data gives periods only when both scenarios are set, so this never returns
    let (Some(w), Some(wo)) = (c.sel.with, c.sel.without) else {
        return String::new();
    };
    let mut out = String::new();
    for ep in &ex {
        let (p, test) = (&ep.p, &ep.x);
        let inc: Vec<&crate::analysis::ExtentRow> = test.rows.iter().filter(|row| row.included).collect();
        let list: Vec<&crate::analysis::ExtentRow> = if inc.is_empty() { test.rows.iter().take(15).collect() } else { inc.clone() };
        let rows = extent_rows(&list);
        let source = format!(
            "{}; Future Connect classes: {}",
            c.source_of(),
            if super::js::get(r.loaded(), "fc").is_some_and(JsonValue::truthy) {
                esc(AT_LICENCE_DEFAULT)
            } else {
                "Spare Construct defaults from road class (Future Connect not loaded)".to_string()
            }
        );
        let nt = extent_note(test, inc.len(), list.len());
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
        out.push_str(&extent_figures(c, w, wo, p));
    }
    if r.scenario(w).extent.get().is_some_and(value_truthy) {
        out
    } else {
        out + &note("The extent sites are not stored on the scenario yet, so the assessed junctions are Spare Construct's own selection. Store the extent test (junctions feature) to use it for the SIDRA export and this report.", false)
    }
}

/// Table J3: each assessed junction without (scenario `wo`) and with (scenario `w`) the proposal, period by period.
fn j3_table(c: &mut Ctx, sel: &Sel, wo: usize, w: usize) -> String {
    let r = c.r;
    let ids = assessed_sites(r, sel.with).ids;
    let mut rows: Vec<Row> = Vec::new();
    for p in c.periods.clone() {
        let (Some(res_without), Some(res_with)) =
            (r.results.get(wo).and_then(|res| res.get(&p)), r.results.get(w).and_then(|res| res.get(&p)))
        else {
            continue;
        };
        let scw = r.scenario(w);
        let name = |id: &str| site_name(scw, Some(res_with), id);
        let Some(ww) = with_without(Some(res_without), Some(res_with), Some(&name)) else {
            continue;
        };
        for wwr in ww.rows.iter().filter(|wwr| ids.contains(&wwr.id) && wwr.delta.is_some()) {
            // a row has its changes only when both sides are there, so this never skips a row the filter kept
            let (Some(delta), Some(before), Some(after)) = (wwr.delta.as_ref(), wwr.without.as_ref(), wwr.with.as_ref()) else {
                continue;
            };
            rows.push(Row::new(vec![
                esc(&wwr.name),
                p.clone(),
                rfmt::dos(before.dos),
                rfmt::dos(after.dos),
                esc(&delta.dos_text),
                rfmt::delay(before.delay),
                rfmt::delay(after.delay),
                esc(&delta.delay_text),
                esc(&delta.los),
                esc(&delta.q95_text),
            ]));
        }
    }
    c.table(
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
    )
}

/// The queue storage check of scenario `sc`: each approach whose 95th-percentile queue passes half its link's length.
fn queue_check(c: &mut Ctx, sel: &Sel, sc: usize) -> String {
    let r = c.r;
    let mut qrows: Vec<Row> = Vec::new();
    let scn = r.scenario(sc);
    for jr in junction_rows(r, sel, sc, None) {
        let Some(nr) = jr.nr else {
            continue;
        };
        for ap in &nr.appr {
            let Some(link) = scn.links.get(&ap.link) else {
                continue;
            };
            // `!(q95 > 0)`: NaN queues are skipped too
            if ap.q95 <= 0.0 || ap.q95.is_nan() {
                continue;
            }
            let st = scn.link_len(link);
            let ratio = if st > 0.0 { Some(ap.q95 / st) } else { None };
            if let Some(ra) = ratio.filter(|v| *v > 0.5) {
                qrows.push(Row::new(vec![
                    esc(&jr.name),
                    jr.p.to_string(),
                    esc(&format!("{}: {}", ap.approach, ap.name)),
                    rfmt::q(ap.q95),
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
    c.table(
        &format!("Queue storage check \u{b7} {}", esc(r.sc(sc).name())),
        Tbl::new(vec![h("Junction"), h("Period"), h("Leg"), hn("Q95 m"), hn("Storage m"), hn("Ratio"), h("Flag")], qrows)
            .empty("No approach queue reaches half its storage length."),
        &TableOpts {
            junction: true,
            note: Some("Storage = the length of the approach link to the next junction. Rows with a ratio over 0.5 are listed.".to_string()),
            ..TableOpts::default()
        },
    )
}

/// The level of service definitions of the project's `LoS` method.
fn los_table(c: &mut Ctx) -> String {
    let r = c.r;
    let method = js::path(Some(r.raw), &["settings", "junction", "losMethod"])
        .filter(|v| v.truthy())
        .map_or("hcm6".to_string(), JsonValue::js_string);
    let th = STANDARDS.los.get(&method).unwrap_or(&STANDARDS.los.hcm6);
    let n = number_to_js_string;
    let mut lrows: Vec<Row> = ["A", "B", "C", "D", "E"]
        .iter()
        .zip(th.signals)
        .zip(th.roundabout)
        .zip(th.sign)
        .map(|(((l, signals), roundabout), sign)| {
            Row::new(vec![
                l.to_string(),
                format!("\u{2264} {}", n(signals)),
                format!("\u{2264} {}", n(roundabout)),
                format!("\u{2264} {}", n(sign)),
            ])
        })
        .collect();
    lrows.push(Row::new(vec![
        "F".to_string(),
        format!("> {} or DoS > 1", n(th.signals[4])),
        format!("> {} or DoS > 1", n(th.roundabout[4])),
        format!("> {} or DoS > 1", n(th.sign[4])),
    ]));
    c.table(
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
    )
}

/// `report-junctions`.
pub(crate) fn junctions(c: &mut Ctx) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let mut parts: Vec<String> = Vec::new();
    for (sc, title) in [
        (sel.without, "Table J2 \u{b7} Junction performance without the proposal"),
        (sel.with, "Table J2 \u{b7} Junction performance with the proposal"),
        (sel.mitigation, "Table J2 \u{b7} Junction performance with mitigation"),
    ] {
        if let Some(sc) = sc
            && !r.rep_periods(sc).is_empty()
        {
            parts.push(j2_table(c, sc, title));
        }
    }
    // J3 without vs with (same year only)
    let yc = year_clash(r, sel.with, sel.without);
    if !yc.is_empty() {
        parts.push(note(&esc(&yc), true));
    } else if let (Some(wo), Some(w)) = (sel.without, sel.with) {
        parts.push(j3_table(c, &sel, wo, w));
    }
    // queue storage check: approach queue against the approach link length
    let sc = sel.mitigation.or(sel.with);
    if let Some(sc) = sc {
        parts.push(queue_check(c, &sel, sc));
    }
    parts.push(los_table(c));
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
        .filter_map(|p| r.results.get(sc).and_then(|res| res.get(p)))
        .collect();
    let mem: Vec<String> = if s.nodes.is_empty() { vec![s.id.clone()] } else { s.nodes.clone() }
        .into_iter()
        .filter(|id| scn.nodes.contains_key(id))
        .collect();
    if rl.is_empty() || !scn.nodes.contains_key(&s.id) || mem.is_empty() {
        return None;
    }
    let ptd = {
        let pd = r.st().peak_to_daily.num();
        if pd == 0.0 || pd.is_nan() { 10.0 } else { pd }
    };
    let mut legs: Vec<(Vec<&crate::project::Link>, f64)> = external_legs(scn, &mem)
        .into_iter()
        .map(|leg| {
            let mut vol = 0.0;
            for res in &rl {
                for link in &leg.links {
                    let ab = res.arcs.get(&format!("{}:ab", link.id));
                    let ba = res.arcs.get(&format!("{}:ba", link.id));
                    vol += ab.map_or(0.0, |arc| arc.v) + ba.map_or(0.0, |arc| arc.v);
                }
            }
            (leg.links, vol / usize_to_f64(rl.len()) * ptd)
        })
        .collect();
    js_sort_by(&mut legs, |lhs, rhs| rhs.1 - lhs.1);
    Some(legs)
}

fn predict(r: &Rep, sc: usize, s: &CrashSite) -> Option<Pred> {
    let legs = leg_aadt(r, sc, s)?;
    if legs.len() < 3 {
        return None;
    }
    let node = r.scenario(sc).nodes.get(&s.id)?;
    let multi = legs
        .iter()
        .any(|leg| leg.0.iter().any(|link| math_max(f64::from(link.lanes_ab.int32()), f64::from(link.lanes_ba.int32())) >= 2.0));
    let control = node.control.as_str().unwrap_or("");
    let mid = cec_model_for(control, legs.len(), multi)?;
    let qs: Vec<f64> = legs.iter().map(|leg| leg.1).collect();
    // legs has at least three entries (checked above), so the last arm never applies
    let (q0, qmaj, qmin) = match qs.as_slice() {
        [q0, q1, q2, q3, ..] => (*q0, f64::midpoint(*q0, *q1), f64::midpoint(*q2, *q3)),
        [q0, q1, q2] => (*q0, f64::midpoint(*q0, *q1), *q2),
        _ => return None,
    };
    let model = super::consts::CEC_MODELS.iter().find(|md| md.key == mid)?;
    let (label, crashes, flag) = if model.per_approach {
        let first = cec_predict(mid, q0, 0.0)?;
        let total = legs.iter().fold(0.0, |sum, leg| sum + cec_predict(mid, leg.1, 0.0).map_or(f64::NAN, |pr| pr.a));
        (first.label, total, "range not recorded (check CEC Table 7-3)".to_string())
    } else {
        let pr = cec_predict(mid, qmaj, qmin)?;
        (pr.label, pr.a, pr.flag)
    };
    Some(Pred {
        label,
        a: crashes,
        dsi: crashes * severity_index("", control, false),
        flag,
    })
}

/// `ssmMatrix(P, c, s)`: the Safe System screening matrix of a site.
fn ssm_matrix(c: &mut Ctx, s: &CrashSite) -> String {
    struct Cell {
        e: Option<u8>,
        l: Option<f64>,
        s: Option<u8>,
        prod: Option<f64>,
    }
    let r = c.r;
    let lik = js::get(Some(&r.rs.ssm), &s.id).filter(|val| val.is_object());
    let aadt = s.legs.iter().fold(0.0, |mx, leg| math_max(mx, leg.aadt.unwrap_or(0.0)));
    let speed = s.speed;
    let vru = |key: &str| js::get(lik, key);
    let cells: Vec<Cell> = ssm::COLS
        .iter()
        .map(|(col, _)| {
            let expo = if ["ped", "cyc", "mc"].contains(col) {
                let key = match *col {
                    "ped" => "pedDay",
                    "cyc" => "cycDay",
                    _ => "mcDay",
                };
                let val = vru(key);
                band4(
                    match val {
                        Some(x) if !x.is_null() && !matches!(x, JsonValue::String(txt) if txt.is_empty()) => Some(x.to_number()),
                        _ => None,
                    },
                    &ssm::VRU_EXPOSURE,
                )
            } else {
                band4(if aadt == 0.0 { None } else { Some(aadt) }, &ssm::VEH_EXPOSURE)
            };
            let lk_v = js::num(vru(col));
            let lk = if lk_v > 0.0 { Some(lk_v) } else { None };
            let sv = sev_hint(speed, ssm::safe(col));
            let prod = match (expo, lk, sv) {
                (Some(ex), Some(lv), Some(sv)) => Some(f64::from(ex) * lv * f64::from(sv)),
                _ => None,
            };
            Cell { e: expo, l: lk, s: sv, prod }
        })
        .collect();
    let row = |lab: &str, cell_text: &dyn Fn(&Cell) -> String| {
        let mut out = vec![lab.to_string()];
        out.extend(cells.iter().map(cell_text));
        Row::new(out)
    };
    let qtext = |x: Option<f64>| x.map_or("?".to_string(), number_to_js_string);
    let rows = vec![
        row("Exposure (1\u{2013}4)", &|x| qtext(x.e.map(f64::from))),
        row("Likelihood (1\u{2013}4, author)", &|x| qtext(x.l)),
        row("Severity (1\u{2013}4)", &|x| qtext(x.s.map(f64::from))),
        row("<b>Product (max 64)</b>", &|x| match x.prod {
            Some(prod) => format!("<b>{}</b>", number_to_js_string(prod)),
            None => fmt::DASH.to_string(),
        }),
    ];
    let mut head = vec![h("")];
    head.extend(ssm::COLS.iter().map(|(_, label)| h(label)));
    c.table(
        &format!("Safe System screening matrix \u{b7} {}", esc(&s.name)),
        Tbl::new(head, rows),
        &TableOpts {
            note: Some(format!(
                "<b>{}.</b> Exposure from the busiest leg's AADT ({} vpd; < 1k = 1, 1\u{2013}5k = 2, 5\u{2013}10k = 3, > 10k = 4) and from pedestrian, cyclist and motorcyclist counts per day where entered (< 10, 10\u{2013}50, 50\u{2013}100, > 100). Severity hints: {} (Safe System speeds: pedestrians, cyclists and motorcyclists 30, run-off-road 40, side impact 50, head-on 70 km/h; posted {} km/h). Likelihood is the author's.",
                esc(ssm::LABEL),
                rfmt::adt(aadt),
                esc(ssm::SEVERITY_BASIS),
                if speed == 0.0 { "?".to_string() } else { number_to_js_string(speed) }
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
        (Some(ex), with) if Some(ex) != with => Some(ex),
        _ => None,
    };
    let mut rows: Vec<Row> = Vec::new();
    for s in sites.iter().take(20) {
        let before = sel.without.and_then(|sc| predict(r, sc, s));
        let after = sel.with.and_then(|sc| predict(r, sc, s));
        let mitig = sel.mitigation.and_then(|sc| predict(r, sc, s));
        let existing = ex_sc.and_then(|sc| predict(r, sc, s));
        let Some(refp) = before.as_ref().or(after.as_ref()) else {
            continue;
        };
        let obs = s.injury / hrig::YEARS;
        rows.push(Row::new(vec![
            esc(&s.name),
            esc(refp.label),
            format!(
                "{} \u{2192} {}{}",
                before.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.a, 3)),
                after.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.a, 3)),
                mitig.as_ref().map_or(String::new(), |x| format!(" \u{2192} {}", rfmt::num(x.a, 3)))
            ),
            format!(
                "{} \u{2192} {}{}",
                before.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.dsi, 4)),
                after.as_ref().map_or(fmt::DASH.to_string(), |x| rfmt::num(x.dsi, 4)),
                mitig.as_ref().map_or(String::new(), |x| format!(" \u{2192} {}", rfmt::num(x.dsi, 4)))
            ),
            match (&before, &after) {
                (Some(bf), Some(af)) => fmt::signed(af.dsi - bf.dsi, 4),
                _ => fmt::DASH.to_string(),
            },
            rfmt::num(obs, 2),
            match &existing {
                Some(ex) if ex.a > 0.0 => rfmt::num(obs / ex.a, 2),
                _ => fmt::DASH.to_string(),
            },
            esc(&refp.flag),
        ]));
    }
    let ssm_rows: String = sites.iter().take(3).map(|s| ssm_matrix(c, s)).collect();
    let nt = "A = b0\u{b7}Qmaj^b1\u{b7}Qmin^b2 with two-way AADT (modelled peak \u{d7} peak-to-daily factor); Qmaj and Qmin are the means of the two busiest and the remaining legs (Spare Construct reading). DSI from the CEC severity indices. A junction of several nodes (such as a roundabout) is one site, with the links that leave it as its approaches. Observed \u{f7} predicted divides the observed injury crashes a year by the prediction from the existing (base-year) scenario's volumes. Midblock and rural (80 km/h and over) models are not applied.".to_string();
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
        for link in scn.links.values() {
            let Some(fc) = link.fc.get().filter(|fcv| value_truthy(fcv)) else {
                continue;
            };
            if link.status.is("closed") {
                continue;
            }
            let class = match fc.get("gt") {
                Some(val) if value_truthy(val) => value_to_js_string(val),
                _ => "Unclassified".to_string(),
            };
            let len = scn.link_len(link) / 1000.0;
            match cls.iter_mut().find(|(name, _)| *name == class) {
                Some(entry) => entry.1 += len,
                None => cls.push((class, len)),
            }
        }
    }
    let has_fc = w.is_some_and(|w| {
        r.scenario(w)
            .links
            .values()
            .any(|link| link.fc.get().is_some_and(value_truthy) && !link.status.is("closed"))
    });
    let t = if has_fc {
        js_sort_by(&mut cls, |lhs, rhs| rhs.1 - lhs.1);
        c.table(
            "Future Connect general traffic classes in the study area",
            Tbl::new(vec![h("Class"), hn("km")], cls.iter().map(|(class, km)| Row::new(vec![esc(class), rfmt::num(*km, 2)])).collect()),
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
    let Some(sv) = r.rs.sens.as_ref().filter(|sens| sens.get("rows").is_some_and(JsonValue::truthy)) else {
        return note("The sensitivity pack has not been run (Report tab \u{203a} Run the sensitivity pack). AT ITA s3.6.1(v) and the Practice Note s9.3(a)(x) expect sensitivity testing.", true);
    };
    let get_owned = |val: &JsonValue, key: &str| js::get(Some(val), key).cloned();
    let worst_text = |val: Option<&JsonValue>| -> String {
        match val {
            Some(obj) if obj.is_object() || obj.is_array() => {
                let name = js::text_or_empty(js::get(Some(obj), "name"));
                let name = if name.is_empty() { js::text_or_empty(js::get(Some(obj), "id")) } else { name };
                format!(
                    "{}{}",
                    esc(&name),
                    match js::get(Some(obj), "dos").filter(|v| !v.is_null()) {
                        Some(dos) => format!(" (DoS {})", rfmt::dos(dos.to_number())),
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
            let gv = |key: &str| js::get(Some(rw), key);
            let notnull = |key: &str| gv(key).filter(|v| !v.is_null());
            let input = js::text_or_empty(gv("input"));
            Row::new(vec![
                esc(&js::text(gv("label"))),
                esc(&if input.is_empty() { fmt::DASH.to_string() } else { input }),
                notnull("trips").map_or(fmt::DASH.to_string(), |v| rfmt::veh(v.to_number())),
                notnull("vht").map_or(fmt::DASH.to_string(), |v| rfmt::num(v.to_number(), 1)),
                worst_text(gv("worst")),
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
    let _ = get_owned;
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
        js::items(js::get(Some(sv), "sentences")).iter().fold(String::new(), |mut acc, sentence| {
            // Writing to a String cannot fail
            let _ = write!(acc, "<li>{}</li>", esc(&js::text(Some(sentence))));
            acc
        }),
        if flags.is_empty() {
            String::new()
        } else {
            note(&flags.iter().map(|flag| esc(&js::text(Some(flag)))).collect::<Vec<_>>().join(" "), true)
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

/// `econ(b, o)` as JSON: the economics of option o against the do-minimum b (None when they cannot be compared).
fn econ_json(r: &Rep<'_>, period: &str, b: usize, o: usize) -> Option<Value> {
    // b and o are positions in the project's scenario list, which the results follow, so both are always there
    let (Some(res_b), Some(res_o)) = (r.results.get(b), r.results.get(o)) else {
        return None;
    };
    // The economics hold numbers (non-finite ones as text) and text, which always serialise
    crate::econ::econ(r.st(), r.scenario(b), res_b, r.scenario(o), res_o, period).and_then(|ec| serde_json::to_value(&ec).ok())
}

/// The economics table's rows for the option: a note when the runs are missing or not comparable, otherwise its results and
/// warnings.
fn econ_rows(r: &Rep<'_>, period: &str, (base, opt): (usize, usize), e: Option<&Value>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let blank = |first: String, second: String| {
        let mut v = vec![first, second];
        v.extend((0..8).map(|_| String::new()));
        Row::new(v)
    };
    match e {
        None => out.push(blank(esc(r.sc(opt).name()), "<span class=\"muted\">Run both scenarios first.</span>".to_string())),
        Some(e) if e.get("mismatch").is_some_and(value_truthy) => out.push(blank(esc(r.sc(opt).name()), esc(&et(e, "note")))),
        Some(e) => {
            let tb = r.results.get(base).and_then(|x| x.get(period));
            let to = r.results.get(opt).and_then(|x| x.get(period));
            let tdiff = match (tb, to) {
                (Some(tb), Some(to)) if tb.kpi.trips > 0.0 => (to.kpi.trips - tb.kpi.trips).abs() / tb.kpi.trips,
                _ => 0.0,
            };
            // tdiff is above the tolerance only when both runs are there
            if let (Some(tb), Some(to)) = (tb, to)
                && tdiff > TRIP_TOL
            {
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
                out.extend(econ_option_rows(r, opt, e));
            }
        }
    }
    out
}

/// The option's row of the economics table (benefits, BCRs, FYRR, NPV, IPM efficiency, signal to noise), then its warnings.
fn econ_option_rows(r: &Rep<'_>, opt: usize, e: &Value) -> Vec<Row> {
    let ipm = e
        .get("ipm")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{}{}: {}",
                        esc(&et(item, "edition")),
                        if item.get("draft").is_some_and(value_truthy) { " (draft, subject to GPS 2027)" } else { "" },
                        esc(&et(item, "label"))
                    )
                })
                .collect::<Vec<_>>()
                .join("<br>")
        })
        .unwrap_or_default();
    let bcr_text = {
        let own = et(e, "bcrText");
        if own.is_empty() { crate::econ::bcr_text(ev(e, "bcr").filter(|val| !val.is_nan())) } else { own }
    };
    let bcr8 = {
        let own = et(e, "bcr8Text");
        if own.is_empty() { fmt::DASH.to_string() } else { own }
    };
    let snr = e.get("snr").filter(|sn| value_truthy(sn));
    let mut out = vec![Row::new(vec![
        esc(r.sc(opt).name()),
        fmt::money(ev(e, "pv").unwrap_or(f64::NAN)),
        fmt::money(ev(e, "pv8").unwrap_or(f64::NAN)),
        fmt::money(ev(e, "cap").unwrap_or(f64::NAN)),
        esc(&bcr_text),
        esc(&bcr8),
        match ev(e, "fyrr") {
            Some(fyrr) if !e.get("fyrr").is_some_and(Value::is_null) => rfmt::pct(fyrr * 100.0),
            _ => fmt::DASH.to_string(),
        },
        fmt::money(ev(e, "npv").unwrap_or(f64::NAN)),
        if ipm.is_empty() { fmt::DASH.to_string() } else { ipm },
        match snr.and_then(|sn| ev(sn, "R")).filter(|_| snr.is_some_and(|sn| sn.get("R").is_some_and(|val| !val.is_null()))) {
            Some(rv) => format!(
                "{}\u{d7}{}",
                rfmt::num(rv, 1),
                if snr.and_then(|sn| sn.get("ok")).is_some_and(value_truthy) { "" } else { " <b class=\"warn\">low</b>" }
            ),
            None => fmt::DASH.to_string(),
        },
    ])];
    for wtxt in e.get("warnings").and_then(Value::as_array).into_iter().flatten() {
        out.push(Row::span(format!("<span class=\"warn\">{}</span>", esc(&val_text(wtxt))), 10));
    }
    out
}

/// The discount rates the economics table's head names: the sensitivity rate, and the central rates.
fn econ_rate_labels(r: &Rep<'_>, e0: Option<&Value>, ok0: bool) -> (String, String) {
    let sens = if ok0 && e0.is_some_and(|e| ev(e, "sensRate").is_some() && !e.get("sensRate").is_some_and(Value::is_null)) {
        format!("{}%", number_to_js_string(e0.and_then(|e| ev(e, "sensRate")).unwrap_or(f64::NAN)))
    } else {
        "sensitivity rate".to_string()
    };
    // ok0 is true only when there are economics
    let cen = match e0.filter(|_| ok0) {
        Some(e) => match e.get("rates").and_then(Value::as_array) {
            Some(rates) if !rates.is_empty() => rates
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
        },
        None => "central rates".to_string(),
    };
    (sens, cen)
}

/// `report-economics`.
pub(crate) fn economics(c: &mut Ctx) -> String {
    let r = c.r;
    let (Some(base), Some(opt)) = (c.sel.with, c.sel.mitigation) else {
        return note("Indicative economics compare a transport option (the mitigation scenario) with the do-minimum (the scenario with the proposal), at the same travel demand. The development itself is not a transport option, so no benefit-cost ratio is given for it. Create a mitigation scenario (Scenarios \u{203a} Create assessment set) or choose one in the Report tab.", false);
    };
    let period = c.period.clone();
    let e = econ_json(r, &period, base, opt);
    let out = econ_rows(r, &period, (base, opt), e.as_ref());
    let ok0 = e.as_ref().is_some_and(|e| !e.get("mismatch").is_some_and(value_truthy));
    let e0 = e.as_ref();
    let (sens, cen) = econ_rate_labels(r, e0, ok0);
    let gc = e0
        .filter(|_| ok0)
        .and_then(|e| e.get("discount"))
        .filter(|dr| value_truthy(dr))
        .map_or(String::new(), |dr| format!(" Discount rates: {} ({}).", esc(&et(dr, "name")), esc(&et(dr, "edition"))));
    let label = e0.map(|e| et(e, "label")).filter(|txt| !txt.is_empty()).unwrap_or_else(|| STANDARDS.mbcm.label.to_string());
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
