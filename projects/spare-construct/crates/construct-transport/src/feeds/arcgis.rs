//! Requests for the host to send and answers to read, with no network here.
//!
//! The prototype fetches in its own code; this port builds each request (address, query, body) for the host to send and
//! takes the answer as input. [`HttpRequest`] is one request, [`FetchFailure`] says what went wrong when the host could not
//! get an answer, and [`ArcgisPager`] runs the paged ArcGIS feature service query of `DataHub.arcgis` (up to 25 pages of
//! the server's own page size, each asked for at the offset after the last). Messages for the user are the prototype's words.

use crate::edit::json::JsonValue;
use crate::jsnum::{math_round, to_fixed};

/// The time limit of one ArcGIS request in the data hub (`ARCGIS_TIMEOUT`), in milliseconds.
pub const ARCGIS_TIMEOUT_MS: f64 = 30_000.0;

/// What kind of answer the host should read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// JSON.
    Json,
    /// Text.
    Text,
    /// Raw bytes (a zip file).
    Buffer,
}

/// One request for the host to send.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    /// "GET", "POST" or "HEAD".
    pub method: String,
    /// The whole address, query included.
    pub url: String,
    /// The body of a POST.
    pub body: Option<String>,
    /// Headers to send, in order.
    pub headers: Vec<(String, String)>,
    /// The time limit (ms) after which the host gives up and reports [`FetchFailure::TimedOut`].
    pub timeout_ms: f64,
    /// How to read the answer.
    pub expect: Expect,
}

impl HttpRequest {
    /// A GET of an address, answer read as JSON.
    pub fn get_json(url: String, timeout_ms: f64) -> HttpRequest {
        HttpRequest { method: "GET".to_string(), url, body: None, headers: Vec::new(), timeout_ms, expect: Expect::Json }
    }
}

/// Why the host got no usable answer.
#[derive(Clone, Debug, PartialEq)]
pub enum FetchFailure {
    /// The user cancelled the load.
    Cancelled,
    /// The server did not answer within the time limit.
    TimedOut,
    /// The server could not be reached (the browser's message).
    Unreachable(String),
    /// The server answered with this HTTP status.
    Http(u16),
    /// The server kept the connection but stopped sending within the time limit.
    TimedOutReading,
    /// The answer could not be read (the browser's message).
    Unreadable(String),
}

/// Why a load stopped.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// The user cancelled: the whole load stops (the prototype throws an AbortError).
    Cancelled,
    /// The source failed; the message says why.
    Failed(String),
}

impl LoadError {
    /// The message (for a cancel, "Cancelled").
    pub fn message(&self) -> &str {
        match self {
            LoadError::Cancelled => "Cancelled",
            LoadError::Failed(m) => m,
        }
    }
}

/// `new URLSearchParams(...)` encoding of one piece: letters, digits and `*-._` as they are, a space as `+`, anything else as %XX bytes of its UTF-8.
pub fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'*' | b'-' | b'.' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `new URLSearchParams(pairs).toString()`: `key=value` pairs joined by `&`, each part encoded.
pub fn query_string(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", form_encode(k), form_encode(v))).collect::<Vec<_>>().join("&")
}

/// `encodeURIComponent(s)`: everything but letters, digits and `-_.!~*'()` as %XX bytes of its UTF-8.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `new URL(url).host`: the host and any port that is not the default for the scheme, in lower case; None when the address is not absolute.
pub fn host_of_url(url: &str) -> Option<String> {
    let t = url.trim();
    let (scheme, rest) = t.split_once("://")?;
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..end];
    let auth = auth.rsplit('@').next().unwrap_or(auth);
    if auth.is_empty() {
        return None;
    }
    let lower = auth.to_lowercase();
    let default = match scheme.to_lowercase().as_str() {
        "https" | "wss" => ":443",
        "http" | "ws" => ":80",
        _ => "",
    };
    Some(if !default.is_empty() && lower.ends_with(default) { lower[..lower.len() - default.len()].to_string() } else { lower })
}

pub use crate::importers::Bounds;

/// The box around a list of [lat, lng] points (`L.latLngBounds(pts)`); None for no points.
pub fn bounds_of_points(pts: &[[f64; 2]]) -> Option<Bounds> {
    let first = pts.first()?;
    let mut b = Bounds { south: first[0], west: first[1], north: first[0], east: first[1] };
    for p in pts {
        b.south = crate::jsnum::math_min(b.south, p[0]);
        b.north = crate::jsnum::math_max(b.north, p[0]);
        b.west = crate::jsnum::math_min(b.west, p[1]);
        b.east = crate::jsnum::math_max(b.east, p[1]);
    }
    Some(b)
}

/// `bounds.pad(ratio)`: Leaflet's padding: each side moves out by `ratio` of the box's height or width.
pub fn bounds_pad(b: &Bounds, ratio: f64) -> Bounds {
    let h = (b.south - b.north).abs() * ratio;
    let w = (b.west - b.east).abs() * ratio;
    Bounds { south: b.south - h, west: b.west - w, north: b.north + h, east: b.east + w }
}

