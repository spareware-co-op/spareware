//! Setting up a study area (studyarea.js): roads from OpenStreetMap, gateways, counts and crashes, existing land use as zones, gateway flows from counts, the
//! AM and PM runs and the optional fit to counts, in the current scenario (and, by default with several scenarios, in all of them).
//!
//! Each step is independent: one that fails (a map server that does not answer) is reported in the warnings and the others still run. The downloads go through
//! the host's [`Net`]; nothing here waits or sleeps.

use super::counts::{LoadOpts, counts_of, crashes_of, data_d, load_all};
use super::mapbox::{center, contains, extend, latlng_bounds, pad};
use crate::assign::assign;
use crate::calibrate::{FitOpts, fit, stored_res};
use crate::census::{Bx, Downloads, Failure, Net, host_of};
use crate::datahub::{Data, calibration, gateways_from_counts};
use crate::edit::IdSource;
use crate::edit::history::mark_stale;
use crate::importers::{Bounds, OVERPASS, OsmOpts, OverpassOpts, auto_gateways, osm_to_scenario, overpass_query};
use crate::jsnum::{Maybe, js_trim, json_number, math_max, math_round, number_to_js_string, usize_to_f64, value_truthy};
use crate::landuse::{self, BuildOpts, StudyBox};
use crate::project::{Project, Scenario};
use crate::results::AssignOpts;
use crate::util::{dist, fmt};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Areas above this (km\u{b2}) get a warning with a rough time.
pub const WARN_KM2: f64 = 36.0;
/// Areas above this (km\u{b2}) are refused.
pub const MAX_KM2: f64 = 120.0;
/// A built zone with the name of one of the user's zones this close (m) is left out.
pub const SAME_ZONE_M: f64 = 150.0;
const SRC: &str = "existing-lu";

/// What `estimate` says of a study area.
#[derive(Clone, Debug, PartialEq)]
pub struct Estimate {
    /// Area in km\u{b2}, rounded for display.
    pub km2: f64,
    /// A warning or the reason the area is refused.
    pub warn: Option<String>,
    /// The area can be set up.
    pub ok: bool,
    /// A rough time in minutes.
    pub minutes: f64,
}

impl Estimate {
    /// The estimate as JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({ "km2": json_number(self.km2), "warn": self.warn, "ok": self.ok, "minutes": json_number(self.minutes) })
    }
}

/// `estimate(bounds)`: the area of a box in km\u{b2}; warns above 36 km\u{b2} (with a rough time) and refuses above 120 km\u{b2}.
#[must_use]
pub fn estimate(b: &Bx) -> Estimate {
    let c = center(b);
    let width = dist(&[c[0], b.w], &[c[0], b.e]);
    let height = dist(&[b.s, c[1]], &[b.n, c[1]]);
    let a = width * height / 1e6;
    let km2 = if a < 1.0 { math_round(a * 100.0) / 100.0 } else { math_round(a * 10.0) / 10.0 };
    let mins = math_max(1.0, math_round(a / 12.0));
    // !(a > 0): no size, NaN included
    let warn = if a <= 0.0 || a.is_nan() {
        Some("The study area has no size. Draw a box around the area you want to model.".to_string())
    } else if a > MAX_KM2 {
        Some(format!("That area is {} km\u{b2}, too large to set up in one go (the limit is {} km\u{b2}). Zoom in or draw a smaller study area.", fmt::num(a, 0), number_to_js_string(MAX_KM2)))
    } else if a > WARN_KM2 {
        Some(format!(
            "That is a large area ({} km\u{b2}). Setting it up may take about {} minute{}, and the free map servers may refuse it. Areas under {} km\u{b2} work best.",
            fmt::num(a, 0),
            number_to_js_string(mins),
            if mins == 1.0 { "" } else { "s" },
            number_to_js_string(WARN_KM2)
        ))
    } else {
        None
    };
    Estimate { km2, warn, ok: a > 0.0 && a <= MAX_KM2, minutes: mins }
}

