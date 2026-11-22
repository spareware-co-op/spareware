//! What the page shows to manage categories, parameters and types: the kinds and categories from
//! the data crates, the project's parameters with where they are bound, and the types with the
//! values they hold in units.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, DOORS, WALLS};
use construct_params::{Definition, Holder, Kind, Length, ProjectParams, Value};
use construct_regen::parameters::Context;
use construct_types::{ElementType, Types};
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn context() -> Context {
    let categories = CategoryTable::new();
    let table = ProjectParams::new()
        .add_project_parameter(
            Definition::new(id("P-width"), "Width", Kind::Length, Holder::Type).unwrap(),
        )
        .unwrap()
        .add_project_parameter(
            Definition::new(id("P-mark"), "mark", Kind::Text, Holder::Instance).unwrap(),
        )
        .unwrap()
        .add_project_parameter(
            Definition::new(id("P-spare"), "Another", Kind::Whole, Holder::Instance).unwrap(),
        )
        .unwrap()
        .bind(
            &categories,
            &id("P-width"),
            &[DOORS, WALLS],
            Holder::Type,
            Some(Value::Length(Length::from_mm(900))),
        )
        .unwrap()
        .bind(&categories, &id("P-mark"), &[DOORS], Holder::Instance, None)
        .unwrap();
    Context::new(table).unwrap()
}

#[test]
fn the_catalog_lists_every_kind_and_category_from_the_data_crates() {
    let catalog = context().catalog();
    assert_eq!(catalog.kinds.len(), Kind::ALL.len());
    assert_eq!(catalog.kinds[0].tag, "length");
    assert_eq!(catalog.kinds[0].label, "a length");
    assert_eq!(catalog.categories.len(), CategoryTable::new().all().len());
    let doors = catalog.categories.iter().find(|c| c.id == DOORS.0).unwrap();
    assert_eq!(doors.name, "Doors");
    assert_eq!(doors.parent, None);
    // A subcategory says which top-level category it is under, so a page can leave it out of a
    // list of what a parameter binds to.
    assert!(catalog.categories.iter().any(|c| c.parent.is_some()));
}

#[test]
fn the_table_lists_every_parameter_bound_or_not_with_its_categories_and_default() {
    let view = context().table_view().unwrap();
    let names: Vec<&str> = view.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Another", "mark", "Width"]);
    let width = &view.parameters[2];
    assert_eq!(width.kind, "length");
    assert_eq!(width.kind_label, "a length");
    assert_eq!(width.holder, "type");
    assert_eq!(width.default.as_deref(), Some("900 mm"));
    let bound: Vec<&str> = width.categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(bound, ["Walls", "Doors"]);
    let spare = &view.parameters[0];
    assert_eq!(spare.categories.len(), 0, "defined and bound nowhere");
    assert_eq!(spare.holder, "instance");
    assert_eq!(spare.default, None);
}

#[test]
fn a_type_shows_the_values_it_holds_in_units_and_where_each_comes_from() {
    let context = context();
    let mut ty = ElementType::new(id("T-1"), "Single", DOORS, &context.categories).unwrap();
    for applicable in context.table.parameters_of(&context.categories, DOORS) {
        ty.add_parameter(&context.params, applicable.definition.id())
            .unwrap();
    }
    let mut types = Types::new();
    types.add(ty.clone()).unwrap();
    ty.set(
        &context.params,
        &id("P-width"),
        Value::Length(Length::from_mm(2400)),
    )
    .unwrap();
    let mut held = Types::new();
    held.add(ty).unwrap();
    let counts = BTreeMap::from([(id("T-1"), 3)]);

    let unset = context
        .types_view(Some(DOORS), &types, &counts)
        .unwrap()
        .types;
    let width = unset[0]
        .parameters
        .iter()
        .find(|p| p.name == "Width")
        .unwrap();
    assert_eq!(
        (width.from, width.shown.as_deref()),
        ("default", Some("900 mm"))
    );
    let mark = unset[0]
        .parameters
        .iter()
        .find(|p| p.name == "mark")
        .unwrap();
    assert_eq!(
        (mark.from, mark.holder, mark.shown.as_deref()),
        ("instance", "instance", None)
    );
    assert_eq!(unset[0].elements, 3);

    let view = context.types_view(Some(DOORS), &held, &counts).unwrap();
    assert_eq!(view.category.unwrap().name, "Doors");
    let width = view.types[0]
        .parameters
        .iter()
        .find(|p| p.name == "Width")
        .unwrap();
    assert_eq!(
        (width.from, width.value.as_deref(), width.shown.as_deref()),
        ("type", Some("length:2400"), Some("2400 mm"))
    );
    // Another category has none of these types.
    let walls = context.types_view(Some(WALLS), &held, &counts).unwrap();
    assert_eq!(walls.types.len(), 0);
}
