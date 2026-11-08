//! ANOP indicative multimodal level of service and fit (compliance-extra.js section 10), with its CSV export and report section.

use super::ctx::{
    active_sc, assumption, data, disc, esc, nn, pair, periods_of, res_of, stamped, today,
};
use super::options::util::{label, or_s, or_v, tbl_th, unique, xs};
use crate::compliance::data::x as xdata;
use crate::compliance::html::{ReportCtx, rt, th};
use crate::compliance::jv::{Jx, NULL, V, csv, jobj};
use crate::compliance::net;
use crate::compliance::rx::rx;
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, math_min, math_round, number_to_js_string};
use crate::util::{self, fmt};

/// The modes ANOP assesses (`MODES`).
pub const MODES: [&str; 5] = ["walk", "cycle", "pt", "freight", "gt"];

/// `SCORE[letter]`: the number a level of service letter stands for (NaN for anything else).
fn score(letter: &str) -> f64 {
    let t = xdata("STD.anop.score");
    if t.has(letter) {
        t.g(letter).n()
    } else {
        f64::NAN
    }
}

/// `fromScore(s)`: the level of service letter for a score (rounded and kept between 0 and 7; "F" when none matches).
fn from_score(s: f64) -> String {
    let want = math_max(0.0, math_min(7.0, math_round(s)));
    xdata("STD.anop.score")
        .entries()
        .into_iter()
        .find(|(_, v)| v.n() == want)
        .map_or_else(|| "F".to_string(), |(k, _)| k.to_string())
}

/// `colOf(lu)`: the column of the preferred level of service table for a land use.
fn col_of(lu: &str) -> usize {
    match lu {
        "city-metro" => 0,
        "town-local" => 1,
        _ => 2,
    }
}

/// `ROAD_H(L)`: the hierarchy of a road from its class (motorway, arterial, collector or local).
fn road_h(l: &V) -> &'static str {
    match l.g("cls").s().as_str() {
        "motorway" | "ramp" => "motorway",
        "arterial" => "arterial",
        "collector" => "collector",
        _ => "local",
    }
}

/// `HIER_LABEL[h]`.
fn hier_label(h: &str) -> V {
    xdata("HIER_LABEL").g(h).clone()
}

/// The link ids of a corridor (`c.links`).
fn ids_of(c: &V) -> Vec<String> {
    c.g("links").items().iter().map(|x| x.s()).collect()
}

