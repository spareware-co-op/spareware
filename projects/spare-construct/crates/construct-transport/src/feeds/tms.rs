//! NZTA TMS daily counts (section 3 of feeds.js): download, clean up and summarise daily traffic at state highway telemetry sites.
//!
//! Duplicate rows are removed by the server (group by and max). Days under half of the 28-day median are dropped (a
//! partial or faulty day). Per site: the Tuesday to Thursday school-term mean with a 95% confidence interval (the
//! calibration target), AADT and AAWDT from the last 12 months, %HCV, day-of-week and monthly factors, and growth.
//! The statistics are put on the NZTA counts (`tms`), and the peak profiles (shares of daily traffic in the AM and PM peaks) too.

use super::arcgis::{FetchFailure, HttpRequest};
use super::flow::{Answer, ArcQuery, Flow, GetOnce};
use super::jsregex::Regex;
use super::nztime::{date_of, date_utc, nz_year, today, DAY_MS};
use super::sources::{term_dates, MATARIKI_DATES, SRC_PROFILES, SRC_TMS, TMS_AADT_DAYS, TMS_CHUNK, TMS_HCV_RULE, TMS_MAX_PAGES, TMS_MEDIAN_DAYS, TMS_MIN_SHARE, TMS_TERM_WEEKS, TMS_Z};
use super::vintage::{feeds_of, stamp};
use crate::datahub::{period_vol, Count};
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{f64_to_i64, i64_to_f64, math_max_of, math_min_of, math_round, usize_to_f64, value_to_number, value_truthy, Maybe};
use crate::project::{Project, Settings};
use crate::util::fmt;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// `dnum(iso)`: days since 1970-01-01 of a date written YYYY-MM-DD.
#[must_use]
pub fn dnum(iso: &str) -> f64 {
    math_round(super::nztime::parse_date(&format!("{iso}T00:00:00Z")).unwrap_or(f64::NAN) / DAY_MS)
}

/// `diso(n)`: the date (YYYY-MM-DD) of a day count since 1970-01-01.
#[must_use]
pub fn diso(n: f64) -> String {
    super::nztime::utc_date(n * DAY_MS)
}

/// `dow(n)`: the weekday of a day count, 0 = Sunday.
#[must_use]
pub fn dow(n: f64) -> f64 {
    (n + 4.0) % 7.0
}

fn md(y: f64, m: f64, d: f64) -> f64 {
    math_round(date_utc(y, m - 1.0, d) / DAY_MS)
}

/// `easter(y)`: Easter Sunday of a year as a day count (the Gregorian computus).
#[must_use]
pub fn easter(y: f64) -> f64 {
    // The anonymous Gregorian algorithm (Meeus, Jones, Butcher); its letters are in brackets
    let golden = y % 19.0; // (a)
    let century = (y / 100.0).floor(); // (b)
    let of_century = y % 100.0; // (c)
    let leap_centuries = (century / 4.0).floor(); // (d)
    let century_rem = century % 4.0; // (e)
    let lunar = ((century + 8.0) / 25.0).floor(); // (f)
    let moon = ((century - lunar + 1.0) / 3.0).floor(); // (g)
    let epact = (19.0 * golden + century - leap_centuries - moon + 15.0) % 30.0; // (h)
    let leap_years = (of_century / 4.0).floor(); // (i)
    let year_rem = of_century % 4.0; // (k)
    let week = (32.0 + 2.0 * century_rem + 2.0 * leap_years - epact - year_rem) % 7.0; // (l)
    let shift = ((golden + 11.0 * epact + 22.0 * week) / 451.0).floor(); // (m)
    let mo = ((epact + week - 7.0 * shift + 114.0) / 31.0).floor();
    let da = ((epact + week - 7.0 * shift + 114.0) % 31.0) + 1.0;
    math_round(date_utc(y, mo - 1.0, da) / DAY_MS)
}

