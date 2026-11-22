//! Reading a parameter of an instance: its own value, then its type's, then the definition's
//! default, by the one function `resolve`; and a type's value read by every instance of it.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{FINISH, HEIGHT, MARK, SILL, WIDTH, id, mm, parameters, text, types};
use construct_types::{Instance, Source, TypeError, resolve};

/// An instance parameter: the instance's own value, else the default, else nothing.
#[test]
fn instance_parameter_reads_instance_then_default() {
    let params = parameters();
    let types = types(&params);
    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);

    let read = resolve(&params, ty, &door, &id(SILL)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(0)), Source::Default));

    door.set(&params, ty, &id(SILL), mm(150)).unwrap();
    let read = resolve(&params, ty, &door, &id(SILL)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(150)), Source::Instance));

    assert_eq!(door.clear(&id(SILL)), Some(mm(150)));
    let read = resolve(&params, ty, &door, &id(SILL)).unwrap();
    assert_eq!(read.from, Source::Default);

    let read = resolve(&params, ty, &door, &id(MARK)).unwrap();
    assert_eq!((read.value, read.from), (None, Source::Unset));
    door.set(&params, ty, &id(MARK), text("D01")).unwrap();
    let read = resolve(&params, ty, &door, &id(MARK)).unwrap();
    assert_eq!(
        (read.value, read.from),
        (Some(&text("D01")), Source::Instance)
    );
}

/// A type parameter: the type's value, else the default, else nothing.
#[test]
fn type_parameter_reads_type_then_default() {
    let params = parameters();
    let mut types = types(&params);
    let door = Instance::new(types.get(&id("T-820")).unwrap());

    let ty = types.get(&id("T-820")).unwrap();
    let read = resolve(&params, ty, &door, &id(WIDTH)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(820)), Source::Type));
    let read = resolve(&params, ty, &door, &id(HEIGHT)).unwrap();
    assert_eq!((read.value, read.from), (None, Source::Unset));

    let ty = types.get_mut(&id("T-820")).unwrap();
    assert_eq!(ty.clear(&id(WIDTH)), Some(mm(820)));
    let ty = types.get(&id("T-820")).unwrap();
    let read = resolve(&params, ty, &door, &id(WIDTH)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(900)), Source::Default));
}

/// The order is instance, type, default, whatever the parameter: each source is read only when
/// the ones before it hold nothing.
#[test]
fn order_is_instance_then_type_then_default() {
    let params = parameters();
    let types = types(&params);
    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);
    door.set(&params, ty, &id(SILL), mm(600)).unwrap();
    let froms: Vec<_> = [SILL, WIDTH, MARK]
        .iter()
        .map(|pid| resolve(&params, ty, &door, &id(pid)).unwrap().from)
        .collect();
    assert_eq!(froms, [Source::Instance, Source::Type, Source::Unset]);
    let second = types.get(&id("T-900")).unwrap();
    let other = Instance::new(second);
    let read = resolve(&params, second, &other, &id(WIDTH)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(900)), Source::Default));
}

/// Changing a type's value changes what every instance of it reads, and no instance holds a copy.
#[test]
fn changing_the_type_changes_every_instance() {
    let params = parameters();
    let mut types = types(&params);
    let ty = types.get(&id("T-820")).unwrap();
    let mut doors: Vec<Instance> = (0..2000).map(|_| Instance::new(ty)).collect();
    for (n, door) in doors.iter_mut().enumerate() {
        door.set(&params, ty, &id(MARK), text(&format!("D{n:04}")))
            .unwrap();
    }
    let before: Vec<_> = doors.iter().map(Instance::to_props).collect();

    types
        .get_mut(&id("T-820"))
        .unwrap()
        .set(&params, &id(WIDTH), mm(1020))
        .unwrap();

    let ty = types.get(&id("T-820")).unwrap();
    for door in &doors {
        let read = resolve(&params, ty, door, &id(WIDTH)).unwrap();
        assert_eq!((read.value, read.from), (Some(&mm(1020)), Source::Type));
        assert!(door.values().get(&id(WIDTH)).is_none());
    }
    let after: Vec<_> = doors.iter().map(Instance::to_props).collect();
    assert_eq!(before, after, "no instance record changed");
    assert!(after.iter().all(|props| !props.contains_key(WIDTH)));
}

/// A parameter the type does not have, one nobody defined, and an instance read against a type
/// that is not its own are each refused.
#[test]
fn resolve_refusals() {
    let params = parameters();
    let types = types(&params);
    let second = types.get(&id("T-900")).unwrap();
    let door = Instance::new(second);
    assert_eq!(
        resolve(&params, second, &door, &id(FINISH)).unwrap_err(),
        TypeError::NotOnType {
            type_name: "Single 900".to_owned(),
            parameter: "Finish".to_owned(),
        }
    );
    assert_eq!(
        resolve(&params, second, &door, &id("P-nobody")).unwrap_err(),
        TypeError::UnknownParameter(id("P-nobody"))
    );
    let first = types.get(&id("T-820")).unwrap();
    assert_eq!(
        resolve(&params, first, &door, &id(WIDTH)).unwrap_err(),
        TypeError::NotItsType {
            instance_type: id("T-900"),
            given: id("T-820"),
        }
    );
}
