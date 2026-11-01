//! Junction truth tests: link and junction performance, the movement-capacity models and the result texts against the prototype.
//!
//! For each project and scenario (and the variants gen-graph.mjs made) the AM graph goes
//! through the same sequence of capacity updates as tools/transport-truth/gen-graph.mjs ran in
//! JavaScript (arc method, and movement method where the settings choose it); after each update
//! every arc's capN, effCap and arcTime, the movement detail and junctionResult for every
//! controller are compared. The movement-capacity functions, delayText, convText and the
//! settings readers are compared over the grids in expected/junction-functions.json.

mod graph_dump;
mod support;

use construct_transport::graph::{Graph, build_graph};
use construct_transport::jsnum::value_to_number;
use construct_transport::junction::{
    ArcFlow, CapOpts, ConvInfo, Lane, Lmv, MajorState, MoveEntry, PerfSet, alloc_lanes,
    approach_cap, arc_time, conv_text, cpot, d_hcm, delay_text, eff_cap, junction_result,
    lane_model, major_delay, p_prime, priority_caps, q95v, rbt_caps, two_stage, update_node_caps,
};
use construct_transport::project::{Scenario, Settings};
use construct_transport::standards::{
    CapMethod, ConvOpts, cap_method_of, conv_set, ef, growth, growth_basis, junction_set, pcu_f,
    year_of,
};
use construct_transport::util::r1;
use graph_dump::{Checker, case_inputs, n};
use serde_json::{Map, Value, json};
use support::{PROJECTS, as_number, load_json, load_project};

fn num(v: &Value) -> f64 {
    as_number(v).unwrap_or_else(|| value_to_number(v))
}

fn opt_num(v: &Value) -> f64 {
    if v.is_null() { f64::NAN } else { num(v) }
}

/// The flow pattern of gen-graph.mjs flowsAt at level l: arcs, turns, site movement slots.
fn flows_at(g: &Graph, l: f64) -> (Vec<f64>, Vec<f64>, Option<Vec<f64>>) {
    let xt = (0..g.m)
        .map(|j| l * (0.4 + ((j * 37) % 11) as f64 / 10.0))
        .collect();
    let x_t = (0..g.n_t)
        .map(|t| l * (0.2 + ((t * 13) % 7) as f64 / 10.0) / 3.0)
        .collect();
    let sm = g.sm.as_ref().map(|sm| {
        (0..sm.n)
            .map(|k| l * (0.3 + ((k * 17) % 5) as f64 / 10.0) / 2.0)
            .collect()
    });
    (xt, x_t, sm)
}

fn state_value(g: &Graph, p: &PerfSet, xt: &[f64], mv: bool) -> Value {
    Value::Array(
        g.arcs
            .iter()
            .map(|a| {
                let mut m = Map::new();
                m.insert("capN".into(), a.cap_n.map_or(Value::Null, n));
                m.insert("eff".into(), n(eff_cap(a, p)));
                m.insert("t".into(), n(arc_time(a, xt[a.i], p)));
                m.insert(
                    "ts".into(),
                    json!(
                        [0.0, 1200.0, 2400.0]
                            .iter()
                            .map(|&x| n(arc_time(a, x, p)))
                            .collect::<Vec<_>>()
                    ),
                );
                if mv {
                    m.insert("mj".into(), a.mj.map_or(Value::Null, |j| j.to_value()));
                    m.insert(
                        "mvRes".into(),
                        a.mv_res.as_ref().map_or(Value::Null, |r| r.to_value()),
                    );
                }
                Value::Object(m)
            })
            .collect(),
    )
}

fn ent_state_value(g: &Graph) -> Value {
    Value::Array(
        g.ctl
            .iter()
            .filter(|c| c.ent.is_some())
            .map(|c| {
                let ent = c.ent.as_ref().unwrap();
                let idx = |k: Option<usize>| k.map_or(Value::Null, |k| json!(ent[k].i));
                json!({ "n": c.n, "ent": ent.iter().map(|e| {
                    let mut lto = Map::new();
                    for (k, v) in &e.l_to { lto.insert(k.clone(), n(*v)); }
                    json!({ "v": e.v.to_value(), "Lto": lto, "vc": e.vc.map_or(Value::Null, n), "nc": e.nc.map_or(Value::Null, n),
                        "n": idx(e.near), "f": idx(e.far), "o": idx(e.opp_minor), "opp": idx(e.opp) })
                }).collect::<Vec<_>>() })
            })
            .collect(),
    )
}

