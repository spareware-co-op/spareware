//! Chapters 5 and 6: the mitigation register, staging triggers, conditions and monitoring, and the conclusion.

use super::ctx::{Ctx, Head, Row, Tbl, TableOpts, day_of, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_env::control_label;
use super::sections_front::int;
use super::shared::{junction_groups, junction_rows, kit_junction_name};
use super::{Rep, Sel};
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, number_to_js_string};
use crate::project::{EconSettings, Scenario, road_class_exact};
use crate::util::{esc, fmt};
use std::fmt::Write as _;

/// One change a scenario makes against another.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// "link" or "node".
    pub kind: &'static str,
    /// Link or node id.
    pub id: String,
    /// The junction control, for a node.
    pub control: Option<String>,
    /// What changes, in words.
    pub text: String,
}

fn mstr(m: &Maybe<String>) -> String {
    match m {
        Maybe::Missing => "undefined".to_string(),
        Maybe::Null => "null".to_string(),
        Maybe::Set(s) => s.clone(),
    }
}

/// `networkDiff(a, b)`: what a scenario changes against another: links added, removed or changed, and junction controls.
pub fn network_diff(a: Option<&Scenario>, b: Option<&Scenario>) -> Vec<Change> {
    let mut out = Vec::new();
    let (Some(a), Some(b)) = (a, b) else {
        return out;
    };
    for l in b.links.values() {
        match a.links.get(&l.id) {
            None => {
                let cls = l.cls.as_str().and_then(road_class_exact).map_or("road".to_string(), |c| c.label.to_lowercase());
                let name = l.name.as_str().filter(|n| !n.is_empty()).unwrap_or("unnamed road");
                out.push(Change {
                    kind: "link",
                    id: l.id.clone(),
                    control: None,
                    text: format!(
                        "New {}: {} ({} m, {}+{} lanes)",
                        cls,
                        name,
                        rfmt::q(b.link_len(l)),
                        l.lanes_ab.js_string(),
                        l.lanes_ba.js_string()
                    ),
                });
            }
            Some(o) => {
                let mut ch: Vec<String> = Vec::new();
                if o.lanes_ab != l.lanes_ab || o.lanes_ba != l.lanes_ba {
                    ch.push(format!(
                        "lanes {}+{} \u{2192} {}+{}",
                        o.lanes_ab.js_string(),
                        o.lanes_ba.js_string(),
                        l.lanes_ab.js_string(),
                        l.lanes_ba.js_string()
                    ));
                }
                if o.speed != l.speed {
                    ch.push(format!("speed {} \u{2192} {} km/h", o.speed.js_string(), l.speed.js_string()));
                }
                if o.status != l.status {
                    ch.push(format!("{} \u{2192} {}", mstr(&o.status), mstr(&l.status)));
                }
                if o.cls != l.cls {
                    ch.push(format!("class {} \u{2192} {}", mstr(&o.cls), mstr(&l.cls)));
                }
                if o.bus_lane != l.bus_lane {
                    ch.push(if l.bus_lane.truthy() { "bus lane added" } else { "bus lane removed" }.to_string());
                }
                if o.cycle != l.cycle {
                    ch.push(format!("cycle facility {} \u{2192} {}", mstr(&o.cycle), mstr(&l.cycle)));
                }
                if !ch.is_empty() {
                    out.push(Change {
                        kind: "link",
                        id: l.id.clone(),
                        control: None,
                        text: format!("{}: {}", l.name.as_str().filter(|n| !n.is_empty()).unwrap_or("Unnamed road"), ch.join(", ")),
                    });
                }
            }
        }
    }
    for l in a.links.values() {
        if !b.links.contains_key(&l.id) {
            out.push(Change {
                kind: "link",
                id: l.id.clone(),
                control: None,
                text: format!("Removed: {}", l.name.as_str().filter(|n| !n.is_empty()).unwrap_or("unnamed road")),
            });
        }
    }
    for n in b.nodes.values() {
        let Some(o) = a.nodes.get(&n.id) else {
            continue;
        };
        if o.control != n.control || (n.upgrade.truthy() && !o.upgrade.truthy()) || (n.cycle != o.cycle && n.control.is("signals")) {
            let ctl = n.control.as_str().unwrap_or("");
            let octl = o.control.as_str().unwrap_or("");
            out.push(Change {
                kind: "node",
                id: n.id.clone(),
                control: n.control.as_str().map(str::to_string),
                text: format!(
                    "{} at {}{}{}",
                    control_label(ctl),
                    kit_junction_name(b, &n.id),
                    if o.control == n.control { String::new() } else { format!(" (was {})", control_label(octl).to_lowercase()) },
                    if n.cycle.truthy() && n.control.is("signals") { format!(", cycle {} s", n.cycle.js_string()) } else { String::new() }
                ),
            });
        }
    }
    out
}

