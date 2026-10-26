//! The appendices: data sources, model settings, trip rates, zones, junction controls, the standards register, calibration,
//! convergence, counts, movement summaries, the crash listing, ANOP inputs, the data pack manifest and the expert sign-off.

use super::app_support::{
    NonDefault, Series, assumption_rows, data_sources, default_settings_json, kit_line, non_defaults, settings_rows, val_text,
};
use super::consts::{PERIODS, attrib, disclaimer_ids, reg};
use super::ctx::{Ctx, Head, Row, Tbl, TableOpts, day_of, disc, disc_with, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_end::network_diff;
use super::sections_env::control_label;
use super::sections_front::{int, int_f, year_txt};
use super::shared::{assessed_sites, j_sum, junction_rows, kit_junction_name, plural};
use super::texts::{COLS, FILES};
use crate::edit::json::JsonValue;
use crate::edit::net::links_at;
use crate::graph::{js_sort_by, zone_class};
use crate::jsnum::{Maybe, math_max_of, number_to_js_string};
use crate::project::{LANDUSES, land_use};
use crate::standards::{STANDARDS, growth_basis};
use crate::util::{esc, fmt};

fn loose(n: &crate::jsnum::Num) -> f64 {
    match n {
        Maybe::Set(v) => v.to_number(),
        _ => f64::NAN,
    }
}

/// `report-app-data`.
pub(crate) fn app_data(c: &mut Ctx) -> String {
    let r = c.r;
    let rows: Vec<Row> = data_sources(r)
        .iter()
        .map(|d| {
            Row::new(vec![
                esc(d.what),
                if d.url.is_empty() { esc(d.source) } else { format!("<a href=\"{}\">{}</a>", esc(d.url), esc(d.source)) },
                esc(if d.licence.is_empty() { fmt::DASH } else { d.licence }),
                esc(d.date.as_deref().unwrap_or("Not loaded")),
                esc(if d.basis.is_empty() { fmt::DASH } else { d.basis }),
                esc(&d.notes),
            ])
        })
        .collect();
    // each feature's own rows from Settings > About the data: the junction tools', this report's and the host's
    struct Entry {
        title: String,
        order: f64,
        rows: Vec<[String; 3]>,
    }
    let mut entries: Vec<Entry> = vec![
        Entry { title: "Junction analysis".to_string(), order: 30.0, rows: crate::junction_tools::about_rows() },
        Entry {
            title: "ITA report and data pack".to_string(),
            order: 90.0,
            rows: vec![
                [
                    "Report structure".to_string(),
                    "AT ITA Guidelines 2025 v2; NZTA RR 422 Table 6.3".to_string(),
                    "<b>Rules as data.</b> RR 422 rows from Spare Construct's research spec (verified: no).".to_string(),
                ],
                [
                    "Crash risk".to_string(),
                    "NZTA CEC (May 2026) severity indices; HRIG collective and personal risk".to_string(),
                    "<b>Indicative.</b> Coefficients not yet checked against the source.".to_string(),
                ],
                [
                    "Disclaimers".to_string(),
                    "docs/regulatory/_disclaimers.md".to_string(),
                    format!("Version {}; printed where the placement rules require.", esc(super::texts::DISCLAIMERS_VERSION)),
                ],
            ],
        },
    ];
    for a in r.host.about_entries() {
        entries.push(Entry { title: a.title, order: a.order, rows: a.rows });
    }
    let mut idx: Vec<usize> = (0..entries.len()).collect();
    idx.sort_by(|a, b| entries[*a].order.partial_cmp(&entries[*b].order).unwrap_or(std::cmp::Ordering::Equal));
    let feat: Vec<Row> = idx
        .iter()
        .flat_map(|&i| {
            let a = &entries[i];
            a.rows.iter().map(|x| Row::new(vec![esc(&a.title), x[0].clone(), x[1].clone(), x[2].clone()])).collect::<Vec<_>>()
        })
        .collect();
    format!(
        "{}\n      {}",
        c.table(
            "Data sources, licences and basis",
            Tbl::new(vec![h("Data"), h("Source"), h("Licence"), h("Loaded"), h("Basis"), h("Notes")], rows).empty("No data sources recorded."),
            &TableOpts {
                source: Some("Spare Construct data-source register (Kit.dataSources); query dates from this project's download log".to_string()),
                ..TableOpts::default()
            }
        ),
        if feat.is_empty() {
            String::new()
        } else {
            c.table(
                "Feature data sources",
                Tbl::new(vec![h("Feature"), h("Data"), h("Source"), h("Notes")], feat),
                &TableOpts { source: Some("Settings \u{203a} About the data (each feature's own register)".to_string()), ..TableOpts::default() },
            )
        }
    )
}

/// `report-app-settings`.
pub(crate) fn app_settings(c: &mut Ctx) -> String {
    let r = c.r;
    let nd: Vec<NonDefault> = non_defaults(r);
    let nd_rows: Vec<Row> = nd
        .iter()
        .map(|a| {
            Row::new(vec![
                esc(&a.label),
                format!("<code>{}</code>", esc(&a.key)),
                esc(&val_text(a.value.as_ref())),
                esc(&val_text(a.def.as_ref())),
                esc(&a.unit),
            ])
        })
        .collect();
    let rows: Vec<Row> = settings_rows(r)
        .iter()
        .map(|x| {
            Row::cls(
                vec![esc(&x.label), format!("<code>{}</code>", esc(&x.key)), esc(&x.value), esc(&x.def), if x.changed { "<b>Changed</b>" } else { "" }.to_string()],
                if x.changed { "chg" } else { "" },
            )
        })
        .collect();
    format!(
        "{}\n      {}\n      {}",
        para("These are every model setting used for the runs in this report, so a second user can reproduce them from the project file (project.corridor.json in the data pack). Values that differ from Spare Construct's current defaults are listed first, then every setting with the changed ones marked."),
        c.table(
            "Settings changed from Spare Construct's defaults (project assumptions)",
            Tbl::new(vec![h("Setting"), h("Key"), h("Value"), h("Default"), h("Unit")], nd_rows)
                .empty("Every setting is at Spare Construct's default. The defaults are indicative: state why they suit this project or replace them."),
            &TableOpts::note("Each change is a project assumption to agree with the reviewer at scoping. The same list is the \"Assumptions\" CSV export.")
        ),
        c.table("Model settings", Tbl::new(vec![h("Setting"), h("Key"), h("Value"), h("Default"), h("Non-default")], rows), &TableOpts::default())
    )
}

/// `report-app-rates`.
pub(crate) fn app_rates(c: &mut Ctx) -> String {
    let r = c.r;
    let s_rates = js::path(Some(r.raw), &["settings", "rates"]);
    let defaults = default_settings_json();
    let d_rates = defaults.get("rates");
    let empty = JsonValue::object();
    let rows: Vec<Row> = LANDUSES
        .iter()
        .map(|l| {
            let rr = js::get(s_rates, l.key).filter(|v| v.truthy()).unwrap_or(&empty);
            let dd = js::get(d_rates, l.key).filter(|v| v.truthy()).unwrap_or(&empty);
            let same = rr.stringify() == dd.stringify();
            let p = |per: &str, f: &str| -> Option<&JsonValue> { js::get(Some(rr), per).filter(|v| v.truthy()).and_then(|v| v.get(f)) };
            let am = js::get(Some(rr), "AM").filter(|v| v.truthy());
            let pm = js::get(Some(rr), "PM").filter(|v| v.truthy());
            Row::new(vec![
                esc(l.label),
                format!("per {}{}", if l.per == 1.0 { String::new() } else { format!("{} ", number_to_js_string(l.per)) }, esc(l.unit.strip_suffix('s').unwrap_or(l.unit))),
                if am.is_some() { rfmt::num(rfmt::v(p("AM", "rate")), 2) } else { fmt::DASH.to_string() },
                if am.is_some() { rfmt::pct(rfmt::v(p("AM", "inPct"))) } else { fmt::DASH.to_string() },
                if pm.is_some() { rfmt::num(rfmt::v(p("PM", "rate")), 2) } else { fmt::DASH.to_string() },
                if pm.is_some() { rfmt::pct(rfmt::v(p("PM", "inPct"))) } else { fmt::DASH.to_string() },
                if same { "Spare Construct indicative default" } else { "<b>User-entered</b>" }.to_string(),
            ])
        })
        .collect();
    c.table(
        "Peak-hour vehicle trip rates",
        Tbl::new(vec![h("Land use"), h("Unit"), hn("AM rate"), hn("AM in %"), hn("PM rate"), hn("PM in %"), h("Source")], rows),
        &TableOpts {
            note: Some("Spare Construct's defaults are indicative NZ/RTA-style rates. Replace them with surveyed rates (RTA, TDB, RR 453) and state the source and data year for a real assessment.".to_string()),
            d: vec!["D13"],
            ..TableOpts::default()
        },
    )
}

fn maybe_text(v: &Maybe<serde_json::Value>, d: &str) -> String {
    match v {
        Maybe::Set(x) if crate::jsnum::value_truthy(x) => crate::jsnum::value_to_js_string(x),
        _ => d.to_string(),
    }
}

fn year_cell(n: &crate::jsnum::Num) -> String {
    let t = match n {
        Maybe::Missing | Maybe::Null => String::new(),
        other => other.js_string(),
    };
    year_txt(&t)
}

/// `report-app-zones`.
pub(crate) fn app_zones(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = c.sel.with.or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let name_or_id = |z: &crate::project::Zone| -> String {
        match z.name.as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => z.id.clone(),
        }
    };
    let zrows: Vec<Row> = scn
        .zones
        .values()
        .filter(|z| !z.kind.is("gateway"))
        .map(|z| {
            let lu: Vec<String> = z
                .lu
                .get()
                .map(|m| {
                    m.iter()
                        .filter(|(_, q)| q.num() > 0.0)
                        .map(|(k, q)| format!("{} {}", land_use(k).map_or(k.to_string(), |l| l.label.to_string()), rfmt::num(q.num(), 0)))
                        .collect()
                })
                .unwrap_or_default();
            Row::new(vec![
                esc(&name_or_id(z)),
                esc(zone_class(Some(z))),
                esc(&if lu.is_empty() { fmt::DASH.to_string() } else { lu.join(", ") }),
                year_cell(&z.year),
                if z.reduction.truthy() { rfmt::pct(z.reduction.num()) } else { fmt::DASH.to_string() },
                if !matches!(z.buildout, Maybe::Missing | Maybe::Null) { rfmt::pct(z.buildout.num()) } else { fmt::DASH.to_string() },
                esc(&maybe_text(&z.src, "user")),
            ])
        })
        .collect();
    let grows: Vec<Row> = scn
        .zones
        .values()
        .filter(|z| z.kind.is("gateway"))
        .map(|z| {
            let g = |k: &str| -> String {
                let v = z.gw.get().and_then(|m| m.get(k)).map_or(f64::NAN, loose);
                rfmt::veh(v)
            };
            Row::new(vec![
                esc(&name_or_id(z)),
                g("AMin"),
                g("AMout"),
                g("PMin"),
                g("PMout"),
                esc(&maybe_text(&z.gw_src, "Estimated by Spare Construct (no matched count)")),
            ])
        })
        .collect();
    format!(
        "{}\n      {}",
        c.table(
            &format!("Zone schedule \u{b7} {}", esc(r.sc(sc).name())),
            Tbl::new(
                vec![h("Zone"), h("Class"), h("Land use"), h("Opening year"), hn("Trip reduction"), hn("Build-out"), h("Source")],
                zrows
            ),
            &TableOpts::default()
        ),
        c.table(
            &format!("Gateway flows (base year {}, before growth)", year_cell(&r.st().base_year)),
            Tbl::new(vec![h("Gateway"), hn("AM in"), hn("AM out"), hn("PM in"), hn("PM out"), h("Count basis")], grows),
            &TableOpts::note(&format!("veh/h. Grown to each scenario's year by {}.", esc(&growth_basis(r.st()))))
        )
    )
}

