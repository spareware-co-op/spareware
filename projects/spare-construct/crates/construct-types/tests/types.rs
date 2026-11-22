//! What a project does with its types: duplicate, rename, delete, ask who uses one, and move an
//! instance to another type.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{FINISH, MARK, SILL, WIDTH, id, mm, parameters, text, types};
use construct_categories::{CategoryTable, DOORS, WALLS};
use construct_types::{ElementType, Instance, Source, TypeError, resolve, users};

/// A duplicate has a new ID and name, the same category, parameters and values, and changes
/// to it leave the original alone.
#[test]
fn duplicate_copies_and_then_stands_alone() {
    let params = parameters();
    let mut types = types(&params);
    types
        .duplicate(&id("T-820"), id("T-820b"), "Single 820 fire")
        .unwrap();
    let copy = types.get(&id("T-820b")).unwrap();
    let original = types.get(&id("T-820")).unwrap();
    assert_eq!(copy.name(), "Single 820 fire");
    assert_eq!(copy.category(), original.category());
    assert_eq!(
        copy.params().collect::<Vec<_>>(),
        original.params().collect::<Vec<_>>()
    );
    assert_eq!(copy.values(), original.values());

    types
        .get_mut(&id("T-820b"))
        .unwrap()
        .set(&params, &id(WIDTH), mm(860))
        .unwrap();
    assert_eq!(
        types.get(&id("T-820")).unwrap().value(&id(WIDTH)),
        Some(&mm(820))
    );
    assert_eq!(
        types.get(&id("T-820b")).unwrap().value(&id(WIDTH)),
        Some(&mm(860))
    );
}

/// A duplicate needs a name of its own and an ID nobody has.
#[test]
fn duplicate_refusals() {
    let params = parameters();
    let mut types = types(&params);
    let before = types.clone();
    assert_eq!(
        types
            .duplicate(&id("T-820"), id("T-new"), "single 900")
            .unwrap_err(),
        TypeError::DuplicateName {
            category: DOORS,
            name: "single 900".to_owned(),
        }
    );
    assert_eq!(
        types
            .duplicate(&id("T-820"), id("T-900"), "Single 1000")
            .unwrap_err(),
        TypeError::DuplicateType(id("T-900"))
    );
    assert_eq!(
        types
            .duplicate(&id("T-none"), id("T-new"), "Single 1000")
            .unwrap_err(),
        TypeError::UnknownType(id("T-none"))
    );
    assert_eq!(types, before, "a refusal changes nothing");
}

/// Names are unique within a category without regard to case, trimmed, and never empty; the
/// same name may be used in another category.
#[test]
fn names_unique_within_a_category() {
    let params = parameters();
    let mut types = types(&params);
    let table = CategoryTable::new();
    let clash = ElementType::new(id("T-x"), "  SINGLE 820 ", DOORS, &table).unwrap();
    assert_eq!(clash.name(), "SINGLE 820");
    assert_eq!(
        types.add(clash).unwrap_err(),
        TypeError::DuplicateName {
            category: DOORS,
            name: "SINGLE 820".to_owned(),
        }
    );
    let wall = ElementType::new(id("T-x"), "Single 820", WALLS, &table).unwrap();
    types.add(wall).unwrap();
    assert_eq!(types.in_category(WALLS).len(), 2);
    assert_eq!(
        types.named(DOORS, "single 820").map(ElementType::id),
        Some(&id("T-820"))
    );

    assert_eq!(
        ElementType::new(id("T-y"), "   ", DOORS, &table).unwrap_err(),
        TypeError::EmptyName
    );
    let unknown = construct_categories::CategoryId(999_999);
    assert_eq!(
        ElementType::new(id("T-y"), "Odd", unknown, &table).unwrap_err(),
        TypeError::UnknownCategory(unknown)
    );
    let again = ElementType::new(id("T-820"), "Another", DOORS, &table).unwrap();
    assert_eq!(
        types.add(again).unwrap_err(),
        TypeError::DuplicateType(id("T-820"))
    );
}

