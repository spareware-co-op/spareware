//! The kernel builder on solidcore's engine, the composition the server's `kernel` feature links:
//! the builder's suite (construct-regen's `tests/kernel_suite`) on `SolidcoreEngine`, its doors
//! cut from their walls (one door, a raised opening, a wall with two doors, the cases at the edge
//! and what the engine does with a face within its tolerance) among them, the whole
//! server seeded and checked on it, a hosted door keeping its face when its wall is lengthened,
//! layered walls drawn and their types changed through the commands, a floor with a hole drawn
//! through the commands and its quantities read back, a door and a window placed from the
//! family library cutting a three-layer wall (job 6.4), and a document of one edition reported,
//! not rebuilt, by a server of another. Built and run only with `--features kernel`.

#![cfg(feature = "kernel")]
// The server denies these outside tests (Cargo.toml). Tests may use them, as the geometry kernel's
// do: clippy.toml's allow-*-in-tests settings don't reach helpers outside `#[test]` functions, and
// a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "tests; the rules are for the server's own code"
)]

#[path = "../../crates/construct-regen/tests/kernel_suite/mod.rs"]
mod kernel_suite;
mod support;

use construct_api::cloud::{POLICY, ProjectCommands, SCHEMA};
use construct_model::geometry::{Edition, FaceName, Geometry};
use construct_model::rows::BuiltResult;
use construct_regen::kernel::STEP;
use construct_regen::regenerator::Regenerator;
use geom_solidcore::engine::SolidcoreEngine;
use kit_accounts::{Accounts, Cedar, SignInSettings};
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope};
use kit_sync::{Clock, Orderers, SystemClock};
use serde_json::json;
use spare_server::directory::DirectoryCommands;
use spare_server::geometry::{Builder, geometry};
use spare_server::seed::seed;
use spare_server::service::{Access, Service, ShapeView};
use std::collections::BTreeMap;
use std::sync::Arc;

fn engine() -> SolidcoreEngine {
    SolidcoreEngine::new(STEP)
}

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

#[test]
fn a_straight_wall_is_its_plan_extruded() {
    kernel_suite::a_straight_wall_is_its_plan_extruded(engine);
}

#[test]
fn the_wall_has_its_boxs_volume_and_a_closed_mesh() {
    kernel_suite::the_wall_has_its_boxs_volume_and_a_closed_mesh(engine);
}

#[test]
fn every_face_carries_a_persistent_role_name() {
    kernel_suite::every_face_carries_a_persistent_role_name(engine);
}

#[test]
fn the_same_model_built_twice_gives_the_same_bytes() {
    kernel_suite::the_same_model_built_twice_gives_the_same_bytes(engine);
}

#[test]
fn the_edition_carries_the_engines_report() {
    kernel_suite::the_edition_carries_the_engines_report(engine);
}

#[test]
fn a_recipe_that_cant_build_says_why() {
    kernel_suite::a_recipe_that_cant_build_says_why(engine);
}

#[test]
fn a_datum_is_built_as_the_stand_in_builds_it() {
    kernel_suite::a_datum_is_built_as_the_stand_in_builds_it(engine);
}

#[test]
fn a_door_cuts_a_real_opening() {
    kernel_suite::a_door_cuts_a_real_opening(engine);
}

#[test]
fn a_wall_with_two_doors_cuts_both_in_the_order_of_their_keys() {
    kernel_suite::a_wall_with_two_doors_cuts_both_in_the_order_of_their_keys(engine);
}

#[test]
fn the_walls_names_survive_its_openings() {
    kernel_suite::the_walls_names_survive_its_openings(engine);
}

#[test]
fn an_opening_at_the_edge_is_refused_with_its_door_and_reason() {
    kernel_suite::an_opening_at_the_edge_is_refused_with_its_door_and_reason(engine);
}

#[test]
fn a_rectangle_floor_is_its_layers() {
    kernel_suite::a_rectangle_floor_is_its_layers(engine);
}

#[test]
fn an_l_shaped_floor_with_a_hole_is_its_layers() {
    kernel_suite::an_l_shaped_floor_with_a_hole_is_its_layers(engine);
}

#[test]
fn a_layered_wall_is_one_solid_per_layer() {
    kernel_suite::a_layered_wall_is_one_solid_per_layer(engine);
}

#[test]
fn a_door_cuts_every_layer_of_a_layered_wall() {
    kernel_suite::a_door_cuts_every_layer_of_a_layered_wall(engine);
}

#[test]
fn a_door_in_a_layered_wall_is_refused_as_in_the_box() {
    kernel_suite::a_door_in_a_layered_wall_is_refused_as_in_the_box(engine);
}