/// Runs gen-graph.mjs perfCase's sequence on g and compares each step.
fn perf_run(ck: &mut Checker, at: &str, mut g: Graph, s: &Settings, mv: bool, expected: &Value) {
    let p = PerfSet::new(s);
    let pf = pcu_f(s);
    let j0 = junction_set(s);
    let steps = expected.as_array().expect("steps");
    for (li, step) in steps.iter().enumerate() {
        let l = num(&step["L"]);
        let w = num(&step["w"]);
        let (xt, x_t, sm_x) = flows_at(&g, l);
        if mv {
            g.sm_x = if li >= 2 { sm_x } else { None };
            g.mv_detail = li != 0;
        }
        update_node_caps(&mut g, &xt, w, if mv { Some(&x_t) } else { None });
        let mut got = Map::new();
        got.insert("L".into(), step["L"].clone());
        got.insert("w".into(), step["w"].clone());
        got.insert("smX".into(), json!(g.sm_x.is_some()));
        got.insert("detail".into(), json!(g.mv_detail));
        got.insert("arcs".into(), state_value(&g, &p, &xt, mv));
        if mv {
            got.insert("ent".into(), ent_state_value(&g));
        }
        if g.mv_detail {
            let flows: Vec<ArcFlow> = g
                .arcs
                .iter()
                .map(|a| ArcFlow {
                    v: r1(xt[a.i] / pf),
                    nd: r1(arc_time(a, xt[a.i], &p) - a.ff * (1.0 + 0.15 * (a.i % 3) as f64)),
                })
                .collect();
            let methods: Vec<&'static str> = if li == steps.len() - 1 {
                vec!["hcm6", "sidra", "rta"]
            } else {
                vec![j0.method]
            };
            let junctions: Vec<Value> = methods
                .iter()
                .map(|&method| {
                    let mut j = j0;
                    j.method = method;
                    j.cap_method = if mv { CapMethod::Movement } else { CapMethod::Arc };
                    json!({ "method": method, "nodes": g.ctl.iter().map(|c| junction_result(&g, c, &xt, &flows, s, &j, pf).to_value()).collect::<Vec<_>>() })
                })
                .collect();
            got.insert("junctions".into(), Value::Array(junctions));
            let mut ar = Map::new();
            for a in &g.arcs {
                ar.insert(
                    a.key.clone(),
                    json!({ "v": n(flows[a.i].v), "nd": n(flows[a.i].nd) }),
                );
            }
            got.insert("arcsRes".into(), Value::Object(ar));
        }
        ck.check(&format!("{at}.step[{li}]"), &Value::Object(got), step);
    }
}

#[test]
fn junction_performance_matches_the_prototype() {
    let mut ck = Checker::default();
    for name in PROJECTS {
        let p = load_project(name);
        let ex = load_json(&format!("expected/graph-{name}.json"));
        for (ci, case) in ex["cases"].as_array().expect("cases").iter().enumerate() {
            let Some(perf) = case.get("perf") else {
                continue;
            };
            let (sc, s) = case_inputs(&p, &ex, case);
            let period = case["period"].as_str();
            let at = format!(
                "{name}.cases[{ci}]({}, {})",
                sc.id,
                period.unwrap_or("none")
            );
            let mut ga = build_graph(&sc, &s, period);
            ga.cap_method = CapMethod::Arc;
            perf_run(&mut ck, &format!("{at}.arc"), ga, &s, false, &perf["arc"]);
            if cap_method_of(&s) == CapMethod::Movement {
                perf_run(
                    &mut ck,
                    &format!("{at}.movement"),
                    build_graph(&sc, &s, period),
                    &s,
                    true,
                    &perf["movement"],
                );
            }
        }
    }
    ck.finish("junction performance");
}

