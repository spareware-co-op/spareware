//! Workshop's assembly posed by the geometry kernel: the demo's hinged stop (`demo/workshop/js/
//! model.js`), with the bracket from `bracket.json`, its hinge turned and its slider slid by the
//! kernel, and each instance's box the kernel's.

use construct_workshop::{build_part, pose_assembly, pose_json};
use serde_json::{Value, json};

fn hinged_stop() -> Value {
    let mut s: Value = serde_json::from_str(include_str!("hinged_stop.json")).unwrap();
    let bracket: Value = serde_json::from_str(include_str!("bracket.json")).unwrap();
    s["parts"].as_array_mut().unwrap().insert(0, bracket);
    s
}

fn pose(s: &Value) -> Value {
    serde_json::to_value(pose_assembly(s)).unwrap()
}

fn apply(m: &[f64], p: [f64; 3]) -> [f64; 3] {
    let row = |i: usize| m[i * 4] * p[0] + m[i * 4 + 1] * p[1] + m[i * 4 + 2] * p[2] + m[i * 4 + 3];
    [row(0), row(1), row(2)]
}

/// The box of a part's bodies where the map puts them, from their profiles: what the page draws.
fn drawn_box(part: &Value, m: &[f64]) -> [[f64; 3]; 2] {
    let a = serde_json::to_value(build_part(part)).unwrap();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut take = |p: [f64; 3]| {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    };
    for b in a["bodies"].as_array().unwrap() {
        let bm: Vec<f64> = b["m"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let (z0, h) = (b["z"].as_f64().unwrap(), b["height"].as_f64().unwrap());
        let mut flat = Vec::new();
        if b["kind"] == "prism" {
            for p in b["outline"].as_array().unwrap() {
                flat.push([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]);
            }
        } else {
            let (c, r) = (&b["center"], b["radius"].as_f64().unwrap());
            for k in 0..3600 {
                let t = f64::from(k) * std::f64::consts::PI / 1800.0;
                flat.push([
                    c[0].as_f64().unwrap() + r * t.cos(),
                    c[1].as_f64().unwrap() + r * t.sin(),
                ]);
            }
        }
        for p in flat {
            for z in [z0, z0 + h] {
                take(apply(m, apply(&bm, [p[0], p[1], z])));
            }
        }
    }
    [lo, hi]
}

fn near(a: &Value, b: [[f64; 3]; 2], within: f64) -> bool {
    (0..2).all(|i| (0..3).all(|k| (a[i][k].as_f64().unwrap() - b[i][k]).abs() <= within))
}

#[test]
fn the_kernel_poses_the_hinged_stop() {
    let s = hinged_stop();
    let a = pose(&s);
    assert_eq!(a["engine"], "kernel");
    let states: Vec<&str> = a["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["built", "built", "built"], "{:#}", a["instances"]);
    for j in a["joints"].as_array().unwrap() {
        assert_eq!(j["state"], "built", "{j}");
    }
    assert!(
        a["joints"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Turned 35° about z by the kernel's transform.rotate")
    );
    assert!(
        a["joints"][1]["message"]
            .as_str()
            .unwrap()
            .contains("transform.move")
    );

    // the arm, turned 35°, is Certified within the model's tolerance, and its box is where the page
    // draws it, to within that tolerance and the drawn circle's chords
    let arm = &a["instances"][1];
    assert_eq!(arm["exactness"], "Certified");
    let bound = arm["bound"].as_u64().unwrap();
    assert!((1..=1600).contains(&bound), "{bound}");
    let m: Vec<f64> = arm["m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert!(
        near(&arm["bbox"], drawn_box(&s["parts"][1], &m), 0.01),
        "{}",
        arm["bbox"]
    );

    // the block, slid by nothing from its joint's origin, is moved exactly
    let block = &a["instances"][2];
    assert_eq!(block["exactness"], "Exact");
    assert_eq!(
        block["bbox"],
        json!([[-45.0, 13.0, 8.0], [-15.0, 29.0, 24.0]])
    );

    // the assembly's box holds every instance's
    let whole = &a["bbox"];
    for i in a["instances"].as_array().unwrap() {
        for k in 0..3 {
            assert!(whole[0][k].as_f64().unwrap() <= i["bbox"][0][k].as_f64().unwrap());
            assert!(whole[1][k].as_f64().unwrap() >= i["bbox"][1][k].as_f64().unwrap());
        }
    }
}

#[test]
fn a_quarter_turn_and_a_slide_are_exact_moves() {
    let mut s = hinged_stop();
    s["assembly"]["joints"][0]["value"] = json!(90);
    s["assembly"]["joints"][1]["value"] = json!(5);
    let a = pose(&s);
    let arm = &a["instances"][1];
    let m: Vec<f64> = arm["m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert!(near(&arm["bbox"], drawn_box(&s["parts"][1], &m), 0.01));
    assert_eq!(a["instances"][2]["bbox"][0][0], json!(-40.0));
    assert_eq!(a["instances"][2]["exactness"], "Exact");
}

#[test]
fn limits_hold_and_what_waits_says_so() {
    let mut s = hinged_stop();
    s["assembly"]["joints"][0]["value"] = json!(200);
    s["assembly"]["joints"][1]["type"] = json!("ball");
    let a = pose(&s);
    assert_eq!(a["joints"][0]["value"], json!(120.0));
    assert!(
        a["joints"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Held at its limit")
    );
    assert_eq!(a["joints"][1]["state"], "waits");
    assert!(a["joints"][1]["message"].as_str().unwrap().contains("G479"));
    // the block, with no joint the kernel can place, stays where it was put
    assert!(
        a["notes"][0]
            .as_str()
            .unwrap()
            .starts_with("Slide block is not joined")
    );
    assert_eq!(a["instances"][2]["state"], "built");

    let mut s = hinged_stop();
    s["assembly"]["instances"][1]["part"] = json!("nothing");
    assert_eq!(pose(&s)["instances"][1]["state"], "failed");
    assert!(pose_json("[]").contains("BadAssembly"));
}
