//! Road-safety metrics (`linkAADT`, `linkRisk`, `nodeRisk`, `junctionOf`, `riskBand`, `linkBand`, `safetySummary`, `crashesNear`).
//!
//! Rates use the window the crashes were downloaded for, not just the years that happen to have crashes. Divided
//! roads mapped as two one-way carriageways count road kilometres once (KiwiRAP collective risk is per kilometre
//! of road, both directions). Personal risk uses current exposure only: counts, or modelled volumes from results
//! no later than the crash window. Links shorter than 500 m, or with under 200 veh/day, get no risk band: KiwiRAP
//! measures risk over sections several kilometres long.

use super::assign::{assign_crashes, Assignment, CasLoad, Tally, Years, LINK_R, LOW_AADT, NODE_R, SHORT_KM};
use super::cas::Crash;
use crate::calibrate::stored_res;
use crate::datahub::{matches, Data};
use crate::edit::net::project;
use crate::graph::js_sort_by;
use crate::jsnum::number_to_js_string;
use crate::project::{OrderedMap, Project, Scenario, Settings};
use crate::results::Res;
use crate::util::dist;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// A risk band (`riskBand`): which of five bands a value falls in, with its label, colour and where the cut-offs come from.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Band {
    /// "low", "low-medium", "medium", "medium-high" or "high"; null when there is no value.
    pub band: Option<&'static str>,
    /// Display label ("Low", "Low-medium", ..., or why there is none).
    pub label: String,
    /// Map colour.
    pub colour: &'static str,
    /// Where the cut-offs come from.
    pub basis: String,
}

struct Bands {
    cuts: [f64; 4],
    basis: &'static str,
}

fn bands_of(kind: &str) -> Bands {
    match kind {
        "personal" => Bands { cuts: [4.0, 5.0, 7.0, 9.0], basis: "KiwiRAP personal risk bands (rural state highways), indicative here" },
        "injury" => Bands { cuts: [0.5, 1.0, 2.0, 4.0], basis: "Indicative bands set by Spare Construct, not an NZTA threshold" },
        "node" => Bands { cuts: [0.1, 0.2, 0.4, 0.6], basis: "Indicative bands set by Spare Construct, not an NZTA threshold" },
        _ => Bands { cuts: [0.04, 0.07, 0.11, 0.19], basis: "KiwiRAP collective risk bands (rural state highways), indicative here" },
    }
}

const BAND_NAMES: [&str; 5] = ["low", "low-medium", "medium", "medium-high", "high"];
const BAND_LABELS: [&str; 5] = ["Low", "Low-medium", "Medium", "Medium-high", "High"];
const BAND_COLOURS: [&str; 5] = ["#2f9e44", "#94d82d", "#fcc419", "#f76707", "#e03131"];
const NO_COLOUR: &str = "#adb5bd";

/// `riskBand(value, kind)`: the band of a rate. Kinds: "collective" (DSI crashes per km per year) and "personal" (DSI crashes per 100 million vehicle-km) use KiwiRAP's risk-map bands; "injury" (injury crashes per km per year) and "node" (DSI crashes per year at a junction) are Spare Construct's own indicative bands. An unknown kind reads as collective; None or a value that is not finite gives "No data".
pub fn risk_band(value: Option<f64>, kind: &str) -> Band {
    let b = bands_of(kind);
    match value {
        Some(v) if v.is_finite() => {
            let mut i = 0;
            while i < b.cuts.len() && v >= b.cuts[i] {
                i += 1;
            }
            Band { band: Some(BAND_NAMES[i]), label: BAND_LABELS[i].to_string(), colour: BAND_COLOURS[i], basis: b.basis.to_string() }
        }
        _ => Band { band: None, label: "No data".to_string(), colour: NO_COLOUR, basis: b.basis.to_string() },
    }
}

/// `noBand(why)`: no band, because the link is too short or too quiet for one.
pub fn no_band(why: &str) -> Band {
    Band {
        band: None,
        label: why.to_string(),
        colour: NO_COLOUR,
        basis: format!("No band: {}. KiwiRAP bands are for sections several kilometres long.", why.to_lowercase()),
    }
}

