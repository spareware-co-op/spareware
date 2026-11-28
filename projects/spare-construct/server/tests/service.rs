//! Every rule a person meets, in-process on the test store, so these run without TiKV. The seed
//! data goes in first through the real doors, then each test tries the doors it shouldn't be able
//! to open. `tests/checklist.rs` runs the same story on the real stack.

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

mod support;

use construct_api::cloud::spaces::folders;
use construct_model::rows::{BuiltResult, LinkKind};
use construct_params::Length;
use construct_styles::Scale;
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{FileStore, Kv, Scope};
use kit_sync::{read_log, verify_log};
use serde_json::{Value, json};
use spare_server::backup;
use spare_server::plan::{Drawer, Drawn, ToDraw};
use spare_server::seed::seed;
use spare_server::service::{Refusal, Service, View};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Stack {
    store: Memory,
    files: MemoryFiles,
    service: Service<Memory, MemoryFiles>,
}

fn build(store: &Memory, files: &MemoryFiles) -> Service<Memory, MemoryFiles> {
    support::build(store, files, construct_regen::geometry())
}

fn moved(element: &str, x: i64, y: i64) -> Value {
    json!({ "element": element, "placement": { "x": x, "y": y, "z": 0 } })
}

/// Waits for the shape keeper's checkpoints to stop landing.
async fn quiet(stack: &Stack) -> String {
    let mut last = backup::digest(&stack.store, &stack.files).await.unwrap();
    loop {
        tokio::time::sleep(Duration::from_millis(1600)).await;
        let now = backup::digest(&stack.store, &stack.files).await.unwrap();
        if now == last {
            return now;
        }
        last = now;
    }
}

async fn seeded() -> Stack {
    let store = Memory::new();
    let files = MemoryFiles::default();
    let service = build(&store, &files);
    seed(&store, &service).await.unwrap();
    Stack {
        store,
        files,
        service,
    }
}

/// Level 1's plan as the views are listed: 1:100, medium, the default range on its own level
/// (it stands at 0), no crop.
fn level_1_summary() -> Value {
    json!({ "id": "VP-1", "name": "Level 1 plan", "kind": "floor_plan", "level": "1",
            "level_name": "Level 1", "scale": 100, "detail": "medium",
            "range": { "top": { "level": "1", "offset": "2300 mm" },
                       "cut": { "level": "1", "offset": "1200 mm" },
                       "bottom": { "level": "1", "offset": "0 mm" },
                       "depth": { "level": "1", "offset": "-500 mm" },
                       "heights": { "top": "2300 mm", "cut": "1200 mm", "bottom": "0 mm",
                                    "depth": "-500 mm" } },
            "crop": null, "template": null })
}

fn refused<T: std::fmt::Debug>(answer: Result<T, Refusal>) -> bool {
    answer
        .err()
        .is_some_and(|refusal| matches!(refusal, Refusal::Refused(_)))
}

fn view() -> View {
    View {
        level: i("1"),
        x0: 0,
        y0: 0,
        x1: 9000,
        y1: 9000,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_log_replays_after_the_seed() {
    let stack = seeded().await;
    let mut snapshot = stack.store.snapshot().await.unwrap();
    assert!(
        verify_log(&mut snapshot, &Scope::Directory)
            .await
            .unwrap()
            .entries
            > 20
    );
    let check = verify_log(&mut snapshot, &Scope::Project(i("example-1")))
        .await
        .unwrap();
    // The elements (the site setout's five grids among them), arch-model's three wall types and
    // its parameter table, and a floor plan for each of the two levels of the arch and interiors
    // models (the setout's grids stand on no level): 30 + 4.
    assert_eq!(check.pieces, 34);
}

#[tokio::test(flavor = "multi_thread")]
async fn seeding_twice_changes_nothing() {
    let stack = seeded().await;
    let directory = |store: &Memory| store.keys().iter().filter(|k| !k.starts_with("p:")).count();
    let before = directory(&stack.store);
    seed(&stack.store, &stack.service).await.unwrap();
    assert_eq!(directory(&stack.store), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_teams_wip_is_shut_to_everyone_else() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, project, model) = (i("dan"), i("example-1"), i("arch-model"));
    let hash = {
        let anna = i("anna");
        let area = s.area(&anna, &project, &model, &view()).await.unwrap();
        i(area.pieces.values().next().unwrap())
    };
    assert!(refused(s.area(&dan, &project, &model, &view()).await));
    assert!(refused(s.kind(&dan, &project, &model, &i("wall")).await));
    assert!(refused(s.claims(&dan, &project, &model).await));
    assert!(refused(s.record(&dan, &project, &model, &hash).await));
    assert!(refused(
        s.shape(&dan, &project, &model, &i("W-101"), None).await
    ));
    assert!(refused(s.links(&dan, &project, &model).await));
    assert!(!s.may_watch(&dan, &project, &model).await.unwrap());
    assert!(refused(
        s.edit(&dan, &project, &model, "element.move", moved("W-101", 1, 1))
            .await
    ));
    let listing = s.project(&dan, &project).await.unwrap();
    let arch = &listing["teams"][0];
    let kinds: Vec<_> = arch["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("shared"), json!("consumed")]);
}

/// Workshop keeps its parts as elements, through the same edit log as walls, and finds them by
/// kind: they have no place in the plan, because this server's geometry doesn't build them.
#[tokio::test(flavor = "multi_thread")]
async fn workshop_parts_are_elements_listed_by_kind() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, project, model) = (i("anna"), i("ben"), i("example-1"), i("arch-model"));
    let recipe = json!({ "owner": "workshop", "schema": 1, "op": "part", "op_version": 1,
        "args": { "end": 0, "features": [], "material": "steel",
                  "params": [{ "name": "width", "value": 120 }] } });
    let args = json!({ "element": "WP-1", "kind": "workshop.part", "name": "Bracket", "level": 0,
        "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe,
        "props": { "category": "Workshop parts" } });
    let seq = s
        .edit(&anna, &project, &model, "element.create", args)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();

    // Ben, on the same team, finds it, reads it and sees no place for it in the plan.
    let parts = s
        .kind(&ben, &project, &model, &i("workshop.part"))
        .await
        .unwrap();
    assert!(parts.through >= seq);
    let hash = i(&parts.pieces["WP-1"]);
    let record = s.record(&ben, &project, &model, &hash).await.unwrap();
    assert_eq!(record.recipe.owner, "workshop");
    assert_eq!(record.name, "Bracket");
    let area = s
        .area(
            &ben,
            &project,
            &model,
            &View {
                level: i("0"),
                ..view()
            },
        )
        .await
        .unwrap();
    assert!(!area.pieces.contains_key("WP-1"));
    let walls = s.kind(&ben, &project, &model, &i("wall")).await.unwrap();
    assert!(!walls.pieces.is_empty() && !walls.pieces.contains_key("WP-1"));

    // A change goes through the log like any other, and the list follows it.
    let width = json!({ "element": "WP-1", "name": "Wide bracket" });
    s.edit(&ben, &project, &model, "element.set", width)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let parts = s
        .kind(&anna, &project, &model, &i("workshop.part"))
        .await
        .unwrap();
    assert_ne!(parts.pieces["WP-1"], hash.to_string());
}

/// A grid and a reference plane are elements of the datum categories, made through the log with
/// their own commands, checked on the server, and found by kind: they stand on no level, so the
/// plan's area of a level does not list them.
#[tokio::test(flavor = "multi_thread")]
async fn grids_and_planes_are_elements_made_by_their_own_commands_and_found_by_kind() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, dan, project, model) = (
        i("anna"),
        i("ben"),
        i("dan"),
        i("example-1"),
        i("arch-model"),
    );
    let line = json!({ "grid": "G-1", "name": null, "x0": "0", "y0": "0", "x1": "6 m", "y1": "0" });
    let seq = s
        .edit(&anna, &project, &model, "grid.create", line)
        .await
        .unwrap();
    let plane = json!({ "plane": "P-1", "name": "Mid", "elevation": "1.2 m" });
    s.edit(&anna, &project, &model, "plane.create", plane)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();

    // Ben, on the same team, finds both by kind and reads them; the first grid is named 1.
    let grids = s.kind(&ben, &project, &model, &i("grid")).await.unwrap();
    assert!(grids.through >= seq);
    let record = s
        .record(&ben, &project, &model, &i(&grids.pieces["G-1"]))
        .await
        .unwrap();
    assert_eq!(record.name, "1");
    assert_eq!(record.props["@category"], "3700");
    let planes = s
        .kind(&ben, &project, &model, &i("refplane"))
        .await
        .unwrap();
    assert_eq!(planes.pieces.len(), 1);
    let area = s.area(&ben, &project, &model, &view()).await.unwrap();
    assert!(!area.pieces.contains_key("G-1"), "a grid is on no level");

    // A wrong value is turned back with the data crate's words, and the element commands leave
    // a grid to its own.
    let name =
        json!({ "grid": "G-2", "name": "1", "x0": "0", "y0": "1 m", "x1": "6 m", "y1": "1 m" });
    let clash = s.edit(&ben, &project, &model, "grid.create", name).await;
    assert!(matches!(clash, Err(Refusal::Refused(why)) if why.contains("already named")));
    let set = json!({ "element": "G-1", "name": "X" });
    let by_hand = s.edit(&anna, &project, &model, "element.set", set).await;
    assert!(matches!(by_hand, Err(Refusal::Refused(why)) if why.contains("their own commands")));
    let delete = json!({ "element": "G-1" });
    let gone = s
        .edit(&anna, &project, &model, "element.delete", delete)
        .await;
    assert!(matches!(gone, Err(Refusal::Refused(why)) if why.contains("grid.delete")));
    // Someone with no access to the model is refused, like any edit.
    let line = json!({ "grid": "G-3", "name": "Q", "x0": "0", "y0": "0", "x1": "1 m", "y1": "0" });
    assert!(refused(
        s.edit(&dan, &project, &model, "grid.create", line).await
    ));
}

/// A parameter is defined, bound and set through the log like any edit, checked on the server
/// with construct-params, and everyone on the team reads it with its category and units.
#[tokio::test(flavor = "multi_thread")]
async fn parameters_go_through_the_log_and_are_read_with_units() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, dan, project, model) = (
        i("anna"),
        i("ben"),
        i("dan"),
        i("example-1"),
        i("arch-model"),
    );
    let sill = json!({ "definition": { "id": "P-sill", "name": "Sill height", "kind": "length",
                                       "holder": "instance", "choices": [] } });
    s.edit(&anna, &project, &model, "parameter.define", sill)
        .await
        .unwrap();
    let bind = json!({ "parameter": "P-sill", "categories": [300], "default": "0" });
    s.edit(&anna, &project, &model, "parameter.bind", bind)
        .await
        .unwrap();
    let set = |value: &str| json!({ "element": "D-101", "parameter": "P-sill", "value": value });
    s.edit(&ben, &project, &model, "element.param", set("0.15 m"))
        .await
        .unwrap();

    // A wrong unit is refused with construct-params' reason, and changes nothing.
    let Err(Refusal::Refused(reason)) = s
        .edit(&ben, &project, &model, "element.param", set("3 kg"))
        .await
    else {
        panic!("a mass is no length");
    };
    assert!(reason.contains("Sill height"), "{reason}");

    let view = s
        .parameters(&anna, &project, &model, &i("D-101"))
        .await
        .unwrap();
    let view = serde_json::to_value(view).unwrap();
    assert_eq!(view["category"], json!({ "id": 300, "name": "Doors" }));
    assert_eq!(view["type"], Value::Null);
    let sill = &view["parameters"][0];
    assert_eq!(sill["name"], "Sill height");
    assert_eq!(sill["value"], "length:q0:960000");
    assert_eq!(sill["shown"], "150 mm");
    assert_eq!(sill["from"], "instance");
    let other = s
        .parameters(&anna, &project, &model, &i("D-102"))
        .await
        .unwrap();
    assert_eq!(other.parameters[0].from, "default");

    // The seed's own text is kept beside the value, and the record says its category.
    s.regenerate(&project).await.unwrap();
    let doors = s.kind(&anna, &project, &model, &i("door")).await.unwrap();
    let hash = i(&doors.pieces["D-101"]);
    let record = s.record(&anna, &project, &model, &hash).await.unwrap();
    assert_eq!(record.props["fire rating"], "FD30");
    assert_eq!(record.props["@category"], "300");
    assert_eq!(record.props["P-sill"], "length:q0:960000");

    // Someone who can't read the model can't read its parameters either.
    assert!(refused(
        s.parameters(&dan, &project, &model, &i("D-101")).await
    ));
    assert!(refused(
        s.edit(&dan, &project, &model, "element.param", set("1"))
            .await
    ));
}

/// A parameter given by a formula is defined through the log, worked out on the server wherever
/// it is read, and shown with its formula; a value for it is refused with the server's reason.
#[tokio::test(flavor = "multi_thread")]
async fn a_formula_parameter_is_worked_out_on_the_server_and_shown_with_its_formula() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    for (definition, default) in [
        (
            json!({ "id": "P-sill", "name": "Sill", "kind": "length", "holder": "instance",
                    "choices": [] }),
            json!("150"),
        ),
        (
            json!({ "id": "P-head", "name": "Head", "kind": "length", "holder": "instance",
                    "choices": [], "formula": "Sill+2100mm" }),
            Value::Null,
        ),
    ] {
        let id = definition["id"].clone();
        let define = json!({ "definition": definition });
        s.edit(&anna, &project, &model, "parameter.define", define)
            .await
            .unwrap();
        let bind = json!({ "parameter": id, "categories": [300], "default": default });
        s.edit(&anna, &project, &model, "parameter.bind", bind)
            .await
            .unwrap();
    }
    let view = s
        .parameters(&anna, &project, &model, &i("D-101"))
        .await
        .unwrap();
    let head = view.parameters.iter().find(|p| p.name == "Head").unwrap();
    assert_eq!(head.from, "formula");
    assert_eq!(head.formula.as_deref(), Some("Sill + 2100 mm"));
    assert_eq!(head.shown.as_deref(), Some("2250 mm"));

    let set = json!({ "element": "D-101", "parameter": "P-head", "value": "2000" });
    let Err(Refusal::Refused(reason)) = s.edit(&anna, &project, &model, "element.param", set).await
    else {
        panic!("the formula gives the head height");
    };
    assert!(
        reason.starts_with("Head is given by its formula"),
        "{reason}"
    );

    // The sill follows into the head; the project's parameters list the formula.
    let set = json!({ "element": "D-101", "parameter": "P-sill", "value": "0.9 m" });
    s.edit(&anna, &project, &model, "element.param", set)
        .await
        .unwrap();
    let view = s
        .parameters(&anna, &project, &model, &i("D-101"))
        .await
        .unwrap();
    let head = view.parameters.iter().find(|p| p.name == "Head").unwrap();
    assert_eq!(head.shown.as_deref(), Some("3000 mm"));
    let table = s.project_parameters(&anna, &project, &model).await.unwrap();
    let head = table.parameters.iter().find(|p| p.name == "Head").unwrap();
    assert_eq!(head.formula.as_deref(), Some("Sill + 2100 mm"));
}

