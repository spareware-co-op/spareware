//! Building the Drury sample project (`Sample.build`): roads, counts and crashes, existing land use, gateway flows, the official feeds, lane coding, committed
//! growth, the proposal, Option A, calibration data, the assessment set, count roles, the regulatory context and the notes on what is real and what is illustrative.

use super::analyse::{AnalyseOpts, analyse};
use super::helpers::{CsvOpts, clip_ways, code_lanes, option_a, option_b, prune_stubs, simplify, site_with, turn_count_csv};
use super::hooks::{Hazards, SampleHooks};
use super::{
    BASE_YEAR, BOUNDS, COMMITTED, DESIGN_YEAR, DROP_HIGHWAY, Failed, GROWTH, ILLUSTRATIVE, JUSTIFY, LODGEMENT, Modules, PERIODS, PROPOSAL, SAMPLE_SRC, SampleError, ZONE_CELL,
    area, area_value, tg,
};
use crate::calibrate::{CalibStore, hold_out_default, import_turn_csv, match_sites};
use crate::census::{Bx, Downloads, FillOpts, FillReport, LastFill, Net, existing_land_use_download, fill_all, set_table, sync};
use crate::datahub::{Data, gateways_from_counts};
use crate::edit::history::mark_stale;
use crate::edit::register::{PRESET_AS_AT, presets};
use crate::edit::scenarios::{CreateSetOpts, create_set};
use crate::edit::sites::{DetectOpts, detect};
use crate::edit::{IdSource, new_project};
use crate::importers::{Bounds, OsmOpts, OverpassOpts, auto_gateways, osm_to_scenario};
use crate::jsnum::{JsNum, Maybe, Num, json_number, math_round, number_to_js_string, value_to_js_string, value_truthy};
use crate::landuse::{self, BuildOpts as LuBuildOpts, StudyBox};
use crate::project::{Project, RegisterRow, TmdgSettings, Zone, land_use};
use crate::studyarea::{LoadOpts, counts_of, crashes_of, latlng_bounds, load_all, roads_download};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// What a build is asked to do (`opts` of `Sample.build`).
#[derive(Clone, Copy)]
pub struct BuildOpts<'a> {
    /// False leaves the bus timetable out (`opts.gtfs === false`); the default loads it when the feeds are there.
    pub gtfs: Option<bool>,
    /// Also run the model, matrix estimation and the report's tests (`opts.run === true`).
    pub run: bool,
    /// With `run`, false leaves out the report's staging and sensitivity tests (`opts.extras === false`).
    pub extras: Option<bool>,
    /// Which optional modules are loaded.
    pub modules: Modules,
    /// Says true when the build is cancelled; it is looked at between steps.
    pub cancel: Option<&'a dyn Fn() -> bool>,
}

impl Default for BuildOpts<'_> {
    fn default() -> Self {
        BuildOpts { gtfs: None, run: false, extras: None, modules: Modules::default(), cancel: None }
    }
}

/// The ids of the four assessment scenarios.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleScenarios {
    /// S0, the base year (existing).
    pub s0: String,
    /// S1, the do-minimum.
    pub s1: String,
    /// S2, with the proposal (Option A).
    pub s2: String,
    /// S3, with mitigation (Option B).
    pub s3: String,
}

/// What a build made (`Sample.build`'s answer).
pub struct Built {
    /// The new project (the caller opens it).
    pub project: Project,
    /// What was built and what is real or illustrative, in order.
    pub notes: Vec<String>,
    /// The sources that did not answer.
    pub failed: Vec<Failed>,
    /// How long it took (ms).
    pub ms: f64,
    /// The assessment scenarios.
    pub scenarios: SampleScenarios,
}

/// `fail(id, label, why)`: names a source that did not answer, once.
pub(crate) fn fail(failed: &mut Vec<Failed>, id: &str, label: &str, why: &str) {
    if failed.iter().any(|f| f.id == id) {
        return;
    }
    failed.push(Failed { id: id.to_string(), label: label.to_string(), why: if why.is_empty() { "no answer".to_string() } else { why.to_string() } });
}

// `stop(signal)`: a cancel ends the build at the next step
fn stop(cancel: Option<&dyn Fn() -> bool>) -> Result<(), SampleError> {
    if cancel.is_some_and(|c| c()) { Err(SampleError::Cancelled) } else { Ok(()) }
}

/// One optional step: a failure is recorded under its label and the build carries on (a cancel always stops it).
#[allow(clippy::too_many_arguments)]
fn soft<T>(
    failed: &mut Vec<Failed>,
    cancel: Option<&dyn Fn() -> bool>,
    status: &mut dyn FnMut(&str),
    id: &str,
    label: &str,
    doing: Option<&str>,
    f: impl FnOnce(&mut dyn FnMut(&str)) -> Result<T, SampleError>,
) -> Result<Option<T>, SampleError> {
    stop(cancel)?;
    status(&format!("{}\u{2026}", doing.map_or_else(|| format!("Downloading {label}"), str::to_string)));
    match f(status) {
        Ok(v) => Ok(Some(v)),
        Err(SampleError::Cancelled) if cancel.is_some_and(|c| c()) => Err(SampleError::Cancelled),
        Err(e) => {
            fail(failed, id, label, &e.message());
            Ok(None)
        }
    }
}

