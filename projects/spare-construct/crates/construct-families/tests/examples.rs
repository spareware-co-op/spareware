//! The three example families: each reads, passes every check, writes back to its own bytes, has
//! a pinned content id, and gives construct-types family types that name it.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{DOOR, EXAMPLES, Fixture, TABLE, WINDOW, read};
use construct_categories::{DOORS, FURNITURE, WINDOWS};
use construct_families::{
    Base, FamilyRef, Hosting, Key, MaterialRef, Operation, ParameterValue, View, family_of,
    family_type, parameters,
};
use construct_params::{Holder, Kind, Value};
use kit_core::Id;

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

#[test]
fn every_example_reads_and_passes_every_check() {
    let fixture = Fixture::new();
    for (name, text) in EXAMPLES {
        let recipe = read(text);
        assert_eq!(recipe.check(&fixture.library()), Ok(()), "{name}");
    }
}

#[test]
fn reading_then_writing_gives_the_same_bytes() {
    for (name, text) in EXAMPLES {
        assert_eq!(read(text).to_json(), text, "{name}");
    }
}

/// A recipe version is named by the SHA-256 of its canonical bytes. Pinned, so a change to the
/// writer or to an example is seen.
#[test]
fn content_ids_are_pinned() {
    let ids: Vec<String> = EXAMPLES
        .iter()
        .map(|(_, text)| read(text).content_id().to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "sha256:49a6410018b54a2b23fb27833d989d45bd31ef32406827cebabc23f1bd4e631a",
            "sha256:ba1da76367be9fbcf2a64b19be57a4825d641643f728e4d44544c10b57482aac",
            "sha256:9e16722a9ae1829c36fc8804d260f27dcf91a3a5b64999d030c9c0c555341dd3",
        ]
    );
}

/// The door has what a single-swing door needs: its sizes and swing as parameters, a wall-hosted
/// opening, a plan swing arc for each hand, and three built-in sizes.
#[test]
fn the_door_is_a_single_swing_door() {
    let door = read(DOOR);
    assert_eq!(door.category, DOORS);
    let held = |k: &str| {
        let p = door
            .parameters
            .iter()
            .find(|p| p.key.as_str() == k)
            .unwrap();
        (p.definition.kind, p.definition.holder)
    };
    for k in ["width", "height", "frame", "panel"] {
        assert_eq!(held(k), (Kind::Length, Holder::Type), "{k}");
    }
    assert_eq!(held("swing"), (Kind::Choice, Holder::Instance));
    let Hosting::Wall { opening } = &door.hosting else {
        panic!("the door is not wall hosted");
    };
    assert_eq!(opening.sketch.as_str(), "elevation");
    assert_eq!(opening.profile.len(), 4);
    let plan: Vec<&str> = door
        .symbols
        .iter()
        .filter(|s| s.view == View::Plan)
        .map(|s| s.key.as_str())
        .collect();
    assert_eq!(plan, ["swing_left", "swing_right"]);
    let plan_sketch = door
        .sketches
        .iter()
        .find(|s| s.key.as_str() == "plan")
        .unwrap();
    assert_eq!(plan_sketch.arcs.len(), 2);
    let swing_left = &door.symbols[0];
    assert!(swing_left.edges.contains(&key("arc_left")));
    assert_eq!(swing_left.when.as_ref().unwrap().is, "choice:left");
    let names: Vec<&str> = door.types.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["720 x 2040", "820 x 2040", "920 x 2040"]);
    let opening_width = door
        .parameters
        .iter()
        .find(|p| p.key.as_str() == "opening_width")
        .unwrap();
    assert_eq!(
        opening_width.value,
        ParameterValue::Formula("width + 2 * frame".into())
    );
}

/// The window's frame is a block less its clear opening, and its plan symbol shows only at the
/// coarse level, where the frame and glass solids do not.
#[test]
fn the_window_cuts_its_frame_and_draws_a_coarse_symbol() {
    let window = read(WINDOW);
    assert_eq!(window.category, WINDOWS);
    let frame = &window.steps[2];
    assert_eq!(
        frame.operation,
        Operation::Subtract {
            target: key("block"),
            tool: key("clear")
        }
    );
    assert_eq!(
        frame.material,
        MaterialRef::Parameter(key("frame_material"))
    );
    let symbol = &window.symbols[0];
    assert!(symbol.visibility.coarse && !symbol.visibility.medium && !symbol.visibility.fine);
    for step in &window.steps {
        assert!(!step.visibility.coarse, "{}", step.key.as_str());
    }
    let sill = window
        .parameters
        .iter()
        .find(|p| p.key.as_str() == "sill_height")
        .unwrap();
    assert_eq!(sill.definition.holder, Holder::Instance);
}

/// The table stands on the floor on its own: unhosted, a top on a plane at its leg height, and
/// four legs.
#[test]
fn the_table_is_unhosted_with_four_legs() {
    let table = read(TABLE);
    assert_eq!(table.category, FURNITURE);
    assert_eq!(table.hosting, Hosting::Unhosted);
    assert_eq!(table.steps.len(), 5);
    let underside = table
        .planes
        .iter()
        .find(|p| p.key.as_str() == "underside")
        .unwrap();
    assert_eq!(underside.base, Base::Xy);
    assert_eq!(underside.offset, "leg_height");
}