/// What the page needs to manage parameters and types is read from the server, so the page holds
/// no list of its own: the kinds, the categories, the project's parameters with where each is
/// bound, and the types with their values in units. Only someone who can read the model reads it.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_reads_kinds_categories_parameters_and_types_to_manage_them() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));

    let catalog = serde_json::to_value(s.catalog(&anna, &project, &model).await.unwrap()).unwrap();
    let kinds = catalog["kinds"].as_array().unwrap();
    assert!(
        kinds
            .iter()
            .any(|k| k["tag"] == "length" && k["label"] == "a length")
    );
    let doors = catalog["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == 300)
        .unwrap();
    assert_eq!(doors["name"], "Doors");

    let width = json!({ "definition": { "id": "P-width", "name": "Leaf width", "kind": "length",
                                       "holder": "type", "choices": [] } });
    s.edit(&anna, &project, &model, "parameter.define", width)
        .await
        .unwrap();
    // The seed's wall types defined the two wall parameters beside it.
    let named = |table: &Value, name: &str| {
        table["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap()
            .clone()
    };
    let table =
        serde_json::to_value(s.project_parameters(&anna, &project, &model).await.unwrap()).unwrap();
    assert_eq!(table["parameters"].as_array().unwrap().len(), 3);
    assert_eq!(
        named(&table, "Leaf width")["categories"],
        json!([]),
        "defined, bound nowhere"
    );
    let bind = json!({ "parameter": "P-width", "categories": [300], "default": "900" });
    s.edit(&anna, &project, &model, "parameter.bind", bind)
        .await
        .unwrap();
    let table =
        serde_json::to_value(s.project_parameters(&anna, &project, &model).await.unwrap()).unwrap();
    let leaf = &named(&table, "Leaf width");
    assert_eq!(leaf["holder"], "type");
    assert_eq!(leaf["default"], "900 mm");
    assert_eq!(leaf["categories"], json!([{ "id": 300, "name": "Doors" }]));

    let make = json!({ "type": "T-flush", "name": "Flush", "category": 300 });
    s.edit(&anna, &project, &model, "type.create", make)
        .await
        .unwrap();
    let set = json!({ "type": "T-flush", "parameter": "P-width", "value": "2.4 m" });
    s.edit(&anna, &project, &model, "type.set", set)
        .await
        .unwrap();
    let retype = json!({ "element": "D-101", "type": "T-flush" });
    s.edit(&anna, &project, &model, "element.type", retype)
        .await
        .unwrap();
    let view =
        serde_json::to_value(s.types(&anna, &project, &model, Some(300)).await.unwrap()).unwrap();
    let flush = &view["types"][0];
    assert_eq!(flush["name"], "Flush");
    assert_eq!(flush["elements"], 1);
    assert_eq!(flush["parameters"][0]["shown"], "2400 mm");
    assert_eq!(flush["parameters"][0]["from"], "type");
    // Walls has the seed's three wall types; every type is listed when none is asked for.
    let walls = s.types(&anna, &project, &model, Some(100)).await.unwrap();
    assert_eq!(walls.types.len(), 3);
    assert_eq!(
        s.types(&anna, &project, &model, None)
            .await
            .unwrap()
            .types
            .len(),
        4
    );

    // A delete that is refused says so in the server's words; the page shows them as they are.
    let Err(Refusal::Refused(reason)) = s
        .edit(
            &anna,
            &project,
            &model,
            "type.delete",
            json!({ "type": "T-flush" }),
        )
        .await
    else {
        panic!("a type in use is not deleted");
    };
    assert_ne!(reason, "");
    // No category has this ID.
    assert!(matches!(
        s.types(&anna, &project, &model, Some(99_999)).await,
        Err(Refusal::Bad(_))
    ));
    // Someone who can't read the model can't read any of it.
    assert!(refused(s.catalog(&dan, &project, &model).await));
    assert!(refused(s.project_parameters(&dan, &project, &model).await));
    assert!(refused(s.types(&dan, &project, &model, None).await));
}

/// Where a wall of the model is now, as Anna reads it.
async fn placed(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
    wall: &str,
) -> construct_model::element::Placement {
    s.regenerate(project).await.unwrap();
    let walls = s
        .kind(&i("anna"), project, model, &i("wall"))
        .await
        .unwrap();
    let hash = i(&walls.pieces[wall]);
    s.record(&i("anna"), project, model, &hash)
        .await
        .unwrap()
        .placement
}

/// The page's levels: the levels the seed's integers imply, then levels made, renamed and moved
/// through the log, shown as the server formats them; a plan at a level by its ID; a delete
/// refused with how many elements are tied to the level.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_lists_levels_and_a_plan_is_at_a_level_by_its_id() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, dan, project, model) = (
        i("anna"),
        i("ben"),
        i("dan"),
        i("example-1"),
        i("arch-model"),
    );
    let listed = |levels: Vec<construct_regen::levels::LevelView>| {
        levels
            .into_iter()
            .map(|l| (l.id, l.name, l.shown, l.implied, l.tied))
            .collect::<Vec<_>>()
    };
    // Level 1 holds 5 walls and 3 doors and the floor plan of it: 9. Level 2 holds 5 walls, 4 doors,
    // the tops of Level 1's five walls and its floor plan: 15.
    let seeded = listed(s.levels(&anna, &project, &model).await.unwrap());
    assert_eq!(
        seeded,
        [
            (
                "1".to_owned(),
                "Level 1".to_owned(),
                "0 mm".to_owned(),
                true,
                9
            ),
            (
                "2".to_owned(),
                "Level 2".to_owned(),
                "3000 mm".to_owned(),
                true,
                15
            ),
        ]
    );
    assert!(refused(s.levels(&dan, &project, &model).await));

    let roof = json!({ "level": "L-roof", "name": "Roof", "elevation": "6.2 m" });
    s.edit(&anna, &project, &model, "level.create", roof)
        .await
        .unwrap();
    let rename = json!({ "level": "2", "name": "First floor" });
    s.edit(&ben, &project, &model, "level.rename", rename)
        .await
        .unwrap();
    let levels = listed(s.levels(&anna, &project, &model).await.unwrap());
    assert_eq!(
        levels[1..],
        [
            (
                "2".to_owned(),
                "First floor".to_owned(),
                "3000 mm".to_owned(),
                false,
                15
            ),
            (
                "L-roof".to_owned(),
                "Roof".to_owned(),
                "6200 mm".to_owned(),
                false,
                0
            ),
        ]
    );

    // A plan asks for a level by its ID; the seed's walls are on the level their integer names.
    let wall = json!({ "element": "W-roof", "kind": "wall", "level": "L-roof",
                       "placement": { "x": 100, "y": 100, "z": 0 },
                       "recipe": { "owner": "construct", "schema": 1, "op": "wall.straight",
                                   "op_version": 1,
                                   "args": { "length": 3000, "thickness": 200, "height": 1000 } } });
    s.edit(&anna, &project, &model, "element.create", wall)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let at = |level: &str| View {
        level: i(level),
        ..view()
    };
    let roof = s
        .area(&anna, &project, &model, &at("L-roof"))
        .await
        .unwrap();
    assert_eq!(roof.pieces.keys().collect::<Vec<_>>(), ["W-roof"]);
    let first = s.area(&anna, &project, &model, &at("2")).await.unwrap();
    assert!(first.pieces.contains_key("W-201") && !first.pieces.contains_key("W-roof"));

    let refused_delete = s
        .edit(
            &anna,
            &project,
            &model,
            "level.delete",
            json!({ "level": "1" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused_delete.to_string(),
        "Level 1 can't be deleted: 9 elements are tied to it."
    );
}

/// Undo is an edit like any other: through the log, behind the same access, and each person's
/// own. What each person can undo is read back from the log alone.
#[tokio::test(flavor = "multi_thread")]
async fn undo_and_redo_go_through_the_log_and_are_each_persons_own() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, ben, dan, project, model) = (
        i("anna"),
        i("ben"),
        i("dan"),
        i("example-1"),
        i("arch-model"),
    );
    let start = placed(s, &project, &model, "W-101").await;

    // Ben has made no change to this model, so he has nothing to undo; Anna's last is the seed's, the floor plan of Level 2.
    let state = s.undo(&ben, &project, &model).await.unwrap();
    assert_eq!((state.undo, state.redo), (None, None));
    let state = s.undo(&anna, &project, &model).await.unwrap();
    assert_eq!(state.undo.unwrap().cmd, "view.create");

    let seq = s
        .edit(
            &ben,
            &project,
            &model,
            "element.move",
            moved("W-101", 1000, 2000),
        )
        .await
        .unwrap();
    let state = s.undo(&ben, &project, &model).await.unwrap();
    assert_eq!(state.undo.as_ref().map(|step| step.seq), Some(seq));
    s.edit(&ben, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    assert_eq!(placed(s, &project, &model, "W-101").await, start);
    let state = s.undo(&ben, &project, &model).await.unwrap();
    assert_eq!(state.undo, None);
    assert_eq!(state.redo.unwrap().cmd, "edit.undo");
    s.edit(&ben, &project, &model, "edit.redo", json!({}))
        .await
        .unwrap();
    assert_eq!(placed(s, &project, &model, "W-101").await.x, 1000);

    // Dan can't read this model, so he can't ask, and he can't undo in it.
    assert!(refused(s.undo(&dan, &project, &model).await));
    assert!(refused(
        s.edit(&dan, &project, &model, "edit.undo", json!({})).await
    ));
    let mut snapshot = stack.store.snapshot().await.unwrap();
    verify_log(&mut snapshot, &Scope::Project(project))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_cant_be_reached_through_another_model() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project) = (i("anna"), i("example-1"));
    let arch = s
        .area(&anna, &project, &i("arch-model"), &view())
        .await
        .unwrap();
    let hash = i(arch.pieces.values().next().unwrap());
    let elsewhere = s
        .record(&anna, &project, &i("interiors-model"), &hash)
        .await;
    assert!(matches!(elsewhere, Err(Refusal::NotFound)));
}

