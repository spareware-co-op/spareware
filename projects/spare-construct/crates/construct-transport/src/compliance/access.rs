//! Access and frontage (AUP E27.6.4), state highways and NZTA approvals, and the affected operators (packs/nz/compliance.js sections 4,
//! 8 and 9).

use super::authority::{AuthCtx, auth_ctx, level_crossing_dist, link_authority_with, pts_of, sh_of, site_info};
use super::data::{c as table, m as model};
use super::e27::sh_involved;
use super::geom::{flat_at, to_line, to_polys};
use super::jv::{Jx, V, jarr, jobj, ns, st, strs};
use super::net;
use super::record::{active_sc, base_sc, num, numf, res_of, settings, state};
use super::rx::rx;
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, math_min, truthy_number};
use crate::util::{self, LatLng, fmt};
use std::collections::HashMap;

fn or0(v: &V) -> f64 {
    let x = v.n();
    if truthy_number(x) { x } else { 0.0 }
}

fn class_label(cls: &V) -> String {
    let t = model("CLASSES");
    let k = cls.s();
    if cls.t() && t.has(&k) { t.g(&k).g("label").s() } else { t.g("local").g("label").s() }
}

/// `degrees(sc)`: the open links at each node.
pub fn degrees(sc: &V) -> HashMap<String, usize> {
    let mut m: HashMap<String, usize> = HashMap::new();
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("closed") {
            continue;
        }
        *m.entry(l.g("a").s()).or_insert(0) += 1;
        if !l.g("b").strict_eq(l.g("a")) {
            *m.entry(l.g("b").s()).or_insert(0) += 1;
        }
    }
    m
}

/// `nearestLink(sc, ll, f)`: the nearest open road; among roads within 1 m of the nearest, an existing one wins. (link, distance).
pub fn nearest_link<'a>(sc: &'a V, ll: &LatLng) -> Option<(&'a V, f64)> {
    let f = flat_at(ll[0]);
    let mut all: Vec<(&V, f64)> = Vec::new();
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("closed") || !sc.g("nodes").has(&l.g("a").s()) || !sc.g("nodes").has(&l.g("b").s()) {
            continue;
        }
        if let Some(g) = net::geom(sc, l) {
            all.push((l, to_line(ll, &g, &f).d));
        }
    }
    if all.is_empty() {
        return None;
    }
    let d_min = all.iter().fold(f64::INFINITY, |a, x| math_min(a, x.1));
    let mut near: Vec<(&V, f64)> = all.into_iter().filter(|x| x.1 <= d_min + 1.0).collect();
    if let Some(x) = near.iter().find(|x| !x.0.g("status").eq_s("proposed")) {
        return Some(*x);
    }
    js_sort_by(&mut near, |a, b| a.1 - b.1);
    near.first().copied()
}

/// `accessPoint(sc, a)`: where an access is: its junction, or its own position.
pub fn access_point(sc: &V, a: &V) -> Option<LatLng> {
    if a.g("node").t() && sc.g("nodes").has(&a.g("node").s()) {
        return Some(net::ll_of(sc.g("nodes").g(&a.g("node").s())));
    }
    if a.g("lat").finite() && a.g("lng").finite() && !a.g("lat").nullish() {
        return Some([a.g("lat").n(), a.g("lng").n()]);
    }
    None
}

/// `accessGeo(P, sc, a)`: geometry facts for an access point: nearest intersection, frontage road, mapped controls within 15 m and the level crossing. Null without a position.
pub fn access_geo(p: &V, sc: &V, a: &V) -> V {
    let Some(ll) = access_point(sc, a) else { return V::Null };
    let f = flat_at(ll[0]);
    let c = state(p);
    let o = c.g("overlays");
    let near_m = table("ACCESS.nearM").n();
    let mut out = jobj! { "ll" => jarr![ll[0], ll[1]] };
    let deg = degrees(sc);
    let mut d_i = f64::INFINITY;
    for n in sc.g("nodes").vals() {
        if n.g("id").strict_eq(a.g("node")) || deg.get(&n.g("id").s()).copied().unwrap_or(0) < 3 {
            continue;
        }
        let d = util::dist(&ll, &net::ll_of(n));
        if d < d_i {
            d_i = d;
            out.set("intersection", n.g("id").clone());
        }
    }
    out.set("intersectionDist", if d_i.is_finite() { V::Number(d_i) } else { V::Null });
    let pick = if a.g("frontageLink").t() && sc.g("links").has(&a.g("frontageLink").s()) && !sc.g("links").g(&a.g("frontageLink").s()).g("status").eq_s("closed") { Some(sc.g("links").g(&a.g("frontageLink").s())) } else { None };
    let nl: Option<(&V, f64)> = match pick {
        Some(l) => net::geom(sc, l).map(|g| (l, to_line(&ll, &g, &f).d)),
        None => nearest_link(sc, &ll),
    };
    out.set("link", nl.map_or(V::Null, |x| x.0.clone()));
    out.set("linkDist", nl.map_or(V::Null, |x| V::Number(x.1)));
    out.set("linkPicked", V::Bool(pick.is_some()));
    let mut controls: Vec<String> = Vec::new();
    let mut arterial: Option<bool> = None;
    if o.t() {
        for v in o.g("var").items() {
            if to_line(&ll, &pts_of(v.g("pts")), &f).d <= near_m {
                controls.push(format!("VAR {}", v.g("type").s()));
            }
        }
        for v in o.g("frontage").items() {
            if to_line(&ll, &pts_of(v.g("pts")), &f).d <= near_m {
                controls.push(v.g("type").s());
            }
        }
        arterial = Some(o.g("arterials").items().iter().any(|q| to_line(&ll, &pts_of(q), &f).d <= near_m));
        let lx: Vec<Vec<LatLng>> = o.g("lx").items().iter().map(pts_of).collect();
        let mut dx = to_polys(&ll, &lx, &f);
        for v in o.g("var").items() {
            if rx("Level", "").test(&v.g("type").s()) {
                dx = math_min(dx, to_line(&ll, &pts_of(v.g("pts")), &f).d);
            }
        }
        out.set("lxDist", if dx.is_finite() { V::Number(dx) } else { V::Null });
    }
    let mut art = arterial.unwrap_or(false);
    if !art && nl.is_some() && c.g("net").g("aupArterial").g(&nl.unwrap().0.g("id").s()).t() {
        art = true;
    }
    if arterial.is_some() || art {
        out.set("arterial", V::Bool(art));
    }
    let mut uniq: Vec<String> = Vec::new();
    for x in controls {
        if !uniq.contains(&x) {
            uniq.push(x);
        }
    }
    out.set("controls", strs(uniq));
    out
}

