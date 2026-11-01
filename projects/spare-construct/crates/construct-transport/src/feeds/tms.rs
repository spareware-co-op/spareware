//! NZTA TMS daily counts (section 3 of feeds.js): download, clean up and summarise daily traffic at state highway telemetry sites.
//!
//! Duplicate rows are removed by the server (group by and max). Days under half of the 28-day median are dropped (a
//! partial or faulty day). Per site: the Tuesday to Thursday school-term mean with a 95% confidence interval (the
//! calibration target), AADT and AAWDT from the last 12 months, %HCV, day-of-week and monthly factors, and growth.
//! The statistics are put on the NZTA counts (`tms`), and the peak profiles (shares of daily traffic in the AM and PM peaks) too.

use super::arcgis::{FetchFailure, HttpRequest};
use super::flow::{Answer, ArcQuery, Flow, GetOnce};
use super::jsregex::Regex;
use super::nztime::{DAY_MS, date_of, date_utc, nz_year, today};
use super::sources::{
    MATARIKI_DATES, SRC_PROFILES, SRC_TMS, TMS_AADT_DAYS, TMS_CHUNK, TMS_HCV_RULE, TMS_MAX_PAGES,
    TMS_MEDIAN_DAYS, TMS_MIN_SHARE, TMS_TERM_WEEKS, TMS_Z, term_dates,
};
use super::vintage::{feeds_of, stamp};
use crate::datahub::{Count, period_vol};
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, math_max_of, math_min_of, math_round, value_to_number, value_truthy};
use crate::project::{Project, Settings};
use crate::util::fmt;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

/// `dnum(iso)`: days since 1970-01-01 of a date written YYYY-MM-DD.
pub fn dnum(iso: &str) -> f64 {
    math_round(super::nztime::parse_date(&format!("{iso}T00:00:00Z")).unwrap_or(f64::NAN) / DAY_MS)
}

/// `diso(n)`: the date (YYYY-MM-DD) of a day count since 1970-01-01.
pub fn diso(n: f64) -> String {
    super::nztime::utc_date(n * DAY_MS)
}

/// `dow(n)`: the weekday of a day count, 0 = Sunday.
pub fn dow(n: f64) -> f64 {
    (n + 4.0) % 7.0
}

fn md(y: f64, m: f64, d: f64) -> f64 {
    math_round(date_utc(y, m - 1.0, d) / DAY_MS)
}

/// `easter(y)`: Easter Sunday of a year as a day count (the Gregorian computus).
pub fn easter(y: f64) -> f64 {
    let a = y % 19.0;
    let b = (y / 100.0).floor();
    let c = y % 100.0;
    let d = (b / 4.0).floor();
    let e = b % 4.0;
    let f = ((b + 8.0) / 25.0).floor();
    let g = ((b - f + 1.0) / 3.0).floor();
    let h = (19.0 * a + b - d - g + 15.0) % 30.0;
    let i = (c / 4.0).floor();
    let k = c % 4.0;
    let l = (32.0 + 2.0 * e + 2.0 * i - h - k) % 7.0;
    let m = ((a + 11.0 * h + 22.0 * l) / 451.0).floor();
    let mo = ((h + l - 7.0 * m + 114.0) / 31.0).floor();
    let da = ((h + l - 7.0 * m + 114.0) % 31.0) + 1.0;
    math_round(date_utc(y, mo - 1.0, da) / DAY_MS)
}

/// `nthMonday(y, m, nth)`: the nth Monday of a month (m counted from 0) as a day count.
pub fn nth_monday(y: f64, m: f64, nth: f64) -> f64 {
    let mut d = math_round(date_utc(y, m, 1.0) / DAY_MS);
    while dow(d) != 1.0 {
        d += 1.0;
    }
    d + 7.0 * (nth - 1.0)
}

