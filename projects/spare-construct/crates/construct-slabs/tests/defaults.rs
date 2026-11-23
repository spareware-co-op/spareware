//! The default metric set, which of its materials construct-materials' default library does not
//! hold yet, and the recipe the builder takes once a floor's type's layers are filled in.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{l_with_hole, mm, slab};
use construct_categories::{CategoryTable, FLOORS};
use construct_materials::Materials;
use construct_slabs::{
    Function, LAYERS_ARG, LOOPS_ARG, default_floor_types, default_floors, filled, floor_recipe,
    layers_of,
};
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn default_set() {
    use Function::{Finish, Structure, Substrate};
    let floors = default_floors().unwrap();
    let ids: Vec<&str> = floors.iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        [
            "floor-concrete-200",
            "floor-concrete-150-screed-50",
            "floor-timber-joist-240"
        ]
    );
    let totals: Vec<_> = floors.iter().map(|f| f.layers.total()).collect();
    assert_eq!(totals, [mm(200), mm(200), mm(260)]);
    let functions: Vec<Vec<Function>> = floors
        .iter()
        .map(|f| f.layers.layers().iter().map(|l| l.function).collect())
        .collect();
    assert_eq!(
        functions,
        [
            vec![Structure],
            vec![Finish, Structure],
            vec![Substrate, Structure]
        ]
    );
    let types = default_floor_types(&CategoryTable::new()).unwrap();
    assert_eq!(types.len(), 3);
    for (ty, floor) in types.iter().zip(&floors) {
        assert_eq!(ty.id().as_str(), floor.id);
        assert_eq!(ty.name(), floor.name);
        assert_eq!(ty.category(), FLOORS);
        assert_eq!(layers_of(ty).unwrap(), floor.layers);
    }
}

/// Screed and particleboard flooring are not in construct-materials' defaults yet; every other
/// layer's material is. When the materials crate adds them, this test says so.
#[test]
fn missing_materials_pinned() {
    let library = Materials::default_metric();
    let mut missing = BTreeSet::new();
    for floor in default_floors().unwrap() {
        for at in floor.layers.missing_materials(&library) {
            missing.insert(floor.layers.layers()[at].material.as_str().to_owned());
        }
    }
    let missing: Vec<String> = missing.into_iter().collect();
    assert_eq!(missing, ["particleboard", "screed-cement"]);
}

/// Filled for the builder, a floor's recipe holds its loops (the outer one anticlockwise, then
/// each hole clockwise, each corner with the ID of the edge that starts there, in grid steps)
/// and its layers top first, each its function, its material and its thickness in grid steps;
/// nothing else.
#[test]
fn the_recipe_the_builder_takes() {
    let recipe = filled(&floor_recipe(&l_with_hole()), &slab()).unwrap();
    assert_eq!(recipe.op, "floor");
    let keys: Vec<&String> = recipe.args.keys().collect();
    assert_eq!(keys, [LAYERS_ARG, LOOPS_ARG]);
    assert_eq!(
        recipe.args[LAYERS_ARG],
        json!([
            { "function": "finish", "material": "screed-cement", "thickness": 320_000 },
            { "function": "structure", "material": "concrete-regular", "thickness": 960_000 },
        ])
    );
    let loops = &recipe.args[LOOPS_ARG];
    assert_eq!(loops.as_array().unwrap().len(), 2);
    assert_eq!(loops[0][1], json!({ "id": "e1", "at": [51_200_000, 0] }));
    assert_eq!(
        loops[1][0],
        json!({ "id": "h3", "at": [6_400_000, 6_400_000] })
    );
    // A recipe whose boundary does not read is refused, naming why.
    let mut broken = floor_recipe(&l_with_hole());
    broken.args.insert("boundary".to_owned(), json!({}));
    assert!(filled(&broken, &slab()).is_err());
}
