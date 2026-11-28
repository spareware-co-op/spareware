//! The kernel builder on solidcore's engine, the composition the server's `kernel` feature links:
//! the builder's suite (construct-regen's `tests/kernel_suite`) on `SolidcoreEngine`, its doors
//! cut from their walls (one door, a raised opening, a wall with two doors, the cases at the edge
//! and what the engine does with a face within its tolerance) among them, the whole
//! server seeded and checked on it, a hosted door keeping its face when its wall is lengthened,
//! layered walls drawn and their types changed through the commands, a floor with a hole drawn
//! through the commands and its quantities read back, a sloped floor and one with slab edges built
//! one solid per layer (job 5.2), gable, mono-pitch and flat roofs built one
//! solid per layer (each sloped layer lofted), a door and a window placed from the family library
//! cutting a three-layer wall (job 6.4), a stair's run lofted and its landing extruded (job 6.7),
//! a family the document holds and no library does built and reloaded (job 7.6), doors, windows
//! and openings in walls at any angle (piece A83), and a document of one edition reported,
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
fn a_sloped_floor_is_its_layers() {
    kernel_suite::a_sloped_floor_is_its_layers(engine);
}

#[test]
fn a_shape_edited_floor_is_its_triangles() {
    kernel_suite::a_shape_edited_floor_is_its_triangles(engine);
}

#[test]
fn a_floor_with_slab_edges_is_one_solid() {
    kernel_suite::a_floor_with_slab_edges_is_one_solid(engine);
}

#[test]
fn a_gable_roof_is_its_layers() {
    kernel_suite::a_gable_roof_is_its_layers(engine);
}

#[test]
fn a_mono_roof_is_its_layers() {
    kernel_suite::a_mono_roof_is_its_layers(engine);
}

#[test]
fn a_flat_roof_is_its_layers() {
    kernel_suite::a_flat_roof_is_its_layers(engine);
}

#[test]
fn a_stair_is_its_pieces() {
    kernel_suite::a_stair_is_its_pieces(engine);
}

#[test]
fn a_room_with_an_island_wall_is_its_loop_extruded() {
    kernel_suite::a_room_is_its_loop_extruded(engine, false);
    kernel_suite::a_room_is_its_loop_extruded(engine, true);
}

#[test]
fn a_hip_roof_is_its_pieces_united() {
    kernel_suite::a_hip_roof_is_its_pieces_united(engine);
}

#[test]
fn an_l_roof_with_its_valley_is_its_pieces_united() {
    kernel_suite::an_l_roof_with_its_valley_is_its_pieces_united(engine);
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
fn a_shaped_floor_that_is_cut_is_refused_by_name() {
    kernel_suite::a_shaped_floor_that_is_cut_is_refused_by_name(engine);
}

#[test]
fn walls_joined_at_an_l_and_a_t_fill_the_joined_shape() {
    kernel_suite::walls_joined_at_an_l_and_a_t_fill_the_joined_shape(engine);
}

#[test]
fn joined_walls_meet_without_overlap_on_the_engine() {
    kernel_suite::joined_walls_meet_without_overlap_on_the_engine(engine);
}

#[test]
fn a_family_cuts_every_layer_and_stands_in_its_opening() {
    kernel_suite::a_family_cuts_every_layer_and_stands_in_its_opening(engine);
}

/// Piece A83: a door, a window, a plain opening and a placed door in walls at 30, 45 and 60
/// degrees and at a 3:4:5 slope, every wall, opening and family solid made by solidcore's engine
/// and passed by the kernel's checker.
#[test]
fn elements_at_any_angle_cut_their_wall_and_stand_in_it() {
    kernel_suite::elements_at_any_angle_cut_their_wall_and_stand_in_it(engine);
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
/// height from its end, each built one solid per layer (the display lists each layer's material)
/// and joined at the corner (the partition's exterior lining runs to the concrete's outer face,
/// the concrete from the partition's stud's exterior face); a door
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
    assert_eq!(bounds, ([50_000, -58, 0], [54_100, 58, 2700]));
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
    assert_eq!(bounds, ([53_900, -45, 0], [54_100, 4000, 2700]));
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
    assert_eq!(bounds, ([50_000, -58, 0], [54_100, 58, 3000]));
    edit(
        "wall.type.layers",
        json!({ "type": "wall-partition", "assembly": partition("140 mm") }),
    )
    .await;
    let (thicker, names, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -83, 0], [54_100, 83, 3000]));
    assert_eq!(thicker.built.input, wall, "no wall was written");
    assert_eq!(cut_roles(&names).len(), 3, "the door still cuts it");
    edit("edit.undo", json!({})).await;
    let (_, _, bounds) = layered(&s, &project, &model, "W-900").await;
    assert_eq!(bounds, ([50_000, -58, 0], [54_100, 58, 3000]));
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

