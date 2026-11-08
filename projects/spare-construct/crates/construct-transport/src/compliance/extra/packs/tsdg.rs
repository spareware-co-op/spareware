//! The ATOC Traffic Signals Design Guidelines s2.5 signal pack (compliance-extra.js section 19, item 2) and the design-life run behind it.

use super::{Section, distinct, or_blank, or_n, pack_csv, r1, r3, section, signal_nodes, sv};
use crate::compliance::data::{m as model, x as xdata};
use crate::compliance::extra::ctx::{
    clone_sc, data, data_mut, node_name, pair, periods_of, res_of,
};
use crate::compliance::jv::{Jx, NULL, V, jobj};
use crate::compliance::{dates, engine};
use crate::jsnum::{math_round, truthy_number};

/// `a.approach || a.name || ''`.
fn approach_name(a: &V) -> V {
    if a.g("approach").t() {
        a.g("approach").clone()
    } else if a.g("name").t() {
        a.g("name").clone()
    } else {
        sv("")
    }
}

/// `runDesignLife(P, opts)`: the option's junctions at the design-life demand (`+pct`%) for each period: `{ at, pct, years, scId, nodes }`. Unless
/// `opts.save` is false the result is stored in the project's `tsdg.result`.
pub fn run_design_life(p: &mut V, opts: &V) -> V {
    let cx = data(p).g("tsdg").clone();
    let pc = {
        let a = opts.g("pct").n();
        if truthy_number(a) {
            a
        } else {
            or_n(cx.g("pct"), 10.0)
        }
    };
    let periods: Vec<String> = if opts.g("periods").t() {
        opts.g("periods").items().iter().map(|x| x.s()).collect()
    } else {
        vec!["AM".to_string(), "PM".to_string()]
    };
    let res = {
        let pr = pair(p);
        let sc = pr.opt;
        let mut nodes = V::object();
        for per in &periods {
            let c = clone_sc(sc, &[]);
            let r = engine::assign(
                p,
                &c,
                None,
                &jobj! { "period" => per.as_str(), "demandFactor" => 1.0 + pc / 100.0 },
            );
            nodes.set(per.as_str(), r.g("nodes").clone());
        }
        jobj! {
            "at" => crate::edit::ids::iso_string(dates::now_ms()), "pct" => pc, "years" => or_n(cx.g("years"), 10.0), "scId" => sc.g("id"), "nodes" => nodes,
        }
    };
    if !opts.g("save").strict_eq(&V::Bool(false)) {
        data_mut(p)
            .get_mut("tsdg")
            .expect("tsdg record")
            .set("result", res.clone());
    }
    res
}

