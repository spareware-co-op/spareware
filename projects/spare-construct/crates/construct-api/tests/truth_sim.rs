//! The simulation commands against the prototype's recorded runs (tools/transport-truth gen-sim.mjs): for each default case the model
//! is run, a simulation is created with the harness's seeded generator, stepped to each checkpoint, and the HUD numbers and the
//! comparison with the model are read.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{PROJECTS, assert_close, input_text, load_json};

#[test]
fn simulation_commands_match_the_recorded_runs() {
    let mut checks = 0;
    for name in PROJECTS {
        let rec = load_json(&format!("expected/sim-{name}.json"));
        let mut s = Session::seeded(11, 1_790_000_000_000.0);
        s.run(
            "transport.project.open",
            &json!({ "text": input_text(name) }),
        )
        .unwrap();
        for case in rec["cases"].as_array().unwrap() {
            if case["label"] != json!("default") {
                continue;
            }
            let (sid, period) = (
                case["scenario"].as_str().unwrap(),
                case["period"].as_str().unwrap(),
            );
            s.run(
                "transport.run",
                &json!({ "scenario": sid, "periods": [period] }),
            )
            .unwrap();
            // the harness's generator had made rngSkip numbers before the simulation drew its own
            let seed = (case["seed"].as_u64().unwrap() as u32).wrapping_add(
                0x6D2B_79F5u32.wrapping_mul(case["rngSkip"].as_u64().unwrap() as u32),
            );
            s.run(
                "transport.sim.create",
                &json!({ "scenario": sid, "period": period, "seed": seed }),
            )
            .unwrap();
            let init = &case["init"];
            let mut at = 0u64;
            for cp in case["checkpoints"].as_array().unwrap() {
                let step = cp["step"].as_u64().unwrap();
                let hud = s
                    .run("transport.sim.step", &json!({ "steps": step - at }))
                    .unwrap();
                at = step;
                let path = format!("{name}.{sid}.{period}.step{step}");
                let want = &cp["hud"];
                let got = json!({
                    "clock": hud["clock"], "n": hud["onRoad"], "q": hud["queued"], "spd": hud["speedKmh"], "done": hud["tripsDone"],
                    "avgDelay": hud["delayPerTrip"], "waiting": hud["waiting"], "spawned": hud["released"], "teleports": hud["teleports"],
                });
                assert_close(&got, want, &format!("{path}.hud"));
                let v = s
                    .run("transport.sim.validation", &json!({ "limit": 100_000 }))
                    .unwrap();
                if cp["validation"] == Value::Null {
                    assert_eq!(v["available"], json!(false), "{path}: no comparison yet");
                } else {
                    let rows: Vec<Value> = v["rows"].as_array().unwrap().iter().map(|r| json!({ "key": r["key"], "link": r["link"], "name": r["name"], "model": r["model"], "sim": r["sim"], "geh": r["geh"] })).collect();
                    assert_close(
                        &json!({ "rows": rows, "pass": v["pass"] }),
                        &cp["validation"],
                        &format!("{path}.validation"),
                    );
                }
                checks += 2;
            }
            assert_eq!(init["period"], json!(period));
            s.run("transport.sim.stop", &json!({})).unwrap();
        }
    }
    println!("sim truth: {checks} checks match the prototype");
    assert!(checks >= 20);
}