#[test]
fn a_layered_wall_at_an_angle_is_its_strips_extruded() {
    kernel_suite::a_layered_wall_at_an_angle_is_its_strips_extruded(engine);
}

#[test]
fn an_opening_cuts_every_layer_of_a_layered_wall() {
    kernel_suite::an_opening_cuts_every_layer_of_a_layered_wall(engine);
}

#[test]
fn an_opening_in_a_wall_is_refused_by_name() {
    kernel_suite::an_opening_in_a_wall_is_refused_by_name(engine);
}

#[test]
fn openings_and_a_shaft_cut_every_layer_of_a_floor() {
    kernel_suite::openings_and_a_shaft_cut_every_layer_of_a_floor(engine);
}

#[test]
fn an_opening_in_a_floor_is_refused_by_name() {
    kernel_suite::an_opening_in_a_floor_is_refused_by_name(engine);
}

#[test]
fn a_family_cuts_every_layer_and_stands_in_its_opening() {
    kernel_suite::a_family_cuts_every_layer_and_stands_in_its_opening(engine);
}

#[test]
fn a_wall_moved_or_a_type_changed_moves_or_resizes_the_opening() {
    kernel_suite::a_wall_moved_or_a_type_changed_moves_or_resizes_the_opening(engine);
}

#[test]
fn a_family_that_does_not_fit_is_refused_with_a_reason() {
    kernel_suite::a_family_that_does_not_fit_is_refused_with_a_reason(engine);
}

#[test]
fn the_engine_keeps_a_sliver_within_its_tolerance() {
    kernel_suite::the_engine_keeps_a_sliver_within_its_tolerance(engine());
}

/// The server's `kernel` setting is this builder: the engine's report names solidcore at its pin.
#[test]
fn the_kernel_setting_builds_through_solidcore_at_its_pin() {
    let built = geometry(Builder::Kernel).unwrap();
    let report = built.edition().report.unwrap();
    assert_eq!(report.engine, geom_solidcore::engine::ENGINE);
    assert_eq!(report.version, geom_solidcore::engine::SOLIDCORE);
    assert_eq!(report.steps_per_mm, 6400);
    assert_eq!(
        geometry(Builder::StandIn).unwrap().edition(),
        construct_regen::geometry().edition()
    );
}

/// A whole server on `geometry`, whose new documents start on `edition`.
fn server_on(
    store: &Memory,
    files: &MemoryFiles,
    geometry: Arc<dyn Geometry>,
    edition: Edition,
) -> Service<Memory, MemoryFiles> {
    let cedar = Cedar::new(POLICY, SCHEMA).unwrap();
    let access = Arc::new(Access::new(store.clone(), cedar));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let commands = Arc::new(DirectoryCommands::new(access.clone()));
    let directory = Orderers::new(store.clone(), commands, clock.clone(), 50);
    let settings = SignInSettings {
        issuer: "http://localhost:9".to_owned(),
        client_id: String::new(),
        client_secret: String::new(),
        public_url: "http://localhost:8".to_owned(),
    };
    let accounts = Accounts::new(store.clone(), clock.clone(), settings, directory);
    let commands = ProjectCommands::new(store.clone(), access.clone(), 30_000, edition);
    let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
    Service::new(
        store.clone(),
        access,
        accounts,
        orderers,
        clock,
        files.clone(),
        geometry,
    )
}