fn cell(o: &JsonValue, k: &str, d: &str) -> String {
    let t = js::text_or_empty(o.get(k));
    esc(&if t.is_empty() { d.to_string() } else { t })
}

/// The "Effect addressed" cell of a mitigation register row: for a junction, its worst period in the proposal, linked to its
/// results; otherwise a dash.
fn mitigation_effect(r: &Rep<'_>, chg: &Change, prop: Option<usize>, prop_id: &str, refs: &[(String, String)]) -> String {
    if chg.kind == "node"
        && let Some(pi) = prop
    {
        let mut hits: Vec<(&str, &crate::results::NodeRes)> = r
            .rep_periods(pi)
            .into_iter()
            .filter_map(|per| {
                r.results.get(pi).and_then(|res| res.get(per)).and_then(|res| res.nodes.get(&chg.id)).map(|nr| (per, nr))
            })
            .collect();
        js_sort_by(&mut hits, |x, y| y.1.dos - x.1.dos);
        if let Some((per, nr)) = hits.first() {
            let jr = refs.iter().find(|(id, _)| *id == chg.id).map(|(_, jref)| jref.as_str()).filter(|jref| !jref.is_empty()).unwrap_or("J");
            return format!(
                "<a href=\"#j-{}-{}\">{} {} DoS {}, LoS {}</a>",
                esc(prop_id),
                esc(&chg.id),
                esc(jr),
                per,
                rfmt::dos(nr.dos),
                esc(if nr.los_text.is_empty() { &nr.los } else { &nr.los_text })
            );
        }
    }
    fmt::DASH.to_string()
}

/// The estimated cost of a mitigation register row: the junction control's rate for a node, the lane-km rate for a link the
/// mitigation adds. `scs` is the (mitigation, proposal) pair of scenarios.
fn mitigation_cost(r: &Rep<'_>, econ: Option<&EconSettings>, chg: &Change, scs: (Option<usize>, Option<usize>)) -> Option<f64> {
    if chg.kind == "node" {
        return econ.and_then(|ec| ec.node.get()).and_then(|rates| rates.get(chg.control.as_deref().unwrap_or(""))).and_then(|val| match val {
            Maybe::Set(num) => Some(num.to_number()),
            _ => None,
        });
    }
    // The register has rows only when both the mitigation and the proposal are set, so this always finds them
    let (Some(mi), Some(pi)) = scs else {
        return None;
    };
    let msc = r.scenario(mi);
    let pr = r.scenario(pi);
    match msc.links.get(&chg.id) {
        Some(l) if l.status.is("proposed") && !pr.links.contains_key(&chg.id) => {
            let per_km = econ
                .and_then(|ec| ec.lane_km.get())
                .and_then(|rates| rates.get(l.cls.as_str().unwrap_or("")))
                .and_then(|val| match val {
                    Maybe::Set(num) => Some(num.to_number()),
                    _ => None,
                })
                .unwrap_or(3.0);
            Some(msc.link_len(l) / 1000.0 * (f64::from(l.lanes_ab.int32()) + f64::from(l.lanes_ba.int32())) * per_km)
        }
        _ => None,
    }
}

/// What each assessed option (with, mitigation) changes against the base: one list item each, at most 12 changes named.
fn mitigation_list(r: &Rep<'_>, sel: &Sel, base_w: Option<usize>) -> String {
    let mut list = String::new();
    for &si in [sel.with, sel.mitigation].iter().flatten() {
        let changes = network_diff(base_w.map(|bi| r.scenario(bi)), Some(r.scenario(si)));
        // Writing to a String cannot fail
        let _ = write!(
            list,
            "<li><b>{}</b>: {}.</li>",
            esc(r.sc(si).name()),
            if changes.is_empty() {
                "no network changes".to_string()
            } else {
                format!(
                    "{}{}",
                    changes.iter().take(12).map(|x| esc(&x.text)).collect::<Vec<_>>().join("; "),
                    if changes.len() > 12 { format!("; and {} more", changes.len().saturating_sub(12)) } else { String::new() }
                )
            }
        );
    }
    list
}

