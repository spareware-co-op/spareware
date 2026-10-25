//! Front matter: cover, contents, regulatory context, document control, data vintage, model status, scope, limitations and the
//! executive summary.

use super::consts::{AT_LICENCE_DEFAULT, Licence, PERIODS, VINTAGE, attrib, level, reg, rr422, scope};
use super::ctx::{Ctx, Head, Row, Tbl, TableOpts, cells, disc, disc_with, h, hn, note, para};
use super::js;
use super::shared::{RegCtx, assessed_sites, crash_years, j_sum, junction_count, junction_rows, model_version, reg_ctx, plural};
use crate::edit::json::JsonValue;
use crate::jsnum::{number_to_js_string, value_truthy};
use crate::results::iso_day_of_ms;
use crate::util::{esc, fmt};

/// `day(t)` of a stored time.
fn day(v: Option<&JsonValue>) -> String {
    super::ctx::day_of(v)
}

/// `int(v)`: a number from the project, or an en dash (never raw text).
pub(crate) fn int(v: Option<&JsonValue>) -> String {
    match v {
        None | Some(JsonValue::Null) => fmt::DASH.to_string(),
        Some(JsonValue::String(s)) if s.is_empty() => fmt::DASH.to_string(),
        Some(x) => {
            let n = x.to_number();
            if n.is_finite() { fmt::int(n) } else { fmt::DASH.to_string() }
        }
    }
}

/// `int(v)` for a number that may be missing.
pub(crate) fn int_f(v: f64) -> String {
    if v.is_finite() { fmt::int(v) } else { fmt::DASH.to_string() }
}

/// `yearTxt(v)`: a four-digit year, or an en dash.
pub(crate) fn year_txt(v: &str) -> String {
    let t = crate::jsnum::js_trim(v);
    if t.len() == 4 && t.bytes().all(|b| b.is_ascii_digit()) {
        t.to_string()
    } else {
        fmt::DASH.to_string()
    }
}

fn at_lic() -> &'static str {
    AT_LICENCE_DEFAULT
}