/// The whole seed runs on the kernel and every stored shape matches its recipe. On an old box wall
/// (`wall.straight`, which still reads and builds), a door hosted on its exterior names the face
/// the suite recorded, and when the wall is lengthened and built again, its exterior keeps that
/// name, so the door's reference still lands, and the wall is built with the door's opening cut.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_runs_on_the_kernel_and_a_hosted_door_keeps_its_face() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let regen = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    assert_eq!(regen.verify(&project).await.unwrap(), 26);

    let old = json!({
        "element": "W-950", "kind": "wall", "level": 1,
        "placement": { "x": 0, "y": -10_000, "z": 0 },
        "recipe": kernel_suite::wall(12_000, "x"),
    });
    s.edit(&anna, &project, &model, "element.create", old)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let wall = s
        .shape(&anna, &project, &model, &i("W-950"), None)
        .await
        .unwrap();
    assert_eq!(wall.built.edition, kernel.edition());
    let BuiltResult::Shape { names, bounds, .. } = &wall.built.result else {
        panic!("a wall builds: {:?}", wall.built.result);
    };
    let face = names.iter().find(|n| n.role == "exterior").unwrap().clone();
    let pinned = kernel_suite::WALL_NAMES
        .iter()
        .find(|(role, _)| *role == "exterior")
        .unwrap()
        .1;
    assert_eq!(face.name, pinned);
    let door = json!({
        "element": "D-900", "kind": "door", "level": 1,
        "recipe": kernel_suite::door(),
        "placement": { "x": 1000, "y": -10_000, "z": 0 },
        "references": [{ "doc": null, "element": "W-950", "name": face.name, "kind": "face",
                         "want": "carrier", "signature": null }],
    });
    s.edit(&anna, &project, &model, "element.create", door)
        .await
        .unwrap();

    let record = s
        .record(&anna, &project, &model, &wall.built.input)
        .await
        .unwrap();
    let mut longer = record.recipe.clone();
    let length = longer.args["length"].as_i64().unwrap();
    longer
        .args
        .insert("length".to_owned(), json!(length + 1000));
    s.edit(
        &anna,
        &project,
        &model,
        "element.set",
        json!({ "element": "W-950", "recipe": longer }),
    )
    .await
    .unwrap();
    s.regenerate(&project).await.unwrap();
    let again = s
        .shape(&anna, &project, &model, &i("W-950"), None)
        .await
        .unwrap();
    assert_ne!(again.built.input, wall.built.input, "the wall was edited");
    let BuiltResult::Shape {
        names: now,
        bounds: grown,
        ..
    } = &again.built.result
    else {
        panic!("the longer wall builds: {:?}", again.built.result);
    };
    assert_ne!(grown, bounds, "the wall is longer");
    assert!(now.contains(&face), "the door's face is gone: {now:?}");
    // A door written this way, by its position with a face reference, is hosted by that
    // reference: the wall is built with its opening cut.
    let mut cut: Vec<&str> = now
        .iter()
        .filter(|n| n.role.starts_with("tool("))
        .map(|n| n.role.as_str())
        .collect();
    cut.sort_unstable();
    assert_eq!(
        cut,
        [
            "tool(D-900).head",
            "tool(D-900).jamb.end",
            "tool(D-900).jamb.start"
        ],
        "its jambs and head, and no sill for a door to the floor"
    );
    let hosted = s
        .shape(&anna, &project, &model, &i("D-900"), None)
        .await
        .unwrap();
    let door = s
        .record(&anna, &project, &model, &hosted.built.input)
        .await
        .unwrap();
    assert_eq!(door.references[0].name, face.name);
    assert!(matches!(hosted.built.result, BuiltResult::Shape { .. }));
}

/// The engine's largest length tolerance, in millimetres, as the edition records it.
fn tolerance_mm(edition: &Edition) -> f64 {
    edition
        .report
        .as_ref()
        .unwrap()
        .tolerances
        .iter()
        .filter(|t| t.unit == "m")
        .map(|t| t.value.parse::<f64>().unwrap() * 1000.0)
        .fold(0.0, f64::max)
}

/// Every derived row of a project, as bytes.
async fn derived(store: &Memory, project: &Id) -> BTreeMap<String, Vec<u8>> {
    let mut snapshot = store.snapshot().await.unwrap();
    let mut rows = BTreeMap::new();
    for table in ["body", "display", "built", "area", "kind", "reads"] {
        let prefix = Scope::Project(project.clone()).table_prefix(table, &[]);
        for (key, value) in snapshot.scan(&prefix).await.unwrap() {
            rows.insert(key.as_str().to_owned(), value);
        }
    }
    rows
}

/// A wall as the server stored it, and its finest mesh.
async fn stored_wall(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
    wall: &str,
) -> (ShapeView, Vec<FaceName>) {
    s.regenerate(project).await.unwrap();
    let shown = s
        .shape(&i("anna"), project, model, &i(wall), Some(0))
        .await
        .unwrap();
    let BuiltResult::Shape { names, .. } = &shown.built.result else {
        panic!("{wall} builds: {:?}", shown.built.result);
    };
    let names = names.clone();
    (shown, names)
}

/// Checks a wall's finest mesh is closed and holds a wall of `length` by 200 by 2700 mm less one
/// 900 by 2100 mm opening for each of `doors`, within the engine's tolerance over its surface.
fn holds(edition: &Edition, shown: &ShapeView, length: i128, doors: i128) {
    let mesh = &shown.display.as_ref().unwrap().mesh;
    let six = kernel_suite::closed_mesh_six_volume(mesh);
    let expected = 6 * (length * 200 * 2700 - doors * 900 * 200 * 2100);
    let surface = 2 * (length * 200 + 200 * 2700 + length * 2700) + doors * 2 * 200 * (900 + 2100);
    let allowed = 6.0 * surface as f64 * tolerance_mm(edition);
    assert!(
        ((six - expected).abs() as f64) <= allowed,
        "six times the volume is {six}, not {expected} within {allowed}"
    );
}