/// `report-app-network`.
pub(crate) fn app_network(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = c.sel.mitigation.or(c.sel.with).or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let assessed = assessed_sites(r, Some(sc)).ids;
    let base = c.sel.base.unwrap_or(0);
    let nodes: Vec<Row> = scn
        .nodes
        .values()
        .filter(|n| {
            let ctl = n.control.as_str().unwrap_or("");
            (!ctl.is_empty() && ctl != "none" && ctl != "priority") || assessed.contains(&n.id)
        })
        .map(|n| {
            let lanes: String = {
                let l: Vec<String> = links_at(scn, &n.id)
                    .into_iter()
                    .filter(|l| !l.status.is("closed"))
                    .map(|l| {
                        let name = l.name.as_str().filter(|x| !x.is_empty()).unwrap_or("Unnamed road");
                        let v = if l.b == n.id { &l.lanes_ab } else { &l.lanes_ba };
                        format!("{} {}", esc(name), int(Some(&to_json(v))))
                    })
                    .collect();
                if l.is_empty() { fmt::DASH.to_string() } else { l.join(", ") }
            };
            let ctl = n.control.as_str().unwrap_or("");
            Row::new(vec![
                esc(&kit_junction_name(scn, &n.id)),
                esc(&if ctl.is_empty() { "Uncontrolled".to_string() } else { control_label(ctl) }),
                lanes,
                if n.cycle.truthy() { format!("{} s", int(Some(&to_json(&n.cycle)))) } else { fmt::DASH.to_string() },
                if n.fixed_timing.truthy() {
                    "Fixed".to_string()
                } else if n.control.is("signals") {
                    "Optimised".to_string()
                } else {
                    fmt::DASH.to_string()
                },
                match n.bans.get() {
                    Some(serde_json::Value::Array(a)) if !a.is_empty() => format!("{} banned", int_f(a.len() as f64)),
                    _ => fmt::DASH.to_string(),
                },
                if n.upgrade.truthy() { "Yes" } else { "" }.to_string(),
                if assessed.contains(&n.id) { "Yes" } else { "" }.to_string(),
            ])
        })
        .collect();
    let edits: Vec<Row> = (0..r.project.scenarios.len())
        .filter(|i| *i != base)
        .map(|i| {
            let d = network_diff(Some(r.scenario(base)), Some(r.scenario(i)));
            let t = format!(
                "{}{}",
                d.iter().take(8).map(|x| x.text.clone()).collect::<Vec<_>>().join("; "),
                if d.len() > 8 { format!("; and {} more", d.len() - 8) } else { String::new() }
            );
            Row::new(vec![esc(r.sc(i).name()), d.len().to_string(), esc(&if t.is_empty() { "None".to_string() } else { t })])
        })
        .collect();
    format!(
        "{}\n      {}",
        c.table(
            &format!("Junction controls, lanes and timings \u{b7} {}", esc(r.sc(sc).name())),
            Tbl::new(
                vec![h("Junction"), h("Control"), h("Approach lanes (arriving)"), h("Cycle"), h("Timing"), h("Turn bans"), h("Upgraded"), h("Assessed")],
                nodes
            )
            .empty("Only give-way and uncontrolled junctions, none of them assessed."),
            &TableOpts::note("Every junction with signals, a roundabout or stop control, and every assessed junction. Lanes are the lanes arriving at the junction on each leg. Other give-way junctions are set from OpenStreetMap and road hierarchy; every junction and its approaches are in junction_summary.csv and junction_movements.csv.")
        ),
        c.table(
            &format!("Network edits against {}", esc(r.sc(base).name())),
            Tbl::new(vec![h("Scenario"), hn("Changes"), h("What changes")], edits).empty("Only one scenario."),
            &TableOpts::default()
        )
    )
}

