//! What makes a wall type's layers and core valid: every refusal of `Assembly::new`, the zones,
//! wrapping, and the join rule from each function's priority.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use std::cmp::Ordering;

use common::{finish, layer, membrane, mm, structure, three};
use construct_walls::{Assembly, Function, Join, MOST_TOTAL, Side, WallError, Zone, join};

/// The three zones, the core test and the total of a valid type.
#[test]
fn zones_core_and_total() {
    let wall = three();
    assert_eq!(wall.layers().len(), 3);
    assert_eq!(wall.boundary(Side::Exterior), 1);
    assert_eq!(wall.boundary(Side::Interior), 2);
    assert_eq!(wall.zone(0), Some(Zone::ExteriorShell));
    assert_eq!(wall.zone(1), Some(Zone::Core));
    assert_eq!(wall.zone(2), Some(Zone::InteriorShell));
    assert_eq!(wall.zone(3), None);
    assert!(!wall.in_core(0) && wall.in_core(1) && !wall.in_core(2) && !wall.in_core(3));
    assert_eq!(wall.total(), mm(60));
    assert_eq!(wall.layer(1), Some(&structure(20)));
    assert_eq!(wall.layer(3), None);
}

/// A wall that is all core has no shells.
#[test]
fn all_core() {
    let wall = Assembly::new(vec![structure(100), structure(50)], 0, 2).unwrap();
    assert!(wall.in_core(0) && wall.in_core(1));
    assert_eq!(wall.total(), mm(150));
}

#[test]
fn refuses_no_layers() {
    assert_eq!(Assembly::new(vec![], 0, 0), Err(WallError::NoLayers));
}

/// Only a membrane may be 0 thick; the refusal names the first layer that breaks it.
#[test]
fn refuses_zero_thickness_but_a_membrane() {
    for function in [
        Function::Structure,
        Function::Substrate,
        Function::Thermal,
        Function::Finish,
    ] {
        let layers = vec![finish(10), structure(20), layer(function, 0, "x", false)];
        assert_eq!(
            Assembly::new(layers, 1, 2),
            Err(WallError::ZeroThickness { layer: 2 })
        );
    }
    let wall = Assembly::new(vec![membrane(0), structure(20)], 1, 2).unwrap();
    assert_eq!(wall.total(), mm(20));
    let thick_membrane = Assembly::new(vec![membrane(1), structure(20)], 1, 2).unwrap();
    assert_eq!(thick_membrane.total(), mm(21));
}

#[test]
fn refuses_negative_thickness() {
    assert_eq!(
        Assembly::new(vec![structure(20), membrane(-1)], 0, 1),
        Err(WallError::NegativeThickness { layer: 1 })
    );
}

/// The total may be 10 m and no more; the refusal names the layer that passes it.
#[test]
fn refuses_more_than_ten_metres() {
    assert_eq!(MOST_TOTAL, mm(10_000));
    let wall = Assembly::new(vec![structure(6_000), finish(4_000)], 0, 1).unwrap();
    assert_eq!(wall.total(), MOST_TOTAL);
    assert_eq!(
        Assembly::new(vec![structure(6_000), finish(4_001)], 0, 1),
        Err(WallError::TooThick { layer: 1 })
    );
    assert_eq!(
        Assembly::new(vec![structure(i64::MAX), finish(i64::MAX)], 0, 1),
        Err(WallError::TooThick { layer: 0 })
    );
}

#[test]
fn refuses_a_boundary_past_the_last_layer() {
    let layers = || vec![finish(10), structure(20), finish(30)];
    assert_eq!(
        Assembly::new(layers(), 4, 4),
        Err(WallError::BoundaryOutOfRange {
            side: Side::Exterior,
            at: 4,
            layers: 3
        })
    );
    assert_eq!(
        Assembly::new(layers(), 1, 4),
        Err(WallError::BoundaryOutOfRange {
            side: Side::Interior,
            at: 4,
            layers: 3
        })
    );
}

