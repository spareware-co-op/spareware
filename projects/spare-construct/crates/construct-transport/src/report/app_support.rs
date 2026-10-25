//! What the appendices read from the shared kit script (`Kit.nonDefaults`, `Kit.valText`, `Kit.dataSources`, `Kit.line`) and the
//! report's own settings table and assumptions register.

use super::consts::{CEC_MODELS, rr422, walk_factor};
use super::js;
use super::texts::ENGINE_ASSUMPTIONS;
use super::{Rep, consts};
use crate::edit::json::JsonValue;
use crate::jsnum::{number_to_js_string, to_fixed};
use crate::project::{default_settings, land_use, road_class_exact};
use crate::util::{esc, fmt};

/// The settings of a new project as JSON, keys in the prototype's order (`defaultSettings()`).
pub(crate) fn default_settings_json() -> JsonValue {
    let text = serde_json::to_string(&default_settings()).expect("settings always serialise");
    let mut v = JsonValue::parse(&text).expect("settings text is JSON");
    // the discount schedule is held as an unordered value: put its keys in the prototype's order, { to, r }
    if let Some(JsonValue::Array(rows)) = v.get_mut("econ").and_then(|e| e.get_mut("rates")) {
        for row in rows.iter_mut() {
            let (to, r) = (row.get("to").cloned(), row.get("r").cloned());
            if let (Some(to), Some(r)) = (to, r) {
                *row = JsonValue::from_pairs([("to", to), ("r", r)]);
            }
        }
    }
    v
}

