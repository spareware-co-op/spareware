//! Schedules: rows for a small model with exact numbers; a row appearing, changing and leaving as
//! elements are added, edited and deleted; each filter comparison; sorting by number and by text;
//! each refusal; a round trip through the stored form; the name rule.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryId, CategoryTable, DOORS, FLOORS, ROOMS, VIEWPORTS, WALLS};
use construct_params::{Area, Choice, Definition, Holder, Kind, Length, ProjectParams, Value};
use construct_phases::{DesignOptions, ElementStaging, PhaseList};
use construct_schedules::{
    Basis, Comparison, Field, Filter, Reading, Schedule, ScheduleError, Schedules, Sort, Subject,
    columns, rows, text_order,
};
use kit_core::Id;
use std::cmp::Ordering;
use std::collections::BTreeMap;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn staging() -> ElementStaging {
    let phases = PhaseList::new().add(i("phase-new"), "New").unwrap();
    ElementStaging::new(&phases, &DesignOptions::new(), i("phase-new"), None, None).unwrap()
}

/// A parameter table with a text "Mark", a whole "Leaves" and a choice "Finish", all on doors.
fn params(categories: &CategoryTable) -> ProjectParams {
    let mark = Definition::new(i("mark"), "Mark", Kind::Text, Holder::Instance).unwrap();
    let leaves = Definition::new(i("leaves"), "Leaves", Kind::Whole, Holder::Instance).unwrap();
    let finish = Definition::choice(
        i("finish"),
        "Finish",
        Holder::Instance,
        vec![Choice::new("oak", "Oak"), Choice::new("paint", "Painted")],
    )
    .unwrap();
    let mut table = ProjectParams::new();
    for definition in [mark, leaves, finish] {
        let id = definition.id().clone();
        table = table.add_project_parameter(definition).unwrap();
        table = table
            .bind(categories, &id, &[DOORS], Holder::Instance, None)
            .unwrap();
    }
    table
}

fn reading(value: &Value, shown: &str) -> Reading {
    Reading {
        value: value.clone(),
        shown: shown.to_owned(),
    }
}

fn door(id: &str, name: &str, mark: &str, leaves: i64, finish: Option<&str>) -> Subject {
    let mut readings = BTreeMap::new();
    readings.insert(
        "parameter:mark".to_owned(),
        reading(&Value::Text(mark.to_owned()), mark),
    );
    readings.insert(
        "parameter:leaves".to_owned(),
        reading(&Value::Whole(leaves), &leaves.to_string()),
    );
    if let Some(finish) = finish {
        readings.insert(
            "parameter:finish".to_owned(),
            reading(&Value::Choice(finish.to_owned()), finish),
        );
    }
    Subject {
        id: i(id),
        category: DOORS,
        name: name.to_owned(),
        type_name: Some("Single".to_owned()),
        readings,
    }
}

fn floor(id: &str, name: &str, mm2: i64, mm: i32) -> Subject {
    let mut readings = BTreeMap::new();
    let area = Value::Area(Area::from_mm2(mm2));
    readings.insert(
        "quantity:Area".to_owned(),
        reading(&area, &format!("{mm2} mm2")),
    );
    let thick = Value::Length(Length::from_mm(mm));
    readings.insert(
        "quantity:Thickness".to_owned(),
        reading(&thick, &format!("{mm} mm")),
    );
    Subject {
        id: i(id),
        category: FLOORS,
        name: name.to_owned(),
        type_name: None,
        readings,
    }
}

fn schedule(name: &str, lists: CategoryId) -> Schedule {
    Schedule::new(i("s1"), name, lists, staging()).unwrap()
}

fn ids(rows: &[construct_schedules::Row]) -> Vec<String> {
    rows.iter().map(|row| row.element.to_string()).collect()
}

fn doors() -> Vec<Subject> {
    vec![
        door("d1", "Front door", "D10", 1, Some("oak")),
        door("d2", "Back door", "D2", 2, Some("paint")),
        door("d3", "Garage door", "d3", 1, None),
        floor("f1", "Slab", 1_000_000, 150),
    ]
}

