//! Your own counts from a CSV file (`DataHub.importCSV`), and the project's data store (`DataHub.D`).

use crate::calibrate::parse_csv;
use crate::edit::IdSource;
use crate::jsnum::{Maybe, js_trim, json_number, string_to_number};
use crate::project::{OrderedMap, Project, ProjectData};
use serde_json::{Map, Value, json};

// `D()` as a value: the project's data store taken out of the project (a new one when missing), with the keys it
// always has filled in. The caller puts it back.
fn take_filled(p: &mut Project) -> ProjectData {
    let mut d = std::mem::take(&mut p.data)
        .into_option()
        .unwrap_or_else(|| ProjectData { loaded: Maybe::Missing, msm: Maybe::Missing, controlled: Maybe::Missing, extra: Map::new() });
    for k in ["counts", "crashes", "jt"] {
        let falsy = matches!(d.extra.get(k), None | Some(Value::Null | Value::Bool(false)));
        if falsy {
            d.extra.insert(k.to_string(), json!([]));
        }
    }
    if !d.loaded.is_set() {
        d.loaded = Maybe::Set(OrderedMap::new());
    }
    let mut use_ = Map::new();
    for (k, v) in [("nzta", true), ("at", true), ("user", true), ("virtual", false), ("ramm", true)] {
        use_.insert(k.to_string(), json!(v));
    }
    if let Some(Value::Object(own)) = d.extra.get("use") {
        for (k, v) in own {
            use_.insert(k.clone(), v.clone());
        }
    }
    d.extra.insert("use".to_string(), Value::Object(use_));
    d
}

/// `D()`: the project's data store with the keys it always has (counts, crashes, loaded, jt and the source switches), made when missing.
///
/// A missing or empty value becomes the default (counts, crashes and jt an empty list, loaded empty); the switches are
/// nzta, at, user and ramm on and virtual off, with the project's own switches over them.
pub fn ensure_data(p: &mut Project) {
    let d = take_filled(p);
    p.data = Maybe::Set(d);
}

/// `dirOf(s)`: the first count direction named in the text as a whole word (NB, SB, EB or WB, any case), else None.
#[must_use]
pub fn dir_of(s: &str) -> Option<&'static str> {
    let up: Vec<char> = s.to_uppercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for (i, two) in up.windows(2).enumerate() {
        let which = match two {
            ['N', 'B'] => "NB",
            ['S', 'B'] => "SB",
            ['E', 'B'] => "EB",
            ['W', 'B'] => "WB",
            _ => continue,
        };
        // Bound: i + 2 is at most up.len()
        let before = i.checked_sub(1).and_then(|j| up.get(j));
        let after = up.get(i.saturating_add(2));
        if before.is_none_or(|&c| !word(c)) && after.is_none_or(|&c| !word(c)) {
            return Some(which);
        }
    }
    None
}

// The columns of an imported file (lat and lng required).
struct Cols {
    lat: usize,
    lng: usize,
    name: Option<usize>,
    am: Option<usize>,
    pm: Option<usize>,
    adt: Option<usize>,
    dir: Option<usize>,
}

// `+r[i]`: a missing cell is NaN, an empty one 0
fn cell(row: &[String], i: usize) -> f64 {
    row.get(i).map_or(f64::NAN, |s| string_to_number(s))
}

// `+x || null`
fn volume(row: &[String], i: Option<usize>) -> Value {
    match i {
        Some(i) => {
            let v = cell(row, i);
            if v.is_nan() || v == 0.0 { Value::Null } else { json_number(v) }
        }
        None => Value::Null,
    }
}

