//! Junction tools: junction sites, SIDRA-style volumes and the junction panel's calculations (port of features/junctions.js).
//!
//! The prototype's junctions workspace builds the junction summary, the SIDRA-style Movement Summary, the comparison across
//! scenarios, the +10% flow-scale test and the SIDRA hand-off, and the exports (junction_movements.csv, turning_movements.csv,
//! the SIDRA volumes CSV, the SIDRA 11 loader script and the printable volume grids), the turning diagram and the report's
//! junction sections. This module ports every calculation and every piece of exported text; the interface wiring (click
//! handlers, inspector sections, the map layer and tool) is left out.
//!
//! Where the JavaScript reads the store (`Store.P`, `Store.S`, `sc.res`) a [`Tools`] holds the project (its settings and
//! scenarios), each scenario's stored results and a [`Features`] host that stands in for the other feature scripts
//! (`Compliance.supplementary`, `Calibration.phf`). A [`Sc`] is one scenario with its results, as the prototype's `sc` with `sc.res`.

mod csv;
mod export_text;
mod figure;
mod j1;
mod panel;
mod scale;
mod sections;
mod sidra;
mod summary;
mod text;

pub use csv::*;
pub use figure::*;
pub use j1::*;
pub use panel::*;
pub use scale::*;
pub use sections::*;
pub use sidra::*;
pub use summary::*;
pub use text::*;

use crate::edit::json::JsonValue;
use crate::graph::{js_sort_by, js_str_lt};
use crate::jsnum::{to_precision, value_to_js_string};
use crate::project::{Project, Scenario, Settings};
use crate::results::{Approach, NodeRes, Res, Results, SiteResult, export_ready};
use crate::standards::STANDARDS as ENGINE_STANDARDS;
use crate::turns::{Arm, arms, code_arms, junction_name, site_rows};
use crate::util::LatLng;
use serde_json::Value;
use std::collections::HashSet;

/* ---------------- the junctions feature's data ---------------- */

/// `Engine.JUNCTION_LABEL`: how every junction result is labelled.
pub const LABEL: &str = crate::junction::JUNCTION_LABEL;

/// The AT Integrated Transport Assessment Guidelines link the storage ratio cites.
pub const ITA_URL: &str = "https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf";
/// The TTOC-06 traffic signals modelling guidelines link.
pub const TTOC_URL: &str =
    "https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf";
/// The SIDRA API reference link.
pub const SIDRA_API: &str = "https://api.sidrasolutions.com/latest/llms-full.txt";

/// Standing text D3: junction results are indicative and not SIDRA output.
pub const D3: &str = "Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.";
/// Standing text D1: Spare Construct is a screening tool.
pub const D1: &str = "Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.";
/// Standing text D12 for the SIDRA hand-over.
pub const D12: &str = "This export is prepared for use in SIDRA Intersection. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.";

/// A threshold or convention the junction tools state as data: its name, edition, source and whether it was checked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standard {
    /// Identifier.
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// Edition or basis.
    pub edition: &'static str,
    /// Link to the source, when there is one.
    pub url: Option<&'static str>,
    /// Checked against the source.
    pub verified: bool,
}

