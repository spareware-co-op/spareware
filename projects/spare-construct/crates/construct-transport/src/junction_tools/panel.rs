//! The junction-site panel's calculations and text, the nearby junctions a node could merge with, and the junction analysis
//! rows of "About the data".

use super::summary::fly_of;
use super::text::{ctl_name, d1, dos_cell, los_badge, n0, per_name};
use super::{
    BALANCE, FLOW_SCALE, FLOW_SCALE_FACTOR, LABEL, SIDRA, STORAGE, STORAGE_AMBER, STORAGE_RED,
    Standard, Tools, entries,
};
use crate::edit::CONTROLS;
use crate::edit::net::links_at;
use crate::edit::sites::{self, SiteRules};
use crate::graph::js_sort_by;
use crate::jsnum::{
    Maybe, math_round, number_to_js_string, to_fixed, value_to_js_string, value_truthy,
};
use crate::project::Scenario;
use crate::standards::STANDARDS;
use crate::turns::junction_name;
use crate::util::{dist, esc};
use serde_json::Value;

/// A junction or site near a node that it could be merged with.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeCandidate {
    /// Site id or node id.
    pub id: String,
    /// A junction site.
    pub is_site: bool,
    /// Name.
    pub name: String,
    /// Distance (m).
    pub d: f64,
}

impl MergeCandidate {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        serde_json::json!({ "id": self.id, "kind": if self.is_site { "site" } else { "node" }, "name": self.name, "d": self.d })
    }
}

/// `mergeCandidates(sc, nodeId)`: nearby junction nodes and sites this node could be merged with (within the site diameter),
/// nearest first, at most six.
pub fn merge_candidates(sc: &Scenario, node_id: &str) -> Vec<MergeCandidate> {
    let Some(n) = sc.nodes.get(node_id) else {
        return Vec::new();
    };
    let r = SiteRules::default().diameter;
    let own = sites::of(sc, node_id);
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<MergeCandidate> = Vec::new();
    for m in sc.nodes.values() {
        if m.id == node_id || own.is_some_and(|o| o.nodes.contains(&m.id)) {
            continue;
        }
        let d = dist(&n.ll(), &m.ll());
        if d > r {
            continue;
        }
        let ls = links_at(sc, &m.id);
        if !(m.control.str_or("none") != "none" || ls.len() >= 3) {
            continue;
        }
        let s = sites::of(sc, &m.id);
        let id = s.map_or(m.id.clone(), |s| s.id.clone());
        if seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        out.push(MergeCandidate {
            id,
            is_site: s.is_some(),
            name: match s {
                Some(s) => sites::name(sc, s),
                None => junction_name(sc, &m.id, None),
            },
            d: math_round(d),
        });
    }
    js_sort_by(&mut out, |a, b| a.d - b.d);
    out.truncate(6);
    out
}

fn kit_fld(label: &str, inner: &str, unit: &str) -> String {
    format!(
        "<label class=\"fld\"><span>{}</span><span class=\"fld-in\">{}{}</span></label>",
        label,
        inner,
        if unit.is_empty() {
            String::new()
        } else {
            format!("<span class=\"unit\">{unit}</span>")
        }
    )
}