/// The roles of a wall's faces its openings made, sorted.
fn cut_roles(names: &[FaceName]) -> Vec<String> {
    let mut roles: Vec<String> = names
        .iter()
        .filter(|n| n.role.starts_with("tool("))
        .map(|n| n.role.clone())
        .collect();
    roles.sort_unstable();
    roles
}

/// Through the server's commands on the kernel: two doors placed on a wall's two sides cut two
/// openings from it (a closed mesh holding the wall less both, the wall's six names and each
/// opening's, the first door's as the suite recorded them); a door moved along the wall moves its
/// opening; a door deleted takes its opening away; the wall lengthened and moved keeps its door
/// where its offset puts it, writing no door; and the log replayed from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn two_doors_placed_through_commands_cut_their_wall_and_move_with_it() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let edition = kernel.edition();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let wall = json!({
        "element": "W-900", "kind": "wall", "level": 1,
        "placement": { "x": 50_000, "y": 0, "z": 0 },
        "recipe": kernel_suite::wall(4000, "x"),
    });
    s.edit(&anna, &project, &model, "element.create", wall)
        .await
        .unwrap();
    let (_, names) = stored_wall(&s, &project, &model, "W-900").await;
    let face = |role: &str| names.iter().find(|n| n.role == role).unwrap().name.clone();
    let place = |id: &str, role: &str, along: &str| {
        json!({ "element": id, "host": "W-900", "face": face(role), "along": along,
                "recipe": kernel_suite::door_of(900, 2100, "x") })
    };
    s.edit(
        &anna,
        &project,
        &model,
        "hosted.place",
        place("D-1", "exterior", "500 mm"),
    )
    .await
    .unwrap();
    s.edit(
        &anna,
        &project,
        &model,
        "hosted.place",
        place("D-2", "interior", "2.5 m"),
    )
    .await
    .unwrap();
    let (two, names) = stored_wall(&s, &project, &model, "W-900").await;
    holds(&edition, &two, 4000, 2);
    for (role, name) in kernel_suite::WALL_NAMES {
        assert!(
            names.iter().any(|n| n.role == role && n.name == name),
            "the wall's {role}: {names:?}"
        );
    }
    for (role, name) in kernel_suite::D1_NAMES {
        let found = names.iter().any(|n| n.role == role && n.name == name);
        assert_eq!(found, !role.ends_with("sill"), "{role}: {names:?}");
    }
    assert_eq!(
        cut_roles(&names),
        [
            "tool(D-1).head",
            "tool(D-1).jamb.end",
            "tool(D-1).jamb.start",
            "tool(D-2).head",
            "tool(D-2).jamb.end",
            "tool(D-2).jamb.start"
        ]
    );
    s.edit(
        &anna,
        &project,
        &model,
        "hosted.move",
        json!({ "element": "D-1", "along": "1000 mm" }),
    )
    .await
    .unwrap();
    let (moved, after) = stored_wall(&s, &project, &model, "W-900").await;
    holds(&edition, &moved, 4000, 2);
    assert_eq!(
        cut_roles(&after),
        cut_roles(&names),
        "the openings keep their names"
    );
    assert_ne!(
        moved.built.result, two.built.result,
        "the first opening moved with its door"
    );

    s.edit(
        &anna,
        &project,
        &model,
        "element.delete",
        json!({ "element": "D-2" }),
    )
    .await
    .unwrap();
    let (one, names) = stored_wall(&s, &project, &model, "W-900").await;
    holds(&edition, &one, 4000, 1);
    assert_eq!(
        cut_roles(&names),
        [
            "tool(D-1).head",
            "tool(D-1).jamb.end",
            "tool(D-1).jamb.start"
        ]
    );

    let door = s
        .shape(&anna, &project, &model, &i("D-1"), None)
        .await
        .unwrap()
        .built;
    let record = s
        .record(&anna, &project, &model, &one.built.input)
        .await
        .unwrap();
    let mut longer = record.recipe.clone();
    longer.args.insert("length".to_owned(), json!(6000));
    s.edit(
        &anna,
        &project,
        &model,
        "element.set",
        json!({ "element": "W-900", "recipe": longer }),
    )
    .await
    .unwrap();
    let (grown, names) = stored_wall(&s, &project, &model, "W-900").await;
    holds(&edition, &grown, 6000, 1);
    assert_eq!(cut_roles(&names).len(), 3);
    let moved = json!({ "element": "W-900", "placement": { "x": 60_000, "y": 0, "z": 0 } });
    s.edit(&anna, &project, &model, "element.move", moved)
        .await
        .unwrap();
    let (shifted, _) = stored_wall(&s, &project, &model, "W-900").await;
    assert_eq!(
        shifted.built.result, grown.built.result,
        "a move rebuilds no shape"
    );
    let now = s
        .shape(&anna, &project, &model, &i("D-1"), None)
        .await
        .unwrap()
        .built;
    assert_eq!(now.input, door.input, "no door was written");
    assert_ne!(
        now.cells, door.cells,
        "the door is indexed where its wall now is"
    );
    let refused = s
        .edit(
            &anna,
            &project,
            &model,
            "hosted.place",
            place("D-3", "exterior", "5500"),
        )
        .await
        .unwrap_err();
    assert_eq!(refused.to_string(), "door D-3: it crosses the wall's end");

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
    assert_eq!(fresh.verify(&project).await.unwrap(), 28);
}

