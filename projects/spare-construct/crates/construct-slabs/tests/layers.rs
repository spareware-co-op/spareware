//! What makes a floor type's layers valid: every refusal of `Layers::new`, the total and the
//! depth of each face, the level rule, and the functions shared with a wall's layers.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{layer, mm, slab};
use construct_materials::Materials;
use construct_params::Length;
use construct_slabs::{Function, Layers, MOST_TOTAL, SlabError};

/// Two layers, top first: their total, the depth of every face below the top, and the height
/// of every face when the top stands at a level's elevation plus its offset.
#[test]
fn total_depths_and_the_level_rule() {
    let floor = slab();
    assert_eq!(floor.layers().len(), 2);
    assert_eq!(floor.total(), mm(200));
    assert_eq!(floor.depths(), [mm(0), mm(50), mm(200)]);
    assert_eq!(floor.layer(1).unwrap().function, Function::Structure);
    assert_eq!(floor.layer(2), None);
    // The top of the floor is at the level plus its offset; the layers hang below it.
    assert_eq!(
        floor.face_heights(mm(3200)).unwrap(),
        [mm(3200), mm(3150), mm(3000)]
    );
    assert_eq!(
        floor.face_heights(mm(-100)).unwrap(),
        [mm(-100), mm(-150), mm(-300)]
    );
    assert_eq!(
        floor.face_heights(Length::from_steps(i64::MIN + 5)),
        Err(SlabError::OutOfRange)
    );
}

/// A membrane may be 0 thick and still counts as a layer; a lengths in steps finer than a
/// millimetre are kept exactly.
#[test]
fn a_membrane_may_be_nothing_thick_and_lengths_are_exact() {
    let floor = Layers::new(vec![
        layer(Function::Finish, 50, "screed-cement"),
        layer(Function::Membrane, 0, "membrane-polyethylene"),
        construct_slabs::FloorLayer::new(
            Function::Structure,
            Length::from_steps(80_000),
            construct_materials::MaterialId::parse("concrete-regular").unwrap(),
        ),
    ])
    .unwrap();
    assert_eq!(floor.total(), Length::from_steps(50 * 6400 + 80_000));
    assert_eq!(floor.depths()[1], floor.depths()[2]);
}

/// Every refusal names the layer, counted from the top (0 first), checked in order.
#[test]
fn every_refusal_names_its_layer() {
    assert_eq!(Layers::new(Vec::new()), Err(SlabError::NoLayers));
    let below = construct_slabs::FloorLayer::new(
        Function::Finish,
        Length::from_steps(-1),
        construct_materials::MaterialId::parse("screed-cement").unwrap(),
    );
    assert_eq!(
        Layers::new(vec![
            layer(Function::Structure, 150, "concrete-regular"),
            below
        ]),
        Err(SlabError::NegativeThickness { layer: 1 })
    );
    assert_eq!(
        Layers::new(vec![layer(Function::Finish, 0, "screed-cement")]),
        Err(SlabError::ZeroThickness { layer: 0 })
    );
    assert_eq!(
        Layers::new(vec![
            layer(Function::Structure, 9_000, "concrete-regular"),
            layer(Function::Structure, 1_001, "concrete-regular"),
        ]),
        Err(SlabError::TooThick { layer: 1 })
    );
    assert_eq!(
        Layers::new(vec![layer(Function::Structure, 10_000, "concrete-regular")])
            .unwrap()
            .total(),
        MOST_TOTAL
    );
    assert_eq!(
        Layers::new(vec![
            layer(Function::Membrane, 0, "membrane-polyethylene"),
            layer(Function::Membrane, 0, "membrane-polyethylene"),
        ]),
        Err(SlabError::NoThickness)
    );
    let messages = [
        (SlabError::NoLayers, "a floor type needs at least one layer"),
        (
            SlabError::ZeroThickness { layer: 2 },
            "layer 2 is 0 thick; only a membrane may be 0",
        ),
        (
            SlabError::NoThickness,
            "a floor type needs a layer thicker than 0",
        ),
    ];
    for (error, text) in messages {
        assert_eq!(error.to_string(), text);
    }
}

/// The layers whose material a library lacks, by index from the top.
#[test]
fn missing_materials_by_layer() {
    let library = Materials::default_metric();
    assert_eq!(slab().missing_materials(&library), vec![0]);
}

/// A floor layer's functions are a wall layer's: the same five, the same stored names and the
/// same join priorities, so a floor meeting a wall settles each pair of layers by one rule. The
/// two lists live in two crates until they share one (the decision note); this keeps them equal.
#[test]
fn the_functions_are_a_walls() {
    let ours: Vec<(String, Option<u8>, bool)> = Function::ALL
        .iter()
        .map(|f| {
            (
                serde_json::to_string(f).unwrap(),
                f.priority(),
                f.may_be_zero(),
            )
        })
        .collect();
    let walls: Vec<(String, Option<u8>, bool)> = construct_walls::Function::ALL
        .iter()
        .map(|f| {
            (
                serde_json::to_string(f).unwrap(),
                f.priority(),
                f.may_be_zero(),
            )
        })
        .collect();
    assert_eq!(ours, walls);
}
