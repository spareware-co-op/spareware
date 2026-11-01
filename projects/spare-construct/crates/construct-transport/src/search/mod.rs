//! Place search answers turned into map positions and study areas (port of search.js).
//!
//! The prototype looks up New Zealand places and addresses with OpenStreetMap Nominatim (free, no key) and reads typed
//! coordinates such as "-37.10, 174.95" without a request. There is no network here: [`plan`] says what a typed query is
//! (nothing, coordinates, or a request for the host to send), and [`read_answer`] turns Nominatim's answer into places.
//! The host keeps the rules the prototype's code keeps around the request: ask only on Enter or a button (never while
//! typing, as Nominatim's usage policy forbids), at most one request a second ([`GAP_MS`]), answers cached by
//! [`cache_key`] (at most [`CACHE_MAX`] of them), and a queued request dropped when a newer search replaced it.

use crate::edit::json::JsonValue;
use crate::feeds::jsregex::Regex;
use crate::feeds::{Expect, FetchFailure, HttpRequest, query_string};
use crate::importers::jsparse::parse_float;
use crate::jsmath;
use crate::jsnum::{js_trim, math_max, to_fixed};
use crate::util::r7;
use serde::Serialize;

/// The place search service (`NOMINATIM`).
pub const NOMINATIM: &str = "https://nominatim.openstreetmap.org/search";
/// The least time between two requests (ms), `Search.GAP`.
pub const GAP_MS: f64 = 1100.0;
/// How many answers the host keeps (`CACHE_MAX`).
pub const CACHE_MAX: usize = 200;
/// Half the side of the least box a result gets (m): results are at least about 500 m across (`MIN_BOX`).
pub const MIN_BOX: f64 = 250.0;

/// A box as [[south, west], [north, east]].
pub type PlaceBounds = [[f64; 2]; 2];

/// One place: a name, a position, the area to show and what kind of place it is.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Place {
    /// The name shortened for display.
    pub name: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// The area: [[south, west], [north, east]].
    pub bounds: PlaceBounds,
    /// The kind ("road", "suburb", "coordinates", ...).
    #[serde(rename = "type")]
    pub kind: String,
}

// Math.abs
fn abs(x: f64) -> f64 {
    x.abs()
}

/// `boxAround(lat, lng, m)`: a box about 2 m metres across around a point (m is the half side; 250 by default).
pub fn box_around(lat: f64, lng: f64, m: f64) -> PlaceBounds {
    let d_lat = m / 110574.0;
    let d_lng = m / (111320.0 * math_max(0.05, jsmath::cos(lat * std::f64::consts::PI / 180.0)));
    [
        [r7(lat - d_lat), r7(lng - d_lng)],
        [r7(lat + d_lat), r7(lng + d_lng)],
    ]
}

/// `parseCoords(q)`: "-37.10, 174.95", "-37.10 174.95", "174.95, -37.10" and "37.1 S, 174.95 E" as a place, else None.
///
/// Without hemisphere letters the pair is read as latitude, longitude, swapped when only that makes sense (the first value
/// beyond 90 either way, or a New Zealand longitude first and a southern latitude second).
pub fn parse_coords(q: &str) -> Option<Place> {
    let re = Regex::lit(
        "^\\s*([-+]?\\d{1,3}(?:\\.\\d+)?)\\s*\u{b0}?\\s*([NSEW])?\\s*[,;\\s]\\s*([-+]?\\d{1,3}(?:\\.\\d+)?)\\s*\u{b0}?\\s*([NSEW])?\\s*$",
        "i",
    );
    let u: Vec<u16> = q.encode_utf16().collect();
    let m = re.exec_units(&u, 0)?;
    let g = |i: usize| m.text(&u, i);
    let hemi = Regex::lit("[SW]", "i");
    let val = |n: Option<String>, h: &Option<String>| {
        let v = parse_float(&n.unwrap_or_default());
        if h.as_deref().is_some_and(|h| !h.is_empty() && hemi.test(h)) {
            -abs(v)
        } else {
            v
        }
    };
    let (h1, h2) = (g(2), g(4));
    let (mut a, mut b) = (val(g(1), &h1), val(g(3), &h2));
    let ha = h1.clone().unwrap_or_default().to_uppercase();
    let hb = h2.clone().unwrap_or_default().to_uppercase();
    let swap = if !ha.is_empty() || !hb.is_empty() {
        Regex::lit("[EW]", "").test(&ha) || Regex::lit("[NS]", "").test(&hb)
    } else {
        (abs(a) > 90.0 && abs(b) <= 90.0) || (a >= 160.0 && b < 0.0 && b >= -90.0)
    };
    if swap {
        std::mem::swap(&mut a, &mut b);
    }
    if !a.is_finite() || !b.is_finite() || abs(a) > 90.0 || abs(b) > 180.0 {
        return None;
    }
    Some(Place {
        name: format!("{}, {}", to_fixed(a, 5), to_fixed(b, 5)),
        lat: r7(a),
        lng: r7(b),
        bounds: box_around(a, b, MIN_BOX),
        kind: "coordinates".to_string(),
    })
}