fn to_json(n: &crate::jsnum::Num) -> JsonValue {
    match n {
        Maybe::Missing => JsonValue::Null,
        Maybe::Null => JsonValue::Null,
        Maybe::Set(v) => JsonValue::from_value(&v.to_value()),
    }
}

/// `report-app-assumptions`.
pub(crate) fn app_assumptions(c: &mut Ctx) -> String {
    let rows: Vec<Row> = assumption_rows()
        .iter()
        .map(|a| {
            Row::new(vec![
                esc(&a.name),
                esc(&a.edition),
                if a.corridor { "<b>Spare Construct assumption</b>" } else { "Published source" }.to_string(),
                if a.verified { "Yes" } else { "No" }.to_string(),
            ])
        })
        .collect();
    c.table(
        "Standards and assumptions register",
        Tbl::new(vec![h("Item"), h("Source and edition"), h("Basis"), h("Checked against source")], rows),
        &TableOpts {
            note: Some("Every threshold Spare Construct uses is held as data with its source and edition. \"Checked against source: No\" means it was taken from Spare Construct's research spec and must be checked before it is relied on.".to_string()),
            d: vec!["D13"],
            ..TableOpts::default()
        },
    )
}

fn vtext(k: &serde_json::Value, f: &str) -> String {
    match k.get(f) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(v) => crate::jsnum::value_to_js_string(v),
    }
}