/// One link's crash risk (`linkRisk` entry): its tally, length, road length, volume and rates.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRisk {
    /// The crash tally.
    #[serde(flatten)]
    pub t: Tally,
    /// Link length (km).
    pub km: f64,
    /// Road length (km): half the link for one carriageway of a divided road.
    pub road_km: f64,
    /// One of two carriageways of a divided road.
    pub divided: bool,
    /// Years of crash data.
    pub years: Option<f64>,
    /// Two-way daily volume (veh/day), or null.
    pub aadt: Option<f64>,
    /// Where the volume came from.
    pub aadt_src: Option<String>,
    /// Injury crashes per road km per year.
    pub per_km_yr: Option<f64>,
    /// DSI crashes per road km per year (KiwiRAP collective risk).
    pub dsi_per_km_yr: Option<f64>,
    /// DSI crashes per 100 million vehicle-km (KiwiRAP personal risk).
    pub personal: Option<f64>,
    /// Shorter than 500 m.
    pub short: bool,
    /// Under 200 veh/day or no volume.
    pub low_volume: bool,
}

/// One junction's crash risk (`nodeRisk` entry).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRisk {
    /// The crash tally.
    #[serde(flatten)]
    pub t: Tally,
    /// Roads meeting here.
    pub legs: usize,
    /// The nodes of the junction.
    pub nodes: Vec<String>,
    /// Only motorway and ramp links meet here.
    pub interchange: bool,
    /// Years of crash data.
    pub years: Option<f64>,
    /// DSI crashes per year.
    pub dsi_per_yr: Option<f64>,
    /// Injury crashes per year.
    pub inj_per_yr: Option<f64>,
    /// Names of the roads meeting here.
    pub name: String,
}

/// A link in the safety summary's top list: the risk with the link's name and the bands.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopLink {
    /// Link id.
    pub id: String,
    /// Name, or "Unnamed road".
    pub name: String,
    /// Road class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    /// The link's risk.
    #[serde(flatten)]
    pub risk: LinkRisk,
    /// Collective risk band.
    pub collective: Band,
    /// Personal risk band (no band for a quiet link).
    pub personal_band: Band,
}

/// A junction in the safety summary's top list: the risk with the node's id and band.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopNode {
    /// The junction's main node.
    pub id: String,
    /// The junction's risk.
    #[serde(flatten)]
    pub risk: NodeRisk,
    /// DSI crashes a year band.
    pub band: Band,
}

/// Crashes in one year (`byYear` entry).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearRow {
    /// The year (null for crashes with none).
    pub year: Option<f64>,
    /// The tally.
    #[serde(flatten)]
    pub t: Tally,
    /// The year to date.
    pub ytd: bool,
    /// A full year still inside the CAS entry lag.
    pub may_be_incomplete: bool,
}

/// Crashes involving vulnerable road users (`vulnerable`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vulnerable {
    /// Walking.
    pub p: Tally,
    /// Cycling.
    pub c: Tally,
    /// Motorcycle.
    pub m: Tally,
    /// Any of them.
    pub any: Tally,
}

/// What `safetySummary` gives: study-area totals by severity and year, vulnerable users, the top ten links and junctions, interchanges.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySummary {
    /// The years of data covered.
    pub years: Years,
    /// Totals.
    pub total: Tally,
    /// Deaths and serious injuries.
    pub dsi: usize,
    /// Injury crashes.
    pub injury: usize,
    /// Crashes by year, earliest first.
    pub by_year: Vec<YearRow>,
    /// Vulnerable road users.
    pub vulnerable: Vulnerable,
    /// Crashes assigned to a link.
    pub on_links: usize,
    /// Crashes assigned to a junction.
    pub at_nodes: usize,
    /// Crashes assigned to no link.
    pub unassigned: usize,
    /// The ten worst links of 500 m or more.
    pub top_links: Vec<TopLink>,
    /// The ten worst junctions.
    pub top_nodes: Vec<TopNode>,
    /// The five worst motorway merges and diverges.
    pub interchanges: Vec<TopNode>,
    /// Links under 500 m with crashes, left out.
    pub short_links: usize,
    /// What the figures are.
    pub basis: String,
}

