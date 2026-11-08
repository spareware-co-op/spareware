//! GTFS bus timetables (section 5 of feeds.js): bus routes with AM and PM frequencies from a feed.
//!
//! The Mobility Database lists the feeds ([`Catalogue`] finds the ones that cover the study area), and a feed is a zip file
//! of text tables. [`parse_gtfs`] reads the zip as the prototype does (a representative Tuesday in service, not a public
//! holiday and in school term where the term dates are known; stops in the box, bus routes, that day's trips; patterns of
//! stops and departures per hour in the AM and PM windows). [`gtfs_routes`] snaps the stops to the network's roads
//! and gives bus routes with waypoints and frequencies for a scenario.

use super::arcgis::{Bounds, Expect, FetchFailure, HttpRequest, encode_uri_component};
use super::flow::{Answer, Flow, get_error};
use super::geo::{box_of_bounds, pad_box};
use super::jsregex::Regex;
use super::nztime::{DAY_MS, date_text, date_utc, nz_year, today};
use super::sources::{
    GTFS_BUFFER_M, GTFS_LINK_M, NZ_FEEDS, PERIOD_AM, PERIOD_PM, SRC_GTFS, SRC_GTFS_CAT, TERM_DATES,
};
use super::vintage::{feeds_of, stamp};
use super::zip::{member_bytes, read_zip};
use crate::edit::collate::locale_compare_f64;
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::edit::net::nearest_link;
use crate::graph::js_sort_by;
use crate::importers::jsparse::parse_float;
use crate::jsnum::string_to_number;
use crate::project::{Project, Route, Scenario, road_class};
use crate::util::{dist, fmt, r1, r7};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// `ROUTE_COLOURS`: the colours bus routes cycle through.
pub const ROUTE_COLOURS: [&str; 7] = [
    "#7048e8", "#0c8599", "#e8590c", "#c2255c", "#2f9e44", "#1971c2", "#9c36b5",
];

/// A GTFS feed the study area may use.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsFeed {
    /// The feed's id in the Mobility Database.
    pub id: Option<String>,
    /// The operator.
    pub provider: String,
    /// The feed's name (catalogue entries only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The country code (catalogue entries only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
    /// The address of the latest copy (catalogue entries only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// The feed's licence address (catalogue entries only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    /// Coverage [south, west, north, east].
    #[serde(rename = "box")]
    pub box_: [f64; 4],
}

fn area4(b: &[f64; 4]) -> f64 {
    ((b[2] - b[0]) * (b[3] - b[1])).abs()
}

// Catalogue boxes can be far too big (the AT feed's reaches the Chathams because of stray stops), so a feed whose Spare Construct coverage box holds the study-area centre comes first; then the smallest box.
fn rank(f: &GtfsFeed, b: &Bounds) -> f64 {
    let (cy, cx) = ((b.south + b.north) / 2.0, (b.west + b.east) / 2.0);
    match NZ_FEEDS.iter().find(|x| Some(x.id) == f.id.as_deref()) {
        Some(k) if cy >= k.box_[0] && cy <= k.box_[2] && cx >= k.box_[1] && cx <= k.box_[3] => 0.0,
        _ => 1.0,
    }
}

fn sort_feeds(mut hit: Vec<GtfsFeed>, b: &Bounds) -> Vec<GtfsFeed> {
    js_sort_by(&mut hit, |p, q| {
        let d = rank(p, b) - rank(q, b);
        if d != 0.0 {
            d
        } else {
            area4(&p.box_) - area4(&q.box_)
        }
    });
    hit
}

fn covers(f: &[f64; 4], b: &Bounds) -> bool {
    f[0] <= b.north && f[2] >= b.south && f[1] <= b.east && f[3] >= b.west
}

/// `parseCatalogue(text)`: the active GTFS feeds of the Mobility Database list (feeds_v2.csv) that have a coverage box.
pub fn parse_catalogue(text: &str) -> Result<Vec<GtfsFeed>, String> {
    let rows = crate::calibrate::parse_csv(text);
    let Some(h0) = rows.first() else {
        return Err("Cannot read properties of undefined (reading 'map')".to_string());
    };
    let h: Vec<String> = h0
        .iter()
        .map(|s| crate::jsnum::js_trim(s).to_string())
        .collect();
    let col = |n: &str| h.iter().position(|x| x == n);
    let (id, ty, status, cc, prov, name, latest, lic) = (
        col("id"),
        col("data_type"),
        col("status"),
        col("location.country_code"),
        col("provider"),
        col("name"),
        col("urls.latest"),
        col("urls.license"),
    );
    let (s, n, w, e) = (
        col("location.bounding_box.minimum_latitude"),
        col("location.bounding_box.maximum_latitude"),
        col("location.bounding_box.minimum_longitude"),
        col("location.bounding_box.maximum_longitude"),
    );
    let cell =
        |r: &Vec<String>, i: Option<usize>| -> Option<String> { i.and_then(|i| r.get(i).cloned()) };
    let num = |r: &Vec<String>, i: Option<usize>| -> f64 {
        cell(r, i).map_or(f64::NAN, |t| string_to_number(&t))
    };
    let or_empty = |v: Option<String>| v.filter(|t| !t.is_empty()).unwrap_or_default();
    Ok(rows[1..]
        .iter()
        .filter(|r| {
            cell(r, ty).as_deref() == Some("gtfs")
                && cell(r, status).is_none_or(|t| t.is_empty() || t == "active")
        })
        .map(|r| GtfsFeed {
            id: cell(r, id),
            provider: or_empty(cell(r, prov)),
            name: Some(if name.is_some() {
                or_empty(cell(r, name))
            } else {
                String::new()
            }),
            cc: Some(or_empty(cell(r, cc))),
            latest: if latest.is_some() {
                cell(r, latest)
            } else {
                Some(String::new())
            },
            licence: if lic.is_some() {
                cell(r, lic)
            } else {
                Some(String::new())
            },
            box_: [num(r, s), num(r, w), num(r, n), num(r, e)],
        })
        .filter(|f| f.box_.iter().all(|v| v.is_finite()))
        .collect())
}