/// `holidays(y, region)`: New Zealand public holidays of a year as day counts, Mondayised (Holidays Act 2003), in the order the prototype adds them.
///
/// Auckland Anniversary (the Monday nearest 29 January) is added for the Auckland region. Matariki only for the years entered in [`MATARIKI_DATES`].
pub fn holidays(y: i64, region: &str) -> Vec<f64> {
    let yf = y as f64;
    let mut out: Vec<f64> = Vec::new();
    let add = |out: &mut Vec<f64>, d: f64| {
        if !out.contains(&d) {
            out.push(d);
        }
    };
    let weekday = |d: f64| dow(d) != 0.0 && dow(d) != 6.0;
    // New Year and Christmas pairs move to the next free weekdays
    for pair in [
        [md(yf, 1.0, 1.0), md(yf, 1.0, 2.0)],
        [md(yf, 12.0, 25.0), md(yf, 12.0, 26.0)],
    ] {
        for d in pair {
            if weekday(d) {
                add(&mut out, d);
            }
        }
        for d in pair {
            if !weekday(d) {
                let mut x = d;
                while !weekday(x) || out.contains(&x) {
                    x += 1.0;
                }
                add(&mut out, x);
            }
        }
    }
    for d in [md(yf, 2.0, 6.0), md(yf, 4.0, 25.0)] {
        if weekday(d) || y < 2014 {
            add(&mut out, d);
        } else {
            let mut x = d;
            while dow(x) != 1.0 {
                x += 1.0;
            }
            add(&mut out, x);
        }
    }
    let e = easter(yf);
    add(&mut out, e - 2.0);
    add(&mut out, e + 1.0);
    add(&mut out, nth_monday(yf, 5.0, 1.0)); // King's Birthday: first Monday in June
    add(&mut out, nth_monday(yf, 9.0, 4.0)); // Labour Day: fourth Monday in October
    if let Some((_, d)) = MATARIKI_DATES.iter().find(|(yy, _)| *yy as i64 == y) {
        let m: f64 = d[..2].parse().unwrap_or(f64::NAN);
        let dd: f64 = d[3..].parse().unwrap_or(f64::NAN);
        add(&mut out, md(yf, m, dd));
    }
    // Auckland Anniversary: the Monday nearest 29 January
    if Regex::lit("auckland", "i").test(region) {
        let j29 = md(yf, 1.0, 29.0);
        let back = (dow(j29) + 6.0) % 7.0;
        add(
            &mut out,
            if back <= 3.0 {
                j29 - back
            } else {
                j29 - back + 7.0
            },
        );
    }
    out
}

/// `inTerm(n)`: whether a day is inside a school term, or None when the term dates of its year are not entered.
pub fn in_term(n: f64) -> Option<bool> {
    let y: i64 = diso(n)[..4].parse().ok()?;
    let t = term_dates(y)?;
    Some(t.iter().any(|(s, e)| n >= dnum(s) && n <= dnum(e)))
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut s = xs.to_vec();
    js_sort_by(&mut s, |p, q| p - q);
    let m = s.len() >> 1;
    Some(if s.len() % 2 == 1 {
        s[m]
    } else {
        (s[m - 1] + s[m]) / 2.0
    })
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().fold(0.0, |s, x| s + x) / xs.len() as f64)
    }
}

fn sdev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs)?;
    Some((xs.iter().fold(0.0, |s, x| s + (x - m) * (x - m)) / (xs.len() as f64 - 1.0)).sqrt())
}

/// The fit `ols(pts)` gives: slope, intercept and the mean of y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ols {
    /// Slope.
    pub slope: f64,
    /// Intercept.
    pub intercept: f64,
    /// Mean of y.
    pub mean: f64,
}

/// `ols(pts)`: the least squares line through [x, y] points; None with fewer than two points or no spread in x.
pub fn ols(pts: &[[f64; 2]]) -> Option<Ols> {
    if pts.len() < 2 {
        return None;
    }
    let xs: Vec<f64> = pts.iter().map(|p| p[0]).collect();
    let ys: Vec<f64> = pts.iter().map(|p| p[1]).collect();
    let (mx, my) = (mean(&xs)?, mean(&ys)?);
    let sxx = pts.iter().fold(0.0, |s, p| s + (p[0] - mx) * (p[0] - mx));
    if sxx == 0.0 || sxx.is_nan() {
        return None;
    }
    let b = pts.iter().fold(0.0, |s, p| s + (p[0] - mx) * (p[1] - my)) / sxx;
    Some(Ols {
        slope: b,
        intercept: my - b * mx,
        mean: my,
    })
}

/// The Tuesday to Thursday school-term mean of a site (the calibration target).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TmsTarget {
    /// Mean daily volume.
    pub mean: f64,
    /// Standard deviation (null with one day).
    pub sd: Option<f64>,
    /// Days used.
    pub n: usize,
    /// 95% confidence interval of the mean (null with one day).
    pub ci95: Option<f64>,
    /// School-term weeks the days fall in.
    pub weeks: usize,
    /// First day used.
    pub from: String,
    /// Last day used.
    pub to: String,
    /// Enough weeks of data and every year's term dates entered.
    pub ok: bool,
    /// Why the mean is indicative ("" when it is not).
    pub note: String,
}

