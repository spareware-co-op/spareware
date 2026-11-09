//! The requests of packs/nz/feeds.js, one at a time, for a host to send.
//!
//! feeds.js fetches with its own `get` (a time limit, errors that say what to do), caches layer descriptions for the
//! session (`layerMeta`) and pages ArcGIS queries (`arcQuery`). Here each of those is a small state machine: ask for
//! the next request, give back the answer. Every download of the feeds is a [`Flow`] of these.

use super::arcgis::{
    Bounds, Expect, FetchFailure, HttpRequest, form_encode, host_of_url, query_string,
};
use crate::edit::json::JsonValue;
use crate::jsnum::math_round;
use crate::project::Project;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What the host read from a response.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// The body as JSON.
    Json(JsonValue),
    /// The body as text.
    Text(String),
    /// The body as bytes, with the response's `Last-Modified` header.
    Bytes {
        /// The body.
        data: Vec<u8>,
        /// The `Last-Modified` header, if any.
        last_modified: Option<String>,
    },
    /// The answer to a HEAD request: the `Last-Modified` header, if any.
    Head {
        /// The `Last-Modified` header, if any.
        last_modified: Option<String>,
    },
}

/// A download made of requests: [`next_request`](Flow::next_request) says what to send, [`answer`](Flow::answer) takes the
/// response. A failure ends the flow with the message the prototype would show the user.
pub trait Flow {
    /// The next request, or None when the flow needs nothing more.
    fn next_request(&mut self) -> Option<HttpRequest>;
    /// Takes the answer to the last request; Err ends the flow.
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String>;
    /// True when the flow needs no more requests.
    fn is_done(&self) -> bool;
}

/// Runs a flow to the end with a function that does the fetching (for tests and simple hosts).
pub fn run_flow<F: Flow>(
    flow: &mut F,
    mut fetch: impl FnMut(&HttpRequest) -> Result<Answer, FetchFailure>,
) -> Result<(), String> {
    while let Some(req) = flow.next_request() {
        let a = fetch(&req);
        flow.answer(a)?;
    }
    Ok(())
}

/// `hostOf(url)`: the host of an address; an empty address gives "", one that is not an address "this site".
pub fn host_text(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    // an address with a scheme is an address (an empty host gives the text back); anything else, such as a path on this site, is not
    let t = url.trim();
    let mut cs = t.chars();
    let scheme = cs.next().is_some_and(|c| c.is_ascii_alphabetic())
        && t.split_once(':').is_some_and(|(s, _)| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        });
    if !scheme {
        return "this site".to_string();
    }
    match host_of_url(t) {
        Some(h) if !h.is_empty() => h,
        _ => url.to_string(),
    }
}

/// The text of the error `get(url)` throws when the host reports a failure; `ms` is the request's time limit.
pub fn get_error(url: &str, ms: f64, f: &FetchFailure) -> String {
    let host = host_text(url);
    let secs = crate::jsnum::number_to_js_string(math_round(ms / 1000.0));
    match f {
        FetchFailure::Cancelled => "Cancelled".to_string(),
        FetchFailure::TimedOut => format!(
            "{host} did not answer within {secs} s. Try again later, or raise the time limit in Settings \u{203a} Official data feeds"
        ),
        FetchFailure::Unreachable(m) => {
            format!("{host} could not be reached ({m}). Check your connection and try again")
        }
        FetchFailure::Http(s) => format!(
            "{host} answered HTTP {s}{}",
            if *s == 404 {
                " (not found: the service may have moved)"
            } else {
                ""
            }
        ),
        FetchFailure::TimedOutReading => {
            format!("{host} stopped sending data after {secs} s. Try again later")
        }
        FetchFailure::Unreadable(m) => {
            format!("{host} sent something Spare Construct could not read ({m})")
        }
    }
}

/// The settings of Settings, Official data feeds (`cfg()`): the defaults with the project's own values over them.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedsConfig {
    /// Time limit of each request, seconds.
    pub timeout: Value,
    /// Future scenario years use the speed limits gazetted for 1 July of that year.
    pub nslr_future: Value,
    /// Days of TMS daily counts to download.
    pub tms_days: Value,
    /// Offer to apply speed limits and bus routes to the other scenarios too.
    pub other_scenarios: Value,
}

impl Default for FeedsConfig {
    fn default() -> Self {
        FeedsConfig {
            timeout: Value::from(30),
            nslr_future: Value::Bool(true),
            tms_days: Value::from(400),
            other_scenarios: Value::Bool(false),
        }
    }
}