#[test]
fn rows_of_a_small_model_hold_exact_numbers_and_only_the_category() {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let list = schedule("Doors", DOORS).with_fields(vec![
        Field::Name,
        Field::Type,
        Field::Parameter(i("leaves")),
        Field::Parameter(i("finish")),
    ]);
    list.check(&basis).unwrap();
    let found = rows(&list, &basis, &doors());
    assert_eq!(ids(&found), ["d1", "d2", "d3"]);
    let first = &found[0].cells;
    assert_eq!(first[0].value, Some(Value::Text("Front door".to_owned())));
    assert_eq!(first[1].shown.as_deref(), Some("Single"));
    assert_eq!(first[2].value, Some(Value::Whole(1)));
    assert_eq!(first[3].value, Some(Value::Choice("oak".to_owned())));
    // A door with no finish has an empty cell, not a made-up one.
    assert_eq!(found[2].cells[3].value, None);
    assert_eq!(found[2].cells[3].shown, None);

    let floors = schedule("Floors", FLOORS).with_fields(vec![
        Field::Quantity("Area".to_owned()),
        Field::Quantity("Thickness".to_owned()),
    ]);
    floors.check(&basis).unwrap();
    let found = rows(&floors, &basis, &doors());
    assert_eq!(ids(&found), ["f1"]);
    assert_eq!(
        found[0].cells[0].value,
        Some(Value::Area(Area::from_mm2(1_000_000)))
    );
    assert_eq!(
        found[0].cells[1].value,
        Some(Value::Length(Length::from_mm(150)))
    );

    let heads = columns(&floors, &basis);
    assert_eq!(heads[0].name, "Area");
    assert_eq!(heads[0].kind, Some(Kind::Area));
    assert_eq!(heads[1].kind, Some(Kind::Length));
}

#[test]
fn a_row_appears_changes_and_leaves_as_the_elements_change() {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let list = schedule("Doors", DOORS)
        .with_fields(vec![Field::Parameter(i("leaves"))])
        .with_filters(vec![Filter {
            field: Field::Parameter(i("leaves")),
            comparison: Comparison::Equals,
            value: Value::Whole(1),
        }]);
    let mut model = doors();
    assert_eq!(ids(&rows(&list, &basis, &model)), ["d1", "d3"]);

    // Added: a new door that passes the filter appears.
    model.push(door("d4", "Side door", "D4", 1, None));
    assert_eq!(ids(&rows(&list, &basis, &model)), ["d1", "d3", "d4"]);

    // Edited: the row's cell changes, and a door that stops passing leaves.
    model[0] = door("d1", "Front door", "D10", 1, Some("paint"));
    model[1] = door("d2", "Back door", "D2", 1, Some("paint"));
    let found = rows(&list, &basis, &model);
    assert_eq!(ids(&found), ["d1", "d2", "d3", "d4"]);
    model[0] = door("d1", "Front door", "D10", 3, Some("paint"));
    let found = rows(&list, &basis, &model);
    assert_eq!(ids(&found), ["d2", "d3", "d4"]);
    assert_eq!(found[0].cells[0].value, Some(Value::Whole(1)));

    // Deleted: the row goes.
    model.retain(|subject| subject.id != i("d3"));
    assert_eq!(ids(&rows(&list, &basis, &model)), ["d2", "d4"]);
    // The query is the same function: nothing was kept between the calls.
    assert_eq!(rows(&list, &basis, &model), rows(&list, &basis, &model));
}

fn passing(comparison: Comparison, value: Value, field: Field) -> Vec<String> {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field,
        comparison,
        value,
    }]);
    list.check(&basis).unwrap();
    ids(&rows(&list, &basis, &doors()))
}

#[test]
fn each_comparison_filters() {
    let leaves = || Field::Parameter(i("leaves"));
    assert_eq!(
        passing(Comparison::Equals, Value::Whole(2), leaves()),
        ["d2"]
    );
    assert_eq!(
        passing(Comparison::NotEquals, Value::Whole(2), leaves()),
        ["d1", "d3"]
    );
    assert_eq!(
        passing(Comparison::Less, Value::Whole(2), leaves()),
        ["d1", "d3"]
    );
    assert_eq!(
        passing(Comparison::LessOrEqual, Value::Whole(2), leaves()),
        ["d1", "d2", "d3"]
    );
    assert_eq!(
        passing(Comparison::Greater, Value::Whole(1), leaves()),
        ["d2"]
    );
    assert_eq!(
        passing(Comparison::GreaterOrEqual, Value::Whole(2), leaves()),
        ["d2"]
    );
    let mark = || Field::Parameter(i("mark"));
    assert_eq!(
        passing(Comparison::Contains, Value::Text("d1".into()), mark()),
        ["d1"]
    );
    assert_eq!(
        passing(Comparison::BeginsWith, Value::Text("D".into()), mark()),
        ["d1", "d2", "d3"]
    );
    assert_eq!(
        passing(Comparison::Equals, Value::Text("d3".into()), mark()),
        ["d3"]
    );
    // Text is ordered by the stated rule, case aside: "d10" < "d2" < "d3".
    assert_eq!(
        passing(Comparison::Greater, Value::Text("D10".into()), mark()),
        ["d2", "d3"]
    );
    // A choice takes equality only, and a door with no finish fails the filter.
    let finish = || Field::Parameter(i("finish"));
    assert_eq!(
        passing(Comparison::Equals, Value::Choice("oak".into()), finish()),
        ["d1"]
    );
    assert_eq!(
        passing(Comparison::NotEquals, Value::Choice("oak".into()), finish()),
        ["d2"]
    );
    // Name and type are text fields of every category.
    assert_eq!(
        passing(Comparison::BeginsWith, Value::Text("g".into()), Field::Name),
        ["d3"]
    );
    assert_eq!(
        passing(
            Comparison::Equals,
            Value::Text("Single".into()),
            Field::Type
        ),
        ["d1", "d2", "d3"]
    );
}