// `text.replace(/^\w+ failed \((.*)\)$/, '$1')`
fn strip_failed(m: &str) -> String {
    let end = m.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(m.len());
    if end > 0
        && let Some(inner) = m[end..].strip_prefix(" failed (").and_then(|r| r.strip_suffix(')'))
        && !inner.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    {
        return inner.to_string();
    }
    m.to_string()
}

// `warnings.find(x => re.test(x)) || 'no answer'` for a case-insensitive list of words
fn why(w: &[String], words: &[&str]) -> String {
    w.iter().find(|x| words.iter().any(|k| x.to_lowercase().contains(k))).cloned().unwrap_or_else(|| "no answer".to_string())
}

fn text(t: &str) -> Num {
    Maybe::Set(JsNum::Text(t.to_string()))
}

fn zone_of(v: Value) -> Zone {
    serde_json::from_value(v).expect("a zone")
}

fn lu_src(lu: &[(&str, f64)], today: &str) -> Value {
    Value::Object(lu.iter().map(|(k, q)| ((*k).to_string(), json!({ "source": SAMPLE_SRC, "basis": "assumed", "date": today, "q": json_number(*q) }))).collect::<Map<_, _>>())
}

fn lu_value(lu: &[(&str, f64)]) -> Value {
    Value::Object(lu.iter().map(|(k, q)| ((*k).to_string(), json_number(*q))).collect::<Map<_, _>>())
}

fn qty(lu: &[(&str, f64)], k: &str) -> f64 {
    lu.iter().find(|(x, _)| *x == k).map_or(0.0, |(_, q)| *q)
}

fn bounds_of(b: &Bx) -> Bounds {
    Bounds { south: b.s, west: b.w, north: b.n, east: b.e }
}