/// `report-app-calibration`.
pub(crate) fn app_calibration(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let mut parts: Vec<String> = Vec::new();
    for p in ["AM", "PM"] {
        let Some(res) = r.results[sc].get(p) else {
            continue;
        };
        let rep = crate::calibrate::report(r.project, scn, p, Some(res));
        let rows: Vec<Row> = rep
            .checks
            .iter()
            .map(|k| {
                let k = k.to_value();
                let or_dash = |s: String| if s.is_empty() { fmt::DASH.to_string() } else { s };
                Row::new(vec![
                    esc(&vtext(&k, "label")),
                    esc(&or_dash(vtext(&k, "target"))),
                    match k.get("value") {
                        None | Some(serde_json::Value::Null) => fmt::DASH.to_string(),
                        Some(serde_json::Value::Number(n)) => {
                            let v = n.as_f64().unwrap_or(f64::NAN);
                            if k.get("kind").and_then(serde_json::Value::as_str) == Some("share") { rfmt::pct(v * 100.0) } else { rfmt::num(v, 3) }
                        }
                        Some(serde_json::Value::String(s)) if matches!(s.as_str(), "NaN" | "Infinity" | "-Infinity") => {
                            let v = match s.as_str() {
                                "NaN" => f64::NAN,
                                "Infinity" => f64::INFINITY,
                                _ => f64::NEG_INFINITY,
                            };
                            if k.get("kind").and_then(serde_json::Value::as_str) == Some("share") { rfmt::pct(v * 100.0) } else { rfmt::num(v, 3) }
                        }
                        Some(v) => esc(&crate::jsnum::value_to_js_string(v)),
                    },
                    esc(&or_dash(vtext(&k, "status"))),
                    esc(&vtext(&k, "source")),
                    esc(&vtext(&k, "table")),
                    if k.get("verified").is_some_and(crate::jsnum::value_truthy) { "Yes" } else { "No" }.to_string(),
                ])
            })
            .collect();
        parts.push(c.table(
            &format!("Calibration checks \u{b7} {} \u{b7} {} peak", esc(r.sc(sc).name()), p),
            Tbl::new(vec![h("Criterion"), h("Target"), h("Value"), h("Status"), h("Source"), h("Table"), h("Verified")], rows),
            &TableOpts::note(&format!(
                "{} {} {}",
                esc(&rep.summary),
                rep.warnings.iter().map(|x| esc(x)).collect::<Vec<_>>().join(" "),
                rep.notes.iter().map(|x| esc(x)).collect::<Vec<_>>().join(" ")
            )),
        ));
        let info = js::path(Some(r.raw), &["me", "info", p]).filter(|v| v.truthy());
        if let Some(info) = info {
            let mc = js::get(Some(info), "matrixChange").filter(|v| v.truthy());
            let mct = mc.and_then(|m| m.get("total")).filter(|t| !t.is_null());
            let r2 = info.get("tripEndR2").filter(|t| !t.is_null());
            let stopped = js::text_or_empty(info.get("stopped"));
            parts.push(c.table(
                &format!("Matrix estimation \u{b7} {p} peak"),
                Tbl::new(
                    vec![h("Item"), h("Value")],
                    vec![
                        Row::new(vec!["Date".to_string(), esc(&day_of(info.get("date")))]),
                        Row::new(vec!["Iterations".to_string(), esc(&js::text(info.get("iters")))]),
                        Row::new(vec!["Matrix total change".to_string(), mct.map_or(fmt::DASH.to_string(), |t| rfmt::pct(t.to_number() * 100.0))]),
                        Row::new(vec!["Trip-end R\u{b2}".to_string(), r2.map_or(fmt::DASH.to_string(), |t| rfmt::num(t.to_number(), 3))]),
                        Row::new(vec!["Stopped because".to_string(), esc(&if stopped.is_empty() { fmt::DASH.to_string() } else { stopped })]),
                    ],
                ),
                &TableOpts::note("Matrix estimation used the same counts as calibration, so they are not independent validation. Agree its use with the peer reviewer."),
            ));
        }
    }
    let body = parts.join("");
    format!(
        "{}{}",
        if body.is_empty() { note("Run the existing (base-year) scenario with counts loaded to report calibration.", false) } else { body },
        disc("D6")
    )
}