impl FeedsConfig {
    /// `cfg()` for a project: the defaults, then whatever `settings.feeds` holds.
    pub fn of(p: &Project) -> FeedsConfig {
        let mut c = FeedsConfig::default();
        if let Some(Value::Object(f)) = p.settings.extra.get("feeds") {
            if let Some(v) = f.get("timeout") {
                c.timeout = v.clone();
            }
            if let Some(v) = f.get("nslrFuture") {
                c.nslr_future = v.clone();
            }
            if let Some(v) = f.get("tmsDays") {
                c.tms_days = v.clone();
            }
            if let Some(v) = f.get("otherScenarios") {
                c.other_scenarios = v.clone();
            }
        }
        c
    }
    /// `limitMs()`: the time limit in ms; a cleared or zero box uses the default 30 s.
    pub fn limit_ms(&self) -> f64 {
        let t = crate::jsnum::value_to_number(&self.timeout);
        (if t > 0.0 { t } else { 30.0 }) * 1000.0
    }
}

/// The layer descriptions fetched so far, kept for the session (`mem.meta`): a flow that needs one asks the cache first.
#[derive(Clone, Debug, Default)]
pub struct MetaCache(Rc<RefCell<HashMap<String, JsonValue>>>);

impl MetaCache {
    /// An empty cache.
    pub fn new() -> MetaCache {
        MetaCache::default()
    }
    /// The description kept for a layer address.
    pub fn get(&self, url: &str) -> Option<JsonValue> {
        self.0.borrow().get(url).cloned()
    }
    /// Keeps a description.
    pub fn put(&self, url: &str, meta: JsonValue) {
        self.0.borrow_mut().insert(url.to_string(), meta);
    }
}

/// `layerMeta(url)`: the layer's own description (`?f=json`), from the cache when it is there.
#[derive(Clone, Debug)]
pub struct LayerMeta {
    url: String,
    cache: MetaCache,
    timeout_ms: f64,
    meta: Option<JsonValue>,
    done: bool,
}

impl LayerMeta {
    /// Reads the description of a layer (its address, no query).
    pub fn new(url: &str, cache: &MetaCache, timeout_ms: f64) -> LayerMeta {
        let meta = cache.get(url);
        LayerMeta {
            url: url.to_string(),
            cache: cache.clone(),
            timeout_ms,
            done: meta.is_some(),
            meta,
        }
    }
    /// The description, once read.
    pub fn meta(&self) -> Option<&JsonValue> {
        self.meta.as_ref()
    }
}

impl Flow for LayerMeta {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.done {
            return None;
        }
        Some(HttpRequest::get_json(
            format!("{}?f=json", self.url),
            self.timeout_ms,
        ))
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        self.done = true;
        let j = match a {
            Ok(Answer::Json(j)) => j,
            Ok(_) => {
                return Err(get_error(
                    &self.url,
                    self.timeout_ms,
                    &FetchFailure::Unreadable("not JSON".to_string()),
                ));
            }
            Err(f) => return Err(get_error(&self.url, self.timeout_ms, &f)),
        };
        if let Some(e) = j.get("error").filter(|e| e.truthy()) {
            let m = e.get("message").filter(|m| m.truthy()).map_or_else(
                || "layer description failed".to_string(),
                JsonValue::js_string,
            );
            return Err(format!("{}: {}", host_text(&self.url), m));
        }
        self.cache.put(&self.url, j.clone());
        self.meta = Some(j);
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.done
    }
}