/// The rows of the data vintage table (`vintageRows`), unescaped.
pub(crate) fn vintage_rows(r: &super::Rep) -> Vec<Vec<String>> {
    let l = r.loaded();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let fv0 = js::path(Some(r.raw), &["feeds", "vintage"]).filter(|v| v.is_object());
    let any_osm = r
        .project
        .scenarios
        .iter()
        .any(|s| s.links.values().any(|lk| lk.osm.get().is_some_and(value_truthy)));
    let mut seen: Vec<&str> = Vec::new();
    for (k, src, ds, lic) in VINTAGE.iter() {
        seen.push(k);
        let x = js::get(l, k).filter(|v| v.truthy());
        let feed_key = if *k == "ags23" { "ags" } else { k };
        if x.is_none() && js::get(fv0, feed_key).is_some_and(JsonValue::truthy) {
            continue; // loaded by the feeds feature: its own row follows
        }
        let lic_text = match lic {
            Licence::Fixed(t) => t.to_string(),
            Licence::At => at_lic().to_string(),
        };
        if *k == "osm" && x.is_none() {
            let n = r
                .project
                .scenarios
                .first()
                .map_or(0.0, |s| s.links.values().filter(|lk| lk.osm.get().is_some_and(value_truthy)).count() as f64);
            rows.push(vec![
                src.to_string(),
                ds.to_string(),
                if any_osm { "Imported (query date not recorded)".to_string() } else { "Not used".to_string() },
                if any_osm { plural(n, "link", "links") } else { fmt::DASH.to_string() },
                lic_text,
            ]);
            continue;
        }
        let at = x.and_then(|x| js::get(Some(x), "at")).filter(|a| a.truthy());
        let n_cell = match x {
            Some(x) => {
                let n = js::get(Some(x), "n").filter(|n| !n.is_null());
                let base = match n {
                    Some(n) => {
                        let v = n.to_number();
                        if v.is_finite() { fmt::int(v) } else { fmt::DASH.to_string() }
                    }
                    None => fmt::DASH.to_string(),
                };
                let lab = js::get(Some(x), "label").filter(|v| v.truthy());
                match lab {
                    Some(l) if *k == "cas" => format!("{base} ({})", l.js_string()),
                    _ => base,
                }
            }
            None => fmt::DASH.to_string(),
        };
        let licence = x
            .and_then(|x| js::get(Some(x), "licence"))
            .filter(|v| v.truthy())
            .map_or(lic_text, JsonValue::js_string);
        rows.push(vec![
            src.to_string(),
            ds.to_string(),
            match at {
                Some(a) => day(Some(a)),
                None => "Not loaded".to_string(),
            },
            n_cell,
            licence,
        ]);
    }
    for (k, x) in js::entries(l) {
        if seen.contains(&k) || !(x.is_object() || x.is_array()) {
            continue;
        }
        if !x.truthy() {
            continue;
        }
        let g = |key: &str| js::get(Some(x), key).filter(|v| v.truthy());
        rows.push(vec![
            g("source").map_or(k.to_string(), JsonValue::js_string),
            g("dataset").or_else(|| g("label")).map_or(k.to_string(), JsonValue::js_string),
            match g("at") {
                Some(a) => day(Some(a)),
                None => fmt::DASH.to_string(),
            },
            match js::get(Some(x), "n").filter(|n| !n.is_null()) {
                Some(n) => int(Some(n)),
                None => fmt::DASH.to_string(),
            },
            g("licence").map_or("See source".to_string(), JsonValue::js_string),
        ]);
    }
    // the official feeds and the land-use sources keep their own logs
    for (k, x) in js::entries(fv0) {
        if !(x.is_object() || x.is_array()) {
            continue;
        }
        let g = |key: &str| js::get(Some(x), key).filter(|v| v.truthy());
        let note_t = g("note").or_else(|| g("label")).map_or(k.to_string(), JsonValue::js_string);
        rows.push(vec![
            g("source").map_or(k.to_string(), JsonValue::js_string),
            format!(
                "{}{}",
                note_t,
                g("error").map_or(String::new(), |e| format!(" (failed: {})", e.js_string()))
            ),
            match g("at") {
                Some(a) => {
                    let d = day(Some(a));
                    if d.is_empty() { fmt::DASH.to_string() } else { d }
                }
                None => fmt::DASH.to_string(),
            },
            match js::get(Some(x), "n").filter(|n| !n.is_null()) {
                Some(n) => int(Some(n)),
                None => fmt::DASH.to_string(),
            },
            g("licence").map_or("See source".to_string(), JsonValue::js_string),
        ]);
    }
    let ls = js::path(Some(r.raw), &["landuse", "sources"]).filter(|v| v.is_object());
    for (k, x) in js::entries(ls) {
        if !(x.is_object() || x.is_array()) {
            continue;
        }
        let g = |key: &str| js::get(Some(x), key).filter(|v| v.truthy());
        rows.push(vec![
            g("name").map_or(k.to_string(), JsonValue::js_string),
            g("vintage").map_or(k.to_string(), JsonValue::js_string),
            match g("at") {
                Some(a) => {
                    let d = day(Some(a));
                    if d.is_empty() { fmt::DASH.to_string() } else { d }
                }
                None => fmt::DASH.to_string(),
            },
            match js::get(Some(x), "n").filter(|n| !n.is_null()) {
                Some(n) => int(Some(n)),
                None => fmt::DASH.to_string(),
            },
            g("licence").map_or("See source".to_string(), JsonValue::js_string),
        ]);
    }
    rows
}

