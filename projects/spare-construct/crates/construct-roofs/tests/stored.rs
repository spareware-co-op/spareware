//! A roof's record, its recipe and its type in their one spelling: written, read back the same,
//! anything else refused; and the recipe the builder takes once the type's layers are filled in.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, gable, id, mm, mono, rafters, slope};
use construct_categories::{CategoryTable, FLOORS, ROOFS};
use construct_levels::Tie;
use construct_roofs::{
    EDGES_ARG, FOOTPRINT_ARG, LAYERS_ARG, OWNER, Roof, RoofError, default_roof_types,
    default_roofs, filled, is_roof, layers_of, new_roof_type, roof_of, roof_recipe, roof_record,
    with_layers, with_roof,
};
use construct_slabs::{layers_arg, reads_layers};
use construct_types::{ElementType, SystemData};
use serde_json::json;

/// The edges are stored in the footprint's order, each its overhang in construct-params' length
/// spelling and, when it slopes, its pitch in its own; the recipe reads back as the same roof.
#[test]
fn a_roofs_recipe_reads_back_as_the_same_roof() {
    let roof = gable(slope(1, 4));
    assert_eq!(
        roof.edges_value(),
        json!([
            { "id": "o0", "overhang": "length:q0:3200000", "pitch": "slope:1/4" },
            { "id": "o1", "overhang": "length:q0:1920000" },
            { "id": "o2", "overhang": "length:q0:3200000", "pitch": "slope:1/4" },
            { "id": "o3", "overhang": "length:q0:1920000" },
        ])
    );
    let recipe = roof_recipe(&roof);
    assert_eq!(recipe.op, "roof");
    assert_eq!(recipe.op_version, 1);
    assert_eq!(recipe.args[LAYERS_ARG], layers_arg());
    assert!(reads_layers(&recipe));
    assert_eq!(roof_of(&recipe).unwrap(), roof);
    let from_plain = Roof::from_plain(&json!({
        "footprint": { "outer": [
            { "at": [0, 0] }, { "at": [6000 * MM, 0] },
            { "at": [6000 * MM, 4000 * MM] }, { "at": [0, 4000 * MM] }
        ] },
        "edges": {
            "o0": { "overhang": 500 * MM, "pitch": "slope:1/4" },
            "o1": { "overhang": 300 * MM },
            "o2": { "overhang": 500 * MM, "pitch": "slope:1/4" },
            "o3": { "overhang": 300 * MM }
        }
    }))
    .unwrap();
    assert_eq!(from_plain, roof);
}

/// Anything but the one spelling is refused, never repaired: an edge left out or out of order, a
/// field added, a length or a pitch spelt another way, another operation or version.
#[test]
fn anything_else_is_refused() {
    let recipe = roof_recipe(&mono());
    let with_edges = |edges: serde_json::Value| {
        let mut changed = recipe.clone();
        changed.args.insert(EDGES_ARG.to_owned(), edges);
        roof_of(&changed)
    };
    let edges = recipe.args[EDGES_ARG].as_array().unwrap().clone();
    let malformed =
        |result: Result<Roof, RoofError>| matches!(result, Err(RoofError::Malformed(_)));
    assert!(malformed(with_edges(json!(edges[..3]))));
    let mut swapped = edges.clone();
    swapped.swap(1, 2);
    assert!(malformed(with_edges(json!(swapped))));
    let mut added = edges.clone();
    added[1]["colour"] = json!("red");
    assert!(malformed(with_edges(json!(added))));
    let mut whole_mm = edges.clone();
    whole_mm[1]["overhang"] = json!("length:300");
    assert!(
        malformed(with_edges(json!(whole_mm))),
        "an old spelling is not this one"
    );
    let mut ratio = edges.clone();
    ratio[0]["pitch"] = json!("slope:2/8");
    assert!(malformed(with_edges(json!(ratio))));
    let mut angle = edges;
    angle[0]["pitch"] = json!("angle:x");
    assert!(malformed(with_edges(json!(angle))));
    let mut other = recipe.clone();
    other.op_version = 2;
    assert!(matches!(roof_of(&other), Err(RoofError::NotARoof(_))));
    let mut bare = recipe;
    bare.args.remove(FOOTPRINT_ARG);
    assert!(matches!(roof_of(&bare), Err(RoofError::NotARoof(_))));
}

