//! Simulation truth tests: the traffic microsimulation replayed against the prototype's recorded runs.
//!
//! tools/transport-truth/gen-sim.mjs ran the prototype's `Sim` the way the app does (Engine.assign for
//! a period, then `new Sim(Store.sc, res)` and `sim.step()`) on every project in inputs/, each scenario
//! and period and busier variants (gridlock, a vehicle limit, bus lanes, stop junctions, a nearly empty
//! network, a run of over two hours), with the harness's seeded random generator. It recorded the opening
//! state and, at several steps, the clock, vehicle counts, per-arc counts and flows, totals, route queues,
//! signal lights, the HUD numbers, the comparison with the model and every vehicle. The tests below make
//! the same runs in Rust from the recorded assignment result and the same generator and compare.

mod graph_dump;
mod support;

use construct_transport::graph::Graph;
use construct_transport::jsnum::{Maybe, plain_number};
use construct_transport::project::{Project, Route, Scenario, Settings, SimSettings};
use construct_transport::results::Res;
use construct_transport::routes::{bus_freq, bus_pcu_of};
use construct_transport::sim::{Sim, SimOptions, SimResults};
use construct_transport::standards::pcu_f;
use engine_sim::{Mulberry32, Rng};
use graph_dump::Checker;
use serde_json::{Value, json};
use support::{PROJECTS, load_expected, load_json, load_project};

fn n(x: f64) -> Value {
    plain_number(x)
}

/// The settings and scenario a case ran with: the project's own with the case's overrides, or the edited scenario.
fn case_inputs(p: &Project, case: &Value) -> (Scenario, Settings) {
    let id = case["scenario"].as_str().expect("scenario id");
    let sc: Scenario = match case["scenarioText"].as_str() {
        Some(t) => serde_json::from_str(t).expect("edited scenario"),
        None => p.scenario(id).expect("scenario").clone(),
    };
    let mut s = p.settings.clone();
    let st = &case["settings"];
    if st["scale"].is_number() || st["maxVeh"].is_number() {
        let mut sim: SimSettings = s.sim.get().cloned().expect("settings.sim");
        if let Some(x) = st["scale"].as_f64() {
            sim.scale = Maybe::number(x);
        }
        if let Some(x) = st["maxVeh"].as_f64() {
            sim.max_veh = Maybe::number(x);
        }
        s.sim = Maybe::Set(sim);
    }
    if let Some(x) = st["hvPct"].as_f64() {
        s.hv_pct = Maybe::number(x);
    }
    (sc, s)
}

fn opt_n(x: Option<f64>) -> Value {
    x.map_or(Value::Null, n)
}

/// The opening state, as gen-sim.mjs records it.
fn init_value(sim: &Sim<Mulberry32>) -> Value {
    let g: &Graph = sim.graph();
    let signals: Vec<Value> = g
        .ctl
        .iter()
        .filter(|c| c.kind == "signals")
        .map(|c| {
            let t = c.signal.as_ref().expect("signal timing");
            let arcs: Vec<Value> = c
                .arcs
                .iter()
                .map(|&i| {
                    let a = &g.arcs[i];
                    json!({ "i": i, "phase": a.phase, "gC": opt_n(a.g_c), "C": opt_n(a.c) })
                })
                .collect();
            json!({ "n": c.n, "C": n(t.c), "lost": n(t.lost), "split": t.split.iter().map(|&x| n(x)).collect::<Vec<_>>(),
                "offset": t.offset, "arcs": arcs })
        })
        .collect();
    let routes: Vec<Value> = sim
        .core()
        .routes()
        .iter()
        .zip(sim.routes())
        .map(|(r, info)| {
            json!({ "arcs": r.route.arcs, "rate": n(r.route.rate), "type": if r.route.bus { "bus" } else { "car" },
                "color": info.colour, "rid": info.route_id, "next": n(r.next) })
        })
        .collect();
    let lanes: Vec<Value> = sim
        .core()
        .net()
        .arcs
        .iter()
        .map(|a| {
            if a.virtual_arc {
                Value::Null
            } else {
                json!(a.lanes + usize::from(a.bus_lane))
            }
        })
        .collect();
    json!({ "period": sim.period(), "scale": n(sim.scale()), "maxVeh": n(sim.max_vehicles()), "hv": n(sim.heavy_share()),
        "pf": n(sim.pcu_factor()), "startClock": n(sim.start_clock()), "M": g.m, "dt": n(sim.core().params().dt),
        "signals": signals, "routes": routes, "lanes": lanes })
}