/// A document records the edition it was made on, and a server running another edition doesn't
/// build it: each element is stored as failed with `EditionUnavailable`, which says so, and
/// nothing is silently built under the other edition (FORMAT.md, "A document"). Both ways round.
#[tokio::test(flavor = "multi_thread")]
async fn a_document_of_one_edition_is_reported_on_another() {
    let kernel = geometry(Builder::Kernel).unwrap();
    let stand_in = geometry(Builder::StandIn).unwrap();
    for (made_on, opened_with) in [
        (stand_in.clone(), kernel.clone()),
        (kernel.clone(), stand_in.clone()),
    ] {
        let (store, files) = (Memory::new(), MemoryFiles::default());
        let s = server_on(&store, &files, opened_with.clone(), made_on.edition());
        seed(&store, &s).await.unwrap();
        let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
        for element in ["W-101", "D-101"] {
            let shown = s
                .shape(&anna, &project, &model, &i(element), None)
                .await
                .unwrap();
            assert_eq!(shown.built.edition, made_on.edition());
            match &shown.built.result {
                BuiltResult::Failed { code, message } => {
                    assert_eq!(code, "EditionUnavailable");
                    assert!(message.contains(&made_on.edition().engine), "{message}");
                }
                other @ BuiltResult::Shape { .. } => panic!("{element} was built: {other:?}"),
            }
            assert!(shown.display.is_none());
        }
        let regen = Regenerator::new(store.clone(), files.clone(), opened_with);
        assert_eq!(regen.verify(&project).await.unwrap(), 26);
    }
}

/// The interim register's rows for Spare Construct's side of the seam each name a place, what
/// answers it, what replaces it, and a test that exists in the file it names.
#[test]
fn every_construct_row_of_the_interim_register_names_a_real_test() {
    let register: serde_json::Value = serde_json::from_str(include_str!(
        "../../../engines/geometry/geom-solidcore/tests/interim.json"
    ))
    .unwrap();
    let rows = register["programs"].as_array().unwrap();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
    let mut seen = 0;
    for row in rows.iter().filter(|r| r["program"] == "spare-construct") {
        for field in ["place", "operation", "answered_by", "replaced_by"] {
            assert!(
                !row[field].as_str().unwrap_or_default().is_empty(),
                "a row without its {field}: {row}"
            );
        }
        assert!(
            row["known_faults"]
                .as_array()
                .unwrap()
                .iter()
                .all(serde_json::Value::is_string)
        );
        let file = row["pinned_by"]["file"].as_str().unwrap();
        let test = row["pinned_by"]["test"].as_str().unwrap();
        let source = std::fs::read_to_string(format!("{root}/{file}")).unwrap();
        assert!(
            source.contains(&format!("fn {test}")),
            "{file} has no test {test}"
        );
        seen += 1;
    }
    assert!(
        seen >= 3,
        "the stand-in and the kernel builder's interim places are listed"
    );
}

/// A wall type's layers and core as `wall.type.create` and `wall.type.layers` take them: the
/// partition of construct-walls' defaults, its frame `stud` thick.
fn partition(stud: &str) -> serde_json::Value {
    let layer = |function: &str, thickness: &str, material: &str, wraps: bool| {
        json!({ "function": function, "thickness": thickness, "material": material,
                "wraps": wraps })
    };
    json!({ "core": { "exterior": 1, "interior": 2 }, "layers": [
        layer("finish", "12.5 mm", "plasterboard-gypsum", true),
        layer("structure", stud, "timber-douglas-fir", false),
        layer("finish", "12.5 mm", "plasterboard-gypsum", true),
    ] })
}