/// `STANDARDS.storage`: the queue storage ratio (95% back of queue over the storage).
pub const STORAGE: Standard = Standard {
    id: "corridor-storage-ratio",
    name: "Queue storage ratio (95% back of queue \u{f7} storage)",
    edition: "Spare Construct assumption; supports AT ITA Guidelines 2025 v2 s4.2.1.1 safety effects",
    url: Some(ITA_URL),
    verified: false,
};
/// Storage ratio above which the cell is amber.
pub const STORAGE_AMBER: f64 = 0.8;
/// Storage ratio above which the cell is red.
pub const STORAGE_RED: f64 = 1.0;
/// Basis of the storage ratio thresholds.
pub const STORAGE_BASIS: &str = "Spare Construct assumption";
/// `STANDARDS.flowScale`: the flow-scale test.
pub const FLOW_SCALE: Standard = Standard {
    id: "ttoc06-flow-scale",
    name: "Flow-scale test",
    edition: "TTOC-06 Traffic Signals Modelling Guidelines",
    url: Some(TTOC_URL),
    verified: false,
};
/// Flow-scale test factor.
pub const FLOW_SCALE_FACTOR: f64 = 1.10;
/// `STANDARDS.uTurn`: the U-turn flag.
pub const U_TURN: Standard = Standard {
    id: "corridor-uturn-flag",
    name: "U-turn flag",
    edition: "Spare Construct assumption",
    url: None,
    verified: false,
};
/// U-turn flow (veh/h) above which a U-turn is flagged.
pub const U_TURN_VPH: f64 = 20.0;
/// `STANDARDS.balance`: turns balance the approach flows.
pub const BALANCE: Standard = Standard {
    id: "corridor-turn-balance",
    name: "Turns balance approach flows",
    edition: "Spare Construct assumption",
    url: None,
    verified: false,
};
/// Tolerance (veh/h) between the turns and the approach flow.
pub const BALANCE_TOL: f64 = 1.0;
/// `STANDARDS.figure`: figure rounding.
pub const FIGURE: Standard = Standard {
    id: "corridor-figure-round",
    name: "Figure rounding",
    edition: "Spare Construct convention (nearest 5 veh/h)",
    url: None,
    verified: false,
};
/// Figure rounding step (veh/h).
pub const FIGURE_STEP: f64 = 5.0;
/// `STANDARDS.sidra`: the SIDRA hand-off.
pub const SIDRA: Standard = Standard {
    id: "sidra11-nz",
    name: "SIDRA 11 hand-off",
    edition: "SIDRA INTERSECTION 11 .NET API (untested against a live install)",
    url: Some(SIDRA_API),
    verified: false,
};

/// SIDRA site type by control (`siteType`), None for other controls.
pub fn sidra_site_type(control: &str) -> Option<f64> {
    match control {
        "signals" => Some(0.0),
        "roundabout" => Some(1.0),
        "priority" => Some(2.0),
        "stop" => Some(3.0),
        "none" => Some(2.0),
        _ => None,
    }
}

/// SIDRA site type names.
pub const SIDRA_TYPE_NAME: [&str; 4] = ["signals", "roundabout", "give-way", "two-way stop"];
/// SIDRA site standard: 2 is New Zealand.
pub const SIDRA_SETUP: f64 = 2.0;
/// Unit time for volumes (min).
pub const SIDRA_UNIT_TIME: f64 = 60.0;
/// Peak flow period (min) for the model's peak flow factor.
pub const SIDRA_PEAK_FLOW_PERIOD: f64 = 30.0;
/// Peak flow period (min) for a counted peak hour factor.
pub const SIDRA_PHF_PERIOD: f64 = 15.0;
/// Growth (%) written to SIDRA: volumes already include growth.
pub const SIDRA_GROWTH_PCT: f64 = 0.0;

/// The periods in the time-periods order, so an enabled inter-peak or Saturday period appears in every table.
pub const ORDER: [&str; 4] = ["AM", "IP", "PM", "SAT"];
/// The turning figures' 'AM (PM)' pair.
pub const PERIODS: [&str; 2] = ["AM", "PM"];
/// The turn designations in table order.
pub const TURN_ORDER: [&str; 8] = ["L3", "L2", "L1", "T1", "R1", "R2", "R3", "U"];

/* ---------------- the host: other feature scripts the tools read ---------------- */

/// What the junction tools ask the other feature scripts for. Each method has the answer for a feature that is not installed.
pub trait Features {
    /// `Compliance.supplementary(P)`: the E27 tier 1 marker, or an empty text.
    fn supplementary(&self, _project: &Project) -> String {
        String::new()
    }
    /// `Calibration.phf(id, period, sc)`: the peak hour factor of an imported turning count at a node, when there is one.
    fn count_phf(&self, _sc: &Scenario, _node_id: &str, _period: &str) -> Option<f64> {
        None
    }
}

/// A host with none of the other features installed.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFeatures;

impl Features for NoFeatures {}

/// One scenario with its stored results (`sc` and `sc.res`).
#[derive(Clone, Copy)]
pub struct Sc<'a> {
    /// The scenario.
    pub sc: &'a Scenario,
    /// Its stored results by period.
    pub res: &'a Results,
    /// Its position in the project's scenario list (the identity the report's scenario choice uses).
    pub idx: usize,
}

