//! The crash history of a road section and the cost benefit disclosure statement (CBDS, clause 3.3 of the Setting of Speed Limits Rule 2024).

use super::{limit_of, road_len};
use crate::compliance::extra::ctx::{active_sc, assumption, assumptions, authority_of, data, disclaimer, link_name, nn, pair, posted, res_of};
use crate::compliance::extra::options::util::{Put, fld_s, label, n0, or_s, xs};
use crate::compliance::data::x as xdata;
use crate::compliance::jv::{Jx, V, jarr, jobj};
use crate::compliance::{engine, net};
use crate::graph::js_sort_by;
use crate::jsmath;
use crate::jsnum::{math_max, math_min, number_to_js_string};
use crate::util::fmt;
use std::collections::HashMap;

/// `sectionCrashes(P, sc, Ls)`: the crashes on a section's links by year and severity (`{ year: { F, S, M, N } }`): each crash goes to its nearest
/// existing road within 25 m, or to a nearby road its crash location names (as DataHub assigns crashes for crash risk). The crashes are the
/// project's own (`P.data.crashes`, each `[lat, lng, severity, year, location]`). None when no crashes are loaded.
pub fn section_crashes(p: &V, sc: &V, ls: &[&V]) -> Option<V> {
    let cr = p.g("data").g("crashes").items();
    if cr.is_empty() {
        // a stored text is read by the prototype one character at a time (each is no crash near a road): no crashes are found
        return match p.g("data").g("crashes") {
            V::String(s) if !s.is_empty() => Some(V::object()),
            _ => None,
        };
    }
    let r = 25.0;
    let pad = r / 90000.0;
    let ids: Vec<String> = ls.iter().map(|l| l.g("id").s()).collect();
    let mut by = V::object();
    let links: Vec<&V> = sc.g("links").vals().into_iter().filter(|l| !l.g("status").eq_s("proposed") && sc.g("nodes").g(&l.g("a").s()).t() && sc.g("nodes").g(&l.g("b").s()).t()).collect();
    let mut boxes: HashMap<String, [f64; 4]> = HashMap::new();
    for l in &links {
        let (mut a, mut b, mut c, mut d) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for pt in net::geom(sc, l).unwrap_or_default() {
            a = math_min(a, pt[0]);
            b = math_max(b, pt[0]);
            c = math_min(c, pt[1]);
            d = math_max(d, pt[1]);
        }
        boxes.insert(l.g("id").s(), [a, b, c, d]);
    }
    let near = |l: &V, c: &V| -> bool {
        match boxes.get(&l.g("id").s()) {
            Some(b) => {
                let (la, ln) = (c.ix(0).n(), c.ix(1).n());
                !(la < b[0] - pad || la > b[1] + pad || ln < b[2] - pad * 1.3 || ln > b[3] + pad * 1.3)
            }
            None => false,
        }
    };
    let norm = |v: &V| engine::norm(&(if v.t() { v.s() } else { String::new() }));
    for c in cr {
        if !ls.iter().any(|l| near(l, c)) {
            continue;
        }
        let mut ds: Vec<(&V, f64)> = Vec::new();
        for l in &links {
            if near(l, c) {
                let d = net::project(sc, l, [c.ix(0).n(), c.ix(1).n()]).map_or(f64::NAN, |x| x.d);
                if d <= r {
                    ds.push((l, d));
                }
            }
        }
        if ds.is_empty() {
            continue;
        }
        js_sort_by(&mut ds, |x, y| x.1 - y.1);
        let first = ds[0];
        let mut best = first;
        let loc_text = if c.ix(4).t() { c.ix(4).s() } else { String::new() };
        let loc = engine::norm(loc_text.split(" / ").next().unwrap_or(""));
        if !loc.is_empty() && norm(best.0.g("name")) != loc {
            let alt = ds.iter().find(|(l, d)| {
                let nm = norm(l.g("name"));
                *d <= first.1 + 5.0 && !nm.is_empty() && (nm == loc || nm.contains(&loc) || loc.contains(&nm))
            });
            if let Some(a) = alt {
                best = *a;
            }
        }
        if !ids.contains(&best.0.g("id").s()) {
            continue;
        }
        let y = n0(c.ix(3));
        let yk = number_to_js_string(y);
        if !by.g(&yk).t() {
            by.put(&yk, jobj! { "F" => 0, "S" => 0, "M" => 0, "N" => 0 });
        }
        let sev = c.ix(2).s();
        let key = if "FSMN".contains(sev.as_str()) { sev } else { "N".to_string() };
        if let Some(t) = by.get_mut(&yk) {
            let cur = if t.has(&key) { t.g(&key).n() } else { f64::NAN };
            t.put(&key, cur + 1.0);
        }
    }
    Some(by)
}