/// `report-app-convergence`.
pub(crate) fn app_convergence(c: &mut Ctx) -> String {
    let r = c.r;
    let mut rows: Vec<Row> = Vec::new();
    let mut series: Vec<Series> = Vec::new();
    for i in 0..r.project.scenarios.len() {
        for p in r.rep_periods(i) {
            let res = r.results[i].get(p).expect("a period with results");
            let last: Vec<f64> = res.conv.iter().rev().take(4).rev().map(|x| x.p.unwrap_or(f64::NAN)).collect();
            let pm = if last.is_empty() { None } else { Some(crate::jsnum::math_min_of(&last)) };
            rows.push(Row::new(vec![
                esc(r.sc(i).name()),
                p.to_string(),
                int(Some(&JsonValue::Number(res.it))),
                format!("{}%", fmt::num(res.gap * 100.0, 4)),
                pm.map_or(fmt::DASH.to_string(), |v| rfmt::pct(v * 100.0)),
                if !res.converged { "<b class=\"bad\">No</b>" } else { "Yes" }.to_string(),
                esc(&res.conv_note),
            ]));
            if Some(i) == c.sel.with && res.conv.len() > 1 {
                series.push(Series {
                    pts: res.conv.iter().filter(|x| x.gap > 0.0).map(|x| [x.it, (-x.gap.log10()).min(6.0)]).collect(),
                    colour: if p == "AM" { "var(--accent)" } else { "var(--amber)" },
                    label: format!("{p} \u{2212}log\u{2081}\u{2080}(gap)"),
                    y_max: 6.0,
                });
            }
        }
    }
    let xs: Vec<f64> = series.iter().flat_map(|s| s.pts.iter().map(|q| q[0])).collect();
    let xm = math_max_of(&[vec![1.0], xs].concat());
    let conv = &STANDARDS.conv;
    format!(
        "{}\n      {}",
        c.table(
            "Table C1 \u{b7} Convergence",
            Tbl::new(
                vec![h("Scenario"), h("Period"), hn("Iterations"), hn("Final %GAP"), hn("P, last 4"), h("Converged"), h("Note")],
                rows
            )
            .empty("No stored runs."),
            &TableOpts { source: Some(esc(conv.src.edition)), note: Some(esc(conv.note)), ..TableOpts::default() }
        ),
        if series.is_empty() {
            String::new()
        } else {
            format!(
                "<figure class=\"chart\">{}<figcaption>Gap trace, {} (higher is better converged)</figcaption></figure>",
                kit_line(&series, xm, 6.0, "10\u{207b}\u{2076}", ["iteration 1", &format!("iteration {}", number_to_js_string(xm))]),
                esc(r.sc(c.sel.with.unwrap_or(0)).name())
            )
        }
    )
}