// The count of the k-th data row, or None when it has no number in lat and lng.
fn count_of_row(row: &[String], k: usize, cols: &Cols, ids: &mut dyn IdSource) -> Option<Value> {
    let (lat, lng) = (cell(row, cols.lat), cell(row, cols.lng));
    if !lat.is_finite() || !lng.is_finite() {
        return None;
    }
    let dir = cols.dir.and_then(|i| dir_of(row.get(i).map_or("", |s| s.as_str())));
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(format!("user:{k}:{}", ids.uid(""))));
    obj.insert("src".to_string(), json!("user"));
    match cols.name {
        Some(i) => {
            if let Some(text) = row.get(i) {
                obj.insert("name".to_string(), json!(text));
            }
        }
        None => {
            // Bound: a row index, far below usize::MAX
            obj.insert("name".to_string(), json!(format!("Count {}", k.saturating_add(1))));
        }
    }
    obj.insert("lat".to_string(), json_number(lat));
    obj.insert("lng".to_string(), json_number(lng));
    obj.insert("adt".to_string(), volume(row, cols.adt));
    obj.insert("am".to_string(), volume(row, cols.am));
    obj.insert("pm".to_string(), volume(row, cols.pm));
    obj.insert("dir".to_string(), dir.map_or(Value::Null, |d| json!(d)));
    obj.insert("directional".to_string(), json!(dir.is_some()));
    obj.insert("note".to_string(), json!("Imported from your file"));
    Some(Value::Object(obj))
}

/// `importCSV(text)`: replaces the project's own counts (source "user") with those in a CSV file; gives how many were read.
///
/// Columns, in any order and case: lat and lng (or latitude, longitude, lon, long, x, y), at least one of am, pm and
/// adt (or aadt, daily; `am_peak`, `pm_peak` and so on), and optionally name (site, location, road) and dir (direction;
/// NB, SB, EB or WB makes the count directional). Rows without a number in lat and lng are skipped. Counts of other
/// sources stay. The time the file was read is kept in `loaded.user`.
///
/// # Errors
///
/// The prototype's message, and the project unchanged, when the file has no data rows, lacks a lat or lng column, or
/// has none of am, pm and adt.
pub fn import_csv(p: &mut Project, text: &str, ids: &mut dyn IdSource) -> Result<usize, String> {
    let rows = parse_csv(text);
    let Some((head, body)) = rows.split_first().filter(|_| rows.len() >= 2) else {
        return Err("The file has no data rows.".to_string());
    };
    let head: Vec<String> = head.iter().map(|text| js_trim(text).to_lowercase()).collect();
    let col = |names: &[&str]| head.iter().position(|h| names.contains(&h.as_str()));
    let (Some(lat), Some(lng)) = (col(&["lat", "latitude", "y"]), col(&["lng", "lon", "long", "longitude", "x"])) else {
        return Err("Columns lat and lng are required.".to_string());
    };
    let cols = Cols {
        lat,
        lng,
        name: col(&["name", "site", "location", "road"]),
        am: col(&["am", "am_peak", "am peak", "am_vph"]),
        pm: col(&["pm", "pm_peak", "pm peak", "pm_vph"]),
        adt: col(&["adt", "aadt", "daily"]),
        dir: col(&["dir", "direction"]),
    };
    if cols.am.is_none() && cols.pm.is_none() && cols.adt.is_none() {
        return Err("Add at least one of: am, pm, adt.".to_string());
    }
    // `D()`: taken out here and put back below
    let mut data = take_filled(p);
    let mut list: Vec<Value> = Vec::new();
    for (k, row) in body.iter().enumerate() {
        if let Some(rec) = count_of_row(row, k, &cols, ids) {
            list.push(rec);
        }
    }
    let read = list.len();
    let mut kept: Vec<Value> = match data.extra.get("counts") {
        Some(Value::Array(old)) => old.iter().filter(|rec| rec.get("src").and_then(Value::as_str) != Some("user")).cloned().collect(),
        _ => Vec::new(),
    };
    kept.extend(list);
    data.extra.insert("counts".to_string(), Value::Array(kept));
    if let Some(loaded) = data.loaded.get_mut() {
        loaded.insert("user".to_string(), json!({ "at": json_number(ids.now_ms()), "n": read }));
    }
    p.data = Maybe::Set(data);
    Ok(read)
}