/// `crashHistory(P, sc, Ls, since)`: the cl 3.3 crash history: the full calendar years of the CAS window (from the year the limit or road existed,
/// `since`, when later), with the year to date reported but not counted (incomplete): `{ hist, years, from, to, since, ytdYear, ytd, lagYear, period }`.
/// Null when no crashes are loaded.
pub fn crash_history(p: &V, sc: &V, ls: &[&V], since: Option<f64>) -> V {
    let Some(by) = section_crashes(p, sc, ls) else { return V::Null };
    let cas = if p.g("data").g("loaded").g("cas").t() { p.g("data").g("loaded").g("cas").clone() } else { V::object() };
    let ys: Vec<f64> = by.keys().iter().map(|k| crate::jsnum::string_to_number(k)).filter(|y| *y > 0.0).collect();
    let ymin = ys.iter().fold(f64::INFINITY, |m, y| math_min(m, *y));
    let ymax = ys.iter().fold(f64::NEG_INFINITY, |m, y| math_max(m, *y));
    let from: Option<f64> = if cas.g("from").n() > 0.0 { Some(cas.g("from").n()) } else if !ys.is_empty() { Some(ymin) } else { None };
    let full_to: Option<f64> = if cas.g("to").n() > 0.0 { Some(cas.g("to").n()) } else if !ys.is_empty() { Some(ymax) } else { None };
    let ytd_y: Option<f64> = if cas.g("ytd").n() > 0.0 { Some(cas.g("ytd").n()) } else if cas.g("to").n() > 0.0 { Some(cas.g("to").n() + 1.0) } else { None };
    let since_t: Option<f64> = since.filter(|s| *s != 0.0 && !s.is_nan());
    let y0 = from.map(|f| match since_t {
        Some(s) => math_max(f, s),
        None => f,
    });
    let n = match (y0, full_to) {
        (Some(y0), Some(t)) => math_max(0.0, t - y0 + 1.0),
        _ => 0.0,
    };
    let mut hist = [0.0f64; 4];
    if n != 0.0 && !n.is_nan() {
        let (mut y, to) = (y0.unwrap_or(0.0), full_to.unwrap_or(0.0));
        while y <= to {
            let row = by.g(&number_to_js_string(y));
            for (i, k) in ["F", "S", "M", "N"].iter().enumerate() {
                hist[i] += n0(row.g(k));
            }
            y += 1.0;
        }
    }
    let ytd = match ytd_y {
        Some(yy) if yy != 0.0 && yy > full_to.unwrap_or(0.0) && by.g(&number_to_js_string(yy)).t() => {
            let row = by.g(&number_to_js_string(yy));
            ["F", "S", "M", "N"].iter().fold(0.0, |s, k| s + row.g(k).n())
        }
        _ => 0.0,
    };
    let period = if n != 0.0 && !n.is_nan() {
        let y0v = y0.unwrap_or(f64::NAN);
        format!(
            "{}\u{2013}{} ({} full year{}{})",
            number_to_js_string(y0v),
            number_to_js_string(full_to.unwrap_or(f64::NAN)),
            number_to_js_string(n),
            if n == 1.0 { "" } else { "s" },
            match since_t {
                Some(s) if s > from.unwrap_or(0.0) => format!(", from {0}, when the limit or road began; part of {0} may be under the earlier limit", number_to_js_string(s)),
                _ => String::new(),
            }
        )
    } else {
        format!("No full year of CAS data{}", match since_t { Some(s) => format!(" since {}", number_to_js_string(s)), None => String::new() })
    };
    jobj! {
        "hist" => jobj! { "F" => hist[0], "S" => hist[1], "M" => hist[2], "N" => hist[3] },
        "years" => n, "from" => y0, "to" => full_to, "since" => since_t, "ytdYear" => ytd_y, "ytd" => ytd, "lagYear" => or_v_null(cas.g("lagYear")), "period" => period,
    }
}