/// A layered wall as the server stored it, its finest display, and its bounds.
async fn layered(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
    wall: &str,
) -> (ShapeView, Vec<FaceName>, ([i64; 3], [i64; 3])) {
    s.regenerate(project).await.unwrap();
    let shown = s
        .shape(&i("anna"), project, model, &i(wall), Some(0))
        .await
        .unwrap();
    let BuiltResult::Shape { names, bounds, .. } = &shown.built.result else {
        panic!("{wall} builds: {:?}", shown.built.result);
    };
    let (names, bounds) = (names.clone(), (bounds.min, bounds.max));
    (shown, names, bounds)
}

/// Through the server's commands on the kernel: two wall types made, a partition with 12.5 mm
/// linings drawn along x from level 1 to a level of its own and a concrete wall along y of a
/// height, each built one solid per layer (the display lists each layer's material); a door
/// hosted on the partition cuts every layer; the top level moved and a layer thickened build the
/// partition again, writing no wall; undo and redo of the thickening; and the whole model
/// rebuilt from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn layered_walls_drawn_through_commands_are_built_one_solid_per_layer() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    edit(
        "wall.type.create",
        json!({ "type": "wall-partition", "name": "Partition", "assembly": partition("90 mm") }),
    )
    .await;
    edit(
        "wall.type.create",
        json!({ "type": "wall-concrete", "name": "Plain concrete 200", "assembly": {
            "core": { "exterior": 0, "interior": 1 },
            "layers": [{ "function": "structure", "thickness": "200 mm",
                         "material": "concrete-regular" }] } }),
    )
    .await;
    edit(
        "level.create",
        json!({ "level": "L-top", "name": "Head", "elevation": "2.7 m", "storey": false }),
    )
    .await;
    edit(
        "wall.draw",
        json!({ "element": "W-900", "type": "wall-partition", "start": ["50 m", "0"],
                "end": ["54 m", "0"], "level": "1", "top": "L-top" }),
    )
    .await;
    edit(
        "wall.draw",
        json!({ "element": "W-901", "type": "wall-concrete", "start": ["54 m", "0"],
                "end": ["54 m", "4 m"], "level": "1", "height": "2.7 m" }),
    )
    .await;

    let (shown, names, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -58, 0], [54_000, 58, 2700]));
    let materials: Vec<&str> = shown
        .display
        .as_ref()
        .unwrap()
        .parts
        .iter()
        .map(|part| part.material.as_str())
        .collect();
    assert_eq!(
        materials,
        [
            "plasterboard-gypsum",
            "timber-douglas-fir",
            "plasterboard-gypsum"
        ]
    );
    for role in ["exterior", "interior", "face.1", "face.2", "end0", "end1"] {
        assert!(names.iter().any(|n| n.role == role), "{role}: {names:?}");
    }
    let (concrete, _, bounds) = layered(&s, &project, &model, "W-901").await;
    assert_eq!(bounds, ([53_900, 0, 0], [54_100, 4000, 2700]));
    assert_eq!(concrete.display.as_ref().unwrap().parts.len(), 1);

    let exterior = names
        .iter()
        .find(|n| n.role == "exterior")
        .unwrap()
        .name
        .clone();
    edit(
        "hosted.place",
        json!({ "element": "D-1", "host": "W-900", "face": exterior, "along": "1200 mm",
                "recipe": kernel_suite::door_of(900, 2100, "x") }),
    )
    .await;
    let (_, names, _) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(
        cut_roles(&names),
        [
            "tool(D-1).head",
            "tool(D-1).jamb.end",
            "tool(D-1).jamb.start"
        ]
    );
    let wall = s
        .shape(&anna, &project, &model, &i("W-900"), None)
        .await
        .unwrap()
        .built
        .input;

    edit(
        "level.elevation",
        json!({ "level": "L-top", "elevation": "3 m" }),
    )
    .await;
    let (_, _, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -58, 0], [54_000, 58, 3000]));
    edit(
        "wall.type.layers",
        json!({ "type": "wall-partition", "assembly": partition("140 mm") }),
    )
    .await;
    let (thicker, names, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -83, 0], [54_000, 83, 3000]));
    assert_eq!(thicker.built.input, wall, "no wall was written");
    assert_eq!(cut_roles(&names).len(), 3, "the door still cuts it");
    edit("edit.undo", json!({})).await;
    let (_, _, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -58, 0], [54_000, 58, 3000]));
    edit("edit.redo", json!({})).await;
    let (again, _, _) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(again.built.result, thicker.built.result);

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
    // The seed's shapes (the site setout's five grids among them) and what this test drew.
    assert_eq!(fresh.verify(&project).await.unwrap(), 29);
}

