//! The data pack's README and project file.

use super::doc::iso_minute;
use super::js;
use super::pack::PackFile;
use super::sections_front::vintage_rows;
use super::shared::model_version;
use super::texts::{COLS, FILES};
use super::{Rep, Sel};
use crate::edit::json::JsonValue;
use crate::results::Res;

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `\bx\b` for a one-letter word.
fn lone(k: &str, x: char) -> bool {
    let cs: Vec<char> = k.chars().collect();
    cs.iter().enumerate().any(|(i, c)| {
        *c == x && (i == 0 || !is_word(cs[i - 1])) && (i + 1 == cs.len() || !is_word(cs[i + 1]))
    })
}

/// `/veh\/h|%|km\/h|\bm\b|\bs\b/.test(k)`.
fn has_unit(k: &str) -> bool {
    k.contains("veh/h") || k.contains('%') || k.contains("km/h") || lone(k, 'm') || lone(k, 's')
}

fn gj_unit(k: &str) -> &'static str {
    match k {
        "id" => "link id",
        "name" => "road name",
        "cls" => "road class",
        "status" => "existing, proposed or closed",
        "speed" => "km/h (coded speed)",
        "lanesAB" => "lanes A to B",
        "lanesBA" => "lanes B to A",
        "busLane" => "true or false",
        "cycle" => "cycle facility: none, painted or separated",
        "length_m" => "m",
        "vol_ab" => "veh/h A to B",
        "vc_ab" => "volume/capacity ratio",
        "spd_ab" => "km/h",
        "vol_ba" => "veh/h B to A",
        "vc_ba" => "volume/capacity ratio",
        "spd_ba" => "km/h",
        _ => "-",
    }
}

fn csv_unit(name: &str) -> Option<&'static str> {
    Some(match name {
        "corridor-sidra-volumes.csv" => {
            "veh/h where the column says so; Period AM or PM; Site is the SIDRA site number"
        }
        "anop.csv" => "AT ANOP tool v10.2 input sheet columns; LoS A to F (indicative, D5)",
        "screenlines.csv" => "veh/h; diff_pct a fraction; geh unitless; completeness a fraction",
        _ => return None,
    })
}

