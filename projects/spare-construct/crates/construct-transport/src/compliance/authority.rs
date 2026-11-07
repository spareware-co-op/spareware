//! Who controls a road, who decides on it, and what the site is (packs/nz/compliance.js sections 4 and 5): the authority table from the
//! lodgement date (Auckland Council became the road controlling authority on 7 May 2026), the state highway status of a link, the
//! decision-maker for each road, and the site's zone, precinct and Future Development Strategy area from the lookup or the author's
//! entries.

use super::data::c as table;
use super::dates::{long_date, today, valid_day};
use super::geom::{flat_at, line_to_polys, to_line};
use super::jv::{Jx, V, jobj, merge, slice, st};
use super::net;
use super::record::{in_auckland, pathway_entry, settings, state};
use super::rx::rx;
use crate::util::LatLng;
use std::collections::HashSet;

/// `parsePrecincts(text)`: the extra transport precincts the author added in Settings, one per line as "I123 | Name | rule".
pub fn parse_precincts(text: &V) -> V {
    let t = if text.t() { text.s() } else { String::new() };
    let esc_re = rx(r"[.*+?^${}()|[\]\\]", "g");
    let mut out: Vec<V> = Vec::new();
    for line in rx(r"\n", "").split(&t) {
        let a: Vec<String> = line
            .split('|')
            .map(|s| super::jv::trim(s).to_string())
            .collect();
        let (id, name) = (
            a.first().cloned().unwrap_or_default(),
            a.get(1).cloned().unwrap_or_default(),
        );
        if id.is_empty() || name.is_empty() {
            continue;
        }
        let rule = a.get(2).cloned().unwrap_or_default();
        out.push(jobj! { "id" => id.as_str(), "name" => name.as_str(), "match" => format!("^{}", esc_re.replace(&name, |m| format!("\\{}", m.text()))),
            "rule" => if rule.is_empty() { format!("{id} standards") } else { rule }, "ita" => format!("{id}.9"),
            "source" => jobj! { "name" => format!("{id} {name} (added in Settings)"), "url" => "", "verified" => false } });
    }
    V::Array(out)
}

/// `siteInfo(P)`: the site's zone, precinct, PC120 zoning and FDS area: the author's entries win over the lookup. `{ zone, precinct,
/// transportPrecinct, cfoc, pc120, fds, board, designations, looked, at }`.
pub fn site_info(p: &V) -> V {
    let c = state(p);
    let l = c.g("site").g("lookup");
    let l = if l.t() { l.clone() } else { V::object() };
    let s = c.g("site");
    let zone: V = if s.g("zoneOverride").t() {
        jobj! { "name" => s.g("zoneOverride"), "src" => "entered" }
    } else if l.g("zone").t() {
        merge(l.g("zone"), &jobj! { "src" => "AUP base zone layer" })
    } else {
        V::Null
    };
    let first_precinct = l
        .g("precincts")
        .items()
        .first()
        .map(|p| p.g("name").clone())
        .unwrap_or(V::Null);
    let precinct_name = if s.g("precinctOverride").t() {
        s.g("precinctOverride").s()
    } else if first_precinct.t() {
        first_precinct.s()
    } else {
        String::new()
    };
    let mut list: Vec<V> = table("PRECINCTS").items().to_vec();
    list.extend(
        parse_precincts(settings(p).g("precincts"))
            .items()
            .iter()
            .cloned(),
    );
    let tp = if !precinct_name.is_empty() {
        list.iter()
            .find(|q| rx(&q.g("match").s(), "i").test(&precinct_name))
            .cloned()
            .unwrap_or(V::Null)
    } else {
        V::Null
    };
    let pc120: V = if s.g("pc120Override").t() {
        jobj! { "zone" => s.g("pc120Override"), "src" => "entered" }
    } else if l.g("pc120").t() {
        merge(l.g("pc120"), &jobj! { "src" => table("LAYERS.pc120.name") })
    } else {
        V::Null
    };
    // FDS area: the author's entry from the strategy document (with its page) wins over the live layer
    let fo = if s.g("fdsOverride").t() {
        s.g("fdsOverride").clone()
    } else {
        V::object()
    };
    let fds: V = if fo.g("name").t() {
        let or = |a: &V, b: &V| if a.t() { a.clone() } else { b.clone() };
        jobj! { "name" => fo.g("name"), "timing" => or(fo.g("timing"), &st("")), "source" => or(fo.g("source"), table("SRC.fds.name")), "page" => or(fo.g("page"), &st("")), "src" => "entered" }
    } else if l.g("fds").t() {
        merge(l.g("fds"), &jobj! { "src" => table("LAYERS.fds.name") })
    } else {
        V::Null
    };
    jobj! { "zone" => zone, "precinct" => if precinct_name.is_empty() { V::Null } else { st(precinct_name) }, "transportPrecinct" => tp, "cfoc" => l.g("cfoc").t(), "pc120" => pc120, "fds" => fds,
    "board" => if l.g("board").t() { l.g("board").clone() } else { V::Null }, "designations" => if l.g("designations").t() { l.g("designations").clone() } else { V::Array(vec![]) },
    "looked" => c.g("site").g("lookup").t(), "at" => if l.g("at").t() { l.g("at").clone() } else { V::Null } }
}