/// `hierOf(sc, c, mode)`: the hierarchy of a corridor for a mode, length-weighted over the links conflated with AT Future Connect (`L.fc`); links without
/// Future Connect data are left out of the tally and the share is stated. With no Future Connect on the corridor, general traffic uses the road class and the
/// other modes a stated default. Walking is entered by the author. `{ h, src, assumed? }`.
pub fn hier_of(sc: &V, c: &V, mode: &str) -> V {
    if mode == "walk" {
        let w = c.g("walk");
        let known = xdata("PREF").g("walk").g(&w.s()).t();
        return jobj! { "h" => if known { w.clone() } else { V::String("none".into()) }, "src" => "entered by the author (Future Connect has no walking layer)" };
    }
    let ids = ids_of(c);
    let mut tally: Vec<(String, f64)> = Vec::new();
    let add = |t: &mut Vec<(String, f64)>, h: &str, w: f64| match t.iter_mut().find(|x| x.0 == h) {
        Some(x) => x.1 += w,
        None => t.push((h.to_string(), w)),
    };
    let (mut all, mut fc_len, mut road_fill) = (0.0, 0.0, 0.0);
    for id in &ids {
        let l = sc.g("links").g(id);
        if !l.t() {
            continue;
        }
        let w = net::len(sc, l);
        all += w;
        if !l.g("fc").t() {
            continue;
        }
        let fc = l.g("fc");
        let mut h: Option<&str> = None;
        fc_len += w;
        let text = |k: &str| {
            if fc.g(k).t() {
                fc.g(k).s()
            } else {
                String::new()
            }
        };
        if mode == "gt" {
            let v = text("gt");
            h = if rx("motorway", "i").test(&v) {
                Some("motorway")
            } else if rx("arterial", "i").test(&v) {
                Some("arterial")
            } else if rx("collector", "i").test(&v) {
                Some("collector")
            } else if !v.is_empty() {
                Some("local")
            } else {
                None
            };
            if h.is_none() {
                h = Some(road_h(l));
                road_fill += w;
            }
        }
        if mode == "freight" {
            let v = text("freight");
            h = if rx("rail", "i").test(&v) {
                None
            } else if rx("1A|1B", "i").test(&v) {
                Some("l1")
            } else {
                Some("l2")
            };
        }
        if mode == "pt" {
            let v = text("pt");
            h = Some(if rx("rapid", "i").test(&v) {
                "rtn"
            } else if rx("frequent|ftn|irtn|strategic", "i").test(&v) {
                "ftn"
            } else if rx("connector", "i").test(&v) {
                "connector"
            } else if rx("peak", "i").test(&v) {
                "peak"
            } else if rx("local", "i").test(&v) {
                "local"
            } else {
                "none"
            });
        }
        if mode == "cycle" {
            let v = text("cycle");
            h = Some(if rx("regional|major", "i").test(&v) {
                "regional"
            } else if rx("connector", "i").test(&v) {
                "connector"
            } else if rx("local|sports", "i").test(&v) {
                "local"
            } else {
                "none"
            });
        }
        if let Some(h) = h {
            add(&mut tally, h, w);
        }
    }
    if fc_len > 0.0 && !tally.is_empty() {
        let mut sorted = tally.clone();
        js_sort_by(&mut sorted, |a, b| b.1 - a.1);
        let best = sorted[0].0.clone();
        let share = if all > 0.0 { fc_len / all } else { 1.0 };
        let src = format!(
            "AT Future Connect{}{}",
            if share < 0.995 {
                format!(
                    " ({}% of the corridor length has Future Connect data; the rest is left out)",
                    number_to_js_string(math_round(share * 100.0))
                )
            } else {
                String::new()
            },
            if road_fill > 0.0 {
                "; road class where Future Connect gives no general traffic class"
            } else {
                ""
            }
        );
        return jobj! { "h" => best, "src" => src };
    }
    if mode == "gt" {
        for id in &ids {
            let l = sc.g("links").g(id);
            if l.t() {
                add(&mut tally, road_h(l), net::len(sc, l));
            }
        }
        let mut sorted = tally.clone();
        js_sort_by(&mut sorted, |a, b| b.1 - a.1);
        return jobj! { "h" => sorted.first().map_or("local", |b| b.0.as_str()), "src" => "road class (Future Connect not loaded)" };
    }
    let h = if mode == "freight" { "l2" } else { "none" };
    jobj! {
        "h" => h, "src" => format!("default {} (Future Connect not loaded; {}: load it in Traffic data)", hier_label(h).s(), assumption()), "assumed" => true,
    }
}

/// `anopPref(mode, h, lu)`: the preferred and acceptable level of service for a hierarchy and land use (the tool's hidden sheet, with the land-use adjustment
/// for freight and general traffic): `{ pref, acc, adj }`, or null when the tool has no row for them.
pub fn anop_pref(mode: &str, h: &str, lu: &str) -> V {
    let row = xdata("PREF").g(mode).g(h);
    if !row.t() {
        return V::Null;
    }
    let mut pref = row.ix(col_of(lu)).s();
    let adj = if mode == "gt" || mode == "freight" {
        if h == "motorway" {
            0.0
        } else if lu == "city-metro" {
            2.0
        } else if lu == "town-local" || lu == "special" {
            1.0
        } else {
            0.0
        }
    } else {
        0.0
    };
    if adj != 0.0 {
        pref = from_score(math_max(1.0, score(&pref) - adj));
    }
    let acc = from_score(math_max(
        1.0,
        score(&pref) - if score(&pref) >= score("B") { 2.0 } else { 1.0 },
    ));
    jobj! { "pref" => pref, "acc" => acc, "adj" => adj }
}

