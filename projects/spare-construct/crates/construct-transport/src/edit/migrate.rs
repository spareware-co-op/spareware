//! Opening an older or hand-made project: model.js `migrate`, `SETTINGS_V2`, `legacySettings` and the settings notice.
//!
//! New projects start from the current defaults ([`crate::project::default_settings`]). A project saved before a
//! default changed keeps its own values: a key it never saved gets the earlier value (so its results do not change),
//! and [`migrate_json`] lists the differences in `settingsNotice` for the person to review. [`adopt_defaults`] takes
//! the new defaults (all, or the listed keys); [`keep_settings`] keeps the project's own. The upgrade runs on
//! [`JsonValue`], the way the JavaScript runs it on the parsed file, so every key keeps its place.

use super::json::{JsonValue, deep_merge};
use super::register::presets;
use super::{EditError, IdSource};
use crate::jsnum::{math_round, number_to_js_string, string_to_number, to_precision};
use crate::project::{Project, SETTINGS_VERSION, Settings, default_settings, new_scenario};
use serde_json::Value;

/// One default that changed after settings version 1 (model.js SETTINGS_V2).
struct Change {
    /// Path in the settings ("conv.stable").
    key: &'static str,
    /// The value projects saved before it had.
    v1: fn() -> JsonValue,
    /// The settings version that brought the new default.
    since: f64,
    /// What it is called.
    label: &'static str,
    /// Its value in words (the value, and the settings it sits in).
    text: fn(Option<&JsonValue>, &JsonValue) -> String,
}

fn js_string(v: Option<&JsonValue>) -> String {
    v.map_or("undefined".to_string(), JsonValue::js_string)
}

fn is_str(v: Option<&JsonValue>, s: &str) -> bool {
    v.and_then(JsonValue::as_str) == Some(s)
}

fn num_of(v: Option<&JsonValue>) -> f64 {
    v.map_or(f64::NAN, JsonValue::to_number)
}

const SETTINGS_V2: [Change; 9] = [
    Change {
        key: "growthMode",
        v1: || "compound".into(),
        since: 2.0,
        label: "Background growth",
        text: |v, _| {
            if is_str(v, "linear") {
                "linear (% of base-year traffic a year, as MBCM)".to_string()
            } else {
                "compound".to_string()
            }
        },
    },
    Change {
        key: "gapTol",
        v1: || 0.0005.into(),
        since: 2.0,
        label: "Convergence target (relative gap)",
        text: |v, _| {
            let p = to_precision(num_of(v) * 100.0, 2);
            format!("{}%", number_to_js_string(string_to_number(&p)))
        },
    },
    Change {
        key: "maxIter",
        v1: || 60.0.into(),
        since: 2.0,
        label: "Maximum iterations",
        text: |v, _| js_string(v),
    },
    Change {
        key: "conv.stable",
        v1: || false.into(),
        since: 2.0,
        label: "Stop only when link flows are stable (TAG M3.1: P > 98% for 4 iterations)",
        text: |v, _| {
            if v.is_some_and(JsonValue::truthy) {
                "on".to_string()
            } else {
                "off".to_string()
            }
        },
    },
    Change {
        key: "conv.capDamp",
        v1: || "none".into(),
        since: 2.0,
        label: "Damped give-way and roundabout capacities during assignment",
        text: |v, _| {
            if is_str(v, "msa") {
                "on (MSA)".to_string()
            } else {
                "off".to_string()
            }
        },
    },
    Change {
        key: "econ.method",
        v1: || "uniform".into(),
        since: 2.0,
        label: "Discounting",
        text: |v, s| {
            if is_str(v, "mbcm") {
                "MBCM v1.7.5 and NZTA GC 25/01: 2% to year 30, then 1.5% to year 100, then 1%; 8% sensitivity; arithmetic benefit growth".to_string()
            } else {
                // `(S.econ || {}).discount`
                let econ = s.get("econ").filter(|e| e.truthy());
                let d = econ.and_then(|e| e.get("discount"));
                format!("{}% a year, flat benefits", js_string(d))
            }
        },
    },
    Change {
        key: "junction.pff",
        v1: || 1.0.into(),
        since: 2.0,
        label: "Peak flow factor",
        text: |v, _| format!("{}%", number_to_js_string(math_round(num_of(v) * 100.0))),
    },
    Change {
        key: "junction.T",
        v1: || 1.0.into(),
        since: 2.0,
        label: "Analysis period for overflow delay and queues",
        text: |v, _| format!("{} min", number_to_js_string(math_round(num_of(v) * 60.0))),
    },
    Change {
        key: "busPcu",
        v1: || 3.0.into(),
        since: 3.0,
        label: "Passenger car units per bus (bus route preload)",
        text: |v, _| format!("{} PCU", js_string(v)),
    },
];

