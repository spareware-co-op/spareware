//! Setting up a study area (studyarea.js): roads from OpenStreetMap, gateways, counts and crashes, existing land use as zones, gateway flows from counts, the
//! AM and PM runs and the optional fit to counts, in the current scenario (and, by default with several scenarios, in all of them).
//!
//! Each step is independent: one that fails (a map server that does not answer) is reported in the warnings and the others still run. The downloads go through
//! the host's [`Net`]; nothing here waits or sleeps.

use super::counts::{LoadOpts, counts_of, data_d, load_all};
use super::mapbox::{center, contains, extend, latlng_bounds, pad};
use crate::assign::assign;
use crate::calibrate::{FitOpts, fit, stored_res};
use crate::census::{Bx, Downloads, Failure, Net, host_of};
use crate::datahub::{Data, calibration, gateways_from_counts};
use crate::edit::IdSource;
use crate::edit::history::mark_stale;
use crate::importers::{
    Bounds, OVERPASS, OsmOpts, OverpassOpts, auto_gateways, osm_to_scenario, overpass_query,
};
use crate::jsnum::{
    Maybe, js_trim, json_number, math_max, math_round, number_to_js_string, value_truthy,
};
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
    pub fn to_value(&self) -> Value {
        json!({ "km2": json_number(self.km2), "warn": self.warn, "ok": self.ok, "minutes": json_number(self.minutes) })
    }
}

/// `estimate(bounds)`: the area of a box in km\u{b2}; warns above 36 km\u{b2} (with a rough time) and refuses above 120 km\u{b2}.
pub fn estimate(b: &Bx) -> Estimate {
    let c = center(b);
    let w = dist(&[c[0], b.w], &[c[0], b.e]);
    let h = dist(&[b.s, c[1]], &[b.n, c[1]]);
    let a = w * h / 1e6;
    let km2 = if a < 1.0 {
        math_round(a * 100.0) / 100.0
    } else {
        math_round(a * 10.0) / 10.0
    };
    let mins = math_max(1.0, math_round(a / 12.0));
    let warn = if !(a > 0.0) {
        Some(
            "The study area has no size. Draw a box around the area you want to model.".to_string(),
        )
    } else if a > MAX_KM2 {
        Some(format!(
            "That area is {} km\u{b2}, too large to set up in one go (the limit is {} km\u{b2}). Zoom in or draw a smaller study area.",
            fmt::num(a, 0),
            number_to_js_string(MAX_KM2)
        ))
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
    Estimate {
        km2,
        warn,
        ok: a > 0.0 && a <= MAX_KM2,
        minutes: mins,
    }
}

/// What a setup is asked to do (the `opts` of `setup`): every step on, except service lanes and fitting to counts.
#[derive(Clone, Copy, Debug, PartialEq)]
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
        SetupOpts {
            roads: true,
            local: true,
            service: false,
            gateways: true,
            counts: true,
            crashes: true,
            existing: true,
            run: true,
            fit_counts: false,
            all: None,
        }
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
    pub fn to_value(&self) -> Value {
        let obj = |l: &[(String, Value)]| {
            Value::Object(
                l.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Map<_, _>>(),
            )
        };
        json!({
            "roads": json_number(self.roads), "gateways": json_number(self.gateways), "gwFromCounts": json_number(self.gw_from_counts), "counts": json_number(self.counts), "crashes": json_number(self.crashes),
            "zones": json_number(self.zones), "replaced": json_number(self.replaced), "dwellings": json_number(self.dwellings), "ran": self.ran, "kpi": obj(&self.kpi), "fit": obj(&self.fit),
            "calibration": self.calibration.as_ref().map_or(Value::Null, |c| obj(c)), "scenarios": json_number(self.scenarios), "warnings": self.warnings, "ms": json_number(self.ms),
        })
    }
}

/// `Importers.overpass(b, opts)`: downloads the roads of a box, trying the Overpass servers in turn. Gives the server's answer (`{ elements }`), or the last server's complaint.
pub fn roads_download(
    net: &mut dyn Net,
    b: &Bx,
    opts: &OverpassOpts,
    status: &mut dyn FnMut(&str),
) -> Result<Value, String> {
    let q = overpass_query(
        &Bounds {
            south: b.s,
            west: b.w,
            north: b.n,
            east: b.e,
        },
        opts,
    );
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
    z.kind.is("zone")
        && z.existing.is_true()
        && (z.src.get().and_then(Value::as_str) == Some(SRC)
            || z.name.str_or("").starts_with("Existing \u{b7} "))
}