/// `bandSpeed(p)`: the level of service of a speed as a share of the posted limit (%), null for no speed.
fn band_speed(p: Option<f64>) -> Option<String> {
    let p = p?;
    for b in xdata("STD.anop.speedBands").items() {
        if p > b.ix(0).n() {
            return Some(b.ix(1).s());
        }
    }
    Some("F".to_string())
}

/// `bandRel(r)`: the level of service of an 85th percentile to median travel time ratio, null when none was entered.
fn band_rel(r: &V) -> Option<String> {
    if r.nullish() || !(r.n() > 0.0) {
        return None;
    }
    for b in xdata("STD.anop.relBands").items() {
        if r.n() <= b.ix(0).n() {
            return Some(b.ix(1).s());
        }
    }
    Some("F".to_string())
}

/// `worseOf(...ls)`: the worst (lowest scoring) of the levels of service that are set.
fn worse_of(ls: &[Option<String>]) -> Option<String> {
    let mut v: Vec<&String> = ls
        .iter()
        .filter_map(|x| x.as_ref())
        .filter(|x| !x.is_empty())
        .collect();
    js_sort_by(&mut v, |a, b| score(a) - score(b));
    v.first().map(|x| x.to_string())
}

/// The posted limit of a link and where it comes from (`postedOf`): the legal limit, the NSLR limit, the modelled link speed or the road class default.
struct Posted {
    v: f64,
    src: &'static str,
}

fn posted_of(l: &V) -> Posted {
    if l.g("speedLimit").n() > 0.0 {
        Posted {
            v: l.g("speedLimit").n(),
            src: "legal limit",
        }
    } else if l.g("nslr").t() && l.g("nslr").g("v").n() > 0.0 {
        Posted {
            v: l.g("nslr").g("v").n(),
            src: "NSLR",
        }
    } else if l.g("speed").n() > 0.0 {
        Posted {
            v: l.g("speed").n(),
            src: "modelled link speed",
        }
    } else {
        Posted {
            v: super::ctx::class_of(l.g("cls")).g("speed").n(),
            src: "road-class default speed",
        }
    }
}

/// `corridorAxis(sc, ids)`: the corridor's axis: the bearing between its two furthest-apart junctions (None when it has fewer than two or they are 1 m apart or less).
pub fn corridor_axis(sc: &V, ids: &[String]) -> Option<f64> {
    let mut pts: Vec<util::LatLng> = Vec::new();
    for id in ids {
        let l = sc.g("links").g(id);
        if !l.t() {
            continue;
        }
        for n in [l.g("a"), l.g("b")] {
            let x = sc.g("nodes").g(&n.s());
            if x.t() {
                pts.push(net::ll_of(x));
            }
        }
    }
    if pts.len() < 2 {
        return None;
    }
    let far = |from: &util::LatLng| {
        pts.iter().fold(pts[0], |b, p| {
            if util::dist(from, p) > util::dist(from, &b) {
                *p
            } else {
                b
            }
        })
    };
    let a = far(&pts[0]);
    let b = far(&a);
    if util::dist(&a, &b) > 1.0 {
        Some(util::bearing(&a, &b))
    } else {
        None
    }
}

/// `bound(b)`: the compass name of a heading ("Northbound").
fn bound(b: f64) -> String {
    let i = math_round((((b % 360.0) + 360.0) % 360.0) / 90.0) % 4.0;
    ["Northbound", "Eastbound", "Southbound", "Westbound"]
        .get(i as usize)
        .map_or_else(|| "undefined".to_string(), |s| s.to_string())
}