/// `report-mitigation`.
pub(crate) fn mitigation(c: &mut Ctx) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let (mit, prop) = (sel.mitigation, sel.with);
    let econ = r.st().econ.get();
    let diff = match (mit, prop) {
        (Some(mi), Some(pi)) => network_diff(Some(r.scenario(pi)), Some(r.scenario(mi))),
        _ => Vec::new(),
    };
    let prop_id = prop.map(|pi| r.scenario(pi).id.clone()).unwrap_or_default();
    let refs: Vec<(String, String)> = c.refs.iter().find(|(key, _)| *key == prop_id).map(|(_, v)| v.clone()).unwrap_or_default();
    let blank = JsonValue::object();
    let rows: Vec<Row> = diff
        .iter()
        .enumerate()
        .map(|(idx, chg)| {
            // Bound: a row count, far below usize::MAX
            let rf = format!("M{}", idx.saturating_add(1));
            let entry = r.rs.mitigation.get(&rf).filter(|v| v.truthy()).unwrap_or(&blank);
            let eff = mitigation_effect(r, chg, prop, &prop_id, &refs);
            let cost = mitigation_cost(r, econ, chg, (mit, prop));
            let trigger = {
                let own = js::text_or_empty(entry.get("trigger"));
                if own.is_empty() {
                    let staged = js::text_or_empty(r.rs.staging.as_ref().and_then(|stg| stg.get("text")));
                    if staged.is_empty() { fmt::DASH.to_string() } else { staged }
                } else {
                    own
                }
            };
            Row::new(vec![
                rf,
                esc(&chg.text),
                eff,
                cell(entry, "options", fmt::DASH),
                esc(&trigger),
                cell(entry, "responsible", "Applicant"),
                cost.map_or(fmt::DASH.to_string(), fmt::money),
                cell(entry, "mechanism", "Condition"),
                cell(entry, "rltp", fmt::DASH),
                cell(entry, "land", fmt::DASH),
            ])
        })
        .collect();
    let base_w = sel.without.or(sel.base);
    let list = mitigation_list(r, &sel, base_w);
    let head: Vec<Head> = vec![
        h("Ref"),
        h("Measure"),
        h("Effect addressed"),
        h("Options considered"),
        h("Trigger"),
        h("Responsible"),
        hn("Est. cost NZ$"),
        h("Mechanism"),
        h("In RLTP/LTP"),
        h("Land outside applicant control"),
    ];
    format!(
        "{}\n      <p class=\"lg\">What each option changes against {}:</p><ul class=\"bul\">{}</ul>{}",
        c.table(
            "Table M1 \u{b7} Mitigation register",
            Tbl::new(head, rows).empty(if mit.is_some() {
                "The mitigation scenario makes no network changes against the proposal."
            } else {
                "No mitigation scenario. Create one (Scenarios \u{203a} Create assessment set) or set it in the Report tab."
            }),
            &TableOpts::note("Seeded from the mitigation scenario's changes against the proposal. Costs are Spare Construct's indicative rates. The author completes the options, trigger, responsibility, mechanism and land columns in the Report tab.")
        ),
        base_w.map_or(String::new(), |b| esc(r.sc(b).name())),
        list,
        c.author("mitigation", "Explain the mitigation chosen, the options considered and why the measures are proportionate.")
    )
}

