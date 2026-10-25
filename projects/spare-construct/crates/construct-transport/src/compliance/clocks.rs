//! Working days, public holidays and the statutory clocks (features/compliance.js section 11, working days and clocks).
//!
//! Working days exclude weekends, Waitangi Day, Good Friday, Easter Monday, Anzac Day, the Sovereign's birthday, Matariki and Labour
//! Day (Waitangi and Anzac Mondayised), and the Christmas period: 20 Dec to 10 Jan for the RMA (s2) or 25 Dec to 2 Jan for the
//! Fast-track Approvals Act (Legislation Act 2019 s13, which also moves a New Year that falls on a Friday or a weekend).

use super::data::c as table;
use super::dates::{DAY, date_utc, iso_utc, long_date, parse_day, parts, valid_day, weekday};
use super::jv::{Jx, V, jobj, st};
use super::record::state;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HOL: RefCell<HashMap<i64, Vec<(String, String)>>> = RefCell::new(HashMap::new());
}

/// `easter(y)`: Easter Sunday of a year as a moment (UTC midnight).
pub fn easter(y: i64) -> f64 {
    let a = y % 19;
    let b = y.div_euclid(100);
    let c = y % 100;
    let d = b.div_euclid(4);
    let e = b % 4;
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c.div_euclid(4);
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let mo = (h + l - 7 * m + 114).div_euclid(31);
    let da = ((h + l - 7 * m + 114) % 31) + 1;
    date_utc(y as f64, (mo - 1) as f64, da as f64)
}

/// `nthMonday(y, m0, n)`: the nth Monday of a month (month 0 to 11) as a moment.
pub fn nth_monday(y: i64, m0: f64, n: f64) -> f64 {
    let first = weekday(date_utc(y as f64, m0, 1.0)) as f64;
    let off = (8.0 - first) % 7.0;
    date_utc(y as f64, m0, 1.0 + off + 7.0 * (n - 1.0))
}

/// `holidays(y)`: the public holidays of a year as (YYYY-MM-DD, name) in the order they are added; a year with no Matariki date in the
/// Act's schedule has the entry ("matariki-unknown", year).
pub fn holidays(y: i64) -> Vec<(String, String)> {
    if let Some(h) = HOL.with(|c| c.borrow().get(&y).cloned()) {
        return h;
    }
    let mut h: Vec<(String, String)> = Vec::new();
    let mut put = |key: String, name: String| {
        if let Some(e) = h.iter_mut().find(|e| e.0 == key) {
            e.1 = name;
        } else {
            h.push((key, name));
        }
    };
    let yf = y as f64;
    let mond = |t: f64, n: &str, put: &mut dyn FnMut(String, String)| {
        put(iso_utc(t), n.to_string());
        let w = weekday(t);
        if w == 6 {
            put(iso_utc(t + 2.0 * DAY), format!("{n} (observed)"));
        }
        if w == 0 {
            put(iso_utc(t + DAY), format!("{n} (observed)"));
        }
    };
    mond(date_utc(yf, 1.0, 6.0), "Waitangi Day", &mut put);
    let e = easter(y);
    put(iso_utc(e - 2.0 * DAY), "Good Friday".into());
    put(iso_utc(e + DAY), "Easter Monday".into());
    mond(date_utc(yf, 3.0, 25.0), "Anzac Day", &mut put);
    put(iso_utc(nth_monday(y, 5.0, 1.0)), "Sovereign's birthday".into());
    put(iso_utc(nth_monday(y, 9.0, 4.0)), "Labour Day".into());
    let mat = table("MATARIKI").g(&y.to_string());
    if mat.t() {
        if let Some(t) = parse_day(&format!("{y}-{}", mat.s())) {
            put(iso_utc(t), "Matariki".into());
        }
    } else {
        put("matariki-unknown".into(), y.to_string());
    }
    HOL.with(|c| c.borrow_mut().insert(y, h.clone()));
    h
}

fn holiday_name(y: i64, key: &str) -> Option<String> {
    holidays(y).into_iter().find(|e| e.0 == key).map(|e| e.1)
}