fn series_value(sim: &Sim<Mulberry32>) -> Value {
    Value::Array(
        sim.stats()
            .series
            .iter()
            .map(|l| json!({ "t": n(l.t), "n": l.n, "q": l.q, "spd": n(l.speed), "done": l.done, "waiting": l.waiting }))
            .collect(),
    )
}

/// The parts of a checkpoint, by name, as gen-sim.mjs records them.
fn checkpoint_parts(sim: &Sim<Mulberry32>, detail: bool) -> Vec<(&'static str, Value)> {
    let core = sim.core();
    let st = sim.stats();
    let g = sim.graph();
    let hud = sim.hud();
    let mut parts = vec![
        ("clock", json!({ "t": n(core.time()), "n": core.vehicles_on_road(), "nid": core.vehicles_made() })),
        ("count", json!(core.arc_counts())),
        ("passed", Value::Array(core.arc_passed().iter().map(|&x| n(x)).collect())),
        (
            "stats",
            json!({ "done": st.done, "tt": n(st.tt), "ff": n(st.ff), "spawned": st.spawned, "teleports": st.teleports,
                "busDone": st.bus_done, "busTT": n(st.bus_tt), "seriesLen": st.series.len(),
                "last": st.series.last().map(|l| json!({ "t": n(l.t), "n": l.n, "q": l.q, "spd": n(l.speed), "done": l.done, "waiting": l.waiting })) }),
        ),
        (
            "routes",
            Value::Array(
                core.routes()
                    .iter()
                    .map(|r| json!({ "next": n(r.next), "q": r.queue.len(), "qIds": r.queue.iter().map(|v| v.id).collect::<Vec<_>>() }))
                    .collect(),
            ),
        ),
        (
            "signals",
            Value::Array(
                g.ctl
                    .iter()
                    .filter(|c| c.kind == "signals")
                    .flat_map(|c| c.arcs.iter())
                    .map(|&ai| json!({ "ai": ai, "light": sim.signal(ai).as_char().to_string() }))
                    .collect(),
            ),
        ),
        (
            "hud",
            json!({ "clock": n(hud.clock), "n": hud.on_road, "q": hud.queued, "spd": n(hud.speed), "done": hud.trips_done,
                "avgDelay": n(hud.delay_per_trip), "waiting": hud.waiting, "spawned": hud.released, "teleports": hud.teleports }),
        ),
        (
            "validation",
            match sim.validation() {
                None => Value::Null,
                Some(v) => json!({ "rows": v.rows.iter().map(|r| json!({ "key": r.key, "link": r.link, "name": r.name,
                    "model": n(r.model), "sim": n(r.sim), "geh": n(r.geh) })).collect::<Vec<_>>(), "pass": n(v.pass) }),
            },
        ),
        ("snaps", Value::Array(core.snapshots().iter().map(|s| n(s.t)).collect())),
    ];
    if detail {
        let mut vs: Vec<Value> = Vec::new();
        sim.for_each_vehicle(|v, _a, li, nl| {
            vs.push(json!({ "id": v.id, "route": v.route, "k": v.k, "ai": v.arc, "li": li, "nl": nl, "s": n(v.s), "v": n(v.v),
                "acc": n(v.acc), "type": v.kind.as_str(), "len": n(v.len), "amax": n(v.a_max), "T": n(v.headway), "v0f": n(v.speed_factor),
                "t0": n(v.t0), "ff": n(v.ff), "wait": n(v.wait), "stopped": v.stopped, "stuck": v.stuck,
                "perm": v.permit.map(|p| json!([p.arc, p.lane])) }));
        });
        parts.push(("vehicles", Value::Array(vs)));
    }
    parts
}

/// What a recorded checkpoint holds for one part (the recorded file keeps them in one object).
fn expected_part(cp: &Value, part: &str) -> Value {
    match part {
        "clock" => json!({ "t": cp["t"], "n": cp["n"], "nid": cp["nid"] }),
        other => cp[other].clone(),
    }
}