impl<'a> Tools<'a> {
    /// `sitePanel(sc, sel)`: the junction-site panel: its name, control, results by period, the zones pinned to its nodes and the
    /// nodes. `inspector_sections` is what the interface adds below the results (`Inspector.SECTIONS`). Empty for no such site.
    pub fn site_panel(&self, i: usize, site_id: &str, inspector_sections: &str) -> String {
        let sc_run = self.sc(i);
        let sc = sc_run.sc;
        let Some(s) = sc.sites.get().and_then(|m| m.get(site_id)) else {
            return String::new();
        };
        let rank = |c: &str| match c {
            "priority" => 1,
            "stop" => 2,
            "roundabout" => 3,
            "signals" => 4,
            _ => 0,
        };
        let mut derived = "none".to_string();
        for id in &s.nodes {
            let c = sc
                .nodes
                .get(id)
                .map_or("none", |n| n.control.str_or("none"))
                .to_string();
            if rank(&c) > rank(&derived) {
                derived = c;
            }
        }
        // 'none' is not offered: the engine models a site set to 'none' with its strongest member control
        let own: Option<String> = match s.control.as_str() {
            Some(c) if !c.is_empty() && c != "none" => Some(c.to_string()),
            _ => None,
        };
        let ctl = own.clone().unwrap_or_else(|| derived.clone());
        let mut c_opts = format!(
            "<option value=\"\" {}>From its junctions ({})</option>",
            if own.is_some() { "" } else { "selected" },
            esc(&ctl_name(&derived))
        );
        for (k, l) in CONTROLS.iter().filter(|(k, _)| *k != "none") {
            c_opts.push_str(&format!(
                "<option value=\"{}\" {}>{}</option>",
                esc(k),
                if own.as_deref() == Some(*k) {
                    "selected"
                } else {
                    ""
                },
                esc(l)
            ));
        }
        let mut per = String::new();
        for p in sc_run.periods() {
            let Some(res) = sc_run.res_of(&p) else {
                continue;
            };
            let e = entries(sc, res, false).into_iter().find(|x| x.id == s.id);
            let m = e.and_then(|e| self.summary(sc, res, &e));
            if let Some(m) = m {
                per.push_str(&format!(
                    "<tr><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td><td class=\"n\">{} m</td></tr>",
                    esc(&per_name(&p)),
                    dos_cell(Some(m.dos), m.xp),
                    d1(Some(m.delay)),
                    los_badge(Some(&m.los)),
                    n0(Some(m.q95))
                ));
            }
        }
        // zone connectors may not attach to site nodes (phantom movements); the engine skips them for automatic connectors,
        // but a zone pinned to a node (z.node) still would
        let pinned: Vec<String> = sc
            .zones
            .values()
            .filter(|z| {
                z.node
                    .as_str()
                    .is_some_and(|n| s.nodes.iter().any(|x| x == n))
            })
            .map(|z| match z.name.as_str() {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => z.id.clone(),
            })
            .collect();
        let zone_warn = if pinned.is_empty() {
            String::new()
        } else {
            format!(
                "<p class=\"note warn\">{} {} straight to a node inside this site, which adds movements SIDRA would not have. Connect {} part-way along a road, or add a driveway leg.</p>",
                pinned.iter().map(|n| esc(n)).collect::<Vec<_>>().join(", "),
                if pinned.len() == 1 {
                    "connects"
                } else {
                    "connect"
                },
                if pinned.len() == 1 { "it" } else { "them" }
            )
        };
        let nodes: String = s
            .nodes
            .iter()
            .filter(|id| sc.nodes.contains_key(*id))
            .map(|id| {
                format!(
                    "<div class=\"lg-row\"><a href=\"#\" data-fly=\"node:{}\">{}</a> <span class=\"note\">{}</span><button class=\"btn small\" style=\"margin-left:auto\" data-jx-do=\"unsite\" data-site=\"{}\" data-node=\"{}\">Remove</button></div>",
                    esc(id),
                    esc(&junction_name(sc, id, None)),
                    esc(&ctl_name(sc.nodes[id].control.as_str().unwrap_or(""))),
                    esc(&s.id),
                    esc(id)
                )
            })
            .collect();
        let name = s.name.as_str().unwrap_or("");
        let mut unnamed = s.clone();
        unnamed.name = Maybe::Set(String::new());
        let cycle = match &s.cycle {
            Maybe::Set(v) => v.to_js_string(),
            _ => String::new(),
        };
        let at_site = match s.at_site_no.get() {
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    if v.is_null() {
                        String::new()
                    } else {
                        value_to_js_string(v)
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
            Some(Value::Null) | None => String::new(),
            Some(v) => value_to_js_string(v),
        };
        let signals = if ctl == "signals" {
            format!(
                "{}<label class=\"check\"><input type=\"checkbox\" data-jsite=\"fixedTiming\" {}> Keep equal green splits (don\u{2019}t optimise)</label>",
                kit_fld(
                    "Cycle time",
                    &format!(
                        "<input type=\"number\" min=\"40\" max=\"180\" step=\"5\" data-jsite=\"cycle\" value=\"{}\" placeholder=\"auto\">",
                        esc(&cycle)
                    ),
                    "s"
                ),
                if s.fixed_timing.truthy() {
                    "checked"
                } else {
                    ""
                }
            )
        } else {
            String::new()
        };
        let auto = s
            .src
            .get()
            .is_some_and(|x| x.nodes.is(&"auto".to_string().as_str().to_string()));
        let rules = SiteRules::default();
        let results = if per.is_empty() {
            "<div class=\"ins-sec\"><p class=\"note\">Run the model to see this site's results.</p></div>".to_string()
        } else {
            format!(
                "<div class=\"ins-sec\"><h4>Results</h4>{}<div class=\"tbl-wrap\"><table class=\"t compact\"><tr><th>Peak</th><th class=\"n\">Worst DoS</th><th class=\"n\">Delay (s)</th><th>LoS</th><th class=\"n\">Longest Q95</th></tr>{}</table></div>\n        <div class=\"actions\"><button class=\"btn small\" data-jx-do=\"j1\" data-site=\"{}\">Movement summary</button></div><p class=\"note\">{}.</p></div>",
                self.supp_note(),
                per,
                esc(&s.id),
                LABEL
            )
        };
        format!(
            "<div class=\"ins-head\"><div class=\"ins-kind\">Junction site<span style=\"margin-left:auto\"><button class=\"btn small icon\" data-act=\"close\" title=\"Close (Esc)\" aria-label=\"Close\">\u{2715}</button></span></div>\n      <input class=\"ins-title\" data-jsite=\"name\" value=\"{}\" placeholder=\"{}\" aria-label=\"Name\"></div>\n      <div class=\"ins-sec\"><h4>Control</h4>{}\n        {}\n        {}\n        <p class=\"note\">{} nodes modelled as one junction: one controller, legs grouped by road, and no junction delay on the links inside the site.{}</p>{}</div>\n      {}\n      {}\n      <div class=\"ins-sec\"><h4>Nodes in this site</h4>{}</div>\n      <div class=\"ins-sec\"><div class=\"actions\"><button class=\"btn small\" data-jx-do=\"zoomsite\" data-site=\"{}\">Zoom to</button><button class=\"btn small danger\" data-jx-do=\"split\" data-site=\"{}\">Split site</button></div></div>",
            esc(name),
            esc(&sites::name(sc, &unnamed)),
            kit_fld(
                "Control",
                &format!("<select data-jsite=\"control\">{c_opts}</select>"),
                ""
            ),
            signals,
            kit_fld(
                "AT site number",
                &format!(
                    "<input data-jsite=\"atSiteNo\" value=\"{}\" placeholder=\"optional\">",
                    esc(&at_site)
                ),
                ""
            ),
            s.nodes.len(),
            if auto {
                format!(" Grouped automatically ({}).", esc(&rules.basis))
            } else {
                " Grouped by hand.".to_string()
            },
            zone_warn,
            results,
            inspector_sections,
            nodes,
            esc(&s.id),
            esc(&s.id)
        )
    }
}

fn verified(v: bool) -> &'static str {
    if v {
        "<b>Checked.</b>"
    } else {
        "<b>To be checked</b> against the source."
    }
}