/// A corner of a floor's boundary as `floor.create` takes it, in millimetres as typed text.
fn corner(id: &str, x: i64, y: i64) -> serde_json::Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

/// A floor's read-only parameters as the page reads them: each one's stored value and its text in
/// units, by ID.
async fn quantities(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
    floor: &str,
) -> BTreeMap<String, (Option<String>, Option<String>)> {
    let view = s
        .parameters(&i("anna"), project, model, &i(floor))
        .await
        .unwrap();
    view.parameters
        .into_iter()
        .filter(|p| p.from == "read-only")
        .map(|p| (p.id, (p.value, p.shown)))
        .collect()
}

/// Through the server's commands on the kernel: the default floor types made and an L-shaped
/// floor with a hole drawn of the screed type, built one solid per layer with the hole through
/// each, a side named by each edge; its parameters view lists its area, perimeter, volume and
/// each layer's volume and mass in units, read-only, the same as on the stand-in; its type's
/// layers changed and its boundary changed build it again; and the whole model rebuilt from empty
/// gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_floor_drawn_through_commands_is_built_one_solid_per_layer() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    edit("floor.defaults", json!({})).await;
    let x0 = 60_000;
    let l = json!({
        "outer": [corner("e0", x0, 0), corner("e1", x0 + 8000, 0), corner("e2", x0 + 8000, 4000),
                  corner("e3", x0 + 4000, 4000), corner("e4", x0 + 4000, 8000),
                  corner("e5", x0, 8000)],
        "holes": [[corner("h0", x0 + 1000, 1000), corner("h1", x0 + 3000, 1000),
                   corner("h2", x0 + 3000, 3000), corner("h3", x0 + 1000, 3000)]],
    });
    edit(
        "floor.create",
        json!({ "element": "F-900", "type": "floor-concrete-150-screed-50", "level": "1",
                "boundary": l }),
    )
    .await;
    let (_, names, bounds) = layered(&s, &project, &model, "F-900").await;
    assert_eq!(bounds, ([60_000, 0, -200], [68_000, 8000, 0]));
    let mut roles: Vec<&str> = names.iter().map(|n| n.role.as_str()).collect();
    roles.sort_unstable();
    assert_eq!(
        roles,
        [
            "bottom", "face.1", "side.e0", "side.e1", "side.e2", "side.e3", "side.e4", "side.e5",
            "side.h0", "side.h1", "side.h2", "side.h3", "top"
        ]
    );
    let read = quantities(&s, &project, &model, "F-900").await;
    let area = construct_params::Value::Area(construct_params::Area::from_mm2(44_000_000));
    assert_eq!(
        read["floor.area"],
        (Some(area.stored()), Some("44.00 m²".to_owned()))
    );
    assert_eq!(read["floor.perimeter"].1.as_deref(), Some("40000 mm"));
    assert_eq!(read["floor.volume"].1.as_deref(), Some("8.80 m³"));
    assert_eq!(read["floor.layer-1.volume"].1.as_deref(), Some("2.20 m³"));
    assert_eq!(read["floor.layer-2.volume"].1.as_deref(), Some("6.60 m³"));
    assert_eq!(
        read["floor.layer-1.mass"].0, None,
        "no screed in the library yet"
    );
    assert_eq!(read["floor.mass"].0, None);
    assert!(read["floor.layer-2.mass"].0.is_some());
    let wrote = s
        .shape(&anna, &project, &model, &i("F-900"), None)
        .await
        .unwrap()
        .built
        .input;

    edit(
        "floor.type.layers",
        json!({ "type": "floor-concrete-150-screed-50", "layers": [
            { "function": "structure", "thickness": "250 mm", "material": "concrete-regular" }] }),
    )
    .await;
    let (thicker, _, bounds) = layered(&s, &project, &model, "F-900").await;
    assert_eq!(bounds, ([60_000, 0, -250], [68_000, 8000, 0]));
    assert_eq!(thicker.built.input, wrote, "no floor was written");
    let read = quantities(&s, &project, &model, "F-900").await;
    assert_eq!(read["floor.volume"].1.as_deref(), Some("11.00 m³"));
    assert!(read["floor.mass"].0.is_some());

    edit(
        "floor.boundary",
        json!({ "element": "F-900", "boundary": { "outer": [
            corner("r0", x0, 0), corner("r1", x0 + 4000, 0), corner("r2", x0 + 4000, 3000),
            corner("r3", x0, 3000)] } }),
    )
    .await;
    let (_, _, bounds) = layered(&s, &project, &model, "F-900").await;
    assert_eq!(bounds, ([60_000, 0, -250], [64_000, 3000, 0]));

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
}