/// `nthMonday(y, m, nth)`: the nth Monday of a month (m counted from 0) as a day count.
#[must_use]
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
#[must_use]
pub fn holidays(y: i64, region: &str) -> Vec<f64> {
    let yf = i64_to_f64(y);
    let mut out: Vec<f64> = Vec::new();
    let add = |out: &mut Vec<f64>, d: f64| {
        if !out.contains(&d) {
            out.push(d);
        }
    };
    let weekday = |d: f64| dow(d) != 0.0 && dow(d) != 6.0;
    // New Year and Christmas pairs move to the next free weekdays
    for pair in [[md(yf, 1.0, 1.0), md(yf, 1.0, 2.0)], [md(yf, 12.0, 25.0), md(yf, 12.0, 26.0)]] {
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
    if let Some((_, d)) = MATARIKI_DATES.iter().find(|(yy, _)| i64::from(*yy) == y) {
        let m: f64 = d[..2].parse().unwrap_or(f64::NAN);
        let dd: f64 = d[3..].parse().unwrap_or(f64::NAN);
        add(&mut out, md(yf, m, dd));
    }
    // Auckland Anniversary: the Monday nearest 29 January
    if Regex::lit("auckland", "i").test(region) {
        let j29 = md(yf, 1.0, 29.0);
        let back = (dow(j29) + 6.0) % 7.0;
        add(&mut out, if back <= 3.0 { j29 - back } else { j29 - back + 7.0 });
    }
    out
}

/// `inTerm(n)`: whether a day is inside a school term, or None when the term dates of its year are not entered.
#[must_use]
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
    // s is not empty, so m is a position in it, and m - 1 is too when the length is even
    let hi = s.get(m).copied()?;
    Some(if s.len() % 2 == 1 { hi } else { f64::midpoint(m.checked_sub(1).and_then(|i| s.get(i)).copied()?, hi) })
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() { None } else { Some(xs.iter().fold(0.0, |s, x| s + x) / usize_to_f64(xs.len())) }
}

fn sdev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs)?;
    Some((xs.iter().fold(0.0, |s, x| s + (x - m) * (x - m)) / (usize_to_f64(xs.len()) - 1.0)).sqrt())
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
#[must_use]
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
    Some(Ols { slope: b, intercept: my - b * mx, mean: my })
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
    /// "`weekday_factored`".
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

// The day totals of the rows by day count, and the flow directions seen in the order met; an error text for a start date that is not a date
fn day_totals(rows: &[JsonValue]) -> Result<(HashMap<i64, Day>, Vec<JsonValue>), String> {
    let heavy = Regex::lit("heavy", "i");
    let light = Regex::lit("light", "i");
    let mut day: HashMap<i64, Day> = HashMap::new();
    let mut dirs: Vec<JsonValue> = Vec::new();
    for r in rows {
        let ms = r.get("startDate").and_then(date_of).ok_or_else(|| "Invalid time value".to_string())?;
        let n = f64_to_i64(dnum(&today(ms)));
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
        if let Some(fd) = r.get("flowDirection").filter(|d| !d.is_null())
            && !dirs.iter().any(|d| d.strict_eq(fd)) {
                dirs.push(fd.clone());
            }
    }
    Ok((day, dirs))
}

// tot(n): a day's Light plus Heavy, else its other classes. Every day asked for is a key of day; another would read as NaN
fn day_total(day: &HashMap<i64, Day>, n: f64) -> f64 {
    let Some(t) = day.get(&f64_to_i64(n)) else { return f64::NAN };
    let s = t.l + t.h;
    if s != 0.0 && !s.is_nan() { s } else { t.o }
}