/// What a setup is asked to do (the `opts` of `setup`): every step on, except service lanes and fitting to counts.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the prototype's setup options: nine independent step switches, each read from its own key of the options"
)]
pub struct SetupOpts {
    /// Download the roads.
    pub roads: bool,
    /// Include local and residential streets.
    pub local: bool,
    /// Include service lanes.
    pub service: bool,
    /// Add gateways where main roads leave the area.
    pub gateways: bool,
    /// Download counts.
    pub counts: bool,
    /// Download crashes.
    pub crashes: bool,
    /// Build existing land use zones.
    pub existing: bool,
    /// Run the model.
    pub run: bool,
    /// Fit the model to the counts.
    pub fit_counts: bool,
    /// Put the new roads, gateways and zones in every scenario (default: when there is more than one).
    pub all: Option<bool>,
}

impl Default for SetupOpts {
    fn default() -> Self {
        SetupOpts { roads: true, local: true, service: false, gateways: true, counts: true, crashes: true, existing: true, run: true, fit_counts: false, all: None }
    }
}

/// What a setup did (its answer).
#[derive(Clone, Debug, PartialEq)]
pub struct SetupOut {
    /// Road segments added.
    pub roads: f64,
    /// Gateways added.
    pub gateways: f64,
    /// Gateways whose flows were set from counts.
    pub gw_from_counts: f64,
    /// Count sites downloaded.
    pub counts: f64,
    /// Crashes downloaded.
    pub crashes: f64,
    /// Existing land use zones added.
    pub zones: f64,
    /// Zones from an earlier setup that were replaced.
    pub replaced: f64,
    /// Dwellings in the new zones.
    pub dwellings: f64,
    /// A model run was made.
    pub ran: bool,
    /// Network totals by period: trips, speed, junctions over capacity, vehicle hours.
    pub kpi: Vec<(String, Value)>,
    /// Fit to counts by period: sites, share passing, R\u{b2} and modelled over counted.
    pub fit: Vec<(String, Value)>,
    /// What fitting to counts did, by period (None when it was not asked for or could not be done).
    pub calibration: Option<Vec<(String, Value)>>,
    /// Scenarios that got the new network.
    pub scenarios: f64,
    /// Things that did not go to plan.
    pub warnings: Vec<String>,
    /// Time taken (ms); this port does not time itself.
    pub ms: f64,
}

impl SetupOut {
    /// The answer as JSON, in the prototype's shape.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let obj = |l: &[(String, Value)]| Value::Object(l.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Map<_, _>>());
        json!({
            "roads": json_number(self.roads), "gateways": json_number(self.gateways), "gwFromCounts": json_number(self.gw_from_counts), "counts": json_number(self.counts), "crashes": json_number(self.crashes),
            "zones": json_number(self.zones), "replaced": json_number(self.replaced), "dwellings": json_number(self.dwellings), "ran": self.ran, "kpi": obj(&self.kpi), "fit": obj(&self.fit),
            "calibration": self.calibration.as_ref().map_or(Value::Null, |c| obj(c)), "scenarios": json_number(self.scenarios), "warnings": self.warnings, "ms": json_number(self.ms),
        })
    }
}

/// `Importers.overpass(b, opts)`: downloads the roads of a box, trying the Overpass servers in turn. Gives the server's answer (`{ elements }`), or the last server's complaint.
///
/// # Errors
///
/// The last server's complaint when no Overpass server gives an answer.
pub fn roads_download(net: &mut dyn Net, b: &Bx, opts: &OverpassOpts, status: &mut dyn FnMut(&str)) -> Result<Value, String> {
    let q = overpass_query(&Bounds { south: b.s, west: b.w, north: b.n, east: b.e }, opts);
    let body = format!("data={}", crate::census::encode_uri_component(&q));
    let mut last: Option<String> = None;
    for url in OVERPASS {
        status(&format!("Downloading roads from {}\u{2026}", host_of(url)));
        match net.post_form(url, &body) {
            Ok(j) => return Ok(j),
            Err(f) => last = Some(f.message(url)),
        }
    }
    Err(last.unwrap_or_else(|| "No OpenStreetMap server answered".to_string()))
}