/// `authorities(P, now)`: the reviewer and approver of each part of the network on the lodgement date (or today): the road controlling
/// authority, the local board and the transport CCO after 7 May 2026, NZTA for state highways and KiwiRail for rail; the transitional
/// period, the requiring authority for a notice of requirement, and the wording for reports.
pub fn authorities(p: &V, now: &V) -> V {
    let c = state(p);
    let ctx = c.g("ctx");
    let d = if valid_day(ctx.g("lodgement")) {
        slice(&ctx.g("lodgement").s(), 0, Some(10))
    } else {
        today(now)
    };
    let site = site_info(p);
    let auth = table("AUTH");
    let mut rows: Vec<V> = Vec::new();
    let transfer = auth.g("transfer").s();
    if !in_auckland(p) {
        rows.push(jobj! { "role" => "Road controlling authority", "name" => auth.g("other").g("rca"), "basis" => "Local Government Act 1974 / Land Transport Act" });
    } else if d.as_str() < transfer.as_str() {
        rows.push(jobj! { "role" => "Road controlling authority", "name" => auth.g("before").g("rca"), "basis" => "LGACA 2009 (before 7 May 2026)" });
    } else {
        rows.push(jobj! { "role" => "Road controlling authority", "name" => auth.g("after").g("rca"), "basis" => "LGACA 2009 s47 (from 7 May 2026)" });
        rows.push(jobj! { "role" => "Local and collector roads (speed, signals, crossings, closures)",
            "name" => if site.g("board").t() { st(format!("{} Local Board (LGACA s47C, Sch 4)", site.g("board").s())) } else { auth.g("after").g("board").clone() }, "basis" => "LGACA s47C, Sch 4" });
        rows.push(jobj! { "role" => "Public transport", "name" => auth.g("after").g("cco"), "basis" => "LGACA s43, s43A" });
    }
    rows.push(jobj! { "role" => "State highways", "name" => auth.g("nzta"), "basis" => "Land Transport Management Act 2003; GRPA 1989" });
    rows.push(jobj! { "role" => "Rail and level crossings", "name" => auth.g("kiwirail"), "basis" => "Railways Act 2005" });
    let mut ends: Vec<String> = vec![auth.g("transitionalEnd").s()];
    if valid_day(ctx.g("cl82Date")) {
        ends.push(ctx.g("cl82Date").s());
    }
    ends.sort();
    let end_by = ends[0].clone();
    let transitional =
        in_auckland(p) && d.as_str() >= transfer.as_str() && d.as_str() < end_by.as_str();
    let mut requiring = V::Null;
    if in_auckland(p) && pathway_entry(ctx.g("pathway")).is_some_and(|e| e.g("proc").eq_s("nor")) {
        let t = if valid_day(ctx.g("transferDate")) {
            ctx.g("transferDate").s()
        } else {
            String::new()
        };
        requiring = st(if t.is_empty() {
            "For former Auckland Transport designations: the transport CCO until the transfer date under LGACA Sch 1AA cl 10 (not entered yet), then Auckland Council.".to_string()
        } else if d < t {
            format!(
                "For former Auckland Transport designations: the transport CCO (Auckland Transport) until {} (Sch 1AA cl 10).",
                long_date(&t)
            )
        } else {
            format!(
                "For former Auckland Transport designations: Auckland Council, since {} (Sch 1AA cl 10).",
                long_date(&t)
            )
        });
    }
    let arcf = auth.g("arcf");
    jobj! { "date" => d.as_str(), "rows" => V::Array(rows), "transitional" => transitional, "transitionalEnd" => end_by.as_str(),
    "transitionalText" => if transitional { format!("Transitional period: Auckland Transport keeps transitional functions until {} (the earlier of 7 Nov 2026 and any cl 8(2) direction). Confirm whether a direction has ended it early.", long_date(&end_by)) } else { String::new() },
    "requiring" => requiring,
    "arcf" => format!("{}: {}. Road classes for local-board decisions use an interim source until it is.", arcf.g("name").s(), arcf.g("warning").s()),
    "reportName" => if d.as_str() >= transfer.as_str() && in_auckland(p) { auth.g("reportName").clone() } else { st("Auckland Transport (road controlling authority)") },
    "disclaimer" => table("DISCLAIMERS.D11.text") }
}

