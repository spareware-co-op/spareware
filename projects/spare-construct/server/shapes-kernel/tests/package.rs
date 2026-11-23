//! The kernel's package for the browser, natively: it answers the page exactly as the stand-in's
//! package does for the same builder, its edition is the server's kernel builder's, and each case
//! of `tests/cases.json` builds to what the kernel builder over solidcore's engine gives, a wall's
//! doors cut in the order of their keys; and every crate it shares with the server is at the
//! server's version. That the browser's build gives these same bytes is the server's
//! `tests/browser/same_bytes.mjs`'s to show, against `examples/answers.rs`.

// The crate denies these outside tests (Cargo.toml). Tests may use them, as the geometry kernel's
// do: a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "tests; the rules are for the crate's own code"
)]

use construct_model::element::Placement;
use construct_model::geometry::{Geometry, Recipe};
use construct_regen::kernel::{ENGINE, Hosted, Kernel, STEP};
use construct_shapes_kernel::{build, build_hosting, build_with, edition};
use geom_solidcore::engine::SolidcoreEngine;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn kernel() -> Kernel<SolidcoreEngine> {
    Kernel::new(SolidcoreEngine::new(STEP)).unwrap()
}

fn cases() -> Vec<Value> {
    let cases: Value = serde_json::from_str(include_str!("cases.json")).unwrap();
    cases["cases"].as_array().unwrap().clone()
}

fn case(name: &str) -> Value {
    cases().into_iter().find(|c| c["name"] == name).unwrap()
}

fn answer(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// The page reads both packages alike: for the same builder, this package's answer is the stand-in
/// package's, byte for byte, a shape, an unknown operation and text that is not a recipe alike.
#[test]
fn the_package_answers_as_the_stand_ins_does() {
    let stand_in = construct_regen::geometry();
    for recipe in [
        r#"{"owner":"construct","schema":1,"op":"block","op_version":1,
            "args":{"width":100,"depth":200,"height":300}}"#,
        r#"{"owner":"construct","schema":1,"op":"wall.straight","op_version":1,
            "args":{"length":4000,"thickness":200,"height":2700}}"#,
        r#"{"owner":"construct","schema":1,"op":"nothing","op_version":1,"args":{}}"#,
        "not json",
    ] {
        assert_eq!(
            build_with(stand_in.as_ref(), recipe),
            construct_shapes::build(recipe),
            "{recipe}"
        );
    }
}

/// Its edition is the kernel builder's over solidcore's engine on the building grid, as the
/// server's `kernel` setting makes it, so a document the server builds on the kernel is one this
/// package may build.
#[test]
fn the_edition_is_the_servers_kernel_builders() {
    let read: construct_model::geometry::Edition = serde_json::from_str(&edition()).unwrap();
    assert_eq!(read, kernel().edition());
    assert_eq!(read.engine, ENGINE);
    let report = read.report.unwrap();
    assert_eq!(report.engine, geom_solidcore::engine::ENGINE);
    assert_eq!(report.version, geom_solidcore::engine::SOLIDCORE);
    assert_eq!(report.steps_per_mm, 6400);
}

/// A recipe builds to the kernel builder's own shape: its body, bounds, names and displays.
#[test]
fn a_recipe_builds_as_the_kernel_builder_does() {
    for name in ["a wall", "a block"] {
        let recipe = case(name)["build"].clone();
        let parsed: Recipe = serde_json::from_value(recipe.clone()).unwrap();
        let native = kernel().build(&parsed).unwrap();
        let got = answer(&build(&recipe.to_string()));
        let shape = &got["shape"];
        assert_eq!(shape["body"], native.body_id(), "{name}");
        assert_eq!(shape["bounds"], json!(native.bounds), "{name}");
        assert_eq!(shape["names"], json!(native.names), "{name}");
        assert_eq!(shape["displays"], json!(native.displays), "{name}");
    }
}

