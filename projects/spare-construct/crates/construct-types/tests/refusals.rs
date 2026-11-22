//! Values put where they do not belong are refused as error values, and a refusal changes
//! nothing.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{FINISH, HEIGHT, MARK, SILL, WIDTH, id, mm, parameters, text, types};
use construct_params::{Definition, Holder, Kind, ParamError};
use construct_types::{Instance, Parameters, TypeError};

/// A parameter the type holds cannot be set on an instance.
#[test]
fn type_parameter_refused_on_instance() {
    let params = parameters();
    let types = types(&params);
    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);
    let before = door.clone();
    assert_eq!(
        door.set(&params, ty, &id(WIDTH), mm(1000)).unwrap_err(),
        TypeError::HeldByType {
            parameter: "Width".to_owned(),
        }
    );
    assert_eq!(
        door.set(&params, ty, &id(HEIGHT), mm(2100)).unwrap_err(),
        TypeError::HeldByType {
            parameter: "Height".to_owned(),
        }
    );
    assert_eq!(door, before);
}

/// A parameter each instance holds cannot be set on the type.
#[test]
fn instance_parameter_refused_on_type() {
    let params = parameters();
    let mut types = types(&params);
    let ty = types.get_mut(&id("T-820")).unwrap();
    let before = ty.clone();
    assert_eq!(
        ty.set(&params, &id(SILL), mm(10)).unwrap_err(),
        TypeError::HeldByInstance {
            parameter: "Sill height".to_owned(),
        }
    );
    assert_eq!(*ty, before);
}

/// A value of the wrong kind is refused with construct-params' own error.
#[test]
fn wrong_kind_refused() {
    let params = parameters();
    let mut types = types(&params);
    let ty = types.get_mut(&id("T-820")).unwrap();
    assert_eq!(
        ty.set(&params, &id(WIDTH), text("wide")).unwrap_err(),
        TypeError::Param(ParamError::WrongKind {
            parameter: "Width".to_owned(),
            expected: Kind::Length,
            found: Kind::Text,
        })
    );
    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);
    assert_eq!(
        door.set(&params, ty, &id(MARK), mm(1)).unwrap_err(),
        TypeError::Param(ParamError::WrongKind {
            parameter: "Mark".to_owned(),
            expected: Kind::Text,
            found: Kind::Length,
        })
    );
}

/// A parameter the type does not have is refused on the type and on its instances; one nobody
/// defined is refused everywhere.
#[test]
fn parameter_not_on_the_type_refused() {
    let params = parameters();
    let mut types = types(&params);
    let second = types.get(&id("T-900")).unwrap();
    let mut door = Instance::new(second);
    assert_eq!(
        door.set(&params, second, &id(FINISH), text("oiled"))
            .unwrap_err(),
        TypeError::NotOnType {
            type_name: "Single 900".to_owned(),
            parameter: "Finish".to_owned(),
        }
    );
    let first = types.get(&id("T-820")).unwrap();
    assert_eq!(
        door.set(&params, first, &id(MARK), text("D1")).unwrap_err(),
        TypeError::NotItsType {
            instance_type: id("T-900"),
            given: id("T-820"),
        }
    );
    let wall = types.get_mut(&id("T-wall")).unwrap();
    assert_eq!(
        wall.set(&params, &id(WIDTH), mm(90)).unwrap_err(),
        TypeError::NotOnType {
            type_name: "Partition 90".to_owned(),
            parameter: "Width".to_owned(),
        }
    );
    assert_eq!(
        wall.add_parameter(&params, &id("P-nobody")).unwrap_err(),
        TypeError::UnknownParameter(id("P-nobody"))
    );
    assert_eq!(
        wall.set(&params, &id("P-nobody"), mm(90)).unwrap_err(),
        TypeError::UnknownParameter(id("P-nobody"))
    );
}

/// A parameter is defined once, and its default must be a value it takes.
#[test]
fn parameter_set_refusals() {
    let mut params = Parameters::new();
    let width = Definition::new(id(WIDTH), "Width", Kind::Length, Holder::Type).unwrap();
    assert_eq!(
        params.add(width.clone(), Some(text("wide"))).unwrap_err(),
        TypeError::Param(ParamError::WrongKind {
            parameter: "Width".to_owned(),
            expected: Kind::Length,
            found: Kind::Text,
        })
    );
    assert!(params.is_empty());
    params.add(width.clone(), Some(mm(900))).unwrap();
    assert_eq!(
        params.add(width, None).unwrap_err(),
        TypeError::DuplicateParameter(id(WIDTH))
    );
    assert_eq!(params.len(), 1);
    assert_eq!(params.default_of(&id(WIDTH)), Some(&mm(900)));
    assert_eq!(
        params.definition(&id(WIDTH)).map(Definition::name),
        Some("Width")
    );
}
