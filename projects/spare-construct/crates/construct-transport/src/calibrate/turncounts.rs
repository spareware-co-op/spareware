//! Turning counts in the calibration panel: reading count files (long, wide and SCATS layouts), peak hours and peak hour
//! factors, flags for weekend and school-holiday counts, and matching each site to the junction it sits on
//! (`Calibration.parseTurnCSV`, `peaks`, `flagDate`, `repeak`, `importTurnCSV`, `matchSites`).
//!
//! Not ported: the conversion of a Spare Construct SIDRA volumes export (`Junctions.asCountCSV`, in the junctions
//! feature); such a file is read as an ordinary count file.

use super::common::{CalibCfg, hhmm, iso_date, to_min};
use super::criteria::school_terms;
use super::store::{BinCounts, CalibStore, ClassCounts, Peak, TurnSite};
use crate::datahub::Count;
use crate::edit::net::{degree, len as link_len, links_at, nearest_node};
use crate::edit::{IdSource, sites};
use crate::jsnum::{JsNum, Maybe, Num, f64_to_usize, js_trim, math_max, string_to_number, usize_to_f64, value_to_number};
use crate::project::{OrderedMap, Project, Scenario};
use crate::util::{fmt, r3, r7};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

/// `LEGS`: the eight legs of a junction by compass point, where traffic comes from.
pub const LEGS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

/// `legBearing(l)`: the bearing of a leg's compass point (N 0, NE 45, ...); -45 for an unknown leg (`indexOf` is -1).
#[must_use]
pub fn leg_bearing(l: &str) -> f64 {
    LEGS.iter().position(|x| *x == l).map_or(-45.0, |i| usize_to_f64(i) * 45.0)
}

fn is_move(mv: &str) -> bool {
    matches!(mv, "L" | "T" | "R" | "U" | "L1" | "L2" | "L3" | "T1" | "R1" | "R2" | "R3")
}

const VEH: [&str; 3] = ["LV", "HV", "BUS"];

/// `classOf(v)`: a vehicle class name from a count file onto LV, HV, BUS or CYC (None when it is not recognised).
pub fn class_of(v: &str) -> Option<&'static str> {
    let key: String = v
        .trim_matches(crate::jsnum::is_js_space)
        .to_uppercase()
        .chars()
        .filter(|c| !(crate::jsnum::is_js_space(*c) || *c == '_' || *c == '.' || *c == '-'))
        .collect();
    Some(match key.as_str() {
        "LV" | "L" | "LIGHT" | "CAR" | "CARS" | "PC" | "LCV" | "VAN" | "UTE" | "MC" | "MCY" | "MOTORCYCLE" | "TAXI" => "LV",
        "BUS" | "BUSES" | "COACH" => "BUS",
        "HV" | "H" | "HEAVY" | "MCV" | "HCV" | "HCVI" | "HCVII" | "HCV1" | "HCV2" | "TRUCK" | "TRUCKS" => "HV",
        "CYC" | "CYCLE" | "CYCLES" | "BIKE" | "BICYCLE" => "CYC",
        _ => return None,
    })
}

