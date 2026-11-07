//! Your own counts from a CSV file (`DataHub.importCSV`), and the project's data store (`DataHub.D`).

use crate::calibrate::parse_csv;
use crate::edit::IdSource;
use crate::jsnum::{Maybe, js_trim, json_number, string_to_number};
use crate::project::{OrderedMap, Project, ProjectData};
use serde_json::{Map, Value, json};

/// `D()`: the project's data store with the keys it always has (counts, crashes, loaded, jt and the source switches), made when missing.
///
/// A missing or empty value becomes the default (counts, crashes and jt an empty list, loaded empty); the switches are
/// nzta, at, user and ramm on and virtual off, with the project's own switches over them.
pub fn ensure_data(p: &mut Project) {
    if !p.data.is_set() {
        p.data = Maybe::Set(ProjectData {
            loaded: Maybe::Missing,
            msm: Maybe::Missing,
            controlled: Maybe::Missing,
            extra: Map::new(),
        });
    }
    let d = p.data.get_mut().expect("just made");
    for k in ["counts", "crashes", "jt"] {
        let falsy = matches!(
            d.extra.get(k),
            None | Some(Value::Null) | Some(Value::Bool(false))
        );
        if falsy {
            d.extra.insert(k.to_string(), json!([]));
        }
    }
    if !d.loaded.is_set() {
        d.loaded = Maybe::Set(OrderedMap::new());
    }
    let mut use_ = Map::new();
    for (k, v) in [
        ("nzta", true),
        ("at", true),
        ("user", true),
        ("virtual", false),
        ("ramm", true),
    ] {
        use_.insert(k.to_string(), json!(v));
    }
    if let Some(Value::Object(own)) = d.extra.get("use") {
        for (k, v) in own {
            use_.insert(k.clone(), v.clone());
        }
    }
    d.extra.insert("use".to_string(), Value::Object(use_));
}

/// `dirOf(s)`: the first count direction named in the text as a whole word (NB, SB, EB or WB, any case), else None.
pub fn dir_of(s: &str) -> Option<&'static str> {
    let up: Vec<char> = s.to_uppercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for i in 0..up.len().saturating_sub(1) {
        let two: String = up[i..i + 2].iter().collect();
        let which = match two.as_str() {
            "NB" => "NB",
            "SB" => "SB",
            "EB" => "EB",
            "WB" => "WB",
            _ => continue,
        };
        if (i == 0 || !word(up[i - 1])) && (i + 2 == up.len() || !word(up[i + 2])) {
            return Some(which);
        }
    }
    None
}

/// `importCSV(text)`: replaces the project's own counts (source "user") with those in a CSV file; gives how many were read.
///
/// Columns, in any order and case: lat and lng (or latitude, longitude, lon, long, x, y), at least one of am, pm and
/// adt (or aadt, daily; am_peak, pm_peak and so on), and optionally name (site, location, road) and dir (direction;
/// NB, SB, EB or WB makes the count directional). Rows without a number in lat and lng are skipped. Counts of other
/// sources stay. The time the file was read is kept in `loaded.user`.
pub fn import_csv(p: &mut Project, text: &str, ids: &mut dyn IdSource) -> Result<usize, String> {
    let rows = parse_csv(text);
    if rows.len() < 2 {
        return Err("The file has no data rows.".to_string());
    }
    let h: Vec<String> = rows[0].iter().map(|s| js_trim(s).to_lowercase()).collect();
    let col = |names: &[&str]| h.iter().position(|x| names.contains(&x.as_str()));
    let i_lat = col(&["lat", "latitude", "y"]);
    let i_lng = col(&["lng", "lon", "long", "longitude", "x"]);
    let i_name = col(&["name", "site", "location", "road"]);
    let i_am = col(&["am", "am_peak", "am peak", "am_vph"]);
    let i_pm = col(&["pm", "pm_peak", "pm peak", "pm_vph"]);
    let i_adt = col(&["adt", "aadt", "daily"]);
    let i_dir = col(&["dir", "direction"]);
    let (Some(i_lat), Some(i_lng)) = (i_lat, i_lng) else {
        return Err("Columns lat and lng are required.".to_string());
    };
    if i_am.is_none() && i_pm.is_none() && i_adt.is_none() {
        return Err("Add at least one of: am, pm, adt.".to_string());
    }
    ensure_data(p);
    // `+r[i]`: a missing cell is NaN, an empty one 0
    let cell = |r: &Vec<String>, i: usize| r.get(i).map_or(f64::NAN, |s| string_to_number(s));
    // `+x || null`
    let volume = |r: &Vec<String>, i: Option<usize>| -> Value {
        match i {
            Some(i) => {
                let v = cell(r, i);
                if v.is_nan() || v == 0.0 {
                    Value::Null
                } else {
                    json_number(v)
                }
            }
            None => Value::Null,
        }
    };
    let mut list: Vec<Value> = Vec::new();
    for (k, r) in rows[1..].iter().enumerate() {
        let (lat, lng) = (cell(r, i_lat), cell(r, i_lng));
        if !lat.is_finite() || !lng.is_finite() {
            continue;
        }
        let dir = i_dir.and_then(|i| dir_of(r.get(i).map_or("", |s| s.as_str())));
        let mut o = Map::new();
        o.insert("id".to_string(), json!(format!("user:{k}:{}", ids.uid(""))));
        o.insert("src".to_string(), json!("user"));
        match i_name {
            Some(i) => {
                if let Some(s) = r.get(i) {
                    o.insert("name".to_string(), json!(s));
                }
            }
            None => {
                o.insert("name".to_string(), json!(format!("Count {}", k + 1)));
            }
        }
        o.insert("lat".to_string(), json_number(lat));
        o.insert("lng".to_string(), json_number(lng));
        o.insert("adt".to_string(), volume(r, i_adt));
        o.insert("am".to_string(), volume(r, i_am));
        o.insert("pm".to_string(), volume(r, i_pm));
        o.insert("dir".to_string(), dir.map_or(Value::Null, |d| json!(d)));
        o.insert("directional".to_string(), json!(dir.is_some()));
        o.insert("note".to_string(), json!("Imported from your file"));
        list.push(Value::Object(o));
    }
    let n = list.len();
    let d = p.data.get_mut().expect("ensured");
    let mut kept: Vec<Value> = match d.extra.get("counts") {
        Some(Value::Array(a)) => a
            .iter()
            .filter(|c| c.get("src").and_then(Value::as_str) != Some("user"))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    kept.extend(list);
    d.extra.insert("counts".to_string(), Value::Array(kept));
    if let Some(l) = d.loaded.get_mut() {
        l.insert(
            "user".to_string(),
            json!({ "at": json_number(ids.now_ms()), "n": n }),
        );
    }
    Ok(n)
}