#[test]
fn quantities_compare_by_exact_value() {
    let categories = CategoryTable::new();
    let table = ProjectParams::new();
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let model = vec![
        floor("f1", "A", 1_000_000, 150),
        floor("f2", "B", 1_000_001, 150),
        floor("f3", "C", 999_999, 149),
    ];
    let list = schedule("Floors", FLOORS).with_filters(vec![Filter {
        field: Field::Quantity("Area".to_owned()),
        comparison: Comparison::Greater,
        value: Value::Area(Area::from_mm2(1_000_000)),
    }]);
    list.check(&basis).unwrap();
    assert_eq!(ids(&rows(&list, &basis, &model)), ["f2"]);
    let list = schedule("Floors", FLOORS).with_filters(vec![Filter {
        field: Field::Quantity("Thickness".to_owned()),
        comparison: Comparison::LessOrEqual,
        value: Value::Length(Length::from_mm(149)),
    }]);
    assert_eq!(ids(&rows(&list, &basis, &model)), ["f3"]);
}

#[test]
fn sorting_by_number_and_by_text_either_way_and_by_several_keys() {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let sorted = |sorting: Vec<Sort>| {
        let list = schedule("Doors", DOORS).with_sorting(sorting);
        list.check(&basis).unwrap();
        ids(&rows(&list, &basis, &doors()))
    };
    let by = |field: Field, descending: bool| Sort::by(field, descending);
    // Numbers by value: 1, 1, 2 with ties in ID order; descending 2 first.
    assert_eq!(
        sorted(vec![by(Field::Parameter(i("leaves")), false)]),
        ["d1", "d3", "d2"]
    );
    assert_eq!(
        sorted(vec![by(Field::Parameter(i("leaves")), true)]),
        ["d2", "d1", "d3"]
    );
    // Text by the stated rule: case aside, then the exact text.
    assert_eq!(
        sorted(vec![by(Field::Parameter(i("mark")), false)]),
        ["d1", "d2", "d3"]
    );
    assert_eq!(sorted(vec![by(Field::Name, false)]), ["d2", "d1", "d3"]);
    assert_eq!(sorted(vec![by(Field::Name, true)]), ["d3", "d1", "d2"]);
    // Two keys: leaves descending, then name ascending.
    assert_eq!(
        sorted(vec![
            by(Field::Parameter(i("leaves")), true),
            by(Field::Name, false)
        ]),
        ["d2", "d1", "d3"]
    );
    // A row with no value comes last whichever way the key runs.
    assert_eq!(
        sorted(vec![by(Field::Parameter(i("finish")), false)]),
        ["d1", "d2", "d3"]
    );
    assert_eq!(
        sorted(vec![by(Field::Parameter(i("finish")), true)]),
        ["d2", "d1", "d3"]
    );
}

#[test]
fn text_order_is_case_aside_then_exact_and_not_natural() {
    assert_eq!(text_order("a", "B"), Ordering::Less);
    assert_eq!(text_order("B", "a"), Ordering::Greater);
    assert_eq!(text_order("a", "A"), Ordering::Greater);
    assert_eq!(text_order("D10", "D2"), Ordering::Less);
    assert_eq!(text_order("x", "x"), Ordering::Equal);
}

#[test]
fn a_subcategory_is_listed_by_its_parent_category() {
    let (categories, sub) = CategoryTable::new()
        .add_subcategory(WALLS, "Party walls")
        .unwrap();
    let table = ProjectParams::new();
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    let mut wall = floor("w1", "Wall", 1, 1);
    wall.category = sub;
    wall.readings.clear();
    let other = Subject {
        category: WALLS,
        id: i("w2"),
        ..wall.clone()
    };
    let list = schedule("Walls", WALLS);
    assert_eq!(
        ids(&rows(&list, &basis, &[wall.clone(), other])),
        ["w1", "w2"]
    );
    let only = schedule("Party", sub);
    assert_eq!(ids(&rows(&only, &basis, &[wall])), ["w1"]);
}