/// `U.parseCSV(text)`: the rows of a CSV text (quoted fields, doubled quotes, any line ending), rows with only blank cells left out.
#[must_use]
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut cs = text.chars().peekable();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    while let Some(c) = cs.next() {
        if q {
            if c == '"' && cs.peek() == Some(&'"') {
                cur.push('"');
                cs.next();
            } else if c == '"' {
                q = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            q = true;
        } else if c == ',' {
            row.push(std::mem::take(&mut cur));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && cs.peek() == Some(&'\n') {
                cs.next();
            }
            row.push(std::mem::take(&mut cur));
            rows.push(std::mem::take(&mut row));
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows.into_iter().filter(|r| r.iter().any(|v| !js_trim(v).is_empty())).collect()
}

/// What reading a turning-count file gives (`parseTurnCSV`'s result).
#[derive(Clone, Debug)]
pub struct ParsedTurnCsv {
    /// The sites read, with their peak hours.
    pub sites: Vec<TurnSite>,
    /// Rows that could not be read.
    pub skipped: usize,
    /// The file line numbers of the first ten.
    pub skipped_rows: Vec<usize>,
    /// Why rows were skipped ("3 \u{d7} time not readable (use HH:MM)").
    pub reasons: Vec<String>,
    /// The file was a modelled volumes export (never: see the module note).
    pub from_model: bool,
}

const SHARED_LANE_FLAG: &str = "shared-lane detector counts split equally (Spare Construct assumption)";

/// `JSON.stringify(cfg().windows)`: the peak-hour windows as text, in the order they are given.
#[must_use]
pub fn windows_json(cfg: &CalibCfg) -> String {
    let mut out = String::from("{");
    for (k, (p, w)) in cfg.windows.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        // Writing text, or a pair of texts, as JSON cannot fail
        out.push_str(&serde_json::to_string(p).unwrap_or_default());
        out.push(':');
        out.push_str(&serde_json::to_string(&[&w[0], &w[1]]).unwrap_or_default());
    }
    out.push('}');
    out
}

fn cell(r: &[String], i: Option<usize>) -> String {
    match i.and_then(|i| r.get(i)) {
        Some(s) => js_trim(s).to_string(),
        None => String::new(),
    }
}

fn num_cell(r: &[String], i: Option<usize>) -> f64 {
    match i.and_then(|i| r.get(i)) {
        Some(s) => string_to_number(s),
        None => f64::NAN,
    }
}

fn col(h: &[String], names: &[&str]) -> Option<usize> {
    h.iter().position(|x| names.contains(&x.as_str()))
}

// /\d{1,4}[-/.]\d{1,2}[-/.]\d{2,4}/.test(s)
fn looks_dated(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    let digs = |from: usize, n: usize| -> bool {
        from.checked_add(n).and_then(|end| c.get(from..end)).is_some_and(|w| w.iter().all(char::is_ascii_digit))
    };
    let sep = |i: usize| c.get(i).is_some_and(|&ch| matches!(ch, '-' | '/' | '.'));
    // Bound: positions stay within the text's length plus 8, far below usize::MAX
    for i in 0..c.len() {
        for a in 1..=4usize {
            if !digs(i, a) || !sep(i.saturating_add(a)) {
                continue;
            }
            for b in 1..=2usize {
                let j = i.saturating_add(a).saturating_add(1);
                if digs(j, b) && sep(j.saturating_add(b)) {
                    let k = j.saturating_add(b).saturating_add(1);
                    if digs(k, 2) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// The counts of a key in a bin, added empty when missing (so always Some)
fn class_entry<'a>(bin: &'a mut BinCounts, key: &str) -> Option<&'a mut ClassCounts> {
    if !bin.contains_key(key) {
        bin.insert(key.to_string(), OrderedMap::new());
    }
    bin.get_mut(key)
}

fn add(s: &mut TurnSite, t: f64, iv: f64, leg: &str, mv: &str, cls: &str, n: f64) {
    if n < 0.0 || n.is_nan() {
        return;
    }
    // 5- or 10-minute intervals fold into 15-minute bins; as `as u32` does, this saturates and reads NaN as 0
    let bin_start = u32::try_from(f64_to_usize((t / 15.0).floor() * 15.0)).unwrap_or(u32::MAX);
    let bin = s.bins.entry(bin_start).or_default();
    if let Some(classes) = class_entry(bin, &format!("{leg}-{mv}")) {
        let cur = classes.get(cls).copied().unwrap_or(0.0);
        classes.insert(cls.to_string(), cur + n);
    }
    s.interval = if s.interval.truthy() { Num::number(math_max(s.interval.num(), iv)) } else { Num::number(iv) };
}

// A wide-layout column: (index, leg, movement, class)
type WideCol = (usize, String, String, String);

// The columns of a turning-count file, by position in the header row
struct Cols {
    time: usize,
    site: Option<usize>,
    name: Option<usize>,
    lat: Option<usize>,
    lng: Option<usize>,
    date: Option<usize>,
    interval: Option<usize>,
    from_leg: Option<usize>,
    movement: Option<usize>,
    class: Option<usize>,
    count: Option<usize>,
    detector: Option<usize>,
    road: Option<usize>,
    wide: Vec<WideCol>,
}

// The header row's names: trimmed, lower case, each run of white space as '_'
fn header_names(row: &[String]) -> Vec<String> {
    row.iter()
        .map(|s| {
            let t = js_trim(s).to_lowercase();
            // .replace(/\s+/g, '_')
            let mut out = String::new();
            let mut in_ws = false;
            for c in t.chars() {
                if crate::jsnum::is_js_space(c) {
                    if !in_ws {
                        out.push('_');
                    }
                    in_ws = true;
                } else {
                    out.push(c);
                    in_ws = false;
                }
            }
            out
        })
        .collect()
}

// The wide columns: `<leg>_<movement>` (light vehicles) or `<leg>_<movement>_<class>`
fn wide_columns(head: &[String]) -> Vec<WideCol> {
    let mut wide: Vec<WideCol> = Vec::new();
    for (i, x) in head.iter().enumerate() {
        let parts: Vec<&str> = x.split('_').collect();
        let ok_leg = |s: &str| ["n", "ne", "e", "se", "s", "sw", "w", "nw"].contains(&s);
        let ok_mv = |s: &str| ["l", "t", "r", "u", "l1", "l2", "l3", "t1", "r1", "r2", "r3", "in", "out"].contains(&s);
        let ok_cl = |s: &str| ["lv", "hv", "bus", "cyc"].contains(&s);
        match parts.as_slice() {
            [l, m] if ok_leg(l) && ok_mv(m) => wide.push((i, l.to_uppercase(), m.to_uppercase(), "LV".to_string())),
            [l, m, c] if ok_leg(l) && ok_mv(m) && ok_cl(c) => wide.push((i, l.to_uppercase(), m.to_uppercase(), c.to_uppercase())),
            _ => {}
        }
    }
    wide
}

// The columns of a file, or the prototype's message for a file it cannot read
fn columns(head: &[String]) -> Result<Cols, String> {
    let time = col(head, &["time_start", "time", "start", "interval_start"]).ok_or("Add a time_start column (HH:MM).")?;
    let cols = Cols {
        time,
        site: col(head, &["site_id", "site"]),
        name: col(head, &["site_name", "name"]),
        lat: col(head, &["lat", "latitude"]),
        lng: col(head, &["lng", "lon", "long", "longitude"]),
        date: col(head, &["date", "count_date"]),
        interval: col(head, &["interval_min", "interval"]),
        from_leg: col(head, &["from_leg", "leg", "approach"]),
        movement: col(head, &["movement", "mov", "turn"]),
        class: col(head, &["class", "vehicle_class"]),
        count: col(head, &["count", "volume", "vehicles"]),
        detector: col(head, &["detector", "det"]),
        road: col(head, &["from_road", "road"]),
        wide: wide_columns(head),
    };
    if cols.lat.is_none() || cols.lng.is_none() {
        return Err("Add lat and lng columns (WGS84) so each site can be matched to a junction.".to_string());
    }
    if cols.wide.is_empty() && (cols.from_leg.is_none() || cols.movement.is_none() || cols.count.is_none()) {
        return Err("Use the long layout (from_leg, movement, count columns) or the wide layout (columns such as N_L_LV).".to_string());
    }
    Ok(cols)
}

// Notes a skipped row: its file line number, and one more for its reason
fn skip(li: usize, w: &str, errs: &mut Vec<usize>, why: &mut Vec<(String, usize)>) {
    // Bound: row numbers and counts of rows of a file in memory, far below usize::MAX
    errs.push(li.saturating_add(2));
    match why.iter_mut().find(|(k, _)| k == w) {
        Some(e) => e.1 = e.1.saturating_add(1),
        None => why.push((w.to_string(), 1)),
    }
}

// siteOf(r): the date column, else the date part of a date-time in the time column; an unreadable date is kept as written
fn row_date(r: &[String], cols: &Cols) -> String {
    let d = cell(r, cols.date);
    if !d.is_empty() {
        d
    } else if looks_dated(&cell(r, Some(cols.time))) {
        cell(r, Some(cols.time))
    } else {
        String::new()
    }
}

// A new site for a row, or None when the row has no readable lat / lng
fn new_site(r: &[String], cols: &Cols, sid: &str, date: &str, name: &str, ids: &mut dyn IdSource) -> Option<TurnSite> {
    let lat = num_cell(r, cols.lat);
    let lng = num_cell(r, cols.lng);
    if !lat.is_finite() || !lng.is_finite() {
        return None;
    }
    let nm = cell(r, cols.name);
    let mut s = TurnSite {
        id: ids.uid("tc"),
        reference: Maybe::Set(JsNum::Text(sid.to_string())),
        name: Maybe::Set(JsNum::Text(if nm.is_empty() { format!("Turning count {sid}") } else { nm })),
        lat: Num::number(r7(lat)),
        lng: Num::number(r7(lng)),
        date: Maybe::Set(JsNum::Text(date.to_string())),
        src: Maybe::Set(JsNum::Text(if cols.detector.is_some() { "scats" } else { "mcc" }.to_string())),
        file: Maybe::Set(JsNum::Text(name.to_string())),
        role: Maybe::Set(JsNum::Text("valid".to_string())),
        obs_method: Maybe::Set(JsNum::Text("measured_hour".to_string())),
        interval: Maybe::Null,
        bins: BTreeMap::new(),
        leg_roads: OrderedMap::new(),
        flags: Vec::new(),
        at: Num::number(ids.now_ms()),
        peak: Maybe::Missing,
        peak_win: Maybe::Missing,
        node: Maybe::Missing,
        site: Maybe::Missing,
        bal: Maybe::Missing,
        audit: Maybe::Missing,
        extra: Map::new(),
    };
    if cols.detector.is_some() {
        s.flags.push("stop-line throughput (SCATS detectors)".to_string());
    }
    Some(s)
}

// Adds the counts of a long-layout row to its site, or gives why the row is skipped
fn read_long_row(s: &mut TurnSite, r: &[String], cols: &Cols, t: f64, iv: f64) -> Result<(), String> {
    let leg = cell(r, cols.from_leg).to_uppercase();
    let mv = cell(r, cols.movement).to_uppercase();
    let n = string_to_number(&cell(r, cols.count));
    let class_text = cell(r, cols.class);
    let veh_class = if class_text.is_empty() { Some("LV") } else { class_of(&class_text) };
    let Some(veh_class) = veh_class else {
        let shown: String = String::from_utf16_lossy(&class_text.encode_utf16().take(20).collect::<Vec<u16>>());
        return Err(format!("vehicle class \u{201c}{shown}\u{201d} not recognised"));
    };
    if !LEGS.contains(&leg.as_str()) {
        return Err("from_leg not N, NE \u{2026} NW".to_string());
    }
    if n < 0.0 || n.is_nan() {
        return Err("count not a number".to_string());
    }
    if cols.road.is_some() && !cell(r, cols.road).is_empty() {
        s.leg_roads.insert(leg.clone(), Maybe::Set(JsNum::Text(cell(r, cols.road))));
    }
    if mv == "IN" || mv == "OUT" || is_move(&mv) {
        add(s, t, iv, &leg, &mv, veh_class, n);
        return Ok(());
    }
    // SCATS shared lane: split equally between its movements
    if (2..=4).contains(&mv.chars().count()) && mv.chars().all(|c| matches!(c, 'L' | 'T' | 'R' | 'U')) {
        let ways = usize_to_f64(mv.chars().count());
        for x in mv.chars() {
            add(s, t, iv, &leg, &x.to_string(), veh_class, n / ways);
        }
        if !s.flags.iter().any(|f| f == SHARED_LANE_FLAG) {
            s.flags.push(SHARED_LANE_FLAG.to_string());
        }
        return Ok(());
    }
    Err("movement not L, T, R, U, IN or OUT".to_string())
}

/// `parseTurnCSV(text, name)`: reads a turning-count file.
///
/// Long layout (`site_id`, `site_name`, lat, lng, date, `time_start`, `interval_min`, `from_leg`, movement, class, count), wide
/// layout (`time_start` plus `<leg>_<movement>_<class>` columns) or SCATS (a detector column; a shared-lane movement such
/// as "LT" is split equally between its movements, a Spare Construct assumption). `from_leg` is where vehicles come
/// FROM; movement IN or OUT gives leg totals for turn estimation. Counts in 5- or 10-minute intervals fold into
/// 15-minute bins. Each site gets its peak hours (`cfg.windows`), flags for weekend and school-holiday counts, and the
/// role "valid". Errors with the prototype's messages.
///
/// # Errors
///
/// The prototype's message when the file has no data rows, lacks a time, lat or lng column, is in neither layout, or
/// has no row that could be read.
pub fn parse_turn_csv(text: &str, name: &str, cfg: &CalibCfg, ids: &mut dyn IdSource) -> Result<ParsedTurnCsv, String> {
    let raw0 = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rows = parse_csv(raw0);
    if rows.len() < 2 {
        return Err("The file has no data rows.".to_string());
    }
    // rows has a first row (checked above)
    let head = rows.first().map(|r| header_names(r)).unwrap_or_default();
    let cols = columns(&head)?;
    let mut sites: Vec<(String, TurnSite)> = Vec::new();
    let mut errs: Vec<usize> = Vec::new();
    let mut why: Vec<(String, usize)> = Vec::new();
    let windows = cfg.windows.clone();
    let wjson = windows_json(cfg);
    for (li, r) in rows.iter().skip(1).enumerate() {
        if r.iter().all(|x| js_trim(x).is_empty()) {
            continue; // blank line
        }
        let raw = row_date(r, &cols);
        let iso = iso_date(&raw);
        let sid = {
            let site = cell(r, cols.site);
            if site.is_empty() { "site 1".to_string() } else { site }
        };
        let date = iso.clone().unwrap_or_else(|| raw.clone());
        let key = format!("{sid}|{date}");
        let pos = if let Some(found) = sites.iter().position(|(k, _)| *k == key) {
            Some(found)
        } else if let Some(site) = new_site(r, &cols, &sid, &date, name, ids) {
            sites.push((key.clone(), site));
            Some(sites.len().saturating_sub(1))
        } else {
            None
        };
        let Some(pos) = pos else {
            skip(li, "no lat / lng", &mut errs, &mut why);
            continue;
        };
        // pos is always a place in sites (found there, or just pushed), so this never skips
        let Some((_, s)) = sites.get_mut(pos) else {
            skip(li, "no lat / lng", &mut errs, &mut why);
            continue;
        };
        let Some(t) = to_min(r.get(cols.time).map_or("", String::as_str)) else {
            skip(li, "time not readable (use HH:MM)", &mut errs, &mut why);
            continue;
        };
        let iv = {
            let given = num_cell(r, cols.interval);
            if cols.interval.is_some() && given > 0.0 { given } else { 15.0 }
        };
        if !cols.wide.is_empty() {
            for (i, leg, mv, cls) in &cols.wide {
                let n = r.get(*i).map_or(f64::NAN, |x| string_to_number(x));
                add(s, t, iv, leg, mv, cls, n);
            }
            continue;
        }
        if let Err(reason) = read_long_row(s, r, &cols, t, iv) {
            skip(li, &reason, &mut errs, &mut why);
        }
    }
    let mut list: Vec<TurnSite> = sites.into_iter().map(|(_, s)| s).filter(|s| !s.bins.is_empty()).collect();
    let reasons: Vec<String> = why.iter().map(|(w, k)| format!("{k} \u{d7} {w}")).collect();
    if list.is_empty() {
        return Err(format!(
            "No rows could be read{}. Check the time_start, from_leg (N, NE \u{2026} NW), movement (L, T, R, U), class and count columns.",
            if reasons.is_empty() { String::new() } else { format!(" ({})", reasons.join("; ")) }
        ));
    }
    for s in &mut list {
        s.interval = if s.interval.truthy() { s.interval.clone() } else { Num::number(15.0) };
        if s.interval.num() > 15.0 {
            s.flags.push(format!("{}-minute intervals: no peak hour factor", s.interval.js_string()));
        }
        peaks(s, &windows);
        s.peak_win = Maybe::Set(wjson.clone());
        flag_date(s);
    }
    Ok(ParsedTurnCsv {
        sites: list,
        skipped: errs.len(),
        skipped_rows: errs.iter().take(10).copied().collect(),
        reasons,
        from_model: false,
    })
}

fn weekday(y: i64, m: i64, d: i64) -> Option<i64> {
    // a real date only
    let dim = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 }
        }
        _ => return None,
    };
    if d < 1 || d > dim {
        return None;
    }
    let t: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    // Bound: y is a four-digit year and m is 1 to 12 here, so none of these steps fails
    let yy = if m < 3 { y.checked_sub(1)? } else { y };
    let month_offset = *t.get(usize::try_from(m.checked_sub(1)?).ok()?)?;
    let sum = yy
        .checked_add(yy / 4)?
        .checked_sub(yy / 100)?
        .checked_add(yy / 400)?
        .checked_add(month_offset)?
        .checked_add(d)?;
    Some(sum % 7)
}

/// `flagDate(s)`: flags a count made on a weekend or in the school holidays (or whose date could not be read).
pub fn flag_date(s: &mut TurnSite) {
    if !s.date.truthy() {
        return;
    }
    let date = s.date.js_string();
    let shape = date.chars().count() == 10
        && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    if !shape {
        let shown: String = String::from_utf16_lossy(&date.encode_utf16().take(20).collect::<Vec<u16>>());
        s.flags.push(format!(
            "count date \u{201c}{shown}\u{201d} could not be read, so the weekend and school-holiday checks were not made"
        ));
        return;
    }
    let y: i64 = date[0..4].parse().unwrap_or(0);
    let m: i64 = date[5..7].parse().unwrap_or(0);
    let d: i64 = date[8..10].parse().unwrap_or(0);
    let terms = school_terms(y);
    let dow = weekday(y, m, d);
    if matches!(dow, Some(0 | 6)) {
        s.flags.push("weekend count".to_string());
    }
    if let Some(t) = &terms
        && !t.iter().any(|(a, b)| date.as_str() >= *a && date.as_str() <= *b)
    {
        s.flags.push("counted in the school holidays (term dates to verify)".to_string());
    }
    if terms.is_none() {
        s.flags.push(format!(
            "school term dates for {y} are not in Spare Construct's table, so the school-holiday check was not made"
        ));
    }
}

fn is_leg_total(k: &str) -> bool {
    k.ends_with("-IN") || k.ends_with("-OUT")
}

// The best rolling hour found so far in a window
struct Best {
    start: u32,
    v: f64,
    ts: Vec<u32>,
    v15: Option<f64>,
}

/// `peaks(s, windows)`: the rolling peak hour of a site within each window, stepping by the count interval (4 x 15 minutes, 2 x 30 minutes or one hour); PHF = V60 / (4 x V15 max), so only 15-minute (or finer) counts have one. Replaces `s.peak`.
pub fn peaks(s: &mut TurnSite, windows: &[(String, [String; 2])]) {
    let mut tot: HashMap<u32, f64> = HashMap::new();
    for (t, bin) in &s.bins {
        let mut v = 0.0f64;
        for (k, e) in bin.iter() {
            if !is_leg_total(k) {
                for c in VEH {
                    v += e.get(c).copied().unwrap_or(0.0);
                }
            }
        }
        tot.insert(*t, v);
    }
    let leg_only = !s.bins.values().any(|b| b.keys().any(|k| !is_leg_total(k)));
    if leg_only {
        for (t, bin) in &s.bins {
            let mut v = 0.0f64;
            for (k, e) in bin.iter() {
                if k.ends_with("-IN") {
                    for c in VEH {
                        v += e.get(c).copied().unwrap_or(0.0);
                    }
                }
            }
            tot.insert(*t, v);
        }
    }
    let mut peak: OrderedMap<Peak> = OrderedMap::new();
    let iv = s.interval.num();
    for (p, w) in windows {
        let (Some(from), Some(to)) = (to_min(&w[0]), to_min(&w[1])) else { continue };
        let mut best: Option<Best> = None;
        let step: u32 = if iv >= 60.0 {
            60
        } else if iv >= 30.0 {
            30
        } else {
            15
        };
        let fine = step == 15;
        let mut keys: Vec<u32> = tot.keys().copied().collect();
        keys.sort_unstable();
        for t in keys {
            let tf = f64::from(t);
            if tf < from || tf + 60.0 > to {
                continue;
            }
            let mut ts: Vec<u32> = Vec::new();
            // Bound: bins are minutes of a day, far below u32::MAX; at the end the loop simply stops
            let end = t.saturating_add(60);
            let mut bin_t = t;
            while bin_t < end {
                ts.push(bin_t);
                bin_t = bin_t.saturating_add(step);
            }
            if !ts.iter().all(|x| tot.contains_key(x)) {
                continue;
            }
            // Every bin in ts is in tot (checked above), so the NaN fallback is never used
            let tot_at = |x: &u32| tot.get(x).copied().unwrap_or(f64::NAN);
            let v = ts.iter().fold(0.0f64, |q, x| q + tot_at(x));
            if best.as_ref().is_none_or(|bst| v > bst.v) {
                let v15 = if fine { Some(ts.iter().fold(f64::NEG_INFINITY, |m, x| math_max(m, tot_at(x)))) } else { None };
                best = Some(Best { start: t, v, ts, v15 });
            }
        }
        let Some(best) = best else { continue };
        let mut mv: OrderedMap<f64> = OrderedMap::new();
        let mut hv: OrderedMap<f64> = OrderedMap::new();
        for t in &best.ts {
            if let Some(bin) = s.bins.get(t) {
                for (k, e) in bin.iter() {
                    let sum = VEH.iter().fold(0.0f64, |q, c| q + e.get(c).copied().unwrap_or(0.0));
                    let cur = mv.get(k).copied().unwrap_or(0.0);
                    mv.insert(k.to_string(), cur + sum);
                    let hvs = e.get("HV").copied().unwrap_or(0.0) + e.get("BUS").copied().unwrap_or(0.0);
                    let cur = hv.get(k).copied().unwrap_or(0.0);
                    hv.insert(k.to_string(), cur + hvs);
                }
            }
        }
        peak.insert(
            p.clone(),
            Peak {
                start: hhmm(f64::from(best.start)),
                end: hhmm(f64::from(best.start) + 60.0),
                total: best.v,
                phf: match best.v15 {
                    Some(v15) if v15 != 0.0 && !v15.is_nan() => Some(r3(best.v / (4.0 * v15))),
                    _ => None,
                },
                mv,
                hv,
                leg_only,
            },
        );
    }
    s.peak = Maybe::Set(peak);
}

/// `repeak(P)`: finds the peak hours again for sites whose peak-hour windows have changed (balanced counts of a moved peak are dropped); returns how many sites changed.
pub fn repeak(p: &mut Project) -> usize {
    let cfg = super::common::calib_cfg(Some(&p.settings));
    let w = windows_json(&cfg);
    let mut st = CalibStore::read(p);
    let mut n = 0usize;
    for s in &mut st.turn_sites {
        if s.peak_win.as_str() == Some(w.as_str()) || s.bins.is_empty() {
            continue;
        }
        let old = serde_json::to_string(&s.peak).unwrap_or_default();
        peaks(s, &cfg.windows);
        s.peak_win = Maybe::Set(w.clone());
        if serde_json::to_string(&s.peak).unwrap_or_default() != old {
            s.bal = Maybe::Missing;
            s.audit = Maybe::Missing;
            // Bound: at most the number of sites
            n = n.saturating_add(1);
        }
    }
    st.write(p);
    n
}

/// `importTurnCSV(text, name)`: reads a turning-count file into the project; a site and date that are already there are replaced.
///
/// # Errors
///
/// The messages of [`parse_turn_csv`] when the file cannot be read; the project is then unchanged.
pub fn import_turn_csv(p: &mut Project, text: &str, name: &str, ids: &mut dyn IdSource) -> Result<ParsedTurnCsv, String> {
    let cfg = super::common::calib_cfg(Some(&p.settings));
    let r = parse_turn_csv(text, name, &cfg, ids)?;
    let mut st = CalibStore::read(p);
    for s in &r.sites {
        match st.turn_sites.iter_mut().find(|x| x.reference == s.reference && x.date == s.date) {
            Some(slot) => *slot = s.clone(),
            None => st.turn_sites.push(s.clone()),
        }
    }
    st.write(p);
    Ok(r)
}

/// `ciOf(c)`: a count's 95% confidence interval: its own `ci95`, else the NZTA TMS term-time mean's (veh/day); (text, csv value).
pub fn ci_of(c: &Count) -> Option<(String, Value)> {
    if !c.ci95.is_nullish() && !matches!(&c.ci95, Maybe::Set(JsNum::Text(t)) if t.is_empty()) {
        let v = match &c.ci95 {
            Maybe::Set(j) => j.to_value(),
            _ => Value::Null,
        };
        return Some((c.ci95.js_string(), v));
    }
    let t = match c.tms.get() {
        Some(Value::Object(m)) => m.get("target"),
        _ => None,
    };
    if let Some(Value::Object(t)) = t
        && let Some(ci) = t.get("ci95")
        && !ci.is_null()
    {
        let x = value_to_number(ci);
        if x.is_finite() {
            let mean = t.get("mean").map_or(f64::NAN, value_to_number);
            let n = t.get("n").map_or_else(|| "undefined".to_string(), crate::jsnum::value_to_js_string);
            return Some((
                format!("\u{b1} {} veh/day on the TMS mean of {} veh/day ({} weekdays)", fmt::int(x), fmt::int(mean), n),
                Value::String(format!("\u{b1}{} veh/day (TMS)", crate::jsnum::number_to_js_string(crate::jsnum::math_round(x)))),
            ));
        }
    }
    None
}

/* ---------------- matching sites to junctions ---------------- */

/// `siteNode(sc, s)`: the junction a turning count sits on: the nearest node with three or more roads within 40 m.
#[must_use]
pub fn site_node(sc: &Scenario, s: &TurnSite) -> Option<String> {
    let deg = degree(sc);
    nearest_node(
        sc,
        [s.lat.num(), s.lng.num()],
        40.0,
        Some(&|n: &crate::project::Node| deg.get(&n.id).copied().unwrap_or(0) >= 3),
    )
}

/// `multiNode(sc, id)`: a node that is one of several making up a junction: on a roundabout ring, or joined to another junction node by a short link.
#[must_use]
pub fn multi_node(sc: &Scenario, id: &str) -> bool {
    let deg = degree(sc);
    let near = 35.0;
    links_at(sc, id).iter().any(|l| {
        !l.status.is("closed")
            && (l.ring.truthy()
                || (deg.get(if l.a == id { &l.b } else { &l.a }).copied().unwrap_or(0) >= 3 && link_len(sc, l) <= near))
    })
}

/// `matchSites(sc)`: matches every turning-count site to the junction it sits on (and that junction's site), as saved in the store.
pub fn match_sites(p: &mut Project, sc_id: &str) {
    let mut st = CalibStore::read(p);
    let Some(sc) = p.scenario(sc_id) else { return };
    for s in &mut st.turn_sites {
        let node = site_node(sc, s);
        let site = node.as_deref().and_then(|n| sites::of(sc, n)).map(|x| x.id.clone());
        s.node = node.map_or(Maybe::Null, Maybe::Set);
        s.site = site.map_or(Maybe::Null, Maybe::Set);
    }
    st.write(p);
}