/// `fieldNames(meta)`: the names of the layer's fields.
pub fn field_names(meta: Option<&JsonValue>) -> Vec<String> {
    match meta.and_then(|m| m.get("fields")) {
        Some(JsonValue::Array(a)) => a
            .iter()
            .map(|f| {
                f.get("name")
                    .map_or_else(|| "undefined".to_string(), JsonValue::js_string)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `resolve(meta, want)`: the layer's own spelling of each wanted field (case-insensitive); fields it lacks are left out, and without a description the wanted names stand.
pub fn resolve_fields(meta: Option<&JsonValue>, want: &[&str]) -> Vec<String> {
    let have = field_names(meta);
    if have.is_empty() {
        return want.iter().map(|w| w.to_string()).collect();
    }
    want.iter()
        .filter_map(|w| {
            have.iter()
                .find(|h| h.to_lowercase() == w.to_lowercase())
                .cloned()
        })
        .collect()
}

/// `env(b)`: the query parameters that restrict a layer to an envelope.
pub fn env_params(b: &Bounds) -> Vec<(String, String)> {
    [
        ("geometry", super::arcgis::bounds_envelope(b)),
        ("geometryType", "esriGeometryEnvelope".to_string()),
        ("inSR", "4326".to_string()),
        ("spatialRel", "esriSpatialRelIntersects".to_string()),
        ("outSR", "4326".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// `arcQuery(url, params, { maxPages, timeout })`: a paged ArcGIS query. Each page is asked for at the offset after the last (the
/// server's own page size) while it says `exceededTransferLimit` and sent features, up to `max_pages`; `complete()` is false when the cap was reached.
#[derive(Clone, Debug)]
pub struct ArcQuery {
    url: String,
    params: Vec<(String, String)>,
    max_pages: usize,
    timeout_ms: f64,
    page: usize,
    offset: usize,
    features: Vec<JsonValue>,
    complete: bool,
    done: bool,
}

impl ArcQuery {
    /// A query of a layer with these parameters (`f=json` is put in front and `resultOffset` after, as the prototype does).
    pub fn new(
        url: &str,
        params: Vec<(String, String)>,
        max_pages: usize,
        timeout_ms: f64,
    ) -> ArcQuery {
        ArcQuery {
            url: url.to_string(),
            params,
            max_pages,
            timeout_ms,
            page: 0,
            offset: 0,
            features: Vec::new(),
            complete: false,
            done: false,
        }
    }
    /// The features collected so far.
    pub fn features(&self) -> &[JsonValue] {
        &self.features
    }
    /// The features, taken.
    pub fn into_features(self) -> Vec<JsonValue> {
        self.features
    }
    /// False when the page cap was reached before the server had no more.
    pub fn complete(&self) -> bool {
        self.complete
    }
}

impl Flow for ArcQuery {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.done || self.max_pages == 0 {
            return None;
        }
        let mut q = vec![("f".to_string(), "json".to_string())];
        q.extend(self.params.iter().cloned());
        q.push(("resultOffset".to_string(), self.offset.to_string()));
        Some(HttpRequest::get_json(
            format!("{}/query?{}", self.url, query_string(&q)),
            self.timeout_ms,
        ))
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        let j = match a {
            Ok(Answer::Json(j)) => j,
            Ok(_) => {
                return Err(get_error(
                    &self.url,
                    self.timeout_ms,
                    &FetchFailure::Unreadable("not JSON".to_string()),
                ));
            }
            Err(f) => {
                self.done = true;
                return Err(get_error(&self.url, self.timeout_ms, &f));
            }
        };
        if j.is_null() {
            self.done = true;
            return Err("Cannot read properties of null (reading 'error')".to_string());
        }
        if let Some(e) = j.get("error").filter(|e| e.truthy()) {
            self.done = true;
            let msg = e
                .get("message")
                .filter(|m| m.truthy())
                .map_or_else(|| "query failed".to_string(), JsonValue::js_string);
            let details = match e.get("details") {
                Some(JsonValue::Array(d)) if !d.is_empty() => format!(
                    " ({})",
                    d.iter()
                        .map(JsonValue::js_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                _ => String::new(),
            };
            return Err(format!("{}: {}{}", host_text(&self.url), msg, details));
        }
        let f: Vec<JsonValue> = match j.get("features") {
            Some(JsonValue::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        let n = f.len();
        self.features.extend(f);
        if !j
            .get("exceededTransferLimit")
            .is_some_and(JsonValue::truthy)
            || n == 0
        {
            self.complete = true;
            self.done = true;
            return Ok(());
        }
        self.offset += n;
        self.page += 1;
        if self.page >= self.max_pages {
            self.done = true;
        }
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.done || self.max_pages == 0
    }
}

/// A GET with the answer read as JSON, one request (`get(url)`); the flow ends with the answer.
#[derive(Clone, Debug)]
pub struct GetOnce {
    req: HttpRequest,
    expect: Expect,
    answer: Option<Answer>,
    done: bool,
}

impl GetOnce {
    /// A request whose single answer is kept (use [`GetOnce::answer_value`] to read it).
    pub fn new(req: HttpRequest) -> GetOnce {
        let expect = req.expect;
        GetOnce {
            req,
            expect,
            answer: None,
            done: false,
        }
    }
    /// The answer, once given.
    pub fn answer_value(&self) -> Option<&Answer> {
        self.answer.as_ref()
    }
    /// The answer, taken.
    pub fn into_answer(self) -> Option<Answer> {
        self.answer
    }
}

impl Flow for GetOnce {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.done {
            None
        } else {
            Some(self.req.clone())
        }
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        self.done = true;
        match a {
            Ok(a) => {
                let _ = self.expect;
                self.answer = Some(a);
                Ok(())
            }
            Err(f) => Err(get_error(&self.req.url, self.req.timeout_ms, &f)),
        }
    }
    fn is_done(&self) -> bool {
        self.done
    }
}

/// `encodeURIComponent` used inside a form body: `data=` plus the text.
pub fn form_body(key: &str, value: &str) -> String {
    format!(
        "{}={}",
        form_encode(key),
        super::arcgis::encode_uri_component(value)
    )
}