/// `x || null`.
fn or_v_null(v: &V) -> V {
    if v.t() { v.clone() } else { V::Null }
}

/// The year's crash counts of a history as numbers `[F, S, M, N]`.
fn hist_of(ch: &V) -> [f64; 4] {
    let h = ch.g("hist");
    [h.g("F").n(), h.g("S").n(), h.g("M").n(), h.g("N").n()]
}

/// `cbds(P, ids, opts)`: the cost benefit disclosure statement of a speed limit change on road sections (clause 3.3 of the Rule): `{ ok, why, newRoad? }`
/// when it is not offered (no road chosen, a new road, a school-gate limit), otherwise `{ ok, ids, name, current, proposed, history, headings, warn,
/// watermark, disclaimer, decision }`. The proposed limit, school and new-road flags are read from `sc` (the active scenario by default); the existing
/// road, its current limit, crash history, modelled speed and AADT from the do-minimum. A mean operating speed must be measured: a modelled one is
/// labelled as a placeholder.
pub fn cbds(p: &V, ids: &[String], sc: Option<&V>) -> V {
    let pr = pair(p);
    let sc = sc.unwrap_or_else(|| active_sc(p));
    let base = pr.base;
    let a = assumptions(p);
    let cxd = data(p);
    let cx = cxd.g("speed");
    let nl = xdata("STD.nilsson");
    let ls: Vec<&V> = ids
        .iter()
        .filter_map(|id| {
            let l = if sc.g("links").g(id).t() { sc.g("links").g(id) } else { base.g("links").g(id) };
            if l.t() { Some(l) } else { None }
        })
        .collect();
    if ls.is_empty() {
        return jobj! { "ok" => false, "why" => "Choose the road section (links with a proposed limit)." };
    }
    let base_of = |l: &V| base.g("links").g(&l.g("id").s());
    let skip = xs("STD.speed.cbdsSkip");
    if let Some(n) = ls.iter().find(|l| l.g("status").eq_s("proposed") || (l.g("cx").t() && l.g("cx").g("newRoad").t()) || !base_of(l).t() || base_of(l).g("status").eq_s("proposed")) {
        return jobj! { "ok" => false, "why" => format!("{} is a new road: {skip}.", link_name(n)), "newRoad" => true };
    }
    if let Some(s) = ls.iter().find(|l| (l.g("cx").t() && l.g("cx").g("school").t()) || (base_of(l).g("cx").t() && base_of(l).g("cx").g("school").t())) {
        return jobj! { "ok" => false, "why" => format!("{} is a school-gate limit: {skip}.", link_name(s)) };
    }
    let bs: Vec<&V> = ls.iter().map(|l| base_of(l)).collect();
    // nn((L.cx || {})[k]) ?? nn((base.links[L.id].cx || {})[k])
    let cx_of = |l: &V, k: &str| -> Option<f64> { nn(l.g("cx").g(k)).or_else(|| nn(base_of(l).g("cx").g(k))) };
    let len = road_len(base, &bs);
    let v0 = posted(bs[0]);
    let v1 = limit_of(ls[0]);
    let risk = engine::link_risk(p, base);
    let since: Option<f64> = {
        let mut v: Vec<f64> = ls.iter().filter_map(|l| cx_of(l, "since")).filter(|x| *x != 0.0).collect();
        js_sort_by(&mut v, |x, y| y - x);
        v.first().copied().filter(|x| *x != 0.0)
    };
    let ch = crash_history(p, base, &bs, since);
    let hist = if ch.t() { hist_of(&ch) } else { [0.0; 4] };
    let yrs: Option<f64> = if ch.t() && ch.g("years").n() > 0.0 { Some(ch.g("years").n()) } else { None };
    let dsi_yr = yrs.map(|y| (hist[0] + hist[1]) / y);
    let inj_yr = yrs.map(|y| (hist[0] + hist[1] + hist[2]) / y);
    let non_yr = yrs.map(|y| hist[3] / y);
    // mean operating speed: measured (author or MegaMaps), otherwise the modelled speed as a placeholder
    let meas: Vec<f64> = ls.iter().filter_map(|l| cx_of(l, "meanSpeed")).filter(|x| *x != 0.0).collect();
    let mm: Vec<f64> = bs.iter().filter_map(|l| if l.g("megamaps").t() { nn(l.g("megamaps").g("mean")) } else { None }).filter(|x| *x != 0.0).collect();
    let r0 = res_of(base, "AM").or_else(|| res_of(base, "PM"));
    let modelled: Option<f64> = r0.and_then(|r0| {
        let (mut d, mut t) = (0.0, 0.0);
        for l in &bs {
            for k in ["ab", "ba"] {
                let x = r0.g("arcs").g(&format!("{}:{k}", l.g("id").s()));
                if x.t() && x.g("t").n() > 0.0 {
                    d += x.g("len").n();
                    t += x.g("t").n();
                }
            }
        }
        if t > 0.0 { Some(d / t * 3.6) } else { None }
    });
    let mean = |v: &[f64]| v.iter().fold(0.0, |s, x| s + x) / v.len() as f64;
    let m0: Option<f64> = if !meas.is_empty() { Some(mean(&meas)) } else if !mm.is_empty() { Some(mean(&mm)) } else { modelled };
    let src_txt: Option<V> = ls
        .iter()
        .map(|l| if l.g("cx").t() && l.g("cx").g("meanSpeedSrc").t() { l.g("cx").g("meanSpeedSrc").clone() } else { base_of(l).g("cx").g("meanSpeedSrc").clone() })
        .find(|v| v.t());
    let m_src = if !meas.is_empty() {
        format!("measured ({})", src_txt.map_or("entered by the author".to_string(), |s| s.s()))
    } else if !mm.is_empty() {
        "MegaMaps".to_string()
    } else if modelled.is_some() {
        format!("modelled: {}", label("meanSpeed"))
    } else {
        "not available".to_string()
    };
    let mean_share = a.g("meanShare").n();
    let m1: Option<f64> = m0.map(|m| math_max(5.0, m + mean_share * (v1 - v0)));
    // Nilsson power model: fatal and serious crashes vary with v^dsiExponent, injury crashes with v^injuryExponent; non-injury held
    let truthy = |x: Option<f64>| x.is_some_and(|v| v != 0.0 && !v.is_nan());
    let k = if truthy(m0) && truthy(m1) { m1.unwrap_or(0.0) / m0.unwrap_or(1.0) } else { 1.0 };
    let dsi1 = dsi_yr.map(|d| d * jsmath::pow(k, a.g("dsiExponent").n()));
    let inj1 = inj_yr.map(|i| i * jsmath::pow(k, a.g("injuryExponent").n()));
    let all0 = inj_yr.map(|i| i + non_yr.unwrap_or(f64::NAN));
    let all1 = inj1.map(|i| i + non_yr.unwrap_or(f64::NAN));
    let aadt: Option<(f64, String)> = {
        let l0 = bs[0];
        let rk = risk.as_ref().map(|r| r.g(&l0.g("id").s()).clone());
        match rk {
            Some(r) if r.t() && r.g("aadt").n() > 0.0 => Some((r.g("aadt").n(), if r.g("aadtSrc").t() { r.g("aadtSrc").s() } else { "count".to_string() })),
            _ => {
                let x: Option<f64> = r0.map(|r0| ["ab", "ba"].iter().fold(0.0, |s, d| s + n0(r0.g("arcs").g(&format!("{}:{d}", l0.g("id").s())).g("v"))));
                if truthy(x) { Some((x.unwrap_or(0.0) * 10.0, format!("modelled peak hour \u{d7} 10 ({})", assumption()))) } else { None }
            }
        }
    };
    let dt_veh: Option<f64> = if truthy(m0) && truthy(m1) { Some(len / (m1.unwrap_or(0.0) / 3.6) - len / (m0.unwrap_or(0.0) / 3.6)) } else { None };
    let hist_rows: Vec<V> = if ch.t() {
        let mut rows = vec![
            jarr!["Fatal", hist[0]], jarr!["Serious", hist[1]], jarr!["Minor", hist[2]], jarr!["Non-injury", hist[3]], jarr!["Period", ch.g("period")],
        ];
        if ch.g("ytd").t() {
            rows.push(jarr![
                "Not counted",
                format!("{} crash{} in {} to date (YTD, incomplete)", ch.g("ytd").s(), if ch.g("ytd").n() == 1.0 { "" } else { "es" }, ch.g("ytdYear").s())
            ]);
        }
        if ch.g("lagYear").t() {
            rows.push(jarr!["Note", format!("{} may be incomplete (CAS entry lag)", ch.g("lagYear").s())]);
        }
        rows
    } else {
        vec![jarr!["CAS", "Not loaded: load NZTA CAS crashes (Traffic data tab)"]]
    };
    let dsi_ex = a.g("dsiExponent").s();
    let inj_ex = a.g("injuryExponent").s();
    let asm = assumption();
    let method = format!(
        "Nilsson power model on mean speed ({}): fatal and serious crashes \u{221d} v^{dsi_ex}, injury crashes \u{221d} v^{inj_ex}; non-injury crashes held at the current rate (the model covers injury crashes only); {asm} exponents (D13). Not an MBCM crash procedure: MBCM v1.7.5 crash trend factors for speed-limit categories (NZTA GC 26/01) are not applied",
        nl.g("edition").s()
    );
    let nv = |x: Option<f64>, d: usize| -> String { x.map_or_else(|| fmt::DASH.to_string(), |v| fmt_num(v, d)) };
    let forecast_rows: Vec<V> = if yrs.is_some() {
        vec![
            jarr!["Without the change: DSI per year", nv(dsi_yr, 2)],
            jarr!["With the change: DSI per year", nv(dsi1, 2)],
            jarr!["Without: injury crashes per year", nv(inj_yr, 2)],
            jarr!["With: injury crashes per year", nv(inj1, 2)],
            jarr!["Without: all crashes per year", nv(all0, 2)],
            jarr!["With: all crashes per year", nv(all1, 2)],
            jarr!["Over 5 years: DSI saved", nv(Some((dsi_yr.unwrap_or(f64::NAN) - dsi1.unwrap_or(f64::NAN)) * 5.0), 1)],
            jarr!["Method", method.as_str()],
        ]
    } else {
        vec![jarr!["Forecast", if ch.t() { "No full year of crash history to forecast from" } else { "Load NZTA CAS crashes to forecast" }], jarr!["Method", method.as_str()]]
    };
    let cost = cx.g("cost").n();
    let headings = vec![
        jobj! {
            "id" => "history", "title" => "Crashes by severity over the previous 5 years", "rule" => "cl 3.3", "rows" => V::Array(hist_rows),
            "basis" => "Each crash on its nearest existing road within 25 m (DataHub method); full calendar years only",
        },
        jobj! { "id" => "forecast", "title" => "Forecast annual crashes and DSIs over the next 5 years, with and without the change", "rule" => "cl 3.3", "rows" => V::Array(forecast_rows) },
        jobj! {
            "id" => "speed", "title" => "Current mean operating speed and estimated new mean speed", "rule" => "cl 3.3",
            "rows" => V::Array(vec![
                jarr!["Current limit", format!("{} km/h", number_to_js_string(v0))],
                jarr!["Proposed limit", format!("{} km/h", number_to_js_string(v1))],
                jarr![
                    "Current mean operating speed",
                    match m0 { Some(m) => format!("{} km/h ({m_src})", fmt_num(m, 1)), None => "Not available: enter a measured mean speed".to_string() }
                ],
                jarr![
                    "Estimated new mean speed",
                    match m1 { Some(m) => format!("{} km/h (current + {} \u{d7} limit change; {asm})", fmt_num(m, 1), a.g("meanShare").s()), None => fmt::DASH.to_string() }
                ],
            ]),
        },
        jobj! {
            "id" => "time", "title" => "Journey time change: per vehicle and AADT-weighted", "rule" => "cl 3.3",
            "rows" => V::Array(vec![
                jarr!["Section length", format!("{} (a divided road counted once)", fmt::len(len))],
                jarr!["Per vehicle", match dt_veh { Some(d) => format!("{} s", fmt::signed(d, 1)), None => fmt::DASH.to_string() }],
                jarr!["AADT", match &aadt { Some((v, s)) => format!("{} veh/day ({s})", fmt::int(*v)), None => "Not available".to_string() }],
                jarr![
                    "AADT-weighted",
                    match (dt_veh, &aadt) { (Some(d), Some((v, _))) => format!("{} veh\u{b7}h a year", fmt::signed(d * v * 365.0 / 3600.0, 0)), _ => fmt::DASH.to_string() }
                ],
            ]),
        },
        jobj! {
            "id" => "cost", "title" => "Implementation cost", "rule" => "cl 3.3",
            "rows" => V::Array(vec![jarr!["Cost", if cost > 0.0 { format!("NZ${}", fmt::int(cost)) } else { "Enter the implementation cost".to_string() }]]),
        },
        jobj! {
            "id" => "tool", "title" => "Tool and assumptions", "rule" => "cl 3.3",
            "rows" => V::Array(vec![
                jarr!["Tool", or_s(cx.g("tool"), "Spare Construct sketch model")],
                jarr!["Counterfactual", "The limit does not change"],
                jarr!["Scenarios", format!("Proposed limit from {}; existing road, crashes and modelled speed from {}", fld_s(sc, "name"), fld_s(base, "name"))],
                jarr!["Assumptions", format!("Mean-speed share {}; power-model exponents {dsi_ex} (fatal and serious) and {inj_ex} (injury); {asm} values (D13)", a.g("meanShare").s())],
            ]),
        },
    ];
    let mut warn: Vec<String> = Vec::new();
    if v1 == v0 {
        warn.push(format!(
            "The proposed limit equals the current limit ({} km/h): set the proposed limit on {} (road inspector \u{203a} Speed limit).",
            number_to_js_string(v0),
            fld_s(sc, "name")
        ));
    }
    if meas.is_empty() && mm.is_empty() {
        warn.push("No measured mean operating speed: a modelled speed is not a mean operating speed. Enter a measured speed (road inspector \u{203a} Speed limit) or load MegaMaps.".to_string());
    }
    if cx.g("consultationChanged").t() {
        warn.push("The proposed limit changed after consultation: re-run this CBDS (cl 3.3(7)).".to_string());
    }
    jobj! {
        "ok" => true, "ids" => V::Array(ls.iter().map(|l| l.g("id").clone()).collect()), "name" => link_name(ls[0]), "current" => v0, "proposed" => v1, "history" => ch,
        "headings" => V::Array(headings), "warn" => crate::compliance::jv::strs(warn.iter()), "watermark" => label("cbds"), "disclaimer" => disclaimer("D9", p, None),
        "decision" => authority_of(p, ls[0], sc).g("decision"),
    }
}

/// `num(x, d)` for a number (an en dash when not finite).
fn fmt_num(x: f64, d: usize) -> String {
    if x.is_finite() { fmt::num(x, d) } else { fmt::DASH.to_string() }
}