/// `report-cover`.
pub(crate) fn cover(c: &mut Ctx) -> String {
    let r = c.r;
    let d = &r.rs.details;
    let rg = reg_ctx(r);
    let pw = if rg.label.is_empty() { "Pathway not stated".to_string() } else { rg.label.clone() };
    let row = |k: &str, v: &str| {
        format!(
            "<tr><th>{}</th><td>{}</td></tr>",
            k,
            if v.is_empty() { "<span class=\"muted\">Not stated</span>" } else { v }
        )
    };
    let stale = (0..r.project.scenarios.len()).any(|i| {
        PERIODS.iter().any(|p| r.results[i].get(p).is_some_and(|res| res.stale))
    });
    let with = c.sel.with.map(|i| {
        format!(
            "{} ({})",
            r.sc(i).name(),
            r.year_of(i).map_or("null".to_string(), number_to_js_string)
        )
    });
    let signed = r.rs.signed_off();
    let site = {
        let s = js::text_or_empty(d.get("site"));
        if s.is_empty() { "Site address not stated".to_string() } else { s }
    };
    format!(
        "<div class=\"cover\"><div class=\"kicker\">Integrated Transport Assessment</div><h1 class=\"title\">{}</h1>\n      <div class=\"subtitle\">{}</div>\n      <table class=\"meta\">{}{}{}{}\n        {}{}{}{}</table>\n      {}\n      {}\n      {}</div>",
        esc(r.name()),
        esc(&site),
        row("Proposal type", &esc(&pw)),
        row("Client", &esc(&js::text(d.get("client")))),
        row("Author", &esc(&js::text(d.get("author")))),
        row("Reviewer", &esc(&js::text(r.rs.signoff.get("reviewer")))),
        row("Report date", &esc(&{
            let t = js::text_or_empty(d.get("date"));
            if t.is_empty() { iso_day_of_ms(r.now_ms).unwrap_or_default() } else { t }
        })),
        row("Lodgement date", &esc(&rg.date)),
        row("Assessed scenario", &esc(&with.unwrap_or_default())),
        row("Model version", &esc(&model_version(r))),
        if signed {
            format!(
                "<p class=\"stamp ok\">Reviewed by {}{}.</p>",
                esc(&js::text(r.rs.signoff.get("reviewer"))),
                if js::truthy(r.rs.signoff.get("date")) {
                    format!(", {}", esc(&js::text(r.rs.signoff.get("date"))))
                } else {
                    String::new()
                }
            )
        } else {
            "<p class=\"stamp draft\">DRAFT: not yet reviewed and signed off.</p>".to_string()
        },
        if stale {
            note("Some results are out of date: the network or demand changed after the last run. Run the model again before issuing this report.", true)
        } else {
            String::new()
        },
        disc("D1")
    )
}

/// `report-contents`: a placeholder the numbering fills in.
pub(crate) fn contents(_c: &mut Ctx) -> String {
    "%%CONTENTS%%".to_string()
}

/// `report-regulatory`.
pub(crate) fn regulatory(c: &mut Ctx) -> String {
    let rg = reg_ctx(c.r);
    let t1 = c.table(
        "Pathway, regime and version stamp",
        Tbl::new(
            vec![h("Item"), h("Value")],
            vec![
                Row::new(cells(&["Pathway".to_string(), esc(if rg.label.is_empty() { "Not stated" } else { &rg.label })])),
                Row::new(cells(&["Lodgement date".to_string(), esc(if rg.date.is_empty() { "Not stated" } else { &rg.date })])),
                Row::new(cells(&["Regime".to_string(), esc(&rg.regime)])),
                Row::new(cells(&["Versions relied on".to_string(), reg::STAMPS.iter().map(|s| esc(s)).collect::<Vec<_>>().join("<br>")])),
                Row::new(cells(&["Law and guidance as at".to_string(), esc(reg::AS_AT)])),
            ],
        ),
        &TableOpts {
            note: Some(
                if rg.from == "compliance" {
                    "Pathway, lodgement date and regime from the Compliance tab."
                } else {
                    "Pathway and lodgement date from the Report tab. Set them in the Compliance tab when it is installed, so every section uses one regime."
                }
                .to_string(),
            ),
            ..TableOpts::default()
        },
    );
    let t2 = c.table(
        "Authorities",
        Tbl::new(
            vec![h("Body"), h("Role")],
            reg::AUTHORITIES.iter().map(|(a, b)| Row::new(vec![esc(a), esc(b)])).collect(),
        ),
        &TableOpts::default(),
    );
    format!("{}\n      {}\n      {}{}", t1, t2, disc_with("D10", Some(reg::AS_AT), None), disc("D11"))
}

/// `report-doc-control`.
pub(crate) fn doc_control(c: &mut Ctx) -> String {
    let rows: Vec<Row> = c
        .r
        .rs
        .revisions
        .iter()
        .map(|r| {
            let g = |k: &str| esc(&js::text(r.get(k)));
            Row::new(vec![g("rev"), g("date"), g("desc"), g("prepared"), g("reviewed"), g("approved")])
        })
        .collect();
    c.table(
        "Document control",
        Tbl::new(vec![h("Rev"), h("Date"), h("Description"), h("Prepared"), h("Reviewed"), h("Approved")], rows)
            .empty("No revisions recorded. Add them in the Report tab."),
        &TableOpts::default(),
    )
}

/// `report-data-vintage`.
pub(crate) fn data_vintage(c: &mut Ctx) -> String {
    let rows: Vec<Row> = vintage_rows(c.r)
        .iter()
        .map(|r| Row::new(r.iter().map(|x| esc(x)).collect()))
        .collect();
    c.table(
        "Data vintage",
        Tbl::new(vec![h("Source"), h("Dataset"), h("Query date"), h("Records"), h("Licence")], rows),
        &TableOpts::note("Every number in this report is tagged with its basis where it is shown: measured, derived, estimated, assumed or user-entered."),
    )
}

