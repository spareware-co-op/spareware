//! The content library (`projects/spare-construct/library`, piece A90, CON-A7): furniture, casework,
//! plumbing fixtures, lighting, planting and a generic model, our own designs as family recipes.
//! Each validates against the published schema, reads, passes the checker, is in the one written
//! form, names the category and hosting its folder promises, and has two or three types; every
//! type builds through the recipe evaluator to the overall size its type gives (read back from
//! the solids' extents); and a flex sweep (the door generator's: both ends of each size's range
//! and a middle half a millimetre off the whole millimetre) builds every recipe at sizes between
//! the types. The plan symbols are where they belong, and everything is the same on a second run.
//! Judged through this crate's types only: the extents are the evaluator's own over a bounds
//! engine, no geometry engine is needed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::Fixture;
use construct_categories::{
    CASEWORK, CategoryId, FURNITURE, GENERIC_MODELS, LIGHTING_FIXTURES, PLANTING, PLUMBING_FIXTURES,
};
use construct_families::{
    Edge, Hosting, Key, Limits, ParameterValue, Recipe, SCHEMA, Turn, TypeValues, extents, symbols,
};
use construct_params::{Length, Value};

/// One library recipe: where it is, its text, its category and what it is placed on, which key
/// gives its overall width, depth and height, how far its lowest point is above its origin when
/// the instance holds its default, and the instance value that sets that.
struct Piece {
    folder: &'static str,
    stem: &'static str,
    text: &'static str,
    category: CategoryId,
    hosting: Hosting,
    keys: [&'static str; 3],
    /// Where the family's lowest point lies, in terms of the type's height and the instance value.
    low: Low,
    /// The range each of the three size keys is swept across, in whole millimetres.
    range: [(i64, i64); 3],
    /// How many types it has.
    types: usize,
}

/// Where a family's lowest point is.
#[derive(Clone, Copy)]
enum Low {
    /// On its origin's level.
    Floor,
    /// At the instance value `key`.
    At(&'static str),
    /// At the instance value `key` less the type's height.
    Below(&'static str),
}

macro_rules! piece {
    ($folder:literal, $stem:literal) => {
        include_str!(concat!("../../../library/", $folder, "/", $stem, ".json"))
    };
}

/// A recipe with the overall size under `width`, `depth` and `height`, three types, and the
/// ranges those sizes are swept across.
fn piece(
    (folder, stem, text): (&'static str, &'static str, &'static str),
    (category, hosting, low): (CategoryId, Hosting, Low),
    range: [(i64, i64); 3],
) -> Piece {
    Piece {
        folder,
        stem,
        text,
        category,
        hosting,
        keys: ["width", "depth", "height"],
        low,
        range,
        types: 3,
    }
}

fn pieces() -> Vec<Piece> {
    use Hosting::{Face, Unhosted};
    let tree = piece(
        ("planting", "tree", piece!("planting", "tree")),
        (PLANTING, Unhosted, Low::Floor),
        [(800, 6000), (800, 6000), (1500, 9000)],
    );
    vec![
        piece(
            ("furniture", "desk", piece!("furniture", "desk")),
            (FURNITURE, Unhosted, Low::Floor),
            [(900, 2400), (500, 1000), (700, 780)],
        ),
        piece(
            ("furniture", "chair", piece!("furniture", "chair")),
            (FURNITURE, Unhosted, Low::Floor),
            [(400, 700), (400, 750), (800, 950)],
        ),
        piece(
            ("furniture", "bed", piece!("furniture", "bed")),
            (FURNITURE, Unhosted, Low::Floor),
            [(900, 2000), (1800, 2200), (600, 1400)],
        ),
        piece(
            ("furniture", "sofa", piece!("furniture", "sofa")),
            (FURNITURE, Unhosted, Low::Floor),
            [(800, 2400), (800, 1000), (800, 1000)],
        ),
        piece(
            (
                "casework",
                "base-cabinet",
                piece!("casework", "base-cabinet"),
            ),
            (CASEWORK, Unhosted, Low::Floor),
            [(300, 1200), (500, 700), (800, 1000)],
        ),
        piece(
            (
                "casework",
                "wall-cabinet",
                piece!("casework", "wall-cabinet"),
            ),
            (CASEWORK, Face, Low::At("mount_height")),
            [(300, 1200), (250, 400), (400, 1000)],
        ),
        piece(
            ("plumbing", "wc-pan", piece!("plumbing", "wc-pan")),
            (PLUMBING_FIXTURES, Unhosted, Low::Floor),
            [(340, 420), (550, 800), (760, 900)],
        ),
        piece(
            ("plumbing", "basin", piece!("plumbing", "basin")),
            (PLUMBING_FIXTURES, Face, Low::Below("rim_height")),
            [(300, 800), (250, 550), (120, 250)],
        ),
        piece(
            ("plumbing", "shower-tray", piece!("plumbing", "shower-tray")),
            (PLUMBING_FIXTURES, Unhosted, Low::Floor),
            [(700, 1600), (700, 1000), (40, 120)],
        ),
        piece(
            (
                "lighting",
                "ceiling-light",
                piece!("lighting", "ceiling-light"),
            ),
            (LIGHTING_FIXTURES, Unhosted, Low::Below("mount_height")),
            [(150, 1500), (100, 700), (40, 120)],
        ),
        // The canopy is as wide one way as the other: one key gives both.
        Piece {
            keys: ["width", "width", "height"],
            ..tree
        },
        piece(
            ("generic", "generic-box", piece!("generic", "generic-box")),
            (GENERIC_MODELS, Unhosted, Low::Floor),
            [(100, 2400), (100, 2400), (100, 2400)],
        ),
    ]
}

fn read(text: &str) -> Recipe {
    Recipe::from_json(text).unwrap()
}

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

fn mm(n: i64) -> Length {
    Length::checked_from_mm(n).unwrap()
}

/// The default a recipe gives the parameter `parameter`, in stored text.
fn default_of(recipe: &Recipe, parameter: &str) -> String {
    recipe
        .parameters
        .iter()
        .find(|p| p.key.as_str() == parameter)
        .and_then(|p| match &p.value {
            ParameterValue::Default(text) => Some(text.clone()),
            ParameterValue::Formula(_) => None,
        })
        .unwrap_or_else(|| panic!("{parameter} has no default"))
}

/// A length parameter's stored value in a type's values, else the recipe's default for it.
fn length_of(recipe: &Recipe, values: &TypeValues, parameter: &str) -> Length {
    let stored = values
        .get(parameter)
        .map_or_else(|| default_of(recipe, parameter), str::to_owned);
    match Value::from_stored(&stored) {
        Some(Value::Length(length)) => length,
        other => panic!("{parameter} is not a length: {other:?}"),
    }
}

/// The overall box of every solid a type builds, low and high corners in grid steps.
fn overall(recipe: &Recipe, values: &TypeValues) -> ([i64; 3], [i64; 3]) {
    let boxes = extents(recipe, values, Limits::DEFAULT, Turn::None).unwrap();
    let [_, ..] = boxes.as_slice() else {
        panic!("a recipe builds a solid");
    };
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for b in &boxes {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(b.low[axis].steps());
            hi[axis] = hi[axis].max(b.high[axis].steps());
        }
    }
    (lo, hi)
}

/// The instance value `parameter` as a recipe holds it by default.
fn instance_default(recipe: &Recipe, parameter: &str) -> Length {
    length_of(recipe, &TypeValues::new(), parameter)
}

/// What the overall box of `piece` at `values` must be: centred on its origin across, its depth
/// across the origin (or from the origin outwards for a family on a face), its height from where
/// its lowest point lies.
fn holds(piece: &Piece, recipe: &Recipe, values: &TypeValues, what: &str) {
    let (lo, hi) = overall(recipe, values);
    let [w, d, h] = piece.keys.map(|k| length_of(recipe, values, k).steps());
    assert_eq!(hi[0] - lo[0], w, "{what}: width");
    assert_eq!(hi[1] - lo[1], d, "{what}: depth");
    assert_eq!(hi[2] - lo[2], h, "{what}: height");
    assert_eq!(lo[0], -w / 2, "{what}: centred along x");
    assert_eq!(hi[0], w / 2, "{what}: centred along x");
    let (y_lo, y_hi) = if piece.hosting == Hosting::Face {
        (0, d)
    } else {
        (-d / 2, d / 2)
    };
    // An odd step count has no exact half: allow the one step the half rounds by.
    assert!(
        (lo[1] - y_lo).abs() <= 1 && (hi[1] - y_hi).abs() <= 1,
        "{what}: across"
    );
    let z_lo = match piece.low {
        Low::Floor => 0,
        Low::At(k) => instance_default(recipe, k).steps(),
        Low::Below(k) => instance_default(recipe, k).steps() - h,
    };
    assert_eq!(lo[2], z_lo, "{what}: lowest point");
}

#[test]
fn every_recipe_validates_reads_checks_and_is_written_in_its_one_form() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let fixture = Fixture::new();
    for piece in pieces() {
        let name = format!("{}/{}", piece.folder, piece.stem);
        let value: serde_json::Value = serde_json::from_str(piece.text).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
        let recipe = read(piece.text);
        assert_eq!(recipe.check(&fixture.library()), Ok(()), "{name}");
        assert_eq!(
            recipe.to_json(),
            piece.text,
            "{name} is not in the written form"
        );
        assert_eq!(
            value["id"],
            format!("library.{}.{}", piece.folder, piece.stem),
            "{name}: the id follows the naming rule"
        );
        assert_eq!(recipe.category, piece.category, "{name}: category");
        assert_eq!(recipe.hosting, piece.hosting, "{name}: hosting");
        let types = recipe.named_types();
        assert_eq!(types.len(), piece.types, "{name}: its types");
        assert!(
            (2..=3).contains(&types.len()),
            "{name} has two or three types"
        );
        // Every step is in the family's category (these families have no subcategories).
        for step in &recipe.steps {
            assert_eq!(step.subcategory, piece.category, "{name}: {}", step.key);
        }
    }
}

#[test]
fn every_type_builds_to_the_overall_size_it_gives() {
    for piece in pieces() {
        let recipe = read(piece.text);
        for ty in recipe.named_types() {
            let values = TypeValues::of(&recipe, ty.key()).unwrap();
            holds(
                &piece,
                &recipe,
                &values,
                &format!("{} {}", piece.stem, ty.key()),
            );
        }
    }
}

/// Both ends of a range and a middle half a millimetre off the whole millimetre.
fn sweep((low, high): (i64, i64)) -> [Length; 3] {
    let middle =
        Length::from_steps(mm(low).steps() + (mm(high).steps() - mm(low).steps()) / 2 + 3200);
    [mm(low), middle, mm(high)]
}

#[test]
fn the_flex_sweep_builds_every_recipe_across_its_sizes() {
    let fixture = Fixture::new();
    for piece in pieces() {
        let recipe = read(piece.text);
        assert_eq!(recipe.check(&fixture.library()), Ok(()));
        let first = recipe.named_types()[0].key().clone();
        let base = TypeValues::of(&recipe, &first).unwrap();
        let widths = sweep(piece.range[0]);
        let depths = sweep(piece.range[1]);
        let heights = sweep(piece.range[2]);
        let same = piece.keys[0] == piece.keys[1];
        for (i, w) in widths.iter().enumerate() {
            for (j, d) in depths.iter().enumerate() {
                // Each corner and the middle: low/low, middle/middle, high/high, and both mixes.
                if !(i == j || i == 0 && j == 2 || i == 2 && j == 0) {
                    continue;
                }
                if same && i != j {
                    continue;
                }
                for h in heights {
                    let mut values = base.clone();
                    values.set(key(piece.keys[0]), Value::Length(*w).stored());
                    if !same {
                        values.set(key(piece.keys[1]), Value::Length(*d).stored());
                    }
                    values.set(key(piece.keys[2]), Value::Length(h).stored());
                    holds(
                        &piece,
                        &recipe,
                        &values,
                        &format!("{} at {w} x {d} x {h}", piece.stem),
                    );
                }
            }
        }
    }
}

/// A point of a symbol in whole steps.
fn steps(p: [Length; 2]) -> [i64; 2] {
    [p[0].steps(), p[1].steps()]
}

#[test]
fn the_plan_symbols_outline_the_footprint_and_the_detail_lies_inside_it() {
    for piece in pieces() {
        let recipe = read(piece.text);
        for ty in recipe.named_types() {
            let what = format!("{} {}", piece.stem, ty.key());
            let values = TypeValues::of(&recipe, ty.key()).unwrap();
            let drawn = symbols(&recipe, &values).unwrap().symbols;
            let [w, d, _] = piece.keys.map(|k| length_of(&recipe, &values, k).steps());
            let outline = drawn
                .iter()
                .find(|s| s.symbol.as_str() == "plan_outline")
                .unwrap_or_else(|| panic!("{what}: a plan outline"));
            let mut lo = [i64::MAX; 2];
            let mut hi = [i64::MIN; 2];
            for e in &outline.edges {
                let (a, b) = match e {
                    Edge::Line { start, end, .. } => (steps(*start), steps(*end)),
                    other => panic!("{what}: the outline is lines, not {other:?}"),
                };
                for p in [a, b] {
                    for axis in 0..2 {
                        lo[axis] = lo[axis].min(p[axis]);
                        hi[axis] = hi[axis].max(p[axis]);
                    }
                }
            }
            assert_eq!(hi[0] - lo[0], w, "{what}: outline width");
            assert_eq!(hi[1] - lo[1], d, "{what}: outline depth");
            // Every other symbol is a plan symbol inside it, a line at a time.
            for s in &drawn {
                for e in &s.edges {
                    if let Edge::Line { start, end, .. } = e {
                        for p in [steps(*start), steps(*end)] {
                            for axis in 0..2 {
                                assert!(
                                    p[axis] >= lo[axis] && p[axis] <= hi[axis],
                                    "{what}: {} leaves the outline",
                                    s.symbol
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_same_recipes_give_the_same_bytes_and_sizes_on_a_second_run() {
    for piece in pieces() {
        let first = read(piece.text);
        let second = read(&first.to_json());
        assert_eq!(first.content_id(), second.content_id(), "{}", piece.stem);
        for ty in first.named_types() {
            let values = TypeValues::of(&first, ty.key()).unwrap();
            assert_eq!(overall(&first, &values), overall(&second, &values));
        }
    }
}

#[test]
fn a_family_hosted_on_a_face_has_its_back_on_the_origin_plane_and_an_instance_height() {
    for piece in pieces().iter().filter(|p| p.hosting == Hosting::Face) {
        let recipe = read(piece.text);
        let values = TypeValues::of(&recipe, recipe.named_types()[0].key()).unwrap();
        let (lo, _) = overall(&recipe, &values);
        assert_eq!(lo[1], 0, "{}: its back is on the face", piece.stem);
        assert_eq!(
            recipe
                .instance_parameters(&construct_params::Units::default())
                .len(),
            1,
            "{}: one instance value, the height it hangs at",
            piece.stem
        );
    }
}