/// `zoneGroup(name)`: the zone group the access width table uses.
pub fn zone_group(name: &V) -> V {
    if !name.t() {
        return V::Null;
    }
    let n = name.s();
    st(if rx("^Residential", "i").test(&n) || rx("Large Lot|Rural and Coastal Settlement", "i").test(&n) {
        "residential"
    } else if rx("^Rural|Future Urban", "i").test(&n) {
        "rural"
    } else if rx("General Business|Business Park|Industry", "i").test(&n) {
        "business"
    } else {
        "centres"
    })
}

fn val(a: &V, k: &str) -> Option<f64> {
    if a.g(k).entered() { Some(a.g(k).n()) } else { None }
}

/// `accessCheck(P, a, sc)`: the E27.6.4 check of one access: `{ id, name, geo, flags, worst, unchecked, uncheckedText, max, basis, frontageLink, sh, evidence }`.
pub fn access_check(p: &V, a: &V, sc: &V) -> V {
    let c = state(p);
    let gv = access_geo(p, sc, a);
    let g = if gv.t() { gv.clone() } else { V::object() };
    let site = site_info(p);
    let mut flags: Vec<V> = Vec::new();
    let city_centre = site.g("zone").t() && rx("City Centre Zone", "i").test(&site.g("zone").g("name").sn());
    let d_int = val(a, "intersectionDist").or(if g.g("intersectionDist").nullish() { None } else { Some(g.g("intersectionDist").n()) });
    let d_lx = val(a, "lxDist").or(if g.g("lxDist").nullish() { None } else { Some(g.g("lxDist").n()) });
    let mut controls: Vec<String> = g.g("controls").items().iter().map(|x| x.s()).collect();
    let addc = |s: String, controls: &mut Vec<String>| {
        if !controls.contains(&s) {
            controls.push(s);
        }
    };
    if a.g("var").t() {
        addc(format!("VAR {}", a.g("var").s()), &mut controls);
    }
    if a.g("krf").t() {
        addc("Key Retail Frontage".into(), &mut controls);
    }
    if a.g("gcf").t() {
        addc("General Commercial Frontage".into(), &mut controls);
    }
    let art_by: Option<bool> = if matches!(a.g("arterial"), V::Bool(true)) || a.g("arterial").eq_s("yes") { Some(true) } else if matches!(a.g("arterial"), V::Bool(false)) || a.g("arterial").eq_s("no") { Some(false) } else { None };
    let arterial = art_by.unwrap_or_else(|| g.g("arterial").t());
    let mut add = |rule: &str, status: &str, text: String| flags.push(jobj! { "rule" => rule, "status" => status, "text" => text });
    let acc = table("ACCESS");
    let krf = controls.iter().any(|x| x == "Key Retail Frontage");
    let cc_var = city_centre && controls.iter().any(|x| x == "VAR General");
    if krf || cc_var {
        let ex = a.g("exception").eq_s("relocate") || a.g("exception").eq_s("only");
        let what = if krf { "Key Retail Frontage" } else { "City Centre VAR General" };
        if a.g("existing").t() {
            add("E27.6.4.1(1)(b)(i)", "RD", format!("{what}: use of an existing crossing for a new or intensified activity is restricted discretionary (A6)."));
        } else if ex {
            add("E27.6.4.1(1)", "RD", format!("New crossing on a {what} to {}: restricted discretionary (A7).", if a.g("exception").eq_s("only") { "give the only means of access" } else { "relocate or amalgamate crossings" }));
        } else {
            add("E27.6.4.1(1)", "NC", format!("New crossing on a {what}: non-complying (A8)."));
        }
    }
    let int_m = acc.g("intersectionM").n();
    let lx_m = acc.g("levelCrossingM").n();
    if let Some(d) = d_int {
        if d < int_m {
            add("E27.6.4.1(3)(a)", "RD", format!("{} m from the nearest intersection (under 10 m): restricted discretionary (A5).", numf(d, 1)));
        }
    }
    for cname in &controls {
        if cname.starts_with("VAR ") && !(city_centre && cname == "VAR General") {
            add("E27.6.4.1(3)(b)", "RD", format!("Mapped Vehicle Access Restriction ({}): restricted discretionary (A5).", &cname[4..]));
        }
    }
    if arterial {
        add("E27.6.4.1(3)(c)", "RD", "Frontage to an arterial road on the AUP planning maps: restricted discretionary (A5).".into());
    }
    if let Some(d) = d_lx {
        if d < lx_m {
            add("E27.6.4.1(3)(d)", "RD", format!("{} m from a level crossing (under 30 m from the limit line): restricted discretionary (A5). KiwiRail is an affected operator.", numf(d, 1)));
        }
    }
    let frontage = val(a, "frontage");
    let n = val(a, "crossings");
    let mut max: Option<f64> = None;
    let mut basis = String::new();
    if let Some(fr) = frontage {
        if krf || cc_var {
            max = Some(0.0);
            basis = "no crossings permitted (T143)".into();
        } else if !controls.is_empty() || arterial || d_int.is_some_and(|d| d < int_m) || d_lx.is_some_and(|d| d < lx_m) || controls.iter().any(|x| x == "General Commercial Frontage") {
            max = Some((fr / acc.g("perM").g("restricted").n()).ceil());
            basis = "1 per 50 m of frontage or part thereof (T144)".into();
        } else {
            max = Some((fr / acc.g("perM").g("normal").n()).ceil());
            basis = "1 per 25 m of frontage or part thereof (T146)".into();
        }
        if let (Some(n), Some(m)) = (n, max) {
            if n > m {
                add("Table E27.6.4.2.1", "RD", format!("{} crossings on a {} m frontage: breach (maximum {}, {}).", ns(n), numf(fr, 0), ns(m), basis));
            }
        }
    }
    let sep_adj = val(a, "sepAdj");
    let sep_same = val(a, "sepSame");
    let comb = val(a, "combinedWidth");
    if let Some(s) = sep_adj {
        if s < acc.g("sepAdj").n() && !comb.is_some_and(|c| c <= acc.g("combined").n()) {
            add("Table E27.6.4.2.1", "RD", format!("{} m from a crossing on the adjacent site: breach (minimum 2 m unless the combined width is 6 m or less).", numf(s, 1)));
        }
    }
    if let Some(s) = sep_same {
        if s < acc.g("sepSame").n() {
            add("Table E27.6.4.2.1", "RD", format!("{} m between crossings on the same site: breach (minimum 6 m).", numf(s, 1)));
        }
    }
    let zg = if a.g("zoneGroup").t() { a.g("zoneGroup").clone() } else { let z = zone_group(site.g("zone").g("name")); if z.t() { z } else { st("centres") } };
    let grp = zg.s();
    let spaces = val(a, "spaces");
    let width = val(a, "width");
    let formed = val(a, "formed");
    let widths = if acc.g("widths").has(&grp) { acc.g("widths").g(&grp) } else { acc.g("widths").g("centres") };
    let band = spaces.and_then(|s| widths.items().iter().find(|b| s <= b.g("to").n()));
    if let (Some(b), Some(w)) = (band, width) {
        let mx = if a.g("hv").t() && b.g("hv").t() { b.g("hv").n() } else { b.g("max").n() };
        if w < b.g("min").n() || w > mx {
            add(&format!("Table E27.6.4.3.2 ({})", b.g("ref").s()), "RD", format!("Crossing width {} m: outside {}\u{2013}{} m for {} parking spaces ({}).", numf(w, 2), numf(b.g("min").n(), 2), numf(mx, 2), ns(spaces.unwrap_or(f64::NAN)), grp));
        }
    }
    if let (Some(b), Some(f)) = (band, formed) {
        if b.g("formed").t() && f < b.g("formed").n() {
            add(&format!("Table E27.6.4.3.2 ({})", b.g("ref").s()), "RD", format!("Formed access width {} m: under the {} m minimum.", numf(f, 2), numf(b.g("formed").n(), 2)));
        }
    }
    if let Some(len) = val(a, "length") {
        let rural = grp == "rural";
        let pb = if rural { acc.g("passing").g("rural") } else { acc.g("passing").g("other") };
        let needs = if rural { len > pb.g("len").n() && formed.is_none_or(|f| f < pb.g("width").n()) } else { len > pb.g("len").n() && !formed.is_some_and(|f| f >= 5.5) };
        if needs && !a.g("passingBays").t() {
            add(&format!("Table E27.6.4.3.1 ({})", pb.g("ref").s()), "RD", format!("Access {} m long{}: passing bays needed at no more than {} m intervals{}. Tick 'passing bays provided' if they are.", numf(len, 0), if rural { " and under 5.5 m wide" } else { "" }, pb.g("every").s(), if rural { "" } else { " (Spare Construct reading: not needed where the formed width is already 5.5 m)" }));
        }
    }
    if let (Some(q95), Some(stack)) = (val(a, "q95"), val(a, "stacking")) {
        if q95 > stack {
            add("E27.6.4.3(2)", "RD", format!("Queues into road reserve: E27.6.4.3(2) (95th-percentile queue {} m against {} m stacking). Breach, restricted discretionary; matters of discretion E27.8.1(9).", numf(q95, 1), numf(stack, 1)));
        }
    }
    let o = c.g("overlays");
    let layers = table("LAYERS");
    let loaded = |k: &str| o.t() && o.g("sources").items().iter().any(|s| s.g("source").strict_eq(layers.g(k).g("name")));
    let by_hand = a.g("controlsChecked").t();
    let mut unchecked: Vec<&str> = Vec::new();
    if !by_hand {
        if !loaded("var") && !a.g("var").t() {
            unchecked.push("mapped VAR controls (E27.6.4.1(3)(b))");
        }
        if !loaded("arterial") && art_by.is_none() && !(g.g("link").t() && c.g("net").g("aupArterial").g(&g.g("link").g("id").s()).t()) {
            unchecked.push("arterial frontage (E27.6.4.1(3)(c))");
        }
        if !loaded("lx") && !loaded("var") && !(a.g("lxDist").t() || (!a.g("lxDist").nullish() && !a.g("lxDist").eq_s(""))) {
            unchecked.push("level crossings (E27.6.4.1(3)(d))");
        }
        if !loaded("frontage") && !a.g("krf").t() && !a.g("gcf").t() {
            unchecked.push("Key Retail and General Commercial Frontage (E27.6.4.1(1), Table E27.6.4.2.1)");
        }
    }
    let worst = if flags.iter().any(|x| x.g("status").eq_s("NC")) { "NC" } else if !flags.is_empty() { "RD" } else if !unchecked.is_empty() { "unchecked" } else { "ok" };
    let unchecked_text = if unchecked.is_empty() { String::new() } else { format!("Not checked: {}. Load the AUP controls (Compliance \u{203a} Access), or check them on the planning maps and tick 'AUP controls checked by hand'.", unchecked.join("; ")) };
    let link = g.g("link");
    let sh = if link.t() { sh_of(p, link).g("sh").clone() } else { V::Bool(false) };
    let mut out = jobj! { "id" => a.g("id"), "name" => if a.g("name").t() { a.g("name").clone() } else { st("Access") }, "geo" => g.clone(), "flags" => V::Array(flags), "worst" => worst, "unchecked" => strs(unchecked), "uncheckedText" => unchecked_text,
        "max" => max, "basis" => basis, "frontageLink" => if link.t() { link.clone() } else { V::Null }, "sh" => sh };
    out.set("evidence", evidence_pack(p, sc, a, &g));
    out
}