/// A wall with two doors is cut as the kernel builder cuts it, each door's place in the wall the
/// difference of their placements, in the order of the doors' keys whatever order they come in.
#[test]
fn a_wall_with_two_doors_builds_as_the_kernel_builder_does() {
    let c = case("a wall with two doors");
    let k = kernel();
    let wall: Recipe = serde_json::from_value(c["host"]["recipe"].clone()).unwrap();
    let at: Placement = serde_json::from_value(c["host"]["placement"].clone()).unwrap();
    let doors: Vec<(String, Recipe, Placement)> = c["hosted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["key"].as_str().unwrap().to_owned(),
                serde_json::from_value(d["recipe"].clone()).unwrap(),
                serde_json::from_value(d["placement"].clone()).unwrap(),
            )
        })
        .collect();
    let hosted: Vec<Hosted<'_>> = doors
        .iter()
        .map(|(key, recipe, placed)| Hosted {
            key,
            recipe,
            at: k.units().offset(&at, placed).unwrap(),
        })
        .collect();
    let native = k.build_hosting(&wall, &hosted).unwrap();
    let got = answer(&build_hosting(
        &c["host"].to_string(),
        &c["hosted"].to_string(),
    ));
    let shape = &got["shape"];
    assert_eq!(shape["body"], native.body_id());
    assert_eq!(shape["bounds"], json!(native.bounds));
    assert_eq!(shape["names"], json!(native.names));
    assert_eq!(shape["displays"], json!(native.displays));
    assert!(
        native.names.iter().any(|n| n.role.starts_with("tool(")),
        "the doors' faces are named"
    );

    let mut backward = c["hosted"].as_array().unwrap().clone();
    backward.reverse();
    let again = build_hosting(&c["host"].to_string(), &Value::Array(backward).to_string());
    assert_eq!(
        again,
        build_hosting(&c["host"].to_string(), &c["hosted"].to_string())
    );
}

/// Every version of a package in a lock file, by name.
fn versions(lock: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            package
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|v| v.trim_matches('"').to_owned())
                .unwrap()
        };
        found
            .entry(field("name = "))
            .or_default()
            .insert(field("version = "));
    }
    found
}

/// The browser builds with the very crates the server's `kernel` feature builds with: every
/// package this workspace's lock shares with the server's is at the server's version, but
/// wasm-bindgen's, which this package pins to the version wasm-bindgen-cli is pinned to and the
/// server builds only for the browser's target, never natively.
#[test]
fn every_crate_the_server_builds_is_at_the_servers_version() {
    const BROWSER_ONLY: [&str; 4] = [
        "wasm-bindgen",
        "wasm-bindgen-macro",
        "wasm-bindgen-macro-support",
        "wasm-bindgen-shared",
    ];
    let here = versions(include_str!("../Cargo.lock"));
    let server = versions(include_str!("../../Cargo.lock"));
    assert!(here.contains_key("geom-solidcore") && here.contains_key("solidcore"));
    let mut shared = 0;
    for (name, mine) in &here {
        let Some(theirs) = server.get(name) else {
            continue;
        };
        shared += 1;
        if BROWSER_ONLY.contains(&name.as_str()) {
            assert_eq!(mine.iter().collect::<Vec<_>>(), ["0.2.118"], "{name}");
        } else {
            assert!(
                mine.is_subset(theirs),
                "{name}: {mine:?} here, {theirs:?} on the server"
            );
        }
    }
    assert!(
        shared > 50,
        "only {shared} packages shared with the server's lock"
    );
}

/// What can't be built comes back as an answer the page stores as the server would: text that is
/// not a host or not a list of hosted elements, and the builder's own refusals, with their codes.
#[test]
fn a_hosting_that_cant_build_says_why() {
    let c = case("a wall with two doors");
    let host = c["host"].to_string();
    let failed = |text: String| answer(&text)["failed"]["code"].as_str().unwrap().to_owned();
    assert_eq!(failed(build_hosting("not json", "[]")), "BadRecipe");
    assert_eq!(failed(build_hosting(&host, "{}")), "BadRecipe");
    let mut past = c["hosted"].clone();
    past[0]["placement"]["x"] = json!(3500);
    assert_eq!(
        failed(build_hosting(&host, &past.to_string())),
        "OpeningPastEnd"
    );
    let unknown = json!({ "recipe": { "owner": "construct", "schema": 1, "op": "nothing",
        "op_version": 1, "args": {} }, "placement": { "x": 0, "y": 0, "z": 0 } });
    assert_eq!(
        failed(build_hosting(&unknown.to_string(), "[]")),
        "UnknownOperation"
    );
    assert_eq!(failed(build("not json")), "BadRecipe");
}
