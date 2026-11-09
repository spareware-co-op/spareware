//! Zone-to-zone travel times, the links a proposal affects and policy alignment (compliance-extra.js sections 13 and 14).

use super::anop::{MODES, anop};
use super::climate::climate;
use super::ctx::{
    GFA_KEYS, arc_link, assumptions, data, dev_zones, esc, esc_s, link_name, lu_of, nn, num, pair,
    periods_of, posted, res_of, stamped,
};
use super::gps::{funding, gps_metrics};
use super::options::rltp_relied;
use super::packs::{n0, or_blank, r1, r3, res_ok, sv, t_table};
use super::shared::{changed_links, crashes_on, econ_of, kpi_p, neg_pp, per_period, pp_text};
use super::subdivision::dist_skim;
use crate::compliance::data::x as xdata;
use crate::compliance::engine;
use crate::compliance::html::{ReportCtx, rt};
use crate::compliance::jv::{Jx, NULL, V, arr, csv, jobj, ns, strs};
use crate::compliance::rx::rx;
use crate::graph::js_sort_by;
use crate::jsnum::{math_round, number_to_js_string, to_int32, truthy_number};
use crate::util::fmt;
use std::collections::HashMap;

/// The shortest times between zones: from-zone id, then to-zone id, then seconds (only the pairs that were reached).
pub type SkimMap = HashMap<String, HashMap<String, f64>>;

/// `skim(sc, res, fromIds, toIds)`: shortest paths between zones on the times a result assigned (free-flow where an arc has no result).
pub fn skim(p: &V, sc: &V, res: Option<&V>, from_ids: &[String], to_ids: &[String]) -> SkimMap {
    let g = engine::build_graph(p, sc, None);
    let proj = engine::project(p);
    let perf = crate::junction::PerfSet::new(&proj.settings);
    let cost: Vec<f64> = g
        .arcs
        .iter()
        .map(|a| {
            let r = res.map_or(&NULL, |r| r.g("arcs").g(&a.key));
            if r.t() {
                r.get("t").map_or(f64::NAN, |t| t.n())
            } else {
                crate::junction::arc_time(a, 0.0, &perf)
            }
        })
        .collect();
    let mut w = g.workspace();
    let mut out: SkimMap = HashMap::new();
    for f in from_ids {
        let Some(&s) = g.idx.get(&format!("z:{f}")) else {
            continue;
        };
        g.spt(&cost, s, &mut w, -1, -1);
        let mut m: HashMap<String, f64> = HashMap::new();
        for t in to_ids {
            if let Some(&i) = g.idx.get(&format!("z:{t}")) {
                if w.nd[i].is_finite() {
                    m.insert(t.clone(), w.nd[i]);
                }
            }
        }
        out.insert(f.clone(), m);
    }
    out
}

/// `jobsOf(z)`: the jobs in a zone (`jobs.n`, else `employees`, else 0).
pub fn jobs_of(z: &V) -> f64 {
    if z.g("jobs").t() {
        let a = z.g("jobs").g("n").n();
        if truthy_number(a) {
            return a;
        }
    }
    n0(z.g("employees"))
}

/// `placeOf(L)`: the RASF place level (1 to 3) of a road: the one entered, else the one named in its RASF text.
pub fn place_of(l: &V) -> Option<f64> {
    let x = if l.g("cx").t() { l.g("cx") } else { &NULL };
    if nn(x.g("place")).is_some() {
        return Some(x.g("place").n());
    }
    let text = if x.g("rasf").t() {
        x.g("rasf").s()
    } else {
        String::new()
    };
    rx(r"P\s?([1-3])", "i")
        .exec(&text)
        .map(|m| m.get(1).parse::<f64>().unwrap_or(f64::NAN))
}

/// `limitOf(L)`: the limit the author proposes for a road, else its posted limit.
pub fn limit_of(l: &V) -> f64 {
    if l.g("cx").t() {
        if let Some(v) = nn(l.g("cx").g("limit")) {
            if truthy_number(v) {
                return v;
            }
        }
    }
    posted(l)
}

/// The roads a proposal affects and why they were chosen.
pub struct Affected {
    /// The road ids.
    pub ids: Vec<V>,
    /// How they were chosen.
    pub basis: String,
}