/// `tsdgPack(P)`: the ATOC Traffic Signals Design Guidelines s2.5 signal pack (CSV).
pub fn tsdg_pack(p: &V) -> String {
    let pr = pair(p);
    let sc = pr.opt;
    let signals = signal_nodes(sc);
    let ids: Vec<String> = if !signals.is_empty() {
        signals
    } else {
        match res_of(sc, "AM").or_else(|| res_of(sc, "PM")) {
            Some(r) => r
                .g("nodes")
                .keys()
                .into_iter()
                .take(10)
                .map(|k| k.to_string())
                .collect(),
            None => vec![],
        }
    };
    let counts: Vec<&V> = p
        .g("data")
        .g("counts")
        .items()
        .iter()
        .filter(|c| {
            ids.iter().any(|id| {
                let n = sc.g("nodes").g(id);
                n.t()
                    && crate::util::dist(
                        &[n.g("lat").n(), n.g("lng").n()],
                        &[c.g("lat").n(), c.g("lng").n()],
                    ) <= 150.0
            })
        })
        .collect();
    let stat = "95th percentile back of queue (m)";
    let mut perf: Vec<Vec<V>> = Vec::new();
    for sx in distinct(&[Some(pr.base), Some(pr.opt), pr.mit]) {
        for per in periods_of(sx) {
            for id in &ids {
                let nr = res_of(sx, per).map_or(&NULL, |r| r.g("nodes")).g(id);
                if !nr.t() {
                    continue;
                }
                for a in nr.g("appr").items() {
                    perf.push(vec![
                        sx.g("name").clone(),
                        V::String(node_name(sx, id)),
                        sv(per),
                        approach_name(a),
                        r1(a.g("delay")),
                        sv("average control delay"),
                        a.g("los").clone(),
                        r3(a.g("dos")),
                        sv(stat),
                        a.g("q95").clone(),
                    ]);
                }
            }
        }
    }
    let dl = data(p).g("tsdg").g("result").clone();
    let mut life: Vec<Vec<V>> = Vec::new();
    if dl.t() {
        for (per, nodes) in dl.g("nodes").entries() {
            for id in &ids {
                let nr = nodes.g(id);
                if nr.t() {
                    for a in nr.g("appr").items() {
                        life.push(vec![
                            V::String(format!(
                                "+{}% (design life {} years)",
                                dl.g("pct").s(),
                                dl.g("years").s()
                            )),
                            V::String(node_name(sc, id)),
                            sv(per),
                            approach_name(a),
                            r1(a.g("delay")),
                            a.g("los").clone(),
                            r3(a.g("dos")),
                            sv(stat),
                            a.g("q95").clone(),
                        ]);
                    }
                }
            }
        }
    }
    let controls = model("CONTROLS");
    let timing: Vec<Vec<V>> = ids
        .iter()
        .map(|id| {
            let n = sc.g("nodes").g(id);
            let nr = res_of(sc, "AM").map_or(&NULL, |r| r.g("nodes")).g(id);
            let ck = n.g("control").s();
            let cl = if controls.has(&ck) && controls.g(&ck).t() {
                controls.g(&ck).clone()
            } else {
                n.g("control").clone()
            };
            let cycle = if n.g("cycle").t() {
                n.g("cycle").clone()
            } else if nr.g("C").t() {
                nr.g("C").clone()
            } else {
                sv("")
            };
            vec![
                V::String(node_name(sc, id)),
                cl,
                cycle,
                if nr.g("split").t() {
                    V::String(nr.g("split").stringify())
                } else {
                    sv("optimised by Spare Construct (Webster)")
                },
            ]
        })
        .collect();
    let counts_rows: Vec<Vec<V>> = counts
        .iter()
        .map(|c| {
            let hv = c.g("hv");
            vec![
                c.g("name").clone(),
                c.g("src").clone(),
                or_blank(c.g("date")),
                c.g("adt").clone(),
                or_blank(c.g("peak")),
                or_blank(c.g("peakTime")),
                if hv.nullish() { sv("") } else { hv.clone() },
                if !hv.nullish() {
                    V::Number(math_round(c.g("adt").n() * (1.0 - hv.n() / 100.0)))
                } else {
                    sv("")
                },
                if !hv.nullish() {
                    V::Number(math_round(c.g("adt").n() * hv.n() / 100.0))
                } else {
                    sv("")
                },
            ]
        })
        .collect();
    let secs: Vec<Section> = vec![
        section(
            "Counts by class and period",
            None,
            &[
                "Site",
                "Source",
                "Date",
                "Daily (veh/day)",
                "Peak hour (veh/h)",
                "Peak time",
                "Heavy (%)",
                "Light (veh/day)",
                "Heavy (veh/day)",
            ],
            counts_rows,
            Some("No counts within 150 m of the signals: load counts or add turning counts."),
        ),
        section(
            "Per-option delay, LoS and queue",
            Some(format!(
                "{}. Queue statistic: {}.",
                xdata("STD.labels.junction").s(),
                stat
            )),
            &[
                "Option",
                "Junction",
                "Period",
                "Approach",
                "Delay (s)",
                "Delay statistic",
                "LoS",
                "DoS",
                "Queue statistic",
                "Queue (m)",
            ],
            perf,
            Some("Run the model."),
        ),
        section(
            "Design-life run",
            Some(if dl.t() {
                format!(
                    "Demand +{}% for a {}-year design life",
                    dl.g("pct").s(),
                    dl.g("years").s()
                )
            } else {
                "Run the design-life test in Compliance+ \u{203a} Hand-over packs.".to_string()
            }),
            &[
                "Run",
                "Junction",
                "Period",
                "Approach",
                "Delay (s)",
                "LoS",
                "DoS",
                "Queue statistic",
                "Queue (m)",
            ],
            life,
            None,
        ),
        section(
            "Phasing and timings",
            None,
            &["Junction", "Control", "Cycle (s)", "Phase splits"],
            timing,
            None,
        ),
    ];
    pack_csv(
        p,
        "ATOC Traffic Signals Design Guidelines s2.5 signal pack",
        "SIDRA (ATOC TSDG signal design)",
        &secs,
    )
}