/// `Sample.build(opts)`: downloads the Drury data and builds the sample project and its assessment set. Throws (here: gives an error) when the roads cannot be
/// downloaded (stage "roads") or on cancel. A source that does not answer is left out and named in `failed`.
pub fn build(
    ids: &mut dyn IdSource,
    net: &mut dyn Net,
    dl: &mut Downloads,
    hooks: &mut dyn SampleHooks,
    opts: &BuildOpts,
    on_status: &mut dyn FnMut(&str),
) -> Result<Built, SampleError> {
    let t0 = ids.now_ms();
    let cancel = opts.cancel;
    let m = opts.modules;
    let mut notes: Vec<String> = Vec::new();
    let mut failed: Vec<Failed> = Vec::new();
    let mut status = |t: &str| on_status(t);
    let mut p = new_project(ids, "Drury East plan change (sample)");
    p.settings.base_year = Num::number(BASE_YEAR);
    p.settings.year = Num::number(BASE_YEAR);
    p.settings.growth = Num::number(GROWTH);
    p.settings.growth_mode = Maybe::Set("linear".to_string());
    p.settings.period = Maybe::Set("AM".to_string());
    p.settings.tmdg = Maybe::Set(TmdgSettings { category: text("E"), justification: text(JUSTIFY), extra: Map::new() });
    p.view = Maybe::Set(json!({ "c": [-37.1012, 174.9605], "z": 15 }));
    let bounds = latlng_bounds(&[BOUNDS[0], BOUNDS[1]]);
    let src_id = p.scenarios[0].id.clone();
    p.scenarios[0].name = Maybe::Set("Sample source network".to_string());
    let si = 0usize;
    let area_box = area();

    /* 1 \u{b7} roads (the one step the sample cannot do without) */
    stop(cancel)?;
    status("Downloading Drury streets from OpenStreetMap\u{2026}");
    let data = roads_download(net, &bounds, &OverpassOpts { service: false, local: Some(true) }, &mut status).map_err(|e| SampleError::Failed {
        message: format!("Could not download the sample streets from OpenStreetMap ({e}). Check your connection and try again."),
        stage: Some("roads".to_string()),
    })?;
    status("Building the network\u{2026}");
    // Residential streets and living streets are left out: zones load onto the nearest through road, which keeps the sample small (the saved project under 3 MB)
    // and quick to run. Motorways down to rural (unclassified) roads stay.
    let all: Vec<Value> = data.get("elements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut local: HashSet<String> = HashSet::new();
    let id_key = |v: Option<&Value>| match v {
        Some(Value::Number(n)) => format!("n{}", number_to_js_string(n.as_f64().unwrap_or(f64::NAN))),
        Some(Value::String(s)) => format!("s{s}"),
        other => format!("o{}", other.map_or("undefined".to_string(), Value::to_string)),
    };
    for e in &all {
        if e.get("type").and_then(Value::as_str) == Some("way") {
            let hw = e.get("tags").and_then(|t| t.get("highway")).filter(|h| value_truthy(h)).map(value_to_js_string).unwrap_or_default();
            if DROP_HIGHWAY.contains(&hw.as_str()) {
                local.insert(id_key(e.get("id")));
            }
        }
    }
    // Roads are cut where they leave the study area and its margin, so the network, counts and crashes cover one area
    let kept: Vec<Value> = all.iter().filter(|e| !(e.get("type").and_then(Value::as_str) == Some("way") && local.contains(&id_key(e.get("id"))))).cloned().collect();
    let clipped = clip_ways(&kept, &area_box);
    let mut d2 = data.as_object().cloned().unwrap_or_default();
    // The new nodes on the edge have text ids ("edge1"); the road importer reads numeric ids only, so they go in as numbers and come back as text on the junctions made
    let mut els = clipped.elements;
    let edge_num = |s: &str| -> Option<f64> { s.strip_prefix("edge").and_then(|n| n.parse::<f64>().ok()).filter(|n| n.fract() == 0.0).map(|n| -1e12 - n) };
    let numbered = |v: &Value| -> Option<Value> { v.as_str().and_then(edge_num).map(|n| json_number(n)) };
    for e in els.iter_mut() {
        if let Some(n) = e.get("id").and_then(numbered) {
            e["id"] = n;
        }
        if let Some(a) = e.get_mut("nodes").and_then(Value::as_array_mut) {
            for x in a.iter_mut() {
                if let Some(n) = numbered(x) {
                    *x = n;
                }
            }
        }
    }
    d2.insert("elements".into(), Value::Array(els));
    osm_to_scenario(&mut p.scenarios[si], ids, &Value::Object(d2), &OsmOpts { sites: true, ..Default::default() });
    for n in p.scenarios[si].nodes.values_mut() {
        if let Some(x) = n.osm.get().and_then(Value::as_f64).filter(|x| *x < -1e12 + 0.5 && x.fract() == 0.0) {
            n.osm = Maybe::Set(Value::String(format!("edge{}", number_to_js_string(-1e12 - x))));
        }
    }
    let thin = simplify(&mut p.scenarios[si], 2.0);
    notes.push(format!(
        "OpenStreetMap roads from motorways down to rural roads, cut at the study area plus a margin ({} roads cut at the edge); {} residential and living streets were left out to keep the sample small, so zones load onto the nearest through road. Road shapes are simplified to within 2 m ({} shape points dropped).",
        clipped.cut,
        local.len(),
        thin
    ));
    if p.scenarios[si].links.len() < 50 {
        return Err(SampleError::Failed {
            message: "Could not download the sample streets: OpenStreetMap sent back no roads for Drury. Check your connection and try again.".to_string(),
            stage: Some("roads".to_string()),
        });
    }
    if p.scenarios[si].sites.get().is_none_or(|s| s.is_empty()) {
        detect(&mut p.scenarios[si], ids, &DetectOpts::default());
    }
    auto_gateways(&mut p.scenarios[si], ids, &bounds_of(&bounds), None);
    mark_stale(&mut p.scenarios[si]);

    /* 2 \u{b7} counts and crashes (one source at a time, so each can fail on its own), for the area the roads were cut to */
    let near = latlng_bounds(&[[area_box.s, area_box.w], [area_box.n, area_box.e]]);
    for (w, label) in [("nzta", "NZTA state highway counts"), ("at", "Auckland Transport counts"), ("cas", "NZTA crash records (CAS)")] {
        stop(cancel)?;
        let msgs = load_all(&mut p, &*ids, net, &near, &[w], &mut status, &LoadOpts::default()).map_err(|e| if stop(cancel).is_err() { SampleError::Cancelled } else { SampleError::msg(e) })?;
        if let Some(bad) = msgs.iter().find(|x| x.contains(" failed ")) {
            fail(&mut failed, w, label, &strip_failed(bad));
        }
    }

    /* 3 \u{b7} existing land use: OpenStreetMap buildings, then Census, employees, schools and zoning */
    let mut els: Option<Vec<Value>> = None;
    if m.elu {
        els = soft(&mut failed, cancel, &mut status, "osm-buildings", "OpenStreetMap buildings and land-use areas", None, |st| {
            existing_land_use_download(net, &mut dl.good, &bounds, st).map_err(SampleError::msg)
        })?;
    } else {
        fail(&mut failed, "osm-buildings", "OpenStreetMap buildings", "the existing land use module is not loaded");
    }
    if let Some(e) = els.as_ref().filter(|e| !e.is_empty()) {
        status("Grouping buildings into zones\u{2026}");
        let sbox = StudyBox { s: bounds.s, w: bounds.w, n: bounds.n, e: bounds.e };
        let built = landuse::build(&Value::Array(e.clone()), Some(&sbox), ids, &LuBuildOpts { cell: Some(ZONE_CELL), links: landuse::road_lines(&p.scenarios[si]) });
        for mut z in built.zones {
            if z.connectors.is_missing() {
                z.connectors = Maybe::Null;
            }
            if !z.lu.is_set() {
                z.lu = Maybe::Set(crate::project::OrderedMap::new());
            }
            z.kind = Maybe::Set("zone".to_string());
            z.existing = true.into();
            z.src = Maybe::Set(json!("existing-lu"));
            p.scenarios[si].zones.insert(z.id.clone(), z);
        }
    }
    let mut fill: Option<FillReport> = None;
    if m.lp {
        stop(cancel)?;
        status("Downloading Census 2023, employees, schools and zoning\u{2026}");
        let o = FillOpts { bounds: Some(bounds.clone()), elements: Some(els.clone().unwrap_or_default()), ..Default::default() };
        let mut last = LastFill::default();
        match fill_all(&mut p, si, ids, net, dl, &mut last, &o, &mut status) {
            Ok(r) => fill = Some(r),
            Err(e) => fail(&mut failed, "census", "Census, employees, schools and zoning", &e),
        }
        if let Some(f) = &fill {
            let w = &f.warnings;
            if f.census.is_none() {
                fail(&mut failed, "census", "Stats NZ Census 2023 (SA1)", &why(w, &["census", "sa1", "2023"]));
            }
            if f.employees.is_none() {
                fail(&mut failed, "employees", "Stats NZ employees by SA2", &why(w, &["employ", "sa2"]));
            }
            if !f.schools.as_ref().is_some_and(|s| s.loaded_schools) {
                fail(&mut failed, "moe-schools", "Ministry of Education schools", &why(w, &["school", "education", "catalogue"]));
            }
            if !f.schools.as_ref().is_some_and(|s| s.loaded_ece) {
                fail(&mut failed, "moe-ece", "Ministry of Education early learning services", &why(w, &["early", "ece", "education", "catalogue"]));
            }
            if f.aup.is_none() {
                fail(&mut failed, "aup", "Auckland Unitary Plan zones", &why(w, &["unitary", "aup", "zone"]));
            }
        }
    } else {
        fail(&mut failed, "census", "Census, employees, schools and zoning", "the land use module is not loaded");
    }
    let existing_n = p.scenarios[si].zones.values().filter(|z| !z.kind.is("gateway") && z.existing.is_true()).count();
    if existing_n == 0 {
        notes.push("No existing land use could be built (OpenStreetMap buildings and the Census did not load), so the base year has gateway traffic only.".to_string());
    }
    mark_stale(&mut p.scenarios[si]);

    /* 4 \u{b7} gateway flows from the counts */
    let n_gw = {
        let s = p.settings.clone();
        let data = Data::read(&p);
        gateways_from_counts(&mut p.scenarios[si], &s, &data)
    };
    mark_stale(&mut p.scenarios[si]);

    /* 5 \u{b7} official feeds: legal speed limits, TMS daily counts, flood hazards, bus timetables */
    let mut hazards: Option<Hazards> = None;
    if m.fe {
        soft(&mut failed, cancel, &mut status, "nslr", "NZTA legal speed limits (National Speed Limit Register)", None, |st| hooks.nslr_all(&mut p, &near, si, st))?;
        if counts_of(&mut p).iter().any(|c| c.get("src").and_then(Value::as_str) == Some("nzta")) {
            soft(&mut failed, cancel, &mut status, "tms", "NZTA TMS daily counts", None, |st| hooks.tms_load(&mut p, st))?;
        }
        hazards = soft(&mut failed, cancel, &mut status, "flood", "Auckland Council flood plains and coastal inundation areas", None, |st| {
            let r = hooks.hazards_load(&mut p, &near, st)?;
            if r.layers.is_empty() {
                return Err(SampleError::msg(r.errors.first().filter(|e| !e.is_empty()).cloned().unwrap_or_else(|| "no hazard layer answered".to_string())));
            }
            Ok(r)
        })?;
        if opts.gtfs != Some(false) {
            soft(&mut failed, cancel, &mut status, "gtfs", "Auckland Transport bus timetable (GTFS)", None, |st| {
                let Some(feed) = hooks.gtfs_first_feed(&near)? else { return Err(SampleError::msg("no bus timetable covers Drury")) };
                hooks.gtfs_apply(&mut p, &feed, &near, si, st)
            })?;
        }
    }

    /* 6 \u{b7} junction sites with lane coding (illustrative), before the scenarios are copied */
    struct Coded {
        label: &'static str,
        site: String,
    }
    let mut sites: Vec<Coded> = Vec::new();
    let coded: [(&[&str], [f64; 2], &str); 2] = [
        (&["Great South", "Waihoehoe"], [-37.1035, 174.9500], "Great South Road / Waihoehoe Road roundabout"),
        (&["Waihoehoe", "Fitzgerald"], [-37.1042, 174.9588], "Waihoehoe Road / Fitzgerald Road"),
    ];
    for (pats, ll, label) in coded {
        match site_with(&p.scenarios[si], pats, ll) {
            Some(hit) if !(crate::util::dist(&hit.c, &ll) > 800.0) => {
                if m.jn {
                    code_lanes(&mut p.scenarios[si], &hit.site, hooks);
                }
                sites.push(Coded { label, site: hit.site });
            }
            _ => notes.push(format!("The junction site {label} was not found in the downloaded roads, so it has no lane coding.")),
        }
    }

    /* 7 \u{b7} committed growth (register rows), the proposal and Option A */
    let today = ids.today();
    let mut register: Vec<RegisterRow> = presets(PRESET_AS_AT).into_iter().filter(|r| !r.id.is("preset:drury-metro")).collect();
    for c in COMMITTED {
        let id = ids.uid("z");
        let z = zone_of(json!({
            "id": id, "kind": "zone", "role": "committed", "reg": c.reg, "name": c.name, "lat": c.ll[0], "lng": c.ll[1], "lu": lu_value(c.lu), "connectors": null,
            "year": c.year, "buildout": c.buildout, "luSrc": lu_src(c.lu, &today),
        }));
        p.scenarios[si].zones.insert(id.clone(), z);
        let yield_text = c.lu.iter().map(|(k, q)| format!("{} {}", number_to_js_string(*q), land_use(k).map_or(*k, |l| l.unit))).collect::<Vec<_>>().join(", ");
        let row: RegisterRow = serde_json::from_value(json!({
            "id": c.reg, "ref": format!("AUP {}", c.precinct),
            "name": format!("{} precinct ({}): committed growth to {} (illustrative yield and staging)", if c.precinct == "I450" { "Drury Centre" } else { "Drury East" }, c.precinct, number_to_js_string(DESIGN_YEAR)),
            "type": "precinct", "yield": yield_text, "rates": null, "status": format!("Operative precinct (AUP {})", c.precinct), "likely": "likely", "funding": null,
            "source": SAMPLE_SRC, "date": today, "asAt": today, "verify": true, "preset": false, "zones": [id], "links": [],
            "note": format!("Sample yield; {}% built by {}, first homes {}. Check the precinct provisions and the developer's programme.", number_to_js_string(c.buildout), number_to_js_string(DESIGN_YEAR), number_to_js_string(c.year)),
        }))
        .expect("a register row");
        register.push(row);
    }
    p.register = Maybe::Set(register);
    let mut tg_zone: Option<String> = None;
    for d in PROPOSAL {
        let id = ids.uid("z");
        let z = zone_of(json!({
            "id": id, "kind": "zone", "role": "development", "name": d.name, "lat": d.ll[0], "lng": d.ll[1], "lu": lu_value(d.lu), "connectors": null, "year": d.year,
            "luSrc": lu_src(d.lu, &today),
        }));
        p.scenarios[si].zones.insert(id.clone(), z);
        if d.tg {
            tg_zone = Some(id);
        }
    }
    if let (Some(tid), true) = (&tg_zone, m.lp) {
        let s = p.settings.clone();
        let mut z = p.scenarios[si].zones.get(tid).cloned().expect("the zone");
        set_table(&mut z, true, &s, &p.scenarios[si]);
        if let (Some(Value::Object(o)), Value::Object(t)) = (z.tg.get_mut(), tg()) {
            for (k, v) in t {
                o.insert(k, v);
            }
        }
        sync(&mut z, &s, &p.scenarios[si]);
        p.scenarios[si].zones.insert(tid.clone(), z);
    }
    let opt_a = option_a(&mut p.scenarios[si], ids);
    if opt_a.len() < 2 {
        notes.push("Option A could not find all of Takotowhenua Avenue, Appleby Road and Sutton Road in the downloaded roads, so it has fewer new links.".to_string());
    }
    mark_stale(&mut p.scenarios[si]);

    /* 8 \u{b7} calibration data: one screenline and one illustrative turning count */
    if m.cal {
        let mut st = CalibStore::read(&p);
        st.screenlines.push(
            serde_json::from_value(json!({
                "id": ids.uid("sl"), "name": "North screenline: Great South Road, SH1 and Parkhaven Drive", "type": "screen", "role": "calib", "motorways": true,
                "pts": [[-37.0870, 174.930], [-37.0870, 174.955]],
                "src": { "source": "Spare Construct sample: drawn across Great South Road and the Southern Motorway north of Drury", "basis": "user", "date": today },
            }))
            .expect("a screenline"),
        );
        st.write(&mut p);
        if let Some(g) = sites.iter().find(|s| s.label.contains("Great South")) {
            let site = p.scenarios[si].sites.get().and_then(|m| m.get(&g.site)).cloned().expect("the site");
            let t = turn_count_csv(&p.scenarios[si], &site, &CsvOpts { name: Some("Great South Road / Waihoehoe Road roundabout".to_string()), date: None });
            match import_turn_csv(&mut p, &t.csv, "drury-turning-count-ILLUSTRATIVE.csv", ids) {
                Ok(r) => {
                    let mut st = CalibStore::read(&p);
                    for s in &r.sites {
                        let Some(x) = st.turn_sites.iter_mut().find(|x| x.reference == s.reference && x.date == s.date) else { continue };
                        x.role = text("off");
                        x.extra.insert("illustrative".into(), json!(true));
                        let mut flags = vec![ILLUSTRATIVE.to_string(), "role Not used: made up for the sample, so it never decides a validation result".to_string()];
                        flags.extend(x.flags.iter().filter(|f| f.as_str() != ILLUSTRATIVE).cloned());
                        x.flags = flags;
                        x.file = text(&format!("{ILLUSTRATIVE} (drury-turning-count-ILLUSTRATIVE.csv)"));
                    }
                    st.write(&mut p);
                }
                Err(e) => notes.push(format!("The illustrative turning count could not be read: {e}")),
            }
        }
    }

    // Dead-end roads that no zone, gateway, count, bus route or junction site uses carry no traffic: leave them out
    let stubs = {
        let s = p.settings.clone();
        let data = Data::read(&p);
        prune_stubs(&mut p.scenarios[si], &s, &data)
    };
    if stubs > 0 {
        notes.push(format!("{stubs} dead-end road links with no zone, gateway, count or bus stop on them were left out (they carry no modelled traffic)."));
    }
    mark_stale(&mut p.scenarios[si]);

    /* 9 \u{b7} the assessment set: S0 existing, S1 do-minimum, S2 proposal, S3 mitigation */
    status("Setting up the assessment scenarios\u{2026}");
    let set = create_set(
        &mut p,
        ids,
        &CreateSetOpts {
            pathway: Some("planchange".to_string()),
            count_year: Some(BASE_YEAR),
            opening: Some(DESIGN_YEAR),
            design: Some(DESIGN_YEAR),
            from: Some(src_id.clone()),
            mitigation: Some(true),
            periods: Some(PERIODS.iter().map(|x| x.to_string()).collect()),
            ..Default::default()
        },
    )
    .map_err(|e| SampleError::msg(e.0))?;
    p.scenarios.retain(|s| s.id != src_id);
    let by = |p: &Project, role: &str| -> String {
        set.scenarios.iter().find(|id| p.scenario(id).is_some_and(|s| s.role.is(role))).cloned().unwrap_or_default()
    };
    let scenarios = SampleScenarios { s0: by(&p, "existing"), s1: by(&p, "baseline"), s2: by(&p, "proposal"), s3: by(&p, "mitigation") };
    let idx = |p: &Project, id: &str| p.scenarios.iter().position(|s| s.id == id).expect("a scenario of the set");
    let (i0, i1, i2, i3) = (idx(&p, &scenarios.s0), idx(&p, &scenarios.s1), idx(&p, &scenarios.s2), idx(&p, &scenarios.s3));
    p.scenarios[i0].name = Maybe::Set(format!("{} base year (existing)", number_to_js_string(BASE_YEAR)));
    p.scenarios[i1].name = Maybe::Set(format!("{} do-minimum", number_to_js_string(DESIGN_YEAR)));
    p.scenarios[i2].name = Maybe::Set(format!("{} Option A \u{b7} East links", number_to_js_string(DESIGN_YEAR)));
    p.scenarios[i3].name = Maybe::Set(format!("{} Option B \u{b7} Widen Waihoehoe", number_to_js_string(DESIGN_YEAR)));
    p.scenarios[i2].buildout = Num::number(100.0);
    p.scenarios[i3].buildout = Num::number(100.0);
    let widened = option_b(&mut p.scenarios[i3]);
    if widened == 0 {
        notes.push("Option B found no Waihoehoe Road links to widen in the downloaded roads.".to_string());
    }
    p.base = Maybe::Set(scenarios.s0.clone());
    p.active = Maybe::Set(scenarios.s2.clone());
    if m.lp {
        let s = p.settings.clone();
        for k in [i2, i3] {
            let zids: Vec<String> = p.scenarios[k]
                .zones
                .values()
                .filter(|z| z.tg.get().is_some_and(|t| value_truthy(t) && t.get("on").is_some_and(value_truthy)))
                .map(|z| z.id.clone())
                .collect();
            for zid in zids {
                let mut z = p.scenarios[k].zones.get(&zid).cloned().expect("the zone");
                sync(&mut z, &s, &p.scenarios[k]);
                p.scenarios[k].zones.insert(zid, z);
            }
        }
    }
    // hazard lengths for the proposal scenario, which the climate adaptation table assesses (kept for one scenario only, for size: Traffic data > Flood and coastal
    // hazards fills the others)
    if let (Some(h), true) = (&hazards, m.fe) {
        let rows = hooks.hazard_rows(&p, i2);
        let at = ids.now_iso();
        let feeds = super::helpers::object_at(&mut p.extra, "feeds");
        let hz = super::helpers::object_at(feeds, "hazards");
        hz.insert(scenarios.s2.clone(), json!({ "at": at, "layers": h.layers, "rows": rows }));
    }
    mark_stale(&mut p.scenarios[i0]);

    /* 10 \u{b7} count roles: about 25% of eligible counts held out for validation; gateway counts are tagged by their use */
    let mut roles: Option<(usize, usize)> = None;
    if m.cal {
        roles = Some(hold_out_default(&mut p, &scenarios.s0));
        match_sites(&mut p, &scenarios.s0);
    }

    /* 11 \u{b7} regulatory context, precinct staging and report details */
    let dev: Vec<Zone> = p.scenarios[i2].zones.values().filter(|z| z.role.is("development")).cloned().collect();
    let site = dev.iter().find(|z| z.tg.get().is_some_and(value_truthy)).or(dev.first()).cloned();
    if m.co {
        let ctx = json!({ "pathway": "pc-private", "lodgement": LODGEMENT, "lodgementActual": false, "region": "auckland", "context": "urban" });
        let site_map = site.as_ref().map(|s| {
            json!({ "lat": json_number(crate::util::r7(s.lat.num())), "lng": json_number(crate::util::r7(s.lng.num())), "src": "Sample: centre of the illustrative plan change area" })
        });
        hooks.compliance_context(&mut p, ctx.as_object().expect("an object"), site_map.as_ref().and_then(Value::as_object));
        soft(&mut failed, cancel, &mut status, "ac-site", "Auckland Council site lookup (zone, precinct, FDS area)", Some("Looking up the site zone and precinct (Auckland Council)"), |_| {
            hooks.lookup_site(&mut p)
        })?;
        soft(&mut failed, cancel, &mut status, "nzta-sh", "NZTA state highway network", None, |_| hooks.load_sh(&mut p, i0))?;
        soft(&mut failed, cancel, &mut status, "aup-controls", "Auckland Unitary Plan arterial and access controls", None, |_| hooks.load_overlays(&mut p, i0))?;
    }
    if m.cx {
        let register: Vec<Value> = COMMITTED
            .iter()
            .filter(|c| c.precinct == "I451")
            .map(|c| {
                let id = ids.uid("reg");
                json!({
                    "id": id, "ref": format!("{} ({})", c.name, SAMPLE_SRC),
                    "dwellings": json_number(math_round((qty(c.lu, "residential") + qty(c.lu, "apartment")) * c.buildout / 100.0)),
                    "retail": json_number(math_round(qty(c.lu, "retail") * c.buildout / 100.0)),
                    "commercial": json_number(math_round(qty(c.lu, "office") * c.buildout / 100.0)),
                    "community": 0, "date": "", "enabled": "",
                })
            })
            .collect();
        let pz = PROPOSAL.iter().find(|x| x.tg).expect("the plan change area");
        let proposal = json!({
            "ref": format!("{}: sample private plan change", pz.name), "dwellings": json_number(qty(pz.lu, "residential") + qty(pz.lu, "apartment")),
            "retail": json_number(qty(pz.lu, "retail")), "commercial": json_number(qty(pz.lu, "office")), "community": 0,
        });
        hooks.precinct(&mut p, "I451", register, proposal);
    }
    p.extra.insert(
        "report".to_string(),
        json!({
            "details": { "site": "Appleby Road and Waihoehoe Road, Drury East, Auckland (sample site, illustrative)", "client": "Sample project (illustrative)", "author": "Sample \u{2014} not signed",
                "pathway": "planchange", "lodgement": LODGEMENT, "date": "" },
            "scope": "broad", "signoff": {},
            "revisions": [{ "rev": "A", "date": today, "desc": "Sample draft made by Spare Construct (illustrative; not for lodgement)", "prepared": "Spare Construct sample", "reviewed": "", "approved": "" }],
            "engagement": [], "text": {}, "sel": {},
        }),
    );

    /* 12 \u{b7} what is real and what is illustrative, on each scenario */
    let got = |failed: &[Failed], id: &str| !failed.iter().any(|f| f.id == id);
    let n_nzta = counts_of(&mut p).iter().filter(|c| c.get("src").and_then(Value::as_str) == Some("nzta")).count();
    let n_at = counts_of(&mut p).iter().filter(|c| c.get("src").and_then(Value::as_str) == Some("at")).count();
    let n_cas = crashes_of(&mut p).len();
    let mut real = vec![format!("OpenStreetMap roads ({} links, ODbL)", p.scenarios[i0].links.len())];
    if existing_n > 0 {
        let from: Vec<&str> = [
            if els.as_ref().is_some_and(|e| !e.is_empty()) { "OpenStreetMap buildings" } else { "" },
            if fill.as_ref().is_some_and(|f| f.census.is_some()) { "Stats NZ Census 2023 dwellings and people" } else { "" },
            if fill.as_ref().is_some_and(|f| f.employees.is_some()) { "Stats NZ employees (2025)" } else { "" },
            if fill.as_ref().and_then(|f| f.schools.as_ref()).is_some_and(|s| s.loaded_schools) { "Ministry of Education school rolls" } else { "" },
        ]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect();
        real.push(format!("existing land use in {existing_n} zones from {}", from.join(", ")));
    }
    if got(&failed, "nzta") || got(&failed, "at") {
        real.push(format!("{n_nzta} NZTA and {n_at} Auckland Transport count sites"));
    }
    if got(&failed, "cas") {
        real.push(format!("{n_cas} NZTA CAS crashes"));
    }
    if n_gw > 0 {
        real.push(format!("{n_gw} gateway flows set from counts"));
    }
    if got(&failed, "nslr") && m.fe {
        real.push("NZTA legal speed limits where the register has a current version".to_string());
    }
    if got(&failed, "gtfs") && m.fe && opts.gtfs != Some(false) {
        real.push("Auckland Transport bus routes and frequencies (GTFS)".to_string());
    }
    if hazards.is_some() {
        real.push("Auckland Council flood plains and coastal inundation areas".to_string());
    }
    let missing: Vec<&str> = failed.iter().map(|f| f.label.as_str()).collect();
    let tail = if missing.is_empty() { String::new() } else { format!(" Not loaded in this build (so missing, not estimated): {}.", missing.join("; ")) };
    let indic = "Indicative: trip rates (Spare Construct defaults), junction delay (HCM 6 / Akcelik-based, not SIDRA output), the lane coding at the two coded junction sites (illustrative).";
    let (by_, dy_, gr_) = (number_to_js_string(BASE_YEAR), number_to_js_string(DESIGN_YEAR), number_to_js_string(GROWTH));
    p.scenarios[i0].notes = text(&format!(
        "{by_} base year. Real: {}. {indic} Matrix estimation is fitted to the calibration counts only; about a quarter of the counts are held out for validation, and counts used to set gateway flows are tagged as gateway counts.{tail}",
        real.join("; ")
    ));
    p.scenarios[i1].notes = text(&format!(
        "{dy_} do-minimum: the {by_} network and existing land use, plus committed growth in the Drury Centre (I450) and Drury East (I451) precincts. Illustrative: their yields, first-home years and {dy_} build-out shares are sample assumptions, not the precinct provisions or a developer programme. Background through traffic grows linearly at {gr_}% of {by_} volume a year (Spare Construct default; check against MSM).{tail}"
    ));
    let dev_names = dev.iter().map(|z| z.name.str_or("").to_string()).collect::<Vec<_>>().join(", ");
    p.scenarios[i2].notes = text(&format!(
        "{dy_} with the proposal: S1 plus an illustrative private plan change ({dev_names}; the plan change area carries a full trip-generation schedule with illustrative person-trip rates and mode shares) and Option A \u{b7} East links (illustrative): a collector from Takotowhenua Avenue through Drury East to Appleby Road and an arterial extension of Takotowhenua Avenue north to Sutton Road. Not Auckland Council, AT or developer proposals.{tail}"
    ));
    p.scenarios[i3].notes = text(&format!(
        "{dy_} with mitigation: everything in S2, plus Option B \u{b7} Waihoehoe Road four-laned from the Great South Road roundabout to Takotowhenua Avenue (illustrative mitigation, {widened} links). S3 against S2 shows the effect of the widening.{tail}"
    ));
    notes.push(format!("Real: {}.", real.join("; ")));
    notes.push(format!(
        "Illustrative: development yields and staging, Options A and B, the trip-generation rates, the junction lane coding and the turning count (labelled {ILLUSTRATIVE})."
    ));
    if let Some((valid, calib)) = roles {
        notes.push(format!("Count roles: {valid} validation counts held out, {calib} calibration counts; gateway counts tagged."));
    }
    notes.push("Journey times are not checked: observed times need a survey, GPS data or a Google or TomTom key, and none is used by the sample.".to_string());
    p.extra.insert(
        "sample".to_string(),
        json!({
            "version": 1, "built": ids.now_iso(), "bounds": [[BOUNDS[0][0], BOUNDS[0][1]], [BOUNDS[1][0], BOUNDS[1][1]]], "area": area_value(), "lodgement": LODGEMENT,
            "failed": failed.iter().map(Failed::to_value).collect::<Vec<_>>(),
            "illustrative": ["development yields, opening years and build-out", "Options A and B", "trip-generation rates and mode shares", "junction lane coding at two sites", "precinct register entries",
                format!("the turning count ({ILLUSTRATIVE})")],
        }),
    );

    /* 13 \u{b7} runs, matrix estimation and the report's staging and sensitivity tests */
    if opts.run {
        let mut ao = AnalyseOpts { extras: opts.extras, modules: m, cancel, runs: None };
        analyse(&mut p, ids, hooks, &mut ao, &mut notes, &mut status)?;
    }
    let ms = ids.now_ms() - t0;
    Ok(Built { project: p, notes, failed, ms, scenarios })
}