/// `report-app-counts`.
pub(crate) fn app_counts(c: &mut Ctx) -> String {
    let r = c.r;
    let counts = js::items(js::get(r.data(), "counts"));
    let mut by: Vec<(String, usize)> = Vec::new();
    for k in counts {
        let src = js::get(Some(k), "src");
        let role = js::text_or_empty(js::get(Some(k), "role"));
        let key = format!(
            "{}|{}|{}",
            match src {
                None => "undefined".to_string(),
                Some(v) => v.js_string(),
            },
            if js::truthy(js::get(Some(k), "virtual")) { "estimate" } else { "count" },
            if role.is_empty() { "calibration".to_string() } else { role }
        );
        match by.iter_mut().find(|(x, _)| *x == key) {
            Some(e) => e.1 += 1,
            None => by.push((key, 1)),
        }
    }
    let rows: Vec<Row> = by
        .iter()
        .map(|(k, n)| {
            let mut it = k.split('|');
            let (s, m, rl) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
            Row::new(vec![
                esc(crate::datahub::source(s).map_or(s, |x| x.label)),
                esc(m),
                esc(rl),
                n.to_string(),
            ])
        })
        .collect();
    let sc = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let scn = r.scenario(sc);
    let mut jt: Vec<Row> = Vec::new();
    for j in js::items(js::get(r.data(), "jt")) {
        for p in ["AM", "PM"] {
            let Some(res) = r.results[sc].get(p) else {
                continue;
            };
            let obs_t = js::get(Some(j), "obs").filter(|v| v.truthy()).map(|o| js::num(o.get(p))).filter(|t| *t > 0.0);
            let ob: Option<(f64, String)> = obs_t.map(|t| {
                let src = js::text_or_empty(js::path(Some(j), &["obs", "src"]));
                (t, if src.is_empty() { "Observed".to_string() } else { src })
            });
            let g = js::path(Some(j), &["g", p]).filter(|v| v.truthy() && js::num(v.get("t")) > 0.0);
            if ob.is_none() && g.is_none() {
                continue;
            }
            let from = js::text(js::get(Some(j), "from"));
            let to = js::text(js::get(Some(j), "to"));
            let mut mt: Option<f64> = None;
            let mut ml: Option<f64> = None;
            if scn.nodes.contains_key(&from) && scn.nodes.contains_key(&to)
                && let Some(crate::results::Travel::Route(t)) = crate::results::travel(scn, r.st(), Some(res), &from, &to)
            {
                mt = Some(t.time);
                ml = Some(t.len);
            }
            let reference = ob.as_ref().map_or_else(|| js::num(g.and_then(|x| x.get("t"))), |o| o.0);
            let k = mt.and_then(|m| crate::datahub::jt_check(m, reference));
            let glen = g.and_then(|x| x.get("len")).filter(|v| v.truthy()).map(JsonValue::to_number);
            let len = glen.or(ml);
            jt.push(Row::new(vec![
                esc(&js::text(js::get(Some(j), "name"))),
                p.to_string(),
                match len {
                    Some(l) if l != 0.0 && !l.is_nan() => rfmt::num(l / 1000.0, 2),
                    _ => fmt::DASH.to_string(),
                },
                rfmt::num(reference, 0),
                match &ob {
                    Some(o) => format!("{} (observed)", esc(&o.1)),
                    None => {
                        let when = js::text_or_empty(g.and_then(|x| x.get("when")));
                        format!(
                            "Google Routes typical time (predicted, not observed{})",
                            if when.is_empty() { String::new() } else { format!(", {}", esc(&when)) }
                        )
                    }
                },
                mt.map_or(fmt::DASH.to_string(), |m| rfmt::num(m, 0)),
                k.as_ref().map_or(fmt::DASH.to_string(), |k| rfmt::pct(k.pct * 100.0)),
                k.as_ref().map_or(fmt::DASH.to_string(), |k| if k.tier1 { "Pass" } else { "Fail" }.to_string()),
                k.as_ref().map_or(fmt::DASH.to_string(), |k| if k.tier2 { "Pass" } else { "Fail" }.to_string()),
            ]));
        }
    }
    format!(
        "{}\n      {}",
        c.table(
            "Counts by source, method and role",
            Tbl::new(vec![h("Source"), h("Method"), h("Role"), hn("Sites")], rows).empty("No counts loaded."),
            &TableOpts::note("Every count with its calibration result is in counts_calibration.csv in the data pack.")
        ),
        c.table(
            "Table J1 \u{b7} Journey times",
            Tbl::new(
                vec![h("Route"), h("Period"), hn("Length km"), hn("Reference s"), h("Reference source"), hn("Modelled s"), hn("\u{394} %"), h("Tier 1"), h("Tier 2")],
                jt
            )
            .empty("No journey-time routes with an observed or Google reference time."),
            &TableOpts::note(&format!(
                "{} Google typical times are predicted, not observed: they support but do not replace surveyed journey times.",
                esc(crate::datahub::JT_CRITERIA.text)
            ))
        )
    )
}