/// Mean weekday volume, when there are too few days for an AAWDT.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TmsWeekday {
    /// Mean volume of the weekdays.
    pub mean: f64,
    /// Weekdays counted.
    pub n: usize,
}

/// Growth of a site's annual traffic (the slope of its five years of AADT).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TmsGrowth {
    /// Vehicles a day per year.
    pub slope: f64,
    /// The slope as a percentage of the mean (null when the mean is zero).
    pub pct: Option<f64>,
    /// Years of history used.
    pub years: usize,
    /// First year.
    pub from: f64,
    /// Last year.
    pub to: f64,
    /// What the figure is.
    pub basis: String,
}

/// The daily series: its first date and each day's volume (negative for a day left out of the statistics).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TmsSeries {
    /// The first day, YYYY-MM-DD.
    pub d0: String,
    /// Volumes in veh/day, one a day from the first day on (days with no data are not listed: this is the days that have data, in order).
    pub v: Vec<f64>,
}

/// What `tmsStats` gives for one site.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmsStats {
    /// The site reference.
    #[serde(rename = "ref")]
    pub reference: Value,
    /// First day with data.
    pub from: String,
    /// Last day with data.
    pub to: String,
    /// Days with data.
    pub days: usize,
    /// Days that passed the 50% of the median rule.
    pub valid: usize,
    /// Days dropped.
    pub dropped: usize,
    /// Mean daily volume of the valid days.
    pub adt: Option<f64>,
    /// Annual average daily traffic (needs 300 valid days in the last 12 months).
    pub aadt: Option<f64>,
    /// Annual average weekday traffic.
    pub aawdt: Option<f64>,
    /// Weekday mean when there are too few days for an AAWDT.
    pub wdt: Option<TmsWeekday>,
    /// The Tuesday to Thursday school-term mean.
    pub target: Option<TmsTarget>,
    /// Heavy share of vehicles.
    pub hcv: Option<f64>,
    /// Day-of-week factors, Sunday first.
    pub dow: Vec<Option<f64>>,
    /// Monthly factors against the AADT, by month number.
    pub madt: Option<BTreeMap<String, f64>>,
    /// Growth from the site's history.
    pub growth: Option<TmsGrowth>,
    /// Flow directions seen, sorted as text.
    pub dirs: Vec<Value>,
    /// The daily series.
    pub series: TmsSeries,
    /// "veh/day".
    pub units: String,
    /// "weekday_factored".
    pub method: String,
    /// When the statistics were made.
    pub at: String,
    /// The source's name.
    pub source: String,
}

#[derive(Default)]
struct Day {
    l: f64,
    h: f64,
    o: f64,
}