impl<'a> Sc<'a> {
    /// `resOf(sc, p)`: the stored result of a period.
    pub fn res_of(&self, p: &str) -> Option<&'a Res> {
        self.res.get(p)
    }
    /// `periodsOf(sc)`: the periods with results, in the time-periods order, then any others in text order.
    pub fn periods(&self) -> Vec<String> {
        periods_of(self.res)
    }
    /// The scenario's name, or its id when it has none (`sc.name || sc.id`).
    pub fn name_or_id(&self) -> String {
        match self.sc.name.as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => self.sc.id.clone(),
        }
    }
    /// The scenario's name as the prototype prints it (`esc(sc.name)`: a missing name is empty).
    pub fn name(&self) -> &'a str {
        self.sc.name.as_str().unwrap_or("")
    }
}

/// The project, each scenario's results and the host: what `Store.P` and the scenarios' `res` give the prototype's functions.
#[derive(Clone, Copy)]
pub struct Tools<'a> {
    /// The project (its settings are `Store.S`).
    pub project: &'a Project,
    /// Results of each scenario, in the project's order.
    pub results: &'a [Results],
    /// The other features.
    pub host: &'a dyn Features,
    /// The project's JSON with its keys in file order: where the author's report inputs (`P.report`) and the other untyped
    /// parts of the project are read from. None reads them as not set.
    pub raw: Option<&'a JsonValue>,
}

impl<'a> Tools<'a> {
    /// Tools for a project and its results (one [`Results`] per scenario).
    pub fn new(project: &'a Project, results: &'a [Results], host: &'a dyn Features) -> Tools<'a> {
        Tools {
            project,
            results,
            host,
            raw: None,
        }
    }
    /// The same tools reading the untyped parts of the project from its JSON.
    pub fn with_raw(mut self, raw: &'a JsonValue) -> Tools<'a> {
        self.raw = Some(raw);
        self
    }
    /// `Store.S`: the project's settings.
    pub fn st(&self) -> &'a Settings {
        &self.project.settings
    }
    /// Scenario i with its results.
    pub fn sc(&self, i: usize) -> Sc<'a> {
        Sc {
            sc: &self.project.scenarios[i],
            res: &self.results[i],
            idx: i,
        }
    }
    /// Every scenario with its results (`P.scenarios`).
    pub fn scs(&self) -> Vec<Sc<'a>> {
        (0..self.project.scenarios.len())
            .map(|i| self.sc(i))
            .collect()
    }
    /// `suppText(P)`: the E27 tier 1 marker the compliance feature gives, or an empty text.
    pub fn supp_text(&self) -> String {
        self.host.supplementary(self.project)
    }
    /// `suppNote(P)`: the marker as a note above a table, or nothing.
    pub fn supp_note(&self) -> String {
        let t = self.supp_text();
        if t.is_empty() {
            String::new()
        } else {
            format!(
                "<p class=\"note supp\" data-supp=\"1\"><b>{}</b></p>",
                crate::util::esc(&t)
            )
        }
    }
}

/* ---------------- text and order helpers ---------------- */

/// `a.sort()` for text: UTF-16 code unit order.
pub(crate) fn js_sort_strings(v: &mut [String]) {
    js_sort_by(v, |a, b| {
        if js_str_lt(a, b) {
            -1.0
        } else if js_str_lt(b, a) {
            1.0
        } else {
            0.0
        }
    });
}

/// `periodsOf(sc)`: the periods that have results, in the time-periods order (AM, IP, PM, SAT), then the others sorted.
pub fn periods_of(res: &Results) -> Vec<String> {
    let ks: Vec<&str> = res.keys().collect();
    let mut out: Vec<String> = ORDER
        .iter()
        .filter(|p| ks.contains(p))
        .map(|p| p.to_string())
        .collect();
    let mut rest: Vec<String> = ks
        .iter()
        .filter(|p| !ORDER.contains(p))
        .map(|p| p.to_string())
        .collect();
    js_sort_strings(&mut rest);
    out.extend(rest);
    out
}