/// Renaming keeps the ID; a type may change the case of its own name, not take another's.
#[test]
fn rename() {
    let params = parameters();
    let mut types = types(&params);
    types.rename(&id("T-820"), "single 820").unwrap();
    assert_eq!(types.get(&id("T-820")).unwrap().name(), "single 820");
    types.rename(&id("T-820"), " Leaf 820 ").unwrap();
    assert_eq!(types.get(&id("T-820")).unwrap().name(), "Leaf 820");
    assert_eq!(
        types.rename(&id("T-820"), "SINGLE 900").unwrap_err(),
        TypeError::DuplicateName {
            category: DOORS,
            name: "SINGLE 900".to_owned(),
        }
    );
    assert_eq!(
        types.rename(&id("T-820"), "").unwrap_err(),
        TypeError::EmptyName
    );
    assert_eq!(
        types.rename(&id("T-none"), "Any").unwrap_err(),
        TypeError::UnknownType(id("T-none"))
    );
    assert_eq!(types.get(&id("T-820")).unwrap().name(), "Leaf 820");
}

/// A type in use cannot be deleted, and the refusal says how many use it; the caller asks which
/// instances those are by handing them over, since this crate keeps no index.
#[test]
fn delete_only_when_unused() {
    let params = parameters();
    let mut types = types(&params);
    let first = types.get(&id("T-820")).unwrap();
    let second = types.get(&id("T-900")).unwrap();
    let placed = [
        (id("E-3"), Instance::new(first)),
        (id("E-1"), Instance::new(first)),
        (id("E-2"), Instance::new(second)),
        (id("E-4"), Instance::new(first)),
    ];
    assert_eq!(
        users(&id("T-820"), placed.iter().map(|(e, i)| (e, i))),
        [&id("E-1"), &id("E-3"), &id("E-4")]
    );
    assert_eq!(
        users(&id("T-wall"), placed.iter().map(|(e, i)| (e, i))),
        Vec::<&kit_core::Id>::new()
    );

    let before = types.clone();
    assert_eq!(
        types
            .delete(&id("T-820"), placed.iter().map(|(_, i)| i))
            .unwrap_err(),
        TypeError::InUse {
            type_name: "Single 820".to_owned(),
            count: 3,
        }
    );
    assert_eq!(types, before);
    let gone = types
        .delete(&id("T-wall"), placed.iter().map(|(_, i)| i))
        .unwrap();
    assert_eq!(gone.name(), "Partition 90");
    assert!(types.get(&id("T-wall")).is_none());
    assert_eq!(
        types
            .delete(&id("T-wall"), placed.iter().map(|(_, i)| i))
            .unwrap_err(),
        TypeError::UnknownType(id("T-wall"))
    );
}

/// Moving an instance to another type of its category keeps the instance values the new type
/// also has, reports the rest as dropped, and reads the new type's values from then on.
#[test]
fn change_type_keeps_shared_values_and_reports_dropped() {
    let params = parameters();
    let types = types(&params);
    let first = types.get(&id("T-820")).unwrap();
    let second = types.get(&id("T-900")).unwrap();
    let mut door = Instance::new(first);
    door.set(&params, first, &id(SILL), mm(300)).unwrap();
    door.set(&params, first, &id(MARK), text("D07")).unwrap();
    door.set(&params, first, &id(FINISH), text("oiled"))
        .unwrap();

    let dropped = door.change_type(first, second).unwrap();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped.get(&id(FINISH)), Some(&text("oiled")));
    assert_eq!(door.type_id(), &id("T-900"));
    assert_eq!(door.values().get(&id(SILL)), Some(&mm(300)));
    assert_eq!(door.values().get(&id(MARK)), Some(&text("D07")));
    let read = resolve(&params, second, &door, &id(WIDTH)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(900)), Source::Default));

    let dropped = door.change_type(second, first).unwrap();
    assert!(dropped.is_empty());
    let read = resolve(&params, first, &door, &id(WIDTH)).unwrap();
    assert_eq!((read.value, read.from), (Some(&mm(820)), Source::Type));
}

/// Another category is refused, and so is a `from` that is not the instance's type; neither
/// changes the instance.
#[test]
fn change_type_refusals() {
    let params = parameters();
    let types = types(&params);
    let first = types.get(&id("T-820")).unwrap();
    let second = types.get(&id("T-900")).unwrap();
    let wall = types.get(&id("T-wall")).unwrap();
    let mut door = Instance::new(first);
    door.set(&params, first, &id(MARK), text("D07")).unwrap();
    let before = door.clone();
    assert_eq!(
        door.change_type(first, wall).unwrap_err(),
        TypeError::OtherCategory {
            from: DOORS,
            to: WALLS,
        }
    );
    assert_eq!(
        door.change_type(second, first).unwrap_err(),
        TypeError::NotItsType {
            instance_type: id("T-820"),
            given: id("T-900"),
        }
    );
    assert_eq!(door, before);
}