/// `excluded(t, kind)`: why a day is not a working day ("Saturday", a holiday's name, the Christmas period ...), None for a working day.
/// `kind` is "rma" (the default) or "ftaa".
pub fn excluded(t: f64, kind: &str) -> Option<String> {
    let (y, m0, day, w) = parts(t)?;
    let m = m0 + 1;
    let wd = table("WD").g(if table("WD").has(kind) { kind } else { "rma" });
    if w == 0 || w == 6 {
        return Some(if w == 0 { "Sunday" } else { "Saturday" }.to_string());
    }
    if let Some(h) = holiday_name(y, &iso_utc(t)) {
        return Some(h);
    }
    let (from, to) = (wd.g("from"), wd.g("to"));
    if (m as f64 == from.ix(0).n() && day as f64 >= from.ix(1).n()) || (m as f64 == to.ix(0).n() && day as f64 <= to.ix(1).n()) {
        return Some(if kind == "ftaa" { "Christmas and New Year period (25 Dec \u{2013} 2 Jan)" } else { "Christmas period (20 Dec \u{2013} 10 Jan)" }.to_string());
    }
    if wd.g("newYear").t() && m == 1 {
        // Legislation Act s13: if 1 Jan is a Friday, the following Monday; if a Saturday or Sunday, the following Monday and Tuesday
        let j1 = weekday(date_utc(y as f64, 0.0, 1.0));
        if j1 == 5 && day == 4 {
            return Some("New Year (1 Jan fell on a Friday)".to_string());
        }
        if (j1 == 6 && (day == 3 || day == 4)) || (j1 == 0 && day == 3) {
            return Some("New Year (1 Jan fell on a weekend)".to_string());
        }
    }
    None
}

/// `isWorkingDay(s, kind)`.
pub fn is_working_day(s: &str, kind: &str) -> bool {
    parse_day(s).is_some_and(|t| excluded(t, kind).is_none())
}

/// `addWorkingDays(s, n, kind)`: the day n working days after (or, for a negative n, before) a day, as YYYY-MM-DD; None when s is not a day.
pub fn add_working_days(s: &str, n: f64, kind: &str) -> Option<String> {
    let mut t = parse_day(s)?;
    let step = if n >= 0.0 { 1.0 } else { -1.0 };
    let mut left = n.abs();
    let mut guard = 0;
    while left > 0.0 && guard < 2000 {
        guard += 1;
        t += step * DAY;
        if excluded(t, kind).is_none() {
            left -= 1.0;
        }
    }
    Some(iso_utc(t))
}

/// `workingDaysBetween(a, b, kind)`: the working days from a to b (negative when b is before a); None when either is not a day.
pub fn working_days_between(a: &str, b: &str, kind: &str) -> Option<f64> {
    let (mut t, e) = (parse_day(a)?, parse_day(b)?);
    let s = if e >= t { 1.0 } else { -1.0 };
    let mut n = 0.0;
    while t != e {
        t += s * DAY;
        if excluded(t, kind).is_none() {
            n += s;
        }
    }
    Some(n)
}

/// `addMonths(s, n)`: the same day n months on, or the last day of a shorter month, as YYYY-MM-DD.
pub fn add_months(s: &str, n: f64) -> Option<String> {
    let t = parse_day(s)?;
    let (y, m0, day, _) = parts(t)?;
    let m = m0 as f64 + n;
    let last = parts(date_utc(y as f64, m + 1.0, 0.0))?.2;
    Some(iso_utc(date_utc(y as f64, m, (day.min(last)) as f64)))
}