/// `readme(P, sel, files, missing)`: the data pack's README.
pub(crate) fn readme(r: &Rep, sel: &Sel, files: &[PackFile], missing: &[String]) -> String {
    let mut l: Vec<String> = Vec::new();
    let d1 = super::consts::disclaimer_text("D1", None, None);
    let d12 = super::consts::disclaimer_text(
        "D12",
        None,
        Some("SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook"),
    );
    l.push(format!("Spare Construct ITA data pack: {}", r.name()));
    l.push(format!(
        "Made {} UTC with {}.",
        iso_minute(r.now_ms),
        model_version(r)
    ));
    l.push(String::new());
    l.push("DISCLAIMER D1 (screening status)".to_string());
    l.push(d1);
    l.push(String::new());
    l.push("DISCLAIMER D12 (hand-over)".to_string());
    l.push(d12);
    l.push(String::new());
    l.push("Format: every CSV is UTF-8 with a byte-order mark, so Excel shows macrons (for example \u{14c}t\u{101}huhu) correctly. The first row of every CSV is Disclaimer D1; skip one row when reading (pandas: skiprows=1). The one exception is corridor-sidra-volumes.csv, the SIDRA hand-over file: it has no disclaimer row so the SIDRA loader reads it as is (D1 and D12 above apply to it). Dates are ISO (YYYY-MM-DD). Units are listed below, not in the headers.".to_string());
    l.push(String::new());
    l.push("SCENARIOS".to_string());
    for (i, s) in r.project.scenarios.iter().enumerate() {
        let role = s.role.as_str().filter(|x| !x.is_empty());
        l.push(format!(
            "- {}: {} ({}{}){}",
            s.id,
            s.name.as_str().unwrap_or(""),
            crate::jsnum::number_to_js_string(crate::standards::year_of(Some(s), r.st())),
            role.map_or(String::new(), |x| format!(", {x}")),
            if Some(i) == sel.with {
                " [assessed]"
            } else if Some(i) == sel.without {
                " [without]"
            } else if Some(i) == sel.mitigation {
                " [mitigation]"
            } else {
                ""
            }
        ));
    }
    l.push(String::new());
    l.push("FILES".to_string());
    for f in files {
        let text = FILES
            .iter()
            .find(|(n, _)| *n == f.name)
            .map(|(_, t)| *t)
            .filter(|t| !t.is_empty());
        l.push(format!(
            "- {}{}",
            f.name,
            match (text, &f.of) {
                (Some(t), _) => format!(": {t}"),
                (None, Some(of)) => format!(": the columns of {of}, for one scenario and period"),
                _ => String::new(),
            }
        ));
    }
    l.push("- README.txt: this file".to_string());
    l.push(String::new());
    l.push("by-scenario/<project>-<scenario>-<period>-<year>-<table>.csv: the rows of links.csv, junction_summary.csv, junction_movements.csv and turning_movements.csv split by scenario and period, so each file names its period and year. Same columns and units as the combined file.".to_string());
    if !missing.is_empty() {
        l.push(String::new());
        l.push("NOT INCLUDED".to_string());
        for m in missing {
            l.push(format!("- {m}"));
        }
    }
    l.push(String::new());
    l.push("COLUMNS (name: unit or meaning)".to_string());
    for (f, cols) in COLS.iter() {
        l.push(String::new());
        l.push(f.to_string());
        for (k, u) in cols.iter() {
            l.push(format!("  {k}: {}", if u.is_empty() { "-" } else { u }));
        }
    }
    // files made outside the dictionary: their header row, as written (units are in the names or listed here)
    for f in files.iter().filter(|f| {
        f.name.ends_with(".csv")
            && !COLS.iter().any(|(n, _)| *n == f.name)
            && f.of.is_none()
            && !f.name.starts_with("od_")
    }) {
        let text = f.data.strip_prefix('\u{feff}').unwrap_or(&f.data);
        let rows: Vec<&str> = text
            .split('\n')
            .map(|x| x.strip_suffix('\r').unwrap_or(x))
            .filter(|x| !x.is_empty() && !(x.starts_with('#') || x.starts_with("\"#")))
            .collect();
        let head: Vec<String> = rows
            .first()
            .map(|h| {
                h.split(',')
                    .map(|x| {
                        let x = x.strip_prefix('"').unwrap_or(x);
                        x.strip_suffix('"').unwrap_or(x).to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();
        l.push(String::new());
        l.push(f.name.clone());
        if let Some(u) = csv_unit(&f.name) {
            l.push(format!("  ({u})"));
        }
        for k in head {
            l.push(format!(
                "  {k}: {}",
                if has_unit(&k) {
                    "unit in the name"
                } else {
                    "-"
                }
            ));
        }
    }
    if let Some(gj) = files.iter().find(|f| f.name == "network.geojson") {
        let props: Vec<String> = JsonValue::parse(&gj.data)
            .ok()
            .and_then(|o| {
                js::idx(js::get(Some(&o), "features"), 0)
                    .and_then(|f| js::get(Some(f), "properties"))
                    .map(|p| {
                        js::entries(Some(p))
                            .into_iter()
                            .map(|(k, _)| k.to_string())
                            .collect()
                    })
            })
            .unwrap_or_default();
        l.push(String::new());
        l.push("network.geojson (WGS84; the file carries D1 and the model version)".to_string());
        l.push("  Link features (LineString), properties:".to_string());
        for k in props {
            l.push(format!("    {k}: {}", gj_unit(&k)));
        }
        l.push("  Junction features (Point): id, control, delay_s (s), los (A to F). Zone features (Point): id, zone (name), kind, land-use quantities in their units, gateway flows (veh/h).".to_string());
    }
    l.push(String::new());
    l.push("od_<period>.csv: rows are origin zones, columns destination zones, veh/h in the peak hour.".to_string());
    l.push("settings.json: every model setting. project.corridor.json: the full project; open it in Spare Construct (Menu \u{203a} Open) to reproduce every run.".to_string());
    l.push(String::new());
    l.push("DATA VINTAGE (Source | Dataset | Query date | Records | Licence)".to_string());
    for row in vintage_rows(r) {
        l.push(row.join(" | "));
    }
    format!("{}\r\n", l.join("\r\n"))
}

/// The JSON text of a result as `JSON.stringify` writes it: non-finite numbers are null.
fn res_json(res: &Res) -> JsonValue {
    fn fix(v: &mut JsonValue) {
        match v {
            JsonValue::String(s) if s == "NaN" || s == "Infinity" || s == "-Infinity" => {
                *v = JsonValue::Null
            }
            JsonValue::Array(a) => a.iter_mut().for_each(fix),
            JsonValue::Object(m) => m.iter_mut().for_each(|(_, x)| fix(x)),
            _ => {}
        }
    }
    let text = serde_json::to_string(res).expect("a result serialises");
    let mut v = JsonValue::parse(&text).expect("a result's text is JSON");
    fix(&mut v);
    v
}

/// `Store.serialise` drops these keys wherever they are.
fn strip_keys(v: &mut JsonValue) {
    match v {
        JsonValue::Array(a) => a.iter_mut().for_each(strip_keys),
        JsonValue::Object(m) => {
            for k in ["paths", "arcKeys", "settingsNotice"] {
                m.remove(k);
            }
            m.iter_mut().for_each(|(_, x)| strip_keys(x));
        }
        _ => {}
    }
}

/// `DataHub.D()`: the project's data block with its counts, crashes, loaded, jt and use keys in place (the prototype's pack has run it).
fn normalise_data(p: &mut JsonValue) {
    if !p.get("data").is_some_and(JsonValue::truthy) {
        p.set("data", JsonValue::object());
    }
    let Some(d) = p.get_mut("data") else {
        return;
    };
    for (k, empty_array) in [
        ("counts", true),
        ("crashes", true),
        ("loaded", false),
        ("jt", true),
    ] {
        if !d.get(k).is_some_and(JsonValue::truthy) {
            d.set(
                k,
                if empty_array {
                    JsonValue::Array(Vec::new())
                } else {
                    JsonValue::object()
                },
            );
        }
    }
    let mut u = JsonValue::object();
    for (k, v) in [
        ("nzta", true),
        ("at", true),
        ("user", true),
        ("virtual", false),
        ("ramm", true),
    ] {
        u.set(k, JsonValue::Bool(v));
    }
    if let Some(JsonValue::Object(m)) = d.get("use").filter(|x| x.truthy()).cloned() {
        for (k, v) in m.iter() {
            u.set(k.to_string(), v.clone());
        }
    }
    d.set("use", u);
}

/// `Store.serialise(P)`: the whole project as JSON text, with each scenario's stored results in place.
pub(crate) fn project_json(r: &Rep) -> String {
    let mut p = r.raw.clone();
    normalise_data(&mut p);
    if let Some(JsonValue::Array(scs)) = p.get_mut("scenarios") {
        for (i, sc) in scs.iter_mut().enumerate() {
            let mut o = JsonValue::object();
            if let Some(res) = r.results.get(i) {
                for (per, x) in res.iter() {
                    o.set(per.to_string(), res_json(x));
                }
            }
            sc.set("res", o);
        }
    }
    strip_keys(&mut p);
    p.stringify()
}