fn link(u: Option<&str>, t: &str) -> String {
    match u {
        Some(u) if !u.is_empty() => {
            format!(
                "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">{}</a>",
                esc(u),
                esc(t)
            )
        }
        _ => esc(t),
    }
}

/// `Registry.about` rows of the junction analysis: (what, source as a link, notes). Used by Settings, About the data and
/// the report's data appendix.
pub fn about_rows() -> Vec<[String; 3]> {
    let st = &STANDARDS;
    let hcm = &st.los.hcm6;
    let std_row = |what: &str, s: &Standard, notes: String| -> [String; 3] {
        [what.to_string(), link(s.url, s.edition), notes]
    };
    let n = |x: f64| number_to_js_string(x);
    vec![
        [
            "Level of service".to_string(),
            link(hcm.src.url, hcm.src.edition),
            format!(
                "{} DoS above 1 is F. Give way and stop report the worst movement.",
                verified(hcm.src.verified)
            ),
        ],
        [
            "Practical degree of saturation".to_string(),
            link(st.practical_dos.src.url, st.practical_dos.src.edition),
            format!(
                "{} Signals {}, roundabouts {}, give way {}.",
                verified(st.practical_dos.src.verified),
                n(st.practical_dos.signals),
                n(st.practical_dos.roundabout),
                n(st.practical_dos.sign)
            ),
        ],
        [
            "Movement capacity".to_string(),
            link(st.gaps.src.url, st.gaps.src.edition),
            format!(
                "{} Used when Capacity method is per movement.",
                verified(st.gaps.src.verified)
            ),
        ],
        [
            "Back of queue".to_string(),
            link(st.queue.src.url, st.queue.src.edition),
            format!("{} {}", verified(st.queue.src.verified), esc(st.queue.note)),
        ],
        std_row(
            "Queue storage ratio",
            &STORAGE,
            format!(
                "{} Amber above {}, red above {}.",
                verified(STORAGE.verified),
                n(STORAGE_AMBER),
                n(STORAGE_RED)
            ),
        ),
        std_row(
            "Flow-scale test",
            &FLOW_SCALE,
            format!(
                "{} Flows \u{d7} {}, junctions only.",
                verified(FLOW_SCALE.verified),
                to_fixed(FLOW_SCALE_FACTOR, 2)
            ),
        ),
        std_row(
            "SIDRA hand-off",
            &SIDRA,
            format!(
                "{} Leg and turn codes follow SIDRA drive-on-left conventions; the loader is untested against a live install.",
                verified(SIDRA.verified)
            ),
        ),
    ]
}

/// The turns balance standard, as the U-turn and balance rows of the standards register read it.
pub const fn balance_standard() -> &'static Standard {
    &BALANCE
}

/// A zone pinned to a node (kept for the site panel's callers).
#[allow(dead_code)]
fn _fly(is_site: bool, id: &str) -> String {
    fly_of(is_site, id)
}

#[allow(dead_code)]
fn _t(v: &Value) -> bool {
    value_truthy(v)
}