/// `tmsStats(rows, opts)`: the statistics of one site from deduplicated rows ({ SiteRef, startDate, laneNumber, flowDirection, classWeight, v }).
///
/// `hist` is the site's five years of AADT, oldest first; `region` decides the regional holidays (Auckland by default); `now_ms` is the date the growth is
/// counted to and `clock_ms` the time the statistics are stamped with (the same in the prototype's own use). Gives
/// None without data, and an error text for a start date that is not a date.
pub fn tms_stats(
    rows: &[JsonValue],
    reference: &Value,
    region: &str,
    hist: Option<&[JsonValue]>,
    now_ms: f64,
    clock_ms: f64,
) -> Result<Option<TmsStats>, String> {
    let heavy = Regex::lit("heavy", "i");
    let light = Regex::lit("light", "i");
    let mut day: HashMap<i64, Day> = HashMap::new();
    let mut dirs: Vec<JsonValue> = Vec::new();
    for r in rows {
        let ms = r
            .get("startDate")
            .map_or(None, date_of)
            .ok_or_else(|| "Invalid time value".to_string())?;
        let n = dnum(&today(ms)) as i64;
        let v = {
            let x = r.get("v").map_or(f64::NAN, JsonValue::to_number);
            if x == 0.0 || x.is_nan() { 0.0 } else { x }
        };
        let cw = match r.get("classWeight") {
            Some(c) if c.truthy() => c.js_string(),
            _ => String::new(),
        };
        let t = day.entry(n).or_default();
        // Light and Heavy add to the day; any other class (a total, say) is used only for a day without the split
        if heavy.test(&cw) {
            t.h += v;
        } else if light.test(&cw) {
            t.l += v;
        } else {
            t.o += v;
        }
        if let Some(fd) = r.get("flowDirection").filter(|d| !d.is_null()) {
            if !dirs.iter().any(|d| d.strict_eq(fd)) {
                dirs.push(fd.clone());
            }
        }
    }
    if day.is_empty() {
        return Ok(None);
    }
    let mut ds: Vec<f64> = day.keys().map(|k| *k as f64).collect();
    js_sort_by(&mut ds, |p, q| p - q);
    let tot = |n: f64| -> f64 {
        let t = &day[&(n as i64)];
        let s = t.l + t.h;
        if s != 0.0 && !s.is_nan() { s } else { t.o }
    };
    let (mut valid, mut dropped): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for &n in &ds {
        let win: Vec<f64> = ds
            .iter()
            .filter(|&&m| (m - n).abs() <= TMS_MEDIAN_DAYS / 2.0)
            .map(|&m| tot(m))
            .collect();
        let med = median(&win);
        let ok = match med {
            Some(m) => tot(n) >= TMS_MIN_SHARE * m && tot(n) > 0.0,
            None => false,
        };
        if ok { valid.push(n) } else { dropped.push(n) }
    }
    let mut years: Vec<i64> = Vec::new();
    for &n in &ds {
        let y: i64 = diso(n)[..4].parse().unwrap_or(0);
        if !years.contains(&y) {
            years.push(y);
        }
    }
    let mut hol: HashSet<i64> = HashSet::new();
    for y in &years {
        for d in holidays(*y, region) {
            hol.insert(d as i64);
        }
    }
    let vals = |ns: &[f64]| -> Vec<f64> { ns.iter().map(|&n| tot(n)).collect() };
    let last = ds[ds.len() - 1];
    let last_year: Vec<f64> = valid
        .iter()
        .copied()
        .filter(|&n| n > last - 365.0)
        .collect();
    let aadt = if last_year.len() as f64 >= TMS_AADT_DAYS {
        mean(&vals(&last_year))
    } else {
        None
    };
    // AAWDT is annual too: weekdays of the same last 12 months, with the same 300-day rule; otherwise only a weekday mean
    let is_wk = |n: f64| dow(n) >= 1.0 && dow(n) <= 5.0 && !hol.contains(&(n as i64));
    let wk: Vec<f64> = valid.iter().copied().filter(|&n| is_wk(n)).collect();
    let aawdt = if aadt.is_some() {
        mean(&vals(
            &last_year
                .iter()
                .copied()
                .filter(|&n| is_wk(n))
                .collect::<Vec<_>>(),
        ))
    } else {
        None
    };
    // Calibration target: Tue to Thu, not a holiday, in school term, latest 12 term weeks
    let mut tt: Vec<f64> = valid
        .iter()
        .copied()
        .filter(|&n| {
            dow(n) >= 2.0
                && dow(n) <= 4.0
                && !hol.contains(&(n as i64))
                && in_term(n) != Some(false)
        })
        .collect();
    js_sort_by(&mut tt, |p, q| q - p);
    let mut weeks: Vec<f64> = Vec::new();
    let mut used: Vec<f64> = Vec::new();
    for &n in &tt {
        let w = ((n + 3.0) / 7.0).floor();
        if !weeks.contains(&w) && weeks.len() as f64 >= TMS_TERM_WEEKS[1] {
            break;
        }
        if !weeks.contains(&w) {
            weeks.push(w);
        }
        used.push(n);
    }
    let tv = vals(&used);
    let tm = mean(&tv);
    let sd = sdev(&tv);
    let unknown_term = used.iter().any(|&n| in_term(n).is_none());
    let target = if !used.is_empty() {
        let nw = weeks.len();
        Some(TmsTarget {
            mean: tm.expect("a mean"),
            sd,
            n: used.len(),
            ci95: sd.map(|s| TMS_Z * s / (used.len() as f64).sqrt()),
            weeks: nw,
            from: diso(math_min_of(&used)),
            to: diso(math_max_of(&used)),
            ok: nw as f64 >= TMS_TERM_WEEKS[0] && !unknown_term,
            note: if (nw as f64) < TMS_TERM_WEEKS[0] {
                format!(
                    "Only {} school-term week{} of Tuesday\u{2013}Thursday data ({}\u{2013}{} wanted): indicative.",
                    nw,
                    if nw == 1 { "" } else { "s" },
                    crate::jsnum::number_to_js_string(TMS_TERM_WEEKS[0]),
                    crate::jsnum::number_to_js_string(TMS_TERM_WEEKS[1])
                )
            } else if unknown_term {
                "School term dates are not entered for every year used: indicative.".to_string()
            } else {
                String::new()
            },
        })
    } else {
        None
    };
    let l: f64 = valid.iter().fold(0.0, |s, n| s + day[&(*n as i64)].l);
    let h: f64 = valid.iter().fold(0.0, |s, n| s + day[&(*n as i64)].h);
    let all = mean(&vals(&valid));
    let dow_f: Vec<Option<f64>> = (0..7)
        .map(|k| {
            let x = vals(
                &valid
                    .iter()
                    .copied()
                    .filter(|&n| dow(n) == k as f64)
                    .collect::<Vec<_>>(),
            );
            match all {
                Some(a) if !x.is_empty() && a != 0.0 && !a.is_nan() => {
                    Some(mean(&x).expect("a mean") / a)
                }
                _ => None,
            }
        })
        .collect();
    let madt = match aadt {
        Some(a) if a != 0.0 && !a.is_nan() => {
            let mut by: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
            for &n in &last_year {
                let m: u32 = diso(n)[5..7].parse().unwrap_or(0);
                by.entry(m).or_default().push(tot(n));
            }
            Some(
                by.into_iter()
                    .map(|(m, v)| (m.to_string(), mean(&v).expect("a mean") / a))
                    .collect::<BTreeMap<String, f64>>(),
            )
        }
        _ => None,
    };
    // Growth: OLS slope of annual ADT (the NZTA site's five years of AADT, oldest first)
    let mut growth = None;
    if let Some(hist) = hist
        && hist.iter().any(JsonValue::truthy)
    {
        let y = nz_year(now_ms) as f64;
        let pts: Vec<[f64; 2]> = hist
            .iter()
            .enumerate()
            .filter(|(_, v)| v.truthy())
            .map(|(i, v)| [y - 5.0 + i as f64, v.to_number()])
            .collect();
        if let Some(f) = ols(&pts) {
            growth = Some(TmsGrowth {
                slope: f.slope,
                pct: if f.mean != 0.0 && !f.mean.is_nan() {
                    Some(f.slope / f.mean * 100.0)
                } else {
                    None
                },
                years: pts.len(),
                from: pts[0][0],
                to: pts[pts.len() - 1][0],
                basis: "OLS slope of annual ADT (NZTA site history)".to_string(),
            });
        }
    }
    let d0 = ds[0];
    let ok: HashSet<i64> = valid.iter().map(|n| *n as i64).collect();
    let mut sorted_dirs = dirs;
    js_sort_by(&mut sorted_dirs, |a, b| {
        let (x, y): (Vec<u16>, Vec<u16>) = (
            a.js_string().encode_utf16().collect(),
            b.js_string().encode_utf16().collect(),
        );
        match x.cmp(&y) {
            std::cmp::Ordering::Less => -1.0,
            std::cmp::Ordering::Equal => 0.0,
            std::cmp::Ordering::Greater => 1.0,
        }
    });
    Ok(Some(TmsStats {
        reference: reference.clone(),
        from: diso(d0),
        to: diso(last),
        days: ds.len(),
        valid: valid.len(),
        dropped: dropped.len(),
        adt: all,
        aadt,
        aawdt,
        wdt: if !wk.is_empty() {
            Some(TmsWeekday {
                mean: mean(&vals(&wk)).expect("a mean"),
                n: wk.len(),
            })
        } else {
            None
        },
        target,
        hcv: if l + h != 0.0 {
            Some(h / (l + h))
        } else {
            None
        },
        dow: dow_f,
        madt,
        growth,
        dirs: sorted_dirs.iter().map(JsonValue::to_value).collect(),
        series: TmsSeries {
            d0: diso(d0),
            v: ds
                .iter()
                .map(|&n| (if ok.contains(&(n as i64)) { 1.0 } else { -1.0 }) * math_round(tot(n)))
                .collect(),
        },
        units: "veh/day".to_string(),
        method: "weekday_factored".to_string(),
        at: iso_string(clock_ms),
        source: SRC_TMS.name.to_string(),
    }))
}