fn lanes_from(v: &Value) -> Vec<Lane> {
    v.as_array()
        .expect("lanes")
        .iter()
        .map(|l| {
            let mut lane = Lane::new(l["d"].as_str().expect("d"), num(&l["len"]));
            lane.slip = l.get("slip").is_some_and(|s| s.as_bool() == Some(true));
            if let Some(c) = l.get("c") {
                lane.c = num(c);
            }
            lane
        })
        .collect()
}

fn lmv(v: &Value) -> Lmv {
    Lmv {
        l: num(&v["L"]),
        t: num(&v["T"]),
        r: num(&v["R"]),
    }
}

fn entry_from(v: &Value) -> MoveEntry {
    MoveEntry {
        i: 0,
        major: v["major"].as_bool().unwrap_or(false),
        in_b: opt_num(&v["inB"]),
        leg: v["leg"].as_str().unwrap_or("").to_string(),
        gl: num(&v["gl"]),
        s_lane: num(&v["sLane"]),
        stop: v["stop"].as_bool().unwrap_or(false),
        gaps: if v["gaps"].is_null() {
            None
        } else {
            Some(v["gaps"].clone())
        },
        lanes: lanes_from(&v["lanes"]),
        median: v["median"].as_bool().unwrap_or(false),
        slip: v["slip"].as_bool().unwrap_or(false),
        lm_src: v["lmSrc"].clone(),
        thru: opt_num(&v["thru"]),
        mv_t: None,
        nc: v.get("nc").filter(|x| !x.is_null()).map(num),
        vc: v.get("vc").filter(|x| !x.is_null()).map(num),
        v: lmv(&v["v"]),
        l_to: v
            .get("Lto")
            .and_then(Value::as_object)
            .map_or(Vec::new(), |m| {
                m.iter().map(|(k, x)| (k.clone(), num(x))).collect()
            }),
        res: None,
        g: None,
        near: None,
        far: None,
        opp_minor: None,
        opp: None,
    }
}

fn settings_from(text: &str, defaults: &[(&str, Value)]) -> Settings {
    let mut v: Value = serde_json::from_str(text).expect("settings JSON");
    let o = v.as_object_mut().expect("settings object");
    for (k, d) in defaults {
        if !o.contains_key(*k) {
            o.insert(k.to_string(), d.clone());
        }
    }
    serde_json::from_value(v).expect("settings")
}