/// `corridorSpeed(sc, res, ids, pt)`: the speed as a share of the posted limit, per direction: `Σlength / Σ(link time incl. junction delay)` against the
/// length-weighted limit (a bus lane at free-flow time when `pt`). `{ spd, limit, pct, dirs [{ name, spd, limit, len, pct }], worst, limitSrc, fallback }` for
/// the worse direction, or null when there is no result; `fallback` is true when a posted limit had to come from the modelled link speed.
pub fn corridor_speed(sc: &V, res: &V, ids: &[String], pt: bool) -> V {
    let axis = corridor_axis(sc, ids);
    let mut acc = [(0.0f64, 0.0f64, 0.0f64); 2];
    let mut srcs: Vec<String> = Vec::new();
    for id in ids {
        let l = sc.g("links").g(id);
        if !l.t() {
            continue;
        }
        let Some(g) = net::geom(sc, l) else { continue };
        let b_ab = util::bearing(&g[0], &g[g.len() - 1]);
        let ps = posted_of(l);
        for k in ["ab", "ba"] {
            let x = res.g("arcs").g(&format!("{id}:{k}"));
            if !x.t() || !(x.g("len").n() > 0.0) {
                continue;
            }
            let b = if k == "ab" {
                b_ab
            } else {
                (b_ab + 180.0) % 360.0
            };
            let i = if axis.is_none_or(|a| util::ang_diff(b, a) <= 90.0) {
                0
            } else {
                1
            };
            acc[i].0 += x.g("len").n();
            acc[i].1 += if pt && l.g("busLane").t() {
                x.g("ff").n()
            } else {
                x.g("t").n()
            };
            acc[i].2 += x.g("len").n() * ps.v;
            if !srcs.iter().any(|s| s == ps.src) {
                srcs.push(ps.src.to_string());
            }
        }
    }
    let mut dirs: Vec<V> = Vec::new();
    for (i, a) in acc.iter().enumerate() {
        if a.1 > 0.0 && a.0 > 0.0 {
            let name = match axis {
                None => "Both directions".to_string(),
                Some(ax) => bound(if i == 1 { (ax + 180.0) % 360.0 } else { ax }),
            };
            let (spd, limit) = (a.0 / a.1 * 3.6, a.2 / a.0);
            dirs.push(jobj! { "name" => name, "spd" => spd, "limit" => limit, "len" => a.0, "pct" => spd / limit * 100.0 });
        }
    }
    if dirs.is_empty() {
        return V::Null;
    }
    let mut sorted: Vec<&V> = dirs.iter().collect();
    js_sort_by(&mut sorted, |a, b| a.g("pct").n() - b.g("pct").n());
    let w = sorted[0].clone();
    let fallback = srcs.iter().any(|x| rx("modelled|default", "").test(x));
    jobj! {
        "spd" => w.g("spd"), "limit" => w.g("limit"), "pct" => w.g("pct"), "dirs" => V::Array(dirs.clone()), "worst" => w.g("name"), "limitSrc" => srcs.join(" and "), "fallback" => fallback,
    }
}

/// `dirText(g, suffix)`: a corridor speed result in words.
fn dir_text(g: &V, suffix: &str) -> String {
    if !g.t() {
        return "no result".to_string();
    }
    let dirs: Vec<String> = g
        .g("dirs")
        .items()
        .iter()
        .map(|d| {
            format!(
                "{} {}% ({} of {} km/h, {})",
                d.g("name").s(),
                super::ctx::num(d.g("pct"), 0),
                super::ctx::num(d.g("spd"), 0),
                super::ctx::num(d.g("limit"), 0),
                band_speed(Some(d.g("pct").n())).unwrap_or_else(|| "null".to_string())
            )
        })
        .collect();
    format!(
        "{}{}{}{}",
        dirs.join("; "),
        if g.g("dirs").jlen() > 1 {
            "; worse direction used"
        } else {
            ""
        },
        suffix,
        if g.g("fallback").t() {
            format!(
                "; posted limit from the {} where no NSLR limit is loaded ({})",
                g.g("limitSrc").s(),
                assumption()
            )
        } else {
            String::new()
        }
    )
}