/// A count site TMS data can be fetched for: its reference and the five years of AADT the NZTA site list gave.
#[derive(Clone, Debug, PartialEq)]
pub struct TmsSite {
    /// The site reference (8 letters and digits).
    pub reference: String,
    /// Five years of AADT, oldest first.
    pub hist: Option<Vec<JsonValue>>,
}

/// `tmsSites(counts)`: the project's NZTA count sites that are real sites (not virtual) with an 8-character reference.
pub fn tms_sites(p: &Project) -> Vec<TmsSite> {
    let re = Regex::lit("^[0-9A-Z]{8}$", "i");
    let Some(Value::Array(counts)) = p.data.get().and_then(|d| d.extra.get("counts")) else {
        return Vec::new();
    };
    counts
        .iter()
        .filter(|c| {
            c.get("src").and_then(Value::as_str) == Some("nzta")
                && !c.get("virtual").is_some_and(value_truthy)
        })
        .filter_map(|c| {
            let r = c
                .get("ref")
                .filter(|r| value_truthy(r))
                .map(crate::jsnum::value_to_js_string)
                .unwrap_or_default();
            if !re.test(&r) {
                return None;
            }
            let hist = match c.get("hist") {
                Some(Value::Array(a)) => Some(a.iter().map(JsonValue::from_value).collect()),
                _ => None,
            };
            Some(TmsSite { reference: r, hist })
        })
        .collect()
}