#[test]
fn simulations_match_the_prototype() {
    let mut ck = Checker::default();
    let mut steps_run = 0usize;
    for name in PROJECTS {
        let p = load_project(name);
        let ex = load_json(&format!("expected/sim-{name}.json"));
        for (ci, case) in ex["cases"].as_array().expect("cases").iter().enumerate() {
            let (sc, s) = case_inputs(&p, case);
            let at = format!(
                "{name}.cases[{ci}]({}, {}, {})",
                case["scenario"].as_str().unwrap_or(""),
                case["period"].as_str().unwrap_or(""),
                case["label"].as_str().unwrap_or("")
            );
            let res = SimResults::from_value(&case["res"]);
            let mut rng = Mulberry32::new(case["seed"].as_u64().expect("seed") as u32);
            for _ in 0..case["rngSkip"].as_u64().expect("rngSkip") {
                rng.next_f64();
            }
            let mut sim = Sim::new(&sc, &s, Some(&res), SimOptions::default(), rng);
            ck.check(&format!("{at}.init"), &init_value(&sim), &case["init"]);
            let mut done = 0usize;
            for cp in case["checkpoints"].as_array().expect("checkpoints") {
                let step = cp["step"].as_u64().expect("step") as usize;
                while done < step {
                    sim.step();
                    done += 1;
                }
                let detail = cp.get("vehicles").is_some();
                for (part, actual) in checkpoint_parts(&sim, detail) {
                    ck.check(
                        &format!("{at}.step{step}.{part}"),
                        &actual,
                        &expected_part(cp, part),
                    );
                }
            }
            ck.check(
                &format!("{at}.series"),
                &series_value(&sim),
                &case["series"],
            );
            steps_run += done;
        }
    }
    println!("simulation steps replayed: {steps_run}");
    ck.finish("simulation truth");
}

#[test]
fn typed_results_give_the_same_simulation_input() {
    // SimResults::from_res (the typed result) reads what from_value (the JSON) reads
    let mut ck = Checker::default();
    for name in PROJECTS {
        let ex = load_expected(name);
        for (id, sc) in ex["scenarios"].as_object().expect("scenarios") {
            for (period, rec) in sc["periods"].as_object().expect("periods") {
                let json_res = &rec["res"];
                let typed: Res =
                    serde_json::from_value(json_res.clone()).expect("result reads as a Res");
                let a = SimResults::from_res(&typed);
                let b = SimResults::from_value(json_res);
                ck.run += 1;
                if a == b {
                    ck.passed += 1;
                } else {
                    ck.check(
                        &format!("{name}.{id}.{period}"),
                        &json!(format!("{a:?}")),
                        &json!(format!("{b:?}")),
                    );
                }
            }
        }
    }
    ck.finish("typed results");
}

#[test]
fn bus_frequencies_and_loads_match_the_prototype() {
    let f = load_json("expected/sim-functions.json");
    let mut ck = Checker::default();
    for (i, c) in f["busFreq"].as_array().expect("cases").iter().enumerate() {
        let r: Route =
            serde_json::from_str(c["route"].as_str().expect("route text")).expect("route");
        let got = bus_freq(&r, c["period"].as_str());
        ck.check(&format!("busFreq[{i}] {}", c["route"]), &n(got), &c["out"]);
    }
    for (i, c) in f["busPcu"].as_array().expect("cases").iter().enumerate() {
        let s: Settings =
            serde_json::from_str(c["settings"].as_str().expect("settings text")).expect("settings");
        ck.check(
            &format!("busPcu[{i}] {}", c["settings"]),
            &n(bus_pcu_of(&s)),
            &c["out"],
        );
    }
    for (i, c) in f["pcu"].as_array().expect("cases").iter().enumerate() {
        let s: Settings =
            serde_json::from_str(c["settings"].as_str().expect("settings text")).expect("settings");
        ck.check(
            &format!("pcu[{i}] {}", c["settings"]),
            &n(pcu_f(&s)),
            &c["out"],
        );
    }
    ck.finish("bus frequencies and loads");
}

#[test]
fn the_seeded_generator_matches_the_harness() {
    let f = load_json("expected/sim-functions.json");
    let mut ck = Checker::default();
    for (seed, nums) in f["rng"].as_object().expect("rng") {
        let mut r = Mulberry32::new(seed.parse().expect("seed"));
        for (i, want) in nums.as_array().expect("numbers").iter().enumerate() {
            // bit for bit, not within a tolerance
            ck.run += 1;
            if r.next_f64().to_bits() == want.as_f64().expect("number").to_bits() {
                ck.passed += 1;
            } else {
                ck.check(
                    &format!("rng[{seed}][{i}]"),
                    &json!("differs"),
                    &json!("same"),
                );
            }
        }
    }
    ck.finish("seeded generator");
}