/// What `catalogue(b)` gives: the feeds that cover the study area, best first, and where they came from.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CatalogueResult {
    /// The feeds.
    pub feeds: Vec<GtfsFeed>,
    /// "catalogue" or "built-in list".
    pub from: String,
    /// Why the built-in list was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The feeds of a catalogue that cover the study area, best first, else (as the prototype does on any problem) the built-in list.
pub fn find_feeds(catalogue: Result<&[GtfsFeed], String>, b: &Bounds) -> CatalogueResult {
    let attempt = match catalogue {
        Ok(list) => {
            let hit: Vec<GtfsFeed> = list
                .iter()
                .filter(|f| covers(&f.box_, b))
                .cloned()
                .collect();
            if hit.is_empty() {
                Err("no feed in the list covers this area".to_string())
            } else {
                Ok(sort_feeds(hit, b))
            }
        }
        Err(e) => Err(e),
    };
    match attempt {
        Ok(feeds) => CatalogueResult {
            feeds,
            from: "catalogue".to_string(),
            error: None,
        },
        Err(e) => {
            let hit: Vec<GtfsFeed> = NZ_FEEDS
                .iter()
                .filter(|f| covers(&f.box_, b))
                .map(|f| GtfsFeed {
                    id: Some(f.id.to_string()),
                    provider: f.provider.to_string(),
                    name: None,
                    cc: None,
                    latest: None,
                    licence: None,
                    box_: f.box_,
                })
                .collect();
            CatalogueResult {
                feeds: sort_feeds(hit, b),
                from: "built-in list".to_string(),
                error: Some(e),
            }
        }
    }
}

/// `catalogue(b)`: the download of the Mobility Database list, then the feeds that cover the study area. Give the list kept from an earlier download to skip it.
pub struct Catalogue {
    bounds: Bounds,
    timeout_ms: f64,
    list: Option<Result<Vec<GtfsFeed>, String>>,
    fresh: bool,
    sent: bool,
}

impl Catalogue {
    /// A search for an area; `timeout_s` is the settings time limit in seconds (the list is allowed at least 45 s); `cached` the list from an earlier download.
    pub fn new(bounds: Bounds, timeout_s: f64, cached: Option<Vec<GtfsFeed>>) -> Catalogue {
        let t = if timeout_s > 45.0 { timeout_s } else { 45.0 };
        Catalogue {
            bounds,
            timeout_ms: t * 1000.0,
            fresh: false,
            sent: cached.is_some(),
            list: cached.map(Ok),
        }
    }
    /// The feeds covering the area, once done.
    pub fn result(&self) -> CatalogueResult {
        match self.list.as_ref().expect("the list was read") {
            Ok(l) => find_feeds(Ok(l), &self.bounds),
            Err(e) => find_feeds(Err(e.clone()), &self.bounds),
        }
    }
    /// The list as downloaded, to keep for the session (None when it was not downloaded or could not be read).
    pub fn fresh_list(&self) -> Option<&[GtfsFeed]> {
        match (&self.list, self.fresh) {
            (Some(Ok(l)), true) => Some(l),
            _ => None,
        }
    }
}

impl Flow for Catalogue {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.sent {
            return None;
        }
        Some(HttpRequest {
            method: "GET".into(),
            url: SRC_GTFS_CAT.url.to_string(),
            body: None,
            headers: Vec::new(),
            timeout_ms: self.timeout_ms,
            expect: Expect::Text,
        })
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        self.sent = true;
        let list = match a {
            Ok(Answer::Text(t)) => parse_catalogue(&t),
            Ok(_) => Err("not text".to_string()),
            Err(f) => Err(get_error(SRC_GTFS_CAT.url, self.timeout_ms, &f)),
        };
        self.fresh = list.is_ok();
        self.list = Some(list);
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.sent
    }
}

/// After the catalogue was downloaded: stamps the vintage table with the number of feeds.
pub fn apply_catalogue(p: &mut Project, feeds: &[GtfsFeed], now_ms: f64) {
    let mut e = Map::new();
    e.insert("n".into(), json!(feeds.len()));
    stamp(p, "gtfsCat", &e, now_ms);
}

/// What the reader of a feed is given (`o`): the box, today, the holidays and school terms, and the peak windows.
#[derive(Clone, Debug, PartialEq)]
pub struct GtfsOpts {
    /// The box [south, west, north, east] stops must be inside.
    pub box_: [f64; 4],
    /// Today as YYYYMMDD.
    pub today: String,
    /// Public holidays as YYYYMMDD.
    pub holidays: Vec<String>,
    /// School terms as [start, end] YYYYMMDD.
    pub terms: Vec<[String; 2]>,
    /// The AM window in hours.
    pub am: [f64; 2],
    /// The PM window in hours.
    pub pm: [f64; 2],
}

/// `feedUrl(f)`: where a feed's zip is: its own latest address, else the Mobility Database mirror.
pub fn feed_url(f: &GtfsFeed) -> String {
    match &f.latest {
        Some(l) if !l.is_empty() => l.clone(),
        _ => SRC_GTFS.url.replace(
            "{id}",
            &encode_uri_component(&f.id.clone().unwrap_or_else(|| "undefined".to_string())),
        ),
    }
}