#[test]
fn movement_functions_match_the_prototype() {
    let f = load_json("expected/junction-functions.json");
    let mut ck = Checker::default();
    let cases = |k: &str| {
        f[k].as_array()
            .unwrap_or_else(|| panic!("{k} cases"))
            .clone()
    };
    for (i, c) in cases("cpot").iter().enumerate() {
        let a = &c["in"];
        ck.check(
            &format!("cpot[{i}]"),
            &n(cpot(num(&a[0]), num(&a[1]), num(&a[2]))),
            &c["out"],
        );
    }
    for (i, c) in cases("pPrime").iter().enumerate() {
        ck.check(
            &format!("pPrime[{i}]"),
            &n(p_prime(num(&c["in"]))),
            &c["out"],
        );
    }
    for (i, c) in cases("twoStage").iter().enumerate() {
        let a = &c["in"];
        ck.check(
            &format!("twoStage[{i}]"),
            &n(two_stage(
                num(&a[0]),
                num(&a[1]),
                num(&a[2]),
                num(&a[3]),
                num(&a[4]),
            )),
            &c["out"],
        );
    }
    for (i, c) in cases("q95").iter().enumerate() {
        let a = &c["in"];
        ck.check(
            &format!("q95[{i}]"),
            &n(q95v(num(&a[0]), num(&a[1]), num(&a[2]))),
            &c["out"],
        );
    }
    for (i, c) in cases("dHCM").iter().enumerate() {
        let a = &c["in"];
        ck.check(
            &format!("dHCM[{i}]"),
            &n(d_hcm(num(&a[0]), num(&a[1]), num(&a[2]))),
            &c["out"],
        );
    }
    for (i, c) in cases("majorDelay").iter().enumerate() {
        let a = &c["in"];
        let mj = MajorState {
            s_r: num(&a["mj"]["sR"]),
            sh_tl: num(&a["mj"]["shTL"]),
            cm_r: num(&a["mj"]["cmR"]),
            rb: num(&a["mj"]["rb"]),
        };
        let s: Settings = serde_json::from_str(a["S"].as_str().expect("S")).expect("settings");
        ck.check(
            &format!("majorDelay[{i}]"),
            &n(major_delay(&mj, num(&a["x"]), &PerfSet::new(&s))),
            &c["out"],
        );
    }
    for (i, c) in cases("laneModel").iter().enumerate() {
        let raw: Value =
            serde_json::from_str(c["in"]["raw"].as_str().expect("raw")).expect("raw JSON");
        ck.check(
            &format!("laneModel[{i}]"),
            &lane_model(Some(&raw), num(&c["in"]["gl"])).to_value(),
            &c["out"],
        );
    }
    let cap_f = |l: &Lane, m: char| {
        l.c * if m == 'L' {
            0.9
        } else if m == 'R' {
            0.8
        } else {
            1.0
        }
    };
    for (i, c) in cases("allocLanes").iter().enumerate() {
        let lanes = lanes_from(&c["in"]["lanes"]);
        ck.check(
            &format!("allocLanes[{i}]"),
            &alloc_lanes(&lanes, &lmv(&c["in"]["v"]), &cap_f).to_value(),
            &c["out"],
        );
    }
    for (i, c) in cases("approachCap").iter().enumerate() {
        let a = &c["in"];
        let lanes = lanes_from(&a["lanes"]);
        let r = approach_cap(
            &lanes,
            &lmv(&a["v"]),
            &cap_f,
            num(&a["T"]),
            num(&a["qs"]),
            a["detail"].as_bool().expect("detail"),
        );
        ck.check(&format!("approachCap[{i}]"), &r.to_value(), &c["out"]);
    }
    for (i, c) in cases("priorityCaps").iter().enumerate() {
        let a = &c["in"];
        let mut ent: Vec<MoveEntry> = a["ent"]
            .as_array()
            .expect("ent")
            .iter()
            .map(entry_from)
            .collect();
        priority_caps(
            &mut ent,
            &CapOpts::new(
                num(&a["T"]),
                num(&a["qs"]),
                a["detail"].as_bool().expect("detail"),
            ),
        );
        let idx = |k: Option<usize>| k.map_or(Value::Null, |k| json!(k));
        let got: Vec<Value> = ent
            .iter()
            .map(|e| json!({ "res": e.res.as_ref().map_or(Value::Null, |r| r.to_value()), "g": e.g.map_or(Value::Null, |g| g.to_value()),
                "n": idx(e.near), "f": idx(e.far), "o": idx(e.opp_minor), "opp": idx(e.opp) }))
            .collect();
        // The prototype keeps an override's gap value as saved ("3.9") and only ever uses it in
        // arithmetic; the port holds the number, so the expected gap values are read as numbers
        let mut want = c["out"].clone();
        for e in want.as_array_mut().expect("out") {
            for gap in e["g"]
                .as_object_mut()
                .into_iter()
                .flat_map(|g| g.values_mut())
            {
                for x in gap.as_object_mut().expect("gap").values_mut() {
                    if x.is_string() {
                        *x = n(num(x));
                    }
                }
            }
        }
        ck.check(&format!("priorityCaps[{i}]"), &json!(got), &want);
    }
    for (i, c) in cases("rbtCaps").iter().enumerate() {
        let a = &c["in"];
        let mut e = entry_from(&a["e"]);
        rbt_caps(
            std::slice::from_mut(&mut e),
            &CapOpts::new(num(&a["T"]), 7.0, a["detail"].as_bool().expect("detail")),
        );
        ck.check(
            &format!("rbtCaps[{i}]"),
            &e.res.as_ref().expect("res").to_value(),
            &c["out"],
        );
    }
    ck.finish("movement functions");
}