/// `evidencePack(P, sc, a, g)`: the E27.8.2(11)(a) evidence for an access: `[[item, value, source]]`, or null without a frontage road.
pub fn evidence_pack(p: &V, sc: &V, a: &V, g: &V) -> V {
    if !g.t() || !g.g("link").t() {
        return V::Null;
    }
    let l = g.g("link");
    let s = p.g("settings");
    let ll: LatLng = [g.g("ll").ix(0).n(), g.g("ll").ix(1).n()];
    let f = flat_at(ll[0]);
    let r_m = settings(p).g("evidence").clone();
    let id = l.g("id").s();
    let vol = |per: &str| -> Option<f64> { res_of(sc, per).map(|r| or0(r.g("arcs").g(&format!("{id}:ab")).g("v")) + or0(r.g("arcs").g(&format!("{id}:ba")).g("v"))) };
    let spd = |per: &str| -> Option<f64> {
        let r = res_of(sc, per)?;
        let x: Vec<&V> = [r.g("arcs").g(&format!("{id}:ab")), r.g("arcs").g(&format!("{id}:ba"))].into_iter().filter(|v| v.t()).collect();
        if x.is_empty() { None } else { Some(x.iter().fold(0.0, |a, y| a + y.g("spd").n()) / x.len() as f64) }
    };
    let (am, pm) = (vol("AM"), vol("PM"));
    let pk = math_max(am.unwrap_or(0.0), pm.unwrap_or(0.0));
    let ptd = { let v = s.g("peakToDaily").n(); if truthy_number(v) { v } else { 10.0 } };
    let lbl = class_label(l.g("cls"));
    let mut rows: Vec<V> = Vec::new();
    rows.push(jarr!["Frontage road", format!("{} ({})", if l.g("name").t() { l.g("name").s() } else { lbl.clone() }, lbl.to_lowercase()), "model network"]);
    let nl = l.g("nslr").t() && l.g("nslr").g("limit").t();
    rows.push(jarr!["Speed limit", if nl { format!("{} km/h", l.g("nslr").g("limit").s()) } else { format!("{} km/h", l.g("speed").s()) }, if nl { "NZTA National Speed Limit Register" } else { "road attribute (check the NSLR)" }]);
    let sp = spd("AM").or_else(|| spd("PM"));
    rows.push(jarr!["Operating speed (modelled)", match sp { Some(x) => format!("{} km/h", numf(x, 0)), None => "run the model".to_string() }, "Spare Construct assignment, indicative"]);
    let opt = |x: Option<f64>| x.map_or(V::Null, V::Number);
    rows.push(jarr!["Traffic volume", if am.is_some() || pm.is_some() { format!("AM {} \u{b7} PM {} veh/h two-way \u{b7} about {} veh/day", num(&opt(am), 0), num(&opt(pm), 0), numf(pk * ptd, 0)) } else { "run the model".to_string() }, format!("model; daily = busiest peak \u{d7} {} (Settings)", ns(ptd))]);
    rows.push(jarr!["Heavy vehicle share", format!("{}%", num(s.g("hvPct"), 1)), "Settings default unless counts say otherwise"]);
    let crash = "crash data not loaded".to_string();
    rows.push(jarr!["Crash rate (CAS)", crash, "NZTA CAS; most recent months may be incomplete (D8)"]);
    rows.push(jarr!["Pedestrian count", if !a.g("peds").nullish() && !a.g("peds").eq_s("") { format!("{} people/h", a.g("peds").s()) } else { "enter a count (field survey)".to_string() }, "field input"]);
    let r_n = r_m.n();
    let near = |pred: &dyn Fn(&V) -> bool| -> usize {
        sc.g("links").vals().into_iter().filter(|x| !x.g("status").eq_s("closed") && pred(x) && sc.g("nodes").has(&x.g("a").s()) && sc.g("nodes").has(&x.g("b").s()) && net::geom(sc, x).is_some_and(|gg| to_line(&ll, &gg, &f).d <= r_n)).count()
    };
    let stops: Vec<String> = sc.g("routes").vals().into_iter().filter(|r| r.g("wp").items().iter().any(|w| sc.g("nodes").has(&w.s()) && util::dist(&ll, &net::ll_of(sc.g("nodes").g(&w.s()))) <= r_n)).map(|r| r.g("name").s()).collect();
    let rs = r_m.s();
    rows.push(jarr![format!("Bus routes / stops within {rs} m"), if stops.is_empty() { "none modelled".to_string() } else { stops.join(", ") }, format!("modelled routes' waypoints, not AT stop data ({} radius)", super::record::assumption())]);
    rows.push(jarr![format!("Bus lanes within {rs} m"), if near(&|x| x.g("busLane").t()) > 0 { "yes" } else { "none modelled" }, table("ACCESS.evidenceBasis")]);
    rows.push(jarr![format!("Cycleways within {rs} m"), if near(&|x| x.g("cycle").t() && !x.g("cycle").eq_s("none")) > 0 { "yes" } else { "none modelled" }, table("ACCESS.evidenceBasis")]);
    V::Array(rows)
}