/// `periodsOfAll(scs)`: the periods that any of the scenarios has results for.
pub fn periods_of_all(scs: &[Sc]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for s in scs {
        for p in s.periods() {
            if !seen.contains(&p) {
                seen.push(p);
            }
        }
    }
    let mut out: Vec<String> = ORDER
        .iter()
        .filter(|p| seen.iter().any(|q| q == *p))
        .map(|p| p.to_string())
        .collect();
    let mut rest: Vec<String> = seen
        .iter()
        .filter(|p| !ORDER.contains(&p.as_str()))
        .cloned()
        .collect();
    js_sort_strings(&mut rest);
    out.extend(rest);
    out
}

/// `norm(b)`: a bearing in [0, 360).
pub(crate) fn norm(b: f64) -> f64 {
    ((b % 360.0) + 360.0) % 360.0
}

/// `hash(s)`: FNV-1a over the UTF-16 code units, as base 36 text.
pub fn hash(s: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(0x01000193);
    }
    to_base36(h)
}

fn to_base36(mut n: u32) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut d = Vec::new();
    while n > 0 {
        let r = (n % 36) as u8;
        d.push(if r < 10 { b'0' + r } else { b'a' + r - 10 });
        n /= 36;
    }
    d.reverse();
    String::from_utf8(d).expect("ASCII")
}

/// `ncText(res)`: the convergence marker for exports forced past the gate, or an empty text.
pub fn nc_text(res: &Res) -> String {
    if export_ready(Some(res)).ok {
        String::new()
    } else {
        format!(
            "NOT CONVERGED: gap {}%, SIDRA needs {}% or better",
            to_precision(res.gap * 100.0, 2),
            to_precision(ENGINE_STANDARDS.sidra_gap.max * 100.0, 1)
        )
    }
}

/* ---------------- a junction result, from a node or a site ---------------- */

/// The fields of a junction result the tools read, from a node's result (`res.nodes[id]`) or a site's combined result.
#[derive(Clone, Copy, Debug)]
pub struct Nr<'a> {
    /// Control type.
    pub kind: &'a str,
    /// Approaches.
    pub appr: &'a [Approach],
    /// Reported delay (s).
    pub delay: f64,
    /// Basis of the delay.
    pub delay_basis: &'a str,
    /// Volume-weighted average delay (s).
    pub avg_delay: f64,
    /// Highest degree of saturation.
    pub dos: f64,
    /// Total approach volume.
    pub vol: f64,
    /// Level of service.
    pub los: &'a str,
    /// Intersection level of service: None when the key is missing, Some(None) when it is null.
    pub intersection_los: Option<Option<&'a str>>,
    /// Level of service as shown.
    pub los_text: &'a str,
    /// Practical degree of saturation (a node's result only).
    pub xp: Option<f64>,
    /// Level of service method (a node's result only).
    pub method: Option<&'a str>,
    /// Signal cycle (s), when signals.
    pub c: Option<f64>,
    /// Signal green split, when signals.
    pub split: Option<&'a [f64]>,
}

impl<'a> From<&'a NodeRes> for Nr<'a> {
    fn from(n: &'a NodeRes) -> Nr<'a> {
        Nr {
            kind: &n.r#type,
            appr: &n.appr,
            delay: n.delay,
            delay_basis: &n.delay_basis,
            avg_delay: n.avg_delay,
            dos: n.dos,
            vol: n.vol,
            los: &n.los,
            intersection_los: Some(n.intersection_los.as_deref()),
            los_text: &n.los_text,
            xp: Some(n.xp),
            method: Some(&n.method),
            c: n.c,
            split: n.split.as_deref(),
        }
    }
}

impl<'a> From<&'a SiteResult> for Nr<'a> {
    fn from(n: &'a SiteResult) -> Nr<'a> {
        Nr {
            kind: &n.r#type,
            appr: &n.appr,
            delay: n.delay,
            delay_basis: &n.delay_basis,
            avg_delay: n.avg_delay,
            dos: n.dos,
            vol: n.vol,
            los: &n.los,
            intersection_los: Some(Some(&n.intersection_los)),
            los_text: &n.los_text,
            xp: None,
            method: None,
            c: None,
            split: None,
        }
    }
}