#[test]
fn every_refusal_is_named_with_its_reason() {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &table,
    };
    // A field the category does not have: a door has no area quantity, a wall no door parameter.
    let list = schedule("Doors", DOORS).with_fields(vec![Field::Quantity("Area".to_owned())]);
    let refused = list.check(&basis).unwrap_err();
    assert!(matches!(refused, ScheduleError::NoSuchField { .. }));
    assert_eq!(refused.to_string(), "Doors have no field quantity:Area.");
    let list = schedule("Walls", WALLS).with_fields(vec![Field::Parameter(i("mark"))]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::NoSuchField { .. }
    ));
    let list = schedule("Doors", DOORS).with_fields(vec![Field::Parameter(i("nothing"))]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::NoSuchField { .. }
    ));
    // A sort key and a filter field the category lacks.
    let list = schedule("Doors", DOORS).with_sorting(vec![Sort {
        field: Field::Quantity("Volume".to_owned()),
        descending: false,
        header: false,
        footer: false,
    }]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::NoSuchField { .. }
    ));
    // A number compared with text, and text with a number.
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field: Field::Parameter(i("leaves")),
        comparison: Comparison::Equals,
        value: Value::Text("two".to_owned()),
    }]);
    let refused = list.check(&basis).unwrap_err();
    assert!(matches!(refused, ScheduleError::FilterKind { .. }));
    assert_eq!(
        refused.to_string(),
        "Leaves holds a whole number, so a filter on it cannot compare it with text."
    );
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field: Field::Name,
        comparison: Comparison::Equals,
        value: Value::Whole(2),
    }]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::FilterKind { .. }
    ));
    // A comparison that makes no sense for the kind.
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field: Field::Parameter(i("leaves")),
        comparison: Comparison::Contains,
        value: Value::Whole(2),
    }]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::ComparisonNotFor { .. }
    ));
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field: Field::Parameter(i("finish")),
        comparison: Comparison::Less,
        value: Value::Choice("oak".to_owned()),
    }]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::ComparisonNotFor { .. }
    ));
    // A choice the definition does not list.
    let list = schedule("Doors", DOORS).with_filters(vec![Filter {
        field: Field::Parameter(i("finish")),
        comparison: Comparison::Equals,
        value: Value::Choice("gold".to_owned()),
    }]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::BadFilterValue { .. }
    ));
    // An unknown category, a category with nothing to list.
    let list = schedule("Nothing", CategoryId(123_456));
    assert_eq!(
        list.check(&basis).unwrap_err(),
        ScheduleError::UnknownCategory(CategoryId(123_456))
    );
    let list = schedule("Viewports", VIEWPORTS);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::NotListable { .. }
    ));
    // A field or a sort key twice; a name of spaces; keys that are not spelt as they are written.
    let list = schedule("Doors", DOORS).with_fields(vec![Field::Name, Field::Name]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::DuplicateField(_)
    ));
    let list = schedule("Doors", DOORS).with_sorting(vec![
        Sort {
            field: Field::Name,
            descending: false,
            header: false,
            footer: false,
        },
        Sort {
            field: Field::Name,
            descending: true,
            header: false,
            footer: false,
        },
    ]);
    assert!(matches!(
        list.check(&basis).unwrap_err(),
        ScheduleError::DuplicateSort(_)
    ));
    assert_eq!(
        Schedule::new(i("s2"), "  ", DOORS, staging()).unwrap_err(),
        ScheduleError::EmptyName
    );
    for bad in [
        "",
        "Name",
        "parameter:",
        "quantity:",
        "parameter: x",
        "colour",
    ] {
        assert!(
            matches!(Field::parse(bad), Err(ScheduleError::BadField(_))),
            "{bad}"
        );
    }
    assert!(matches!(
        Comparison::parse("like"),
        Err(ScheduleError::UnknownComparison(_))
    ));
    // Rooms have their quantities; a room has no thickness.
    let list = schedule("Rooms", ROOMS).with_fields(vec![
        Field::Quantity("State".to_owned()),
        Field::Quantity("Volume".to_owned()),
    ]);
    list.check(&basis).unwrap();
    let list = schedule("Rooms", ROOMS).with_fields(vec![Field::Quantity("Thickness".to_owned())]);
    assert!(list.check(&basis).is_err());
}

