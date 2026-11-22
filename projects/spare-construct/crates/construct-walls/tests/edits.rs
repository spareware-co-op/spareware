//! Editing a wall type through pure functions: each returns a valid new type or a refusal, and
//! the type it was called on never changes.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{finish, layer, membrane, mm, mms, structure, three};
use construct_params::Length;
use construct_walls::{Assembly, Function, Side, WallError, Zone};

/// The boundaries of a type.
fn core(wall: &Assembly) -> (usize, usize) {
    (wall.boundary(Side::Exterior), wall.boundary(Side::Interior))
}

/// The thicknesses of a type's layers.
fn thicknesses(wall: &Assembly) -> Vec<Length> {
    wall.layers().iter().map(|l| l.thickness).collect()
}

/// Inserted strictly inside the core, a layer joins the core.
#[test]
fn insert_inside_the_core() {
    let wall = Assembly::new(
        vec![finish(10), structure(20), structure(30), finish(5)],
        1,
        3,
    )
    .unwrap();
    let edited = wall.with_inserted(2, membrane(0)).unwrap();
    assert_eq!(thicknesses(&edited), mms(&[10, 20, 0, 30, 5]));
    assert_eq!(core(&edited), (1, 4));
    assert_eq!(edited.zone(2), Some(Zone::Core));
}

/// Inserted at a boundary, a layer goes into the shell beside it, never into the core.
#[test]
fn insert_at_a_boundary_goes_to_the_shell() {
    let wall = three();
    let at_exterior = wall.with_inserted(1, finish(7)).unwrap();
    assert_eq!(thicknesses(&at_exterior), mms(&[10, 7, 20, 30]));
    assert_eq!(core(&at_exterior), (2, 3));
    assert_eq!(at_exterior.zone(1), Some(Zone::ExteriorShell));
    let at_interior = wall.with_inserted(2, finish(7)).unwrap();
    assert_eq!(thicknesses(&at_interior), mms(&[10, 20, 7, 30]));
    assert_eq!(core(&at_interior), (1, 2));
    assert_eq!(at_interior.zone(2), Some(Zone::InteriorShell));
}

/// At either end the boundaries move with the layers.
#[test]
fn insert_at_the_ends() {
    let wall = three();
    let first = wall.with_inserted(0, finish(1)).unwrap();
    assert_eq!(thicknesses(&first), mms(&[1, 10, 20, 30]));
    assert_eq!(core(&first), (2, 3));
    let last = wall.with_inserted(3, finish(1)).unwrap();
    assert_eq!(thicknesses(&last), mms(&[10, 20, 30, 1]));
    assert_eq!(core(&last), (1, 2));
    assert_eq!(last.total(), mm(61));
    // An all-core wall: a layer inserted at either end is a shell layer.
    let solid = Assembly::new(vec![structure(200)], 0, 1).unwrap();
    assert_eq!(core(&solid.with_inserted(0, finish(10)).unwrap()), (1, 2));
    assert_eq!(core(&solid.with_inserted(1, finish(10)).unwrap()), (0, 1));
}

#[test]
fn insert_refusals() {
    let wall = three();
    assert_eq!(
        wall.with_inserted(4, finish(1)),
        Err(WallError::NoSuchLayer {
            layer: 4,
            layers: 3
        })
    );
    assert_eq!(
        wall.with_inserted(1, finish(0)),
        Err(WallError::ZeroThickness { layer: 1 })
    );
    let core_wall = Assembly::new(vec![structure(20), structure(30)], 0, 2).unwrap();
    assert_eq!(
        core_wall.with_inserted(1, layer(Function::Thermal, 10, "x", true)),
        Err(WallError::CoreLayerWraps { layer: 1 })
    );
    assert_eq!(
        wall.with_inserted(0, finish(9_941)),
        Err(WallError::TooThick { layer: 3 })
    );
    assert_eq!(wall, three(), "a refused edit changes nothing");
}

/// Removing a shell layer closes the boundaries round the gap.
#[test]
fn remove() {
    let wall = three();
    let no_outer = wall.without_layer(0).unwrap();
    assert_eq!(thicknesses(&no_outer), mms(&[20, 30]));
    assert_eq!(core(&no_outer), (0, 1));
    let no_inner = wall.without_layer(2).unwrap();
    assert_eq!(thicknesses(&no_inner), mms(&[10, 20]));
    assert_eq!(core(&no_inner), (1, 2));
    assert_eq!(wall, three());
}

