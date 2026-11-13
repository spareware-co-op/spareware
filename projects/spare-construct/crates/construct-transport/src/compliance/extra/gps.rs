//! IPM 2027-30 stage 2 metrics and funding attribution (compliance-extra.js sections 15 and 16).

use super::ctx::{Pair, active_sc, add_months, context, data, data_mut, day, dev_zones, dwellings, esc, long_date, lu_of, nn, num, pair, periods_of, res_of, link_name};
use super::packs::{n0, t_table};
use super::policy::{jobs_of, skim};
use super::shared::{changed_links, econ_of};
use crate::compliance::data::x as xdata;
use crate::compliance::dates;
use crate::compliance::engine;
use crate::compliance::html::ReportCtx;
use crate::compliance::jv::{Jx, NULL, V, arr, jobj, merge, strs};
use crate::compliance::net;
use crate::compliance::rx::rx;
use crate::graph::js_sort_by;
use crate::jsnum::truthy_number;
use crate::util::fmt;
use std::collections::HashMap;

fn sig_label(k: &V) -> V {
    let t = xdata("SIG");
    let key = k.s();
    if t.has(&key) { t.g(&key).clone() } else { V::Null }
}

/// `freightRating(saving, sig)`: the draft IPM rating of a heavy-vehicle travel-time saving (%) for a route significance.
#[must_use]
pub fn freight_rating(saving: f64, sig: &str) -> &'static str {
    if sig == "national" && saving > 30.0 {
        return "Very High";
    }
    if sig == "national" && saving > 10.0 {
        return "High";
    }
    if sig == "regional" && saving > 30.0 {
        return "Medium";
    }
    if saving > 5.0 {
        return "Low";
    }
    "Below the Low threshold"
}

/// `gtRating(saving)`: the draft IPM rating of a general traffic travel-time saving (%).
#[must_use]
pub fn gt_rating(saving: f64) -> &'static str {
    if saving > 5.0 {
        "High"
    } else if saving > 0.0 {
        "Medium"
    } else {
        "No improvement"
    }
}

/// `roadsToLinks(sc, names)`: the ids of the open roads whose names are in a list separated by commas, semicolons or new lines (case ignored).
#[must_use]
pub fn roads_to_links(sc: &V, names: &V) -> Vec<V> {
    let text = if names.t() { names.s() } else { String::new() };
    let mut want: Vec<String> = Vec::new();
    for part in rx(r"[,;\n]+", "").split(&text) {
        let x = crate::compliance::jv::trim(&part).to_lowercase();
        if !x.is_empty() && !want.contains(&x) {
            want.push(x);
        }
    }
    if want.is_empty() {
        return vec![];
    }
    sc.g("links")
        .vals()
        .into_iter()
        .filter(|l| !l.g("status").eq_s("closed") && want.contains(&(if l.g("name").t() { l.g("name").s() } else { String::new() }).to_lowercase()))
        .map(|l| l.g("id").clone())
        .collect()
}

/// `gpsRoutes(P)`: the routes the author entered (links by id or by road names), else the scheme network (arterials and motorways) with its
/// significance not set.
#[must_use]
pub fn gps_routes(p: &V) -> Vec<V> {
    let sc = active_sc(p);
    let cx = data(p);
    let own: Vec<V> = cx
        .g("gps")
        .g("routes")
        .items()
        .iter()
        .filter(|r| r.t() && (r.g("name").t() || r.g("roads").t() || super::packs::js_length(if r.g("links").t() { r.g("links") } else { &NULL }) > 0))
        .map(|r| {
            let links = if r.g("links").is_arr() && !r.g("links").items().is_empty() { r.g("links").clone() } else { arr(roads_to_links(sc, r.g("roads"))) };
            let mut o = merge(r, &V::object());
            o.set("links", links);
            o
        })
        .collect();
    if !own.is_empty() {
        return own;
    }
    let ids: Vec<V> = sc
        .g("links")
        .vals()
        .into_iter()
        .filter(|l| (l.g("cls").eq_s("arterial") || l.g("cls").eq_s("motorway")) && !l.g("status").eq_s("closed"))
        .map(|l| l.g("id").clone())
        .collect();
    if ids.is_empty() {
        vec![]
    } else {
        vec![jobj! { "id" => "auto", "name" => "Scheme network (arterials and motorways)", "links" => arr(ids), "significance" => "", "auto" => true }]
    }
}

