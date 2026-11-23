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
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{FileStore, Kv, Scope};
use kit_sync::{read_log, verify_log};
use serde_json::{Value, json};
use spare_server::backup;
use spare_server::seed::seed;
use spare_server::service::{Refusal, Service, View};
use std::collections::BTreeMap;
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
    assert_eq!(regen.verify(&project).await.unwrap(), 33);
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
              "level_name": "Level 1", "scale": 100, "detail": "medium" },
            { "id": "VP-2", "name": "Level 2 plan", "kind": "floor_plan", "level": "2",
              "level_name": "Level 2", "scale": 100, "detail": "medium" },
        ])
    );
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
            "view": { "id": "VP-1", "name": "Level 1 plan", "kind": "floor_plan", "level": "1",
                      "level_name": "Level 1", "scale": 100, "detail": "medium" },
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