/// A row of Table M2: one build-out increment of the staging run, with the measure it triggers.
fn staging_row(st: &JsonValue, tr: Option<&JsonValue>, rw: &JsonValue) -> Row {
    let field = |key: &str| rw.get(key);
    let worst = field("worst").filter(|w| w.truthy());
    let is_trigger = tr.is_some_and(|trg| js::num(trg.get("pct")) == js::num(rw.get("pct")));
    Row::cls(
        vec![
            format!("{}%", int(field("pct"))),
            rfmt::num(js::num(field("dwellings")), 0),
            rfmt::num(js::num(field("gfa")), 0),
            worst.map_or(esc(fmt::DASH), |w| esc(&js::text(w.get("name")))),
            worst.map_or(fmt::DASH.to_string(), |w| rfmt::dos(js::num(w.get("dos")))),
            worst.map_or(fmt::DASH.to_string(), |w| esc(&js::text(w.get("los")))),
            if js::truthy(field("queue")) { "Yes" } else { "No" }.to_string(),
            if is_trigger {
                format!("<b>{}</b>", esc(&{
                    let measure = js::text_or_empty(st.get("measure"));
                    if measure.is_empty() { "Mitigation".to_string() } else { measure }
                }))
            } else {
                fmt::DASH.to_string()
            },
            if is_trigger {
                match st.get("resolved") {
                    None | Some(JsonValue::Null) => "Not tested".to_string(),
                    Some(v) if v.truthy() => "Yes".to_string(),
                    _ => "<b class=\"bad\">No</b>".to_string(),
                }
            } else {
                String::new()
            },
        ],
        if is_trigger { "miss" } else { "" },
    )
}

/// The sites that trigger the staging measure, with why and how, as a list (empty when none do).
fn staging_triggers(tr: Option<&JsonValue>) -> String {
    match tr {
        Some(trg) if !js::items(trg.get("sites")).is_empty() => {
            let mut items = String::new();
            for (i, id) in js::items(trg.get("sites")).iter().enumerate() {
                let name = js::text_or_empty(js::idx(trg.get("names"), i));
                let why = js::text_or_empty(js::idx(trg.get("why"), i));
                let how = js::idx(trg.get("how"), i).filter(|v| v.truthy());
                // Writing to a String cannot fail
                let _ = write!(
                    items,
                    "<li>{}: {}{}</li>",
                    esc(&if name.is_empty() { js::text(Some(id)) } else { name }),
                    esc(&why),
                    how.map_or(String::new(), |hw| format!(" ({})", esc(&hw.js_string())))
                );
            }
            format!("<ul class=\"bul\">{items}</ul>")
        }
        _ => String::new(),
    }
}

/// `report-staging`.
pub(crate) fn staging(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(st) = r.rs.staging.as_ref() else {
        return note("Run the staging test in the Report tab: it runs the proposal at 10% build-out steps and finds the first increment where an assessed junction fails.", true);
    };
    let tr = st.get("trigger").filter(|trg| trg.truthy());
    let rows: Vec<Row> = js::items(st.get("rows")).iter().map(|rw| staging_row(st, tr, rw)).collect();
    let trig = staging_triggers(tr);
    let exw = js::items(st.get("existingWhy"));
    let ex = if exw.is_empty() {
        String::new()
    } else {
        c.table(
            "Existing deficiencies at 0% build-out",
            Tbl::new(
                vec![h("Site"), h("Fails at 0% because")],
                exw.iter()
                    .map(|x| {
                        let why = js::text_or_empty(x.get("why"));
                        Row::new(vec![esc(&js::text(x.get("name"))), esc(&if why.is_empty() { fmt::DASH.to_string() } else { why })])
                    })
                    .collect(),
            ),
            &TableOpts {
                junction: true,
                note: Some("These sites already fail without the development. They still trigger a stage when they fail a further criterion, or when their DoS rises by 0.05 or more over the 0% value (Spare Construct assumption).".to_string()),
                d: vec!["D13"],
                ..TableOpts::default()
            },
        )
    };
    let periods: Vec<String> = js::items(st.get("periods")).iter().map(|p| js::text(Some(p))).collect();
    let text = js::text_or_empty(st.get("text"));
    format!(
        "{}\n      {}{}{}",
        c.table(
            &format!(
                "Table M2 \u{b7} Build-out increments ({}, {}, run {})",
                esc(&js::text(st.get("scName"))),
                esc(&periods.join(" and ")),
                esc(&day_of(st.get("at")))
            ),
            Tbl::new(
                vec![h("Build-out"), hn("Dwellings"), hn("GFA m\u{b2}"), h("Worst site"), hn("Worst DoS"), h("LoS"), h("Queue > storage"), h("Measure triggered"), h("Resolved")],
                rows,
            ),
            &TableOpts {
                junction: true,
                note: Some("A site fails when it exceeds its practical DoS, reaches LoS E or F, or its 95th-percentile queue exceeds the approach storage. Dwellings and GFA are those the model loads at each step: each zone's quantity \u{d7} its own build-out, for zones open by the scenario year.".to_string()),
                ..TableOpts::default()
            },
        ),
        para(&if text.is_empty() { "No assessed site fails at any increment.".to_string() } else { format!("<b>{}</b>", esc(&text)) }),
        trig,
        ex
    )
}