fn in_bounds(b: &Bx, z: &crate::project::Zone) -> bool {
    let (lat, lng) = (z.lat.num(), z.lng.num());
    lat.is_finite() && lng.is_finite() && contains(b, lat, lng)
}

fn res_object(sc: &mut Scenario) -> &mut Map<String, Value> {
    if !matches!(sc.res, Maybe::Set(Value::Object(_))) {
        sc.res = Maybe::Set(Value::Object(Map::new()));
    }
    match &mut sc.res {
        Maybe::Set(Value::Object(m)) => m,
        _ => unreachable!("made an object"),
    }
}

fn active_index(p: &Project) -> usize {
    p.active
        .as_str()
        .and_then(|id| p.scenarios.iter().position(|s| s.id == id))
        .unwrap_or(0)
}

// `Engine.assign(sc, { period })`, stored as the scenario's result for the period
fn run_period(
    p: &mut Project,
    idx: usize,
    period: &str,
    progress: Option<&mut dyn FnMut(f64, f64, &str)>,
) {
    let opts = AssignOpts {
        period: Some(period.to_string()),
        ..Default::default()
    };
    let s = p.settings.clone();
    let mut adapt = progress;
    let mut cb = |it: u32, gap: f64, msg: &str| {
        if let Some(f) = adapt.as_deref_mut() {
            f(f64::from(it), gap, msg);
        }
    };
    let res = assign(p, &p.scenarios[idx], &s, &opts, Some(&mut cb));
    res_object(&mut p.scenarios[idx]).insert(
        period.to_string(),
        serde_json::to_value(&res).expect("a result serialises"),
    );
}