#[test]
fn the_name_rule_is_the_documents_and_ignores_case() {
    let mut all = Schedules::new();
    all.add(schedule("Doors", DOORS)).unwrap();
    let again = Schedule::new(i("s2"), "DOORS", DOORS, staging()).unwrap();
    assert_eq!(
        all.add(again).unwrap_err(),
        ScheduleError::DuplicateName("DOORS".to_owned())
    );
    assert_eq!(
        all.add(schedule("Other", DOORS)).unwrap_err(),
        ScheduleError::DuplicateSchedule(i("s1"))
    );
    let two = Schedule::new(i("s2"), "Windows", DOORS, staging()).unwrap();
    all.add(two.clone()).unwrap();
    // A schedule may change the case of its own name, and may not take another's.
    all.replace(two.clone().renamed("WINDOWS").unwrap())
        .unwrap();
    assert!(all.replace(two.renamed("doors").unwrap()).is_err());
    assert_eq!(all.len(), 2);
    assert_eq!(
        all.remove(&i("s9")).unwrap_err(),
        ScheduleError::UnknownSchedule(i("s9"))
    );
    let names: Vec<&str> = all.ordered().iter().map(|s| s.name()).collect();
    assert_eq!(names, ["Doors", "WINDOWS"]);
}

#[test]
fn a_schedule_round_trips_through_its_record_and_reads_strictly() {
    let list = schedule("Doors", DOORS)
        .with_fields(vec![
            Field::Name,
            Field::Parameter(i("leaves")),
            Field::Quantity("Area".to_owned()),
        ])
        .with_filters(vec![
            Filter {
                field: Field::Parameter(i("leaves")),
                comparison: Comparison::GreaterOrEqual,
                value: Value::Whole(2),
            },
            Filter {
                field: Field::Quantity("Area".to_owned()),
                comparison: Comparison::Less,
                value: Value::Area(Area::from_mm2(5_000_000)),
            },
        ])
        .with_sorting(vec![Sort {
            field: Field::Name,
            descending: true,
            header: false,
            footer: false,
        }]);
    let document = i("doc");
    let record = list.to_record(&document);
    assert_eq!(record.kind, "schedule");
    assert_eq!(record.props["@category"], "7200");
    assert_eq!(
        record.props["@schedule.fields"],
        r#"["name","parameter:leaves","quantity:Area"]"#
    );
    assert_eq!(Schedule::from_record(&record).unwrap(), list);
    // The same schedules in any order write the same bytes.
    let mut all = Schedules::new();
    all.add(list.clone()).unwrap();
    all.add(Schedule::new(i("s0"), "Other", WALLS, staging()).unwrap())
        .unwrap();
    let mut records = all.to_records(&document);
    records.reverse();
    let read = Schedules::from_records(&records).unwrap();
    assert_eq!(read, all);
    assert_eq!(read.to_records(&document), all.to_records(&document));
    // Anything the writer would not give is refused, never repaired.
    let mut spaced = record.clone();
    spaced
        .props
        .insert("@schedule.fields".into(), r#"[ "name" ]"#.into());
    assert!(Schedule::from_record(&spaced).is_err());
    let mut bad = record.clone();
    bad.props.insert("@schedule.lists".into(), "07200".into());
    assert!(Schedule::from_record(&bad).is_err());
    let mut bad = record.clone();
    bad.props.insert(
        "@schedule.sorting".into(),
        r#"[{"field":"name","order":"up"}]"#.into(),
    );
    assert!(Schedule::from_record(&bad).is_err());
    let mut bad = record.clone();
    bad.props.remove("@staging");
    assert!(Schedule::from_record(&bad).is_err());
    let mut bad = record;
    bad.level = 1;
    assert!(Schedule::from_record(&bad).is_err());
}

#[test]
fn a_field_that_has_gone_leaves_the_schedule_readable_and_its_rows_empty() {
    let categories = CategoryTable::new();
    let table = params(&categories);
    let without = ProjectParams::new();
    let list =
        schedule("Doors", DOORS).with_fields(vec![Field::Name, Field::Parameter(i("leaves"))]);
    let there = Basis {
        categories: &categories,
        params: &table,
    };
    let gone = Basis {
        categories: &categories,
        params: &without,
    };
    assert!(rows(&list, &there, &doors())[0].cells[1].value.is_some());
    // The parameter is no longer the category's: the check says so, the rows go on, empty there.
    assert!(list.check(&gone).is_err());
    let found = rows(&list, &gone, &doors());
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].cells[1].value, None);
    assert_eq!(columns(&list, &gone)[1].kind, None);
}