/// A junction in a result: a junction site, or a single controlled node.
#[derive(Clone, Debug)]
pub struct Entry<'a> {
    /// Site id or node id.
    pub id: String,
    /// A junction site (otherwise a node).
    pub is_site: bool,
    /// Name.
    pub name: String,
    /// The junction's result; None for an uncontrolled junction with turning traffic.
    pub nr: Option<Nr<'a>>,
    /// The controller's node (a site's representative node).
    pub node: String,
    /// Member node ids.
    pub nodes: Vec<String>,
    /// Control.
    pub kind: String,
    /// AT site number(s), or null.
    pub at_site_no: Value,
    /// Position (the mean of the nodes').
    pub ll: Option<LatLng>,
}

impl Entry<'_> {
    /// `kind`: "site" or "node".
    pub fn kind_name(&self) -> &'static str {
        if self.is_site { "site" } else { "node" }
    }
}

/// `centre(sc, ids)`: the mean position of the nodes that exist.
pub(crate) fn centre(sc: &Scenario, ids: &[String]) -> Option<LatLng> {
    let ns: Vec<&crate::project::Node> = ids.iter().filter_map(|id| sc.nodes.get(id)).collect();
    if ns.is_empty() {
        return None;
    }
    let n = ns.len() as f64;
    let lat = ns.iter().fold(0.0, |s, x| s + x.lat.num()) / n;
    let lng = ns.iter().fold(0.0, |s, x| s + x.lng.num()) / n;
    Some([lat, lng])
}

fn node_at_site_no(sc: &Scenario, id: &str) -> Value {
    sc.nodes
        .get(id)
        .and_then(|n| n.extra.get("atSiteNo"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// `entries(sc, res, opts)`: the junctions in a result, sorted by name then id. `all` adds uncontrolled junctions with turning traffic.
pub fn entries<'a>(sc: &Scenario, res: &'a Res, all: bool) -> Vec<Entry<'a>> {
    let mut out: Vec<Entry<'a>> = Vec::new();
    let mut in_site: HashSet<&str> = HashSet::new();
    if let Some(sites) = res.sites.as_ref() {
        for s in sites.values() {
            for n in &s.nodes {
                in_site.insert(n.as_str());
            }
            let nr: Option<Nr<'a>> = if s.one {
                res.nodes.get(&s.rep).map(Nr::from)
            } else {
                s.result.as_ref().map(Nr::from)
            };
            if nr.is_none_or(|x| x.appr.is_empty()) && !all {
                continue;
            }
            let own = sc.sites.get().and_then(|m| m.get(&s.id));
            let own_name = own.and_then(|o| o.name.as_str()).filter(|n| !n.is_empty());
            let name = match own_name {
                Some(n) => n.to_string(),
                None if !s.name.is_empty() => s.name.clone(),
                None => "Unnamed junction".to_string(),
            };
            let at_site_no = if s.at_site_no.is_null() {
                own.and_then(|o| o.at_site_no.get().cloned())
                    .unwrap_or(Value::Null)
            } else {
                s.at_site_no.clone()
            };
            let kind = match nr.map(|x| x.kind).filter(|k| !k.is_empty()) {
                Some(k) => k.to_string(),
                None if !s.r#type.is_empty() => s.r#type.clone(),
                None => "none".to_string(),
            };
            out.push(Entry {
                id: s.id.clone(),
                is_site: true,
                name,
                nr,
                node: s.rep.clone(),
                nodes: s.nodes.clone(),
                kind,
                at_site_no,
                ll: centre(sc, &s.nodes),
            });
        }
    }
    for (id, nr) in res.nodes.iter() {
        let in_a_site = nr
            .site
            .as_ref()
            .is_some_and(|sid| res.sites.as_ref().is_some_and(|m| m.contains_key(sid)));
        if in_a_site || in_site.contains(id) || !sc.nodes.contains_key(id) {
            continue;
        }
        let n = &sc.nodes[id];
        out.push(Entry {
            id: id.to_string(),
            is_site: false,
            name: junction_name(sc, id, None),
            nr: Some(Nr::from(nr)),
            node: id.to_string(),
            nodes: vec![id.to_string()],
            kind: nr.r#type.clone(),
            at_site_no: node_at_site_no(sc, id),
            ll: Some(n.ll()),
        });
    }
    if all {
        for id in res.turns.keys() {
            if res.nodes.contains_key(id) || in_site.contains(id) || !sc.nodes.contains_key(id) {
                continue;
            }
            let n = &sc.nodes[id];
            out.push(Entry {
                id: id.to_string(),
                is_site: false,
                name: junction_name(sc, id, None),
                nr: None,
                node: id.to_string(),
                nodes: vec![id.to_string()],
                kind: match n.control.as_str() {
                    Some(c) if !c.is_empty() => c.to_string(),
                    _ => "none".to_string(),
                },
                at_site_no: Value::Null,
                ll: Some(n.ll()),
            });
        }
    }
    js_sort_by(&mut out, |p, q| {
        let c = crate::edit::collate::locale_compare_f64(&p.name, &q.name);
        if c != 0.0 {
            c
        } else if js_str_lt(&p.id, &q.id) {
            -1.0
        } else {
            1.0
        }
    });
    out
}