/// `shOf(P, L)`: whether a link is a state highway and why: the author's tick, the NZTA layer (a matched link with a blank highway number
/// still counts), or the motorway or ramp class. `{ sh, ref, basis }`.
pub fn sh_of(p: &V, l: &V) -> V {
    let c = state(p);
    let n = c.g("net");
    let id = l.g("id").s();
    let has = n.g("sh").has(&id);
    let tag = if has {
        let t = n.g("sh").g(&id);
        if t.t() { t.s() } else { String::new() }
    } else {
        String::new()
    };
    let user = n.g("shUser").g(&id);
    let sh_ref = if tag.is_empty() {
        "SH".to_string()
    } else {
        format!("SH{tag}")
    };
    if user.strict_eq(&V::Bool(true)) {
        return jobj! { "sh" => true, "ref" => sh_ref, "basis" => "marked by the user" };
    }
    if user.strict_eq(&V::Bool(false)) {
        return jobj! { "sh" => false, "basis" => "marked not a state highway by the user" };
    }
    if has {
        return jobj! { "sh" => true, "ref" => sh_ref, "basis" => "NZTA SH_Network layer (conflated)" };
    }
    if l.g("cls").eq_s("motorway") || l.g("cls").eq_s("ramp") {
        return jobj! { "sh" => true, "ref" => "SH (motorway)", "basis" => "motorway or ramp road class (assumed NZTA)" };
    }
    jobj! { "sh" => false, "basis" => if n.g("loaded").g("sh").t() { "not on the NZTA layer" } else { "NZTA layer not loaded" } }
}

/// The project facts [`link_authority_with`] needs, computed once rather than once per link.
pub struct AuthCtx {
    /// The authority table.
    pub a: V,
    /// The site.
    pub site: V,
    /// In Auckland.
    pub auck: bool,
    /// In Auckland and on or after the transfer date.
    pub post: bool,
    /// The waypoints of each bus route of the scenario.
    pub routes: Vec<HashSet<String>>,
}

/// `authCtx(P, sc)`.
pub fn auth_ctx(p: &V, sc: &V) -> AuthCtx {
    let a = authorities(p, &V::Null);
    let auck = in_auckland(p);
    let post = auck && a.g("date").s().as_str() >= table("AUTH.transfer").s().as_str();
    let routes = sc
        .g("routes")
        .vals()
        .iter()
        .map(|r| r.g("wp").items().iter().map(|w| w.s()).collect())
        .collect();
    AuthCtx {
        a,
        site: site_info(p),
        auck,
        post,
        routes,
    }
}

/// `levelCrossingDist(P, sc, L)`: the shortest distance (m) from anywhere on the link to a level-crossing control area or VAR line, None when
/// the AUP controls are not loaded or none is near.
pub fn level_crossing_dist(p: &V, sc: &V, l: &V) -> Option<f64> {
    let c = state(p);
    let o = c.g("overlays");
    let has_var_level = o
        .g("var")
        .items()
        .iter()
        .any(|v| rx("Level", "").test(&v.g("type").s()));
    if !o.t() || !(o.g("lx").jlen() > 0 || has_var_level) {
        return None;
    }
    let g = net::geom(sc, l)?;
    let f = flat_at(g[0][0]);
    let lines: Vec<Vec<LatLng>> = o
        .g("var")
        .items()
        .iter()
        .filter(|v| rx("Level", "").test(&v.g("type").s()))
        .map(|v| pts_of(v.g("pts")))
        .collect();
    let lx: Vec<Vec<LatLng>> = o.g("lx").items().iter().map(pts_of).collect();
    let d = crate::jsnum::math_min(
        line_to_polys(&g, &lx, &f, 200.0, false),
        line_to_polys(&g, &lines, &f, 200.0, true),
    );
    if d.is_finite() { Some(d) } else { None }
}

/// A stored polyline as points.
pub fn pts_of(v: &V) -> Vec<LatLng> {
    v.items().iter().map(net::pt).collect()
}