fn from_lu(z: &crate::project::Zone) -> bool {
    z.kind.is("zone") && z.existing.is_true() && (z.src.get().and_then(Value::as_str) == Some(SRC) || z.name.str_or("").starts_with("Existing \u{b7} "))
}

fn in_bounds(b: &Bx, z: &crate::project::Zone) -> bool {
    let (lat, lng) = (z.lat.num(), z.lng.num());
    lat.is_finite() && lng.is_finite() && contains(b, lat, lng)
}

// sc.res as an object (anything else is replaced by {}), with res[period] = value when one is given
fn set_res(sc: &mut Scenario, period: Option<&str>, value: Value) {
    if let Maybe::Set(Value::Object(m)) = &mut sc.res {
        if let Some(k) = period {
            m.insert(k.to_string(), value);
        }
    } else {
        let mut m = Map::new();
        if let Some(k) = period {
            m.insert(k.to_string(), value);
        }
        sc.res = Maybe::Set(Value::Object(m));
    }
}

fn active_index(p: &Project) -> usize {
    p.active.as_str().and_then(|id| p.scenarios.iter().position(|s| s.id == id)).unwrap_or(0)
}

// A model run's progress callback: (iteration, gap, message)
type Progress<'a> = &'a mut (dyn FnMut(f64, f64, &str) + 'a);

// `Engine.assign(sc, { period })`, stored as the scenario's result for the period (nothing for a scenario that is not there)
fn run_period(p: &mut Project, idx: usize, period: &str, progress: Option<Progress<'_>>) {
    let opts = AssignOpts { period: Some(period.to_string()), ..Default::default() };
    let settings = p.settings.clone();
    let mut adapt = progress;
    let mut cb = |it: u32, gap: f64, msg: &str| {
        if let Some(f) = adapt.as_deref_mut() {
            f(f64::from(it), gap, msg);
        }
    };
    let Some(sc) = p.scenarios.get(idx) else { return };
    let res = assign(p, sc, &settings, &opts, Some(&mut cb));
    // A result always serialises (serde_json writes a non-finite number as null), so the null fallback never happens
    let value = serde_json::to_value(&res).unwrap_or(Value::Null);
    if let Some(sc) = p.scenarios.get_mut(idx) {
        set_res(sc, Some(period), value);
    }
}

// The ids of the active scenario's nodes, links and zones before the setup changed it
struct Before {
    nodes: HashSet<String>,
    links: HashSet<String>,
    zones: HashSet<String>,
}

// What a setup carries from step to step
struct Steps<'a> {
    p: &'a mut Project,
    ids: &'a mut dyn IdSource,
    net: &'a mut dyn Net,
    dl: &'a mut Downloads,
    area: Bx,
    opts: SetupOpts,
    sc_idx: usize,
    status: &'a mut dyn FnMut(&str, f64),
    out: SetupOut,
    changed: bool,
    removed: Vec<String>,
}