/// model.js SETTINGS_NOTICE: what the notice says.
pub const SETTINGS_NOTICE: &str = "Spare Construct's defaults changed to match NZ guidance (growth, discounting, convergence, buses). This project keeps its own settings until you choose.";

/// `getPath(o, k)`: the value at a dotted path, None where any step is missing or not an object.
pub fn get_path<'a>(o: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    let mut a = o;
    for p in k.split('.') {
        a = a.get(p)?;
    }
    Some(a)
}

/// `setPath(o, k, v)`: sets the value at a dotted path, making an object of any step that is missing, null or not an object.
pub fn set_path(o: &mut JsonValue, k: &str, v: JsonValue) {
    let mut parts: Vec<&str> = k.split('.').collect();
    let last = parts.pop().expect("a path has a last step");
    let mut a = o;
    for q in parts {
        let make = !a.get(q).is_some_and(JsonValue::is_object_like);
        if make {
            a.set(q, JsonValue::object());
        }
        match a.get_mut(q) {
            Some(next) => a = next,
            None => return,
        }
    }
    a.set(last, v);
}

fn default_settings_json() -> JsonValue {
    settings_to_json(&default_settings())
}

fn settings_to_json(s: &Settings) -> JsonValue {
    let text = serde_json::to_string(s).expect("settings always serialise");
    JsonValue::parse(&text).expect("settings text is JSON")
}

/// model.js legacySettings(P, saved, ver): fills the keys a project never saved with their version-1 values and lists the
/// differences from the current defaults in `P.settingsNotice` (or clears it when there are none).
fn legacy_settings(p: &mut JsonValue, saved: &JsonValue, ver: f64) {
    let list: Vec<&Change> = SETTINGS_V2.iter().filter(|c| c.since > ver).collect();
    for c in &list {
        if get_path(saved, c.key).is_none() {
            let s = p.get_mut("settings").expect("settings are set");
            set_path(s, c.key, (c.v1)());
        }
    }
    let d = default_settings_json();
    let s = p.get("settings").expect("settings are set").clone();
    let mut changed: Vec<JsonValue> = Vec::new();
    for c in &list {
        let yours = get_path(&s, c.key);
        let now = get_path(&d, c.key);
        let same = match (yours, now) {
            (None, None) => true,
            (Some(a), Some(b)) => a.strict_eq(b),
            _ => false,
        };
        if same {
            continue;
        }
        let mut o = JsonValue::object();
        o.set("key", c.key.into());
        o.set("label", c.label.into());
        if let Some(y) = yours {
            o.set("yours", y.clone());
        }
        if let Some(n) = now {
            o.set("now", n.clone());
        }
        o.set("yoursText", (c.text)(yours, &s).into());
        o.set("nowText", (c.text)(now, &d).into());
        changed.push(o);
    }
    let sv_now;
    if !changed.is_empty() {
        sv_now = ver;
        let mut n = JsonValue::object();
        n.set("from", ver.into());
        n.set("to", SETTINGS_VERSION.into());
        n.set("text", SETTINGS_NOTICE.into());
        n.set("changed", JsonValue::Array(changed));
        p.set("settingsNotice", n);
    } else {
        sv_now = SETTINGS_VERSION;
        p.remove("settingsNotice");
    }
    p.get_mut("settings")
        .expect("settings are set")
        .set("sv", sv_now.into());
}