/// What `loadTMS` reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TmsResult {
    /// Sites with statistics.
    pub sites: usize,
    /// Sites asked for.
    #[serde(rename = "of")]
    pub of_: usize,
    /// Rows used.
    pub rows: usize,
    /// The first day asked for.
    pub since: String,
    /// Every page of every query came.
    pub complete: bool,
    /// The statistics by site reference.
    pub stats: BTreeMap<String, TmsStats>,
}

/// `loadTMS(counts, opts)`: the download of daily counts, 8 sites a query: each site's rows since a date 400 days back (or the days asked for).
pub struct TmsLoad {
    sites: Vec<TmsSite>,
    refs: Vec<String>,
    since: String,
    chunk: usize,
    query: Option<ArcQuery>,
    rows: HashMap<String, Vec<JsonValue>>,
    n: usize,
    complete: bool,
    timeout_ms: f64,
    now_ms: f64,
    statuses: Vec<String>,
}

impl TmsLoad {
    /// A download for these sites. `days` is the "days to download" setting (a cleared or text value gives 400).
    pub fn new(sites: Vec<TmsSite>, days: &Value, now_ms: f64, timeout_ms: f64) -> TmsLoad {
        let d = math_round(value_to_number(days));
        let days = if d > 0.0 { d } else { 400.0 };
        let mut refs: Vec<String> = Vec::new();
        for s in &sites {
            if !refs.contains(&s.reference) {
                refs.push(s.reference.clone());
            }
        }
        let since = diso(dnum(&today(now_ms)) - days);
        let rows = refs.iter().map(|r| (r.clone(), Vec::new())).collect();
        let mut l = TmsLoad {
            sites,
            refs,
            since,
            chunk: 0,
            query: None,
            rows,
            n: 0,
            complete: true,
            timeout_ms,
            now_ms,
            statuses: Vec::new(),
        };
        l.start_chunk();
        l
    }
    fn start_chunk(&mut self) {
        let i = self.chunk * TMS_CHUNK;
        if i >= self.refs.len() {
            self.query = None;
            return;
        }
        let part = &self.refs[i..(i + TMS_CHUNK).min(self.refs.len())];
        self.statuses.push(format!(
            "Downloading daily counts for sites {}\u{2013}{} of {} (NZTA TMS)\u{2026}",
            i + 1,
            i + part.len(),
            self.refs.len()
        ));
        let list = part
            .iter()
            .map(|s| format!("'{}'", s.replace('\'', "")))
            .collect::<Vec<_>>()
            .join(",");
        let params = vec![
            ("where".to_string(), format!("SiteRef IN ({}) AND startDate >= DATE '{}'", list, self.since)),
            ("groupByFieldsForStatistics".to_string(), "SiteRef,startDate,laneNumber,flowDirection,classWeight".to_string()),
            (
                "outStatistics".to_string(),
                "[{\"statisticType\":\"max\",\"onStatisticField\":\"trafficCount\",\"outStatisticFieldName\":\"v\"}]".to_string(),
            ),
            ("orderByFields".to_string(), "SiteRef,startDate,laneNumber,flowDirection,classWeight".to_string()),
            ("resultRecordCount".to_string(), "2000".to_string()),
        ];
        self.query = Some(ArcQuery::new(
            SRC_TMS.url,
            params,
            TMS_MAX_PAGES,
            self.timeout_ms,
        ));
    }
    /// The progress lines the prototype passes to its status callback, in order.
    pub fn statuses(&self) -> &[String] {
        &self.statuses
    }
    /// After the download: works out each site's statistics, puts them in `P.feeds.tms` and on the NZTA counts, and stamps the vintage table.
    pub fn finish(self, p: &mut Project) -> Result<TmsResult, String> {
        let mut out: BTreeMap<String, TmsStats> = BTreeMap::new();
        for s in &self.sites {
            let empty: Vec<JsonValue> = Vec::new();
            let rows = self.rows.get(&s.reference).unwrap_or(&empty);
            if let Some(st) = tms_stats(
                rows,
                &json!(s.reference),
                "Auckland",
                s.hist.as_deref(),
                self.now_ms,
                self.now_ms,
            )? {
                out.insert(s.reference.clone(), st);
            }
        }
        let f = feeds_of(p);
        if !f.get("tms").is_some_and(value_truthy) {
            f.insert("tms".to_string(), json!({}));
        }
        if let Some(Value::Object(t)) = f.get_mut("tms") {
            for (k, v) in &out {
                t.insert(k.clone(), serde_json::to_value(v).expect("stats"));
            }
        }
        attach_tms(p);
        let mut extra = Map::new();
        extra.insert("n".into(), json!(out.len()));
        extra.insert(
            "note".into(),
            json!(format!(
                "{} deduplicated rows (groupBy and max) since {}. {}.{}",
                fmt::int(self.n as f64),
                self.since,
                TMS_HCV_RULE,
                if self.complete {
                    ""
                } else {
                    " Some sites hit the download cap: shorten the period in Settings."
                }
            )),
        );
        stamp(p, "tms", &extra, self.now_ms);
        Ok(TmsResult {
            sites: out.len(),
            of_: self.refs.len(),
            rows: self.n,
            since: self.since,
            complete: self.complete,
            stats: out,
        })
    }
}

