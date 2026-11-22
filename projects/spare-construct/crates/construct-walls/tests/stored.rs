//! The stored form: a wall type's layers and core as text in one spelling, kept in its type's
//! system block, pinned byte for byte, and every text that is not in that spelling refused.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{finish, id, mm, structure, wall_parameters};
use construct_categories::{CategoryTable, DOORS, WALLS};
use construct_model::element::Element;
use construct_types::{ElementType, SystemData};
use construct_walls::{
    Assembly, FORMAT_VERSION, OWNER, WallError, assembly_of, default_wall_types, default_walls,
    new_wall_type, with_assembly,
};

const PARTITION: &str = concat!(
    r#"{"version":1,"core":{"exterior":1,"interior":2},"layers":["#,
    r#"{"function":"finish","thickness":"length:q0:80000","material":"plasterboard-gypsum","wraps":true},"#,
    r#"{"function":"structure","thickness":"length:q0:576000","material":"timber-douglas-fir","wraps":false},"#,
    r#"{"function":"finish","thickness":"length:q0:80000","material":"plasterboard-gypsum","wraps":true}]}"#
);

const CONCRETE: &str = concat!(
    r#"{"version":1,"core":{"exterior":0,"interior":1},"layers":["#,
    r#"{"function":"structure","thickness":"length:q0:1280000","material":"concrete-regular","wraps":false}]}"#
);

const BRICK_VENEER: &str = concat!(
    r#"{"version":1,"core":{"exterior":3,"interior":4},"layers":["#,
    r#"{"function":"finish","thickness":"length:q0:704000","material":"brick-clay","wraps":false},"#,
    r#"{"function":"thermal","thickness":"length:q0:256000","material":"air-dry","wraps":false},"#,
    r#"{"function":"membrane","thickness":"length:q0:0","material":"membrane-polyethylene","wraps":true},"#,
    r#"{"function":"structure","thickness":"length:q0:576000","material":"timber-douglas-fir","wraps":false},"#,
    r#"{"function":"finish","thickness":"length:q0:64000","material":"plasterboard-gypsum","wraps":true}]}"#
);

/// The concrete wall type's whole element record, as construct-model writes it.
const CONCRETE_ROW: &str = concat!(
    r#"{"id":"wall-concrete-200","document":"DOC-1","kind":"type","name":"Concrete 200","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"type","op_version":2,"args":{}},"#,
    r#""props":{"@category":"100","@params":"wall.flipped,wall.location-line","#,
    r#""@system.wall":"{\"version\":1,\"core\":{\"exterior\":0,\"interior\":1},\"layers\":["#,
    r#"{\"function\":\"structure\",\"thickness\":\"length:q0:1280000\",\"material\":\"concrete-regular\","#,
    r#"\"wraps\":false}]}"},"references":[]}"#
);

/// Each default type's text, byte for byte, and read back to the same type.
#[test]
fn default_texts_pinned() {
    assert_eq!(FORMAT_VERSION, 1);
    let texts = [PARTITION, CONCRETE, BRICK_VENEER];
    for (wall, text) in default_walls().unwrap().iter().zip(texts) {
        assert_eq!(wall.assembly.to_text().unwrap(), text, "{}", wall.id);
        assert_eq!(Assembly::from_text(text).unwrap(), wall.assembly);
    }
}

/// The same data gives the same bytes, however it was reached.
#[test]
fn same_data_same_bytes() {
    let direct = Assembly::new(vec![finish(10), structure(20)], 1, 2).unwrap();
    let edited = Assembly::new(vec![structure(20)], 0, 1)
        .unwrap()
        .with_inserted(0, finish(10))
        .unwrap();
    assert_eq!(direct.to_text().unwrap(), edited.to_text().unwrap());
}

/// A wall type's record holds its text in the `@system.wall` key: pinned byte for byte, and read
/// back by construct-types alone to the same type and the same layers.
#[test]
fn wall_type_record_pinned() {
    let params = wall_parameters();
    let categories = CategoryTable::new();
    let types = default_wall_types(&params, &categories).unwrap();
    let concrete = types.get(1).unwrap();
    let row = concrete.to_record(&id("DOC-1"));
    assert_eq!(serde_json::to_string(&row).unwrap(), CONCRETE_ROW);
    let read: Element = serde_json::from_str(CONCRETE_ROW).unwrap();
    let ty = ElementType::from_record(&read, &params, &categories).unwrap();
    assert_eq!(&ty, concrete);
    assert_eq!(ty.system().map(SystemData::owner), Some(OWNER));
    assert_eq!(ty.system().map(SystemData::data), Some(CONCRETE));
    assert_eq!(
        assembly_of(&ty).unwrap(),
        default_walls().unwrap().get(1).unwrap().assembly
    );
}