/// `flatten(o)`: every non-object value with its dotted path, in the order the object lists them.
pub(crate) fn flatten(o: Option<&JsonValue>) -> Vec<(String, JsonValue)> {
    fn walk(o: &JsonValue, pre: &str, out: &mut Vec<(String, JsonValue)>) {
        if let JsonValue::Object(m) = o {
            for (k, v) in m.iter() {
                let key = if pre.is_empty() { k.to_string() } else { format!("{pre}.{k}") };
                match v {
                    JsonValue::Object(_) => walk(v, &key, out),
                    _ => {
                        if let Some(slot) = out.iter_mut().find(|(x, _)| *x == key) {
                            slot.1 = v.clone();
                        } else {
                            out.push((key, v.clone()));
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(o) = o {
        walk(o, "", &mut out);
    }
    out
}

/// One row of the model settings table.
pub(crate) struct SettingRow {
    pub key: String,
    pub label: String,
    pub value: String,
    pub def: String,
    pub changed: bool,
}

/// `seg(o)`: an object's entries as 'key value' pairs.
fn seg(o: &JsonValue) -> String {
    let pair = |k: &str, x: &JsonValue| format!("{k} {}", if matches!(x, JsonValue::Null) { "none".to_string() } else { x.js_string() });
    match o {
        JsonValue::Object(m) => m.iter().map(|(k, x)| pair(k, x)).collect::<Vec<_>>().join(", "),
        JsonValue::Array(a) => a.iter().enumerate().map(|(i, x)| pair(&i.to_string(), x)).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

fn setting_text(x: Option<&JsonValue>) -> String {
    match x {
        None => fmt::DASH.to_string(),
        Some(JsonValue::Null) => "none".to_string(),
        Some(JsonValue::Array(a)) => {
            if a.iter().all(|e| e.is_object_like()) {
                a.iter().map(seg).collect::<Vec<_>>().join("; ")
            } else {
                a.iter()
                    .map(|e| match e {
                        JsonValue::Null => String::new(),
                        JsonValue::Object(_) => "[object Object]".to_string(),
                        other => other.js_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        Some(o @ JsonValue::Object(_)) => seg(o),
        Some(other) => other.js_string(),
    }
}

/// Compares two strings by UTF-16 code units, as the default `Array.sort` does.
pub(crate) fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `settingsRows(P)`: every setting with its default and a non-default flag (rates are listed apart; keys and the basemap are left out).
pub(crate) fn settings_rows(r: &Rep) -> Vec<SettingRow> {
    let s = flatten(js::get(Some(r.raw), "settings"));
    let d = flatten(Some(&default_settings_json()));
    let mut keys: Vec<String> = Vec::new();
    for (k, _) in d.iter().chain(s.iter()) {
        if !keys.contains(k) {
            keys.push(k.clone());
        }
    }
    keys.retain(|k| !(k.starts_with("rates.") || k == "linzKey" || k == "basemap"));
    keys.sort_by(|a, b| utf16_cmp(a, b));
    let find = |l: &[(String, JsonValue)], k: &str| l.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
    keys.into_iter()
        .map(|k| {
            let sv = find(&s, &k);
            let dv = find(&d, &k);
            SettingRow {
                label: consts::key_label(&k).map_or(k.clone(), str::to_string),
                value: setting_text(sv.as_ref()),
                def: setting_text(dv.as_ref()),
                changed: sv.as_ref().map(JsonValue::stringify) != dv.as_ref().map(JsonValue::stringify),
                key: k,
            }
        })
        .collect()
}

/* ---------- Kit.nonDefaults ---------- */

/// `SETTING_LABELS[k]`: a label and a unit.
fn fixed_label(k: &str) -> Option<(&'static str, &'static str)> {
    Some(match k {
        "baseYear" => ("Base year", ""),
        "year" => ("Default forecast year", ""),
        "growth" => ("Background growth", "% a year"),
        "growthMode" => ("Growth method", ""),
        "hvPct" => ("Heavy vehicles", "%"),
        "pcuHV" => ("PCU per heavy vehicle", ""),
        "peakToDaily" => ("Peak hour to daily factor", "\u{d7}"),
        "gravityBeta" => ("Trip-length sensitivity (gravity \u{3b2})", "per min"),
        "connectors" => ("Zone access lines per zone", ""),
        "connectorSpeed" => ("Zone access line speed", "km/h"),
        "bprAlpha" => ("Speed-flow curve \u{3b1} (BPR)", ""),
        "bprBeta" => ("Speed-flow curve \u{3b2} (BPR)", ""),
        "satFlow" => ("Saturation flow", "pcu/h/lane"),
        "cycle" => ("Default signal cycle", "s"),
        "lostPerPhase" => ("Lost time per phase", "s"),
        "signalOpt" => ("Optimise signal timings", ""),
        "maxIter" => ("Maximum iterations", ""),
        "gapTol" => ("Convergence target (relative gap)", ""),
        "conv.stable" => ("Stop only when link flows are stable (TAG M3.1)", ""),
        "conv.capDamp" => ("Damped junction capacities during assignment", ""),
        "conv.chg" => ("Stable link: flow change under", ""),
        "conv.share" => ("Stable links needed", ""),
        "conv.consec" => ("Stable iterations in a row", ""),
        "junction.losMethod" => ("Junction LoS method", ""),
        "junction.pff" => ("Peak flow factor", ""),
        "junction.T" => ("Analysis period for overflow delay", "h"),
        "junction.queueSpace" => ("Queue space per vehicle", "m"),
        "junction.f95" => ("95th-percentile queue factor", ""),
        "junction.capMethod" => ("Junction capacity method", ""),
        "tmdg.category" => ("TMDG model category", ""),
        "tmdg.justification" => ("TMDG category justification", ""),
        "assess.msmPeakShare" => ("Peak-hour share of an MSM 2-hour period", ""),
        "turns.left" => ("Left-turn penalty", "s"),
        "turns.right" => ("Right-turn penalty", "s"),
        "turns.rightSignals" => ("Right-turn penalty at signals", "s"),
        "turns.uturn" => ("U-turns", ""),
        "econ.method" => ("Discounting method", ""),
        "econ.vot" => ("Value of time", "$/veh\u{b7}h"),
        "econ.votUpdate" => ("Apply the MBCM update factor to the value of time", ""),
        "econ.annualHours" => ("Peak hour to year (uniform method)", "h"),
        "econ.periodHours.AM" => ("AM peak hours a year", "h"),
        "econ.periodHours.PM" => ("PM peak hours a year", "h"),
        "econ.periodHours.IP" => ("Inter-peak hours a year", "h"),
        "econ.rates" => ("Discount rate schedule", ""),
        "econ.sensRate" => ("Sensitivity discount rate", "%"),
        "econ.discount" => ("Discount rate (uniform method)", "%"),
        "econ.years" => ("Evaluation period", "years"),
        "econ.longLifeNote" => ("Reason for a period over 40 years", ""),
        "econ.approval" => ("Approval for a period over 60 years", ""),
        "econ.benefitGrowth" => ("Benefit growth", "% a year"),
        "econ.p95" => ("P95 cost estimate prepared", ""),
        "sim.scale" => ("Simulation demand", "\u{d7}"),
        "sim.speed" => ("Simulation speed", "\u{d7}"),
        "sim.maxVeh" => ("Simulation vehicle limit", ""),
        "data.amShare" => ("AM peak hour share of daily", "%"),
        "data.pmShare" => ("PM peak hour share of daily", "%"),
        "data.matchRadius" => ("Match counts to roads within", "m"),
        _ => return None,
    })
}

fn is_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `settingLabel(k)`: the label and unit of a setting path.
pub(crate) fn setting_label(k: &str) -> (String, String) {
    if let Some((l, u)) = fixed_label(k) {
        return (l.to_string(), u.to_string());
    }
    let parts: Vec<&str> = k.split('.').collect();
    if parts.len() == 4 && parts[0] == "rates" && is_word(parts[1]) && (parts[2] == "AM" || parts[2] == "PM") && (parts[3] == "rate" || parts[3] == "inPct") {
        let lu = land_use(parts[1]);
        let label = lu.map_or(parts[1], |l| l.label);
        return if parts[3] == "rate" {
            (
                format!("Trip rate: {label}, {} peak", parts[2]),
                format!(
                    "trips per {}",
                    match lu {
                        Some(l) if l.per == 100.0 => "100 m\u{b2} GFA".to_string(),
                        Some(l) => l.unit.strip_suffix('s').unwrap_or(l.unit).to_string(),
                        None => String::new(),
                    }
                ),
            )
        } else {
            (format!("Trip rate: {label}, {} peak, share arriving", parts[2]), "%".to_string())
        };
    }
    if parts.len() == 3 && parts[0] == "econ" && parts[1] == "laneKm" && is_word(parts[2]) {
        let label = road_class_exact(parts[2]).map_or(parts[2], |c| c.label);
        return (format!("Cost of new {} road", label.to_lowercase()), "$m per lane-km".to_string());
    }
    if parts.len() == 3 && parts[0] == "econ" && parts[1] == "node" && is_word(parts[2]) {
        return (format!("Cost of a new or upgraded {}", super::sections_env::control_label(parts[2]).to_lowercase()), "$m".to_string());
    }
    (k.to_string(), String::new())
}

/// `NOT_ASSUMPTION`: view state, keys and the settings version.
fn not_assumption(k: &str) -> bool {
    matches!(k, "sv" | "period" | "basemap" | "linzKey" | "sim.speed") || k == "display" || k.starts_with("display.")
}

/// `valText(v)`.
pub(crate) fn val_text(v: Option<&JsonValue>) -> String {
    match v {
        None | Some(JsonValue::Null) => "(blank)".to_string(),
        Some(JsonValue::String(s)) if s.is_empty() => "(blank)".to_string(),
        Some(JsonValue::Bool(b)) => if *b { "on" } else { "off" }.to_string(),
        Some(JsonValue::Array(a)) => a
            .iter()
            .map(|x| match x {
                JsonValue::Object(m) => m.iter().map(|(_, y)| y.js_string()).collect::<Vec<_>>().join(" "),
                JsonValue::Array(_) => x.js_string(),
                JsonValue::Null => String::new(),
                other => other.js_string(),
            })
            .collect::<Vec<_>>()
            .join("; "),
        Some(other) => other.js_string(),
    }
}

/// A setting that differs from its default.
pub(crate) struct NonDefault {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub value: Option<JsonValue>,
    pub def: Option<JsonValue>,
}

fn walk_defaults(d: &JsonValue, s: Option<&JsonValue>, pre: &str, out: &mut Vec<NonDefault>) {
    let JsonValue::Object(dm) = d else {
        return;
    };
    for (k, dv) in dm.iter() {
        let key = if pre.is_empty() { k.to_string() } else { format!("{pre}.{k}") };
        if not_assumption(&key) {
            continue;
        }
        let sv: Option<&JsonValue> = match s {
            Some(JsonValue::Object(sm)) => sm.get(k),
            Some(JsonValue::Array(a)) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => None,
        };
        if let JsonValue::Object(_) = dv {
            let next = match sv {
                Some(x) if x.is_object_like() => Some(x),
                _ => None,
            };
            walk_defaults(dv, next, &key, out);
            continue;
        }
        let same = if let JsonValue::Array(_) = dv {
            sv.map(JsonValue::stringify) == Some(dv.stringify())
        } else {
            let strict = sv.is_some_and(|x| dv.strict_eq(x));
            let blank = matches!(dv, JsonValue::Null) && (sv.is_none() || matches!(sv, Some(JsonValue::Null)) || matches!(sv, Some(JsonValue::String(t)) if t.is_empty()));
            let numeric = match (dv, sv) {
                (JsonValue::Number(n), Some(x)) => {
                    let nonblank = !matches!(x, JsonValue::String(t) if t.is_empty()) && !matches!(x, JsonValue::Null);
                    x.to_number() == *n && nonblank
                }
                _ => false,
            };
            strict || blank || numeric
        };
        if same || sv.is_none() {
            continue;
        }
        let (label, unit) = setting_label(&key);
        out.push(NonDefault { key, label, unit, value: sv.cloned(), def: Some(dv.clone()) });
    }
}

/// `Kit.nonDefaults(S)`: every setting that differs from the defaults.
pub(crate) fn non_defaults(r: &Rep) -> Vec<NonDefault> {
    let mut out = Vec::new();
    let Some(s) = js::get(Some(r.raw), "settings").filter(|s| s.truthy()) else {
        return out;
    };
    walk_defaults(&default_settings_json(), Some(s), "", &mut out);
    out
}

/* ---------- Kit.dataSources ---------- */

/// One row of the data sources register.
pub(crate) struct DataSource {
    pub what: &'static str,
    pub source: &'static str,
    pub url: &'static str,
    pub licence: &'static str,
    pub date: Option<String>,
    pub basis: &'static str,
    pub notes: String,
}

/// `new Date(t).toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' })` in UTC.
fn locale_date(at: Option<&JsonValue>) -> Option<String> {
    let at = at.filter(|v| v.truthy())?;
    let Some(iso) = crate::results::js_iso_day(&at.to_value()) else {
        return Some("Invalid Date".to_string());
    };
    let (y, rest) = iso.split_at(4);
    let m: usize = rest[1..3].parse().unwrap_or(1);
    let d: usize = rest[4..6].parse().unwrap_or(1);
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];
    Some(format!("{d} {} {y}", MONTHS[m - 1]))
}

fn records(x: Option<&JsonValue>) -> String {
    match js::get(x, "n") {
        Some(n) if !n.is_null() => format!(" \u{b7} {} records", fmt::int(n.to_number())),
        _ => String::new(),
    }
}

/// `Kit.dataSources(P)`: each source with its licence, query date and basis.
pub(crate) fn data_sources(r: &Rep) -> Vec<DataSource> {
    let ld = r.loaded();
    let s0 = js::get(Some(r.raw), "settings");
    let g = |k: &str| js::get(ld, k).filter(|v| v.truthy());
    let cas_notes = format!(
        "{}The latest months may be incomplete.{}",
        match g("cas").and_then(|c| js::get(Some(c), "label")).filter(|l| l.truthy()) {
            Some(l) => format!("{}. ", l.js_string()),
            None => "Five full years plus the year to date. ".to_string(),
        },
        records(g("cas"))
    );
    let nn = |v: Option<&JsonValue>, d: &str| -> String {
        match v {
            None | Some(JsonValue::Null) => d.to_string(),
            Some(x) => x.js_string(),
        }
    };
    let rate = nn(js::path(s0, &["rates", "residential", "AM", "rate"]), fmt::DASH);
    let growth = nn(js::get(s0, "growth"), fmt::DASH);
    let base_year = nn(js::get(s0, "baseYear"), fmt::DASH);
    let compound = js::get(s0, "growthMode").and_then(JsonValue::as_str) == Some("compound");
    vec![
        DataSource {
            what: "Existing roads",
            source: "OpenStreetMap contributors, via the Overpass API",
            url: "https://www.openstreetmap.org/copyright",
            licence: "ODbL 1.0",
            date: locale_date(js::get(g("osm"), "at")),
            basis: "measured",
            notes: format!("Road class, lanes, speed limit, one-way, roundabouts, signals and cycleways from OSM tags; where a tag is missing Spare Construct uses the road-class default. Good, but not survey-grade.{}", records(g("osm"))),
        },
        DataSource {
            what: "State highway counts",
            source: "NZTA open data: State highway traffic monitoring sites",
            url: "https://opendata-nzta.opendata.arcgis.com/",
            licence: "CC BY 4.0",
            date: locale_date(js::get(g("nzta"), "at")),
            basis: "measured",
            notes: format!("AADT by direction for five years and heavy-vehicle share; peak hour = AADT \u{d7} the peak share in Settings. Virtual sites are NZTA estimates and are left out unless included.{}", records(g("nzta"))),
        },
        DataSource {
            what: "Local road counts",
            source: "Auckland Transport: Average Daily Traffic Counts",
            url: "https://data-atgis.opendata.arcgis.com/",
            licence: "AT open data terms (check before republishing)",
            date: locale_date(js::get(g("at"), "at")),
            basis: "measured",
            notes: format!("Latest count per site with its measured peak hour, time and vehicle mix.{}", records(g("at"))),
        },
        DataSource {
            what: "Crashes",
            source: "NZTA Crash Analysis System (CAS)",
            url: "https://opendata-nzta.opendata.arcgis.com/",
            licence: "CC BY 4.0",
            date: locale_date(js::get(g("cas"), "at")),
            basis: "measured",
            notes: cas_notes,
        },
        DataSource {
            what: "Your counts",
            source: "Your CSV file",
            url: "",
            licence: "Yours",
            date: locale_date(js::get(g("user"), "at")),
            basis: "user",
            notes: format!("SCATS, tube or survey counts you load.{}", records(g("user"))),
        },
        DataSource {
            what: "Background maps",
            source: "Esri, OpenStreetMap or LINZ Basemaps tiles",
            url: "https://basemaps.linz.govt.nz/",
            licence: "Esri terms; OSM ODbL; LINZ CC BY 4.0",
            date: None,
            basis: "measured",
            notes: "Pictures only; the model never reads them.".to_string(),
        },
        DataSource {
            what: "Trip rates",
            source: "Spare Construct defaults in the range of the RTA Guide and NZ Trips Database",
            url: "https://www.nzta.govt.nz/resources/research/reports/453/",
            licence: "",
            date: None,
            basis: "assumed",
            notes: format!("Indicative, e.g. houses {rate} trips per dwelling in the AM peak. Replace with surveyed or NZTA Research Report 453 rates."),
        },
        DataSource {
            what: "Background growth",
            source: "Settings",
            url: "",
            licence: "",
            date: None,
            basis: "assumed",
            notes: format!("{growth}% a year from {base_year}, {}.", if compound { "compound" } else { "linear (% of base-year traffic)" }),
        },
    ]
}

/* ---------- Kit.line ---------- */

/// One line of a chart.
pub(crate) struct Series {
    pub pts: Vec<[f64; 2]>,
    pub colour: &'static str,
    pub label: String,
    pub y_max: f64,
}

/// `Kit.line(series, { xMax, yMax, yLabel, xLabels })`: a time series chart with its legend.
pub(crate) fn kit_line(series: &[Series], x_max: f64, y_max: f64, y_label: &str, x_labels: [&str; 2]) -> String {
    let (w, h) = (560.0f64, 140.0f64);
    let px = |x: f64| 40.0 + (w - 50.0) * x / x_max;
    let paths: String = series
        .iter()
        .map(|s| {
            let ym = if s.y_max == 0.0 || s.y_max.is_nan() { y_max } else { s.y_max };
            let py = |y: f64| h - 22.0 - (h - 34.0) * y / ym;
            let d: String = s
                .pts
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{}{},{}", if i > 0 { "L" } else { "M" }, to_fixed(px(p[0]), 1), to_fixed(py(p[1]), 1)))
                .collect();
            format!("<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.6\"/>", s.colour)
        })
        .collect();
    let n = number_to_js_string;
    format!(
        "<svg class=\"mini-chart\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" style=\"height:150px\">\n        <line x1=\"40\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y1}\" stroke=\"var(--line)\"/><line x1=\"40\" y1=\"12\" x2=\"{x2}\" y2=\"12\" stroke=\"var(--line-2)\"/>\n        {paths}\n        <text x=\"36\" y=\"16\" text-anchor=\"end\">{yl}</text><text x=\"36\" y=\"{y3}\" text-anchor=\"end\">0</text>\n        <text x=\"40\" y=\"{y4}\">{xl0}</text><text x=\"{x2}\" y=\"{y4}\" text-anchor=\"end\">{xl1}</text></svg><div class=\"hud-row\" style=\"font-size:12px;gap:14px\">{legend}</div>",
        w = n(w),
        h = n(h),
        y1 = n(h - 22.0),
        x2 = n(w - 10.0),
        paths = paths,
        yl = y_label,
        y3 = n(h - 20.0),
        y4 = n(h - 6.0),
        xl0 = x_labels[0],
        xl1 = x_labels[1],
        legend = series
            .iter()
            .filter(|s| !s.label.is_empty())
            .map(|s| format!("<span style=\"color:{}\">\u{2501} {}</span>", s.colour, s.label))
            .collect::<String>()
    )
}

/* ---------- the standards and assumptions register ---------- */

/// One row of the standards and assumptions register.
pub(crate) struct AssumptionRow {
    pub group: &'static str,
    pub name: String,
    pub edition: String,
    pub verified: bool,
    pub corridor: bool,
}

/// `assumptionRows(P)`: every standard Spare Construct holds as data, with its source and whether it is an assumption.
pub(crate) fn assumption_rows() -> Vec<AssumptionRow> {
    let mut rows: Vec<AssumptionRow> = ENGINE_ASSUMPTIONS
        .iter()
        .map(|(name, edition, verified, corridor)| AssumptionRow {
            group: "Engine",
            name: name.to_string(),
            edition: edition.to_string(),
            verified: *verified,
            corridor: *corridor,
        })
        .collect();
    let report = |name: &str, edition: &str| AssumptionRow {
        group: "Report",
        name: name.to_string(),
        edition: edition.to_string(),
        verified: false,
        corridor: edition.to_ascii_lowercase().contains("spare construct assumption"),
    };
    rows.push(report(rr422::NAME, rr422::EDITION));
    rows.push(report(consts::cec::NAME, consts::cec::EDITION));
    rows.push(report(consts::hrig::NAME, consts::hrig::EDITION));
    rows.push(report(consts::ssm::NAME, consts::ssm::EDITION));
    rows.push(report(consts::ped::NAME, consts::ped::EDITION));
    rows.push(AssumptionRow {
        group: "Report",
        name: "Network walk distance factor".to_string(),
        edition: walk_factor::BASIS.to_string(),
        verified: false,
        corridor: true,
    });
    rows.push(AssumptionRow {
        group: "Settings",
        name: "Trip rates, peak-to-daily factor, heavy-vehicle share, value of time".to_string(),
        edition: "Spare Construct indicative defaults unless changed (Appendix A)".to_string(),
        verified: false,
        corridor: true,
    });
    let _ = (&CEC_MODELS, esc("x"));
    rows
}