/// `report-model-status`.
pub(crate) fn model_status(c: &mut Ctx) -> String {
    let r = c.r;
    let tm = r.st().tmdg.get();
    if let Some(cm) = r.host.foreign_section("compliance-model-status")
        && !cm.html.trim().is_empty()
    {
        return super::build::merged_html(c, &cm, "Calibration and validation status");
    }
    let cat = tm
        .and_then(|t| if t.category.truthy() { Some(t.category.js_string()) } else { None })
        .unwrap_or_else(|| "E".to_string())
        .to_uppercase();
    let just = tm.map_or(String::new(), |t| if t.justification.truthy() { t.justification.js_string() } else { String::new() });
    let rows: Vec<Row> = c
        .periods
        .iter()
        .map(|p| {
            let s = crate::results::model_status(r.project, p);
            let d = if crate::jsnum::value_truthy(&s.date) {
                crate::results::js_iso_day(&s.date).unwrap_or_default()
            } else {
                fmt::DASH.to_string()
            };
            Row::new(vec![p.clone(), esc(&s.label), esc(&d)])
        })
        .collect();
    format!(
        "{}\n      {}{}",
        para(&format!(
            "Model purpose: NZTA TMDG category <b>{}</b>. {}",
            esc(&cat),
            if !just.is_empty() {
                format!("Justification: {}", esc(&just))
            } else {
                "<span class=\"warn\">The justification for this category has not been given (Settings \u{203a} Calibration and validation \u{203a} Why this category).</span>".to_string()
            }
        )),
        c.table("Calibration and validation status", Tbl::new(vec![h("Period"), h("Status"), h("Date")], rows), &TableOpts::default()),
        note(&esc(crate::standards::STANDARDS.tmdg.standing), false)
    )
}

/// `report-scope`.
pub(crate) fn scope_section(c: &mut Ctx) -> String {
    let r = c.r;
    let s = &r.rs.scope;
    let s0 = scope(s);
    let cs = r
        .host
        .foreign_section("compliance-scope")
        .filter(|f| !f.html.trim().is_empty())
        .map(|f| super::build::merged_html(c, &f, "Proportionality drivers"))
        .unwrap_or_default();
    let own = js::text_or_empty(r.rs.text_of("scope"));
    let own = crate::jsnum::js_trim(&own).to_string();
    format!(
        "{}\n      {}{}",
        para(&match s0 {
            Some(x) => format!("Scope (NZTA RR 422 Table 6.1): <b>{}</b>, {}.", x.label, esc(x.text)),
            None => "<span class=\"warn\">The RR 422 scope (Simple, Moderate, Broad or Extensive) has not been chosen (Report tab).</span>".to_string(),
        }),
        cs,
        if !cs.is_empty() && own.is_empty() {
            String::new()
        } else {
            c.author("scope", "State why this depth of assessment is enough for the decision (RMA s88(2AA)\u{2013}(2AB), Sch 4 cl 2(3)(c); from 30 Oct 2026 s18B; FTAA s44): net peak trips, the largest % change on a link, crash flags and any state highway involvement.")
        }
    )
}

/// `report-limitations`.
pub(crate) fn limitations(c: &mut Ctx) -> String {
    format!(
        "{}{}{}{}\n      {}",
        para("Spare Construct is a sketch model. Junction results are HCM 6 / Akcelik-based and indicative, not SIDRA output."),
        disc("D1"),
        disc("D2"),
        disc("D13"),
        c.author("limitations", "Add any project-specific limitations (data gaps, untested periods, tools still to be run).")
    )
}

