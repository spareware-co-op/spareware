//! The kernel builder on solidcore's engine, the composition the server's `kernel` feature links:
//! the builder's suite (construct-regen's `tests/kernel_suite`) on `SolidcoreEngine`, its doors
//! cut from their walls (one door, a raised opening, a wall with two doors, the cases at the edge
//! and what the engine does with a face within its tolerance) among them, the whole
//! server seeded and checked on it, a hosted door keeping its face when its wall is lengthened, and
//! a document of one edition reported, not rebuilt, by a server of another. Built and run only
//! with `--features kernel`.

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

/// The whole seed runs on the kernel and every stored shape matches its recipe. A door hosted on
/// a wall's exterior names the face the suite recorded, and when the wall is lengthened and built
/// again, its exterior keeps that name, so the door's reference still lands, and the wall is built
/// with the door's opening cut.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_runs_on_the_kernel_and_a_hosted_door_keeps_its_face() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let regen = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    assert_eq!(regen.verify(&project).await.unwrap(), 26);

    let wall = s
        .shape(&anna, &project, &model, &i("W-101"), None)
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
        "placement": { "x": 1000, "y": 0, "z": 0 },
        "references": [{ "doc": null, "element": "W-101", "name": face.name, "kind": "face",
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
        json!({ "element": "W-101", "recipe": longer }),
    )
    .await
    .unwrap();
    s.regenerate(&project).await.unwrap();
    let again = s
        .shape(&anna, &project, &model, &i("W-101"), None)
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