// The minutes saved to each tagged destination of `gpsMetrics`, trip-weighted over the origins, in the first period both scenarios ran
fn destination_minutes(p: &V, pr: &Pair<'_>, cx: &V, ps: &[&'static str]) -> Vec<V> {
    let mut destinations: Vec<V> = Vec::new();
    let dests: Vec<&V> = cx.g("destinations").items().iter().filter(|dest| pr.opt.g("zones").g(&dest.g("zoneId").s()).t()).collect();
    let Some(&per) = ps.first() else { return destinations };
    if dests.is_empty() {
        return destinations;
    }
    // ps holds the option's periods that the base also ran, so both results are there
    let (Some(ro), Some(rb)) = (res_of(pr.opt, per), res_of(pr.base, per)) else { return destinations };
    let zone_ids: Vec<String> = ro.g("zoneIds").items().iter().map(crate::compliance::jv::Jx::s).collect();
    let orig: Vec<String> = zone_ids.iter().filter(|id| pr.base.g("zones").g(id).t()).cloned().collect();
    let to_ids: Vec<String> = dests.iter().map(|dest| dest.g("zoneId").s()).collect();
    let sb = skim(p, pr.base, Some(rb), &orig, &to_ids);
    let so = skim(p, pr.opt, Some(ro), &orig, &to_ids);
    let zn_count = zone_ids.len();
    for dest in &dests {
        let (mut wsum, mut ssum) = (0.0, 0.0);
        let dz = dest.g("zoneId").s();
        let dj = zone_ids.iter().position(|x| *x == dz);
        for org in &orig {
            let oi = zone_ids.iter().position(|x| x == org);
            let t0 = sb.get(org).and_then(|m| m.get(&dz));
            let t1 = so.get(org).and_then(|m| m.get(&dz));
            let (Some(t0), Some(t1)) = (t0, t1) else { continue };
            let trips = if ro.g("T").t() && dj.is_some() {
                // Bound: both positions are below the zone count, so the cell index is inside the matrix
                let tx = ro.g("T").ix(oi.map_or(usize::MAX, |i| i.saturating_mul(zn_count).saturating_add(dj.unwrap_or(0)))).n();
                if truthy_number(tx) { tx } else { 0.0 }
            } else {
                1.0
            };
            wsum += trips;
            ssum += trips * (t0 - t1);
        }
        let zn = pr.opt.g("zones").g(&dz);
        let sl = sig_label(dest.g("significance"));
        destinations.push(jobj! {
            "name" => if zn.g("name").t() { zn.g("name").clone() } else { dest.g("zoneId").clone() },
            "significance" => if sl.t() { sl } else { V::String("Road".to_string()) },
            "minutes" => if wsum > 0.0 { Some(ssum / wsum / 60.0) } else { None },
            "period" => per,
        });
    }
    destinations
}

// The houses enabled by stage of `gpsMetrics`, a stage counted when 75% complete by the standard's year (2035)
fn houses_enabled(pr: &Pair<'_>, cx: &V, std: &V) -> V {
    let by = std.g("by").n();
    let derived = cx.g("stages").items().is_empty();
    let stages: Vec<V> = if derived {
        dev_zones(pr.opt)
            .iter()
            .map(|z| {
                let y = z.g("year").n();
                jobj! { "name" => if z.g("name").t() { z.g("name").clone() } else { z.g("id").clone() }, "dwellings" => dwellings(z, pr.opt), "year75" => if truthy_number(y) { Some(y) } else { None }, "derived" => true }
            })
            .collect()
    } else {
        cx.g("stages").items().to_vec()
    };
    let rows: Vec<V> = stages
        .iter()
        .map(|s| {
            let mut o = merge(s, &V::object());
            o.set("counted", V::Bool(s.g("year75").n() > 0.0 && s.g("year75").n() <= by));
            o
        })
        .collect();
    let total = rows.iter().filter(|r| r.g("counted").t()).fold(0.0, |s, r| s + n0(r.g("dwellings")));
    jobj! {
        "rows" => arr(rows), "by" => std.g("by"),
        "basis" => format!("A stage counts when 75% complete by {}{}", std.g("by").s(), if derived { "; years are the zone opening years: replace them with the year each stage is 75% complete" } else { "" }),
        "total" => total,
    }
}