/// `anopLoS(sc, res, c)`: the level of service of a corridor by mode (`{ gt, freight, pt, walk, cycle }`, each `{ los, metric, ... }`): speed as a share of the
/// posted limit and the author's reliability ratio for general traffic, freight and public transport, crossing spacing and delay for walking, and the cycle
/// facility for cycling.
pub fn anop_los(sc: &V, res: &V, c: &V) -> V {
    let ids = ids_of(c);
    let g = corridor_speed(sc, res, &ids, false);
    let p = corridor_speed(sc, res, &ids, true);
    let rel = if c.g("rel").t() {
        c.g("rel").clone()
    } else {
        V::object()
    };
    let gpct = if g.t() { Some(g.g("pct").n()) } else { None };
    let ppct = if p.t() { Some(p.g("pct").n()) } else { None };
    let rel_text = |k: &str| {
        if band_rel(rel.g(k)).is_some() {
            format!("85th/median {}", rel.g(k).s())
        } else {
            "reliability N/A".to_string()
        }
    };
    let dirs_of = |x: &V| {
        if x.t() {
            x.g("dirs").clone()
        } else {
            V::Array(vec![])
        }
    };
    let gt = jobj! {
        "los" => worse_of(&[band_speed(gpct), band_rel(rel.g("gt"))]), "metric" => dir_text(&g, ""), "dirs" => dirs_of(&g), "rel" => rel_text("gt"),
    };
    let freight = jobj! {
        "los" => worse_of(&[band_speed(gpct), band_rel(rel.g("freight"))]),
        "metric" => if g.t() { format!("{} (general traffic speed)", dir_text(&g, "")) } else { "no result".to_string() }, "dirs" => dirs_of(&g), "rel" => rel_text("freight"),
    };
    let pt = jobj! {
        "los" => worse_of(&[band_speed(ppct), band_rel(rel.g("pt"))]), "metric" => dir_text(&p, &format!(" (bus lanes at free-flow; {})", assumption())), "dirs" => dirs_of(&p), "rel" => rel_text("pt"),
    };
    // walking: crossing opportunity spacing and crossing delay (bands halved in centres and outside schools)
    let mut len = 0.0;
    let mut sig: Vec<String> = Vec::new();
    for id in &ids {
        let l = sc.g("links").g(id);
        if !l.t() {
            continue;
        }
        len += net::len(sc, l);
        for n in [l.g("a"), l.g("b")] {
            if sc.g("nodes").g(&n.s()).g("control").eq_s("signals") {
                sig.push(n.s());
            }
        }
    }
    let sig = unique(sig);
    let crossings = nn(c.g("crossings"));
    let nx = crossings.map_or(sig.len() as f64, |_| c.g("crossings").n());
    let half = if c.g("centre").t() || c.g("school").t() {
        0.5
    } else {
        1.0
    };
    let spacing = len / math_max(1.0, nx + 1.0);
    let letters = xdata("LETTERS");
    let sp = xdata("STD.anop.crossBands")
        .items()
        .iter()
        .position(|b| spacing <= b.n() * half);
    let wl1 = letters.ix(sp.unwrap_or(5)).s();
    let dl: Option<String> = nn(c.g("crossDelay")).map(|d| {
        let i = xdata("STD.anop.crossDelay")
            .items()
            .iter()
            .position(|b| d < b.n())
            .unwrap_or(5);
        letters.ix(i).s()
    });
    let walk = jobj! {
        "los" => worse_of(&[Some(wl1), dl.clone()]),
        "metric" => format!(
            "crossing every {} m ({} crossing{}{}){}", fmt::int(spacing), number_to_js_string(nx), if nx == 1.0 { "" } else { "s" }, if crossings.is_none() { " at signals" } else { "" },
            match &dl { Some(_) => format!(", delay {} s", c.g("crossDelay").s()), None => String::new() }
        ),
    };
    // cycling: facility-based, length-weighted (Spare Construct mapping)
    let (mut cs, mut cl) = (0.0, 0.0);
    let f = xdata("STD.anop.cycleFacility");
    for id in &ids {
        let l = sc.g("links").g(id);
        if !l.t() {
            continue;
        }
        let w = net::len(sc, l);
        let lv = if l.g("cycle").eq_s("separated") {
            f.g("separated").s()
        } else if l.g("cycle").eq_s("painted") {
            f.g("painted").s()
        } else if l.g("cls").eq_s("arterial") {
            f.g("none").g("arterial").s()
        } else {
            f.g("none").g("other").s()
        };
        cs += score(&lv) * w;
        cl += w;
    }
    let cycle = jobj! {
        "los" => if cl != 0.0 && !cl.is_nan() { Some(from_score(cs / cl)) } else { None },
        "metric" => format!("facility-based ({})", f.g("basis").s()),
    };
    jobj! { "gt" => gt, "freight" => freight, "pt" => pt, "walk" => walk, "cycle" => cycle }
}