/// The options `loadGTFS` gives the reader: the study area widened by 500 m, today, the holidays of last year, this year and next (with the feed's region), every school term, and the peak windows.
pub fn gtfs_opts(feed: &GtfsFeed, b: &Bounds, now_ms: f64) -> GtfsOpts {
    let bb = pad_box(&box_of_bounds(b), GTFS_BUFFER_M);
    let d = today(now_ms);
    let yr = nz_year(now_ms);
    let mut hol: Vec<String> = Vec::new();
    for y in [yr - 1, yr, yr + 1] {
        for n in super::tms::holidays(y, &feed.provider) {
            hol.push(super::tms::diso(n).replace('-', ""));
        }
    }
    let terms: Vec<[String; 2]> = TERM_DATES
        .iter()
        .flat_map(|(_, t)| {
            t.iter()
                .map(|(s, e)| [s.replace('-', ""), e.replace('-', "")])
        })
        .collect();
    GtfsOpts {
        box_: [bb.s, bb.w, bb.n, bb.e],
        today: d.replace('-', ""),
        holidays: hol,
        terms,
        am: PERIOD_AM,
        pm: PERIOD_PM,
    }
}

/// One stop the reader found.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsStop {
    /// The stop id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The stop's code.
    pub code: String,
    /// Name.
    pub name: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// Route labels serving it, in route number order.
    pub routes: Vec<String>,
    /// Departures per hour in the AM window.
    pub am: f64,
    /// Departures per hour in the PM window.
    pub pm: f64,
}

/// A route, direction and run of stops with the trips that start in the AM and PM windows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsPattern {
    /// The route id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// The route's short name.
    pub short: String,
    /// The route's long name.
    pub long: String,
    /// Direction ("0" or "1").
    pub dir: String,
    /// Trip headsign.
    pub head: String,
    /// The stop ids in order.
    pub stops: Vec<Option<String>>,
    /// Trips per hour starting in the AM window.
    pub am: f64,
    /// Trips per hour starting in the PM window.
    pub pm: f64,
}

/// The feed's own description (`feed_info.txt`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsFeedInfo {
    /// Publisher.
    pub publisher: String,
    /// The publisher's address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// First day the feed is valid (YYYYMMDD).
    pub start: String,
    /// Last day the feed is valid.
    pub end: String,
    /// The feed's version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// What the reader gives (`gtfsWork`'s result).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GtfsResult {
    /// The service day, YYYYMMDD.
    pub date: String,
    /// The feed has no later Tuesday: the day is the last one before today.
    pub stale: bool,
    /// The day is in a school term (null when the year's term dates are not entered).
    pub term: Option<bool>,
    /// Services running that day.
    pub services: usize,
    /// Bus trips that day.
    pub trips: usize,
    /// Rows of stop_times.txt.
    pub stop_times: usize,
    /// The feed's description.
    pub feed_info: Option<GtfsFeedInfo>,
    /// The agencies' names.
    pub agencies: Vec<String>,
    /// Stops in the box that a bus serves.
    pub stops: Vec<GtfsStop>,
    /// Patterns that start in the AM or PM window.
    pub patterns: Vec<GtfsPattern>,
    /// Time taken (ms); the port reports 0.
    pub ms: f64,
}

// One CSV line the way the reader splits it: quotes only matter within the line
fn split_line(s: &str) -> Vec<String> {
    if !s.contains('"') {
        return s.split(',').map(str::to_string).collect();
    }
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        if q {
            if ch == '"' {
                if cs.get(i + 1) == Some(&'"') {
                    cur.push('"');
                    i += 1;
                } else {
                    q = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            q = true;
        } else if ch == ',' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    out.push(cur);
    out
}

type Head = HashMap<String, usize>;

// A member as text, the way `TextDecoder('utf-8')` reads it: a byte order mark is dropped, bad bytes become U+FFFD
fn decode_text(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    String::from_utf8_lossy(b).to_string()
}

// rows(name, fn): every data row of a table with its header, and how many rows there were
fn for_each_row(text: &str, mut f: impl FnMut(&[String], &Head)) -> usize {
    let mut head: Option<Head> = None;
    let mut count = 0;
    for part in text.split('\n') {
        let line = part.strip_suffix('\r').unwrap_or(part);
        if line.is_empty() {
            continue;
        }
        let c = split_line(line);
        match &head {
            None => {
                let mut h = Head::new();
                for (i, k) in c.iter().enumerate() {
                    h.insert(
                        crate::jsnum::js_trim(k.trim_start_matches('\u{feff}')).to_string(),
                        i,
                    );
                }
                head = Some(h);
            }
            Some(h) => {
                count += 1;
                f(&c, h);
            }
        }
    }
    count
}

fn cell<'a>(c: &'a [String], h: &Head, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|i| c.get(*i)).map(String::as_str)
}

// c[h.x] || ''
fn cell_or_empty(c: &[String], h: &Head, name: &str) -> String {
    cell(c, h, name).unwrap_or("").to_string()
}

fn num_cell(c: &[String], h: &Head, name: &str) -> f64 {
    cell(c, h, name).map_or(f64::NAN, string_to_number)
}

fn ymd(n: f64) -> String {
    super::nztime::utc_date(n * DAY_MS).replace('-', "")
}

fn num_of_ymd(s: &str) -> f64 {
    let n =
        |a: usize, b: usize| string_to_number(&s.chars().skip(a).take(b - a).collect::<String>());
    crate::jsnum::math_round(date_utc(n(0, 4), n(4, 6) - 1.0, n(6, 8)) / DAY_MS)
}

// seconds of a time like 07:45:00 (hours may pass 24), NaN when it is not one
fn sec(s: Option<&str>) -> f64 {
    let re = Regex::lit(r"^\s*(\d+):(\d{2})(?::(\d{2}))?", "");
    let text = s.unwrap_or("");
    let u: Vec<u16> = text.encode_utf16().collect();
    match re.exec_units(&u, 0) {
        Some(m) => {
            let g = |i: usize| m.text(&u, i);
            let n = |t: Option<String>| t.map_or(0.0, |t| string_to_number(&t));
            n(g(1)) * 3600.0 + n(g(2)) * 60.0 + n(g(3))
        }
        None => f64::NAN,
    }
}