impl Flow for TmsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        self.query.as_mut().and_then(Flow::next_request)
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        let q = self.query.as_mut().expect("a query is running");
        q.answer(a)?;
        if q.is_done() {
            let q = self.query.take().expect("query");
            self.complete = self.complete && q.complete();
            for f in q.into_features() {
                let empty = JsonValue::object();
                let a = f.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
                let r = ["SiteRef", "siteref", "SITEREF"]
                    .iter()
                    .find_map(|k| a.get(k).filter(|v| !v.is_null()));
                if let Some(r) = r
                    && let Some(list) = self.rows.get_mut(&r.js_string())
                {
                    list.push(a.clone());
                    self.n += 1;
                }
            }
            self.chunk += 1;
            self.start_chunk();
        }
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.query.is_none()
    }
}

/// `periodUsesTMS()`: whether the model turns a count's TMS weekday mean into its peak volume (true for this port).
pub fn period_uses_tms(s: &Settings) -> bool {
    let mut a = Count::new("a", "nzta");
    a.adt = Maybe::number(1000.0);
    a.tms = Maybe::Set(json!({ "target": { "mean": 2000, "n": 30, "ok": true } }));
    let mut b = Count::new("b", "nzta");
    b.adt = Maybe::number(2000.0);
    match (period_vol(&a, "AM", s), period_vol(&b, "AM", s)) {
        (Some(a), Some(b)) => b.v > 0.0 && (a.v / b.v - 1.0).abs() < 1e-9,
        _ => false,
    }
}

/// `periodUsesShare()`: whether the model reads a site's own peak shares (`share`) (true for this port).
pub fn period_uses_share(s: &Settings) -> bool {
    let mut a = Count::new("a", "nzta");
    a.adt = Maybe::number(1000.0);
    a.share = Maybe::Set(json!({ "AM": 0.0123, "PM": 0.0123 }));
    period_vol(&a, "AM", s).is_some_and(|a| (a.v - 12.3).abs() < 1e-9)
}