/// `entryFor(sc, res, id)`: the entry that shows a node: its site when it belongs to one in this result, else the node itself.
pub fn entry_for<'a>(sc: &Scenario, res: &'a Res, id: &str) -> Option<Entry<'a>> {
    let all = entries(sc, res, true);
    if let Some(i) = all.iter().position(|e| e.id == id) {
        return all.into_iter().nth(i);
    }
    let i = all.iter().position(|e| e.nodes.iter().any(|n| n == id))?;
    all.into_iter().nth(i)
}

/* ---------------- legs ---------------- */

/// An arm of a junction coded with NZ / SIDRA legs, with the arc keys that enter and leave along it.
#[derive(Clone, Debug)]
pub struct Leg {
    /// The coded arm.
    pub arm: Arm,
    /// Arc keys entering the junction along it (`inKeys`).
    pub in_keys: Vec<String>,
    /// Arc keys leaving along it (`outKeys`).
    pub out_keys: Vec<String>,
}

/// `nodeLegs(sc, res, nodeId)`: the arms of a node coded with NZ / SIDRA legs.
pub fn node_legs(sc: &Scenario, res: Option<&Res>, node_id: &str) -> Vec<Leg> {
    let empty: Vec<crate::results::TurnRow> = Vec::new();
    let list = res.and_then(|r| r.turns.get(node_id)).unwrap_or(&empty);
    let mut a = arms(sc, node_id, list, None);
    code_arms(&mut a);
    a.into_iter()
        .map(|arm| Leg {
            in_keys: vec![arm.in_key.clone()],
            out_keys: vec![arm.out_key.clone()],
            arm,
        })
        .collect()
}

/// `legsOf(sc, res, e)`: a site's legs, or a node's arms.
pub fn legs_of(sc: &Scenario, st: &Settings, res: &Res, e: &Entry) -> Vec<Leg> {
    if e.is_site && res.sites.as_ref().is_some_and(|m| m.contains_key(&e.id)) {
        return site_rows(sc, Some(res), st, &e.id)
            .arms
            .into_iter()
            .map(|arm| Leg {
                in_keys: arm.ins.clone(),
                out_keys: arm.outs.clone(),
                arm,
            })
            .collect();
    }
    node_legs(sc, Some(res), &e.node)
}

/// `movesOf(sc, res, e)`: the movement rows of a site, or of a node.
pub fn moves_of(
    sc: &Scenario,
    st: &Settings,
    res: &Res,
    e: &Entry,
) -> Vec<crate::turns::MovementRow> {
    if e.is_site && res.sites.as_ref().is_some_and(|m| m.contains_key(&e.id)) {
        site_rows(sc, Some(res), st, &e.id).rows
    } else {
        crate::turns::movements(sc, Some(res), st, &e.node)
    }
}

/// `xid(e)`: the stable junction id for files that leave Spare Construct: a node keeps its id; a site is 'J' and a hash of its sorted member nodes.
pub fn xid(e: &Entry) -> String {
    if e.is_site {
        let mut ns = e.nodes.clone();
        js_sort_strings(&mut ns);
        format!("J{}", hash(&ns.join("|")))
    } else {
        e.id.clone()
    }
}

/// `String(v)` of a loose JSON value, as the exports write a cell.
pub(crate) fn js_str(v: &Value) -> String {
    value_to_js_string(v)
}