fn truthy(s: Option<&str>) -> bool {
    s.is_some_and(|x| !x.is_empty())
}

struct Trip {
    r: Option<String>,
    dir: String,
    head: String,
    st: Vec<(f64, Option<String>, f64)>,
}

/// `gtfsWork(buf, o)`: reads a GTFS zip.
///
/// Finds a representative Tuesday in service that is not a public holiday (in a school term first, where the term dates
/// are known; else the next one; else the latest before today, marked stale), the stops inside the box, the bus routes
/// (route type 3 or 700 to 799) and that day's trips, and builds the patterns (route, direction and stops) with the
/// trips per hour that start in the AM and PM windows. Errors say what is wrong with the feed.
pub fn parse_gtfs(buf: &[u8], o: &GtfsOpts) -> Result<GtfsResult, String> {
    let files = read_zip(buf)?;
    let rows_of = |name: &str, need: bool| -> Result<Option<String>, String> {
        match files.get(name) {
            None => {
                if need {
                    Err(format!("The feed has no {name}, so it cannot be used."))
                } else {
                    Ok(None)
                }
            }
            Some(f) => Ok(Some(decode_text(&member_bytes(buf, f, name)?))),
        }
    };
    // 1. calendar: a representative Tuesday in service that is not a public holiday
    struct Cal {
        id: Option<String>,
        tue: bool,
        s: Option<String>,
        e: Option<String>,
    }
    let mut cal: Vec<Cal> = Vec::new();
    let mut ex: Vec<(Option<String>, Vec<(Option<String>, Option<String>)>)> = Vec::new();
    let hol: HashSet<&str> = o.holidays.iter().map(String::as_str).collect();
    if let Some(t) = rows_of("calendar.txt", false)? {
        for_each_row(&t, |c, h| {
            cal.push(Cal {
                id: cell(c, h, "service_id").map(str::to_string),
                tue: cell(c, h, "tuesday") == Some("1"),
                s: cell(c, h, "start_date").map(str::to_string),
                e: cell(c, h, "end_date").map(str::to_string),
            });
        });
    }
    if let Some(t) = rows_of("calendar_dates.txt", false)? {
        for_each_row(&t, |c, h| {
            let d = cell(c, h, "date").map(str::to_string);
            let sid = cell(c, h, "service_id").map(str::to_string);
            let ty = cell(c, h, "exception_type").map(str::to_string);
            let m = match ex.iter_mut().find(|(k, _)| *k == d) {
                Some(m) => m,
                None => {
                    ex.push((d, Vec::new()));
                    ex.last_mut().expect("just pushed")
                }
            };
            match m.1.iter_mut().find(|(k, _)| *k == sid) {
                Some(e) => e.1 = ty,
                None => m.1.push((sid, ty)),
            }
        });
    }
    let js_le = |a: &Option<String>, b: &str| {
        a.as_deref()
            .is_some_and(|x| x.encode_utf16().cmp(b.encode_utf16()) != std::cmp::Ordering::Greater)
    };
    let js_ge_opt = |a: &str, b: &Option<String>| {
        b.as_deref()
            .is_some_and(|x| a.encode_utf16().cmp(x.encode_utf16()) != std::cmp::Ordering::Greater)
    };
    let active = |d: &str| -> Vec<Option<String>> {
        let mut s: Vec<Option<String>> = Vec::new();
        for r in &cal {
            if r.tue && js_le(&r.s, d) && js_ge_opt(d, &r.e) && !s.contains(&r.id) {
                s.push(r.id.clone());
            }
        }
        if let Some((_, m)) = ex.iter().find(|(k, _)| k.as_deref() == Some(d)) {
            for (id, t) in m {
                if t.as_deref() == Some("1") {
                    if !s.contains(id) {
                        s.push(id.clone());
                    }
                } else {
                    s.retain(|x| x != id);
                }
            }
        }
        s
    };
    let mut date: Option<String> = None;
    let mut services: Vec<Option<String>> = Vec::new();
    let mut stale = false;
    let t = num_of_ymd(&o.today);
    // School terms: a term-time Tuesday first; a year without term dates counts as unknown
    let t_years: HashSet<String> = o
        .terms
        .iter()
        .map(|x| x[0].chars().take(4).collect())
        .collect();
    let term_of = |d: &str| -> Option<bool> {
        let y: String = d.chars().take(4).collect();
        if t_years.contains(&y) {
            Some(
                o.terms
                    .iter()
                    .any(|[s, e]| d >= s.as_str() && d <= e.as_str()),
            )
        } else {
            None
        }
    };
    for need_term in [true, false] {
        let mut k = 0;
        while k < 200 && date.is_none() {
            let n = t + k as f64;
            k += 1;
            let day = ymd(n);
            if (n + 4.0) % 7.0 != 2.0
                || hol.contains(day.as_str())
                || (need_term && term_of(&day) == Some(false))
            {
                continue;
            }
            let s = active(&day);
            if !s.is_empty() {
                date = Some(day);
                services = s;
            }
        }
    }
    let mut k = 1;
    while k < 400 && date.is_none() {
        let n = t - k as f64;
        k += 1;
        let day = ymd(n);
        if (n + 4.0) % 7.0 != 2.0 || hol.contains(day.as_str()) {
            continue;
        }
        let s = active(&day);
        if !s.is_empty() {
            date = Some(day);
            services = s;
            stale = true;
        }
    }
    let Some(date) = date else {
        return Err(
            "The feed has no Tuesday with buses in service. It may have expired.".to_string(),
        );
    };
    // 2. stops in the box, bus routes, trips that day
    let [bs, bw, bn, be] = o.box_;
    struct StopB {
        id: Option<String>,
        code: String,
        name: String,
        lat: f64,
        lng: f64,
        routes: Vec<String>,
        am: f64,
        pm: f64,
    }
    let mut stops: Vec<StopB> = Vec::new();
    let mut stop_ix: HashMap<Option<String>, usize> = HashMap::new();
    if let Some(t) = rows_of("stops.txt", true)? {
        for_each_row(&t, |c, h| {
            let (lat, lng) = (num_cell(c, h, "stop_lat"), num_cell(c, h, "stop_lon"));
            if lat >= bs && lat <= bn && lng >= bw && lng <= be {
                let id = cell(c, h, "stop_id").map(str::to_string);
                let s = StopB {
                    id: id.clone(),
                    code: if h.contains_key("stop_code") {
                        cell_or_empty(c, h, "stop_code")
                    } else {
                        String::new()
                    },
                    name: cell_or_empty(c, h, "stop_name"),
                    lat,
                    lng,
                    routes: Vec::new(),
                    am: 0.0,
                    pm: 0.0,
                };
                match stop_ix.get(&id) {
                    Some(&i) => stops[i] = s,
                    None => {
                        stop_ix.insert(id, stops.len());
                        stops.push(s);
                    }
                }
            }
        });
    }
    let mut routes: HashMap<Option<String>, (String, String)> = HashMap::new();
    if let Some(t) = rows_of("routes.txt", true)? {
        for_each_row(&t, |c, h| {
            let ty = num_cell(c, h, "route_type");
            if ty == 3.0 || (ty >= 700.0 && ty < 800.0) {
                routes.insert(
                    cell(c, h, "route_id").map(str::to_string),
                    (
                        cell_or_empty(c, h, "route_short_name"),
                        cell_or_empty(c, h, "route_long_name"),
                    ),
                );
            }
        });
    }
    let mut trips: Vec<Trip> = Vec::new();
    let mut trip_ix: HashMap<Option<String>, usize> = HashMap::new();
    if let Some(t) = rows_of("trips.txt", true)? {
        for_each_row(&t, |c, h| {
            let r = cell(c, h, "route_id").map(str::to_string);
            let sid = cell(c, h, "service_id").map(str::to_string);
            if routes.contains_key(&r) && services.contains(&sid) {
                let id = cell(c, h, "trip_id").map(str::to_string);
                let trip = Trip {
                    r,
                    dir: if h.contains_key("direction_id") {
                        let v = cell_or_empty(c, h, "direction_id");
                        if v.is_empty() { "0".to_string() } else { v }
                    } else {
                        "0".to_string()
                    },
                    head: if h.contains_key("trip_headsign") {
                        cell_or_empty(c, h, "trip_headsign")
                    } else {
                        String::new()
                    },
                    st: Vec::new(),
                };
                match trip_ix.get(&id) {
                    Some(&i) => trips[i] = trip,
                    None => {
                        trip_ix.insert(id, trips.len());
                        trips.push(trip);
                    }
                }
            }
        });
    }
    let mut nst = 0;
    if let Some(t) = rows_of("stop_times.txt", true)? {
        nst = for_each_row(&t, |c, h| {
            let Some(&ti) = trip_ix.get(&cell(c, h, "trip_id").map(str::to_string)) else {
                return;
            };
            let sid = cell(c, h, "stop_id").map(str::to_string);
            if !stop_ix.contains_key(&sid) {
                return;
            }
            let dep = cell(c, h, "departure_time");
            let when = if truthy(dep) {
                dep
            } else {
                cell(c, h, "arrival_time")
            };
            trips[ti]
                .st
                .push((num_cell(c, h, "stop_sequence"), sid, sec(when)));
        });
    }
    // 3. patterns (route, direction and the stops in the box) and departures in the AM and PM windows
    let win = |x: f64, w: [f64; 2]| x >= w[0] * 3600.0 && x < w[1] * 3600.0;
    let mut pats: Vec<GtfsPattern> = Vec::new();
    let mut pat_ix: HashMap<String, usize> = HashMap::new();
    for tr in trips.iter_mut() {
        if tr.st.is_empty() {
            continue;
        }
        js_sort_by(&mut tr.st, |x, y| x.0 - y.0);
        let (short, long) = routes[&tr.r].clone();
        let label = if !short.is_empty() {
            short.clone()
        } else {
            long.clone()
        };
        let t1 = tr
            .st
            .iter()
            .find(|x| x.2.is_finite())
            .map_or(f64::NAN, |x| x.2);
        for x in &tr.st {
            let s = &mut stops[stop_ix[&x.1]];
            if !s.routes.contains(&label) {
                s.routes.push(label.clone());
            }
            if x.2.is_finite() {
                if win(x.2, o.am) {
                    s.am += 1.0;
                }
                if win(x.2, o.pm) {
                    s.pm += 1.0;
                }
            }
        }
        let (am, pm) = (win(t1, o.am), win(t1, o.pm));
        let seq: Vec<Option<String>> = tr.st.iter().map(|x| x.1.clone()).collect();
        if seq.len() < 2 || !(am || pm) {
            continue;
        }
        let key = format!(
            "{}|{}|{}",
            tr.r.clone().unwrap_or_else(|| "undefined".to_string()),
            tr.dir,
            seq.iter()
                .map(|s| s.clone().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(">")
        );
        let i = match pat_ix.get(&key) {
            Some(&i) => i,
            None => {
                pat_ix.insert(key, pats.len());
                pats.push(GtfsPattern {
                    route: tr.r.clone(),
                    short,
                    long,
                    dir: tr.dir.clone(),
                    head: tr.head.clone(),
                    stops: seq,
                    am: 0.0,
                    pm: 0.0,
                });
                pats.len() - 1
            }
        };
        if am {
            pats[i].am += 1.0;
        }
        if pm {
            pats[i].pm += 1.0;
        }
    }
    let mut feed_info: Option<GtfsFeedInfo> = None;
    let mut agencies: Vec<String> = Vec::new();
    if let Some(t) = rows_of("feed_info.txt", false)? {
        for_each_row(&t, |c, h| {
            feed_info = Some(GtfsFeedInfo {
                publisher: cell_or_empty(c, h, "feed_publisher_name"),
                url: if h.contains_key("feed_publisher_url") {
                    cell(c, h, "feed_publisher_url").map(str::to_string)
                } else {
                    Some(String::new())
                },
                start: cell_or_empty(c, h, "feed_start_date"),
                end: cell_or_empty(c, h, "feed_end_date"),
                version: if h.contains_key("feed_version") {
                    cell(c, h, "feed_version").map(str::to_string)
                } else {
                    Some(String::new())
                },
            });
        });
    }
    if let Some(t) = rows_of("agency.txt", false)? {
        for_each_row(&t, |c, h| agencies.push(cell_or_empty(c, h, "agency_name")));
    }
    let h_a = o.am[1] - o.am[0];
    let h_p = o.pm[1] - o.pm[0];
    let by_num = |x: &String, y: &String| -> f64 {
        let key = |s: &String| {
            let v = parse_float(s);
            if v == 0.0 || v.is_nan() { 1e9 } else { v }
        };
        let d = key(x) - key(y);
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            locale_compare_f64(x, y)
        }
    };
    let out_stops: Vec<GtfsStop> = stops
        .into_iter()
        .filter(|s| !s.routes.is_empty())
        .map(|s| {
            let mut r = s.routes.clone();
            js_sort_by(&mut r, by_num);
            GtfsStop {
                id: s.id,
                code: s.code,
                name: s.name,
                lat: s.lat,
                lng: s.lng,
                routes: r,
                am: s.am / h_a,
                pm: s.pm / h_p,
            }
        })
        .collect();
    let patterns: Vec<GtfsPattern> = pats
        .into_iter()
        .map(|q| GtfsPattern {
            am: q.am / h_a,
            pm: q.pm / h_p,
            ..q
        })
        .collect();
    Ok(GtfsResult {
        term: term_of(&date),
        date,
        stale,
        services: services.len(),
        trips: trips.len(),
        stop_times: nst,
        feed_info,
        agencies,
        stops: out_stops,
        patterns,
        ms: 0.0,
    })
}