/// `setup(bounds, opts, onStatus)`: builds a working model of an area in the current scenario. Roads from OpenStreetMap, then gateways, counts and crashes,
/// existing land use zones, gateway flows from counts, the AM and PM runs and (if asked) the fit to counts. Running it again replaces the existing land use
/// inside the new box instead of adding to it.
///
/// `status` is called with what is being done and how far along it is (0 to 1). Fails (nothing changed) when the area is empty or larger than 120 km\u{b2}.
///
/// # Errors
///
/// With nothing changed: the reason the area is refused (empty, or larger than 120 km\u{b2}), or that the project has no scenario.
#[allow(clippy::too_many_arguments)]
pub fn setup(
    p: &mut Project,
    ids: &mut dyn IdSource,
    net: &mut dyn Net,
    dl: &mut Downloads,
    bounds: &Bx,
    opts: &SetupOpts,
    on_status: &mut dyn FnMut(&str, f64),
) -> Result<SetupOut, String> {
    let area = bounds.clone();
    let est = estimate(&area);
    if !est.ok {
        return Err(est.warn.unwrap_or_default());
    }
    let sc_idx = active_index(p);
    let multi = p.scenarios.len() > 1;
    let o = SetupOpts { all: Some(opts.all.unwrap_or(multi)), ..*opts };
    // active_index gives a scenario's position, or 0, which only a project without scenarios lacks
    let Some(sc) = p.scenarios.get_mut(sc_idx) else {
        return Err("The project has no scenario to set up.".to_string());
    };
    set_res(sc, None, Value::Null);
    let out = SetupOut {
        roads: 0.0,
        gateways: 0.0,
        gw_from_counts: 0.0,
        counts: 0.0,
        crashes: 0.0,
        zones: 0.0,
        replaced: 0.0,
        dwellings: 0.0,
        ran: false,
        kpi: Vec::new(),
        fit: Vec::new(),
        calibration: None,
        scenarios: 1.0,
        warnings: est.warn.into_iter().collect(),
        ms: 0.0,
    };
    let before = Before {
        nodes: sc.nodes.keys().map(str::to_string).collect(),
        links: sc.links.keys().map(str::to_string).collect(),
        zones: sc.zones.keys().map(str::to_string).collect(),
    };
    let mut status = |text: &str, frac: f64| on_status(text, frac.clamp(0.0, 1.0));
    let mut steps = Steps { p, ids, net, dl, area, opts: o, sc_idx, status: &mut status, out, changed: false, removed: Vec::new() };

    steps.roads();
    let local = steps.local_nodes(&before.nodes);
    steps.gateways(&local);
    steps.counts_and_crashes(&local);
    steps.existing();
    let others = steps.share(&before, multi);
    // Count-to-road matches follow the network, so the results are marked out of date
    if steps.changed
        && let Some(sc) = steps.p.scenarios.get_mut(sc_idx)
    {
        mark_stale(sc);
    }
    steps.gateway_flows(&others);
    let can_run = steps.p.scenarios.get(sc_idx).is_some_and(|sc| !sc.links.is_empty() && !sc.zones.is_empty());
    steps.run(can_run);
    if o.fit_counts {
        steps.fit_counts(can_run);
    }
    steps.summary();
    (steps.status)("Study area ready", 1.0);
    Ok(steps.out)
}