/// `report-app-movements`.
pub(crate) fn app_movements(c: &mut Ctx) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let mut parts: Vec<String> = Vec::new();
    for sc in [sel.without, sel.with, sel.mitigation].into_iter().flatten() {
        let scn = r.scenario(sc);
        let mut rows: Vec<Row> = Vec::new();
        for x in junction_rows(r, &sel, sc, None) {
            let Some(nr) = x.nr else {
                continue;
            };
            rows.push(Row::cls(
                vec![
                    format!("<b>{}</b>", esc(&x.name)),
                    x.p.to_string(),
                    format!("<b>{}</b>", esc(&control_label(&nr.r#type))),
                    rfmt::veh(nr.vol),
                    String::new(),
                    format!("<b>{}</b>", rfmt::dos(nr.dos)),
                    format!("<b>{}</b>", rfmt::delay(j_sum(nr).delay)),
                    format!("<b>{}</b>", esc(if nr.los_text.is_empty() { &nr.los } else { &nr.los_text })),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ],
                "tot",
            ));
            for a in &nr.appr {
                let st = scn.links.get(&a.link).map(|l| scn.link_len(l));
                rows.push(Row::new(vec![
                    String::new(),
                    String::new(),
                    esc(&format!("{}: {}", a.approach, a.name)),
                    rfmt::veh(a.v),
                    rfmt::veh(a.cap),
                    rfmt::dos(a.dos),
                    rfmt::delay(a.delay),
                    esc(&if a.los.is_empty() { fmt::DASH.to_string() } else { a.los.clone() }),
                    if a.q95veh.is_nan() { fmt::DASH.to_string() } else { rfmt::num(a.q95veh, 1) },
                    rfmt::q(a.q95),
                    a.psc.map_or(fmt::DASH.to_string(), rfmt::pct),
                    match st {
                        Some(s) if s != 0.0 && !s.is_nan() && a.q95 != 0.0 && !a.q95.is_nan() => rfmt::num(a.q95 / s, 2),
                        _ => fmt::DASH.to_string(),
                    },
                ]));
            }
        }
        let head: Vec<Head> = vec![
            h("Junction"),
            h("Period"),
            h("Approach"),
            hn("Demand veh/h"),
            hn("Capacity veh/h"),
            hn("DoS"),
            hn("Delay s"),
            h("LoS"),
            hn("Q95 veh"),
            hn("Q95 m"),
            hn("PSC %"),
            hn("Storage ratio"),
        ];
        parts.push(c.table(
            &format!("Movement summary by approach \u{b7} {}", esc(r.sc(sc).name())),
            Tbl::new(head, rows).empty("No junction results."),
            &TableOpts {
                junction: true,
                note: Some("Approach rows for the assessed junctions; bold rows are the junction as a whole. SIDRA-style movement summaries by turn for the key junctions follow in this appendix. Turn-level movements (Mov ID, turn designation) for every junction are in turning_movements.csv and the SIDRA volumes file.".to_string()),
                ..TableOpts::default()
            },
        ));
    }
    let body = parts.join("");
    if body.is_empty() { note("No junction results.", false) } else { body }
}

/// `report-app-crashes`.
pub(crate) fn app_crashes(c: &mut Ctx) -> String {
    let r = c.r;
    let mut cr: Vec<&JsonValue> = js::items(js::get(r.data(), "crashes")).iter().collect();
    js_sort_by(&mut cr, |a, b| {
        let d = js::num(js::idx(Some(b), 3)) - js::num(js::idx(Some(a), 3));
        if d != 0.0 && !d.is_nan() {
            return d;
        }
        let pos = |k: &JsonValue| -> f64 {
            let s = match js::idx(Some(k), 2) {
                None => "undefined".to_string(),
                Some(v) => v.js_string(),
            };
            "FSMN".find(s.as_str()).map_or(-1.0, |i| i as f64)
        };
        pos(a) - pos(b)
    });
    let sev = |s: &str| match s {
        "F" => Some("Fatal"),
        "S" => Some("Serious"),
        "M" => Some("Minor"),
        "N" => Some("Non-injury"),
        _ => None,
    };
    let rows: Vec<Row> = cr
        .iter()
        .map(|k| {
            let g = |i: usize| js::idx(Some(k), i);
            let flags: String = {
                let f = js::text_or_empty(g(5));
                let names: Vec<&str> = f
                    .chars()
                    .map(|ch| match ch {
                        'p' => "pedestrian",
                        'c' => "cyclist",
                        'm' => "motorcyclist",
                        _ => "",
                    })
                    .collect();
                names.join(", ")
            };
            let sv = js::text(g(2));
            Row::new(vec![
                year_txt(&js::text(g(3))),
                sev(&sv).map_or(esc(&sv), str::to_string),
                esc(&js::text_or_empty(g(4))),
                int(g(6)),
                esc(&if flags.is_empty() { fmt::DASH.to_string() } else { flags }),
                format!("{}, {}", rfmt::num(rfmt::v(g(0)), 5), rfmt::num(rfmt::v(g(1)), 5)),
            ])
        })
        .collect();
    c.table(
        &format!("Reported crashes ({})", plural(cr.len() as f64, "crash", "crashes")),
        Tbl::new(vec![h("Year"), h("Severity"), h("Location"), hn("Speed limit"), h("Vulnerable users"), h("Lat, lng")], rows).empty("No crashes loaded."),
        &TableOpts { source: Some(esc(attrib::CAS)), d: vec!["D8"], ..TableOpts::default() },
    )
}

/// `report-app-anop`.
pub(crate) fn app_anop(_c: &mut Ctx) -> String {
    format!(
        "{}{}",
        note("The ANOP tool inputs (anop.csv) are produced by the ANOP feature when it is installed. None are recorded in this project.", false),
        disc("D5")
    )
}

/// `report-app-datapack`.
pub(crate) fn app_datapack(c: &mut Ctx) -> String {
    let rows: Vec<Row> = COLS
        .iter()
        .map(|(f, cols)| {
            Row::new(vec![
                format!("<code>{}</code>", esc(f)),
                esc(FILES.iter().find(|(n, _)| n == f).map_or("", |(_, t)| t)),
                cols.iter().map(|(k, u)| format!("<code>{}</code> {}", esc(k), esc(u))).collect::<Vec<_>>().join("; "),
            ])
        })
        .collect();
    format!(
        "{}\n      {}{}",
        para(&format!(
            "The ITA data pack (Report \u{203a} Download the data pack) holds every table of this report as UTF-8 CSV, with the network, settings and project file: <code>{}</code>.",
            esc(&c.r.pack_name())
        )),
        c.table("Data pack files and columns", Tbl::new(vec![h("File"), h("Contents"), h("Columns (units)")], rows), &TableOpts::default()),
        disc_with("D12", None, Some("SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook"))
    )
}

/// `report-app-expert`.
pub(crate) fn app_expert(c: &mut Ctx) -> String {
    let r = c.r;
    let so = &r.rs.signoff;
    let d = &r.rs.details;
    let cell = |o: &JsonValue, k: &str| esc(&js::text_or_empty(o.get(k)));
    format!(
        "{}{}\n      {}\n      {}\n      <h3>Standing disclaimers (version {})</h3>{}",
        disc("D2"),
        para("Assumptions are listed in Appendix A (standards and assumptions register), methods in section 4.2, uncertainty and sensitivity in section 4.6, and limitations in the front matter."),
        c.author("expert", "State your qualifications and experience, the data and assumptions relied on, the uncertainty in the results, and your confidence in them (Practice Note 2023 s9.3)."),
        c.table(
            "Sign-off",
            Tbl::new(
                vec![h("Role"), h("Name"), h("Date"), h("Signature")],
                vec![
                    Row::new(vec!["Author".to_string(), cell(d, "author"), cell(d, "date"), String::new()]),
                    Row::new(vec![
                        "Reviewer".to_string(),
                        cell(so, "reviewer"),
                        cell(so, "date"),
                        if so.get("reviewed").is_some_and(JsonValue::truthy) { "Reviewed (ticked in Spare Construct)" } else { "" }.to_string(),
                    ]),
                    Row::new(vec!["Approved".to_string(), cell(so, "approver"), String::new(), String::new()]),
                ]
            ),
            &TableOpts::default()
        ),
        esc(super::texts::DISCLAIMERS_VERSION),
        disclaimer_ids().into_iter().map(|id| disc_with(id, Some(reg::AS_AT), None)).collect::<String>()
    )
}

/// Keeps helpers other appendices share linked.
#[allow(dead_code)]
fn _link() {
    let _ = (PERIODS, i32::MAX);
}