/// `sisd(speed, measured85)`: the stopping sight distance required: `{ design, m, basis, source }`.
pub fn sisd(speed: &V, measured85: f64) -> V {
    let sh = table("SH");
    let add = sh.g("sisdAdd").n();
    let v = if measured85 > 0.0 { (measured85 / 10.0).ceil() * 10.0 } else { or0(speed) + add };
    let key = math_min(120.0, math_max(40.0, crate::jsnum::math_round(v / 10.0) * 10.0));
    jobj! { "design" => v, "m" => sh.g("sisd").g(&ns(key)), "basis" => if measured85 > 0.0 { format!("measured 85th percentile {} km/h", ns(measured85)) } else { format!("posted {} km/h + {} km/h", speed.s(), ns(add)) }, "source" => table("SRC.austroads") }
}

/// `accessType({ daily, peak, hcvWeek, sisdOk })`: the NZTA Table 9-1 access type: `{ type, text }`.
pub fn access_type(daily: f64, peak: f64, hcv_week: f64, sisd_ok: Option<bool>) -> V {
    let sh = table("SH");
    if daily > sh.g("design").g("daily").n() || peak > sh.g("design").g("hourly").n() {
        return jobj! { "type" => "design", "text" => "intersection design required" };
    }
    if daily < sh.g("typeC").g("daily").n() && (if truthy_number(hcv_week) { hcv_week } else { 0.0 }) <= sh.g("typeC").g("hcvWeek").n() {
        return match sisd_ok {
            Some(true) => jobj! { "type" => "C", "text" => "Type C" },
            Some(false) => jobj! { "type" => "E", "text" => "Type E (the SISD is not met, so not Type C)" },
            None => jobj! { "type" => "C?", "text" => "Type C if the SISD is met (enter the available sight distance); otherwise Type E" },
        };
    }
    jobj! { "type" => "E", "text" => "Type E" }
}