/// model.js OLD_RATES: the discount schedule saved before NZTA GC 25/01, as JSON text.
const OLD_RATES: &str = r#"[{"to":30,"r":2},{"to":60,"r":1.5}]"#;

/// model.js upgradeRates(S): a saved copy of the pre-GC 25/01 schedule becomes the full schedule when the analysis period is 100
/// years or less (the results are identical; past 100 years GC 25/01 uses 1%).
fn upgrade_rates(s: &mut JsonValue) {
    let years = s
        .get("econ")
        .and_then(|e| e.get("years"))
        .map_or(f64::NAN, JsonValue::to_number);
    let Some(e) = s.get_mut("econ") else {
        return;
    };
    let old = e
        .get("rates")
        .is_some_and(|r| r.is_array() && r.stringify() == OLD_RATES);
    if old && !(years > 100.0) {
        let d = default_settings_json();
        if let Some(r) = d.get("econ").and_then(|e| e.get("rates")) {
            e.set("rates", r.clone());
        }
    }
}

/// model.js migYear(v, d): a year typed or read from a file as text ("2036") is stored as a number; anything else gives d.
fn mig_year(v: Option<&JsonValue>, d: Option<f64>) -> Option<f64> {
    let ok = match v {
        Some(JsonValue::Number(_)) => true,
        Some(JsonValue::String(s)) => !crate::jsnum::js_trim(s).is_empty(),
        _ => false,
    };
    if ok {
        let x = v.expect("checked").to_number();
        if x.is_finite() && x > 0.0 {
            return Some(math_round(x));
        }
    }
    d
}

/// JavaScript's `a ||= b` on an object key: sets the key to b when it is missing or falsy.
fn or_assign(o: &mut JsonValue, k: &str, b: JsonValue) {
    if !o.get(k).is_some_and(JsonValue::truthy) {
        o.set(k, b);
    }
}