/// `gpsMetrics(P)`: the stage 2 GPS metrics: `{ routes, label, verified, network, destinations, houses, dsi }`.
pub fn gps_metrics(p: &V) -> V {
    let pr = pair(p);
    let cx = data(p).g("gps").clone();
    let ps: Vec<&'static str> = periods_of(pr.opt).into_iter().filter(|x| res_of(pr.base, x).is_some()).collect();
    let std = xdata("STD.gps");
    let assume = xdata("ASSUMPTION").s();
    let time = |sc: &V, per: &str, ids: &[V]| -> f64 {
        ids.iter().fold(0.0, |sum, id| {
            sum + ["ab", "ba"].iter().fold(0.0, |tsum, dir| {
                let arc = res_of(sc, per).map_or(&NULL, |res| res.g("arcs")).g(&format!("{}:{}", id.s(), dir));
                tsum + if arc.t() { arc.g("t").n() } else { 0.0 }
            })
        })
    };
    let (mut tb, mut to, mut sig, mut set) = (0.0, 0.0, "road".to_string(), false);
    let mut routes: Vec<V> = Vec::new();
    for route in gps_routes(p) {
        let ids: Vec<V> = route.g("links").items().iter().filter(|id| pr.base.g("links").g(&id.s()).t() && pr.opt.g("links").g(&id.s()).t()).cloned().collect();
        if ids.is_empty() || ps.is_empty() {
            continue;
        }
        let base_t = ps.iter().fold(0.0, |sum, per| sum + time(pr.base, per, &ids));
        let opt_t = ps.iter().fold(0.0, |sum, per| sum + time(pr.opt, per, &ids));
        let saving = if base_t > 0.0 { (base_t - opt_t) / base_t * 100.0 } else { 0.0 };
        if sig_label(route.g("significance")).t() {
            set = true;
        }
        tb += base_t;
        to += opt_t;
        if route.g("significance").eq_s("national") || (route.g("significance").eq_s("regional") && sig == "road") {
            sig = route.g("significance").s();
        }
        let sl = sig_label(route.g("significance"));
        routes.push(jobj! {
            "name" => if route.g("name").t() { route.g("name").clone() } else if route.g("roads").t() { route.g("roads").clone() } else { V::String("Route".to_string()) },
            "significance" => if sl.t() { sl } else { V::String("Not set".to_string()) },
            "base" => base_t, "opt" => opt_t, "saving" => saving, "auto" => route.g("auto").t(),
        });
    }
    let saving = if tb > 0.0 { Some((tb - to) / tb * 100.0) } else { None };
    let network = jobj! {
        "saving" => saving,
        "significance" => if set { sig_label(&V::String(sig.clone())) } else { V::String(format!("Not set (rated as Road, {assume}: set each route's significance)")) },
        "sigSet" => set,
        "hcv" => saving.map(|sv| freight_rating(sv, &sig)),
        "gt" => saving.map(gt_rating),
        "note" => "Measured at scheme network level (sum over the routes), not per segment. Spare Construct has one vehicle class, so HCV and general-traffic travel times are the same. Reliability is not modelled: enter it from observed data.",
    };
    // minutes saved to tagged destinations, trip-weighted over origins
    let destinations = destination_minutes(p, &pr, &cx, &ps);
    // houses enabled by stage, counted when 75% complete by 2035
    let houses = houses_enabled(&pr, &cx, std);
    let cost: Option<f64> = if nn(cx.g("cost")).is_some() {
        Some(cx.g("cost").n())
    } else {
        Some(engine::capital(p, pr.opt).g("total").n() - engine::capital(p, pr.base).g("total").n())
    };
    let red = nn(cx.g("dsiReduction"));
    let dsi = jobj! {
        "reduction" => red,
        "cost" => cost,
        "per100m" => if red.is_some() && cost.unwrap_or(0.0) > 0.0 { Some(cx.g("dsiReduction").n() / cost.unwrap_or(0.0) * 100.0) } else { None },
        "collective" => if cx.g("collective").t() { cx.g("collective").clone() } else { V::String("Not entered".to_string()) },
    };
    jobj! { "routes" => arr(routes), "label" => xdata("STD.labels.gps"), "verified" => std.g("verified"), "network" => network, "destinations" => arr(destinations), "houses" => houses, "dsi" => dsi }
}

