//! Workshop's parts, built by the geometry kernel: the demo's sample bracket, and each way a recipe can
//! go right or wrong. The bracket is the same recipe as the demo's (`demo/workshop/js/model.js`).

use construct_workshop::{build_json, build_part};
use serde_json::{Value, json};

fn bracket() -> Value {
    serde_json::from_str(include_str!("bracket.json")).unwrap()
}

fn answer(part: &Value) -> Value {
    serde_json::to_value(build_part(part)).unwrap()
}

fn states(a: &Value) -> Vec<String> {
    a["features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["state"].as_str().unwrap().to_owned())
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// The corners of a prism body, where its map puts them.
fn corners(b: &Value) -> Vec<[f64; 3]> {
    let m: Vec<f64> = b["m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let z0 = b["z"].as_f64().unwrap();
    let z1 = z0 + b["height"].as_f64().unwrap();
    let mut out = Vec::new();
    for p in b["outline"].as_array().unwrap() {
        let (x, y) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
        for z in [z0, z1] {
            out.push(
                [0, 1, 2]
                    .map(|i| m[i * 4] * x + m[i * 4 + 1] * y + m[i * 4 + 2] * z + m[i * 4 + 3]),
            );
        }
    }
    out
}

#[test]
fn the_sample_bracket_builds_and_waits_where_the_kernel_does() {
    let a = answer(&bracket());
    assert_eq!(a["engine"], "kernel");
    assert_eq!(
        states(&a),
        [
            "built", "built", "built", "built", "built", "built", "built", "built", "waits",
            "waits"
        ]
    );
    assert!(
        a["features"][8]["message"]
            .as_str()
            .unwrap()
            .contains("G130")
    );
    assert!(
        a["features"][9]["message"]
            .as_str()
            .unwrap()
            .contains("G263")
    );
    let bodies = a["bodies"].as_array().unwrap();
    let ids: Vec<&str> = bodies.iter().map(|b| b["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["f2", "f4", "f5:f4", "f7", "f8:f7:0.1", "f8:f7:0.2"]);

    let plate = &bodies[0];
    assert_eq!(
        (
            plate["kind"].as_str(),
            plate["faces"].as_u64(),
            plate["exactness"].as_str()
        ),
        (Some("prism"), Some(6), Some("Exact"))
    );
    assert_eq!(plate["volume_from"], "kernel");
    assert!(close(plate["volume"].as_f64().unwrap(), 120.0 * 60.0 * 8.0));

    let boss = &bodies[1];
    assert_eq!(
        (
            boss["kind"].as_str(),
            boss["faces"].as_u64(),
            boss["exactness"].as_str()
        ),
        (Some("cylinder"), Some(3), Some("Certified"))
    );
    assert!(close(
        boss["volume"].as_f64().unwrap(),
        std::f64::consts::PI * 144.0 * 30.0
    ));

    let mirrored = &bodies[2];
    assert_eq!(mirrored["by"], "f5");
    assert!(close(mirrored["bbox"][0][0].as_f64().unwrap(), -52.0));
    assert!(close(mirrored["bbox"][1][0].as_f64().unwrap(), -28.0));

    let lug = &bodies[3];
    assert_eq!(lug["faces"], 8);
    let copy = &bodies[5];
    assert!(close(
        copy["bbox"][0][1].as_f64().unwrap() - lug["bbox"][0][1].as_f64().unwrap(),
        40.0
    ));
    assert!(close(
        copy["volume"].as_f64().unwrap(),
        lug["volume"].as_f64().unwrap()
    ));

    let bb = &a["bbox"];
    assert_eq!(bb, &json!([[-60.0, -30.0, 0.0], [60.0, 30.0, 38.0]]));
}

#[test]
fn every_prism_is_drawn_where_the_kernel_says_it_is() {
    // The page draws a body from its profile and map; the kernel's box of the body must match exactly.
    let a = answer(&bracket());
    for b in a["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["kind"] == "prism")
    {
        let pts = corners(b);
        for i in 0..3 {
            let lo = pts.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
            let hi = pts.iter().map(|p| p[i]).fold(f64::NEG_INFINITY, f64::max);
            assert!(
                close(lo, b["bbox"][0][i].as_f64().unwrap()),
                "{} min {i}",
                b["id"]
            );
            assert!(
                close(hi, b["bbox"][1][i].as_f64().unwrap()),
                "{} max {i}",
                b["id"]
            );
        }
        // and the kernel's exact volume is the profile's area times the height
        let o = b["outline"].as_array().unwrap();
        let area2: f64 = (0..o.len())
            .map(|i| {
                let (p, q) = (&o[i], &o[(i + 1) % o.len()]);
                p[0].as_f64().unwrap() * q[1].as_f64().unwrap()
                    - q[0].as_f64().unwrap() * p[1].as_f64().unwrap()
            })
            .sum();
        assert!(
            close(
                b["volume"].as_f64().unwrap(),
                area2 / 2.0 * b["height"].as_f64().unwrap()
            ),
            "{}",
            b["id"]
        );
    }
}

#[test]
fn rolling_back_builds_the_part_as_it_was() {
    let mut p = bracket();
    p["end"] = json!(2);
    let a = answer(&p);
    assert_eq!(states(&a)[..3], ["built", "built", "rolled_back"]);
    assert_eq!(a["bodies"].as_array().unwrap().len(), 1);
    p["end"] = json!(0);
    let a = answer(&p);
    assert!(a["bodies"].as_array().unwrap().is_empty());
    assert!(a["bbox"].is_null());
}

#[test]
fn a_suppressed_feature_fails_the_ones_that_need_it_and_no_others() {
    let mut p = bracket();
    p["features"][2]["suppressed"] = json!(true);
    let a = answer(&p);
    assert_eq!(
        states(&a)[..8],
        [
            "built",
            "built",
            "suppressed",
            "failed",
            "failed",
            "built",
            "built",
            "built"
        ]
    );
    assert!(
        a["features"][3]["message"]
            .as_str()
            .unwrap()
            .contains("not built (suppressed)")
    );
}

#[test]
fn a_parameter_drives_every_feature_that_names_it() {
    let mut p = bracket();
    p["params"][0]["value"] = json!(200);
    let a = answer(&p);
    assert!(close(
        a["bodies"][0]["volume"].as_f64().unwrap(),
        200.0 * 60.0 * 8.0
    ));
    p["params"][0]["value"] = json!("nothing_called_this");
    let a = answer(&p);
    assert_eq!(a["params"][0]["state"], "failed");
    assert_eq!(states(&a)[..3], ["failed", "failed", "built"]);
    assert!(
        a["features"][0]["message"]
            .as_str()
            .unwrap()
            .contains("no parameter called width")
    );
}

#[test]
fn a_parameter_can_be_a_formula_of_others_with_units() {
    let mut p = bracket();
    // depth is defined from width, which comes before it or after it alike
    p["params"][1]["value"] = json!("width / 2 + 0.5 in");
    p["params"][2]["value"] = json!("round(depth / 10, 0.5 mm)");
    let a = answer(&p);
    assert_eq!(a["params"][1]["value"], json!(72.7));
    assert_eq!(a["params"][1]["formula"], json!(true));
    assert_eq!(a["params"][2]["value"], json!(7.5));
    assert!(close(
        a["bodies"][0]["volume"].as_f64().unwrap(),
        120.0 * 72.7 * 7.5
    ));
    // a field takes a formula too
    p["features"][1]["depth"] = json!("thickness * 2");
    let a = answer(&p);
    assert!(close(a["bodies"][0]["height"].as_f64().unwrap(), 15.0));

    // units that don't agree, and a loop, are refused, and say why
    p["params"][1]["value"] = json!("width + 5");
    let a = answer(&p);
    assert_eq!(a["params"][1]["state"], "failed");
    let why = a["params"][1]["message"].as_str().unwrap();
    assert!(why.contains("give the number a unit"), "{why}");
    assert_eq!(a["params"][2]["state"], "failed", "thickness uses depth");
    p["params"][0]["value"] = json!("depth * 2");
    p["params"][1]["value"] = json!("width / 2");
    let a = answer(&p);
    let why = a["params"][0]["message"].as_str().unwrap();
    assert!(
        why.contains("width, depth are defined from each other"),
        "{why}"
    );
    assert_eq!(states(&a)[0], "failed");

    // a count is a count: a length won't do for the number of sides
    let mut p = bracket();
    p["params"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "sides", "value": 8, "kind": "count" }));
    p["features"][5]["n"] = json!("sides - 2");
    let a = answer(&p);
    assert_eq!(states(&a)[5], "built");
    assert_eq!(a["params"][7]["kind"], json!("count"));
    p["features"][5]["n"] = json!("lug_pitch");
    let a = answer(&p);
    let why = a["features"][5]["message"].as_str().unwrap();
    assert!(
        why.contains("a length, and it should be a plain number"),
        "{why}"
    );
}

fn one_outline(pts: Value) -> Value {
    json!({ "params": [], "end": 2, "features": [
        { "id": "s", "type": "sketch", "name": "S", "shape": "outline", "pts": pts },
        { "id": "e", "type": "extrude", "name": "E", "sketch": "s", "depth": 5, "op": "new" } ] })
}

#[test]
fn an_outline_that_is_not_convex_waits_for_the_kernel() {
    let a = answer(&one_outline(json!([
        [0, 0],
        [20, 0],
        [10, 5],
        [20, 20],
        [0, 20]
    ])));
    assert_eq!(a["features"][0]["convex"], false);
    assert_eq!(states(&a), ["built", "waits"]);
    assert!(
        a["features"][1]["message"]
            .as_str()
            .unwrap()
            .contains("G255")
    );
    // a star turns left at every corner, but goes round twice: not convex either
    let star = json!([[0, -10], [6, 8], [-9.5, -3], [9.5, -3], [-6, 8]]);
    assert_eq!(answer(&one_outline(star))["features"][0]["convex"], false);
}

#[test]
fn a_clockwise_outline_is_taken_anticlockwise() {
    let a = answer(&one_outline(json!([[0, 0], [0, 10], [10, 10], [10, 0]])));
    assert_eq!(states(&a), ["built", "built"]);
    assert!(
        a["features"][0]["message"]
            .as_str()
            .unwrap()
            .contains("taken anticlockwise")
    );
    assert!(close(a["volume"].as_f64().unwrap(), 500.0));
}

#[test]
fn joins_and_cuts_wait_for_the_booleans() {
    let mut p = one_outline(json!([[0, 0], [10, 0], [10, 10], [0, 10]]));
    for op in ["join", "cut", "intersect"] {
        p["features"][1]["op"] = json!(op);
        let a = answer(&p);
        assert_eq!(states(&a), ["built", "waits"]);
        assert!(
            a["features"][1]["message"]
                .as_str()
                .unwrap()
                .contains("G130")
        );
    }
}

#[test]
fn a_grid_pattern_and_a_move_use_the_kernels_transforms() {
    let p = json!({ "params": [], "features": [
        { "id": "s", "type": "sketch", "name": "S", "shape": "rect", "w": 4, "h": 2, "cx": 0, "cy": 0 },
        { "id": "e", "type": "extrude", "name": "E", "sketch": "s", "depth": 1 },
        { "id": "g", "type": "pattern", "name": "G", "of": "e", "count": 3, "dx": 10, "count2": 2, "dy2": 5 },
        { "id": "m", "type": "move", "name": "M", "of": "g", "dx": 0, "dy": 0, "dz": 7.5 } ] });
    let a = answer(&p);
    assert_eq!(states(&a), ["built", "built", "built", "built"]);
    let bodies = a["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 6);
    assert!(close(a["volume"].as_f64().unwrap(), 6.0 * 8.0));
    let moved: Vec<&Value> = bodies
        .iter()
        .filter(|b| b["bbox"][0][2].as_f64().unwrap() > 7.0)
        .collect();
    assert_eq!(
        moved.len(),
        5,
        "the move moves the pattern's copies, not the first body"
    );
    assert_eq!(a["bbox"], json!([[-2.0, -1.0, 0.0], [22.0, 6.0, 8.5]]));
}

#[test]
fn sizes_past_the_kernels_reach_fail_with_a_reason() {
    let p = json!({ "params": [], "features": [
        { "id": "s", "type": "sketch", "name": "S", "shape": "circle", "r": 5, "cx": 7.0e9, "cy": 0 } ] });
    let a = answer(&p);
    assert_eq!(states(&a), ["failed"]);
    assert!(
        a["features"][0]["message"]
            .as_str()
            .unwrap()
            .contains("reach")
    );
}

#[test]
fn references_must_point_back_to_something_built() {
    let p = json!({ "params": [], "features": [
        { "id": "e", "type": "extrude", "name": "E", "sketch": "s", "depth": 1 },
        { "id": "s", "type": "sketch", "name": "S", "shape": "rect", "w": 4, "h": 2 },
        { "id": "x", "type": "extrude", "name": "X", "sketch": "", "depth": 1 },
        { "id": "m", "type": "mirror", "name": "M", "of": "s" },
        { "id": "w", "type": "warp", "name": "W" } ] });
    let a = answer(&p);
    assert_eq!(
        states(&a),
        ["failed", "built", "failed", "failed", "failed"]
    );
    assert!(
        a["features"][0]["message"]
            .as_str()
            .unwrap()
            .contains("later")
    );
    assert!(
        a["features"][3]["message"]
            .as_str()
            .unwrap()
            .contains("made a body")
    );
}

#[test]
fn text_that_is_not_a_recipe_comes_back_as_an_answer() {
    let bad: Value = serde_json::from_str(&build_json("not json")).unwrap();
    assert_eq!(bad["failed"]["code"], "BadRecipe");
    let arr: Value = serde_json::from_str(&build_json("[1, 2]")).unwrap();
    assert_eq!(arr["failed"]["code"], "BadRecipe");
    let ok: Value = serde_json::from_str(&build_json(include_str!("bracket.json"))).unwrap();
    assert_eq!(ok["bodies"].as_array().unwrap().len(), 6);
}