/// `linkAADT(sc, pair)`: two-way daily volume on a link from matched counts in use (NZTA estimates only while the NZTA source is on).
///
/// Two-way link: a two-way count wins, then both directions summed, then one direction doubled. One-way carriageway:
/// counts in its own direction win; a count with no direction is a two-way total, so it is halved where the road's other
/// carriageway runs alongside. Gives the volume and where it came from, by link id.
pub fn link_aadt(sc: &Scenario, s: &Settings, data: &Data, pair: &HashMap<String, String>) -> HashMap<String, (f64, &'static str)> {
    #[derive(Default)]
    struct Entry {
        both: Vec<f64>,
        ab: Vec<f64>,
        ba: Vec<f64>,
    }
    let m = matches(sc, s, data);
    let mut by: HashMap<String, Entry> = HashMap::new();
    let nzta_on = data.use_of("nzta").truthy();
    for c in &data.counts {
        if c.off.truthy() || !(c.adt.num() > 0.0) || !(data.in_use(c) || (c.virtual_.truthy() && nzta_on)) {
            continue;
        }
        let Some(mt) = m.get(&c.id) else { continue };
        let e = by.entry(mt.link.clone()).or_default();
        let list = if c.directional.truthy() && c.dir.truthy() && mt.dir != "both" {
            if mt.dir == "ab" { &mut e.ab } else { &mut e.ba }
        } else {
            &mut e.both
        };
        list.push(c.adt.num());
    }
    let mean = |a: &[f64]| a.iter().fold(0.0, |s, v| s + v) / a.len() as f64;
    let mut out = HashMap::new();
    for (id, e) in &by {
        let Some(lk) = sc.links.get(id) else { continue };
        let two = lk.lanes_ab.num() > 0.0 && lk.lanes_ba.num() > 0.0;
        let own = if lk.lanes_ab.num() > 0.0 { &e.ab } else { &e.ba };
        if two {
            if !e.both.is_empty() {
                out.insert(id.clone(), (mean(&e.both), "count"));
            } else if !e.ab.is_empty() && !e.ba.is_empty() {
                out.insert(id.clone(), (mean(&e.ab) + mean(&e.ba), "count"));
            } else {
                out.insert(id.clone(), (mean(if !e.ab.is_empty() { &e.ab } else { &e.ba }) * 2.0, "count (one direction, doubled)"));
            }
        } else if !own.is_empty() {
            out.insert(id.clone(), (mean(own), "count"));
        } else if !e.both.is_empty() {
            if pair.contains_key(id) {
                out.insert(id.clone(), (mean(&e.both) / 2.0, "count (two-way total, halved for this carriageway)"));
            } else {
                out.insert(id.clone(), (mean(&e.both), "count"));
            }
        }
    }
    out
}