/// `attachTMS()`: puts TMS results (and peak profiles) on the NZTA counts; gives how many counts got results.
///
/// A count whose TMS mean the model uses gets `obs_method` "weekday_factored" and `n_days`; one it does not use (an indicative mean) keeps the AADT basis.
/// The prototype counts a count as changed when its `tms` is not the same object as the result; a project read from a file never holds the same object, so every count with a result counts.
pub fn attach_tms(p: &mut Project) -> usize {
    let feeds = p.extra.get("feeds").cloned();
    let t = feeds
        .as_ref()
        .and_then(|f| f.get("tms"))
        .filter(|t| value_truthy(t))
        .cloned();
    let prof = feeds
        .as_ref()
        .and_then(|f| f.get("profiles"))
        .filter(|t| value_truthy(t))
        .cloned();
    if t.is_none() && prof.is_none() {
        return 0;
    }
    let weekday = t.is_some() && period_uses_tms(&p.settings);
    let settings = p.settings.clone();
    let Some(d) = p.data.get_mut() else { return 0 };
    let Some(Value::Array(counts)) = d.extra.get_mut("counts") else {
        return 0;
    };
    let mut n = 0;
    for c in counts.iter_mut() {
        let Some(o) = c.as_object_mut() else { continue };
        if o.get("src").and_then(Value::as_str) != Some("nzta") {
            continue;
        }
        let key = match o.get("ref") {
            Some(r) => crate::jsnum::value_to_js_string(r),
            None => "undefined".to_string(),
        };
        let s = t
            .as_ref()
            .and_then(|t| t.get(&key))
            .filter(|s| value_truthy(s))
            .cloned();
        // Older Spare Construct versions wrote c.obsMethod 'weekday_factored' although the volume came from the AADT
        o.remove("obsMethod");
        if let Some(s) = &s {
            o.insert("tms".to_string(), s.clone());
            n += 1;
        }
        if let Some(s) = &s
            && weekday
            && s.get("target").is_some_and(|t| !t.is_null())
        {
            let uses = match serde_json::from_value::<Count>(Value::Object(o.clone())) {
                Ok(cc) => {
                    period_vol(&cc, "AM", &settings).is_some_and(|r| r.basis == "weekday_factored")
                }
                Err(_) => false,
            };
            if uses {
                o.insert("obs_method".to_string(), json!("weekday_factored"));
                o.insert(
                    "n_days".to_string(),
                    s["target"].get("n").cloned().unwrap_or(Value::Null),
                );
            } else if o.get("obs_method").and_then(Value::as_str) == Some("weekday_factored") {
                o.remove("obs_method");
                o.remove("n_days");
            }
        }
        let site = prof
            .as_ref()
            .and_then(|pr| pr.get("sites"))
            .and_then(|st| st.get(&key))
            .filter(|x| value_truthy(x));
        if let (Some(pr), Some(site)) = (prof.as_ref(), site) {
            let src_text = pr.get("source").cloned().unwrap_or(Value::Null);
            let same = o
                .get("share")
                .filter(|s| value_truthy(s))
                .is_some_and(|s| s.get("src") == Some(&src_text) && !src_text.is_null());
            if !same {
                o.insert(
                    "share".to_string(),
                    json!({ "AM": site.get("amShare").cloned().unwrap_or(Value::Null), "PM": site.get("pmShare").cloned().unwrap_or(Value::Null),
                        "amStart": site.get("amStart").cloned().unwrap_or(Value::Null), "pmStart": site.get("pmStart").cloned().unwrap_or(Value::Null), "src": src_text }),
                );
            }
        }
    }
    n
}

/// `loadProfiles()`: the request for the peak profiles file (data/tms_profiles.json on the host's own site).
pub fn profiles_request() -> GetOnce {
    GetOnce::new(HttpRequest::get_json(SRC_PROFILES.url.to_string(), 8000.0))
}

/// After `loadProfiles`: keeps the profiles in `P.feeds.profiles`, attaches them to the counts and stamps the vintage table. Gives how many counts changed.
pub fn apply_profiles(p: &mut Project, answer: &JsonValue, now_ms: f64) -> Result<usize, String> {
    let sites = answer.get("sites").filter(|s| s.truthy());
    let Some(sites) = (answer.truthy()).then_some(sites).flatten() else {
        return Err("data/tms_profiles.json has no sites".to_string());
    };
    let source = match answer.get("source") {
        Some(s) if s.truthy() => s.to_value(),
        _ => json!(SRC_PROFILES.name),
    };
    let built = match answer.get("built") {
        Some(b) if b.truthy() => b.to_value(),
        _ => Value::Null,
    };
    let n_sites = match sites {
        JsonValue::Object(o) => o.len(),
        JsonValue::Array(a) => a.len(),
        _ => 0,
    };
    feeds_of(p).insert(
        "profiles".to_string(),
        json!({ "source": source, "built": built, "sites": sites.to_value() }),
    );
    let n = attach_tms(p);
    let mut extra = Map::new();
    extra.insert("n".into(), json!(n_sites));
    extra.insert(
        "note".into(),
        json!(match answer.get("built") {
            Some(b) if b.truthy() => format!("Built {}.", b.js_string()),
            _ => String::new(),
        }),
    );
    stamp(p, "profiles", &extra, now_ms);
    Ok(n)
}