#[test]
fn remove_refusals() {
    let wall = three();
    assert_eq!(
        wall.without_layer(1),
        Err(WallError::EmptyCore {
            exterior: 1,
            interior: 1
        })
    );
    assert_eq!(
        wall.without_layer(3),
        Err(WallError::NoSuchLayer {
            layer: 3,
            layers: 3
        })
    );
    let with_membrane = Assembly::new(
        vec![finish(10), membrane(0), structure(20), finish(5)],
        1,
        3,
    )
    .unwrap();
    assert_eq!(
        with_membrane.without_layer(2),
        Err(WallError::CoreWithoutThickness { first: 1, last: 1 })
    );
    let one = Assembly::new(vec![structure(20)], 0, 1).unwrap();
    assert_eq!(one.without_layer(0), Err(WallError::NoLayers));
}

/// A moved layer lands at its new index; the boundaries stay, so its zone is where it lands.
#[test]
fn reorder() {
    let wall = Assembly::new(
        vec![finish(1), finish(2), structure(3), finish(4), finish(5)],
        2,
        3,
    )
    .unwrap();
    let swapped = wall.with_moved(0, 1).unwrap();
    assert_eq!(thicknesses(&swapped), mms(&[2, 1, 3, 4, 5]));
    assert_eq!(core(&swapped), (2, 3));
    let to_back = wall.with_moved(0, 4).unwrap();
    assert_eq!(thicknesses(&to_back), mms(&[2, 3, 4, 5, 1]));
    // The finish now at index 2 is the core, and is thick, so the type is valid.
    assert_eq!(to_back.zone(2), Some(Zone::Core));
    let back_again = to_back.with_moved(4, 0).unwrap();
    assert_eq!(back_again, wall);
}

#[test]
fn reorder_refusals() {
    let wall = three();
    let out = WallError::NoSuchLayer {
        layer: 3,
        layers: 3,
    };
    assert_eq!(wall.with_moved(3, 0), Err(out.clone()));
    assert_eq!(wall.with_moved(0, 3), Err(out));
    let wrapping = Assembly::new(
        vec![
            layer(Function::Finish, 13, "board", true),
            structure(90),
            finish(13),
        ],
        1,
        2,
    )
    .unwrap();
    assert_eq!(
        wrapping.with_moved(0, 1),
        Err(WallError::CoreLayerWraps { layer: 1 })
    );
    let with_membrane = Assembly::new(vec![membrane(0), structure(20)], 1, 2).unwrap();
    assert_eq!(
        with_membrane.with_moved(0, 1),
        Err(WallError::CoreWithoutThickness { first: 1, last: 1 })
    );
}

#[test]
fn change() {
    let wall = three();
    let thicker = wall.with_changed(1, structure(140)).unwrap();
    assert_eq!(thicknesses(&thicker), mms(&[10, 140, 30]));
    assert_eq!(thicker.total(), mm(180));
    assert_eq!(
        wall.with_changed(1, membrane(0)),
        Err(WallError::CoreWithoutThickness { first: 1, last: 1 })
    );
    assert_eq!(
        wall.with_changed(0, finish(0)),
        Err(WallError::ZeroThickness { layer: 0 })
    );
    assert_eq!(
        wall.with_changed(1, layer(Function::Structure, 20, "x", true)),
        Err(WallError::CoreLayerWraps { layer: 1 })
    );
    assert_eq!(
        wall.with_changed(5, finish(1)),
        Err(WallError::NoSuchLayer {
            layer: 5,
            layers: 3
        })
    );
}

#[test]
fn move_a_boundary() {
    let wall = three();
    let wider = wall.with_boundary(Side::Exterior, 0).unwrap();
    assert_eq!(core(&wider), (0, 2));
    let all = wider.with_boundary(Side::Interior, 3).unwrap();
    assert_eq!(core(&all), (0, 3));
    assert_eq!(
        wall.with_boundary(Side::Exterior, 2),
        Err(WallError::EmptyCore {
            exterior: 2,
            interior: 2
        })
    );
    assert_eq!(
        wall.with_boundary(Side::Interior, 4),
        Err(WallError::BoundaryOutOfRange {
            side: Side::Interior,
            at: 4,
            layers: 3
        })
    );
    let wrapping = Assembly::new(
        vec![
            layer(Function::Finish, 13, "board", true),
            structure(90),
            finish(13),
        ],
        1,
        2,
    )
    .unwrap();
    assert_eq!(
        wrapping.with_boundary(Side::Exterior, 0),
        Err(WallError::CoreLayerWraps { layer: 0 })
    );
    let with_membrane = Assembly::new(vec![membrane(0), structure(20)], 0, 2).unwrap();
    assert_eq!(
        with_membrane.with_boundary(Side::Interior, 1),
        Err(WallError::CoreWithoutThickness { first: 0, last: 0 })
    );
}
