//! The values a placed instance holds (CON-A7-F3-P2, piece A64, job 7.1): the parameters each
//! instance holds are offered with their defaults; a typed value is read as its parameter reads
//! it and refused in words for a parameter the type holds, one worked out, one the recipe lacks or
//! a value of the wrong kind; evaluation takes an instance's value (a window's opening rises with
//! its sill height) and refuses one the type holds; and the same recipe with the same values is
//! the same cache key, a changed instance value another.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_families::{CacheKey, Key, Reason, Recipe, TypeValues, opening};
use construct_params::{Kind, Length, Units};

const FIXED: &str = include_str!("../../../library/doors-windows/fixed-window.json");
const DOOR: &str = include_str!("../../../library/doors-windows/single-swing-door.json");

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// A window holds its sill height itself, a door its swing; each is offered with its default as a
/// person reads it, and a choice with its choices.
#[test]
fn the_parameters_each_instance_holds_are_offered_with_their_defaults() {
    let window = Recipe::from_json(FIXED).unwrap();
    let held = window.instance_parameters(&Units::default());
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].key, key("sill_height"));
    assert_eq!(held[0].name, "Sill height");
    assert_eq!(held[0].kind, Kind::Length);
    assert_eq!(held[0].shown, "900 mm");
    assert_eq!(held[0].stored, "length:q0:5760000");
    let door = Recipe::from_json(DOOR).unwrap();
    let held = door.instance_parameters(&Units::default());
    assert_eq!(held[0].key, key("swing"));
    assert_eq!(held[0].shown, "Hinged on the left");
    assert_eq!(held[0].choices.len(), 2);
}

/// What is typed is read as the parameter reads it, and each refusal says why in words.
#[test]
fn a_typed_value_is_read_or_refused_in_words() {
    let window = Recipe::from_json(FIXED).unwrap();
    let units = Units::default();
    let sill = key("sill_height");
    assert_eq!(
        window.instance_value(&sill, "1.1 m", &units).unwrap(),
        "length:q0:7040000"
    );
    assert_eq!(
        window.instance_value(&sill, "1100", &units).unwrap(),
        "length:q0:7040000"
    );
    assert_eq!(
        window
            .instance_value(&key("width"), "1 m", &units)
            .unwrap_err(),
        "Width belongs to the type of Fixed window: change its type to change it."
    );
    assert_eq!(
        window
            .instance_value(&key("glass_width"), "1 m", &units)
            .unwrap_err(),
        "Glass width is worked out, so nothing sets it."
    );
    assert_eq!(
        window
            .instance_value(&key("colour"), "red", &units)
            .unwrap_err(),
        "Fixed window has no value called colour to set on a placed one."
    );
    assert!(window.instance_value(&sill, "tall", &units).is_err());
    let door = Recipe::from_json(DOOR).unwrap();
    assert_eq!(
        door.instance_value(&key("swing"), "Hinged on the right", &units)
            .unwrap(),
        "choice:right"
    );
}

/// A window's opening rises with its sill height, which an instance gives; a type may not give
/// it, nor an instance a value the type holds.
#[test]
fn evaluation_takes_an_instances_value_and_refuses_the_wrong_holder() {
    let window = Recipe::from_json(FIXED).unwrap();
    let ty = key("w1200x1200");
    let plain = TypeValues::of(&window, &ty).unwrap();
    let low = opening(&window, &plain).unwrap();
    let z = |o: &construct_families::SolvedOpening| {
        let zs: Vec<Length> = o.outline.iter().map(|p| p[1]).collect();
        (*zs.iter().min().unwrap(), *zs.iter().max().unwrap())
    };
    assert_eq!(z(&low), (mm(900), mm(2100)));
    let raised = plain
        .clone()
        .with_instance(key("sill_height"), "length:q0:7040000");
    let high = opening(&window, &raised).unwrap();
    assert_eq!(z(&high), (mm(1100), mm(2300)));
    // an instance may not set what the type holds
    let wrong = plain
        .clone()
        .with_instance(key("width"), "length:q0:6400000");
    let refused = opening(&window, &wrong).unwrap_err();
    assert_eq!(refused.reason(), &Reason::HeldByType);
    // and a type may not set what each instance holds, as before
    let wrong = plain.with(key("sill_height"), "length:q0:6400000");
    assert_eq!(
        opening(&window, &wrong).unwrap_err().reason(),
        &Reason::HeldByInstance
    );
}

/// An instance's value is part of what a built family is kept under: the same values are one key,
/// a changed value is another.
#[test]
fn an_instance_value_is_part_of_the_cache_key() {
    let window = Recipe::from_json(FIXED).unwrap();
    let plain = TypeValues::of(&window, &key("w1200x1200")).unwrap();
    let raised = plain
        .clone()
        .with_instance(key("sill_height"), "length:q0:7040000");
    let again = plain
        .clone()
        .with_instance(key("sill_height"), "length:q0:7040000");
    assert_ne!(
        CacheKey::of(&window, &plain),
        CacheKey::of(&window, &raised)
    );
    assert_eq!(
        CacheKey::of(&window, &raised),
        CacheKey::of(&window, &again)
    );
    assert_eq!(raised.instance_values().count(), 1);
    assert_eq!(raised.get("sill_height"), Some("length:q0:7040000"));
}