impl Steps<'_> {
    /* 1 \u{b7} roads */
    fn roads(&mut self) {
        if !self.opts.roads {
            return;
        }
        (self.status)("Downloading roads from OpenStreetMap\u{2026}", 0.02);
        if let Err(e) = self.import_roads() {
            self.out.warnings.push(format!("Downloading roads from OpenStreetMap failed: {e}"));
        }
    }

    fn import_roads(&mut self) -> Result<(), String> {
        let osm = OverpassOpts { service: self.opts.service, local: Some(self.opts.local) };
        let status = &mut *self.status;
        let data = roads_download(self.net, &self.area, &osm, &mut |text| status(text, 0.05))?;
        status("Building the network\u{2026}", 0.2);
        if data.is_null() {
            return Err("Cannot read properties of null (reading 'elements')".to_string());
        }
        // setup checked the scenario is there
        let Some(sc) = self.p.scenarios.get_mut(self.sc_idx) else { return Ok(()) };
        let added = osm_to_scenario(sc, self.ids, &data, &OsmOpts::default());
        self.out.roads = usize_to_f64(added);
        if added > 0 {
            self.changed = true;
        }
        if sc.links.is_empty() {
            self.out.warnings.push("No roads were found in this area. Check the box covers the roads you want to model.".to_string());
        }
        Ok(())
    }

    // This area's roads: the ones this setup imported, else those around the box (never roads elsewhere)
    fn local_nodes(&self, before: &HashSet<String>) -> Vec<String> {
        let Some(sc) = self.p.scenarios.get(self.sc_idx) else { return Vec::new() };
        let fresh: Vec<String> = sc.nodes.keys().filter(|id| !before.contains(*id)).map(str::to_string).collect();
        let far = pad(&self.area, 2.0);
        if fresh.is_empty() {
            sc.nodes.values().filter(|n| contains(&far, n.lat.num(), n.lng.num())).map(|n| n.id.clone()).collect()
        } else {
            fresh
        }
    }

    /* 2 \u{b7} gateways where main roads leave the area */
    fn gateways(&mut self, local: &[String]) {
        if !self.opts.gateways {
            return;
        }
        let Some(sc) = self.p.scenarios.get_mut(self.sc_idx) else { return };
        if sc.links.is_empty() {
            return;
        }
        (self.status)("Adding gateways\u{2026}", 0.25);
        let local_set: HashSet<String> = local.iter().cloned().collect();
        let area = &self.area;
        let made = auto_gateways(sc, self.ids, &Bounds { south: area.s, west: area.w, north: area.n, east: area.e }, Some(&local_set));
        self.out.gateways = usize_to_f64(made);
        if made > 0 {
            self.changed = true;
        }
    }

    /* 3 \u{b7} counts and crashes for the study box */
    fn counts_and_crashes(&mut self, local: &[String]) {
        let mut which: Vec<&str> = Vec::new();
        if self.opts.counts {
            which.extend(["nzta", "at"]);
        }
        if self.opts.crashes {
            which.push("cas");
        }
        if which.is_empty() {
            return;
        }
        (self.status)("Downloading counts and crashes\u{2026}", 0.3);
        if let Err(e) = self.download_data(&which, local) {
            self.out.warnings.push(format!("Downloading counts and crashes failed: {e}"));
        }
    }

    fn download_data(&mut self, which: &[&str], local: &[String]) -> Result<(), String> {
        let area = &self.area;
        let mut wide = latlng_bounds(&[[area.s, area.w], [area.n, area.e]]);
        if let Some(sc) = self.p.scenarios.get(self.sc_idx) {
            for id in local {
                if let Some(node) = sc.nodes.get(id) {
                    extend(&mut wide, node.lat.num(), node.lng.num());
                }
            }
        }
        let status = &mut *self.status;
        let msgs = load_all(self.p, &*self.ids, self.net, &pad(&wide, 0.05), which, &mut |text| status(text, 0.35), &LoadOpts::default())?;
        let failed: Vec<&str> = which.iter().copied().filter(|src| msgs.iter().any(|msg| msg.starts_with(&format!("{} failed", src.to_uppercase())))).collect();
        for msg in &msgs {
            if msg.contains(" failed ") {
                self.out.warnings.push(format!("Data download: {msg}"));
            }
        }
        let d_counts = counts_of(self.p).map_or(&[] as &[Value], |c| c.as_slice());
        self.out.counts = usize_to_f64(
            d_counts
                .iter()
                .filter(|c| c.get("src").and_then(Value::as_str).is_some_and(|s| which.contains(&s)) && !failed.iter().any(|f| c.get("src").and_then(Value::as_str) == Some(*f)))
                .count(),
        );
        let cas_ok = which.contains(&"cas") && !failed.contains(&"cas");
        self.out.crashes = if cas_ok { usize_to_f64(crashes_of(self.p).map_or(&[] as &[Value], |c| c.as_slice()).len()) } else { 0.0 };
        Ok(())
    }

    /* 4 \u{b7} existing land use as zones: replaces what an earlier setup made inside this box */
    fn existing(&mut self) {
        if !self.opts.existing {
            return;
        }
        (self.status)("Downloading buildings and land use\u{2026}", 0.5);
        if let Err(e) = self.existing_zones() {
            self.out.warnings.push(format!("Downloading buildings and land use failed: {e}"));
        }
    }

    fn existing_zones(&mut self) -> Result<(), String> {
        let status = &mut *self.status;
        let els = crate::census::existing_land_use_download(self.net, &mut self.dl.good, &self.area, &mut |text| status(text, 0.55))?;
        status("Grouping buildings into zones\u{2026}", 0.65);
        let area = &self.area;
        let sbox = StudyBox { s: area.s, w: area.w, n: area.n, e: area.e };
        // setup checked the scenario is there
        let Some(sc) = self.p.scenarios.get_mut(self.sc_idx) else { return Ok(()) };
        let built = landuse::build(&Value::Array(els), Some(&sbox), self.ids, &BuildOpts { cell: None, links: landuse::road_lines(sc) });
        let doomed: Vec<String> = sc.zones.iter().filter(|(_, z)| from_lu(z) && in_bounds(area, z)).map(|(k, _)| k.to_string()).collect();
        for id in doomed {
            sc.zones.remove(&id);
            self.removed.push(id);
        }
        self.out.replaced = usize_to_f64(self.removed.len());
        for mut zone in built.zones {
            if !(zone.lat.num().is_finite() && zone.lng.num().is_finite()) {
                continue;
            }
            let nm = js_trim(zone.name.str_or("")).to_lowercase();
            let zl = [zone.lat.num(), zone.lng.num()];
            let dup = sc.zones.values().any(|other| !other.kind.is("gateway") && js_trim(other.name.str_or("")).to_lowercase() == nm && dist(&[other.lat.num(), other.lng.num()], &zl) < SAME_ZONE_M);
            if dup {
                continue;
            }
            let id = if !zone.id.is_empty() && !sc.zones.contains_key(&zone.id) { zone.id.clone() } else { self.ids.uid("z") };
            zone.id.clone_from(&id);
            if zone.connectors.is_missing() {
                zone.connectors = Maybe::Null;
            }
            if !zone.lu.is_set() {
                zone.lu = Maybe::Set(crate::project::OrderedMap::new());
            }
            zone.kind = Maybe::Set("zone".to_string());
            zone.existing = true.into();
            zone.src = Maybe::Set(json!(SRC));
            self.out.zones += 1.0;
            // The land use was set just above when it was not, so this always adds
            if let Some(lu) = zone.lu.get() {
                self.out.dwellings += crate::jsnum::or_default(lu.num_at("residential").num(), 0.0) + crate::jsnum::or_default(lu.num_at("apartment").num(), 0.0);
            }
            sc.zones.insert(id, zone);
        }
        self.out.dwellings = math_round(self.out.dwellings);
        if self.out.zones > 0.0 || !self.removed.is_empty() {
            self.changed = true;
        }
        let removed = &self.removed;
        let me_hit = !removed.is_empty()
            && self.p.me.get().is_some_and(|me| {
                ["AM", "PM"].iter().any(|per| me.factors.get(per).and_then(|f| f.get()).is_some_and(|f| f.keys().any(|k| removed.iter().any(|id| k.split('>').any(|part| part == id)))))
            });
        if me_hit {
            self.out.warnings.push("Existing land use was replaced, so matrix estimation factors for the old zones no longer apply. Fit to counts again.".to_string());
        }
        Ok(())
    }

    // Other scenarios get the same new roads, gateways and zones, and lose the replaced zones; gives the other scenarios
    fn share(&mut self, before: &Before, multi: bool) -> Vec<usize> {
        let sc_idx = self.sc_idx;
        let others: Vec<usize> = if self.opts.all == Some(true) { (0..self.p.scenarios.len()).filter(|i| *i != sc_idx).collect() } else { Vec::new() };
        if !others.is_empty() && self.changed {
            // setup checked the scenario is there
            let Some(sc) = self.p.scenarios.get(sc_idx) else { return others };
            let new_nodes: Vec<_> = sc.nodes.iter().filter(|(k, _)| !before.nodes.contains(*k)).map(|(k, v)| (k.to_string(), v.clone())).collect();
            let new_links: Vec<_> = sc.links.iter().filter(|(k, _)| !before.links.contains(*k)).map(|(k, v)| (k.to_string(), v.clone())).collect();
            let new_zones: Vec<_> = sc.zones.iter().filter(|(k, _)| !before.zones.contains(*k)).map(|(k, v)| (k.to_string(), v.clone())).collect();
            for &si in &others {
                // others holds positions in the list, so this never skips one
                let Some(other) = self.p.scenarios.get_mut(si) else { continue };
                for id in &self.removed {
                    if other.zones.get(id).is_some_and(from_lu) {
                        other.zones.remove(id);
                    }
                }
                for (k, v) in &new_nodes {
                    if !other.nodes.contains_key(k) {
                        other.nodes.insert(k.clone(), v.clone());
                    }
                }
                for (k, v) in &new_links {
                    if !other.links.contains_key(k) {
                        other.links.insert(k.clone(), v.clone());
                    }
                }
                for (k, v) in &new_zones {
                    if !other.zones.contains_key(k) {
                        other.zones.insert(k.clone(), v.clone());
                    }
                }
                if let Maybe::Set(Value::Object(res)) = &mut other.res {
                    for r in res.values_mut() {
                        if value_truthy(r)
                            && let Some(m) = r.as_object_mut() {
                                m.insert("stale".to_string(), json!(true));
                            }
                    }
                }
            }
            self.out.scenarios += usize_to_f64(others.len());
        } else if multi && self.changed {
            self.out.warnings.push("Only the current scenario was set up; the other scenarios are unchanged.".to_string());
        }
        others
    }

    /* 5 \u{b7} gateway flows from counts (every scenario that got the new network) */
    fn gateway_flows(&mut self, others: &[usize]) {
        let has_gw = self.p.scenarios.get(self.sc_idx).is_some_and(|sc| sc.zones.values().any(|z| z.kind.is("gateway")));
        if !has_gw || counts_of(self.p).map_or(&[] as &[Value], |c| c.as_slice()).is_empty() {
            return;
        }
        (self.status)("Setting gateway flows from counts\u{2026}", 0.7);
        let settings = self.p.settings.clone();
        let data = Data::read(self.p);
        let Some(sc) = self.p.scenarios.get_mut(self.sc_idx) else { return };
        let set = gateways_from_counts(sc, &settings, &data);
        self.out.gw_from_counts = usize_to_f64(set);
        for &si in others {
            let data = Data::read(self.p);
            if let Some(other) = self.p.scenarios.get_mut(si) {
                gateways_from_counts(other, &settings, &data);
            }
        }
        if set > 0
            && let Some(sc) = self.p.scenarios.get_mut(self.sc_idx)
        {
            mark_stale(sc);
        }
    }

    /* 6 \u{b7} run the model */
    fn run(&mut self, can_run: bool) {
        if self.opts.run && !can_run {
            let no_roads = self.p.scenarios.get(self.sc_idx).is_none_or(|sc| sc.links.is_empty());
            self.out.warnings.push(if no_roads { "There are no roads, so the model was not run." } else { "There are no zones or gateways, so the model was not run." }.to_string());
        }
        // Fitting to counts runs the model itself, so the plain runs are only needed without it
        let fit_on = self.opts.fit_counts && !counts_of(self.p).map_or(&[] as &[Value], |c| c.as_slice()).is_empty() && can_run;
        if self.opts.run && can_run && !fit_on {
            for (k, period) in ["AM", "PM"].iter().enumerate() {
                let what = format!("Running the {period} peak");
                let status = &mut *self.status;
                status(&format!("{what}\u{2026}"), 0.75 + usize_to_f64(k) * 0.08);
                let mut prog = |it: f64, _gap: f64, msg: &str| {
                    let text = if msg.is_empty() { format!("iteration {}", number_to_js_string(it)) } else { msg.to_string() };
                    status(&format!("{period} peak \u{b7} {text}"), 0.75 + usize_to_f64(k) * 0.08 + 0.06);
                };
                run_period(self.p, self.sc_idx, period, Some(&mut prog));
                self.out.ran = true;
            }
        }
    }

    /* 7 \u{b7} fit the model to counts */
    fn fit_counts(&mut self, can_run: bool) {
        if counts_of(self.p).map_or(&[] as &[Value], |c| c.as_slice()).is_empty() {
            self.out.warnings.push("No counts were loaded, so the model was not fitted to counts.".to_string());
        } else if can_run {
            let mut cal: Vec<(String, Value)> = Vec::new();
            for (k, period) in ["AM", "PM"].into_iter().enumerate() {
                let what = format!("Fitting the {period} peak to counts");
                (self.status)(&format!("{what}\u{2026}"), 0.85 + usize_to_f64(k) * 0.07);
                if let Err(e) = self.fit_period(k, period, &mut cal) {
                    self.out.warnings.push(format!("{what} failed: {e}"));
                }
            }
            self.out.calibration = if cal.is_empty() { None } else { Some(cal) };
        }
    }

    fn fit_period(&mut self, k: usize, period: &str, cal: &mut Vec<(String, Value)>) -> Result<(), String> {
        // can_run held, so the scenario is there
        let sc_id = self.p.scenarios.get(self.sc_idx).map(|sc| sc.id.clone()).unwrap_or_default();
        let date = self.ids.now_iso();
        let status = &mut *self.status;
        let mut prog = |_k: f64, _n: f64, _msg: &str| status(&format!("Fitting the {period} peak to counts\u{2026}"), 0.87 + usize_to_f64(k) * 0.07);
        let res = fit(self.p, &sc_id, &FitOpts { period: Some(period), date: Some(&date), ..Default::default() }, Some(&mut prog))?;
        let kept_prev = res.res.is_none();
        let mut answer = serde_json::to_value(&res).map_err(|e| e.to_string())?;
        null_non_finite(&mut answer);
        // the fitted result itself is the scenario's own; the answer says whether there is one (a fit result serialises as an object)
        if let Some(obj) = answer.as_object_mut() {
            obj.insert("hasRes".to_string(), json!(!kept_prev));
        }
        cal.push((period.to_string(), answer));
        // keep the stored results in step with the fitted matrix
        let stale_or_missing = match self.p.scenarios.get(self.sc_idx).and_then(|sc| sc.res.get()) {
            Some(Value::Object(m)) => m.get(period).is_none_or(|r| !value_truthy(r) || r.get("stale").is_some_and(value_truthy)),
            _ => true,
        };
        if self.opts.run && (kept_prev || stale_or_missing) {
            run_period(self.p, self.sc_idx, period, None);
        }
        self.out.ran = self.out.ran || self.p.scenarios.get(self.sc_idx).and_then(|sc| sc.res.get()).and_then(|stored| stored.get(period)).is_some_and(value_truthy);
        Ok(())
    }

    // Only current results go in the summary; out-of-date ones are flagged instead
    fn summary(&mut self) {
        for period in ["AM", "PM"] {
            let Some(res) = self.p.scenarios.get(self.sc_idx).and_then(|sc| stored_res(sc, period)) else { continue };
            if res.stale {
                self.out.warnings.push(format!("The {period} peak results are out of date (not run in this setup). Run the model to update them."));
                continue;
            }
            self.out.kpi.push((period.to_string(), json!({ "trips": json_number(res.kpi.trips), "spd": json_number(res.kpi.spd), "over": json_number(res.kpi.over), "vht": json_number(res.kpi.vht) })));
            let data = Data::read(self.p);
            data_d(self.p);
            let settings = self.p.settings.clone();
            if let Some(cal) = self.p.scenarios.get(self.sc_idx).and_then(|sc| calibration(sc, Some(&res), period, &settings, &data))
                && cal.n > 0
            {
                let st = cal.stats.as_ref();
                self.out.fit.push((
                    period.to_string(),
                    json!({
                        "n": cal.n,
                        "pass": st.map_or(Value::Null, |x| json_number(x.pass)),
                        "r2": st.and_then(|x| x.r2).map_or(Value::Null, json_number),
                        "ratio": st.map_or(Value::Null, |x| if x.tot_obs > 0.0 { json_number(x.tot_mod / x.tot_obs) } else { Value::Null }),
                    }),
                ));
            }
        }
    }
}

fn null_non_finite(v: &mut Value) {
    match v {
        Value::String(s) if s == "NaN" || s == "Infinity" || s == "-Infinity" => *v = Value::Null,
        Value::Array(a) => a.iter_mut().for_each(null_non_finite),
        Value::Object(m) => m.values_mut().for_each(null_non_finite),
        _ => {}
    }
}

/// A [`Failure`] as the text of its message for a download that has no label (re-exported so hosts can word their own failures the same way).
#[must_use]
pub fn failure_text(f: &Failure, url: &str) -> String {
    f.message(url)
}