/* ---------- funding attribution ---------- */

/// `inPoly(pt, poly)`: whether a point `[lat, lng]` is inside a polygon of `[lat, lng]` points (even-odd rule).
#[must_use]
pub fn in_poly(pt: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for (i, &[yi, xi]) in poly.iter().enumerate() {
        // j is the previous point (the last for the first), always in range
        let Some(&[yj, xj]) = poly.get(j) else {
            j = i;
            continue;
        };
        if ((yi > pt[0]) != (yj > pt[0])) && pt[1] < (xj - xi) * (pt[0] - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// The points of a stored polygon: the entries that are pairs, as numbers (`polygon.filter(x => Array.isArray(x) && x.length === 2)`).
#[must_use]
pub fn polygon_of(v: &V) -> Vec<[f64; 2]> {
    v.items().iter().filter(|x| x.is_arr() && x.items().len() == 2).map(|x| [x.ix(0).n(), x.ix(1).n()]).collect()
}

/// `growthShare(P, sc)`: the growth share of the households and employment (`{ He, Hf, Ee, Ef, share, formula, label }`).
#[must_use]
pub fn growth_share(p: &V, sc: &V) -> V {
    let em = {
        let e = data(p).g("funding").g("employment").clone();
        if e.t() { e } else { V::object() }
    };
    let (mut he, mut hf) = (0.0, 0.0);
    for z in sc.g("zones").vals() {
        if z.g("kind").eq_s("gateway") {
            continue;
        }
        let h = lu_of(z, "residential") + lu_of(z, "apartment") + lu_of(z, "retirement");
        let k = net::zone_class(z);
        if k == "existing" {
            he += h;
        }
        hf += h * if k == "existing" { 1.0 } else { super::ctx::stage_of(z, sc) };
    }
    let jz: Vec<&V> = sc.g("zones").vals().into_iter().filter(|z| !z.g("kind").eq_s("gateway")).collect();
    let ee = if nn(em.g("existing")).is_some() { em.g("existing").n() } else { jz.iter().filter(|z| net::zone_class(z) == "existing").fold(0.0, |s, z| s + jobs_of(z)) };
    let ef = if nn(em.g("future")).is_some() { em.g("future").n() } else { jz.iter().fold(0.0, |s, z| s + jobs_of(z)) };
    let den = hf + 0.5 * ef;
    let share = if den > 0.0 { Some(((hf - he) + 0.5 * (ef - ee)) / den) } else { None };
    jobj! {
        "He" => he, "Hf" => hf, "Ee" => ee, "Ef" => ef, "share" => share,
        "formula" => "((future \u{2212} existing) households + 0.5 \u{d7} (future \u{2212} existing) employment) \u{f7} (future households + 0.5 \u{d7} future employment)",
        "label" => xdata("STD.labels.dcMethod"),
    }
}

/// The upgraded assets of a funding attribution and why they were chosen.
pub struct FundingAssets {
    /// The link ids.
    pub ids: Vec<String>,
    /// How they were chosen.
    pub basis: &'static str,
}

/// `fundingAssets(P, sc)`: the upgraded assets: the chosen link ids, else the roads named in the asset roads text, else the roads the option changes.
#[must_use]
pub fn funding_assets(p: &V, sc: &V) -> FundingAssets {
    let cx = data(p).g("funding").clone();
    let mut own: Vec<String> = Vec::new();
    let chosen = cx.g("assets").items().iter().filter(|id| sc.g("links").g(&id.s()).t()).cloned().collect::<Vec<_>>();
    for id in chosen.into_iter().chain(roads_to_links(sc, cx.g("assetRoads"))) {
        if !own.contains(&id.s()) {
            own.push(id.s());
        }
    }
    if own.is_empty() {
        let mut ids: Vec<String> = Vec::new();
        for id in changed_links(pair(p).base, sc) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        FundingAssets { ids, basis: "roads the option changes (choose the upgraded assets to change this)" }
    } else {
        FundingAssets { ids: own, basis: "chosen assets" }
    }
}

/// `fundingShares(P, opts)`: the flow on each upgraded asset from trips to or from each development zone (one select-zone assignment per zone and
/// period): `{ ok, at, scId, periods, basis, assets, method }` or `{ ok: false, why }`. Unless `opts.save` is false the result is stored in the
/// project's `funding.shares`. Options: `periods`, `gapTol`, `maxIter`.
pub fn funding_shares(p: &mut V, opts: &V) -> V {
    let out = {
        let pr = pair(p);
        let sc = pr.opt;
        let fa = funding_assets(p, sc);
        let zs = dev_zones(sc);
        let periods: Vec<String> = if opts.g("periods").t() { opts.g("periods").items().iter().map(crate::compliance::jv::Jx::s).collect() } else { periods_of(sc).into_iter().filter(|x| *x != "IP").map(std::string::ToString::to_string).collect() };
        if fa.ids.is_empty() {
            return jobj! { "ok" => false, "why" => "No upgraded assets: choose them (road names) or make the option change a road." };
        }
        if zs.is_empty() {
            return jobj! { "ok" => false, "why" => "No development zones in the option." };
        }
        if periods.is_empty() {
            return jobj! { "ok" => false, "why" => "Run the option first." };
        }
        let flow = |res: &V, id: &str| -> f64 {
            ["ab", "ba"].iter().fold(0.0, |sum, dir| {
                let fv = res.g("arcs").g(&format!("{id}:{dir}")).g("v");
                sum + if fv.t() { fv.n() } else { 0.0 }
            })
        };
        let mut assets: Vec<V> = Vec::new();
        for per in &periods {
            let owned;
            let res: &V = if let Some(found) = res_of(sc, per) { found } else {
                owned = engine::assign(p, sc, None, &jobj! { "period" => per.as_str() });
                &owned
            };
            let tot: HashMap<&String, f64> = fa.ids.iter().map(|id| (id, flow(res, id))).collect();
            // tot has every asset id, so the lookups below always find one
            let tot_of = |id: &String| tot.get(id).copied().unwrap_or(0.0);
            let mut by: HashMap<&String, Vec<(f64, V)>> = fa.ids.iter().map(|id| (id, Vec::new())).collect();
            for zone in &zs {
                let mut zopts = jobj! { "period" => per.as_str(), "select" => jobj! { "zone" => zone.g("id") } };
                if !opts.g("gapTol").nullish() {
                    zopts.set("gapTol", opts.g("gapTol").clone());
                }
                if !opts.g("maxIter").nullish() {
                    zopts.set("maxIter", opts.g("maxIter").clone());
                }
                let rz = engine::assign(p, sc, None, &zopts);
                let fl = if rz.g("select").t() && rz.g("select").g("flows").t() { rz.g("select").g("flows") } else { &NULL };
                for id in &fa.ids {
                    let zflow = ["ab", "ba"].iter().fold(0.0, |sum, dir| sum + n0(fl.g(&format!("{id}:{dir}"))));
                    if zflow > 0.0 {
                        let total = tot_of(id);
                        // by has every asset id too
                        if let Some(list) = by.get_mut(id) {
                            list.push((
                                zflow,
                                jobj! { "zone" => zone.g("id"), "name" => if zone.g("name").t() { zone.g("name").clone() } else { zone.g("id").clone() }, "v" => zflow, "share" => if total > 0.0 { Some(zflow / total) } else { None } },
                            ));
                        }
                    }
                }
            }
            for id in &fa.ids {
                if tot_of(id) > 0.0 {
                    let mut areas = by.remove(id).unwrap_or_default();
                    js_sort_by(&mut areas, |a, b| b.0 - a.0);
                    assets.push(jobj! { "id" => id, "name" => link_name(sc.g("links").g(id)), "period" => per.as_str(), "total" => tot_of(id), "areas" => arr(areas.into_iter().map(|x| x.1)) });
                }
            }
        }
        jobj! {
            "ok" => true, "at" => crate::edit::ids::iso_string(dates::now_ms()), "scId" => sc.g("id"), "periods" => strs(periods), "basis" => fa.basis, "assets" => arr(assets),
            "method" => "Select-zone assignment per development zone (Engine select { zone }); a trip between two development zones counts for both",
        }
    };
    // the record always has a funding object once its defaults are filled in
    if !opts.g("save").strict_eq(&V::Bool(false))
        && let Some(rec) = data_mut(p).and_then(|d| d.get_mut("funding")) {
        rec.set("shares", out.clone());
    }
    out
}

// The flows of `funding` on each upgraded asset by class (existing, committed, development) in each period the option ran
fn asset_flows(sc: &V, ps: &[&'static str], fa: &FundingAssets) -> Vec<V> {
    let mut assets: Vec<V> = Vec::new();
    for per in ps {
        // ps comes from periods_of(sc), so the results are there
        let Some(r) = res_of(sc, per) else { continue };
        let cls = if r.g("cls").t() { r.g("cls").g("arcs") } else { &NULL };
        if !cls.t() {
            continue;
        }
        for id in &fa.ids {
            let mut f = [0.0, 0.0, 0.0];
            for dir in ["ab", "ba"] {
                let arc_key = format!("{id}:{dir}");
                let x = cls.g(&arc_key);
                let vv = r.g("arcs").g(&arc_key).g("v");
                let v = if vv.t() { vv.n() } else { 0.0 };
                if x.t() {
                    f[0] += x.ix(0).n();
                    f[1] += x.ix(1).n();
                    f[2] += x.ix(2).n();
                } else {
                    f[0] += v;
                }
            }
            let total = f[0] + f[1] + f[2];
            if total > 0.0 {
                assets.push(jobj! { "id" => id, "name" => link_name(sc.g("links").g(id)), "period" => *per, "existing" => f[0], "committed" => f[1], "development" => f[2], "devShare" => f[2] / total });
            }
        }
    }
    assets
}

// The benefit incidence of `funding` in period p0: rule-of-half OD time savings, half to each end, inside vs outside the levy polygon
fn benefit_incidence(p: &V, pr: &Pair<'_>, p0: &'static str, poly: &[[f64; 2]]) -> V {
    let sc = pr.opt;
    // p0 is a period both scenarios ran, so both results are there
    let (Some(ro), Some(rb)) = (res_of(sc, p0), res_of(pr.base, p0)) else { return V::Null };
    let zo: Vec<String> = ro.g("zoneIds").items().iter().map(crate::compliance::jv::Jx::s).collect();
    let zb: Vec<String> = rb.g("zoneIds").items().iter().map(crate::compliance::jv::Jx::s).collect();
    let (zon, zbn) = (zo.len(), zb.len());
    if !(ro.g("T").t() && rb.g("T").t()) {
        return jobj! { "error" => "Benefit incidence needs the OD matrix (150 zones or fewer)." };
    }
    let ids2: Vec<String> = zo.iter().filter(|id| sc.g("zones").g(id).t()).cloned().collect();
    let from_b: Vec<String> = ids2.iter().filter(|id| pr.base.g("zones").g(id).t()).cloned().collect();
    let sb = skim(p, pr.base, Some(rb), &from_b, &ids2);
    let so = skim(p, sc, Some(ro), &ids2, &ids2);
    let mut by: Vec<(String, f64)> = Vec::new();
    let mut pos: HashMap<String, usize> = HashMap::new();
    for from_id in &ids2 {
        for to_id in &ids2 {
            let t0 = sb.get(from_id).and_then(|m| m.get(to_id));
            let t1 = so.get(from_id).and_then(|m| m.get(to_id));
            let (Some(t0), Some(t1)) = (t0, t1) else { continue };
            if from_id == to_id {
                continue;
            }
            let idx_o = zo.iter().position(|x| x == from_id).map_or(usize::MAX, |at| at.wrapping_mul(zon).wrapping_add(zo.iter().position(|x| x == to_id).unwrap_or(usize::MAX)));
            let qo = n0(ro.g("T").ix(idx_o));
            let (ib, jb) = (zb.iter().position(|x| x == from_id), zb.iter().position(|x| x == to_id));
            let qb = match (ib, jb) {
                // Bound: both positions are below the zone count, so the cell index is inside the matrix
                (Some(ib), Some(jb)) => n0(rb.g("T").ix(ib.saturating_mul(zbn).saturating_add(jb))),
                _ => 0.0,
            };
            let ben = f64::midpoint(qb, qo) * (t0 - t1) / 3600.0;
            if !truthy_number(ben) {
                continue;
            }
            for z in [from_id, to_id] {
                if let Some(&k) = pos.get(z) {
                    // pos only holds positions in by
                    if let Some(slot) = by.get_mut(k) {
                        let cur = slot.1;
                        slot.1 = (if truthy_number(cur) { cur } else { 0.0 }) + ben / 2.0;
                    }
                } else {
                    pos.insert(z.clone(), by.len());
                    by.push((z.clone(), 0.0 + ben / 2.0));
                }
            }
        }
    }
    let (mut inside, mut outside) = (0.0, 0.0);
    let mut zones: Vec<(f64, V)> = Vec::new();
    for (id, b) in &by {
        let z = sc.g("zones").g(id);
        let is_in = in_poly([z.g("lat").n(), z.g("lng").n()], poly);
        if is_in {
            inside += b;
        } else {
            outside += b;
        }
        zones.push((*b, jobj! { "id" => id, "name" => if z.g("name").t() { z.g("name").clone() } else { V::String(id.clone()) }, "benefit" => *b, "inside" => is_in }));
    }
    let tot = inside + outside;
    js_sort_by(&mut zones, |a, b| b.0 - a.0);
    jobj! {
        "period" => p0, "inside" => inside, "outside" => outside, "insideShare" => if truthy_number(tot) { Some(inside / tot) } else { None },
        "zones" => arr(zones.into_iter().take(15).map(|x| x.1)),
        "basis" => "Rule-of-half veh\u{b7}h saved per modelled peak hour, half to each trip end; the same share applies to PV benefits (one discounting profile)",
    }
}

/// `funding(P)`: the funding attribution: `{ assets, assetsBasis, shares, growth, incidence, bcrG, levy, dc, caveat }`.
pub fn funding(p: &V) -> V {
    let pr = pair(p);
    let cx = data(p).g("funding").clone();
    let sc = pr.opt;
    let ps = periods_of(sc);
    let fa = funding_assets(p, sc);
    let assets = asset_flows(sc, &ps, &fa);
    // benefit incidence: rule-of-half OD time savings, half to each end, inside vs outside the levy polygon
    let poly = polygon_of(cx.g("polygon"));
    let p0 = ps.iter().find(|x| res_of(pr.base, x).is_some()).copied();
    let incidence = match p0 {
        Some(per0) if poly.len() >= 3 && !pr.same => benefit_incidence(p, &pr, per0, &poly),
        _ => V::Null,
    };
    let e = if p0.is_some() { econ_of(p, pr.base, sc) } else { V::Null };
    let levy = n0(cx.g("levy"));
    let bcr_g = if e.t() && !e.g("mismatch").t() && !e.g("pv").nullish() {
        if e.g("cap").n() - levy > 0.0 {
            let v = e.g("pv").n() / (e.g("cap").n() - levy);
            jobj! {
                "value" => v,
                "text" => format!("{} (PV benefits {} \u{f7} (cost {} \u{2212} levy {}))", num(&V::Number(v), 1), fmt::money(e.g("pv").n()), fmt::money(e.g("cap").n()), fmt::money(levy)),
            }
        } else {
            jobj! { "value" => V::Null, "text" => "The levy covers the cost: BCR_G is not meaningful" }
        }
    } else {
        jobj! { "value" => V::Null, "text" => if e.t() && e.g("mismatch").t() { e.g("note").clone() } else { V::String("Run the base and the option".to_string()) } }
    };
    let appr = {
        let a = day(cx.g("approval"));
        if a.is_empty() { day(data(p).g("ftaa").g("approval")) } else { a }
    };
    let sh = if cx.g("shares").t() && cx.g("shares").g("ok").t() && cx.g("shares").g("scId").strict_eq(sc.g("id")) { cx.g("shares").clone() } else { V::Null };
    let dc = if appr.is_empty() {
        V::Null
    } else {
        let months = xdata("STD.ftaa.dcMonths").n();
        let until = add_months(&V::String(appr.clone()), months);
        let until_v = until.map_or(V::Null, V::String);
        jobj! {
            "approval" => appr.as_str(), "until" => &until_v,
            "text" => format!(
                "Fast-track approval {}: the development contributions policy window under LGA s102A(3) runs to {}.",
                long_date(&V::String(appr.clone())),
                long_date(&until_v)
            ),
        }
    };
    jobj! {
        "assets" => arr(assets), "assetsBasis" => fa.basis, "shares" => sh, "growth" => growth_share(p, sc), "incidence" => incidence, "bcrG" => bcr_g, "levy" => levy, "dc" => dc,
        "caveat" => xdata("STD.labels.funding"),
    }
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-gps", "compliance-extra-funding"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
#[must_use]
pub fn report(id: &str, p: &V, _ctx: Option<&dyn ReportCtx>) -> Option<String> {
    match id {
        "compliance-extra-gps" => {
            if !context(p).g("kind").eq_s("fund") {
                return Some(String::new());
            }
            let g = gps_metrics(p);
            let net = g.g("network");
            let dash = || "\u{2013}".to_string();
            let rows: Vec<Vec<String>> = vec![
                vec![
                    "Scheme network travel-time saving".to_string(),
                    if net.g("saving").nullish() { dash() } else { format!("{}% ({})", num(net.g("saving"), 1), esc(net.g("significance"))) },
                    format!(
                        "HCV {}; general traffic {}",
                        if net.g("hcv").t() { esc(net.g("hcv")) } else { dash() },
                        if net.g("gt").t() { esc(net.g("gt")) } else { dash() }
                    ),
                ],
                vec!["Houses enabled (75% by 2035)".to_string(), fmt::int(g.g("houses").g("total").n()), String::new()],
                vec![
                    "DSI reduction per NZ$100m".to_string(),
                    if g.g("dsi").g("per100m").nullish() { dash() } else { num(g.g("dsi").g("per100m"), 2) },
                    esc(g.g("dsi").g("collective")),
                ],
            ];
            Some(format!(
                "<p class=\"note\"><b>{}</b></p>{}<p class=\"note\">{}</p>",
                esc(g.g("label")),
                t_table(&["Metric", "Value", "Draft rating"], rows, ""),
                esc(net.g("note"))
            ))
        }
        "compliance-extra-funding" => {
            if !context(p).g("kind").eq_s("dc") {
                return Some(String::new());
            }
            let f = funding(p);
            let gr = f.g("growth");
            let inc = f.g("incidence");
            let shares = if f.g("shares").t() {
                let rows: Vec<Vec<String>> = f
                    .g("shares")
                    .g("assets")
                    .items()
                    .iter()
                    .flat_map(|a| {
                        a.g("areas").items().iter().take(5).map(|z| vec![esc(a.g("name")), esc(a.g("period")), esc(z.g("name")), fmt::pct(z.g("share").n(), 0)]).collect::<Vec<_>>()
                    })
                    .collect();
                format!(
                    "{}<p class=\"note\">{}.</p>",
                    t_table(&["Asset", "Period", "Development area", "Share of the asset flow"], rows, ""),
                    esc(f.g("shares").g("method"))
                )
            } else {
                String::new()
            };
            Some(format!(
                "<p class=\"note\"><b>{}</b></p><p>Growth share {} ({}; {}). {} BCR_G with the levy as Cp: {}.</p>{}",
                esc(f.g("caveat")),
                if gr.g("share").nullish() { "\u{2013}".to_string() } else { fmt::pct(gr.g("share").n(), 1) },
                esc(gr.g("formula")),
                esc(gr.g("label")),
                if inc.t() && !inc.g("insideShare").nullish() { format!("Benefit inside the levy area {}.", fmt::pct(inc.g("insideShare").n(), 0)) } else { String::new() },
                esc(f.g("bcrG").g("text")),
                shares
            ))
        }
        _ => None,
    }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &[];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
#[must_use]
pub fn export_build(_id: &str, _p: &V) -> Option<String> {
    None
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
#[must_use]
pub fn export_enabled(_id: &str, _p: &V) -> Option<bool> {
    None
}