/// `pavement(hv0, hv1)`: the change in heavy vehicles a day: `{ change, flagged, text }`.
pub fn pavement(hv0: f64, hv1: f64) -> V {
    let ch = if hv0 > 0.0 { (hv1 - hv0) / hv0 } else if hv1 > 0.0 { f64::INFINITY } else { 0.0 };
    let flagged = if hv0 > 0.0 { (hv1 - hv0) / hv0 > table("SH.pavement").n() } else { hv1 > 0.0 };
    jobj! { "change" => ch, "flagged" => flagged, "text" => format!("{} \u{2192} {} heavy vehicles a day{}", numf(hv0, 0), numf(hv1, 0), if hv0 > 0.0 { format!(" ({})", fmt::signed_pct((hv1 - hv0) / hv0, 1)) } else { String::new() }) }
}

/// `accessVolume(sc, a)`: the modelled movements through an access placed at a node: `{ peak, period }`, or null.
pub fn access_volume(sc: &V, a: &V) -> V {
    if !a.t() || !a.g("node").t() {
        return V::Null;
    }
    let mut best: Option<(f64, &str)> = None;
    for per in ["AM", "PM"] {
        let Some(r) = res_of(sc, per) else { continue };
        if !r.g("turns").t() {
            continue;
        }
        let v = r.g("turns").g(&a.g("node").s()).items().iter().filter(|m| rx("^c:", "").test(&m.g("from").s()) || rx("^c:", "").test(&m.g("to").s())).fold(0.0, |s, m| s + or0(m.g("v")));
        let v = util::r1(v);
        if best.is_none_or(|b| v > b.0) {
            best = Some((v, per));
        }
    }
    match best {
        Some((v, per)) if v > 0.0 => jobj! { "peak" => v, "period" => per },
        _ => V::Null,
    }
}

/// `motorwayConnectors(sc, P)`: zones pinned straight to motorway nodes (GRPA s84), with the author's confirmation.
pub fn motorway_connectors(sc: &V, p: &V) -> V {
    let ok = state(p);
    let ok = ok.g("mwConfirm");
    let mut out: Vec<V> = Vec::new();
    for z in sc.g("zones").vals() {
        if z.g("kind").eq_s("gateway") || !z.g("node").t() || !sc.g("nodes").has(&z.g("node").s()) {
            continue;
        }
        let ls = net::road_links(sc, &z.g("node").s());
        if !(!ls.is_empty() && ls.iter().all(|l| l.g("cls").eq_s("motorway") || l.g("cls").eq_s("ramp"))) {
            continue;
        }
        let key = rx(r"[^w~-]", "g").replace_with(&format!("{}~{}", z.g("id").s(), z.g("node").s()), "_");
        let cf = ok.g(&key);
        let cf = if cf.t() && cf.g("confirmed").t() { Some(cf) } else { None };
        let name = if z.g("name").t() { z.g("name").s() } else { "A zone".to_string() };
        out.push(jobj! { "zone" => z.g("id"), "name" => name.as_str(), "node" => z.g("node"), "key" => key.as_str(), "confirmed" => cf.is_some(), "by" => cf.map_or(st(""), |c| if c.g("by").t() { c.g("by").clone() } else { st("") }),
            "date" => cf.map_or(st(""), |c| if c.g("date").t() { c.g("date").clone() } else { st("") }), "reason" => if ok.g(&key).g("reason").t() { ok.g(&key).g("reason").clone() } else { st("") },
            "text" => format!("{name} connects straight to a motorway. GRPA s84 allows access to a motorway only by special authorisation where no other road access is reasonably available (NZTA PPM Access guide s4). Move the connector to a local road, or confirm below why none is available.") });
    }
    V::Array(out)
}

fn pk_of(r: &V, k: &str) -> f64 {
    let a = or0(r.g("per").g("AM").g(k));
    let b = or0(r.g("per").g("PM").g(k));
    math_max(a, b)
}