// Calibration target: Tue to Thu, not a holiday, in school term, latest 12 term weeks; None without such days
fn school_target(valid: &[f64], hol: &HashSet<i64>, day: &HashMap<i64, Day>) -> Option<TmsTarget> {
    let mut tt: Vec<f64> = valid.iter().copied().filter(|&n| dow(n) >= 2.0 && dow(n) <= 4.0 && !hol.contains(&f64_to_i64(n)) && in_term(n) != Some(false)).collect();
    js_sort_by(&mut tt, |p, q| q - p);
    let mut weeks: Vec<f64> = Vec::new();
    let mut used: Vec<f64> = Vec::new();
    for &n in &tt {
        let w = ((n + 3.0) / 7.0).floor();
        if !weeks.contains(&w) && usize_to_f64(weeks.len()) >= TMS_TERM_WEEKS[1] {
            break;
        }
        if !weeks.contains(&w) {
            weeks.push(w);
        }
        used.push(n);
    }
    let tv: Vec<f64> = used.iter().map(|&n| day_total(day, n)).collect();
    // the mean is None exactly when no day is used
    let tm = mean(&tv)?;
    let sd = sdev(&tv);
    let unknown_term = used.iter().any(|&n| in_term(n).is_none());
    let nw = weeks.len();
    Some(TmsTarget {
        mean: tm,
        sd,
        n: used.len(),
        ci95: sd.map(|s| TMS_Z * s / usize_to_f64(used.len()).sqrt()),
        weeks: nw,
        from: diso(math_min_of(&used)),
        to: diso(math_max_of(&used)),
        ok: usize_to_f64(nw) >= TMS_TERM_WEEKS[0] && !unknown_term,
        note: if usize_to_f64(nw) < TMS_TERM_WEEKS[0] {
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
}

// Day-of-week factors, Sunday first: each weekday's mean against the mean of every valid day
fn dow_factors(valid: &[f64], all: Option<f64>, day: &HashMap<i64, Day>) -> Vec<Option<f64>> {
    (0..7)
        .map(|k| {
            let x: Vec<f64> = valid.iter().copied().filter(|&n| dow(n) == f64::from(k)).map(|n| day_total(day, n)).collect();
            // mean(&x) is None exactly when x is empty
            match (all, mean(&x)) {
                (Some(a), Some(mx)) if a != 0.0 && !a.is_nan() => Some(mx / a),
                _ => None,
            }
        })
        .collect()
}

// Monthly factors against the AADT, by month number
fn monthly(last_year: &[f64], aadt: Option<f64>, day: &HashMap<i64, Day>) -> Option<BTreeMap<String, f64>> {
    let a = aadt.filter(|a| *a != 0.0 && !a.is_nan())?;
    let mut by: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    for &n in last_year {
        let m: u32 = diso(n)[5..7].parse().unwrap_or(0);
        by.entry(m).or_default().push(day_total(day, n));
    }
    // every month listed holds a day, so each has a mean
    Some(by.into_iter().filter_map(|(m, v)| mean(&v).map(|mv| (m.to_string(), mv / a))).collect::<BTreeMap<String, f64>>())
}

// Growth: OLS slope of annual ADT (the NZTA site's five years of AADT, oldest first)
fn growth_of(hist: Option<&[JsonValue]>, now_ms: f64) -> Option<TmsGrowth> {
    let hist = hist.filter(|h| h.iter().any(JsonValue::truthy))?;
    let y = i64_to_f64(nz_year(now_ms));
    let pts: Vec<[f64; 2]> = hist.iter().enumerate().filter(|(_, v)| v.truthy()).map(|(i, v)| [y - 5.0 + usize_to_f64(i), v.to_number()]).collect();
    let f = ols(&pts)?;
    Some(TmsGrowth {
        slope: f.slope,
        pct: if f.mean != 0.0 && !f.mean.is_nan() { Some(f.slope / f.mean * 100.0) } else { None },
        years: pts.len(),
        // ols needs two points, so there is a first and a last
        from: pts.first().map_or(f64::NAN, |p| p[0]),
        to: pts.last().map_or(f64::NAN, |p| p[0]),
        basis: "OLS slope of annual ADT (NZTA site history)".to_string(),
    })
}

// The flow directions sorted as text (by UTF-16 code units, as the prototype's sort compares)
fn sort_dirs(mut dirs: Vec<JsonValue>) -> Vec<Value> {
    js_sort_by(&mut dirs, |a, b| {
        let (x, y): (Vec<u16>, Vec<u16>) = (a.js_string().encode_utf16().collect(), b.js_string().encode_utf16().collect());
        match x.cmp(&y) {
            std::cmp::Ordering::Less => -1.0,
            std::cmp::Ordering::Equal => 0.0,
            std::cmp::Ordering::Greater => 1.0,
        }
    });
    dirs.iter().map(JsonValue::to_value).collect()
}

/// `tmsStats(rows, opts)`: the statistics of one site from deduplicated rows ({ `SiteRef`, startDate, laneNumber, flowDirection, classWeight, v }).
///
/// `hist` is the site's five years of AADT, oldest first; `region` decides the regional holidays (Auckland by default); `now_ms` is the date the growth is
/// counted to and `clock_ms` the time the statistics are stamped with (the same in the prototype's own use). Gives
/// None without data, and an error text for a start date that is not a date.
///
/// # Errors
///
/// "Invalid time value" when a row's start date is not a date.
pub fn tms_stats(rows: &[JsonValue], reference: &Value, region: &str, hist: Option<&[JsonValue]>, now_ms: f64, clock_ms: f64) -> Result<Option<TmsStats>, String> {
    let (day, dirs) = day_totals(rows)?;
    if day.is_empty() {
        return Ok(None);
    }
    let mut ds: Vec<f64> = day.keys().map(|k| i64_to_f64(*k)).collect();
    js_sort_by(&mut ds, |p, q| p - q);
    // day is not empty, so neither is ds
    let (Some(&d0), Some(&last)) = (ds.first(), ds.last()) else { return Ok(None) };
    let tot = |n: f64| -> f64 { day_total(&day, n) };
    let (mut valid, mut dropped): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for &n in &ds {
        let win: Vec<f64> = ds.iter().filter(|&&m| (m - n).abs() <= TMS_MEDIAN_DAYS / 2.0).map(|&m| tot(m)).collect();
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
            hol.insert(f64_to_i64(d));
        }
    }
    let vals = |ns: &[f64]| -> Vec<f64> { ns.iter().map(|&n| tot(n)).collect() };
    let last_year: Vec<f64> = valid.iter().copied().filter(|&n| n > last - 365.0).collect();
    let aadt = if usize_to_f64(last_year.len()) >= TMS_AADT_DAYS { mean(&vals(&last_year)) } else { None };
    // AAWDT is annual too: weekdays of the same last 12 months, with the same 300-day rule; otherwise only a weekday mean
    let is_wk = |n: f64| dow(n) >= 1.0 && dow(n) <= 5.0 && !hol.contains(&f64_to_i64(n));
    let wk: Vec<f64> = valid.iter().copied().filter(|&n| is_wk(n)).collect();
    let annual_weekday = if aadt.is_some() { mean(&vals(&last_year.iter().copied().filter(|&n| is_wk(n)).collect::<Vec<_>>())) } else { None };
    let target = school_target(&valid, &hol, &day);
    // valid days are keys of day; another would add nothing
    let l: f64 = valid.iter().fold(0.0, |s, n| s + day.get(&f64_to_i64(*n)).map_or(0.0, |t| t.l));
    let h: f64 = valid.iter().fold(0.0, |s, n| s + day.get(&f64_to_i64(*n)).map_or(0.0, |t| t.h));
    let all = mean(&vals(&valid));
    let dow_f = dow_factors(&valid, all, &day);
    let madt = monthly(&last_year, aadt, &day);
    let growth = growth_of(hist, now_ms);
    let ok: HashSet<i64> = valid.iter().map(|n| f64_to_i64(*n)).collect();
    Ok(Some(TmsStats {
        reference: reference.clone(),
        from: diso(d0),
        to: diso(last),
        days: ds.len(),
        valid: valid.len(),
        dropped: dropped.len(),
        adt: all,
        aadt,
        aawdt: annual_weekday,
        // mean is None exactly when there are no weekdays
        wdt: mean(&vals(&wk)).map(|m| TmsWeekday { mean: m, n: wk.len() }),
        target,
        hcv: if l + h == 0.0 { None } else { Some(h / (l + h)) },
        dow: dow_f,
        madt,
        growth,
        dirs: sort_dirs(dirs),
        series: TmsSeries { d0: diso(d0), v: ds.iter().map(|&n| (if ok.contains(&f64_to_i64(n)) { 1.0 } else { -1.0 }) * math_round(tot(n))).collect() },
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
#[must_use]
pub fn tms_sites(p: &Project) -> Vec<TmsSite> {
    let re = Regex::lit("^[0-9A-Z]{8}$", "i");
    let Some(Value::Array(counts)) = p.data.get().and_then(|d| d.extra.get("counts")) else { return Vec::new() };
    counts
        .iter()
        .filter(|c| c.get("src").and_then(Value::as_str) == Some("nzta") && !c.get("virtual").is_some_and(value_truthy))
        .filter_map(|c| {
            let r = c.get("ref").filter(|r| value_truthy(r)).map(crate::jsnum::value_to_js_string).unwrap_or_default();
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
    #[must_use]
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
        let mut l = TmsLoad { sites, refs, since, chunk: 0, query: None, rows, n: 0, complete: true, timeout_ms, now_ms, statuses: Vec::new() };
        l.start_chunk();
        l
    }
    fn start_chunk(&mut self) {
        // Bound: positions in the list of sites, far below usize::MAX
        let i = self.chunk.saturating_mul(TMS_CHUNK);
        if i >= self.refs.len() {
            self.query = None;
            return;
        }
        // i is inside the list here, so the part is there
        let Some(part) = self.refs.get(i..i.saturating_add(TMS_CHUNK).min(self.refs.len())) else {
            self.query = None;
            return;
        };
        self.statuses.push(format!(
            "Downloading daily counts for sites {}\u{2013}{} of {} (NZTA TMS)\u{2026}",
            i.saturating_add(1),
            i.saturating_add(part.len()),
            self.refs.len()
        ));
        let list = part.iter().map(|s| format!("'{}'", s.replace('\'', ""))).collect::<Vec<_>>().join(",");
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
        self.query = Some(ArcQuery::new(SRC_TMS.url, params, TMS_MAX_PAGES, self.timeout_ms));
    }
    /// The progress lines the prototype passes to its status callback, in order.
    #[must_use]
    pub fn statuses(&self) -> &[String] {
        &self.statuses
    }
    /// After the download: works out each site's statistics, puts them in `P.feeds.tms` and on the NZTA counts, and stamps the vintage table.
    ///
    /// # Errors
    ///
    /// "Invalid time value" when a downloaded row's start date is not a date.
    pub fn finish(self, p: &mut Project) -> Result<TmsResult, String> {
        let mut out: BTreeMap<String, TmsStats> = BTreeMap::new();
        for s in &self.sites {
            let empty: Vec<JsonValue> = Vec::new();
            let rows = self.rows.get(&s.reference).unwrap_or(&empty);
            if let Some(st) = tms_stats(rows, &json!(s.reference), "Auckland", s.hist.as_deref(), self.now_ms, self.now_ms)? {
                out.insert(s.reference.clone(), st);
            }
        }
        let f = feeds_of(p);
        if !f.get("tms").is_some_and(value_truthy) {
            f.insert("tms".to_string(), json!({}));
        }
        if let Some(Value::Object(t)) = f.get_mut("tms") {
            for (k, v) in &out {
                // the statistics are numbers, text and lists, which always convert
                t.insert(k.clone(), serde_json::to_value(v).map_err(|e| e.to_string())?);
            }
        }
        attach_tms(p);
        let mut extra = Map::new();
        extra.insert("n".into(), json!(out.len()));
        extra.insert(
            "note".into(),
            json!(format!(
                "{} deduplicated rows (groupBy and max) since {}. {}.{}",
                fmt::int(usize_to_f64(self.n)),
                self.since,
                TMS_HCV_RULE,
                if self.complete { "" } else { " Some sites hit the download cap: shorten the period in Settings." }
            )),
        );
        stamp(p, "tms", &extra, self.now_ms);
        Ok(TmsResult { sites: out.len(), of_: self.refs.len(), rows: self.n, since: self.since, complete: self.complete, stats: out })
    }
}

impl Flow for TmsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        self.query.as_mut().and_then(Flow::next_request)
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        // a request is only asked for while a query runs; an answer without one has nothing to go to
        let Some(q) = self.query.as_mut() else { return Ok(()) };
        q.answer(a)?;
        if q.is_done() {
            let Some(q) = self.query.take() else { return Ok(()) };
            self.complete = self.complete && q.complete();
            for f in q.into_features() {
                let empty = JsonValue::object();
                let a = f.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
                let r = ["SiteRef", "siteref", "SITEREF"].iter().find_map(|k| a.get(k).filter(|v| !v.is_null()));
                if let Some(r) = r
                    && let Some(list) = self.rows.get_mut(&r.js_string())
                {
                    list.push(a.clone());
                    // Bound: counts of rows and chunks in memory, far below usize::MAX
                    self.n = self.n.saturating_add(1);
                }
            }
            self.chunk = self.chunk.saturating_add(1);
            self.start_chunk();
        }
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.query.is_none()
    }
}

/// `periodUsesTMS()`: whether the model turns a count's TMS weekday mean into its peak volume (true for this port).
#[must_use]
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
#[must_use]
pub fn period_uses_share(s: &Settings) -> bool {
    let mut a = Count::new("a", "nzta");
    a.adt = Maybe::number(1000.0);
    a.share = Maybe::Set(json!({ "AM": 0.0123, "PM": 0.0123 }));
    period_vol(&a, "AM", s).is_some_and(|a| (a.v - 12.3).abs() < 1e-9)
}

/// `attachTMS()`: puts TMS results (and peak profiles) on the NZTA counts; gives how many counts got results.
///
/// A count whose TMS mean the model uses gets `obs_method` "`weekday_factored`" and `n_days`; one it does not use (an indicative mean) keeps the AADT basis.
/// The prototype counts a count as changed when its `tms` is not the same object as the result; a project read from a file never holds the same object, so every count with a result counts.
pub fn attach_tms(p: &mut Project) -> usize {
    let feeds = p.extra.get("feeds").cloned();
    let tms_all = feeds.as_ref().and_then(|f| f.get("tms")).filter(|t| value_truthy(t)).cloned();
    let prof = feeds.as_ref().and_then(|f| f.get("profiles")).filter(|t| value_truthy(t)).cloned();
    if tms_all.is_none() && prof.is_none() {
        return 0;
    }
    let weekday = tms_all.is_some() && period_uses_tms(&p.settings);
    let settings = p.settings.clone();
    let Some(data) = p.data.get_mut() else { return 0 };
    let Some(Value::Array(counts)) = data.extra.get_mut("counts") else { return 0 };
    let mut changed: usize = 0;
    for count in counts.iter_mut() {
        let Some(obj) = count.as_object_mut() else { continue };
        if obj.get("src").and_then(Value::as_str) != Some("nzta") {
            continue;
        }
        let key = match obj.get("ref") {
            Some(r) => crate::jsnum::value_to_js_string(r),
            None => "undefined".to_string(),
        };
        let stats = tms_all.as_ref().and_then(|t| t.get(&key)).filter(|s| value_truthy(s)).cloned();
        // Older Spare Construct versions wrote c.obsMethod 'weekday_factored' although the volume came from the AADT
        obj.remove("obsMethod");
        if let Some(found) = &stats {
            obj.insert("tms".to_string(), found.clone());
            // Bound: a count of the project's counts
            changed = changed.saturating_add(1);
        }
        if let Some(found) = &stats
            && weekday
            && found.get("target").is_some_and(|t| !t.is_null())
        {
            let uses = match serde_json::from_value::<Count>(Value::Object(obj.clone())) {
                Ok(cc) => period_vol(&cc, "AM", &settings).is_some_and(|r| r.basis == "weekday_factored"),
                Err(_) => false,
            };
            if uses {
                obj.insert("obs_method".to_string(), json!("weekday_factored"));
                obj.insert("n_days".to_string(), found["target"].get("n").cloned().unwrap_or(Value::Null));
            } else if obj.get("obs_method").and_then(Value::as_str) == Some("weekday_factored") {
                obj.remove("obs_method");
                obj.remove("n_days");
            }
        }
        let site = prof.as_ref().and_then(|pr| pr.get("sites")).and_then(|st| st.get(&key)).filter(|x| value_truthy(x));
        if let (Some(pr), Some(site)) = (prof.as_ref(), site) {
            let src_text = pr.get("source").cloned().unwrap_or(Value::Null);
            let same = obj.get("share").filter(|s| value_truthy(s)).is_some_and(|s| s.get("src") == Some(&src_text) && !src_text.is_null());
            if !same {
                obj.insert(
                    "share".to_string(),
                    json!({ "AM": site.get("amShare").cloned().unwrap_or(Value::Null), "PM": site.get("pmShare").cloned().unwrap_or(Value::Null),
                        "amStart": site.get("amStart").cloned().unwrap_or(Value::Null), "pmStart": site.get("pmStart").cloned().unwrap_or(Value::Null), "src": src_text }),
                );
            }
        }
    }
    changed
}

/// `loadProfiles()`: the request for the peak profiles file (`data/tms_profiles.json` on the host's own site).
#[must_use]
pub fn profiles_request() -> GetOnce {
    GetOnce::new(HttpRequest::get_json(SRC_PROFILES.url.to_string(), 8000.0))
}

/// After `loadProfiles`: keeps the profiles in `P.feeds.profiles`, attaches them to the counts and stamps the vintage table. Gives how many counts changed.
///
/// # Errors
///
/// "`data/tms_profiles.json` has no sites" when the answer holds no sites.
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
    feeds_of(p).insert("profiles".to_string(), json!({ "source": source, "built": built, "sites": sites.to_value() }));
    let n = attach_tms(p);
    let mut extra = Map::new();
    extra.insert("n".into(), json!(n_sites));
    extra.insert("note".into(), json!(match answer.get("built") {
        Some(b) if b.truthy() => format!("Built {}.", b.js_string()),
        _ => String::new(),
    }));
    stamp(p, "profiles", &extra, now_ms);
    Ok(n)
}