/// model.js `migrate(P)`: brings a parsed project up to date. Settings merge onto the current defaults (the saved keys win),
/// a project saved before a default changed keeps its own value (see SETTINGS_V2 in model.js), years read as text become numbers, a
/// missing id or scenario list is made, junction sites are checked, and the active and base scenarios must exist.
pub fn migrate_json(mut p: JsonValue, ids: &mut dyn IdSource) -> Result<JsonValue, EditError> {
    if !p.is_object() {
        return Err(EditError("a project file holds an object".to_string()));
    }
    let saved: Option<JsonValue> = p.get("settings").filter(|s| s.is_object()).cloned();
    // The saved settings version (1 when a project predates versions); a project without settings is new
    let ver = match &saved {
        Some(s) => {
            let sv = s.get("sv").map_or(f64::NAN, JsonValue::to_number);
            if sv >= 1.0 {
                crate::jsnum::math_min(SETTINGS_VERSION, sv.floor())
            } else {
                1.0
            }
        }
        None => SETTINGS_VERSION,
    };
    let empty = JsonValue::object();
    let merged = deep_merge(&default_settings_json(), saved.as_ref().unwrap_or(&empty));
    p.set("settings", merged);
    if ver < SETTINGS_VERSION {
        legacy_settings(
            &mut p,
            saved.as_ref().expect("older settings were saved"),
            ver,
        );
    } else {
        p.remove("settingsNotice");
    }
    // Years from a file can be text ('2036'): numbers from here on
    let d0 = default_settings_json();
    {
        let s = p.get_mut("settings").expect("settings are set");
        let by = mig_year(
            s.get("baseYear"),
            d0.get("baseYear").and_then(JsonValue::as_f64),
        );
        if let Some(by) = by {
            s.set("baseYear", by.into());
        }
        let y = mig_year(s.get("year"), by);
        if let Some(y) = y {
            s.set("year", y.into());
        }
        upgrade_rates(s);
    }
    if !p
        .get("id")
        .is_some_and(|v| v.as_str().is_some() && v.truthy())
    {
        p.set("id", ids.uid("p").into());
    }
    // P.scenarios && P.scenarios.length ? P.scenarios : [newScenario('Base case')]
    let have = p
        .get("scenarios")
        .and_then(JsonValue::as_array)
        .is_some_and(|a| !a.is_empty());
    if !have {
        let s = new_scenario(&ids.uid("s"), "Base case");
        let text = serde_json::to_string(&s).expect("a scenario serialises");
        p.set(
            "scenarios",
            JsonValue::Array(vec![
                JsonValue::parse(&text).expect("scenario text is JSON"),
            ]),
        );
    }
    if let Some(JsonValue::Array(scenarios)) = p.get_mut("scenarios") {
        for s in scenarios.iter_mut() {
            migrate_scenario(s);
        }
    }
    // The register
    if p.get("register")
        .is_some_and(|r| !r.is_null() && !r.is_array())
    {
        p.remove("register");
    }
    if let Some(JsonValue::Array(rows)) = p.get_mut("register") {
        // Saved Mill Road preset rows gain the designation 6786 record (data only)
        let fresh: Vec<(String, Value)> = presets(super::register::PRESET_AS_AT)
            .into_iter()
            .filter_map(|r| {
                let id = r.id.as_str()?.to_string();
                r.designation.get().cloned().map(|d| (id, d))
            })
            .collect();
        for r in rows.iter_mut() {
            if !r.is_object() || !r.get("preset").is_some_and(JsonValue::truthy) {
                continue;
            }
            let id = r.get("id").and_then(JsonValue::as_str);
            let found = id.and_then(|id| fresh.iter().find(|(k, _)| k == id));
            if let Some((_, d)) = found
                && !r.get("designation").is_some_and(JsonValue::truthy)
            {
                r.set("designation", JsonValue::from_value(d));
            }
        }
    }
    if p.get("me")
        .is_some_and(|m| !m.is_null() && !m.is_object_like())
    {
        p.remove("me");
    }
    let first_id: Option<JsonValue> = p
        .get("scenarios")
        .and_then(JsonValue::as_array)
        .and_then(|a| a.first())
        .and_then(|s| s.get("id").cloned());
    for key in ["active", "base"] {
        let ok = match (p.get(key), p.get("scenarios").and_then(JsonValue::as_array)) {
            (Some(v), Some(list)) => list
                .iter()
                .any(|s| s.get("id").is_some_and(|id| id.strict_eq(v))),
            _ => false,
        };
        if !ok {
            if let Some(id) = &first_id {
                p.set(key, id.clone());
            } else {
                p.remove(key);
            }
        }
    }
    if !p.get("view").is_some_and(JsonValue::truthy) {
        let mut v = JsonValue::object();
        v.set(
            "c",
            JsonValue::Array(vec![(-36.8485).into(), 174.7633.into()]),
        );
        v.set("z", 14.0.into());
        p.set("view", v);
    }
    Ok(p)
}