/// `loadGTFS(feed, b)`: the download of a feed's timetable (with the optional HEAD request that tells when it was last changed) and its reading.
///
/// The feed's own cache (the prototype keeps parsed feeds in IndexedDB) is the host's affair: when the HEAD answer shows a
/// copy it holds is current, it can stop the flow after the first answer and use its own result.
pub struct GtfsLoad {
    feed: GtfsFeed,
    opts: GtfsOpts,
    url: String,
    timeout_ms: f64,
    stage: u8,
    last_modified: Option<String>,
    result: Option<GtfsResult>,
    statuses: Vec<String>,
}

/// What `loadGTFS` gives: the reading and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GtfsLoaded {
    /// The reading.
    #[serde(flatten)]
    pub result: GtfsResult,
    /// The download's `Last-Modified` header (else the HEAD answer's).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// The feed.
    pub feed: GtfsFeedRef,
}

/// The feed a reading came from (`r.feed`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsFeedRef {
    /// The feed's id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The operator.
    pub provider: String,
    /// The feed's name.
    pub name: String,
    /// The address it was downloaded from.
    pub url: String,
    /// The feed's licence.
    pub licence: String,
}

impl GtfsLoad {
    /// A download for a feed and an area. `timeout_s` is the settings time limit in seconds (the zip is allowed at least 120 s).
    pub fn new(feed: GtfsFeed, b: &Bounds, now_ms: f64, timeout_s: f64) -> GtfsLoad {
        let url = feed_url(&feed);
        let opts = gtfs_opts(&feed, b, now_ms);
        GtfsLoad {
            feed,
            opts,
            url,
            timeout_ms: (if timeout_s > 120.0 { timeout_s } else { 120.0 }) * 1000.0,
            stage: 0,
            last_modified: None,
            result: None,
            statuses: Vec::new(),
        }
    }
    /// The options the reader was given.
    pub fn opts(&self) -> &GtfsOpts {
        &self.opts
    }
    /// The progress lines the prototype passes to its status callback, in order.
    pub fn statuses(&self) -> &[String] {
        &self.statuses
    }
    /// The reading, with the feed and the time the file was last changed.
    pub fn finish(self) -> Option<GtfsLoaded> {
        let result = self.result?;
        Some(GtfsLoaded {
            result,
            last_modified: self.last_modified,
            feed: GtfsFeedRef {
                id: self.feed.id.clone(),
                provider: self.feed.provider.clone(),
                name: self
                    .feed
                    .name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_default(),
                url: self.url,
                licence: self
                    .feed
                    .licence
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_default(),
            },
        })
    }
}