/// `clockFor(ev)`: the dates a statutory clock gives for an event (`{ kind, date, ... }`): the rows due, with warnings. None for an unknown
/// kind or an event without a date.
pub fn clock_for(ev: &V) -> Option<V> {
    let def = table("CLOCKS").g(&ev.g("kind").s());
    if !def.t() || !valid_day(ev.g("date")) {
        return None;
    }
    let kind = def.g("kind").s();
    let date = ev.g("date").s();
    let mut out: Vec<V> = Vec::new();
    let mut warns: Vec<V> = Vec::new();
    let year: i64 = date.chars().take(4).collect::<String>().parse().unwrap_or(0);
    if holidays(year).iter().any(|e| e.0 == "matariki-unknown") || holidays(year + 1).iter().any(|e| e.0 == "matariki-unknown") {
        warns.push(st("Matariki is not in the Act's schedule for this year: check the dates by hand."));
    }
    for r in def.g("rules").items() {
        if r.g("months").t() {
            if let Some(prev) = out.iter().find(|o| o.g("id").strict_eq(r.g("after"))) {
                let d = add_months(&prev.g("date").s(), r.g("months").n());
                out.push(jobj! { "id" => r.g("id"), "date" => d, "text" => r.g("text") });
            }
            continue;
        }
        if r.g("max").t() {
            let limit = add_working_days(&date, r.g("max").n(), &kind);
            let set = if valid_day(ev.g("set")) { ev.g("set").s() } else { limit.clone().unwrap_or_default() };
            if limit.as_deref().is_some_and(|l| set.as_str() > l) {
                warns.push(st(format!("The panel-set date {} is more than {} working days after the direction (latest {}, s67(3)).", long_date(&set), super::jv::ns(r.g("max").n()), long_date(limit.as_deref().unwrap_or("")))));
            }
            out.push(jobj! { "id" => r.g("id"), "date" => set.as_str(), "limit" => limit, "text" => r.g("text") });
            continue;
        }
        out.push(jobj! { "id" => r.g("id"), "date" => add_working_days(&date, r.g("n").n(), &kind), "text" => r.g("text") });
    }
    for r in def.g("back").items() {
        out.push(jobj! { "id" => r.g("id"), "date" => add_working_days(&date, -r.g("n").n(), &kind), "text" => r.g("text") });
    }
    let mut res = ev.clone();
    res.set("label", def.g("label").clone());
    res.set("wd", table("WD").g(&kind).g("name").clone());
    res.set("rows", V::Array(out));
    res.set("warns", V::Array(warns));
    Some(res)
}

/// `clocks(P)`: every statutory clock of the project (further-information requests, plan change requests, Fast-track comments, the hearing,
/// state highway works and the lodgement of a resource consent), each with its dates, in order of event date.
pub fn clocks(p: &V) -> V {
    let c = state(p);
    let mut ev: Vec<V> = c.g("rfi").g("events").items().to_vec();
    if valid_day(c.g("rfi").g("hearing")) && !ev.iter().any(|e| e.g("kind").eq_s("hearing")) {
        ev.push(jobj! { "id" => "hearing", "kind" => "hearing", "date" => c.g("rfi").g("hearing") });
    }
    if valid_day(c.g("rfi").g("worksDate")) {
        ev.push(jobj! { "id" => "works", "kind" => "works", "date" => c.g("rfi").g("worksDate") });
    }
    let ctx = c.g("ctx");
    if valid_day(ctx.g("lodgement")) && ctx.g("lodgementActual").t() && !ev.iter().any(|e| e.g("kind").eq_s("lodged")) && super::record::pathway_entry(ctx.g("pathway")).is_some_and(|e| e.g("kind").eq_s("rc")) {
        ev.push(jobj! { "id" => "lodged", "kind" => "lodged", "date" => ctx.g("lodgement") });
    }
    V::Array(ev.iter().filter_map(clock_for).collect()).into_sorted_by_date()
}

/// `rfiTemplate(P)`: the response to a request for further information.
pub fn rfi_template(p: &V) -> String {
    let r = super::record::regime(p);
    let c = state(p);
    let pe = super::record::pathway_entry(c.g("ctx").g("pathway"));
    let s2aa = r.g("side").eq_s("transition") && pe.is_some_and(|e| e.g("kind").eq_s("rc")) && !c.g("ctx").g("pathway").eq_s("rc-subdivision");
    let mut t = "Response to the request for further information dated [date]. The applicant has provided the information below in proportion to the scale and significance of the transport effects (RMA s92(2B)).".to_string();
    if s2aa {
        t.push_str(" Under s92(2AA) the council may not request information about effects that must be disregarded under s104(1A) for a district land use consent lodged on or after 30 October 2026; items [x] are outside that scope.");
    }
    t.push_str(" [For each item: the request, the response, and where the evidence is in the assessment.]");
    t
}

trait SortByDate {
    fn into_sorted_by_date(self) -> V;
}

impl SortByDate for V {
    /// `.sort((a, b) => a.date < b.date ? -1 : 1)`: a comparator that never says equal, run as V8 runs it.
    fn into_sorted_by_date(self) -> V {
        let V::Array(mut items) = self else { return self };
        crate::graph::js_sort_by(&mut items, |a, b| if a.g("date").s().as_str() < b.g("date").s().as_str() { -1.0 } else { 1.0 });
        V::Array(items)
    }
}