/// `anopFit(modes)`: the fit rule. `modes` are `[{ base, pref, proj }]` (letters; null = not assessed). "High" when the proposal improves a mode and worsens
/// none, or lowers the total deficit against the preferred level without raising any mode's; "Moderate" when it does either of those; otherwise "Low".
/// None when no mode could be assessed.
pub fn anop_fit(modes: &[V]) -> Option<String> {
    let m: Vec<(f64, f64, f64)> = modes
        .iter()
        .filter(|x| x.g("base").t() && x.g("pref").t() && x.g("proj").t())
        .map(|x| {
            (
                score(&x.g("base").s()),
                score(&x.g("pref").s()),
                score(&x.g("proj").s()),
            )
        })
        .collect();
    if m.is_empty() {
        return None;
    }
    let db: Vec<f64> = m.iter().map(|x| math_max(0.0, x.1 - x.0)).collect();
    let dj: Vec<f64> = m.iter().map(|x| math_max(0.0, x.1 - x.2)).collect();
    let improves = m.iter().any(|x| x.2 > x.0);
    let worsens = m.iter().any(|x| x.2 < x.0);
    let falls = dj.iter().fold(0.0, |s, v| s + v) < db.iter().fold(0.0, |s, v| s + v);
    let no_rise = dj.iter().enumerate().all(|(i, v)| *v <= db[i]);
    if (improves && !worsens) || (falls && no_rise) {
        return Some("High".into());
    }
    if falls || no_rise {
        return Some("Moderate".into());
    }
    Some("Low".into())
}

/// `anopCorridors(P)`: the corridors assessed: the author's list, or the named arterials and collectors at least 300 m long in the active scenario (the 12 longest).
pub fn anop_corridors(p: &V) -> V {
    let cx = data(p);
    let cx = cx.g("anop");
    if cx.g("corridors").jlen() > 0 {
        return cx.g("corridors").clone();
    }
    let sc = active_sc(p);
    let mut by: Vec<(String, Vec<&V>)> = Vec::new();
    for l in sc.g("links").vals() {
        if l.g("name").t()
            && !l.g("status").eq_s("closed")
            && (l.g("cls").eq_s("arterial") || l.g("cls").eq_s("collector"))
        {
            let name = l.g("name").s();
            match by.iter_mut().find(|x| x.0 == name) {
                Some(x) => x.1.push(l),
                None => by.push((name, vec![l])),
            }
        }
    }
    let mut cors: Vec<V> = by
        .into_iter()
        .map(|(name, ls)| {
            let len = ls.iter().fold(0.0, |s, l| s + net::len(sc, l));
            jobj! {
                "id" => format!("auto:{name}"), "name" => name.as_str(), "links" => V::Array(ls.iter().map(|l| l.g("id").clone()).collect()), "landUse" => "other", "walk" => "none",
                "auto" => true, "len" => len,
            }
        })
        .filter(|c| c.g("len").n() >= 300.0)
        .collect();
    js_sort_by(&mut cors, |a, b| b.g("len").n() - a.g("len").n());
    cors.truncate(12);
    V::Array(cors)
}

/// The note given when the do-minimum and the option are the same scenario (`NO_FIT`).
fn no_fit() -> String {
    xs("NO_FIT")
}