fn push_unique(ids: &mut Vec<V>, v: V) {
    if !ids.iter().any(|x| x.strict_eq(&v)) {
        ids.push(v);
    }
}

/// `affectedLinks(P)`: the roads the option affects: those of the stored extent-of-effects test, else those whose flow changes by at least 20 veh/h
/// and the assumed percentage, plus every changed road.
pub fn affected_links(p: &V) -> Affected {
    let pr = pair(p);
    let sc = pr.opt;
    let a = assumptions(p);
    let ext = sc.g("extent");
    if ext.t() && ext.g("links").is_arr() && !ext.g("links").items().is_empty() {
        let mut ids: Vec<V> = Vec::new();
        for x in ext.g("links").items() {
            let id = if x.is_str() {
                let s = x.s();
                V::String(arc_link(&if s.contains(':') {
                    s
                } else {
                    format!("{s}:ab")
                }))
            } else if x.g("link").t() {
                x.g("link").clone()
            } else {
                x.g("id").clone()
            };
            push_unique(&mut ids, id);
        }
        return Affected {
            ids,
            basis: "stored extent-of-effects test (AT 5%/10% rule)".to_string(),
        };
    }
    let mut ids: Vec<V> = Vec::new();
    let affected_pct = a.g("affectedPct").n();
    for per in periods_of(sc)
        .into_iter()
        .filter(|x| res_of(pr.base, x).is_some() && !pr.same)
    {
        let rb = res_of(pr.base, per).expect("base results");
        let ro = res_of(sc, per).expect("option results");
        for (k, x) in ro.g("arcs").entries() {
            let b = rb.g("arcs").g(k);
            let d = x.g("v").n() - if b.t() { b.g("v").n() } else { 0.0 };
            if d >= 20.0
                && (!b.t() || !(b.g("v").n() > 0.0) || d / b.g("v").n() * 100.0 >= affected_pct)
            {
                push_unique(&mut ids, V::String(arc_link(k)));
            }
        }
    }
    for id in changed_links(pr.base, sc) {
        push_unique(&mut ids, V::String(id));
    }
    Affected {
        ids,
        basis: format!(
            "flow change of at least 20 veh/h and {}%, or a changed road ({}; run the extent test for the AT rule)",
            ns(affected_pct),
            xdata("ASSUMPTION").s()
        ),
    }
}