/// `report-conditions`.
pub(crate) fn conditions(c: &mut Ctx) -> String {
    let r = c.r;
    let text = r.rs.staging.as_ref().map(|s| js::text_or_empty(s.get("text"))).unwrap_or_default();
    format!(
        "{}\n      {}\n      {}",
        if text.is_empty() {
            String::new()
        } else {
            para(&format!("Staging condition from the staging test (section 5.2), for the author to confirm: <b>{}</b>", esc(&text)))
        },
        c.author("conditions", "Draft condition parameters (RMA s108, s108AA): for each measure in Table M1, the effect it addresses (its nexus), the trigger, who delivers it and how compliance is shown."),
        c.author("monitoring", "Monitoring and travel plan (RMA Sch 4 cl 6(1)(g); AUP E27.9(2)): what is monitored, when, the threshold, and what happens when it is crossed.")
    )
}

/// `conclusionText(P, c)`: the sentence that states whether the assessed junctions stay within their practical limits.
pub fn conclusion_text(c: &Ctx) -> String {
    let r = c.r;
    let Some(s) = c.sel.mitigation.or(c.sel.with) else {
        return "The assessed scenario has not been run, so no conclusion can be drawn from the model.".to_string();
    };
    if r.rep_periods(s).is_empty() {
        return "The assessed scenario has not been run, so no conclusion can be drawn from the model.".to_string();
    }
    let rows: Vec<_> = junction_rows(r, &c.sel, s, None).into_iter().filter(|x| x.s.is_some()).collect();
    let mut ids: Vec<String> = Vec::new();
    for x in &rows {
        if !ids.contains(&x.id) {
            ids.push(x.id.clone());
        }
    }
    if rows.is_empty() {
        return format!("{} has no assessed junctions.", r.sc(s).name());
    }
    let mut sorted: Vec<&super::shared::JRow> = rows.iter().collect();
    js_sort_by(&mut sorted, |a, b| b.s.as_ref().map_or(f64::NAN, |x| x.dos) - a.s.as_ref().map_or(f64::NAN, |x| x.dos));
    // rows is not empty and every row has a summary (the filter above), so the fallback never applies
    let Some((worst, ws)) = sorted.first().and_then(|w| w.s.as_ref().map(|sm| (*w, sm))) else {
        return format!("{} has no assessed junctions.", r.sc(s).name());
    };
    let over: Vec<&str> = rows
        .iter()
        .filter(|x| {
            x.s.as_ref().is_some_and(|sm| sm.dos > (if sm.xp == 0.0 || sm.xp.is_nan() { 1.0 } else { sm.xp }))
        })
        .map(|x| x.id.as_str())
        .collect();
    let who = if c.sel.mitigation.is_some() { "With the proposed mitigation" } else { "With the proposal" };
    let pers = r.rep_periods(s).join(" and ");
    let tail = format!("the worst is {} (DoS {}, LoS {}, {} peak)", worst.name, rfmt::dos(ws.dos), ws.los_text, worst.p);
    let base = c.sel.existing.or(c.sel.base).unwrap_or(0);
    let gs = junction_groups(r, base, s, &ids);
    let n_over = gs.iter().filter(|g| g.ids.iter().any(|id| over.contains(&id.as_str()))).count();
    let year = r.year_of(s).map_or("null".to_string(), number_to_js_string);
    if over.is_empty() {
        format!("{who}, all {} assessed junctions operate within practical DoS in the {year} {pers} peaks; {tail}.", gs.len())
    } else {
        format!(
            "{who}, {n_over} of {} assessed junctions exceed their practical degree of saturation in the {year} {pers} peaks; {tail}. Further mitigation or a staging condition is needed at those junctions.",
            gs.len()
        )
    }
}

/// `report-conclusion`.
pub(crate) fn conclusion(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        para(&format!("{} Junction results are indicative (HCM 6 / Akcelik-based), not SIDRA output.", esc(&conclusion_text(c)))),
        c.author("conclusion", "State whether the effects are avoided, remedied or mitigated, and whether the proposal aligns with the transport network for all modes (AT ITA s6).")
    )
}