#[test]
fn texts_and_settings_match_the_prototype() {
    let f = load_json("expected/junction-functions.json");
    let mut ck = Checker::default();
    for (i, c) in f["delayText"].as_array().expect("cases").iter().enumerate() {
        let d = opt_num(&c["in"][0]);
        let dos = opt_num(&c["in"][1]);
        ck.check(
            &format!("delayText[{i}]"),
            &json!(delay_text(d, dos)),
            &c["out"],
        );
    }
    for (i, c) in f["convText"].as_array().expect("cases").iter().enumerate() {
        let a = &c["in"];
        let o = ConvInfo {
            converged: a["converged"].as_bool().unwrap(),
            hit_max: a["hitMax"].as_bool().unwrap(),
            gap: num(&a["gap"]),
            it: num(&a["it"]),
            gap_tol: num(&a["gapTol"]),
            stable: a["stable"].as_bool().unwrap(),
            share: num(&a["share"]),
            consec: num(&a["consec"]),
        };
        ck.check(&format!("convText[{i}]"), &json!(conv_text(&o)), &c["out"]);
    }
    for (i, c) in f["settings"].as_array().expect("cases").iter().enumerate() {
        let text = c["in"].as_str().expect("settings");
        let s = settings_from(text, &[]);
        let j = junction_set(&s);
        ck.check(
            &format!("settings[{i}].junctionSet"),
            &json!({ "method": j.method, "pff": n(j.pff), "T": n(j.t), "queueSpace": n(j.queue_space), "f95": n(j.f95), "capMethod": j.cap_method.as_str() }),
            &c["junctionSet"],
        );
        ck.check(
            &format!("settings[{i}].capMethod"),
            &json!(cap_method_of(&s).as_str()),
            &c["capMethod"],
        );
        let sc = settings_from(text, &[("gapTol", json!(0.0001)), ("maxIter", json!(200))]);
        let cv = |c: construct_transport::standards::ConvSet| json!({ "gapTol": n(c.gap_tol), "maxIter": n(c.max_iter), "stable": c.stable, "msa": c.msa, "chg": n(c.chg), "share": n(c.share), "consec": n(c.consec) });
        ck.check(
            &format!("settings[{i}].convSet"),
            &cv(conv_set(&sc, &ConvOpts::default())),
            &c["convSet"],
        );
        ck.check(
            &format!("settings[{i}].convSetOpts"),
            &cv(conv_set(
                &sc,
                &ConvOpts {
                    gap_tol: Some(0.002),
                    max_iter: Some(33.5),
                },
            )),
            &c["convSetOpts"],
        );
        let sp = settings_from(text, &[("hvPct", json!(6)), ("pcuHV", json!(2.3))]);
        ck.check(&format!("settings[{i}].pcuF"), &n(pcu_f(&sp)), &c["pcuF"]);
        let se = settings_from(text, &[("hvPct", json!(6))]);
        let efs: Vec<Value> = [3.0, 5.0, 20.0, 50.0, 80.0, 130.0, 150.0]
            .iter()
            .map(|&v| n(ef(v, &se)))
            .collect();
        ck.check(&format!("settings[{i}].ef"), &json!(efs), &c["ef"]);
    }
    for (i, c) in f["growth"].as_array().expect("cases").iter().enumerate() {
        let s: Settings =
            serde_json::from_str(c["in"]["S"].as_str().expect("S")).expect("settings");
        let sc: Scenario =
            serde_json::from_str(c["in"]["sc"].as_str().expect("sc")).expect("scenario");
        ck.check(
            &format!("growth[{i}].yearOf"),
            &n(year_of(Some(&sc), &s)),
            &c["yearOf"],
        );
        ck.check(
            &format!("growth[{i}].growth"),
            &n(growth(&s, Some(&sc))),
            &c["growth"],
        );
        ck.check(
            &format!("growth[{i}].basis"),
            &json!(growth_basis(&s)),
            &c["basis"],
        );
    }
    ck.finish("texts and settings");
}