/// Six times the volume a shape's finest display holds, part by part, each part closed, in cubic
/// millimetres.
fn parts_six(shown: &ShapeView) -> i128 {
    let display = shown.display.as_ref().unwrap();
    (0..display.parts.len())
        .map(|at| kernel_suite::closed_mesh_six_volume(&kernel_suite::part_mesh(display, at)))
        .sum()
}

/// Through the server's commands on the kernel: three walls of a lined type (20, 100 and 20 mm)
/// drawn to make an L and a T are each built with their joints, and their pieces' volumes add up
/// exactly to the joined shape's, so they neither overlap nor leave a gap; moving the corner wall
/// away builds the first again with a square end, its record unwritten; the whole model rebuilt
/// from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn walls_drawn_to_meet_through_commands_are_joined_on_the_kernel() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let layer = |function: &str, thickness: &str, material: &str| json!({ "function": function, "thickness": thickness, "material": material });
    edit(
        "wall.type.create",
        json!({ "type": "wall-lined", "name": "Lined", "assembly": {
            "core": { "exterior": 1, "interior": 2 }, "layers": [
                layer("finish", "20 mm", "plasterboard-gypsum"),
                layer("structure", "100 mm", "timber-douglas-fir"),
                layer("finish", "20 mm", "plasterboard-gypsum"),
            ] } }),
    )
    .await;
    for (id, start, end) in [
        ("W-910", ["60 m", "-20 m"], ["64 m", "-20 m"]),
        ("W-911", ["64 m", "-20 m"], ["64 m", "-17 m"]),
        ("W-912", ["62 m", "-23 m"], ["62 m", "-20 m"]),
    ] {
        edit(
            "wall.draw",
            json!({ "element": id, "type": "wall-lined", "start": start, "end": end,
                    "level": "1", "height": "2.7 m" }),
        )
        .await;
    }
    let (first, _, bounds) = layered(&s, &project, &model, "W-910").await;
    assert_eq!(bounds, ([60_000, -20_070, 0], [64_070, -19_930, 2700]));
    assert_eq!(
        first.display.as_ref().unwrap().parts.len(),
        4,
        "its exterior lining is in two pieces"
    );
    let mut six = parts_six(&first);
    for wall in ["W-911", "W-912"] {
        six += parts_six(&layered(&s, &project, &model, wall).await.0);
    }
    let area = 4070 * 140 + 140 * 2930 + 140 * 2930;
    assert_eq!(six, 6 * area * 2700, "the walls fill the joined shape");

    edit(
        "element.move",
        json!({ "element": "W-911", "placement": { "x": 1000, "y": 0, "z": 0 } }),
    )
    .await;
    let (moved, _, bounds) = layered(&s, &project, &model, "W-910").await;
    assert_eq!(bounds, ([60_000, -20_070, 0], [64_000, -19_930, 2700]));
    assert_eq!(moved.built.input, first.built.input, "no wall was written");

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
    assert_eq!(fresh.verify(&project).await.unwrap(), 29);
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