// Road-type abbreviations as NZ addresses write them (the same list DataHub uses to match counts)
const ABBR: [(&str, &str); 11] = [
    ("rd", "Road"),
    ("ave", "Avenue"),
    ("av", "Avenue"),
    ("pl", "Place"),
    ("cres", "Crescent"),
    ("hwy", "Highway"),
    ("tce", "Terrace"),
    ("pde", "Parade"),
    ("ln", "Lane"),
    ("mwy", "Motorway"),
    ("blvd", "Boulevard"),
];
// St, Dr and Ct are also Saint, Doctor and Court names, so they expand only at the end of a part
const ABBR_END: [(&str, &str); 6] = [
    ("st", "Street"),
    ("dr", "Drive"),
    ("ct", "Court"),
    ("cl", "Close"),
    ("cr", "Crescent"),
    ("sq", "Square"),
];

/// `expand(q)`: common New Zealand abbreviations written out ('Waihoehoe Rd' gives 'Waihoehoe Road').
///
/// They are always after another word, so 'St Heliers' and 'Rd 2' style names keep their first word; St, Dr, Ct, Cl, Cr and Sq
/// expand only at the end of a part (before a comma or the end).
pub fn expand(q: &str) -> String {
    let mut s = q.to_string();
    for (a, full) in ABBR {
        let re = Regex::lit(&format!("(\\S\\s+){a}\\.?(?=\\s|,|$)"), "gi");
        s = re.replace(&s, |m, u| {
            format!("{}{}", m.text(u, 1).unwrap_or_default(), full)
        });
    }
    for (a, full) in ABBR_END {
        let re = Regex::lit(&format!("(\\S\\s+){a}\\.?(?=\\s*(?:,|$))"), "gi");
        s = re.replace(&s, |m, u| {
            format!("{}{}", m.text(u, 1).unwrap_or_default(), full)
        });
    }
    s
}

/// `shorten(display)`: "12, St Marks Road, Remuera, Auckland, 1050, New Zealand / Aotearoa" as "12 St Marks Road, Remuera, Auckland".
pub fn shorten(display: &str) -> String {
    let nz = Regex::lit("^new zealand(\\s*\\/\\s*aotearoa)?$", "i");
    let ao = Regex::lit("^aotearoa(\\s*\\/\\s*new zealand)?$", "i");
    let post = Regex::lit("^\\d{4}$", "");
    let mut parts: Vec<String> = display
        .split(',')
        .map(|s| js_trim(s).to_string())
        .enumerate()
        .filter(|(i, s)| !s.is_empty() && !nz.test(s) && !ao.test(s) && !(*i > 0 && post.test(s)))
        .map(|(_, s)| s)
        .collect();
    if parts.len() > 1 && Regex::lit("^\\d+[a-z]?(?:[-/]\\d+[a-z]?)?$", "i").test(&parts[0]) {
        let joined = format!("{} {}", parts[0], parts[1]);
        parts.splice(0..2, [joined]);
    }
    let out = parts.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if out.is_empty() {
        display.to_string()
    } else {
        out
    }
}

fn text_of(v: Option<&JsonValue>) -> String {
    v.map_or_else(|| "undefined".to_string(), JsonValue::js_string)
}

fn first_truthy(vs: &[Option<&JsonValue>]) -> Option<String> {
    vs.iter()
        .flatten()
        .find(|v| v.truthy())
        .map(|v| v.js_string())
}