/// `[west, south, east, north]` to six decimals, joined by commas: the envelope the services take.
pub fn bounds_envelope(b: &Bounds) -> String {
    [b.west, b.south, b.east, b.north].iter().map(|v| to_fixed(*v, 6)).collect::<Vec<_>>().join(",")
}

/// Runs the paged feature query of `DataHub.arcgis`: ask for a page ([`next_request`](ArcgisPager::next_request)), hand in the
/// answer ([`answer`](ArcgisPager::answer)), repeat until [`is_done`](ArcgisPager::is_done).
///
/// A page is the server's own size; the next is asked for at the offset after the last, while the server says
/// `exceededTransferLimit` and sent features, for at most 25 pages.
#[derive(Clone, Debug)]
pub struct ArcgisPager {
    url: String,
    host: String,
    envelope: String,
    where_: String,
    fields: String,
    timeout_ms: f64,
    page: usize,
    offset: usize,
    features: Vec<JsonValue>,
    done: bool,
}

impl ArcgisPager {
    /// A query of a layer (`url` ends in /FeatureServer/0) for the features meeting a where clause inside the bounds, with the named fields ("*" for all), ordered by OBJECTID.
    pub fn new(url: &str, b: &Bounds, where_: &str, fields: &str, timeout_ms: f64) -> ArcgisPager {
        ArcgisPager {
            url: url.to_string(),
            host: host_of_url(url).unwrap_or_default(),
            envelope: bounds_envelope(b),
            where_: where_.to_string(),
            fields: fields.to_string(),
            timeout_ms,
            page: 0,
            offset: 0,
            features: Vec::new(),
            done: false,
        }
    }
    /// The request for the next page; None when the query is finished.
    pub fn next_request(&self) -> Option<HttpRequest> {
        if self.done {
            return None;
        }
        let q: Vec<(String, String)> = [
            ("where", self.where_.as_str()),
            ("geometry", self.envelope.as_str()),
            ("geometryType", "esriGeometryEnvelope"),
            ("inSR", "4326"),
            ("spatialRel", "esriSpatialRelIntersects"),
            ("outFields", self.fields.as_str()),
            ("outSR", "4326"),
            ("f", "json"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .chain([("resultOffset".to_string(), self.offset.to_string()), ("orderByFields".to_string(), "OBJECTID".to_string())])
        .collect();
        Some(HttpRequest::get_json(format!("{}/query?{}", self.url, query_string(&q)), self.timeout_ms))
    }
    /// Takes the answer to the last request: adds its features and works out whether another page is needed. A failure ends the query with the prototype's message (or [`LoadError::Cancelled`]).
    pub fn answer(&mut self, a: Result<JsonValue, FetchFailure>) -> Result<(), LoadError> {
        let host = &self.host;
        let j = match a {
            Err(FetchFailure::Cancelled) => return Err(LoadError::Cancelled),
            Err(FetchFailure::TimedOut) | Err(FetchFailure::TimedOutReading) => {
                return Err(LoadError::Failed(format!("{host} did not answer within {} s. Try again later", crate::jsnum::number_to_js_string(math_round(self.timeout_ms / 1000.0)))));
            }
            Err(FetchFailure::Unreachable(m)) => return Err(LoadError::Failed(format!("{host} could not be reached ({m})"))),
            Err(FetchFailure::Http(s)) => return Err(LoadError::Failed(format!("{host} answered HTTP {s}"))),
            Err(FetchFailure::Unreadable(_)) => return Err(LoadError::Failed(format!("{host} sent an answer Spare Construct could not read"))),
            Ok(j) => j,
        };
        if !j.is_object() {
            return Err(LoadError::Failed(format!("{host} sent an answer Spare Construct could not read")));
        }
        if let Some(e) = j.get("error").filter(|e| e.truthy()) {
            let m = e.get("message").filter(|m| m.truthy()).map_or_else(|| "query failed".to_string(), JsonValue::js_string);
            return Err(LoadError::Failed(m));
        }
        let f: Vec<JsonValue> = match j.get("features") {
            Some(JsonValue::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        let n = f.len();
        self.features.extend(f);
        self.page += 1;
        if !j.get("exceededTransferLimit").is_some_and(JsonValue::truthy) || n == 0 || self.page >= 25 {
            self.done = true;
        } else {
            self.offset += n;
        }
        Ok(())
    }
    /// True when no more pages are needed.
    pub fn is_done(&self) -> bool {
        self.done
    }
    /// The features collected so far, in order.
    pub fn features(&self) -> &[JsonValue] {
        &self.features
    }
    /// The features, when finished.
    pub fn into_features(self) -> Vec<JsonValue> {
        self.features
    }
}