/// `setup(bounds, opts, onStatus)`: builds a working model of an area in the current scenario. Roads from OpenStreetMap, then gateways, counts and crashes,
/// existing land use zones, gateway flows from counts, the AM and PM runs and (if asked) the fit to counts. Running it again replaces the existing land use
/// inside the new box instead of adding to it.
///
/// `status` is called with what is being done and how far along it is (0 to 1). Fails (nothing changed) when the area is empty or larger than 120 km\u{b2}.
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
    let b = bounds.clone();
    let est = estimate(&b);
    if !est.ok {
        return Err(est.warn.unwrap_or_default());
    }
    let sc_idx = active_index(p);
    let multi = p.scenarios.len() > 1;
    let o = SetupOpts {
        all: Some(opts.all.unwrap_or(multi)),
        ..*opts
    };
    res_object(&mut p.scenarios[sc_idx]);
    let mut out = SetupOut {
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
    let mut status = |t: &str, f: f64| on_status(t, f.clamp(0.0, 1.0));
    let before_n: HashSet<String> = p.scenarios[sc_idx]
        .nodes
        .keys()
        .map(str::to_string)
        .collect();
    let before_l: HashSet<String> = p.scenarios[sc_idx]
        .links
        .keys()
        .map(str::to_string)
        .collect();
    let before_z: HashSet<String> = p.scenarios[sc_idx]
        .zones
        .keys()
        .map(str::to_string)
        .collect();
    let mut removed: Vec<String> = Vec::new();
    let mut changed = false;

    /* 1 \u{b7} roads */
    if o.roads {
        status("Downloading roads from OpenStreetMap\u{2026}", 0.02);
        let r = (|| -> Result<(), String> {
            let data = roads_download(
                net,
                &b,
                &OverpassOpts {
                    service: o.service,
                    local: Some(o.local),
                },
                &mut |t| status(t, 0.05),
            )?;
            status("Building the network\u{2026}", 0.2);
            if data.is_null() {
                return Err("Cannot read properties of null (reading 'elements')".to_string());
            }
            let n = osm_to_scenario(&mut p.scenarios[sc_idx], ids, &data, &OsmOpts::default());
            out.roads = n as f64;
            if n > 0 {
                changed = true;
            }
            if p.scenarios[sc_idx].links.is_empty() {
                out.warnings.push("No roads were found in this area. Check the box covers the roads you want to model.".to_string());
            }
            Ok(())
        })();
        if let Err(e) = r {
            out.warnings
                .push(format!("Downloading roads from OpenStreetMap failed: {e}"));
        }
    }

    // This area's roads: the ones this setup imported, else those around the box (never roads elsewhere)
    let fresh: Vec<String> = p.scenarios[sc_idx]
        .nodes
        .keys()
        .filter(|id| !before_n.contains(*id))
        .map(str::to_string)
        .collect();
    let far = pad(&b, 2.0);
    let local: Vec<String> = if !fresh.is_empty() {
        fresh
    } else {
        p.scenarios[sc_idx]
            .nodes
            .values()
            .filter(|n| contains(&far, n.lat.num(), n.lng.num()))
            .map(|n| n.id.clone())
            .collect()
    };
    let local_set: HashSet<String> = local.iter().cloned().collect();

    /* 2 \u{b7} gateways where main roads leave the area */
    if o.gateways && !p.scenarios[sc_idx].links.is_empty() {
        status("Adding gateways\u{2026}", 0.25);
        let n = auto_gateways(
            &mut p.scenarios[sc_idx],
            ids,
            &Bounds {
                south: b.s,
                west: b.w,
                north: b.n,
                east: b.e,
            },
            Some(&local_set),
        );
        out.gateways = n as f64;
        if n > 0 {
            changed = true;
        }
    }

    /* 3 \u{b7} counts and crashes for the study box */
    let mut which: Vec<&str> = Vec::new();
    if o.counts {
        which.extend(["nzta", "at"]);
    }
    if o.crashes {
        which.push("cas");
    }
    if !which.is_empty() {
        status("Downloading counts and crashes\u{2026}", 0.3);
        let r = (|| -> Result<(), String> {
            let mut area = latlng_bounds(&[[b.s, b.w], [b.n, b.e]]);
            for id in &local {
                if let Some(n) = p.scenarios[sc_idx].nodes.get(id) {
                    extend(&mut area, n.lat.num(), n.lng.num());
                }
            }
            let msgs = load_all(
                p,
                ids,
                net,
                &pad(&area, 0.05),
                &which,
                &mut |t| status(t, 0.35),
                &LoadOpts::default(),
            )?;
            let failed: Vec<&str> = which
                .iter()
                .copied()
                .filter(|w| {
                    msgs.iter()
                        .any(|m| m.starts_with(&format!("{} failed", w.to_uppercase())))
                })
                .collect();
            for m in &msgs {
                if m.contains(" failed ") {
                    out.warnings.push(format!("Data download: {m}"));
                }
            }
            let d_counts = counts_of(p);
            out.counts = d_counts
                .iter()
                .filter(|c| {
                    c.get("src")
                        .and_then(Value::as_str)
                        .is_some_and(|s| which.contains(&s))
                        && !failed
                            .iter()
                            .any(|f| c.get("src").and_then(Value::as_str) == Some(*f))
                })
                .count() as f64;
            let cas_ok = which.contains(&"cas") && !failed.contains(&"cas");
            out.crashes = if cas_ok {
                super::counts::crashes_of(p).len() as f64
            } else {
                0.0
            };
            Ok(())
        })();
        if let Err(e) = r {
            out.warnings
                .push(format!("Downloading counts and crashes failed: {e}"));
        }
    }

    /* 4 \u{b7} existing land use as zones: replaces what an earlier setup made inside this box */
    if o.existing {
        status("Downloading buildings and land use\u{2026}", 0.5);
        let r = (|| -> Result<(), String> {
            let els = crate::census::existing_land_use_download(net, &mut dl.good, &b, &mut |t| {
                status(t, 0.55)
            })?;
            status("Grouping buildings into zones\u{2026}", 0.65);
            let sbox = StudyBox {
                s: b.s,
                w: b.w,
                n: b.n,
                e: b.e,
            };
            let built = landuse::build(
                &Value::Array(els),
                Some(&sbox),
                ids,
                &BuildOpts {
                    cell: None,
                    links: landuse::road_lines(&p.scenarios[sc_idx]),
                },
            );
            let sc = &mut p.scenarios[sc_idx];
            let doomed: Vec<String> = sc
                .zones
                .iter()
                .filter(|(_, z)| from_lu(z) && in_bounds(&b, z))
                .map(|(k, _)| k.to_string())
                .collect();
            for id in doomed {
                sc.zones.remove(&id);
                removed.push(id);
            }
            out.replaced = removed.len() as f64;
            for mut z in built.zones {
                if !(z.lat.num().is_finite() && z.lng.num().is_finite()) {
                    continue;
                }
                let nm = js_trim(z.name.str_or("")).to_lowercase();
                let zl = [z.lat.num(), z.lng.num()];
                let dup = sc.zones.values().any(|y| {
                    !y.kind.is("gateway")
                        && js_trim(y.name.str_or("")).to_lowercase() == nm
                        && dist(&[y.lat.num(), y.lng.num()], &zl) < SAME_ZONE_M
                });
                if dup {
                    continue;
                }
                let id = if !z.id.is_empty() && !sc.zones.contains_key(&z.id) {
                    z.id.clone()
                } else {
                    ids.uid("z")
                };
                z.id = id.clone();
                if z.connectors.is_missing() {
                    z.connectors = Maybe::Null;
                }
                if !z.lu.is_set() {
                    z.lu = Maybe::Set(crate::project::OrderedMap::new());
                }
                z.kind = Maybe::Set("zone".to_string());
                z.existing = true.into();
                z.src = Maybe::Set(json!(SRC));
                out.zones += 1.0;
                let lu = z.lu.get().expect("lu");
                out.dwellings += crate::jsnum::or_default(lu.num_at("residential").num(), 0.0)
                    + crate::jsnum::or_default(lu.num_at("apartment").num(), 0.0);
                sc.zones.insert(id, z);
            }
            out.dwellings = math_round(out.dwellings);
            if out.zones > 0.0 || !removed.is_empty() {
                changed = true;
            }
            let me_hit = !removed.is_empty()
                && p.me.get().is_some_and(|me| {
                    ["AM", "PM"].iter().any(|per| {
                        me.factors.get(*per).and_then(|f| f.get()).is_some_and(|f| {
                            f.keys().any(|k| {
                                removed.iter().any(|id| k.split('>').any(|part| part == id))
                            })
                        })
                    })
                });
            if me_hit {
                out.warnings.push("Existing land use was replaced, so matrix estimation factors for the old zones no longer apply. Fit to counts again.".to_string());
            }
            Ok(())
        })();
        if let Err(e) = r {
            out.warnings
                .push(format!("Downloading buildings and land use failed: {e}"));
        }
    }

    // Other scenarios get the same new roads, gateways and zones, and lose the replaced zones
    let others: Vec<usize> = if o.all == Some(true) {
        (0..p.scenarios.len()).filter(|i| *i != sc_idx).collect()
    } else {
        Vec::new()
    };
    if !others.is_empty() && changed {
        let new_nodes: Vec<_> = p.scenarios[sc_idx]
            .nodes
            .iter()
            .filter(|(k, _)| !before_n.contains(*k))
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let new_links: Vec<_> = p.scenarios[sc_idx]
            .links
            .iter()
            .filter(|(k, _)| !before_l.contains(*k))
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let new_zones: Vec<_> = p.scenarios[sc_idx]
            .zones
            .iter()
            .filter(|(k, _)| !before_z.contains(*k))
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for &si in &others {
            let s = &mut p.scenarios[si];
            for id in &removed {
                if s.zones.get(id).is_some_and(from_lu) {
                    s.zones.remove(id);
                }
            }
            for (k, v) in &new_nodes {
                if !s.nodes.contains_key(k) {
                    s.nodes.insert(k.clone(), v.clone());
                }
            }
            for (k, v) in &new_links {
                if !s.links.contains_key(k) {
                    s.links.insert(k.clone(), v.clone());
                }
            }
            for (k, v) in &new_zones {
                if !s.zones.contains_key(k) {
                    s.zones.insert(k.clone(), v.clone());
                }
            }
            if let Maybe::Set(Value::Object(res)) = &mut s.res {
                for r in res.values_mut() {
                    if value_truthy(r) {
                        if let Some(m) = r.as_object_mut() {
                            m.insert("stale".to_string(), json!(true));
                        }
                    }
                }
            }
        }
        out.scenarios += others.len() as f64;
    } else if multi && changed {
        out.warnings.push(
            "Only the current scenario was set up; the other scenarios are unchanged.".to_string(),
        );
    }
    // Count-to-road matches follow the network, so the results are marked out of date
    if changed {
        mark_stale(&mut p.scenarios[sc_idx]);
    }

    /* 5 \u{b7} gateway flows from counts (every scenario that got the new network) */
    let has_gw = p.scenarios[sc_idx]
        .zones
        .values()
        .any(|z| z.kind.is("gateway"));
    if has_gw && !counts_of(p).is_empty() {
        status("Setting gateway flows from counts\u{2026}", 0.7);
        let s = p.settings.clone();
        let data = Data::read(p);
        let n = gateways_from_counts(&mut p.scenarios[sc_idx], &s, &data);
        out.gw_from_counts = n as f64;
        for &si in &others {
            let data = Data::read(p);
            gateways_from_counts(&mut p.scenarios[si], &s, &data);
        }
        if n > 0 {
            mark_stale(&mut p.scenarios[sc_idx]);
        }
    }

    /* 6 \u{b7} run the model */
    let can_run = !p.scenarios[sc_idx].links.is_empty() && !p.scenarios[sc_idx].zones.is_empty();
    if o.run && !can_run {
        out.warnings.push(
            if !p.scenarios[sc_idx].links.is_empty() {
                "There are no zones or gateways, so the model was not run."
            } else {
                "There are no roads, so the model was not run."
            }
            .to_string(),
        );
    }
    // Fitting to counts runs the model itself, so the plain runs are only needed without it
    let fit_on = o.fit_counts && !counts_of(p).is_empty() && can_run;
    if o.run && can_run && !fit_on {
        for (k, period) in ["AM", "PM"].iter().enumerate() {
            let what = format!("Running the {period} peak");
            status(&format!("{what}\u{2026}"), 0.75 + k as f64 * 0.08);
            let mut prog = |it: f64, _gap: f64, msg: &str| {
                let text = if msg.is_empty() {
                    format!("iteration {}", number_to_js_string(it))
                } else {
                    msg.to_string()
                };
                status(
                    &format!("{period} peak \u{b7} {text}"),
                    0.75 + k as f64 * 0.08 + 0.06,
                );
            };
            run_period(p, sc_idx, period, Some(&mut prog));
            out.ran = true;
        }
    }

    /* 7 \u{b7} fit the model to counts */
    if o.fit_counts {
        if counts_of(p).is_empty() {
            out.warnings
                .push("No counts were loaded, so the model was not fitted to counts.".to_string());
        } else if can_run {
            let mut cal: Vec<(String, Value)> = Vec::new();
            for (k, period) in ["AM", "PM"].iter().enumerate() {
                let what = format!("Fitting the {period} peak to counts");
                status(&format!("{what}\u{2026}"), 0.85 + k as f64 * 0.07);
                let sc_id = p.scenarios[sc_idx].id.clone();
                let date = ids.now_iso();
                let r = (|| -> Result<(), String> {
                    let mut prog = |_k: f64, _n: f64, _msg: &str| {
                        status(
                            &format!("Fitting the {period} peak to counts\u{2026}"),
                            0.87 + k as f64 * 0.07,
                        )
                    };
                    let res = fit(
                        p,
                        &sc_id,
                        &FitOpts {
                            period: Some(period),
                            date: Some(&date),
                            ..Default::default()
                        },
                        Some(&mut prog),
                    )?;
                    let kept_prev = res.res.is_none();
                    let mut v = serde_json::to_value(&res).map_err(|e| e.to_string())?;
                    null_non_finite(&mut v);
                    // the fitted result itself is the scenario's own; the answer says whether there is one
                    v["hasRes"] = json!(!kept_prev);
                    cal.push((period.to_string(), v));
                    // keep the stored results in step with the fitted matrix
                    let stale_or_missing = match p.scenarios[sc_idx].res.get() {
                        Some(Value::Object(m)) => m.get(*period).is_none_or(|r| {
                            !value_truthy(r) || r.get("stale").is_some_and(value_truthy)
                        }),
                        _ => true,
                    };
                    if o.run && (kept_prev || stale_or_missing) {
                        run_period(p, sc_idx, period, None);
                    }
                    out.ran = out.ran
                        || p.scenarios[sc_idx]
                            .res
                            .get()
                            .and_then(|m| m.get(*period))
                            .is_some_and(value_truthy);
                    Ok(())
                })();
                if let Err(e) = r {
                    out.warnings.push(format!("{what} failed: {e}"));
                }
            }
            out.calibration = if cal.is_empty() { None } else { Some(cal) };
        }
    }

    // Only current results go in the summary; out-of-date ones are flagged instead
    for period in ["AM", "PM"] {
        let Some(r) = stored_res(&p.scenarios[sc_idx], period) else {
            continue;
        };
        if r.stale {
            out.warnings.push(format!("The {period} peak results are out of date (not run in this setup). Run the model to update them."));
            continue;
        }
        out.kpi.push((period.to_string(), json!({ "trips": json_number(r.kpi.trips), "spd": json_number(r.kpi.spd), "over": json_number(r.kpi.over), "vht": json_number(r.kpi.vht) })));
        let data = Data::read(p);
        data_d(p);
        let s = p.settings.clone();
        if let Some(c) = calibration(&p.scenarios[sc_idx], Some(&r), period, &s, &data)
            && c.n > 0
        {
            let st = c.stats.as_ref();
            out.fit.push((
                period.to_string(),
                json!({
                    "n": c.n,
                    "pass": st.map_or(Value::Null, |x| json_number(x.pass)),
                    "r2": st.and_then(|x| x.r2).map_or(Value::Null, json_number),
                    "ratio": st.map_or(Value::Null, |x| if x.tot_obs > 0.0 { json_number(x.tot_mod / x.tot_obs) } else { Value::Null }),
                }),
            ));
        }
    }
    status("Study area ready", 1.0);
    Ok(out)
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
pub fn failure_text(f: &Failure, url: &str) -> String {
    f.message(url)
}
