//! Live traffic from TomTom and traffic-aware travel times from Google (the end of data.js).
//!
//! Both need the user's own key (never saved in a project file). Each has a function that builds the request and one that
//! reads the answer; the host sends the request.

use super::arcgis::{encode_uri_component, Expect, FetchFailure, HttpRequest};
use super::nztime::{date_utc, nz_parts, DAY_MS};
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::importers::jsparse::parse_float;
use crate::jsnum::to_fixed;
use crate::util::LatLng;
use serde::Serialize;
use serde_json::Value;

/// `tomtomTiles()`: the address pattern of TomTom's traffic flow map tiles ({z}, {x} and {y} are filled in by the map), or None without a key.
pub fn tomtom_tiles(key: Option<&str>) -> Option<String> {
    let key = key.filter(|k| !k.is_empty())?;
    Some(format!("https://api.tomtom.com/traffic/map/4/tile/flow/relative0/{{z}}/{{x}}/{{y}}.png?key={}&tileSize=256", encode_uri_component(key)))
}

/// `tomtomSpeed(ll)`: the request for the current speed and free-flow speed on the road nearest a point; Err without a key.
pub fn tomtom_speed_request(ll: LatLng, key: Option<&str>) -> Result<HttpRequest, String> {
    let Some(key) = key.filter(|k| !k.is_empty()) else { return Err("Add a TomTom key in the Data tab first.".to_string()) };
    Ok(HttpRequest {
        method: "GET".to_string(),
        url: format!(
            "https://api.tomtom.com/traffic/services/4/flowSegmentData/absolute/10/json?point={},{}&unit=KMPH&key={}",
            to_fixed(ll[0], 6),
            to_fixed(ll[1], 6),
            encode_uri_component(key)
        ),
        body: None,
        headers: Vec::new(),
        timeout_ms: 30_000.0,
        expect: Expect::Json,
    })
}

/// What `tomtomSpeed` gives: the speed now, the free-flow speed, how sure TomTom is, and whether the road is closed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TomtomSpeed {
    /// Current speed (km/h).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now: Option<Value>,
    /// Free-flow speed (km/h).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free: Option<Value>,
    /// Confidence (0 to 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf: Option<Value>,
    /// The road is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<Value>,
    /// When it was asked (ms since 1970).
    pub at: f64,
}

/// The message for a request that did not get a usable answer (the browser's own message passes through).
fn failure_text(f: &FetchFailure) -> String {
    match f {
        FetchFailure::Cancelled => "Cancelled".to_string(),
        FetchFailure::TimedOut | FetchFailure::TimedOutReading => "The request timed out".to_string(),
        FetchFailure::Unreachable(m) | FetchFailure::Unreadable(m) => m.clone(),
        FetchFailure::Http(s) => format!("HTTP {s}"),
    }
}

/// The answer of [`tomtom_speed_request`]: the speed, or why not (a refused key, an error status).
pub fn tomtom_speed_result(a: Result<&JsonValue, FetchFailure>, now_ms: f64) -> Result<TomtomSpeed, String> {
    match a {
        Err(FetchFailure::Http(s)) => Err(if s == 403 || s == 401 { "TomTom refused the key".to_string() } else { format!("TomTom answered HTTP {s}") }),
        Err(f) => Err(failure_text(&f)),
        Ok(j) => {
            let Some(f) = j.get("flowSegmentData").filter(|f| !f.is_null()) else {
                return Err("Cannot read properties of undefined (reading 'currentSpeed')".to_string());
            };
            let g = |k: &str| f.get(k).map(JsonValue::to_value);
            Ok(TomtomSpeed { now: g("currentSpeed"), free: g("freeFlowSpeed"), conf: g("confidence"), closed: g("roadClosure"), at: now_ms })
        }
    }
}