/// A built-in type becomes a construct-types type in the family's category, holding its type
/// values, with every parameter the family has, and a `family` system block that names the
/// family, the recipe version and the type.
#[test]
fn a_built_in_type_becomes_a_family_type() {
    let fixture = Fixture::new();
    let door = read(DOOR);
    let params = parameters(&door).unwrap();
    assert_eq!(params.len(), door.parameters.len());
    let id = Id::parse("T-920").unwrap();
    let ty = family_type(&door, &fixture.library(), &key("d920"), id.clone(), &params).unwrap();
    assert_eq!(ty.name(), "920 x 2040");
    assert_eq!(ty.category(), DOORS);
    let width = Id::parse("01KAXF3M1Y0D00R5W1NG000001.width").unwrap();
    assert_eq!(ty.value(&width).unwrap().stored(), "length:q0:5888000");
    let swing = Id::parse("01KAXF3M1Y0D00R5W1NG000001.swing").unwrap();
    assert!(ty.has(&swing) && ty.value(&swing).is_none());
    let opening = Id::parse("01KAXF3M1Y0D00R5W1NG000001.opening_width").unwrap();
    assert!(ty.has(&opening) && ty.value(&opening).is_none());
    let block = ty.system().unwrap();
    assert_eq!(block.owner(), "family");
    let expected = format!(
        r#"{{"version":1,"family":"01KAXF3M1Y0D00R5W1NG000001","recipe":"{}","type":"d920"}}"#,
        door.content_id()
    );
    assert_eq!(block.data(), expected);
    let named = family_of(&ty).unwrap().unwrap();
    assert_eq!(
        named,
        FamilyRef {
            family: door.id.clone(),
            recipe: door.content_id(),
            type_key: key("d920"),
        }
    );
    assert_eq!(FamilyRef::from_text(&named.to_text()), Ok(named));
    let record = ty.to_record(&Id::parse("DOC-1").unwrap());
    assert_eq!(record.props["@system.family"], expected);
    // A default the type does not set reads through the definition's default.
    let frame = Id::parse("01KAXF3M1Y0D00R5W1NG000001.frame").unwrap();
    assert_eq!(
        params.default_of(&frame).unwrap().stored(),
        "length:q0:256000"
    );
    assert_eq!(
        params.default_of(&Id::parse("01KAXF3M1Y0D00R5W1NG000001.panel_material").unwrap()),
        Some(&Value::from_stored("material:timber-white-oak").unwrap())
    );
}

/// A type table's row becomes a family type as a built-in type does: its name, its values column
/// by column, and a block naming the row's key.
#[test]
fn a_type_table_row_becomes_a_family_type() {
    let fixture = Fixture::new();
    let door = read(&common::door_with_table());
    assert_eq!(door.check(&fixture.library()), Ok(()));
    let params = parameters(&door).unwrap();
    let id = Id::parse("T-921").unwrap();
    let ty = family_type(&door, &fixture.library(), &key("d921"), id, &params).unwrap();
    assert_eq!(ty.name(), "920 x 2100");
    let value = |name: &str| {
        let id = Id::parse(&format!("01KAXF3M1Y0D00R5W1NG000001.{name}")).unwrap();
        ty.value(&id).map(Value::stored)
    };
    assert_eq!(value("width").as_deref(), Some("length:q0:5888000"));
    assert_eq!(value("height").as_deref(), Some("length:q0:13440000"));
    assert_eq!(value("frame"), None);
    let named = family_of(&ty).unwrap().unwrap();
    assert_eq!(named.type_key, key("d921"));
    assert_eq!(named.recipe, door.content_id());
    let keys: Vec<&str> = door
        .named_types()
        .iter()
        .map(|t| t.key().as_str())
        .collect();
    assert_eq!(keys, ["d720", "d820", "d920", "d620", "d921", "d820t"]);
    assert_eq!(door.named_type(&key("d620")).unwrap().name(), "620 x 2040");
    assert!(door.named_type(&key("d999")).is_none());
}

/// A type that is not in the recipe, and a recipe that does not pass its checks, give no type.
#[test]
fn a_family_type_needs_a_checked_recipe_and_a_known_type() {
    let fixture = Fixture::new();
    let door = read(DOOR);
    let params = parameters(&door).unwrap();
    let id = Id::parse("T-1").unwrap();
    assert!(family_type(&door, &fixture.library(), &key("d999"), id.clone(), &params).is_err());
    let broken = read(&common::edit(DOOR, |v| {
        v["category"] = serde_json::json!(99_999);
    }));
    assert!(family_type(&broken, &fixture.library(), &key("d820"), id, &params).is_err());
}

/// A family block's text is read strictly: anything its writer would not give is refused.
#[test]
fn a_family_block_is_read_strictly() {
    let door = read(DOOR);
    let good = FamilyRef {
        family: door.id.clone(),
        recipe: door.content_id(),
        type_key: key("d820"),
    }
    .to_text();
    assert!(FamilyRef::from_text(&good).is_ok());
    for bad in [
        good.replace("\"version\":1", "\"version\":2"),
        good.replace(',', ", "),
        good.replace("sha256:", "md5:"),
        good.replace("\"type\":\"d820\"", "\"type\":\"D820\""),
        format!("{good} "),
        good.replace("{\"version\":1,", "{\"version\":1,\"extra\":1,"),
    ] {
        assert!(FamilyRef::from_text(&bad).is_err(), "{bad}");
    }
}
