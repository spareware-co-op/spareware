//! Time-of-use charging screening (compliance-extra.js section 17). Available from 18 Nov 2026 (commencement).
//!
//! The charge (light and heavy averaged by the heavy-vehicle share: Spare Construct has one vehicle class) is an additive generalised-cost term on
//! each charged road (charge divided by the value of time, in seconds), passed to the assignment as `arcPenalty`: it steers route choice and the
//! equilibrium, but the travel times, VHT and delay of a result never include it.

use super::ctx::{
    arc_link, assumptions, data, data_mut, esc, is_sh, long_date_s, num, pair, res_of, stamped,
    today,
};
use super::gps::{in_poly, polygon_of};
use super::packs::{n0, r1, r3, sv, t_table};
use crate::compliance::data::x as xdata;
use crate::compliance::dates;
use crate::compliance::engine;
use crate::compliance::html::{ReportCtx, rt};
use crate::compliance::jv::{Jx, NULL, V, arr, csv, jobj, ns};
use crate::graph::{Graph, js_sort_by};
use crate::jsnum::{math_max, math_round, truthy_number};
use std::collections::{HashMap, HashSet};

/// `touCharged(sc, poly, kind)`: the roads the charge applies to and how: for an area, the roads inside it ("inside"); for a cordon, the roads that
/// cross it ("leaves" or "enters"). In the scenario's road order.
pub fn tou_charged(sc: &V, poly: &[[f64; 2]], kind: &str) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("closed") {
            continue;
        }
        let (a, b) = (
            sc.g("nodes").g(&l.g("a").s()),
            sc.g("nodes").g(&l.g("b").s()),
        );
        if !a.t() || !b.t() {
            continue;
        }
        let ia = in_poly([a.g("lat").n(), a.g("lng").n()], poly);
        let ib = in_poly([b.g("lat").n(), b.g("lng").n()], poly);
        let hit = if kind == "area" { ia && ib } else { ia != ib };
        if hit {
            let how = if kind == "area" {
                "inside"
            } else if ia {
                "leaves"
            } else {
                "enters"
            };
            let id = l.g("id").s();
            if let Some(e) = out.iter_mut().find(|x| x.0 == id) {
                e.1 = how;
            } else {
                out.push((id, how));
            }
        }
    }
    out
}

/// The times between a pair of zones on a time-of-use run: generalised time `gc` (s), the charged roads crossed `n`, and the real time `real` (s).
#[derive(Clone, Copy, Debug)]
pub struct TouTime {
    /// Generalised time: real time plus the charge as time on each charged road.
    pub gc: f64,
    /// Charged roads on the route.
    pub n: f64,
    /// Real time (generalised time less the charges).
    pub real: f64,
}