/// `anop(P)`: the ANOP assessment of every corridor and period: `{ rows [{ corridor, period, modes [{ mode, label, hier, hierKey, hierSrc, pref, acc, base, proj,
/// defBase, defProj, metric, baseMetric }], fit, fitText, landUse }], base, opt, same, note, label, disclaimer }`.
pub fn anop(p: &V) -> V {
    let pr = pair(p);
    let same = pr.same;
    let mut rows: Vec<V> = Vec::new();
    let labels = xdata("MODE_LABEL");
    let lu_labels = xdata("LU");
    for c in anop_corridors(p).items() {
        for per in periods_of(pr.opt)
            .into_iter()
            .filter(|x| res_of(pr.base, x).is_some())
        {
            let lb = anop_los(pr.base, res_of(pr.base, per).unwrap_or(&NULL), c);
            let lj = anop_los(pr.opt, res_of(pr.opt, per).unwrap_or(&NULL), c);
            let lu = if c.g("landUse").t() {
                c.g("landUse").s()
            } else {
                "other".to_string()
            };
            let mut modes: Vec<V> = Vec::new();
            for m in MODES {
                let hs = hier_of(pr.opt, c, m);
                let h = hs.g("h").s();
                let pf = anop_pref(m, &h, &lu);
                let (base, proj) = (lb.g(m).g("los"), lj.g(m).g("los"));
                let pref = pf.g("pref");
                let defs = |x: &V| {
                    if pref.t() && x.t() {
                        V::Number(math_max(0.0, score(&pref.s()) - score(&x.s())))
                    } else {
                        V::Null
                    }
                };
                let hl = hier_label(&h);
                modes.push(jobj! {
                    "mode" => m, "label" => labels.g(m), "hier" => or_v(&hl, hs.g("h")), "hierKey" => hs.g("h"), "hierSrc" => hs.g("src"), "pref" => or_v(pref, &V::Null), "acc" => or_v(pf.g("acc"), &V::Null),
                    "base" => base, "proj" => proj, "defBase" => defs(base), "defProj" => defs(proj), "metric" => lj.g(m).g("metric"), "baseMetric" => lb.g(m).g("metric"),
                });
            }
            let fit = if same {
                V::Null
            } else {
                anop_fit(&modes).map_or(V::Null, V::String)
            };
            let lu_key = if c.g("landUse").t() {
                c.g("landUse").s()
            } else {
                "other".to_string()
            };
            let mut row = jobj! { "corridor" => c, "period" => per, "modes" => V::Array(modes), "fit" => fit, "fitText" => if same { no_fit() } else { String::new() } };
            // an unknown land use has no label: the field stays out, as an undefined one does
            if lu_labels.has(&lu_key) {
                row.set("landUse", lu_labels.g(&lu_key).clone());
            }
            rows.push(row);
        }
    }
    jobj! {
        "rows" => V::Array(rows), "base" => pr.base.g("name"), "opt" => pr.opt.g("name"), "same" => same, "note" => if same { no_fit() } else { String::new() }, "label" => label("anop"),
        "disclaimer" => super::ctx::disclaimer("D5", p, None),
    }
}

/// `anopCSV(P)`: the ANOP inputs in the order of the tool's v10.2 input sheet (no version stamp: see the export, which adds it).
pub fn anop_csv(p: &V) -> String {
    let cxd = data(p);
    let cx = cxd.g("anop");
    let r = anop(p);
    let mut head: Vec<V> = [
        "project",
        "location",
        "option",
        "period",
        "date",
        "assessor",
        "walk_net",
        "cycle_net",
        "pt_net",
        "freight_net",
        "gt_net",
        "land_use",
    ]
    .iter()
    .map(|s| V::String(s.to_string()))
    .collect();
    for pre in ["base_los_", "manual_pref_", "project_los_", "notes_"] {
        head.extend(MODES.iter().map(|m| V::String(format!("{pre}{m}"))));
    }
    let mut rows: Vec<Vec<V>> = vec![head];
    for x in r.g("rows").items() {
        let mode_of = |m: &str| {
            x.g("modes")
                .items()
                .iter()
                .find(|q| q.g("mode").eq_s(m))
                .unwrap_or(&NULL)
        };
        let mut row: Vec<V> = vec![
            if cx.g("project").t() {
                cx.g("project").clone()
            } else {
                or_s(p.g("name"), "")
            },
            x.g("corridor").g("name").clone(),
            if cx.g("option").t() {
                cx.g("option").clone()
            } else {
                r.g("opt").clone()
            },
            x.g("period").clone(),
            V::String(today()),
            or_s(cx.g("assessor"), ""),
        ];
        row.extend(MODES.iter().map(|m| mode_of(m).g("hier").clone()));
        row.push(x.g("landUse").clone());
        row.extend(MODES.iter().map(|m| or_s(mode_of(m).g("base"), "N/A")));
        row.extend(MODES.iter().map(|_| V::String("Use ANOP".into())));
        row.extend(MODES.iter().map(|m| or_s(mode_of(m).g("proj"), "N/A")));
        row.extend(MODES.iter().map(|m| {
            V::String(format!(
                "{}; indicative Spare Construct estimate",
                mode_of(m).g("metric").s()
            ))
        }));
        rows.push(row);
    }
    csv(&rows)
}

