//! Chapters 5 and 6: the mitigation register, staging triggers, conditions and monitoring, and the conclusion.

use super::ctx::{Ctx, Head, Row, Tbl, TableOpts, day_of, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_env::control_label;
use super::sections_front::int;
use super::shared::{junction_groups, junction_rows, kit_junction_name};
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, number_to_js_string};
use crate::project::{Scenario, road_class_exact};
use crate::util::{esc, fmt};

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
                    if o.control != n.control { format!(" (was {})", control_label(octl).to_lowercase()) } else { String::new() },
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

/// `report-mitigation`.
pub(crate) fn mitigation(c: &mut Ctx) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let (mit, prop) = (sel.mitigation, sel.with);
    let econ = r.st().econ.get();
    let diff = match (mit, prop) {
        (Some(m), Some(p)) => network_diff(Some(r.scenario(p)), Some(r.scenario(m))),
        _ => Vec::new(),
    };
    let prop_id = prop.map(|p| r.scenario(p).id.clone()).unwrap_or_default();
    let refs: Vec<(String, String)> = c.refs.iter().find(|(k, _)| *k == prop_id).map(|(_, v)| v.clone()).unwrap_or_default();
    let blank = JsonValue::object();
    let rows: Vec<Row> = diff
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let rf = format!("M{}", i + 1);
            let a = r.rs.mitigation.get(&rf).filter(|v| v.truthy()).unwrap_or(&blank);
            let mut eff = fmt::DASH.to_string();
            if d.kind == "node"
                && let Some(p) = prop
            {
                let mut hits: Vec<(&str, &crate::results::NodeRes)> = r
                    .rep_periods(p)
                    .into_iter()
                    .filter_map(|per| r.results[p].get(per).and_then(|res| res.nodes.get(&d.id)).map(|nr| (per, nr)))
                    .collect();
                js_sort_by(&mut hits, |x, y| y.1.dos - x.1.dos);
                if let Some((per, nr)) = hits.first() {
                    let jr = refs.iter().find(|(k, _)| *k == d.id).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty()).unwrap_or("J");
                    eff = format!(
                        "<a href=\"#j-{}-{}\">{} {} DoS {}, LoS {}</a>",
                        esc(&prop_id),
                        esc(&d.id),
                        esc(jr),
                        per,
                        rfmt::dos(nr.dos),
                        esc(if nr.los_text.is_empty() { &nr.los } else { &nr.los_text })
                    );
                }
            }
            let cost: Option<f64> = if d.kind == "node" {
                econ.and_then(|e| e.node.get()).and_then(|m| m.get(d.control.as_deref().unwrap_or(""))).and_then(|v| match v {
                    Maybe::Set(x) => Some(x.to_number()),
                    _ => None,
                })
            } else {
                let m = r.scenario(mit.expect("a mitigation scenario"));
                let pr = r.scenario(prop.expect("a proposal scenario"));
                match m.links.get(&d.id) {
                    Some(l) if l.status.is("proposed") && !pr.links.contains_key(&d.id) => {
                        let per_km = econ
                            .and_then(|e| e.lane_km.get())
                            .and_then(|m| m.get(l.cls.as_str().unwrap_or("")))
                            .and_then(|v| match v {
                                Maybe::Set(x) => Some(x.to_number()),
                                _ => None,
                            })
                            .unwrap_or(3.0);
                        Some(m.link_len(l) / 1000.0 * ((l.lanes_ab.int32() | 0) as f64 + (l.lanes_ba.int32() | 0) as f64) * per_km)
                    }
                    _ => None,
                }
            };
            let trigger = {
                let t = js::text_or_empty(a.get("trigger"));
                if !t.is_empty() {
                    t
                } else {
                    let st = js::text_or_empty(r.rs.staging.as_ref().and_then(|s| s.get("text")));
                    if st.is_empty() { fmt::DASH.to_string() } else { st }
                }
            };
            Row::new(vec![
                rf,
                esc(&d.text),
                eff,
                cell(a, "options", fmt::DASH),
                esc(&trigger),
                cell(a, "responsible", "Applicant"),
                cost.map_or(fmt::DASH.to_string(), fmt::money),
                cell(a, "mechanism", "Condition"),
                cell(a, "rltp", fmt::DASH),
                cell(a, "land", fmt::DASH),
            ])
        })
        .collect();
    let base_w = sel.without.or(sel.base);
    let list: String = [sel.with, sel.mitigation]
        .iter()
        .flatten()
        .map(|&s| {
            let d = network_diff(base_w.map(|b| r.scenario(b)), Some(r.scenario(s)));
            format!(
                "<li><b>{}</b>: {}.</li>",
                esc(r.sc(s).name()),
                if d.is_empty() {
                    "no network changes".to_string()
                } else {
                    format!(
                        "{}{}",
                        d.iter().take(12).map(|x| esc(&x.text)).collect::<Vec<_>>().join("; "),
                        if d.len() > 12 { format!("; and {} more", d.len() - 12) } else { String::new() }
                    )
                }
            )
        })
        .collect();
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

