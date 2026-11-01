//! Fuzz targets for building shapes. Each runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package construct-regen` (the `kit` workflow).

use bolero::check;
use construct_model::geometry::{Geometry, Recipe};
use construct_regen::stand_in::Boxes;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const OPS: [&str; 4] = ["wall.straight", "door.single", "block", "not.an.operation"];
const NAMES: [&str; 7] = [
    "length",
    "thickness",
    "height",
    "width",
    "depth",
    "axis",
    "other",
];

/// A recipe from fuzzer input: real operations and argument names, with values of any kind.
fn recipe(op: u8, args: &[(u8, i64, bool)]) -> Recipe {
    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    for (name, number, text) in args {
        let name = NAMES[usize::from(*name) % NAMES.len()];
        let value = if *text {
            json!(number.to_string())
        } else {
            json!(number)
        };
        map.insert(name.to_owned(), value);
    }
    Recipe::construct(OPS[usize::from(op) % OPS.len()], map)
}

/// Whatever a recipe says, building gives an answer, the same one every time, and one the rest of
/// the program can rely on: bounds that hold every outline point, and meshes with valid indices.
#[test]
fn any_recipe_builds_or_says_why() {
    check!()
        .with_type::<(u8, Vec<(u8, i64, bool)>)>()
        .for_each(|(op, args)| {
            let recipe = recipe(*op, args);
            let (first, second) = (Boxes.build(&recipe), Boxes.build(&recipe));
            assert_eq!(first, second, "a build is a pure function");
            let Ok(built) = first else { return };
            assert!(!built.body.is_empty());
            assert!(!built.displays.is_empty());
            for display in &built.displays {
                let points = (display.mesh.positions.len() / 3) as u32;
                assert_eq!(display.mesh.positions.len() % 3, 0);
                assert!(display.mesh.indices.iter().all(|index| *index < points));
                for [x, y] in &display.outline {
                    assert!((built.bounds.min[0]..=built.bounds.max[0]).contains(x));
                    assert!((built.bounds.min[1]..=built.bounds.max[1]).contains(y));
                }
            }
        });
}

/// Any bytes at all fail to read as a recipe, or read as one that builds or says why.
#[test]
fn reading_any_recipe_text_is_safe() {
    check!().for_each(|bytes: &[u8]| {
        if let Ok(recipe) = serde_json::from_slice::<Recipe>(bytes) {
            let _ = Boxes.build(&recipe);
        }
    });
}