impl Flow for GtfsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        match self.stage {
            0 => Some(HttpRequest {
                method: "HEAD".into(),
                url: self.url.clone(),
                body: None,
                headers: Vec::new(),
                timeout_ms: 10_000.0,
                expect: Expect::Json,
            }),
            1 => {
                self.statuses.push(format!(
                    "Downloading the {} timetable\u{2026}",
                    self.feed.provider_or_id()
                ));
                Some(HttpRequest {
                    method: "GET".into(),
                    url: self.url.clone(),
                    body: None,
                    headers: Vec::new(),
                    timeout_ms: self.timeout_ms,
                    expect: Expect::Buffer,
                })
            }
            _ => None,
        }
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        if self.stage == 0 {
            // the HEAD request only serves the cache: a failure is ignored
            if let Ok(Answer::Head { last_modified }) = a {
                self.last_modified = last_modified;
            }
            self.stage = 1;
            return Ok(());
        }
        let (data, lm) = match a {
            Ok(Answer::Bytes {
                data,
                last_modified,
            }) => (data, last_modified),
            Ok(_) => {
                return Err(get_error(
                    &self.url,
                    self.timeout_ms,
                    &FetchFailure::Unreadable("not a file".to_string()),
                ));
            }
            Err(f) => return Err(get_error(&self.url, self.timeout_ms, &f)),
        };
        self.statuses.push(format!(
            "Reading {} MB of timetable for {}\u{2026}",
            fmt::num(data.len() as f64 / 1_048_576.0, 1),
            self.opts.today.chars().take(4).collect::<String>()
        ));
        self.result = Some(parse_gtfs(&data, &self.opts)?);
        if lm.as_deref().is_some_and(|l| !l.is_empty()) {
            self.last_modified = lm;
        }
        self.stage = 2;
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.stage >= 2
    }
}