/// Piece A83: a plan draws a door hosted on a wall at an angle seated in its wall, as its shape is
/// built: its cut edges run along the wall (a 3:4:5 slope, so neither along x nor along y), and
/// nothing is left out of the plan.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_draws_a_door_in_a_wall_at_an_angle_seated_in_its_wall() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
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
        "wall.draw",
        json!({ "element": "W-A83", "type": "wall-partition", "start": ["50 m", "0"],
                "end": ["53 m", "4 m"], "level": "1", "height": "2.7 m" }),
    )
    .await;
    let (_, names, _) = layered(&s, &project, &model, "W-A83").await;
    let exterior = names
        .iter()
        .find(|n| n.role == "exterior")
        .unwrap()
        .name
        .clone();
    edit(
        "hosted.place",
        json!({ "element": "D-A83", "host": "W-A83", "face": exterior, "along": "1200 mm",
                "recipe": kernel_suite::door_of(900, 2100, "x") }),
    )
    .await;
    s.regenerate(&project).await.unwrap();
    let plan = s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    let number = |line: &str, name: &str| -> f64 {
        let at = line.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
        line[at..].split('"').next().unwrap().parse().unwrap()
    };
    let door: Vec<&str> = plan
        .svg
        .split("<line ")
        .filter(|line| line.contains("data-element=\"D-A83\""))
        .collect();
    assert!(!door.is_empty(), "the door's cut edges are drawn");
    for line in door {
        let (dx, dy) = (
            number(line, "x2") - number(line, "x1"),
            number(line, "y2") - number(line, "y1"),
        );
        // The drawing's y runs up the sheet, so the wall runs along (3, -4) in it: an edge is along
        // it or square to it (4, 3), each within the nearest-step rounding.
        let (along, square) = (dx * 4.0 + dy * 3.0, dx * 3.0 - dy * 4.0);
        assert!(
            along.abs() < 0.1 || square.abs() < 0.1,
            "an edge of the door runs ({dx}, {dy}), along neither the wall nor across it"
        );
    }
}

/// A view's graphics rules reach the drawing on the kernel: with no rule the plan is the bytes it
/// was; a heavier pen, another fill and colour, and halftone for what walls cut change those lines
/// and hatches and nothing else; a hidden category leaves its edges and hatches out (the walls', not
/// the doors'); clearing the
/// rule gives the first bytes back, and undo and redo each go there and back.
#[tokio::test(flavor = "multi_thread")]
async fn a_views_graphics_rules_change_the_plan_it_draws() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let draw = || async {
        s.plan(&anna, &project, &model, &i("VP-1"))
            .await
            .unwrap()
            .svg
    };
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let set = |style: serde_json::Value| json!({ "view": "VP-1", "category": 100, "edge": "cut", "style": style });
    let plain = draw().await;
    assert!(plain.contains("data-material="));
    assert!(!plain.contains("opacity"));

    let heavy = json!({ "pen": 6, "colour": [200, 16, 32],
                        "fill": { "pattern": "earth", "colour": [10, 90, 20] },
                        "halftone": true });
    edit("view.graphics.set", set(heavy)).await;
    let ruled = draw().await;
    assert_ne!(ruled, plain);
    assert!(ruled.contains("stroke=\"#c81020\""), "the rule's colour");
    assert!(
        ruled.contains("stroke=\"#0a5a14\""),
        "the rule's fill colour"
    );
    assert!(ruled.contains("opacity=\"0.5\""), "halftone");
    assert_eq!(ruled, draw().await, "the same rule draws the same bytes");

    edit("view.graphics.set", set(json!({ "shown": false }))).await;
    let hidden = draw().await;
    for wall in ["W-101", "W-102", "W-103", "W-104", "W-105"] {
        assert!(
            !hidden.contains(&format!("data-element=\"{wall}\"")),
            "{wall}"
        );
    }
    assert!(hidden.contains("data-element=\"D-101\""), "the doors stay");
    assert!(hidden.matches("data-material=").count() < plain.matches("data-material=").count());

    edit(
        "view.graphics.clear",
        json!({ "view": "VP-1", "category": 100 }),
    )
    .await;
    assert_eq!(draw().await, plain, "clearing gives the first drawing back");
    edit("edit.undo", json!({})).await;
    assert_eq!(draw().await, hidden);
    edit("edit.undo", json!({})).await;
    assert_eq!(draw().await, ruled);
    edit("edit.redo", json!({})).await;
    edit("edit.redo", json!({})).await;
    assert_eq!(draw().await, plain);
}