#[test]
fn refuses_an_empty_core() {
    let layers = || vec![finish(10), structure(20), finish(30)];
    for (exterior, interior) in [(1, 1), (2, 1), (3, 3), (0, 0)] {
        assert_eq!(
            Assembly::new(layers(), exterior, interior),
            Err(WallError::EmptyCore { exterior, interior })
        );
    }
}

/// A core of membranes alone has no thickness.
#[test]
fn refuses_a_core_without_thickness() {
    let layers = vec![finish(10), membrane(0), membrane(0), structure(20)];
    assert_eq!(
        Assembly::new(layers, 1, 3),
        Err(WallError::CoreWithoutThickness { first: 1, last: 2 })
    );
}

/// A shell layer may wrap; a core layer may not, and the refusal names it.
#[test]
fn only_a_shell_layer_wraps() {
    let wrapping = |function, thickness| layer(function, thickness, "x", true);
    let wall = Assembly::new(
        vec![
            wrapping(Function::Finish, 10),
            structure(20),
            wrapping(Function::Membrane, 0),
            wrapping(Function::Finish, 10),
        ],
        1,
        2,
    )
    .unwrap();
    assert_eq!(wall.wrapping(), vec![0, 2, 3]);
    assert_eq!(
        Assembly::new(
            vec![finish(10), structure(20), wrapping(Function::Structure, 20)],
            1,
            3
        ),
        Err(WallError::CoreLayerWraps { layer: 2 })
    );
}

/// The refusals are checked in a fixed order, so one input always gives the same refusal.
#[test]
fn first_broken_rule_is_refused() {
    let layers = vec![finish(0), layer(Function::Structure, 20, "x", true)];
    assert_eq!(
        Assembly::new(layers, 5, 1),
        Err(WallError::ZeroThickness { layer: 0 })
    );
}

/// The messages name the layer.
#[test]
fn messages_name_the_layer() {
    assert_eq!(
        WallError::ZeroThickness { layer: 2 }.to_string(),
        "layer 2 is 0 thick; only a membrane may be 0"
    );
    assert_eq!(
        WallError::CoreLayerWraps { layer: 1 }.to_string(),
        "layer 1 is in the core and so cannot wrap; only a shell layer wraps"
    );
}

/// The standard order: structure 1, substrate 2, insulation or air 3, finish 4, membrane none.
#[test]
fn priorities_pinned() {
    let priorities: Vec<Option<u8>> = Function::ALL.iter().map(|f| f.priority()).collect();
    assert_eq!(priorities, vec![Some(1), Some(2), Some(3), Some(4), None]);
    assert!(Function::Membrane.may_be_zero());
    assert!(!Function::Finish.may_be_zero());
}

/// The stronger priority cuts the weaker, whichever wall is first; a membrane never cuts.
#[test]
fn join_stronger_cuts_weaker() {
    for (a, first) in Function::ALL.iter().enumerate() {
        for (b, second) in Function::ALL.iter().enumerate() {
            let x = layer(*first, 10, "one", false);
            let y = layer(*second, 10, "two", false);
            let expected = match a.cmp(&b) {
                Ordering::Less => Join::FirstCuts,
                Ordering::Greater => Join::SecondCuts,
                Ordering::Equal => Join::Meet {
                    drawn_as_one: false,
                },
            };
            assert_eq!(join(&x, &y), expected, "{first:?} against {second:?}");
        }
    }
}

/// A tie meets on the join line; of one material it is drawn as one.
#[test]
fn join_tie_rule() {
    let a = layer(Function::Finish, 13, "board", false);
    let b = layer(Function::Finish, 10, "board", true);
    let c = layer(Function::Finish, 13, "paint", false);
    assert_eq!(join(&a, &b), Join::Meet { drawn_as_one: true });
    assert_eq!(
        join(&a, &c),
        Join::Meet {
            drawn_as_one: false
        }
    );
    let wrap = layer(Function::Membrane, 0, "wrap", true);
    assert_eq!(join(&wrap, &wrap), Join::Meet { drawn_as_one: true });
    assert_eq!(join(&wrap, &a), Join::SecondCuts);
}