/// `shCheck(P, sc)`: state highway involvement, NZTA triggers and approvals, the impact table, junctions and the access technical check.
pub fn sh_check(p: &V, sc: &V) -> V {
    let c = state(p);
    let inv = sh_involved(p, sc);
    let pe = super::record::pathway_entry(c.g("ctx").g("pathway"));
    let base = base_sc(p);
    let s = p.g("settings");
    let ptd = { let v = s.g("peakToDaily").n(); if truthy_number(v) { v } else { 10.0 } };
    let acc: Vec<V> = c.g("access").items().iter().map(|a| access_check(p, a, sc)).filter(|x| x.g("sh").t()).collect();
    let find_a = |id: &V| c.g("access").items().iter().find(|y| y.g("id").strict_eq(id)).cloned().unwrap_or(V::object());
    let mut triggers: Vec<String> = Vec::new();
    for x in &acc {
        let a = find_a(x.g("id"));
        triggers.push(if a.g("existing").t() { if a.g("changed").t() { format!("Changed access: {}", x.g("name").s()) } else { format!("Land-use change at an existing access: {}", x.g("name").s()) } } else { format!("New access: {}", x.g("name").s()) });
    }
    if inv.g("why").items().iter().any(|w| rx("development traffic", "").test(&w.s())) {
        triggers.push("Nearby development whose traffic may affect the state highway".into());
    }
    let desig: Vec<V> = site_info(p).g("designations").items().iter().filter(|d| rx("Transport Agency|Waka Kotahi|NZTA|KiwiRail", "i").test(&format!("{} {}", d.g("by").sn(), d.g("name").sn()))).cloned().collect();
    let motorway_acc = acc.iter().any(|x| x.g("frontageLink").t() && x.g("frontageLink").g("cls").eq_s("motorway"));
    let car_days = table("SH.carDays").s();
    let mut approvals: Vec<V> = vec![
        jobj! { "id" => "s95E", "label" => "Written approval from NZTA as an affected party (RMA s95E(3)(a))", "needed" => !triggers.is_empty() },
        jobj! { "id" => "s176", "label" => if !desig.is_empty() { format!("Requiring authority consent on designated land (RMA s176(1)(b), or s178 if not yet confirmed): {}", desig.iter().map(|d| format!("{} {}", d.g("by").sn(), d.g("name").sn()).trim().to_string()).collect::<Vec<_>>().join("; ")) } else { "Requiring authority consent on designated land (RMA s176(1)(b) / s178): no NZTA or KiwiRail designation found at the site point; check the planning maps".to_string() }, "needed" => !desig.is_empty(), "check" => desig.is_empty() },
        jobj! { "id" => "grpa91", "label" => "Crossing place authorised by NZTA notice (GRPA s91) and s93 approval before the s224(c) certificate, where the frontage is a limited access road (no open LAR layer: check the LAR status)", "needed" => !acc.is_empty(), "check" => true },
        jobj! { "id" => "grpa84", "label" => "Motorway special access authorisation (GRPA s84)", "needed" => motorway_acc },
        jobj! { "id" => "car", "label" => format!("Spare Construct access request to NZTA at least {car_days} working days before works (PPM Access guide s7.4)"), "needed" => !acc.is_empty() },
    ];
    if pe.is_some_and(|e| e.g("kind").eq_s("ftaa")) {
        approvals.push(jobj! { "id" => "ftaa", "label" => "GRPA crossing-place (s91) and s93 approvals and corridor access approvals are outside Fast-track (FTAA s42(4)): seek them from NZTA separately", "needed" => !acc.is_empty() || !triggers.is_empty() });
    }
    let sh_ids: Vec<String> = inv.g("links").items().iter().map(|x| x.s()).collect();
    let dh = c.g("ctx").g("devHvPct");
    let dev_hv: Option<f64> = if dh.entered() { Some(util::clamp(dh.n(), 0.0, 100.0)) } else { None };
    let mut rows: Vec<V> = Vec::new();
    let hv = s.g("hvPct");
    for id in &sh_ids {
        let l = if sc.g("links").g(id).t() { sc.g("links").g(id) } else { base.g("links").g(id) };
        if !l.t() {
            continue;
        }
        let shl = sh_of(p, l);
        let mut r = jobj! { "id" => id.as_str(), "name" => if l.g("name").t() { l.g("name").clone() } else { shl.g("ref").clone() }, "ref" => shl.g("ref"), "per" => V::object() };
        for per in ["AM", "PM"] {
            let w = res_of(sc, per);
            let wo = if std::ptr::eq(sc, base) { None } else { res_of(base, per) };
            let v = |res: Option<&V>| -> V { res.map_or(V::Null, |r| V::Number(or0(r.g("arcs").g(&format!("{id}:ab")).g("v")) + or0(r.g("arcs").g(&format!("{id}:ba")).g("v")))) };
            let mut per_o = r.g("per").clone();
            per_o.set(per, jobj! { "with" => v(w), "without" => v(wo) });
            r.set("per", per_o);
        }
        let day_with = pk_of(&r, "with") * ptd;
        r.set("dayWith", V::Number(day_with));
        let has_wo = !r.g("per").g("AM").g("without").nullish() || !r.g("per").g("PM").g("without").nullish();
        let day_without = if has_wo { Some(pk_of(&r, "without") * ptd) } else { None };
        r.set("dayWithout", day_without.map_or(V::Null, V::Number));
        r.set("hvPct", hv.clone());
        if let Some(dw) = day_without {
            let hv0 = dw * hv.n() / 100.0;
            let hv1 = hv0 + (day_with - dw) * dev_hv.unwrap_or(hv.n()) / 100.0;
            let mut pv = pavement(hv0, hv1);
            pv.set("proxy", V::Bool(dev_hv.is_none()));
            r.set("pavement", pv);
        } else {
            r.set("pavement", V::Null);
        }
        if !r.g("per").g("AM").g("with").nullish() || !r.g("per").g("PM").g("with").nullish() {
            rows.push(r);
        }
    }
    js_sort_by(&mut rows, |a, b| (b.g("dayWith").n() - or0(b.g("dayWithout"))) - (a.g("dayWith").n() - or0(a.g("dayWithout"))));
    let mut sh_nodes: Vec<String> = Vec::new();
    for id in &sh_ids {
        let l = sc.g("links").g(id);
        if l.t() && !l.g("status").eq_s("closed") {
            sh_nodes.push(l.g("a").s());
            sh_nodes.push(l.g("b").s());
        }
    }
    let mut nodes: Vec<V> = Vec::new();
    for per in ["AM", "PM"] {
        let w = res_of(sc, per);
        let wo = if std::ptr::eq(sc, base) { None } else { res_of(base, per) };
        let Some(w) = w else { continue };
        for (nid, n) in w.g("nodes").entries() {
            if sh_nodes.iter().any(|x| x == nid) {
                let d0 = wo.filter(|x| x.g("nodes").g(nid).t()).map_or(V::Null, |x| x.g("nodes").g(nid).g("delay").clone());
                nodes.push(jobj! { "id" => nid, "period" => per, "name" => net::junction_name(sc, nid), "d1" => n.g("delay"), "d0" => d0, "los" => n.g("los") });
            }
        }
    }
    let tech: Vec<V> = acc
        .iter()
        .map(|x| {
            let a = find_a(x.g("id"));
            let l = x.g("frontageLink");
            let lim = if l.g("nslr").t() && l.g("nslr").g("limit").t() { l.g("nslr").g("limit").clone() } else { l.g("speed").clone() };
            let sd = sisd(&lim, or0(a.g("speed85")));
            let avail: Option<f64> = if a.g("sightAvail").entered() { Some(a.g("sightAvail").n()) } else { None };
            let ok = avail.map(|v| v >= sd.g("m").n());
            let mod_ = access_volume(sc, &a);
            let hcv_share = or0(a.g("hcvPct"));
            let typed = |v: &V| v.entered();
            let peak = if typed(a.g("peak")) { a.g("peak").n() } else if mod_.t() { mod_.g("peak").n() } else { 0.0 };
            let daily = if typed(a.g("daily")) { a.g("daily").n() } else if mod_.t() { mod_.g("peak").n() * ptd } else { 0.0 };
            let vol_basis = if typed(a.g("daily")) || typed(a.g("peak")) { "entered".to_string() } else if mod_.t() { format!("modelled: {} veh/h in the {} peak through the access node; a day = busiest peak \u{d7} {} (Settings), indicative", numf(mod_.g("peak").n(), 0), mod_.g("period").s(), ns(ptd)) } else { "no movements entered or modelled".to_string() };
            let hcvp = table("SH.hcvPcu");
            let over = hcv_share > hcvp.g("share").n();
            let pcu_note = if over { format!("HCV share {}% is over 20%: count each HCV as {} PCU", ns(hcv_share), hcvp.g("pcu").s()) } else { String::new() };
            let factor = if over { 1.0 + hcv_share / 100.0 * (hcvp.g("pcu").n() - 1.0) } else { 1.0 };
            jobj! { "id" => x.g("id"), "name" => x.g("name"), "road" => if l.g("name").t() { l.g("name").clone() } else { sh_of(p, l).g("ref").clone() }, "posted" => lim, "sisd" => sd.clone(), "available" => avail, "sisdOk" => ok, "daily" => util::r1(daily), "peak" => util::r1(peak), "volBasis" => vol_basis,
                "type" => access_type(daily * factor, peak * factor, or0(a.g("hcvWeek")), ok), "pcuNote" => pcu_note, "spacing" => "Intersection and access spacing (Tables 8-2 and 8-3): check by hand; Spare Construct has not encoded them", "crashYears" => table("SH.crashYears") }
        })
        .collect();
    let noise = inv.g("why").items().iter().any(|w| rx("within", "").test(&w.s()))
        && ["residential", "apartment", "retirement", "hotel", "school", "childcare", "medical"].iter().any(|k| net::dev_zones(sc).iter().any(|z| z.g("lu").g(k).n() > 0.0));
    let sh = table("SH");
    let pav_basis = match dev_hv {
        None => format!("Heavy vehicles a day from one HV share for all traffic ({}%, Settings): a total-traffic proxy. Enter the development's HV share, or use an HV forecast, for the G00907 s5.4.6 test.", num(hv, 1)),
        Some(d) => format!("Heavy vehicles a day: existing traffic at {}% (Settings), added traffic at the development's {}% (entered).", num(hv, 1), numf(d, 1)),
    };
    jobj! { "involved" => inv.g("involved"), "why" => inv.g("why"), "inArea" => inv.g("inArea"), "triggers" => strs(triggers), "approvals" => V::Array(approvals), "rows" => V::Array(rows), "nodes" => V::Array(nodes), "tech" => V::Array(tech), "noise" => noise,
        "motorway" => motorway_connectors(sc, p), "devHvPct" => dev_hv, "pavementBasis" => pav_basis, "labels" => jarr![sh.g("itaLabel"), sh.g("accessLabel")], "loaded" => if c.g("net").g("loaded").g("sh").t() { c.g("net").g("loaded").g("sh").clone() } else { V::Null } }
}

