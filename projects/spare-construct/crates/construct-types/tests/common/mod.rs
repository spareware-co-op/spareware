//! The parameters, categories and types the tests share: a door category with two door types
//! and a wall type, and five door parameters, two held by the type and three by each instance.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_categories::{CategoryTable, DOORS, WALLS};
use construct_params::{Definition, Holder, Kind, Length, Value};
use construct_types::{ElementType, Parameters, Types};
use kit_core::Id;

/// Text as an ID.
pub fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A length value.
pub fn mm(mm: i32) -> Value {
    Value::Length(Length::from_mm(mm))
}

/// A text value.
pub fn text(text: &str) -> Value {
    Value::Text(text.to_owned())
}

/// Held by the type, default 900 mm.
pub const WIDTH: &str = "P-width";
/// Held by the type, no default.
pub const HEIGHT: &str = "P-height";
/// Held by each instance, default 0 mm.
pub const SILL: &str = "P-sill";
/// Held by each instance, no default.
pub const MARK: &str = "P-mark";
/// Held by each instance, no default; only the first door type has it.
pub const FINISH: &str = "P-finish";

/// The five door parameters, with their defaults.
pub fn parameters() -> Parameters {
    let mut params = Parameters::new();
    let def = |pid, name, kind, holder| Definition::new(id(pid), name, kind, holder).unwrap();
    params
        .add(
            def(WIDTH, "Width", Kind::Length, Holder::Type),
            Some(mm(900)),
        )
        .unwrap();
    params
        .add(def(HEIGHT, "Height", Kind::Length, Holder::Type), None)
        .unwrap();
    params
        .add(
            def(SILL, "Sill height", Kind::Length, Holder::Instance),
            Some(mm(0)),
        )
        .unwrap();
    params
        .add(def(MARK, "Mark", Kind::Text, Holder::Instance), None)
        .unwrap();
    params
        .add(def(FINISH, "Finish", Kind::Text, Holder::Instance), None)
        .unwrap();
    params
}

/// A type with the given parameters (by ID) and no values of its own.
pub fn door_type(type_id: &str, name: &str, with: &[&str], params: &Parameters) -> ElementType {
    let mut ty = ElementType::new(id(type_id), name, DOORS, &CategoryTable::new()).unwrap();
    for pid in with {
        ty.add_parameter(params, &id(pid)).unwrap();
    }
    ty
}

/// Two door types and a wall type:
/// - `T-820`, "Single 820": every door parameter, width 820 mm;
/// - `T-900`, "Single 900": all but the finish, no width of its own;
/// - `T-wall`, "Partition 90": a wall type with the sill and mark only.
pub fn types(params: &Parameters) -> Types {
    let mut first = door_type(
        "T-820",
        "Single 820",
        &[WIDTH, HEIGHT, SILL, MARK, FINISH],
        params,
    );
    first.set(params, &id(WIDTH), mm(820)).unwrap();
    let second = door_type("T-900", "Single 900", &[WIDTH, HEIGHT, SILL, MARK], params);
    let mut wall =
        ElementType::new(id("T-wall"), "Partition 90", WALLS, &CategoryTable::new()).unwrap();
    wall.add_parameter(params, &id(SILL)).unwrap();
    wall.add_parameter(params, &id(MARK)).unwrap();
    let mut types = Types::new();
    types.add(first).unwrap();
    types.add(second).unwrap();
    types.add(wall).unwrap();
    types
}