/// `nextWeekdayNZ(hour)`: the next Tuesday, Wednesday or Thursday (from tomorrow) at hh:00 New Zealand time, as an ISO instant and a label ("Thu, 22 Oct, 8:00 am"); None when none of the next nine days is one.
pub fn next_weekday_nz(now_ms: f64, hour: f64) -> Option<(String, String)> {
    for add in 1..10 {
        let t = now_ms + add as f64 * DAY_MS;
        let p = nz_parts(t);
        if !matches!(p.weekday, 2..=4) {
            continue;
        }
        let (y, mo, dd) = (p.year as f64, p.month as f64 - 1.0, p.day as f64);
        let mut guess = date_utc(y, mo, dd) + hour * 3_600_000.0 - 12.0 * 3_600_000.0;
        for _ in 0..3 {
            let q = nz_parts(guess);
            let diff_h = hour - q.hour as f64 + if q.day as f64 != dd { if q.day as f64 > dd { -24.0 } else { 24.0 } } else { 0.0 };
            if diff_h == 0.0 {
                break;
            }
            guess += diff_h * 3_600_000.0;
        }
        let q = nz_parts(guess);
        let wk = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][q.weekday as usize];
        let h12 = if q.hour % 12 == 0 { 12 } else { q.hour % 12 };
        let label = format!("{}, {} {}, {}:{:02} {}", wk, q.day, super::nztime::month_short(q.month), h12, q.minute, if q.hour >= 12 { "pm" } else { "am" });
        return Some((iso_string(guess), label));
    }
    None
}

/// `googleTime(from, to, period)`: the Routes API request for the traffic-aware drive time between two points for the next Tuesday to Thursday at 8:00 (AM) or 17:00 (any other period); Err without a key or a day. Gives the request and the departure label.
pub fn google_request(from: LatLng, to: LatLng, period: &str, key: Option<&str>, now_ms: f64) -> Result<(HttpRequest, String), String> {
    let Some(key) = key.filter(|k| !k.is_empty()) else { return Err("Add a Google Maps Platform key in the Data tab first.".to_string()) };
    let Some((iso, label)) = next_weekday_nz(now_ms, if period == "AM" { 8.0 } else { 17.0 }) else {
        return Err("Cannot read properties of null (reading 'iso')".to_string());
    };
    let pt = |ll: LatLng| {
        JsonValue::from_pairs([("location", JsonValue::from_pairs([("latLng", JsonValue::from_pairs([("latitude", JsonValue::Number(ll[0])), ("longitude", JsonValue::Number(ll[1]))]))]))])
    };
    let body = JsonValue::from_pairs([
        ("origin", pt(from)),
        ("destination", pt(to)),
        ("travelMode", JsonValue::String("DRIVE".into())),
        ("routingPreference", JsonValue::String("TRAFFIC_AWARE_OPTIMAL".into())),
        ("departureTime", JsonValue::String(iso)),
        ("units", JsonValue::String("METRIC".into())),
    ]);
    Ok((
        HttpRequest {
            method: "POST".to_string(),
            url: "https://routes.googleapis.com/directions/v2:computeRoutes".to_string(),
            body: Some(body.stringify()),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Goog-Api-Key".to_string(), key.to_string()),
                ("X-Goog-FieldMask".to_string(), "routes.duration,routes.staticDuration,routes.distanceMeters".to_string()),
            ],
            timeout_ms: 30_000.0,
            expect: Expect::Json,
        },
        label,
    ))
}

/// What `googleTime` gives: the traffic-aware and the free-flow drive time, the distance and when the trip was for.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GoogleTime {
    /// Drive time with typical traffic (s).
    pub t: f64,
    /// Drive time with no traffic (s).
    #[serde(rename = "tFree")]
    pub t_free: f64,
    /// Distance (m).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub len: Option<Value>,
    /// The departure, as the label ("Thu, 22 Oct, 8:00 am").
    pub when: String,
    /// When it was asked (ms since 1970).
    pub at: f64,
}

/// The answer of [`google_request`]: `status` and the body (an unreadable body is read as an empty object, as the prototype does).
pub fn google_result(status: u16, ok: bool, body: &JsonValue, when: &str, now_ms: f64) -> Result<GoogleTime, String> {
    let err = body.get("error").filter(|e| e.truthy());
    if !ok || err.is_some() {
        let m = err.and_then(|e| e.get("message")).filter(|m| m.truthy()).map(JsonValue::js_string);
        return Err(m.unwrap_or_else(|| format!("Google answered HTTP {status}")));
    }
    let rt = match body.get("routes") {
        Some(JsonValue::Array(a)) => a.first(),
        _ => None,
    };
    let Some(rt) = rt else { return Err("Google found no route".to_string()) };
    let pf = |k: &str| parse_float(&rt.get(k).map_or_else(|| "undefined".to_string(), JsonValue::js_string));
    Ok(GoogleTime { t: pf("duration"), t_free: pf("staticDuration"), len: rt.get("distanceMeters").map(JsonValue::to_value), when: when.to_string(), at: now_ms })
}