/// Through the server's commands on the kernel: the default roof types made and a gable roof over
/// a rectangle drawn of the insulated concrete type, built one solid per layer; its parameters
/// view lists its area, ridge height and each layer's volume in units, read-only, the same as on
/// the stand-in; its pitch changed builds it again, writing no roof; and the whole model rebuilt
/// from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_roof_drawn_through_commands_is_built_one_solid_per_layer() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    edit("roof.defaults", json!({})).await;
    let x0 = 60_000;
    edit(
        "roof.create",
        json!({ "element": "R-900", "type": "roof-concrete-200-insulated", "level": "1",
                "form": "gable", "pitch": "1:4",
                "footprint": { "outer": [corner("r0", x0, 0), corner("r1", x0 + 8000, 0),
                                         corner("r2", x0 + 8000, 6000), corner("r3", x0, 6000)] } }),
    )
    .await;
    let (shown, names, bounds) = layered(&s, &project, &model, "R-900").await;
    assert_eq!(
        ([bounds.0[0], bounds.0[1]], [bounds.1[0], bounds.1[1]]),
        ([x0, 0], [x0 + 8000, 6000])
    );
    assert_eq!(bounds.0[2], 0);
    assert!(bounds.1[2] > 1000, "the ridge rises: {bounds:?}");
    assert!(
        names.iter().any(|n| n.role.starts_with("top.")),
        "{names:?}"
    );
    let wrote = shown.built.input;
    let read = quantities(&s, &project, &model, "R-900").await;
    assert_eq!(read["roof.area"].1.as_deref(), Some("48.00 m²"));
    assert_eq!(read["roof.ridge-height"].1.as_deref(), Some("1059 mm"));
    assert!(read["roof.layer-1.volume"].0.is_some() && read["roof.layer-2.volume"].0.is_some());

    edit("roof.pitch", json!({ "element": "R-900", "pitch": "1:2" })).await;
    let (steeper, _, bounds) = layered(&s, &project, &model, "R-900").await;
    assert!(bounds.1[2] > 1500, "{bounds:?}");
    assert_ne!(steeper.built.input, wrote, "the roof's record was written");
    assert_ne!(
        quantities(&s, &project, &model, "R-900").await["roof.ridge-height"],
        read["roof.ridge-height"]
    );

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
}

/// Through the server's commands on the kernel (job 6.7): a common stairway from level 1 to level 2
/// (3000 mm) in two runs of 8 risers of 187.5 mm, an L on a 1000 mm landing, built one solid per
/// piece (each run's profile lofted across its width, reaching under the landing; the landing
/// extruded), each a part of the display, named by its pieces' faces; turned into a U and widened
/// it is built again; its quantities read back as on the stand-in; and the whole model rebuilt
/// from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_stair_made_through_commands_is_built_one_solid_per_piece() {
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
        "stair.create",
        json!({ "element": "S-900", "level": "1", "top": "2", "start": ["60 m", "0"],
                "runs": [{}, {}], "landings": [{ "length": "1 m", "turn": "left" }] }),
    )
    .await;
    let (shown, names, bounds) = layered(&s, &project, &model, "S-900").await;
    // 7 treads of 280 mm, the landing to 2960 mm, the second run from 1000 mm to 2960 mm up +y;
    // the second run's last inner corner is 3000 - 187.5 mm up.
    assert_eq!(bounds, ([60_000, 0, 0], [62_960, 2960, 2813]));
    let parts = &shown.display.as_ref().unwrap().parts;
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|p| p.material == "concrete-regular"));
    for role in [
        "run.1.tread.7",
        "run.2.riser.1",
        "landing.1.top",
        "run.2.underside",
    ] {
        assert!(names.iter().any(|n| n.role == role), "{role}: {names:?}");
    }
    let read = quantities(&s, &project, &model, "S-900").await;
    assert_eq!(read["stair.risers"].1.as_deref(), Some("16"));
    assert_eq!(
        read["stair.riser-height"].0.as_deref(),
        Some("length:q0:1200000")
    );

    edit(
        "stair.runs",
        json!({ "element": "S-900", "runs": [{}, {}],
                "landings": [{ "length": "1200 mm", "turn": "back-right" }] }),
    )
    .await;
    edit(
        "stair.width",
        json!({ "element": "S-900", "width": "1100 mm" }),
    )
    .await;
    let (_, _, bounds) = layered(&s, &project, &model, "S-900").await;
    assert_eq!((bounds.0[1], bounds.1[1]), (-1100, 1100), "{bounds:?}");

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