/// `accessibility(P)`: the jobs and services reachable from the development zones within the catchment, by mode: `{ rows, unit, text, basis }`
/// (`{ text, rows }` alone when there are no development zones).
pub fn accessibility(p: &V) -> V {
    let pr = pair(p);
    let sc = pr.opt;
    let a = assumptions(p);
    let dz = dev_zones(sc);
    let dests: Vec<&V> = sc
        .g("zones")
        .vals()
        .into_iter()
        .filter(|z| !z.g("kind").eq_s("gateway"))
        .collect();
    if dz.is_empty() {
        return jobj! { "text" => "No development zones.", "rows" => arr([]) };
    }
    let jobs = |z: &V| -> f64 {
        let j = jobs_of(z);
        if truthy_number(j) { j } else { 0.0 }
    };
    let svc = |z: &V| -> f64 {
        if lu_of(z, "school") > 0.0
            || lu_of(z, "medical") > 0.0
            || lu_of(z, "retail") + lu_of(z, "supermarket") > 0.0
        {
            1.0
        } else {
            0.0
        }
    };
    let have_jobs = dests.iter().any(|z| jobs(z) > 0.0);
    let w: Vec<(&V, f64)> = dests
        .iter()
        .map(|z| {
            (
                *z,
                if have_jobs {
                    jobs(z)
                } else {
                    GFA_KEYS.iter().fold(0.0, |s, k| s + lu_of(z, k))
                },
            )
        })
        .collect();
    let lim = a.g("catchMin").n() * 60.0;
    let walk_speed = a.g("walkSpeed").n();
    let cycle_speed = a.g("cycleSpeed").n();
    let dz_ids: Vec<String> = dz.iter().map(|z| format!("z:{}", z.g("id").s())).collect();
    let dist = dist_skim(p, sc, &dz_ids);
    let periods = periods_of(sc);
    let r = periods.first().and_then(|per| res_of(sc, per));
    let car = r.map(|r| {
        let from: Vec<String> = dz.iter().map(|z| z.g("id").s()).collect();
        let to: Vec<String> = dests.iter().map(|z| z.g("id").s()).collect();
        skim(p, sc, Some(r), &from, &to)
    });
    let mut walk = [0.0, 0.0];
    let mut cycle = [0.0, 0.0];
    let mut carv = [0.0, 0.0];
    for z in &dz {
        let d = dist.nd.get(&format!("z:{}", z.g("id").s()));
        for (o, wt) in &w {
            let i = d.and_then(|_| dist.g.idx.get(&format!("z:{}", o.g("id").s())).copied());
            let m = match (i, d) {
                (Some(i), Some(nd)) => nd[i],
                _ => f64::INFINITY,
            };
            if m / walk_speed <= lim {
                walk[0] += wt;
                walk[1] += svc(o);
            }
            if m / (cycle_speed / 3.6) <= lim {
                cycle[0] += wt;
                cycle[1] += svc(o);
            }
            let t = car
                .as_ref()
                .and_then(|c| c.get(&z.g("id").s()))
                .and_then(|m| m.get(&o.g("id").s()));
            if let Some(t) = t {
                if *t <= lim {
                    carv[0] += wt;
                    carv[1] += svc(o);
                }
            }
        }
    }
    let n = dz.len() as f64;
    let unit = if have_jobs {
        "jobs"
    } else {
        "m\u{b2} commercial GFA (no job data: fill employment in the Land use tab)"
    };
    let table: [(&str, Option<[f64; 2]>); 4] = [
        ("Walking", Some(walk)),
        ("Cycling", Some(cycle)),
        ("Public transport", None),
        ("Car", if car.is_some() { Some(carv) } else { None }),
    ];
    let rows: Vec<V> = table.iter().map(|(m, t)| jobj! { "mode" => *m, "value" => t.map(|t| t[0] / n), "services" => t.map(|t| t[1] / n) }).collect();
    let parts: Vec<String> = rows
        .iter()
        .map(|x| {
            let body = if x.g("value").nullish() {
                if x.g("mode").eq_s("Public transport") {
                    "not modelled (use the AT journey planner or MSM)".to_string()
                } else {
                    "run the model".to_string()
                }
            } else {
                format!(
                    "{} {}, {} zones with services",
                    fmt::int(x.g("value").n()),
                    if have_jobs { "jobs" } else { "m\u{b2} GFA" },
                    num(x.g("services"), 1)
                )
            };
            format!("{}: {}", x.g("mode").s(), body)
        })
        .collect();
    let text = format!(
        "{} within {} min (average over development zones; {} speeds).",
        parts.join("; "),
        a.g("catchMin").s(),
        xdata("ASSUMPTION").s()
    );
    jobj! { "rows" => arr(rows), "unit" => unit, "text" => text, "basis" => unit }
}