/// `toResult(r)`: one Nominatim result as a place; None when it has no position.
///
/// The area is the result's bounding box, or a box about 500 m across around the point when the result is smaller than that
/// (an address is a few metres across: it gets a study-sized box instead). A result that is null, or whose bounding box
/// is something other than a list, stops the prototype with an error, and so it does here.
pub fn to_result(r: &JsonValue) -> Result<Option<Place>, String> {
    if r.is_null() {
        return Err("Cannot read properties of null (reading 'lat')".to_string());
    }
    let lat = parse_float(&text_of(r.get("lat")));
    let lng = parse_float(&text_of(r.get("lon")));
    if !lat.is_finite() || !lng.is_finite() {
        return Ok(None);
    }
    let bb: Vec<f64> = match r.get("boundingbox") {
        Some(JsonValue::Array(a)) => a.iter().map(|v| parse_float(&v.js_string())).collect(),
        Some(v) if v.truthy() => {
            return Err("(r.boundingbox || []).map is not a function".to_string());
        }
        _ => Vec::new(),
    };
    let mut bounds = if bb.len() == 4 && bb.iter().all(|v| v.is_finite()) {
        [[bb[0], bb[2]], [bb[1], bb[3]]]
    } else {
        box_around(lat, lng, MIN_BOX)
    };
    let small = (bounds[1][0] - bounds[0][0]) * 110574.0 < 2.0 * MIN_BOX
        && (bounds[1][1] - bounds[0][1])
            * 111320.0
            * jsmath::cos(lat * std::f64::consts::PI / 180.0)
            < 2.0 * MIN_BOX;
    if small {
        bounds = box_around(lat, lng, MIN_BOX);
    }
    let display = first_truthy(&[r.get("display_name"), r.get("name")]);
    Ok(Some(Place {
        name: shorten(&display.unwrap_or_default()),
        lat: r7(lat),
        lng: r7(lng),
        bounds,
        kind: first_truthy(&[r.get("addresstype"), r.get("type"), r.get("category")])
            .unwrap_or_else(|| "place".to_string()),
    }))
}

/// What to do with a typed query.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// Nothing typed: no places.
    Empty,
    /// Coordinates: the one place, with no request.
    Coords(Place),
    /// A place name: send the request; keep the answer under the key.
    Ask {
        /// The request to send (when the host's rate limit allows).
        request: HttpRequest,
        /// The key to cache the answer under (the expanded text in lower case with spaces collapsed).
        key: String,
    },
}

/// The key the prototype caches answers under: the expanded query in lower case with runs of white space made one space.
pub fn cache_key(expanded: &str) -> String {
    Regex::lit("\\s+", "g").replace(&expanded.to_lowercase(), |_, _| " ".to_string())
}

/// `geocode(q)` before the request: nothing for an empty query, a place for coordinates, else the request for the expanded text.
pub fn plan(q: &str) -> Plan {
    let s = js_trim(q);
    if s.is_empty() {
        return Plan::Empty;
    }
    if let Some(c) = parse_coords(s) {
        return Plan::Coords(c);
    }
    let qx = expand(s);
    let key = cache_key(&qx);
    let pairs: Vec<(String, String)> = [
        ("format", "jsonv2"),
        ("countrycodes", "nz"),
        ("limit", "6"),
        ("addressdetails", "0"),
        ("accept-language", "en"),
        ("q", qx.as_str()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let request = HttpRequest {
        method: "GET".to_string(),
        url: format!("{}?{}", NOMINATIM, query_string(&pairs)),
        body: None,
        headers: Vec::new(),
        timeout_ms: 30_000.0,
        expect: Expect::Json,
    };
    Plan::Ask { request, key }
}

/// Turns Nominatim's answer into places (those with a position, in order); or the message the prototype shows when the search failed (HTTP 429 means busy).
pub fn read_answer(a: Result<&JsonValue, FetchFailure>) -> Result<Vec<Place>, String> {
    match a {
        Err(FetchFailure::Unreachable(m)) => Err(format!("The place search could not be reached ({m}). Check your internet connection and try again.")),
        Err(FetchFailure::Http(429)) => Err("The place search is busy (HTTP 429). Wait a few seconds and try again.".to_string()),
        Err(FetchFailure::Http(s)) => Err(format!("The place search answered HTTP {s}. Try again shortly.")),
        Err(FetchFailure::Cancelled) => Err("The search was replaced by a newer one.".to_string()),
        Err(FetchFailure::TimedOut) | Err(FetchFailure::TimedOutReading) => Err("The place search could not be reached (timed out). Check your internet connection and try again.".to_string()),
        Err(FetchFailure::Unreadable(m)) => Err(m),
        Ok(j) => match j {
            JsonValue::Array(list) => {
                let mut out = Vec::new();
                for r in list {
                    if let Some(p) = to_result(r)? {
                        out.push(p);
                    }
                }
                Ok(out)
            }
            _ => Ok(Vec::new()),
        },
    }
}