/// The crashes saved in a project under `key` ("crashes" for the five-year set, "crashesLong" for the ten-year set), as tuples read for the analysis.
pub fn crashes_of(p: &Project, key: &str) -> Vec<Crash> {
    match p.data.get().and_then(|d| d.extra.get(key)) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|t| match t {
                Value::Array(t) => Some(Crash::from_tuple(t)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The crash analysis of one scenario: crashes assigned to its links and junctions, ready for risk rates and the summary.
///
/// Make it with [`SafetyModel::new`]; it holds the assignment (`assignCrashes`) so that asking for link risk, junction risk
/// and the summary does the matching once.
pub struct SafetyModel<'a> {
    sc: &'a Scenario,
    settings: &'a Settings,
    data: Data,
    base: Option<&'a Scenario>,
    crashes: Vec<Crash>,
    /// The assignment of crashes to links and junctions.
    pub assignment: Assignment<'a>,
}

impl<'a> SafetyModel<'a> {
    /// Reads the project's crashes (`P.data.crashes`) and assigns them to a scenario's links and junctions; None when the project holds no crashes (the prototype answers null then).
    ///
    /// The base scenario (`Store.baseSc`) supplies modelled volumes when the scenario's own results are for a later year.
    pub fn new(p: &'a Project, sc: &'a Scenario) -> Option<SafetyModel<'a>> {
        let crashes = crashes_of(p, "crashes");
        if crashes.is_empty() {
            return None;
        }
        let cas = CasLoad::from_value(p.data.get().and_then(|d| d.loaded.get()).and_then(|l| l.get("cas")));
        let assignment = assign_crashes(sc, &crashes, &cas);
        let base = p.base_scenario().filter(|b| !std::ptr::eq(*b, sc));
        Some(SafetyModel { sc, settings: &p.settings, data: Data::read(p), base, crashes, assignment })
    }

    /// The crashes, as read.
    pub fn crashes(&self) -> &[Crash] {
        &self.crashes
    }

    /// `linkRisk(sc, res)`: the risk of every existing link, keyed by link id.
    ///
    /// Volume comes from a matched count where there is one, otherwise (AB plus BA modelled veh/h) times `peakToDaily`
    /// (10 when unset) from `res`, or the mean of the scenario's AM and PM results when `res` is None; results for a year
    /// after the crash window are not used (the base scenario's are tried instead).
    pub fn link_risk(&self, res: Option<&Res>) -> OrderedMap<LinkRisk> {
        let a = &self.assignment;
        let yr = &a.years;
        let current = |r: &Res| !(r.year > 0.0 && yr.to.is_some_and(|t| t > 0.0 && r.year > t + 1.0));
        let mut rs: Vec<Res> = match res {
            Some(r) => vec![r.clone()],
            None => ["AM", "PM"].iter().filter_map(|p| stored_res(self.sc, p)).collect(),
        };
        let forecast = if !rs.is_empty() && !rs.iter().any(current) { Some(rs[0].year) } else { None };
        rs.retain(current);
        if rs.is_empty()
            && let Some(b) = self.base
        {
            rs = ["AM", "PM"].iter().filter_map(|p| stored_res(b, p)).filter(current).collect();
        }
        let ptd = self.settings.peak_to_daily.or(10.0);
        let aadt = link_aadt(self.sc, self.settings, &self.data, &a.pair);
        let yrs = yr.span;
        let yrs_n = yrs.unwrap_or(0.0);
        let mut out = OrderedMap::new();
        for it in &a.lk {
            let id = &it.link.id;
            let t = &it.t;
            let (mut vol, mut src): (Option<f64>, Option<String>) = match aadt.get(id) {
                Some((v, s)) => (Some(*v), Some(s.to_string())),
                None => (None, None),
            };
            if vol.is_none() && !rs.is_empty() {
                let mut s = 0.0;
                for r in &rs {
                    let av = r.arcs.get(&format!("{id}:ab")).map_or(0.0, |x| x.v);
                    let bv = r.arcs.get(&format!("{id}:ba")).map_or(0.0, |x| x.v);
                    s += av + bv;
                }
                vol = Some(s / rs.len() as f64 * ptd);
                src = Some("model".to_string());
            } else if vol.is_none()
                && let Some(f) = forecast
                && f != 0.0
                && !f.is_nan()
            {
                src = Some(format!("model (forecast year {}, not used)", number_to_js_string(f)));
            }
            let divided = a.pair.contains_key(id);
            let road_km = if divided { it.km / 2.0 } else { it.km };
            let ky = road_km * yrs_n;
            let v = vol.unwrap_or(0.0);
            let vkm = if vol.is_some() && v > 0.0 && it.km > 0.0 && yrs_n > 0.0 { v * 365.0 * it.km * yrs_n } else { 0.0 };
            out.insert(
                id.clone(),
                LinkRisk {
                    t: t.clone(),
                    km: it.km,
                    road_km,
                    divided,
                    years: yrs,
                    aadt: vol,
                    aadt_src: src,
                    per_km_yr: if ky > 0.0 { Some(t.injury as f64 / ky) } else { None },
                    dsi_per_km_yr: if ky > 0.0 { Some(t.dsi as f64 / ky) } else { None },
                    personal: if vkm > 0.0 { Some(t.dsi as f64 / vkm * 1e8) } else { None },
                    short: it.km < SHORT_KM,
                    low_volume: !(vol.is_some() && v >= LOW_AADT),
                },
            );
        }
        out
    }

    /// `nodeRisk(sc)`: the risk of every junction, keyed by its main node.
    pub fn node_risk(&self) -> OrderedMap<NodeRisk> {
        let yrs = self.assignment.years.span;
        let mut out = OrderedMap::new();
        for j in &self.assignment.jn {
            out.insert(
                j.id.clone(),
                NodeRisk {
                    t: j.t.clone(),
                    legs: j.legs,
                    nodes: j.nodes.clone(),
                    interchange: j.interchange,
                    years: yrs,
                    dsi_per_yr: yrs.filter(|y| *y != 0.0).map(|y| j.t.dsi as f64 / y),
                    inj_per_yr: yrs.filter(|y| *y != 0.0).map(|y| j.t.injury as f64 / y),
                    name: j.name.clone(),
                },
            );
        }
        out
    }

    /// `junctionOf(sc, nodeId)`: the junction (the `node_risk` key) a node belongs to, or None.
    pub fn junction_of(&self, node_id: &str) -> Option<&str> {
        self.assignment.j_of_node.get(node_id).map(|&j| self.assignment.jn[j].id.as_str())
    }

    /// `safetySummary(sc, res)`: study-area totals by severity and year, vulnerable road users, the top ten links (500 m or longer) and junctions by DSI then injury crashes, and interchange merges apart.
    pub fn summary(&self, res: Option<&Res>) -> SafetySummary {
        let a = &self.assignment;
        let lr = self.link_risk(res);
        let nr = self.node_risk();
        let mut total = Tally::default();
        let mut years: Vec<(Option<f64>, Tally)> = Vec::new();
        let mut vul = Vulnerable { p: Tally::default(), c: Tally::default(), m: Tally::default(), any: Tally::default() };
        for c in &self.crashes {
            total.add(c);
            let y = if c.year > 0.0 { Some(c.year) } else { None };
            match years.iter_mut().find(|(k, _)| *k == y) {
                Some(e) => e.1.add(c),
                None => {
                    let mut t = Tally::default();
                    t.add(c);
                    years.push((y, t));
                }
            }
            let f = &c.flags;
            if f.contains('p') {
                vul.p.add(c);
            }
            if f.contains('c') {
                vul.c.add(c);
            }
            if f.contains('m') {
                vul.m.add(c);
            }
            if !f.is_empty() {
                vul.any.add(c);
            }
        }
        let rank = |x: &Tally, y: &Tally| -> f64 {
            let d = y.dsi as f64 - x.dsi as f64;
            if d != 0.0 {
                return d;
            }
            let d = y.injury as f64 - x.injury as f64;
            if d != 0.0 {
                return d;
            }
            y.crashes as f64 - x.crashes as f64
        };
        let mut top_links: Vec<TopLink> = lr
            .iter()
            .filter(|(_, r)| r.t.crashes > 0 && !r.short)
            .map(|(id, r)| {
                let lk = &self.sc.links[id];
                TopLink {
                    id: id.to_string(),
                    name: lk.name.str_or("Unnamed road").to_string(),
                    cls: lk.cls.as_str().map(str::to_string),
                    risk: r.clone(),
                    collective: risk_band(r.dsi_per_km_yr, "collective"),
                    personal_band: if r.low_volume {
                        no_band(if r.aadt.is_some_and(|v| v > 0.0) { "Under 200 veh/day" } else { "No traffic volume" })
                    } else {
                        risk_band(r.personal, "personal")
                    },
                }
            })
            .collect();
        js_sort_by(&mut top_links, |p, q| rank(&p.risk.t, &q.risk.t));
        top_links.truncate(10);
        let mut nodes: Vec<TopNode> = nr
            .iter()
            .filter(|(_, r)| r.t.crashes > 0)
            .map(|(id, r)| TopNode { id: id.to_string(), risk: r.clone(), band: risk_band(r.dsi_per_yr, "node") })
            .collect();
        js_sort_by(&mut nodes, |p, q| rank(&p.risk.t, &q.risk.t));
        let top_nodes: Vec<TopNode> = nodes.iter().filter(|n| !n.risk.interchange).take(10).cloned().collect();
        let interchanges: Vec<TopNode> = nodes.iter().filter(|n| n.risk.interchange).take(5).cloned().collect();
        let on_links = a.link_of.iter().filter(|&&i| i >= 0).count();
        let at_nodes = a.node_of.iter().filter(|&&i| i >= 0).count();
        let short_n = lr.values().filter(|r| r.t.crashes > 0 && r.short).count();
        let mut by_year: Vec<(Option<f64>, Tally)> = years;
        js_sort_by(&mut by_year, |x, y| x.0.unwrap_or(9999.0) - y.0.unwrap_or(9999.0));
        let by_year: Vec<YearRow> = by_year
            .into_iter()
            .map(|(year, t)| YearRow {
                year,
                ytd: a.years.window && year.is_some() && year == a.years.ytd,
                may_be_incomplete: year.is_some() && a.years.lag_year.flatten().is_some_and(|l| l != 0.0) && year == a.years.lag_year.flatten(),
                t,
            })
            .collect();
        let yr_text = |v: Option<f64>| match v {
            Some(x) if x != 0.0 && !x.is_nan() => number_to_js_string(x),
            _ => "?".to_string(),
        };
        let basis = format!(
            "Reported crashes (NZTA CAS) within {} m of a road and {} m of a junction, {}\u{2013}{}{}. Links under {} m are left out of the top links and get no band. Risk bands are indicative.",
            number_to_js_string(LINK_R),
            number_to_js_string(NODE_R),
            yr_text(a.years.from),
            yr_text(a.years.to),
            if a.years.partial { " (latest year to date)" } else { "" },
            number_to_js_string(SHORT_KM * 1000.0)
        );
        SafetySummary {
            years: a.years.clone(),
            dsi: total.dsi,
            injury: total.injury,
            total,
            by_year,
            vulnerable: vul,
            on_links,
            at_nodes,
            unassigned: self.crashes.len() - on_links,
            top_links,
            top_nodes,
            interchanges,
            short_links: short_n,
            basis,
        }
    }
}

/// `linkBand(r, kind)`: the band of one link's rate for display: no data without a risk, no band for a short link (or a quiet one for personal risk).
pub fn link_band(r: Option<&LinkRisk>, kind: &str) -> Band {
    let Some(r) = r else { return risk_band(None, kind) };
    if r.short {
        return no_band("Under 500 m");
    }
    if kind == "personal" && r.low_volume {
        return no_band(if r.aadt.is_some_and(|v| v > 0.0) { "Under 200 veh/day" } else { "No traffic volume" });
    }
    risk_band(
        match kind {
            "personal" => r.personal,
            "injury" => r.per_km_yr,
            _ => r.dsi_per_km_yr,
        },
        kind,
    )
}

/// What `crashesNear` finds: crashes by severity near a link or junction, and the crashes themselves.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CrashesNear {
    /// Fatal.
    #[serde(rename = "F")]
    pub f: usize,
    /// Serious injury.
    #[serde(rename = "S")]
    pub s: usize,
    /// Minor injury.
    #[serde(rename = "M")]
    pub m: usize,
    /// Non-injury.
    #[serde(rename = "N")]
    pub n: usize,
    /// The crash tuples, in the order they were saved.
    pub list: Vec<Vec<Value>>,
}