/// `anopTable(a)`: Table ANOP-1: the multimodal level of service and fit of each corridor.
pub fn anop_table(a: &V) -> String {
    let head = [
        th("Spare Construct"),
        th("Period"),
        th("Mode"),
        th("Future Connect"),
        th("Baseline LoS"),
        th("Preferred"),
        th("Acceptable"),
        th("With proposal"),
        th("\u{394} deficit"),
        th("Fit"),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();
    for r in a.g("rows").items() {
        for (i, m) in r.g("modes").items().iter().enumerate() {
            let na = |v: &V, d: &str| esc(&or_s(v, d));
            rows.push(vec![
                if i > 0 {
                    String::new()
                } else {
                    format!(
                        "{}<br><span class=\"note\">{}</span>",
                        esc(r.g("corridor").g("name")),
                        esc(r.g("landUse"))
                    )
                },
                if i > 0 {
                    String::new()
                } else {
                    esc(r.g("period"))
                },
                esc(m.g("label")),
                format!(
                    "{}<br><span class=\"note\">{}</span>",
                    esc(m.g("hier")),
                    esc(&or_s(m.g("hierSrc"), ""))
                ),
                na(m.g("base"), "N/A"),
                na(m.g("pref"), fmt::DASH),
                na(m.g("acc"), fmt::DASH),
                format!(
                    "{} <span class=\"note\">{}</span>",
                    na(m.g("proj"), "N/A"),
                    esc(m.g("metric"))
                ),
                if !m.g("defBase").nullish() && !m.g("defProj").nullish() {
                    fmt::signed(m.g("defProj").n() - m.g("defBase").n(), 0)
                } else {
                    fmt::DASH.to_string()
                },
                if i > 0 {
                    String::new()
                } else {
                    format!(
                        "<b>{}</b>",
                        esc(&if r.g("fit").t() {
                            r.g("fit").clone()
                        } else {
                            or_s(r.g("fitText"), fmt::DASH)
                        })
                    )
                },
            ]);
        }
    }
    tbl_th(
        &head,
        rows,
        "Run the do-minimum and the option to assess ANOP.",
    )
}

fn report_anop(p: &V, ctx: Option<&dyn ReportCtx>) -> String {
    let a = anop(p);
    if a.g("rows").jlen() == 0 {
        return String::new();
    }
    let note = format!(
        "{} Speeds per direction; the worse direction sets the LoS.{}",
        esc(a.g("label")),
        if a.g("same").t() {
            format!(" {}.", esc(a.g("note")))
        } else {
            String::new()
        }
    );
    format!(
        "{}{}",
        rt(
            ctx,
            "Table ANOP-1 \u{b7} Multimodal level of service and fit",
            &anop_table(&a),
            &note
        ),
        disc("D5", p, None)
    )
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-anop"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id == "compliance-extra-anop" {
        Some(report_anop(p, ctx))
    } else {
        None
    }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-anop"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's. The export is the ANOP inputs stamped as a hand-over
/// file for the AT ANOP tool v10.2.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    if id == "compliance-extra-anop" {
        Some(stamped(&anop_csv(p), p, Some("the AT ANOP tool v10.2")))
    } else {
        None
    }
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    if id == "compliance-extra-anop" {
        Some(anop(p).g("rows").jlen() > 0)
    } else {
        None
    }
}