/// The warnings view on the kernel engine: the list is read from the document, so it is the same
/// as on the default engine. A room whose wall is deleted is listed not enclosed; two walls drawn
/// along each other are listed as an overlap, and the list is empty of both again when the
/// second wall is deleted and the first wall of the room is back by undo.
#[tokio::test(flavor = "multi_thread")]
async fn the_warnings_view_lists_a_room_not_enclosed_and_walls_that_overlap() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel.clone());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let identities = || async {
        s.warnings(&anna, &project, &model)
            .await
            .unwrap()
            .warnings
            .into_iter()
            .map(|w| w.identity)
            .collect::<Vec<_>>()
    };
    let base = identities().await;
    assert!(base.iter().all(|w| !w.starts_with("room.")), "{base:?}");
    edit(
        "room.place",
        json!({ "element": "R-900", "name": "Studio", "level": "1", "at": ["3 m", "4 m"],
                "top": "2" }),
    )
    .await;
    assert_eq!(identities().await, base, "enclosed: nothing to say");
    edit("element.delete", json!({ "element": "W-103" })).await;
    let open = identities().await;
    assert_eq!(open.len(), base.len() + 1);
    assert!(
        open.contains(&"room.not_enclosed|R-900".to_owned()),
        "{open:?}"
    );

    edit(
        "wall.type.create",
        json!({ "type": "wall-partition", "name": "Partition", "assembly": partition("90 mm") }),
    )
    .await;
    for (wall, from, to) in [("W-900", "50 m", "54 m"), ("W-901", "52 m", "56 m")] {
        edit(
            "wall.draw",
            json!({ "element": wall, "type": "wall-partition", "start": [from, "0"],
                    "end": [to, "0"], "level": "1", "height": "2.7 m" }),
        )
        .await;
    }
    let both = identities().await;
    assert!(
        both.contains(&"wall.overlap|W-900|W-901".to_owned()),
        "{both:?}"
    );
    assert!(both.contains(&"room.not_enclosed|R-900".to_owned()));
    edit("element.delete", json!({ "element": "W-901" })).await;
    assert_eq!(identities().await, open);
    for _ in 0..8 {
        edit("edit.undo", json!({})).await;
        if !identities()
            .await
            .contains(&"room.not_enclosed|R-900".to_owned())
        {
            break;
        }
    }
    assert_eq!(identities().await, base);
}

/// A plan's stored view range feeds the drawing's cut height: with the cut plane at 1200 mm the
/// plan draws the walls and the doors (2100 mm high) they hold; raised to 2500 mm it draws the
/// walls and no door; raised above the walls' tops (3000 mm) it has nothing cut to draw. The
/// default range draws the bytes it always did, undo and redo each go there and back, and a
/// crop stored on the view is the next test's.
#[tokio::test(flavor = "multi_thread")]
async fn a_views_stored_range_sets_where_the_plan_is_cut() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let draw = || async {
        s.plan(&anna, &project, &model, &i("VP-1"))
            .await
            .unwrap()
            .svg
    };
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let has = |svg: &str, element: &str| svg.contains(&format!("data-element=\"{element}\""));
    let low = draw().await;
    assert!(has(&low, "W-101") && has(&low, "D-101"));

    // Cut at 2500 mm, the top raised with it so the range stays in order.
    let range = json!({ "top": { "offset": "3.5 m" }, "cut": { "offset": "2.5 m" } });
    edit("view.set", json!({ "view": "VP-1", "range": range })).await;
    let high = draw().await;
    assert_ne!(high, low);
    assert!(has(&high, "W-101"), "a wall 3000 mm high is cut at 2500");
    assert!(!has(&high, "D-101"), "a door 2100 mm high is not");
    assert_eq!(high, draw().await, "the same range draws the same bytes");

    // Above the walls' tops nothing is cut.
    let range = json!({ "top": { "offset": "4.5 m" }, "cut": { "offset": "3.5 m" } });
    edit("view.set", json!({ "view": "VP-1", "range": range })).await;
    let above = draw().await;
    assert!(!has(&above, "W-101") && !has(&above, "D-101"));

    // The default range again.
    edit(
        "view.set",
        json!({ "view": "VP-1", "range": { "top": { "offset": "2.3 m" },
                "cut": { "offset": "1.2 m" } } }),
    )
    .await;
    assert_eq!(
        draw().await,
        low,
        "the default range draws what it always did"
    );
    // Undo and redo take the range there and back.
    edit("edit.undo", json!({})).await;
    assert_eq!(draw().await, above);
    edit("edit.redo", json!({})).await;
    assert_eq!(draw().await, low);
}

