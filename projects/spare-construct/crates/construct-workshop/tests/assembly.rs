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

/// An instance's or a body's class and bound.
fn class_of(v: &Value) -> (String, Option<u64>) {
    (
        v["exactness"].as_str().unwrap().to_owned(),
        v["bound"].as_u64(),
    )
}

/// The least exact of a part's bodies as the kernel built them, and the largest bound.
fn built_class(part: &Value) -> (String, Option<u64>) {
    let a = serde_json::to_value(build_part(part)).unwrap();
    let mut worst = ("Exact".to_owned(), None);
    for b in a["bodies"].as_array().unwrap() {
        let (c, bound) = class_of(b);
        if c != "Exact" {
            worst.0 = c;
        }
        worst.1 = worst.1.max(bound);
    }
    worst
}

fn matrix(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

/// The box round every built instance's box.
fn holds_every_built_box(a: &Value) {
    let whole = &a["bbox"];
    for i in a["instances"].as_array().unwrap() {
        if i["state"] != "built" {
            assert_eq!(i["bbox"], Value::Null, "{i}");
            continue;
        }
        for k in 0..3 {
            assert!(whole[0][k].as_f64().unwrap() <= i["bbox"][0][k].as_f64().unwrap());
            assert!(whole[1][k].as_f64().unwrap() >= i["bbox"][1][k].as_f64().unwrap());
        }
    }
}

#[test]
fn the_kernel_poses_the_hinged_stop_and_its_hinge_waits() {
    let s = hinged_stop();
    let a = pose(&s);
    assert_eq!(a["engine"], "kernel");
    let states: Vec<&str> = a["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["state"].as_str().unwrap())
        .collect();
    // the bracket where it was put, the block slid; the arm, turned 35°, the kernel can't certify
    // yet, so it waits, naming the checker piece, and the page places it
    assert_eq!(states, ["built", "waits", "built"], "{:#}", a["instances"]);
    let arm = &a["instances"][1];
    let said = arm["message"].as_str().unwrap();
    assert!(said.contains("G134") || said.contains("G177"), "{said}");
    assert!(said.contains("the page places it"), "{said}");
    assert_eq!(arm["bbox"], Value::Null);
    // its map is still the one the page draws it by: turned 35° about z at the hinge's origin
    let m = matrix(&arm["m"]);
    let (c, sn) = (35f64.to_radians().cos(), 35f64.to_radians().sin());
    for (got, want) in m
        .iter()
        .zip([c, -sn, 0.0, 40.0, sn, c, 0.0, 0.0, 0.0, 0.0, 1.0, 38.0])
    {
        assert!((got - want).abs() < 1e-8, "{m:?}");
    }

    // the hinge waits as its arm does; the slider is built
    let hinge = &a["joints"][0];
    assert_eq!(hinge["state"], "waits", "{hinge}");
    assert_eq!(hinge["message"], arm["message"]);
    assert_eq!(a["joints"][1]["state"], "built");
    assert!(
        a["joints"][1]["message"]
            .as_str()
            .unwrap()
            .contains("Slid 0 mm along x by the kernel's transform.move")
    );

    // the block, slid by nothing from its joint's origin, is moved exactly
    let block = &a["instances"][2];
    assert_eq!(block["exactness"], "Exact");
    assert_eq!(
        block["bbox"],
        json!([[-45.0, 13.0, 8.0], [-15.0, 29.0, 24.0]])
    );

    // the bracket, grounded at the origin, is its own box
    let bracket = serde_json::to_value(build_part(&s["parts"][0])).unwrap();
    assert_eq!(a["instances"][0]["bbox"], bracket["bbox"]);
    assert_eq!(class_of(&a["instances"][0]), built_class(&s["parts"][0]));

    holds_every_built_box(&a);
}

#[test]
fn a_quarter_turn_and_a_slide_are_exact_moves() {
    let mut s = hinged_stop();
    s["assembly"]["joints"][0]["value"] = json!(90);
    s["assembly"]["joints"][1]["value"] = json!(5);
    let a = pose(&s);
    let arm = &a["instances"][1];
    assert_eq!(arm["state"], "built", "{arm}");
    // a quarter turn rounds nothing: the arm is as exact as the kernel built it
    assert_eq!(class_of(arm), built_class(&s["parts"][1]), "{arm}");
    assert_eq!(a["joints"][0]["state"], "built");
    assert!(
        a["joints"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Turned 90° about z by the kernel's transform.rotate")
    );
    // the arm's box is where the page draws it, to within the drawn circle's chords
    let m = matrix(&arm["m"]);
    assert!(
        near(&arm["bbox"], drawn_box(&s["parts"][1], &m), 0.01),
        "{}",
        arm["bbox"]
    );
    assert_eq!(a["instances"][2]["bbox"][0][0], json!(-40.0));
    assert_eq!(a["instances"][2]["exactness"], "Exact");
    holds_every_built_box(&a);
}

#[test]
fn limits_hold_and_what_waits_says_so() {
    let mut s = hinged_stop();
    s["assembly"]["joints"][0]["value"] = json!(200);
    s["assembly"]["joints"][1]["type"] = json!("ball");
    let a = pose(&s);
    assert_eq!(a["joints"][0]["value"], json!(120.0));
    // held at 120°, not a quarter turn, so it waits for the kernel's checker, and says both
    assert_eq!(a["joints"][0]["state"], "waits");
    let said = a["joints"][0]["message"].as_str().unwrap();
    assert!(
        said.contains("Held at its limit: it was set to 200."),
        "{said}"
    );
    assert!(said.contains("waits for the kernel's checker"), "{said}");
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