impl GtfsFeed {
    fn provider_or_id(&self) -> String {
        if !self.provider.is_empty() {
            self.provider.clone()
        } else {
            self.id.clone().unwrap_or_else(|| "undefined".to_string())
        }
    }
}

/// `hashStr(s)`: a short text from a text (32-bit FNV-1a, in base 36).
pub fn hash_str(s: &str) -> String {
    let mut h: u32 = 2_166_136_261;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(16_777_619);
    }
    crate::edit::ids::integer_radix36(h as f64)
}

/// A bus route made from a pattern (`gtfsRoutes` entry).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GtfsRoute {
    /// Route id, "gtfs-" and a hash of the route, direction and waypoints.
    pub id: String,
    /// Name, "<number> to <headsign>".
    pub name: String,
    /// Map colour.
    pub color: String,
    /// The busier peak's buses per hour.
    pub freq: f64,
    /// Buses per hour by peak.
    pub freqs: Value,
    /// Waypoint junction ids.
    pub wp: Vec<String>,
    /// "GTFS".
    pub src: String,
    /// Where the route came from.
    pub gtfs: Value,
}

/// `gtfsRoutes(sc, g)`: bus routes for a scenario from a reading.
///
/// Each stop snaps to the nearest road (not a motorway or ramp) within 60 m, and the route passes the end of that road
/// towards the next stop, so stops in the middle of long rural links still count. Stops outside the network are skipped.
/// Patterns that give the same waypoints are merged (their frequencies add). Busiest first.
pub fn gtfs_routes(sc: &Scenario, g: &GtfsLoaded) -> Vec<GtfsRoute> {
    let by_stop: HashMap<Option<String>, &GtfsStop> =
        g.result.stops.iter().map(|s| (s.id.clone(), s)).collect();
    let mut snap: HashMap<Option<String>, Option<String>> = HashMap::new();
    let bus_road =
        |l: &crate::project::Link| !l.status.is("closed") && road_class(l.cls.as_str()).rank < 5.0;
    let mut link_of = |id: &Option<String>| -> Option<String> {
        if !snap.contains_key(id) {
            let v = by_stop
                .get(id)
                .and_then(|s| nearest_link(sc, [s.lat, s.lng], GTFS_LINK_M, Some(&bus_road)))
                .map(|h| h.id);
            snap.insert(id.clone(), v);
        }
        snap[id].clone()
    };
    let ll = |id: Option<&Option<String>>| -> Option<[f64; 2]> {
        id.and_then(|i| by_stop.get(i)).map(|s| [s.lat, s.lng])
    };
    let node = |id: &str| -> [f64; 2] {
        let n = &sc.nodes[id];
        [n.lat.num(), n.lng.num()]
    };
    struct Merged {
        p: GtfsPattern,
        wp: Vec<String>,
    }
    let mut merged: Vec<Merged> = Vec::new();
    let mut m_ix: HashMap<String, usize> = HashMap::new();
    for p in &g.result.patterns {
        let mut wp: Vec<String> = Vec::new();
        for (i, s) in p.stops.iter().enumerate() {
            let Some(hit) = link_of(s) else { continue };
            let l = &sc.links[hit.as_str()];
            let nx = ll(p.stops.get(i + 1));
            let pv = if i > 0 { ll(p.stops.get(i - 1)) } else { None };
            // the exit end: nearer the next stop, or (for the last stop) further from the previous one
            let n = if let Some(nx) = nx {
                if dist(&node(&l.a), &nx) <= dist(&node(&l.b), &nx) {
                    l.a.clone()
                } else {
                    l.b.clone()
                }
            } else if let Some(pv) = pv {
                if dist(&node(&l.a), &pv) >= dist(&node(&l.b), &pv) {
                    l.a.clone()
                } else {
                    l.b.clone()
                }
            } else {
                l.b.clone()
            };
            if wp.last() != Some(&n) {
                wp.push(n);
            }
        }
        let distinct: HashSet<&String> = wp.iter().collect();
        if distinct.len() < 2 {
            continue;
        }
        let key = format!(
            "{}|{}|{}",
            p.route.clone().unwrap_or_else(|| "undefined".to_string()),
            p.dir,
            wp.join(">")
        );
        match m_ix.get(&key) {
            Some(&i) => {
                merged[i].p.am += p.am;
                merged[i].p.pm += p.pm;
            }
            None => {
                m_ix.insert(key, merged.len());
                merged.push(Merged { p: p.clone(), wp });
            }
        }
    }
    js_sort_by(&mut merged, |x, y| (y.p.am + y.p.pm) - (x.p.am + x.p.pm));
    merged
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let p = &m.p;
            let (am, pm) = (r1(p.am), r1(p.pm));
            let label = if !p.short.is_empty() { p.short.clone() } else if !p.long.is_empty() { p.long.clone() } else { "Bus".to_string() };
            let name = format!("{}{}", label, if p.head.is_empty() { String::new() } else { format!(" to {}", p.head) });
            GtfsRoute {
                id: format!("gtfs-{}", hash_str(&format!("{}|{}|{}", p.route.clone().unwrap_or_else(|| "undefined".to_string()), p.dir, m.wp.join(">")))),
                name,
                color: ROUTE_COLOURS[i % ROUTE_COLOURS.len()].to_string(),
                freq: crate::jsnum::math_max(am, pm),
                freqs: json!({ "AM": am, "PM": pm }),
                wp: m.wp.clone(),
                src: "GTFS".to_string(),
                gtfs: json!({ "feed": g.feed.id, "provider": g.feed.provider, "date": g.result.date, "route": p.short, "dir": p.dir, "stops": p.stops.len() }),
            }
        })
        .collect()
}