/// The x of every `x1` and `x2` of an SVG's lines and every point of its hatch paths, in
/// millimetres.
fn xs(svg: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for key in ["x1=\"", "x2=\""] {
        for part in svg.split(key).skip(1) {
            out.push(part.split('"').next().unwrap().parse().unwrap());
        }
    }
    for part in svg.split(" d=\"").skip(1) {
        let d = part.split('"').next().unwrap();
        let words: Vec<&str> = d.split_whitespace().collect();
        for w in words.windows(2) {
            if w[0] == "M" || w[0] == "L" {
                out.push(w[1].parse().unwrap());
            }
        }
    }
    out
}

/// A view's crop clips its plan exactly: while it is on, no line or hatch line is drawn beyond its
/// edge and the walls it crosses are cut at it; while it is shown, its outline is four strokes
/// with the role `crop`, apart from the drawing's own; hidden, there is no outline; off, the
/// whole plan is drawn again (with the outline if it is shown); cleared, the plan is the bytes it
/// was; and undo and redo take it there and back.
#[tokio::test(flavor = "multi_thread")]
async fn a_views_crop_clips_the_plan_and_draws_its_outline() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
    seed(&store, &s).await.unwrap();
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let draw = || async {
        s.plan(&anna, &project, &model, &i("VP-1"))
            .await
            .unwrap()
            .svg
    };
    let edit = |cmd: &'static str, args: serde_json::Value| {
        let (s, anna, project, model) = (&s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let outline = |svg: &str| svg.matches("data-role=\"crop\"").count();
    let low = draw().await;
    assert_eq!(outline(&low), 0);
    let widest = |svg: &str| xs(svg).into_iter().fold(f64::MIN, f64::max);
    assert!(widest(&low) > 6000.0, "the whole plan runs past 6 m");

    // A crop 0 to 6 m across and all the way up: its outline and nothing beyond its right edge.
    edit(
        "view.crop.set",
        json!({ "view": "VP-1", "region": { "rectangle": {
            "min": { "x": "0", "y": "-500" }, "max": { "x": "6 m", "y": "8.5 m" } } } }),
    )
    .await;
    let cropped = draw().await;
    assert_eq!(outline(&cropped), 4, "a new crop is shown");
    assert!(
        widest(&cropped) <= 6000.0,
        "nothing drawn past the crop's right edge: {}",
        widest(&cropped)
    );
    assert!(
        xs(&cropped).contains(&6000.0),
        "a wall the crop's edge crosses is cut at the edge"
    );
    assert_eq!(cropped, draw().await, "the same crop draws the same bytes");
    let elements: Vec<&str> = cropped
        .split("data-element=\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .collect();
    assert!(
        elements.contains(&"D-101"),
        "the door inside the crop is drawn: {elements:?}"
    );

    // Hidden: the same clip with no outline. Off but shown: the whole plan with the outline.
    edit("view.crop.show", json!({ "view": "VP-1", "shown": false })).await;
    let hidden = draw().await;
    assert_eq!(outline(&hidden), 0);
    assert!(widest(&hidden) <= 6000.0);
    assert_ne!(hidden, low);
    assert!(
        hidden.contains("data-element=\"D-101\""),
        "hidden: the door is drawn"
    );
    edit("view.crop.show", json!({ "view": "VP-1", "shown": true })).await;
    edit("view.crop.set", json!({ "view": "VP-1", "on": false })).await;
    let off = draw().await;
    assert_eq!(outline(&off), 4);
    assert!(widest(&off) > 6000.0, "a crop that is off clips nothing");
    assert!(
        off.contains("data-element=\"D-101\""),
        "off: the door is drawn"
    );

    // Moving the right edge in to 4 m keeps the door at 2500 mm and still cuts there.
    edit(
        "view.crop.set",
        json!({ "view": "VP-1", "on": true, "region": { "rectangle": {
            "min": { "x": "0", "y": "-500" }, "max": { "x": "4 m", "y": "8.5 m" } } } }),
    )
    .await;
    let narrow = draw().await;
    assert!(widest(&narrow) <= 4000.0, "{}", widest(&narrow));
    assert!(
        narrow.contains("data-element=\"D-101\""),
        "the door is inside 4 m"
    );
    // A shaped crop (a triangle off the axes) cuts at fractions of a millimetre and still draws
    // nothing outside it.
    edit(
        "view.crop.set",
        json!({ "view": "VP-1", "on": true, "region": { "outline": [
            { "x": "0", "y": "-500" }, { "x": "6 m", "y": "-500" }, { "x": "0", "y": "8.5 m" }] } }),
    )
    .await;
    let triangle = draw().await;
    assert_eq!(outline(&triangle), 3);
    assert!(widest(&triangle) <= 6000.0);
    assert_ne!(triangle, hidden);

    // Cleared, the plan is what it was; undo brings the crop back, redo clears it.
    edit("view.crop.clear", json!({ "view": "VP-1" })).await;
    assert_eq!(draw().await, low);
    edit("edit.undo", json!({})).await;
    assert_eq!(draw().await, triangle);
    edit("edit.redo", json!({})).await;
    assert_eq!(draw().await, low);
}

#[test]
fn a_curtain_wall_is_its_panels_and_mullions() {
    kernel_suite::a_curtain_wall_is_its_panels_and_mullions(engine);
}

#[test]
fn a_door_panel_is_its_familys_solids() {
    kernel_suite::a_door_panel_is_its_familys_solids(engine);
}

/// Through the server's commands on the kernel: a curtain wall of the default 1.5 m by 1 m type,
/// 4 m long and 2.4 m high, embedded 1 m along a 6 m partition 3 m high, cuts its own opening
/// through every layer of the partition, named by the curtain wall as a plain opening's faces are
/// named: the stud layer, whose faces are on whole millimetres, keeps exactly its volume less the
/// opening's. The curtain wall itself stands on the partition's line there, one solid per panel and
/// per mullion (12 and 31).
#[tokio::test(flavor = "multi_thread")]
async fn a_curtain_wall_embedded_through_commands_cuts_its_wall() {
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
        "wall.draw",
        json!({ "element": "W-950", "type": "wall-partition", "start": ["70 m", "0"],
                "end": ["76 m", "0"], "level": "1", "height": "3 m" }),
    )
    .await;
    edit("curtain.defaults", json!({})).await;
    edit(
        "curtain.embed",
        json!({ "element": "CW-950", "type": "curtain-1500x1000", "host": "W-950",
                "along": "1 m", "length": "4 m", "height": "2.4 m" }),
    )
    .await;
    let (shown, names, bounds) = layered(&s, &project, &model, "W-950").await;
    assert_eq!(bounds, ([70_000, -58, 0], [76_000, 58, 3000]));
    assert_eq!(
        cut_roles(&names),
        [
            "tool(CW-950).head",
            "tool(CW-950).jamb.end",
            "tool(CW-950).jamb.start"
        ]
    );
    let display = shown.display.as_ref().unwrap();
    let stud = kernel_suite::closed_mesh_six_volume(&kernel_suite::part_mesh(display, 1));
    assert_eq!(stud, 6 * (6000 * 90 * 3000 - 4000 * 90 * 2400));
    let (curtain, _, bounds) = layered(&s, &project, &model, "CW-950").await;
    assert_eq!(bounds, ([71_000, -75, 0], [75_000, 75, 2400]));
    assert_eq!(curtain.display.as_ref().unwrap().parts.len(), 12 + 31);
}