/// The per-scenario part of `migrate`: containers, results without paths, the year, sites and build-out.
fn migrate_scenario(s: &mut JsonValue) {
    if !s.is_object() {
        return;
    }
    for k in ["nodes", "links", "zones", "routes", "res"] {
        or_assign(s, k, JsonValue::object());
    }
    if let Some(JsonValue::Object(res)) = s.get_mut("res") {
        for r in res.values_mut() {
            if r.truthy() && r.is_object() && !r.get("paths").is_some_and(JsonValue::truthy) {
                r.set("noPaths", true.into());
            }
        }
    }
    if s.get("year").is_some_and(|y| !y.is_null()) {
        match mig_year(s.get("year"), None) {
            Some(y) => s.set("year", y.into()),
            None => {
                s.remove("year");
            }
        }
    }
    // Junction sites from a file: keep well-formed ones (id = key, nodes an array)
    let sites_present = s.get("sites").is_some_and(|v| !v.is_null());
    if sites_present {
        let bad_type = !matches!(s.get("sites"), Some(JsonValue::Object(_)));
        if bad_type {
            s.remove("sites");
        } else if let Some(JsonValue::Object(m)) = s.get_mut("sites") {
            let keys: Vec<String> = m.keys().map(str::to_string).collect();
            for k in keys {
                let good = m.get(&k).is_some_and(|v| {
                    v.is_object() && v.get("nodes").is_some_and(JsonValue::is_array)
                });
                if !good {
                    m.remove(&k);
                } else if let Some(v) = m.get_mut(&k) {
                    v.set("id", k.as_str().into());
                }
            }
        }
    }
    if s.get("buildout").is_some_and(|b| !b.is_null())
        && !s
            .get("buildout")
            .map_or(f64::NAN, JsonValue::to_number)
            .is_finite()
    {
        s.remove("buildout");
    }
}

/// `migrate(JSON.parse(text))` as a typed project: reads the text (keeping the order of every object's keys), upgrades it
/// and returns the project. Fails when the text is not JSON or the upgraded project does not read as a [`Project`].
pub fn migrate_text(text: &str, ids: &mut dyn IdSource) -> Result<Project, EditError> {
    let v = JsonValue::parse(text).map_err(|e| EditError(format!("not JSON: {e}")))?;
    let m = migrate_json(v, ids)?;
    Project::from_json(&m.stringify())
        .map_err(|e| EditError(format!("the project does not read as a project: {e}")))
}

/// `migrate(P)` of a project already in memory (`Store.setProject`): the project written out, upgraded and read back.
pub fn migrate_project(p: &Project, ids: &mut dyn IdSource) -> Result<Project, EditError> {
    migrate_text(
        &serde_json::to_string(p).expect("a project serialises"),
        ids,
    )
}

/// The notice `migrate` left on a project (`P.settingsNotice`): the keys whose defaults changed, if any.
pub fn settings_notice(p: &Project) -> Option<&Value> {
    p.extra.get("settingsNotice")
}

/// `Store.adoptDefaults(keys)`: adopts the new defaults the notice lists (all of them, or those named), marks the project's
/// results out of date when any were adopted, and clears the notice. Returns how many settings changed.
pub fn adopt_defaults(p: &mut Project, keys: Option<&[String]>) -> Result<usize, EditError> {
    let Some(notice) = p.extra.get("settingsNotice").cloned() else {
        return Ok(0);
    };
    let d = default_settings_json();
    let mut s = settings_to_json(&p.settings);
    let mut picked = 0usize;
    if let Some(list) = notice.get("changed").and_then(Value::as_array) {
        for c in list {
            let Some(key) = c.get("key").and_then(Value::as_str) else {
                continue;
            };
            if keys.is_some_and(|ks| !ks.iter().any(|k| k == key)) {
                continue;
            }
            if let Some(v) = get_path(&d, key) {
                set_path(&mut s, key, v.clone());
            }
            picked += 1;
        }
    }
    s.set("sv", SETTINGS_VERSION.into());
    p.settings = serde_json::from_str(&s.stringify())
        .map_err(|e| EditError(format!("the settings do not read as settings: {e}")))?;
    p.extra.remove("settingsNotice");
    if picked > 0 {
        for sc in p.scenarios.iter_mut() {
            super::history::mark_stale(sc);
        }
    }
    Ok(picked)
}

/// `Store.keepSettings()`: keeps the project's own settings, writes the current settings version and clears the notice.
/// False when there was no notice.
pub fn keep_settings(p: &mut Project) -> bool {
    if p.extra.get("settingsNotice").is_none() {
        return false;
    }
    p.settings.sv = crate::jsnum::Num::number(SETTINGS_VERSION);
    p.extra.remove("settingsNotice");
    true
}