/// What `crashesNear` is asked about: a junction or a link by id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Near<'a> {
    /// A junction (a node id).
    Node(&'a str),
    /// A link.
    Link(&'a str),
    /// Anything else (counts nothing).
    Other,
}

/// `crashesNear(sc, sel, R, opts)`: crashes within `r` metres of a link (or r + 15 m of a junction), by severity.
///
/// `long` reads the ten-year set (`crashesLong`) for state highway accesses. None when the project holds no crashes of
/// that set, or the junction or link is not in the scenario; the prototype's default radius is 25 m.
pub fn crashes_near(p: &Project, sc: &Scenario, sel: Near, r: f64, long: bool) -> Option<CrashesNear> {
    let cr = crashes_of(p, if long { "crashesLong" } else { "crashes" });
    if cr.is_empty() {
        return None;
    }
    let mut out = CrashesNear { f: 0, s: 0, m: 0, n: 0, list: Vec::new() };
    let mut count = |c: &Crash| {
        match c.severity.as_str() {
            "F" => out.f += 1,
            "S" => out.s += 1,
            "M" => out.m += 1,
            "N" => out.n += 1,
            _ => {}
        }
        out.list.push(c.tuple.clone());
    };
    match sel {
        Near::Node(id) => {
            let n = sc.nodes.get(id)?;
            let ll = [n.lat.num(), n.lng.num()];
            for c in &cr {
                if dist(&ll, &[c.lat, c.lng]) <= r + 15.0 {
                    count(c);
                }
            }
        }
        Near::Link(id) => {
            let lk = sc.links.get(id)?;
            let g = sc.geom(lk);
            let (mut la0, mut la1, mut ln0, mut ln1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
            for q in &g {
                la0 = crate::jsnum::math_min(la0, q[0]);
                la1 = crate::jsnum::math_max(la1, q[0]);
                ln0 = crate::jsnum::math_min(ln0, q[1]);
                ln1 = crate::jsnum::math_max(ln1, q[1]);
            }
            let pad = r / 90000.0;
            for c in &cr {
                if c.lat < la0 - pad || c.lat > la1 + pad || c.lng < ln0 - pad * 1.3 || c.lng > ln1 + pad * 1.3 {
                    continue;
                }
                if project(sc, lk, [c.lat, c.lng]).d <= r {
                    count(c);
                }
            }
        }
        Near::Other => {}
    }
    Some(out)
}