/// `linkAuthority(P, L, sc)`: the operator of a link (NZTA for a state highway, otherwise the road controlling authority), KiwiRail where it
/// is within 30 m of a level crossing, the transport CCO for a bus route, and who decides on speed, signals, crossings and closures.
pub fn link_authority(p: &V, l: &V, sc: &V) -> V {
    link_authority_with(p, l, sc, &auth_ctx(p, sc))
}

/// [`link_authority`] with the project facts already worked out (for many links).
pub fn link_authority_with(p: &V, l: &V, sc: &V, x: &AuthCtx) -> V {
    let auth = table("AUTH");
    let sh = sh_of(p, l);
    let mut also: Vec<V> = Vec::new();
    if let Some(d) = level_crossing_dist(p, sc, l) {
        if d <= auth.g("levelCrossingM").n() {
            also.push(auth.g("kiwirail").clone());
        }
    }
    let on_route = l.g("busLane").t()
        || x.routes
            .iter()
            .any(|r| r.contains(&l.g("a").s()) && r.contains(&l.g("b").s()));
    if x.auck && x.post && on_route {
        also.push(auth.g("after").g("cco").clone());
    }
    if sh.g("sh").t() {
        return jobj! { "operator" => "nzta", "name" => auth.g("nzta"), "sh" => sh, "also" => V::Array(also), "decision" => jobj! { "who" => auth.g("nzta"), "basis" => "State highway" } };
    }
    let name = if !x.auck {
        auth.g("other").g("rca")
    } else if x.post {
        auth.g("after").g("rca")
    } else {
        auth.g("before").g("rca")
    };
    let art = state(p).g("net").g("aupArterial").g(&l.g("id").s()).clone();
    let cls = if art.t() {
        st("arterial")
    } else {
        l.g("cls").clone()
    };
    let basis = if art.t() {
        "AUP planning-map arterial overlay (interim: the s48 framework is not yet published)"
    } else {
        "Spare Construct road class (interim: the s48 framework is not yet published)"
    };
    let board = x.auck && x.post && (cls.eq_s("local") || cls.eq_s("collector"));
    let who: V = if board {
        st(format!(
            "{} (LGACA s47C, Sch 4)",
            if x.site.g("board").t() {
                format!("{} Local Board", x.site.g("board").s())
            } else {
                "The local board".to_string()
            }
        ))
    } else if x.auck && x.post {
        st("Auckland Council governing body")
    } else {
        name.clone()
    };
    jobj! { "operator" => "rca", "name" => name, "sh" => sh, "also" => V::Array(also), "decision" => jobj! { "who" => who, "basis" => basis, "cls" => cls } }
}

/// `alsoKey(n)`.
pub fn also_key(n: &V) -> &'static str {
    if n.strict_eq(table("AUTH.kiwirail")) {
        "kiwirail"
    } else {
        "cco"
    }
}

/// `sitePoint(P, sc)`: where the site is: the author's point, the centre of the development zones, or the centre of the network.
/// `{ lat, lng, src }`, None for an empty project.
pub fn site_point(p: &V, sc: &V) -> Option<V> {
    let c = state(p);
    let s = c.g("site");
    if s.g("lat").finite() && s.g("lng").finite() && !s.g("lat").nullish() {
        return Some(
            jobj! { "lat" => s.g("lat").n(), "lng" => s.g("lng").n(), "src" => if s.g("src").t() { s.g("src").clone() } else { st("set by the user") } },
        );
    }
    let dev = net::dev_zones(sc);
    if !dev.is_empty() {
        let n = dev.len() as f64;
        let lat = dev.iter().fold(0.0, |a, z| a + z.g("lat").n()) / n;
        let lng = dev.iter().fold(0.0, |a, z| a + z.g("lng").n()) / n;
        return Some(
            jobj! { "lat" => lat, "lng" => lng, "src" => format!("centre of {} development zone{}", dev.len(), if dev.len() == 1 { "" } else { "s" }) },
        );
    }
    super::geom::bounds_of(sc).map(|(s, w, n, e)| jobj! { "lat" => (s + n) / 2.0, "lng" => (w + e) / 2.0, "src" => "centre of the network" })
}

/// `toLine` for a site point and a link, for callers that need the distance.
pub fn dist_to_link(sc: &V, l: &V, ll: &LatLng) -> f64 {
    let f = flat_at(ll[0]);
    net::geom(sc, l).map_or(f64::INFINITY, |g| to_line(ll, &g, &f).d)
}
