//! A wall instance's own data: its location line and whether it is flipped, as instance
//! parameters through construct-types; and changing it to another wall type keeps its location
//! line on the same reference line.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{id, mm, mms, wall_parameters};
use construct_categories::{CategoryTable, WALLS};
use construct_params::Value;
use construct_types::{ElementType, Instance, Parameters, TypeError, Types};
use construct_walls::{
    FLIPPED, LOCATION_LINE, LocationLine, WallError, add_parameters, assembly_of,
    default_wall_types, location_line_value, offsets_of, placement,
};

/// The wall parameters and the three default wall types, in their order.
fn fixture() -> (Parameters, Vec<ElementType>) {
    let params = wall_parameters();
    let types = default_wall_types(&params, &CategoryTable::new()).unwrap();
    (params, types)
}

#[test]
fn parameters_defined_once() {
    let mut params = wall_parameters();
    let line = params.definition(&id(LOCATION_LINE)).unwrap();
    assert_eq!(line.choices().len(), 6);
    let keys: Vec<&str> = line.choices().iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "wall-centre",
            "core-centre",
            "exterior-finish-face",
            "interior-finish-face",
            "exterior-core-face",
            "interior-core-face"
        ]
    );
    assert_eq!(
        params.default_of(&id(LOCATION_LINE)),
        Some(&Value::Choice("wall-centre".to_owned()))
    );
    assert_eq!(params.default_of(&id(FLIPPED)), Some(&Value::YesNo(false)));
    assert_eq!(
        add_parameters(&mut params),
        Err(WallError::Type(TypeError::DuplicateParameter(id(
            LOCATION_LINE
        ))))
    );
    for line in LocationLine::ALL {
        assert_eq!(LocationLine::from_key(line.key()), Some(line));
    }
    assert_eq!(LocationLine::from_key("centre"), None);
}

/// A wall type lists both parameters, and an instance with no values of its own reads the
/// defaults: the wall centreline, not flipped.
#[test]
fn defaults_read_through_the_type() {
    let (params, types) = fixture();
    for ty in &types {
        assert_eq!(ty.category(), WALLS);
        assert!(ty.has(&id(LOCATION_LINE)) && ty.has(&id(FLIPPED)));
        let wall = Instance::new(ty);
        assert_eq!(
            placement(&params, ty, &wall).unwrap(),
            (LocationLine::WallCentre, false)
        );
        assert_eq!(
            offsets_of(&params, ty, &wall).unwrap(),
            assembly_of(ty)
                .unwrap()
                .offsets(LocationLine::WallCentre, false)
                .unwrap()
        );
    }
}

/// The instance's own choices are what its offsets are laid out by.
#[test]
fn instance_values_choose_the_offsets() {
    let (params, types) = fixture();
    let partition = types.first().unwrap();
    let mut wall = Instance::new(partition);
    wall.set(
        &params,
        partition,
        &id(LOCATION_LINE),
        location_line_value(LocationLine::ExteriorCoreFace),
    )
    .unwrap();
    wall.set(&params, partition, &id(FLIPPED), Value::YesNo(true))
        .unwrap();
    assert_eq!(
        placement(&params, partition, &wall).unwrap(),
        (LocationLine::ExteriorCoreFace, true)
    );
    let offsets = offsets_of(&params, partition, &wall).unwrap();
    assert_eq!(offsets.faces(), mms(&[-13, 0, 90, 103]).as_slice());
    let props = wall.to_props();
    assert_eq!(
        props.get(LOCATION_LINE).map(String::as_str),
        Some("choice:exterior-core-face")
    );
    assert_eq!(props.get(FLIPPED).map(String::as_str), Some("yesno:yes"));
}

#[test]
fn instance_refusals() {
    let (params, types) = fixture();
    let partition = types.first().unwrap();
    let concrete = types.get(1).unwrap();
    let mut wall = Instance::new(partition);
    assert!(matches!(
        wall.set(
            &params,
            partition,
            &id(LOCATION_LINE),
            Value::Choice("middle".to_owned())
        ),
        Err(TypeError::Param(_))
    ));
    assert!(matches!(
        placement(&params, concrete, &wall),
        Err(WallError::Type(TypeError::NotItsType { .. }))
    ));
    assert!(matches!(
        offsets_of(&Parameters::new(), partition, &wall),
        Err(WallError::Type(TypeError::UnknownParameter(_)))
    ));
}

/// Changing a wall to another wall type keeps its location line and its side, so the chosen
/// line stays on the reference line (offset 0) and the new type's faces are laid out round it.
#[test]
fn change_type_keeps_the_location_line_on_the_reference_line() {
    let (params, types) = fixture();
    let partition = types.first().unwrap();
    let veneer = types.get(2).unwrap();
    let cases = [
        (
            LocationLine::InteriorFinishFace,
            &[116, 103, 13, 0][..],
            &[250, 140, 100, 100, 10, 0][..],
        ),
        (
            LocationLine::ExteriorCoreFace,
            &[13, 0, -90, -103][..],
            &[150, 40, 0, 0, -90, -100][..],
        ),
        (
            LocationLine::WallCentre,
            &[58, 45, -45, -58][..],
            &[125, 15, -25, -25, -115, -125][..],
        ),
    ];
    for (line, before, after) in cases {
        let mut wall = Instance::new(partition);
        wall.set(
            &params,
            partition,
            &id(LOCATION_LINE),
            location_line_value(line),
        )
        .unwrap();
        let old = offsets_of(&params, partition, &wall).unwrap();
        assert_eq!(old.faces(), mms(before).as_slice());
        assert_eq!(old.line(line), Some(mm(0)));
        let dropped = wall.change_type(partition, veneer).unwrap();
        assert!(dropped.is_empty());
        assert_eq!(placement(&params, veneer, &wall).unwrap(), (line, false));
        let new = offsets_of(&params, veneer, &wall).unwrap();
        assert_eq!(new.faces(), mms(after).as_slice(), "{line:?}");
        assert_eq!(new.line(line), Some(mm(0)));
    }
}

/// A flipped wall stays flipped on its new type.
#[test]
fn change_type_keeps_flipped() {
    let (params, types) = fixture();
    let concrete = types.get(1).unwrap();
    let veneer = types.get(2).unwrap();
    let mut wall = Instance::new(concrete);
    wall.set(
        &params,
        concrete,
        &id(LOCATION_LINE),
        location_line_value(LocationLine::InteriorCoreFace),
    )
    .unwrap();
    wall.set(&params, concrete, &id(FLIPPED), Value::YesNo(true))
        .unwrap();
    assert_eq!(
        offsets_of(&params, concrete, &wall).unwrap().faces(),
        mms(&[-200, 0]).as_slice()
    );
    wall.change_type(concrete, veneer).unwrap();
    assert_eq!(
        offsets_of(&params, veneer, &wall).unwrap().faces(),
        mms(&[-240, -130, -90, -90, 0, 10]).as_slice()
    );
}

/// The default types sit in one project's types with names unique in the walls category.
#[test]
fn defaults_fit_a_project() {
    let (_, types) = fixture();
    let mut project = Types::new();
    for ty in types {
        project.add(ty).unwrap();
    }
    assert_eq!(project.in_category(WALLS).len(), 3);
}