/// `policy(P)`: the policy alignment: `{ rows, basis, modes, policy, accessibility, fds, label, inconsistencies }`.
pub fn policy(p: &V) -> V {
    let pr = pair(p);
    let sc = pr.opt;
    let base = pr.base;
    let a = assumptions(p);
    let aff = affected_links(p);
    let fds = crate::compliance::fds_check(p);
    let an = anop(p);
    let mut fit_low: Vec<V> = Vec::new();
    for r in an
        .g("rows")
        .items()
        .iter()
        .filter(|r| r.g("fit").eq_s("Low"))
    {
        for id in r.g("corridor").g("links").items() {
            fit_low.push(id.clone());
        }
    }
    let ps: Vec<&'static str> = periods_of(sc)
        .into_iter()
        .filter(|x| res_of(base, x).is_some())
        .collect();
    let speed_up = a.g("speedUp").n();
    struct Row {
        v: V,
        flags: usize,
        name: String,
    }
    let mut rows: Vec<Row> = Vec::new();
    for idv in &aff.ids {
        let id = idv.s();
        let l = sc.g("links").g(&id);
        if !l.t() {
            continue;
        }
        let b = base.g("links").g(&id);
        let fc = if l.g("fc").t() { l.g("fc") } else { &NULL };
        let mut flags: Vec<String> = Vec::new();
        let place = place_of(l);
        let spd_up = ps.iter().any(|per| {
            let (o, bb) = (
                res_of(sc, per).expect("option results"),
                res_of(base, per).expect("base results"),
            );
            ["ab", "ba"].iter().any(|d| {
                let k = format!("{id}:{d}");
                let (x, y) = (o.g("arcs").g(&k), bb.g("arcs").g(&k));
                x.t() && y.t() && x.g("spd").n() - y.g("spd").n() >= speed_up
            })
        }) || (b.t()
            && (l.g("speed").n() > b.g("speed").n() || limit_of(l) > posted(b)));
        if let Some(pl) = place {
            if pl >= 3.0 && spd_up {
                flags.push(format!(
                    "The proposal raises vehicle speed on a high-Place (P{}) RASF section",
                    ns(pl)
                ));
            }
        }
        let lanes =
            |x: &V| to_int32(x.g("lanesAB").n()) as f64 + to_int32(x.g("lanesBA").n()) as f64;
        if b.t()
            && lanes(l) > lanes(b)
            && fc.g("gt").t()
            && !rx("arterial|motorway", "i").test(&fc.g("gt").s())
        {
            flags.push(format!(
                "Adds traffic lanes where Future Connect classes the road {}",
                fc.g("gt").s()
            ));
        }
        if fc.g("pt").t()
            && rx("rapid|frequent|ftn|rtn", "i").test(&fc.g("pt").s())
            && !l.g("busLane").t()
            && ps.iter().any(|per| {
                let k = format!("{id}:ab");
                let (x, y) = (
                    res_of(sc, per).expect("option results").g("arcs").g(&k),
                    res_of(base, per).expect("base results").g("arcs").g(&k),
                );
                x.t() && y.t() && x.g("t").n() > y.g("t").n() * 1.1
            })
        {
            flags.push(format!(
                "Slows a {} link by more than 10% without bus priority",
                fc.g("pt").s()
            ));
        }
        if fit_low.iter().any(|x| x.strict_eq(idv)) {
            flags.push("ANOP fit is Low on this corridor".to_string());
        }
        let name = link_name(l);
        let cx = l.g("cx");
        let rasf = if cx.t() && cx.g("rasf").t() {
            cx.g("rasf").clone()
        } else if let Some(pl) = place {
            V::String(format!("P{}", ns(pl)))
        } else {
            sv("\u{2013}")
        };
        let onf = if cx.t() && cx.g("onf").t() {
            cx.g("onf").clone()
        } else {
            sv("\u{2013}")
        };
        let fcs: Vec<V> = ["gt", "pt", "freight", "cycle"]
            .iter()
            .map(|k| {
                if fc.g(k).t() {
                    fc.g(k).clone()
                } else {
                    sv("\u{2013}")
                }
            })
            .collect();
        rows.push(Row {
            v: jobj! { "id" => idv, "name" => name.as_str(), "fc" => arr(fcs), "rasf" => rasf, "onf" => onf, "flags" => strs(flags.clone()) },
            flags: flags.len(),
            name,
        });
    }
    js_sort_by(&mut rows, |x, y| {
        let d = y.flags as f64 - x.flags as f64;
        if d != 0.0 {
            d
        } else {
            crate::edit::collate::locale_compare_f64(&x.name, &y.name)
        }
    });
    // conclusion checklist: alignment by mode from the ANOP results
    let score = xdata("STD.anop.score");
    let mode_label = xdata("MODE_LABEL");
    let modes: Vec<V> = MODES
        .iter()
        .map(|m| {
            let ms: Vec<&V> = an
                .g("rows")
                .items()
                .iter()
                .filter_map(|r| r.g("modes").items().iter().find(|x| x.g("mode").eq_s(m)))
                .filter(|x| x.g("proj").t() && x.g("acc").t())
                .collect();
            let label = mode_label.g(m).clone();
            if an.g("same").t() {
                return jobj! { "mode" => &label, "state" => "Not assessed", "why" => an.g("note") };
            }
            if ms.is_empty() {
                return jobj! { "mode" => &label, "state" => "Not assessed", "why" => "Run the model for the base and the option to assess (ANOP)." };
            }
            let below = ms.iter().filter(|x| score.g(&x.g("proj").s()).n() < score.g(&x.g("acc").s()).n()).count();
            let rise = ms.iter().filter(|x| x.g("defProj").n() > x.g("defBase").n()).count();
            let state = if below > 0 {
                "Not aligned"
            } else if rise > 0 {
                "Partly aligned"
            } else {
                "Aligned"
            };
            let why = if below > 0 {
                format!("{} corridor-period{} below acceptable LoS", below, if below == 1 { "" } else { "s" })
            } else if rise > 0 {
                format!("deficit rises on {rise}")
            } else {
                "at or above acceptable LoS, no deficit rises".to_string()
            };
            jobj! { "mode" => &label, "state" => state, "why" => why }
        })
        .collect();
    // NPS-UD 1(c): jobs and services reachable from the development zones within the catchment, by mode
    let acc = accessibility(p);
    // per modelled peak hour, each period on its own (never AM + PM)
    let d_vkt = per_period(
        base,
        sc,
        &|s: &V, per: &str| kpi_p("vkt", s, per),
        &ps,
        1,
        "",
    );
    let d_co2 = per_period(
        base,
        sc,
        &|s: &V, per: &str| kpi_p("co2", s, per),
        &ps,
        1,
        "",
    );
    let d_vht_saved = neg_pp(&per_period(
        base,
        sc,
        &|s: &V, per: &str| kpi_p("vht", s, per),
        &ps,
        1,
        "",
    ));
    let e = econ_of(p, base, sc);
    let haz = climate(p, None);
    let labels_vkt = xdata("STD.labels.vkt").s();
    let mode_text = modes
        .iter()
        .map(|m| format!("{}: {}", m.g("mode").s(), m.g("state").s()))
        .collect::<Vec<_>>()
        .join("; ");
    let haz_rows = haz.g("rows").items();
    let mut hazard_links: Vec<&V> = Vec::new();
    for r in haz_rows {
        if !hazard_links.iter().any(|x| x.strict_eq(r.g("link"))) {
            hazard_links.push(r.g("link"));
        }
    }
    let crash_text = {
        let c = crashes_on(p, base, &changed_links(base, sc));
        if c.t() {
            format!(
                "{} crashes ({} DSI) on changed roads in the CAS window",
                c.g("crashes").s(),
                c.g("dsi").s()
            )
        } else {
            "Load CAS crashes".to_string()
        }
    };
    let npsi = {
        let d = data(p).g("policy").g("npsiDoc").clone();
        if d.t() { d } else { V::String(rltp_relied(p)) }
    };
    let value_text: V = if e.t() && !e.g("mismatch").t() && !e.g("bcr").nullish() {
        V::String(format!(
            "Indicative BCR {} ({})",
            if e.g("bcrText").t() {
                e.g("bcrText").s()
            } else {
                num(e.g("bcr"), 1)
            },
            xdata("D4short").s()
        ))
    } else if e.t() && e.g("mismatch").t() {
        e.g("note").clone()
    } else {
        sv("Not available")
    };
    let policy_rows = arr([
        jobj! { "ref" => "AT ITA s4.4, s6", "policy" => "Network alignment conclusion, every mode", "text" => mode_text },
        jobj! { "ref" => "NPS-UD Policy 1(c)", "policy" => "Accessibility to jobs and services by walking, cycling, public transport and car", "text" => acc.g("text") },
        jobj! {
            "ref" => "NPS-UD Policy 1(e)", "policy" => "Supports reductions in greenhouse gas emissions",
            "text" => if !ps.is_empty() { format!("\u{394}VKT {} and \u{394}CO2e {} per modelled peak hour: {}.", pp_text(&d_vkt, 0, "veh-km"), pp_text(&d_co2, 2, "t"), labels_vkt) } else { "Run the base and the option.".to_string() },
            "info" => true,
        },
        jobj! { "ref" => "NPS-I Policy 3", "policy" => "Infrastructure identified in a strategic planning document", "text" => npsi },
        jobj! {
            "ref" => "NPS-I Policy 7(1)(d)", "policy" => "Method statement",
            "text" => "Spare Construct sketch model: static user-equilibrium assignment, HCM 6 / Akcelik-based junctions, trip rates with stated sources; indicative, to be confirmed with approved tools (D1).",
        },
        jobj! {
            "ref" => "FDS (RMA s104(1D), s168A(3C), s171(1D))", "policy" => "Consistency with the Future Development Strategy",
            "text" => if fds.t() { fds.g("text").clone() } else { sv("Look up FDS timing in the Compliance tab.") },
        },
        jobj! {
            "ref" => "GPS 2024", "policy" => "Economic growth and productivity",
            "text" => if !ps.is_empty() { format!("{} saved per modelled peak hour", pp_text(&d_vht_saved, 1, "veh\u{b7}h")) } else { "\u{2013}".to_string() },
        },
        jobj! {
            "ref" => "GPS 2024", "policy" => "Maintenance and resilience",
            "text" => if !haz_rows.is_empty() {
                format!(
                    "{} assessed roads in flood or coastal hazard areas{}",
                    hazard_links.len(),
                    if haz_rows.iter().any(|r| r.g("only").t()) { ", including an only access" } else { "" }
                )
            } else if haz.g("loaded").t() {
                "No assessed road in a mapped hazard area".to_string()
            } else {
                "Check hazards (Traffic data \u{203a} Data sources)".to_string()
            },
        },
        jobj! { "ref" => "GPS 2024", "policy" => "Safety", "text" => crash_text },
        jobj! { "ref" => "GPS 2024", "policy" => "Value for money", "text" => value_text },
    ]);
    let inconsistencies = rows.iter().filter(|r| r.flags > 0).count();
    jobj! {
        "rows" => arr(rows.into_iter().map(|r| r.v)),
        "basis" => aff.basis.as_str(),
        "modes" => arr(modes),
        "policy" => policy_rows,
        "accessibility" => acc,
        "fds" => fds,
        "label" => labels_vkt.as_str(),
        "inconsistencies" => inconsistencies,
    }
}