/// A roof's record: kind `roof`, of the roofs category and its type, its base tied to its level;
/// its recipe changes with its roof, nothing else; its bytes are pinned (FORMAT.md's example).
#[test]
fn a_roofs_record() {
    let roof = Roof::new(
        construct_slabs::Boundary::from_corners(
            &[
                common::at(0, 0),
                common::at(4000, 0),
                common::at(4000, 3000),
                common::at(0, 3000),
            ],
            &[],
        )
        .unwrap(),
        &common::edges(&[("o0", 400, Some(slope(1, 4)))]),
    )
    .unwrap();
    let tie = Tie {
        level: id("L-2"),
        offset: mm(0),
    };
    let record = roof_record(
        &id("R-1"),
        &id("DOC-1"),
        "Lean-to",
        &id("roof-steel"),
        &tie,
        &roof,
    );
    assert!(is_roof(&record));
    assert_eq!(record.props["@category"], ROOFS.0.to_string());
    assert_eq!(
        serde_json::to_string(&record).unwrap(),
        concat!(
            r#"{"id":"R-1","document":"DOC-1","kind":"roof","name":"Lean-to","level":0,"#,
            r#""placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"#,
            r#""op":"roof","op_version":1,"args":{"edges":[{"id":"o0","overhang":"length:q0:2560000","pitch":"slope:1/4"},"#,
            r#"{"id":"o1","overhang":"length:q0:0"},{"id":"o2","overhang":"length:q0:0"},"#,
            r#"{"id":"o3","overhang":"length:q0:0"}],"footprint":{"holes":[],"outer":[{"id":"o0","line":0},"#,
            r#"{"id":"o1","line":1},{"id":"o2","line":2},{"id":"o3","line":3}],"sketch":{"constraints":[],"#,
            r#""lines":[{"end":1,"start":0},{"end":2,"start":1},{"end":3,"start":2},{"end":0,"start":3}],"#,
            r#""points":[{"x":0,"y":0},{"x":25600000,"y":0},{"x":25600000,"y":19200000},{"x":0,"y":19200000}]},"#,
            r#""version":1},"layers":{"type":"layers"}}},"props":{"@category":"600","@level":"L-2","#,
            r#""@level.offset":"length:q0:0","@type":"roof-steel"},"references":[]}"#
        )
    );
    let flat = Roof::new(roof.footprint().clone(), &common::edges(&[])).unwrap();
    let changed = with_roof(record.clone(), &flat).unwrap();
    assert_eq!(roof_of(&changed.recipe).unwrap(), flat);
    assert_eq!(changed.props, record.props);
    let mut wall = record;
    wall.kind = "wall.layered".to_owned();
    assert!(matches!(
        with_roof(wall, &flat),
        Err(RoofError::NotARoof(_))
    ));
}

/// Filled from its type, the recipe the builder takes holds its form, the edges it rises from,
/// its eaves outline and each layer with its solid, and nothing else; a membrane has no solid.
#[test]
fn the_builders_recipe_is_the_shape() {
    let steel = default_roofs().unwrap().remove(0).layers;
    let recipe = filled(&roof_recipe(&gable(slope(1, 4))), &steel).unwrap();
    let keys: Vec<&str> = recipe.args.keys().map(String::as_str).collect();
    assert_eq!(keys, ["form", "layers", "outline", "slopes"]);
    assert!(!reads_layers(&recipe));
    assert_eq!(recipe.args["form"], json!("gable"));
    assert_eq!(recipe.args["slopes"], json!(["o0", "o2"]));
    assert_eq!(
        recipe.args["outline"][0],
        json!({ "id": "o0", "at": [-300 * MM, -500 * MM] })
    );
    let layers = recipe.args["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 3);
    assert_eq!(
        layers[1],
        json!({ "function": "membrane", "material": "membrane-polyethylene", "thickness": 0 })
    );
    assert_eq!(layers[2]["thickness"], json!(150 * MM));
    let loft = layers[2]["solid"]["loft"].as_array().unwrap();
    assert_eq!(loft.len(), 2);
    assert_eq!(loft[0][1], json!([-300 * MM, 2000 * MM, 500 * MM]));
    let flat = filled(
        &roof_recipe(&Roof::new(common::rectangle(), &common::edges(&[])).unwrap()),
        &rafters(),
    )
    .unwrap();
    assert_eq!(flat.args["form"], json!("flat"));
    assert_eq!(flat.args["slopes"], json!([]));
    assert_eq!(
        flat.args["layers"][0]["solid"],
        json!({
            "extrude": [[0, 0, 0], [6000 * MM, 0, 0], [6000 * MM, 4000 * MM, 0], [0, 4000 * MM, 0]],
            "by": [0, 0, 150 * MM]
        })
    );
}

/// A roof type is of the roofs category with its layers in its `@system.roof` block, spelt as a
/// floor type's; a type of another category, or one without the block, is refused. The defaults
/// read back, every material in construct-materials' library.
#[test]
fn a_roof_type_holds_its_layers() {
    let categories = CategoryTable::new();
    let ty = new_roof_type(id("roof-own"), "Own", &rafters(), &categories).unwrap();
    assert_eq!(OWNER, "roof");
    assert_eq!(ty.category(), ROOFS);
    assert_eq!(ty.system().unwrap().owner(), "roof");
    assert_eq!(ty.system().unwrap().data(), rafters().to_text().unwrap());
    assert_eq!(layers_of(&ty).unwrap(), rafters());
    let floor_type = ElementType::new(id("f"), "Floor", FLOORS, &categories).unwrap();
    assert_eq!(
        with_layers(floor_type.clone(), &rafters()).unwrap_err(),
        RoofError::NotARoofType { category: FLOORS }
    );
    assert!(matches!(
        layers_of(&floor_type),
        Err(RoofError::NotARoofType { .. })
    ));
    let bare = ElementType::new(id("r"), "Bare", ROOFS, &categories).unwrap();
    assert_eq!(
        layers_of(&bare).unwrap_err(),
        RoofError::NoRoofLayers { ty: id("r") }
    );
    let other_owner =
        bare.with_system(SystemData::new("floor", &rafters().to_text().unwrap()).unwrap());
    assert!(matches!(
        layers_of(&other_owner),
        Err(RoofError::NoRoofLayers { .. })
    ));
    let library = construct_materials::Materials::default_metric();
    let types = default_roof_types(&categories).unwrap();
    let ids: Vec<&str> = types.iter().map(|t| t.id().as_str()).collect();
    assert_eq!(
        ids,
        ["roof-steel-rafter-150", "roof-concrete-200-insulated"]
    );
    for ty in &types {
        assert_eq!(
            layers_of(ty).unwrap().missing_materials(&library),
            Vec::<usize>::new()
        );
    }
}