/// `applyGTFS(sc, routes)`: replaces the scenario's earlier timetable routes with these; gives how many were added.
pub fn apply_gtfs(sc: &mut Scenario, routes: &[GtfsRoute]) -> usize {
    let old: Vec<String> = sc
        .routes
        .iter()
        .filter(|(_, r)| r.extra.get("src").and_then(Value::as_str) == Some("GTFS"))
        .map(|(id, _)| id.to_string())
        .collect();
    for id in old {
        sc.routes.remove(&id);
    }
    for r in routes {
        let v = serde_json::to_value(r).expect("route");
        let route: Route = serde_json::from_value(v).expect("a route reads");
        sc.routes.insert(r.id.clone(), route);
    }
    routes.len()
}

/// `feedDate(g)`: the feed's own date: the download's Last-Modified, then the feed's version and validity ("feed dated 21 Oct 2026; version 3, valid 20260101\u{2013}20261231").
pub fn feed_date(g: &GtfsLoaded) -> String {
    let fi = g.result.feed_info.as_ref();
    let lm = match &g.last_modified {
        Some(l) if !l.is_empty() => date_text(Some(&JsonValue::String(l.clone()))),
        _ => String::new(),
    };
    let mut v: Vec<String> = Vec::new();
    if let Some(fi) = fi {
        if let Some(ver) = fi.version.as_ref().filter(|x| !x.is_empty()) {
            v.push(format!("version {ver}"));
        }
        if !fi.start.is_empty() && !fi.end.is_empty() {
            v.push(format!("valid {}\u{2013}{}", fi.start, fi.end));
        }
    }
    let v = v.join(", ");
    let mut parts: Vec<String> = Vec::new();
    if !lm.is_empty() {
        parts.push(format!("feed dated {lm}"));
    }
    if !v.is_empty() {
        parts.push(v);
    }
    parts.join("; ")
}

/// `keepGTFS(g, routes)`: records the reading in `P.feeds.gtfs` (the feed, service day, publisher and every stop with its frequencies) and stamps the vintage table.
pub fn keep_gtfs(p: &mut Project, g: &GtfsLoaded, routes: &[GtfsRoute], now_ms: f64) {
    let fd = feed_date(g);
    let publisher = match g
        .result
        .feed_info
        .as_ref()
        .map(|f| f.publisher.clone())
        .filter(|x| !x.is_empty())
    {
        Some(pb) => pb,
        None => g.result.agencies.join(", "),
    };
    let stops: Vec<Value> = g
        .result
        .stops
        .iter()
        .map(|s| {
            let mut o = Map::new();
            if let Some(id) = &s.id {
                o.insert("id".into(), json!(id));
            }
            o.insert("code".into(), json!(s.code));
            o.insert("name".into(), json!(s.name));
            o.insert("mode".into(), json!("Bus"));
            o.insert("lat".into(), crate::jsnum::json_number(r7(s.lat)));
            o.insert("lng".into(), crate::jsnum::json_number(r7(s.lng)));
            o.insert("routes".into(), json!(s.routes));
            o.insert("am".into(), crate::jsnum::json_number(r1(s.am)));
            o.insert("pm".into(), crate::jsnum::json_number(r1(s.pm)));
            Value::Object(o)
        })
        .collect();
    let feed = serde_json::to_value(&g.feed).expect("feed");
    feeds_of(p).insert(
        "gtfs".to_string(),
        json!({
            "at": iso_string(now_ms), "feed": feed, "date": g.result.date, "stale": g.result.stale, "term": g.result.term, "feedDate": fd,
            "publisher": publisher, "routes": routes.len(), "stops": stops,
        }),
    );
    let d = &g.result.date;
    let pub_or_provider = if !publisher.is_empty() {
        publisher.clone()
    } else {
        g.feed.provider.clone()
    };
    let tail = if g.result.stale {
        " (the feed has no later Tuesday: it may be out of date)"
    } else if g.result.term == Some(false) {
        " (not in a school term: the feed has no term-time Tuesday)"
    } else {
        ""
    };
    let day = |a: usize, b: usize| -> String { d.chars().skip(a).take(b - a).collect() };
    let note = format!(
        "{}{}; service day {}/{}/{}{}; {} route patterns in the study area.",
        pub_or_provider,
        if fd.is_empty() {
            String::new()
        } else {
            format!(" ({fd})")
        },
        day(6, 8),
        day(4, 6),
        day(0, 4),
        tail,
        routes.len()
    );
    let mut extra = Map::new();
    extra.insert("key".into(), json!("gtfs"));
    extra.insert("url".into(), json!(g.feed.url));
    extra.insert(
        "licence".into(),
        json!(if !g.feed.licence.is_empty() {
            g.feed.licence.clone()
        } else {
            SRC_GTFS.licence.to_string()
        }),
    );
    extra.insert("n".into(), json!(g.result.stops.len()));
    extra.insert("note".into(), json!(note));
    stamp(p, "gtfs", &extra, now_ms);
}