/// `policyCSV(P)`: the policy alignment, the GPS metrics and the funding attribution as a stamped CSV.
pub fn policy_csv(p: &V) -> String {
    let po = policy(p);
    let g = gps_metrics(p);
    let f = funding(p);
    let s = sv;
    let head = |h: &[&str]| -> Vec<V> { h.iter().map(|x| s(x)).collect() };
    let mut rows: Vec<Vec<V>> = Vec::new();
    rows.push(vec![s("Policy alignment"), po.g("basis").clone()]);
    rows.push(head(&[
        "Road",
        "FC general traffic",
        "FC public transport",
        "FC freight",
        "FC cycling",
        "RASF",
        "ONF",
        "Inconsistencies",
    ]));
    for r in po.g("rows").items() {
        let mut row = vec![r.g("name").clone()];
        row.extend(r.g("fc").items().iter().cloned());
        row.push(r.g("rasf").clone());
        row.push(r.g("onf").clone());
        row.push(V::String(
            r.g("flags")
                .items()
                .iter()
                .map(|x| x.s())
                .collect::<Vec<_>>()
                .join("; "),
        ));
        rows.push(row);
    }
    rows.push(vec![]);
    rows.push(head(&["Reference", "Policy", "Spare Construct evidence"]));
    for r in po.g("policy").items() {
        rows.push(vec![
            r.g("ref").clone(),
            r.g("policy").clone(),
            r.g("text").clone(),
        ]);
    }
    rows.push(vec![]);
    rows.push(vec![s("IPM stage 2 GPS metrics"), g.g("label").clone()]);
    rows.push(head(&[
        "Route",
        "Significance",
        "Base (s)",
        "Option (s)",
        "Saving (%)",
    ]));
    for r in g.g("routes").items() {
        rows.push(vec![
            r.g("name").clone(),
            r.g("significance").clone(),
            r1(r.g("base")),
            r1(r.g("opt")),
            r1(r.g("saving")),
        ]);
    }
    let netw = g.g("network");
    rows.push(vec![
        s("Scheme network"),
        netw.g("significance").clone(),
        s(""),
        s(""),
        r1(netw.g("saving")),
    ]);
    rows.push(vec![s("HCV rating"), or_blank(netw.g("hcv"))]);
    rows.push(vec![s("General traffic rating"), or_blank(netw.g("gt"))]);
    rows.push(vec![s("Note"), netw.g("note").clone()]);
    rows.push(head(&["Destination", "Significance", "Minutes saved"]));
    for d in g.g("destinations").items() {
        rows.push(vec![
            d.g("name").clone(),
            d.g("significance").clone(),
            r3(d.g("minutes")),
        ]);
    }
    rows.push(vec![
        s("Houses enabled (75% complete by 2035)"),
        g.g("houses").g("total").clone(),
        g.g("houses").g("basis").clone(),
    ]);
    rows.push(vec![
        s("DSI reduction per NZ$100m"),
        if !g.g("dsi").g("per100m").nullish() {
            r1(g.g("dsi").g("per100m"))
        } else {
            s("enter the DSI reduction")
        },
    ]);
    rows.push(vec![]);
    rows.push(vec![
        s("Funding attribution"),
        f.g("caveat").clone(),
        f.g("assetsBasis").clone(),
    ]);
    rows.push(head(&[
        "Asset",
        "Period",
        "Existing (veh/h)",
        "Committed (veh/h)",
        "Development (veh/h)",
        "Development share",
    ]));
    for a in f.g("assets").items() {
        rows.push(vec![
            a.g("name").clone(),
            a.g("period").clone(),
            r1(a.g("existing")),
            r1(a.g("committed")),
            r1(a.g("development")),
            r3(a.g("devShare")),
        ]);
    }
    if f.g("shares").t() {
        let sh = f.g("shares");
        rows.push(vec![]);
        rows.push(vec![
            s("Select-link shares by development area"),
            sh.g("method").clone(),
        ]);
        rows.push(head(&[
            "Asset",
            "Period",
            "Total (veh/h)",
            "Development area",
            "Flow (veh/h)",
            "Share",
        ]));
        for a in sh.g("assets").items() {
            for z in a.g("areas").items() {
                rows.push(vec![
                    a.g("name").clone(),
                    a.g("period").clone(),
                    r1(a.g("total")),
                    z.g("name").clone(),
                    r1(z.g("v")),
                    r3(z.g("share")),
                ]);
            }
        }
    }
    let gr = f.g("growth");
    rows.push(vec![
        s("Growth share"),
        if !gr.g("share").nullish() {
            r3(gr.g("share"))
        } else {
            s("")
        },
        gr.g("formula").clone(),
        gr.g("label").clone(),
    ]);
    let inc = f.g("incidence");
    rows.push(vec![
        s("Benefit inside the levy area"),
        if inc.t() && !inc.g("insideShare").nullish() {
            V::String(format!(
                "{}%",
                number_to_js_string(math_round(inc.g("insideShare").n() * 100.0))
            ))
        } else {
            s("draw the levy area")
        },
    ]);
    rows.push(vec![s("BCR_G (levy as Cp)"), f.g("bcrG").g("text").clone()]);
    if f.g("dc").t() {
        rows.push(vec![s("DC policy window"), f.g("dc").g("text").clone()]);
    }
    stamped(&csv(&rows), p, None)
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-policy"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-policy" {
        return None;
    }
    let po = policy(p);
    if po.g("rows").items().is_empty() && periods_of(pair(p).opt).is_empty() {
        return Some(String::new());
    }
    let main = t_table(
        &["Reference", "Policy", "Spare Construct evidence"],
        po.g("policy")
            .items()
            .iter()
            .map(|r| vec![esc(r.g("ref")), esc(r.g("policy")), esc(r.g("text"))])
            .collect(),
        "",
    );
    let first = rt(
        ctx,
        "Policy alignment",
        &main,
        &esc_s(&xdata("STD.labels.vkt").s()),
    );
    let second = if po.g("inconsistencies").t() {
        let tbl = t_table(
            &["Road", "Inconsistency"],
            po.g("rows")
                .items()
                .iter()
                .filter(|r| !r.g("flags").items().is_empty())
                .map(|r| {
                    vec![
                        esc(r.g("name")),
                        esc_s(
                            &r.g("flags")
                                .items()
                                .iter()
                                .map(|f| f.s())
                                .collect::<Vec<_>>()
                                .join("; "),
                        ),
                    ]
                })
                .collect(),
            "",
        );
        rt(
            ctx,
            "Inconsistencies with Future Connect, ANOP, RASF and FDS",
            &tbl,
            "",
        )
    } else {
        String::new()
    };
    Some(format!("{first}{second}"))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-policy"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-policy").then(|| policy_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-policy").then(|| res_ok(p))
}