/// The report's executive summary bullets (`execBullets`).
fn exec_bullets(c: &mut Ctx) -> Vec<String> {
    let r = c.r;
    let mut out: Vec<String> = Vec::new();
    let rg: RegCtx = reg_ctx(r);
    let w = c.sel.with;
    if !rg.pathway.is_empty() {
        out.push(format!(
            "Pathway: {}{}. Regime: {}.",
            esc(&rg.label),
            if rg.date.is_empty() { String::new() } else { format!(", lodgement {}", esc(&rg.date)) },
            esc(&rg.regime)
        ));
    }
    if let Some(w) = w {
        let per: Vec<String> = r
            .rep_periods(w)
            .iter()
            .filter_map(|p| {
                let res = r.results[w].get(p)?;
                Some(format!("{} veh/h in the {} peak hour", r_veh(res.dev.trips), p))
            })
            .collect();
        if !per.is_empty() {
            out.push(format!(
                "The development generates about {} (vehicle trips, {}, {}).",
                per.join(" and "),
                r.year_of(w).map_or("null".to_string(), number_to_js_string),
                esc(r.sc(w).name())
            ));
        }
        let js_rows = junction_rows(r, &c.sel.clone(), w, None);
        let mut worst: Option<&super::shared::JRow> = None;
        let mut with_s: Vec<&super::shared::JRow> = js_rows.iter().filter(|x| x.s.is_some()).collect();
        crate::graph::js_sort_by(&mut with_s, |a, b| b.s.as_ref().map_or(0.0, |s| s.dos) - a.s.as_ref().map_or(0.0, |s| s.dos));
        if let Some(x) = with_s.first() {
            worst = Some(x);
        }
        let mut ids: Vec<String> = Vec::new();
        for x in &js_rows {
            if !ids.contains(&x.id) {
                ids.push(x.id.clone());
            }
        }
        let (nodes, junctions) = junction_count(r, &c.sel, &ids);
        if let Some(wr) = worst {
            let s = wr.s.as_ref().expect("a summary");
            out.push(format!(
                "{} assessed{}; the highest degree of saturation is {} in the {} peak (DoS {}, LoS {}). Indicative, not SIDRA output.",
                plural(junctions as f64, "junction", "junctions"),
                if junctions != nodes { format!(" ({})", plural(nodes as f64, "modelled node", "modelled nodes")) } else { String::new() },
                esc(&wr.name),
                wr.p,
                r_dos(s.dos),
                esc(&s.los_text)
            ));
        }
    }
    let ex = super::sections_assess::extent_data(c);
    if !ex.is_empty() {
        out.push(format!(
            "Extent of effects (AT ITA s4.2.1.1): {}.",
            ex.iter()
                .map(|e| format!(
                    "{} peak {} and {} over the 5%/10% thresholds",
                    e.p,
                    plural(e.x.rows.iter().filter(|r| r.included).count() as f64, "link direction", "link directions"),
                    plural(e.x.sites.len() as f64, "junction", "junctions")
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if let Some(cr1) = crash_years(r) {
        out.push(format!(
            "Crash history {}\u{2013}{}: {} in the downloaded CAS area, {} fatal or serious (DSI).",
            number_to_js_string(cr1.from),
            number_to_js_string(cr1.to),
            plural(cr1.total.n, "reported crash", "reported crashes"),
            number_to_js_string(cr1.total.f + cr1.total.s)
        ));
    }
    let conv_n = (0..r.project.scenarios.len())
        .flat_map(|i| r.rep_periods(i).into_iter().filter_map(move |p| r.results[i].get(p)))
        .filter(|res| !res.converged)
        .count();
    out.push(format!(
        "Model status: {}; {}.",
        c.periods
            .iter()
            .map(|p| format!("{} {}", p, esc(&crate::results::model_status(r.project, p).short)))
            .collect::<Vec<_>>()
            .join(", "),
        if conv_n > 0 {
            format!("<span class=\"warn\">{} did not converge</span>", plural(conv_n as f64, "run", "runs"))
        } else {
            "every stored run converged".to_string()
        }
    ));
    if let Some(st) = &r.rs.staging
        && js::truthy(st.get("text"))
    {
        out.push(esc(&js::text(st.get("text"))));
    }
    out
}

pub(crate) fn r_veh(v: f64) -> String {
    if v.is_finite() { fmt::int(v) } else { fmt::DASH.to_string() }
}

pub(crate) fn r_dos(v: f64) -> String {
    if v.is_finite() { fmt::num(v, 2) } else { fmt::DASH.to_string() }
}

/// `report-exec`.
pub(crate) fn exec(c: &mut Ctx) -> String {
    let bullets = exec_bullets(c);
    format!(
        "<ul class=\"bul\">{}</ul>{}",
        bullets.iter().map(|b| format!("<li>{b}</li>")).collect::<String>(),
        c.author("exec", "Summarise the pathway, the triggers, the key effects, the mitigation and its triggers, and whether the proposal aligns with the transport network and policy.")
    )
}

/// Keeps helper imports used by other front-matter sections linked.
#[allow(dead_code)]
fn _link(_: Head, _: &str) {
    let _ = (hn("x"), level('P'), rr422::ID, attrib::OSM, assessed_sites, j_sum, number_to_js_string(1.0));
}