/// `touSkim(G, r, charged, applied, pen, zs, S)`: zone-to-zone generalised and real times on a time-of-use run: each arc costs its real time in the result
/// plus the charge (`pen`, s) on charged roads, so the shortest route sees the true charge. `applied` is the time an earlier method added to an
/// arc, taken off with its congestion factor (older callers).
pub fn tou_skim(
    p: &V,
    g: &Graph,
    r: Option<&V>,
    charged: Option<&[(String, &'static str)]>,
    applied: Option<&HashMap<String, f64>>,
    pen: f64,
    zs: &[String],
) -> HashMap<String, HashMap<String, TouTime>> {
    let proj = engine::project(p);
    let perf = crate::junction::PerfSet::new(&proj.settings);
    let set: Option<HashSet<&str>> = charged.map(|c| c.iter().map(|x| x.0.as_str()).collect());
    let mut cost = vec![0.0; g.arcs.len()];
    let mut chg = vec![false; g.arcs.len()];
    for (i, a) in g.arcs.iter().enumerate() {
        let x = r.map_or(&NULL, |r| r.g("arcs").g(&a.key));
        let ap = applied.map_or(0.0, |m| {
            let v = m.get(&a.key).copied().unwrap_or(0.0);
            if truthy_number(v) { v } else { 0.0 }
        });
        let mut t = if x.t() {
            x.get("t").map_or(f64::NAN, |t| t.n())
        } else {
            crate::junction::arc_time(a, 0.0, &perf)
        };
        if x.t() && ap > 0.0 && x.g("ff").n() > 0.0 {
            t = math_max(
                0.1,
                t - ap * math_max(1.0, (x.g("t").n() - n0(x.g("nd"))) / x.g("ff").n()),
            );
        }
        if let Some(s) = &set {
            if !a.conn && s.contains(a.link.as_str()) {
                chg[i] = true;
                t += pen;
            }
        }
        cost[i] = t;
    }
    let mut w = g.workspace();
    let mut out: HashMap<String, HashMap<String, TouTime>> = HashMap::new();
    for f in zs {
        let Some(&s) = g.idx.get(&format!("z:{f}")) else {
            continue;
        };
        g.spt(&cost, s, &mut w, -1, -1);
        let mut m: HashMap<String, TouTime> = HashMap::new();
        for t in zs {
            if t == f {
                continue;
            }
            let Some(&i) = g.idx.get(&format!("z:{t}")) else {
                continue;
            };
            if !w.nd[i].is_finite() {
                continue;
            }
            let mut n = 0.0;
            if charged.is_some() {
                for ai in g.path_to(&w, i).unwrap_or_default() {
                    if chg[ai] {
                        n += 1.0;
                    }
                }
            }
            m.insert(
                t.clone(),
                TouTime {
                    gc: w.nd[i],
                    n,
                    real: w.nd[i] - n * pen,
                },
            );
        }
        out.insert(f.clone(), m);
    }
    out
}

struct Od {
    i: String,
    j: String,
    from: V,
    to: V,
    q0: f64,
    q1: f64,
    t0: f64,
    /// Set when the pair was reached in the network run: `(t1, n, paid, pay)`.
    after: Option<(f64, f64, bool, f64)>,
}

struct Inc {
    id: String,
    name: V,
    trips: f64,
    paid_trips: f64,
    d_gc: f64,
    d_real: f64,
    income: Option<V>,
}

struct NetTotals {
    vkt: f64,
    vht: f64,
    co2: f64,
}

impl NetTotals {
    fn to_v(&self) -> V {
        jobj! { "vkt" => self.vkt, "vht" => self.vht, "co2" => self.co2 }
    }
}

fn od_sample(o: &Od) -> V {
    let (t1, _, paid, pay) = o.after.expect("sampled pair");
    jobj! { "from" => &o.from, "to" => &o.to, "q0" => o.q0, "q1" => o.q1, "t0" => o.t0, "t1" => t1, "paid" => paid, "pay" => pay }
}

/// `touRun(P, opts)`: the time-of-use screening of the project's charging area: `{ ok: false, why }` or the full result (charge, demand factor, sampled
/// trips, network impacts, incidence by zone, monetised summary and BCRs). Options: `today`, `period`, `save` (unless false the result is stored in the
/// project's `tou.result`).
pub fn tou_run(p: &mut V, opts: &V) -> V {
    let res = tou_compute(p, opts);
    if res.g("ok").t() && !opts.g("save").strict_eq(&V::Bool(false)) {
        data_mut(p)
            .get_mut("tou")
            .expect("tou record")
            .set("result", res.clone());
    }
    res
}

fn tou_compute(p: &V, opts: &V) -> V {
    let cx = data(p).g("tou").clone();
    let now = if opts.g("today").t() {
        opts.g("today").s()
    } else {
        today()
    };
    let std_tou = xdata("STD.dates.tou").s();
    if now.as_str() < std_tou.as_str() {
        return jobj! { "ok" => false, "why" => format!("Time-of-use schemes can be proposed from {}, when LTMA ss65A-65ZO commence.", long_date_s(&std_tou)) };
    }
    let poly = polygon_of(cx.g("polygon"));
    if poly.len() < 3 {
        return jobj! { "ok" => false, "why" => "Draw or enter the charging area (at least 3 points)." };
    }
    let pr = pair(p);
    let sc = pr.opt;
    let period = if opts.g("period").t() {
        opts.g("period").s()
    } else if cx.g("period").t() {
        cx.g("period").s()
    } else {
        "AM".to_string()
    };
    let s_ = p.g("settings");
    let e_set = if s_.g("econ").t() {
        s_.g("econ")
    } else {
        &NULL
    };
    let a = assumptions(p);
    let hv = n0(s_.g("hvPct")) / 100.0;
    let charge = n0(cx.g("light")) * (1.0 - hv) + n0(cx.g("heavy")) * hv;
    let vot = {
        let v = e_set.g("vot").n();
        if truthy_number(v) { v } else { 32.0 }
    };
    let pen = charge / vot * 3600.0;
    let kind = if cx.g("kind").eq_s("area") {
        "area"
    } else {
        "cordon"
    };
    let charged = tou_charged(sc, &poly, kind);
    if charged.is_empty() {
        return jobj! { "ok" => false, "why" => "No road crosses (or lies inside) the charging area." };
    }
    let owned0;
    let r0: &V = match res_of(sc, &period) {
        Some(r) => r,
        None => {
            owned0 = engine::assign(p, sc, None, &jobj! { "period" => period.as_str() });
            &owned0
        }
    };
    if !r0.g("T").t() || r0.g("zoneIds").items().is_empty() {
        return jobj! { "ok" => false, "why" => "The screening needs the OD matrix, which Spare Construct keeps for 150 zones or fewer." };
    }
    // the charge as an additive cost on every charged link, both directions (Engine.assign opts.arcPenalty, seconds)
    let mut arc_penalty = V::object();
    for (id, _) in &charged {
        arc_penalty.set(id.as_str(), V::Number(pen));
    }
    let g1 = engine::build_graph(p, sc, None);
    let zone_ids: Vec<String> = r0.g("zoneIds").items().iter().map(|x| x.s()).collect();
    let zs: Vec<String> = zone_ids
        .iter()
        .filter(|id| sc.g("zones").g(id).t())
        .cloned()
        .collect();
    let zn = zone_ids.len();
    let zi: HashMap<&String, usize> = zone_ids.iter().enumerate().map(|(k, id)| (id, k)).collect();
    let s0 = tou_skim(p, &g1, Some(r0), None, None, 0.0, &zs);
    let r1_ = engine::assign(
        p,
        sc,
        None,
        &jobj! { "period" => period.as_str(), "arcPenalty" => &arc_penalty },
    );
    let s1 = tou_skim(p, &g1, Some(&r1_), Some(&charged), None, pen, &zs);
    // one elastic-demand step on generalised time
    let mut ods: Vec<Od> = Vec::new();
    let (mut t0s, mut t1s) = (0.0, 0.0);
    let elasticity = a.g("elasticity").n();
    for i in &zs {
        for j in &zs {
            if i == j {
                continue;
            }
            let q = n0(r0.g("T").ix(zi[i] * zn + zi[j]));
            let b0 = s0.get(i).and_then(|m| m.get(j));
            let b1 = s1.get(i).and_then(|m| m.get(j));
            let (Some(b0), Some(b1)) = (b0, b1) else {
                continue;
            };
            if !(q > 0.0) || !(b0.gc > 0.0) {
                continue;
            }
            let q1 = q * crate::jsmath::pow(b1.gc / b0.gc, elasticity);
            t0s += q;
            t1s += q1;
            let nm = |id: &String| {
                let z = sc.g("zones").g(id);
                if z.g("name").t() {
                    z.g("name").clone()
                } else {
                    V::String(id.clone())
                }
            };
            ods.push(Od {
                i: i.clone(),
                j: j.clone(),
                from: nm(i),
                to: nm(j),
                q0: q,
                q1,
                t0: b0.gc,
                after: None,
            });
        }
    }
    let d_f = if t0s > 0.0 { t1s / t0s } else { 1.0 };
    let r2 = engine::assign(
        p,
        sc,
        None,
        &jobj! { "period" => period.as_str(), "arcPenalty" => &arc_penalty, "demandFactor" => d_f },
    );
    let s2 = tou_skim(p, &g1, Some(&r2), Some(&charged), None, pen, &zs);
    // rule of half per OD on generalised cost; revenue and the deadweight loss of priced-off trips (NZ$ per modelled hour)
    let (mut user, mut time, mut revenue, mut dwl) = (0.0, 0.0, 0.0, 0.0);
    let mut inc: Vec<Inc> = Vec::new();
    for o in ods.iter_mut() {
        let Some(b) = s2.get(&o.i).and_then(|m| m.get(&o.j)) else {
            continue;
        };
        let pay = b.n * charge;
        o.after = Some((b.real, b.n, b.n > 0.0, pay));
        user += 0.5 * (o.q0 + o.q1) * (o.t0 - b.gc) / 3600.0 * vot;
        time += 0.5 * (o.q0 + o.q1) * (o.t0 - b.real) / 3600.0 * vot;
        revenue += o.q1 * pay;
        dwl += 0.5 * (o.q0 - o.q1) * pay;
        let k = match inc.iter().position(|z| z.id == o.i) {
            Some(k) => k,
            None => {
                let cxz = sc.g("zones").g(&o.i).g("cx");
                inc.push(Inc {
                    id: o.i.clone(),
                    name: o.from.clone(),
                    trips: 0.0,
                    paid_trips: 0.0,
                    d_gc: 0.0,
                    d_real: 0.0,
                    income: if cxz.t() && cxz.has("income") {
                        Some(cxz.g("income").clone())
                    } else {
                        None
                    },
                });
                inc.len() - 1
            }
        };
        let z = &mut inc[k];
        z.trips += o.q0;
        if b.n > 0.0 {
            z.paid_trips += o.q1;
        }
        z.d_gc += o.q0 * (b.gc - o.t0);
        z.d_real += o.q0 * (b.real - o.t0);
    }
    // network VKT, real VHT and CO2 (res.arcs t is real time: the charge is never in it)
    let proj = engine::project(p);
    let by_net = |r: &V, sh: Option<bool>| -> NetTotals {
        let (mut vkt, mut vht, mut co2) = (0.0, 0.0, 0.0);
        for (k, x) in r.g("arcs").entries() {
            let l = sc.g("links").g(&arc_link(k));
            if !l.t()
                || (sh.is_some() && is_sh(p, l) != sh.unwrap_or(false))
                || !(x.g("len").n() > 0.0)
            {
                continue;
            }
            let t = x.g("t").n();
            vkt += x.g("v").n() * x.g("len").n() / 1000.0;
            vht += x.g("v").n() * t / 3600.0;
            co2 += x.g("v").n() * x.g("len").n() / 1000.0
                * crate::standards::ef(x.g("len").n() / t * 3.6, &proj.settings)
                / 1e6;
        }
        NetTotals { vkt, vht, co2 }
    };
    let n0t = by_net(r0, None);
    let n2t = by_net(&r2, None);
    let pt = |r: &V| {
        r.g("routes")
            .vals()
            .iter()
            .fold(0.0, |s, x| s + n0(x.g("t")))
    };
    let hrs = {
        let h = e_set.g("periodHours").g(&period);
        let h = if h.nullish() { 600.0 } else { h.n() };
        if truthy_number(h) { h } else { 600.0 }
    };
    let std_tab = engine::standards_table();
    let mb = if std_tab.g("mbcm").t() {
        std_tab.g("mbcm")
    } else {
        &NULL
    };
    let rates: V = if e_set.g("rates").is_arr() && !e_set.g("rates").items().is_empty() {
        e_set.g("rates").clone()
    } else if mb.g("rates").t() {
        mb.g("rates").clone()
    } else {
        arr([
            jobj! { "to" => 30, "r" => 2 },
            jobj! { "to" => 60, "r" => 1.5 },
        ])
    };
    let yrs = {
        let y = e_set.g("years").n();
        let y = if truthy_number(y) {
            y
        } else {
            let m = mb.g("years").n();
            if truthy_number(m) { m } else { 40.0 }
        };
        math_max(1.0, math_round(y))
    };
    let m = |x: f64| x * hrs / 1e6;
    let voc = m((n0t.vkt - n2t.vkt) * a.g("vocPerKm").n());
    let co2v = m((n0t.co2 - n2t.co2) * a.g("co2PerT").n());
    let safety = m((n0t.vkt - n2t.vkt) * a.g("crashPerKm").n());
    let (a_user, a_time, a_charges, a_dwl, a_revenue) =
        (m(user), m(time), -m(revenue + dwl), m(dwl), m(revenue));
    let ext = voc + co2v + safety;
    let a_total = a_user + a_revenue + ext;
    let a_gov = a_user + ext;
    let annual = jobj! {
        "user" => a_user, "time" => a_time, "charges" => a_charges, "deadweight" => a_dwl, "revenue" => a_revenue, "voc" => voc, "co2" => co2v, "safety" => safety, "total" => a_total, "government" => a_gov,
    };
    let uspwf = engine::pwf(&rates, yrs).g("uspwf").n();
    let (pv_b, pv_g, pv_r) = (a_total * uspwf, a_gov * uspwf, a_revenue * uspwf);
    let cost = n0(cx.g("cost"));
    let mut paid: Vec<&Od> = ods
        .iter()
        .filter(|o| o.after.is_some_and(|x| x.2))
        .collect();
    let mut unpaid: Vec<&Od> = ods
        .iter()
        .filter(|o| o.after.is_some_and(|x| !x.2))
        .collect();
    js_sort_by(&mut paid, |x, y| y.q0 - x.q0);
    js_sort_by(&mut unpaid, |x, y| y.q0 - x.q0);
    let sample: Vec<V> = paid
        .iter()
        .take(12)
        .chain(unpaid.iter().take(4))
        .map(|o| od_sample(o))
        .collect();
    let mut incidence: Vec<(f64, V)> = inc
        .iter()
        .map(|z| {
            let mut o = jobj! { "id" => z.id.as_str(), "name" => &z.name, "trips" => z.trips, "paidTrips" => z.paid_trips, "dGc" => z.d_gc, "dReal" => z.d_real };
            if let Some(i) = &z.income {
                o.set("income", i.clone());
            }
            o.set("dtAvg", V::Number(if truthy_number(z.trips) { z.d_real / z.trips } else { 0.0 }));
            o.set("dGcAvg", V::Number(if truthy_number(z.trips) { z.d_gc / z.trips } else { 0.0 }));
            o.set("paidShare", V::Number(if truthy_number(z.trips) { z.paid_trips / z.trips } else { 0.0 }));
            (z.paid_trips, o)
        })
        .collect();
    js_sort_by(&mut incidence, |x, y| y.0 - x.0);
    let rate_text = rates
        .items()
        .iter()
        .enumerate()
        .map(|(k, s)| {
            format!(
                "{}%{}",
                s.g("r").s(),
                if k > 0 {
                    format!(" to year {}", s.g("to").s())
                } else {
                    format!(" years 1-{}", s.g("to").s())
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", then ");
    let appraisal = xdata("STD.tou.appraisal").s();
    let method = format!(
        "Charge NZ${} (light {}, heavy {}, HV {}%; one vehicle class) as {} min at NZ${}/h of generalised cost on {} charged road{}{}, in route choice and the equilibrium (Spare Construct's additive arc cost; travel times, VHT and delay exclude it)",
        num(&V::Number(charge), 2),
        cx.g("light").s(),
        cx.g("heavy").s(),
        num(&V::Number(hv * 100.0), 0),
        num(&V::Number(pen / 60.0), 1),
        ns(vot),
        charged.len(),
        if charged.len() == 1 { "" } else { "s" },
        if cx.g("kind").eq_s("area") {
            ""
        } else {
            " (both directions of each cordon crossing)"
        }
    ) + &format!(
        "; the gravity distribution does not see the charge; one elastic step (elasticity {}, {}) gives a demand factor of {}, applied to every OD pair in the network run",
        a.g("elasticity").s(),
        xdata("ASSUMPTION").s(),
        num(&V::Number(d_f), 3)
    ) + &format!(
        ". {}. Discounted at {} over {} years, benefits flat (NZTA GC 25/01)",
        appraisal,
        rate_text,
        ns(yrs)
    );
    let impacts = jobj! {
        "sh" => arr([by_net(r0, Some(true)).to_v(), by_net(&r2, Some(true)).to_v()]),
        "local" => arr([by_net(r0, Some(false)).to_v(), by_net(&r2, Some(false)).to_v()]),
        "pt" => arr([V::Number(pt(r0)), V::Number(pt(&r2))]),
        "total" => arr([n0t.to_v(), n2t.to_v()]),
    };
    jobj! {
        "ok" => true, "at" => crate::edit::ids::iso_string(dates::now_ms()), "period" => period.as_str(), "charge" => charge, "pen" => pen, "vot" => vot, "kind" => cx.g("kind"),
        "charged" => charged.len(), "demandFactor" => d_f, "leaks" => arr([]),
        "sample" => arr(sample), "impacts" => impacts, "incidence" => arr(incidence.into_iter().take(20).map(|x| x.1)),
        "annual" => annual, "revenue" => a_revenue, "pvB" => pv_b, "pvG" => pv_g, "pvR" => pv_r, "cost" => cost, "uspwf" => uspwf, "years" => yrs, "rates" => rates,
        "bcrN" => if cost > 0.0 { Some(pv_b / cost) } else { None },
        "bcrG" => if cost - pv_r > 0.0 { Some(pv_g / (cost - pv_r)) } else { None },
        "bcrGText" => if cost > 0.0 && cost - pv_r <= 0.0 { "Revenue covers the cost: BCR_G is not meaningful" } else { "" },
        "label" => xdata("STD.labels.tou"), "appraisal" => appraisal.as_str(), "method" => method,
    }
}

/// `touCSV(P)`: the stored time-of-use screening as a stamped CSV (a note when it has not been run).
pub fn tou_csv(p: &V) -> String {
    let r = data(p).g("tou").g("result").clone();
    if !r.t() || !r.g("ok").t() {
        return stamped(
            &csv(&[vec![sv("Time-of-use screening has not been run.")]]),
            p,
            None,
        );
    }
    let a = r.g("annual");
    let imp = r.g("impacts");
    let s = sv;
    let head = |h: &[&str]| -> Vec<V> { h.iter().map(|x| s(x)).collect() };
    let mut rows: Vec<Vec<V>> = vec![
        vec![s("Time-of-use screening"), r.g("label").clone()],
        vec![s("Method"), r.g("method").clone()],
        vec![],
        vec![s("Sampled trips (s65D(3))")],
        head(&[
            "From",
            "To",
            "Trips before (veh/h)",
            "Trips after (veh/h)",
            "Travel time before (min)",
            "Travel time after (min)",
            "Charge paid (NZ$)",
            "Charged",
        ]),
    ];
    for o in r.g("sample").items() {
        rows.push(vec![
            o.g("from").clone(),
            o.g("to").clone(),
            r1(o.g("q0")),
            r1(o.g("q1")),
            r1(&V::Number(o.g("t0").n() / 60.0)),
            r1(&V::Number(o.g("t1").n() / 60.0)),
            if !o.g("pay").nullish() {
                r3(o.g("pay"))
            } else {
                s("")
            },
            s(if o.g("paid").t() { "yes" } else { "no" }),
        ]);
    }
    rows.push(vec![]);
    rows.push(head(&[
        "Network impacts (real travel times)",
        "VKT before",
        "VKT after",
        "VHT before",
        "VHT after",
    ]));
    let net_row = |label: &str, k: &str| -> Vec<V> {
        let (b, a2) = (imp.g(k).ix(0), imp.g(k).ix(1));
        vec![
            s(label),
            r1(b.g("vkt")),
            r1(a2.g("vkt")),
            r1(b.g("vht")),
            r1(a2.g("vht")),
        ]
    };
    rows.push(net_row("State highways", "sh"));
    rows.push(net_row("Local roads", "local"));
    rows.push(vec![
        s("Bus routes (total time, s)"),
        r1(imp.g("pt").ix(0)),
        r1(imp.g("pt").ix(1)),
    ]);
    rows.push(vec![]);
    rows.push(vec![s("Incidence by zone")]);
    rows.push(head(&[
        "Zone",
        "Trips (veh/h)",
        "Charged share",
        "Average travel time change (s)",
        "Average generalised cost change (s, charge as time)",
        "Income proxy",
    ]));
    for z in r.g("incidence").items() {
        rows.push(vec![
            z.g("name").clone(),
            r1(z.g("trips")),
            r3(z.g("paidShare")),
            r1(z.g("dtAvg")),
            r1(z.g("dGcAvg")),
            if z.g("income").nullish() {
                s("")
            } else {
                z.g("income").clone()
            },
        ]);
    }
    rows.push(vec![]);
    rows.push(head(&[
        "Monetised summary (s65ZD), NZ$m a year",
        "User benefit (rule of half, charge included)",
        "of which time",
        "of which charges paid",
        "Revenue (transfer)",
        "Deadweight loss",
        "VOC",
        "CO2",
        "Safety",
        "National total",
    ]));
    rows.push(vec![
        s(""),
        r3(a.g("user")),
        r3(a.g("time")),
        r3(a.g("charges")),
        r3(a.g("revenue")),
        r3(a.g("deadweight")),
        r3(a.g("voc")),
        r3(a.g("co2")),
        r3(a.g("safety")),
        r3(a.g("total")),
    ]);
    rows.push(vec![s("Appraisal"), r.g("appraisal").clone()]);
    rows.push(vec![
        s("BCR_N"),
        if !r.g("bcrN").nullish() {
            r1(r.g("bcrN"))
        } else {
            s("enter the scheme cost")
        },
    ]);
    rows.push(vec![
        s("BCR_G (revenue as Cp)"),
        if !r.g("bcrG").nullish() {
            r1(r.g("bcrG"))
        } else if r.g("bcrGText").t() {
            r.g("bcrGText").clone()
        } else {
            s("enter the scheme cost")
        },
    ]);
    stamped(&csv(&rows), p, None)
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-tou"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-tou" {
        return None;
    }
    let r = data(p).g("tou").g("result").clone();
    if !r.t() || !r.g("ok").t() {
        return Some(String::new());
    }
    let rows: Vec<Vec<String>> = r
        .g("sample")
        .items()
        .iter()
        .map(|o| {
            vec![
                esc(o.g("from")),
                esc(o.g("to")),
                num(o.g("q0"), 1),
                num(o.g("q1"), 1),
                num(&V::Number(o.g("t0").n() / 60.0), 1),
                num(&V::Number(o.g("t1").n() / 60.0), 1),
                if o.g("paid").t() {
                    num(o.g("pay"), 2)
                } else {
                    "\u{2013}".to_string()
                },
            ]
        })
        .collect();
    let tbl = t_table(
        &[
            "From",
            "To",
            "Trips before",
            "Trips after",
            "Time before (min)",
            "Time after (min)",
            "Charge (NZ$)",
        ],
        rows,
        "",
    );
    Some(format!(
        "<p class=\"note\"><b>{}</b></p>{}",
        esc(r.g("label")),
        rt(
            ctx,
            "Sampled trips before and after the charge (s65D(3))",
            &tbl,
            &esc(r.g("method"))
        )
    ))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-tou"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-tou").then(|| tou_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-tou").then(|| {
        let r = data(p).g("tou").g("result").clone();
        r.t() && r.g("ok").t()
    })
}