struct Grp {
    key: &'static str,
    name: V,
    assets: f64,
    links: Vec<V>,
    nodes: Vec<V>,
    lidx: HashMap<String, usize>,
    nidx: HashMap<String, usize>,
    access: bool,
}

fn put(idx: &mut HashMap<String, usize>, list: &mut Vec<V>, id: &str, row: &V, size: &str) {
    match idx.get(id).copied() {
        None => {
            let mut r = row.clone();
            r.set("periods", V::Array(vec![row.g("period").clone()]));
            idx.insert(id.to_string(), list.len());
            list.push(r);
        }
        Some(i) => {
            let o = &mut list[i];
            let mut periods: Vec<V> = o.g("periods").items().to_vec();
            if !periods.iter().any(|x| x.strict_eq(row.g("period"))) {
                periods.push(row.g("period").clone());
            }
            if row.g(size).n() > o.g(size).n() {
                let mut r = row.clone();
                r.set("periods", V::Array(periods.clone()));
                *o = r;
            } else {
                o.set("periods", V::Array(periods));
            }
        }
    }
}

/// `operators(P)`: the affected operators for the active scenario against the base, in both peaks.
pub fn operators(p: &V) -> V {
    operators_with(p, active_sc(p), base_sc(p), None)
}

/// `operators(P, opts)`: the affected operators: links and junctions whose with-development change passes the threshold in either modelled peak (or only `period`). `{ period, periods, periodText, compared, withName, withoutName, rows, thresholds, label, note }`.
pub fn operators_with(p: &V, sc: &V, base: &V, period: Option<&str>) -> V {
    let s = settings(p);
    let c = state(p);
    let periods: Vec<&str> = match period { Some(x) => vec![if x == "AM" { "AM" } else { "PM" }], None => vec!["AM", "PM"] };
    let same = std::ptr::eq(sc, base);
    let pairs: Vec<(&str, Option<&V>, Option<&V>)> = periods.iter().map(|&pp| (pp, res_of(sc, pp), if same { None } else { res_of(base, pp) })).collect();
    let run: Vec<(&str, &V, &V)> = pairs.iter().filter_map(|(pp, w, wo)| Some((*pp, (*w)?, (*wo)?))).collect();
    let x: AuthCtx = auth_ctx(p, sc);
    let mut groups: Vec<Grp> = Vec::new();
    fn add(groups: &mut Vec<Grp>, key: &'static str, name: &V) -> usize {
        if let Some(i) = groups.iter().position(|g| g.key == key) {
            return i;
        }
        groups.push(Grp { key, name: name.clone(), assets: 0.0, links: vec![], nodes: vec![], lidx: HashMap::new(), nidx: HashMap::new(), access: false });
        groups.len() - 1
    }
    let mut by_link: HashMap<String, (V, usize, Vec<usize>)> = HashMap::new();
    let dvc_t = s.g("dvc").n();
    let dd_t = s.g("ddelay").n();
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("closed") {
            continue;
        }
        let a = link_authority_with(p, l, sc, &x);
        let key: &'static str = if a.g("operator").eq_s("nzta") { "nzta" } else { "rca" };
        let gi = add(&mut groups, key, a.g("name"));
        let extra: Vec<usize> = a.g("also").items().iter().map(|n| { let k = super::authority::also_key(n); add(&mut groups, if k == "kiwirail" { "kiwirail" } else { "cco" }, n) }).collect();
        groups[gi].assets += 1.0;
        for &e in &extra {
            groups[e].assets += 1.0;
        }
        by_link.insert(l.g("id").s(), (a, gi, extra.clone()));
        for (per, w, wo) in &run {
            for d in ["ab", "ba"] {
                let k = format!("{}:{}", l.g("id").s(), d);
                let (r1v, r0v) = (w.g("arcs").g(&k), wo.g("arcs").g(&k));
                if !r1v.t() && !r0v.t() {
                    continue;
                }
                let dvc = or0(r1v.g("vc")) - or0(r0v.g("vc"));
                if !(dvc >= dvc_t) {
                    continue;
                }
                let row = jobj! { "id" => l.g("id"), "dir" => d, "period" => *per, "name" => if l.g("name").t() { l.g("name").s() } else { class_label(l.g("cls")) }, "dvc" => util::r3(dvc), "v0" => util::r1(or0(r0v.g("v"))), "v1" => util::r1(or0(r1v.g("v"))) };
                for &gx in std::iter::once(&gi).chain(extra.iter()) {
                    let g = &mut groups[gx];
                    put(&mut g.lidx, &mut g.links, &k, &row, "dvc");
                }
            }
        }
    }
    for (per, w, wo) in &run {
        for (nid, n1) in w.g("nodes").entries() {
            let n0 = wo.g("nodes").g(nid);
            let dd = or0(n1.g("delay")) - or0(n0.g("delay"));
            if !(dd >= dd_t) {
                continue;
            }
            let at: Vec<&(V, usize, Vec<usize>)> = net::road_links(sc, nid).iter().filter_map(|l| by_link.get(&l.g("id").s())).collect();
            let sh = at.iter().find(|x| x.0.g("operator").eq_s("nzta"));
            let first = if let Some(sh) = sh { sh.1 } else if let Some(a0) = at.first() { a0.1 } else {
                let nm = link_authority_with(p, &jobj! { "id" => "", "cls" => "local" }, sc, &x).g("name").clone();
                add(&mut groups, "rca", &nm)
            };
            let mut gs: Vec<usize> = vec![first];
            for xx in &at {
                for &e in &xx.2 {
                    if !gs.contains(&e) {
                        gs.push(e);
                    }
                }
            }
            let row = jobj! { "id" => nid, "period" => *per, "name" => net::junction_name(sc, nid), "dd" => util::r1(dd), "d0" => util::r1(or0(n0.g("delay"))), "d1" => util::r1(or0(n1.g("delay"))) };
            for gx in gs {
                let g = &mut groups[gx];
                put(&mut g.nidx, &mut g.nodes, nid, &row, "dd");
            }
        }
    }
    let lxm = table("AUTH.levelCrossingM").n();
    if c.g("access").items().iter().any(|a| { let g = access_geo(p, sc, a); g.t() && !g.g("lxDist").nullish() && g.g("lxDist").n() <= lxm }) {
        let gi = add(&mut groups, "kiwirail", table("AUTH.kiwirail"));
        groups[gi].access = true;
    }
    let order = |k: &str| match k { "nzta" => 0.0, "rca" => 1.0, "cco" => 2.0, _ => 3.0 };
    let mut idx: Vec<usize> = (0..groups.len()).collect();
    js_sort_by(&mut idx, |a, b| order(groups[*a].key) - order(groups[*b].key));
    let peaks = |ps: &[&str]| format!("{} peak{}", ps.join(" and "), if ps.len() > 1 { "s" } else { "" });
    let done: Vec<&str> = run.iter().map(|x| x.0).collect();
    let used: Vec<&str> = if done.is_empty() { periods.clone() } else { done.clone() };
    let period_text = peaks(&used);
    let missing: Vec<&str> = pairs.iter().filter(|x| !(x.1.is_some() && x.2.is_some())).map(|x| x.0).collect();
    let (sc_name, base_name) = (sc.g("name").s(), base.g("name").s());
    let note = if run.is_empty() {
        if same { "Make another scenario active (with development) to compare it with the base scenario.".to_string() } else { format!("Run {sc_name} and {base_name} for the {} to compare.", peaks(&periods)) }
    } else if !missing.is_empty() {
        format!("Only the {period_text} compared: run both scenarios for the {} too.", peaks(&missing))
    } else {
        String::new()
    };
    let rows: Vec<V> = idx
        .iter()
        .map(|&i| {
            let g = &groups[i];
            let mut o = jobj! { "key" => g.key, "name" => &g.name, "assets" => g.assets, "links" => V::Array(g.links.clone()), "nodes" => V::Array(g.nodes.clone()) };
            if g.access {
                o.set("access", V::Bool(true));
            }
            o.set("affected", V::Bool(!g.links.is_empty() || !g.nodes.is_empty() || g.access));
            o
        })
        .collect();
    let basis = table("OPS.basis").s();
    jobj! { "period" => used.join("+"), "periods" => strs(done), "periodText" => period_text.as_str(), "compared" => !run.is_empty(), "withName" => sc.g("name"), "withoutName" => base.g("name"), "rows" => V::Array(rows),
        "thresholds" => jobj! { "dvc" => s.g("dvc"), "ddelay" => s.g("ddelay"), "basis" => basis.as_str() },
        "label" => format!("Affected when the with-development change ({sc_name} against {base_name}, {period_text}) is a V/C increase of {} or more, or a junction delay increase of {} s or more ({basis}). Links on a modelled bus route or with a bus lane also count for the transport CCO, and links within 30 m of a level crossing for KiwiRail. Feeds Sch 4 cl 6(1)(f) and s95E advice.", s.g("dvc").s(), s.g("ddelay").s()),
        "note" => note }
}

/// A helper for callers: the level-crossing distance of a link (see [`level_crossing_dist`]).
pub fn lx_dist(p: &V, sc: &V, l: &V) -> Option<f64> {
    level_crossing_dist(p, sc, l)
}