/// Every spelling but the one written is refused, never repaired.
#[test]
fn other_spellings_refused() {
    let malformed = |text: &str| {
        assert!(
            matches!(Assembly::from_text(text), Err(WallError::Malformed(_))),
            "{text}"
        );
    };
    malformed("");
    malformed("{}");
    malformed(&CONCRETE.replace(r#""core":"#, r#""core": "#));
    malformed(&CONCRETE.replace(
        r#"{"version":1,"core":{"exterior":0,"interior":1},"#,
        r#"{"core":{"exterior":0,"interior":1},"version":1,"#,
    ));
    malformed(&CONCRETE.replace(r#""version":1"#, r#""version":2"#));
    malformed(&CONCRETE.replace(r#""wraps":false"#, r#""wraps":false,"colour":"red""#));
    malformed(&CONCRETE.replace(r#","wraps":false"#, ""));
    malformed(&CONCRETE.replace(r#""length:q0:1280000""#, "200"));
    malformed(&CONCRETE.replace("length:q0:1280000", "length:q0:01280000"));
    malformed(&CONCRETE.replace("length:q0:1280000", "length:q0:1280000.0"));
    malformed(&CONCRETE.replace("length:q0:1280000", "whole:200"));
    malformed(&CONCRETE.replace("length:q0:1280000", "200 mm"));
    malformed(&CONCRETE.replace("concrete-regular", "@concrete"));
    malformed(&CONCRETE.replace(r#""structure""#, r#""glazing""#));
    malformed(&CONCRETE.replace(r#""exterior":0"#, r#""exterior":-1"#));
    malformed(&format!("{CONCRETE} "));
}

/// A text well spelt but not a valid wall type gets the wall type's own refusal.
#[test]
fn invalid_layers_refused_as_themselves() {
    assert_eq!(
        Assembly::from_text(&CONCRETE.replace(r#""interior":1"#, r#""interior":0"#)),
        Err(WallError::EmptyCore {
            exterior: 0,
            interior: 0
        })
    );
    assert_eq!(
        Assembly::from_text(&CONCRETE.replace("length:q0:1280000", "length:q0:0")),
        Err(WallError::ZeroThickness { layer: 0 })
    );
    assert_eq!(
        Assembly::from_text(&CONCRETE.replace(r#""wraps":false"#, r#""wraps":true"#)),
        Err(WallError::CoreLayerWraps { layer: 0 })
    );
}

/// Only a type of the walls category with a wall block is a wall type.
#[test]
fn not_a_wall_type() {
    let params = wall_parameters();
    let categories = CategoryTable::new();
    let concrete = || Assembly::new(vec![structure(200)], 0, 1).unwrap();
    let door = ElementType::new(id("T-door"), "Door", DOORS, &categories).unwrap();
    assert_eq!(
        assembly_of(&door),
        Err(WallError::NotAWallType { category: DOORS })
    );
    assert_eq!(
        with_assembly(door, &concrete()),
        Err(WallError::NotAWallType { category: DOORS })
    );
    let bare = ElementType::new(id("T-bare"), "Bare", WALLS, &categories).unwrap();
    assert!(matches!(assembly_of(&bare), Err(WallError::Malformed(_))));
    let other = bare
        .clone()
        .with_system(SystemData::new("floor", CONCRETE).unwrap());
    assert!(matches!(assembly_of(&other), Err(WallError::Malformed(_))));
    let garbled = bare.with_system(SystemData::new(OWNER, "{").unwrap());
    assert!(matches!(
        assembly_of(&garbled),
        Err(WallError::Malformed(_))
    ));
    let made = new_wall_type(id("T-c"), "C", &concrete(), &params, &categories).unwrap();
    assert_eq!(assembly_of(&made).unwrap(), concrete());
    assert_eq!(assembly_of(&made).unwrap().total(), mm(200));
}

/// Keeping an edit's result: the type's block is replaced and nothing else changes.
#[test]
fn with_assembly_replaces_the_block() {
    let params = wall_parameters();
    let categories = CategoryTable::new();
    let one = Assembly::new(vec![structure(200)], 0, 1).unwrap();
    let ty = new_wall_type(id("T-c"), "C", &one, &params, &categories).unwrap();
    let two = one.with_inserted(1, finish(10)).unwrap();
    let edited = with_assembly(ty.clone(), &two).unwrap();
    assert_eq!(assembly_of(&edited).unwrap(), two);
    assert_eq!(edited.name(), ty.name());
    assert_eq!(edited.id(), ty.id());
    assert_eq!(edited.params().count(), 2);
    assert_eq!(assembly_of(&ty).unwrap(), one);
}
