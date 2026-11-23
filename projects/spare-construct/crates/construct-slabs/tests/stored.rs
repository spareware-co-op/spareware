//! The stored forms, each in one spelling: a floor type's layers under `@system.floor`, the type
//! record, the boundary in a floor's recipe, and a floor's element record.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{id, l_with_hole, mm, slab};
use construct_categories::{CategoryTable, FLOORS, WALLS};
use construct_levels::Tie;
use construct_model::geometry::Recipe;
use construct_slabs::{
    BOUNDARY_ARG, Boundary, FLOOR_KIND, LAYERS_ARG, Layers, OWNER, SlabError, boundary_of,
    floor_recipe, floor_record, layers_arg, layers_of, new_floor_type, reads_layers, with_boundary,
    with_layers,
};
use construct_types::ElementType;
use serde_json::json;

const SLAB_TEXT: &str = r#"{"version":1,"layers":[{"function":"finish","thickness":"length:q0:320000","material":"screed-cement"},{"function":"structure","thickness":"length:q0:960000","material":"concrete-regular"}]}"#;

/// A floor type's layers are compact JSON, top layer first, every length in construct-params'
/// stored spelling; read back they are the same layers, and nothing else is read.
#[test]
fn the_layers_text_is_pinned() {
    assert_eq!(slab().to_text().unwrap(), SLAB_TEXT);
    assert_eq!(Layers::from_text(SLAB_TEXT).unwrap(), slab());
    for bad in [
        SLAB_TEXT.replace("\"version\":1", "\"version\":2"),
        SLAB_TEXT.replace(",\"material\":\"screed-cement\"", ""),
        SLAB_TEXT.replace("{\"version\"", "{\"extra\":0,\"version\""),
        SLAB_TEXT.replace("length:q0:320000", "length:50"),
        SLAB_TEXT.replace("length:q0:320000", "50 mm"),
        SLAB_TEXT.replace(":1,", ": 1,"),
        SLAB_TEXT.replace("\"finish\"", "\"floor\""),
    ] {
        assert!(
            matches!(Layers::from_text(&bad), Err(SlabError::Malformed(_))),
            "{bad}"
        );
    }
    // Well spelt, but no layers: refused by the rules, not the spelling.
    assert_eq!(
        Layers::from_text(r#"{"version":1,"layers":[]}"#),
        Err(SlabError::NoLayers)
    );
}

/// A floor type is a type record of the floors category with its layers in its system block;
/// the record's bytes are pinned, and a type of another category, or one without a block, is
/// no floor type.
#[test]
fn a_floor_type_and_its_record() {
    let categories = CategoryTable::new();
    let ty = new_floor_type(id("floor-slab"), "Slab 200", &slab(), &categories).unwrap();
    assert_eq!(ty.category(), FLOORS);
    assert_eq!(ty.system().unwrap().owner(), OWNER);
    assert_eq!(layers_of(&ty).unwrap(), slab());
    let record = ty.to_record(&id("DOC-1"));
    let bytes = serde_json::to_string(&record).unwrap();
    let expected = format!(
        r#"{{"id":"floor-slab","document":"DOC-1","kind":"type","name":"Slab 200","level":0,"placement":{{"x":0,"y":0,"z":0}},"recipe":{{"owner":"construct","schema":1,"op":"type","op_version":2,"args":{{}}}},"props":{{"@category":"500","@params":"","@system.floor":{}}},"references":[]}}"#,
        serde_json::to_string(SLAB_TEXT).unwrap()
    );
    assert_eq!(bytes, expected);

    let wall = ElementType::new(id("w"), "Wall", WALLS, &categories).unwrap();
    assert_eq!(
        layers_of(&wall),
        Err(SlabError::NotAFloorType { category: WALLS })
    );
    assert_eq!(
        with_layers(wall, &slab()).unwrap_err(),
        SlabError::NotAFloorType { category: WALLS }
    );
    let bare = ElementType::new(id("f"), "Floor", FLOORS, &categories).unwrap();
    assert_eq!(
        layers_of(&bare).unwrap_err().to_string(),
        "floor type f has no layers: a floor type carries a \"floor\" system block"
    );
    let thinner = Layers::new(vec![common::layer(
        construct_slabs::Function::Structure,
        100,
        "concrete-regular",
    )])
    .unwrap();
    let changed = with_layers(ty, &thinner).unwrap();
    assert_eq!(layers_of(&changed).unwrap().total(), mm(100));
}

/// A floor's recipe holds its boundary in one spelling, and reads its type's layers through the
/// argument `{"type":"layers"}`, which the regenerator fills.
#[test]
fn a_floors_recipe() {
    let floor = l_with_hole();
    let recipe = floor_recipe(&floor);
    assert_eq!(recipe.op, "floor");
    assert_eq!(recipe.op_version, 1);
    assert_eq!(recipe.args[LAYERS_ARG], layers_arg());
    assert_eq!(layers_arg(), json!({ "type": "layers" }));
    assert!(reads_layers(&recipe));
    assert_eq!(boundary_of(&recipe).unwrap(), floor);
    let stored = &recipe.args[BOUNDARY_ARG];
    assert_eq!(stored["version"], json!(1));
    assert_eq!(stored["outer"][0], json!({ "id": "e0", "line": 0 }));
    assert_eq!(stored["holes"][0][3], json!({ "id": "h3", "line": 9 }));
    assert_eq!(
        stored["sketch"]["points"][1],
        json!({ "x": 51_200_000, "y": 0 })
    );
    assert_eq!(Boundary::from_value(stored).unwrap(), floor);

    let mut changed = stored.clone();
    changed["outer"][0]["extra"] = json!(1);
    assert!(matches!(
        Boundary::from_value(&changed),
        Err(SlabError::Malformed(_))
    ));
    let mut later = stored.clone();
    later["version"] = json!(2);
    assert!(matches!(
        Boundary::from_value(&later),
        Err(SlabError::Malformed(_))
    ));
    // Well spelt but not a floor: its outer loop's edges out of order.
    let mut shuffled = stored.clone();
    shuffled["outer"][1] = json!({ "id": "e1", "line": 2 });
    shuffled["outer"][2] = json!({ "id": "e2", "line": 1 });
    assert!(matches!(
        Boundary::from_value(&shuffled),
        Err(SlabError::NotClosed { .. })
    ));
    let arc = json!({ "id": "a", "arc": 0, "reversed": false });
    let mut with_arc = stored.clone();
    with_arc["outer"][0] = arc;
    assert!(Boundary::from_value(&with_arc).is_err());

    let wall = Recipe::construct("wall.straight", std::collections::BTreeMap::new());
    assert!(!reads_layers(&wall));
    assert!(matches!(boundary_of(&wall), Err(SlabError::NotAFloor(_))));
}

/// FORMAT.md's example floor record, byte for byte: a 4 m by 3 m right triangle tied 20 mm below
/// level `L-1`, its edges `a`, `b` and `c`.
const TRIANGLE_RECORD: &str = r#"{"id":"F-1","document":"DOC-1","kind":"floor","name":"Ground floor","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"floor","op_version":1,"args":{"boundary":{"holes":[],"outer":[{"id":"a","line":0},{"id":"b","line":1},{"id":"c","line":2}],"sketch":{"constraints":[],"lines":[{"end":1,"start":0},{"end":2,"start":1},{"end":0,"start":2}],"points":[{"x":0,"y":0},{"x":25600000,"y":0},{"x":0,"y":19200000}]},"version":1},"layers":{"type":"layers"}}},"props":{"@category":"500","@level":"L-1","@level.offset":"length:q0:-128000","@type":"floor-slab"},"references":[]}"#;

/// The example in FORMAT.md is what this crate writes, and it reads back as the same floor.
#[test]
fn the_format_example_is_what_the_crate_writes() {
    let tie = Tie {
        level: id("L-1"),
        offset: mm(-20),
    };
    let triangle = Boundary::from_corners(
        &[
            common::at("a", 0, 0),
            common::at("b", 4000, 0),
            common::at("c", 0, 3000),
        ],
        &[],
    )
    .unwrap();
    let record = floor_record(
        &id("F-1"),
        &id("DOC-1"),
        "Ground floor",
        &id("floor-slab"),
        &tie,
        &triangle,
    );
    let bytes = serde_json::to_string(&record).unwrap();
    assert_eq!(bytes, TRIANGLE_RECORD);
    assert!(include_str!("../FORMAT.md").contains(TRIANGLE_RECORD));
    assert!(include_str!("../FORMAT.md").contains(SLAB_TEXT));
    let read: construct_model::element::Element = serde_json::from_str(&bytes).unwrap();
    assert_eq!(boundary_of(&read.recipe).unwrap(), triangle);
}

/// A floor's element record: kind `floor`, the floors category, its type, its tie to a level,
/// placement at the origin and its recipe; replacing its boundary changes only the recipe.
#[test]
fn a_floors_record() {
    let tie = Tie {
        level: id("L-1"),
        offset: mm(-20),
    };
    let record = floor_record(
        &id("F-1"),
        &id("DOC-1"),
        "Ground floor",
        &id("floor-slab"),
        &tie,
        &l_with_hole(),
    );
    assert_eq!(record.kind, FLOOR_KIND);
    assert_eq!(record.level, 0);
    assert_eq!(record.props["@category"], "500");
    assert_eq!(record.props["@type"], "floor-slab");
    assert_eq!(record.props["@level"], "L-1");
    assert_eq!(record.props["@level.offset"], "length:q0:-128000");
    assert_eq!(record.props.len(), 4);
    assert_eq!(record.recipe, floor_recipe(&l_with_hole()));
    assert_eq!(record.references, Vec::new());

    let square = Boundary::from_corners(&common::rectangle("s", 0, 0, 3000, 3000), &[]).unwrap();
    let moved = with_boundary(record.clone(), &square).unwrap();
    assert_eq!(moved.recipe, floor_recipe(&square));
    assert_eq!(moved.props, record.props);
    assert_eq!(moved.name, record.name);
    let mut wall = record;
    wall.kind = "wall".to_owned();
    assert!(matches!(
        with_boundary(wall, &square),
        Err(SlabError::NotAFloor(_))
    ));
}