#[tokio::test(flavor = "multi_thread")]
async fn people_off_a_project_cant_open_it_or_its_hub() {
    let stack = seeded().await;
    let s = &stack.service;
    let (cara, anna, project) = (i("cara"), i("anna"), i("example-1"));
    assert!(refused(s.project(&cara, &project).await));
    assert!(refused(s.bridge_targets(&cara, &project).await));
    assert!(refused(s.may_open_project(&cara, &project).await));
    assert!(refused(s.hub(&anna, &i("company-2")).await));
    assert!(s.hub(&i("dan"), &i("company-1")).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn nobody_outside_a_team_can_publish_or_take_for_it() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, ben, project, arch) = (i("dan"), i("ben"), i("example-1"), i("arch"));
    assert!(refused(
        s.publish(&dan, &project, &arch, &i("arch-model")).await
    ));
    assert!(refused(
        s.publish(&ben, &project, &i("civil"), &i("arch-model"))
            .await
    ));
    assert!(refused(
        s.consume(&dan, &project, &arch, &i("nothing")).await
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bridge_needs_both_hubs_and_the_owning_team() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, arch) = (i("anna"), i("dan"), i("example-1"), i("arch"));
    let item = s
        .publish(&anna, &project, &arch, &i("arch-model"))
        .await
        .unwrap();
    assert_eq!(item.name, "Arch package v12");

    let one_sided = s
        .bridge(
            &anna,
            &project,
            &arch,
            &item.id,
            &i("riverside-bridged"),
            &i("landscape"),
        )
        .await;
    assert!(refused(one_sided), "Company 3 hasn't permitted Company 1");
    let not_theirs = s
        .bridge(
            &dan,
            &project,
            &arch,
            &item.id,
            &i("example-1-bridged"),
            &i("structure"),
        )
        .await;
    assert!(refused(not_theirs));
    s.bridge(
        &anna,
        &project,
        &arch,
        &item.id,
        &i("example-1-bridged"),
        &i("structure"),
    )
    .await
    .unwrap();

    // The arrival is in the receiving project's own log, written in the same transaction.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let bridged = Scope::Project(i("example-1-bridged"));
    let head = verify_log(&mut snapshot, &bridged).await.unwrap().entries;
    let log = read_log(&mut snapshot, &bridged, 0, head).await.unwrap();
    assert!(log.iter().any(|entry| entry.cmd == "package.arrived"));

    let incoming = i("company-1.arch-package-v12");
    let taken = s
        .consume(&dan, &i("example-1-bridged"), &i("structure"), &incoming)
        .await
        .unwrap();
    assert_eq!(taken.from.as_ref().unwrap().hub_name, "Company 1");
    assert_eq!(taken.sha256, item.sha256);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_whose_file_changed_is_refused() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, arch) = (i("anna"), i("dan"), i("example-1"), i("arch"));
    let item = s
        .publish(&anna, &project, &arch, &i("arch-model"))
        .await
        .unwrap();
    s.bridge(
        &anna,
        &project,
        &arch,
        &item.id,
        &i("example-1-bridged"),
        &i("structure"),
    )
    .await
    .unwrap();
    stack
        .files
        .put(&item.file, b"something else".to_vec())
        .await
        .unwrap();
    let incoming = i("company-1.arch-package-v12");
    let taken = s
        .consume(&dan, &i("example-1-bridged"), &i("structure"), &incoming)
        .await;
    assert!(matches!(taken, Err(Refusal::Bad(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn links_follow_their_rules() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project) = (i("anna"), i("dan"), i("example-1"));
    let site = i("site-setout");
    let across = s
        .add_link(
            &anna,
            &project,
            &site,
            LinkKind::Live,
            None,
            &i("arch-model"),
        )
        .await;
    assert!(refused(across), "she isn't on the Civil team");
    let across = s
        .add_link(
            &i("ben"),
            &project,
            &site,
            LinkKind::Live,
            None,
            &i("arch-model"),
        )
        .await;
    assert!(refused(across), "a live link stays inside one team's WIP");
    let civil = i("civil");
    let outside = s
        .add_link(
            &dan,
            &project,
            &i("arch-model"),
            LinkKind::Shared,
            Some(&civil),
            &i("site-setout-v3"),
        )
        .await;
    assert!(refused(outside));
    let kinds: Vec<_> = s
        .links(&anna, &project, &i("arch-model"))
        .await
        .unwrap()
        .iter()
        .map(|l| l.kind)
        .collect();
    assert_eq!(kinds.len(), 3);
    assert!(
        kinds.contains(&LinkKind::Live)
            && kinds.contains(&LinkKind::Shared)
            && kinds.contains(&LinkKind::Consumed)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_is_read_from_its_own_folder() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, project) = (i("dan"), i("example-1"));
    let shared = folders::shared();
    let file = s
        .package_file(&dan, &project, &i("arch"), &shared, &i("arch-package-v11"))
        .await;
    assert!(
        !file.unwrap().is_empty(),
        "Shared opens for the whole project"
    );
    let wip = folders::wip();
    let closed = s
        .package_file(&dan, &project, &i("arch"), &wip, &i("arch-package-v11"))
        .await;
    assert!(refused(closed));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backup_restores_every_key_and_file_into_an_empty_store() {
    let stack = seeded().await;
    let target = MemoryFiles::default();
    let before = quiet(&stack).await;
    backup::backup(&stack.store, &stack.files, &target, "b")
        .await
        .unwrap();

    let (fresh, fresh_files) = (Memory::new(), MemoryFiles::default());
    backup::restore(&fresh, &fresh_files, &target, "b")
        .await
        .unwrap();
    assert_eq!(backup::digest(&fresh, &fresh_files).await.unwrap(), before);
    assert!(
        backup::restore(&fresh, &fresh_files, &target, "b")
            .await
            .is_err(),
        "never over live work"
    );

    // The restored store is a working one.
    let service = build(&fresh, &fresh_files);
    service
        .edit(
            &i("anna"),
            &i("example-1"),
            &i("arch-model"),
            "element.move",
            moved("W-101", 5, 5),
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shape_cant_be_reached_through_another_model() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project) = (i("anna"), i("example-1"));
    let there = s
        .shape(&anna, &project, &i("arch-model"), &i("W-101"), None)
        .await;
    assert!(there.is_ok());
    let elsewhere = s
        .shape(&anna, &project, &i("interiors-model"), &i("W-101"), None)
        .await;
    assert!(matches!(elsewhere, Err(Refusal::NotFound)));
}

/// The export read gives an IFC 4.3 file named for the model, only to a person who may read it.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_is_exported_as_an_ifc_file_to_those_who_may_read_it() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let (name, text) = s.export_ifc(&anna, &project, &model).await.unwrap();
    assert!(name.to_ascii_lowercase().ends_with(".ifc"), "{name}");
    assert!(text.starts_with(
        "ISO-10303-21;
"
    ));
    assert!(text.contains("FILE_SCHEMA(('IFC4X3_ADD2'));"));
    assert!(text.contains("=IFCPROJECT("));
    let elsewhere = s.export_ifc(&anna, &project, &i("no-such-model")).await;
    assert!(elsewhere.is_err());
    let outsider = s.export_ifc(&i("dan"), &project, &model).await;
    assert!(outsider.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_says_how_far_regeneration_has_got() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let before = s.area(&anna, &project, &model, &view()).await.unwrap();
    assert!(before.through > 0);
    let seq = s
        .edit(
            &anna,
            &project,
            &model,
            "element.move",
            moved("D-101", 3000, 100),
        )
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let after = s.area(&anna, &project, &model, &view()).await.unwrap();
    assert!(after.through >= seq);
    assert_ne!(after.pieces["D-101"], before.pieces["D-101"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_tells_everyone_what_it_built_and_only_the_server_can() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let args = json!({ "through": 1, "bodies": { "W-101": "made-up" } });
    assert!(refused(
        s.edit(&anna, &project, &model, "document.checkpoint", args)
            .await
    ));

    let seq = s
        .edit(
            &anna,
            &project,
            &model,
            "element.move",
            moved("D-102", 700, 700),
        )
        .await
        .unwrap();
    let shown = s
        .shape(&anna, &project, &model, &i("D-102"), None)
        .await
        .unwrap();
    let BuiltResult::Shape { body, .. } = &shown.built.result else {
        panic!("a door builds");
    };
    let wanted = body.to_string();

    // The checkpoint that covers the move lands by itself, in the log, in order.
    let scope = Scope::Project(project.clone());
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let mut snapshot = stack.store.snapshot().await.unwrap();
        let log = read_log(&mut snapshot, &scope, seq, u64::MAX - 1)
            .await
            .unwrap();
        let found = log.iter().find(|entry| {
            entry.cmd == "document.checkpoint" && entry.args["bodies"].get("D-102").is_some()
        });
        if let Some(entry) = found {
            assert_eq!(entry.args["bodies"]["D-102"], wanted.as_str());
            assert_eq!(entry.author.as_str(), "system");
            assert!(entry.args["through"].as_u64().unwrap() >= seq);
            return;
        }
    }
    panic!("no checkpoint came");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_carries_finished_shapes() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, project) = (i("dan"), i("example-1"));
    let shared = folders::shared();
    let bytes = s
        .package_file(&dan, &project, &i("arch"), &shared, &i("arch-package-v11"))
        .await
        .unwrap();
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(file["edition"]["engine"], "construct.stand-in");
    let first = &file["elements"][0];
    assert!(first["built"]["result"]["shape"]["body"].is_string());
    assert!(first["display"]["outline"].is_array());
    assert!(first["element"]["recipe"]["op"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hosted_door_keeps_its_face_through_the_log_and_a_package() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let wall = s
        .shape(&anna, &project, &model, &i("W-101"), None)
        .await
        .unwrap();
    let BuiltResult::Shape { names, .. } = &wall.built.result else {
        panic!("a wall builds");
    };
    let face = names.iter().find(|n| n.role == "exterior").unwrap();
    let recipe = json!({ "owner": "construct", "schema": 1, "op": "door.single", "op_version": 1,
        "args": { "width": 900, "thickness": 300, "height": 2100 } });
    let args = json!({
        "element": "D-900", "kind": "door", "level": 1, "recipe": recipe,
        "placement": { "x": 1000, "y": 0, "z": 0 },
        "references": [{ "doc": null, "element": "W-101", "name": face.name, "kind": "face",
                         "want": "carrier", "signature": null }],
    });
    s.edit(&anna, &project, &model, "element.create", args)
        .await
        .unwrap();
    let item = s
        .publish(&anna, &project, &i("arch"), &model)
        .await
        .unwrap();
    let bytes = s
        .package_file(&anna, &project, &i("arch"), &folders::shared(), &item.id)
        .await
        .unwrap();
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    let door = file["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["element"]["id"] == "D-900")
        .unwrap();
    assert_eq!(door["element"]["references"][0]["name"], face.name.as_str());
}

#[tokio::test(flavor = "multi_thread")]
async fn every_stored_shape_matches_its_recipe_after_the_seed() {
    let stack = seeded().await;
    let regen = construct_regen::regenerator::Regenerator::new(
        stack.store.clone(),
        stack.files.clone(),
        construct_regen::geometry(),
    );
    assert_eq!(regen.verify(&i("example-1")).await.unwrap(), 26);
}

/// The partition and the plain concrete wall of construct-walls' defaults, as `wall.type.create`
/// takes them: layers from the exterior face, each thickness typed text.
fn wall_types() -> [Value; 2] {
    let layer = |function: &str, thickness: &str, material: &str, wraps: bool| {
        json!({ "function": function, "thickness": thickness, "material": material,
                "wraps": wraps })
    };
    [
        json!({ "type": "wall-partition", "name": "Partition, 90 stud",
                "assembly": { "core": { "exterior": 1, "interior": 2 }, "layers": [
                    layer("finish", "12.5 mm", "plasterboard-gypsum", true),
                    layer("structure", "90 mm", "timber-douglas-fir", false),
                    layer("finish", "12.5 mm", "plasterboard-gypsum", true),
                ] } }),
        json!({ "type": "wall-concrete", "name": "Plain concrete 200",
                "assembly": { "core": { "exterior": 0, "interior": 1 }, "layers": [
                    layer("structure", "200 mm", "concrete-regular", false),
                ] } }),
    ]
}

/// A wall's built bounds, after the server has caught up.
async fn wall_bounds(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
    wall: &str,
) -> ([i64; 3], [i64; 3]) {
    s.regenerate(project).await.unwrap();
    let shown = s
        .shape(&i("anna"), project, model, &i(wall), None)
        .await
        .unwrap();
    match shown.built.result {
        BuiltResult::Shape { bounds, .. } => (bounds.min, bounds.max),
        failed @ BuiltResult::Failed { .. } => panic!("{wall}: {failed:?}"),
    }
}

/// Through the server's commands on the stand-in: wall types made, a layered wall drawn between
/// two points of one, from level 1 to level 2, built as one box of the type's thickness; a door
/// hosted on it; a change to its type's layers builds it again, and undo and redo take that back
/// and forth; a person off the team can't draw one; every stored shape matches its recipe.
#[tokio::test(flavor = "multi_thread")]
async fn a_layered_wall_drawn_by_command_is_built_from_its_type() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    for ty in wall_types() {
        s.edit(&anna, &project, &model, "wall.type.create", ty)
            .await
            .unwrap();
    }
    let draw = json!({ "element": "W-900", "type": "wall-partition", "start": ["50 m", "0"],
                       "end": ["54 m", "0"], "level": "1", "top": "2" });
    assert!(refused(
        s.edit(&dan, &project, &model, "wall.draw", draw.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "wall.draw", draw)
        .await
        .unwrap();
    assert_eq!(
        wall_bounds(s, &project, &model, "W-900").await,
        ([50_000, -58, 0], [54_000, 58, 3000])
    );
    let door = json!({ "element": "D-900", "host": "W-900", "face": "box.exterior",
                       "along": "1200 mm",
                       "recipe": { "owner": "construct", "schema": 1, "op": "door.single",
                                   "op_version": 1,
                                   "args": { "width": 900, "thickness": 100, "height": 2100 } } });
    s.edit(&anna, &project, &model, "hosted.place", door)
        .await
        .unwrap();
    s.regenerate(&project).await.unwrap();
    let hosted = s
        .shape(&anna, &project, &model, &i("D-900"), None)
        .await
        .unwrap();
    assert!(matches!(hosted.built.result, BuiltResult::Shape { .. }));

    let concrete = wall_types()[1]["assembly"].clone();
    let layers = json!({ "type": "wall-partition", "assembly": concrete });
    s.edit(&anna, &project, &model, "wall.type.layers", layers)
        .await
        .unwrap();
    let thick = ([50_000, -100, 0], [54_000, 100, 3000]);
    assert_eq!(wall_bounds(s, &project, &model, "W-900").await, thick);
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    assert_eq!(
        wall_bounds(s, &project, &model, "W-900").await,
        ([50_000, -58, 0], [54_000, 58, 3000])
    );
    s.edit(&anna, &project, &model, "edit.redo", json!({}))
        .await
        .unwrap();
    assert_eq!(wall_bounds(s, &project, &model, "W-900").await, thick);

    let regen = construct_regen::regenerator::Regenerator::new(
        stack.store.clone(),
        stack.files.clone(),
        construct_regen::geometry(),
    );
    // The seed's shapes (the site setout's five grids among them) and what this test drew.
    assert_eq!(regen.verify(&project).await.unwrap(), 28);
}

/// A corner of a floor's boundary as `floor.create` takes it, in millimetres as typed text.
fn corner(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

/// The L-shaped floor, 8 m by 8 m less its notch, with a 2 m square hole, from `x0` mm along x.
fn l_floor(x0: i64) -> Value {
    json!({
        "outer": [corner("e0", x0, 0), corner("e1", x0 + 8000, 0), corner("e2", x0 + 8000, 4000),
                  corner("e3", x0 + 4000, 4000), corner("e4", x0 + 4000, 8000),
                  corner("e5", x0, 8000)],
        "holes": [[corner("h0", x0 + 1000, 1000), corner("h1", x0 + 3000, 1000),
                   corner("h2", x0 + 3000, 3000), corner("h3", x0 + 1000, 3000)]],
    })
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

/// Through the server's commands on the stand-in: the default floor types made, an L-shaped
/// floor with a hole drawn on level 1 of the 200 mm concrete type, built as the box around it;
/// its parameters view lists its area, perimeter, thickness, volume and mass, and each layer's
/// volume and mass, in units and read-only; its boundary changed, then undone, changes them; a
/// person off the team can't draw one.
#[tokio::test(flavor = "multi_thread")]
async fn a_floor_drawn_by_command_shows_its_quantities_in_units() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    // The page's floor tool reads the floor types: none until it sends the defaults.
    let none = s.floor_types(&anna, &project, &model).await.unwrap();
    assert!(none.types.is_empty());
    assert!(none.materials.iter().any(|m| m.id == "concrete-regular"));
    assert!(refused(s.floor_types(&dan, &project, &model).await));
    s.edit(&anna, &project, &model, "floor.defaults", json!({}))
        .await
        .unwrap();
    let types = s.floor_types(&anna, &project, &model).await.unwrap();
    let ids: Vec<&str> = types.types.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids.len(), 3);
    let concrete = types
        .types
        .iter()
        .find(|t| t.id == "floor-concrete-200")
        .unwrap();
    assert_eq!(concrete.thickness, "200 mm");
    assert_eq!(
        concrete.layers,
        json!([{ "function": "structure", "thickness": "200 mm", "material": "concrete-regular" }])
    );
    let floor = json!({ "element": "F-900", "type": "floor-concrete-200", "level": "1",
                        "boundary": l_floor(60_000) });
    assert!(refused(
        s.edit(&dan, &project, &model, "floor.create", floor.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "floor.create", floor)
        .await
        .unwrap();
    assert_eq!(
        wall_bounds(s, &project, &model, "F-900").await,
        ([60_000, 0, -200], [68_000, 8000, 0])
    );
    let view = serde_json::to_value(
        s.parameters(&anna, &project, &model, &i("F-900"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(view["category"], json!({ "id": 500, "name": "Floors" }));
    assert_eq!(
        view["type"],
        json!({ "id": "floor-concrete-200", "name": "Concrete 200" })
    );

    let read = quantities(s, &project, &model, "F-900").await;
    let ids: Vec<&str> = read.keys().map(String::as_str).collect();
    assert_eq!(
        ids,
        [
            "floor.area",
            "floor.layer-1.mass",
            "floor.layer-1.volume",
            "floor.mass",
            "floor.perimeter",
            "floor.thickness",
            "floor.volume"
        ]
    );
    let area = construct_params::Value::Area(construct_params::Area::from_mm2(44_000_000));
    assert_eq!(
        read["floor.area"],
        (Some(area.stored()), Some("44.00 m²".to_owned()))
    );
    assert_eq!(read["floor.perimeter"].1.as_deref(), Some("40000 mm"));
    assert_eq!(read["floor.thickness"].1.as_deref(), Some("200 mm"));
    assert_eq!(read["floor.volume"].1.as_deref(), Some("8.80 m³"));
    assert_eq!(read["floor.layer-1.volume"], read["floor.volume"]);
    assert!(read["floor.mass"].0.is_some(), "concrete is in the library");
    assert_eq!(read["floor.layer-1.mass"], read["floor.mass"]);

    let rectangle = json!({ "outer": [corner("r0", 60_000, 0), corner("r1", 64_000, 0),
                                      corner("r2", 64_000, 3000), corner("r3", 60_000, 3000)] });
    s.edit(
        &anna,
        &project,
        &model,
        "floor.boundary",
        json!({ "element": "F-900", "boundary": rectangle }),
    )
    .await
    .unwrap();
    let smaller = quantities(s, &project, &model, "F-900").await;
    assert_eq!(smaller["floor.area"].1.as_deref(), Some("12.00 m²"));
    assert_eq!(smaller["floor.volume"].1.as_deref(), Some("2.40 m³"));
    assert_eq!(
        wall_bounds(s, &project, &model, "F-900").await,
        ([60_000, 0, -200], [64_000, 3000, 0])
    );
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    assert_eq!(quantities(s, &project, &model, "F-900").await, read);
}

/// An outline as `opening.draw` and `shaft.create` take it: the rectangle from `(x0, y0)` to
/// `(x1, y1)` millimetres, as typed text.
fn rectangle_text(x0: i64, y0: i64, x1: i64, y1: i64) -> Value {
    json!([
        [x0.to_string(), y0.to_string()],
        [x1.to_string(), y0.to_string()],
        [x1.to_string(), y1.to_string()],
        [x0.to_string(), y1.to_string()]
    ])
}

/// An opening drawn in a floor and a shaft through it, by the server's commands: the floor's area,
/// volume and mass as the page reads them are those after the cut (44 m2 less 1 m2 and 1 m2, by
/// 200 mm), each cut taken back by undo, and the floor's filled recipe says what cuts it, which is
/// what the page draws and builds the floor with. A person off the team can neither draw nor read.
#[tokio::test(flavor = "multi_thread")]
async fn an_opening_and_a_shaft_cut_a_floors_quantities_and_the_page_is_told() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    s.edit(&anna, &project, &model, "floor.defaults", json!({}))
        .await
        .unwrap();
    let floor = json!({ "element": "F-900", "type": "floor-concrete-200", "level": "1",
                        "boundary": l_floor(60_000) });
    s.edit(&anna, &project, &model, "floor.create", floor)
        .await
        .unwrap();
    let whole = quantities(s, &project, &model, "F-900").await;
    assert_eq!(whole["floor.area"].1.as_deref(), Some("44.00 m²"));
    let filled = s
        .filled(&anna, &project, &model, &i("F-900"))
        .await
        .unwrap();
    assert_eq!(filled.cut_by.unwrap().cuts.len(), 0);

    let draw = json!({ "element": "O-900", "host": "F-900",
                       "outline": rectangle_text(65_000, 1000, 66_000, 2000) });
    assert!(refused(
        s.edit(&dan, &project, &model, "opening.draw", draw.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "opening.draw", draw)
        .await
        .unwrap();
    let cut = quantities(s, &project, &model, "F-900").await;
    assert_eq!(cut["floor.area"].1.as_deref(), Some("43.00 m²"));
    assert_eq!(cut["floor.volume"].1.as_deref(), Some("8.60 m³"));
    assert_ne!(cut["floor.mass"], whole["floor.mass"]);

    let shaft = json!({ "element": "S-900", "outline": rectangle_text(61_000, 5000, 62_000, 6000),
                        "level": "1", "offset": "-500 mm", "top": "2" });
    s.edit(&anna, &project, &model, "shaft.create", shaft)
        .await
        .unwrap();
    let both = quantities(s, &project, &model, "F-900").await;
    assert_eq!(both["floor.area"].1.as_deref(), Some("42.00 m²"));
    assert_eq!(both["floor.volume"].1.as_deref(), Some("8.40 m³"));
    let filled = s
        .filled(&anna, &project, &model, &i("F-900"))
        .await
        .unwrap();
    let cut_by = serde_json::to_value(filled.cut_by.unwrap()).unwrap();
    let keys: Vec<&str> = cut_by["cuts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["O-900", "S-900"]);
    assert_eq!(cut_by["shafts"].as_array().unwrap().len(), 1);
    assert_eq!(cut_by["shafts"][0]["key"], "S-900");
    assert_eq!(
        cut_by["shafts"][0]["placement"],
        json!({ "x": 0, "y": 0, "z": 0 })
    );
    assert!(
        cut_by["shafts"][0]["recipe"]["args"]["height"]
            .as_str()
            .unwrap()
            .starts_with("length:q0:")
    );
    // The generic commands name the right one, through the service as the page sends them.
    let refusal = s
        .edit(
            &anna,
            &project,
            &model,
            "element.delete",
            json!({ "element": "S-900" }),
        )
        .await
        .unwrap_err();
    assert!(
        format!("{refusal:?}").contains("delete it with shaft.delete"),
        "{refusal:?}"
    );

    // Each cut is one step: the shaft, then the opening, taken back in turn.
    for back_to in [&cut, &whole] {
        s.edit(&anna, &project, &model, "edit.undo", json!({}))
            .await
            .unwrap();
        let now = quantities(s, &project, &model, "F-900").await;
        assert_eq!(now["floor.area"], back_to["floor.area"]);
        assert_eq!(now["floor.volume"], back_to["floor.volume"]);
    }
}

/// Through the server's commands on the stand-in: a floor sloped by `floor.slope` shows its
/// lowest and highest points and its sloped top area (exact where its arrow is whole, as 3 in 4),
/// built as the box between them; its slope cleared, the rows are gone; a floor given slab edges
/// shows each edge's length and volume; both on one floor is refused in words, and the page's
/// filled recipe of a shaped floor holds its tops and its slab edges.
#[tokio::test(flavor = "multi_thread")]
async fn a_sloped_floor_and_a_floor_with_slab_edges_show_their_quantities() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    s.edit(&anna, &project, &model, "floor.defaults", json!({}))
        .await
        .unwrap();
    let rectangle = |x0: i64| {
        json!({ "outer": [corner("r0", x0, 0), corner("r1", x0 + 4000, 0),
                          corner("r2", x0 + 4000, 3000), corner("r3", x0, 3000)] })
    };
    for (id, x0) in [("F-901", 60_000), ("F-902", 70_000)] {
        let floor = json!({ "element": id, "type": "floor-concrete-200", "level": "1",
                            "boundary": rectangle(x0) });
        s.edit(&anna, &project, &model, "floor.create", floor)
            .await
            .unwrap();
    }
    // A plain floor has none of the rows.
    let plain = quantities(s, &project, &model, "F-901").await;
    assert!(!plain.contains_key("floor.lowest") && !plain.contains_key("floor.top-area"));

    // A 3 in 4 fall along x: 4 m along, 3 m down; the top face's true area is 5/4 of the plan's.
    let slope = json!({ "element": "F-901", "slope": { "tail": ["60000", "0"],
                        "head": ["64000", "0"], "at_tail": "0", "gradient": { "rise": -3, "run": 4 } } });
    s.edit(&anna, &project, &model, "floor.slope", slope)
        .await
        .unwrap();
    let read = quantities(s, &project, &model, "F-901").await;
    assert_eq!(read["floor.top-area"].1.as_deref(), Some("15.00 m²"));
    assert_eq!(read["floor.highest"].1.as_deref(), Some("0 mm"));
    assert_eq!(read["floor.lowest"].1.as_deref(), Some("-3200 mm"));
    assert_eq!(read["floor.volume"].1.as_deref(), Some("2.40 m³"));
    assert_eq!(
        wall_bounds(s, &project, &model, "F-901").await,
        ([60_000, 0, -3200], [64_000, 3000, 0])
    );
    let filled = s
        .filled(&anna, &project, &model, &i("F-901"))
        .await
        .unwrap();
    let args = filled.recipe.unwrap().args;
    assert!(args.contains_key("tops") && !args.contains_key("edges"));
    let refused_both = s
        .edit(
            &anna,
            &project,
            &model,
            "floor.edge",
            json!({ "element": "F-901", "edge": "r0",
                    "profile": { "kind": "downturn", "width": "300 mm", "depth": "400 mm" } }),
        )
        .await;
    assert!(refused(refused_both));
    s.edit(
        &anna,
        &project,
        &model,
        "floor.slope.clear",
        json!({ "element": "F-901" }),
    )
    .await
    .unwrap();
    assert_eq!(quantities(s, &project, &model, "F-901").await, plain);

    // A downturn 4 m long, 300 mm wide and 400 mm deep: 0.48 m3.
    s.edit(
        &anna,
        &project,
        &model,
        "floor.edge",
        json!({ "element": "F-902", "edge": "r0",
                "profile": { "kind": "downturn", "width": "300 mm", "depth": "400 mm" } }),
    )
    .await
    .unwrap();
    let read = quantities(s, &project, &model, "F-902").await;
    assert_eq!(read["floor.edge-r0.length"].1.as_deref(), Some("4000 mm"));
    assert_eq!(read["floor.edge-r0.volume"].1.as_deref(), Some("0.48 m³"));
    let filled = s
        .filled(&anna, &project, &model, &i("F-902"))
        .await
        .unwrap();
    let args = filled.recipe.unwrap().args;
    assert_eq!(args["edges"].as_array().map(Vec::len), Some(1));
    assert_eq!(args["edges"][0]["id"], "r0");
    let refused_both = s
        .edit(
            &anna,
            &project,
            &model,
            "floor.slope",
            json!({ "element": "F-902", "slope": { "tail": ["70000", "0"], "head": ["74000", "0"],
                    "at_tail": "0", "at_head": "-40 mm" } }),
        )
        .await;
    assert!(refused(refused_both));
}

/// The seed draws its walls as a person does, layered walls of construct-walls' default types made
/// through the log, and writes no old box wall. The page reads a model's wall types with their
/// layers, the materials and the defaults, and an element's recipe filled as it is built, with how
/// far the log had got; the stand-in's displays draw each layer as a part. Someone off the team
/// reads neither.
#[tokio::test(flavor = "multi_thread")]
async fn the_seeds_walls_are_layered_and_the_page_reads_their_types_and_filled_recipes() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let types = s.wall_types(&anna, &project, &model).await.unwrap();
    let ids: Vec<&str> = types.types.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "wall-brick-veneer-90",
            "wall-concrete-200",
            "wall-partition-stud-90"
        ]
    );
    assert_eq!(types.defaults.len(), 3);
    assert!(types.materials.iter().any(|m| m.id == "timber-douglas-fir"));
    assert!(refused(s.wall_types(&dan, &project, &model).await));

    let mut snapshot = stack.store.snapshot().await.unwrap();
    let head: u64 = kit_store::read_row(&mut snapshot, &Scope::Project(project.clone()).head())
        .await
        .unwrap()
        .unwrap();
    for wall in ["W-101", "W-105", "W-201"] {
        let shown = s
            .shape(&anna, &project, &model, &i(wall), Some(0))
            .await
            .unwrap();
        let record = s
            .record(&anna, &project, &model, &shown.built.input)
            .await
            .unwrap();
        assert_eq!(record.recipe.op, "wall.layered", "{wall}");
        assert_eq!(record.recipe.args["layers"], json!({ "wall": "type" }));
        let filled = s.filled(&anna, &project, &model, &i(wall)).await.unwrap();
        assert!(filled.head >= head && filled.failed.is_none(), "{wall}");
        let recipe = filled.recipe.unwrap();
        let layers = recipe.args["layers"].as_array().unwrap();
        let parts = &shown.display.as_ref().unwrap().parts;
        let materials: Vec<&str> = parts.iter().map(|p| p.material.as_str()).collect();
        if wall == "W-105" {
            assert_eq!(layers.len(), 3);
            assert_eq!(
                materials,
                [
                    "plasterboard-gypsum",
                    "timber-douglas-fir",
                    "plasterboard-gypsum"
                ]
            );
        } else {
            // The wrap has no thickness, so it is a layer and no part.
            assert_eq!(layers.len(), 5);
            assert_eq!(
                materials,
                [
                    "brick-clay",
                    "air-dry",
                    "timber-douglas-fir",
                    "plasterboard-gypsum"
                ]
            );
        }
        assert!(recipe.args["height"].is_string(), "the top tie is filled");
    }
    // The bottom wall runs along x on its line 100 mm in, 250 mm thick about it, from level 1 to 2.
    let BuiltResult::Shape { bounds, .. } = s
        .shape(&anna, &project, &model, &i("W-101"), None)
        .await
        .unwrap()
        .built
        .result
    else {
        panic!("W-101 builds");
    };
    assert_eq!(
        (bounds.min, bounds.max),
        ([-25, -25, 0], [12_025, 225, 3000])
    );
    // A door's recipe reads nothing, so it is as stored.
    let door = s
        .filled(&anna, &project, &model, &i("D-101"))
        .await
        .unwrap();
    assert_eq!(door.recipe.unwrap().op, "door.single");
    assert!(refused(s.filled(&dan, &project, &model, &i("W-101")).await));
    assert!(matches!(
        s.filled(&anna, &project, &model, &i("W-999")).await,
        Err(Refusal::NotFound)
    ));
    // The seed writes no old box wall.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    for project in ["example-1", "riverside"] {
        let log = read_log(&mut snapshot, &Scope::Project(i(project)), 0, u64::MAX - 1)
            .await
            .unwrap();
        assert!(
            !log.iter()
                .any(|entry| entry.args.to_string().contains("wall.straight")),
            "{project}"
        );
    }
}

/// The page's views: the seed made one floor plan for each level of a model, and a view shows what
/// construct-views says over the stored shapes: what its cut plane crosses is cut, what lies below
/// is seen. Both are read behind the same access as the levels.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_lists_views_and_what_a_plan_shows() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));

    let listed = s.views(&anna, &project, &model).await.unwrap();
    assert_eq!(
        serde_json::to_value(&listed).unwrap(),
        json!([
            { "id": "VP-1", "name": "Level 1 plan", "kind": "floor_plan", "level": "1",
              "level_name": "Level 1", "scale": 100, "detail": "medium",
              "range": { "top": { "level": "1", "offset": "2300 mm" },
                         "cut": { "level": "1", "offset": "1200 mm" },
                         "bottom": { "level": "1", "offset": "0 mm" },
                         "depth": { "level": "1", "offset": "-500 mm" },
                         "heights": { "top": "2300 mm", "cut": "1200 mm", "bottom": "0 mm",
                                      "depth": "-500 mm" } },
              "crop": null, "template": null },
            { "id": "VP-2", "name": "Level 2 plan", "kind": "floor_plan", "level": "2",
              "level_name": "Level 2", "scale": 100, "detail": "medium",
              "range": { "top": { "level": "2", "offset": "2300 mm" },
                         "cut": { "level": "2", "offset": "1200 mm" },
                         "bottom": { "level": "2", "offset": "0 mm" },
                         "depth": { "level": "2", "offset": "-500 mm" },
                         "heights": { "top": "5300 mm", "cut": "4200 mm", "bottom": "3000 mm",
                                      "depth": "2500 mm" } },
              "crop": null, "template": null },
        ])
    );
    // A crop and a new range are read back with the view, as text a person reads.
    s.edit(
        &anna,
        &project,
        &model,
        "view.crop.set",
        json!({ "view": "VP-1", "region": { "outline": [
            { "x": "0", "y": "0" }, { "x": "4 m", "y": "0" }, { "x": "4 m", "y": "3 m" }] },
            "annotation": { "rectangle": { "min": { "x": "-1 m", "y": "-1 m" },
                                           "max": { "x": "5 m", "y": "4 m" } } } }),
    )
    .await
    .unwrap();
    s.edit(
        &anna,
        &project,
        &model,
        "view.crop.show",
        json!({ "view": "VP-1", "shown": false }),
    )
    .await
    .unwrap();
    s.edit(
        &anna,
        &project,
        &model,
        "view.set",
        json!({ "view": "VP-1", "range": { "cut": { "offset": "1.5 m" } } }),
    )
    .await
    .unwrap();
    let listed = serde_json::to_value(s.views(&anna, &project, &model).await.unwrap()).unwrap();
    assert_eq!(
        listed[0]["crop"],
        json!({ "region": { "shape": "outline", "corners": [
                    { "x": "0 mm", "y": "0 mm" }, { "x": "4000 mm", "y": "0 mm" },
                    { "x": "4000 mm", "y": "3000 mm" }] },
                "annotation": { "shape": "rectangle", "corners": [
                    { "x": "-1000 mm", "y": "-1000 mm" }, { "x": "5000 mm", "y": "-1000 mm" },
                    { "x": "5000 mm", "y": "4000 mm" }, { "x": "-1000 mm", "y": "4000 mm" }] },
                "on": true, "shown": false })
    );
    assert_eq!(listed[0]["range"]["cut"]["offset"], "1500 mm");
    assert_eq!(listed[0]["range"]["heights"]["cut"], "1500 mm");
    assert_eq!(listed[1]["crop"], json!(null));
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    let listed = serde_json::to_value(s.views(&anna, &project, &model).await.unwrap()).unwrap();
    assert_eq!(listed[0]["crop"], json!(null));
    assert_eq!(listed[0]["range"]["heights"]["cut"], "1200 mm");
    // Each model has the plans of its own levels: the riverside model stands on one.
    let riverside = s
        .views(&i("cara"), &i("riverside"), &i("riverside-model"))
        .await
        .unwrap();
    assert_eq!(riverside.len(), 1);
    assert_eq!(riverside[0].name, "Level 1 plan");
    assert!(refused(s.views(&dan, &project, &model).await));

    // Level 1 stands at 0 and Level 2 at 3000 mm, and a plan's cut plane is 1200 mm above its
    // level: Level 1's plan cuts the five walls and three doors there and sees nothing above its
    // top at 2300 mm; Level 2's cuts its five walls and four doors and sees Level 1's five walls,
    // whose tops (3000 mm) are not below its bottom. Level 1's doors are 2100 mm high, below its
    // view depth (2500 mm), so it does not show them.
    let level_1 = s.view(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert_eq!(
        serde_json::to_value(&level_1).unwrap(),
        json!({
            "view": level_1_summary(),
            "cut": ["D-101", "D-102", "D-103", "W-101", "W-102", "W-103", "W-104", "W-105"],
            "seen": [],
            "beyond": [],
        })
    );
    let level_2 = s.view(&anna, &project, &model, &i("VP-2")).await.unwrap();
    assert_eq!(
        level_2.cut,
        [
            "D-104", "D-201", "D-202", "D-203", "W-201", "W-202", "W-203", "W-204", "W-205"
        ]
    );
    assert_eq!(level_2.seen, ["W-101", "W-102", "W-103", "W-104", "W-105"]);
    assert_eq!(level_2.beyond, Vec::<String>::new());

    // A range moved through the log changes what is shown, and the views are read again: a
    // bottom at 200 mm over Level 2 is above Level 1's wall tops, so those are now beyond it.
    let range = json!({ "bottom": { "offset": "200" } });
    s.edit(
        &anna,
        &project,
        &model,
        "view.set",
        json!({ "view": "VP-2", "range": range }),
    )
    .await
    .unwrap();
    let level_2 = s.view(&anna, &project, &model, &i("VP-2")).await.unwrap();
    assert_eq!(level_2.cut.len(), 9);
    assert_eq!(level_2.seen, Vec::<String>::new());
    assert_eq!(
        level_2.beyond,
        ["W-101", "W-102", "W-103", "W-104", "W-105"]
    );

    // A view that is not there, and a person who may not read the model.
    let none = s.view(&anna, &project, &model, &i("VP-9")).await;
    assert!(matches!(none, Err(Refusal::NotFound)));
    let wall = s.view(&anna, &project, &model, &i("W-101")).await;
    assert!(matches!(wall, Err(Refusal::NotFound)));
    assert!(refused(s.view(&dan, &project, &model, &i("VP-1")).await));

    // The plans are views, not shapes: the model still publishes.
    let item = s
        .publish(&anna, &project, &i("arch"), &model)
        .await
        .unwrap();
    assert_eq!(item.name, "Arch package v12");
}

/// A view's graphics rules are read behind the same access as the views, and as the log's head
/// has them: none at first, then what `view.graphics.set` wrote, by category, with the category's
/// name. The fill patterns a rule may name are the drawing's own, no more and no fewer.
#[tokio::test(flavor = "multi_thread")]
async fn a_views_graphics_rules_are_read_as_the_log_has_them() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let (plan_1, plan_2) = (i("VP-1"), i("VP-2"));
    let summary = level_1_summary();

    let none = s.graphics(&anna, &project, &model, &plan_1).await.unwrap();
    let none = serde_json::to_value(&none).unwrap();
    assert_eq!(none["view"], summary);
    assert_eq!(none["rules"], json!([]));
    // What the page edits from: every category as it looks now, a category before its
    // subcategories, each edge with where it came from; the pens, patterns and fills a rule may
    // name. The views category is not there, and what cannot be cut has no cut.
    let looks = none["looks"].as_array().unwrap();
    let walls = looks.iter().find(|l| l["category"] == 100).unwrap();
    assert_eq!(walls["name"], "Walls");
    assert_eq!(walls["cut"]["origin"], "project");
    assert_eq!(walls["projection"]["style"]["shown"], true);
    assert!(
        walls["cut"]["width"].as_u64().unwrap() > walls["projection"]["width"].as_u64().unwrap()
    );
    let sub = looks.iter().position(|l| l["parent"] == 100).unwrap();
    assert!(looks.iter().position(|l| l["category"] == 100).unwrap() < sub);
    assert!(
        looks
            .iter()
            .all(|l| l["cut"].is_null() != l["cuttable"].as_bool().unwrap())
    );
    assert!(none["pens"].as_array().unwrap().len() > 1);
    assert!(none["annotation_pens"].as_array().unwrap().len() > 1);
    assert!(none["line_patterns"].as_array().unwrap().len() > 1);
    assert_eq!(none["fill_patterns"].as_array().unwrap().len(), 9);

    let hide = json!({ "shown": false, "transparency": 30 });
    let set = |view: &str, category: u32, edge: &str, style: Value| json!({ "view": view, "category": category, "edge": edge, "style": style });
    s.edit(
        &anna,
        &project,
        &model,
        "view.graphics.set",
        set("VP-1", 101, "cut", hide),
    )
    .await
    .unwrap();
    let doors = json!({ "halftone": true });
    s.edit(
        &anna,
        &project,
        &model,
        "view.graphics.set",
        set("VP-1", 300, "projection", doors),
    )
    .await
    .unwrap();
    let after =
        serde_json::to_value(s.graphics(&anna, &project, &model, &plan_1).await.unwrap()).unwrap();
    let rules = after["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0]["category"], 101);
    assert_eq!(rules[0]["parent"], 100);
    assert_eq!(rules[0]["projection"], Value::Null);
    assert_eq!(rules[0]["cut"]["shown"], false);
    assert_eq!(rules[0]["cut"]["transparency"], 30);
    assert_eq!(rules[0]["cut"]["fill"], Value::Null);
    assert_eq!(rules[1]["category"], 300);
    assert_eq!(rules[1]["name"], "Doors");
    assert_eq!(rules[1]["parent"], Value::Null);
    assert_eq!(rules[1]["cut"], Value::Null);
    assert_eq!(rules[1]["projection"]["halftone"], true);
    // Another view says nothing; a view that is not there, a wall, a person who may not read.
    let other = s.graphics(&anna, &project, &model, &plan_2).await.unwrap();
    assert_eq!(other.rules.len(), 0);
    let missing = s.graphics(&anna, &project, &model, &i("VP-9")).await;
    assert!(matches!(missing, Err(Refusal::NotFound)));
    let wall = s.graphics(&anna, &project, &model, &i("W-101")).await;
    assert!(matches!(wall, Err(Refusal::NotFound)));
    assert!(refused(s.graphics(&dan, &project, &model, &plan_1).await));
    // A refusal is the command's words, and the rules stay as they were.
    let bad = s
        .edit(
            &anna,
            &project,
            &model,
            "view.graphics.set",
            set("VP-1", 31, "cut", json!({})),
        )
        .await;
    assert!(matches!(bad, Err(Refusal::Refused(why)) if why == "There is no category 31."));
    let again = s.graphics(&anna, &project, &model, &plan_1).await.unwrap();
    assert_eq!(again.rules.len(), 2);

    // The fill patterns a rule may name are the drawing's own: each of them is taken, and the
    // refusal of any other lists exactly them.
    let drawing = construct_drawing::hatch::FillPatterns::default_metric();
    let names: Vec<&str> = drawing.patterns().iter().map(|p| p.name.as_str()).collect();
    for name in &names {
        let fill = json!({ "fill": { "pattern": name, "colour": [0, 0, 0] } });
        s.edit(
            &anna,
            &project,
            &model,
            "view.graphics.set",
            set("VP-2", 100, "cut", fill),
        )
        .await
        .unwrap();
    }
    let fill = json!({ "fill": { "pattern": "tartan", "colour": [0, 0, 0] } });
    let Err(Refusal::Refused(why)) = s
        .edit(
            &anna,
            &project,
            &model,
            "view.graphics.set",
            set("VP-2", 100, "cut", fill),
        )
        .await
    else {
        panic!("a fill pattern the drawing does not have was taken");
    };
    assert_eq!(
        why,
        format!(
            "There is no fill pattern \"tartan\". The fill patterns are {}.",
            names.join(", ")
        )
    );
}

/// A crop as the drawer was given it: the view, on, shown and the corners in grid steps.
type AskedCrop = (String, bool, bool, Vec<(i64, i64)>);

/// View templates, filters and overrides are read behind the same access as the views and as the
/// log's head has them: no template at first, then the one `view.template.create` made with what it
/// includes and the views tied to it; a template's graphics listed as a view's are; the filters of a
/// view or a template in their order, with the parameters a new one may name; and a view's
/// overrides with the element's name.
#[tokio::test(flavor = "multi_thread")]
async fn view_templates_filters_and_overrides_are_read_as_the_log_has_them() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let none = s.view_templates(&anna, &project, &model).await.unwrap();
    assert_eq!(none.len(), 0);
    let edit = |cmd: &'static str, args: Value| {
        let (anna, project, model) = (anna.clone(), project.clone(), model.clone());
        async move { s.edit(&anna, &project, &model, cmd, args).await }
    };
    edit(
        "view.template.create",
        json!({ "template": "VT-1", "name": "Working", "source": "VP-1" }),
    )
    .await
    .unwrap();
    edit(
        "view.template.tie",
        json!({ "view": "VP-2", "template": "VT-1" }),
    )
    .await
    .unwrap();
    let listed =
        serde_json::to_value(s.view_templates(&anna, &project, &model).await.unwrap()).unwrap();
    assert_eq!(listed[0]["id"], "VT-1");
    assert_eq!(listed[0]["name"], "Working");
    assert_eq!(listed[0]["scale"], 100);
    assert_eq!(
        listed[0]["included"],
        json!(["scale", "detail", "graphics", "categories", "filters"])
    );
    assert_eq!(listed[0]["views"], json!(["VP-2"]));
    let views = s.views(&anna, &project, &model).await.unwrap();
    let tied = views.iter().find(|view| view.id == "VP-2").unwrap();
    assert_eq!(tied.template.as_deref(), Some("VT-1"));
    assert!(
        views
            .iter()
            .find(|view| view.id == "VP-1")
            .unwrap()
            .template
            .is_none()
    );

    // The template's graphics, listed as a view's, and what it hides; the tied view shows them.
    edit(
        "view.graphics.set",
        json!({ "template": "VT-1", "category": 100, "edge": "cut",
                "style": { "shown": false } }),
    )
    .await
    .unwrap();
    edit(
        "view.categories.set",
        json!({ "template": "VT-1", "category": 300, "shown": false }),
    )
    .await
    .unwrap();
    let graphics = serde_json::to_value(
        s.template_graphics(&anna, &project, &model, &i("VT-1"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(graphics["template"]["name"], "Working");
    assert_eq!(graphics["rules"][0]["category"], 100);
    assert_eq!(graphics["rules"][0]["cut"]["shown"], false);
    assert_eq!(graphics["hidden"], json!([300]));
    let walls = graphics["looks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["category"] == 100)
        .unwrap();
    assert_eq!(walls["cut"]["origin"], "category");
    let through = s
        .graphics(&anna, &project, &model, &i("VP-2"))
        .await
        .unwrap();
    assert_eq!(through.rules.len(), 1);
    assert_eq!(through.hidden, [300]);
    let free = s
        .graphics(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    assert_eq!(free.rules.len(), 0);

    // Filters: the view that follows the template says so; a template and a view are both read at
    // the same path, and anything else is not found.
    let of_view = s
        .filters(&anna, &project, &model, &i("VP-2"))
        .await
        .unwrap();
    assert_eq!(of_view.of, "view");
    assert_eq!(of_view.follows_template.as_deref(), Some("Working"));
    assert_eq!(of_view.filters.len(), 0);
    assert_eq!(of_view.operators, ["equals", "contains", "greater_than"]);
    let of_template = s
        .filters(&anna, &project, &model, &i("VT-1"))
        .await
        .unwrap();
    assert_eq!(of_template.of, "template");
    assert!(of_template.follows_template.is_none());
    let missing = s.filters(&anna, &project, &model, &i("VP-9")).await;
    assert!(matches!(missing, Err(Refusal::NotFound)));
    let wall = s.filters(&anna, &project, &model, &i("W-101")).await;
    assert!(matches!(wall, Err(Refusal::NotFound)));

    // Overrides: one element in one view, with its name.
    edit(
        "view.override.set",
        json!({ "view": "VP-1", "element": "W-101", "appearance": { "halftone": true } }),
    )
    .await
    .unwrap();
    let overrides = s
        .overrides(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    assert_eq!(overrides.view.id, "VP-1");
    assert_eq!(overrides.overrides.len(), 1);
    assert_eq!(overrides.overrides[0].element, "W-101");
    assert!(overrides.overrides[0].name.is_some());
    assert_eq!(overrides.overrides[0].look.halftone, Some(true));
    let other = s
        .overrides(&anna, &project, &model, &i("VP-2"))
        .await
        .unwrap();
    assert_eq!(other.overrides.len(), 0);
    let missing = s.overrides(&anna, &project, &model, &i("VT-1")).await;
    assert!(matches!(missing, Err(Refusal::NotFound)));

    // Behind the same access as the views.
    assert!(refused(s.view_templates(&dan, &project, &model).await));
    assert!(refused(
        s.template_graphics(&dan, &project, &model, &i("VT-1"))
            .await
    ));
    assert!(refused(s.filters(&dan, &project, &model, &i("VP-1")).await));
    assert!(refused(
        s.overrides(&dan, &project, &model, &i("VP-1")).await
    ));
    let gone = s
        .template_graphics(&anna, &project, &model, &i("VT-9"))
        .await;
    assert!(matches!(gone, Err(Refusal::NotFound)));
}

/// A drawer that only records what the server asks of it, so the read path is tested without an
/// engine: the cut height and scale of the view, and the elements the plan cuts as the model
/// holds them.
#[derive(Default)]
struct Recording {
    asked: std::sync::Mutex<Vec<(Length, u32, Vec<ToDraw>)>>,
    /// Each request's crop: the view it is of and the crop's on, shown and corners, as text.
    crops: std::sync::Mutex<Vec<Option<AskedCrop>>>,
}

impl Drawer for Recording {
    fn draw(
        &self,
        cut: Length,
        scale: Scale,
        elements: &[ToDraw],
        _looks: Option<&dyn construct_drawing::look::Looks>,
        crop: Option<spare_server::plan::PlanCrop<'_>>,
    ) -> Result<Drawn, String> {
        self.crops.lock().unwrap().push(crop.map(|c| {
            (
                c.view.to_string(),
                c.crop.is_on(),
                c.crop.is_shown(),
                c.crop
                    .region()
                    .corners()
                    .iter()
                    .map(|p| (p.x.steps(), p.y.steps()))
                    .collect(),
            )
        }));
        self.asked
            .lock()
            .unwrap()
            .push((cut, scale.denominator(), elements.to_vec()));
        Ok(Drawn {
            svg: "<svg/>".to_owned(),
            skipped: Vec::new(),
        })
    }
}

/// The page asks the server for a plan: the server cuts it at the view's own height, hands the
/// drawer what that plane crosses with each recipe filled and each base's height added, and sends
/// back what the drawer drew with how far regeneration had got. A server with no drawer, a view
/// that is not there, and a person who may not read the model are each answered as for any read.
#[tokio::test(flavor = "multi_thread")]
async fn a_plan_is_the_drawers_picture_of_what_the_views_plane_cuts() {
    let (store, files) = (Memory::new(), MemoryFiles::default());
    let recording = Arc::new(Recording::default());
    let drawer: Arc<dyn Drawer> = recording.clone();
    let service = build(&store, &files).with_drawer(Some(drawer));
    seed(&store, &service).await.unwrap();
    let s = &service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));

    let one = s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    assert_eq!(one.svg, "<svg/>");
    assert_eq!(one.view.name, "Level 1 plan");
    assert_eq!(one.skipped, Vec::new());
    assert!(one.through > 0);
    let two = s.plan(&anna, &project, &model, &i("VP-2")).await.unwrap();
    assert_eq!(two.view.name, "Level 2 plan");

    {
        let asked = recording.asked.lock().unwrap();
        let (cut, scale, elements) = &asked[0];
        assert_eq!((*cut, *scale), (Length::from_mm(1200), 100));
        let mut ids: Vec<String> = elements.iter().map(|e| e.id.to_string()).collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "D-101", "D-102", "D-103", "W-101", "W-102", "W-103", "W-104", "W-105"
            ]
        );
        let wall = elements.iter().find(|e| e.id.as_str() == "W-101").unwrap();
        assert_eq!(wall.recipe.op, "wall.layered");
        assert!(
            wall.recipe.args["layers"].is_array(),
            "the layers are read from the wall's type: {:?}",
            wall.recipe.args
        );
        assert_eq!(wall.placement.z, 0);
        // Level 2 is cut 1200 mm above its own level, which stands at 3000 mm, and what stands on it
        // is drawn from that base.
        let (cut, _, elements) = &asked[1];
        assert_eq!(*cut, Length::from_mm(4200));
        let wall = elements.iter().find(|e| e.id.as_str() == "W-201").unwrap();
        assert_eq!(wall.placement.z, 3000);
    }

    // A view with no crop passes none; a crop is passed with its view, switches and corners.
    assert_eq!(*recording.crops.lock().unwrap(), [None, None]);
    s.edit(
        &anna,
        &project,
        &model,
        "view.crop.set",
        json!({ "view": "VP-1", "region": { "rectangle": {
            "min": { "x": "0", "y": "0" }, "max": { "x": "6 m", "y": "4 m" } } } }),
    )
    .await
    .unwrap();
    s.plan(&anna, &project, &model, &i("VP-1")).await.unwrap();
    let (right, top) = (6000 * 6400, 4000 * 6400);
    assert_eq!(
        recording.crops.lock().unwrap().last().cloned().unwrap(),
        Some((
            "VP-1".to_owned(),
            true,
            true,
            vec![(0, 0), (right, 0), (right, top), (0, top)]
        ))
    );

    // Only floor plans are drawn yet: a three-dimensional view says so.
    s.edit(
        &anna,
        &project,
        &model,
        "view.create",
        json!({ "view": "V3", "kind": "three_d", "name": "House in 3D" }),
    )
    .await
    .unwrap();
    let Err(Refusal::Bad(why)) = s.plan(&anna, &project, &model, &i("V3")).await else {
        panic!("a 3D view is not drawn as a plan");
    };
    assert!(why.contains("only floor plans are drawn yet"), "{why}");

    let none = s.plan(&anna, &project, &model, &i("VP-9")).await;
    assert!(matches!(none, Err(Refusal::NotFound)));
    let wall = s.plan(&anna, &project, &model, &i("W-101")).await;
    assert!(matches!(wall, Err(Refusal::NotFound)));
    assert!(refused(s.plan(&dan, &project, &model, &i("VP-1")).await));
}

/// Without the kernel a server cannot cut a plan, and says so in words the page shows.
#[tokio::test(flavor = "multi_thread")]
async fn a_server_without_a_drawer_says_it_cannot_draw_plans() {
    let stack = seeded().await;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let Err(Refusal::Bad(why)) = stack
        .service
        .plan(&anna, &project, &model, &i("VP-1"))
        .await
    else {
        panic!("a plan needs a drawer");
    };
    assert_eq!(
        why,
        "This server was built without the geometry kernel, so it cannot draw plans."
    );
}

/// Through the server's commands on the stand-in: the default roof types made, a gable roof over a
/// rectangle drawn on level 1 of the steel type, built as the box around its eaves; its parameters
/// view lists its area, slope area, pitch, ridge height, thickness, volume and mass, and each
/// layer's volume and mass, in units and read-only; its pitch changed, then undone, changes them;
/// a person off the team can't draw one; a flat roof the form's change makes has no pitch to
/// list; and it is deleted by its own command.
#[tokio::test(flavor = "multi_thread")]
async fn a_roof_drawn_by_command_shows_its_quantities_in_units() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    // The page's roof tool reads the roof types: none until it sends the defaults.
    let none = s.roof_types(&anna, &project, &model).await.unwrap();
    assert!(none.types.is_empty());
    assert!(none.materials.iter().any(|m| m.id == "concrete-regular"));
    assert!(refused(s.roof_types(&dan, &project, &model).await));
    s.edit(&anna, &project, &model, "roof.defaults", json!({}))
        .await
        .unwrap();
    let types = s.roof_types(&anna, &project, &model).await.unwrap();
    let ids: Vec<&str> = types.types.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        ["roof-concrete-200-insulated", "roof-steel-rafter-150"]
    );
    let steel = types
        .types
        .iter()
        .find(|t| t.id == "roof-steel-rafter-150")
        .unwrap();
    assert_eq!(steel.layers.as_array().map(Vec::len), Some(3));
    let roof = json!({ "element": "R-900", "type": "roof-concrete-200-insulated",
                       "level": "1", "form": "gable", "pitch": "1:4",
                       "footprint": { "outer": [corner("r0", 60_000, 0), corner("r1", 68_000, 0),
                                                corner("r2", 68_000, 6000),
                                                corner("r3", 60_000, 6000)] } });
    assert!(refused(
        s.edit(&dan, &project, &model, "roof.create", roof.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "roof.create", roof)
        .await
        .unwrap();
    let view = serde_json::to_value(
        s.parameters(&anna, &project, &model, &i("R-900"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(view["category"], json!({ "id": 600, "name": "Roofs" }));

    let read = quantities(s, &project, &model, "R-900").await;
    let ids: Vec<&str> = read.keys().map(String::as_str).collect();
    assert_eq!(
        ids,
        [
            "roof.area",
            "roof.layer-1.mass",
            "roof.layer-1.volume",
            "roof.layer-2.mass",
            "roof.layer-2.volume",
            "roof.mass",
            "roof.pitch",
            "roof.ridge-height",
            "roof.slope-area",
            "roof.thickness",
            "roof.volume"
        ]
    );
    assert_eq!(read["roof.area"].1.as_deref(), Some("48.00 m²"));
    assert_eq!(read["roof.ridge-height"].1.as_deref(), Some("1059 mm"));
    assert_eq!(read["roof.thickness"].1.as_deref(), Some("300 mm"));
    assert!(
        read["roof.mass"].0.is_some(),
        "both materials are in the library"
    );
    let flatter = read["roof.slope-area"].1.clone();

    s.edit(
        &anna,
        &project,
        &model,
        "roof.pitch",
        json!({ "element": "R-900", "pitch": "1:2" }),
    )
    .await
    .unwrap();
    let steeper = quantities(s, &project, &model, "R-900").await;
    assert_eq!(
        steeper["roof.area"], read["roof.area"],
        "the plan does not change"
    );
    assert_ne!(steeper["roof.ridge-height"], read["roof.ridge-height"]);
    assert_ne!(steeper["roof.slope-area"].1, flatter);
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    assert_eq!(quantities(s, &project, &model, "R-900").await, read);

    // Flat: no pitch and no ridge to list.
    s.edit(
        &anna,
        &project,
        &model,
        "roof.form",
        json!({ "element": "R-900", "form": "flat" }),
    )
    .await
    .unwrap();
    let flat = quantities(s, &project, &model, "R-900").await;
    assert!(!flat.contains_key("roof.pitch") && !flat.contains_key("roof.ridge-height"));
    assert_eq!(flat["roof.area"], read["roof.area"]);

    s.edit(
        &anna,
        &project,
        &model,
        "roof.delete",
        json!({ "element": "R-900" }),
    )
    .await
    .unwrap();
    assert!(
        s.parameters(&anna, &project, &model, &i("R-900"))
            .await
            .is_err()
    );
}

/// A room placed in the seed's building: the server's views of it say what became of it, in the
/// document's units. The filled view carries the loops and the room's state and measures, its
/// parameters view lists them read-only, the next free number is offered; a wall deleted makes it
/// not enclosed, with its words and no area, and the same walls restored enclose it again.
#[tokio::test(flavor = "multi_thread")]
async fn a_room_in_the_seeded_building_reports_its_state_and_measures() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    assert_eq!(
        s.rooms(&anna, &project, &model).await.unwrap().next_number,
        "1"
    );
    assert!(refused(s.rooms(&dan, &project, &model).await));
    let place = json!({ "element": "R-900", "name": "Studio", "level": "1", "at": ["3 m", "4 m"],
                        "top": "2" });
    assert!(refused(
        s.edit(&dan, &project, &model, "room.place", place.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "room.place", place)
        .await
        .unwrap();
    assert_eq!(
        s.rooms(&anna, &project, &model).await.unwrap().next_number,
        "2"
    );

    let filled = s
        .filled(&anna, &project, &model, &i("R-900"))
        .await
        .unwrap();
    let recipe = filled.recipe.unwrap();
    assert_eq!(recipe.args["loops"].as_array().unwrap().len(), 1);
    let room = filled.room.unwrap();
    assert_eq!((room.state, room.words.as_str()), ("enclosed", ""));
    let area = room.area.clone().unwrap();
    assert!(area.ends_with(" m²"), "{area}");
    let read = quantities(s, &project, &model, "R-900").await;
    assert_eq!(read["room.area"].1.as_deref(), Some(area.as_str()));
    assert_eq!(read["room.state"].1.as_deref(), Some("enclosed"));
    assert_eq!(read["room.perimeter"].1, room.perimeter);
    assert_eq!(read["room.volume"].1, room.volume);

    // The left outer wall goes: the room stays and says why it has no area.
    s.edit(
        &anna,
        &project,
        &model,
        "element.delete",
        json!({ "element": "W-103" }),
    )
    .await
    .unwrap();
    let open = s
        .filled(&anna, &project, &model, &i("R-900"))
        .await
        .unwrap();
    assert!(open.recipe.is_none());
    let room = open.room.unwrap();
    assert_eq!(room.state, "not enclosed");
    assert!(
        room.area.is_none() && room.words.contains("is not enclosed"),
        "{room:?}"
    );
    let read = quantities(s, &project, &model, "R-900").await;
    assert_eq!(
        read.keys().map(String::as_str).collect::<Vec<_>>(),
        ["room.state"]
    );
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    let back = s
        .filled(&anna, &project, &model, &i("R-900"))
        .await
        .unwrap();
    assert_eq!(back.room.unwrap().area, Some(area));
}

/// The identities of a model's warnings now, in the order the server gives them.
async fn warning_identities(
    s: &Service<Memory, MemoryFiles>,
    project: &Id,
    model: &Id,
) -> Vec<String> {
    s.warnings(&i("anna"), project, model)
        .await
        .unwrap()
        .warnings
        .into_iter()
        .map(|w| w.identity)
        .collect()
}

/// The server's list of a model's problems is worked out from the document each time it is read:
/// a person off the team cannot read it; a room whose wall is deleted is listed not enclosed, with
/// its words, and is gone again when the wall is back (by undo); a wall drawn far away leaves it
/// as it was; a second wall drawn along the first is listed as an overlap, both walls named, and
/// leaves when it is deleted; the same list comes back for the same document.
#[tokio::test(flavor = "multi_thread")]
async fn the_warnings_view_lists_a_room_not_enclosed_and_walls_that_overlap() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    assert!(refused(s.warnings(&dan, &project, &model).await));
    let edit = |cmd: &'static str, args: Value| {
        let (s, anna, project, model) = (s, &anna, &project, &model);
        async move { s.edit(anna, project, model, cmd, args).await.unwrap() }
    };
    let base = warning_identities(s, &project, &model).await;
    assert!(
        base.iter().all(|w| !w.starts_with("room.")),
        "the seeded building has no rooms: {base:?}"
    );
    edit(
        "room.place",
        json!({ "element": "R-900", "name": "Studio", "number": "101", "level": "1",
                "at": ["3 m", "4 m"], "top": "2" }),
    )
    .await;
    assert_eq!(warning_identities(s, &project, &model).await, base);

    edit("element.delete", json!({ "element": "W-103" })).await;
    let open = s.warnings(&anna, &project, &model).await.unwrap();
    let room: Vec<_> = open
        .warnings
        .iter()
        .filter(|w| w.kind == "room.not_enclosed")
        .collect();
    assert_eq!(room.len(), 1, "{open:?}");
    assert_eq!(room[0].identity, "room.not_enclosed|R-900");
    assert_eq!(room[0].elements, ["R-900"]);
    assert_eq!(room[0].severity, "warning");
    assert_eq!(
        room[0].words,
        "Room 101 Studio is not enclosed: the walls round its point do not close"
    );
    let open_list = warning_identities(s, &project, &model).await;
    assert_eq!(open_list.len(), base.len() + 1);

    // An unrelated edit: two walls far away, one along the other.
    for ty in wall_types() {
        edit("wall.type.create", ty).await;
    }
    let draw = |element: &str, from: &str, to: &str| {
        json!({ "element": element, "type": "wall-partition", "start": [from, "0"],
                "end": [to, "0"], "level": "1", "top": "2" })
    };
    edit("wall.draw", draw("W-900", "50 m", "54 m")).await;
    assert_eq!(warning_identities(s, &project, &model).await, open_list);
    edit("wall.draw", draw("W-901", "52 m", "56 m")).await;
    let both = warning_identities(s, &project, &model).await;
    assert_eq!(both.len(), open_list.len() + 1);
    assert!(both.contains(&"room.not_enclosed|R-900".to_owned()));
    assert!(
        both.contains(&"wall.overlap|W-900|W-901".to_owned()),
        "{both:?}"
    );
    let mut sorted = both.clone();
    sorted.sort();
    assert_eq!(both, sorted, "by kind, then by elements");
    assert_eq!(warning_identities(s, &project, &model).await, both);

    // The cause fixed: the second wall is deleted, then the first wall of the room comes back.
    edit("element.delete", json!({ "element": "W-901" })).await;
    assert_eq!(warning_identities(s, &project, &model).await, open_list);
    edit("edit.undo", json!({})).await;
    assert_eq!(warning_identities(s, &project, &model).await, both);
    edit("edit.redo", json!({})).await;
    assert_eq!(warning_identities(s, &project, &model).await, open_list);
    // Undone step by step, the wall comes back last and the room is enclosed again.
    for _ in 0..8 {
        edit("edit.undo", json!({})).await;
        if !warning_identities(s, &project, &model)
            .await
            .contains(&"room.not_enclosed|R-900".to_owned())
        {
            break;
        }
    }
    assert_eq!(warning_identities(s, &project, &model).await, base);
}

/// The page's schedules: made and changed through the log by whoever may edit the model, read by
/// whoever may read it. A schedule lists the seeded building's doors by name, and its rows are
/// worked out from the elements on every read, so a door deleted through the log leaves them.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_lists_schedules_and_their_live_rows() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let none = s.schedules(&anna, &project, &model).await.unwrap();
    assert_eq!(none.len(), 0);
    assert!(refused(s.schedules(&dan, &project, &model).await));

    let make = json!({ "schedule": "SC-1", "name": "Doors", "category": 300,
                       "fields": ["name", "type"],
                       "sorting": [{ "field": "name", "order": "descending" }] });
    assert!(refused(
        s.edit(&dan, &project, &model, "schedule.create", make.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "schedule.create", make)
        .await
        .unwrap();
    let listed = s.schedules(&anna, &project, &model).await.unwrap();
    assert_eq!(
        serde_json::to_value(&listed).unwrap(),
        json!([{ "id": "SC-1", "name": "Doors", "category": 300, "category_name": "Doors",
                 "fields": ["name", "type"], "filters": [],
                 "sorting": [{ "field": "name", "order": "descending" }], "problem": null }])
    );

    let table = s
        .schedule(&anna, &project, &model, &i("SC-1"))
        .await
        .unwrap();
    assert_eq!(table.rows.len(), 7, "the seeded building's doors");
    // The fields the category offers come with their kinds and the comparisons that fit them.
    let offered = |key: &str| table.offered.iter().find(|field| field.field == key);
    let name = offered("name").expect("a door's name is offered");
    assert_eq!((name.name.as_str(), name.kind), ("Name", "text"));
    assert!(name.comparisons.contains(&"contains"));
    assert!(offered("type").is_some());
    // The categories a schedule may list include the doors, and leave out the views.
    let listable = s
        .schedule_categories(&anna, &project, &model)
        .await
        .unwrap();
    assert!(
        listable
            .iter()
            .any(|c| (c.id, c.name.as_str()) == (300, "Doors"))
    );
    assert!(listable.iter().all(|c| c.name != "Views"));
    assert!(refused(s.schedule_categories(&dan, &project, &model).await));
    let names: Vec<String> = table
        .rows
        .iter()
        .map(|row| row.cells[0].shown.clone().unwrap().to_lowercase())
        .collect();
    let mut descending = names.clone();
    descending.sort_by(|a, b| b.cmp(a));
    assert_eq!(names, descending);
    assert!(table.rows.iter().any(|row| row.element == "D-101"));

    // The same read after an edit through the log gives the new rows: nothing was stored.
    s.edit(
        &anna,
        &project,
        &model,
        "element.delete",
        json!({ "element": "D-101" }),
    )
    .await
    .unwrap();
    let table = s
        .schedule(&anna, &project, &model, &i("SC-1"))
        .await
        .unwrap();
    assert_eq!(table.rows.len(), 6);
    assert!(table.rows.iter().all(|row| row.element != "D-101"));

    // A filter through the command, in typed text; the generic command is no way round it.
    let filter = json!({ "schedule": "SC-1",
        "filters": [{ "field": "name", "is": "begins_with", "to": "zzz" }] });
    s.edit(&anna, &project, &model, "schedule.filters", filter)
        .await
        .unwrap();
    let table = s
        .schedule(&anna, &project, &model, &i("SC-1"))
        .await
        .unwrap();
    assert_eq!(table.rows.len(), 0);
    let hacked = s
        .edit(
            &anna,
            &project,
            &model,
            "element.set",
            json!({ "element": "SC-1", "name": "Hacked" }),
        )
        .await;
    assert!(refused(hacked));

    // A schedule that is not there, an element that is not a schedule, a person who may not read.
    let none = s.schedule(&anna, &project, &model, &i("SC-9")).await;
    assert!(matches!(none, Err(Refusal::NotFound)));
    let wall = s.schedule(&anna, &project, &model, &i("W-101")).await;
    assert!(matches!(wall, Err(Refusal::NotFound)));
    assert!(refused(
        s.schedule(&dan, &project, &model, &i("SC-1")).await
    ));

    // A schedule has no shape: the model still publishes, and replays from the log.
    let item = s
        .publish(&anna, &project, &i("arch"), &model)
        .await
        .unwrap();
    assert_eq!(item.name, "Arch package v12");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_page_lists_sheets_and_where_their_viewports_lie_on_the_paper() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: Value| {
        let (anna, project, model) = (anna.clone(), project.clone(), model.clone());
        async move { s.edit(&anna, &project, &model, cmd, args).await }
    };
    assert_eq!(s.sheets(&anna, &project, &model).await.unwrap().len(), 0);

    edit(
        "sheet.create",
        json!({ "sheet": "SH-1", "number": "A101", "name": "Ground floor plan" }),
    )
    .await
    .unwrap();
    edit(
        "sheet.create",
        json!({ "sheet": "SH-2", "number": "A102", "name": "First floor plan",
                "paper": { "size": "a3", "orientation": "portrait" } }),
    )
    .await
    .unwrap();
    edit(
        "viewport.create",
        json!({ "viewport": "PT-1", "sheet": "SH-1", "view": "VP-1", "x": "300 mm", "y": "300 mm" }),
    )
    .await
    .unwrap();
    edit(
        "viewport.create",
        json!({ "viewport": "PT-2", "sheet": "SH-1", "view": "VP-2", "x": "600 mm", "y": "300 mm" }),
    )
    .await
    .unwrap();

    let listed = s.sheets(&anna, &project, &model).await.unwrap();
    assert_eq!(
        serde_json::to_value(&listed).unwrap(),
        json!([
            { "id": "SH-1", "number": "A101", "name": "Ground floor plan", "paper": "a1",
              "orientation": "landscape", "width": "length:q0:5382400", "height": "length:q0:3801600",
              "shown": "A1 landscape (841 mm by 594 mm)", "viewports": 2 },
            { "id": "SH-2", "number": "A102", "name": "First floor plan", "paper": "a3",
              "orientation": "portrait", "width": "length:q0:1900800", "height": "length:q0:2688000",
              "shown": "A3 portrait (297 mm by 420 mm)", "viewports": 0 },
        ])
    );

    // Each viewport takes its view's scale and an outline from what the plan shows.
    let first = s.sheet(&anna, &project, &model, &i("SH-1")).await.unwrap();
    let shown = serde_json::to_value(&first).unwrap();
    assert_eq!(shown["sheet"]["number"], "A101");
    let ports = shown["viewports"].as_array().unwrap();
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0]["id"], "PT-1");
    assert_eq!(ports[0]["view_name"], "Level 1 plan");
    assert_eq!(ports[0]["scale"], 100);
    assert_eq!(ports[0]["x"], "length:q0:1920000");
    assert_eq!(ports[0]["title"], true);
    // The plan shows 12,050 by 8,050 mm of walls at 1:100: 120.5 by 80.5 mm on the paper, about the
    // centre.
    assert_eq!(
        ports[0]["outline"],
        json!({ "min_x": "length:q0:1534400", "min_y": "length:q0:1662400",
                "max_x": "length:q0:2305600", "max_y": "length:q0:2177600" })
    );
    assert_eq!(ports[0]["on_paper"], true);

    // An alignment is data: the second viewport's left edge follows the first's, and the answer
    // is worked from the views' extents every time it is asked.
    edit(
        "viewport.align",
        json!({ "viewport": "PT-2", "across": { "to": "PT-1", "anchor": "min" } }),
    )
    .await
    .unwrap();
    let again = s.sheet(&anna, &project, &model, &i("SH-1")).await.unwrap();
    let again = serde_json::to_value(&again).unwrap();
    let ports = again["viewports"].as_array().unwrap();
    assert_eq!(ports[0]["outline"]["min_x"], ports[1]["outline"]["min_x"]);
    assert_eq!(
        ports[1]["align_x"],
        json!({ "to": "PT-1", "anchor": "min" })
    );

    // A view that is placed is not deleted, and the record is read back as it was written.
    assert!(refused(
        s.edit(
            &anna,
            &project,
            &model,
            "view.delete",
            json!({ "view": "VP-1" })
        )
        .await
    ));
    assert!(refused(s.sheets(&dan, &project, &model).await));
    assert!(matches!(
        s.sheet(&anna, &project, &model, &i("SH-9")).await,
        Err(Refusal::NotFound)
    ));
    assert!(matches!(
        s.sheet(&anna, &project, &model, &i("VP-1")).await,
        Err(Refusal::NotFound)
    ));
}

/// A viewport on a sheet reads its view's crop for its size: while the crop is on, the outline is
/// the crop's box at the view's scale (exact, not rounded out to whole millimetres), whether or not
/// the crop is shown; switched off or cleared, it is what the plan shows again.
#[tokio::test(flavor = "multi_thread")]
async fn a_viewports_outline_is_its_views_crop_while_the_crop_is_on() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: Value| {
        let (anna, project, model) = (anna.clone(), project.clone(), model.clone());
        async move { s.edit(&anna, &project, &model, cmd, args).await }
    };
    edit(
        "sheet.create",
        json!({ "sheet": "SH-1", "number": "A101", "name": "Ground floor plan" }),
    )
    .await
    .unwrap();
    edit(
        "viewport.create",
        json!({ "viewport": "PT-1", "sheet": "SH-1", "view": "VP-1", "x": "300 mm", "y": "300 mm" }),
    )
    .await
    .unwrap();
    let outline = || async {
        let sheet = s.sheet(&anna, &project, &model, &i("SH-1")).await.unwrap();
        serde_json::to_value(&sheet).unwrap()["viewports"][0]["outline"].clone()
    };
    let plain = outline().await;
    assert_eq!(plain["min_x"], "length:q0:1534400");

    // A crop of 6 m by 4 m at 1:100 is 60 by 40 mm on the paper, about the viewport's centre.
    edit(
        "view.crop.set",
        json!({ "view": "VP-1", "region": { "rectangle": {
            "min": { "x": "0", "y": "0" }, "max": { "x": "6 m", "y": "4 m" } } } }),
    )
    .await
    .unwrap();
    let cropped = json!({ "min_x": "length:q0:1728000", "min_y": "length:q0:1792000",
                          "max_x": "length:q0:2112000", "max_y": "length:q0:2048000" });
    assert_eq!(outline().await, cropped);
    // A shaped crop's box, and hiding the outline, change nothing about the size.
    edit("view.crop.show", json!({ "view": "VP-1", "shown": false }))
        .await
        .unwrap();
    assert_eq!(outline().await, cropped);
    // Off, and cleared, the plan's own extent is back.
    edit("view.crop.set", json!({ "view": "VP-1", "on": false }))
        .await
        .unwrap();
    assert_eq!(outline().await, plain);
    edit("view.crop.set", json!({ "view": "VP-1", "on": true }))
        .await
        .unwrap();
    assert_eq!(outline().await, cropped);
    edit("view.crop.clear", json!({ "view": "VP-1" }))
        .await
        .unwrap();
    assert_eq!(outline().await, plain);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_page_reads_a_views_dimensions_with_their_values_worked_out() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: Value| {
        let (anna, project, model) = (anna.clone(), project.clone(), model.clone());
        async move { s.edit(&anna, &project, &model, cmd, args).await }
    };
    let none = s
        .dimensions(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    assert_eq!(none.view_name, "Level 1 plan");
    assert_eq!(none.dimensions.len(), 0);

    // Across the first floor at mid height: the east wall's line, the partition's faces and line,
    // and the west wall's line (the seed draws them at x = 100, 6100 and 11900 mm).
    let wall = |id: &str, feature: &str| json!({ "element": id, "feature": feature });
    edit(
        "dimension.create",
        json!({ "dimension": "DM-1", "view": "VP-1",
                "layout": { "kind": "horizontal", "y": "4000" },
                "references": [
                    wall("W-103", "wall.line"),
                    wall("W-105", "wall.face.interior"),
                    wall("W-105", "wall.face.exterior"),
                    wall("W-104", "wall.line"),
                ] }),
    )
    .await
    .unwrap();
    let listed = s
        .dimensions(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    let shown = serde_json::to_value(&listed).unwrap();
    let dimension = &shown["dimensions"][0];
    assert_eq!(dimension["id"], "DM-1");
    assert_eq!(dimension["layout"]["kind"], "horizontal");
    assert_eq!(dimension["layout"]["y"], "length:q0:25600000");
    let segments = dimension["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 3);
    // Its west face is 57.5 mm before 6100 and its east face 57.5 mm after: the distances run
    // 5942.5, 115 and 5742.5 mm.
    assert_eq!(segments[1]["text"], "115 mm");
    assert_eq!(segments[1]["value"], "length:q0:736000");
    assert_eq!(segments[1]["exact"], true);
    let whole: i64 = [0, 2]
        .iter()
        .map(|at| {
            let text = segments[*at]["value"].as_str().unwrap();
            text.strip_prefix("length:q0:")
                .unwrap()
                .parse::<i64>()
                .unwrap()
        })
        .sum::<i64>()
        + 736_000;
    assert_eq!(whole, 11_800 * 6400);
    assert_eq!(dimension["overall_text"], "11800 mm");
    assert_eq!(dimension["warnings"], json!([]));
    assert_eq!(
        dimension["references"][0]["words"],
        "the reference line of W-103"
    );

    // A wall is deleted: the dimension is not touched, and names the reference that is gone.
    edit("element.delete", json!({ "element": "W-105" }))
        .await
        .unwrap();
    let after = s
        .dimensions(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    let after = serde_json::to_value(&after).unwrap();
    let dimension = &after["dimensions"][0];
    assert_eq!(dimension["segments"][1]["value"], Value::Null);
    assert_eq!(dimension["overall"], Value::Null);
    assert_eq!(
        dimension["warnings"][0]["identity"],
        "dimension.reference_missing|DM-1|W-105"
    );
    assert_eq!(dimension["references"][1]["usable"], false);
    // The undo gives the wall, and the numbers, back.
    edit("edit.undo", json!({})).await.unwrap();
    let back = s
        .dimensions(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), shown);

    assert!(refused(
        s.dimensions(&dan, &project, &model, &i("VP-1")).await
    ));
    assert!(matches!(
        s.dimensions(&anna, &project, &model, &i("VP-9")).await,
        Err(Refusal::NotFound)
    ));
}

/// A stair made by command in the seeded building, from level 1 to level 2 (3000 mm): a common
/// stairway in two runs with a landing between, 16 risers of 187.5 mm, 8 to a run, on 280 mm
/// goings. The stairs view lists it with its layout and the rule set's categories; its parameters
/// view reads its quantities back read-only; a person off the team can neither make one nor read
/// the view; deleted, it is gone from the view.
#[tokio::test(flavor = "multi_thread")]
async fn a_stair_made_by_command_is_laid_out_and_read_back() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let none = s.stairs(&anna, &project, &model).await.unwrap();
    assert_eq!(none.stairs.len(), 0);
    assert!(none.categories.iter().any(|c| c.id == "common"));
    assert!(refused(s.stairs(&dan, &project, &model).await));
    let create = json!({ "element": "S-900", "name": "Stair", "level": "1", "top": "2",
                         "start": ["20 m", "0"], "runs": [{}, {}],
                         "landings": [{ "length": "1 m", "turn": "back-left" }] });
    assert!(refused(
        s.edit(&dan, &project, &model, "stair.create", create.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "stair.create", create)
        .await
        .unwrap();
    let view = s.stairs(&anna, &project, &model).await.unwrap();
    let layout = view.stairs[0].layout.as_ref().unwrap();
    assert_eq!(layout.risers, 16);
    assert_eq!(layout.rise.value, "length:q0:19200000");
    assert_eq!(layout.riser_height.value, "length:q0:1200000");
    assert_eq!(
        layout.runs.iter().map(|r| r.risers).collect::<Vec<_>>(),
        vec![8, 8]
    );
    let read = quantities(s, &project, &model, "S-900").await;
    assert_eq!(read["stair.risers"].1.as_deref(), Some("16"));
    assert_eq!(read["stair.going"].1.as_deref(), Some("280 mm"));
    assert_eq!(read["stair.treads"].1.as_deref(), Some("14"));
    assert_eq!(
        read["stair.riser-height"].0.as_deref(),
        Some("length:q0:1200000")
    );
    s.edit(
        &anna,
        &project,
        &model,
        "stair.delete",
        json!({ "element": "S-900" }),
    )
    .await
    .unwrap();
    assert_eq!(
        s.stairs(&anna, &project, &model)
            .await
            .unwrap()
            .stairs
            .len(),
        0
    );
}

/// Through the server's commands on the stand-in: the page's curtain wall tool reads the types
/// (none, then the three defaults once it sends them), a person off the team can read none; a 6 m
/// by 3 m curtain wall of the 1.5 m by 1 m type drawn on level 1 is built as the box round its
/// mullions, and its parameters view lists its length, height, area, panels and their area, glazed
/// area, and mullions and their length, in units and read-only; a grid line added changes them,
/// and undone puts them back.
#[tokio::test(flavor = "multi_thread")]
async fn a_curtain_wall_drawn_by_command_shows_its_quantities_in_units() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let none = s.curtain_types(&anna, &project, &model).await.unwrap();
    assert!(none.types.is_empty());
    assert_eq!(none.defaults.len(), 3);
    assert!(none.materials.iter().any(|m| m.id == "glass-plate"));
    assert!(refused(s.curtain_types(&dan, &project, &model).await));
    s.edit(&anna, &project, &model, "curtain.defaults", json!({}))
        .await
        .unwrap();
    let types = s.curtain_types(&anna, &project, &model).await.unwrap();
    let ids: Vec<&str> = types.types.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        ["curtain-1500x1000", "curtain-glazing", "curtain-storefront"]
    );
    let wall = json!({ "element": "CW-900", "type": "curtain-1500x1000", "level": "1",
                       "start": ["60 m", "0"], "end": ["66 m", "0"], "height": "3 m" });
    assert!(refused(
        s.edit(&dan, &project, &model, "curtain.draw", wall.clone())
            .await
    ));
    s.edit(&anna, &project, &model, "curtain.draw", wall)
        .await
        .unwrap();
    let view = serde_json::to_value(
        s.parameters(&anna, &project, &model, &i("CW-900"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        view["category"],
        json!({ "id": 200, "name": "Curtain walls" })
    );
    let read = quantities(s, &project, &model, "CW-900").await;
    let ids: Vec<&str> = read.keys().map(String::as_str).collect();
    assert_eq!(
        ids,
        [
            "curtain.area",
            "curtain.glazed-area",
            "curtain.height",
            "curtain.length",
            "curtain.mullion-length",
            "curtain.mullions",
            "curtain.panel-area",
            "curtain.panels",
        ]
    );
    assert_eq!(read["curtain.panels"].0.as_deref(), Some("number:12"));
    assert_eq!(read["curtain.mullions"].0.as_deref(), Some("number:31"));
    assert_eq!(read["curtain.panel-area"].1.as_deref(), Some("16.10 m²"));
    assert_eq!(read["curtain.area"].1.as_deref(), Some("18.00 m²"));
    s.edit(
        &anna,
        &project,
        &model,
        "curtain.grid.add",
        json!({ "element": "CW-900", "direction": "vertical", "at": "750 mm" }),
    )
    .await
    .unwrap();
    let more = quantities(s, &project, &model, "CW-900").await;
    assert_eq!(more["curtain.panels"].0.as_deref(), Some("number:15"));
    s.edit(&anna, &project, &model, "edit.undo", json!({}))
        .await
        .unwrap();
    assert_eq!(quantities(s, &project, &model, "CW-900").await, read);
}

/// A dimension whose reference is gone is listed by the warnings view as well as by the dimension
/// listing: the same kind, elements and sentence, by the same identity; it leaves with an undo.
#[tokio::test(flavor = "multi_thread")]
async fn a_dimensions_lost_reference_is_in_the_warnings_view() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let edit = |cmd: &'static str, args: Value| {
        let (anna, project, model) = (anna.clone(), project.clone(), model.clone());
        async move { s.edit(&anna, &project, &model, cmd, args).await }
    };
    let ours = || async {
        s.warnings(&anna, &project, &model)
            .await
            .unwrap()
            .warnings
            .into_iter()
            .filter(|w| w.kind.starts_with("dimension."))
            .collect::<Vec<_>>()
    };
    let wall = |id: &str, feature: &str| json!({ "element": id, "feature": feature });
    edit(
        "dimension.create",
        json!({ "dimension": "DM-1", "view": "VP-1",
                "layout": { "kind": "horizontal", "y": "4000" },
                "references": [wall("W-103", "wall.line"), wall("W-104", "wall.line")] }),
    )
    .await
    .unwrap();
    assert_eq!(ours().await, Vec::new());
    edit("element.delete", json!({ "element": "W-104" }))
        .await
        .unwrap();
    let lost = ours().await;
    assert_eq!(lost.len(), 1, "{lost:?}");
    assert_eq!(lost[0].identity, "dimension.reference_missing|DM-1|W-104");
    assert_eq!(lost[0].severity, "warning");
    let listed = s
        .dimensions(&anna, &project, &model, &i("VP-1"))
        .await
        .unwrap();
    assert_eq!(listed.dimensions[0].warnings[0].words, lost[0].words);
    edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(ours().await, Vec::new());
}

/// A model's families (job 7.6): a recipe loaded as text is listed first for the page, with where
/// it came from, before the library's families the model does not hold; a reload, and a remove,
/// go through the log, each refused in words where they cannot be; someone off the project cannot
/// read the list.
#[tokio::test(flavor = "multi_thread")]
async fn a_models_own_families_are_loaded_and_listed_for_the_page() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, model) = (i("anna"), i("dan"), i("example-1"), i("arch-model"));
    let (_, window) = construct_regen::families::Library::baked()
        .family("library.doors-windows.fixed-window")
        .unwrap();
    let mut office = (*window).clone();
    office.id = i("office.window");
    "Office window".clone_into(&mut office.name);
    let load = json!({ "recipe": office.to_json() });
    s.edit(&anna, &project, &model, "family.load", load)
        .await
        .unwrap();
    let listed = s.families(&anna, &project, &model).await.unwrap();
    assert_eq!(listed[0].id, "office.window");
    assert!(listed[0].in_document);
    assert_eq!(listed[0].source, "text");
    assert_eq!(listed[0].placed, 0);
    assert_eq!(listed[0].types.len(), 8);
    assert!(listed[1..].iter().all(|f| !f.in_document));
    assert!(refused(s.families(&dan, &project, &model).await));

    "Office window, second issue".clone_into(&mut office.name);
    let reload = json!({ "family": "office.window", "recipe": office.to_json() });
    s.edit(&anna, &project, &model, "family.reload", reload)
        .await
        .unwrap();
    let listed = s.families(&anna, &project, &model).await.unwrap();
    assert_eq!(listed[0].name, "Office window, second issue");
    let missing = s
        .edit(
            &anna,
            &project,
            &model,
            "family.remove",
            json!({ "family": "shop.window" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        missing.to_string(),
        "There is no family shop.window in this document; it has Office window, second issue \
         (office.window)."
    );
    s.edit(
        &anna,
        &project,
        &model,
        "family.remove",
        json!({ "family": "office.window" }),
    )
    .await
    .unwrap();
    let listed = s.families(&anna, &project, &model).await.unwrap();
    assert!(listed.iter().all(|f| !f.in_document));
}