/// Through the server's commands on the kernel: the plan cuts a curtain wall one part at a time
/// (every panel and mullion its cut edges, glass hatched in its material), free and embedded alike,
/// and leaves nothing out; and the page is told what it needs of each: the grid as it is laid out
/// (12 panels and the lines between them), and for the one embedded in a partition where the
/// server seats it (its recipe with its line written in, its place) and the opening it cuts.
#[tokio::test(flavor = "multi_thread")]
async fn the_plan_cuts_a_curtain_wall_and_the_page_is_told_where_it_stands() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let kernel = geometry(Builder::Kernel).unwrap();
    let s = support::build(&store, &files, kernel).with_drawer(spare_server::geometry::drawer());
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
        "wall.draw",
        json!({ "element": "W-960", "type": "wall-partition", "start": ["90 m", "0"],
                "end": ["96 m", "0"], "level": "1", "height": "3 m" }),
    )
    .await;
    edit("curtain.defaults", json!({})).await;
    edit(
        "curtain.draw",
        json!({ "element": "CW-960", "type": "curtain-1500x1000", "level": "1",
                "start": ["80 m", "-5 m"], "end": ["86 m", "-5 m"], "height": "3 m" }),
    )
    .await;
    edit(
        "curtain.embed",
        json!({ "element": "CW-961", "type": "curtain-1500x1000", "host": "W-960",
                "along": "1 m", "length": "4 m", "height": "2.4 m" }),
    )
    .await;
    let plan = s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    for curtain in ["CW-960", "CW-961"] {
        assert!(
            plan.svg.contains(&format!("data-element=\"{curtain}\"")),
            "{curtain} has cut edges"
        );
    }
    assert!(plan.svg.contains("data-material=\"glass-plate\""));
    let free = serde_json::to_value(
        s.filled(&anna, &project, &model, &i("CW-960"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(free["curtain"]["panels"].as_array().unwrap().len(), 12);
    // 5 vertical lines and 4 horizontal ones, borders included.
    assert_eq!(free["curtain"]["lines"].as_array().unwrap().len(), 9);
    assert!(free.get("placement").is_none() && free.get("cut").is_none());
    let seated = serde_json::to_value(
        s.filled(&anna, &project, &model, &i("CW-961"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(seated["curtain"]["panels"].as_array().unwrap().len(), 12);
    assert_eq!(seated["placement"], json!({ "x": 0, "y": 0, "z": 0 }));
    assert!(seated["recipe"]["args"]["start"].is_array());
    assert_eq!(seated["cut"]["recipe"]["op"], "opening.wall");
}

/// The library's fixed window under the id `office.window` with its `w1200x1200` type `width`
/// millimetres wide: a family no library holds, as recipe text.
fn office_window(width: i64) -> String {
    let (_, window) = construct_regen::families::Library::baked()
        .family("library.doors-windows.fixed-window")
        .unwrap();
    let mut recipe = (*window).clone();
    recipe.id = i("office.window");
    "Office window".clone_into(&mut recipe.name);
    let table = recipe.type_table.as_mut().unwrap();
    let at = table
        .columns
        .iter()
        .position(|c| c.as_str() == "width")
        .unwrap();
    let row = table
        .rows
        .iter_mut()
        .find(|r| r.key.as_str() == "w1200x1200")
        .unwrap();
    row.values[at] = format!("length:q0:{}", width * 6400);
    recipe.to_json()
}

/// Through the server's commands on the kernel (job 7.6): a family the document holds and no
/// library does is placed, cuts its wall and is built from the document's record; a reload at a
/// wider version cuts a wider opening, its copy kept, and is undone and redone; the log replayed
/// from empty gives the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn a_documents_own_family_is_built_and_reloaded_on_the_kernel() {
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
        json!({ "element": "W-960", "type": "wall-partition", "start": ["60 m", "20 m"],
                "end": ["64 m", "20 m"], "level": "1", "height": "2.7 m" }),
    )
    .await
    .unwrap();
    edit("family.load", json!({ "recipe": office_window(1200) }))
        .await
        .unwrap();
    let place = json!({ "element": "F-1", "family": "office.window", "type": "w1200x1200",
                        "host": "W-960", "side": "exterior", "along": "1500 mm" });
    edit("family.place", place).await.unwrap();
    let (plain, names) = stored_wall(&s, &project, &model, "W-960").await;
    assert_eq!(cut_roles(&names).len(), 4, "the window cuts its opening");
    let window = s
        .shape(&anna, &project, &model, &i("F-1"), Some(0))
        .await
        .unwrap();
    let BuiltResult::Shape { bounds, .. } = &window.built.result else {
        panic!("F-1 builds from the document: {:?}", window.built.result);
    };
    assert_eq!(bounds.max[2] - bounds.min[2], 1200);

    edit(
        "family.reload",
        json!({ "family": "office.window", "recipe": office_window(1500) }),
    )
    .await
    .unwrap();
    let (wide, _) = stored_wall(&s, &project, &model, "W-960").await;
    assert_ne!(
        wide.built.result, plain.built.result,
        "the opening widened with the new version"
    );
    edit("edit.undo", json!({})).await.unwrap();
    let (back, _) = stored_wall(&s, &project, &model, "W-960").await;
    assert_eq!(back.built.result, plain.built.result);
    edit("edit.redo", json!({})).await.unwrap();
    let (again, _) = stored_wall(&s, &project, &model, "W-960").await;
    assert_eq!(again.built.result, wide.built.result);

    let before = derived(&store, &project).await;
    let fresh = Regenerator::new(store.clone(), files.clone(), kernel.clone());
    fresh.rebuild(&project).await.unwrap();
    assert_eq!(derived(&store, &project).await, before);
}