/// `report-staging`.
pub(crate) fn staging(c: &mut Ctx) -> String {
    let r = c.r;
    let Some(st) = r.rs.staging.as_ref() else {
        return note("Run the staging test in the Report tab: it runs the proposal at 10% build-out steps and finds the first increment where an assessed junction fails.", true);
    };
    let tr = st.get("trigger").filter(|t| t.truthy());
    let is_t = |row: &JsonValue| tr.is_some_and(|t| js::num(t.get("pct")) == js::num(row.get("pct")));
    let rows: Vec<Row> = js::items(st.get("rows"))
        .iter()
        .map(|rw| {
            let g = |k: &str| rw.get(k);
            let worst = g("worst").filter(|w| w.truthy());
            let t = is_t(rw);
            Row::cls(
                vec![
                    format!("{}%", int(g("pct"))),
                    rfmt::num(js::num(g("dwellings")), 0),
                    rfmt::num(js::num(g("gfa")), 0),
                    worst.map_or(esc(fmt::DASH), |w| esc(&js::text(w.get("name")))),
                    worst.map_or(fmt::DASH.to_string(), |w| rfmt::dos(js::num(w.get("dos")))),
                    worst.map_or(fmt::DASH.to_string(), |w| esc(&js::text(w.get("los")))),
                    if js::truthy(g("queue")) { "Yes" } else { "No" }.to_string(),
                    if t {
                        format!("<b>{}</b>", esc(&{
                            let m = js::text_or_empty(st.get("measure"));
                            if m.is_empty() { "Mitigation".to_string() } else { m }
                        }))
                    } else {
                        fmt::DASH.to_string()
                    },
                    if t {
                        match st.get("resolved") {
                            None | Some(JsonValue::Null) => "Not tested".to_string(),
                            Some(v) if v.truthy() => "Yes".to_string(),
                            _ => "<b class=\"bad\">No</b>".to_string(),
                        }
                    } else {
                        String::new()
                    },
                ],
                if t { "miss" } else { "" },
            )
        })
        .collect();
    let trig = match tr {
        Some(t) if !js::items(t.get("sites")).is_empty() => {
            let items: String = js::items(t.get("sites"))
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let name = js::text_or_empty(js::idx(t.get("names"), i));
                    let why = js::text_or_empty(js::idx(t.get("why"), i));
                    let how = js::idx(t.get("how"), i).filter(|v| v.truthy());
                    format!(
                        "<li>{}: {}{}</li>",
                        esc(&if name.is_empty() { js::text(Some(id)) } else { name }),
                        esc(&why),
                        how.map_or(String::new(), |hw| format!(" ({})", esc(&hw.js_string())))
                    )
                })
                .collect();
            format!("<ul class=\"bul\">{items}</ul>")
        }
        _ => String::new(),
    };
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
        para(&if !text.is_empty() { format!("<b>{}</b>", esc(&text)) } else { "No assessed site fails at any increment.".to_string() }),
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
    let worst = sorted[0];
    let ws = worst.s.as_ref().expect("a summary");
    let over: Vec<&str> = rows
        .iter()
        .filter(|x| {
            let sm = x.s.as_ref().expect("a summary");
            sm.dos > (if sm.xp == 0.0 || sm.xp.is_nan() { 1.0 } else { sm.xp })
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
    if !over.is_empty() {
        format!(
            "{who}, {n_over} of {} assessed junctions exceed their practical degree of saturation in the {year} {pers} peaks; {tail}. Further mitigation or a staging condition is needed at those junctions.",
            gs.len()
        )
    } else {
        format!("{who}, all {} assessed junctions operate within practical DoS in the {year} {pers} peaks; {tail}.", gs.len())
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