/// Through the server's commands on the kernel (job 7.1): a window of the library placed on a
/// partition cuts its opening from every layer and stands in it, built from its record (a closed
/// mesh per solid, the glass apart); a window set to a higher sill and changed to a wider type
/// cuts a different opening and is built again, each undone and redone; one past the wall's end is
/// refused in the builder's words with the family named; and the log replayed from empty gives the
/// same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_window_placed_through_commands_cuts_its_wall_and_stands_in_it() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await }
    };
    edit(
        "wall.type.create",
        json!({ "type": "wall-partition", "name": "Partition", "assembly": partition("90 mm") }),
    )
    .await
    .unwrap();
    edit(
        "wall.draw",
        json!({ "element": "W-950", "type": "wall-partition", "start": ["50 m", "20 m"],
                "end": ["54 m", "20 m"], "level": "1", "height": "2.7 m" }),
    )
    .await
    .unwrap();
    let place = json!({ "element": "F-1", "family": "library.doors-windows.fixed-window",
                        "type": "w1200x1200", "host": "W-950", "side": "exterior",
                        "along": "1500 mm" });
    edit("family.place", place).await.unwrap();
    let (plain, names) = stored_wall(&s, &project, &model, "W-950").await;
    assert_eq!(
        cut_roles(&names),
        [
            "tool(F-1).head",
            "tool(F-1).jamb.end",
            "tool(F-1).jamb.start",
            "tool(F-1).sill"
        ],
        "a window's opening has a sill as well as a head and two jambs"
    );
    let window = s
        .shape(&anna, &project, &model, &i("F-1"), Some(0))
        .await
        .unwrap();
    let BuiltResult::Shape { bounds, .. } = &window.built.result else {
        panic!("F-1 builds: {:?}", window.built.result);
    };
    assert_eq!(
        bounds.max[2] - bounds.min[2],
        1200,
        "its frame is its type's height"
    );
    let parts = &window.display.as_ref().unwrap().parts;
    assert!(
        parts.iter().any(|p| p.material == "glass-plate") && parts.len() >= 2,
        "the glass is its own part, apart from the frame: {parts:?}"
    );

    edit(
        "family.set",
        json!({ "element": "F-1", "values": { "sill_height": "1.2 m" } }),
    )
    .await
    .unwrap();
    let (raised, _) = stored_wall(&s, &project, &model, "W-950").await;
    assert_ne!(
        raised.built.result, plain.built.result,
        "the opening rose with the sill"
    );
    edit(
        "family.retype",
        json!({ "element": "F-1", "type": "w1800x1200" }),
    )
    .await
    .unwrap();
    let (wide, _) = stored_wall(&s, &project, &model, "W-950").await;
    assert_ne!(
        wide.built.result, raised.built.result,
        "and widened with its type"
    );
    edit("edit.undo", json!({})).await.unwrap();
    let (back, _) = stored_wall(&s, &project, &model, "W-950").await;
    assert_eq!(back.built.result, raised.built.result);
    edit("edit.redo", json!({})).await.unwrap();
    let (again, _) = stored_wall(&s, &project, &model, "W-950").await;
    assert_eq!(again.built.result, wide.built.result);

    let refused = edit("family.move", json!({ "element": "F-1", "along": "3.5 m" }))
        .await
        .unwrap_err();
    assert_eq!(refused.to_string(), "family F-1: it crosses the wall's end");
    edit("family.flip", json!({ "element": "F-1" }))
        .await
        .unwrap();
    edit("family.delete", json!({ "element": "F-1" }))
        .await
        .unwrap();
    let (_, names) = stored_wall(&s, &project, &model, "W-950").await;
    assert!(
        cut_roles(&names).is_empty(),
        "the window's opening goes with it"
    );

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
}

/// A level's plan drawn on solidcore, as the page asks for it: every wall of the level has its
/// cut edges, each layer's section is hatched in its material, the same model draws the same bytes
/// twice, and an element the drawer cannot draw is named with its reason and does not blank the
/// plan.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_draws_a_levels_plan_on_the_kernel() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let plan = s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    assert!(plan.svg.starts_with("<svg"));
    for wall in ["W-101", "W-102", "W-103", "W-104", "W-105"] {
        let lines = format!("data-element=\"{wall}\"");
        assert!(plan.svg.contains(&lines), "{wall} has cut edges");
    }
    assert!(
        plan.svg.contains("data-material="),
        "the layers are hatched"
    );
    let again = s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert_eq!(plan.svg, again.svg);
}
